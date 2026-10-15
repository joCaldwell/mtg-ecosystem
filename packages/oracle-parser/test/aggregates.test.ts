import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, Condition, Effect, ObjectRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAggregateAmount, parseAggregateCount } from "../src/parser/aggregates.ts";
import { parseAmount, parseCondition, parseObjectRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle clauses checked in the frozen corpus: Wandering Stream, Power Armor,
// Unified Front, Glinting Creeper, Feed the Infection, Atarka Beastbreaker,
// Radha's Firebrand, General Tazri and Thraben Foulbloods.
// CR 105.2c, 205.2b, 305.6, 601.2f/h, 608.2c/h, 609.4b, 614.12:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const lands: ObjectRef = { ref: "each", filter: { types: ["land"], control: "you" } };
const creatures: ObjectRef = { ref: "each", filter: { types: ["creature"], control: "you" } };
const graveyard: ObjectRef = { ref: "each", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } } };
const domain: Amount = { amount: "distinct-count", characteristic: "basic-land-type", among: lands };
const delirium: Amount = { amount: "distinct-count", characteristic: "card-type", among: graveyard };
const converge: Amount = { amount: "mana-spent", quantity: "distinct-colors", what: { ref: "this", noun: "spell" } };
const product = (quantity: number, multiplier: Amount): Amount => ({ amount: "product", factors: [n(quantity), multiplier] });
function amount(text: string, expected: Amount) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseAmount(c), expected);
  assert.ok(c.done(), c.errorMessage());
}
function condition(text: string, expected: Condition) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseCondition(c), expected);
  assert.ok(c.done(), c.errorMessage());
}
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function spell(text: string, steps: Effect[], abilityWord?: string) {
  assert.deepEqual(parsed(text), [{ kind: "spell", ...(abilityWord ? { abilityWord } : {}), effects: [{ steps }] }]);
}

describe("aggregate values retain the measured set and characteristic", () => {
  it("distinguishes object counts, distinct card/basic-land types and colors", () => {
    amount("the number of lands you control", { amount: "count", of: { types: ["land"], control: "you" } });
    amount("the number of basic land types among lands you control", domain);
    amount("the number of card types among cards in your graveyard", delirium);
    amount("the number of different colors among creatures you control", { amount: "distinct-count", characteristic: "color", among: creatures });
    amount("the number of colors among those creatures", { amount: "distinct-count", characteristic: "color", among: { ref: "that", filter: { types: ["creature"] }, plural: true } });
    amount("the number of colors among colorless permanents", { amount: "distinct-count", characteristic: "color", among: { ref: "each", filter: { cls: "permanent", colorless: true } } });
  });
  it("separates actual total mana from distinct colors paid and printed mana value", () => {
    amount("the number of colors of mana spent to cast this spell", converge);
    amount("the amount of mana spent to cast this spell", { amount: "mana-spent", what: { ref: "this", noun: "spell" }, quantity: "total" });
    amount("this spell's mana value", { amount: "attribute", of: { ref: "this", noun: "spell" }, attribute: "mana-value" });
  });
  it("retains exact attribute sums instead of a selected object's value", () => {
    amount("the total power of creatures you control", { amount: "attribute-sum", attribute: "power", among: creatures });
    amount("the total toughness of those creatures", { amount: "attribute-sum", attribute: "toughness", among: { ref: "that", filter: { types: ["creature"] }, plural: true } });
    amount("the total mana value of cards in your graveyard", { amount: "attribute-sum", attribute: "mana-value", among: graveyard });
    condition("creatures you control have total power 8 or greater", { condition: "compare", value: { amount: "attribute-sum", attribute: "power", among: creatures }, comparison: { op: "ge", value: n(8) } });
  });
  it("rewinds malformed descriptors and leaves unrelated iteration unsupported", () => {
    for (const text of ["the number of basic land types among", "the number of card types of cards in your graveyard", "the number of colors of mana spent to cast", "the total loyalty of creatures you control"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseAggregateAmount(c, { parseObjectRef }), null, text);
      assert.equal(c.pos, 0, text);
    }
    const c = new Cursor(lex("basic land types among lands you control"));
    assert.equal(parseAggregateCount(c, { parseObjectRef }, { singular: true }), null);
    assert.equal(c.pos, 0);
    for (const text of ["Draw a card for each color among permanents you control for each card type among cards in your graveyard.",
      "Target creature gains flying for each basic land type among lands you control."])
      assert.equal(parseOracleText(text).ok, false, text);
    // Now a complete production: one mana of each color among them (CR 106.5).
    const eachColor = parseOracleText("For each color among permanents you control, add one mana of that color.");
    assert.ok(eachColor.ok);
    assert.deepEqual(JSON.parse(JSON.stringify(eachColor.abilities)), [{ kind: "spell", effects: [{ steps: [{ effect: "add-mana", who: { player: "you" },
      mana: { mana: "each-color-among", among: { ref: "each", filter: { cls: "permanent", control: "you" } } } }] }] }]);
  });
});

describe("aggregate quantities preserve event grouping and entry timing", () => {
  it("scales one life event and one damage event with simultaneous recipients", () => {
    spell("Domain — You gain 2 life for each basic land type among lands you control.", [{ effect: "gain-life", who: { player: "you" }, amount: product(2, domain) }], "Domain");
    spell("This spell deals 1 damage to each creature and each player for each card type among cards in your graveyard.", [{ effect: "damage", source: { ref: "this", noun: "spell" }, amount: product(1, delirium), to: { to: "each", refs: [{ ref: "each", filter: { types: ["creature"] } }, { player: "each-player" }] } }]);
  });
  it("preserves the duration and sign of a domain power/toughness effect", () => {
    spell("All creatures get -1/-1 until end of turn for each basic land type among lands you control.", [{ effect: "pump", what: { ref: "all", filter: { types: ["creature"] } }, power: { sign: -1, amount: product(1, domain) }, toughness: { sign: -1, amount: product(1, domain) }, duration: { duration: "end-of-turn" } }]);
  });
  it("creates one token group and modifies an entering event without later counters", () => {
    spell("Converge — Create a 1/1 white Kor Ally creature token for each color of mana spent to cast this spell.", [{ effect: "create-token", count: product(1, converge), token: { colors: ["white"], supertypes: [], types: ["creature"], subtypes: ["Kor", "Ally"], keywords: [], power: n(1), toughness: n(1) } }], "Converge");
    assert.deepEqual(parsed("Converge — This creature enters with two +1/+1 counters on it for each color of mana spent to cast it."), [{ kind: "static", abilityWord: "Converge", effect: { static: "enters-with-counters", what: { ref: "this", noun: "creature" }, count: n(2), counter: { type: "+1/+1" }, multiplier: { amount: "mana-spent", quantity: "distinct-colors", what: { ref: "it" } } } }]);
  });
  it("reduces the casting cost once per represented basic land type", () => {
    assert.deepEqual(parsed("This spell costs {1} less to cast for each basic land type among lands you control."), [{ kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } }, change: { direction: "reduce", symbols: ["1"] }, multiplier: domain } }]);
    assert.deepEqual(parsed("This spell costs {1} less to cast for each different color among permanents you control."), [{ kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } }, change: { direction: "reduce", symbols: ["1"] }, multiplier: { amount: "distinct-count", characteristic: "color", among: { ref: "each", filter: { cls: "permanent", control: "you" } } } } }]);
  });
  it("keeps Prismatic Ending's paid-color check at resolution, outside target eligibility", () => {
    // Wizards explicitly distinguishes this resolution condition from a
    // target restriction; mana paid is not yet known at target declaration.
    // https://magic.wizards.com/en/news/feature/modern-horizons-3-release-notes
    assert.deepEqual(parsed("Converge — Exile target nonland permanent if its mana value is less than or equal to the number of colors of mana spent to cast this spell."), [{ kind: "spell", abilityWord: "Converge", effects: [{
      steps: [{ effect: "exile", what: { ref: "target", filter: { nonTypes: ["land"], cls: "permanent" }, count: n(1) } }],
      condition: { condition: "compare", value: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" }, comparison: { op: "le", value: converge } }, conditionKind: "if",
    }] }]);
  });
});

describe("aggregate thresholds and player counters", () => {
  it("keeps delirium thresholds separate from four cards and preserves opponent quantification", () => {
    condition("there are four or more card types among cards in your graveyard", { condition: "compare", value: delirium, comparison: { op: "ge", value: n(4) } });
    condition("an opponent has three or more poison counters", { condition: "player-counters", who: { player: "an-opponent" }, counter: { type: "poison" }, comparison: { op: "ge", value: n(3) } });
    condition("you have no poison counters", { condition: "player-counters", who: { player: "you" }, counter: { type: "poison" }, comparison: { op: "eq", value: n(0) } });
  });
  it("filters each opponent independently and preserves target selection provenance", () => {
    spell("Corrupted — Each opponent who has three or more poison counters loses 3 life.", [{ effect: "lose-life", amount: n(3), who: { player: "filtered", base: { player: "each-opponent" }, filter: { counterConstraints: [{ counter: { type: "poison" }, comparison: { op: "ge", value: n(3) } }] } } }], "Corrupted");
    const result = parseOracleText("Target opponent who has fewer than nine poison counters draws a card.", "Counter Test", { includeSourceProvenance: true });
    assert.ok(result.ok);
    const first = result.abilities[0];
    assert.equal(first.kind, "spell");
    if (first.kind !== "spell") return;
    assert.deepEqual(first.effects[0].steps[0], { effect: "draw", amount: n(1), who: { player: "filtered", base: { player: "target-opponent", selectionId: "line:0:0" }, filter: { counterConstraints: [{ counter: { type: "poison" }, comparison: { op: "lt", value: n(9) } }] } } });
  });
  it("does not claim payment history or per-player predicate bindings are complete", () => {
    for (const text of ["You gain life equal to the number of colors of mana spent to cast this spell.", "Each opponent who has three or more poison counters loses 3 life.", "If an opponent has three or more poison counters, draw a card."]) {
      const result = parseOracleText(text, "Aggregate Test", { includeSourceProvenance: true });
      assert.ok(result.ok);
      const binding = analyzeCardBindings({ identity: "aggregate-test", source: { name: "Aggregate Test", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(binding.fullyBound, false, text);
      assert.ok(binding.diagnostics.some(d => d.code === "unsupported-scope"), text);
    }
  });
});
