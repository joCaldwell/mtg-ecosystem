import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Keyword actions as effects: CR 701 (https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
const you = { player: "you" } as const;
const n = (value: number) => ({ amount: "fixed", value }) as const;

function parse(text: string, typeLine = "Creature — Test"): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function steps(text: string, typeLine?: string): Effect[][] {
  const [ability] = parse(text, typeLine);
  assert.ok(ability.kind === "triggered" || ability.kind === "spell" || ability.kind === "activated");
  return ability.effects.map(sentence => sentence.steps);
}
function rejected(text: string, typeLine = "Creature — Test"): void {
  assert.equal(parseOracleText(text, "Test", { typeLine }).ok, false, text);
}

describe("collect evidence as an effect (CR 701.59)", () => {
  it("is an optional performed action that later text can test", () => {
    const [first, second] = steps("Whenever this creature attacks, you may collect evidence 4. If you do, draw a card.");
    assert.deepEqual(first, [{ effect: "collect-evidence", who: you, amount: n(4), optional: true }]);
    assert.equal(second.length, 1);
  });
  it("keeps the whole 'may exile it and collect evidence' choice together", () => {
    const [first] = steps("When this creature dies, you may exile it and collect evidence 4. If you do, return this card to the battlefield tapped.");
    assert.deepEqual(first, [{ effect: "optional-group", who: you, steps: [
      { effect: "exile", what: { ref: "it" } }, { effect: "collect-evidence", who: you, amount: n(4) }] }]);
  });
  it("rejects collecting evidence without a threshold", () => {
    rejected("Whenever this creature attacks, you may collect evidence.");
  });
});

describe("repeated investigate / proliferate", () => {
  it("parses 'that many times' as the event quantity and 'once for each' as the per-object rider", () => {
    assert.deepEqual(steps("Whenever you gain life, investigate that many times.")[0],
      [{ effect: "keyword-action", action: "investigate", who: you, times: { amount: "that-much" } }]);
    assert.deepEqual(steps("Whenever ~ attacks, investigate once for each nontoken attacking creature.")[0], [{
      effect: "keyword-action", action: "investigate", who: you, forEach: { nonToken: true, status: ["attacking"], types: ["creature"] } }]);
  });
  it("parses 'proliferate, then proliferate again' as two proliferations", () => {
    assert.deepEqual(steps("When ~ dies, proliferate, then proliferate again.")[0],
      [{ effect: "keyword-action", action: "proliferate", who: you, times: n(2) }]);
  });
  it("rejects near-miss repetitions", () => {
    rejected("When ~ dies, proliferate, then investigate again.");
    rejected("When ~ dies, investigate once.");
    rejected("When ~ dies, investigate that many.");
  });
});

describe("fateseal (CR 701.29a)", () => {
  it("is its own action, not scry", () => {
    assert.deepEqual(steps("When ~ enters, fateseal 2.")[0], [{ effect: "fateseal", who: you, amount: n(2) }]);
    rejected("Whenever ~ attacks, target opponent fateseals 1.");
    rejected("When ~ enters, fateseal.");
  });
});

describe("goad forms (CR 701.15)", () => {
  const targetCreature = { ref: "target", filter: { types: ["creature"] }, count: n(1) } as const;
  it("keeps a stated duration on a goaded designation", () => {
    assert.deepEqual(steps("When ~ enters, target creature is goaded for the rest of the game.")[0],
      [{ effect: "goad", what: targetCreature, duration: { duration: "rest-of-game" } }]);
    assert.deepEqual(steps("When ~ enters, create a 1/1 white Soldier creature token. That token is goaded for the rest of the game.")[1],
      [{ effect: "goad", what: { ref: "that", filter: { cls: "token" } }, duration: { duration: "rest-of-game" } }]);
  });
  it("parses 'tap and goad' / 'untap and goad' as two instructions on one object", () => {
    for (const verb of ["tap", "untap"] as const) {
      assert.deepEqual(steps(`${verb === "tap" ? "Tap" : "Untap"} and goad target creature.`, "Sorcery")[0],
        [{ effect: "simultaneous", steps: [{ effect: verb, what: targetCreature }, { effect: "goad", what: targetCreature }] }]);
    }
  });
  it("rejects a goaded state with no stated duration and unsupported conjunctions", () => {
    rejected("When ~ enters, target creature is goaded.");
    rejected("Tap and goad.", "Sorcery");
    rejected("Sacrifice and goad target creature.", "Sorcery");
  });
});

describe("keyword X defined by 'where X is' (CR 702.189a, 107.3)", () => {
  it("keeps the definition of X on the keyword", () => {
    assert.deepEqual(parse("Firebending X, where X is this creature's power.")[0], { kind: "keywords", keywords: [{
      keyword: "firebending", amount: "x",
      bindings: [{ variable: "x", value: { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" } }] }] });
  });
  it("rejects a definition without a variable keyword amount or with an unknown quantity", () => {
    rejected("Firebending 2, where X is this creature's power.");
    rejected("Firebending X, where X is the flavor of this creature.");
  });
});
