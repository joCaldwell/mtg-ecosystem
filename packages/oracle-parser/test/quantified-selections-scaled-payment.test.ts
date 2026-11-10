import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: "unless <player> pays {N} for each <unit>".
// Oracle wordings checked in the frozen corpus:
// Circular Logic, Evasive Action, Rites of Refusal, Energy Vortex,
// Fettergeist, Cowed by Wisdom.
// CR 118.5, 118.12a, 608.2h:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const targetSpell = { ref: "target", filter: { cls: "spell" }, count: n(1) };
const itsController = { player: "controller", of: { ref: "it" } };
function parsed(text: string, typeLine?: string) {
  const result = parseOracleText(text, undefined, typeLine ? { typeLine } : undefined);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string, typeLine?: string) {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : undefined).ok, false, text);
}

describe("an unless-payment multiplied by a 'for each' unit", () => {
  it("multiplies the printed mana by an object count", () => {
    assert.deepEqual(parsed("Counter target spell unless its controller pays {1} for each card in your graveyard.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "counter", what: targetSpell }], conditionKind: "unless",
        condition: { condition: "pays", who: itsController, cost: [{ cost: "mana", symbols: ["1"] }],
          multiplier: { amount: "count", of: { cls: "card", zone: { zone: "graveyard", owner: "your" } } } } }] }]);
  });
  it("differs from the same payment without a multiplier", () => {
    assert.deepEqual(parsed("Counter target spell unless its controller pays {1}.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "counter", what: targetSpell }], conditionKind: "unless",
        condition: { condition: "pays", who: itsController, cost: [{ cost: "mana", symbols: ["1"] }] } }] }]);
  });
  it("accepts distinct values, event results and counters as the unit", () => {
    const multiplier = (text: string, typeLine: string) => parsed(text, typeLine)[0].effects.at(-1).condition.multiplier;
    assert.deepEqual(multiplier("Domain — Counter target spell unless its controller pays {1} for each basic land type among lands you control.", "Instant"),
      { amount: "distinct-count", characteristic: "basic-land-type", among: { ref: "each", filter: { types: ["land"], control: "you" } } });
    assert.deepEqual(multiplier("Discard any number of cards. Counter target spell unless its controller pays {3} for each card discarded this way.", "Instant"),
      { amount: "count", of: { cls: "card", relation: { relation: "event-result", event: "discard", scope: "this-way" } } });
    assert.deepEqual(multiplier("At the beginning of your upkeep, sacrifice this enchantment unless you pay {1} for each vortex counter on this enchantment.", "Enchantment"),
      { amount: "counter-count", counter: { type: "vortex" }, on: { ref: "this", noun: "enchantment" } });
  });
  it("keeps the payer and the lexical 'other' of the counted objects", () => {
    assert.deepEqual(parsed("At the beginning of your upkeep, sacrifice this creature unless you pay {1} for each other creature you control.", "Creature — Spirit"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      effects: [{ steps: [{ effect: "sacrifice", who: { player: "you" }, what: { ref: "this", noun: "creature" } }], conditionKind: "unless",
        condition: { condition: "pays", who: { player: "you" }, cost: [{ cost: "mana", symbols: ["1"] }],
          multiplier: { amount: "count", of: { other: true, types: ["creature"], control: "you" } } } }] }]);
  });
  it("rejects a multiplied X and a multiplied non-mana payment", () => {
    fails("Counter target spell unless its controller pays {X} for each card in your graveyard.", "Instant");
    fails("Counter target spell unless its controller pays 2 life for each card in your graveyard.", "Instant");
    fails("Counter target spell unless its controller pays {1} for each card in your graveyard and each card in your hand.", "Instant");
    // CR 508.1g-h: a cost to attack or block is the combat rule `declaration-cost`
    // (cost-rules-targets-and-limits.test.ts), never a resolving unless-payment.
  });
});
