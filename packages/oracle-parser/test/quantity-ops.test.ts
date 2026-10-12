import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// CR 701.10-12, 705, 613.4d, 122.5, 601.2d/608.2d: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const creature = { types: ["creature"] };
const targetCreature = { ref: "target", filter: creature, count: one } as const;

function sentences(text: string): Sentence[] {
  const result = parseOracleText(text, "Fixture");
  assert.ok(result.ok, JSON.stringify(result));
  const ability = result.abilities[0];
  assert.equal(ability.kind, "spell");
  if (ability.kind !== "spell") throw new Error("spell expected");
  return ability.effects;
}
const steps = (text: string) => sentences(text)[0].steps;
const fails = (text: string) => assert.equal(parseOracleText(text, "Fixture").ok, false, text);

describe("doubling and tripling", () => {
  it("doubles counters, power and life as distinct operations", () => {
    assert.deepEqual(steps("Double the number of +1/+1 counters on target creature."),
      [{ effect: "double-counters", on: targetCreature, counter: { type: "+1/+1" } }]);
    assert.deepEqual(steps("Double the number of each kind of counter on target creature."),
      [{ effect: "double-counters", on: targetCreature, counter: "each-kind" }]);
    assert.deepEqual(steps("Double your life total."), [{ effect: "double-life-total", who: you }]);
  });
  it("keeps power/toughness doubling as a duration-bearing operation on the object", () => {
    assert.deepEqual(steps("Double target creature's power until end of turn."),
      [{ effect: "multiply-pt", what: targetCreature, stat: "power", factor: "double", duration: { duration: "end-of-turn" } }]);
    assert.deepEqual(steps("Triple the power and toughness of each creature you control until end of turn."),
      [{ effect: "multiply-pt", what: { ref: "each", filter: { ...creature, control: "you" } }, stat: "both", factor: "triple",
        duration: { duration: "end-of-turn" } }]);
    assert.deepEqual(steps("Until end of turn, double target creature's power X times.")[0],
      { effect: "multiply-pt", what: targetCreature, stat: "power", factor: "double", times: { amount: "x" }, duration: { duration: "end-of-turn" } });
  });
  it("rejects near-misses", () => {
    fails("Triple your life total.");
    fails("Double the number of +1/+1 counters.");
    fails("Double target creature's mana value.");
  });
});

describe("switching and exchanging", () => {
  it("switches power and toughness with a duration", () => {
    assert.deepEqual(steps("Switch target creature's power and toughness until end of turn."),
      [{ effect: "switch-pt", what: targetCreature, duration: { duration: "end-of-turn" } }]);
    assert.deepEqual(steps("Switch each creature's power and toughness until end of turn."),
      [{ effect: "switch-pt", what: { ref: "each", filter: creature }, duration: { duration: "end-of-turn" } }]);
  });
  it("exchanges control, life totals, values and zones", () => {
    assert.deepEqual(steps("Exchange control of two target creatures."),
      [{ effect: "exchange-control", what: [{ ref: "target", filter: creature, count: { amount: "fixed", value: 2 } }] }]);
    assert.deepEqual(steps("Exchange life totals with target opponent."),
      [{ effect: "exchange-life-totals", between: [you, { player: "target-opponent" }] }]);
    assert.deepEqual(steps("Exchange your life total with this creature's toughness."),
      [{ effect: "exchange-values", a: { value: "life-total", of: you }, b: { value: "toughness", of: { ref: "this", noun: "creature" } } }]);
    assert.deepEqual(steps("Exchange your hand and graveyard."),
      [{ effect: "exchange-zones", a: { zone: "hand", owner: "your" }, b: { zone: "graveyard", owner: "your" } }]);
  });
  it("rejects near-misses", () => {
    fails("Exchange control of target creature.");
    fails("Exchange your hand and hand.");
    fails("Switch target creature's power.");
  });
});

describe("counter movement and distribution", () => {
  it("moves counters between two objects", () => {
    assert.deepEqual(steps("Move all +1/+1 counters from target creature onto another target creature."),
      [{ effect: "move-counters", counter: { type: "+1/+1" }, count: "all", from: targetCreature,
        to: { ref: "target", filter: { ...creature, other: true }, count: one } }]);
    assert.deepEqual(steps("Move a counter from target creature onto a second target creature.")[0],
      { effect: "move-counters", counter: "any", count: one, from: targetCreature,
        to: { ref: "target", filter: { ...creature, other: true }, count: one } });
  });
  it("distributes counters among a chosen number of recipients", () => {
    assert.deepEqual(steps("Distribute three +1/+1 counters among one, two, or three target creatures."),
      [{ effect: "distribute-counters", counter: { type: "+1/+1" }, count: { amount: "fixed", value: 3 },
        among: { ref: "target", filter: creature, count: { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: 3 } } } }]);
    assert.deepEqual(steps("~ deals 3 damage divided as you choose among one, two, or three targets."),
      [{ effect: "damage", source: { ref: "self" }, amount: { amount: "fixed", value: 3 },
        to: { to: "divided", among: { ref: "any-target", count: { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: 3 } } } } }]);
    assert.deepEqual(steps("~ deals X damage divided evenly, rounded down, among any number of targets.")[0],
      { effect: "damage", source: { ref: "self" }, amount: { amount: "x" },
        to: { to: "divided", among: { ref: "any-target", count: { amount: "chosen", minimum: 0 } }, evenly: "round-down" } });
  });
  it("rejects near-misses", () => {
    fails("Move a +1/+1 counter from each creature onto target creature.");
    fails("Distribute three +1/+1 counters among each creature.");
    fails("Distribute three +1/+1 counters among one, three, or five target creatures.");
  });
});

describe("coin flips", () => {
  it("attaches both outcomes to the flip", () => {
    assert.deepEqual(steps("Flip a coin. If you win the flip, draw a card. If you lose the flip, you gain 2 life.")[0],
      { effect: "flip-coin", who: you, count: one, outcomes: [
        { result: "win", effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }] },
        { result: "lose", effects: [{ steps: [{ effect: "gain-life", who: you, amount: { amount: "fixed", value: 2 } }] }] },
      ] });
  });
  it("supports heads/tails, otherwise, until-lose and optional flips", () => {
    const s = steps("Flip a coin. If it comes up heads, draw a card. If it comes up tails, you lose 1 life.")[0];
    assert.equal(s.effect === "flip-coin" && s.outcomes?.map(o => o.result).join(), "heads,tails");
    const o = steps("Flip a coin. If you win the flip, draw a card. Otherwise, you lose 1 life.")[0];
    assert.equal(o.effect === "flip-coin" && o.outcomes?.map(x => x.result).join(), "win,lose");
    assert.deepEqual(steps("Flip a coin until you lose a flip."), [{ effect: "flip-coin", who: you, count: one, until: "lose" }]);
    assert.equal(steps("You may flip a coin. If you win the flip, draw a card.")[0].optional, true);
  });
  it("fails the whole line when an outcome cannot be represented or is ambiguous", () => {
    fails("Flip a coin. If you win the flip, draw a card and frobnicate.");
    fails("Flip a coin. If you win the flip, draw a card. If you win the flip, draw a card.");
    fails("Flip a coin. If you win the flip, draw a card. If it comes up tails, draw a card.");
    fails("Flip a coin. If you lose the flip, choose one of your opponents. That player gains control of this enchantment.");
    fails("Flip three coins. If you win the flip, draw a card.");
  });
});

describe("random selection", () => {
  it("marks the selection as random on the selected reference", () => {
    assert.deepEqual(steps("Return a creature card at random from your graveyard to your hand."),
      [{ effect: "return-to-hand", to: "your", what: { ref: "filter", count: one, random: true,
        filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } } }]);
    assert.deepEqual(steps("~ deals 2 damage to any target chosen at random.")[0],
      { effect: "damage", source: { ref: "self" }, amount: { amount: "fixed", value: 2 }, to: { to: "any-target", random: true } });
  });
  it("turns an imperative random choice into choose-random", () => {
    assert.deepEqual(steps("Choose an opponent at random."), [{ effect: "choose-random", what: { player: "an-opponent" } }]);
  });
  it("keeps the existing random discard", () => {
    assert.equal((steps("Discard a card at random.")[0] as { random?: boolean }).random, true);
  });
  it("rejects a random choice made by another player and unsupported targets", () => {
    fails("Target opponent chooses a permanent they control at random and sacrifices it.");
    fails("Destroy each creature at random.");
  });
  it("records random modal selection", () => {
    const result = parseOracleText("Choose one at random —\n• Draw a card.\n• You gain 2 life.", "Fixture");
    assert.ok(result.ok, JSON.stringify(result));
    const modal = (result.abilities[0] as { effects: Sentence[] }).effects[0].steps[0];
    assert.equal(modal.effect === "modal" && modal.random, true);
  });
});
