import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { lex } from "../src/lexer.ts";
import { normalizeOracleText } from "../src/normalize.ts";
import { parseOracleText } from "../src/parser/index.ts";

// CR 201.5c: text may refer to its card by a shortened version of its name.
// A legendary character whose name has an epithet and no comma is called by
// the part before " the " / " of " (Bontu the Glorified, Crovax the Cursed,
// Éomer of the Riddermark, Sorin of House Markov, Rosie Cotton of South Lane).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const LEGEND = "Legendary Creature — God";
const normalize = (text: string, name: string, typeLine: string | undefined = LEGEND) =>
  normalizeOracleText(text, name, undefined, typeLine);

describe("short names of legendary characters without a comma", () => {
  it("replaces the part of the name before the epithet", () => {
    assert.deepEqual(normalize("Bontu can't attack or block unless a creature died under your control this turn.", "Bontu the Glorified"),
      ["~ can't attack or block unless a creature died under your control this turn."]);
    assert.deepEqual(normalize("Crovax enters with four +1/+1 counters on it.", "Crovax the Cursed"),
      ["~ enters with four +1/+1 counters on it."]);
    assert.deepEqual(normalize("Whenever Éomer attacks, draw a card.", "Éomer of the Riddermark"), ["Whenever ~ attacks, draw a card."]);
    assert.deepEqual(normalize("Sacrifice a creature you control other than Rosie Cotton.", "Rosie Cotton of South Lane"),
      ["Sacrifice a creature you control other than ~."]);
    assert.deepEqual(normalize("When Bontu the Glorified dies, draw a card. Bontu's power is 4.", "Bontu the Glorified"),
      ["When ~ dies, draw a card. ~'s power is 4."]);
  });

  it("gives the short and the full name the same reference", () => {
    const parse = (text: string) => {
      const result = parseOracleText(text, "Crovax the Cursed", { typeLine: "Legendary Creature — Vampire Noble" });
      assert.ok(result.ok, JSON.stringify(result.lines));
      return JSON.parse(JSON.stringify(result.abilities));
    };
    const expected = [{ kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] }] }];
    assert.deepEqual(parse("Whenever Crovax attacks, draw a card."), expected);
    assert.deepEqual(parse("Whenever Crovax the Cursed attacks, draw a card."), expected);
  });

  it("requires a legendary creature or planeswalker and an epithet connector", () => {
    // Not legendary, not a character, or no type line: the words stay.
    assert.deepEqual(normalize("Bontu can't block.", "Bontu the Glorified", "Creature — God"), ["Bontu can't block."]);
    assert.deepEqual(normalize("Heart can't block.", "Heart of Kiran", "Legendary Artifact — Vehicle"), ["Heart can't block."]);
    assert.deepEqual(normalizeOracleText("Bontu can't block.", "Bontu the Glorified"), ["Bontu can't block."]);
    // A leading word that is one of the card's own subtypes is never a short name (Sliver Queen).
    assert.deepEqual(normalize("Sliver creatures you control get +1/+1.", "Sliver Queen", "Legendary Creature — Sliver"),
      ["Sliver creatures you control get +1/+1."]);
    // Without an epithet, the first word is a short name only as a trigger subject (CR 201.5c, Edgar Markov).
    assert.deepEqual(normalize("Whenever Edgar attacks, draw a card.", "Edgar Markov", "Legendary Creature — Vampire Knight"),
      ["Whenever ~ attacks, draw a card."]);
    assert.deepEqual(normalize("Edgar can't block.", "Edgar Markov", "Legendary Creature — Vampire Knight"),
      ["Edgar can't block."]);
    // A name beginning with its article has no one-word short form.
    assert.deepEqual(normalize("The creature can't block.", "The Master of Keys", "Legendary Enchantment Creature — Horror"),
      ["The creature can't block."]);
  });

  it("leaves type-like and literal uses of the short name alone", () => {
    assert.deepEqual(normalize("Whenever another Bontu creature you control dies, draw a card.", "Bontu the Glorified"),
      ["Whenever another Bontu creature you control dies, draw a card."]);
    // A short flavor label before a dash is printed text (Magnus the Red).
    assert.deepEqual(normalize("Blade of Magnus — Whenever Magnus the Red deals combat damage to a player, draw a card.", "Magnus the Red", "Legendary Creature — Demon Primarch"),
      ["Blade of Magnus — Whenever ~ deals combat damage to a player, draw a card."]);
  });

  it("lexes letters outside the basic Latin range instead of rejecting the line", () => {
    assert.deepEqual(lex("Óin Sevā Éomer").map(token => token.kind === "word" ? token.raw : null), ["Óin", "Sevā", "Éomer"]);
    assert.deepEqual(lex("Éomer's").map(token => token.kind === "word" ? [token.value, token.possessive] : null), [["éomer", true]]);
  });
});
