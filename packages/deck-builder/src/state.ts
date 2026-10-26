import type { Actor } from "./actor.ts";
// The composite deck view: one payload with everything the deck page needs,
// returned by every mutation so the client never reconciles partial updates.

import type { Database } from "./sqlite.ts";
import { getPower } from "./deck/power.ts";
import { getBrief } from "./deck/brief.ts";
import { checkDeck } from "./deck/checks.ts";
import { type LogCard, type LogEntry, listLog } from "./deck/log.ts";
import { listProposals } from "./deck/proposals.ts";
import { getDeck, requireViewableDeck } from "./deck/service.ts";
import { publicParent } from "./sharing.ts";
import { listDocumentEdits } from "./documents.ts";
import { loadScope } from "./memory.ts";

export function deckState(db: Database, actor: Actor, deckId: number) {
  const memories = loadScope(db, actor, "deck", deckId);
  return {
    ...getDeck(db, actor, deckId),
    brief: getBrief(db, actor, deckId),
    power: getPower(db, actor, deckId),
    checks: checkDeck(db, actor, deckId),
    proposals: listProposals(db, actor, { deckId, status: "open" }),
    document_edits: listDocumentEdits(db, actor, { deckId, status: "pending" }),
    log: listLog(db, actor, { deckId, limit: 40 }),
    // What an agent opening this deck is shown, and whether it all fit.
    memory_budget: { budget: memories.budget, used: memories.used, omitted: memories.omitted },
  };
}

export type DeckState = ReturnType<typeof deckState>;

// ---------- what a viewer sees (spec §18.3, plan §4 PublicDeckState) ----------

/** Who did something, as a viewer may know it: never which agent. */
export type PublicActor = "owner" | "agent";

export interface PublicDecision {
  kind: LogEntry["kind"];
  summary: string;
  /** The owner's own reason for the ruling; empty when none was given. */
  reason: string;
  by: PublicActor;
  at: string;
  cards: Array<{ oracle_id: string; card_name: string; action: LogCard["action"]; quantity: number }>;
}

/** What a published entry says, written by the app from the cards it moved.
 * The stored summary is free text that can quote a proposal or an agent
 * (a brief ruling's "proposed because: …", an agent's edit summary), and
 * proposals are never shown (§18.1); only the owner's own reason is. */
export function publicSummary(e: Pick<LogEntry, "kind" | "cards" | "document_edit_id">, editKind?: string | null): string {
  const names = (action: LogCard["action"]) => e.cards.filter(c => c.action === action).map(c => `${c.quantity > 1 ? `${c.quantity}× ` : ""}${c.card_name}`);
  const [adds, cuts, roles] = [names("add"), names("cut"), names("role")];
  if (!e.cards.length)
    return e.document_edit_id != null
      ? `${e.kind === "reject" ? "Rejected" : "Accepted"} a proposed change to the ${editKind === "power_target" ? "power target" : editKind === "profile" ? "profile" : "brief"}`
      : e.kind === "undo" ? "Undid an earlier change" : "Changed the deck";
  const list = (items: string[]) => items.length > 3 ? `${items.slice(0, 3).join(", ")} and ${items.length - 3} more` : items.join(", ");
  if (e.kind === "reject")
    return `Rejected ${[adds.length ? `adding ${list(adds)}` : "", cuts.length ? `cutting ${list(cuts)}` : "", roles.length ? `moving ${list(roles)}` : ""].filter(Boolean).join("; ")}`;
  const lead = e.kind === "accept" ? "Accepted: " : e.kind === "import" ? "Imported a list: " : e.kind === "undo" ? "Undid an earlier change: " : "";
  return lead + [adds.length ? `added ${list(adds)}` : "", cuts.length ? `cut ${list(cuts)}` : "", roles.length ? `moved ${list(roles)} in the command zone` : ""]
    .filter(Boolean).join("; ").replace(/^./, c => lead ? c : c.toUpperCase());
}

/** A deck as anyone its visibility admits may read it. Built field by field
 * from an allow-list; nothing private is copied and then trimmed. */
export function publicDeckState(db: Database, viewer: Actor | null, slug: string) {
  const row = requireViewableDeck(db, viewer, slug);
  // The owner's read functions, read-only, for the owner's own deck; only the
  // projection below leaves this function.
  const reader: Actor = { user_id: row.user_id, via: "ui", label: "ui" };
  // Priced as the viewer would see any card: their own choice when signed in,
  // else USD/lowest (plan §4, D18); counted with nothing pending.
  const view = getDeck(db, reader, row.id, { pricesFor: viewer, withoutPending: true });
  const brief = getBrief(db, reader, row.id);
  const power = getPower(db, reader, row.id);
  const owner = db.prepare("SELECT handle, display_name FROM users WHERE id = ?").get(row.user_id) as { handle: string; display_name: string };
  const ownAssessment = power.history.find(a => a.author_via === "ui" && a.author_user_id === row.user_id) ?? null;
  const editKind = new Map((db.prepare("SELECT id, kind FROM document_edits WHERE deck_id = ?").all(row.id) as { id: number; kind: string }[]).map(r => [r.id, r.kind]));
  // An undo of a hidden entry repeats it, so it is hidden with it.
  const hidden = new Set((db.prepare("SELECT id FROM log WHERE deck_id = ? AND hidden = 1").all(row.id) as { id: number }[]).map(r => r.id));
  const log = row.publish_decisions
    ? listLog(db, reader, { deckId: row.id, limit: 200 }).filter(e => !e.hidden && !(e.undo_of != null && hidden.has(e.undo_of))).slice(0, 100)
    : null;
  return {
    deck: {
      name: view.deck.name,
      slug: row.slug!,
      owner: { handle: owner.handle, display_name: owner.display_name },
      commander: view.cards.filter(c => c.role === "commander" && !c.maybe).map(c => c.name),
      color_identity: view.deck.color_identity,
      category: view.deck.category,
      visibility: row.visibility,
      parent: publicParent(db, view.deck.upstream),
    },
    // Tags record sourcing and status ("Have" is ownership evidence): the
    // owner's, not the deck's (§18.3). The key stays, always empty.
    cards: view.cards.map(c => ({ ...c, maybe_note: "", tag_ids: [] as number[] })),
    slots: view.slots,
    tags: [] as typeof view.tags,
    brief: { thesis: brief.thesis, constraints_md: brief.constraints_md },
    engines: brief.engines.map(e => ({ name: e.name, description: e.description, pieces: e.pieces.map(p => ({ oracle_id: p.oracle_id, name: p.name, note: p.note, in_deck: p.in_deck })) })),
    // Power findings read the latest assessment, which may be an agent's (§18.3).
    checks: checkDeck(db, reader, row.id).filter(f => !f.key.startsWith("power_")),
    power: {
      target: power.target ? { profile: power.target.profile, game_changers_max: power.target.game_changers_max, notes: power.target.notes, mapping: power.target.mapping } : null,
      latest_owner_assessment: ownAssessment ? {
        profile: ownAssessment.profile, evidence: ownAssessment.evidence, confidence: ownAssessment.confidence,
        notes: ownAssessment.notes, mapping: ownAssessment.mapping, stale: ownAssessment.stale, created_at: ownAssessment.created_at,
      } : null,
    },
    price: {
      total_cents: view.computed.cost.cents,
      currency: view.computed.cost.currency,
      store: view.computed.cost.store,
      unpriced: view.computed.cost.unpriced,
      as_of: view.computed.cost.as_of,
      source: view.computed.cost.source,
    },
    ...(log ? { decisions: log.map((e): PublicDecision => ({
      kind: e.kind, summary: publicSummary(e, e.document_edit_id == null ? null : editKind.get(e.document_edit_id)), reason: e.ruling_reason, by: e.author_via === "ui" ? "owner" : "agent", at: e.ts,
      cards: e.cards.map(c => ({ oracle_id: c.oracle_id, card_name: c.card_name, action: c.action, quantity: c.quantity })),
    })) } : {}),
    computed: view.computed,
  };
}

export type PublicDeckState = ReturnType<typeof publicDeckState>;
