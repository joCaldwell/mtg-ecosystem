import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ObjectRef, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Static abilities. Wordings are taken from
// the cached Scryfall corpus; card names are noted per test.
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
const eachOtherPlayer = { player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "you" } } };

describe("day/night as a permanent enters (CR 614.1c, 731.1)", () => {
  const neither = { condition: "day-night", state: "neither" };
  it("keeps the condition and the entering permanent (Gavony Dawnguard, The Celestus)", () => {
    assert.deepEqual(effect("If it's neither day nor night, it becomes day as this creature enters.", "Creature — Human Soldier"), {
      static: "as-enters", what: creature,
      effects: [{ condition: neither, conditionKind: "if", steps: [{ effect: "set-day-night", to: "day" }] }],
    });
    assert.deepEqual(effect("If it's neither day nor night, it becomes day as The Celestus enters.", "Legendary Artifact", "The Celestus"), {
      static: "as-enters", what: { ref: "self" },
      effects: [{ condition: neither, conditionKind: "if", steps: [{ effect: "set-day-night", to: "day" }] }],
    });
    assert.deepEqual(effect("If it's neither day nor night, it becomes night as this creature enters.", "Creature — Human Soldier"), {
      static: "as-enters", what: creature,
      effects: [{ condition: neither, conditionKind: "if", steps: [{ effect: "set-day-night", to: "night" }] }],
    });
  });

  it("does not turn other trailing entering clauses into entry instructions", () => {
    rejects("If it's neither day nor night, draw a card as this creature enters.", "Creature — Human Soldier");
    rejects("If it's neither day nor night, it becomes day as another creature enters.", "Creature — Human Soldier");
    rejects("If it's neither day nor night, it becomes day as this creature attacks.", "Creature — Human Soldier");
    rejects("If it's neither day nor night, it becomes day.", "Creature — Human Soldier");
  });
});

describe("untap-step rules (CR 502.3)", () => {
  it("untaps the controller's permanents during other players' untap steps (Seedborn Muse, Endbringer, Ivorytusk Fortress)", () => {
    assert.deepEqual(effect("Untap all permanents you control during each other player's untap step.", "Creature — Spirit"), {
      static: "untap-during-step", what: { ref: "all", filter: { cls: "permanent", control: "you" } },
      step: { occurrence: "each", owner: eachOtherPlayer },
    });
    assert.deepEqual(effect("Untap this creature during each other player's untap step.", "Creature — Eldrazi"), {
      static: "untap-during-step", what: creature, step: { occurrence: "each", owner: eachOtherPlayer },
    });
    assert.deepEqual(effect("Untap each creature you control with a +1/+1 counter on it during each other player's untap step.", "Creature — Elephant"), {
      static: "untap-during-step",
      what: { ref: "each", filter: { types: ["creature"], control: "you",
        counterConstraints: [{ counter: { type: "+1/+1" }, comparison: { op: "ge", value: fixed(1) } }] } },
      step: { occurrence: "each", owner: eachOtherPlayer },
    });
    assert.deepEqual(effect("Untap all permanents you control during each opponent's untap step.", "Enchantment"), {
      static: "untap-during-step", what: { ref: "all", filter: { cls: "permanent", control: "you" } },
      step: { occurrence: "each", owner: { player: "each-opponent" } },
    });
  });

  it("keeps the untap instruction a spell effect without the step clause, and rejects other owners", () => {
    rejects("Untap all permanents you control.", "Creature — Spirit");
    rejects("Untap all permanents you control during your untap step.", "Creature — Spirit");
    rejects("Untap all permanents during each other player's untap step.", "Creature — Spirit");
    rejects("Untap target creature during each other player's untap step.", "Creature — Spirit");
    rejects("Untap all permanents you control during each other player's upkeep.", "Creature — Spirit");
  });

  it("limits how many permanents a player untaps (Winter Orb, Smoke, Winter Moon, Mungha Wurm)", () => {
    assert.deepEqual(effect("Players can't untap more than one creature during their untap steps.", "Enchantment"), {
      static: "untap-step-limit", who: { player: "each-player" }, what: { types: ["creature"] }, maximum: fixed(1),
    });
    assert.deepEqual(effect("Players can't untap more than one nonbasic land during their untap steps.", "Artifact"), {
      static: "untap-step-limit", who: { player: "each-player" }, what: { nonSupertypes: ["basic"], types: ["land"] }, maximum: fixed(1),
    });
    assert.deepEqual(effect("You can't untap more than one land during your untap step.", "Creature — Wurm"), {
      static: "untap-step-limit", who: { player: "you" }, what: { types: ["land"] }, maximum: fixed(1),
    });
    assert.deepEqual(effect("As long as this artifact is untapped, players can't untap more than two permanents during their untap steps.", "Artifact"), {
      static: "conditional", condition: { condition: "status", what: { ref: "this", noun: "artifact" }, status: "untapped" },
      effect: { static: "untap-step-limit", who: { player: "each-player" }, what: { cls: "permanent" }, maximum: fixed(2) },
    });
  });

  it("rejects untap limits outside the limited player's own untap step", () => {
    rejects("Players can't untap more than one creature.", "Enchantment");
    rejects("Players can't untap more than one creature during your untap step.", "Enchantment");
    rejects("You can't untap more than one land during their untap steps.", "Creature — Wurm");
    rejects("Players can't untap more than one creature you control during their untap steps.", "Enchantment");
    rejects("Players can't untap one creature during their untap steps.", "Enchantment");
  });
});

describe("abilities of the ordinal spell cast in a turn (CR 601.2)", () => {
  const keyword = (name: string, extra: object = {}) => ({ gained: "keyword", keyword: { keyword: name, ...extra } });
  const first = { ordinal: 1, period: "turn" };
  it("keeps the caster, the counted spells and the ordinal (Maelstrom Nexus, Ashad, Anhelo)", () => {
    assert.deepEqual(effect("The first spell you cast each turn has cascade.", "Enchantment"), {
      static: "cast-spell-abilities",
      scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "you" }, occurrence: first },
      abilities: [keyword("cascade")],
    });
    assert.deepEqual(effect("The first nonlegendary artifact spell you cast each turn has casualty 2.", "Legendary Artifact Creature — Cyberman"), {
      static: "cast-spell-abilities",
      scope: { action: "cast", spells: { ref: "each", filter: { nonSupertypes: ["legendary"], types: ["artifact"], cls: "spell" } },
        caster: { player: "you" }, occurrence: first },
      abilities: [keyword("casualty", { amount: 2 })],
    });
    assert.deepEqual(effect("The second spell you cast each turn has cascade.", "Enchantment"), {
      static: "cast-spell-abilities",
      scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "you" }, occurrence: { ordinal: 2, period: "turn" } },
      abilities: [keyword("cascade")],
    });
  });

  it("counts only spells cast from the stated origin (Wild-Magic Sorcerer, The Twelfth Doctor)", () => {
    assert.deepEqual(effect("The first spell you cast from exile each turn has cascade.", "Creature — Orc Shaman"), {
      static: "cast-spell-abilities",
      scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "you" },
        from: { zone: "exile" }, occurrence: first },
      abilities: [keyword("cascade")],
    });
    assert.deepEqual(effect("The first spell you cast from anywhere other than your hand each turn has demonstrate.", "Legendary Creature — Time Lord Doctor"), {
      static: "cast-spell-abilities",
      scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "you" },
        notFrom: { zone: "hand", owner: "your" }, occurrence: first },
      abilities: [keyword("demonstrate")],
    });
  });

  it("leaves unordered grants as ordinary modifiers and rejects other periods or riders", () => {
    assert.equal(effect("Creature spells you cast have convoke.", "Enchantment").static, "modify");
    rejects("The first spell you cast has cascade.", "Enchantment");
    rejects("The first spell cast each turn has cascade.", "Enchantment");
    rejects("The first spell you cast during each of your turns has assist.", "Enchantment");
    rejects("The first spell you cast each turn that mana from a Treasure was spent to cast has cascade.", "Enchantment");
    rejects("The next spell you cast each turn has cascade.", "Enchantment");
    rejects("The first spell you cast each turn has cascade until end of turn.", "Enchantment");
  });
});

describe("Attraction visit abilities (CR 702.159a, 717.5)", () => {
  it("is its own ability kind whose body is resolution text (Information Booth, Spinny Ride, Kiddie Coaster)", () => {
    assert.deepEqual(only("Visit — Draw a card.", "Artifact — Attraction"), {
      kind: "visit", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
    });
    assert.deepEqual(only("Visit — Tap target creature an opponent controls.", "Artifact — Attraction"), {
      kind: "visit", effects: [{ steps: [{ effect: "tap",
        what: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: fixed(1) } }] }],
    });
    assert.deepEqual(only("Visit — Creatures you control get +1/+0 until end of turn.", "Artifact — Attraction"), {
      kind: "visit", effects: [{ steps: [{ effect: "pump", what: { ref: "each", filter: { types: ["creature"], control: "you" } },
        power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) }, duration: { duration: "end-of-turn" } }] }],
    });
  });

  it("is never an ability word, a static ability or a prize it cannot represent", () => {
    // The same text without the label is a one-shot instruction a permanent cannot have.
    rejects("Draw a card.", "Artifact — Attraction");
    rejects("Visit — Flying", "Artifact — Attraction");
    rejects("Visit —", "Artifact — Attraction");
    rejects("Visit — Create a Treasure token. Then if there are six or more luck counters on Pick-a-Beeble, claim the prize!", "Artifact — Attraction", "Pick-a-Beeble");
    rejects("Visit — Draw a card. Activate only as a sorcery.", "Artifact — Attraction");
    rejects("Visit: Draw a card.", "Artifact — Attraction");
  });
});

describe("control and ownership joined by \"but\" (CR 108.3, 108.4)", () => {
  const pt = (power: number, toughness: number) => ({ modifier: "pt", power: { sign: 1, amount: fixed(power) }, toughness: { sign: 1, amount: fixed(toughness) } });
  it("requires both on the same object (Herald of Leshrac, Laughing Jasper Flint, Coveted Falcon)", () => {
    assert.deepEqual(effect("This creature gets +1/+1 for each land you control but don't own.", "Creature — Avatar"), {
      static: "modify", what: creature,
      modifiers: [{ ...pt(1, 1), forEach: { types: ["land"], control: "you", own: "not-you" } }],
    });
    assert.deepEqual(effect("Creatures you control but don't own are Mercenaries in addition to their other types.", "Legendary Creature — Lizard Rogue"), {
      static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "you", own: "not-you" } },
      modifiers: [{ modifier: "types", change: { subtypes: [{ family: "creature", operation: "add", types: ["Mercenary"] }] } }],
    });
    assert.deepEqual(only("Whenever this creature attacks, gain control of target permanent you own but don't control.", "Artifact Creature — Bird"), {
      kind: "triggered", trigger: { trigger: "attacks", what: creature },
      effects: [{ steps: [{ effect: "gain-control", who: { player: "you" },
        what: { ref: "target", filter: { cls: "permanent", own: "you", control: "not-you" }, count: fixed(1) } }] }],
    });
  });

  it("rejects an incomplete or contradictory second half", () => {
    rejects("Creatures you control but don't get +2/+2.", "Creature — Avatar");
    rejects("Creatures you control but don't control get +2/+2.", "Creature — Avatar");
    rejects("Creatures you control but own get +2/+2.", "Creature — Avatar");
  });
});

describe("permanents that can't be sacrificed (CR 701.21a)", () => {
  it("is a prohibition on the permanent, alone or among other modifiers (Hithlain Rope, Garland, Alexios)", () => {
    assert.deepEqual(effect("This artifact can't be sacrificed.", "Artifact"), {
      static: "cant", what: { ref: "this", noun: "artifact" }, action: "be-sacrificed",
    });
    assert.deepEqual(effect("Creatures you control but don't own get +2/+2 and can't be sacrificed.", "Legendary Creature — Human Knight"), {
      static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "you", own: "not-you" } },
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(2) } },
        { modifier: "cant", action: "be-sacrificed" }],
    });
    assert.deepEqual(effect("Alexios attacks each combat if able, can't be sacrificed, and can't attack its owner.", "Legendary Creature — Human Berserker", "Alexios"), {
      static: "modify", what: { ref: "self" }, modifiers: [
        { modifier: "combat-rule", rule: { rule: "attack-requirement", frequency: "each-combat" } },
        { modifier: "cant", action: "be-sacrificed" },
        { modifier: "combat-rule", rule: { rule: "attack-restriction", defenders: [{ defender: "player", who: { player: "owner", of: { ref: "it" } } }] } },
      ],
    });
  });

  it("does not accept rules about who may cause a sacrifice as the same prohibition", () => {
    assert.deepEqual(effect("Spells and abilities your opponents control can't cause you to sacrifice permanents.", "Creature — Elf Shaman"), {
      static: "prohibit", prohibition: { prohibition: "caused-action", who: { player: "you" }, actions: ["sacrifice"],
        what: { ref: "each", filter: { cls: "permanent" } },
        by: { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }], control: "opponent" } },
    });
    rejects("This artifact can't be sacrificed by opponents.", "Artifact");
    rejects("This artifact can't sacrifice.", "Artifact");
  });
});
