import assert from "node:assert/strict";
import { test } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Exact clauses from the frozen Oracle archive. Rules edition 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed", value });
const you = { player: "you" };

function one(text: string, typeLine: string): Ability {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0])) as Ability;
}

function rejects(text: string, typeLine: string): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

test("domain grants one landwalk per distinct basic land subtype", () => {
  const text = "Domain — For each basic land type among lands you control, this creature has landwalk of that type.";
  assert.deepEqual(one(text, "Creature — Treefolk"), { kind: "static", abilityWord: "Domain", effect: {
    static: "landwalk-for-basic-land-types", what: { ref: "this", noun: "creature" },
    among: { ref: "each", filter: { types: ["land"], control: "you" } },
  } });
  rejects(text.replace("basic land type", "basic land"), "Creature — Treefolk");
  rejects(text.replace("landwalk of that type", "landwalk of a type of your choice"), "Creature — Treefolk");
});

test("boast limit changes only the count during your own turn", () => {
  const text = "Creatures you control can boast twice during each of your turns rather than once.";
  assert.deepEqual(one(text, "Legendary Creature — God"), { kind: "static", effect: {
    static: "keyword-activation-limit", what: { ref: "each", filter: { types: ["creature"], control: "you" } },
    keyword: "boast", maximum: 2, normally: 1, during: "your-turn",
  } });
  rejects(text.replace("can boast", "can exhaust"), "Legendary Creature — God");
  rejects(text.replace("rather than once", "rather than twice"), "Legendary Creature — God");
});

test("one chosen color is added across distinct game-object domains", () => {
  const text = "All cards that aren't on the battlefield, spells, and permanents are the chosen color in addition to their other colors.";
  assert.deepEqual(one(text, "Artifact Creature"), { kind: "static", effect: {
    static: "modify", what: { ref: "all", filter: { anyOf: [
      { cls: "card", outsideZone: { zone: "battlefield" } }, { cls: "spell" }, { cls: "permanent" },
    ] } },
    modifiers: [{ modifier: "colors", operation: "add", colors: {
      choice: "chosen-value", category: "color", which: "chosen",
    } }],
  } });
  rejects(text.replace("and permanents are", "or permanents are"), "Artifact Creature");
  rejects(text.replace("the chosen color", "the color of your choice"), "Artifact Creature");
});

test("one control and mana-value restriction applies across both animation arms", () => {
  const text = 'During your turn, each non-Equipment artifact and non-Aura enchantment you control with mana value 4 or greater is a 4/4 Elemental creature in addition to its other types and has indestructible, haste, and "Whenever this creature deals combat damage to a player, draw a card."';
  const ability = one(text, "Legendary Creature");
  assert.equal(ability.kind, "static");
  assert.equal(ability.effect.static, "conditional");
  assert.deepEqual(ability.effect.condition, { condition: "your-turn" });
  const effect = ability.effect.effect;
  assert.equal(effect.static, "modify");
  assert.deepEqual(effect.what, { ref: "each", filter: {
    anyOf: [{ types: ["artifact"], nonSubtypes: ["Equipment"] }, { types: ["enchantment"], nonSubtypes: ["Aura"] }],
    control: "you", properties: [{ property: "mana-value", comparison: { op: "ge", value: fixed(4) } }],
  } });
  assert.deepEqual(effect.modifiers.slice(0, 2), [
    { modifier: "types", change: {
      cardTypes: { operation: "add", types: ["creature"] },
      subtypes: [{ family: "creature", operation: "add", types: ["Elemental"] }],
    } },
    { modifier: "base-pt", power: fixed(4), toughness: fixed(4) },
  ]);
  const grants = effect.modifiers[2];
  assert.equal(grants.modifier, "abilities");
  assert.deepEqual(grants.abilities.slice(0, 2), [
    { gained: "keyword", keyword: { keyword: "indestructible" } }, { gained: "keyword", keyword: { keyword: "haste" } },
  ]);
  const quoted = grants.abilities[2];
  assert.equal(quoted.gained, "quoted");
  assert.equal(quoted.ability.kind, "triggered");
  assert.deepEqual(quoted.ability.trigger, {
    trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true, to: "any",
  });
  assert.deepEqual(quoted.ability.effects[0].steps, [{ effect: "draw", who: you, amount: fixed(1) }]);
  rejects(text.replace("and non-Aura", "and each non-Aura"), "Legendary Creature");
  rejects(text.replace("artifact and", "artifact your opponents control and"), "Legendary Creature");
  rejects(text.replace("draw a card.", "draw a card and."), "Legendary Creature");
});

test("hexproof shares the single color fixed by the enclosing effect", () => {
  const text = "{1}, Discard a card: Until end of turn, ~ becomes the color of your choice and gains hexproof from that color.";
  const ability = one(text, "Legendary Creature");
  assert.equal(ability.kind, "activated");
  assert.deepEqual(ability.effects[0].steps, [{ effect: "modify-characteristics", what: { ref: "self" },
    modifiers: [
      { modifier: "colors", operation: "set", colors: {
        choice: "fresh-choice", category: "color", who: you, when: "effect-created", quantity: "one",
      } },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: {
        keyword: "hexproof", from: [{ scope: "color-chosen-by-characteristic-effect" }],
      } }] },
    ],
    snapshot: "effect-created", duration: { duration: "end-of-turn" },
  }]);
  rejects(text.replace("the color of your choice", "the color or colors of your choice"), "Legendary Creature");
  rejects(text.replace("the color of your choice", "red"), "Legendary Creature");
});

test("permanent type choice is a card-type choice restricted by CR 110.4", () => {
  const text = "Choose a permanent type. Return all cards of the chosen type from your graveyard to your hand.";
  assert.deepEqual(one(text, "Sorcery"), { kind: "spell", effects: [
    { steps: [{ effect: "choose-value", who: you, category: "card-type", within: "permanent-types" }] },
    { steps: [{ effect: "return-to-hand", to: "your", what: { ref: "all", filter: {
      cls: "card", chosenType: { choice: "chosen-type" }, zone: { zone: "graveyard", owner: "your" },
    } } }] },
  ] });
  rejects(text.replace("permanent type", "permanent subtype"), "Sorcery");
});

test("prevention tests both different creatures in each source–recipient pair", () => {
  const text = "Prevent all damage that would be dealt to a creature by another creature if they share a color.";
  const source = { ref: "prevention-event-object", role: "source" };
  const recipient = { ref: "prevention-event-object", role: "recipient" };
  assert.deepEqual(one(text, "Enchantment"), { kind: "static", effect: { static: "prevent-damage", prevention: {
    kind: "all", eventScope: { sources: "each", recipients: "each" },
    shield: { shield: "events", occurrence: "each", prevent: { prevent: "all" } },
    by: { ref: "filter", filter: { types: ["creature"], otherThan: recipient }, count: fixed(1) },
    to: [{ ref: "filter", filter: { types: ["creature"] }, count: fixed(1) }],
    condition: { condition: "characteristic-relation", what: source, test: {
      relation: "shared-value", characteristic: "color", with: { operand: "object", what: recipient },
    } },
  } } });
  rejects(text.replace("they share a color", "it shares a color"), "Enchantment");
  rejects(text.replace("another creature", "another permanent"), "Enchantment");
});

test("attachment chooses among the controller's other opponents at random", () => {
  const text = "Whenever enchanted player casts a noncreature spell, roll a d6. This Aura deals damage to that player equal to the result. Then attach this Aura to another one of your opponents chosen at random.";
  const ability = one(text, "Enchantment — Aura");
  assert.equal(ability.kind, "triggered");
  assert.deepEqual(ability.trigger, {
    trigger: "cast", who: { player: "enchanted-player" }, what: { nonTypes: ["creature"], cls: "spell" },
  });
  assert.deepEqual(ability.effects[2].steps, [{
    effect: "attach", what: { ref: "this", noun: "aura" }, who: you,
    to: { player: "filtered", base: { player: "an-opponent" },
      filter: { otherThan: { player: "enchanted-player" } }, random: true },
  }]);
  rejects(text.replace("chosen at random", "chosen"), "Enchantment — Aura");
  rejects(text.replace("this Aura to another", "target Aura to another"), "Enchantment — Aura");
});

test("one opponent controller scopes across the destroy target, with basic subtypes used in the search", () => {
  const text = "Destroy target artifact, enchantment, or nonbasic land an opponent controls. That player may search their library for a land card with a basic land type, put it onto the battlefield, then shuffle.";
  const ability = one(text, "Sorcery");
  assert.equal(ability.kind, "spell");
  assert.deepEqual(ability.effects[0].steps, [{
    effect: "destroy", what: { ref: "target", count: fixed(1), filter: { control: "opponent", anyOf: [
      { types: ["artifact"] }, { types: ["enchantment"] }, { types: ["land"], nonSupertypes: ["basic"] },
    ] } },
  }]);
  const player = { player: "that-player" };
  assert.deepEqual(ability.effects[1].steps, [{ effect: "optional-group", who: player, steps: [
    { effect: "search", who: player, zones: { selection: "all", of: [{ zone: "library", owner: "their" }] },
      for: { ref: "filter", count: fixed(1), filter: {
        types: ["land"], cls: "card", anyOf: ["Plains", "Island", "Swamp", "Mountain", "Forest"].map(type => ({ subtypes: [type] })),
      } } },
    { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, who: player },
    { effect: "shuffle", who: player, library: { zone: "library", owner: player } },
  ] }]);
  rejects(text.substring(0, text.indexOf(".") + 1), "Sorcery");
  rejects(text.replace("target artifact", "two target artifact"), "Sorcery");
  rejects(text.replace("enchantment, or", "enchantment you control, or"), "Sorcery");
});

test("followup to an optional copy of a distinct creature preserves both recipients", () => {
  const copy = "At the beginning of combat on your turn, you may have ~ become a copy of another target creature you control until end of turn, except its name is ~.";
  const followup = "If you do, ~ and that creature each get +2/+0 and gain haste until end of turn and attack this turn if able.";
  const text = `${copy} ${followup}`;
  const ability = one(text, "Legendary Creature");
  assert.equal(ability.kind, "triggered");
  assert.equal(ability.effects.length, 2);
  const pair = [{ ref: "self" }, { ref: "that", filter: { types: ["creature"] } }];
  assert.deepEqual(ability.effects[1], { condition: { condition: "you-do" }, conditionKind: "if", steps: [
    { effect: "simultaneous", steps: pair.map(what => ({
      effect: "pump", what, power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(0) },
      duration: { duration: "end-of-turn" },
    })) },
    { effect: "simultaneous", steps: pair.map(what => ({
      effect: "gain-abilities", what, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }],
      duration: { duration: "end-of-turn" },
    })) },
    { effect: "simultaneous", steps: pair.map(what => ({
      effect: "combat-rule", what, rule: { rule: "attack-requirement", frequency: "each-combat" },
      duration: { duration: "this-turn" },
    })) },
  ] });
  rejects(followup, "Legendary Creature");
  rejects(text.replace("another target", "target"), "Legendary Creature");
  rejects(text.replace("if able.", "if able unless you pay {1}."), "Legendary Creature");
});
