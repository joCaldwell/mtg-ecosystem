import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: "<sources> each deal damage equal to their power to ...".
// Oracle wordings checked in the frozen corpus:
// Allies at Last, Common Goal, Coordinated Clobbering, Friendly Rivalry,
// Tandem Takedown, Band Together.
// CR 120.3, 601.2c, 608.2c/h:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const ownPower: Amount = { amount: "attribute", of: { ref: "it" }, attribute: "power" };
function parsed(text: string) {
  const result = parseOracleText(text, undefined, { typeLine: "Instant" });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine: "Instant" }).ok, false, text);
}

describe("several sources each dealing damage equal to their own power", () => {
  const opposing = { to: "object", ref: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: n(1) } };
  it("is the same instruction as 'each of <selection> deals damage equal to its power'", () => {
    const floating = parsed("Up to two target creatures you control each deal damage equal to their power to target creature an opponent controls.");
    assert.deepEqual(floating, [{ kind: "spell", effects: [{ steps: [{ effect: "damage",
      source: { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(2), upTo: true }, amount: ownPower, to: opposing }] }] }]);
    assert.deepEqual(floating, parsed("Each of up to two target creatures you control deals damage equal to its power to target creature an opponent controls."));
  });
  it("keeps a one-or-two selection", () => {
    assert.deepEqual(parsed("One or two target creatures you control each deal damage equal to their power to target creature an opponent controls."), [{
      kind: "spell", effects: [{ steps: [{ effect: "damage",
        source: { ref: "target", filter: { types: ["creature"], control: "you" }, count: { amount: "chosen", minimum: 1, maximum: n(2) } },
        amount: ownPower, to: opposing }] }] }]);
  });
  it("ranges 'they each' over the members of the established group", () => {
    assert.deepEqual(parsed("Tap one or two target untapped creatures you control. They each deal damage equal to their power to target creature an opponent controls."), [{
      kind: "spell", effects: [
        { steps: [{ effect: "tap", what: { ref: "target", filter: { status: ["untapped"], types: ["creature"], control: "you" },
          count: { amount: "chosen", minimum: 1, maximum: n(2) } } }] },
        { steps: [{ effect: "damage", source: { ref: "each", filter: { fromAmong: { ref: "them" } } }, amount: ownPower, to: opposing }] },
      ] }]);
  });
  it("makes separately printed subjects simultaneous sources with one shared recipient", () => {
    const recipient = { to: "object", ref: { ref: "target", filter: { types: ["creature"], control: "not-you" }, count: n(1) } };
    assert.deepEqual(parsed("Target creature you control and up to one other target legendary creature you control each deal damage equal to their power to target creature you don't control."), [{
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "damage", source: { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(1) }, amount: ownPower, to: recipient },
        { effect: "damage", source: { ref: "target", filter: { supertypes: ["legendary"], types: ["creature"], control: "you", other: true }, count: n(1), upTo: true },
          amount: ownPower, to: recipient },
      ] }] }] }]);
  });
  it("shares one recipient target declaration between the sources", () => {
    const result = parseOracleText("Target creature you control and up to one other target creature you control each deal damage equal to their power to target creature you don't control.",
      undefined, { typeLine: "Instant", includeSourceProvenance: true });
    assert.ok(result.ok);
    const [ability] = JSON.parse(JSON.stringify(result.abilities));
    const [first, second] = ability.effects[0].steps[0].steps;
    assert.ok(first.to.ref.selectionId);
    assert.equal(first.to.ref.selectionId, second.to.ref.selectionId);
    assert.notEqual(first.source.selectionId, second.source.selectionId);
  });
  it("rejects a single source, a fixed distributed amount and an unidentified group", () => {
    fails("Target creature you control each deal damage equal to their power to target creature.");
    fails("Two target creatures each deal 2 damage to target player.");
    fails("Creatures you control each deal damage equal to their power to target creature.");
    fails("Up to two target creatures you control each deal damage equal to their power.");
  });
});
