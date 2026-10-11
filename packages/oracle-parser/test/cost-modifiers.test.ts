import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { StaticEffect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseActivationCostModifications, parseCostModification } from "../src/parser/cost-modifiers.ts";
import { parseEligibleCardRef } from "../src/parser/permissions.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
function modifier(text: string, effect: StaticEffect): void {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.deepEqual(parsed.abilities, [{ kind: "static", abilityWord: undefined, effect }]);
}
const thisSpell = { action: "cast" as const, spells: { ref: "this" as const, noun: "spell" } };
const reduceOne = { direction: "reduce" as const, symbols: ["1"] };

// Oracle examples verified in cached Scryfall: Goblin Electromancer, Goreclaw,
// Sphere of Resistance, The Great Henge, Training Grounds, Bard Class,
// Oppressive Rays, Discontinuity, and Naiad of Hidden Coves.
// CR 118.7 and 601.2f: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("casting and activation cost adjustments", () => {
  it("keeps spell eligibility and caster separate from control", () => {
    modifier("Artifact spells you cast cost {1} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" },
        spells: { ref: "each", filter: { types: ["artifact"], cls: "spell" } } }, change: reduceOne,
    });
    modifier("Spells your opponents cast cost {2} more to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "each-opponent" },
        spells: { ref: "each", filter: { cls: "spell" } } }, change: { direction: "increase", symbols: ["2"] },
    });
  });

  it("treats plural listed card types as alternatives while retaining card-type intersections", () => {
    modifier("Instant and sorcery spells you cast cost {1} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each",
        filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } } }, change: reduceOne,
    });
    modifier("Artifact creature spells you cast cost {1} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" },
        spells: { ref: "each", filter: { types: ["artifact", "creature"], cls: "spell" } } }, change: reduceOne,
    });
  });

  it("preserves characteristics appearing after a caster clause", () => {
    modifier("Creature spells you cast with power 4 or greater cost {2} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter: {
        types: ["creature"], cls: "spell", properties: [{ property: "power", comparison: { op: "ge", value: fixed(4) } }],
      } } }, change: { direction: "reduce", symbols: ["2"] },
    });
  });

  it("keeps casting-origin zones separate from spell-location filters", () => {
    modifier("Spells you cast from your graveyard cost {1} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" },
        spells: { ref: "each", filter: { cls: "spell" } }, from: { zone: "graveyard", owner: "your" } }, change: reduceOne,
    });
  });

  it("retains the targeted object or player independently of the spell's caster", () => {
    modifier("Spells your opponents cast that target this creature cost {2} more to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "each-opponent" },
        spells: { ref: "each", filter: { cls: "spell", targets: { ref: "this", noun: "creature" } } } },
      change: { direction: "increase", symbols: ["2"] },
    });
    modifier("Spells that target you cost {2} less to cast.", {
      static: "cost-modification", scope: { action: "cast",
        spells: { ref: "each", filter: { cls: "spell", targets: { player: "you" } } } },
      change: { direction: "reduce", symbols: ["2"] },
    });
    modifier("This spell costs {2} less to cast if it targets a blue spell.", {
      static: "conditional", condition: { condition: "targets", what: { ref: "it" },
        target: { ref: "filter", count: fixed(1), filter: { colors: ["blue"], cls: "spell" } } },
      effect: { static: "cost-modification", scope: thisSpell, change: { direction: "reduce", symbols: ["2"] } },
    });
  });

  it("retains an X definition and dynamic per-object multipliers", () => {
    modifier("This spell costs {X} less to cast, where X is the greatest power among creatures you control.", {
      static: "cost-modification", scope: thisSpell, change: { direction: "reduce", symbols: ["X"] },
      bindings: [{ variable: "x", value: { amount: "attribute-extremum", extremum: "maximum", attribute: "power", among: { types: ["creature"], control: "you" } } }],
    });
    modifier("This spell costs {1} less to cast for each creature you control.", {
      static: "cost-modification", scope: thisSpell, change: reduceOne, multiplier: { amount: "count", of: { types: ["creature"], control: "you" } },
    });
  });

  it("counts either listed card type once under an explicit for-each quantifier", () => {
    modifier("This spell costs {1} less to cast for each instant and sorcery card in your graveyard.", {
      static: "cost-modification", scope: thisSpell, change: reduceOne,
      multiplier: { amount: "count", of: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }],
        cls: "card", zone: { zone: "graveyard", owner: "your" } } },
    });
    modifier("This spell costs {1} less to cast for each green and blue creature card in your graveyard.", {
      static: "cost-modification", scope: thisSpell, change: reduceOne,
      multiplier: { amount: "count", of: { colors: ["green", "blue"], types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } },
    });
    const normal = new Cursor(lex("a green and blue creature spell"));
    assert.deepEqual(parseEligibleCardRef(normal), { ref: "filter", count: fixed(1),
      filter: { colors: ["green", "blue"], types: ["creature"], cls: "spell" } });
    assert.equal(normal.done(), true);
  });

  it("adds explicit count groups while retaining their distinct zones (Gargantuan Leech)", () => {
    modifier("This spell costs {1} less to cast for each Cave you control and each Cave card in your graveyard.", {
      static: "cost-modification", scope: thisSpell, change: reduceOne,
      multiplier: { amount: "plus", a: { amount: "count", of: { subtypes: ["Cave"], control: "you" } },
        b: { amount: "count", of: { subtypes: ["Cave"], cls: "card", zone: { zone: "graveyard", owner: "your" } } } },
    });
    modifier("This spell costs {1} less to cast for each artifact card in your graveyard and each creature card in your graveyard.", {
      static: "cost-modification", scope: thisSpell, change: reduceOne,
      multiplier: { amount: "plus", a: { amount: "count", of: { types: ["artifact"], cls: "card", zone: { zone: "graveyard", owner: "your" } } },
        b: { amount: "count", of: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } } },
    });
  });

  it("counts the specified counters on their holders, including a player or object set", () => {
    modifier("Instant and sorcery spells you cast cost {1} less to cast for each experience counter you have.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each",
        filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } } }, change: reduceOne,
      multiplier: { amount: "counter-count", counter: { type: "experience" }, on: { player: "you" } },
    });
    const c = new Cursor(lex("This ability costs {1} less to activate for each +1/+1 counter on creatures you control."));
    assert.deepEqual(parseActivationCostModifications(c), [{ change: reduceOne,
      multiplier: { amount: "counter-count", counter: { type: "+1/+1" },
        on: { ref: "each", filter: { types: ["creature"], control: "you" } } } }]);
    assert.equal(c.done(), true);
  });

  it("retains conditions and your-turn restrictions", () => {
    const base: StaticEffect = { static: "cost-modification", scope: thisSpell, change: { direction: "reduce", symbols: ["2", "U", "U"] } };
    modifier("During your turn, this spell costs {2}{U}{U} less to cast.", {
      static: "conditional", condition: { condition: "your-turn" }, effect: base,
    });
    modifier("This spell costs {2}{U}{U} less to cast during your turn.", {
      static: "conditional", condition: { condition: "your-turn" }, effect: base,
    });
    modifier("During turns other than yours, this spell costs {2}{U}{U} less to cast.", {
      static: "conditional", condition: { condition: "not-your-turn" }, effect: base,
    });
    modifier("This spell costs {1} less to cast if you control an artifact.", {
      static: "conditional", condition: { condition: "control", who: { player: "you" }, what: { ref: "filter", count: fixed(1), filter: { types: ["artifact"] } } },
      effect: { static: "cost-modification", scope: thisSpell, change: reduceOne },
    });
  });

  it("records the distinct mana floor on activated-ability reductions", () => {
    modifier("Activated abilities of creatures you control cost {2} less to activate. This effect can't reduce the mana in that cost to less than one mana.", {
      static: "cost-modification", scope: { action: "activate", source: { ref: "each", filter: { types: ["creature"], control: "you" } } },
      change: { direction: "reduce", symbols: ["2"] }, minimumTotalMana: fixed(1),
    });
    modifier("Activated abilities of enchanted creature cost {3} more to activate.", {
      static: "cost-modification", scope: { action: "activate", source: { ref: "enchanted", noun: "creature" } },
      change: { direction: "increase", symbols: ["3"] },
    });
  });

  it("preserves the target of an activated ability separately from its source", () => {
    modifier("Abilities your opponents activate that target a Merfolk you control cost {2} more to activate.", {
      static: "cost-modification", scope: { action: "activate", activator: { player: "each-opponent" },
        abilities: { ref: "each", filter: { cls: "ability", targets: { ref: "filter", count: fixed(1),
          filter: { subtypes: ["Merfolk"], control: "you" } } } } },
      change: { direction: "increase", symbols: ["2"] },
    });
    modifier("Equip abilities you activate that target enchanted creature cost {3} less to activate.", {
      static: "cost-modification", scope: { action: "activate", activator: { player: "you" }, keyword: "equip",
        abilities: { ref: "each", filter: { cls: "ability", targets: { ref: "enchanted", noun: "creature" } } } },
      change: { direction: "reduce", symbols: ["3"] },
    });
  });

  it("selects equip and boast abilities without attributing those keywords to their source", () => {
    modifier("Boast abilities you activate cost {1} less to activate for each Dragon you control.", {
      static: "cost-modification", scope: { action: "activate", activator: { player: "you" }, keyword: "boast",
        abilities: { ref: "each", filter: { cls: "ability" } } }, change: reduceOne,
      multiplier: { amount: "count", of: { subtypes: ["Dragon"], control: "you" } },
    });
    modifier("Equip abilities you activate of other Equipment cost {1} less to activate.", {
      static: "cost-modification", scope: { action: "activate", activator: { player: "you" }, keyword: "equip",
        abilities: { ref: "each", filter: { cls: "ability" } },
        source: { ref: "each", filter: { other: true, subtypes: ["Equipment"] } } }, change: reduceOne,
    });
  });

  it("selects the actual activated keyword category and distinguishes loyalty abilities", () => {
    for (const keyword of ["cycling", "ninjutsu", "exhaust", "power-up"] as const) {
      modifier(`${keyword} abilities you activate cost {1} less to activate.`, {
        static: "cost-modification", scope: { action: "activate", activator: { player: "you" }, keyword,
          abilities: { ref: "each", filter: { cls: "ability" } } }, change: reduceOne,
      });
    }
    modifier("Loyalty abilities of planeswalkers your opponents control cost {1} more to activate.", {
      static: "cost-modification", scope: { action: "activate", loyalty: true,
        abilities: { ref: "each", filter: { cls: "ability" } },
        source: { ref: "each", filter: { types: ["planeswalker"], control: "opponent" } } },
      change: { direction: "increase", symbols: ["1"] },
    });
  });

  it("retains a possessive source and the ability's conditional targeting requirement", () => {
    modifier("This Equipment's equip ability costs {2} less to activate if it targets a colorless creature.", {
      static: "conditional", condition: { condition: "targets", what: { ref: "it" },
        target: { ref: "filter", count: fixed(1), filter: { colorless: true, types: ["creature"] } } },
      effect: { static: "cost-modification", scope: { action: "activate", source: { ref: "this", noun: "equipment" },
        keyword: "equip", abilities: { ref: "each", filter: { cls: "ability" } } },
        change: { direction: "reduce", symbols: ["2"] } },
    });
  });

  it("retains an ongoing threshold condition after the cost adjustment", () => {
    modifier("Noncreature spells you cast cost {1} less to cast as long as there are three or more Lesson cards in your graveyard.", {
      static: "conditional", condition: { condition: "count",
        of: { subtypes: ["Lesson"], cls: "card", zone: { zone: "graveyard", owner: "your" } },
        comparison: { op: "ge", value: fixed(3) } },
      effect: { static: "cost-modification", scope: { action: "cast", caster: { player: "you" },
        spells: { ref: "each", filter: { nonTypes: ["creature"], cls: "spell" } } }, change: reduceOne },
    });
  });

  it("preserves colored-only reduction instead of applying normal generic fallback", () => {
    modifier("Legendary spells you cast cost {R}{G} less to cast. This effect reduces only the amount of colored mana you pay.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter: { supertypes: ["legendary"], cls: "spell" } } },
      change: { direction: "reduce", symbols: ["R", "G"] }, coloredOnly: true,
    });
  });

  it("retains an unresolved chosen-type eligibility instead of dropping it", () => {
    modifier("Spells you cast of the chosen type cost {1} less to cast.", {
      static: "cost-modification", scope: { action: "cast", caster: { player: "you" },
        spells: { ref: "each", filter: { cls: "spell", chosenType: { choice: "chosen-type" } } } }, change: reduceOne,
    });
  });

  it("rejects unsupported qualifiers instead of accepting a cost prefix", () => {
    for (const text of [
      "This spell costs {1} less to cast for each target beyond the first.",
      "This spell costs {1} less to cast if invented condition.",
      "This spell costs {T} less to cast.",
      "This spell costs {2} more to cast. This effect reduces only the amount of colored mana you pay.",
      "Activated abilities cost {1} less to activate unless they're mana abilities.",
      "Equip abilities you activate cost {1} less to activate until end of turn.",
      "Triggered abilities you activate cost {1} less to activate.",
      "Flying abilities you activate cost {1} less to activate.",
      "This Equipment's invented ability costs {1} less to activate.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseCostModification(c), null, text);
      assert.equal(c.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
  });
});

describe("activation payment instructions", () => {
  // Scryfall: Sanctum of Tranquil Light, Hylda's Crown of Winter, and
  // Loreseeker's Stone. CR 602.1b separates these from resolving effects.
  it("consumes a complete adjustment while leaving activation restrictions", () => {
    const c = new Cursor(lex("This ability costs {1} less to activate for each Shrine you control. Activate only as a sorcery."));
    assert.deepEqual(parseActivationCostModifications(c), [{ change: reduceOne,
      multiplier: { amount: "count", of: { subtypes: ["Shrine"], control: "you" } } }]);
    assert.equal(c.words("activate", "only", "as", "a", "sorcery"), true);
    assert.equal(c.punct("."), true);
    assert.equal(c.done(), true);
  });

  it("keeps conditional payment logic on the ability instead of its effect", () => {
    const c = new Cursor(lex("This ability costs {1} less to activate during your turn."));
    assert.deepEqual(parseActivationCostModifications(c), [{ change: reduceOne, condition: { condition: "your-turn" } }]);
    assert.equal(c.done(), true);
    const increased = new Cursor(lex("This ability costs {1} more to activate for each card in your hand."));
    assert.deepEqual(parseActivationCostModifications(increased), [{ change: { direction: "increase", symbols: ["1"] },
      multiplier: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } } }]);
    assert.equal(increased.done(), true);
  });

  it("keeps every adjustment and rolls back an incomplete subsequent adjustment", () => {
    const first = "This ability costs {1} less to activate if you control an artifact. ";
    const c = new Cursor(lex(first + "This ability costs {2} more to activate during your turn."));
    assert.deepEqual(parseActivationCostModifications(c), [
      { change: reduceOne, condition: { condition: "control", who: { player: "you" },
        what: { ref: "filter", count: fixed(1), filter: { types: ["artifact"] } } } },
      { change: { direction: "increase", symbols: ["2"] }, condition: { condition: "your-turn" } },
    ]);
    assert.equal(c.done(), true);
    for (const text of [
      first + "This ability costs {2} less to activate if invented qualifier.",
      "This ability costs {1} less to activate during your turn Activate only as a sorcery.",
      "This ability costs {1} less to cast.",
      "This ability costs {T} less to activate.",
      "This spell costs {1} less to cast.",
    ]) {
      const failed = new Cursor(lex(text));
      assert.equal(parseActivationCostModifications(failed), null, text);
      assert.equal(failed.pos, 0, text);
    }
  });

  it("attaches a payment instruction to the single equip ability (Plate Armor)", () => {
    const text = "Equip {3}. This ability costs {1} less to activate for each other Equipment you control.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    if (!result.ok) throw new Error(text);
    assert.deepEqual(result.abilities, [{ kind: "keywords", keywords: [{ keyword: "equip",
      cost: [{ cost: "mana", symbols: ["3"] }], costModifications: [{ change: reduceOne,
        multiplier: { amount: "count", of: { other: true, subtypes: ["Equipment"], control: "you" } } }],
    }] }]);
    for (const invalid of [
      "Flying, equip {3}. This ability costs {1} less to activate.",
      "Ward {3}. This ability costs {1} less to activate.",
      "Equip {3}. This ability costs {1} less to cast.",
      "Equip {3}. This ability costs {1} less to activate. Draw a card.",
    ]) assert.equal(parseOracleText(invalid).ok, false, invalid);
  });

  it("keeps base costs, complete resolving effects, and activation restrictions together", () => {
    const text = "{4}, {T}: Draw a card. This ability costs {1} less to activate for each modified creature you control. Activate only as a sorcery.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    if (!result.ok) throw new Error(text);
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
      costModifications: [{ change: reduceOne, multiplier: { amount: "count", of: { status: ["modified"], types: ["creature"], control: "you" } } }],
      restriction: { restriction: "sorcery" },
    }]);
  });

  it("does not overwrite resolving actors or conditions with the payment instruction", () => {
    const text = "{1}, {T}: Target opponent draws a card. You gain 2 life. This ability costs {1} less to activate during your turn.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    if (!result.ok) throw new Error(text);
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }],
      effects: [
        { steps: [{ effect: "draw", who: { player: "target-opponent" }, amount: fixed(1) }] },
        { steps: [{ effect: "gain-life", who: { player: "you" }, amount: fixed(2) }] },
      ],
      costModifications: [{ change: reduceOne, condition: { condition: "your-turn" } }], restriction: undefined,
    }]);
    for (const invalid of [
      "Draw a card. This ability costs {1} less to activate.",
      "Whenever this creature attacks, draw a card. This ability costs {1} less to activate.",
      "I — Draw a card. This ability costs {1} less to activate.",
      "{T}: Draw a card. This ability costs {1} less to activate if invented condition.",
      "{T}: Draw a card. This ability costs {1} less to activate. Discard a card.",
    ]) assert.equal(parseOracleText(invalid).ok, false, invalid);
  });
});
