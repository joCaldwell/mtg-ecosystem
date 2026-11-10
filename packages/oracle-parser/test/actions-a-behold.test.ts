import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// CR 701.4 (behold): https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value }) as const;

function parse(text: string, typeLine?: string): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function rejected(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, "Test", { typeLine }).ok, false, text);
}

describe("behold", () => {
  it("parses behold as an additional cost, alone, with exile, and as an alternative to mana", () => {
    assert.deepEqual(parse("As an additional cost to cast this spell, behold a Kithkin and exile it.", "Creature — Kithkin"),
      [{ kind: "additional-cost", costs: [{ cost: "behold", quality: { subtypes: ["Kithkin"] }, count: n(1), exile: true }] }]);
    assert.deepEqual(parse("As an additional cost to cast this spell, behold a Dragon or pay {1}.", "Instant"),
      [{ kind: "additional-cost", costs: [{ cost: "choice", options: [
        [{ cost: "behold", quality: { subtypes: ["Dragon"] }, count: n(1) }], [{ cost: "mana", symbols: ["1"] }]] }] }]);
    assert.deepEqual(parse("As an additional cost to cast this spell, you may behold a Gamma creature.", "Instant"),
      [{ kind: "additional-cost", optional: true, costs: [{ cost: "behold", quality: { subtypes: ["Gamma"], types: ["creature"] }, count: n(1) }] }]);
  });

  it("parses behold in a keyword cost with a quantity", () => {
    const [ability] = parse("Flashback—{1}{R}, Behold three Elementals.", "Sorcery");
    assert.deepEqual(ability, { kind: "keywords", keywords: [{ keyword: "flashback", cost: [
      { cost: "mana", symbols: ["1", "R"] }, { cost: "behold", quality: { subtypes: ["Elemental"] }, count: n(3) }] }] });
  });

  it("parses behold as an optional effect and as an entry choice", () => {
    const [ability] = parse("When ~ enters, you may behold a Dragon. If you do, create a Treasure token.", "Creature — Human");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps, [
      { effect: "behold", who: { player: "you" }, quality: { subtypes: ["Dragon"] }, count: n(1), optional: true }]);
    assert.deepEqual(ability.effects[1].condition, { condition: "you-do" });
    const [entry] = parse("As this land enters, you may behold a Jace. If you don't, this land enters tapped.", "Land");
    assert.deepEqual(entry, { kind: "static", effect: { static: "entry-choice", what: { ref: "this", noun: "land" }, who: { player: "you" },
      costs: [{ cost: "behold", quality: { subtypes: ["Jace"] }, count: n(1) }], declined: { tapped: true } } });
  });

  it("parses 'if a [quality] was beheld' as a condition, not a you-do test", () => {
    const [, spell] = parse("As an additional cost to cast this spell, you may behold a Hero.\nDestroy target tapped creature. If a Hero was beheld, scry 2.", "Instant");
    assert.ok(spell.kind === "spell");
    assert.deepEqual(spell.effects[1], { steps: [{ effect: "scry", amount: n(2) }],
      condition: { condition: "beheld", quality: { subtypes: ["Hero"] } }, conditionKind: "if" });
  });

  it("rejects behold wordings it cannot represent", () => {
    rejected("As an additional cost to cast this spell, you may choose a creature type and behold two creatures of that type.", "Instant");
    rejected("As an additional cost to cast this spell, behold a Dragon you control.", "Instant");
    rejected("As an additional cost to cast this spell, behold a Dragon card.", "Instant");
  });
});
