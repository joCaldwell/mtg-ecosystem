import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 302.6, 702.10c
// (haste and {T}/{Q} abilities), 606.3 (loyalty abilities once per turn),
// 609.4, 611.2a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Artifact"): ParsedLine {
  const result = parseOracleText(text, "Urza, Planeswalker", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const you = { player: "you" } as const;
const yourPlaneswalkers = { ref: "each", filter: { types: ["planeswalker"], control: "you" } } as const;

describe("activating abilities as though the source had haste (CR 302.6, 702.10c)", () => {
  it("is a rule about activation, not a haste grant", () => {
    // Thousand-Year Elixir; Dynaheir, Invoker Adept.
    assert.deepEqual(ability("You may activate abilities of creatures you control as though those creatures had haste."), {
      kind: "static", effect: { static: "activation-as-though-haste", who: you, sources: { ref: "each", filter: { types: ["creature"], control: "you" } } },
    });
    assert.deepEqual(ability("You may activate abilities of other creatures you control as though those creatures had haste.", "Legendary Creature — Human Wizard"), {
      kind: "static", effect: { static: "activation-as-though-haste", who: you,
        sources: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } } },
    });
  });

  it("rejects other pretenses, selections and number disagreement", () => {
    fails("You may activate abilities of creatures you control as though those creatures had flash.");
    fails("You may activate abilities of target creature as though it had haste.");
    fails("You may activate abilities of creatures you control as though it had haste.");
    fails("You may activate abilities of creatures you control as though those creatures had haste and vigilance.");
  });
});

describe("activating loyalty abilities twice in a turn (CR 606.3)", () => {
  it("is static for 'each turn'", () => {
    // Oath of Teferi; Urza, Planeswalker.
    assert.deepEqual(ability("You may activate the loyalty abilities of planeswalkers you control twice each turn rather than only once.", "Legendary Enchantment"), {
      kind: "static", effect: { static: "loyalty-activation-limit", who: you, sources: yourPlaneswalkers, maximum: 2 },
    });
    assert.deepEqual(ability("You may activate the loyalty abilities of Urza, Planeswalker twice each turn rather than only once.", "Legendary Planeswalker — Urza"), {
      kind: "static", effect: { static: "loyalty-activation-limit", who: you, sources: { ref: "self" }, maximum: 2 },
    });
  });

  it("is a lasting rule with its duration for 'this turn', never an optional action", () => {
    // Urza Assembles the Titans (chapter III); Kaito, Dancing Shadow.
    const step = { effect: "loyalty-activation-limit", who: you, sources: yourPlaneswalkers, maximum: 2, duration: { duration: "this-turn" } };
    assert.deepEqual(ability("III — You may activate the loyalty abilities of planeswalkers you control twice this turn rather than only once.", "Enchantment — Saga"),
      { kind: "chapter", chapters: [3], effects: [{ steps: [step] }] });
    const kaito = ability("Whenever one or more creatures you control deal combat damage to a player, you may return one of them to its owner's hand. If you do, you may activate loyalty abilities of Urza, Planeswalker twice this turn rather than only once.", "Legendary Planeswalker — Kaito");
    assert.deepEqual(kaito.kind === "triggered" && kaito.effects[1], {
      condition: { condition: "you-do" }, conditionKind: "if",
      steps: [{ ...step, sources: { ref: "self" } }],
    });
  });

  it("rejects other counts and incomplete wordings", () => {
    fails("You may activate the loyalty abilities of planeswalkers you control three times each turn rather than only once.");
    fails("You may activate the loyalty abilities of planeswalkers you control twice each turn.");
    fails("You may activate the loyalty abilities of target planeswalker twice each turn rather than only once.");
    // The Chain Veil's per-permanent "as though none … have been activated" is a different rule.
    fails("{4}, {T}: For each planeswalker you control, you may activate one of its loyalty abilities once this turn as though none of its loyalty abilities have been activated this turn.");
  });
});
