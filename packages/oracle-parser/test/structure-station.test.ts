import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Station cards, CR 702.184 and 721.2-721.4: a station symbol and its
// striation are one static ability gated on charge counters.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Oracle text of Dawnsire, Sunstar Dreadnought; Land Station; Evendo, Waking
// Haven; and the d20 table of Diviner's Portent from the frozen corpus.
const REMINDER = "Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery.";
const SPACECRAFT = "Artifact — Spacecraft";
const parse = (text: string, typeLine = SPACECRAFT) => {
  const result = parseOracleText(text, "Test Vessel", { typeLine });
  return JSON.parse(JSON.stringify(result)) as { ok: boolean; lines: { text: string; ok: boolean; ability?: unknown; error?: string }[] };
};
const station = { kind: "keywords", keywords: [{ keyword: "station" }] };
const flying = { kind: "keywords", keywords: [{ keyword: "flying" }] };

describe("station symbols (CR 721)", () => {
  it("gates each striation on its charge-counter threshold and marks the P/T box striation", () => {
    const result = parse(`Station (${REMINDER} It's an artifact creature at 20+.)\n` +
      "10+ | Whenever you attack, draw a card.\n20+ | Flying");
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines.map(line => line.text), ["Station", "10+ | Whenever you attack, draw a card.", "20+ | Flying"]);
    assert.deepEqual(result.lines.map(line => line.ability), [
      station,
      { kind: "static", effect: { static: "station-band", threshold: 10, abilities: [{
        kind: "triggered", trigger: { trigger: "you-attack" },
        effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] }],
      }] } },
      { kind: "static", effect: { static: "station-band", threshold: 20, abilities: [flying], creature: { basePT: "printed-box" } } },
    ]);
  });

  it("keeps later lines inside the last striation and earlier lines unconditional", () => {
    const result = parse("Flash\n" + `Station (${REMINDER} It's an artifact creature at 10+.)\n` +
      "10+ | Flying\nThis creature gets +1/+1 for each basic land you control.");
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines.map(line => line.text),
      ["Flash", "Station", "10+ | Flying\nThis creature gets +1/+1 for each basic land you control."]);
    assert.deepEqual(result.lines[0].ability, { kind: "keywords", keywords: [{ keyword: "flash" }] });
    assert.deepEqual(result.lines[2].ability, { kind: "static", effect: { static: "station-band", threshold: 10, abilities: [
      flying,
      { kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{
        modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 1 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 1 } },
        forEach: { supertypes: ["basic"], types: ["land"], control: "you" } }] } },
    ], creature: { basePT: "printed-box" } } });
  });

  it("has no creature marker when the reminder names no power/toughness striation (Planets)", () => {
    const result = parse("Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n" +
      "12+ | {G}, {T}: Add {G}{G}.", "Land — Planet");
    assert.equal(result.ok, true);
    assert.deepEqual(result.lines[1].ability, { kind: "static", effect: { static: "station-band", threshold: 12, abilities: [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["G"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["G", "G"] } }] }],
    }] } });
  });

  it("fails when the power/toughness striation cannot be established from the text", () => {
    // No reminder: Oracle text alone cannot say whether a P/T box exists.
    assert.equal(parse("Station\n8+ | Flying").ok, false);
    // The reminder names a threshold that is not a printed symbol.
    assert.equal(parse(`Station (${REMINDER} It's an artifact creature at 9+.)\n8+ | Flying`).ok, false);
    // Symbols must increase, and a card has at most two (CR 721.1).
    assert.equal(parse(`Station (${REMINDER} It's an artifact creature at 8+.)\n8+ | Flying\n3+ | Haste`).ok, false);
    assert.equal(parse(`Station (${REMINDER} It's an artifact creature at 8+.)\n1+ | Haste\n3+ | Trample\n8+ | Flying`).ok, false);
  });

  it("fails the whole striation when one of its abilities is unsupported or not a permanent ability", () => {
    const unsupported = parse(`Station (${REMINDER} It's an artifact creature at 8+.)\n8+ | Flying\nFrobnicate the gizmo.`);
    assert.equal(unsupported.ok, false);
    assert.deepEqual(unsupported.lines.map(line => [line.text, line.ok]), [["Station", true], ["8+ | Flying\nFrobnicate the gizmo.", false]]);
    assert.equal(parse(`Station (${REMINDER} It's an artifact creature at 8+.)\n8+ | Draw a card.`).ok, false);
  });

  it("does not treat a dice results row as a station symbol", () => {
    const result = parse("Roll a d20.\n1—14 | Draw a card.\n15+ | Draw two cards.", "Instant");
    assert.equal(result.ok, true);
    assert.equal(result.lines.length, 1);
    assert.equal(JSON.stringify(result.lines[0].ability).includes("station-band"), false);
    assert.equal(JSON.stringify(result.lines[0].ability).includes('"effect":"roll-dice"'), true);
  });
});
