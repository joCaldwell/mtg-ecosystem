import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

const n = (value: number) => ({ amount: "fixed", value });
const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
const duration = { duration: "end-of-turn" };
function one(text: string) {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}

describe("characteristic changes retain layers, choice, and shared subjects", () => {
  it("distinguishes removing abilities from setting base power and toughness", () => {
    // Cached Oracle text: Ovinize, Humility, Diminish. CR 613.1f, 613.4b.
    assert.deepEqual(one("Until end of turn, target creature loses all abilities and has base power and toughness 0/1."), {
      kind: "spell", effects: [{ steps: [
        { effect: "lose-abilities", what: target, removal: { removal: "all" }, duration },
        { effect: "set-pt", what: target, power: n(0), toughness: n(1), duration },
      ] }],
    });
    assert.deepEqual(one("All creatures lose all abilities and have base power and toughness 1/1."), {
      kind: "static", effect: { static: "modify", what: { ref: "all", filter: { types: ["creature"] } }, modifiers: [
        { modifier: "lose-abilities", removal: { removal: "all" } },
        { modifier: "base-pt", power: n(1), toughness: n(1) },
      ] },
    });
    assert.deepEqual(one("Target creature has base power and toughness 1/1 until end of turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "set-pt", what: target, power: n(1), toughness: n(1), duration }] }],
    });
  });
  it("uses the same subject for a following become clause and preserves unmentioned types", () => {
    assert.deepEqual(one("Until end of turn, target creature loses all abilities and becomes a blue Frog with base power and toughness 1/1."), {
      kind: "spell", effects: [{ steps: [
        { effect: "lose-abilities", what: target, removal: { removal: "all" }, duration },
        { effect: "become", what: target, spec: { colors: ["blue"], subtypes: ["Frog"], power: n(1), toughness: n(1) }, duration },
      ] }],
    });
    assert.equal(parseOracleText("Target creature becomes a Frog and.").ok, false);
    assert.equal(parseOracleText("Target creature loses all abilities. Becomes a Frog.").ok, false);
    assert.equal(parseOracleText("Target creature has base power and toughness 1/.").ok, false);
  });
  it("keeps a choice of one lost ability distinct from losing the list", () => {
    const keywords = ["flying", "first strike", "trample"].map(keyword => ({ keyword }));
    assert.deepEqual(one("Target creature loses your choice of flying, first strike, or trample until end of turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "lose-abilities", what: target, duration,
        removal: { removal: "choice", who: { player: "you" }, count: n(1), options: keywords },
      }] }],
    });
    assert.deepEqual(one("Target creature loses flying and trample until end of turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "lose-abilities", what: target, duration,
        removal: { removal: "listed", abilities: ["flying", "trample"].map(keyword => ({ gained: "keyword", keyword: { keyword } })) },
      }] }],
    });
    assert.equal(parseOracleText("Target creature loses your choice of flying, first strike, and trample until end of turn.").ok, false);
    assert.equal(parseOracleText("Target creature loses arbitrary prose until end of turn.").ok, false);
  });
});

describe("calculated counts and duration scope", () => {
  const count = { amount: "count", of: { types: ["creature"], control: "you" } };
  it("retains formulas in draw, counters, and token creation", () => {
    assert.deepEqual(one("Draw cards equal to the number of creatures you control."), {
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: count }] }],
    });
    assert.deepEqual(one("Put a number of +1/+1 counters on target creature equal to its power."), {
      kind: "spell", effects: [{ steps: [{ effect: "put-counters", on: target, counter: { type: "+1/+1" }, count: { amount: "attribute", of: { ref: "it" }, attribute: "power" } }] }],
    });
    assert.deepEqual(one("Create a number of 1/1 white Soldier creature tokens equal to the number of creatures you control."), {
      kind: "spell", effects: [{ steps: [{ effect: "create-token", count,
        token: { colors: ["white"], supertypes: [], types: ["creature"], subtypes: ["Soldier"], keywords: [], power: n(1), toughness: n(1) },
      }] }],
    });
    assert.equal(parseOracleText("Create a number of Soldier creature tokens.").ok, false);
  });
  it("retains distribution and the chosen number of targets", () => {
    assert.deepEqual(one("This spell deals 3 damage divided as you choose among any number of targets."), {
      kind: "spell", effects: [{ steps: [{ effect: "damage", source: { ref: "this", noun: "spell" }, amount: n(3),
        to: { to: "divided", among: { ref: "any-target", count: { amount: "chosen", minimum: 0 } } },
      }] }],
    });
    assert.equal(parseOracleText("This spell deals 3 damage divided as you choose among each creature.").ok, false);
  });
  it("repeats delayed triggers within an explicit lifetime and rejects durations on one-shot actions", () => {
    assert.deepEqual(one("Until end of turn, whenever a creature dies, draw a card."), {
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", occurrence: "each", targets: "delayed-ability", duration,
        trigger: { trigger: "dies", what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } },
        effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }],
      }] }],
    });
    for (const text of ["Until end of turn, draw a card.", "Exile target creature until end of turn.", "Until end of turn, exile target creature.", "Draw a card until end of turn."]) {
      assert.equal(parseOracleText(text).ok, false, text);
    }
  });
});
