import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 118.9: an
// alternative cost is chosen while casting (601.2b) and replaces only the
// mana cost.
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
const you = { player: "you" } as const;
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const zero = [{ cost: "mana", symbols: ["0"] }];

describe("alternative costs limited to one spell per turn (CR 118.9)", () => {
  it("distinguishes any turn from the controller's own turns", () => {
    // Charred Foyer; Dragon's Smile; Darksteel Monolith.
    const fromExile = { ref: "filter", filter: { cls: "spell", cast: { by: you, from: { zone: "exile" } } }, count: fixed(1) };
    assert.deepEqual(ability("Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast from exile."), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: fromExile, costs: zero,
        useLimit: { maximum: 1, period: "turn", during: "any" } },
    });
    assert.deepEqual(ability("Once during each of your turns, you may pay {0} rather than pay the mana cost for a spell you cast from exile."), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: fromExile, costs: zero,
        useLimit: { maximum: 1, period: "turn", during: "your" } },
    });
    assert.deepEqual(ability("Once each turn, you may pay {0} rather than pay the mana cost for a colorless spell you cast from your hand.", "Artifact"), {
      kind: "static", effect: { static: "alternative-cost", who: you, costs: zero, useLimit: { maximum: 1, period: "turn", during: "any" },
        spells: { ref: "filter", filter: { colorless: true, cls: "spell", cast: { by: you, from: { zone: "hand", owner: "your" } } }, count: fixed(1) } },
    });
  });

  it("leaves the unlimited wording without a limit and rejects a limit on one spell", () => {
    const unlimited = ability("You may pay {0} rather than pay the mana cost for a spell you cast from exile.");
    assert.equal(unlimited.kind === "static" && "useLimit" in unlimited.effect, false);
    fails("Once each turn, you may pay {0} rather than pay this spell's mana cost.", "Instant");
    fails("Once each turn you may pay {0} rather than pay the mana cost for a spell you cast from exile.");
  });
});

describe("having a player gain life as an alternative cost (CR 118.1, 118.9)", () => {
  it("records the life gain as the cost and keeps the condition", () => {
    // Invigorate.
    assert.deepEqual(ability("If you control a Forest, rather than pay this spell's mana cost, you may have an opponent gain 3 life.", "Instant"), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" },
        costs: [{ cost: "have-gain-life", who: { player: "an-opponent" }, amount: fixed(3) }],
        condition: { condition: "control", who: you, what: { ref: "filter", filter: { subtypes: ["Forest"] }, count: fixed(1) } } },
    });
  });

  it("rejects a targeted player, a different effect and unrepresented players", () => {
    fails("If you control a Forest, rather than pay this spell's mana cost, you may have target opponent gain 3 life.", "Instant");
    fails("If you control a Forest, rather than pay this spell's mana cost, you may have an opponent draw a card.", "Instant");
    // Reverent Silence: "each other player" keeps its unresolved contrast
    // (PlayerFilter.other); every such player gains the life.
    assert.deepEqual(ability("If you control a Forest, rather than pay this spell's mana cost, you may have each other player gain 6 life.", "Instant"), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" },
        costs: [{ cost: "have-gain-life", who: { player: "filtered", base: { player: "each-player" }, filter: { other: true } }, amount: fixed(6) }],
        condition: { condition: "control", who: you, what: { ref: "filter", filter: { subtypes: ["Forest"] }, count: fixed(1) } } },
    });
    fails("Rather than pay this spell's mana cost, an opponent may gain 3 life.", "Instant");
  });
});
