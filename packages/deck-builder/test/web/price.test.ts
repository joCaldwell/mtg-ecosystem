import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { matchDeckList } from "../../web/src/domain/deckSearch.ts";
import { layoutDeck } from "../../web/src/domain/grouping.ts";
import { costCaption, costOf, formatPrice, groupCostCaption } from "../../web/src/domain/price.ts";
import { card, deckState, slot } from "./fixtures.ts";

describe("formatPrice", () => {
  test("cents become dollars, always with two decimals", () => {
    assert.equal(formatPrice(0), "$0.00");
    assert.equal(formatPrice(5), "$0.05");
    assert.equal(formatPrice(5, "EUR"), "€0.05");
    assert.equal(formatPrice(162623, "EUR"), "€1,626.23");
    assert.equal(formatPrice(41237), "$412.37");
    assert.equal(formatPrice(162623), "$1,626.23");
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
    card({ name: "Test Land B", type_line: "Basic Land — Test", quantity: 6, cents: 8, slot_id: 10 }),
    card({ name: "Test Card A", type_line: "Creature — Test", cents: 250, slot_id: 10 }),
    card({ name: "Test Card C", type_line: "Creature — Test", cents: null }),
    card({ name: "Test Card D", type_line: "Instant", cents: 1999 }),
    card({ name: "Test Maybe Z", type_line: "Instant", cents: 5000, maybe: 1 }),
  ];
  const state = deckState({ slots: [slot({ id: 10, name: "Test Ramp" })], cards });
  const costs = (by: "slot" | "type") =>
    Object.fromEntries(layoutDeck(state, by).groups.map((g) => [g.title, g.cost]));

  test("costOf counts copies and leaves out, but reports, cards with no price", () => {
    assert.deepEqual(costOf(cards.slice(0, 3)), { cents: 48 + 250, priced: 2, unpriced: 1, currency: "USD" });
    assert.deepEqual(costOf([]), { cents: 0, priced: 0, unpriced: 0, currency: undefined });
  });

  test("by type, each type group totals its own cards and the maybe list is outside all of them", () => {
    assert.deepEqual(costs("type"), {
      Creature: { cents: 250, priced: 1, unpriced: 1, currency: "USD" },
      Instant: { cents: 1999, priced: 1, unpriced: 0, currency: "USD" },
      Land: { cents: 48, priced: 1, unpriced: 0, currency: "USD" },
    });
  });

  test("by slot, each slot and the unfiled group total their own cards", () => {
    assert.deepEqual(costs("slot"), {
      "Test Ramp": { cents: 298, priced: 2, unpriced: 0, currency: "USD" },
      Unfiled: { cents: 1999, priced: 1, unpriced: 1, currency: "USD" },
    });
  });

  test("searching the deck identifies matches without changing rows or group costs", () => {
    const layout = layoutDeck(state, "type");
    const snapshot = structuredClone(layout);
    const matches = matchDeckList(layout, new Map(), "card a");
    assert.equal(matches.cardIds.size, 1);
    assert.deepEqual(layout, snapshot);
  });

  test("the caption says what was left out", () => {
    assert.equal(groupCostCaption({ cents: 1, priced: 1, unpriced: 0, currency: "USD" }), "This group, every card at its cheapest printing.");
    assert.match(groupCostCaption({ cents: 1, priced: 1, unpriced: 2, currency: "USD" }), /2 cards have no price and are not counted\.$/);
  });
});
