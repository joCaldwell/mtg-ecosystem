import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Conditions. Wordings are taken from the
// cached Scryfall corpus; card names are noted per test.
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
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}
/** The intervening-if condition of a triggered ability. */
function interveningIf(text: string, typeLine: string, name?: string): unknown {
  const ability = only(text, typeLine, name);
  assert.equal(ability.kind, "triggered");
  if (ability.kind !== "triggered") throw new Error(text);
  return ability.condition;
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const creatureType = "Creature — Dwarf";

describe("enduring story (CR 702.195b)", () => {
  const story = { condition: "designation", who: { player: "you" }, designation: "enduring-story" };
  it("is a player designation tested continuously (Fíli the Pathfinder)", () => {
    assert.deepEqual(only("As long as you have an enduring story, creatures you control get +1/+1.", creatureType), {
      kind: "static", effect: { static: "conditional", condition: story, effect: { static: "modify",
        what: { ref: "each", filter: { types: ["creature"], control: "you" } },
        modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) } }] } },
    });
  });
  it("negates under unless (Bombur, Gentle Dreamer)", () => {
    assert.deepEqual(only("Bombur doesn't untap during your untap step unless you have an enduring story.", creatureType, "Bombur, Gentle Dreamer"), {
      kind: "static", effect: { static: "conditional",
        effect: { static: "untap-step-suppression", what: { ref: "self" }, step: { occurrence: "each", owner: { player: "you" } } },
        condition: { condition: "not", of: story } },
    });
  });
  it("is not the monarch, and other stories are not designations", () => {
    assert.deepEqual(interveningIf("When this creature enters, if you don't have an enduring story, you gain 1 life.", creatureType),
      { condition: "not", of: story });
    rejects("As long as you have a story, creatures you control get +1/+1.", creatureType);
    rejects("As long as you have the enduring story, creatures you control get +1/+1.", creatureType);
  });
});

describe("cards exiled with an object (CR 607.2a)", () => {
  const linked = (source: object, value: number) => ({ condition: "count",
    of: { cls: "card", relation: { relation: "linked-exile", source } }, comparison: { op: "ge", value: fixed(value) } });
  const bear = "Creature — Bear";
  const attacking = "Whenever this creature attacks, if ";
  it("reads a card being exiled with it as one or more linked cards (Smirking Spelljacker)", () => {
    assert.deepEqual(interveningIf(`${attacking}a card is exiled with it, you gain 1 life.`, bear), linked({ ref: "it" }, 1));
  });
  it("reads a bare plural as one or more (Evercoat Ursine, Valakut Exploration)", () => {
    assert.deepEqual(interveningIf(`${attacking}there are cards exiled with this creature, you gain 1 life.`, bear),
      linked({ ref: "this", noun: "creature" }, 1));
  });
  it("counts the linked cards under the perfect tense (Colfenor's Urn)", () => {
    assert.deepEqual(interveningIf("At the beginning of the end step, if three or more cards have been exiled with this artifact, you gain 1 life.", "Artifact"),
      linked({ ref: "this", noun: "artifact" }, 3));
  });
  it("needs the link and agreement, and a card rather than another noun", () => {
    rejects(`${attacking}a card is exiled, you gain 1 life.`, bear);
    rejects(`${attacking}a card are exiled with it, you gain 1 life.`, bear);
    rejects(`${attacking}three or more cards is exiled with it, you gain 1 life.`, bear);
    rejects(`${attacking}a creature is exiled with it, you gain 1 life.`, bear);
    rejects(`${attacking}there are cards in exile, you gain 1 life.`, bear);
    rejects(`${attacking}a card is exiled with target creature, you gain 1 life.`, bear);
  });
});

describe("the Ring-bearer chosen as the Ring tempted you (CR 701.54a, 701.54d)", () => {
  const human = "Legendary Creature — Human";
  it("is the temptation's own choice, not the current Ring-bearer (Gandalf, Friend of the Shire)", () => {
    assert.deepEqual(only("Whenever the Ring tempts you, if you chose a creature other than ~ as your Ring-bearer, draw a card.", human), {
      kind: "triggered", trigger: { trigger: "ring-tempts", who: { player: "you" } },
      condition: { condition: "ring-bearer-choice", who: { player: "you" }, chosen: { types: ["creature"], otherThan: { ref: "self" } } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
    });
  });
  it("exists only for a temptation event", () => {
    rejects("Whenever ~ attacks, if you chose a creature other than ~ as your Ring-bearer, draw a card.", human);
    rejects("As long as you chose a creature other than ~ as your Ring-bearer, ~ gets +1/+1.", human);
    rejects("Draw a card. If you chose a creature other than ~ as your Ring-bearer, draw a card.", "Instant");
    rejects("Whenever the Ring tempts you, draw a card if you chose a creature other than ~ as your Ring-bearer.", human);
  });
});
