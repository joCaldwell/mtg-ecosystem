import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 602.2b, 601.2b,
// 601.2f, 118.9 (alternative costs), 702.6a (equip), 702.29a/c (cycling).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Legendary Creature — Dwarf"): ParsedLine {
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
const you = { player: "you" } as const;
const zero = [{ cost: "mana", symbols: ["0"] }] as const;
const abilities = { ref: "each", filter: { cls: "ability" } } as const;
const first = { ordinal: 1, period: "turn" } as const;

describe("alternative activation costs (CR 602.2b, 118.9)", () => {
  it("replaces the cost of the first matching activation each turn", () => {
    // Bruenor Battlehammer.
    assert.deepEqual(ability("You may pay {0} rather than pay the equip cost of the first equip ability you activate each turn."), {
      kind: "static", effect: { static: "alternative-activation-cost", who: you, costs: zero,
        scope: { action: "activate", activator: you, occurrence: first, keyword: "equip", abilities } },
    });
    // Gavi, Nest Warden: cycling a card is activating its cycling ability.
    assert.deepEqual(ability("You may pay {0} rather than pay the cycling cost of the first card you cycle each turn."), {
      kind: "static", effect: { static: "alternative-activation-cost", who: you, costs: zero,
        scope: { action: "activate", activator: you, occurrence: first, keyword: "cycling", abilities } },
    });
  });

  it("covers every such activation, under a stated condition", () => {
    // New Perspectives.
    assert.deepEqual(ability("As long as you have seven or more cards in hand, you may pay {0} rather than pay cycling costs.", "Enchantment"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "cards-in-hand", who: you, comparison: { op: "ge", value: { amount: "fixed", value: 7 } } },
        effect: { static: "alternative-activation-cost", who: you, costs: zero, scope: { action: "activate", activator: you, keyword: "cycling", abilities } } },
    });
  });

  it("rejects a named cost that is not the described abilities' cost, and unrepresented periods", () => {
    fails("You may pay {0} rather than pay the cycling cost of the first equip ability you activate each turn.");
    fails("You may pay {0} rather than pay the equip cost of the first card you cycle each turn.");
    // "during each of your turns" is not an ordinal over every turn: it is
    // the per-turn ordinal under a your-turn condition.
    assert.deepEqual(ability("You may pay {0} rather than pay the equip cost of the first equip ability you activate during each of your turns."), {
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "alternative-activation-cost", who: you, costs: zero,
          scope: { action: "activate", activator: you, occurrence: first, keyword: "equip", abilities } } },
    });
    fails("You may pay {0} rather than pay the cycling cost.");
  });
});
