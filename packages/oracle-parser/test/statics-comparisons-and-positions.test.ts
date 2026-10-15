import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
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
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}

const thisCreature = { ref: "this", noun: "creature" } as const;
const thatCreature = { ref: "that", filter: { types: ["creature"] } } as const;
const fixed = (value: number) => ({ amount: "fixed", value }) as const;
const anotherEnters = { trigger: "enters", what: { ref: "filter", filter: { other: true, types: ["creature"], control: "you" }, count: fixed(1) } };
const attr = (of: object, attribute: string) => ({ amount: "attribute", of, attribute });
const greater = (attribute: string, of: object, than: object) =>
  ({ condition: "compare", value: attr(of, attribute), comparison: { op: "gt", value: attr(than, attribute) } });

describe("comparing one characteristic of two objects (CR 603.4, 702.100a)", () => {
  it("reads greater power or toughness as either comparison (Evolving Adaptive)", () => {
    assert.deepEqual(only("Whenever another creature you control enters, if that creature has greater power or toughness than this creature, put an oil counter on this creature.", "Creature — Phyrexian Warrior"), {
      kind: "triggered", trigger: anotherEnters,
      condition: { condition: "any-of", conditions: [greater("power", thatCreature, thisCreature), greater("toughness", thatCreature, thisCreature)] },
      effects: [{ steps: [{ effect: "put-counters", on: thisCreature, counter: { type: "oil" }, count: fixed(1) }] }],
    });
  });
  it("compares with the source by name (Hulkling, Burgeoning Bruiser)", () => {
    const ability = only("Whenever another creature you control enters, if it has greater power or toughness than ~, put a +1/+1 counter on ~.", "Legendary Creature — Kree Skrull Hero");
    assert.deepEqual(ability.kind === "triggered" && ability.condition, { condition: "any-of", conditions: [
      greater("power", { ref: "it" }, { ref: "self" }), greater("toughness", { ref: "it" }, { ref: "self" })] });
  });
  it("leaves the compared characteristic unrepeated (Pelt Collector)", () => {
    const ability = only("Whenever another creature you control enters or dies, if that creature's power is greater than this creature's, put a +1/+1 counter on this creature.", "Creature — Elf Warrior");
    assert.deepEqual(ability.kind === "triggered" && ability.condition, greater("power", thatCreature, thisCreature));
    // The unelided wording is the same condition.
    const full = only("Whenever another creature you control enters or dies, if that creature's power is greater than this creature's power, put a +1/+1 counter on this creature.", "Creature — Elf Warrior");
    assert.deepEqual(full.kind === "triggered" && full.condition, greater("power", thatCreature, thisCreature));
  });
  it("keeps a single comparison and the less-than direction distinct", () => {
    const ability = only("Whenever another creature you control enters, if it has less power than ~, draw a card.", "Creature — Elf");
    assert.deepEqual(ability.kind === "triggered" && ability.condition, { condition: "compare", value: attr({ ref: "it" }, "power"),
      comparison: { op: "lt", value: attr({ ref: "self" }, "power") } });
  });
  it("rejects quantified or unidentified comparison objects", () => {
    rejects("Whenever another creature you control enters, if it has greater power than each other creature, draw a card.", "Creature — Elf");
    rejects("Whenever another creature you control enters, if it has greater power than a creature you control, draw a card.", "Creature — Elf");
    rejects("Whenever another creature you control enters, if it has greater power or power than ~, draw a card.", "Creature — Elf");
  });
});

describe("a card's position in its graveyard (CR 404.2)", () => {
  const thisCard = { ref: "this", noun: "card" } as const;
  const yours = { zone: "graveyard", owner: "your" } as const;
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "your" } as const;
  it("tests the card directly above it (Krovikan Horror)", () => {
    const ability = only("At the beginning of the end step, if this card is in your graveyard with a creature card directly above it, you may return this card to your hand.", "Creature — Horror Spirit");
    assert.deepEqual(ability, { kind: "triggered", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      condition: { condition: "graveyard-position", what: thisCard, graveyard: yours, above: { match: "directly", filter: { types: ["creature"] } } },
      effects: [{ steps: [{ effect: "return-to-hand", what: thisCard, to: "your", optional: true }] }] });
  });
  it("counts every matching card above it (Nether Shadow)", () => {
    const ability = only("At the beginning of your upkeep, if this card is in your graveyard with three or more creature cards above it, you may put this card onto the battlefield.", "Creature — Spirit");
    assert.deepEqual(ability, { kind: "triggered", trigger: upkeep,
      condition: { condition: "graveyard-position", what: thisCard, graveyard: yours,
        above: { match: "count", filter: { types: ["creature"] }, comparison: { op: "ge", value: fixed(3) } } },
      effects: [{ steps: [{ effect: "move-zone", what: thisCard, to: { zone: "battlefield" }, optional: true }] }] });
  });
  it("rejects mismatched number, a count directly above, and an unordered graveyard", () => {
    rejects("At the beginning of your upkeep, if this card is in your graveyard with a creature cards directly above it, draw a card.", "Creature — Spirit");
    rejects("At the beginning of your upkeep, if this card is in your graveyard with three or more creature cards directly above it, draw a card.", "Creature — Spirit");
    rejects("At the beginning of your upkeep, if this card is in a graveyard with a creature card directly above it, draw a card.", "Creature — Spirit");
    rejects("At the beginning of your upkeep, if this card is in your graveyard with a creature card directly below it, draw a card.", "Creature — Spirit");
  });
});

describe("mana spent compared in the past tense (CR 601.2h)", () => {
  it("compares the mana paid with the spell's mana value (Ancient Cellarspawn)", () => {
    const it_ = { ref: "it" } as const;
    const spent = { amount: "mana-spent", what: it_, quantity: "total" };
    const value = attr(it_, "mana-value");
    assert.deepEqual(only("Whenever you cast a spell, if the amount of mana spent to cast it was less than its mana value, target opponent loses life equal to the difference.", "Enchantment Creature — Horror"), {
      kind: "triggered", trigger: { trigger: "cast", who: { player: "you" }, what: { cls: "spell" } },
      condition: { condition: "compare", value: spent, comparison: { op: "lt", value } },
      effects: [{ steps: [{ effect: "lose-life", who: { player: "target-opponent" }, amount: { amount: "minus", a: value, b: spent } }] }],
    });
  });
  it("keeps a past object characteristic as last-known information, not a current comparison", () => {
    const ability = only("Whenever a creature you control dies, if its power was 3 or greater, draw a card.", "Creature — Elf");
    assert.deepEqual(ability.kind === "triggered" && ability.condition, { condition: "is-filter", what: { ref: "it" },
      filter: { properties: [{ property: "power", comparison: { op: "ge", value: fixed(3) } }] }, time: "previous" });
  });
});

describe("an ability of a permanent countered this way (CR 113.7, 608.2c)", () => {
  it("counts countered abilities whose source is a permanent (Teferi's Response)", () => {
    const parsed = parse("Counter target spell or ability an opponent controls that targets a land you control. If a permanent's ability is countered this way, destroy that permanent.", "Instant");
    assert.equal(parsed.ok, true);
    if (!parsed.ok) return;
    const ability = JSON.parse(JSON.stringify(parsed.abilities[0]));
    assert.deepEqual(ability.effects[1], {
      steps: [{ effect: "destroy", what: { ref: "that", filter: { cls: "permanent" } } }],
      condition: { condition: "count", of: { cls: "ability", abilitySource: { ref: "filter", filter: { cls: "permanent" }, count: fixed(1) },
        relation: { relation: "event-result", event: "counter", scope: "this-way" } }, comparison: { op: "ge", value: fixed(1) } },
      conditionKind: "if",
    });
  });
  it("rejects a plural or relation-bearing possessor", () => {
    rejects("Counter target activated ability. If a permanent's abilities are countered this way, destroy that permanent.", "Instant");
    rejects("Counter target activated ability. If a permanent you controlled this turn's ability is countered this way, draw a card.", "Instant");
  });
});

describe("activation cost increase excepting mana abilities (CR 605.1a)", () => {
  const nonMana = { ref: "each", filter: { cls: "ability", manaAbility: false } };
  it("reads the unless clause as the excluded group (Suppression Field)", () => {
    const ability = only("Activated abilities cost {2} more to activate unless they're mana abilities.", "Enchantment");
    const same = only("Activated abilities that aren't mana abilities cost {2} more to activate.", "Enchantment");
    assert.deepEqual(ability, { kind: "static", effect: { static: "cost-modification", scope: { action: "activate", abilities: nonMana },
      change: { direction: "increase", symbols: ["2"] } } });
    assert.deepEqual(ability, same);
  });
  it("keeps the source restriction (Anointed Peacekeeper)", () => {
    assert.deepEqual(only("Activated abilities of sources with the chosen name cost {2} more to activate unless they're mana abilities.", "Creature — Human Cleric"), {
      kind: "static", effect: { static: "cost-modification", scope: { action: "activate",
        source: { ref: "each", filter: { cls: "source", chosenName: { choice: "chosen-value", category: "card-name", which: "chosen" } } }, abilities: nonMana },
      change: { direction: "increase", symbols: ["2"] } } });
  });
  it("rejects the exception for spells and a repeated exception", () => {
    rejects("Spells cost {2} more to cast unless they're mana abilities.", "Enchantment");
    rejects("Activated abilities that aren't mana abilities cost {2} more to activate unless they're mana abilities.", "Enchantment");
  });
});

describe("a requirement on opponents' target choices (CR 601.2c, 602.2b)", () => {
  const flag = "While an opponent is choosing targets as part of casting a spell they control or activating an ability they control, that player must choose at least one Flagbearer on the battlefield if able.";
  it("records chooser, choice moments, required objects and if-able (Standard Bearer)", () => {
    assert.deepEqual(only(flag, "Creature — Human Flagbearer"), { kind: "static", effect: { static: "target-choice-requirement",
      who: { player: "each-opponent" }, when: ["casting-spell", "activating-ability"],
      mustChoose: { subtypes: ["Flagbearer"], zone: { zone: "battlefield" } }, minimum: fixed(1), ifAble: true } });
  });
  it("rejects a dropped if-able, a dropped controller restatement, or only one choice moment", () => {
    rejects(flag.replace(" if able", ""), "Creature — Human Flagbearer");
    rejects(flag.replace("a spell they control", "a spell"), "Creature — Human Flagbearer");
    rejects("While an opponent is choosing targets as part of casting a spell they control, that player must choose at least one Flagbearer on the battlefield if able.", "Creature — Human Flagbearer");
    rejects(flag.replace("that player", "you"), "Creature — Human Flagbearer");
  });
});

describe("revealing this card from the deck before the game (CR 103.3, 905.1d)", () => {
  const head = "Before you shuffle your deck to start the game, you may reveal this card from your deck and exile ";
  const pool = { draftedNotInDeck: { drafter: { player: "you" } } };
  const reveal = (what: object) => ({ kind: "static", effect: { static: "pregame-deck-reveal", what: { ref: "this", noun: "card" },
    who: { player: "you" }, effects: [{ effect: "exile", what }] } });
  it("exiles one drafted card outside the deck (Caller of the Untamed)", () => {
    assert.deepEqual(only(head + "a creature card you drafted that isn't in your deck.", "Creature — Elf Shaman"),
      reveal({ ref: "filter", filter: { types: ["creature"], cls: "card", ...pool }, count: fixed(1) }));
  });
  it("keeps a minimum number of chosen cards (Volatile Chimera)", () => {
    assert.deepEqual(only(head + "three or more creature cards you drafted that aren't in your deck.", "Creature — Elemental Chimera"),
      reveal({ ref: "filter", filter: { types: ["creature"], cls: "card", ...pool }, count: { amount: "chosen", minimum: 3 } }));
  });
  it("rejects number disagreement, a dropped deck exclusion, and a different time", () => {
    rejects(head + "three or more creature cards you drafted that isn't in your deck.", "Creature — Elemental Chimera");
    rejects(head + "a creature card you drafted.", "Creature — Elf Shaman");
    rejects(head + "two creature cards you drafted that aren't in your deck.", "Creature — Elf Shaman");
    rejects("Before you draw your opening hand, you may reveal this card from your deck and exile a creature card you drafted that isn't in your deck.", "Creature — Elf Shaman");
  });
});

describe("a leading payment offer governs the whole sentence (CR 118.12a)", () => {
  it("covers every instruction of the sentence (Rhystic Syphon)", () => {
    assert.deepEqual(only("Unless target player pays {3}, that player loses 5 life and you gain 5 life.", "Sorcery"), { kind: "spell", effects: [{
      steps: [{ effect: "lose-life", who: { player: "that-player" }, amount: fixed(5) }, { effect: "gain-life", who: { player: "you" }, amount: fixed(5) }],
      condition: { condition: "pays", who: { player: "target-player" }, cost: [{ cost: "mana", symbols: ["3"] }] }, conditionKind: "unless" }] });
  });
  it("lets any player pay (Rhystic Tutor)", () => {
    const ability = only("Unless any player pays {2}, search your library for a card, put that card into your hand, then shuffle.", "Sorcery");
    assert.equal(ability.kind, "spell");
    if (ability.kind !== "spell") return;
    assert.equal(ability.effects[0].steps.length, 3);
    assert.deepEqual(ability.effects[0].condition, { condition: "pays", who: { player: "a-player" }, cost: [{ cost: "mana", symbols: ["2"] }] });
    assert.equal(ability.effects[0].conditionKind, "unless");
  });
  it("rejects a second condition and a leading unless that is not a payment", () => {
    rejects("Unless any player pays {2}, draw a card unless an opponent pays {1}.", "Sorcery");
    rejects("Unless any player pays {2}, draw a card if you control a Forest.", "Sorcery");
    rejects("Unless you control a Forest, draw a card.", "Sorcery");
    // A trailing offer after several instructions keeps its unresolved scope.
    rejects("Draw a card, then gain 2 life unless an opponent pays {2}.", "Sorcery");
  });
});
