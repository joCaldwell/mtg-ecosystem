import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Oracle wordings checked in the frozen corpus;
// the cards are named with each construct.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function rejected(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("the owner or controller of an object", () => {
  // Oblation, Chaos Warp, Cathartic Parting. CR 108.3-4: the same player as
  // "target nonland permanent's owner".
  it("is the possessive owner reference", () => {
    const target = { ref: "target", filter: { nonTypes: ["land"], cls: "permanent" }, count: n(1) };
    assert.deepEqual(parsed("The owner of target nonland permanent shuffles it into their library, then draws two cards.", "Instant"), [{
      kind: "spell", effects: [{ steps: [
        { effect: "shuffle-into-library", who: { player: "owner", of: target }, what: [{ ref: "it" }], library: { zone: "library", owner: "their" } },
        { effect: "draw", who: { player: "owner", of: target }, amount: n(2) },
      ] }],
    }]);
    assert.deepEqual(parsed("The owner of target nonland permanent shuffles it into their library, then draws two cards.", "Instant"),
      parsed("Target nonland permanent's owner shuffles it into their library, then draws two cards.", "Instant"));
  });
  it("rejects a distributive owner of several objects", () => {
    rejected("The owner of each creature draws a card.", "Sorcery");
    rejected("The owner of two target creatures draws a card.", "Sorcery");
  });
});

describe("library placement chosen between two positions", () => {
  // Run Behind, Misleading Motes, Deem Inferior, Not Forgotten, Arashin
  // Sovereign. CR 608.2d: exactly one placement, chosen by the putting player
  // as the instruction is carried out; CR 401.7 for "second from the top".
  const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
  const owner = { player: "owner", of: target };
  it("represents their choice of the top or bottom as a choice of two moves", () => {
    assert.deepEqual(parsed("Target creature's owner puts it on their choice of the top or bottom of their library.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "choice", who: owner, options: [
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "their", position: "top" }, who: owner },
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "their", position: "bottom" }, who: owner },
      ] }] }],
    }]);
  });
  it("keeps your choice and its optionality", () => {
    assert.deepEqual(parsed("When this creature dies, you may put it on your choice of the top or bottom of its owner's library.", "Creature — Dragon"), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "choice", who: { player: "you" }, optional: true, options: [
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "its-owner", position: "top" } },
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "its-owner", position: "bottom" } },
      ] }] }],
    }]);
  });
  it("chooses between a depth from the top and the bottom", () => {
    const permanent = { player: "owner", of: { ref: "target", filter: { nonTypes: ["land"], cls: "permanent" }, count: n(1) } };
    assert.deepEqual(parsed("The owner of target nonland permanent puts it into their library second from the top or on the bottom.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "choice", who: permanent, options: [
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "their", position: "top", depth: 2 }, who: permanent },
        { effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "their", position: "bottom" }, who: permanent },
      ] }] }],
    }]);
  });
  it("rejects a chooser other than the putting player and single positions stay moves", () => {
    rejected("Its owner puts it on your choice of the top or bottom of their library.", "Instant");
    rejected("Put target creature on their choice of the top or bottom of its owner's library.", "Instant");
    assert.deepEqual(parsed("Target creature's owner puts it on top of their library.", "Instant")[0].effects[0].steps[0].effect, "move-zone");
  });
});

describe("each other player and each other opponent", () => {
  // Syphon Mind, Grave Pact, Zenith Chronicler, Hydra Omnivore. "Other"
  // contrasts with you or with a player the ability named (Zenith Chronicler:
  // the caster); it stays an unresolved contrast, as ObjectFilter.other does.
  const otherPlayer = { player: "filtered", base: { player: "each-player" }, filter: { other: true } };
  const otherOpponent = { player: "filtered", base: { player: "each-opponent" }, filter: { other: true } };
  it("quantifies over every player but the contrasted one", () => {
    assert.deepEqual(parsed("Each other player discards a card. You draw a card for each card discarded this way.", "Sorcery")[0].effects[0], {
      steps: [{ effect: "discard", who: otherPlayer, what: { ref: "filter", filter: { cls: "card" }, count: n(1) } }],
    });
    assert.deepEqual(parsed("Whenever a player casts their first multicolored spell each turn, each other player draws a card.", "Creature — Human Wizard")[0].effects, [
      { steps: [{ effect: "draw", who: otherPlayer, amount: n(1) }] }]);
    assert.deepEqual(parsed("Whenever a creature you control dies, each other player sacrifices a creature of their choice.", "Enchantment")[0].effects, [
      { steps: [{ effect: "sacrifice", who: otherPlayer, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: otherPlayer }] }]);
    assert.deepEqual(parsed("Whenever this creature deals combat damage to an opponent, it deals that much damage to each other opponent.", "Creature — Hydra")[0].effects, [
      { steps: [{ effect: "damage", source: { ref: "it" }, amount: { amount: "that-much" }, to: { to: "player", ref: otherOpponent } }] }]);
  });
  it("is not bound inside a replacement and does not read other quantifiers", () => {
    rejected("If you would lose the game, instead each other player draws a card.", "Enchantment");
    rejected("Whenever a player casts a spell, any other player may pay that spell's mana cost. If a player does, counter the spell.", "Snow Land");
  });
});

describe("several target players declared by one target word", () => {
  // Hunters' Feast, Huddle Up, Donatello's Science Lesson, Priest of Forgotten
  // Gods, Wheel and Deal, Curse of Surveillance. CR 115.3, 601.2c.
  const anyNumber = { player: "target-player", count: { amount: "chosen", minimum: 0 } };
  it("keeps the count and makes each of them act", () => {
    assert.deepEqual(parsed("Any number of target players each gain 6 life.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "gain-life", who: anyNumber, amount: n(6) }] }] }]);
    assert.deepEqual(parsed("Two target players each draw a card.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "draw", who: { player: "target-player", count: n(2) }, amount: n(1) }] }] }]);
    assert.deepEqual(parsed("Tap up to two target creatures. Up to two target players each draw a card.", "Instant")[0].effects[1], { steps: [
      { effect: "draw", who: { player: "target-player", count: n(2), upTo: true }, amount: n(1) }] });
  });
  it("keeps them as the actor of coordinated plural verbs", () => {
    assert.deepEqual(parsed("{T}, Sacrifice two other creatures: Any number of target players each lose 2 life and sacrifice a creature of their choice. You add {B}{B} and draw a card.", "Creature — Human Cleric")[0].effects[0], { steps: [
      { effect: "lose-life", who: anyNumber, amount: n(2) },
      { effect: "sacrifice", who: anyNumber, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: anyNumber }] });
    const opponents = { player: "target-opponent", count: { amount: "chosen", minimum: 0 } };
    assert.deepEqual(parsed("Any number of target opponents each discard their hands, then draw seven cards.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "discard", who: opponents, what: "hand" }, { effect: "draw", who: opponents, amount: n(7) }] }] }]);
  });
  it("keeps a player qualifier on the declaration", () => {
    assert.deepEqual(parsed("At the beginning of enchanted player's upkeep, any number of target players other than that player each draw cards equal to the number of Curses attached to that player.", "Enchantment — Aura Curse")[0].effects, [{ steps: [{
      effect: "draw", who: { player: "filtered", base: anyNumber, filter: { otherThan: { player: "that-player" } } },
      amount: { amount: "count", of: { subtypes: ["Curse"], attachedTo: { player: "that-player" } } } }] }]);
  });
  it("rejects a plural target subject without the distributive each and a single number", () => {
    rejected("Two target players draw a card.", "Sorcery");
    rejected("One target players each draw a card.", "Sorcery");
  });
});
