import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { layoutDeck } from "../../web/src/domain/grouping.ts";
import { proposalQueue, reviewOverlay, reviewUnits } from "../../web/src/domain/review.ts";
import { firstUnitOf, proposalParam } from "../../web/src/domain/reviewStart.ts";
import { card, deckState, item, proposal, slot } from "./fixtures.ts";

describe("reviewUnits", () => {
  test("a lone item is its own unit; the kind follows its action", () => {
    const add = item({ action: "add", name: "Test Card A" });
    const cut = item({ action: "cut", name: "Test Card B" });
    const p = proposal([add, cut]);
    const units = reviewUnits([p]);
    assert.deepEqual(units.map((u) => u.kind), ["add", "cut"]);
    assert.equal(units[0].key, `${p.id}:item-${p.items[0].id}`);
    assert.deepEqual(units[1].itemIds, [p.items[1].id]);
  });

  test("items sharing a group_id are one unit; one add with one cut is a swap, adds first", () => {
    const p = proposal([
      item({ action: "cut", name: "Test Card B", group_id: "g1" }),
      item({ action: "add", name: "Test Card A", group_id: "g1" }),
    ]);
    const units = reviewUnits([p]);
    assert.equal(units.length, 1);
    assert.equal(units[0].kind, "swap");
    assert.equal(units[0].key, `${p.id}:g1`);
    assert.deepEqual(units[0].items.map((i) => i.action), ["add", "cut"]);
    assert.deepEqual(units[0].itemIds, units[0].items.map((i) => i.id));
  });

  test("any other multi-item group is a group", () => {
    const three = proposal([
      item({ action: "add", name: "Test Card A", group_id: "g" }),
      item({ action: "add", name: "Test Card B", group_id: "g" }),
      item({ action: "cut", name: "Test Card C", group_id: "g" }),
    ]);
    assert.equal(reviewUnits([three])[0].kind, "group");
    const twoAdds = proposal([
      item({ action: "add", name: "Test Card A", group_id: "g" }),
      item({ action: "add", name: "Test Card B", group_id: "g" }),
    ]);
    assert.equal(reviewUnits([twoAdds])[0].kind, "group");
  });

  test("the same group_id in two proposals is two units", () => {
    const a = proposal([item({ name: "Test Card A", group_id: "g" })]);
    const b = proposal([item({ name: "Test Card B", group_id: "g" })]);
    assert.equal(reviewUnits([a, b]).length, 2);
    assert.notEqual(reviewUnits([a, b])[0].key, reviewUnits([a, b])[1].key);
  });

  test("only pending items count, and proposals read oldest first", () => {
    const newer = proposal([item({ name: "Test Card A" })], { id: 50 });
    const older = proposal(
      [item({ name: "Test Card B", status: "accepted" }), item({ name: "Test Card C" })],
      { id: 20 },
    );
    const done = proposal([item({ name: "Test Card D", status: "rejected" })], { id: 10 });
    // The inbox arrives newest first.
    const units = reviewUnits([newer, older, done]);
    assert.deepEqual(units.map((u) => u.proposalId), [20, 50]);
    assert.deepEqual(units[0].items.map((i) => i.card_name), ["Test Card C"]);
  });

  test("a group whose other items are already ruled is judged by what is left", () => {
    const p = proposal([
      item({ action: "add", name: "Test Card A", group_id: "g", status: "accepted" }),
      item({ action: "cut", name: "Test Card B", group_id: "g" }),
    ]);
    const [unit] = reviewUnits([p]);
    assert.equal(unit.kind, "cut");
  });

  test("units appear in first-appearance order", () => {
    const p = proposal([
      item({ name: "Test Card A", group_id: "g" }),
      item({ name: "Test Card B" }),
      item({ name: "Test Card C", action: "cut", group_id: "g" }),
    ]);
    assert.deepEqual(reviewUnits([p]).map((u) => u.kind), ["swap", "add"]);
  });
});

describe("reviewOverlay", () => {
  test("slot mode uses the server's by_slot, not a recount", () => {
    const s = slot({ id: 10 });
    const state = deckState({
      slots: [s],
      cards: [card({ slot_id: 10 })],
      proposals: [proposal([item({ action: "add", slot_id: 10, name: "Test Card B" })])],
    });
    state.computed.pending.by_slot = { 10: 5 };
    state.computed.pending.projected_count = 77;
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.equal(overlay.projected.get("slot-10"), 1 + 5);
    assert.equal(overlay.projectedTotal, 77);
    assert.equal(overlay.addsByGroup.get("slot-10")?.length, 1);
  });

  test("only groups whose count would change are projected", () => {
    const state = deckState({
      slots: [slot({ id: 10 }), slot({ id: 11 })],
      cards: [card({ slot_id: 10 }), card({ name: "Test Card B", slot_id: 11 })],
      proposals: [proposal([item({ action: "add", slot_id: 10, name: "Test Card C" })])],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.deepEqual([...overlay.projected], [["slot-10", 2]]);
  });

  test("a cut in a slot lowers its projection and is listed in cuts", () => {
    const state = deckState({
      slots: [slot({ id: 10 })],
      cards: [card({ name: "Test Card A", slot_id: 10, quantity: 2 })],
      proposals: [proposal([item({ action: "cut", name: "Test Card A" })])],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.deepEqual([...overlay.cuts], ["oid-Test Card A"]);
    assert.equal(overlay.projected.get("slot-10"), 1);
  });

  test("the unfiled group is derived from the pending items", () => {
    const state = deckState({
      slots: [slot({ id: 10 })],
      cards: [card({ name: "Test Card A", slot_id: null }), card({ name: "Test Card B", slot_id: null })],
      proposals: [
        proposal([
          item({ action: "add", slot_id: null, name: "Test Card C" }),
          item({ action: "add", slot_id: null, name: "Test Card D" }),
          item({ action: "cut", name: "Test Card A" }),
        ]),
      ],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.equal(overlay.addsByGroup.get("unslotted")?.length, 2);
    assert.equal(overlay.projected.get("unslotted"), 2 + 2 - 1);
    assert.equal(overlay.projected.has("slot-10"), false);
  });

  test("an add naming a slot that no longer exists goes to the unfiled group", () => {
    const state = deckState({
      slots: [],
      proposals: [proposal([item({ action: "add", slot_id: 999, name: "Test Card A" })])],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.equal(overlay.addsByGroup.get("unslotted")?.length, 1);
    assert.equal(overlay.projected.get("unslotted"), 1);
  });

  test("type mode routes adds by primary type and derives every projection", () => {
    const state = deckState({
      cards: [
        card({ name: "Test Card A", type_line: "Creature — Test" }),
        card({ name: "Test Land B", type_line: "Basic Land — Test", quantity: 6 }),
      ],
      proposals: [
        proposal([
          item({ action: "add", type_line: "Artifact Creature — Test", name: "Test Card C" }),
          item({ action: "add", type_line: "Artifact Land", name: "Test Land D" }),
          item({ action: "cut", name: "Test Land B" }),
        ]),
      ],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "type"), "type");
    assert.equal(overlay.addsByGroup.get("type-Creature")?.length, 1);
    assert.equal(overlay.addsByGroup.get("type-Land")?.length, 1);
    assert.equal(overlay.projected.get("type-Creature"), 2);
    // One copy of the cut land goes; one land is added: 6 + 1 - 1.
    assert.equal(overlay.projected.has("type-Land"), false);
  });

  test("type mode: a cut that nothing offsets lowers the count by one copy", () => {
    const state = deckState({
      cards: [card({ name: "Test Land B", type_line: "Basic Land — Test", quantity: 6 })],
      proposals: [proposal([item({ action: "cut", name: "Test Land B" })])],
    });
    const overlay = reviewOverlay(state, layoutDeck(state, "type"), "type");
    assert.equal(overlay.projected.get("type-Land"), 5);
  });

  test("nothing pending, nothing projected", () => {
    const state = deckState({ slots: [slot({ id: 10 })], cards: [card({ slot_id: 10 })] });
    const overlay = reviewOverlay(state, layoutDeck(state, "slot"), "slot");
    assert.equal(overlay.cuts.size, 0);
    assert.equal(overlay.addsByGroup.size, 0);
    assert.equal(overlay.projected.size, 0);
    assert.equal(overlay.projectedTotal, 1);
  });

  test("resolved items are ignored", () => {
    const state = deckState({
      cards: [card({ name: "Test Card A" })],
      proposals: [proposal([item({ action: "cut", name: "Test Card A", status: "accepted" })])],
    });
    assert.equal(reviewOverlay(state, layoutDeck(state, "type"), "type").cuts.size, 0);
  });
});

describe("firstUnitOf", () => {
  test("a review handed a proposal starts at that proposal's first open decision", () => {
    const older = proposal([item({ name: "Test Card A" }), item({ name: "Test Card B" })], { id: 20 });
    const newer = proposal([item({ name: "Test Card C", group_id: "g" }), item({ name: "Test Card D", action: "cut", group_id: "g" })], { id: 50 });
    const units = reviewUnits([newer, older]);
    assert.equal(firstUnitOf(units, 50), 2);
    assert.equal(units[firstUnitOf(units, 50)].proposalId, 50);
    assert.equal(firstUnitOf(units, 20), 0);
  });

  test("an unknown or absent proposal starts at the first decision", () => {
    const units = reviewUnits([proposal([item({ name: "Test Card A" })], { id: 20 })]);
    assert.equal(firstUnitOf(units, 99), 0);
    assert.equal(firstUnitOf(units, undefined), 0);
    assert.equal(firstUnitOf([], 20), 0);
  });
});

describe("proposalParam", () => {
  test("reads the proposal a review link names, and only a positive whole number", () => {
    assert.equal(proposalParam("?proposal=12"), 12);
    assert.equal(proposalParam(""), undefined);
    for (const bad of ["0", "-1", "1.5", "x", ""]) assert.equal(proposalParam(`?proposal=${bad}`), undefined, bad);
  });
});

test("the review queue numbers proposals by position, not stored id, and counts their changes", () => {
  const first = proposal([item({ action: "add", name: "Test Card A" }), item({ action: "cut", name: "Test Card B" })], { id: 4 });
  const second = proposal([item({ action: "add", name: "Test Card C", group_id: "g" }), item({ action: "cut", name: "Test Card D", group_id: "g" })], { id: 5 });
  const queue = proposalQueue(reviewUnits([second, first]));
  assert.deepEqual(queue.map((q) => [q.proposalId, `${q.position} of ${q.of}`, q.firstUnit, q.changes]), [[4, "1 of 2", 0, 2], [5, "2 of 2", 2, 2]]);
  assert.deepEqual(proposalQueue([]), []);
});
