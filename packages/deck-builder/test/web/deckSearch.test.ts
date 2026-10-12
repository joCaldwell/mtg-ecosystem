import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { matchDeckList } from "../../web/src/domain/deckSearch.ts";
import { layoutDeck } from "../../web/src/domain/grouping.ts";
import { reviewOverlay } from "../../web/src/domain/review.ts";
import { card, deckState, item, proposal, slot } from "./fixtures.ts";

describe("deck list matches", () => {
  for (const groupBy of ["slot", "type"] as const) {
    test(`finds names in every section without changing the full layout (${groupBy})`, () => {
      const state = deckState({
        slots: [slot({ id: 10, target_min: 5 }), slot({ id: 11 })],
        cards: [
          card({ name: "Test Match Commander", role: "commander" }),
          card({ name: "Test Match Companion", role: "companion" }),
          card({ name: "Test Match Maybe", maybe: 1 }),
          card({ name: "Test Match Card", slot_id: 10 }),
          card({ name: "Test Other Card", slot_id: 10 }),
          card({ name: "Test Other Land", slot_id: 11, type_line: "Land" }),
          card({ name: "Test Other Maybe", maybe: 1 }),
        ],
      });
      const layout = layoutDeck(state, groupBy);
      const snapshot = structuredClone(layout);
      const result = matchDeckList(layout, new Map(), "  mAtCh  ");
      assert.equal(result.active, true);
      assert.deepEqual([...result.cardIds].sort(), state.cards
        .filter((c) => c.name.includes("Match")).map((c) => c.oracle_id).sort());
      assert.equal(result.addIds.size, 0);
      assert.deepEqual(layout, snapshot, "all cards, empty groups, order and totals remain intact");
    });

    test(`any/all tags compose with name search and retain context (${groupBy})`, () => {
      const state = deckState({
        cards: [
          card({ name: "Test Commander", role: "commander", tag_ids: [1] }),
          card({ name: "Test Companion", role: "companion", tag_ids: [1, 2] }),
          card({ name: "Test Both", tag_ids: [1, 2], quantity: 4 }),
          card({ name: "Test One", tag_ids: [1] }),
          card({ name: "Test Two", tag_ids: [2], maybe: 1 }),
          card({ name: "Test Untagged" }),
        ],
      });
      const layout = layoutDeck(state, groupBy);
      const snapshot = structuredClone(layout);
      const any = matchDeckList(layout, new Map(), "", { ids: [1, 2], match: "any" });
      assert.deepEqual([...any.cardIds].sort(), state.cards
        .filter((c) => c.tag_ids.length > 0).map((c) => c.oracle_id).sort());
      const all = matchDeckList(layout, new Map(), "", { ids: [1, 2], match: "all" });
      assert.deepEqual([...all.cardIds].sort(), ["oid-Test Both", "oid-Test Companion"]);
      const search = matchDeckList(layout, new Map(), "  bOtH  ", { ids: [1], match: "any" });
      assert.deepEqual([...search.cardIds], ["oid-Test Both"]);
      const noMatch = matchDeckList(layout, new Map(), "both", { ids: [99], match: "any" });
      assert.equal(noMatch.active, true);
      assert.equal(noMatch.cardIds.size, 0);
      assert.deepEqual(layout, snapshot, "tag filtering never removes rows or changes quantities and costs");
    });
  }

  test("review additions are matched separately and remain in their groups", () => {
    const matchingAdd = item({ name: "Test Match Proposed", slot_id: 11 });
    const otherAdd = item({ name: "Test Other Proposed", slot_id: 10 });
    const state = deckState({
      slots: [slot({ id: 10 }), slot({ id: 11 })],
      cards: [card({ name: "Test Other Card", slot_id: 10 })],
      proposals: [proposal([matchingAdd, otherAdd])],
    });
    const layout = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, layout, "slot");
    const snapshot = structuredClone(overlay);
    const result = matchDeckList(layout, overlay.addsByGroup, "match");
    assert.deepEqual([...result.addIds], [matchingAdd.id]);
    assert.equal(result.cardIds.size, 0);
    assert.deepEqual(overlay, snapshot, "the complete review overlay is preserved");
    assert.equal(matchDeckList(layout, new Map(), "match").addIds.size, 0,
      "proposals are not considered outside review");
  });

  test("proposed adds inherit parked tags; untagged adds are nonmatches", () => {
    const parked = card({ oracle_id: "parked", name: "Test Parked", maybe: 1, tag_ids: [1] });
    const matchingAdd = item({ oracle_id: "parked", name: "Test Parked" });
    const state = deckState({
      cards: [parked],
      proposals: [proposal([matchingAdd, item({ oracle_id: "new", name: "Test New" })])],
    });
    const layout = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, layout, "slot");
    const result = matchDeckList(layout, overlay.addsByGroup, "", { ids: [1], match: "any" }, state.cards);
    assert.deepEqual([...result.addIds], [matchingAdd.id]);
    assert.deepEqual([...result.cardIds], ["parked"]);
    assert.equal([...overlay.addsByGroup.values()].flat().length, 2);
  });

  test("cleared filters remove emphasis; a failed search leaves the original list intact", () => {
    const layout = layoutDeck(deckState({ cards: [card()], slots: [slot({ id: 10 })] }), "slot");
    const snapshot = structuredClone(layout);
    for (const query of ["", "   "]) {
      const result = matchDeckList(layout, new Map(), query, { ids: [], match: "all" });
      assert.equal(result.active, false);
      assert.equal(result.cardIds.size, 0);
      assert.equal(result.addIds.size, 0);
    }
    const result = matchDeckList(layout, new Map(), "no-match");
    assert.equal(result.active, true);
    assert.equal(result.cardIds.size, 0);
    assert.deepEqual(layout, snapshot);
  });
});
