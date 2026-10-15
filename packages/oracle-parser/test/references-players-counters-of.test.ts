import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const yourCreature = { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(1) };

// Oracle texts are complete cards from the fixed Scryfall corpus. Counters of
// an object that left the battlefield ceased to exist (CR 122.2); the effect
// puts new counters of the kinds and numbers it had (608.2h) and moves none
// (122.5): https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("putting the counters an object had onto another", () => {
  it("puts Star Pupil's counters on a target without moving them", () => {
    assert.deepEqual(parsed("When this creature dies, put its counters on target creature you control.", "Star Pupil", "Creature — Human Wizard"), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "put-counters-of", counters: { of: { ref: "it" } }, on: yourCreature }] }],
    }]);
  });
  it("keeps \"those counters\" as a reference to the counters the condition named (The Ozolith)", () => {
    assert.deepEqual(parsed("Whenever a creature you control leaves the battlefield, if it had counters on it, put those counters on The Ozolith.", "The Ozolith", "Legendary Artifact"), [{
      kind: "triggered", trigger: { trigger: "leaves", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: n(1) } },
      condition: { condition: "is-filter", what: { ref: "it" }, time: "previous",
        filter: { counterConstraints: [{ counter: "any", comparison: { op: "ge", value: n(1) } }] } },
      effects: [{ steps: [{ effect: "put-counters-of", counters: { ref: "those-counters", scope: "contextual" }, on: { ref: "self" } }] }],
    }]);
  });
  it("differs from moving counters between two permanents", () => {
    assert.deepEqual(parsed("Move all counters from target creature onto another target creature.", undefined, "Sorcery")[0].effects[0].steps[0].effect, "move-counters");
    assert.deepEqual(parsed("When this creature dies, put its counters on up to one target creature you control.", "Graft Surgeon", "Creature — Human Artificer")[0].effects[0].steps[0],
      { effect: "put-counters-of", counters: { of: { ref: "it" } }, on: { ...yourCreature, upTo: true } });
  });
  it("rejects other actors, quantities and counter kinds on this instruction", () => {
    fails("That player may put its counters on target creature.");
    fails("Put two of its counters on target creature you control.");
    fails("Put its +1/+1 counters on target creature you control.");
    fails("Put those counters on target creature you control for each creature you control.");
    fails("Remove those counters.");
  });
});
