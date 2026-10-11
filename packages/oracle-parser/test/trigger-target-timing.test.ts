import assert from "node:assert/strict";
import { it } from "node:test";
import { parseOracleText } from "../src/index.ts";

// CR603.3d/603.4: an ordinary ability's targets are chosen after triggering.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
it("rejects fresh targets in printed trigger match and intervening-if predicates", () => {
  for (const text of [
    "Whenever target creature attacks, draw a card.",
    "Whenever target opponent draws a card, draw a card.",
    "Whenever a creature target opponent controls attacks, draw a card.",
    "Whenever this creature attacks, if target creature is tapped, draw a card.",
    "Whenever this creature attacks, if target opponent has a card in hand, draw a card.",
    "Whenever a counter is put on this creature, if target creature is tapped, draw a card.",
    'This creature has "Whenever target creature attacks, draw a card."',
  ]) assert.equal(parseOracleText(text).ok, false, text);
});

it("retains ordinary body targets, definite trigger participants, and delayed capture", () => {
  const parse = (text: string) => {
    const result = parseOracleText(text, undefined, { includeSourceProvenance: true });
    assert.ok(result.ok, JSON.stringify(result));
    return result.abilities;
  };
  const ability = parse("Whenever a creature attacks, if that creature is tapped, destroy target artifact.")[0];
  assert.equal(ability.kind, "triggered");
  if (ability.kind !== "triggered") return;
  assert.deepEqual(ability.condition, { condition: "status", what: { ref: "that", filter: { types: ["creature"] } }, status: "tapped" });
  assert.deepEqual(ability.effects, [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["artifact"] }, count: { amount: "fixed", value: 1 }, selectionId: "line:0:12" } }] }]);
  parse("Whenever this creature attacks, tap target creature. If target creature is tapped, draw a card.");
  const delayed = parse("When target creature dies this turn, draw a card.")[0];
  assert.equal(delayed.kind, "spell");
  if (delayed.kind === "spell") assert.equal(delayed.effects[0].steps[0].effect, "create-delayed-trigger");
});
