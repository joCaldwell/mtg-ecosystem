import { priceResolver, formatPrice } from "../prices.ts";
import { priceSourceLine } from "./render.ts";
// The agent's tools (spec §10.3). Few, shaped around tasks, each a thin
// adapter: resolve names, call the capability layer, render text. Every rule
// an agent is held to is enforced below this file, in the same functions the
// UI calls — nothing here decides what an agent may do.
//
// A tool is a plain function of (context, arguments) so it can be tested
// without a transport; mcp/http.ts registers the same list with the SDK.

import type { Database } from "../sqlite.ts";
import * as z from "zod/v4";
import { type Actor, requireDeckOwner } from "../actor.ts";
import { recordActivity } from "../activity.ts";
import { type CardRow, lintProse, resolveCard, resolveCards, tryResolveCard } from "../cards.ts";
import { withTransaction } from "../db.ts";
import { ServiceError } from "../errors.ts";
import { getFocus } from "../focus.ts";
import { compareDecks, forkDeck } from "../deck/fork.ts";
import { compareDeckSources, recordOwnership } from "../deck/sourcing.ts";
import { exportDeck, fillFromList } from "../deck/interop.ts";
import { cardHistories, listLog, markReviewed, unreviewedCount } from "../deck/log.ts";
import { type Change, applyChanges } from "../deck/membership.ts";
import { createProposal, getProposal, listProposals, amendProposal, withdrawProposal } from "../deck/proposals.ts";
import {
  createDeck,
  deleteDeck,
  defineSlots,
  fileCards,
  findDeckCard,
  getDeck,
  listDecks,
  requireDeck,
  requireDirectCardEdit,
  requireSlotByName,
  renameDeck,
  shortlistCard,
  unshortlistCard,
} from "../deck/service.ts";
import { PowerTargetSchema, PowerAssessmentSchema, getPower, recordPowerAssessment } from "../deck/power.ts";
import { getProfile, proposeDocumentEdit, getDocumentEdit, listDocumentEdits, proposeDocumentGroup, withdrawDocumentEdit } from "../documents.ts";
import {
  type MemoryView,
  listMemories,
  loadScope,
  memoriesForCards,
  remember,
  reviseMemory,
} from "../memory.ts";
import { SearchError, search } from "../search/index.ts";
import { sharedCounts, slugFrom } from "../sharing.ts";
import { publicDeckState } from "../state.ts";
import { playerStats } from "../stats.ts";
import { PLAYBOOK } from "./playbook.ts";
import {
  DECK_SECTIONS,
  afterWrite,
  cardWithContext,
  logLines,
  memoryLine,
  renderCompare,
  renderDeck,
  renderPower,
  renderPublicDeck,
  renderMemories,
  renderScope,
  renderStats,
  resolutionFailureLine,
  rosterLines,
  rulingLine,
} from "./render.ts";

export interface ToolContext {
  db: Database;
  actor: Actor;
}

export interface ToolResult {
  text: string;
  structuredContent?: Record<string, unknown>;
  /** Set by tools that wrote something, so the app can refresh what is on screen. */
  changed?: boolean;
}

export interface ToolDef {
  name: string;
  title: string;
  description: string;
  input: z.ZodRawShape;
  readOnly: boolean;
  destructive?: boolean;
  /** Includes personal prices in text/structured data, or orients the agent. */
  prices?: boolean;
  run: (ctx: ToolContext, args: any) => string | ToolResult;
}

// ---------- shared argument shapes ----------

const deckRef = z
  .union([z.number().int(), z.string()])
  .describe(
    'A deck: its id (a number from the roster), its exact name, or "focus" for the deck the owner has open in the app.',
  );
const cardRef = z
  .string()
  .describe("A card: its exact name as a tool returned it, or its oracle_id. Never a guess.");

function deckId(db: Database, actor: Actor, ref: number | string, forWrite = false): number {
  const resolve = (id: number) => forWrite ? requireDeckOwner(db, actor, id).id : requireDeck(db, actor, id).id;
  if (typeof ref === "number") return resolve(ref);
  const text = ref.trim();
  if (/^\d+$/.test(text)) return resolve(Number(text));
  if (text.toLowerCase() === "focus") {
    const focus = getFocus(db, actor);
    if (!focus)
      throw new ServiceError(
        `The owner has no deck open in the app, so "focus" names nothing. Use a deck id:\n${rosterLines(listDecks(db, actor))}`,
      );
    return focus.deck_id;
  }
  const named = listDecks(db, actor).filter((d) => d.name.toLowerCase() === text.toLowerCase());
  if (named.length === 1) return named[0].id;
  const slug = slugFrom(text);
  if (slug) {
    const own = db.prepare("SELECT id FROM decks WHERE slug = ? AND user_id = ?").get(slug, actor.user_id) as { id: number } | undefined;
    if (own) return own.id;
    throw new ServiceError(`"${ref}" is a link to a shared deck. Only get_deck, compare_decks (as other) and fork_deck read shared decks; it cannot be changed here.`, 404);
  }
  throw new ServiceError(`There is no deck named "${ref}". The decks are:\n${rosterLines(listDecks(db, actor))}`, 404);
}

/** A deck by any reference, or another user's shared deck by its slug or link (§18.7). */
function deckOrShared(db: Database, actor: Actor, ref: number | string): number | { slug: string } {
  try {
    return deckId(db, actor, ref);
  } catch (e) {
    const slug = typeof ref === "string" ? slugFrom(ref) : null;
    if (slug && e instanceof ServiceError && e.status === 404) return { slug };
    throw e;
  }
}

const sharedRef = z
  .union([z.number().int(), z.string()])
  .describe(
    'A deck: its id, its exact name, "focus", or a shared deck\'s link (/d/<slug>) or slug.',
  );

const done = (text: string): ToolResult => ({ text, changed: true });

/** §18.7: how many decks anyone with a link can read. Only the owner changes it, in the app. */
function sharingLine(counts: { public: number; unlisted: number }): string {
  return `\nShared: ${counts.public} public (listed on the owner's profile), ${counts.unlisted} unlisted (anyone with the link); the rest are private. ` +
    "Viewers see the list, brief, checks and power, never memories or proposals. Only the owner changes who can see a deck.";
}

const documentChange = z.object({
  kind: z.enum(["profile", "thesis", "constraints", "engine_set", "engine_remove", "power_target"]),
  text: z.string().optional(), engine_name: z.string().optional(), description: z.string().optional(),
  pieces: z.array(z.object({ card: cardRef, note: z.string().optional() })).optional(),
  target: PowerTargetSchema.nullable().optional(), reason: z.string().trim().min(1),
});
const proposalItem = z.object({
  action: z.enum(["add", "cut"]), card: cardRef, reason: z.string().trim().min(1),
  slot: z.string().optional(), secondary_slots: z.array(z.string()).optional(),
  to_maybe: z.boolean().optional(), group: z.string().optional(),
});
function documentChanges(db: Database, actor: Actor, changes: z.infer<typeof documentChange>[]) {
  return changes.map(c => ({ kind: c.kind, reason: c.reason, payload: {
    content: c.text, engine_name: c.engine_name, description: c.description, target: c.target,
    pieces: c.pieces?.map(p => ({ oracle_id: resolveCard(db, p.card).oracle_id, note: p.note })),
  } }));
}


function myDecksFor(db: Database, actor: Actor, oracleIds: string[], exceptDeck?: number): Map<string, string[]> {
  const stmt = db.prepare(
    `SELECT d.name FROM deck_cards dc JOIN decks d ON d.id = dc.deck_id
     WHERE dc.oracle_id = ? AND dc.maybe = 0 AND dc.deck_id != ? AND d.user_id = ? ORDER BY d.name`,
  );
  const out = new Map<string, string[]>();
  for (const id of new Set(oracleIds)) {
    const names = (stmt.all(id, exceptDeck ?? -1, actor.user_id) as unknown as { name: string }[]).map((r) => r.name);
    if (names.length) out.set(id, names);
  }
  return out;
}

// The context the app attaches to cards wherever they appear (spec §7.2,
// §8.3): linked memories always, prior rulings and membership when a deck is
// named, other decks only when the call asked (§4.2).
function contextFor(
  db: Database, actor: Actor,
  cards: CardRow[],
  deck: number | undefined,
  includeMyDecks: boolean,
) {
  const ids = cards.map((c) => c.oracle_id);
  const memories = memoriesForCards(db, actor, ids, deck);
  const rulings = deck === undefined ? new Map() : cardHistories(db, actor, deck, ids);
  const mine = includeMyDecks ? myDecksFor(db, actor, ids, deck) : new Map<string, string[]>();
  return (card: CardRow) => {
    const row = deck === undefined ? undefined : findDeckCard(db, actor, deck, card.oracle_id);
    return {
      memories: memories.get(card.oracle_id),
      rulings: rulings.get(card.oracle_id),
      myDecks: mine.get(card.oracle_id),
      membership: row ? (row.maybe ? "on this deck's maybe list" : "already in this deck") : undefined,
    };
  };
}

// ---------- the tools ----------

export const TOOLS: ToolDef[] = [
  {
    name: "start_session",
    prices: true,
    title: "Start a session",
    readOnly: true,
    description:
      "Call this first, once per conversation. Returns the playbook (the rules for working with this app), " +
      "the owner's profile, your global memories, the deck roster with ids, the deck the owner has open " +
      "(the focus), and how many of the owner's rulings you have not read yet.",
    input: {},
    run: ({ db, actor }) => {
      const identity = db.prepare("SELECT handle, display_name FROM users WHERE id = ?").get(actor.user_id) as { handle: string; display_name: string };
      const profile = getProfile(db, actor);
      const focus = getFocus(db, actor);
      const unreviewed = unreviewedCount(db, actor);
      return [
        PLAYBOOK,
        `# Identity\n${identity.handle} — ${identity.display_name}`,
        `# The owner's profile\n${profile.body_md || "(empty — the owner has not written one. If you learn durable things about them, propose a profile with propose_profile_edit.)"}` +
          (profile.over_cap
            ? `\n(The profile is ${profile.word_count} words, over its soft cap. Suggest tightening it.)`
            : ""),
        `# Your global memories\n${renderScope(loadScope(db, actor, "global"), "global")}`,
        `# Decks\n${rosterLines(listDecks(db, actor))}${sharingLine(sharedCounts(db, actor))}`,
        `# Focus\n${
          focus
            ? `The owner has deck ${focus.deck_id} "${focus.deck_name}" open` +
              (focus.selected.length ? `, with these cards selected: ${focus.selected.join(", ")}` : "") +
              ` (reported ${focus.age_minutes} minute(s) ago).`
            : "The owner has no deck open in the app right now."
        }`,
        `# Rulings you have not read\n${
          unreviewed
            ? `${unreviewed}. Read them with list_rulings (unreviewed_only: true) before proposing anything, then mark_rulings_reviewed.`
            : "None."
        }`,
      ].join("\n\n");
    },
  },
  {
    name: "player_stats",
    title: "Player statistics",
    readOnly: true,
    description:
      "What the owner builds, computed only across their Paper decks (category paper): colours and identities, commanders and their " +
      "creature types, average curve and land counts, slot habits, and the cards they reuse across decks. " +
      "Use this for questions about the owner's tastes or what to build next. These are facts the app " +
      "computes; never store them as memories.",
    input: {},
    run: ({ db, actor }) => renderStats(playerStats(db, actor)),
  },
  {
    name: "get_deck",
    prices: true,
    title: "Read a deck",
    readOnly: true,
    description:
      "One deck in full: its brief, the list grouped by slot with oracle text, the maybe list, the app's " +
      "computed counts, check results, the memories for this deck, what is awaiting the owner's ruling, " +
      "and recent rulings. Read a deck before discussing or changing it. To re-read cheaply after a " +
      "change, pass only the sections you need, or card_text: false for names without oracle text. " +
      "Given another user's shared deck link or slug, returns what its viewers see, read-only.",
    input: {
      deck: sharedRef,
      sections: z
        .array(z.enum(DECK_SECTIONS))
        .optional()
        .describe("Which parts to return. Omit for everything."),
      card_text: z
        .boolean()
        .optional()
        .describe("Include each card's oracle text (default true)."),
    },
    run: ({ db, actor }, a) => {
      const ref = deckOrShared(db, actor, a.deck);
      return typeof ref === "number"
        ? renderDeck(db, actor, ref, a.sections ?? DECK_SECTIONS, a.card_text ?? true)
        : renderPublicDeck(publicDeckState(db, actor, ref.slug), a.sections ?? DECK_SECTIONS, a.card_text ?? true);
    },
  },
  {
    name: "propose_power_target",
    prices: true,
    title: "Set or propose a power target",
    readOnly: false,
    description:
      "Set a deck's desired power profile: intent, usual win range, consistency, resilience, interaction, " +
      "win conditions and allowed play patterns, plus a Game Changer limit. Agents always propose targets " +
      "for the owner to accept. Legacy is read-only. Unknown fields may be omitted. " +
      "This changes the target, never the current assessment or cards. Give a reason grounded in the owner's preferences. " +
      "Use get_deck sections: [power] for field anchors and the app's explained bracket mapping.",
    input: {
      deck: deckRef,
      expected_revision: z.number().int().nonnegative().optional(),
      target: PowerTargetSchema.nullable().describe("The desired profile and limits, or null to clear the target. Turn ranges are inclusive; occasional early wins are outliers, not frequent lines."),
      reason: z.string().trim().min(1).describe("Why this target fits what the owner wants; card references as [[Card Name]]."),
    },
    run: ({ db, actor }, a) => {
      const id = deckId(db, actor, a.deck, true);
      const r = proposeDocumentEdit(db, actor, id, "power_target", { target: a.target }, a.reason, { expected_revision: a.expected_revision });
      return done(`${r.applied ? "Set the power target." : `Proposed power target edit #${r.edit_id}; awaiting the owner.`}\n${renderPower(getPower(db, actor, id))}\n${afterWrite(db, actor, id)}`);
    },
  },
  {
    name: "assess_deck_power",
    prices: true,
    title: "Record a deck power assessment",
    readOnly: false,
    description:
      "Record an attributed assessment of the deck's current power after reading its exact list and target. " +
      "Include the revision you read, your evidence (list, goldfish or games), confidence and reasoning. " +
      "Do not invent playtest results: use list and tentative for an untested estimate. The app computes " +
      "Game Changers and maps the profile to a suggested community bracket. Card edits make the assessment stale. " +
      "An agent cannot overwrite the owner's assessment of the same revision. This never changes the target or cards.",
    input: { deck: deckRef, assessment: PowerAssessmentSchema.describe("Gameplay judgments, not card counts. Unknown profile fields may be omitted. Explain evidence in notes, with cards as [[Card Name]].") },
    run: ({ db, actor }, a) => {
      const id = deckId(db, actor, a.deck, true);
      const assessmentId = recordPowerAssessment(db, actor, id, a.assessment);
      return done(`Recorded power assessment #${assessmentId}.\n${renderPower(getPower(db, actor, id))}\n${afterWrite(db, actor, id)}`);
    },
  },
  {
    name: "compare_deck_sources",
    prices: true,
    title: "Compare a deck against its sources",
    readOnly: false,
    description: "Compare a target with multiple source decks in one operation. Returns actual shared cards and expensive remaining cards, IDs, exact names, list quantities, source decks, persistent ownership evidence, availability, estimated prices and timestamps, and app-computed totals as structured data. Membership never proves ownership or physical quantity; unknown ownership is not a shopping list. Optionally add all source tags in bulk, preserving slots and unrelated tags; revision guards are required when tagging. Filtering affects cards/result totals, not shared_cards, remaining_cards or tagging.",
    input: {
      deck: deckRef, sources: z.array(deckRef).min(1),
      sourcing: z.enum(["shared", "not_in_sources"]).optional(),
      ownership: z.enum(["owned", "unowned", "unknown"]).optional(),
      not_marked_owned: z.boolean().optional(),
      availability: z.enum(["available", "insufficient", "unknown"]).optional(),
      sort: z.enum(["name", "price_desc", "price_asc", "ownership", "sourcing"]).optional(),
      apply_source_tags: z.boolean().optional(),
      expected_revision: z.number().int().nonnegative().optional(),
      expected_source_revisions: z.record(z.string(), z.number().int().nonnegative()).optional(),
    },
    run: ({ db, actor }, a) => {
      const result = compareDeckSources(db, actor, deckId(db, actor, a.deck, !!a.apply_source_tags), a.sources.map((s: string | number) => deckId(db, actor, s, !!a.apply_source_tags)), a);
      const s = result.summary;
      const price = (cents: number | null) => cents == null ? "price unknown" : `${formatPrice(cents, result.price.currency)} est.`;
      return {
        changed: !!a.apply_source_tags, structuredContent: result,
        text: `${result.target.name}: ${s.target.cards} distinct cards / ${s.target.copies} copies. ${s.shared.cards} shared; ${s.remaining.cards} absent from sources and not explicitly owned (${price(s.remaining.cost.cents)}, ${s.remaining.cost.unpriced} unpriced). ${s.unknown_ownership.cards} have unknown ownership. Physical quantities are never summed across decks.${a.apply_source_tags ? ` Source tags applied to ${s.tagged} card${s.tagged === 1 ? "" : "s"}.` : ""}\n` +
          `Shared: ${result.shared_cards.map(c => `${c.name} [${c.source_decks.map(d => d.name).join("; ")}]`).join(", ") || "none"}\n` +
          `Most expensive remaining (first 10; full data attached): ${result.remaining_cards.slice(0, 10).map(c => `${c.name} — ${price(c.price.cents)}, ${c.ownership.status}`).join("; ") || "none"}\nUnknown ownership and source membership do not establish availability or a purchase requirement.`,
      };
    },
  },
  {
    name: "record_ownership",
    prices: true,
    title: "Record explicit ownership evidence",
    readOnly: false,
    description: "Record explicit owner-provided ownership and optional physical/available quantities, with evidence. Persists independently of deck membership; never infer it from list appearances or keep preferences. Latest statement is current and all history remains. Requires an editable context deck, supports revision/evidence guards, and changes no cards, slots, brief or power target.",
    input: {
      deck: deckRef, expected_revision: z.number().int().nonnegative().optional(),
      cards: z.array(z.object({ card: cardRef, status: z.enum(["owned", "unowned", "unknown"]),
        quantity: z.number().int().nonnegative().nullable().optional(), available_quantity: z.number().int().nonnegative().nullable().optional(),
        basis: z.string().trim().min(1), expected_evidence_id: z.number().int().positive().nullable().optional(),
      })).min(1),
    },
    run: ({ db, actor }, a) => {
      const id = deckId(db, actor, a.deck, true);
      const cards = resolveCards(db, a.cards.map((c: { card: string }) => c.card));
      recordOwnership(db, actor, id, a.cards.map((c: object, i: number) => ({ ...c, oracle_id: cards[i].oracle_id })), a.expected_revision);
      return done(`Recorded explicit ownership evidence for ${cards.length} card(s).\n${afterWrite(db, actor, id)}`);
    },
  },
  {
    name: "compare_decks",
    title: "Compare two decks",
    readOnly: true,
    description:
      "The cards only in one deck, only in the other, and how many they share. With one deck named, " +
      "compares a fork against the deck it was forked from. Use it to see how a fork has diverged, or how " +
      "two of the owner's decks overlap. other may be another user's shared deck, by link or slug.",
    input: {
      deck: deckRef,
      other: sharedRef.optional().describe("The deck to compare against: one of the owner's, or a shared deck's link. Omit to use the first deck's parent."),
    },
    run: ({ db, actor }, a) =>
      renderCompare(compareDecks(db, actor, deckId(db, actor, a.deck), a.other === undefined ? undefined : deckOrShared(db, actor, a.other))),
  },
  {
    name: "search_cards",
    prices: true,
    title: "Search cards",
    readOnly: true,
    description:
      "Search every Magic card with a subset of Scryfall syntax. Bare words match the name; " +
      't:creature (type), o:"draw a card" (oracle text), fo: (text including reminder text), ' +
      "mv<=2 or cmc>=5, c:rg (colours, at least these), id<=wub (colour identity within), " +
      "pow>=4, tou<3, loy=3, usd<=2 or eur<=2 (cheapest playable printing in that currency, at your chosen store or lowest), is:commander, " +
      "f:commander, banned:commander. Combine with spaces (AND), " +
      "'or', '-' or 'not', and parentheses. With a deck named, results are limited to that deck's colour " +
      "identity, marked if already in it, and carry the owner's prior rulings on each card in that deck; " +
      "slot:ramp, slot:none and tag:name then filter to that deck's own cards. Memories linked to a card " +
      "are always attached. Results come back alphabetically, 20 at a time: make several narrow searches " +
      "rather than one broad one.",
    input: {
      query: z.string().describe('The search, e.g. t:artifact o:"add {C}" mv<=2'),
      deck: deckRef.optional().describe("Search in the context of this deck (recommended when building one)."),
      include_my_decks: z
        .boolean()
        .optional()
        .describe(
          "Also say which of the owner's other decks contain each card. Off by default, because it " +
            "biases recommendations toward cards they already use; turn it on only when that is the question.",
        ),
      ignore_identity: z
        .boolean()
        .optional()
        .describe("With a deck named, do not limit results to its colour identity."),
      limit: z.number().int().min(1).max(50).optional().describe("How many results (default 20, max 50)."),
    },
    run: ({ db, actor }, a) => {
      const deck = a.deck === undefined ? undefined : deckId(db, actor, a.deck);
      const limit = a.limit ?? 20;
      let results: CardRow[];
      try {
        results = search(db, actor, a.query, {
          deckId: deck,
          filterByDeckIdentity: !a.ignore_identity,
          limit: limit + 1,
        }) as unknown as CardRow[];
      } catch (e) {
        if (e instanceof SearchError)
          throw new ServiceError(
            `That search could not be read: ${e.message}. Supported keys: t, o, fo, mv/cmc, c, id, pow, tou, loy, usd, eur, ` +
              `is:commander, f:commander, banned:commander, and with a deck: slot:, tag:. Quote phrases: o:"draw a card".`,
          );
        throw e;
      }
      const more = results.length > limit;
      const shown = results.slice(0, limit);
      if (!shown.length)
        return (
          `No cards match "${a.query}"` +
          (deck !== undefined && !a.ignore_identity
            ? ` within this deck's colour identity. If the card may be outside it, search again with ignore_identity: true.`
            : `. Loosen the query, or check the syntax.`)
        );
      const ctx = contextFor(db, actor, shown, deck, !!a.include_my_decks);
      return [
        `${shown.length} result(s) for "${a.query}"${deck !== undefined ? ` in the context of deck ${deck}` : ""}` +
          (more ? ` — there are more; narrow the query to see them.` : "."),
        ...shown.map((c) => cardWithContext(c, ctx(c))),
      ].join("\n");
    },
  },
  {
    name: "get_cards",
    prices: true,
    title: "Look up cards by name",
    readOnly: true,
    description:
      "Exact-name lookup of up to 25 cards: full oracle text, memories linked to each card, and, with a " +
      "deck named, whether it is in that deck and the owner's prior rulings on it there. This is also how " +
      "to check that names are real before you rely on them: a name that is not a card comes back marked " +
      "NOT A CARD with the closest real names, and the rest of the lookup still succeeds.",
    input: {
      cards: z.array(cardRef).min(1).max(25),
      deck: deckRef.optional(),
      include_my_decks: z
        .boolean()
        .optional()
        .describe("Also say which of the owner's other decks contain each card. Off by default."),
    },
    run: ({ db, actor }, a) => {
      const deck = a.deck === undefined ? undefined : deckId(db, actor, a.deck);
      const resolved = (a.cards as string[]).map((ref) => tryResolveCard(db, ref));
      const found = priceResolver(db, actor).resolve(resolved.filter((r): r is CardRow => !("problem" in r)));
      const priced = new Map(found.map(c => [c.oracle_id, c]));
      const ctx = contextFor(db, actor, found, deck, !!a.include_my_decks);
      return {
        text: resolved.map(r => "problem" in r ? resolutionFailureLine(r) : cardWithContext(priced.get(r.oracle_id)!, ctx(r))).join("\n"),
        structuredContent: { cards: found, failures: resolved.filter(r => "problem" in r) },
      };
    },
  },
  {
    name: "list_rulings",
    title: "List rulings and changes",
    readOnly: true,
    description:
      "The rulings log, newest first: every accept and reject with the owner's reason, plus direct edits, " +
      "imports and undos. Use unreviewed_only: true to read what the owner decided while you were not " +
      "running; after deciding what is worth a memory, call mark_rulings_reviewed.",
    input: {
      deck: deckRef.optional().describe("Only this deck. Omit for every deck and the profile."),
      unreviewed_only: z.boolean().optional(),
      since: z.string().optional().describe("Only entries on or after this date (YYYY-MM-DD)."),
      limit: z.number().int().min(1).max(200).optional().describe("Default 30."),
    },
    run: ({ db, actor }, a) => {
      const entries = listLog(db, actor, {
        deckId: a.deck === undefined ? undefined : deckId(db, actor, a.deck),
        unreviewedOnly: a.unreviewed_only,
        since: a.since,
        limit: a.limit ?? 30,
      });
      const waiting = unreviewedCount(db, actor);
      const unread = entries.filter((e) => e.reviewed_at == null);
      return (
        `${entries.length} entr${entries.length === 1 ? "y" : "ies"}` +
        (a.unreviewed_only ? " awaiting your review" : "") +
        `. ${waiting} unreviewed in total.\n${logLines(entries)}` +
        // The exact follow-up, because the first agent to do this guessed at
        // the arguments.
        (unread.length
          ? `\n\nOnce you have decided what here is worth a memory, call mark_rulings_reviewed with ` +
            (new Set(unread.map(e => e.deck_id)).size === 1
              ? `{"through": ${Math.max(...unread.map((e) => e.id))}}.`
              : `${JSON.stringify({ ids: unread.map(e => e.id) })}.`)
          : "")
      );
    },
  },
  {
    name: "search_memories",
    title: "Search memories",
    readOnly: true,
    description:
      "Find memories beyond those already loaded: by words (full-text, best match first), by scope, by " +
      "deck, or by a linked card. Global memories load in start_session and a deck's load in get_deck; " +
      "use this when one of those said some did not fit, when you need another deck's memories, or " +
      "before writing a memory you suspect already exists.",
    input: {
      query: z.string().optional().describe("Words to look for."),
      scope: z.enum(["global", "deck"]).optional(),
      deck: deckRef.optional(),
      card: cardRef.optional().describe("Only memories linked to this card."),
      include_retired: z
        .boolean()
        .optional()
        .describe("Also return superseded and retired memories (the history)."),
      limit: z.number().int().min(1).max(100).optional().describe("Default 25."),
    },
    run: ({ db, actor }, a) =>
      renderMemories(
        listMemories(db, actor, {
          query: a.query,
          scope: a.scope,
          deckId: a.deck === undefined ? undefined : deckId(db, actor, a.deck),
          cardId: a.card ? resolveCard(db, a.card).oracle_id : undefined,
          status: a.include_retired ? "any" : "active",
          limit: a.limit ?? 25,
        }),
      ),
  },
  {
    name: "export_deck",
    title: "Export a deck as text",
    readOnly: true,
    description:
      "The deck as plain text that Archidekt and most other sites import: one line per card, " +
      '"1 Card Name [Category]", command zone first. The maybe list is not included.',
    input: {
      deck: deckRef,
      categories: z.boolean().optional().describe("Include [Category] from slots (default true)."),
    },
    run: ({ db, actor }, a) => {
      const r = exportDeck(db, actor, deckId(db, actor, a.deck), {
        categories: a.categories,
      });
      return `${r.card_count} card(s) at revision ${r.revision}:\n\n${r.text}`;
    },
  },

  // ---------- writes that need no ruling ----------

  {
    name: "rename_deck",
    prices: true,
    title: "Rename a deck",
    readOnly: false,
    description:
      "Rename a Paper or Theorycrafted deck. The name must be nonempty and unique across your decks. " +
      "Its cards, revision, category and editing permissions stay the same. The change is attributed in Activity.",
    input: { deck: deckRef, name: z.string().describe("The new deck name.") },
    run: ({ db, actor }, a) => {
      const id = deckId(db, actor, a.deck, true);
      renameDeck(db, actor, id, a.name);
      return done(`Renamed deck ${id} to "${requireDeck(db, actor, id).name}".\n${afterWrite(db, actor, id)}`);
    },
  },
  {
    name: "delete_deck",
    title: "Delete a deck",
    readOnly: false,
    destructive: true,
    description:
      "Permanently delete a deck and its card list, slots, tags, brief, proposals, rulings and deck-scoped " +
      "memories. This cannot be undone. Global memories and fork decks remain; forks lose their parent link. " +
      "Use only when the owner asks to delete this deck. The deletion stays attributed in Activity.",
    input: { deck: deckRef },
    run: ({ db, actor }, a) => {
      const id = deckId(db, actor, a.deck, true);
      const name = requireDeck(db, actor, id).name;
      deleteDeck(db, actor, id);
      return done(`Deleted deck ${id} "${name}" and its associated records. This cannot be undone.\n${rosterLines(listDecks(db, actor))}`);
    },
  },

  {
    name: "create_deck",
    prices: true,
    title: "Create a Theorycrafted deck",
    readOnly: false,
    description:
      "Create a Theorycrafted deck, whose cards you can edit directly with edit_cards. Optionally set " +
      "commanders or paste a whole list. A supplied thesis becomes a proposed brief edit for the owner’s approval. " +
      "Only the owner changes lifecycle. If any card name does not resolve, nothing is created.",
    input: {
      name: z.string().describe("A name not already used by one of your decks."),
      commanders: z.array(cardRef).max(2).optional().describe("One commander, or two that may be paired."),
      list: z
        .string()
        .optional()
        .describe(
          'A decklist as text, one card per line: "1 Card Name". A line ending in [Commander] goes to the ' +
            "command zone; other [Category] names that match a slot file the card there.",
        ),
      thesis: z.string().optional().describe("What the deck is trying to do. Cards as [[Card Name]]."),
    },
    run: ({ db, actor }, a) => {
      const commanders = resolveCards(db, a.commanders ?? []);
      let thesisProposal: number | null = null;
      const id = withTransaction(db, () => {
        const deck = createDeck(db, actor, a.name);
        if (a.list?.trim()) fillFromList(db, actor, deck, a.list);
        const changes: Change[] = commanders.map((c) => {
          const row = findDeckCard(db, actor, deck, c.oracle_id);
          return row && !row.maybe
            ? { action: "role", oracle_id: c.oracle_id, role: "commander" }
            : { action: "add", oracle_id: c.oracle_id, role: "commander" };
        });
        if (changes.length) applyChanges(db, actor, deck, changes, { kind: "edit", summary: "Set the commander" });
        if (a.thesis?.trim()) {
          const edit = proposeDocumentEdit(db, actor, deck, "thesis", { content: a.thesis }, "Initial thesis proposed while creating the requested deck.");
          thesisProposal = edit.edit_id;
        }
        return deck;
      });
      return done(`Created Theorycrafted deck ${id} "${a.name.trim()}" (lifecycle ${requireDeck(db, actor, id).category}).\n${thesisProposal != null ? `Initial thesis proposed as edit #${thesisProposal}; awaiting the owner.\n` : ""}${afterWrite(db, actor, id)}`);
    },
  },
  {
    name: "fork_deck",
    prices: true,
    title: "Fork a deck",
    readOnly: false,
    description:
      "Copy a deck — its list, slots, tags, maybe list, brief and deck memories — into a new Theorycrafted deck that " +
      "records where it came from. Use it when the owner wants to make big changes to a deck but keep the " +
      "original. The original is untouched; compare_decks shows how the two diverge. " +
      "Another user's shared deck (its link or slug) forks too, without its memories, rulings or power records.",
    input: { deck: sharedRef, name: z.string().describe("The new deck's name.") },
    run: ({ db, actor }, a) => {
      const source = deckOrShared(db, actor, a.deck);
      const id = forkDeck(db, actor, typeof source === "number" ? requireDeckOwner(db, actor, source).id : source, a.name);
      const from = requireDeck(db, actor, id);
      if (typeof source !== "number" && from.parent_owner_handle) {
        recordActivity(db, actor, "fork_deck", id, `Forked ${from.parent_owner_handle}/${from.parent_name} as "${a.name}"`);
        return done(
          `Forked ${from.parent_owner_handle}'s shared deck "${from.parent_name}" into your Theorycrafted deck ${id} "${a.name.trim()}". ` +
            `It copied the list, slots, tags, brief and engines; it has none of the original's memories, rulings or power records. ` +
            `Edit its cards with edit_cards.\n${afterWrite(db, actor, id)}`,
        );
      }
      const parent = from.parent_id!;
      recordActivity(db, actor, "fork_deck", id, `Forked "${requireDeck(db, actor, parent).name}" as "${a.name}"`);
      return done(
        `Forked into Theorycrafted deck ${id} "${a.name.trim()}". Edit its cards with edit_cards.\n` +
          `It starts with the original's brief, memories and rulings history. If the fork has a different ` +
          `aim, propose a new thesis first (propose_brief_edit), then change the cards.\n${afterWrite(db, actor, id)}`,
      );
    },
  },
  {
    name: "edit_cards",
    prices: true,
    title: "Edit Theorycrafted cards",
    readOnly: false,
    description:
      "Change a Theorycrafted deck’s cards directly: add and remove cards or set commanders. " +
      "Everything in one call lands together as one logged change, or none of it does. On a Paper deck " +
      "use propose_changes; Legacy is read-only. Brief changes always use propose_brief_edit. " +
      "Returns the new counts and check results.",
    input: {
      deck: deckRef,
      add: z
        .array(
          z.object({
            card: cardRef,
            quantity: z.number().int().min(1).optional().describe("Copies to add (default 1)."),
            slot: z.string().optional().describe("An existing primary purpose slot."),
            secondary_slots: z.array(z.string()).optional().describe("Additional purpose slots; omit to retain a maybe card's assignments."),
          }),
        )
        .optional(),
      remove: z
        .array(
          z.object({
            card: cardRef,
            quantity: z
              .number()
              .int()
              .min(1)
              .optional()
              .describe("Copies to remove. Omit to remove the card entirely."),
            to_maybe: z.boolean().optional().describe("Move it to the maybe list instead of out of the deck."),
          }),
        )
        .optional(),
      commanders: z
        .array(cardRef)
        .max(2)
        .optional()
        .describe(
          "Replace the command zone with exactly these cards (added if not in the deck). A former " +
            "commander stays in the deck as an ordinary card unless you also remove it.",
        ),
      note: z.string().optional().describe("One line for the log saying what this edit was."),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      requireDirectCardEdit(db, actor, deck);
      const adds = (a.add ?? []) as Array<{ card: string; quantity?: number; slot?: string; secondary_slots?: string[] }>;
      const removes = (a.remove ?? []) as Array<{ card: string; quantity?: number; to_maybe?: boolean }>;
      const addCards = resolveCards(db, adds.map((x) => x.card));
      const removeCards = resolveCards(db, removes.map((x) => x.card));
      const commanders = a.commanders === undefined ? null : resolveCards(db, a.commanders);

      const changes: Change[] = [];
      if (commanders) {
        const keep = new Set(commanders.map((c) => c.oracle_id));
        for (const c of getDeck(db, actor, deck).cards)
          if (c.role === "commander" && !c.maybe && !keep.has(c.oracle_id))
            changes.push({ action: "role", oracle_id: c.oracle_id, role: "card" });
      }
      removes.forEach((r, i) =>
        changes.push({
          action: "cut",
          oracle_id: removeCards[i].oracle_id,
          quantity: r.quantity ?? "all",
          park: r.to_maybe,
        }),
      );
      adds.forEach((x, i) =>
        changes.push({
          action: "add",
          oracle_id: addCards[i].oracle_id,
          quantity: x.quantity,
          slot_id: x.slot ? requireSlotByName(db, actor, deck, x.slot).id : undefined,
          secondary_slot_ids: x.secondary_slots?.map(name => requireSlotByName(db, actor, deck, name).id),
        }),
      );
      for (const c of commanders ?? []) {
        const row = findDeckCard(db, actor, deck, c.oracle_id);
        const beingAdded = addCards.some((x) => x.oracle_id === c.oracle_id);
        changes.push(
          (row && !row.maybe) || beingAdded
            ? { action: "role", oracle_id: c.oracle_id, role: "commander" }
            : { action: "add", oracle_id: c.oracle_id, role: "commander" },
        );
      }

      withTransaction(db, () => {
        if (changes.length)
          applyChanges(db, actor, deck, changes, { kind: "edit", summary: a.note?.trim() || undefined });
      });
      const did = [
        adds.length ? `added ${adds.length}` : null,
        removes.length ? `removed ${removes.length}` : null,
        commanders ? "set commanders" : null,
      ].filter(Boolean);
      if (!did.length) throw new ServiceError("Nothing to do: give cards to add or remove, or commanders");
      recordActivity(db, actor, "edit_cards", deck, `${did.join(", ")}${a.note ? ` — ${a.note}` : ""}`);
      return done(`Done (${did.join(", ")}).\n${afterWrite(db, actor, deck)}`);
    },
  },
  {
    name: "define_slots",
    prices: true,
    title: "Define a deck's slots",
    readOnly: false,
    description:
      "Create, retarget, rename and remove slots on Paper or Theorycrafted decks; this is organization and " +
      "needs no ruling. A slot says what a card does (ramp, interaction, card advantage) and may carry a " +
      "soft target count or range. A name you give that does not exist is created; one that exists is " +
      "updated. Card types (land, creatures…) cannot be slot names. All of it lands or none does.",
    input: {
      deck: deckRef,
      slots: z
        .array(
          z.object({
            name: z.string(),
            rename_to: z.string().optional(),
            target_min: z.number().int().min(0).nullable().optional().describe("null clears it."),
            target_max: z.number().int().min(0).nullable().optional().describe("null clears it."),
            remove: z.boolean().optional().describe("Remove this purpose from all cards; their other slot assignments stay."),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      defineSlots(db, actor, deck, a.slots);
      recordActivity(db, actor, "define_slots", deck, `Slots: ${a.slots.map((s: any) => (s.remove ? `removed ${s.name}` : s.rename_to ?? s.name)).join(", ")}`);
      return done(`Slots updated.\n${renderDeck(db, actor, deck, ["state"])}`);
    },
  },
  {
    name: "file_cards",
    prices: true,
    title: "File cards into slots and tags",
    readOnly: false,
    description:
      "Set primary and secondary purpose slots and management tags of cards already in a deck (or on its maybe list), on Paper or Theorycrafted decks; this is " +
      "organization and needs no ruling. File many cards in one call; if any card or slot name is wrong, " +
      "nothing is filed. A tag that does not exist yet is created, unless it would duplicate an existing " +
      "tag (same words, different spelling), in which case the call fails and names the tag to use. " +
      "remove_unused_tags removes explicitly named vocabulary entries after filing, only if no card still uses them.",
    input: {
      deck: deckRef,
      remove_unused_tags: z.array(z.string()).optional().describe("Remove these tags from the vocabulary after filing, only if no card still uses them. Other unused tags stay."),
      filings: z
        .array(
          z.object({
            card: cardRef,
            slot: z.string().nullable().optional().describe("Primary slot name; null clears the primary; omit to leave it."),
            secondary_slots: z.array(z.string()).optional().describe("Replace secondary purpose slots; [] clears them; omit to leave them. Cannot include the primary slot."),
            add_tags: z.array(z.string()).optional(),
            remove_tags: z.array(z.string()).optional(),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      const filings = a.filings as Array<{ card: string; slot?: string | null; secondary_slots?: string[]; add_tags?: string[]; remove_tags?: string[] }>;
      const cards = resolveCards(db, filings.map((f) => f.card));
      fileCards(
        db, actor,
        deck,
        filings.map((f, i) => ({ ...f, oracle_id: cards[i].oracle_id })),
        a.remove_unused_tags,
      );
      recordActivity(db, actor, "file_cards", deck, `Filed ${cards.length} card(s)${a.remove_unused_tags?.length ? `; removed unused tags: ${a.remove_unused_tags.join(", ")}` : ""}`);
      return done(`Filed ${cards.length} card(s).\n${renderDeck(db, actor, deck, ["state"])}`);
    },
  },
  {
    name: "shortlist_cards",
    prices: true,
    title: "Add to or remove from the maybe list",
    readOnly: false,
    description:
      "Put cards on a deck's maybe list, or take them off it, on Paper or Theorycrafted decks; needs no ruling. The maybe list " +
      "is outside the 100: use it to park candidates worth considering, each with a note saying why. " +
      "Moving a card from the deck itself to the maybe list is a cut and goes through edit_cards or " +
      "propose_changes.",
    input: {
      deck: deckRef,
      add: z
        .array(z.object({ card: cardRef, note: z.string().optional().describe("Why it is worth considering. Cards as [[Card Name]].") }))
        .optional(),
      remove: z.array(cardRef).optional(),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      const adds = (a.add ?? []) as Array<{ card: string; note?: string }>;
      const addCards = resolveCards(db, adds.map((x) => x.card));
      const removeCards = resolveCards(db, a.remove ?? []);
      if (!adds.length && !removeCards.length) throw new ServiceError("Give cards to add or remove");
      withTransaction(db, () => {
        adds.forEach((x, i) => {
          if (x.note) lintProse(db, x.note);
          shortlistCard(db, actor, deck, addCards[i].oracle_id, x.note ?? "");
        });
        for (const c of removeCards) unshortlistCard(db, actor, deck, c.oracle_id);
      });
      recordActivity(
        db,
        actor,
        "shortlist_cards",
        deck,
        [
          addCards.length ? `shortlisted ${addCards.map((c) => c.name).join(", ")}` : null,
          removeCards.length ? `dropped ${removeCards.map((c) => c.name).join(", ")}` : null,
        ]
          .filter(Boolean)
          .join("; "),
      );
      return done(`Maybe list updated.\n${renderDeck(db, actor, deck, ["maybe"], false)}`);
    },
  },
  {
    name: "remember",
    title: "Write a memory",
    readOnly: false,
    description:
      "Save one thing worth knowing next time: a preference, a correction, the reason behind a ruling, a " +
      "fact about the owner's playgroup, how a deck played. No approval is needed; the owner can see and " +
      "edit every memory. One statement and its reason per memory. Never save what a tool can tell you or " +
      "what a card says. The result lists existing memories that resemble this one: if one is the same " +
      "idea, use revise_memory on it instead of leaving both.",
    input: {
      text: z.string().describe("The statement and its reason. Card names as [[Card Name]]; they are linked automatically."),
      scope: z.enum(["global", "deck"]).describe("'global' is about the owner; 'deck' is about one deck."),
      deck: deckRef.optional().describe("Required when scope is 'deck'."),
      cards: z.array(cardRef).optional().describe("Other cards to link, beyond those named in the text."),
      basis: z
        .enum(["stated", "inferred"])
        .optional()
        .describe("'stated' only if the owner said it; 'inferred' (the default) for your own conclusion."),
      source_ruling: z.number().int().optional().describe("The id of the ruling that prompted this, if one did."),
    },
    run: ({ db, actor }, a) => {
      const deck = a.scope === "deck" ? (a.deck === undefined ? null : deckId(db, actor, a.deck, true)) : null;
      const { memory, similar } = remember(db, actor, {
        text: a.text,
        scope: a.scope,
        deck_id: deck,
        cards: resolveCards(db, a.cards ?? []).map((c) => c.oracle_id),
        basis: a.basis,
        source_log_id: a.source_ruling,
      });
      recordActivity(db, actor, "remember", deck, `Memory #${memory.id}: ${memory.text}`);
      const loaded = loadScope(db, actor, memory.scope, deck ?? undefined);
      return done(
        [
          `Saved.\n${memoryLine(memory)}`,
          similar.length
            ? `These existing memories resemble it. If one is the same idea, supersede or retire it with revise_memory so both do not stand:\n${similar.map(memoryLine).join("\n")}`
            : null,
          loaded.over
            ? `This scope is now over budget (${loaded.omitted} memories no longer load). Consolidate it.`
            : null,
        ]
          .filter(Boolean)
          .join("\n\n"),
      );
    },
  },
  {
    name: "revise_memory",
    title: "Revise a memory",
    readOnly: false,
    description:
      "Change an existing memory. 'update' edits it in place (a correction or tightening). 'supersede' " +
      "replaces it with new text and keeps the old one as history — use this when the fact has changed, " +
      "or with scope 'global' to promote a deck memory. 'retire' takes it out of use without deleting it. " +
      "'confirm' marks an inferred memory as stated once the owner has confirmed it. 'restore' brings a " +
      "retired memory back. You cannot change a memory the owner wrote or edited.",
    input: {
      id: z.number().int(),
      action: z.enum(["update", "supersede", "retire", "confirm", "restore"]),
      text: z.string().optional().describe("New text, for update and supersede."),
      cards: z.array(cardRef).optional().describe("Replace the linked cards."),
      basis: z.enum(["stated", "inferred"]).optional(),
      scope: z.enum(["global", "deck"]).optional().describe("For supersede: the replacement's scope."),
      deck: deckRef.optional().describe("For supersede into deck scope."),
    },
    run: ({ db, actor }, a) => {
      const cards = a.cards ? resolveCards(db, a.cards).map((c) => c.oracle_id) : undefined;
      let result: MemoryView;
      if (a.action === "update") {
        result = reviseMemory(db, actor, a.id, { action: "update", text: a.text, cards, basis: a.basis });
      } else if (a.action === "supersede") {
        if (!a.text?.trim()) throw new ServiceError("Superseding needs the replacement text");
        result = reviseMemory(db, actor, a.id, {
          action: "supersede",
          text: a.text,
          cards,
          basis: a.basis,
          scope: a.scope,
          deck_id: a.deck === undefined ? undefined : deckId(db, actor, a.deck, true),
        });
      } else {
        result = reviseMemory(db, actor, a.id, { action: a.action });
      }
      recordActivity(db, actor, "revise_memory", result.deck_id, `${a.action} memory #${a.id}${result.id !== a.id ? ` → #${result.id}` : ""}: ${result.text}`);
      return done(
        `${a.action === "supersede" ? `Memory #${a.id} is now superseded by:` : "Done:"}\n${memoryLine(result)}`,
      );
    },
  },
  {
    name: "mark_rulings_reviewed",
    title: "Mark rulings as read",
    readOnly: false,
    description:
      "Tell the app you have read rulings from list_rulings and decided what, if anything, is worth a " +
      "memory. Give the ids, or 'through' to clear that ruling's deck up to and including its id.",
    input: {
      ids: z.array(z.number().int()).optional(),
      through: z.number().int().optional(),
    },
    run: ({ db, actor }, a) => {
      const n = markReviewed(db, actor, { ids: a.ids, through: a.through });
      recordActivity(db, actor, "mark_rulings_reviewed", null, `Reviewed ${n} ruling(s)`);
      return done(`Marked ${n} as reviewed. ${unreviewedCount(db, actor)} still unreviewed.`);
    },
  },

  // ---------- proposals ----------

  {
    name: "propose_changes",
    title: "Propose changes to a deck",
    readOnly: false,
    description:
      "Propose adding and cutting cards. This is the only way to change which cards are in a Paper deck: " +
      "the owner rules on each item in the app, and you learn the outcome from list_rulings. Propose as " +
      "many items as the request calls for. Every item needs its own reason (write cards as [[Card Name]], " +
      "and quote the oracle text you rely on). Give items that only make sense together the same group " +
      "so they are accepted or rejected as a unit. A cut removes the card from the deck; set to_maybe on " +
      "a cut to move it to the maybe list instead, when it is worth keeping in view. The result shows " +
      "what the deck would become and any prior rulings and memories on the cards involved.",
    input: {
      deck: deckRef,
      expected_revision: z.number().int().nonnegative().optional(),
      summary: z.string().optional().describe("One line on what this proposal is for."),
      items: z
        .array(
          z.object({
            action: z.enum(["add", "cut"]),
            card: cardRef,
            reason: z.string().describe("Why. The owner reads this when ruling."),
            slot: z.string().optional().describe("For an add: an existing primary purpose slot."),
            secondary_slots: z.array(z.string()).optional().describe("For an add: additional purpose slots; omit to retain a maybe card's assignments."),
            to_maybe: z
              .boolean()
              .optional()
              .describe("For a cut: if accepted, move the card to the maybe list instead of out of the deck (default false)."),
            group: z.string().optional().describe("A label shared by items to be ruled on together."),
          }),
        )
        .min(1),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      const items = a.items as Array<{
        action: "add" | "cut";
        card: string;
        reason: string;
        slot?: string;
        secondary_slots?: string[];
        to_maybe?: boolean;
        group?: string;
      }>;
      const cards = resolveCards(db, items.map((i) => i.card));
      const id = createProposal(db, actor, deck, {
        summary: a.summary,
        expected_revision: a.expected_revision,
        items: items.map((i, n) => ({
          action: i.action,
          oracle_id: cards[n].oracle_id,
          reason: i.reason,
          slot_id: i.slot ? requireSlotByName(db, actor, deck, i.slot).id : undefined,
          secondary_slot_ids: i.secondary_slots?.map(name => requireSlotByName(db, actor, deck, name).id),
          to_maybe: i.to_maybe,
          group: i.group,
        })),
      });
      const proposal = getProposal(db, actor, id, true)!;
      recordActivity(db, actor, "propose_changes", deck, `Proposal #${id}: ${proposal.items.length} item(s)${a.summary ? ` — ${a.summary}` : ""}`);

      const view = getDeck(db, actor, deck);
      const c = view.computed;
      const ids = cards.map((x) => x.oracle_id);
      const rulings = cardHistories(db, actor, deck, ids);
      const memories = memoriesForCards(db, actor, ids, deck);
      const context = cards
        .filter((x) => rulings.has(x.oracle_id) || memories.has(x.oracle_id))
        .map((x) =>
          [
            `- ${x.name}:`,
            ...(rulings.get(x.oracle_id) ?? []).map((r) => `  ${rulingLine(r)}`),
            ...(memories.get(x.oracle_id) ?? []).map((m) => `  ${memoryLine(m)}`),
          ].join("\n"),
        );
      return done(
        [
          `Proposal #${id} is waiting for the owner's ruling in the app (${proposal.items.length} item(s)).`,
          `The deck is ${c.card_count}/100. Awaiting ruling across all proposals: +${c.pending.adds} / −${c.pending.cuts}; ` +
            `if all were accepted it would be ${c.pending.projected_count}/100.`,
          context.length
            ? `What the app already knows about these cards. If your reasons did not account for it, tell the owner now:\n${context.join("\n")}`
            : null,
        ]
          .filter(Boolean)
          .join("\n\n"),
      );
    },
  },
  {
    name: "list_proposals",
    prices: true,
    title: "Read current proposals and their history",
    readOnly: true,
    description: "Read card and document proposals with current status, supersession links, revision bases, stale flags, and full preserved drafts. Defaults to current pending work; include_history returns resolved, withdrawn and superseded versions. No approval or mutation is performed.",
    input: { deck: deckRef.optional(), include_history: z.boolean().optional() },
    run: ({ db, actor }, a) => {
      const id = a.deck === undefined ? undefined : deckId(db, actor, a.deck);
      const proposals = listProposals(db, actor, { deckId: id, status: a.include_history ? undefined : "open" });
      const document_edits = listDocumentEdits(db, actor, { deckId: id, status: a.include_history ? undefined : "pending" });
      return { structuredContent: { proposals, document_edits }, text:
        [...proposals.map(p => `Card proposal #${p.id}: ${p.is_current ? "current open" : p.disposition === "current" ? p.status : p.disposition}, based on r${p.base_revision ?? "unknown"}${p.stale ? ", stale" : ""}${p.superseded_by ? ` → #${p.superseded_by}` : ""}`),
          ...document_edits.map(e => `Document edit #${e.id}${e.group_id ? ` (group #${e.group_id})` : ""}: ${e.kind}, ${e.is_current ? "current pending" : e.disposition === "current" ? e.status : e.disposition}, based on r${e.base_revision ?? "unknown"}${e.stale ? ", stale" : ""}${e.superseded_by ? ` → #${e.superseded_by}` : ""}`)].join("\n") || "No proposals.",
      };
    },
  },
  {
    name: "propose_document_changes",
    title: "Propose a related set of document changes",
    readOnly: false,
    description: "Propose related brief or power-target changes as one atomic owner approval group. Use supersedes to explicitly replace your previous pending edits/groups; their history and links remain. The full replacement group becomes current. Always awaits owner approval for agents; Legacy is read-only. Profile groups omit deck and contain only profile edits. Duplicate current drafts for the same field require explicit supersession.",
    input: { deck: deckRef.optional(), changes: z.array(documentChange).min(1),
      supersedes: z.array(z.number().int().positive()).optional(), expected_revision: z.number().int().nonnegative().optional() },
    run: ({ db, actor }, a) => {
      const id = a.deck === undefined ? null : deckId(db, actor, a.deck, true);
      const result = proposeDocumentGroup(db, actor, id, documentChanges(db, actor, a.changes), a);
      return { changed: true, structuredContent: { ...result, document_edits: listDocumentEdits(db, actor, id == null ? { profileOnly: true } : { deckId: id }) },
        text: `Document group #${result.group_id}: edits ${result.edit_ids.join(", ")}; awaiting the owner's grouped approval. Superseded drafts remain in history.` };
    },
  },
  {
    name: "revise_proposal",
    prices: true,
    title: "Amend or withdraw your pending proposal",
    readOnly: false,
    description: "Amend or withdraw only your own pending card or document proposal. Amendments are linked replacements retaining the original audit history. For documents name any edit in a group and supply the complete replacement changes; the whole group is superseded. For partially ruled card proposals, supply only remaining work; earlier rulings stay in history. Never accepts, changes lifecycle, or edits an approved brief/power target.",
    input: { type: z.enum(["cards", "document"]), id: z.number().int().positive(), action: z.enum(["amend", "withdraw"]),
      reason: z.string().trim().min(1), expected_revision: z.number().int().nonnegative().optional(),
      summary: z.string().optional(), items: z.array(proposalItem).min(1).optional(), changes: z.array(documentChange).min(1).optional() },
    run: ({ db, actor }, a) => {
      if (a.action === "withdraw") {
        const scope = a.type === "cards" ? getProposal(db, actor, a.id, true)?.deck_id : getDocumentEdit(db, actor, a.id, true)?.deck_id;
        if (a.type === "cards") withdrawProposal(db, actor, a.id, a.reason, a.expected_revision);
        else withdrawDocumentEdit(db, actor, a.id, a.reason, a.expected_revision);
        return done(`Withdrew ${a.type} proposal #${a.id}; history retained.${scope == null ? "" : `\n${afterWrite(db, actor, scope)}`}`);
      }
      if (a.type === "document") {
        const edit = getDocumentEdit(db, actor, a.id, true);
        if (!edit) throw new ServiceError(`Proposed edit ${a.id} not found`, 404);
        if (!a.changes) throw new ServiceError("A document amendment needs the complete replacement changes");
        lintProse(db, a.reason);
        const result = proposeDocumentGroup(db, actor, edit.deck_id, documentChanges(db, actor, a.changes), { supersedes: [a.id], expected_revision: a.expected_revision });
        recordActivity(db, actor, "amend_document_proposal", edit.deck_id, `Amended edit #${a.id} with group #${result.group_id}: ${a.reason}`);
        return { changed: true, structuredContent: result, text: `Current document group #${result.group_id} supersedes #${a.id}; awaiting owner approval.` };
      }
      const old = getProposal(db, actor, a.id, true);
      if (!old) throw new ServiceError(`Proposal ${a.id} not found`, 404);
      if (!a.items) throw new ServiceError("A card amendment needs the complete replacement items");
      lintProse(db, a.reason);
      const items = (a.items as z.infer<typeof proposalItem>[]).map(i => ({ ...i,
        oracle_id: resolveCard(db, i.card).oracle_id,
        slot_id: i.slot ? requireSlotByName(db, actor, old.deck_id, i.slot).id : undefined,
        secondary_slot_ids: i.secondary_slots?.map(name => requireSlotByName(db, actor, old.deck_id, name).id),
      }));
      const id = amendProposal(db, actor, a.id, { summary: a.summary, items, expected_revision: a.expected_revision });
      recordActivity(db, actor, "amend_card_proposal", old.deck_id, `Amended #${a.id} as #${id}: ${a.reason}`);
      return { changed: true, structuredContent: { proposal: getProposal(db, actor, id, true) }, text: `Current card proposal #${id} supersedes #${a.id}; awaiting owner approval.\n${afterWrite(db, actor, old.deck_id)}` };
    },
  },
  {
    name: "propose_brief_edit",
    prices: true,
    title: "Propose an edit to a deck's brief",
    readOnly: false,
    description:
      "Propose a change to a deck's brief: replace the thesis or the constraints, define or replace a " +
      "named engine (with the cards that are its required pieces), or remove one. The brief is the " +
      "owner's document, so this always waits for their ruling and needs a reason citing what " +
      "prompted it. Legacy is read-only.",
    input: {
      deck: deckRef,
      expected_revision: z.number().int().nonnegative().optional(),
      kind: z.enum(["thesis", "constraints", "engine_set", "engine_remove"]),
      text: z.string().optional().describe("For thesis / constraints: the full replacement text."),
      engine_name: z.string().optional().describe("For engine_set / engine_remove."),
      description: z.string().optional().describe("For engine_set: what the engine does."),
      pieces: z
        .array(z.object({ card: cardRef, note: z.string().optional() }))
        .optional()
        .describe("For engine_set: the cards the engine needs."),
      reason: z.string().trim().min(1).describe("What prompted this (rulings, memories). Required for every brief proposal."),
    },
    run: ({ db, actor }, a) => {
      const deck = deckId(db, actor, a.deck, true);
      const pieces = (a.pieces ?? []) as Array<{ card: string; note?: string }>;
      const cards = resolveCards(db, pieces.map((p) => p.card));
      const r = proposeDocumentEdit(
        db,
        actor,
        deck,
        a.kind,
        {
          content: a.text,
          engine_name: a.engine_name,
          description: a.description,
          pieces: a.kind === "engine_set" ? pieces.map((p, i) => ({ oracle_id: cards[i].oracle_id, note: p.note })) : undefined,
        },
        a.reason ?? "",
        { expected_revision: a.expected_revision },
      );
      recordActivity(db, actor, "propose_brief_edit", deck, r.applied ? `Wrote the brief (${a.kind})` : `Proposed brief edit #${r.edit_id} (${a.kind})`);
      return done(
        r.applied
          ? `Applied the brief edit.\n${renderDeck(db, actor, deck, ["brief"])}`
          : `Proposed edit #${r.edit_id} is waiting for the owner's ruling in the app.`,
      );
    },
  },
  {
    name: "propose_profile_edit",
    title: "Propose an edit to the owner's profile",
    readOnly: false,
    description:
      "Propose new text for the owner's profile: who they are as a builder — tastes, playgroup and power " +
      "level, budget stance, standing dislikes. Give the complete replacement text; the owner sees a diff " +
      "against the current profile and rules on it. Use this when memories have settled into something " +
      "durable, and cite them in the reason.",
    input: {
      text: z.string().describe("The complete new profile, in markdown. Keep it under about 600 words."),
      reason: z.string().describe("What prompted this: the rulings and memories it rests on."),
    },
    run: ({ db, actor }, a) => {
      const r = proposeDocumentEdit(db, actor, null, "profile", { content: a.text }, a.reason);
      recordActivity(db, actor, "propose_profile_edit", null, r.applied ? "Edited the profile" : `Proposed profile edit #${r.edit_id}`);
      return done(
        r.applied ? "The profile was updated." : `Proposed edit #${r.edit_id} is waiting for the owner's ruling in the app.`,
      );
    },
  },
];

// ---------- calling a tool without a transport ----------

const SCHEMAS = new Map(TOOLS.map((t) => [t.name, (["create_deck", "edit_cards"].includes(t.name) ? z.strictObject(t.input) : z.object(t.input))]));

/** Validate arguments and run a tool. Errors propagate; the transport turns them into tool errors. */
export function callTool(ctx: ToolContext, name: string, args: unknown = {}): ToolResult {
  const tool = TOOLS.find((t) => t.name === name);
  if (!tool) throw new ServiceError(`Unknown tool '${name}'`, 404);
  const parsed = SCHEMAS.get(name)!.safeParse(args ?? {});
  if (!parsed.success)
    throw new ServiceError(
      `Invalid arguments for ${name}: ${parsed.error.issues.map((i) => `${i.path.join(".") || "(arguments)"}: ${i.message}`).join("; ")}`,
    );
  return runTool(ctx, tool, parsed.data);
}

/** Shared execution for SDK-validated HTTP calls and local validated calls.
 * Non-price tools retain their exact text, particularly pasteable exports. */
export function runTool(ctx: ToolContext, tool: ToolDef, args: unknown): ToolResult {
  const result = tool.run(ctx, args);
  const output = typeof result === "string" ? { text: result } : result;
  if (!tool.prices) return output;
  const line = priceSourceLine(priceResolver(ctx.db, ctx.actor));
  return { ...output, text: [output.text.split("\n" + line).join("").split(line).join("").trimEnd(), line].join("\n") };
}
