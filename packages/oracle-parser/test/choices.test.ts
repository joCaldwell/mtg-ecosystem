import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ValueChoice } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseValueChoice } from "../src/parser/choices.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Actual Oracle examples: Roots of Life 29ec9b49-fbe5-49ac-8ad6-7159cb88bc66,
// Cloud Key 2a838818-d590-4374-9a63-d9e6381a0f0d, Dawn-Blessed Pennant
// 311eef28-21e9-44fa-96df-d281f0a9ba34. Outpost Siege's "Dragons" is an
// anchor word, not the creature subtype Dragon (CR 614.12b, 207.2c, 205.3e).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
function choice(text: string): ValueChoice {
  const c = new Cursor(lex(text));
  const result = parseValueChoice(c);
  assert.ok(result, c.errorMessage());
  assert.equal(c.done(), true);
  return result;
}
function subtypeOptions(...values: string[]): ValueChoice {
  return { category: "type", options: values.map(value => ({ type: "subtype", value })) };
}
function cardTypeOptions(...values: string[]): ValueChoice {
  return { category: "type", options: values.map(value => ({ type: "card-type", value })) };
}

describe("literal game-value choices", () => {
  it("keeps unrestricted value categories separate from object selection", () => {
    for (const [text, category] of [["a color", "color"], ["a creature type", "creature-type"], ["a card type", "card-type"],
      ["a basic land type", "basic-land-type"], ["a land type", "land-type"], ["a card name", "card-name"], ["a number", "number"]])
      assert.deepEqual(choice(text), { category });
    const c = new Cursor(lex("a Forest"));
    assert.equal(parseValueChoice(c), null);
    assert.equal(c.pos, 0);
  });
  it("retains every exact allowed type value with its namespace", () => {
    assert.deepEqual(choice("Island or Swamp"), subtypeOptions("Island", "Swamp"));
    assert.deepEqual(choice("artifact, creature, enchantment, instant, or sorcery"), cardTypeOptions("artifact", "creature", "enchantment", "instant", "sorcery"));
    assert.deepEqual(choice("Elemental, Elf, Faerie, Giant, Goblin, Kithkin, Merfolk, or Treefolk"), subtypeOptions("Elemental", "Elf", "Faerie", "Giant", "Goblin", "Kithkin", "Merfolk", "Treefolk"));
    assert.deepEqual(choice("Time Lord or Assembly-Worker"), subtypeOptions("Time Lord", "Assembly-Worker"));
    assert.deepEqual(choice("Urza's or Desert"), subtypeOptions("Urza's", "Desert"));
    assert.deepEqual(choice("conspiracy or phenomenon"), cardTypeOptions("conspiracy", "phenomenon"));
  });
  it("rejects malformed, plural, mixed-namespace, and qualified options with rewind", () => {
    for (const text of ["Island", "Island, Swamp", "Island and Swamp", "Island or", "Island or invented", "Island or Island", "Island or an Island",
      "artifact or Goblin", "artifact or artifacts", "Elf or Elves", "Khans or Dragons", "Dragons or Wolves", "tribal or creature", "other creature type",
      "white Citizen, blue Camarid, black Thrull, red Goblin, or green Saproling"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseValueChoice(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });
  it("leaves a later instruction outside an explicitly complete option list", () => {
    const c = new Cursor(lex("Island or Swamp, then draw a card"));
    assert.deepEqual(parseValueChoice(c), subtypeOptions("Island", "Swamp"));
    assert.equal(c.isPunct(","), true);
  });
});

describe("choose-value integration preserves resolution and entry timing", () => {
  it("chooses a single subtype value as Roots of Life enters", () => {
    const result = parseOracleText("As this enchantment enters, choose Island or Swamp.\nWhenever a land of the chosen type an opponent controls becomes tapped, you gain 1 life.", "Roots of Life");
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [
      { kind: "static", abilityWord: undefined, effect: { static: "as-enters", what: { ref: "this", noun: "enchantment" }, effects: [
        { steps: [{ effect: "choose-value", who: { player: "you" }, ...subtypeOptions("Island", "Swamp") }] },
      ] } },
      { kind: "triggered", abilityWord: undefined, condition: undefined,
        trigger: { trigger: "becomes-tapped", what: { ref: "filter", filter: { types: ["land"], chosenType: { choice: "chosen-type" }, control: "opponent" }, count: fixed(1) } },
        effects: [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: fixed(1) }] }] },
    ]);
  });
  it("preserves Cloud Key's complete card-type option list and later filter", () => {
    const result = parseOracleText("As this artifact enters, choose artifact, creature, enchantment, instant, or sorcery.\nSpells you cast of the chosen type cost {1} less to cast.", "Cloud Key");
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [
      { kind: "static", abilityWord: undefined, effect: { static: "as-enters", what: { ref: "this", noun: "artifact" }, effects: [{ steps: [
        { effect: "choose-value", who: { player: "you" }, ...cardTypeOptions("artifact", "creature", "enchantment", "instant", "sorcery") },
      ] }] } },
      { kind: "static", abilityWord: undefined, effect: { static: "cost-modification", scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter: { cls: "spell", chosenType: { choice: "chosen-type" } } } }, change: { direction: "reduce", symbols: ["1"] } } },
    ]);
  });
  it("preserves an explicit object choice, a target declaration, and later actions", () => {
    const result = parseOracleText("Choose a Forest. Choose target creature. Choose Island or Swamp, then draw a card.");
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [
      { steps: [{ effect: "choose-object", who: { player: "you" }, what: { ref: "filter", filter: { subtypes: ["Forest"] }, count: fixed(1) } }] },
      { steps: [{ effect: "declare-target", what: { ref: "target", filter: { types: ["creature"] }, count: fixed(1) } }] },
      { steps: [{ effect: "choose-value", who: { player: "you" }, ...subtypeOptions("Island", "Swamp") }, { effect: "draw", who: { player: "you" }, amount: fixed(1) }] },
    ] }]);
  });
  it("does not fall back to an implicit each-object selection for unsupported value choices", () => {
    for (const text of ["Choose Dragons or Wolves.", "Choose Island, Swamp.", "Choose creature and land.", "Choose Island or an Island.",
      "Choose Island or Swamp with flying.", "Choose white Citizen, blue Camarid, black Thrull, red Goblin, or green Saproling."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});
