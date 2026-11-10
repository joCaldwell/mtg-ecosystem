import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall
// Oracle corpus; rules from the Comprehensive Rules (2026-09-25).
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
const zero = [{ cost: "mana", symbols: ["0"] }] as const;
const abilities = { ref: "each", filter: { cls: "ability" } } as const;
const first = { ordinal: 1, period: "turn" } as const;

describe("alternative activation cost on your own turns (CR 602.2b, 118.9)", () => {
  it("counts the ordinal within each of your turns", () => {
    // Advancing the Spirit.
    assert.deepEqual(ability("You may pay {0} rather than pay the power-up cost of the first power-up ability you activate during each of your turns."), {
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "alternative-activation-cost", who: you, costs: zero,
          scope: { action: "activate", activator: you, occurrence: first, keyword: "power-up", abilities } } },
    });
  });
  it("still rejects an unmatched keyword or another player's turns", () => {
    fails("You may pay {0} rather than pay the equip cost of the first power-up ability you activate during each of your turns.");
    fails("You may pay {0} rather than pay the equip cost of the first equip ability you activate during each opponent's turn.");
  });
});

describe("counters your opponents have (CR 122.1)", () => {
  const total = { amount: "counter-count", counter: { type: "poison" }, on: { player: "each-opponent" }, total: true } as const;
  it("totals the counters of every opponent", () => {
    // Vishgraz, the Doomhive.
    assert.deepEqual(ability("This creature gets +1/+1 for each poison counter your opponents have.", "Creature — Phyrexian"), {
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "pt",
        power: { sign: 1, amount: { amount: "fixed", value: 1 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 1 } },
        multiplier: total, valueScope: "each-affected-object" }] },
    });
  });
  it("keeps distributive or unrepresented holders rejected", () => {
    fails("This creature gets +1/+1 for each poison counter each opponent has.", "Creature — Phyrexian");
    fails("This creature gets +1/+1 for each poison counter your opponents had.", "Creature — Phyrexian");
  });
});

describe("parity of an object's characteristic as a condition", () => {
  it("tests the referenced object's power or mana value parity", () => {
    // Kianne, Corrupted Memory.
    assert.deepEqual(ability("As long as ~'s power is odd, you may cast creature spells as though they had flash.", "Legendary Creature — Illusion"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast",
        what: { ref: "each", filter: { types: ["creature"], cls: "spell" } }, timing: "as-though-flash",
        condition: { condition: "is-filter", what: { ref: "self" }, filter: { parity: { property: "power", parity: "odd" } } } } },
    });
    // Yennett, Cryptic Sovereign (the cast instruction's own condition).
    const yennett = ability("Whenever ~ attacks, reveal the top card of your library. You may cast it without paying its mana cost if its mana value is odd.", "Legendary Creature — Sphinx");
    assert.deepEqual(yennett, {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } }, effects: [
        { steps: [{ effect: "reveal", who: you, what: { ref: "library-cards", position: "top", count: { amount: "fixed", value: 1 }, of: { zone: "library", owner: "your" } } }] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: { ref: "it" }, alternativeCost: { cost: "without-mana-cost" },
          condition: { condition: "is-filter", what: { ref: "it" }, filter: { parity: { property: "mana-value", parity: "odd" } } } } }] },
      ],
    });
  });
  it("does not read parity of toughness or of a count", () => {
    fails("As long as ~'s toughness is odd, you may cast creature spells as though they had flash.", "Legendary Creature — Illusion");
    fails("As long as the number of creatures you control is odd, you may cast creature spells as though they had flash.", "Legendary Creature — Illusion");
  });
});

describe("comparison against an unrounded half (CR 107.1a)", () => {
  const halfStart = { amount: "half", of: { amount: "starting-life-total", of: you }, round: "exact" } as const;
  it("compares exactly, without inventing a rounding direction", () => {
    // Bane, Lord of Darkness.
    assert.deepEqual(ability("As long as your life total is less than or equal to half your starting life total, ~ has indestructible.", "Legendary Creature — God"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "compare", value: { amount: "life-total", of: you }, comparison: { op: "le", value: halfStart } },
        effect: { static: "modify", what: { ref: "self" }, modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }] }] } },
    });
  });
  it("keeps a stated rounding and rejects an unrounded half outside a comparison", () => {
    const stated = ability("As long as your life total is less than half your starting life total, rounded down, ~ has indestructible.", "Legendary Creature — God");
    assert.deepEqual(stated, {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "compare", value: { amount: "life-total", of: you },
          comparison: { op: "lt", value: { amount: "half", of: { amount: "starting-life-total", of: you }, round: "down" } } },
        effect: { static: "modify", what: { ref: "self" }, modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }] }] } },
    });
    fails("You gain life equal to half your starting life total.", "Sorcery");
    fails("As long as your life total is half your starting life total, ~ has indestructible.", "Legendary Creature — God");
  });
});

describe("trap alternative costs conditioned on the number of attacking creatures", () => {
  const trap = (symbols: string[], op: string, value: number) => ({
    kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" }, costs: [{ cost: "mana", symbols }],
      condition: { condition: "count", of: { types: ["creature"], status: ["attacking"] }, comparison: { op, value: { amount: "fixed", value } } } },
  });
  it("counts the creatures currently attacking (CR 506.4, 118.9)", () => {
    // Arrow Volley Trap, Pitfall Trap.
    assert.deepEqual(ability("If four or more creatures are attacking, you may pay {1}{W} rather than pay this spell's mana cost.", "Instant — Trap"), trap(["1", "W"], "ge", 4));
    assert.deepEqual(ability("If exactly one creature is attacking, you may pay {W} rather than pay this spell's mana cost.", "Instant — Trap"), trap(["W"], "eq", 1));
  });
  it("does not drop what the creatures are attacking", () => {
    fails("If two or more creatures are attacking you, you may pay {W} rather than pay this spell's mana cost.", "Instant — Trap");
    fails("If two or more creatures are attacking a planeswalker, you may pay {W} rather than pay this spell's mana cost.", "Instant — Trap");
    fails("If two or more attacking creatures are attacking, you may pay {W} rather than pay this spell's mana cost.", "Instant — Trap");
  });
});
