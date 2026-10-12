import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, PlayerRef } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// CR 701 keyword actions, 725 (monarch), 726 (initiative), 701.38 (vote),
// 701.30 (clash), 111.10j-r (Roles):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };
const n = (value: number) => ({ amount: "fixed", value }) as const;
const targetCreature: ObjectRef = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
const myCreature: ObjectRef = { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(1) };

function parse(text: string): Ability[] {
  const result = parseOracleText(text, "Test");
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function steps(text: string): Effect[] {
  const [ability] = parse(text);
  assert.ok(ability.kind === "triggered" || ability.kind === "spell");
  return ability.effects.flatMap(sentence => sentence.steps);
}
function rejected(text: string): void {
  assert.equal(parseOracleText(text, "Test").ok, false, text);
}

describe("player designations and keyword actions", () => {
  it("parses monarch, initiative, Ring, venture and parameterless actions with their actor", () => {
    assert.deepEqual(steps("When this creature enters, you become the monarch."), [{ effect: "become-monarch", who: you }]);
    assert.deepEqual(steps("When this creature enters, target opponent takes the initiative."),
      [{ effect: "take-initiative", who: { player: "target-opponent" } }]);
    assert.deepEqual(steps("When this creature enters, the Ring tempts you."), [{ effect: "ring-tempts", who: you }]);
    assert.deepEqual(steps("When this creature enters, venture into the dungeon."), [{ effect: "venture", who: you }]);
    assert.deepEqual(steps("When this creature enters, open an Attraction."), [{ effect: "open-attraction", who: you }]);
    assert.deepEqual(steps("When this creature dies, learn."), [{ effect: "learn", who: you }]);
    assert.deepEqual(steps("When this creature enters, you take the initiative and create a Treasure token.").map(e => e.effect),
      ["take-initiative", "create-token"]);
  });

  it("rejects near misses that would drop the actor or a variant", () => {
    rejected("When this creature enters, take the initiative.");
    rejected("When this creature enters, venture into the Undercity.");
    rejected("When this creature enters, you become the monarch of the realm.");
  });

  it("keeps amounts, repetition and implied targets of incubate, earthbend, endure", () => {
    assert.deepEqual(steps("When this creature enters, incubate 3."), [{ effect: "incubate", who: you, amount: n(3) }]);
    assert.deepEqual(steps("Incubate X twice, where X is the number of lands you control."), [{ effect: "incubate", who: you, amount: { amount: "x" }, times: n(2) }]);
    assert.deepEqual(steps("When this creature enters, earthbend 2."), [{ effect: "earthbend", who: you, amount: n(2),
      what: { ref: "target", filter: { types: ["land"], control: "you" }, count: n(1) } }]);
    assert.deepEqual(steps("When this creature enters, it endures 2."), [{ effect: "endure", what: { ref: "it" }, amount: n(2) }]);
    rejected("When this creature enters, incubate 2 that many times.");
  });

  it("goad keeps its object; airbend keeps its object", () => {
    assert.deepEqual(steps("Whenever this creature attacks, goad target creature."), [{ effect: "goad", what: targetCreature }]);
    assert.deepEqual(steps("When this creature enters, airbend target creature you control."), [{ effect: "airbend", who: you, what: myCreature }]);
    rejected("Goad.");
  });

  it("repeats proliferate/investigate and parameterises explore/connive", () => {
    assert.deepEqual(steps("When this creature enters, proliferate twice."), [{ effect: "keyword-action", action: "proliferate", who: you, times: n(2) }]);
    assert.deepEqual(steps("Investigate X times, where X is the number of creatures you control.")[0],
      { effect: "keyword-action", action: "investigate", who: you, times: { amount: "x" } });
    assert.deepEqual(steps("Target creature you control explores, then it explores again."), [{ effect: "explore", what: myCreature, times: n(2) }]);
    assert.deepEqual(steps("Target creature you control connives 2."), [{ effect: "connive", what: myCreature, amount: n(2) }]);
    // plain forms are unchanged
    assert.deepEqual(steps("Target creature you control connives."), [{ effect: "connive", what: myCreature }]);
    assert.deepEqual(steps("Proliferate."), [{ effect: "keyword-action", action: "proliferate", who: you }]);
  });

  it("scry by another player records the actor; scry by you does not", () => {
    assert.deepEqual(steps("Target player scries 2."), [{ effect: "scry", amount: n(2), who: { player: "target-player" } }]);
    assert.deepEqual(steps("You scry 2."), [{ effect: "scry", amount: n(2) }]);
  });

  it("Role tokens keep role, count and attachment; undefined roles fail", () => {
    assert.deepEqual(steps("When this creature enters, create a Young Hero Role token attached to target creature you control."),
      [{ effect: "create-role", role: "Young Hero", count: n(1), attachedTo: myCreature }]);
    rejected("When this creature enters, create a Chef Role token attached to target creature you control.");
    rejected("When this creature enters, create a Wicked Role token.");
  });

  it("clash result is a condition on the following sentence and needs a clash", () => {
    const sentences = (parse("When this creature enters, clash with an opponent. If you win, put a +1/+1 counter on this creature.")[0] as Extract<Ability, { kind: "triggered" }>).effects;
    assert.deepEqual(sentences[0].steps, [{ effect: "clash", who: you, with: { player: "an-opponent" } }]);
    assert.deepEqual(sentences[1].condition, { condition: "clash-result", who: you, result: "won" });
    rejected("When this creature enters, draw a card. If you win, put a +1/+1 counter on this creature.");
  });

  it("goaded static modifier", () => {
    const [ability] = parse("Enchanted creature gets +2/+2 and is goaded.");
    assert.ok(ability.kind === "static" && ability.effect.static === "modify");
    assert.deepEqual(ability.effect.modifiers.at(-1), { modifier: "goaded" });
  });

  it("speed is a quantity of the player", () => {
    const [ability] = parse("Other creatures you control get +X/+0, where X is your speed.");
    assert.ok(ability.kind === "static" && ability.effect.static === "modify");
    assert.deepEqual(ability.effect.bindings, [{ variable: "x", value: { amount: "speed", who: you } }]);
  });
});

describe("voting", () => {
  const text = "Starting with you, each player votes for death or taxes. If death gets more votes, each opponent loses 4 life. If taxes gets more votes or the vote is tied, draw a card.";
  it("records the vote, its choices and result conditions", () => {
    const [ability] = parse(text);
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "vote", starting: you, choices: { choices: "named", names: ["death", "taxes"] } }]);
    assert.deepEqual(ability.effects[1].condition, { condition: "vote-result", choice: "death", result: "more" });
    assert.deepEqual(ability.effects[2].condition, { condition: "vote-result", choice: "taxes", result: "more-or-tied" });
  });

  it("scales or repeats per vote and counts votes", () => {
    const [ability] = parse("Starting with you, each player votes for profit or security. You create a number of Treasure tokens equal to twice the number of profit votes. Each opponent sacrifices a creature for each security vote.");
    assert.ok(ability.kind === "spell");
    assert.deepEqual((ability.effects[1].steps[0] as Extract<Effect, { effect: "create-token" }>).count,
      { amount: "twice", of: { amount: "vote-count", choice: "profit" } });
    assert.deepEqual(ability.effects[2].steps[0].forEachVote, { choice: "security" });
  });

  it("rejects results naming no offered choice, and per-vote scans/optional repeats", () => {
    rejected("If death gets more votes, draw a card.");
    rejected("Starting with you, each player votes for death or taxes. If war gets more votes, draw a card.");
    rejected("Starting with you, each player votes for wild or free. Reveal cards from the top of your library until you reveal a creature card for each wild vote.");
    rejected("Starting with you, each player votes for wild or free. You may put a permanent card from your hand onto the battlefield for each free vote.");
  });
});
