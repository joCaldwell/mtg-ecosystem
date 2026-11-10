import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings are from the cached
// Scryfall corpus; rules are the Comprehensive Rules of 2026-09-25.
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

const indestructible = [{ gained: "keyword", keyword: { keyword: "indestructible" } }];

describe("bare plural unions with a subtype whose plural is unchanged", () => {
  // Timber Protector; CR 608.2c: the trailing qualifier restricts both nouns.
  it("reads 'Treefolk and Forests you control' as one union", () => {
    assert.deepEqual(only("Other Treefolk and Forests you control have indestructible.", "Creature — Treefolk Warrior"), {
      kind: "static", effect: { static: "modify",
        what: { ref: "each", filter: { other: true, allOf: [{ anyOf: [{ subtypes: ["Treefolk"] }, { subtypes: ["Forest"] }] }], control: "you" } },
        modifiers: [{ modifier: "abilities", abilities: indestructible }] } });
  });

  // Sapling Nursery.
  it("reads the resolving form with the invariant noun first", () => {
    const ability = only("{1}{G}, Exile this enchantment: Treefolk and Forests you control gain indestructible until end of turn.", "Enchantment");
    assert.deepEqual(ability.kind === "activated" && ability.effects, [{ steps: [{ effect: "gain-abilities",
      what: { ref: "each", filter: { allOf: [{ anyOf: [{ subtypes: ["Treefolk"] }, { subtypes: ["Forest"] }] }], control: "you" } },
      abilities: indestructible, duration: { duration: "end-of-turn" } }] }]);
  });

  it("needs one visibly plural noun and no singular determiner", () => {
    rejects("Treefolk and Merfolk you control gain indestructible until end of turn.", "Instant");
    rejects("Sacrifice a Treefolk and Forests you control.", "Instant");
  });
});

describe("a subject list that gets +N/+N and gains abilities", () => {
  const plus = (value: number) => ({ sign: 1, amount: { amount: "fixed", value } });
  const endOfTurn = { duration: "end-of-turn" };
  const sharing = { ref: "each", filter: { other: true, types: ["creature"],
    characteristicRelations: [{ relation: "shared-value", characteristic: "creature-type", with: { operand: "object", what: { ref: "it" } } }] } };
  const firstStrike = [{ gained: "keyword", keyword: { keyword: "first strike" } }];
  // Crown of Fury; CR 608.2c: one instruction, both verbs apply to every subject.
  it("pumps and grants every subject at once with the shared duration", () => {
    const ability = only("Sacrifice this Aura: Enchanted creature and other creatures that share a creature type with it get +1/+0 and gain first strike until end of turn.", "Enchantment — Aura");
    assert.deepEqual(ability.kind === "activated" && ability.effects, [{ steps: [{ effect: "simultaneous", steps: [
      { effect: "pump", what: { ref: "enchanted", noun: "creature" }, power: plus(1), toughness: plus(0), duration: endOfTurn },
      { effect: "pump", what: sharing, power: plus(1), toughness: plus(0), duration: endOfTurn },
      { effect: "gain-abilities", what: { ref: "enchanted", noun: "creature" }, abilities: firstStrike, duration: endOfTurn },
      { effect: "gain-abilities", what: sharing, abilities: firstStrike, duration: endOfTurn },
    ] }] }]);
  });

  it("still rejects overlapping subjects, players getting P/T and a shared fresh choice", () => {
    rejects("Target creature and target land get +1/+1 and gain haste until end of turn.", "Instant");
    rejects("You and creatures you control get +1/+1 and gain haste until end of turn.", "Instant");
    rejects("This creature and other creatures you control get +1/+1 and gain protection from the color of your choice until end of turn.", "Creature — Human");
  });
});

describe("paying an object's mana cost", () => {
  // Pendrell Flux; CR 202.1, 118.1.
  it("reads 'unless you pay its mana cost' inside a granted upkeep trigger", () => {
    assert.deepEqual(only("Enchanted creature has \"At the beginning of your upkeep, sacrifice this creature unless you pay its mana cost.\"", "Enchantment — Aura"), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted", ability: {
        kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
        effects: [{ steps: [{ effect: "sacrifice", who: { player: "you" }, what: { ref: "this", noun: "creature" } }],
          condition: { condition: "pays", who: { player: "you" }, cost: [{ cost: "mana-cost-of", of: { ref: "it" } }] }, conditionKind: "unless" }] } }] }] } });
  });

  // Back from the Brink: the same payment as an activation cost.
  it("reads it as an activation cost item", () => {
    const ability = only("Exile a creature card from your graveyard and pay its mana cost: Create a token that's a copy of that card. Activate only as a sorcery.", "Enchantment");
    assert.deepEqual(ability.kind === "activated" && ability.costs[1], { cost: "mana-cost-of", of: { ref: "it" } });
  });

  it("does not drop a printed adjustment of the mana cost", () => {
    rejects("Sacrifice it unless you pay its mana cost reduced by {2}.", "Instant");
    rejects("Exile a creature card from your graveyard and pay its mana cost plus {1}: Draw a card.", "Enchantment");
  });
});

describe("a token copy exception naming the token by its noun", () => {
  // Irenicus's Vile Duplication; CR 707.9b: the exception is part of the copy effect.
  it("reads 'except the token has flying' like 'except it has flying'", () => {
    const ability = only("Create a token that's a copy of target creature you control, except the token has flying and it isn't legendary.", "Sorcery");
    assert.deepEqual(ability.kind === "spell" && ability.effects, [{ steps: [{ effect: "create-token", count: { amount: "fixed", value: 1 },
      token: { copyOf: { ref: "target", filter: { types: ["creature"], control: "you" }, count: { amount: "fixed", value: 1 } },
        modifications: [{ modification: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] }, { modification: "not-legendary" }] } }] }]);
  });

  it("does not read another object's abilities as the token's", () => {
    rejects("Create a token that's a copy of target creature you control, except the creature has flying.", "Sorcery");
  });
});

describe("losing every ability of a parameterized keyword family", () => {
  const target = { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } };
  const lose = (keyword: string) => [{ steps: [{ effect: "lose-abilities", what: target,
    removal: { removal: "listed", abilities: [{ gained: "keyword", keyword: { keyword } }] }, duration: { duration: "end-of-turn" } }] }];
  // Hammerheim; CR 702.14a.
  it("reads 'all landwalk abilities' as the unparameterized landwalk keyword", () => {
    const ability = only("{T}: Target creature loses all landwalk abilities until end of turn.", "Land");
    assert.deepEqual(ability.kind === "activated" && ability.effects, lose("landwalk"));
  });
  // Shelkin Brownie; CR 702.22b.
  it("reads 'all \"bands with other\" abilities' the same way", () => {
    const ability = only("{T}: Target creature loses all \"bands with other\" abilities until end of turn.", "Creature — Ouphe");
    assert.deepEqual(ability.kind === "activated" && ability.effects, lose("bands with other"));
  });
  it("rejects other keyword families and an unquoted banding variant", () => {
    rejects("{T}: Target creature loses all flying abilities until end of turn.", "Land");
    rejects("{T}: Target creature loses all bands with other abilities until end of turn.", "Land");
  });
});
