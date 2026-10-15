import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, Condition, PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parseCondition, parsePlayerRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle clauses checked in the frozen corpus: Gray Merchant of Asphodel,
// Athreos, Children of Korlis, Wound Reflection, Tainted Sigil, Exsanguinate,
// Goblin Negotiation, Venerable Warsinger, Cradle of Vitality, False Cure,
// Righteous Valkyrie, Cosmos Elixir, Beza, Avatar of Might, Ward of Bones,
// Powder Keg and Goblin Chainwhirler.
// CR 103.4, 107.1, 119.1–119.4, 120.3–120.4, 120.10, 608.2c/h, 700.5:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const you: PlayerRef = { player: "you" };
function amount(text: string, expected: Amount) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseAmount(c), expected, text);
  assert.ok(c.done(), `${text}: ${c.errorMessage()}`);
}
function noAmount(text: string) {
  const c = new Cursor(lex(text));
  const parsed = parseAmount(c);
  assert.ok(parsed === null || !c.done(), `${text} must not be one complete amount`);
}
function condition(text: string, expected: Condition) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseCondition(c), expected, text);
  assert.ok(c.done(), `${text}: ${c.errorMessage()}`);
}
function noCondition(text: string) {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.ok(parsed === null || !c.done(), `${text} must not be one complete condition`);
}
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe("devotion counts mana symbols for one color or a color pair", () => {
  it("keeps the player and the exact colors", () => {
    amount("your devotion to black", { amount: "devotion", who: you, colors: ["black"] });
    amount("your devotion to white and black", { amount: "devotion", who: you, colors: ["white", "black"] });
    condition("your devotion to red and green is less than seven", { condition: "compare",
      value: { amount: "devotion", who: you, colors: ["red", "green"] }, comparison: { op: "lt", value: n(7) } });
  });
  it("is usable as an X definition without becoming an object count", () => {
    assert.deepEqual(parsed("Each opponent loses X life, where X is your devotion to black. You gain life equal to the life lost this way."), [{
      kind: "spell", effects: [
        { steps: [{ effect: "lose-life", who: { player: "each-opponent" }, amount: { amount: "x" } }],
          bindings: [{ variable: "x", value: { amount: "devotion", who: you, colors: ["black"] } }] },
        { steps: [{ effect: "gain-life", who: you, amount: { amount: "event-quantity", event: "life-change", change: "lost", scope: "this-way" } }] },
      ] }]);
  });
  it("rejects factions, hybrid, repeated and contextual colors", () => {
    for (const text of ["your devotion to Mardu", "your devotion to hybrid", "your devotion to that color",
      "your devotion to red and red", "each player's devotion to red", "your devotion"]) noAmount(text);
    assert.equal(parseOracleText("Draw cards equal to your devotion to that color.").ok, false);
  });
});

describe("life quantities keep who, direction and period or event", () => {
  it("separates turn history from an event result", () => {
    amount("the amount of life you gained this turn", { amount: "life-change", who: you, change: "gained", during: "this-turn" });
    amount("the life you've lost this turn", { amount: "life-change", who: you, change: "lost", during: "this-turn" });
    amount("the life they lost this turn", { amount: "life-change", who: { player: "they" }, change: "lost", during: "this-turn" });
    amount("the life that player lost this turn", { amount: "life-change", who: { player: "that-player" }, change: "lost", during: "this-turn" });
    amount("the total life lost by all players this turn", { amount: "life-change", who: "all-players-combined", change: "lost", during: "this-turn" });
    amount("the life lost this way", { amount: "event-quantity", event: "life-change", change: "lost", scope: "this-way" });
    amount("the amount of life you gained", { amount: "event-quantity", event: "life-change", change: "gained", who: you, scope: "contextual" });
  });
  it("scales one instruction by each 1 life of the referenced event", () => {
    assert.deepEqual(parsed("Whenever you lose life, you gain 2 life for each 1 life you lost."), [{
      kind: "triggered", trigger: { trigger: "loses-life", who: you },
      effects: [{ steps: [{ effect: "gain-life", who: you, amount: { amount: "product", factors: [n(2),
        { amount: "event-quantity", event: "life-change", change: "lost", who: you, scope: "contextual" }] } }] }] }]);
  });
  it("adds a group's changes together and rejects unrepresented scopes", () => {
    amount("the total amount of life your opponents have lost this turn", { amount: "life-change", who: { player: "each-opponent" }, change: "lost", during: "this-turn" });
    // CR 607.2g: the life paid by the linked "as this enters" payment.
    amount("the life paid as it entered", { amount: "entry-payment", of: { ref: "it" }, paid: "life" });
    for (const text of ["the life you lost when it entered",
      "the life lost", "the amount of life each opponent lost this turn"]) noAmount(text);
  });
});

describe("damage event quantities keep measure, participants and scope", () => {
  it("distinguishes dealt from excess damage and this-way from contextual events", () => {
    amount("the damage dealt this way", { amount: "event-quantity", event: "damage", measure: "dealt", scope: "this-way" });
    amount("the amount of excess damage dealt to that creature this way", { amount: "event-quantity", event: "damage", measure: "excess",
      to: { ref: "that", filter: { types: ["creature"] } }, scope: "this-way" });
    amount("the amount of damage this creature dealt to that player", { amount: "event-quantity", event: "damage", measure: "dealt",
      by: { ref: "this", noun: "creature" }, to: { player: "that-player" }, scope: "contextual" });
    amount("the damage prevented this way", { amount: "prevention-event-quantity", quantity: "prevented" });
  });
  it("rejects turn-history and source-restricted damage totals", () => {
    for (const text of ["the damage dealt to you this turn", "the amount of damage dealt to it this turn",
      "the damage dealt by a creature this way", "the excess damage", "the damage dealt to each opponent this way"]) noAmount(text);
  });
});

describe("starting life total and comparative conditions", () => {
  const startingPlus = (value: number): Amount => ({ amount: "plus", a: { amount: "starting-life-total", of: you }, b: n(value) });
  const yourLands: Amount = { amount: "count", of: { types: ["land"], control: "you" } };
  it("compares against the starting total, not a fixed number", () => {
    condition("your life total is greater than your starting life total", { condition: "compare",
      value: { amount: "life-total", of: you }, comparison: { op: "gt", value: { amount: "starting-life-total", of: you } } });
    condition("you have at least 7 life more than your starting life total", { condition: "life-total", who: you,
      comparison: { op: "ge", value: startingPlus(7) } });
  });
  it("measures the same thing for the reference player", () => {
    condition("an opponent controls more lands than you", { condition: "control-count", who: { player: "an-opponent" },
      of: { types: ["land"] }, comparison: { op: "gt", value: yourLands } });
    condition("an opponent controls at least four more creatures than you", { condition: "control-count", who: { player: "an-opponent" },
      of: { types: ["creature"] }, comparison: { op: "ge", value: { amount: "plus", a: { amount: "count", of: { types: ["creature"], control: "you" } }, b: n(4) } } });
    condition("they control fewer lands than you", { condition: "control-count", who: { player: "they" },
      of: { types: ["land"] }, comparison: { op: "lt", value: yourLands } });
    condition("you control more lands than defending player", { condition: "control-count", who: you, of: { types: ["land"] },
      comparison: { op: "gt", value: { amount: "count", of: { types: ["land"], controlledBy: { player: "defending-player" } } } } });
    condition("an opponent has more life than you", { condition: "life-total", who: { player: "an-opponent" },
      comparison: { op: "gt", value: { amount: "life-total", of: you } } });
    condition("that player has more cards in hand than you", { condition: "cards-in-hand", who: { player: "that-player" },
      comparison: { op: "gt", value: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } } } });
  });
  it("qualifies each candidate player against a fixed reference", () => {
    const c = new Cursor(lex("each opponent who controls more lands than you"));
    assert.deepEqual(parsePlayerRef(c), { player: "filtered", base: { player: "each-opponent" },
      filter: { controls: { filter: { types: ["land"] }, comparison: { op: "gt", value: yourLands } } } });
    assert.ok(c.done());
  });
  it("rejects quantified or unknown reference players and pre-scoped nouns", () => {
    // Quantified reference players are the separate player-comparison node
    // (condition-histories.test.ts); they never become a count against one player.
    for (const text of ["you control more creatures than each other player", "you control fewer creatures than an opponent",
      "you have more cards in hand than each opponent"]) {
      const parsed = parseCondition(new Cursor(lex(text)));
      assert.equal(parsed?.condition, "player-comparison", text);
    }
    for (const text of ["an opponent controls more lands you control than you", "an opponent controls more lands than"]) noCondition(text);
  });
});

describe("coordinated 'each' nouns with one trailing qualifier are not split", () => {
  // The union under one singular "each" is now built, so the former
  // rejections are structural assertions that the qualifier covers both nouns
  // (a split into "each A" plus "B <qualifier>" fails every one of them).
  it("rejects a split that would drop the qualifier from the first noun", () => {
    const union = (a: string, b: string) => ({ allOf: [{ anyOf: [{ types: [a] }, { types: [b] }] }] });
    assert.deepEqual(parsed("Destroy each artifact and creature with mana value 2."), [{ kind: "spell", effects: [{ steps: [{
      effect: "destroy", what: { ref: "each", filter: { ...union("artifact", "creature"),
        properties: [{ property: "mana-value", comparison: { op: "eq", value: n(2) } }] } } }] }] }]);
    assert.deepEqual(parsed("~ deals 1 damage to each opponent and each creature and planeswalker they control."), [{ kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "self" }, amount: n(1), to: { to: "each", refs: [{ player: "each-opponent" },
        { ref: "each", filter: { ...union("creature", "planeswalker"), controlledBy: { player: "they" } } }] } }] }] }]);
    assert.deepEqual(parsed("~ deals 2 damage to each creature and planeswalker you don't control."), [{ kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "self" }, amount: n(2),
      to: { to: "object", ref: { ref: "each", filter: { ...union("creature", "planeswalker"), control: "not-you" } } } }] }] }]);
    assert.deepEqual(parsed("Exile each artifact and creature your opponents control."), [{ kind: "spell", effects: [{ steps: [{
      effect: "exile", what: { ref: "each", filter: { ...union("artifact", "creature"), control: "opponent" } } }] }] }]);
  });
  it("keeps unqualified and separately determined lists", () => {
    assert.deepEqual(parsed("~ deals 2 damage to each creature and each player."), [{ kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "self" }, amount: n(2),
      to: { to: "each", refs: [{ ref: "each", filter: { types: ["creature"] } }, { player: "each-player" }] } }] }] }]);
    assert.equal(parseOracleText("Destroy all artifacts and creatures your opponents control.").ok, true);
  });
});

describe("new quantities stay visible to reference analysis", () => {
  it("does not report event or devotion quantities as fully bound", () => {
    for (const text of ["You gain life equal to the life you've lost this turn.", "Draw cards equal to your devotion to blue."]) {
      const result = parseOracleText(text, "Test", { typeLine: "Sorcery", includeSourceProvenance: true });
      const analysis = analyzeCardBindings({ identity: "test", source: { name: "Test", layout: "normal", type_line: "Sorcery" },
        faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(analysis.fullyBound, false, text);
    }
  });
});
