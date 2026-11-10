import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings from the cached Scryfall corpus (Arterial Alchemy, Blazing
// Sunsteel, Inspired Sphinx, Goblin Goliath, Embereth Skyblazer, Rasputin).
// CR 102.2-102.3: opponents are players, never objects.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string) {
  return parseOracleText(text, undefined, { typeLine });
}
function only(text: string, typeLine: string): Ability {
  const parsed = parse(text, typeLine);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function rejects(text: string, typeLine: string) {
  assert.equal(parse(text, typeLine).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const opponents = { amount: "opponent-count", who: { player: "you" } };
const enters = { trigger: "enters", what: { ref: "this", noun: "creature" } };
const goblin = { colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Goblin"], keywords: [], power: fixed(1), toughness: fixed(1) };

describe("number of opponents a player has", () => {
  it("is an amount of its own, not an object count", () => {
    assert.deepEqual(only("When this creature enters, draw cards equal to the number of opponents you have.", "Creature — Sphinx"), {
      kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: opponents }] }] });
    assert.deepEqual(only("When this creature enters, create a number of 1/1 red Goblin creature tokens equal to the number of opponents you have.", "Creature — Goblin Mutant"), {
      kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "create-token", count: opponents, token: goblin }] }] });
  });

  it("defines X from the opponent count", () => {
    assert.deepEqual(only("Whenever this creature attacks, you may pay {2}{R}. If you do, creatures you control get +X/+0 until end of turn, where X is the number of opponents you have.", "Creature — Human Knight"), {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
      effects: [
        { steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "mana", symbols: ["2", "R"] }], optional: true }] },
        { condition: { condition: "you-do" }, conditionKind: "if",
          steps: [{ effect: "pump", what: { ref: "each", filter: { types: ["creature"], control: "you" } },
            power: { sign: 1, amount: { amount: "x" } }, toughness: { sign: 1, amount: fixed(0) }, duration: { duration: "end-of-turn" } }],
          bindings: [{ variable: "x", value: opponents }] },
      ],
    });
  });

  it("scales one instruction's quantity for each opponent", () => {
    assert.deepEqual(only("When this enchantment enters, create a Blood token for each opponent you have.", "Enchantment"), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "enchantment" } },
      effects: [{ steps: [{ effect: "create-token", count: { amount: "product", factors: [fixed(1), opponents] }, token: { predefined: "Blood" } }] }] });
    assert.deepEqual(only("When ~ enters, put a dream counter on it for each opponent you have.", "Legendary Creature — Human Wizard"), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "put-counters", on: { ref: "it" }, counter: { type: "dream" }, count: { amount: "product", factors: [fixed(1), opponents] } }] }] });
  });

  it("multiplies a continuous modifier", () => {
    assert.deepEqual(only("Equipped creature gets +1/+0 for each opponent you have.", "Artifact — Equipment"), {
      kind: "static", effect: { static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [{
        modifier: "pt", power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) },
        multiplier: opponents, valueScope: "each-affected-object" }] } });
  });

  it("rejects quantified holders and qualified opponents", () => {
    rejects("When this creature enters, draw cards equal to the number of opponents each player has.", "Creature — Sphinx");
    rejects("When this creature enters, draw cards equal to the number of opponents you have who control a creature.", "Creature — Sphinx");
    rejects("When this creature enters, draw cards equal to the number of opponents you had.", "Creature — Sphinx");
    rejects("When this creature enters, destroy target creature for each opponent you have.", "Creature — Sphinx");
  });
});
