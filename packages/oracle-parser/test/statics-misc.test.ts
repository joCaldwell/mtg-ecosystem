import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ObjectRef, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings are taken from the cached Scryfall corpus (Control Magic, Xantcha,
// Panharmonicon, Torpor Orb, Sanctum of All, Vivid Crag, Grimgrin, Prime
// Speaker Zegana, Kavu Titan, Dust Animus, Spelunking, Sea Serpent, Steel of
// the Godhead, Basking Capybara). Rules: CR 613.1b, 603.2d, 614.1c-d, 614.12.
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

const creature: ObjectRef = { ref: "this", noun: "creature" };
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const one = (filter: object) => ({ ref: "filter", filter, count: fixed(1) });
const yourPermanents = { cls: "ability", abilityKind: "triggered", abilitySource: one({ cls: "permanent", control: "you" }) };
const keyword = (name: string) => ({ gained: "keyword", keyword: { keyword: name } });

describe("control-changing statics", () => {
  it("represents an Aura's continuous control of the enchanted permanent", () => {
    assert.deepEqual(effect("You control enchanted creature.", "Enchantment — Aura"),
      { static: "control", who: { player: "you" }, what: { ref: "enchanted", noun: "creature" } });
    assert.deepEqual(effect("You control enchanted land.", "Enchantment — Aura"),
      { static: "control", who: { player: "you" }, what: { ref: "enchanted", noun: "land" } });
  });

  it("does not accept targets or durations as a control static", () => {
    rejects("You control target creature.", "Enchantment — Aura");
    rejects("You control enchanted creature until end of turn.", "Enchantment — Aura");
  });

  it("keeps the chooser of the opponent a permanent enters under", () => {
    assert.deepEqual(effect("Xantcha enters under the control of an opponent of your choice.", "Legendary Creature — Phyrexian Minion", "Xantcha"),
      { static: "enters-under-control", what: { ref: "self" }, controller: { choice: "opponent", who: { player: "you" } } });
    rejects("Xantcha enters under the control of an opponent.", "Legendary Creature — Phyrexian Minion", "Xantcha");
  });
});

describe("rules about how often abilities trigger", () => {
  it("restricts additional triggers to the causing event and the ability's source", () => {
    assert.deepEqual(effect("If an artifact or creature entering causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.", "Artifact"), {
      static: "additional-triggers", abilities: yourPermanents,
      cause: { trigger: "enters", what: one({ allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }] }) },
      additional: 1,
    });
    assert.deepEqual(effect("If a creature dying causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.", "Legendary Creature — Human Advisor"), {
      static: "additional-triggers", abilities: yourPermanents,
      cause: { trigger: "dies", what: one({ types: ["creature"] }) }, additional: 1,
    });
  });

  it("omits the cause for any trigger event and keeps a while condition", () => {
    const shrines = { cls: "ability", abilityKind: "triggered", abilitySource: one({ subtypes: ["Shrine"], control: "you", other: true }) };
    assert.deepEqual(effect("If a triggered ability of another Shrine you control triggers, it triggers an additional time.", "Legendary Enchantment — Shrine"),
      { static: "additional-triggers", abilities: shrines, additional: 1 });
    assert.deepEqual(effect("If a triggered ability of another Shrine you control triggers while you control six or more Shrines, that ability triggers an additional time.", "Legendary Enchantment — Shrine"), {
      static: "additional-triggers", abilities: shrines, additional: 1,
      condition: { condition: "control-count", who: { player: "you" }, of: { subtypes: ["Shrine"] }, comparison: { op: "ge", value: fixed(6) } },
    });
  });

  it("rejects scaled counts and non-triggered ability categories", () => {
    rejects("If a triggered ability of a Fish you control triggers, that ability triggers an additional time for each Fish you control.", "Creature — Fish");
    rejects("If a creature dying causes an activated ability of a permanent you control to trigger, that ability triggers an additional time.", "Creature — Fish");
  });

  it("distinguishes suppressing triggers from additional triggers", () => {
    assert.deepEqual(effect("Creatures entering don't cause abilities to trigger.", "Artifact"), {
      static: "trigger-suppression", abilities: { cls: "ability" },
      cause: { trigger: "enters", what: one({ types: ["creature"] }) },
    });
    assert.deepEqual(effect("Permanents entering don't cause abilities of permanents your opponents control to trigger.", "Legendary Creature — Phyrexian Praetor"), {
      static: "trigger-suppression",
      abilities: { cls: "ability", abilitySource: { ref: "each", filter: { cls: "permanent", control: "opponent" } } },
      cause: { trigger: "enters", what: one({ cls: "permanent" }) },
    });
    assert.deepEqual(effect("Creatures entering or dying don't cause abilities to trigger.", "Creature — Faerie"), {
      static: "trigger-suppression", abilities: { cls: "ability" },
      cause: { trigger: "any-of", events: [
        { trigger: "enters", what: one({ types: ["creature"] }) }, { trigger: "dies", what: one({ types: ["creature"] }) }] },
    });
    rejects("Creatures entering don't cause backup abilities to trigger.", "Creature — Human");
    rejects("Creatures don't cause abilities to trigger.", "Creature — Human");
  });
});

describe("turn-conditioned statics", () => {
  it("treats 'during your turn' as the continuously checked your-turn condition", () => {
    assert.deepEqual(effect("During your turn, equipped creature has flying.", "Artifact — Equipment"), {
      static: "conditional", condition: { condition: "your-turn" },
      effect: { static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [keyword("flying")] }] },
    });
    assert.deepEqual(effect("During turns other than yours, this creature gets +0/+2.", "Creature — Cat"), {
      static: "conditional", condition: { condition: "not-your-turn" },
      effect: { static: "modify", what: creature, modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(0) }, toughness: { sign: 1, amount: fixed(2) } }] },
    });
  });

  it("does not accept other periods or one-shot instructions", () => {
    rejects("During your end step, creatures you control have hexproof.", "Creature — Orc");
    rejects("During your turn, draw a card.", "Creature — Orc");
  });
});

describe("compound entry replacements", () => {
  const land: ObjectRef = { ref: "this", noun: "land" };
  it("keeps tapped and counters as parts of one entry", () => {
    assert.deepEqual(effect("This land enters tapped with two charge counters on it.", "Land"), {
      static: "group", effects: [{ static: "enters-tapped", what: land },
        { static: "enters-with-counters", what: land, counter: { type: "charge" }, count: fixed(2) }],
    });
  });

  it("keeps an untap-step restriction separate from entering tapped", () => {
    assert.deepEqual(effect("This creature enters tapped and doesn't untap during your untap step.", "Creature — Leviathan"), {
      static: "group", effects: [{ static: "enters-tapped", what: creature },
        { static: "untap-step-suppression", what: creature, step: { occurrence: "each", owner: { player: "you" } } }],
    });
    // A trailing condition could govern either clause.
    rejects("This creature enters tapped and doesn't untap during your untap step if you control a Swamp.", "Creature — Leviathan");
    rejects("This creature enters tapped with flying.", "Creature — Leviathan");
  });

  it("binds a printed definition of X to the entry count", () => {
    assert.deepEqual(effect("This creature enters with X +1/+1 counters on it, where X is the number of other creatures on the battlefield.", "Creature — Insect"), {
      static: "enters-with-counters", what: creature, counter: { type: "+1/+1" }, count: { amount: "x" },
      bindings: [{ variable: "x", value: { amount: "count", of: { other: true, types: ["creature"], zone: { zone: "battlefield" } } } }],
    });
    rejects("This creature enters with two +1/+1 counters on it, where X is the number of creatures you control.", "Creature — Insect");
  });

  it("represents abilities and several counter kinds a permanent enters with", () => {
    const kicked = { condition: "kicked", what: creature, comparison: { op: "ge", value: fixed(1) } };
    assert.deepEqual(effect("If this creature was kicked, it enters with three +1/+1 counters on it and with trample.", "Creature — Kavu"), {
      static: "conditional", condition: kicked,
      effect: { static: "group", effects: [
        { static: "enters-with-counters", what: { ref: "it" }, counter: { type: "+1/+1" }, count: fixed(3) },
        { static: "enters-with-abilities", what: { ref: "it" }, abilities: [keyword("trample")] }] },
    });
    assert.deepEqual(effect("This creature enters with two +1/+1 counters and a lifelink counter on it.", "Creature — Elemental"), {
      static: "group", effects: [
        { static: "enters-with-counters", what: creature, counter: { type: "+1/+1" }, count: fixed(2) },
        { static: "enters-with-counters", what: creature, counter: { type: "lifelink" }, count: fixed(1) }],
    });
    rejects("If this creature was kicked, it enters with two +1/+1 counters on it and with flying if you control a Swamp.", "Creature — Kavu");
  });

  it("represents entering untapped as its own entry rule", () => {
    assert.deepEqual(effect("Lands you control enter untapped.", "Enchantment"),
      { static: "enters-untapped", what: { ref: "each", filter: { types: ["land"], control: "you" } } });
    rejects("Lands you control enter untapped unless you control a Swamp.", "Enchantment");
  });
});

describe("conditions used by attack restrictions and color-keyed Auras", () => {
  it("tests what the defending player controls", () => {
    assert.deepEqual(effect("This creature can't attack unless defending player controls an Island.", "Creature — Serpent"), {
      static: "cant", what: creature, action: "attack", conditionKind: "unless",
      condition: { condition: "control", who: { player: "defending-player" }, what: one({ subtypes: ["Island"] }) },
    });
  });

  it("accepts a color-only predicate and nothing looser", () => {
    assert.deepEqual(effect("As long as enchanted creature is blue, it gets +1/+1.", "Enchantment — Aura"), {
      static: "conditional",
      condition: { condition: "is-filter", what: { ref: "enchanted", noun: "creature" }, filter: { colors: ["blue"] } },
      effect: { static: "modify", what: { ref: "it" }, modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) } }] },
    });
    rejects("As long as enchanted creature is big, it gets +1/+1.", "Enchantment — Aura");
  });
});

describe("numbered ability words", () => {
  it("keeps Descend 4 as a label on the complete ability (CR 207.2c)", () => {
    assert.deepEqual(only("Descend 4 — This creature has flying as long as there are four or more permanent cards in your graveyard.", "Creature — Spirit"), {
      kind: "static", abilityWord: "Descend 4",
      effect: { static: "modify", what: creature, modifiers: [{ modifier: "abilities", abilities: [keyword("flying")] }],
        condition: { condition: "count", of: { cls: "card", permanent: true, zone: { zone: "graveyard", owner: "your" } },
          comparison: { op: "ge", value: fixed(4) } } },
    });
  });

  it("does not turn other numbered prefixes into ability words", () => {
    rejects("Descend 5 — This creature has flying.", "Creature — Spirit");
    rejects("Delve 4 — This creature has flying.", "Creature — Spirit");
  });
});
