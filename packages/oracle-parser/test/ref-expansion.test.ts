import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parseComparison, parseCondition, parseDuration, parseFilter, parseObjectRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/index.ts";

// Card facts: Scryfall oracle-cards.json SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 122.1, 107.2, 611.2a, 511.2, 700.9, 701.15b, 701.60b, 702.1:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens`);
  return result;
}
function incomplete<T>(parser: (c: Cursor) => T | null, text: string): void {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result === null || !c.done(), `must reject: ${text}`);
  if (result === null) assert.equal(c.pos, 0, "null restores cursor");
}

describe("counter amounts and qualifications", () => {
  it("retains counter kind and precise object or player", () => {
    // Torture Chamber 12436f2f-eaed-4b46-8570-81316ce15962.
    assert.deepEqual(complete(parseAmount, "the number of pain counters on it"), {
      amount: "counter-count", counter: { type: "pain" }, on: { ref: "it" },
    });
    assert.deepEqual(complete(parseAmount, "the number of +1/+1 counters on this creature"), {
      amount: "counter-count", counter: { type: "+1/+1" }, on: { ref: "this", noun: "creature" },
    });
    assert.deepEqual(complete(parseAmount, "the number of counters on you"), {
      amount: "counter-count", counter: "any", on: { player: "you" },
    });
    incomplete(parseAmount, "the number of different kinds of counters on it");
  });

  it("distinguishes presence, absence, and explicit count bounds", () => {
    // Cytoplast Manipulator 026ef98e-4f0a-4a6d-b74f-d17ac8b10e37.
    assert.deepEqual(complete(parseObjectRef, "target creature with a +1/+1 counter on it"), {
      ref: "target", count: fixed(1), filter: { types: ["creature"], counterConstraints: [
        { counter: { type: "+1/+1" }, comparison: { op: "ge", value: fixed(1) } },
      ] },
    });
    assert.deepEqual(complete(parseFilter, "creatures without counters on them"), {
      types: ["creature"], counterConstraints: [{ counter: "any", comparison: { op: "eq", value: fixed(0) } }],
    });
    assert.deepEqual(complete(parseFilter, "creatures with three or more charge counters on them"), {
      types: ["creature"], counterConstraints: [{ counter: { type: "charge" }, comparison: { op: "ge", value: fixed(3) } }],
    });
    incomplete(parseFilter, "creatures with an odd number of counters on them");
    incomplete(parseFilter, "creatures with a charge counter on another permanent");
    assert.deepEqual(complete(parseFilter, "creatures with any counters"), {
      types: ["creature"], counterConstraints: [{ counter: "any", comparison: { op: "ge", value: fixed(1) } }],
    });
    assert.deepEqual(complete(parseFilter, "creatures with at least three counters"), {
      types: ["creature"], counterConstraints: [{ counter: "any", comparison: { op: "ge", value: fixed(3) } }],
    });
    incomplete(parseFilter, "creatures with at least counters");
    incomplete(parseFilter, "creatures without two counters");
    incomplete(parseFilter, "creatures without 2 charge counters");
  });

  it("parses the full Skyclave Shadowcat trigger without losing controller or counter requirement", () => {
    // 043ded41-2506-4dab-91c4-142d9287e7d7.
    const result = parseOracleText("Whenever a creature you control with a +1/+1 counter on it dies, draw a card.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "dies", what: { ref: "filter", count: fixed(1), filter: {
        types: ["creature"], control: "you", counterConstraints: [
          { counter: { type: "+1/+1" }, comparison: { op: "ge", value: fixed(1) } },
        ],
      } } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
    }]);
  });
});

describe("extremum values and property bounds", () => {
  it("keeps a symbolic extremum with all eligible-set qualifiers", () => {
    // Torrent of Fire 00181784-9213-492a-ba8c-2028969b049e.
    assert.deepEqual(complete(parseAmount, "the greatest mana value among permanents you control"), {
      amount: "attribute-extremum", extremum: "maximum", attribute: "mana-value", among: { cls: "permanent", control: "you" },
    });
    assert.deepEqual(complete(parseAmount, "the least toughness among creatures you control"), {
      amount: "attribute-extremum", extremum: "minimum", attribute: "toughness", among: { types: ["creature"], control: "you" },
    });
    // Pools in different zones cannot overlap, so the extremum ranges over their union (pooled-counts.ts).
    assert.deepEqual(complete(parseAmount, "the greatest power among creatures you control and creature cards in your graveyard"), {
      amount: "attribute-extremum", extremum: "maximum", attribute: "power",
      among: { anyOf: [{ types: ["creature"], control: "you" },
        { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }] },
    });
  });

  it("preserves comparison operators and tied greatest-value eligibility", () => {
    assert.deepEqual(complete(parseComparison, "less than or equal to its power"), {
      op: "le", value: { amount: "attribute", of: { ref: "it" }, attribute: "power" },
    });
    assert.deepEqual(complete(parseFilter, "creature that has power greater than or equal to 3"), {
      types: ["creature"], properties: [{ property: "power", comparison: { op: "ge", value: fixed(3) } }],
    });
    // Will of the Abzan 1ae29791-aa7c-4050-bf72-dd0f739b11b8.
    assert.deepEqual(complete(parseFilter, "creature with the greatest power among creatures that player controls"), {
      types: ["creature"], properties: [{ property: "power", comparison: { op: "eq", value: {
        amount: "attribute-extremum", extremum: "maximum", attribute: "power", among: { types: ["creature"], control: "that-player" },
      } } }],
      // The chosen creature is one of that pool, not any creature with that power.
      allOf: [{ types: ["creature"], control: "that-player" }],
    });
    // A comparison with the extremum states no membership (Cosmic Cube).
    assert.deepEqual(complete(parseFilter, "spell with mana value less than or equal to the greatest power among creatures you control"), {
      cls: "spell", properties: [{ property: "mana-value", comparison: { op: "le", value: {
        amount: "attribute-extremum", extremum: "maximum", attribute: "power", among: { types: ["creature"], control: "you" },
      } } }],
    });
  });
});

describe("keyword and repeated filter constraints", () => {
  it("distinguishes conjunctive abilities, alternatives, and absence", () => {
    assert.deepEqual(complete(parseFilter, "creatures with first strike and haste"), { types: ["creature"], withAbility: ["first strike", "haste"] });
    assert.deepEqual(complete(parseFilter, "creatures with flying or reach"), { types: ["creature"], anyOf: [{ withAbility: ["flying"] }, { withAbility: ["reach"] }] });
    assert.deepEqual(complete(parseFilter, "creatures without flying or reach"), { types: ["creature"], withoutAbility: ["flying", "reach"] });
    assert.deepEqual(complete(parseFilter, "cards with cycling"), { cls: "card", withAbility: ["cycling"] });
    incomplete(parseFilter, "creatures with superflight");
    incomplete(parseFilter, "creatures with flying and reach or haste");
    incomplete(parseFilter, "creatures without flying and reach");
    incomplete(parseFilter, "creatures with ward {2}");
  });

  it("retains all repeated qualifiers rather than overwriting fields", () => {
    assert.deepEqual(complete(parseFilter, "creatures you control and that player controls"), {
      types: ["creature"], control: "you", allOf: [{ control: "that-player" }],
    });
    // No object is in two zones at once: the zones are alternatives (Crackling Drake).
    assert.deepEqual(complete(parseFilter, "cards in your graveyard and in exile"), {
      cls: "card", allOf: [{ anyOf: [{ zone: { zone: "graveyard", owner: "your" } }, { zone: { zone: "exile", owner: undefined } }] }],
    });
    assert.deepEqual(complete(parseFilter, "creatures with flying and with power 3 or less"), {
      types: ["creature"], withAbility: ["flying"], properties: [{ property: "power", comparison: { op: "le", value: fixed(3) } }],
    });
    const c = new Cursor(lex("creatures with flying and draw a card"));
    assert.deepEqual(parseFilter(c), { types: ["creature"], withAbility: ["flying"] });
    assert.equal(c.peek()?.kind, "word");
    assert.equal(c.isWord("and"), true, "effect conjunction stays unconsumed");
  });

  it("retains independent qualifiers on explicit repeated OR clauses", () => {
    assert.deepEqual(complete(parseFilter, "creatures you control or artifacts an opponent controls"), { anyOf: [
      { types: ["creature"], control: "you" }, { types: ["artifact"], control: "opponent" },
    ] });
    assert.deepEqual(complete(parseFilter, "instant or sorcery cards in your graveyard"), {
      allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card", zone: { zone: "graveyard", owner: "your" },
    });
  });

  it("keeps targeted controllers distinct from generic opponents", () => {
    assert.deepEqual(complete(parseFilter, "creatures target opponent controls"), {
      types: ["creature"], controlledBy: { player: "target-opponent" },
    });
    assert.deepEqual(complete(parseFilter, "creatures they control"), {
      types: ["creature"], controlledBy: { player: "they" },
    });
  });

  it("uses defined designations and OR for alternative combat statuses", () => {
    for (const status of ["modified", "goaded", "suspected"]) {
      assert.deepEqual(complete(parseFilter, `${status} creatures`), { status: [status], types: ["creature"] });
    }
    assert.deepEqual(complete(parseObjectRef, "any number of target attacking or blocking creatures without flying"), {
      ref: "target", count: { amount: "chosen", minimum: 0 }, filter: {
        allOf: [{ anyOf: [{ status: ["attacking"] }, { status: ["blocking"] }] }], types: ["creature"], withoutAbility: ["flying"],
      },
    });
    incomplete(parseFilter, "Imaginary creatures");
  });
});

describe("target choice counts and duration boundaries", () => {
  it("retains present and previous property predicates on the actual subject", () => {
    // Strider, Ranger of the North 118f2cb6-2af9-4acc-b386-a8e8dded5793.
    assert.deepEqual(complete(parseCondition, "that creature has power 4 or greater"), {
      condition: "is-filter", what: { ref: "that", filter: { types: ["creature"] } },
      filter: { properties: [{ property: "power", comparison: { op: "ge", value: fixed(4) } }] },
    });
    assert.deepEqual(complete(parseCondition, "it had power greater than ~'s power"), {
      condition: "is-filter", what: { ref: "it" }, time: "previous", filter: { properties: [
        { property: "power", comparison: { op: "gt", value: { amount: "attribute", of: { ref: "self" }, attribute: "power" } } },
      ] },
    });
    // Veldt 10b9bfb3-c478-45c6-b227-9c66b63bc79b.
    assert.deepEqual(complete(parseCondition, "it has a depletion counter on it"), {
      condition: "is-filter", what: { ref: "it" }, filter: { counterConstraints: [
        { counter: { type: "depletion" }, comparison: { op: "ge", value: fixed(1) } },
      ] },
    });
    incomplete(parseCondition, "it has counters on another creature");
  });

  it("retains status and zone conditions on ongoing durations", () => {
    // Kill Switch 0cd5eef6-6830-4c6d-8578-2726900c39a5.
    assert.deepEqual(complete(parseDuration, "for as long as this artifact remains tapped"), {
      duration: "as-long-as", condition: { condition: "status", what: { ref: "this", noun: "artifact" }, status: "tapped" },
    });
    // Brainstealer Dragon 13cb958f-1e66-4219-901a-ff58275c8475.
    assert.deepEqual(complete(parseDuration, "for as long as they remain exiled"), {
      duration: "as-long-as", condition: { condition: "remains", what: { ref: "them" }, zone: { zone: "exile" } },
    });
    assert.deepEqual(complete(parseDuration, "for as long as it remains exiled"), {
      duration: "as-long-as", condition: { condition: "remains", what: { ref: "it" }, zone: { zone: "exile" } },
    });
    incomplete(parseDuration, "for as long as it remains Imaginary");
  });

  it("keeps chosen target counts and filter bounds", () => {
    // Arc Mage 011bb6cc-f1a1-4790-befc-373acabf63b8, Rock Slide
    // 0b2e7c1a-dc07-4138-a560-dc80fa723860.
    assert.deepEqual(complete(parseObjectRef, "one or two targets"), { ref: "any-target", count: { amount: "chosen", minimum: 1, maximum: fixed(2) } });
    assert.deepEqual(complete(parseObjectRef, "any number of targets"), { ref: "any-target", count: { amount: "chosen", minimum: 0 } });
    assert.deepEqual(complete(parseObjectRef, "one or two target creatures"), { ref: "target", count: { amount: "chosen", minimum: 1, maximum: fixed(2) }, filter: { types: ["creature"] } });
    incomplete(parseObjectRef, "one or three targets");
  });

  it("distinguishes next-turn cleanup, next end-step beginning, and combat end", () => {
    assert.deepEqual(complete(parseDuration, "until end of your next turn"), { duration: "end-of-your-next-turn" });
    assert.deepEqual(complete(parseDuration, "until the end of your next turn"), { duration: "end-of-your-next-turn" });
    assert.deepEqual(complete(parseDuration, "until your next end step"), { duration: "your-next-end-step" });
    assert.deepEqual(complete(parseDuration, "until the beginning of your next end step"), { duration: "your-next-end-step" });
    assert.deepEqual(complete(parseDuration, "until end of combat"), { duration: "end-of-combat" });
    assert.deepEqual(complete(parseDuration, "until your next turn"), { duration: "your-next-turn" });
    incomplete(parseDuration, "until the end of your next end step");
  });
});
