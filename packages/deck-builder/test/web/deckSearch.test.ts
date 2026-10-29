import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { filterGroup, matchCount, matchDeckList, noMatchesLine } from "../../web/src/domain/deckSearch.ts";
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
      assert.equal(matchCount(result), 4, "each matching card once");
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
      const notAny = matchDeckList(layout, new Map(), "", { ids: [1, 2], match: "any", inverted: true });
      assert.deepEqual([...notAny.cardIds], ["oid-Test Untagged"]);
      const notAll = matchDeckList(layout, new Map(), "", { ids: [1, 2], match: "all", inverted: true });
      assert.deepEqual([...notAll.cardIds].sort(), ["oid-Test Commander", "oid-Test One", "oid-Test Two", "oid-Test Untagged"]);
      const notOne = matchDeckList(layout, new Map(), "", { ids: [1], match: "any", inverted: true });
      assert.deepEqual([...notOne.cardIds].sort(), ["oid-Test Two", "oid-Test Untagged"]);
      const invertedSearch = matchDeckList(layout, new Map(), "  tWo  ", { ids: [1], match: "any", inverted: true });
      assert.deepEqual([...invertedSearch.cardIds], ["oid-Test Two"], "inversion applies only to tags, not name search");
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
    assert.equal(matchCount(result), 1);
    assert.deepEqual(overlay, snapshot, "the complete review overlay is preserved");
    assert.equal(matchDeckList(layout, new Map(), "match").addIds.size, 0,
      "proposals are not considered outside review");
  });

  test("proposed adds inherit parked tags; untagged adds are nonmatches", () => {
    const parked = card({ oracle_id: "parked", name: "Test Parked", maybe: 1, tag_ids: [1] });
    const matchingAdd = item({ oracle_id: "parked", name: "Test Parked" });
    const newAdd = item({ oracle_id: "new", name: "Test New" });
    const state = deckState({
      cards: [parked],
      proposals: [proposal([matchingAdd, newAdd])],
    });
    const layout = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, layout, "slot");
    const result = matchDeckList(layout, overlay.addsByGroup, "", { ids: [1], match: "any" }, state.cards);
    assert.deepEqual([...result.addIds], [matchingAdd.id]);
    assert.deepEqual([...result.cardIds], ["parked"]);
    const inverted = matchDeckList(layout, overlay.addsByGroup, "", { ids: [1], match: "any", inverted: true }, state.cards);
    assert.deepEqual([...inverted.addIds], [newAdd.id]);
    assert.equal(inverted.cardIds.size, 0);
    assert.equal([...overlay.addsByGroup.values()].flat().length, 2);
  });

  test("cleared filters remove emphasis; a failed search leaves the original list intact", () => {
    const layout = layoutDeck(deckState({ cards: [card()], slots: [slot({ id: 10 })] }), "slot");
    const snapshot = structuredClone(layout);
    for (const query of ["", "   "]) {
      const result = matchDeckList(layout, new Map(), query, { ids: [], match: "all", inverted: true });
      assert.equal(result.active, false);
      assert.equal(result.cardIds.size, 0);
      assert.equal(result.addIds.size, 0);
    }
    const searchOnly = matchDeckList(layout, new Map(), "test", { ids: [], match: "any", inverted: true });
    assert.deepEqual([...searchOnly.cardIds], [layout.groups.flatMap((group) => group.cards)[0].oracle_id]);
    const result = matchDeckList(layout, new Map(), "no-match");
    assert.equal(result.active, true);
    assert.equal(result.cardIds.size, 0);
    assert.deepEqual(layout, snapshot);
  });

  test("a filtered group draws only its matching rows and counts their copies", () => {
    const state = deckState({
      slots: [slot({ id: 10, target_min: 8 }), slot({ id: 11 })],
      cards: [
        card({ name: "Test Match One", slot_id: 10, quantity: 2 }),
        card({ name: "Test Other One", slot_id: 10 }),
        card({ name: "Test Match Two", slot_id: 10 }),
        card({ name: "Test Other Land", slot_id: 11, type_line: "Land" }),
      ],
    });
    const layout = layoutDeck(state, "slot");
    const matches = matchDeckList(layout, new Map(), "match");
    const [ramp, lands] = layout.groups.filter((group) => group.cards.length);
    const shown = filterGroup({ cards: ramp.cards, adds: [] }, matches);
    assert.ok(shown);
    assert.deepEqual(shown.cards.map((c) => c.name), ["Test Match One", "Test Match Two"], "drawing order is kept");
    assert.equal(shown.matched, 3, "copies, like the header's group count");
    assert.equal(ramp.count, 4, "the group's own count still describes the whole group");
    assert.equal(filterGroup({ cards: lands.cards, adds: [] }, matches), null, "a group with no matches is hidden");
    assert.equal(filterGroup({ cards: [], adds: [] }, matches), null, "so is an empty group");
  });

  test("a filtered review group keeps matching proposed adds and cuts in place", () => {
    const matchingAdd = item({ name: "Test Match Proposed", slot_id: 10 });
    const otherAdd = item({ name: "Test Other Proposed", slot_id: 10 });
    const state = deckState({
      slots: [slot({ id: 10 })],
      cards: [card({ name: "Test Match Kept", slot_id: 10 }), card({ name: "Test Other Kept", slot_id: 10 })],
      proposals: [proposal([matchingAdd, otherAdd])],
    });
    const layout = layoutDeck(state, "slot");
    const overlay = reviewOverlay(state, layout, "slot");
    const matches = matchDeckList(layout, overlay.addsByGroup, "match");
    const group = layout.groups.find((g) => g.cards.length)!;
    const shown = filterGroup({ cards: group.cards, adds: overlay.addsByGroup.get(group.key) ?? [] }, matches);
    assert.deepEqual(shown?.cards.map((c) => c.name), ["Test Match Kept"]);
    assert.deepEqual(shown?.adds.map((a) => a.id), [matchingAdd.id]);
    const addsOnly = matchDeckList(layout, overlay.addsByGroup, "proposed");
    const onlyAdd = filterGroup({ cards: group.cards, adds: overlay.addsByGroup.get(group.key) ?? [] }, addsOnly);
    assert.equal(onlyAdd?.cards.length, 0);
    assert.equal(onlyAdd?.adds.length, 2, "a group whose only matches are proposed adds still shows");
    assert.equal(onlyAdd?.matched, 2, "adds count: the list shows the deck as it would be");
    const kept = matchDeckList(layout, overlay.addsByGroup, "kept");
    const cutKept = group.cards.find((c) => c.name === "Test Match Kept")!.oracle_id;
    assert.equal(filterGroup({ cards: group.cards, adds: [] }, kept, new Set([cutKept]))?.matched, 1, "a matching cut is drawn but not counted");
  });

  test("the empty-result line names the query, or the tags when there is none", () => {
    assert.equal(noMatchesLine("  zzz ", false), "No cards in this deck match “zzz”.");
    assert.equal(noMatchesLine("zzz", true), "No cards in this deck match “zzz” with these tags.");
    assert.equal(noMatchesLine("", true), "No cards in this deck match these tags.");
  });
});
