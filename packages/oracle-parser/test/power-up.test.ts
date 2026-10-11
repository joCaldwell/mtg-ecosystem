import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle text: Bold Biochemist, Unliving Legionnaire, Hulk, Gamma Goliath.
// Power-up governs each individual activated ability, not a counter payment
// keyword or an ability word. CR 702.193a/b, 602.5b/c, 118.7:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value });
function parse(text: string, name = "", typeLine = "Creature") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
describe("Power-up activated keyword", () => {
  it("retains the printed cost and complete body under the intrinsic payment/history rule", () => {
    const text = "Power-up — {5}{U}: Put a +1/+1 counter on this creature and draw two cards. (Activate each power-up ability only once. Reduce the cost by its mana cost if it entered this turn.)";
    assert.deepEqual(parse(text, "Bold Biochemist"), [{ kind: "activated", keyword: "power-up", costs: [{ cost: "mana", symbols: ["5", "U"] }], effects: [{ steps: [
      { effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: n(1) },
      { effect: "draw", who: { player: "you" }, amount: n(2) },
    ] }] }]);
  });
  it("preserves target choice and later instructions without adding sorcery timing", () => {
    const text = "Flying\nPower-up — {5}{B}{B}: Return up to one target creature card from your graveyard to your hand. Put two +1/+1 counters on this creature. (Activate each power-up ability only once. Reduce the cost by its mana cost if it entered this turn.)";
    assert.deepEqual(parse(text, "Unliving Legionnaire"), [
      { kind: "keywords", keywords: [{ keyword: "flying" }] },
      { kind: "activated", keyword: "power-up", costs: [{ cost: "mana", symbols: ["5", "B", "B"] }], effects: [
        { steps: [{ effect: "return-to-hand", what: { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: n(1), upTo: true }, to: "your" }] },
        { steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: n(2) }] },
      ] },
    ]);
  });
  it("keeps an external reduction for other power-up abilities distinct from its own intrinsic rule", () => {
    assert.deepEqual(parse("Reach, trample\nPower-up abilities of other creatures you control cost {3} less to activate.\nPower-up — {6}{R}{G}: Put five +1/+1 counters on Hulk.", "Hulk, Gamma Goliath"), [
      { kind: "keywords", keywords: [{ keyword: "reach" }, { keyword: "trample" }] },
      { kind: "static", effect: { static: "cost-modification", scope: { action: "activate", keyword: "power-up", source: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } }, abilities: { ref: "each", filter: { cls: "ability" } } }, change: { direction: "reduce", symbols: ["3"] } } },
      { kind: "activated", keyword: "power-up", costs: [{ cost: "mana", symbols: ["6", "R", "G"] }], effects: [{ steps: [{ effect: "put-counters", on: { ref: "self" }, counter: { type: "+1/+1" }, count: n(5) }] }] },
    ]);
  });
  it("does not turn an unsupported header or body into an ordinary ability word", () => {
    for (const text of ["Power-up", "Power-up — Draw a card.", "Power-up — When this creature enters, draw a card.",
      "Power-up — {5}{U}: Invent a mechanic.", "Power-up — {5}{U}: Draw a card. An unsupported sentence.", "Power-up — Flying"])
      assert.equal(parseOracleText(text).ok, false, text);
    assert.deepEqual(parse("Exhaust — {5}{U}: Draw a card."), [{ kind: "activated", keyword: "exhaust", costs: [{ cost: "mana", symbols: ["5", "U"] }], effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] }]);
  });
  it("retains separate activation histories for two abilities and diagnoses unbound intrinsic rules", () => {
    const text = "Power-up — {5}{U}: Draw a card.\nPower-up — {5}{U}: Draw a card.";
    const result = parseOracleText(text, "Two Power-ups", { typeLine: "Creature", includeSourceProvenance: true });
    assert.ok(result.ok);
    assert.equal(result.abilities.length, 2);
    const analysis = analyzeCardBindings({ identity: "two-power-ups", source: { name: "Two Power-ups", layout: "normal", type_line: "Creature", mana_cost: "{2}{U}" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.equal(analysis.diagnostics.filter(d => d.code === "unsupported-scope" && d.path.endsWith("/keyword")).length, 2);
    const abilities = analysis.nodes.filter(n => n.node === "ability");
    assert.equal(abilities.length, 2);
    assert.notEqual(abilities[0].id, abilities[1].id);
  });
});
