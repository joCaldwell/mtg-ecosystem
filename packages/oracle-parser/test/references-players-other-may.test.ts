import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const you = { player: "you" } as const;
const thatPlayer = { player: "that-player" } as const;
const they = { player: "they" } as const;
const eachPlayer = { player: "each-player" } as const;
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// Oracle texts are complete cards (or complete abilities of cards) from the
// fixed Scryfall corpus. "[Player] may [do something]. If [that player] does
// / doesn't, …" is CR 118.12; the choice is made by that player while the
// effect is applied (608.2d), by several players in APNAP order (101.4):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("optional instructions decided by another player", () => {
  it("records Fecundity's controller as both decider and actor", () => {
    const controller = { player: "controller", of: { ref: "that", filter: { types: ["creature"] } } };
    assert.deepEqual(parsed("Whenever a creature dies, that creature's controller may draw a card.", "Fecundity", "Enchantment"), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } },
      effects: [{ steps: [{ effect: "draw", who: controller, amount: n(1), optional: true, decidedBy: controller }] }],
    }]);
  });
  it("leaves your own optional instruction without a decider", () => {
    assert.deepEqual(parsed("You may draw a card.", undefined, "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "draw", who: you, amount: n(1), optional: true }] }] }]);
  });
  it("lets each player decide separately in Show and Tell", () => {
    assert.deepEqual(parsed("Each player may put an artifact, creature, enchantment, or land card from their hand onto the battlefield.", "Show and Tell", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "move-zone",
        what: { ref: "filter", filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] }, { types: ["land"] }] }],
          cls: "card", zone: { zone: "hand", owner: "their" } }, count: n(1) },
        to: { zone: "battlefield" }, who: eachPlayer, optional: true, decidedBy: eachPlayer }] }],
    }]);
  });
  it("groups the coordinated verbs of one \"may\" under that player (Ghost Quarter)", () => {
    const controller = { player: "controller", of: { ref: "it" } };
    assert.deepEqual(parsed("{T}, Sacrifice this land: Destroy target land. Its controller may search their library for a basic land card, put it onto the battlefield, then shuffle.", "Ghost Quarter", "Land")[0].effects, [
      { steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["land"] }, count: n(1) } }] },
      { steps: [{ effect: "optional-group", who: controller, steps: [
        { effect: "search", who: controller, zones: { selection: "all", of: [{ zone: "library", owner: "their" }] },
          for: { ref: "filter", filter: { supertypes: ["basic"], types: ["land"], cls: "card" }, count: n(1) } },
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, who: controller },
        { effect: "shuffle", who: controller, library: { zone: "library", owner: controller } },
      ] }] },
    ]);
  });
  it("keeps a mandatory instruction of yours outside the other player's choice (Tataru Taru)", () => {
    const opponent = { player: "target-opponent" };
    assert.deepEqual(parsed("When Tataru Taru enters, you draw a card and target opponent may draw a card.", "Tataru Taru", "Legendary Creature — Dwarf Advisor")[0].effects, [{ steps: [
      { effect: "draw", who: you, amount: n(1) },
      { effect: "draw", who: opponent, amount: n(1), optional: true, decidedBy: opponent },
    ] }]);
  });
  it("offers one of two actions to a single player", () => {
    const put = { effect: "put-counters", on: { ref: "this", noun: "artifact" }, counter: { type: "doom" }, count: n(1), who: thatPlayer };
    assert.deepEqual(parsed("At the beginning of each player's upkeep, that player may put a doom counter on this artifact or put two doom counters on this artifact.", undefined, "Artifact")[0].effects, [{ steps: [
      { effect: "choice", who: thatPlayer, options: [put, { ...put, count: n(2) }], optional: true, decidedBy: thatPlayer }] }]);
  });
  it("rejects instructions the deciding player would not perform", () => {
    // No actor of its own: accepting these would make the controller act.
    fails("That player may destroy target creature.");
    // Scry carries its own actor (CR 701.22a), so this is now represented: the opponents scry.
    assert.deepEqual(parsed("Each opponent may scry 2.", undefined, "Sorcery")[0].effects, [{ steps: [
      { effect: "scry", amount: n(2), who: { player: "each-opponent" }, optional: true, decidedBy: { player: "each-opponent" } }] }]);
    fails("That player may draw a card, then destroy target creature.");
    // One choice per player cannot be a single choice of a group.
    fails("Each opponent may discard a card or sacrifice a permanent.");
    fails("Any player may draw a card.");
  });
});

describe("whether another player took the preceding action", () => {
  const pay = (who: object, symbols: string[]) => ({ effect: "pay-cost", who, costs: [{ cost: "mana", symbols }], optional: true, decidedBy: who });
  it("tests Smothering Tithe's declined payment", () => {
    assert.deepEqual(parsed("Whenever an opponent draws a card, that player may pay {2}. If the player doesn't, you create a Treasure token.", "Smothering Tithe", "Enchantment")[0].effects, [
      { steps: [pay(thatPlayer, ["2"])] },
      { steps: [{ effect: "create-token", count: n(1), token: { predefined: "Treasure" } }], conditionKind: "if",
        condition: { condition: "preceding-action", who: { player: "the-player" }, outcome: "not-performed" } },
    ]);
  });
  it("keeps the printed pronoun of each follow-up (Kuroki, Thief of Talents)", () => {
    const opponent = { player: "target-opponent" };
    const effects = parsed("At the beginning of your end step, target opponent may draw four cards. If they do, look at that player's hand and you may cast a spell from their hand without paying its mana cost. If they don't, put two +1/+1 counters on Kuroki.", "Kuroki, Thief of Talents", "Legendary Creature — Human Ninja")[0].effects;
    assert.deepEqual(effects[0], { steps: [{ effect: "draw", who: opponent, amount: n(4), optional: true, decidedBy: opponent }] });
    assert.deepEqual(effects[1].condition, { condition: "preceding-action", who: they, outcome: "performed" });
    assert.deepEqual(effects[2], { steps: [{ effect: "put-counters", on: { ref: "self" }, counter: { type: "+1/+1" }, count: n(2) }],
      conditionKind: "if", condition: { condition: "preceding-action", who: they, outcome: "not-performed" } });
  });
  it("also follows a mandatory action (Gisa's Favorite Shovel)", () => {
    const defender = { player: "defending-player" };
    assert.deepEqual(parsed("Whenever equipped creature attacks, defending player sacrifices a creature of their choice. If they do, you create a Walker token.", undefined, "Artifact — Equipment")[0].effects, [
      { steps: [{ effect: "sacrifice", who: defender, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: defender }] },
      { steps: [{ effect: "create-token", count: n(1), token: { predefined: "Walker" } }], conditionKind: "if",
        condition: { condition: "preceding-action", who: they, outcome: "performed" } },
    ]);
  });
  it("gives the fallback of Fraying Line to you, not to the paying player", () => {
    const effects = parsed("At the beginning of each player's upkeep, that player may pay {2}. If they do, they put a rope counter on a creature they control. Otherwise, exile this artifact and each creature without a rope counter on it, then remove all rope counters from all creatures.", "Fraying Line", "Artifact")[0].effects;
    assert.deepEqual(effects[0], { steps: [pay(thatPlayer, ["2"])] });
    assert.deepEqual(effects[1].steps, [{ effect: "put-counters", on: { ref: "filter", filter: { types: ["creature"], controlledBy: they }, count: n(1) },
      counter: { type: "rope" }, count: n(1), who: they }]);
    assert.deepEqual(effects[1].condition, { condition: "preceding-action", who: they, outcome: "performed" });
    assert.deepEqual(effects[1].otherwise, { on: "condition-false", steps: [
      { effect: "simultaneous", steps: [
        { effect: "exile", what: { ref: "this", noun: "artifact" } },
        { effect: "exile", what: { ref: "each", filter: { types: ["creature"], counterConstraints: [{ counter: { type: "rope" }, comparison: { op: "eq", value: n(0) } }] } } },
      ] },
      { effect: "remove-counters", counter: { type: "rope" }, count: "all", from: { ref: "all", filter: { types: ["creature"] } } },
    ] });
  });
  it("keeps your own outcome nodes and rejects restated or ungrammatical clauses", () => {
    assert.deepEqual(parsed("You may draw a card. If you do, discard a card.", undefined, "Sorcery")[0].effects[1].condition, { condition: "you-do" });
    fails("Target opponent may draw a card. If they does, you draw a card.");
    fails("Target opponent may draw a card. If that player do, you draw a card.");
    fails("Target opponent may draw a card. If they don't draw a card, you draw a card.");
    fails("Target opponent may draw a card. If an opponent does, you draw a card.");
  });
});

describe("players selected by what they did with the preceding instruction", () => {
  const eachOpponent = { player: "each-opponent" } as const;
  const opponentsWho = (precedingAction: string) => ({ player: "filtered", base: eachOpponent, filter: { precedingAction } });
  const treasure = { effect: "create-token", count: n(1), token: { predefined: "Treasure" } };
  it("repeats Tempting Contract's reward for each opponent who accepted", () => {
    assert.deepEqual(parsed("At the beginning of your upkeep, each opponent may create a Treasure token. For each opponent who does, you create a Treasure token.", "Tempting Contract", "Enchantment")[0].effects, [
      { steps: [{ ...treasure, who: eachOpponent, optional: true, decidedBy: eachOpponent }] },
      { steps: [{ effect: "for-each", each: opponentsWho("performed"), effects: [{ steps: [treasure] }], processing: "instruction-wise" }] },
    ]);
  });
  it("draws for the opponents who declined (Kynaios and Tiro of Meletis)", () => {
    assert.deepEqual(parsed("At the beginning of your end step, draw a card. Each player may put a land card from their hand onto the battlefield, then each opponent who didn't draws a card.", "Kynaios and Tiro of Meletis", "Legendary Creature — Human Soldier")[0].effects[1].steps, [
      { effect: "move-zone", what: { ref: "filter", filter: { types: ["land"], cls: "card", zone: { zone: "hand", owner: "their" } }, count: n(1) },
        to: { zone: "battlefield" }, who: eachPlayer, optional: true, decidedBy: eachPlayer },
      { effect: "draw", who: opponentsWho("not-performed"), amount: n(1) },
    ]);
  });
  it("distinguishes being unable from declining (Cruel Grimnarch)", () => {
    assert.deepEqual(parsed("When this creature enters, each opponent discards a card. For each opponent who can't, you gain 4 life.", "Cruel Grimnarch", "Creature — Phyrexian Cleric")[0].effects[1].steps, [
      { effect: "for-each", each: opponentsWho("unable"), effects: [{ steps: [{ effect: "gain-life", who: you, amount: n(4) }] }], processing: "instruction-wise" }]);
  });
  it("reads a restated control test as its own predicate (Thornbow Archer)", () => {
    assert.deepEqual(parsed("Whenever this creature attacks, each opponent who doesn't control an Elf loses 1 life.", "Thornbow Archer", "Creature — Elf Archer")[0].effects, [{ steps: [
      { effect: "lose-life", who: { player: "filtered", base: eachOpponent, filter: { controls: { filter: { subtypes: ["Elf"] }, comparison: { op: "eq", value: n(0) } } } }, amount: n(1) }] }]);
    assert.deepEqual(parsed("Each opponent who controls no Elves loses 1 life.", undefined, "Sorcery")[0].effects[0].steps[0].who.filter,
      { controls: { filter: { subtypes: ["Elf"] }, comparison: { op: "eq", value: n(0) } } });
  });
  it("does not take a restated action for the bare auxiliary", () => {
    fails("Each opponent discards a card. Then each opponent who didn't discard a creature card loses 4 life.");
    fails("Each opponent who doesn't draw a card loses 1 life.");
    fails("Each opponent who doesn't control two Elves loses 1 life.");
  });
});
