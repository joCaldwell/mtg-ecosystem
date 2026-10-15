import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Chroma mana-symbol counts (CR 107.4e–f,
// 207.2c, 700.5a). Wordings are taken from the cached Scryfall corpus.
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

describe("mana symbols of one color in mana costs (chroma)", () => {
  const elemental = "Creature — Elemental";
  it("counts the symbols among permanents you control, not devotion (Primalcrux)", () => {
    assert.deepEqual(only("Chroma — This creature's power and toughness are each equal to the number of green mana symbols in the mana costs of permanents you control.", elemental), {
      kind: "static", abilityWord: "Chroma", effect: { static: "cda-pt", what: { ref: "this", noun: "creature" }, stat: "both",
        value: { amount: "mana-symbols", color: "green", in: { cls: "permanent", control: "you" } } },
    });
  });
  it("counts the symbols among cards in a graveyard (Umbra Stalker)", () => {
    assert.deepEqual(only("Chroma — ~'s power and toughness are each equal to the number of black mana symbols in the mana costs of cards in your graveyard.", elemental, "Umbra Stalker"), {
      kind: "static", abilityWord: "Chroma", effect: { static: "cda-pt", what: { ref: "self" }, stat: "both",
        value: { amount: "mana-symbols", color: "black", in: { cls: "card", zone: { zone: "graveyard", owner: "your" } } } },
    });
  });
  it("is an amount wherever the number is used (Outrage Shaman)", () => {
    assert.deepEqual(only("Chroma — When this creature enters, it deals damage to target creature equal to the number of red mana symbols in the mana costs of permanents you control.", elemental), {
      kind: "triggered", abilityWord: "Chroma", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "damage", source: { ref: "it" },
        amount: { amount: "mana-symbols", color: "red", in: { cls: "permanent", control: "you" } },
        to: { to: "object", ref: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } } } }] }],
    });
  });
  it("needs one color and a described set of objects", () => {
    rejects("This creature's power and toughness are each equal to the number of mana symbols in the mana costs of permanents you control.", elemental);
    rejects("This creature's power and toughness are each equal to the number of colored mana symbols in the mana costs of permanents you control.", elemental);
    rejects("This creature's power and toughness are each equal to the number of green mana symbols in the mana cost of permanents you control.", elemental);
  });
});
