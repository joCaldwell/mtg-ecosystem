import { test, describe } from "node:test";
import assert from "node:assert/strict";
import {
  CARD_ORDER,
  TYPE_GROUPS,
  byCurve,
  byName,
  byPrice,
  isLand,
  primaryType,
  ptString,
  quantityOf,
} from "../../web/src/domain/cards.ts";

describe("primaryType", () => {
  test("a card with several types files under the first group that matches", () => {
    assert.equal(primaryType("Artifact Creature — Test"), "Creature");
    assert.equal(primaryType("Legendary Enchantment Creature — Test"), "Creature");
    assert.equal(primaryType("Instant"), "Instant");
    assert.equal(primaryType("Enchantment — Aura"), "Enchantment");
  });

  test("land wins outright", () => {
    assert.equal(primaryType("Artifact Land"), "Land");
    assert.equal(primaryType("Creature — Test Land"), "Land");
    assert.equal(primaryType("Basic Land — Test"), "Land");
  });

  test("anything else is Other", () => {
    assert.equal(primaryType("Kindred"), "Other");
    assert.equal(primaryType(""), "Other");
  });

  test("matches whole words only", () => {
    assert.equal(primaryType("Landscape"), "Other");
    assert.equal(isLand({ type_line: "Landscape" }), false);
    assert.equal(isLand({ type_line: "Land — Test" }), true);
  });

  test("TYPE_GROUPS is the display order", () => {
    assert.deepEqual([...TYPE_GROUPS], [
      "Creature", "Planeswalker", "Battle", "Instant", "Sorcery", "Artifact", "Enchantment", "Land",
    ]);
  });
});

describe("ptString", () => {
  test("power/toughness, loyalty in brackets, or nothing", () => {
    assert.equal(ptString({ power: "2", toughness: "5", loyalty: null }), "2/5");
    assert.equal(ptString({ power: "*", toughness: "1+*", loyalty: null }), "*/1+*");
    assert.equal(ptString({ power: null, toughness: null, loyalty: "4" }), "[4]");
    assert.equal(ptString({ power: null, toughness: null, loyalty: null }), "");
    assert.equal(ptString({ power: "2", toughness: null, loyalty: null }), "");
  });
});

describe("byCurve and quantityOf", () => {
  test("mana value first, then name", () => {
    const cards = [
      { cmc: 3, name: "Test Card C" },
      { cmc: 1, name: "Test Card B" },
      { cmc: 1, name: "Test Card A" },
    ];
    assert.deepEqual([...cards].sort(byCurve).map((c) => c.name), ["Test Card A", "Test Card B", "Test Card C"]);
  });

  test("quantityOf counts copies", () => {
    assert.equal(quantityOf([{ quantity: 6 }, { quantity: 1 }]), 7);
    assert.equal(quantityOf([]), 0);
  });
});

describe("byName and byPrice", () => {
  const cards = [
    { cmc: 1, name: "Test Card B", cents: 500 },
    { cmc: 5, name: "Test Card D", cents: null },
    { cmc: 3, name: "Test Card A", cents: 500 },
    { cmc: 2, name: "Test Card C", cents: 1999 },
    { cmc: 0, name: "Test Card E" },
  ];
  const order = (cmp: (a: (typeof cards)[number], b: (typeof cards)[number]) => number) =>
    [...cards].sort(cmp).map((c) => c.name.slice(-1)).join("");

  test("byName is A to Z", () => {
    assert.equal(order(byName), "ABCDE");
  });

  test("byPrice is most expensive first, ties by name, and cards with no price last", () => {
    assert.equal(order(byPrice), "CABDE");
  });

  test("CARD_ORDER names the three orders", () => {
    assert.equal(order(CARD_ORDER.curve), "EBCAD");
    assert.equal(order(CARD_ORDER.name), "ABCDE");
    assert.equal(order(CARD_ORDER.price), "CABDE");
  });
});
