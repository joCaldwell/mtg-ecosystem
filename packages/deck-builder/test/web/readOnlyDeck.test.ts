import { test } from "node:test";
import assert from "node:assert/strict";
import type { PublicDeckState } from "../../web/src/api/client.ts";
import { fromOwner, fromPublic, priceSource } from "../../web/src/domain/readOnlyDeck.ts";
import { card, deckState, logEntry, item, proposal } from "./fixtures.ts";

function shared(over: Partial<PublicDeckState> = {}): PublicDeckState {
  const state = deckState({ cards: [card({ name: "Forest", quantity: 7 })] });
  return {
    deck: { name: "Shared", slug: "Ab3dE6gH9k", owner: { handle: "owner", display_name: "Owner" }, commander: [], color_identity: "G", category: "paper", visibility: "unlisted", parent: null },
    cards: state.cards, slots: state.slots, tags: state.tags,
    brief: { thesis: "Ramp", constraints_md: "" }, engines: [], checks: [],
    power: { target: null, latest_owner_assessment: null },
    price: { total_cents: 120, currency: "USD", store: "tcgplayer", unpriced: 1, as_of: "2026-10-25T00:00:00Z", source: "Scryfall" },
    computed: state.computed,
    ...over,
  };
}

test("a shared deck reads with nothing pending and no numeric id (spec §18.3)", () => {
  const model = fromPublic(shared());
  assert.equal(model.list.deck.id, 0);
  assert.deepEqual(model.list.proposals, []);
  assert.deepEqual(model.list.log, []);
  assert.equal(model.decisions, null, "unpublished decisions stay absent");
  assert.equal(model.brief.thesis, "Ramp");
  const published = fromPublic(shared({ decisions: [{ kind: "accept", summary: "Added Forest", reason: "", by: "agent", at: "2026-10-25 10:00:00", cards: [] }] }));
  assert.equal(published.decisions!.length, 1);
});

test("the owner's phone keeps pending proposals in the list, so they are drawn in place (spec §11.2)", () => {
  const pending = proposal([item({ name: "Sol Ring" })]);
  const state = deckState({ proposals: [pending], log: [
    logEntry({ summary: "Added Forest", author_via: "ui" }),
    logEntry({ summary: "Cut Island", author_via: "agent", author_display: "owner via test-agent" }),
  ] });
  const model = fromOwner(state);
  assert.equal(model.list, state);
  assert.equal(model.list.proposals.length, 1);
  // The owner sees which agent did what (DESIGN.md principle 5, review S15).
  assert.deepEqual(model.decisions!.map((d) => [d.summary, d.by, d.who]), [["Added Forest", "owner", "owner"], ["Cut Island", "agent", "owner via test-agent"]]);
  assert.ok(fromPublic(shared({ decisions: [{ kind: "edit", summary: "x", reason: "", by: "agent", at: "", cards: [] }] })).decisions!.every((d) => !("who" in d)), "viewers never get a name");
  assert.equal(model.price.store, "tcgplayer");
});

test("a price names its source, store, currency and date (D18)", () => {
  assert.equal(priceSource(shared().price), "Scryfall · TCGplayer · USD · 2026-10-25");
});
