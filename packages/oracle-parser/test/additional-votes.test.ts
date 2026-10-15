import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Static abilities. Wordings are taken from
// the cached Scryfall corpus; card names are noted per test.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string, name?: string) {
  return parseOracleText(text, name, { typeLine });
}
function only(text: string, typeLine: string, name?: string): Ability {
  const parsed = parse(text, typeLine, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function effect(text: string, typeLine: string, name?: string): StaticEffect {
  const ability = only(text, typeLine, name);
  assert.equal(ability.kind, "static");
  if (ability.kind !== "static") throw new Error(text);
  return ability.effect;
}
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}

describe("additional votes (CR 701.38c–d)", () => {
  const human = "Creature — Human Advisor";
  it("keeps whether the extra vote is optional (Ballot Broker, Brago's Representative)", () => {
    assert.deepEqual(effect("While voting, you may vote an additional time.", human), {
      static: "player-rule", rule: { who: { player: "you" }, rule: "additional-votes", additional: 1, optional: true } });
    assert.deepEqual(effect("While voting, you get an additional vote.", human), {
      static: "player-rule", rule: { who: { player: "you" }, rule: "additional-votes", additional: 1, optional: false } });
  });
  it("needs the voting scope and the player's own vote", () => {
    rejects("You may vote an additional time.", human);
    rejects("While voting, each opponent may vote an additional time.", human);
    rejects("While voting, you may vote an additional time this turn.", human);
  });
});
