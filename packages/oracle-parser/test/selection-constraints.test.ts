import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, Comparison, GroupConstraint, ObjectRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseComparison, parseCondition, parseObjectRef, parseObjectSelection } from "../src/parser/refs.ts";
import { parseGroupConstraint } from "../src/parser/selection-constraints.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle fixtures: Patch Up, Protean Hulk, Ormos, Archive Keeper,
// Fabrication Foundry, Mossbridge Troll and Ecological Appreciation.
// CR 107.1b, 201.2b, 601.2c/h, 608.2d:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const total = (attribute: "power" | "toughness" | "mana-value", value: Amount, op: Comparison["op"] = "le"): GroupConstraint =>
  ({ constraint: "attribute-sum", attribute, comparison: { op, value } } as GroupConstraint);
const different = (property: "name" | "power" | "mana-value"): GroupConstraint => ({ constraint: "pairwise-different", property });
const chosen: Amount = { amount: "chosen", minimum: 0 };
function parsed(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function selection(text: string, expected: ObjectRef) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseObjectSelection(c), expected);
  assert.ok(c.done(), c.errorMessage());
}

describe("group selection restrictions", () => {
  it("retains one target group, its count, shared total and graveyard eligibility", () => {
    assert.deepEqual(parsed("Return up to three target creature cards with total mana value 3 or less from your graveyard to the battlefield."), [{ kind: "spell", effects: [{ steps: [{
      effect: "move-zone", what: { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } },
        count: n(3), upTo: true, groupConstraints: [total("mana-value", n(3))] }, to: { zone: "battlefield" },
    }] }] }]);
    selection("up to three target creature cards with mana value 3 or less from your graveyard", {
      ref: "target", filter: { types: ["creature"], cls: "card", properties: [{ property: "mana-value", comparison: { op: "le", value: n(3) } }], zone: { zone: "graveyard", owner: "your" } }, count: n(3), upTo: true,
    });
  });
  it("keeps the search choice, found group, move and shuffle distinct", () => {
    assert.deepEqual(parsed("When this creature dies, search your library for any number of creature cards with total mana value 6 or less, put them onto the battlefield, then shuffle."), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [
        { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: {
          ref: "filter", filter: { types: ["creature"], cls: "card" }, count: chosen, groupConstraints: [total("mana-value", n(6))],
        } },
        { effect: "move-zone", what: { ref: "them" }, to: { zone: "battlefield" } },
        { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } },
      ] }],
    }]);
  });
  it("represents pairwise different names in a payment without counting names", () => {
    assert.deepEqual(parsed("{1}{U}{U}, Discard three cards with different names: Draw five cards."), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1", "U", "U"] }, { cost: "discard", what: {
        ref: "filter", filter: { cls: "card" }, count: n(3), groupConstraints: [different("name")],
      } }], effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(5) }] }],
    }]);
  });
  it("separates Fabrication Foundry's total payment value from the target's individual value", () => {
    assert.deepEqual(parsed("{2}{W}, {T}, Exile one or more other artifacts you control with total mana value X: Return target artifact card with mana value X or less from your graveyard to the battlefield. Activate only as a sorcery."), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2", "W"] }, { cost: "tap-self" }, { cost: "exile", what: {
        ref: "filter", filter: { types: ["artifact"], control: "you", other: true }, count: { amount: "chosen", minimum: 1 }, groupConstraints: [total("mana-value", { amount: "x" }, "eq")],
      } }], effects: [{ steps: [{ effect: "move-zone", what: {
        ref: "target", filter: { types: ["artifact"], cls: "card", properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "x" } } }], zone: { zone: "graveyard", owner: "your" } }, count: n(1),
      }, to: { zone: "battlefield" } }] }], restriction: { restriction: "sorcery" },
    }]);
  });
  it("retains the untapped payment group and source exclusion for a signed-power sum", () => {
    assert.deepEqual(parsed("Tap any number of untapped creatures you control other than this creature with total power 10 or greater: This creature gets +20/+20 until end of turn."), [{
      kind: "activated", costs: [{ cost: "tap-objects", what: { ref: "filter", filter: { status: ["untapped"], types: ["creature"], control: "you", otherThan: { ref: "this", noun: "creature" } }, count: chosen, groupConstraints: [total("power", n(10), "ge")] } }],
      effects: [{ steps: [{ effect: "pump", what: { ref: "this", noun: "creature" }, power: { sign: 1, amount: n(20) }, toughness: { sign: 1, amount: n(20) }, duration: { duration: "end-of-turn" } }] }],
    }]);
  });
  it("keeps constraints on a selected subset separate from the antecedent pool", () => {
    selection("two of those cards with different names", { ref: "from-pool", pool: { ref: "that", filter: { cls: "card" }, plural: true }, count: n(2), groupConstraints: [different("name")] });
    selection("any number of creature cards with different powers from your graveyard", { ref: "filter", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: chosen, groupConstraints: [different("power")] });
    selection("any number of cards with different mana values from your graveyard", { ref: "filter", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: chosen, groupConstraints: [different("mana-value")] });
    selection("two creatures with total toughness 7 or greater", { ref: "filter", filter: { types: ["creature"] }, count: n(2), groupConstraints: [total("toughness", n(7), "ge")] });
  });
  it("preserves signed thresholds and conjunctions without clamping or replacing restrictions", () => {
    selection("any number of creatures with total power -1 or less", { ref: "filter", filter: { types: ["creature"] }, count: chosen, groupConstraints: [total("power", n(-1))] });
    selection("two creature cards with different names and with total mana value 4 or less from your graveyard", {
      ref: "filter", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: n(2), groupConstraints: [different("name"), total("mana-value", n(4))],
    });
  });
  it("retains later individual qualifiers across a two-zone search", () => {
    assert.deepEqual(parsed("Search your library and graveyard for up to four creature cards with different names that each have mana value X or less and reveal them."), [{ kind: "spell", effects: [{ steps: [
      { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }, { zone: "graveyard", owner: "your" }] }, for: {
        ref: "filter", filter: { types: ["creature"], cls: "card", properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "x" } } }] }, count: n(4), upTo: true, groupConstraints: [different("name")],
      } }, { effect: "reveal", who: { player: "you" }, what: { ref: "them" } },
    ] }] }]);
  });
});

describe("selection grammar scopes and diagnostics", () => {
  it("does not enable collective choices in queries, all/each, histories or Coven conditions", () => {
    for (const text of ["three creatures with different powers", "two target creatures with total power 3 or less", "two of those cards with different names"]) {
      const c = new Cursor(lex(text));
      const result = parseObjectRef(c);
      assert.ok(!result || !c.done(), text);
      assert.ok(!JSON.stringify(result).includes("groupConstraints"), text);
    }
    const c = new Cursor(lex("you control three creatures with different powers"));
    const result = parseCondition(c);
    assert.ok(!result || !c.done());
    for (const text of [
      "Destroy each creature with total power 3 or less.", "Exile all cards with different names.",
      "Creatures with different powers get +1/+1.", "If you control three creatures with different powers, draw a card.",
      "Whenever three creatures with different powers attack, draw a card.",
      "Destroy target creature with total power and toughness 5 or less.",
      "Sacrifice two target creatures with total power 3 or greater: Draw a card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    // A dedicated total over the creatures attacked
    // with, not a collective choice in a history (formerly asserted to fail).
    assert.deepEqual(parsed("Whenever this creature attacks, if you attacked with creatures with total power 6 or greater this combat, draw a card.")[0].condition, {
      condition: "attacked", who: { player: "you" }, during: "this-combat", with: { types: ["creature"] }, groupConstraints: [total("power", n(6), "ge")] });
  });
  it("rejects malformed clauses and invalid fixed cardinalities without swallowing text", () => {
    for (const text of ["with total mana value", "with total loyalty 4 or less", "with different colors", "with different name"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseGroupConstraint(c, parseComparison), null, text);
      assert.equal(c.pos, 0, text);
    }
    for (const text of ["Return -2 creature cards with total mana value 3 or less from your graveyard to your hand.",
      "Return two creature cards with total mana value 3 or less unless from your graveyard to your hand.",
      "Choose one or three cards with different names.", "Choose two cards with different names or total mana value 3 or less."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("preserves target identities but diagnoses unimplemented collective binding scopes", () => {
    const text = "Return up to two target creature cards with total mana value 3 or less from your graveyard to your hand.";
    const result = parseOracleText(text, "Group Fixture", { includeSourceProvenance: true });
    assert.ok(result.ok);
    assert.match(JSON.stringify(result.abilities), /"selectionId":"line:0:4"/);
    const graph = analyzeCardBindings({ identity: "group-fixture", source: { name: "Group Fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope" && d.message.includes("Collective selection")));
  });
});
