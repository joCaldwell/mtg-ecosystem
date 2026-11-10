import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Square brackets in Oracle text. CR 107.7 / 701.71a: a loyalty symbol inside
// rules text is written "[+N]", "[-N]" or "[0]" (Way of the Wildspeaker,
// Elspeth's Talent, Ichormoon Gauntlet).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const ability = (text: string, typeLine = "Enchantment") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};
const n = (value: number) => ({ amount: "fixed", value });
const draw = (value: number) => ({ steps: [{ effect: "draw", who: { player: "you" }, amount: n(value) }] });
const planeswalkers = { ref: "each", filter: { types: ["planeswalker"], control: "you" } };

describe("square brackets", () => {
  it("lexes brackets as punctuation instead of rejecting the line", () => {
    assert.deepEqual(lex("[-3]:").map(token => token.kind === "punct" ? token.value : token.kind === "number" ? token.value : null),
      ["[", "-", 3, "]", ":"]);
  });

  it("reads a bracketed loyalty symbol in a quoted ability as a loyalty cost", () => {
    assert.deepEqual(ability('Planeswalkers you control have "[-4]: Draw two cards."'), {
      kind: "static", effect: { static: "modify", what: planeswalkers, modifiers: [{ modifier: "abilities", abilities: [
        { gained: "quoted", ability: { kind: "loyalty", cost: { sign: -1, amount: 4 }, effects: [draw(2)] } }] }] },
    });
    assert.deepEqual(ability('Planeswalkers you control have "[0]: Draw a card" and "[+1]: Draw two cards."'), {
      kind: "static", effect: { static: "modify", what: planeswalkers, modifiers: [{ modifier: "abilities", abilities: [
        { gained: "quoted", ability: { kind: "loyalty", cost: { sign: 0, amount: 0 }, effects: [draw(1)] } },
        { gained: "quoted", ability: { kind: "loyalty", cost: { sign: 1, amount: 1 }, effects: [draw(2)] } }] }] },
    });
    assert.deepEqual(ability('Enchanted planeswalker has "[-X]: Draw X cards."', "Enchantment — Aura"), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "planeswalker" }, modifiers: [{ modifier: "abilities", abilities: [
        { gained: "quoted", ability: { kind: "loyalty", cost: { sign: -1, amount: "x" },
          effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }] }] } }] }] },
    });
  });

  it("gives the bracketed and the printed symbol the same node", () => {
    const printed = parseOracleText("-4: Draw two cards.", "Test Walker", { typeLine: "Legendary Planeswalker — Test" });
    const bracketed = parseOracleText("[-4]: Draw two cards.", "Test Walker", { typeLine: "Legendary Planeswalker — Test" });
    assert.ok(printed.ok && bracketed.ok);
    assert.deepEqual(bracketed.abilities, printed.abilities);
  });

  it("rejects malformed or non-cost brackets", () => {
    assert.equal(ability('Planeswalkers you control have "[-4: Draw two cards."'), null);
    assert.equal(ability('Planeswalkers you control have "[4]: Draw two cards."'), null);
    assert.equal(ability('Planeswalkers you control have "[-4] Draw two cards."'), null);
    // Bracketed words outside a cleave spell have no defined meaning.
    assert.equal(ability("Destroy target [attacking] creature.", "Instant"), null);
    assert.equal(ability("[1] Put a +1/+1 counter on target creature.", "Sorcery"), null);
  });
});
