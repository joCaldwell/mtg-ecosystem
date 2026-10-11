import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parseCondition, parseFilter, parseObjectRef } from "../src/parser/refs.ts";

const n = (value: number) => ({ amount: "fixed", value });
const you = { player: "you" };
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
function complete<T>(parser: (c: Cursor) => T | null, text: string) {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.ok(c.done(), `${text}: ${c.errorMessage()}`);
  return result;
}

describe("numeric expressions retain operands and references", () => {
  it("keeps calculations around counts and quantities", () => {
    assert.deepEqual(complete(parseAmount, "the number of cards in your hand minus 4"), {
      amount: "minus", a: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } }, b: n(4),
    });
    assert.deepEqual(complete(parseAmount, "that number plus 1"), { amount: "plus", a: { amount: "that-number" }, b: n(1) });
    // Heartwarming Redemption / Chandra, Flamecaller, checked in source data.
    assert.deepEqual(parsed("Discard all the cards in your hand, then draw that many cards plus one."), {
      kind: "spell", effects: [{ steps: [
        { effect: "discard", who: you, what: { ref: "all", filter: { cls: "card", zone: { zone: "hand", owner: "your" } } } },
        { effect: "draw", who: you, amount: { amount: "plus", a: { amount: "that-much" }, b: n(1) } },
      ] }],
    });
    assert.equal(parseOracleText("Draw that many cards plus.").ok, false);
    assert.equal(parseOracleText("Draw that many cards minus an unknown quantity.").ok, false);
  });
  it("retains both sides of comparisons and negation of a control condition", () => {
    assert.deepEqual(complete(parseCondition, "that creature's power is greater than this creature's power"), {
      condition: "compare", value: { amount: "attribute", of: { ref: "that", filter: { types: ["creature"] } }, attribute: "power" },
      comparison: { op: "gt", value: { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" } },
    });
    assert.deepEqual(complete(parseCondition, "you don't control a creature"), { condition: "not", of: {
      condition: "control", who: you, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) },
    } });
    assert.deepEqual(complete(parseCondition, "you don't"), { condition: "you-dont" });
  });
  it("preserves explicit unions and shared qualifications", () => {
    assert.deepEqual(complete(parseFilter, "spell or ability you control"), { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }], control: "you" });
    assert.deepEqual(complete(parseFilter, "artifact and/or creature cards in your graveyard"), {
      allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }], cls: "card", zone: { zone: "graveyard", owner: "your" },
    });
    assert.deepEqual(complete(parseFilter, "Elves, Goblins, and Orcs you control"), {
      allOf: [{ anyOf: [{ subtypes: ["Elf"] }, { subtypes: ["Goblin"] }, { subtypes: ["Orc"] }] }], control: "you",
    });
    assert.deepEqual(complete(parseFilter, "artifact creature"), { types: ["artifact", "creature"] });
    assert.deepEqual(complete(parseFilter, "creature and land cards revealed this way"), {
      allOf: [{ anyOf: [{ types: ["creature"] }, { types: ["land"] }] }], cls: "card",
      relation: { relation: "event-result", event: "reveal", scope: "this-way" },
    });
    assert.deepEqual(complete(parseFilter, "instant and sorcery cards from among them"), {
      allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card", fromAmong: { ref: "them" },
    });
  });
});

describe("coordinated effects", () => {
  it("preserves pool selections, complement moves, and destination order (Impulse)", () => {
    const hand = { effect: "move-zone", what: { ref: "from-pool", pool: { ref: "them" }, count: n(1) }, to: { zone: "hand", owner: "your" } };
    const bottom = { effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "any" };
    assert.deepEqual(parsed("Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order."), {
      kind: "spell", effects: [
        { steps: [{ effect: "look-at", who: you, what: { top: n(4), of: { zone: "library", owner: "your" } } }] },
        { steps: [{ effect: "simultaneous", steps: [hand, bottom] }] },
      ],
    });
    assert.deepEqual(parsed("Put one of them into your hand, then put the rest on the bottom of your library in any order."), {
      kind: "spell", effects: [{ steps: [hand, bottom] }],
    });
    assert.deepEqual(complete(parseObjectRef, "up to two of those cards"), {
      ref: "from-pool", pool: { ref: "that", filter: { cls: "card" }, plural: true }, count: n(2), upTo: true,
    });
    for (const text of ["Put one of them into your hand and the rest.", "Put one of them into your hand in any order.", "Put one of them into your hand tapped."]) {
      assert.equal(parseOracleText(text).ok, false, text);
    }
    assert.deepEqual(parsed("Put a creature card and a land card from your hand onto the battlefield."), {
      kind: "spell", effects: [{ steps: [{ effect: "move-zone", what: {
        ref: "card-extraction-group", members: [
          { filter: { types: ["creature"], cls: "card" }, count: n(1) },
          { filter: { types: ["land"], cls: "card" }, count: n(1) },
        ], combine: "all", allocation: "distinct-objects", source: { kind: "zone", zone: { zone: "hand", owner: "your" } },
      }, to: { zone: "battlefield" } }] }],
    });
    assert.deepEqual(parsed("Put all creature and land cards revealed this way into your hand and the rest into your graveyard."), {
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "move-zone", what: { ref: "all", filter: { allOf: [{ anyOf: [{ types: ["creature"] }, { types: ["land"] }] }], cls: "card", relation: { relation: "event-result", event: "reveal", scope: "this-way" } } }, to: { zone: "hand", owner: "your" } },
        { effect: "move-zone", what: { ref: "rest" }, to: { zone: "graveyard", owner: "your" } },
      ] }] }],
    });
  });
  it("retains the reveal/selection pool and the separately stated discarding player (Gix's Caress)", () => {
    assert.deepEqual(parsed("Target opponent reveals their hand. You choose a nonland card from it. That player discards that card."), {
      kind: "spell", effects: [
        { steps: [{ effect: "reveal", who: { player: "target-opponent" }, what: { ref: "all", filter: { cls: "card", zone: { zone: "hand", owner: "their" } } } }] },
        { steps: [{ effect: "choose-object", who: you, what: { ref: "filter", filter: { nonTypes: ["land"], cls: "card", fromAmong: { ref: "it" } }, count: n(1) } }] },
        { steps: [{ effect: "discard", who: { player: "that-player" }, what: { ref: "that", filter: { cls: "card" } } }] },
      ],
    });
    assert.deepEqual(parsed("They put two +1/+1 counters on a creature they control."), {
      kind: "spell", effects: [{ steps: [{ effect: "put-counters", counter: { type: "+1/+1" }, count: n(2), who: { player: "they" },
        on: { ref: "filter", filter: { types: ["creature"], controlledBy: { player: "they" } }, count: n(1) },
      }] }],
    });
    assert.deepEqual(parsed("That player puts it into their hand."), {
      kind: "spell", effects: [{ steps: [{ effect: "move-zone", who: { player: "that-player" }, what: { ref: "it" }, to: { zone: "hand", owner: "their" } }] }],
    });
  });
  it("keeps a chosen quantity distinct from targeting and preserves its downstream amount", () => {
    assert.deepEqual(parsed("Discard any number of cards, then draw that many cards."), {
      kind: "spell", effects: [{ steps: [
        { effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: { amount: "chosen", minimum: 0 } } },
        { effect: "draw", who: you, amount: { amount: "that-much" } },
      ] }],
    });
    const c = new Cursor(lex("half your life total"));
    assert.equal(parseAmount(c), null);
    assert.equal(c.pos, 0);
  });
  it("keeps the actor of They across base-form verbs but respects a new you", () => {
    const draw = { effect: "draw", who: { player: "they" }, amount: n(1) };
    const card = { ref: "filter", filter: { cls: "card" }, count: n(1) };
    assert.deepEqual(parsed("They draw a card and discard a card."), { kind: "spell", effects: [{ steps: [draw, { effect: "discard", who: { player: "they" }, what: card }] }] });
    assert.deepEqual(parsed("They draw a card and you may discard a card."), { kind: "spell", effects: [{ steps: [draw, { effect: "discard", who: you, what: card, optional: true }] }] });
    assert.deepEqual(parsed("They add {G} and add {U}."), { kind: "spell", effects: [{ steps: ["G", "U"].map(symbol => ({ effect: "add-mana", who: { player: "they" }, mana: { mana: "fixed", symbols: [symbol] } })) }] });
  });
  it("keeps shared damage and different damage amounts simultaneous", () => {
    assert.deepEqual(parsed("This spell deals X damage to each nonartifact creature and each player."), { kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "this", noun: "spell" }, amount: { amount: "x" }, to: { to: "each", refs: [
        { ref: "each", filter: { nonTypes: ["artifact"], types: ["creature"] } }, { player: "each-player" },
      ] },
    }] }] });
    const source = { ref: "this", noun: "creature" };
    assert.deepEqual(parsed("This creature deals 1 damage to any target and 1 damage to you."), { kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
      { effect: "damage", source, amount: n(1), to: { to: "any-target" } },
      { effect: "damage", source, amount: n(1), to: { to: "player", ref: you } },
    ] }] }] });
  });
  it("keeps mixed permanent modifiers static and temporary modifiers resolving", () => {
    assert.deepEqual(parsed("Enchanted creature gets +2/+2 and can't attack you."), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: n(2) }, toughness: { sign: 1, amount: n(2) } },
        { modifier: "combat-rule", rule: { rule: "attack-restriction", defenders: [{ defender: "player", who: you }] } },
      ] },
    });
    const what = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
    const duration = { duration: "end-of-turn" };
    assert.deepEqual(parsed("Target creature can't be blocked by green creatures and gains flying until end of turn."), {
      kind: "spell", effects: [{ steps: [
        { effect: "combat-rule", what, duration, rule: { rule: "blocker-filter", mode: "exclude", blockers: { ref: "each", filter: { colors: ["green"], types: ["creature"] } } } },
        { effect: "gain-abilities", what, duration, abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] },
      ] }],
    });
  });
});
