import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, GameProhibition, ObjectFilter, ObjectRef, PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseProhibitionEffect, parseStaticProhibition } from "../src/parser/prohibitions.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Card clauses checked against cached Scryfall oracle corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 101.2, 305.1, 601.3, 602.1c, 605.1, 606.2, 611.2c, 615.12:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you: PlayerRef = { player: "you" };
const players: PlayerRef = { player: "each-player" };
const opponents: PlayerRef = { player: "each-opponent" };
const each = (filter: ObjectFilter): ObjectRef => ({ ref: "each", filter });
const spells = each({ cls: "spell" });
const ban = (prohibition: GameProhibition) => ({ static: "prohibit", prohibition });
const abilityQuery = (filter: ObjectFilter = {}): ObjectRef => each({ cls: "ability", abilityKind: "activated", ...filter });
function one(text: string): Ability {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe("casting and land-play prohibitions", () => {
  it("preserves actors and complete spell eligibility without removing abilities", () => {
    // Steel Golem, Nullhide Ferox, Llawan, Brisela.
    for (const [text, who, filter] of [
      ["You can't cast creature spells.", you, { types: ["creature"], cls: "spell" }],
      ["You can't cast noncreature spells.", you, { nonTypes: ["creature"], cls: "spell" }],
      ["Your opponents can't cast blue creature spells.", opponents, { colors: ["blue"], types: ["creature"], cls: "spell" }],
      ["Your opponents can't cast spells with mana value 3 or less.", opponents, { cls: "spell",
        properties: [{ property: "mana-value", comparison: { op: "le", value: fixed(3) } }] }],
    ] as [string, PlayerRef, ObjectFilter][]) assert.deepEqual(one(text), { kind: "static", effect: ban({ prohibition: "cast", who, what: each(filter) }) });
    assert.deepEqual(one("Enchanted creature's controller can't cast creature spells."), {
      kind: "static", effect: ban({ prohibition: "cast", who: { player: "controller", of: { ref: "enchanted", noun: "creature" } },
        what: each({ types: ["creature"], cls: "spell" }) }),
    });
  });

  it("keeps source-zone alternatives and each acting player's own hand explicit", () => {
    // Grafdigger's Cage, Soulless Jailer, Drannith Magistrate.
    assert.deepEqual(one("Players can't cast spells from graveyards or libraries."), {
      kind: "static", effect: ban({ prohibition: "cast", who: players, what: spells,
        from: { relation: "in", zones: [{ zone: "graveyard" }, { zone: "library" }] } }),
    });
    assert.deepEqual(one("Players can't cast noncreature spells from graveyards or exile."), {
      kind: "static", effect: ban({ prohibition: "cast", who: players, what: each({ nonTypes: ["creature"], cls: "spell" }),
        from: { relation: "in", zones: [{ zone: "graveyard" }, { zone: "exile" }] } }),
    });
    assert.deepEqual(one("Your opponents can't cast spells from anywhere other than their hands."), {
      kind: "static", effect: ban({ prohibition: "cast", who: opponents, what: spells,
        from: { relation: "outside", zones: [{ zone: "hand", owner: "acting-player" }] } }),
    });
  });

  it("retains both Experimental Frenzy actions and their shared hand restriction", () => {
    const from = { relation: "in" as const, zones: [{ zone: "hand" as const, owner: "your" as const }] };
    assert.deepEqual(one("You can't play lands or cast spells from your hand."), {
      kind: "static", effect: { static: "group", effects: [
        ban({ prohibition: "play", who: you, what: each({ types: ["land"] }), from }),
        ban({ prohibition: "cast", who: you, what: spells, from }),
      ] },
    });
    assert.deepEqual(one("Players can't play lands."), {
      kind: "static", effect: ban({ prohibition: "play", who: players, what: each({ types: ["land"] }) }),
    });
  });

  it("does not narrow all-source activation bans to permanent cards on the battlefield", () => {
    // Ashes of the Abhorrent explicitly covers cards in graveyards.
    assert.deepEqual(one("Players can't cast spells from graveyards or activate abilities of cards in graveyards."), {
      kind: "static", effect: { static: "group", effects: [
        ban({ prohibition: "cast", who: players, what: spells, from: { relation: "in", zones: [{ zone: "graveyard" }] } }),
        ban({ prohibition: "activate", who: players,
          abilities: abilityQuery({ abilitySource: each({ cls: "card", zone: { zone: "graveyard" } }) }) }),
      ] },
    });
  });

  it("retains a casting restriction's unless condition as continuous applicability", () => {
    // Rakdos, Lord of Riots: this is a restriction on proposing the cast,
    // not a resolving spell that grants a restriction after being cast.
    assert.deepEqual(one("You can't cast this spell unless an opponent lost life this turn."), {
      kind: "static", effect: { static: "conditional", condition: { condition: "not", of: {
        condition: "life-change", who: { player: "an-opponent" }, change: "lost",
        comparison: { op: "gt", value: fixed(0) }, during: "this-turn",
      } }, effect: ban({ prohibition: "cast", who: you, what: { ref: "this", noun: "spell" } }) },
    });
  });
});

describe("activation restrictions", () => {
  it("distinguishes the source filter from its abilities and retains mana exceptions exactly", () => {
    assert.deepEqual(one("Activated abilities of artifacts can't be activated."), {
      kind: "static", effect: ban({ prohibition: "activate", abilities: abilityQuery({ abilitySource: each({ types: ["artifact"] }) }) }),
    });
    assert.deepEqual(one("Activated abilities of artifacts and creatures can't be activated unless they're mana abilities."), {
      kind: "static", effect: ban({ prohibition: "activate", abilities: abilityQuery({
        abilitySource: each({ allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }] }), manaAbility: false,
      }) }),
    });
    assert.deepEqual(one("Activated abilities of artifact creatures can't be activated."), {
      kind: "static", effect: ban({ prohibition: "activate", abilities: abilityQuery({ abilitySource: each({ types: ["artifact", "creature"] }) }) }),
    });
  });

  it("keeps independent mana and loyalty exclusions conjunctive", () => {
    // Overwhelming Splendor versus The Immortal Sun.
    assert.deepEqual(one("Enchanted player can't activate abilities that aren't mana abilities or loyalty abilities."), {
      kind: "static", effect: ban({ prohibition: "activate", who: { player: "enchanted-player" },
        abilities: abilityQuery({ manaAbility: false, loyaltyAbility: false }) }),
    });
    assert.deepEqual(one("Players can't activate planeswalkers' loyalty abilities."), {
      kind: "static", effect: ban({ prohibition: "activate", who: players,
        abilities: abilityQuery({ abilitySource: each({ types: ["planeswalker"] }), loyaltyAbility: true }) }),
    });
  });

  it("binds the repeated source of Arrest and Faith's Fetters without changing quoted abilities", () => {
    for (const [noun, exception] of [["creature", false], ["permanent", true]] as const) {
      const what = { ref: "enchanted", noun } as const;
      assert.deepEqual(one(`Enchanted ${noun} can't attack or block, and its activated abilities can't be activated${exception ? " unless they're mana abilities" : ""}.`), {
        kind: "static", effect: { static: "group", effects: [
          { static: "cant", what, action: "attack-or-block" },
          ban({ prohibition: "activate", abilities: abilityQuery({ abilitySource: what, ...(exception ? { manaAbility: false } : {}) }) }),
        ] },
      });
    }
  });
});

describe("recurring windows versus resolving lifetimes", () => {
  it("keeps during combat recurring and this turn temporary", () => {
    assert.deepEqual(one("Your opponents can't cast spells during combat."), {
      kind: "static", effect: ban({ prohibition: "cast", who: opponents, what: spells, when: { during: "combat" } }),
    });
    for (const [text, duration] of [
      ["Your opponents can't cast spells this turn.", { duration: "this-turn" }],
      ["Until end of turn, your opponents can't cast spells.", { duration: "end-of-turn" }],
      ["Your opponents can't cast spells until your next turn.", { duration: "your-next-turn" }],
    ] as const) assert.deepEqual(one(text), { kind: "spell", effects: [{ steps: [
      { effect: "prohibit", prohibition: { prohibition: "cast", who: opponents, what: spells }, duration },
    ] }] });
  });

  it("applies Grand Abolisher's recurring turn window to both actions", () => {
    const when = { during: "turn" as const, whose: you };
    assert.deepEqual(one("During your turn, your opponents can't cast spells or activate abilities of artifacts, creatures, or enchantments."), {
      kind: "static", effect: { static: "group", effects: [
        ban({ prohibition: "cast", who: opponents, what: spells, when }),
        ban({ prohibition: "activate", who: opponents, when, abilities: abilityQuery({ abilitySource: each({
          allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] }] }],
        }) }) }),
      ] },
    });
  });

  it("applies Revel in Silence's temporary duration to both actions", () => {
    assert.deepEqual(one("Your opponents can't cast spells or activate planeswalkers' loyalty abilities this turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "prohibit", prohibition: { prohibition: "cast", who: opponents, what: spells }, duration: { duration: "this-turn" } },
        { effect: "prohibit", prohibition: { prohibition: "activate", who: opponents,
          abilities: abilityQuery({ abilitySource: each({ types: ["planeswalker"] }), loyaltyAbility: true }) }, duration: { duration: "this-turn" } },
      ] }] }],
    });
  });
});

describe("per-player casting quotas", () => {
  it("counts the complete turn separately for each player and retains eligible subsets", () => {
    // Rule of Law, Deafening Silence, Phyrexian Censor, Curse of Exhaustion.
    for (const [text, who, what] of [
      ["Each player can't cast more than one spell each turn.", players, { cls: "spell" }],
      ["Each player can't cast more than one noncreature spell each turn.", players, { nonTypes: ["creature"], cls: "spell" }],
      ["Each player can't cast more than one non-Phyrexian spell each turn.", players, { nonSubtypes: ["Phyrexian"], cls: "spell" }],
      ["Enchanted player can't cast more than one spell each turn.", { player: "enchanted-player" }, { cls: "spell" }],
    ] as const) assert.deepEqual(one(text), { kind: "static", effect: { static: "cast-limit", limit: {
      who, what, maximum: fixed(1), period: "turn", count: "each-player-all-casts",
    } } });
  });
});

describe("damage prevention prohibitions", () => {
  it("separates global, combat-only, source-filtered and source-object prohibitions", () => {
    for (const [text, prohibition] of [
      ["Damage can't be prevented.", { prohibition: "prevent-damage", kind: "all" }],
      ["Combat damage can't be prevented.", { prohibition: "prevent-damage", kind: "combat" }],
      ["Damage that would be dealt by this creature can't be prevented.", { prohibition: "prevent-damage", kind: "all", by: { ref: "this", noun: "creature" } }],
      ["Combat damage that would be dealt by creatures you control can't be prevented.", { prohibition: "prevent-damage", kind: "combat", by: each({ types: ["creature"], control: "you" }) }],
    ] as const) assert.deepEqual(one(text), { kind: "static", effect: ban(prohibition) });
    assert.deepEqual(one("Damage can't be prevented this turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "prohibit", prohibition: { prohibition: "prevent-damage", kind: "all" }, duration: { duration: "this-turn" } }] }],
    });
  });

  it("refuses to turn a particular damage event into a global prohibition", () => {
    for (const text of [
      "This spell deals 4 damage to target creature. The damage can't be prevented.",
      "This damage can't be prevented.",
      "Damage can't be prevented or dealt instead to another player.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("prohibition rejection and binding boundaries", () => {
  it("rejects unsupported costs, quota scopes, negation lists, and incomplete instructions transactionally", () => {
    for (const text of [
      "Players can't activate abilities that aren't mana abilities or triggered abilities.",
      "Activated abilities of artifacts can't be activated unless you pay {2}.",
      "Each player can't cast more than one spell this way each turn.",
      "Players can't cast spells from anywhere other than.",
      "Players can't cast spells during the first combat of each turn.",
      "Players can't cast spells or play.",
      "You can't cast spells unless an opponent pays {2}.",
      "You can't cast spells if an opponent pays {2}.",
    ]) {
      const cursor = new Cursor(lex(text));
      const parsed = parseProhibitionEffect(cursor);
      // Lower-level parsers can return a complete prefix, but no card line
      // can pass while an unsupported qualifier remains unconsumed.
      if (!parsed) assert.equal(cursor.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
    const cursor = new Cursor(lex("Target player can't cast spells this turn."));
    assert.equal(parseStaticProhibition(cursor), null);
    assert.equal(cursor.pos, 0);
  });

  it("does not certify unimplemented dynamic action-scope bindings", () => {
    for (const text of [
      "Your opponents can't cast spells from anywhere other than their hands.",
      "Activated abilities of artifacts can't be activated.",
      "Damage can't be prevented this turn.",
      "Each player can't cast more than one spell each turn.",
    ]) {
      const result = parseOracleText(text, "Prohibition fixture", { includeSourceProvenance: true });
      assert.ok(result.ok);
      const binding = analyzeCardBindings({ identity: "prohibition", source: { name: "Prohibition fixture", layout: "normal" },
        faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(binding.fullyBound, false);
      assert.ok(binding.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"), text);
    }
  });
});
