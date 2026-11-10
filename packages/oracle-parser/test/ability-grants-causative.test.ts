import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings from the cached Scryfall corpus (Hollowsage, Blood Seeker, Ob
// Nixilis the Fallen, Assault Suit, Predatory Nightstalker, Faerie
// Tauntings). "You may have [player] [act]": you decide (CR 608.2d), the
// named player performs the action.
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
function effects(text: string, typeLine: string) {
  const ability = only(text, typeLine);
  if (ability.kind !== "triggered") throw new Error(text);
  return ability.effects;
}
function rejects(text: string, typeLine: string) {
  assert.equal(parse(text, typeLine).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const aCard = { ref: "filter", filter: { cls: "card" }, count: fixed(1) };

describe("causative 'you may have [player] ...'", () => {
  it("keeps you as the decider and the named player as the actor", () => {
    assert.deepEqual(effects("Whenever this creature becomes untapped, you may have target player discard a card.", "Creature — Merfolk Wizard"),
      [{ steps: [{ effect: "discard", who: { player: "target-player" }, what: aCard, optional: true }] }]);
    assert.deepEqual(effects("Whenever a creature an opponent controls enters, you may have that player lose 1 life.", "Creature — Vampire Shaman"),
      [{ steps: [{ effect: "lose-life", who: { player: "that-player" }, amount: fixed(1), optional: true }] }]);
    assert.deepEqual(effects("Whenever you cast a spell during an opponent's turn, you may have each opponent lose 1 life.", "Kindred Enchantment — Faerie"),
      [{ steps: [{ effect: "lose-life", who: { player: "each-opponent" }, amount: fixed(1), optional: true }] }]);
  });

  it("differs from the mandatory instruction only by the choice", () => {
    assert.deepEqual(effects("Whenever this creature becomes untapped, target player discards a card.", "Creature — Merfolk Wizard"),
      [{ steps: [{ effect: "discard", who: { player: "target-player" }, what: aCard }] }]);
  });

  it("lets 'if you do' test your decision", () => {
    assert.deepEqual(effects("Landfall — Whenever a land you control enters, you may have target player lose 3 life. If you do, put three +1/+1 counters on ~.", "Legendary Creature — Demon"), [
      { steps: [{ effect: "lose-life", who: { player: "target-player" }, amount: fixed(3), optional: true }] },
      { condition: { condition: "you-do" }, conditionKind: "if",
        steps: [{ effect: "put-counters", on: { ref: "self" }, counter: { type: "+1/+1" }, count: fixed(3) }] },
    ]);
  });

  it("keeps the acting player's own choice and a control duration", () => {
    assert.deepEqual(effects("When this creature enters, you may have target opponent sacrifice a creature of their choice.", "Creature — Nightstalker"),
      [{ steps: [{ effect: "sacrifice", who: { player: "target-opponent" },
        what: { ref: "filter", filter: { types: ["creature"] }, count: fixed(1) }, chosenBy: { player: "target-opponent" }, optional: true }] }]);
    assert.deepEqual(effects("At the beginning of each opponent's upkeep, you may have that player gain control of equipped creature until end of turn. If you do, untap it.", "Artifact — Equipment"), [
      { steps: [{ effect: "gain-control", who: { player: "that-player" }, what: { ref: "equipped", noun: "creature" },
        duration: { duration: "end-of-turn" }, optional: true }] },
      { condition: { condition: "you-do" }, conditionKind: "if", steps: [{ effect: "untap", what: { ref: "it" } }] },
    ]);
  });

  it("rejects a named player who is not the performer", () => {
    rejects("When this creature enters, you may have target player creatures.", "Creature — Zombie");
    rejects("When this creature enters, you may have target player.", "Creature — Zombie");
    rejects("When this creature enters, have target player discard a card.", "Creature — Zombie");
  });
});

// Risk Factor, Shakedown Heavy, Terrapact Intimidator: another player
// decides, but the instruction keeps its own actor or damage source.
describe("causative '[player] may have ...'", () => {
  it("records the decider apart from the damage source and recipient", () => {
    assert.deepEqual(only("Target opponent may have ~ deal 4 damage to them. If that player doesn't, you draw three cards.", "Instant"), {
      kind: "spell", effects: [
        { steps: [{ effect: "damage", source: { ref: "self" }, amount: fixed(4), to: { to: "player", ref: { player: "they" } },
          optional: true, decidedBy: { player: "target-opponent" } }] },
        { condition: { condition: "preceding-action", who: { player: "that-player" }, outcome: "not-performed" }, conditionKind: "if",
          steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(3) }] },
      ],
    });
  });

  it("keeps you as the actor when an opponent decides", () => {
    assert.deepEqual(effects("Whenever this creature attacks, defending player may have you draw a card. If they do, untap this creature and remove it from combat.", "Creature — Ogre Warrior"), [
      { steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1), optional: true, decidedBy: { player: "defending-player" } }] },
      { condition: { condition: "preceding-action", who: { player: "they" }, outcome: "performed" }, conditionKind: "if",
        steps: [{ effect: "untap", what: { ref: "this", noun: "creature" } }, { effect: "remove-from-combat", what: { ref: "it" } }] },
    ]);
    assert.deepEqual(effects("When this creature enters, target opponent may have you create two Lander tokens.", "Creature — Kavu Scout"),
      [{ steps: [{ effect: "create-token", count: fixed(2), token: { predefined: "Lander" }, optional: true, decidedBy: { player: "target-opponent" } }] }]);
  });

  it("differs from the decider acting themselves", () => {
    assert.deepEqual(effects("When this creature enters, target opponent may create two Lander tokens.", "Creature — Kavu Scout"),
      [{ steps: [{ effect: "create-token", count: fixed(2), token: { predefined: "Lander" }, who: { player: "target-opponent" },
        optional: true, decidedBy: { player: "target-opponent" } }] }]);
  });

  it("rejects non-damage object subjects and a pronoun without one antecedent", () => {
    rejects("Target opponent may have ~ destroy target creature.", "Instant");
    rejects("Each opponent may have ~ deal 4 damage to them.", "Instant");
    rejects("Target opponent may have.", "Instant");
  });
});
