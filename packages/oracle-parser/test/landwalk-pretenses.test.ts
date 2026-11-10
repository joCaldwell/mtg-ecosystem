import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 609.4 ("as
// though"), 702.14a–c (landwalk), 509.1b.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Enchantment"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
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
const one = { amount: "fixed", value: 1 } as const;
const walk = (land: string) => ({ keyword: "landwalk", filter: { types: ["land"], subtypes: [land] } });

describe("a specific landwalk in filters and combat pretenses (CR 702.14a, 609.4)", () => {
  it("names the one landwalk ability, in the filter and in the pretense", () => {
    // Crevasse; Undertow.
    assert.deepEqual(ability("Creatures with mountainwalk can be blocked as though they didn't have mountainwalk."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "each", filter: { types: ["creature"], withAbility: ["mountainwalk"] } },
        rule: { rule: "as-though", action: "be-blocked", pretense: { pretend: "lacks-keyword", of: "affected-creature", keyword: walk("Mountain") } } },
    });
    assert.deepEqual(ability("Creatures with islandwalk can be blocked as though they didn't have islandwalk."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "each", filter: { types: ["creature"], withAbility: ["islandwalk"] } },
        rule: { rule: "as-though", action: "be-blocked", pretense: { pretend: "lacks-keyword", of: "affected-creature", keyword: walk("Island") } } },
    });
  });

  it("stays distinct from the generic landwalk wording", () => {
    // Staff of the Ages.
    assert.deepEqual(ability("Creatures with landwalk abilities can be blocked as though they didn't have those abilities.", "Artifact"), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "each", filter: { types: ["creature"], withAbility: ["landwalk"] } },
        rule: { rule: "as-though", action: "be-blocked", pretense: { pretend: "lacks-keyword", of: "affected-creature", keyword: { keyword: "landwalk" } } } },
    });
    // "those abilities" needs the generic landwalk filter as its antecedent.
    fails("Creatures with mountainwalk can be blocked as though they didn't have those abilities.");
  });

  it("applies the pretense to the blocked creatures that the filter names", () => {
    assert.deepEqual(ability("This creature can block creatures with swampwalk as though they didn't have swampwalk.", "Creature — Wall"), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" },
        rule: { rule: "as-though", action: "block", pretense: { pretend: "lacks-keyword", of: "blocked-creature", keyword: walk("Swamp") },
          attackers: { ref: "each", filter: { types: ["creature"], withAbility: ["swampwalk"] } } } },
    });
    // The blocked creatures are not said to have the ability that is ignored.
    fails("This creature can block creatures with swampwalk as though they didn't have forestwalk.", "Creature — Wall");
  });

  it("selects by a specific landwalk elsewhere", () => {
    // Merfolk Assassin.
    assert.deepEqual(ability("{T}: Destroy target creature with islandwalk.", "Creature — Merfolk Assassin"), {
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["creature"], withAbility: ["islandwalk"] }, count: one } }] }],
    });
    fails("{T}: Destroy target creature with riverwalk.", "Creature — Merfolk Assassin");
  });
});
