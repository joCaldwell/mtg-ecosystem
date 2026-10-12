import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { Effect, ObjectRef, Sentence } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseObjectRef, parseObjectSelection } from "../src/parser/refs.ts";

// Wording from .scryfall-cache/oracle-cards.json: Sparkmage's Gambit,
// Drakuseth, Arc Trail, Magma Burst, Hope and Glory, Living Inferno, Coordinated
// Assault, Join Forces, Dark Bargain, Goldwardens' Gambit, Kaboom!, Winter Blast.
// CR 115.3/115.4 (one "target" word, "another target"/"two targets"),
// 601.2c, 608.2c/f/h.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed", value }) as const;
const self = { ref: "self" } as const;
const eot = { duration: "end-of-turn" } as const;
const thoseCreatures: ObjectRef = { ref: "that", filter: { types: ["creature"] }, plural: true };

function sentences(text: string, typeLine = "Sorcery"): Sentence[] {
  const result = parseOracleText(text, "Fixture", { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  const ability = result.abilities[0];
  assert.ok(ability.kind === "spell");
  return ability.effects;
}
function fails(text: string, typeLine = "Sorcery"): void {
  assert.equal(parseOracleText(text, "Fixture", { typeLine }).ok, false, text);
}
function ref(text: string, parse = parseObjectRef): ObjectRef | null {
  const c = new Cursor(lex(text, "example"));
  const result = parse(c);
  return result && c.done() ? result : null;
}

describe("each of a counted selection", () => {
  it("keeps one declaration of N targets and lets follow-up references attach", () => {
    const targets = { ref: "target", filter: { types: ["creature"] }, count: fixed(2), upTo: true } as const;
    assert.deepEqual(sentences("~ deals 1 damage to each of up to two target creatures. Those creatures can't block this turn."), [
      { steps: [{ effect: "damage", source: self, amount: fixed(1), to: { to: "object", ref: targets } }] },
      { steps: [{ effect: "cant", what: thoseCreatures, action: "block", duration: { duration: "this-turn" } }] },
    ]);
    // The identity comes from the single "target" word (CR 115.3).
    assert.deepEqual(ref("each of two other target creatures"), {
      ref: "target", filter: { types: ["creature"], other: true }, count: fixed(2), selectionId: "example:4",
    });
    assert.deepEqual(ref("each of two creatures you control"), {
      ref: "filter", filter: { types: ["creature"], control: "you" }, count: fixed(2),
    });
  });
  it("does not read a singular or universal operand as a counted group", () => {
    for (const text of ["each of target creature", "each of a creature", "each of all creatures", "each of it"])
      assert.equal(ref(text), null, text);
  });
});

describe("targets without a noun (CR 115.4)", () => {
  it("parses counted, optional and other bare targets with their own identity", () => {
    const cases: [string, ObjectRef][] = [
      ["two targets", { ref: "any-target", count: fixed(2), selectionId: "example:1" }],
      ["up to three targets", { ref: "any-target", count: fixed(3), upTo: true, selectionId: "example:3" }],
      ["each of up to two other targets", { ref: "any-target", count: fixed(2), upTo: true, other: true, selectionId: "example:6" }],
      ["x targets", { ref: "any-target", count: { amount: "x" }, selectionId: "example:1" }],
      ["any other target", { ref: "any-target", other: true, selectionId: "example:2" }],
      ["another target", { ref: "any-target", other: true, selectionId: "example:1" }],
    ];
    for (const [text, expected] of cases) assert.deepEqual(ref(text), expected, text);
  });
  it("keeps the other-target contrast in a two-recipient damage instruction", () => {
    assert.deepEqual(sentences("~ deals 2 damage to any target and 1 damage to any other target."), [{ steps: [{
      effect: "simultaneous", steps: [
        { effect: "damage", source: self, amount: fixed(2), to: { to: "any-target" } },
        { effect: "damage", source: self, amount: fixed(1), to: { to: "object", ref: { ref: "any-target", other: true } } },
      ],
    }] }]);
  });
  it("leaves a following noun to the filtered-target and player grammars", () => {
    assert.deepEqual(ref("another target creature"), {
      ref: "target", filter: { types: ["creature"], other: true }, count: fixed(1), selectionId: "example:1",
    });
    for (const text of ["another target player", "two target", "one targets", "any other targets"])
      assert.equal(ref(text), null, text);
  });
});

describe("each of an established group", () => {
  const each = (group: ObjectRef): ObjectRef => ({ ref: "each", filter: { fromAmong: group } });
  it("quantifies the members individually without reselecting the group", () => {
    assert.deepEqual(ref("each of them"), each({ ref: "them" }));
    assert.deepEqual(ref("each of those creatures"), each(thoseCreatures));
    assert.deepEqual(sentences("Untap two target creatures. Each of them gets +1/+1 until end of turn.")[1], { steps: [{
      effect: "pump", what: each({ ref: "them" }), power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) }, duration: eot,
    }] });
  });
  it("keeps a member pronoun under the distributive source (Living Inferno)", () => {
    assert.deepEqual(sentences("Each of those creatures deals damage equal to its power to this creature."), [{ steps: [{
      effect: "damage", source: each(thoseCreatures), amount: { amount: "attribute", of: { ref: "it" }, attribute: "power" },
      to: { to: "object", ref: { ref: "this", noun: "creature" } },
    }] }]);
  });
  it("rejects an unrepresented restriction of the group and reciprocal references", () => {
    fails("Tap X target creatures. ~ deals 2 damage to each of those creatures with flying.");
    fails("Each of those creatures deals damage equal to its toughness to the other.");
  });
});

describe("floating distributive each", () => {
  it("is the same pump and grant over the plural subject", () => {
    const what: ObjectRef = { ref: "target", filter: { types: ["creature"] }, count: fixed(2), upTo: true };
    const expected: Effect[] = [
      { effect: "pump", what, power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) }, duration: eot },
      { effect: "gain-abilities", what, abilities: [{ gained: "keyword", keyword: { keyword: "first strike" } }], duration: eot },
    ];
    assert.deepEqual(sentences("Up to two target creatures each get +1/+0 and gain first strike until end of turn."), [{ steps: expected }]);
    assert.deepEqual(sentences("Up to two target creatures get +1/+0 and gain first strike until end of turn."), [{ steps: expected }]);
    assert.deepEqual(sentences("Untap up to two target creatures. They each get +2/+2 until end of turn.")[1], { steps: [{
      effect: "pump", what: { ref: "them" }, power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(2) }, duration: eot,
    }] });
  });
  it("requires a plural subject and plural agreement", () => {
    fails("Target creature each gets +2/+2 until end of turn.");
    fails("Two target creatures each gets +2/+2 until end of turn.");
    fails("This creature each get +2/+2 until end of turn.", "Creature — Elf");
  });
});

describe("the other member of an established group", () => {
  it("is the complement of the preceding selection, as the rest is", () => {
    const moves = (text: string) => sentences(text)[1];
    const expected = { steps: [{ effect: "simultaneous", steps: [
      { effect: "move-zone", what: { ref: "from-pool", pool: { ref: "them" }, count: fixed(2) }, to: { zone: "hand", owner: "your" } },
      { effect: "move-zone", what: { ref: "rest" }, to: { zone: "graveyard", owner: "your" } },
    ] }] };
    assert.deepEqual(moves("Look at the top three cards of your library. Put two of them into your hand and the other into your graveyard."), expected);
    assert.deepEqual(moves("Look at the top three cards of your library. Put two of them into your hand and the rest into your graveyard."), expected);
    assert.deepEqual(ref("the other", parseObjectSelection), { ref: "rest" });
  });
  it("is not offered outside action slots or for the other participant of an event", () => {
    assert.equal(ref("the other"), null);
    assert.equal(ref("the other creature", parseObjectSelection), null);
    fails("Whenever enchanted creature deals damage to a creature, destroy the other creature.", "Enchantment — Aura");
  });
});

describe("for each member of an established group", () => {
  it("accepts one action per member (Goldwardens' Gambit)", () => {
    assert.deepEqual(sentences("Create two 2/2 red Rebel creature tokens. For each of those tokens, you may attach an Equipment you control to it.")[1], { steps: [{
      effect: "for-each", processing: "instruction-wise",
      each: { ref: "each", filter: { fromAmong: { ref: "that", filter: { cls: "token" }, plural: true } } },
      effects: [{ steps: [{ effect: "attach", optional: true, who: { player: "you" }, to: { ref: "it" },
        what: { ref: "filter", filter: { subtypes: ["Equipment"], control: "you" }, count: fixed(1) } }] }],
    }] });
  });
  it("rejects a dependent action sequence that CR 608.2f completes member by member (Kaboom!)", () => {
    fails("Choose any number of target players or planeswalkers. For each of them, reveal cards from the top of your library until you reveal a nonland card, ~ deals damage equal to that card's mana value to that player or planeswalker, then you put the revealed cards on the bottom of your library in any order.");
  });
});
