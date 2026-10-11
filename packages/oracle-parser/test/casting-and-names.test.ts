import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText, normalizeOracleText } from "../src/index.ts";

const you = { player: "you" };
const n = (value: number) => ({ amount: "fixed", value });
function one(text: string, name?: string) {
  const parsed = parseOracleText(text, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}

describe("casting alternatives are optional costs, not resolving actions", () => {
  it("distinguishes all-player totals from a single player's history (Village Ironsmith / Ironfang)", () => {
    const trigger = { trigger: "phase", phase: "upkeep", whose: "each-player" };
    const effects = [{ steps: [{ effect: "transform", what: { ref: "this", noun: "creature" }, who: you }] }];
    assert.deepEqual(one("At the beginning of each upkeep, if no spells were cast last turn, transform this creature."), {
      kind: "triggered", trigger, condition: { condition: "cast-count", who: "all-players-combined", what: { cls: "spell" }, comparison: { op: "eq", value: n(0) }, during: "last-turn" }, effects,
    });
    assert.deepEqual(one("At the beginning of each upkeep, if a player cast two or more spells last turn, transform this creature."), {
      kind: "triggered", trigger, condition: { condition: "cast-count", who: { player: "a-player" }, what: { cls: "spell" }, comparison: { op: "ge", value: n(2) }, during: "last-turn" }, effects,
    });
    for (const text of ["If no spells were cast during some earlier turn, draw a card.", "If a player cast two or more spells, draw a card."]) assert.equal(parseOracleText(text).ok, false);
  });
  it("preserves a sacrifice payment and the replaced spell mana cost (Thunderclap)", () => {
    assert.deepEqual(one("You may sacrifice a Mountain rather than pay this spell's mana cost."), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" },
        costs: [{ cost: "sacrifice", what: { ref: "filter", filter: { subtypes: ["Mountain"] }, count: n(1) } }],
      },
    });
    assert.deepEqual(one("You may sacrifice a Mountain."), {
      kind: "spell", effects: [{ steps: [{ effect: "sacrifice", who: you,
        what: { ref: "filter", filter: { subtypes: ["Mountain"] }, count: n(1) }, optional: true,
      }] }],
    });
  });
  it("keeps the eligibility condition and exile's exact card filter (Force of Despair)", () => {
    assert.deepEqual(one("If it's not your turn, you may exile a black card from your hand rather than pay this spell's mana cost."), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" },
        costs: [{ cost: "exile", what: { ref: "filter", filter: { colors: ["black"], cls: "card", zone: { zone: "hand", owner: "your" } }, count: n(1) } }],
        condition: { condition: "not-your-turn" },
      },
    });
  });
  it("keeps conjunctive mana and return payments (Mistvein Borderpost)", () => {
    assert.deepEqual(one("You may pay {1} and return a basic land you control to its owner's hand rather than pay this spell's mana cost."), {
      kind: "static", effect: { static: "alternative-cost", who: you, spells: { ref: "this", noun: "spell" }, costs: [
        { cost: "mana", symbols: ["1"] },
        { cost: "return", what: { ref: "filter", filter: { supertypes: ["basic"], types: ["land"], control: "you" }, count: n(1) }, to: "hand", owner: "owner" },
      ] },
    });
  });
  it("distinguishes the caster, controller and casting origin (Nissa, Worldsoul Speaker / Chief Engineer)", () => {
    assert.deepEqual(one("You may pay eight {E} rather than pay the mana cost for permanent spells you cast."), {
      kind: "static", effect: { static: "alternative-cost", who: you,
        spells: { ref: "each", filter: { cls: "spell", permanent: true, cast: { by: you } } }, costs: [{ cost: "energy", amount: n(8) }],
      },
    });
    assert.deepEqual(one("Artifact spells you cast have convoke."), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["artifact"], cls: "spell", cast: { by: you } } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "convoke" } }] }],
      },
    });
    assert.deepEqual(one("Creature spells you cast from your graveyard have convoke."), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"], cls: "spell", cast: { by: you, from: { zone: "graveyard", owner: "your" } } } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "convoke" } }] }],
      },
    });
    for (const text of ["You may pay {1} rather than pay this spell's mana cost and draw a card.", "You may pay {1} rather than pay this spell's mana cost only once each turn.", "Artifact spells you cast from have convoke."]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("literal named cards remain literal when the source changes name", () => {
  it("separates self references from same-name card selection (Squadron Hawk)", () => {
    const text = "When Squadron Hawk enters, you may search your library for up to three cards named Squadron Hawk, reveal them, put them into your hand, then shuffle.";
    assert.deepEqual(normalizeOracleText(text, "Squadron Hawk"), [
      'When ~ enters, you may search your library for up to three cards named "Squadron Hawk", reveal them, put them into your hand, then shuffle.',
    ]);
    assert.deepEqual(one(text, "Squadron Hawk"), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "self" } }, effects: [{ steps: [{ effect: "optional-group", who: you, steps: [
        { effect: "search", who: you, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { cls: "card", name: { name: "literal", value: "Squadron Hawk" } }, count: n(3), upTo: true } },
        { effect: "reveal", who: you, what: { ref: "them" } },
        { effect: "move-zone", what: { ref: "them" }, to: { zone: "hand", owner: "your" } },
        { effect: "shuffle", who: you, library: { zone: "library", owner: you } },
      ] }] }],
    });
  });
  it("protects comma-bearing names from a second short-name substitution", () => {
    assert.deepEqual(normalizeOracleText('Search your library for a card named Niv-Mizzet, Parun.', 'Niv-Mizzet, Parun'), ['Search your library for a card named "Niv-Mizzet, Parun".']);
    assert.deepEqual(normalizeOracleText('Search your library for a card named "Niv-Mizzet, Parun".', 'Niv-Mizzet, Parun'), ['Search your library for a card named "Niv-Mizzet, Parun".']);
    assert.equal(parseOracleText('Search your library for a card named ~.').ok, false);
    assert.equal(parseOracleText('Search your library for a card named "".').ok, false);
  });
});
