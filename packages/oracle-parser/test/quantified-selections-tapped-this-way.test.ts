import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: "for each <permanent> tapped this way".
// Oracle wordings checked in the frozen corpus:
// Harmony of Nature, Guild Summit, Siege Striker, Devout Invocation.
// CR 701.26a (only untapped permanents can be tapped), 608.2h/i:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const tappedThisWay = { relation: "event-result", event: "tap", scope: "this-way" };
function parsed(text: string, typeLine?: string) {
  const result = parseOracleText(text, undefined, typeLine ? { typeLine } : undefined);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe("permanents tapped this way are an event result, not a status", () => {
  it("counts the permanents the preceding instruction tapped", () => {
    assert.deepEqual(parsed("Tap any number of untapped creatures you control. You gain 4 life for each creature tapped this way.", "Instant"), [{
      kind: "spell", effects: [
        { steps: [{ effect: "tap", what: { ref: "filter", filter: { status: ["untapped"], types: ["creature"], control: "you" }, count: { amount: "chosen", minimum: 0 } } }] },
        { steps: [{ effect: "gain-life", who: { player: "you" }, amount: n(4), forEach: { types: ["creature"], relation: tappedThisWay } }] },
      ] }]);
  });
  it("keeps the optional tap and the pump's duration", () => {
    assert.deepEqual(parsed("Whenever this creature attacks, you may tap any number of untapped creatures you control. This creature gets +1/+1 until end of turn for each creature tapped this way.", "Creature — Human Soldier"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
      effects: [
        { steps: [{ effect: "tap", what: { ref: "filter", filter: { status: ["untapped"], types: ["creature"], control: "you" }, count: { amount: "chosen", minimum: 0 } }, optional: true }] },
        { steps: [{ effect: "pump", what: { ref: "this", noun: "creature" }, power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) },
          duration: { duration: "end-of-turn" }, forEach: { types: ["creature"], relation: tappedThisWay } }] },
      ] }]);
  });
  it("keeps the subtype of the counted permanents", () => {
    const [ability] = parsed("When this enchantment enters, you may tap any number of untapped Gates you control. Draw a card for each Gate tapped this way.", "Enchantment");
    assert.deepEqual(ability.effects[1], { steps: [{ effect: "draw", who: { player: "you" }, amount: n(1), forEach: { subtypes: ["Gate"], relation: tappedThisWay } }] });
  });
  it("leaves tapped status alone and rejects other periods", () => {
    assert.deepEqual(parsed("Destroy each tapped creature.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "each", filter: { status: ["tapped"], types: ["creature"] } } }] }] }]);
    assert.deepEqual(parsed("You gain 1 life for each creature tapped.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: n(1), forEach: { types: ["creature"], status: ["tapped"] } }] }] }]);
    assert.equal(parseOracleText("You gain 1 life for each creature tapped this turn.", undefined, { typeLine: "Sorcery" }).ok, false);
  });
});
