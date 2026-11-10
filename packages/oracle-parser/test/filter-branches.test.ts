import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { ObjectFilter } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseFilter } from "../src/parser/refs.ts";

// Named Oracle wording checked against cached Scryfall oracle-cards.json.
// CR 608.2c: the complete instruction determines English modifier scope.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const legendary = { supertypes: ["legendary"], types: ["creature"] };
const manaOne = { types: ["creature"], properties: [{ property: "mana-value", comparison: { op: "eq", value: one } }] } as const;

function filter(text: string): ObjectFilter {
  const c = new Cursor(lex(text));
  const result = parseFilter(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.ok(c.done(), text);
  return result;
}

describe("branch-local filter restrictions", () => {
  it("keeps Isamaru and Yoshimaru's legendary and mana-value predicates alternative", () => {
    const text = "Whenever another legendary creature or creature with mana value one enters the battlefield under your control, put a +1/+1 counter on Isamaru and Yoshimaru.";
    const result = parseOracleText(text, "Isamaru and Yoshimaru");
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "enters", controller: { player: "you" }, what: {
        ref: "filter", count: one, filter: { other: true, anyOf: [legendary, manaOne] },
      } },
      effects: [{ steps: [{ effect: "put-counters", count: one, counter: { type: "+1/+1" }, on: { ref: "self" } }] }],
    }]);
  });

  it("preserves independent restrictions with repeated heads or articles", () => {
    assert.deepEqual(filter("legendary creature or creature with mana value one"), { anyOf: [legendary, manaOne] });
    assert.deepEqual(filter("legendary creature or a creature with mana value one"), { anyOf: [legendary, manaOne] });
    assert.deepEqual(filter("legendary creature, or creature with mana value one"), { anyOf: [legendary, manaOne] });
    assert.deepEqual(filter("nonlegendary creature or artifact creature"), { anyOf: [
      { nonSupertypes: ["legendary"], types: ["creature"] }, { types: ["artifact", "creature"] },
    ] });
    assert.deepEqual(filter("red creature or blue creature"), { anyOf: [
      { colors: ["red"], types: ["creature"] }, { colors: ["blue"], types: ["creature"] },
    ] });
  });

  it("preserves repeated noncreature restrictions in Haywire Mite's complete activation", () => {
    const result = parseOracleText("{G}, Sacrifice this creature: Exile target noncreature artifact or noncreature enchantment.", undefined, { includeSourceProvenance: true });
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "mana", symbols: ["G"] }, { cost: "sacrifice", what: { ref: "this", noun: "creature" } }],
      effects: [{ steps: [{ effect: "exile", what: { ref: "target", count: one, selectionId: "line:0:7", filter: { anyOf: [
        { nonTypes: ["creature"], types: ["artifact"] }, { nonTypes: ["creature"], types: ["enchantment"] },
      ] } } }] }],
    }]);
  });

  it("retains explicit branch-local zones and controllers", () => {
    assert.deepEqual(filter("creature cards in your graveyard or artifact cards in exile"), { anyOf: [
      { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } },
      { types: ["artifact"], cls: "card", zone: { zone: "exile", owner: undefined } },
    ] });
    assert.deepEqual(filter("creatures you control or artifacts an opponent controls"), { anyOf: [
      { types: ["creature"], control: "you" }, { types: ["artifact"], control: "opponent" },
    ] });
  });

  it("continues sharing a single class head and unambiguous prenominal color", () => {
    assert.deepEqual(filter("artifact or creature cards in your graveyard"), {
      allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }], cls: "card", zone: { zone: "graveyard", owner: "your" },
    });
    assert.deepEqual(filter("red instant or sorcery spells you cast"), {
      colors: ["red"], allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell", cast: { by: { player: "you" } },
    });
    assert.deepEqual(filter("Elf or Goblin creature cards"), {
      allOf: [{ anyOf: [{ subtypes: ["Elf"] }, { subtypes: ["Goblin"] }] }], types: ["creature"], cls: "card",
    });
  });

  it("rejects ambiguous shared scope and malformed alternatives with full rewind", () => {
    // A closing card-zone phrase covers every card alternative before it.
    assert.deepEqual(parseFilter(new Cursor(lex("creature card or artifact card in your graveyard"))), { anyOf: [
      { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } },
      { types: ["artifact"], cls: "card", zone: { zone: "graveyard", owner: "your" } }] });
    for (const text of [
      "legendary creature or creature with mana value one you control",
      "nonlegendary creature or artifact creature you control",
      "artifact or enchantment creature",
      "Elf or Goblin Warrior creature",
      "legendary creature or a",
      "legendary creature or nonimaginary creature",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseFilter(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    assert.equal(parseOracleText("Destroy target artifact or enchantment creature.").ok, false);
    assert.equal(parseOracleText("Destroy target nonlegendary creature or artifact creature you control.").ok, false);
  });
});
