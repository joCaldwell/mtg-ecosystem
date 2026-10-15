import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Flavor words (CR 207.2c–d). Wordings are
// taken from the cached Scryfall corpus; card names are noted per test.
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

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const gainOne = [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: fixed(1) }] }];
const attacks = { trigger: "attacks", what: { ref: "this", noun: "creature" } };

describe("flavor words on chapter abilities (CR 207.2d, 714.2)", () => {
  const saga = "Enchantment Creature — Saga Bird";
  it("keeps the label next to the chapter numbers (Summon: Anima)", () => {
    assert.deepEqual(only("IV — Oblivion — Each opponent loses 3 life.", saga), {
      kind: "chapter", chapters: [4], abilityWord: "Oblivion",
      effects: [{ steps: [{ effect: "lose-life", who: { player: "each-opponent" }, amount: fixed(3) }] }],
    });
  });
  it("keeps an exclamation mark in the label (Summon: Choco/Mog)", () => {
    assert.deepEqual(only("I, II, III, IV — Stampede! — Other creatures you control get +1/+0 until end of turn.", saga), {
      kind: "chapter", chapters: [1, 2, 3, 4], abilityWord: "Stampede!",
      effects: [{ steps: [{ effect: "pump", what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } },
        power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(0) }, duration: { duration: "end-of-turn" } }] }],
    });
  });
  it("leaves an unlabeled chapter without a label", () => {
    assert.deepEqual(only("I — You gain 1 life.", saga), { kind: "chapter", chapters: [1], effects: gainOne });
  });
  it("does not take a keyword or rules text for a chapter label", () => {
    rejects("I — Flying — You gain 1 life.", saga);
    rejects("I — Kicker {2} — You gain 1 life.", saga);
    rejects("I — You gain 1 life. — You gain 1 life.", saga);
  });
});

describe("flavor words with punctuation, digits, or more than four words (CR 207.2d)", () => {
  const creature = "Creature — Human";
  it("keeps the printed label (Barret, Ace, Jumbo Cactuar, Aloy, Vislor Turlough)", () => {
    for (const label of ["Avalanche!", "Nitro-9", "10,000 Needles", "In You, All Things Are Possible", "Deal with the Black Guardian"]) {
      assert.deepEqual(only(`${label} — Whenever this creature attacks, you gain 1 life.`, creature),
        { kind: "triggered", abilityWord: label, trigger: attacks, effects: gainOne }, label);
    }
  });
  it("labels a static ability (Kraven, Proud Predator)", () => {
    const ability = only("Top of the Food Chain — Creatures you control have haste.", creature);
    assert.equal(ability.kind, "static");
    if (ability.kind === "static") assert.equal(ability.abilityWord, "Top of the Food Chain");
  });
  it("does not let such a label hide a keyword line or a parameter", () => {
    rejects("Stage 2 — Evolve", creature);
    rejects("Stage 2! — Flying", creature);
    rejects("Avalanche! —", creature);
  });
  it("does not read rules text in front of a dash as a label", () => {
    rejects("{T} — Whenever this creature attacks, you gain 1 life.", creature);
    rejects("You gain 1 life. Avalanche — Whenever this creature attacks, you gain 1 life.", creature);
    rejects("! — Whenever this creature attacks, you gain 1 life.", creature);
  });
});
