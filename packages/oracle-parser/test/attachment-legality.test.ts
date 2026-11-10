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

const affected = { ref: "replacement-event-object", role: "affected" } as const;

describe("shuffling the replaced object into its owner's library (CR 614.1a, 701.24c)", () => {
  it("binds both the reveal and the shuffle to the object that would move (Progenitus)", () => {
    assert.deepEqual(effect("If ~ would be put into a graveyard from anywhere, reveal ~ and shuffle it into its owner's library instead.",
      "Legendary Creature — Hydra Avatar", "Progenitus"), {
      static: "replacement", replacement: {
        event: { event: "move-zone", what: { ref: "self" }, to: { zone: "graveyard", owner: "any" }, from: "anywhere" },
        occurrence: "each",
        replace: { replacement: "effects", effects: [{ steps: [
          { effect: "reveal", who: { player: "you" }, what: affected },
          { effect: "shuffle-into-library", who: { player: "you" }, what: [affected], library: { zone: "library", owner: "its-owner" } },
        ] }] },
      } });
  });
  it("does not bind an object other than the watched one", () => {
    rejects("If ~ would be put into a graveyard from anywhere, shuffle target card from a graveyard into its owner's library instead.",
      "Legendary Creature — Hydra Avatar", "Progenitus");
  });
});

describe("attachment legality (CR 301.5, 303.4c, 701.3b, 704.5n)", () => {
  const equipment = "Artifact — Equipment";
  it("limits what the source can be attached to (Gate Smasher, Konda's Banner)", () => {
    assert.deepEqual(effect("This Equipment can be attached only to a creature with toughness 4 or greater.", equipment), {
      static: "attachment-limit", what: { ref: "this", noun: "equipment" },
      to: { types: ["creature"], properties: [{ property: "toughness", comparison: { op: "ge", value: { amount: "fixed", value: 4 } } }] } });
    assert.deepEqual(effect("Konda's Banner can be attached only to a legendary creature.", "Legendary Artifact — Equipment", "Konda's Banner"), {
      static: "attachment-limit", what: { ref: "self" }, to: { supertypes: ["legendary"], types: ["creature"] } });
  });
  it("keeps a permanent that can't be equipped or enchanted (Goblin Brawler)", () => {
    assert.deepEqual(effect("This creature can't be equipped.", "Creature — Goblin Warrior"), {
      static: "cant", what: { ref: "this", noun: "creature" }, action: "be-equipped" });
    assert.deepEqual(effect("Creatures you control can't be enchanted.", "Enchantment"), {
      static: "cant", what: { ref: "each", filter: { types: ["creature"], control: "you" } }, action: "be-enchanted" });
  });
  it("rejects targets, plural limits and unrepresented Aura sources", () => {
    rejects("This Equipment can be attached only to target creature.", equipment);
    rejects("This Equipment can be attached only to creatures.", equipment);
    rejects("Enchanted land has indestructible and can't be enchanted by other Auras.", "Enchantment — Aura");
  });
});

describe("actions spells and abilities can't cause (CR 101.2, 609.1)", () => {
  it("keeps the causing objects, the player, the actions and their objects (Sigarda, The Master)", () => {
    assert.deepEqual(effect("Spells and abilities your opponents control can't cause you to sacrifice permanents.", "Legendary Creature — Angel"), {
      static: "prohibit", prohibition: { prohibition: "caused-action", who: { player: "you" }, actions: ["sacrifice"],
        what: { ref: "each", filter: { cls: "permanent" } },
        by: { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }], control: "opponent" } } });
    assert.deepEqual(effect("Triggered abilities you control can't cause you to sacrifice or exile creature tokens you control.", "Legendary Creature — Time Lord Rogue"), {
      static: "prohibit", prohibition: { prohibition: "caused-action", who: { player: "you" }, actions: ["sacrifice", "exile"],
        what: { ref: "each", filter: { types: ["creature"], cls: "token", control: "you" } },
        by: { cls: "ability", abilityKind: "triggered", control: "you" } } });
  });
  it("needs spells or abilities as the cause and a general object description", () => {
    rejects("Creatures your opponents control can't cause you to sacrifice permanents.", "Creature — Elf Shaman");
    rejects("Spells your opponents control can't cause you to sacrifice target creature.", "Creature — Elf Shaman");
    rejects("Spells your opponents control can't cause you to discard cards.", "Creature — Elf Shaman");
  });
});
