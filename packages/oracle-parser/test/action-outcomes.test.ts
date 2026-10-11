import assert from "node:assert/strict";
import { it } from "node:test";
import type { Condition } from "../src/ast.ts";
import { lex, parseOracleText, analyzeCardBindings } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";

// Oracle fixtures from frozen SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// Risen Reef: https://magic.wizards.com/en/news/feature/core-set-2020-release-notes-2019-06-25
// Inscribed Tablet: https://media.wizards.com/2022/downloads/DMU_Release_Notes/EN_MTGDMU_ReleaseNotes_20220825_Final.pdf
const you = { player: "you" } as const;
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const itself = { ref: "it" } as const;
const battlefield = { zone: "battlefield" } as const;
const hand = { zone: "hand", owner: "your" } as const;
const library = { zone: "library", owner: "your" } as const;
const card = { cls: "card" } as const;
const condition = (text: string): Condition => {
  const c = new Cursor(lex(text, "fixture"));
  const parsed = parseCondition(c);
  assert.ok(parsed && c.done(), `${text}: ${c.errorMessage()}`);
  return parsed;
};
const abilities = (text: string) => {
  const result = parseOracleText(text, undefined, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return JSON.parse(JSON.stringify(result.abilities));
};
const handOutcome: Condition = { condition: "put-action-history", who: you,
  subject: { match: "filter", filter: card, comparison: { op: "ge", value: fixed(1) } }, to: hand, scope: "this-way" };

it("distinguishes specific objects, definite descriptions, and existential moved-card queries", () => {
  assert.deepEqual(condition("you don't put it onto the battlefield"), { condition: "not", of: {
    condition: "put-action-history", who: you, subject: { match: "object", what: itself }, to: battlefield, scope: "contextual",
  } });
  assert.deepEqual(condition("you don't put the card onto the battlefield"), { condition: "not", of: {
    condition: "put-action-history", who: you, subject: { match: "definite-description", filter: card }, to: battlefield, scope: "contextual",
  } });
  for (const verb of ["didn't", "don't", "do not", "did not", "have not"])
    assert.deepEqual(condition(`you ${verb} put a card into your hand this way`), { condition: "not", of: handOutcome });
  assert.deepEqual(condition("you put a card into your hand this way"), handOutcome);
  assert.deepEqual(condition("that player doesn't put it onto the battlefield this way"), { condition: "not", of: {
    condition: "put-action-history", who: { player: "that-player" }, subject: { match: "object", what: itself }, to: battlefield, scope: "this-way",
  } });
});

it("keeps named movement outcomes distinct from cost decisions and search performance", () => {
  assert.deepEqual(condition("you don't"), { condition: "you-dont" });
  assert.deepEqual(condition("you do"), { condition: "you-do" });
  assert.deepEqual(condition("you searched your library this way"), {
    condition: "searched-zone", who: you, zone: library, scope: "this-way",
  });
  assert.notDeepEqual(condition("you don't"), condition("you don't put a card into your hand this way"));
});

it("preserves Risen Reef's independent fallback after the conditional optional move", () => {
  assert.deepEqual(abilities("Whenever this creature or another Elemental you control enters, look at the top card of your library. If it's a land card, you may put it onto the battlefield tapped. If you don't put the card onto the battlefield, put it into your hand."), [{
    kind: "triggered", trigger: { trigger: "any-of", events: [
      { trigger: "enters", what: { ref: "this", noun: "creature" } },
      { trigger: "enters", what: { ref: "filter", filter: { other: true, subtypes: ["Elemental"], control: "you" }, count: fixed(1) } },
    ] }, effects: [
      { steps: [{ effect: "look-at", who: you, what: { top: fixed(1), of: library } }] },
      { steps: [{ effect: "move-zone", what: itself, to: battlefield, tapped: true, optional: true }],
        condition: { condition: "is-filter", what: itself, filter: { types: ["land"], cls: "card" } }, conditionKind: "if" },
      { steps: [{ effect: "move-zone", what: itself, to: hand }], conditionKind: "if", condition: { condition: "not", of: {
        condition: "put-action-history", who: you, subject: { match: "definite-description", filter: card }, to: battlefield, scope: "contextual",
      } } },
    ],
  }]);
});

it("retains Inscribed Tablet's mandatory extraction, rest order, and actual hand-movement check", () => {
  assert.deepEqual(abilities("{1}, {T}, Sacrifice this artifact: Reveal the top five cards of your library. Put a land card from among them into your hand and the rest on the bottom of your library in a random order. If you didn't put a card into your hand this way, draw a card."), [{
    kind: "activated", costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "artifact" } }],
    effects: [
      { steps: [{ effect: "reveal", who: you, what: { ref: "library-cards", position: "top", count: fixed(5), of: library } }] },
      { steps: [{ effect: "simultaneous", steps: [
        { effect: "move-zone", what: { ref: "filter", filter: { types: ["land"], cls: "card", fromAmong: { ref: "them" } }, count: fixed(1) }, to: hand },
        { effect: "move-zone", what: { ref: "rest" }, to: { ...library, position: "bottom" }, order: "random" },
      ] }] },
      { steps: [{ effect: "draw", who: you, amount: fixed(1) }], condition: { condition: "not", of: handOutcome }, conditionKind: "if" },
    ],
  }]);
});

it("allows Courier's fallback even when the chosen search finds nothing", () => {
  assert.deepEqual(abilities("When this creature enters, you may search your library for a Food card, reveal it, put it into your hand, then shuffle. If you don't put a card into your hand this way, create a Food token."), [{
    kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } }, effects: [
      { steps: [{ effect: "optional-group", who: you, steps: [
        { effect: "search", who: you, zones: { selection: "all", of: [library] }, for: { ref: "filter", filter: { subtypes: ["Food"], cls: "card" }, count: fixed(1) } },
        { effect: "reveal", who: you, what: itself }, { effect: "move-zone", what: itself, to: hand },
        { effect: "shuffle", who: you, library: { zone: "library", owner: you } },
      ] }] },
      { steps: [{ effect: "create-token", count: fixed(1), token: { predefined: "Food" } }], condition: { condition: "not", of: handOutcome }, conditionKind: "if" },
    ],
  }]);
});

it("preserves negative cast counts over this turn without confusing originating-spell history", () => {
  for (const negative of ["didn't", "did not", "haven't", "have not"])
    assert.deepEqual(condition(`you ${negative} cast a creature spell this turn`), { condition: "not", of: {
      condition: "cast-count", who: you, what: { types: ["creature"], cls: "spell" }, comparison: { op: "ge", value: fixed(1) }, during: "this-turn",
    } });
  assert.deepEqual(condition("you didn't cast it from your hand"), { condition: "not", of: {
    condition: "was-cast", what: itself, by: you, from: hand,
  } });
  assert.deepEqual(condition("that player didn't cast a spell this turn"), { condition: "not", of: {
    condition: "cast-count", who: { player: "that-player" }, what: { cls: "spell" }, comparison: { op: "ge", value: fixed(1) }, during: "this-turn",
  } });
});

it("rejects unsupported histories, fresh condition targets, and present-tense unless payments", () => {
  for (const clause of [
    "you didn't put two cards into your hand this way", "you didn't put a card into your hand this turn",
    "you don't put target card onto the battlefield", "target opponent didn't put a card into their hand this way",
    "you don't put a card into target opponent's hand this way", "you don't put a card from your graveyard into your hand this way",
    "you didn't put a card into your hand this way with a counter on it", "you put a card into your hand",
  ]) assert.equal(parseOracleText(`If ${clause}, draw a card.`).ok, false, clause);
  assert.equal(parseOracleText("Draw a card unless you put a card into your hand.").ok, false);
  assert.equal(parseOracleText("Draw a card unless you don't put a card into your hand.").ok, false);
  const c = new Cursor(lex("you put a card into your hand"));
  const result = parseCondition(c);
  assert.equal(result, null);
  assert.equal(c.pos, 0);
});

it("reports unresolved event linkage instead of guessing the preceding movement", () => {
  const parsed = parseOracleText("Look at the top card of your library. If you don't put the card onto the battlefield, put it into your hand.", "Fixture");
  assert.ok(parsed.ok);
  const report = analyzeCardBindings({ identity: "outcome-fixture", source: { name: "Fixture", layout: "normal", type_line: "Instant" },
    faces: [{ sourceFaceIndex: 0, result: parsed }] });
  assert.equal(report.fullyBound, false);
  assert.ok(report.diagnostics.some(d => d.message.includes("named movement outcome")));
});
