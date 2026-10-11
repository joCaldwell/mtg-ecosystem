import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, ObjectFilter, ObjectRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseObjectRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle clauses from Victimize, Gifts Ungiven, Ecological Appreciation,
// Dauntless Bodyguard, Tyrannical Pitlord and Forgotten Lore.
// CR601.2c, 608.2d, 607.2d: announcement targets, choices while resolving and
// linked choices have different origins. A chosen-object reference declares
// no new target and must not bind to the most recent unrelated object.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const chosen = (number: "singular" | "plural", filter: ObjectFilter, which: "chosen" | "last-chosen" = "chosen"): ObjectRef => ({ ref: "choice-result", number, filter, which, scope: "contextual" });
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function ref(text: string, expected: ObjectRef) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseObjectRef(c), expected);
  assert.ok(c.done(), c.errorMessage());
}

describe("printed chosen-object references", () => {
  it("preserves number, descriptor, last-choice qualifier and contextual origin", () => {
    ref("the chosen card", chosen("singular", { cls: "card" }));
    ref("the chosen cards", chosen("plural", { cls: "card" }));
    ref("the last chosen card", chosen("singular", { cls: "card" }, "last-chosen"));
    ref("the chosen creature you control", chosen("singular", { types: ["creature"], control: "you" }));
    ref("the chosen permanents you don't control", chosen("plural", { cls: "permanent", control: "not-you" }));
    ref("one of the chosen cards", { ref: "from-pool", pool: chosen("plural", { cls: "card" }), count: n(1) });
  });
  it("does not mistake Victimize's intervening sacrifice for the chosen target group", () => {
    assert.deepEqual(parsed("Choose two target creature cards in your graveyard. Sacrifice a creature. If you do, return the chosen cards to the battlefield tapped."), [{ kind: "spell", effects: [
      { steps: [{ effect: "declare-target", what: { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: n(2) } }] },
      { steps: [{ effect: "sacrifice", who: { player: "you" }, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } }] },
      { steps: [{ effect: "move-zone", what: chosen("plural", { cls: "card" }), to: { zone: "battlefield" }, tapped: true }], condition: { condition: "you-do" }, conditionKind: "if" },
    ] }]);
  });
  it("retains opponent choice, source pool and simultaneous destinations in Gifts Ungiven", () => {
    assert.deepEqual(parsed("Search your library for up to four cards with different names and reveal them. Target opponent chooses two of those cards. Put the chosen cards into your graveyard and the rest into your hand. Then shuffle."), [{ kind: "spell", effects: [
      { steps: [{ effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { cls: "card" }, count: n(4), upTo: true, groupConstraints: [{ constraint: "pairwise-different", property: "name" }] } }, { effect: "reveal", who: { player: "you" }, what: { ref: "them" } }] },
      { steps: [{ effect: "choose-object", who: { player: "target-opponent" }, what: { ref: "from-pool", pool: { ref: "that", filter: { cls: "card" }, plural: true }, count: n(2) } }] },
      { steps: [{ effect: "simultaneous", steps: [{ effect: "move-zone", what: chosen("plural", { cls: "card" }), to: { zone: "graveyard", owner: "your" } }, { effect: "move-zone", what: { ref: "rest" }, to: { zone: "hand", owner: "your" } }] }] },
      { steps: [{ effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } }] },
    ] }]);
  });
  it("preserves a linked choice across an enters replacement and sacrificed source", () => {
    assert.deepEqual(parsed("As this creature enters, choose another creature you control.\nSacrifice this creature: The chosen creature gains indestructible until end of turn."), [
      { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "creature" }, effects: [{ steps: [{ effect: "choose-object", who: { player: "you" }, what: { ref: "filter", filter: { types: ["creature"], control: "you", other: true }, count: n(1) } }] }] } },
      { kind: "activated", costs: [{ cost: "sacrifice", what: { ref: "this", noun: "creature" } }], effects: [{ steps: [{ effect: "gain-abilities", what: chosen("singular", { types: ["creature"] }), abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }], duration: { duration: "end-of-turn" } }] }] },
    ]);
  });
  it("keeps a continuous chosen-creature modifier static", () => {
    assert.deepEqual(parsed("The chosen creature gets +3/+3 and has flying."), [{ kind: "static", effect: { static: "modify", what: chosen("singular", { types: ["creature"] }), modifiers: [
      { modifier: "pt", power: { sign: 1, amount: n(3) }, toughness: { sign: 1, amount: n(3) } },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] },
    ] } }]);
  });
  it("does not manufacture object references for chosen values, players or unsupported relations", () => {
    for (const text of ["the chosen color", "the chosen player", "the chosen opponent", "the chosen creature type", "the chosen card name", "the previously chosen card", "the chosen creature or player"]) {
      const c = new Cursor(lex(text));
      const result = parseObjectRef(c);
      assert.ok(!result || !c.done(), text);
    }
    for (const text of ["Return the chosen card name to your hand.", "Exile the chosen creature type.", "Destroy the previously chosen permanent."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("retains one target identity and leaves chosen-object binding explicitly unresolved", () => {
    const result = parseOracleText("Choose target creature. Tap the chosen creature.", "Choice Fixture", { includeSourceProvenance: true });
    assert.ok(result.ok);
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [{ kind: "spell", effects: [
      { steps: [{ effect: "declare-target", what: { ref: "target", filter: { types: ["creature"] }, count: n(1), selectionId: "line:0:1" } }] },
      { steps: [{ effect: "tap", what: chosen("singular", { types: ["creature"] }) }] },
    ] }]);
    const graph = analyzeCardBindings({ identity: "choice-fixture", source: { name: "Choice Fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.equal(graph.nodes.filter(node => node.node === "selection" && node.selection === "target").length, 1);
    assert.ok(graph.references.some(binding => "ref" in binding.expression && binding.expression.ref === "choice-result" && binding.binding === "unresolved"));
  });
});
