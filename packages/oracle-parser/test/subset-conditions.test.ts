import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, Condition, ObjectFilter } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Actual Oracle clauses: Candlegrove Witch, Ambitious Farmhand, Might of the
// Old Ways, Maze's End, Liliana's Contract, Field of the Dead, The Necrobloom.
// CR201.2b and 603.4; ordinary duplicate-powered creatures outside the subset
// do not invalidate Coven, and the subset can change before resolution.
// https://magic.wizards.com/en/news/feature/innistrad-midnight-hunt-mechanics
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const subset = (of: ObjectFilter, minimum: number, property: "name" | "power" | "mana-value" = "power"): Condition => ({
  condition: "controls-subset", who: { player: "you" }, of, minimum: n(minimum), groupConstraints: [{ constraint: "pairwise-different", property }],
});
const coven = subset({ types: ["creature"] }, 3);
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function condition(text: string, expected: Condition) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseCondition(c), expected);
  assert.ok(c.done(), c.errorMessage());
}

describe("existence of a controlled qualifying subset", () => {
  it("keeps minimum cardinality and pairwise relations instead of unioning names", () => {
    condition("you control three or more creatures with different powers", coven);
    condition("you control at least three creatures with different powers", coven);
    condition("you control ten or more Gates with different names", subset({ subtypes: ["Gate"] }, 10, "name"));
    condition("you control four or more Demons with different names", subset({ subtypes: ["Demon"] }, 4, "name"));
    condition("you control seven or more lands with different names", subset({ types: ["land"] }, 7, "name"));
    condition("an opponent controls two or more artifacts with different mana values", {
      condition: "controls-subset", who: { player: "an-opponent" }, of: { types: ["artifact"] }, minimum: n(2), groupConstraints: [{ constraint: "pairwise-different", property: "mana-value" }],
    });
  });
  it("keeps intervening-if checks at trigger level without capturing the matching creatures", () => {
    assert.deepEqual(parsed("Coven — At the beginning of combat on your turn, if you control three or more creatures with different powers, this creature gains flying until end of turn."), [{
      kind: "triggered", abilityWord: "Coven", trigger: { trigger: "phase", phase: "combat", whose: "your" }, condition: coven,
      effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "this", noun: "creature" }, abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }], duration: { duration: "end-of-turn" } }] }],
    }]);
    assert.deepEqual(parsed("At the beginning of your upkeep, if you control four or more Demons with different names, you win the game."), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, condition: subset({ subtypes: ["Demon"] }, 4, "name"),
      effects: [{ steps: [{ effect: "win-game", who: { player: "you" } }] }],
    }]);
  });
  it("checks an activation restriction once at activation, not as a resolution rider", () => {
    assert.deepEqual(parsed("Coven — {1}{W}{W}: Transform this creature. Activate only if you control three or more creatures with different powers."), [{
      kind: "activated", abilityWord: "Coven", costs: [{ cost: "mana", symbols: ["1", "W", "W"] }], effects: [{ steps: [{ effect: "transform", what: { ref: "this", noun: "creature" }, who: { player: "you" } }] }],
      restriction: { restriction: "condition", condition: coven },
    }]);
  });
  it("evaluates an ordinary condition after preceding resolution instructions", () => {
    assert.deepEqual(parsed("Search your library for a Gate card, put it onto the battlefield, then shuffle. If you control ten or more Gates with different names, you win the game."), [{
      kind: "spell", effects: [
        { steps: [
          { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { subtypes: ["Gate"], cls: "card" }, count: n(1) } },
          { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" } },
          { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } },
        ] },
        { steps: [{ effect: "win-game", who: { player: "you" } }], condition: subset({ subtypes: ["Gate"] }, 10, "name"), conditionKind: "if" },
      ],
    }]);
  });
  it("preserves composition with a second full condition without swallowing it", () => {
    condition("you control three or more creatures with different powers and you have five or more cards in hand", {
      condition: "all-of", conditions: [coven, { condition: "cards-in-hand", who: { player: "you" }, comparison: { op: "ge", value: n(5) } }],
    });
  });
  it("does not accept unsupported negation, upper bounds, totals or history as subset existence", () => {
    for (const text of [
      "If you control exactly three creatures with different powers, draw a card.",
      "If you control fewer than three creatures with different powers, draw a card.",
      "If you don't control three creatures with different powers, draw a card.",
      "If you controlled three or more creatures with different powers this turn, draw a card.",
      "If you control three or more creatures with total power 6 or greater, draw a card.",
      "If there are nine or more lands with different names among lands you control, draw a card.",
      "If you control three or more creatures with different powers except this creature, draw a card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it("diagnoses the new query scope independently of structural acceptance", () => {
    const result = parseOracleText("If you control seven or more lands with different names, draw a card.", "Subset Fixture");
    assert.ok(result.ok);
    const graph = analyzeCardBindings({ identity: "subset-fixture", source: { name: "Subset Fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope" && d.message.includes("subset existence")));
    assert.equal(graph.nodes.some(node => node.node === "selection"), false);
  });
});
