import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Condition } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses checked in Scryfall corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 603.4, 702.33d–g, 601.2i, 119:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;
const itRef = { ref: "it" } as const;
const thisCreature = { ref: "this", noun: "creature" } as const;
function condition(text: string): Condition {
  const c = new Cursor(lex(text));
  const result = parseCondition(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens`);
  return result;
}
function unsupported(text: string): void {
  const result = parseOracleText(`If ${text}, draw a card.`);
  assert.equal(result.ok, false, text);
}

describe("current zone predicates", () => {
  it("keeps the object, zone and owner without treating location as movement", () => {
    assert.deepEqual(condition("this card is in your graveyard"), {
      condition: "is-filter", what: { ref: "this", noun: "card" }, filter: { zone: { zone: "graveyard", owner: "your" } },
    });
    assert.deepEqual(condition("that card isn't in its owner's hand"), { condition: "not", of: {
      condition: "is-filter", what: { ref: "that", filter: { cls: "card" } }, filter: { zone: { zone: "hand", owner: "its-owner" } },
    } });
    assert.deepEqual(condition("it's in exile"), {
      condition: "is-filter", what: itRef, filter: { zone: { zone: "exile", owner: undefined } },
    });
    assert.deepEqual(condition("this permanent is on the battlefield"), {
      condition: "is-filter", what: { ref: "this", noun: "permanent" }, filter: { zone: { zone: "battlefield" } },
    });
    for (const text of ["this card was in your graveyard", "this card is in graveyard", "this card is on the top of your library", "this card is in your graveyard this turn"])
      unsupported(text);
  });
});

describe("kicker conditions retain linked cost and kick count", () => {
  it("distinguishes kicked at least once from kicked twice", () => {
    // Archangel of Wrath 022e97af-2a3a-4e13-9b6b-d34fcc8cf168 has both.
    assert.deepEqual(condition("it was kicked"), {
      condition: "kicked", what: itRef, comparison: { op: "ge", value: fixed(1) },
    });
    assert.deepEqual(condition("it was kicked twice"), {
      condition: "kicked", what: itRef, comparison: { op: "eq", value: fixed(2) },
    });
    assert.deepEqual(condition("this spell was kicked three or more times"), {
      condition: "kicked", what: { ref: "this", noun: "spell" }, comparison: { op: "ge", value: fixed(3) },
    });
  });

  it("retains the particular kicker cost being tested", () => {
    // Stormscape Battlemage 38ee748d-adcd-41df-9b23-d2a34829784c.
    for (const [symbols, text] of [[['W'], '{W}'], [['2', 'B'], '{2}{B}']] as const) {
      assert.deepEqual(condition(`it was kicked with its ${text} kicker`), {
        condition: "kicked", what: itRef, comparison: { op: "ge", value: fixed(1) },
        kickerCost: [{ cost: "mana", symbols: [...symbols] }],
      });
    }
    assert.deepEqual(condition("it wasn't kicked"), { condition: "not", of: {
      condition: "kicked", what: itRef, comparison: { op: "ge", value: fixed(1) },
    } });
    unsupported("it was kicked with its {W}");
    unsupported("it was kicked with its {T} kicker");
    unsupported("it was kicked by another player");
  });

  it("keeps an intervening kicker check on the triggered ability", () => {
    // Krosan Druid 1fbed9e4-86f6-4115-8468-8593e340701f.
    const result = parseOracleText("When this creature enters, if it was kicked, you gain 10 life.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "triggered", abilityWord: undefined, trigger: { trigger: "enters", what: thisCreature },
      condition: { condition: "kicked", what: itRef, comparison: { op: "ge", value: fixed(1) } },
      effects: [{ steps: [{ effect: "gain-life", who: you, amount: fixed(10) }] }],
    }]);
  });
});

describe("explicit count conditions", () => {
  it("retains the owner and filter when counting current personal-zone cards", () => {
    assert.deepEqual(condition("that player has three or fewer creature cards in their graveyard"), {
      condition: "count", of: { cls: "card", types: ["creature"], zone: { zone: "graveyard", owner: { player: "that-player" } } },
      comparison: { op: "le", value: fixed(3) },
    });
    assert.deepEqual(condition("two or more artifact cards are in your graveyard"), {
      condition: "count", of: { cls: "card", types: ["artifact"], zone: { zone: "graveyard", owner: "your" } },
      comparison: { op: "ge", value: fixed(2) },
    });
    for (const text of [
      "you have three cards in their graveyard", "that player has three cards in your library",
      "three creature cards were put into your graveyard", "three cards have left your graveyard",
      "three creature cards are in graveyard", "three cards in your hand are in your graveyard",
      "each opponent has three cards in their graveyard",
    ]) unsupported(text);
  });

  it("retains empty, exact, and bounded hand sizes", () => {
    // Academy at Tolaria West 15b979de-c8ee-4664-9ca7-6c4eb3346967.
    assert.deepEqual(condition("you have no cards in hand"), { condition: "cards-in-hand", who: you, comparison: { op: "eq", value: fixed(0) } });
    assert.deepEqual(condition("an opponent has exactly seven cards in hand"), {
      condition: "cards-in-hand", who: { player: "an-opponent" }, comparison: { op: "eq", value: fixed(7) },
    });
    assert.deepEqual(condition("you have three or fewer cards in hand"), {
      condition: "cards-in-hand", who: you, comparison: { op: "le", value: fixed(3) },
    });
    assert.deepEqual(condition("you have a card in hand"), {
      condition: "cards-in-hand", who: you, comparison: { op: "ge", value: fixed(1) },
    });
    assert.deepEqual(condition("you have at least three cards in your hand"), {
      condition: "cards-in-hand", who: you, comparison: { op: "ge", value: fixed(3) },
    });
    unsupported("you had no cards in hand at the beginning of this turn");
    unsupported("an opponent has three cards in your hand");
    unsupported("you have three cards in exile");
  });

  it("retains zero versus exact and minimum controlled object counts", () => {
    // Stenchskipper 0817d902-08c8-4ec3-9a84-2de65bc70e6a.
    assert.deepEqual(condition("you control no Goblins"), {
      condition: "control-count", who: you, of: { subtypes: ["Goblin"] }, comparison: { op: "eq", value: fixed(0) },
    });
    assert.deepEqual(condition("you control exactly three creatures"), {
      condition: "control-count", who: you, of: { types: ["creature"] }, comparison: { op: "eq", value: fixed(3) },
    });
    assert.deepEqual(condition("you control at least three creatures"), {
      condition: "control-count", who: you, of: { types: ["creature"] }, comparison: { op: "ge", value: fixed(3) },
    });
    assert.deepEqual(condition("there are at least seven cards in your graveyard"), {
      condition: "count", of: { cls: "card", zone: { zone: "graveyard", owner: "your" } }, comparison: { op: "ge", value: fixed(7) },
    });
    assert.deepEqual(condition("you control no Goblins other than this creature"), {
      condition: "control-count", who: you, of: { subtypes: ["Goblin"], otherThan: thisCreature },
      comparison: { op: "eq", value: fixed(0) },
    });
    unsupported("there are exactly seven cards of an unknown type");
  });
});

describe("historical conditions retain event and time scope", () => {
  it("keeps gross life gained separate from life lost and current life total", () => {
    // The Book of Exalted Deeds 046add85-952f-46ac-974a-463d2a7d3fd4.
    assert.deepEqual(condition("you gained 3 or more life this turn"), {
      condition: "life-change", who: you, change: "gained", comparison: { op: "ge", value: fixed(3) }, during: "this-turn",
    });
    assert.deepEqual(condition("an opponent lost life this turn"), {
      condition: "life-change", who: { player: "an-opponent" }, change: "lost", comparison: { op: "gt", value: fixed(0) }, during: "this-turn",
    });
    assert.deepEqual(condition("you have 3 or more life"), {
      condition: "life-total", who: you, comparison: { op: "ge", value: fixed(3) },
    });
    // Starlit Soothsayer 0c0940c6-8126-4d58-809e-871cf8588ef9.
    assert.deepEqual(condition("you gained or lost life this turn"), {
      condition: "any-of", conditions: [
        { condition: "life-change", who: you, change: "gained", comparison: { op: "gt", value: fixed(0) }, during: "this-turn" },
        { condition: "life-change", who: you, change: "lost", comparison: { op: "gt", value: fixed(0) }, during: "this-turn" },
      ],
    });
    // The previous turn is now a represented lookback window.
    assert.deepEqual(condition("you gained life last turn"), {
      condition: "life-change", who: you, change: "gained", comparison: { op: "gt", value: fixed(0) }, during: "last-turn",
    });
    unsupported("you gained 3 or more life");
    // "your team" (CR 102.4) is now a represented player reference.
    assert.deepEqual(condition("your team gained life this turn"), {
      condition: "life-change", who: { player: "your-team" }, change: "gained", comparison: { op: "gt", value: fixed(0) }, during: "this-turn",
    });
  });

  it("retains spell characteristics, counts, and the historical casting zone", () => {
    // H.E.R.B.I.E., Lovable Robot 064a2f4a-ff3f-4622-8727-24b22965416d;
    // Loan Shark 15030c1d-7c0a-4273-a022-eb98d6c7a1ce.
    assert.deepEqual(condition("you've cast a noncreature spell this turn"), {
      condition: "cast-count", who: you, what: { nonTypes: ["creature"], cls: "spell" }, comparison: { op: "ge", value: fixed(1) }, during: "this-turn",
    });
    assert.deepEqual(condition("you've cast two or more spells this turn"), {
      condition: "cast-count", who: you, what: { cls: "spell" }, comparison: { op: "ge", value: fixed(2) }, during: "this-turn",
    });
    // Wrangler of the Damned 0d8fded4-0393-43c2-82a1-383f29de65ce.
    assert.deepEqual(condition("you haven't cast a spell from your hand this turn"), { condition: "not", of: {
      condition: "cast-count", who: you, what: { cls: "spell" }, comparison: { op: "ge", value: fixed(1) }, during: "this-turn", from: { zone: "hand", owner: "your" },
    } });
    assert.deepEqual(condition("you've cast no spells this turn"), {
      condition: "cast-count", who: you, what: { cls: "spell" }, comparison: { op: "eq", value: fixed(0) }, during: "this-turn",
    });
    unsupported("you've cast a spell in your hand this turn");
    assert.deepEqual(condition("you've cast a spell last turn"), {
      condition: "cast-count", who: you, what: { cls: "spell" }, comparison: { op: "ge", value: fixed(1) }, during: "last-turn",
    });
    unsupported("you've cast two or more spells");
  });

  it("does not merge a current object state with a previous state", () => {
    assert.deepEqual(condition("it is a Human"), { condition: "is-filter", what: itRef, filter: { subtypes: ["Human"] } });
    assert.deepEqual(condition("it was a Human"), { condition: "is-filter", what: itRef, filter: { subtypes: ["Human"] }, time: "previous" });
  });
});

describe("boolean condition structure", () => {
  it("preserves AND/OR between complete clauses and alternatives inside a filter", () => {
    // Mogg Salvage 0923ad85-2ea4-4463-8f71-b7d0a162b486.
    const island: Condition = { condition: "control", who: { player: "an-opponent" }, what: {
      ref: "filter", filter: { subtypes: ["Island"] }, count: fixed(1),
    } };
    const mountain: Condition = { condition: "control", who: you, what: {
      ref: "filter", filter: { subtypes: ["Mountain"] }, count: fixed(1),
    } };
    assert.deepEqual(condition("an opponent controls an Island and you control a Mountain"), {
      condition: "all-of", conditions: [island, mountain],
    });
    assert.deepEqual(condition("an opponent controls an Island or if you control a Mountain"), {
      condition: "any-of", conditions: [island, mountain],
    });
    assert.deepEqual(condition("you control a Plains or an Island"), {
      condition: "control", who: you, what: { ref: "filter", count: fixed(1), filter: {
        anyOf: [{ subtypes: ["Plains"] }, { subtypes: ["Island"] }],
      } },
    });
  });

  it("does not consume activation-rule conjunctions as boolean condition clauses", () => {
    const result = parseOracleText("{W}{W}: Draw a card. Activate only if you control no creatures and only once each turn.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "mana", symbols: ["W", "W"] }],
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) }] }], restriction: {
        restriction: "all-of", restrictions: [
          { restriction: "condition", condition: { condition: "control-count", who: you, of: { types: ["creature"] }, comparison: { op: "eq", value: fixed(0) } } },
          { restriction: "once-each-turn" },
        ],
      },
    }]);
  });

  it("rejects mixed boolean scope and backtracks on a malformed atom", () => {
    for (const text of ["you control a Plains or you control an Island and you control a Swamp", "it was kicked with its {W}", "you've cast a spell during an unknown turn"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseCondition(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    unsupported("you control a Plains and");
    unsupported("you control a Plains or unknown condition");
  });
});
