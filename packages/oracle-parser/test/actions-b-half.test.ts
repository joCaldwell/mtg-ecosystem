import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// CR 107.1a: a spell or ability that could generate a fractional number tells
// you whether to round up or down; every half amount keeps that direction.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function steps(text: string, typeLine = "Sorcery"): Effect[] | null {
  const result = parseOracleText(text, "Test", { typeLine });
  const line = result.lines[0];
  if (!line || line.ok !== true) return null;
  const ast: Ability = line.ability;
  if (ast.kind !== "spell") return null;
  return JSON.parse(JSON.stringify(ast.effects.flatMap(e => e.steps))) as Effect[];
}
const life = (player: "you" | "they") => ({ amount: "life-total", of: { player } });

describe("half amounts with stated rounding", () => {
  it("loses half their / your life", () => {
    assert.deepEqual(steps("Target opponent loses half their life, rounded up."), [
      { effect: "lose-life", who: { player: "target-opponent" }, amount: { amount: "half", of: life("they"), round: "up" } },
    ]);
    assert.deepEqual(steps("You lose half your life, rounded down."), [
      { effect: "lose-life", who: { player: "you" }, amount: { amount: "half", of: life("you"), round: "down" } },
    ]);
  });
  it("mills half their library", () => {
    assert.deepEqual(steps("Target player mills half their library, rounded down."), [
      { effect: "mill", who: { player: "target-player" }, amount: {
        amount: "half", of: { amount: "count", of: { cls: "card", zone: { zone: "library", owner: "their" } } }, round: "down" } },
    ]);
  });
  it("takes a rounding direction printed after the whole instruction", () => {
    assert.deepEqual(steps("You gain half X life, rounded down."), [
      { effect: "gain-life", who: { player: "you" }, amount: { amount: "half", of: { amount: "x" }, round: "down" } },
    ]);
    assert.deepEqual(steps("Draw half X cards, rounded up."), [
      { effect: "draw", who: { player: "you" }, amount: { amount: "half", of: { amount: "x" }, round: "up" } },
    ]);
  });
  it("never invents a rounding direction", () => {
    assert.equal(steps("Target opponent loses half their life."), null);
    assert.equal(steps("You gain half X life."), null);
    assert.equal(steps("Target player mills half their library."), null);
    assert.equal(steps("Draw half X cards."), null);
    // Two halves would need two directions.
    assert.equal(steps("Draw half X cards and gain half X life, rounded up."), null);
  });
});

describe("half of a set of objects", () => {
  const they = { player: "they" };
  const creatures = { types: ["creature"], controlledBy: they };
  it("sacrifices half the creatures they control of their choice", () => {
    assert.deepEqual(steps("Target player sacrifices half the creatures they control of their choice, rounded down."), [
      { effect: "sacrifice", who: { player: "target-player" }, chosenBy: { player: "target-player" }, what: {
        ref: "filter", filter: creatures, count: { amount: "half", of: { amount: "count", of: creatures }, round: "down" } } },
    ]);
  });
  it("discards half the cards in their hand", () => {
    const hand = { cls: "card", zone: { zone: "hand", owner: "their" } };
    assert.deepEqual(steps("Target opponent discards half the cards in their hand, rounded up."), [
      { effect: "discard", who: { player: "target-opponent" }, what: {
        ref: "filter", filter: hand, count: { amount: "half", of: { amount: "count", of: hand }, round: "up" } } },
    ]);
  });
  it("returns half the creatures they control to their owner's hand", () => {
    assert.deepEqual(steps("Return half the creatures they control to their owner's hand, rounded up."), [
      { effect: "return-to-hand", to: "owner", what: {
        ref: "filter", filter: creatures, count: { amount: "half", of: { amount: "count", of: creatures }, round: "up" } } },
    ]);
  });
  it("needs a stated rounding direction", () => {
    assert.equal(steps("Target player sacrifices half the creatures they control of their choice."), null);
    assert.equal(steps("Target opponent discards half the cards in their hand."), null);
  });
});
