import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { filterDeckList } from "../../web/src/domain/deckSearch.ts";
import { layoutDeck } from "../../web/src/domain/grouping.ts";
import { costCaption, costOf, formatUsd, groupCostCaption } from "../../web/src/domain/price.ts";
import { card, deckState, slot } from "./fixtures.ts";

describe("formatUsd", () => {
  test("cents become dollars, always with two decimals", () => {
    assert.equal(formatUsd(0), "$0.00");
    assert.equal(formatUsd(5), "$0.05");
    assert.equal(formatUsd(41237), "$412.37");
    assert.equal(formatUsd(162623), "$1,626.23");
  });
});

describe("costCaption", () => {
  test("says which printing, how old the prices are, and what is left out", () => {
    assert.equal(
      costCaption({ unpriced: 0, as_of: "2026-10-11T09:05:44.341+00:00" }),
      "Every card at its cheapest printing. Scryfall prices of 2026-10-11.",
    );
    assert.match(costCaption({ unpriced: 1, as_of: null }), /\. 1 card has no price and is not counted\.$/);
    assert.match(costCaption({ unpriced: 3, as_of: null }), /3 cards have no price and are not counted\.$/);
  });
});

describe("what a group costs", () => {
  const cards = [
    card({ name: "Test Land B", type_line: "Basic Land — Test", quantity: 6, usd_cents: 8, slot_id: 10 }),
    card({ name: "Test Card A", type_line: "Creature — Test", usd_cents: 250, slot_id: 10 }),
    card({ name: "Test Card C", type_line: "Creature — Test", usd_cents: null }),
    card({ name: "Test Card D", type_line: "Instant", usd_cents: 1999 }),
    card({ name: "Test Maybe Z", type_line: "Instant", usd_cents: 5000, maybe: 1 }),
  ];
  const state = deckState({ slots: [slot({ id: 10, name: "Test Ramp" })], cards });
  const costs = (by: "slot" | "type") =>
    Object.fromEntries(layoutDeck(state, by).groups.map((g) => [g.title, g.cost]));

  test("costOf counts copies and leaves out, but reports, cards with no price", () => {
    assert.deepEqual(costOf(cards.slice(0, 3)), { usd_cents: 48 + 250, priced: 2, unpriced: 1 });
    assert.deepEqual(costOf([]), { usd_cents: 0, priced: 0, unpriced: 0 });
  });

  test("by type, each type group totals its own cards and the maybe list is outside all of them", () => {
    assert.deepEqual(costs("type"), {
      Creature: { usd_cents: 250, priced: 1, unpriced: 1 },
      Instant: { usd_cents: 1999, priced: 1, unpriced: 0 },
      Land: { usd_cents: 48, priced: 1, unpriced: 0 },
    });
  });

  test("by slot, each slot and the unfiled group total their own cards", () => {
    assert.deepEqual(costs("slot"), {
      "Test Ramp": { usd_cents: 298, priced: 2, unpriced: 0 },
      Unfiled: { usd_cents: 1999, priced: 1, unpriced: 1 },
    });
  });

  test("searching the deck hides rows without changing a group's total", () => {
    const { layout } = filterDeckList(layoutDeck(state, "type"), new Map(), "card a");
    assert.deepEqual(layout.groups.map((g) => [g.title, g.cards.length, g.cost.usd_cents]), [["Creature", 1, 250]]);
  });

  test("the caption says what was left out", () => {
    assert.equal(groupCostCaption({ usd_cents: 1, priced: 1, unpriced: 0 }), "This group, every card at its cheapest printing.");
    assert.match(groupCostCaption({ usd_cents: 1, priced: 1, unpriced: 2 }), /2 cards have no price and are not counted\.$/);
  });
});
