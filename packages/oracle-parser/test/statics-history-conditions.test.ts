import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings are taken from the cached
// Scryfall corpus; card names are noted per test.
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
function effect(text: string, typeLine: string, name?: string): StaticEffect {
  const ability = only(text, typeLine, name);
  assert.equal(ability.kind, "static");
  if (ability.kind !== "static") throw new Error(text);
  return ability.effect;
}
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}

const thisCreature = { ref: "this", noun: "creature" } as const;
const fixed = (value: number) => ({ amount: "fixed", value }) as const;

describe("turn-scoped base power and toughness (CR 604.3a, 613.4b)", () => {
  it("sets base P/T only during your turn (Snowmelt Stag)", () => {
    assert.deepEqual(effect("During your turn, this creature has base power and toughness 5/2.", "Creature — Elemental Elk"), {
      static: "conditional", condition: { condition: "your-turn" }, effect: { static: "group", effects: [
        { static: "modify", what: thisCreature, modifiers: [{ modifier: "base-pt", power: fixed(5) }] },
        { static: "modify", what: thisCreature, modifiers: [{ modifier: "base-pt", toughness: fixed(2) }] },
      ] } });
  });
  it("sets the creature type too (Circle of the Moon Druid)", () => {
    assert.deepEqual(effect("Bear Form — During your turn, this creature is a Bear with base power and toughness 4/2.", "Creature — Human Elf Druid"), {
      static: "conditional", condition: { condition: "your-turn" }, effect: { static: "group", effects: [
        { static: "modify", what: thisCreature, modifiers: [{ modifier: "types", change: { subtypes: [{ family: "creature", operation: "set", types: ["Bear"] }] } }] },
        { static: "modify", what: thisCreature, modifiers: [{ modifier: "base-pt", power: fixed(4) }] },
        { static: "modify", what: thisCreature, modifiers: [{ modifier: "base-pt", toughness: fixed(2) }] },
      ] } });
  });
  it("still rejects a turn-scoped entry replacement", () => {
    rejects("During your turn, this creature enters tapped.", "Creature — Elemental Elk");
  });
});

describe("objects with no abilities (CR 113.12)", () => {
  it("filters creatures with no abilities (Muraganda Petroglyphs, Ruxa)", () => {
    const pump = (value: number) => ({ sign: 1, amount: fixed(value) });
    assert.deepEqual(effect("Creatures with no abilities get +2/+2.", "Enchantment"), {
      static: "modify", what: { ref: "each", filter: { types: ["creature"], hasAbilities: false } },
      modifiers: [{ modifier: "pt", power: pump(2), toughness: pump(2) }] });
    assert.deepEqual(effect("Creatures you control with no abilities get +1/+1.", "Legendary Creature — Bear Druid"), {
      static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "you", hasAbilities: false } },
      modifiers: [{ modifier: "pt", power: pump(1), toughness: pump(1) }] });
  });
  it("does not read 'with no' as a missing keyword list", () => {
    rejects("Creatures with no flying get +2/+2.", "Enchantment");
  });
});

describe("protection from a mana value (CR 702.16a)", () => {
  it("keeps the mana-value comparison as the quality (Mistmeadow Skulk, Reaver Titan)", () => {
    const ability = only("Lifelink, protection from mana value 3 or greater", "Creature — Kithkin Rogue");
    assert.deepEqual(ability, { kind: "keywords", keywords: [{ keyword: "lifelink" }, { keyword: "protection", from: [{ scope: "filter",
      filter: { properties: [{ property: "mana-value", comparison: { op: "ge", value: fixed(3) } }] } }] }] });
    assert.deepEqual(only("Protection from mana value 3 or less", "Artifact — Vehicle"), { kind: "keywords", keywords: [{ keyword: "protection",
      from: [{ scope: "filter", filter: { properties: [{ property: "mana-value", comparison: { op: "le", value: fixed(3) } }] } }] }] });
  });
  it("rejects an unbound X and parity qualities", () => {
    rejects("Protection from mana value X or less", "Creature — Kithkin Rogue");
    rejects("Protection from even mana values", "Creature — Kithkin Rogue");
  });
});

function condition(text: string, typeLine: string, name?: string) {
  const ability = only(text, typeLine, name);
  assert.equal(ability.kind, "triggered");
  if (ability.kind !== "triggered") throw new Error(text);
  return ability.condition;
}
const atLeastOne = { op: "ge", value: fixed(1) } as const;
const thatCreature = { ref: "that", filter: { types: ["creature"] } } as const;

describe("one spell of each of two disjoint kinds (CR 601.2i)", () => {
  it("requires a creature spell and a noncreature spell (Fae Offering, Eshki Dragonclaw)", () => {
    assert.deepEqual(condition("At the beginning of each end step, if you've cast both a creature spell and a noncreature spell this turn, draw a card.", "Enchantment"), {
      condition: "all-of", conditions: [
        { condition: "cast-count", who: { player: "you" }, what: { types: ["creature"], cls: "spell" }, comparison: atLeastOne, during: "this-turn" },
        { condition: "cast-count", who: { player: "you" }, what: { nonTypes: ["creature"], cls: "spell" }, comparison: atLeastOne, during: "this-turn" },
      ] });
  });
  it("rejects kinds one spell could satisfy together", () => {
    rejects("At the beginning of each end step, if you've cast both a creature spell and a red spell this turn, draw a card.", "Enchantment");
  });
});

describe("excess damage history (CR 120.10)", () => {
  it("asks whether that creature was dealt excess damage this turn (Maarika, Brutal Gladiator)", () => {
    assert.deepEqual(condition("Whenever ~ deals damage to a creature, if that creature was dealt excess damage this turn, you draw a card.",
      "Legendary Creature — Human Warrior", "Maarika, Brutal Gladiator"), {
      condition: "damage-history", comparison: { op: "gt", value: fixed(0) }, quantity: "positive-event", excess: true,
      to: thatCreature, during: "this-turn" });
  });
  it("rejects excess damage to a player", () => {
    rejects("At the beginning of your end step, if you were dealt excess damage this turn, draw a card.", "Enchantment");
  });
});

describe("having become blocked (CR 509.1h)", () => {
  it("asks whether it was blocked this turn (Fyndhorn Druid)", () => {
    assert.deepEqual(condition("When this creature dies, if it was blocked this turn, you gain 4 life.", "Creature — Elf Druid"), {
      condition: "combat-action-history", what: { ref: "it" }, action: "was-blocked", during: "this-turn" });
    assert.deepEqual(condition("When this creature dies, if it wasn't blocked this turn, you gain 4 life.", "Creature — Elf Druid"), {
      condition: "not", of: { condition: "combat-action-history", what: { ref: "it" }, action: "was-blocked", during: "this-turn" } });
  });
});

describe("activation history (CR 602.2)", () => {
  it("asks whether you activated a loyalty ability this turn (Kiora of Salt and Sand)", () => {
    assert.deepEqual(condition("Whenever you attack, if you've activated a loyalty ability this turn, draw a card.", "Legendary Planeswalker — Kiora"), {
      condition: "player-action-history", who: { player: "you" }, action: { action: "activate", what: { cls: "ability", loyaltyAbility: true } },
      comparison: atLeastOne, during: "this-turn" });
  });
  it("does not count activating a permanent", () => {
    rejects("Whenever you attack, if you've activated a creature this turn, draw a card.", "Legendary Planeswalker — Kiora");
  });
});

describe("cast ordinals (CR 601.2i, 400.7d)", () => {
  it("asks whether the cast spell is the second creature spell (Vengevine)", () => {
    assert.deepEqual(condition("Whenever you cast a spell, if it's the second creature spell you cast this turn, draw a card.", "Creature — Elemental"), {
      condition: "cast-ordinal", what: { ref: "it" }, who: { player: "you" }, of: { types: ["creature"], cls: "spell" }, ordinal: 2, during: "this-turn" });
  });
  it("asks about the spell an entering permanent was (Codespell Cleric)", () => {
    assert.deepEqual(condition("When this creature enters, if it was the second spell you cast this turn, draw a card.", "Creature — Human Cleric"), {
      condition: "cast-ordinal", what: { ref: "it" }, who: { player: "you" }, of: { cls: "spell" }, ordinal: 2, during: "this-turn" });
  });
  it("rejects a spell cast by another player or in another period", () => {
    rejects("Whenever you cast a spell, if it's the second creature spell an opponent cast this turn, draw a card.", "Creature — Elemental");
    rejects("Whenever you cast a spell, if it's the second creature spell you cast this game, draw a card.", "Creature — Elemental");
  });
});

describe("first matching event of the turn (CR 603.4)", () => {
  it("first time counters were put on that creature (Stalwart Successor)", () => {
    assert.deepEqual(condition("Whenever one or more counters are put on a creature you control, if it's the first time counters have been put on that creature this turn, put a +1/+1 counter on that creature.",
      "Creature — Human Warrior"), { condition: "first-event", during: "this-turn", event: { trigger: "counter-change", direction: "put", counter: "any",
      on: { recipient: "object", what: thatCreature }, match: { counters: "one-or-more", recipients: "each" } } });
  });
  it("first time that creature became tapped (Captain America, Living Legend)", () => {
    assert.deepEqual(condition("Whenever a creature you control becomes tapped during your turn, if it's the first time that creature has become tapped this turn, untap it.",
      "Legendary Creature — Human Soldier Hero"), { condition: "first-event", during: "this-turn", event: { trigger: "becomes-tapped", what: thatCreature } });
  });
  it("rejects a plural or undefined object", () => {
    rejects("Whenever a creature you control becomes tapped, if it's the first time those creatures have become tapped this turn, untap it.", "Creature — Human");
  });
});

describe("a static reveal linked to triggered abilities (CR 603.11, 607.2a)", () => {
  const draw = { effect: "draw", who: { player: "you" }, amount: fixed(1) };
  it("links each trigger to the reveal of the first draw (Primitive Etchings)", () => {
    assert.deepEqual(only("Reveal the first card you draw each turn. Whenever you reveal a creature card this way, draw a card.", "Enchantment"), {
      kind: "linked-triggers", static: { kind: "static", effect: { static: "reveal-first-draw", who: { player: "you" } } },
      triggers: [{ kind: "triggered", trigger: { trigger: "reveals-linked", who: { player: "you" }, what: { types: ["creature"], cls: "card" } },
        effects: [{ steps: [draw] }] }] });
  });
  it("keeps several triggers, your turns only (Keranos, God of Storms)", () => {
    assert.deepEqual(only("Reveal the first card you draw on each of your turns. Whenever you reveal a land card this way, draw a card. Whenever you reveal a nonland card this way, ~ deals 3 damage to any target.",
      "Legendary Enchantment Creature — God", "Keranos, God of Storms"), {
      kind: "linked-triggers", static: { kind: "static", effect: { static: "reveal-first-draw", who: { player: "you" }, turns: "your" } },
      triggers: [
        { kind: "triggered", trigger: { trigger: "reveals-linked", who: { player: "you" }, what: { types: ["land"], cls: "card" } }, effects: [{ steps: [draw] }] },
        { kind: "triggered", trigger: { trigger: "reveals-linked", who: { player: "you" }, what: { nonTypes: ["land"], cls: "card" } },
          effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: fixed(3), to: { to: "any-target" } }] }] },
      ] });
  });
  it("keeps the optional reveal (Inquisitor Eisenhorn)", () => {
    const ability = only("You may reveal the first card you draw each turn as you draw it. Whenever you reveal a land card this way, draw a card.", "Legendary Creature — Human");
    assert.equal(ability.kind, "linked-triggers");
    if (ability.kind !== "linked-triggers") return;
    assert.deepEqual(ability.static.effect, { static: "reveal-first-draw", who: { player: "you" }, optional: true });
  });
  it("rejects the static alone, other draws, and unlinked reveals", () => {
    rejects("Reveal the first card you draw each turn.", "Enchantment");
    rejects("Reveal the second card you draw each turn. Whenever you reveal a creature card this way, draw a card.", "Enchantment");
    rejects("Whenever you reveal a creature card this way, draw a card.", "Enchantment");
    rejects("Reveal the first card you draw each turn. Whenever you reveal a creature card, draw a card.", "Enchantment");
  });
});

describe("your team (CR 102.4)", () => {
  const team = { player: "your-team" } as const;
  it("is the controller of a filtered group (Rushblade Commander)", () => {
    assert.deepEqual(effect("Warriors your team controls have haste.", "Creature — Human Warrior"), {
      static: "modify", what: { ref: "each", filter: { subtypes: ["Warrior"], controlledBy: team } },
      modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] });
  });
  it("controls or gains life in a condition (Sickle Dancer, Regna, the Redeemer)", () => {
    assert.deepEqual(condition("Whenever this creature attacks, if your team controls another Warrior, draw a card.", "Creature — Human Warrior"), {
      condition: "control", who: team, what: { ref: "filter", filter: { subtypes: ["Warrior"], other: true }, count: fixed(1) } });
    assert.deepEqual(condition("At the beginning of each end step, if your team gained life this turn, draw a card.", "Legendary Creature — Angel"), {
      condition: "life-change", who: team, change: "gained", comparison: { op: "gt", value: fixed(0) }, during: "this-turn" });
  });
  it("does not read 'your teammates' as the team", () => {
    rejects("When this enchantment enters, each of your teammates draws a card.", "Enchantment");
  });
});

describe("activating abilities at instant speed (CR 304.5, 606.3)", () => {
  it("equip abilities (Leonin Shikari, Forge Anew)", () => {
    const timing = { static: "activation-timing", who: { player: "you" }, abilities: { cls: "ability", abilityKeyword: "equip" }, timing: "instant" };
    assert.deepEqual(effect("You may activate equip abilities any time you could cast an instant.", "Creature — Cat Soldier"), timing);
    assert.deepEqual(effect("During your turn, you may activate equip abilities any time you could cast an instant.", "Enchantment"), {
      static: "conditional", condition: { condition: "your-turn" }, effect: timing });
  });
  it("loyalty abilities on any player's turn (Teferi, Master of Time)", () => {
    assert.deepEqual(effect("You may activate loyalty abilities of ~ on any player's turn any time you could cast an instant.",
      "Legendary Planeswalker — Teferi", "Teferi, Master of Time"), { static: "activation-timing", who: { player: "you" },
      abilities: { cls: "ability", loyaltyAbility: true, abilitySource: { ref: "self" } }, timing: "instant", anyPlayersTurn: true });
  });
  it("rejects other abilities and other timings", () => {
    rejects("You may activate abilities of creatures you control any time you could cast an instant.", "Enchantment");
    rejects("You may activate equip abilities any time you could cast a sorcery.", "Enchantment");
  });
});

describe("owning a card in exile (CR 108.3, 406.1)", () => {
  it("counts exiled cards you own (Howling Galefang)", () => {
    assert.deepEqual(effect("This creature has haste as long as you own a card in exile that has an Adventure.", "Creature — Wolf"), {
      static: "modify", what: thisCreature, modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }],
      condition: { condition: "count", of: { cls: "card", zone: { zone: "exile" }, adventure: true, own: "you" }, comparison: atLeastOne } });
  });
  it("rejects cards owned in another zone this way", () => {
    rejects("This creature has haste as long as you own a card on the battlefield.", "Creature — Wolf");
  });
});

describe("number-invariant creature types in a plural union", () => {
  it("joins Treefolk with a visibly plural subtype (Kirri, Talented Sprout)", () => {
    assert.deepEqual(effect("Other Plants and Treefolk you control get +2/+0.", "Legendary Creature — Plant Druid"), {
      static: "modify", what: { ref: "each", filter: { other: true, allOf: [{ anyOf: [{ subtypes: ["Plant"] }, { subtypes: ["Treefolk"] }] }], control: "you" } },
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(0) } }] });
  });
  it("still rejects a singular noun in the union", () => {
    rejects("Other Elf and Treefolk you control get +1/+1.", "Creature — Elf");
  });
});
