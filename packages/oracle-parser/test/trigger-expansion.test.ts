import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import type { ObjectRef, Trigger } from "../src/ast.ts";

// Oracle clauses below were checked against the cached Scryfall oracle corpus
// SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// Rules: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// CR 603.1–2, 603.6, 603.7, 702.29, 702.37, 702.140, 701.27.
const thisCreature: ObjectRef = { ref: "this", noun: "creature" };
const fixedOne = { amount: "fixed", value: 1 } as const;

function trigger(text: string): Trigger {
  const c = new Cursor(lex(text));
  const result = parseTrigger(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens at ${c.pos}`);
  return result;
}

function unsupported(text: string): void {
  const result = parseOracleText(`${text}, draw a card.`);
  assert.equal(result.ok, false, text);
}

describe("trigger semantic preservation", () => {
  it("keeps the source's attached object distinct from objects with attachment status", () => {
    // Slayer's Plate 05004b4a-2e2c-4a88-a994-8576684fb64a and Avacyn's Collar
    // 842e3835-fff3-4684-bfd1-e7064a1fd2e6 use 'equipped creature dies'.
    assert.deepEqual(trigger("Whenever equipped creature dies"), {
      trigger: "dies", what: { ref: "equipped", noun: "creature" },
    });
    assert.deepEqual(trigger("Whenever enchanted creature dies"), {
      trigger: "dies", what: { ref: "enchanted", noun: "creature" },
    });
    assert.deepEqual(trigger("Whenever an equipped creature dies"), {
      trigger: "dies", what: { ref: "filter", filter: { status: ["equipped"], types: ["creature"] }, count: fixedOne },
    });
  });

  it("retains entering-controller qualifiers for specific objects without overwriting filters", () => {
    assert.deepEqual(trigger("When this creature enters under your control"), {
      trigger: "enters", what: thisCreature, controller: { player: "you" },
    });
    assert.deepEqual(trigger("Whenever a creature an opponent controls enters under your control"), {
      trigger: "enters", what: { ref: "filter", filter: { types: ["creature"], control: "opponent" }, count: fixedOne },
      controller: { player: "you" },
    });
    assert.deepEqual(trigger("When this creature enters under your control or dies"), {
      trigger: "any-of", events: [
        { trigger: "enters", what: thisCreature, controller: { player: "you" } },
        { trigger: "dies", what: thisCreature },
      ],
    });
    unsupported("When this creature enters under your");
  });

  it("does not erase delayed-event or first-occurrence qualifiers", () => {
    unsupported("At the beginning of next turn");
    // No longer rejected. The "next" qualifier is kept as a delayed
    // trigger for that one occurrence created by a spell instruction (CR
    // 603.7b; Pact of Negation), never an ordinary recurring upkeep trigger.
    const next = parseOracleText("At the beginning of your next upkeep, draw a card.");
    assert.equal(next.ok, true);
    assert.deepEqual(JSON.parse(JSON.stringify(next.ok ? next.abilities : null)), [{ kind: "spell", effects: [{ steps: [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability",
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixedOne }] }],
    }] }] }]);
    assert.equal(parseOracleText("At the beginning of your next upkeep, draw a card.", undefined, { typeLine: "Creature — Elf" }).ok, false);
    unsupported("At the beginning of your first upkeep");
  });

  it("preserves singular versus collective damage events", () => {
    // Professional Face-Breaker 04152e7a-969c-4858-841b-0a569a9fc1bf.
    const what: ObjectRef = { ref: "filter", filter: { types: ["creature"], control: "you" }, count: fixedOne };
    const event: Trigger = { trigger: "deals-damage", what, combat: true, to: "any" };
    assert.deepEqual(trigger("Whenever a creature you control deals combat damage to a player"), event);
    assert.deepEqual(trigger("Whenever one or more creatures you control deal combat damage to a player"), {
      ...event, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more creatures you control deal damage to a player"), {
      ...event, combat: undefined, grouping: "one-or-more",
    });
  });

  it("retains the endpoints and grouping of plural graveyard events", () => {
    const what: ObjectRef = { ref: "filter", filter: { types: ["land"], cls: "card" }, count: fixedOne };
    assert.deepEqual(trigger("Whenever one or more land cards are put into your graveyard from anywhere"), {
      trigger: "put-into-graveyard", what, from: "anywhere", to: { zone: "graveyard", owner: "your" }, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more land cards are put into your graveyard from your library"), {
      trigger: "put-into-graveyard", what: { ref: "filter", filter: { types: ["land"], cls: "card" }, count: { amount: "fixed", value: 1 } },
      grouping: "one-or-more", from: { zone: "library", owner: "your" }, to: { zone: "graveyard", owner: "your" },
    });
  });
});

describe("structured trigger alternatives", () => {
  it("keeps mixed specific and filtered subjects separate", () => {
    // Midnight Entourage 02d6b7f6-1fe0-4a93-8385-69ab160ab732.
    const header = "Whenever this creature or another Aetherborn you control dies";
    assert.deepEqual(trigger(header), {
      trigger: "any-of", events: [
        { trigger: "dies", what: thisCreature },
        { trigger: "dies", what: { ref: "filter", filter: { other: true, subtypes: ["Aetherborn"], control: "you" }, count: fixedOne } },
      ],
    });
    const result = parseOracleText(`${header}, you draw a card and you lose 1 life.`);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: trigger(header), effects: [{ steps: [
        { effect: "draw", who: { player: "you" }, amount: fixedOne },
        { effect: "lose-life", who: { player: "you" }, amount: fixedOne },
      ] }],
    }]);
  });

  it("retains attack/die alternatives and independently introduced trigger clauses", () => {
    assert.deepEqual(trigger("Whenever this creature attacks or dies"), {
      trigger: "any-of", events: [
        { trigger: "attacks", what: thisCreature, alone: undefined, whom: undefined },
        { trigger: "dies", what: thisCreature },
      ],
    });
    // Up the Beanstalk 050f5733-7c0b-4991-9a6c-7ea12ccf0ca9: both clauses
    // belong to the same triggered ability; one is not an effect condition.
    assert.deepEqual(trigger("When this enchantment enters and whenever you cast a spell with mana value 5 or greater"), {
      trigger: "any-of", events: [
        { trigger: "enters", what: { ref: "this", noun: "enchantment" } },
        { trigger: "cast", who: { player: "you" }, what: {
          cls: "spell", properties: [{ property: "mana-value", comparison: { op: "ge", value: { amount: "fixed", value: 5 } } }],
        } },
      ],
    });
    // CR 509.3g: now its own declare-blockers event (see trigger-combat-and-origins.test.ts).
    assert.deepEqual(trigger("Whenever this creature attacks and isn't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: thisCreature,
    });
    unsupported("Whenever this creature dies or");
    unsupported("Whenever this creature or one or more creatures die");
  });

  it("preserves the opposing object for blocks-or-becomes-blocked triggers", () => {
    const other: ObjectRef = { ref: "filter", filter: { nonColors: ["black"], types: ["creature"] }, count: fixedOne };
    assert.deepEqual(trigger("Whenever this creature blocks or becomes blocked by a nonblack creature"), {
      trigger: "any-of", events: [
        { trigger: "blocks", what: thisCreature, blocking: other },
        { trigger: "becomes-blocked", what: thisCreature, by: other },
      ],
    });
    assert.deepEqual(trigger("Whenever this creature blocks or becomes blocked"), {
      trigger: "blocks", what: thisCreature, orBecomesBlocked: true,
    });
    unsupported("Whenever this creature blocks or becomes blocked by");
  });

  it("distinguishes casts from copies and preserves their common filter", () => {
    const what = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" };
    assert.deepEqual(trigger("Whenever you cast or copy an instant or sorcery spell"), {
      trigger: "any-of", events: [
        { trigger: "cast", who: { player: "you" }, what },
        { trigger: "copy", who: { player: "you" }, what },
      ],
    });
    unsupported("Whenever you cast or copy");
  });

  it("retains ordinal matching events and their turn restrictions", () => {
    // Sunstar Lightsmith 132de6c2-4ca2-41da-9c78-caa59838b641;
    // Lat-Nam Adept 0d5a1870-6d26-47e7-a86c-f3ff15cb0707;
    // Dreamstalker Manticore 1ecb3349-ccb8-4b54-bd81-8d649d6f96f0.
    assert.deepEqual(trigger("Whenever you cast your second spell each turn"), {
      trigger: "cast", who: { player: "you" }, what: { cls: "spell" }, occurrence: { ordinal: 2, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever you draw your second card each turn"), {
      trigger: "draws", who: { player: "you" }, occurrence: { ordinal: 2, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever you cast your first spell during each opponent's turn"), {
      trigger: "cast", who: { player: "you" }, what: { cls: "spell" },
      occurrence: { ordinal: 1, period: "turn" }, during: "each-opponent",
    });
    unsupported("Whenever you cast your second spell");
    unsupported("Whenever you draw your second card each game");
    unsupported("Whenever an opponent casts your first spell each turn");
    unsupported("Whenever you cast your second spell during each opponent's upkeep");
  });

  it("distinguishes cycling this card, another card, and cycling-or-discarding", () => {
    assert.deepEqual(trigger("When you cycle this card"), {
      trigger: "cycle", who: { player: "you" }, what: { ref: "this", noun: "card" },
    });
    // Drannith Healer 13e2a2c3-5ab5-4de4-96ca-5f9c72137000.
    const another: ObjectRef = { ref: "filter", filter: { other: true, cls: "card" }, count: fixedOne };
    assert.deepEqual(trigger("Whenever you cycle another card"), {
      trigger: "cycle", who: { player: "you" }, what: another,
    });
    assert.deepEqual(trigger("Whenever you cycle or discard another card"), {
      trigger: "any-of", events: [
        { trigger: "cycle", who: { player: "you" }, what: another },
        { trigger: "discards", who: { player: "you" }, what: another },
      ],
    });
    unsupported("Whenever you cycle or discard");
  });

  it("preserves face-up, mutate, and transformation events", () => {
    assert.deepEqual(trigger("When this creature is turned face up"), { trigger: "turned-face-up", what: thisCreature });
    // Dreamtail Heron 1cadb1e1-19b6-4ce5-8eb5-7851cec7dfa3.
    assert.deepEqual(trigger("Whenever this creature mutates"), { trigger: "mutates", what: thisCreature });
    assert.deepEqual(trigger("Whenever this creature transforms"), { trigger: "transforms", what: thisCreature });
    assert.deepEqual(trigger("Whenever this creature transforms into ~"), {
      trigger: "transforms", what: thisCreature, into: { face: "source-face" },
    });
    unsupported("Whenever this creature transforms into");
    unsupported("Whenever this creature is turned face down");
  });

  it("backtracks after incomplete event alternatives", () => {
    for (const text of ["Whenever this creature or dies", "Whenever this creature attacks or", "Whenever you cast or copy", "When you cycle", "Whenever this creature transforms into"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseTrigger(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });
});
