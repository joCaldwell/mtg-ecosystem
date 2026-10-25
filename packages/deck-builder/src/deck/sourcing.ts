import type { Database } from "../sqlite.ts";
import type { Actor } from "../actor.ts";
import { authorOf } from "../actor.ts";
import { recordActivity } from "../activity.ts";
import { lintProse, requireCardById } from "../cards.ts";
import { withTransaction } from "../db.ts";
import { ServiceError } from "../errors.ts";
import { estimateCost, pricesAsOf } from "../prices.ts";
import { fileCards, getDeck, requireDeck, requireEditableDeck, requireRevision } from "./service.ts";

export type OwnershipStatus = "owned" | "unowned" | "unknown";
export interface OwnershipInput {
  oracle_id: string;
  status: OwnershipStatus;
  quantity?: number | null;
  available_quantity?: number | null;
  basis: string;
  /** Optimistic concurrency for this card's evidence; null means none read. */
  expected_evidence_id?: number | null;
}
export interface OwnershipEvidence {
  id: number;
  oracle_id: string;
  status: OwnershipStatus;
  quantity: number | null;
  available_quantity: number | null;
  basis: string;
  author: string;
  source_deck_id: number | null;
  source_deck_name: string | null;
  created_at: string;
}

export function ownershipEvidence(db: Database, oracleId: string): OwnershipEvidence[] {
  return db.prepare("SELECT * FROM ownership_evidence WHERE oracle_id = ? ORDER BY id DESC").all(oracleId) as unknown as OwnershipEvidence[];
}

/** A statement of evidence, not an inferred inventory. Latest statement wins;
 * all earlier statements stay available for audit. Deck removal never edits it. */
export function recordOwnership(db: Database, actor: Actor, deckId: number, inputs: OwnershipInput[], expectedRevision?: number): void {
  requireEditableDeck(db, deckId);
  requireRevision(db, deckId, expectedRevision);
  if (!inputs.length) throw new ServiceError("Record at least one ownership statement");
  withTransaction(db, () => {
    const seen = new Set<string>();
    for (const input of inputs) {
      requireCardById(db, input.oracle_id);
      if (seen.has(input.oracle_id)) throw new ServiceError("Record each card's ownership once per batch");
      seen.add(input.oracle_id);
      const latest = ownershipEvidence(db, input.oracle_id)[0];
      if (input.expected_evidence_id !== undefined && input.expected_evidence_id !== (latest?.id ?? null))
        throw new ServiceError("Ownership evidence changed; compare again before recording it", 409);
      if (!["owned", "unowned", "unknown"].includes(input.status)) throw new ServiceError("Invalid ownership status");
      if (!input.basis?.trim()) throw new ServiceError("Ownership needs explicit evidence in basis");
      lintProse(db, input.basis);
      const quantity = input.quantity ?? null;
      const available = input.available_quantity ?? null;
      for (const value of [quantity, available])
        if (value != null && (!Number.isSafeInteger(value) || value < 0)) throw new ServiceError("Physical quantities must be nonnegative integers or null (unknown)");
      if ((input.status === "unowned" && (quantity !== 0 || (available != null && available !== 0))) ||
          (input.status === "unknown" && (quantity != null || available != null)) ||
          (input.status === "owned" && quantity === 0) ||
          (quantity != null && available != null && available > quantity))
        throw new ServiceError("Ownership quantities contradict the status or available quantity exceeds owned quantity; unowned requires quantity 0");
      const deck = requireDeck(db, deckId);
      db.prepare(`INSERT INTO ownership_evidence
        (oracle_id, status, quantity, available_quantity, basis, author, source_deck_id, source_deck_name)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)`).run(input.oracle_id, input.status, quantity, available, input.basis.trim(), authorOf(actor), deckId, deck.name);
    }
    recordActivity(db, actor, "record_ownership", deckId, `Recorded explicit ownership evidence for ${inputs.length} card(s)`);
  });
}

export interface SourcingOptions {
  sourcing?: "shared" | "not_in_sources";
  ownership?: OwnershipStatus;
  /** Unknowns remain distinct; this filter never claims they are unowned. */
  not_marked_owned?: boolean;
  availability?: "available" | "insufficient" | "unknown";
  sort?: "name" | "price_desc" | "price_asc" | "ownership" | "sourcing";
  apply_source_tags?: boolean;
  expected_revision?: number;
  expected_source_revisions?: Record<string, number>;
}

export function compareDeckSources(db: Database, actor: Actor, targetId: number, sourceIds: number[], opts: SourcingOptions = {}) {
  return withTransaction(db, () => {
    const ids = [...new Set(sourceIds)];
    if (!ids.length || ids.includes(targetId)) throw new ServiceError("Name at least one source deck, different from the target");
    requireRevision(db, targetId, opts.expected_revision);
    const target = getDeck(db, targetId);
    const sources = ids.map(id => getDeck(db, id));
    for (const source of sources) requireRevision(db, source.deck.id, opts.expected_source_revisions?.[source.deck.id]);
    const asOf = pricesAsOf(db);
    const all = target.cards.filter(c => !c.maybe).map(c => {
      const source_decks = sources.flatMap(s => {
        const match = s.cards.find(card => !card.maybe && card.oracle_id === c.oracle_id);
        return match ? [{ id: s.deck.id, name: s.deck.name, revision: s.deck.revision, quantity: match.quantity, tag: `From ${s.deck.name}` }] : [];
      });
      const evidence = ownershipEvidence(db, c.oracle_id);
      const latest = evidence[0];
      const available = latest?.available_quantity ?? null;
      return {
        oracle_id: c.oracle_id, name: c.name, quantity: c.quantity,
        source_decks, sourcing: source_decks.length ? "shared" as const : "not_in_sources" as const,
        ownership: { status: latest?.status ?? "unknown", quantity: latest?.quantity ?? null, evidence },
        reuse: { status: available == null ? "unknown" as const : available >= c.quantity ? "available" as const : "insufficient" as const, available_quantity: available },
        price: { usd_cents: c.usd_cents, as_of: asOf, estimated: true },
      };
    });
    let tagged = 0;
    if (opts.apply_source_tags) {
      requireEditableDeck(db, targetId);
      if (opts.expected_revision === undefined || sources.some(s => opts.expected_source_revisions?.[s.deck.id] === undefined))
        throw new ServiceError("Applying source tags requires expected_revision and expected_source_revisions for every source");
      const filings = all.filter(c => c.source_decks.length).map(c => ({ oracle_id: c.oracle_id, add_tags: c.source_decks.map(s => s.tag) }));
      fileCards(db, targetId, filings, [], actor);
      tagged = filings.length;
      recordActivity(db, actor, "compare_deck_sources", targetId, `Applied source tags to ${tagged} shared card(s)`);
    }
    const cards = all.filter(c => (!opts.sourcing || c.sourcing === opts.sourcing) &&
      (!opts.ownership || c.ownership.status === opts.ownership) &&
      (!opts.not_marked_owned || c.ownership.status !== "owned") &&
      (!opts.availability || c.reuse.status === opts.availability));
    cards.sort((a, b) => {
      if (opts.sort === "price_asc" || opts.sort === "price_desc") {
        if (a.price.usd_cents == null || b.price.usd_cents == null)
          return a.price.usd_cents == null ? (b.price.usd_cents == null ? a.name.localeCompare(b.name) : 1) : -1;
        return (opts.sort === "price_desc" ? -1 : 1) * (a.price.usd_cents - b.price.usd_cents) || a.name.localeCompare(b.name);
      }
      return (opts.sort === "ownership" ? a.ownership.status.localeCompare(b.ownership.status) : opts.sort === "sourcing" ? a.sourcing.localeCompare(b.sourcing) : 0) || a.name.localeCompare(b.name);
    });
    const summarize = (rows: typeof all) => ({
      cards: rows.length, copies: rows.reduce((sum, c) => sum + c.quantity, 0),
      cost: estimateCost(rows.map(c => ({ usd_cents: c.price.usd_cents, quantity: c.quantity }))),
    });
    const remaining = all.filter(c => c.sourcing === "not_in_sources" && c.ownership.status !== "owned");
    return {
      target: target.deck, sources: sources.map(s => s.deck), prices_as_of: asOf,
      cards, shared_cards: all.filter(c => c.sourcing === "shared"),
      remaining_cards: remaining.slice().sort((a, b) => (b.price.usd_cents ?? -1) - (a.price.usd_cents ?? -1) || a.name.localeCompare(b.name)),
      summary: { target: summarize(all), shared: summarize(all.filter(c => c.sourcing === "shared")), remaining: summarize(remaining),
        unknown_ownership: summarize(all.filter(c => c.ownership.status === "unknown")), result: summarize(cards), tagged },
    };
  });
}
