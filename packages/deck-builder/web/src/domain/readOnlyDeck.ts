// What ReadOnlyDeck draws (spec §18.5, §11.2), built from either source: a
// shared deck's PublicDeckState for a viewer, or the owner's own DeckState on
// a phone. One shape, so the public page and the phone view are the same
// component rather than two copies.

import type { DeckListState, DeckState, Engine, Finding, PublicDeckState, PublicDecision } from "../api/client.ts";

export interface ReadOnlyDeckModel {
  list: DeckListState;
  /** A shared deck read by anyone (true), or the owner's own deck (false). */
  viewer: boolean;
  checks: Finding[];
  brief: { thesis: string; constraints_md: string };
  engines: Array<Pick<Engine, "name" | "description" | "pieces">>;
  power: {
    target: PublicDeckState["power"]["target"];
    assessment: PublicDeckState["power"]["latest_owner_assessment"];
  };
  price: PublicDeckState["price"];
  /** Null when the owner has not published them (or, on a phone, there are none).
   *  `who` is set only for the owner's own deck: they always see which agent
   *  wrote what (DESIGN.md principle 5); viewers see only "an agent". */
  decisions: Array<PublicDecision & { who?: string }> | null;
}

/** A viewer's deck. A shared deck has no numeric id for the viewer; the list
 *  draws nothing pending, because nothing pending is shared. */
export function fromPublic(shared: PublicDeckState): ReadOnlyDeckModel {
  return {
    viewer: true,
    list: {
      deck: { id: 0, category: shared.deck.category },
      cards: shared.cards, slots: shared.slots, tags: shared.tags, computed: shared.computed,
      proposals: [], log: [],
    },
    checks: shared.checks,
    brief: shared.brief,
    engines: shared.engines,
    power: { target: shared.power.target, assessment: shared.power.latest_owner_assessment },
    price: shared.price,
    decisions: shared.decisions ?? null,
  };
}

/** The owner's own deck, read on a phone: everything they may see, with
 *  pending proposals left in the list state so they are drawn in place. */
export function fromOwner(state: DeckState): ReadOnlyDeckModel {
  const t = state.power.target, a = state.power.assessment;
  return {
    viewer: false,
    list: state,
    checks: state.checks,
    brief: { thesis: state.brief.thesis, constraints_md: state.brief.constraints_md },
    engines: state.brief.engines,
    power: {
      target: t ? { profile: t.profile, game_changers_max: t.game_changers_max, notes: t.notes, mapping: t.mapping } : null,
      assessment: a ? { profile: a.profile, evidence: a.evidence, confidence: a.confidence, notes: a.notes, mapping: a.mapping, stale: a.stale, created_at: a.created_at } : null,
    },
    price: {
      total_cents: state.computed.cost.cents, currency: state.computed.cost.currency, store: state.computed.cost.store,
      unpriced: state.computed.cost.unpriced, as_of: state.computed.cost.as_of, source: state.computed.cost.source,
    },
    decisions: state.log.length
      ? state.log.map((e) => ({
          kind: e.kind, summary: e.summary, reason: e.decision_reason, by: e.author_via === "ui" ? "owner" : "agent", at: e.ts,
          who: e.author_display ?? e.author,
          cards: e.cards.map((c) => ({ oracle_id: c.oracle_id, card_name: c.card_name, action: c.action, quantity: c.quantity })),
        }))
      : null,
  };
}

/** "Scryfall · TCGplayer · USD · 2026-10-25": where a price estimate comes from (D18). */
export function priceSource(price: ReadOnlyDeckModel["price"]): string {
  const store = price.store === "tcgplayer" ? "TCGplayer" : price.store === "cardmarket" ? "Cardmarket" : price.store;
  return [price.source, store, price.currency, price.as_of?.slice(0, 10)].filter(Boolean).join(" · ");
}
