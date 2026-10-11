import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { filterDeckList } from "../../web/src/domain/deckSearch.ts";
import { layoutDeck } from "../../web/src/domain/grouping.ts";
import { reviewOverlay } from "../../web/src/domain/review.ts";
import { card, deckState, item, proposal, slot } from "./fixtures.ts";

describe("deck list search", () => {
  for (const groupBy of ["slot", "type"] as const) {
    test(`filters names in every section when grouped by ${groupBy}`, () => {
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
      const original = layoutDeck(state, groupBy);
      const snapshot = structuredClone(original);
      const { layout } = filterDeckList(original, new Map(), "  mAtCh  ");

      assert.deepEqual(layout.commanders.map((c) => c.name), ["Test Match Commander"]);
      assert.deepEqual(layout.companions.map((c) => c.name), ["Test Match Companion"]);
      assert.deepEqual(layout.maybe.map((c) => c.name), ["Test Match Maybe"]);
      assert.equal(layout.groups.length, 1, "groups with no matching rows are hidden");
      assert.deepEqual(layout.groups[0].cards.map((c) => c.name), ["Test Match Card"]);
      const group = original.groups.find((g) => g.key === layout.groups[0].key)!;
      assert.deepEqual({ ...layout.groups[0], cards: group.cards }, group, "counts and targets describe the full deck");
      assert.deepEqual(original, snapshot, "search never changes the underlying list");
    });
  }

  test("filters review additions and keeps a group containing only a matching proposed card", () => {
    const state = deckState({
      slots: [slot({ id: 10 }), slot({ id: 11 })],
      cards: [card({ name: "Test Other Card", slot_id: 10 })],
      proposals: [proposal([
        item({ name: "Test Match Proposed", slot_id: 11 }),
        item({ name: "Test Other Proposed", slot_id: 10 }),
      ])],
    });
    const original = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, original, "slot");
    const { layout, addsByGroup } = filterDeckList(original, overlay.addsByGroup, "match");
    assert.deepEqual(layout.groups.map((g) => g.key), ["slot-11"]);
    assert.equal(layout.groups[0].cards.length, 0);
    assert.deepEqual([...addsByGroup.keys()], ["slot-11"]);
    assert.equal(addsByGroup.get("slot-11")![0].card_name, "Test Match Proposed");
    assert.equal(overlay.addsByGroup.size, 2);
    assert.equal(filterDeckList(original, new Map(), "match").layout.groups.length, 0,
      "proposals do not show as matches outside review");
  });

  test("clearing the query restores all rows and empty groups; unmatched text returns no rows", () => {
    const original = layoutDeck(deckState({
      cards: [card()],
      slots: [slot({ id: 10 })],
    }), "slot");
    const adds = new Map();
    for (const query of ["", "   "]) {
      const result = filterDeckList(original, adds, query);
      assert.equal(result.layout, original);
      assert.equal(result.addsByGroup, adds);
    }
    assert.deepEqual(filterDeckList(original, adds, "no-match").layout, {
      commanders: [], companions: [], groups: [], maybe: [],
    });
  });
});

describe("deck list tag filters", () => {
  for (const groupBy of ["slot", "type"] as const) {
    test(`any/all filters cover every section and compose with name search (${groupBy})`, () => {
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
      const original = layoutDeck(state, groupBy);
      const snapshot = structuredClone(original);
      const any = filterDeckList(original, new Map(), "", { ids: [1, 2], match: "any" }).layout;
      assert.equal(any.commanders.length, 1);
      assert.equal(any.companions.length, 1);
      assert.equal(any.maybe.length, 1);
      assert.deepEqual(any.groups.flatMap((g) => g.cards.map((c) => c.name)).sort(), ["Test Both", "Test One"]);
      const all = filterDeckList(original, new Map(), "", { ids: [1, 2], match: "all" }).layout;
      assert.equal(all.commanders.length, 0);
      assert.equal(all.companions.length, 1);
      assert.equal(all.maybe.length, 0);
      assert.deepEqual(all.groups.flatMap((g) => g.cards.map((c) => c.name)), ["Test Both"]);
      const search = filterDeckList(original, new Map(), "  bOtH  ", { ids: [1], match: "any" }).layout;
      assert.equal(search.companions.length, 0);
      assert.deepEqual(search.groups.flatMap((g) => g.cards.map((c) => c.name)), ["Test Both"]);
      const group = original.groups.find((g) => g.key === all.groups[0].key)!;
      assert.deepEqual({ ...all.groups[0], cards: group.cards }, group, "full deck totals, targets and costs stay unchanged");
      assert.deepEqual(original, snapshot);
      assert.equal(filterDeckList(original, new Map(), "", { ids: [], match: "all" }).layout, original);
      assert.equal(filterDeckList(original, new Map(), "", { ids: [99], match: "any" }).layout.groups.length, 0);
    });
  }

  test("review additions match existing parked tags; new untagged proposals stay hidden", () => {
    const parked = card({ oracle_id: "parked", name: "Test Parked", maybe: 1, tag_ids: [1] });
    const state = deckState({
      cards: [parked],
      proposals: [proposal([item({ oracle_id: "parked", name: "Test Parked" }), item({ oracle_id: "new", name: "Test New" })])],
    });
    const original = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, original, "slot");
    const result = filterDeckList(original, overlay.addsByGroup, "", { ids: [1], match: "any" }, state.cards);
    assert.deepEqual([...result.addsByGroup.values()].flat().map((i) => i.oracle_id), ["parked"]);
    assert.deepEqual(result.layout.maybe, [parked]);
    assert.equal([...overlay.addsByGroup.values()].flat().length, 2);
  });
});
