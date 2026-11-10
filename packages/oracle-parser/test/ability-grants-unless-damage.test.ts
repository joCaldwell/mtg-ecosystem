import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings from the cached Scryfall corpus (Lava Blister, Combustion Man,
// Barbarian Bully, Skullscorch, Pia's Revolution). CR 118.12a.
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

describe("unless a player has a source deal damage to them", () => {
  it("records the deciding player, the source and the amount", () => {
    assert.deepEqual(only("Destroy target nonbasic land unless its controller has ~ deal 6 damage to them.", "Sorcery"), {
      kind: "spell", effects: [{
        steps: [{ effect: "destroy", what: { ref: "target", filter: { nonSupertypes: ["basic"], types: ["land"] }, count: fixed(1) } }],
        condition: { condition: "has-damage-dealt", who: { player: "controller", of: { ref: "it" } }, source: { ref: "self" }, amount: fixed(6) },
        conditionKind: "unless",
      }] });
    assert.deepEqual(only("Target player discards two cards at random unless that player has ~ deal 4 damage to them.", "Sorcery"), {
      kind: "spell", effects: [{
        steps: [{ effect: "discard", who: { player: "target-player" }, what: { ref: "filter", filter: { cls: "card" }, count: fixed(2) }, random: true }],
        condition: { condition: "has-damage-dealt", who: { player: "that-player" }, source: { ref: "self" }, amount: fixed(4) },
        conditionKind: "unless",
      }] });
  });

  it("accepts a trailing computed amount and any player as decider", () => {
    assert.deepEqual(only("Whenever ~ attacks, destroy target permanent unless its controller has ~ deal damage to them equal to ~'s power.", "Legendary Creature — Human Assassin"), {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } }, effects: [{
        steps: [{ effect: "destroy", what: { ref: "target", filter: { cls: "permanent" }, count: fixed(1) } }],
        condition: { condition: "has-damage-dealt", who: { player: "controller", of: { ref: "it" } }, source: { ref: "self" },
          amount: { amount: "attribute", of: { ref: "self" }, attribute: "power" } },
        conditionKind: "unless",
      }] });
    assert.deepEqual(only("Discard a card at random: This creature gets +2/+2 until end of turn unless a player has this creature deal 4 damage to them. Activate only once each turn.", "Creature — Human Barbarian"), {
      kind: "activated", costs: [{ cost: "discard", what: { ref: "filter", filter: { cls: "card" }, count: fixed(1) }, random: true }],
      effects: [{
        steps: [{ effect: "pump", what: { ref: "this", noun: "creature" }, power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(2) },
          duration: { duration: "end-of-turn" } }],
        condition: { condition: "has-damage-dealt", who: { player: "a-player" }, source: { ref: "this", noun: "creature" }, amount: fixed(4) },
        conditionKind: "unless",
      }],
      restriction: { restriction: "once-each-turn" },
    });
  });

  it("is a different condition from paying a cost", () => {
    const paid = only("Destroy target nonbasic land unless its controller pays 6 life.", "Sorcery");
    if (paid.kind !== "spell") throw new Error("kind");
    assert.deepEqual(paid.effects[0].condition, { condition: "pays", who: { player: "controller", of: { ref: "it" } }, cost: [{ cost: "pay-life", amount: fixed(6) }] });
  });

  it("rejects other recipients and other sources", () => {
    rejects("Destroy target nonbasic land unless its controller has ~ deal 6 damage to you.", "Sorcery");
    rejects("Destroy target nonbasic land unless its controller has target creature deal 6 damage to them.", "Sorcery");
    rejects("Destroy target nonbasic land unless its controller has ~ deal damage to them.", "Sorcery");
  });
});
