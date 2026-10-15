import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// The Ring-bearer designation and "the Ring has tempted you": CR 701.54b-e
// (https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
function parse(text: string, typeLine = "Creature — Test"): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function rejected(text: string): void {
  assert.equal(parseOracleText(text, "Test", { typeLine: "Creature — Test" }).ok, false, text);
}

describe("Ring-bearer", () => {
  it("'is your Ring-bearer' is true for a creature you control with the designation", () => {
    assert.deepEqual(parse("As long as ~ is your Ring-bearer, it must be blocked if able.")[0], { kind: "static", effect: {
      static: "conditional", condition: { condition: "is-filter", what: { ref: "self" }, filter: { ringBearer: true, control: "you" } },
      effect: { static: "combat-rule", what: { ref: "it" }, rule: { rule: "blocked-requirement" } } } });
  });
  it("keeps Ring-bearer as a designation filter in protection and control tests", () => {
    const [protection] = parse("Wraiths you control have protection from Ring-bearers.");
    assert.ok(protection.kind === "static" && protection.effect.static === "modify");
    assert.deepEqual(JSON.parse(JSON.stringify(protection.effect.modifiers)), [{ modifier: "abilities", abilities: [{ gained: "keyword",
      keyword: { keyword: "protection", from: [{ scope: "filter", filter: { ringBearer: true } }] } }] }]);
    const [landfall] = parse("Whenever a land you control enters, if you don't control a Ring-bearer, the Ring tempts you.");
    assert.ok(landfall.kind === "triggered");
    assert.deepEqual(landfall.condition, { condition: "not", of: { condition: "control", who: { player: "you" },
      what: { ref: "filter", filter: { ringBearer: true }, count: { amount: "fixed", value: 1 } } } });
  });
  it("parses the number of times the Ring has tempted a player this game", () => {
    const [ability] = parse("Whenever ~ attacks, if the Ring has tempted you two or more times this game, draw a card.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.condition, { condition: "ring-tempted", who: { player: "you" }, comparison: { op: "ge", value: { amount: "fixed", value: 2 } } });
  });
  it("rejects wordings it does not represent", () => {
    rejected("As long as ~ is their Ring-bearer, it must be blocked if able.");
    rejected("Whenever ~ attacks, if the Ring has tempted you two times this turn, draw a card.");
  });
});

describe("max speed (CR 702.179e)", () => {
  it("'you have max speed' is its own player condition, negation and unless included", () => {
    const [unless] = parse("~ can't attack or block unless you have max speed.");
    assert.deepEqual(unless, { kind: "static", effect: { static: "cant", what: { ref: "self" }, action: "attack-or-block",
      condition: { condition: "max-speed", who: { player: "you" } }, conditionKind: "unless" } });
    const [trigger] = parse("Whenever ~ attacks, if you don't have max speed, draw a card.");
    assert.ok(trigger.kind === "triggered");
    assert.deepEqual(trigger.condition, { condition: "not", of: { condition: "max-speed", who: { player: "you" } } });
  });
  it("rejects max speed with other possession wording", () => {
    rejected("Whenever ~ attacks, if you have maximum speed, draw a card.");
    rejected("Whenever ~ attacks, if you have max speed twice, draw a card.");
  });
});
