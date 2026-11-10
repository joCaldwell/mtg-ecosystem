import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ObjectRef, StaticEffect, StaticModifier } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseStatic } from "../src/parser/statics.ts";

// Exact card clauses checked against cached Scryfall corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 611.3a-b: continuous effects continually evaluate their affected objects.
// CR 508.1h/509.1d-f: combat costs belong to declaration, not resolution.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;
const enchanted = { ref: "enchanted", noun: "creature" } as const;
const source = { ref: "this", noun: "creature" } as const;
const loseAll: StaticModifier = { modifier: "lose-abilities", removal: { removal: "all" } };
const pt = (power: number, toughness: number): StaticModifier => ({ modifier: "pt",
  power: { sign: 1, amount: fixed(power) }, toughness: { sign: 1, amount: fixed(toughness) } });
const keyword = (name: "vigilance" | "trample"): StaticModifier => ({ modifier: "abilities",
  abilities: [{ gained: "keyword", keyword: { keyword: name } }] });
const modify = (what: ObjectRef, modifiers: StaticModifier[]): StaticEffect => ({ static: "modify", what, modifiers });
const suppress = (what: ObjectRef): StaticEffect => ({ static: "untap-step-suppression", what,
  step: { occurrence: "each", owner: "each-affected-objects-controller" } });

function one(text: string, name?: string): Ability {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe("coordinated continuous static clauses", () => {
  it("treats indefinite static subjects as generic classes while preserving targets", () => {
    // Grammar regression: static restrictions/characteristic grants do not
    // instruct a player to select one member of an indefinite subject class.
    const flying = { ref: "each", filter: { types: ["creature"], withAbility: ["flying"] } } as const;
    const haste = [{ gained: "keyword", keyword: { keyword: "haste" } }];
    assert.deepEqual(one("A creature with flying has haste."), { kind: "static", effect: {
      static: "modify", what: flying, modifiers: [{ modifier: "abilities", abilities: haste }],
    } });
    assert.deepEqual(one("An artifact creature can't block."), { kind: "static", effect: {
      static: "cant", what: { ref: "each", filter: { types: ["artifact", "creature"] } }, action: "block",
    } });
    assert.deepEqual(one("A creature with flying attacks each combat if able."), { kind: "static", effect: {
      static: "must-attack", what: flying,
    } });
    assert.deepEqual(one("Target creature gains haste until end of turn."), { kind: "spell", effects: [{ steps: [{
      effect: "gain-abilities", what: { ref: "target", filter: { types: ["creature"] }, count: fixed(1) },
      abilities: haste, duration: { duration: "end-of-turn" },
    }] }] });
  });
  it("keeps a land's two entry replacements attached to the same entering object", () => {
    // Thriving Moor versus Night Market and Sunken Citadel: both clauses
    // alter entry, neither is a post-entry resolving tap/choice sequence.
    const land = { ref: "this", noun: "land" } as const;
    for (const [text, choice] of [
      ["This land enters tapped. As it enters, choose a color.", { category: "color" }],
      ["This land enters tapped. As it enters, choose a color other than black.", { category: "color", excluding: ["black"] }],
    ] as const) assert.deepEqual(one(text), { kind: "static", effect: { static: "group", effects: [
      { static: "enters-tapped", what: land },
      { static: "as-enters", what: land, effects: [{ steps: [{ effect: "choose-value", who: you, ...choice }] }] },
    ] } });
    // An explicitly different entering object cannot inherit this land.
    const wrongSource = new Cursor(lex("This land enters tapped. As this creature enters, choose a color."));
    assert.equal(parseStatic(wrongSource), null);
    assert.equal(wrongSource.pos, 0);
  });
  it("keeps an Aura or Equipment bonus and recurring untap restriction continuous", () => {
    // Leaden Fists; Vulshok Gauntlets.
    for (const [subject, what, power, toughness] of [
      ["Enchanted creature", enchanted, 3, 3],
      ["Equipped creature", { ref: "equipped", noun: "creature" }, 4, 2],
    ] as const) assert.deepEqual(one(`${subject} gets +${power}/+${toughness} and doesn't untap during its controller's untap step.`), {
      kind: "static", effect: { static: "group", effects: [modify(what, [pt(power, toughness)]), suppress(what)] },
    });
  });

  it("keeps ability removal and untap suppression distinct regardless of clause order", () => {
    // Tamiyo's Compleation, Stop Cold, and Flood the Engine share this clause.
    const permanent = { ref: "enchanted", noun: "permanent" } as const;
    assert.deepEqual(one("Enchanted permanent loses all abilities and doesn't untap during its controller's untap step."), {
      kind: "static", effect: { static: "group", effects: [modify(permanent, [loseAll]), suppress(permanent)] },
    });
    assert.deepEqual(one("This creature doesn't untap during your untap step and loses all abilities."), {
      kind: "static", effect: { static: "group", effects: [
        { static: "untap-step-suppression", what: source, step: { occurrence: "each", owner: you } },
        modify(source, [loseAll]),
      ] },
    });
  });

  it("retains Varchild's same Survivor population and both forbidden defenders", () => {
    const survivors = { ref: "each", filter: { subtypes: ["Survivor"], control: "opponent" } } as const;
    assert.deepEqual(one("Survivors your opponents control can't block, and they can't attack you or planeswalkers you control."), {
      kind: "static", effect: { static: "group", effects: [
        { static: "cant", what: survivors, action: "block" },
        { static: "combat-rule", what: survivors, rule: { rule: "attack-restriction", defenders: [
          { defender: "player", who: you },
          { defender: "object", what: { ref: "each", filter: { types: ["planeswalker"], control: "you" } } },
        ] } },
      ] },
    });
  });

  it("keeps a shared Lieutenant condition around the complete subject-changing group", () => {
    // Angelic Field Marshal; Thunderfoot Baloth.
    const condition = { condition: "control", who: you,
      what: { ref: "commander", owner: you, count: fixed(1) } };
    assert.deepEqual(one("Lieutenant — As long as you control your commander, this creature gets +2/+2 and creatures you control have vigilance."), {
      kind: "static", abilityWord: "Lieutenant", effect: { static: "conditional", condition,
        effect: { static: "group", effects: [modify(source, [pt(2, 2)]),
          modify({ ref: "each", filter: { types: ["creature"], control: "you" } }, [keyword("vigilance")])] } },
    });
    assert.deepEqual(one("Lieutenant — As long as you control your commander, this creature gets +2/+2 and other creatures you control get +2/+2 and have trample."), {
      kind: "static", abilityWord: "Lieutenant", effect: { static: "conditional", condition,
        effect: { static: "group", effects: [modify(source, [pt(2, 2)]),
          modify({ ref: "each", filter: { other: true, types: ["creature"], control: "you" } }, [pt(2, 2), keyword("trample")])] } },
    });
  });

  it("keeps Winter's Rest's leading condition on the recurring untap rule", () => {
    assert.deepEqual(one("As long as you control another snow permanent, enchanted creature doesn't untap during its controller's untap step."), {
      kind: "static", effect: { static: "conditional", condition: { condition: "control", who: you,
        what: { ref: "filter", filter: { supertypes: ["snow"], cls: "permanent", other: true }, count: fixed(1) } },
      effect: suppress(enchanted) },
    });
  });

  it("retains Katilda's keyword list, dynamic X expression, and repeated enchanted subject", () => {
    assert.deepEqual(one("Enchanted creature has flying, lifelink, and protection from Vampires, and it gets +X/+X, where X is the number of permanents you control that are Spirits and/or enchantments."), {
      kind: "static", effect: { static: "group", effects: [
        modify(enchanted, [{ modifier: "abilities", abilities: [
          { gained: "keyword", keyword: { keyword: "flying" } },
          { gained: "keyword", keyword: { keyword: "lifelink" } },
          { gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "filter", filter: { subtypes: ["Vampire"] } }] } },
        ] }]),
        { static: "modify", what: enchanted, modifiers: [{ modifier: "pt",
          power: { sign: 1, amount: { amount: "x" } }, toughness: { sign: 1, amount: { amount: "x" } } }],
        bindings: [{ variable: "x", value: { amount: "count", of: { cls: "permanent", control: "you",
          allOf: [{ anyOf: [{ subtypes: ["Spirit"] }, { types: ["enchantment"] }] }] } } }] },
      ] },
    });
  });

  it("preserves the separate granted activated ability on Heliod's Punishment", () => {
    assert.deepEqual(one('Enchanted creature can\'t attack or block. It loses all abilities and has "{T}: Remove a task counter from Heliod\'s Punishment. Then if it has no task counters on it, destroy Heliod\'s Punishment."', "Heliod's Punishment"), {
      kind: "static", effect: { static: "group", effects: [
        { static: "cant", what: enchanted, action: "attack-or-block" },
        modify(enchanted, [loseAll, { modifier: "abilities", abilities: [{ gained: "quoted", ability: {
          kind: "activated", costs: [{ cost: "tap-self" }], effects: [
            { steps: [{ effect: "remove-counters", counter: { type: "task" }, count: fixed(1), from: { ref: "self" } }] },
            { steps: [{ effect: "destroy", what: { ref: "self" } }], condition: {
              condition: "is-filter", what: { ref: "it" }, filter: { counterConstraints: [{
                counter: { type: "task" }, comparison: { op: "eq", value: fixed(0) },
              }] },
            }, conditionKind: "if" },
          ],
        } }] }]),
      ] },
    });
  });

  it("rejects ambiguous pronouns, free-variable scope, mixed durations, and trailing condition scope", () => {
    for (const text of [
      "Enchanted creature gets +1/+1 and they have flying.",
      "Creatures you control get +1/+1 and it has flying.",
      "This creature gets +X/+X and other creatures get +X/+X, where X is the number of artifacts you control.",
      "This creature gets +1/+1 and other creatures have flying if you control an artifact.",
      "This creature gets +1/+1 until end of turn and creatures you control have flying.",
      "This creature gets +1/+1 and target creature has flying.",
    ]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseStatic(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
  });

  it("does not reinterpret a combat declaration payment as a resolving unless choice", () => {
    // Floodtide Serpent; Qal Sisma Behemoth. These former rejections
    // are now the declaration-cost combat rule (CR 508.1h, 509.1d), still
    // never a resolving "unless" sentence.
    assert.deepEqual(one("This creature can't attack unless you return an enchantment you control to its owner's hand."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: {
        rule: "declaration-cost", action: "attack", who: { player: "you" }, scope: "each-affected-creature",
        costs: [{ cost: "return", what: { ref: "filter", filter: { types: ["enchantment"], control: "you" }, count: fixed(1) }, to: "hand", owner: "owner" }],
      } },
    });
    assert.deepEqual(one("This creature can't attack or block unless you pay {2}."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: {
        rule: "declaration-cost", action: "attack-or-block", who: { player: "you" }, scope: "each-affected-creature",
        costs: [{ cost: "mana", symbols: ["2"] }],
      } },
    });
    assert.deepEqual(one("Counter target spell unless its controller pays {2}."), {
      kind: "spell", effects: [{ steps: [{ effect: "counter", what: {
        ref: "target", filter: { cls: "spell" }, count: fixed(1),
      } }], condition: { condition: "pays", who: { player: "controller", of: { ref: "it" } }, cost: [{ cost: "mana", symbols: ["2"] }] },
      conditionKind: "unless" }],
    });
  });
});
