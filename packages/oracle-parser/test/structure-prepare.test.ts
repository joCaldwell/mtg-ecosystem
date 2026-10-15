import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Preparation cards, CR 722.3a-c (prepared is a designation of a permanent):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Wordings from Adventurous Eater, Paradox Shaper, Infinite Coursework,
// Biblioplex Tomekeeper, Hexhaven Dueling Arena and Codie, Ravenous Codex.
const CREATURE = "Creature — Human Wizard";
const ability = (text: string, typeLine = CREATURE) => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};
const thisCreature = { ref: "this", noun: "creature" };

describe("preparation cards (CR 722)", () => {
  it("represents entering prepared as an entry modification, not an instruction", () => {
    assert.deepEqual(ability("This creature enters prepared."),
      { kind: "static", effect: { static: "enters-prepared", what: thisCreature } });
    assert.deepEqual(ability("This creature enters tapped."),
      { kind: "static", effect: { static: "enters-tapped", what: thisCreature } });
  });

  it("represents becoming prepared and unprepared as distinct effects", () => {
    assert.deepEqual(ability("Whenever this creature attacks, it becomes prepared."), {
      kind: "triggered", trigger: { trigger: "attacks", what: thisCreature },
      effects: [{ steps: [{ effect: "prepare", what: { ref: "it" } }] }],
    });
    assert.deepEqual(ability("When this Aura enters, tap enchanted creature. It becomes unprepared.", "Enchantment — Aura"), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "aura" } },
      effects: [
        { steps: [{ effect: "tap", what: { ref: "enchanted", noun: "creature" } }] },
        { steps: [{ effect: "unprepare", what: { ref: "it" } }] },
      ],
    });
    assert.deepEqual(ability("{W}{U}{B}{R}{G}, {T}: Each creature you control becomes prepared."), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["W", "U", "B", "R", "G"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "prepare", what: { ref: "each", filter: { types: ["creature"], control: "you" } } }] }],
    });
  });

  it("keeps the target of an activated prepare effect and its restriction", () => {
    assert.deepEqual(ability("{4}, {T}: Target creature becomes prepared. Activate only as a sorcery.", "Land"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "prepare",
        what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } } }] }],
      restriction: { restriction: "sorcery" },
    });
  });

  it("tests the prepared designation in an intervening-if clause", () => {
    assert.deepEqual(ability("At the beginning of your upkeep, if this creature isn't prepared, it becomes prepared."), {
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      condition: { condition: "not", of: { condition: "status", what: thisCreature, status: "prepared" } },
      effects: [{ steps: [{ effect: "prepare", what: { ref: "it" } }] }],
    });
    assert.deepEqual(ability("At the beginning of your upkeep, if this creature is prepared, it becomes unprepared."), {
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      condition: { condition: "status", what: thisCreature, status: "prepared" },
      effects: [{ steps: [{ effect: "unprepare", what: { ref: "it" } }] }],
    });
  });

  it("rejects unsupported riders instead of dropping them", () => {
    assert.equal(ability("This creature enters prepared unless you control a Forest."), null);
    assert.equal(ability("Whenever this creature attacks, it becomes prepared until end of turn."), null);
    assert.equal(ability("Whenever this creature attacks, it becomes prepared tapped."), null);
    // "prepared spell" is not defined by CR 722; it must not pass as a filter.
    assert.equal(ability("Whenever you cast a prepared spell, copy it. You may choose new targets for the copy."), null);
  });
});
