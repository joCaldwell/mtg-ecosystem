import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { formatTarget, layoutDeck } from "../../web/src/domain/grouping.ts";
import { card, deckState, item, proposal, slot } from "./fixtures.ts";

describe("formatTarget", () => {
  test("every shape", () => {
    assert.equal(formatTarget(8, 10), "8–10");
    assert.equal(formatTarget(36, 36), "36");
    assert.equal(formatTarget(3, null), "3+");
    assert.equal(formatTarget(null, 5), "up to 5");
    assert.equal(formatTarget(null, null), null);
    assert.equal(formatTarget(0, 0), "0");
  });
});

describe("layoutDeck, by slot", () => {
  test("secondary contributors stay separate and counts use server primary targets", () => {
    const state = deckState({
      slots: [slot({ id: 10 }), slot({ id: 11, target_min: 2 })],
      cards: [card({ name: "Test Dual Purpose", slot_id: 10, secondary_slot_ids: [11], quantity: 3 }),
        card({ name: "Test Parked Purpose", slot_id: 10, secondary_slot_ids: [11], maybe: 1 })],
    });
    const [primary, secondary] = layoutDeck(state, "slot").groups;
    assert.equal(primary.cards.length, 1);
    assert.equal(secondary.cards.length, 0);
    assert.equal(secondary.secondaryCards!.length, 1);
    assert.equal(secondary.secondaryCount, 3);
    assert.equal(secondary.count, 0);
    assert.equal(secondary.status, "under");
    assert.equal(secondary.delta, -2);
  });
  test("one group per slot in state order, empty slots kept, with the server's numbers", () => {
    const ramp = slot({ id: 10, name: "Test Ramp", target_min: 8, target_max: 10 });
    const draw = slot({ id: 11, name: "Test Draw", target_min: 5, target_max: null });
    const state = deckState({
      slots: [ramp, draw],
      cards: [card({ name: "Test Card A", slot_id: 10 }), card({ name: "Test Card B", slot_id: 10 })],
    });
    const layout = layoutDeck(state, "slot");
    assert.deepEqual(layout.groups.map((g) => g.key), ["slot-10", "slot-11"]);
    const [g10, g11] = layout.groups;
    assert.equal(g10.title, "Test Ramp");
    assert.equal(g10.slotId, 10);
    assert.equal(g10.target, "8–10");
    assert.equal(g10.status, "under");
    assert.equal(g10.delta, -6);
    assert.equal(g10.cards.length, 2);
    assert.equal(g11.cards.length, 0, "an empty slot is kept so its unmet target shows");
    assert.equal(g11.target, "5+");
    assert.equal(g11.status, "under");
  });

  test("count, status and delta come from the server, never a recount", () => {
    const s = slot({ id: 10, target_min: 1, target_max: 2 });
    const base = deckState({ slots: [s], cards: [card({ slot_id: 10 })] });
    const state = deckState({
      slots: [s],
      cards: [card({ slot_id: 10 })],
      computed: {
        ...base.computed,
        slot_deltas: [{ slot_id: 10, name: s.name, count: 99, primary_count: 99, secondary_count: 12, coverage_count: 111, target_min: 1, target_max: 2, status: "over", delta: 97 }],
        unslotted_count: 42,
      },
    });
    const [group] = layoutDeck(state, "slot").groups;
    assert.equal(group.count, 99);
    assert.equal(group.status, "over");
    assert.equal(group.delta, 97);
  });

  test("the unfiled group appears when it has cards, and takes the server's count", () => {
    const state = deckState({ slots: [slot({ id: 10 })], cards: [card({ slot_id: null, quantity: 3 })] });
    const groups = layoutDeck(state, "slot").groups;
    const unfiled = groups[groups.length - 1];
    assert.equal(unfiled.key, "unslotted");
    assert.equal(unfiled.title, "Unfiled");
    assert.equal(unfiled.slotId, null);
    assert.equal(unfiled.count, 3);
    assert.equal(unfiled.target, null);
    assert.equal(unfiled.status, "untargeted");
  });

  test("the unfiled group is absent when empty and nothing pending would land there", () => {
    const state = deckState({
      slots: [slot({ id: 10 })],
      cards: [card({ slot_id: 10 })],
      proposals: [proposal([item({ action: "add", slot_id: 10, name: "Test Card B" })])],
    });
    assert.deepEqual(layoutDeck(state, "slot").groups.map((g) => g.key), ["slot-10"]);
  });

  test("the unfiled group appears for a pending add with no slot", () => {
    const state = deckState({
      slots: [slot({ id: 10 })],
      cards: [card({ slot_id: 10 })],
      proposals: [proposal([item({ action: "add", slot_id: null, name: "Test Card B" })])],
    });
    assert.deepEqual(layoutDeck(state, "slot").groups.map((g) => g.key), ["slot-10", "unslotted"]);
  });

  test("a resolved add does not summon the unfiled group", () => {
    const state = deckState({
      slots: [],
      proposals: [proposal([item({ action: "add", status: "accepted" })])],
    });
    assert.deepEqual(layoutDeck(state, "slot").groups, []);
  });

  test("cards sort by curve; commanders, companions and maybe are kept out of the groups", () => {
    const state = deckState({
      cards: [
        card({ name: "Test Card B", cmc: 3 }),
        card({ name: "Test Card A", cmc: 3 }),
        card({ name: "Test Card C", cmc: 1 }),
        card({ name: "Test Commander", role: "commander" }),
        card({ name: "Test Companion", role: "companion" }),
        card({ name: "Test Maybe Z", maybe: 1 }),
        card({ name: "Test Maybe Y", maybe: 1, role: "commander" }),
      ],
    });
    const layout = layoutDeck(state, "slot");
    assert.deepEqual(layout.groups[0].cards.map((c) => c.name), ["Test Card C", "Test Card A", "Test Card B"]);
    assert.deepEqual(layout.commanders.map((c) => c.name), ["Test Commander"]);
    assert.deepEqual(layout.companions.map((c) => c.name), ["Test Companion"]);
    assert.deepEqual(layout.maybe.map((c) => c.name), ["Test Maybe Y", "Test Maybe Z"]);
  });
});

describe("layoutDeck, by type", () => {
  test("groups in TYPE_GROUPS order then Other, quantity-weighted, untargeted", () => {
    const state = deckState({
      slots: [slot({ id: 10, target_min: 5, target_max: 5 })],
      cards: [
        card({ name: "Test Land B", type_line: "Basic Land — Test", quantity: 6, slot_id: 10 }),
        card({ name: "Test Card A", type_line: "Artifact Creature — Test" }),
        card({ name: "Test Card C", type_line: "Kindred" }),
        card({ name: "Test Card D", type_line: "Instant" }),
      ],
    });
    const groups = layoutDeck(state, "type").groups;
    assert.deepEqual(groups.map((g) => g.key), ["type-Creature", "type-Instant", "type-Land", "type-Other"]);
    const land = groups.find((g) => g.title === "Land")!;
    assert.equal(land.count, 6, "six copies on one row read 6, not 1");
    assert.equal(land.cards.length, 1);
    for (const g of groups) {
      assert.equal(g.slotId, undefined);
      assert.equal(g.target, null);
      assert.equal(g.status, "untargeted");
      assert.equal(g.delta, 0);
    }
  });

  test("a group appears for a pending add of that type, even when empty", () => {
    const state = deckState({
      cards: [card({ type_line: "Creature — Test" })],
      proposals: [proposal([item({ action: "add", type_line: "Sorcery", name: "Test Card B" })])],
    });
    const groups = layoutDeck(state, "type").groups;
    assert.deepEqual(groups.map((g) => g.key), ["type-Creature", "type-Sorcery"]);
    assert.equal(groups[1].cards.length, 0);
    assert.equal(groups[1].count, 0);
  });

  test("an empty deck has no groups", () => {
    assert.deepEqual(layoutDeck(deckState(), "type").groups, []);
  });
});

describe("layoutDeck, card order within a group", () => {
  const state = deckState({
    cards: [
      card({ name: "Test Card B", cmc: 1, usd_cents: 500 }),
      card({ name: "Test Card A", cmc: 3, usd_cents: 50 }),
      card({ name: "Test Card C", cmc: 2, usd_cents: null }),
      card({ name: "Test Maybe Z", usd_cents: 9000, maybe: 1 }),
      card({ name: "Test Maybe Y", usd_cents: 10, maybe: 1 }),
    ],
  });
  const rows = (layout: ReturnType<typeof layoutDeck>) => layout.groups[0].cards.map((c) => c.name.slice(-1)).join("");

  test("mana value unless told otherwise", () => {
    assert.equal(rows(layoutDeck(state, "slot")), "BCA");
    assert.equal(rows(layoutDeck(state, "type", "curve")), "BCA");
  });

  test("by name or by price, in either grouping, without changing a group's count or cost", () => {
    assert.equal(rows(layoutDeck(state, "slot", "name")), "ABC");
    assert.equal(rows(layoutDeck(state, "type", "name")), "ABC");
    assert.equal(rows(layoutDeck(state, "slot", "price")), "BAC");
    assert.equal(rows(layoutDeck(state, "type", "price")), "BAC");
    assert.deepEqual(layoutDeck(state, "type", "price").groups[0].cost, layoutDeck(state, "type").groups[0].cost);
  });

  test("the maybe list stays in name order except when sorting by price", () => {
    assert.deepEqual(layoutDeck(state, "slot", "name").maybe.map((c) => c.name), ["Test Maybe Y", "Test Maybe Z"]);
    assert.deepEqual(layoutDeck(state, "slot", "price").maybe.map((c) => c.name), ["Test Maybe Z", "Test Maybe Y"]);
  });
});
