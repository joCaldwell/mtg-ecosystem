import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Actual Oracle wording from cached Scryfall data: Skyscribing, Steeling
// Stance, Pride of the Clouds, Paladin of Prahv, Proclamation of Rebirth.
// Forecast's intrinsic hand/upkeep/once-turn/reveal rules are CR 702.57a/b;
// Companion's outside-game deck condition and special action are 702.139.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const hand = { zone: "hand", owner: "your" } as const;
const revealCard = { cost: "reveal", what: { ref: "this", noun: "card" }, from: hand } as const;
function parse(text: string, name = "Fixture", typeLine = "Creature", provenance = false) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result.abilities;
}
const activated = (symbols: string[]) => ({ kind: "activated", keyword: "forecast", abilityWord: undefined,
  restriction: undefined, costs: [{ cost: "mana", symbols }, revealCard] });

describe("Forecast preserves its intrinsic zone, timing and reveal rules", () => {
  it("keeps a Forecast activation separate from a sorcery's resolving spell instructions", () => {
    assert.deepEqual(parse("Each player draws X cards.\nForecast — {2}{U}, Reveal this card from your hand: Each player draws a card. (Activate only during your upkeep and only once each turn.)", "Skyscribing", "Sorcery"), [
      { kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "draw", who: { player: "each-player" }, amount: { amount: "x" } }] }] },
      { ...activated(["2", "U"]), effects: [{ steps: [{ effect: "draw", who: { player: "each-player" }, amount: one }] }] },
    ]);
  });
  it("retains the reveal payment, complete target and original target position", () => {
    assert.deepEqual(parse("Creatures you control get +1/+1 until end of turn.\nForecast — {W}, Reveal this card from your hand: Target creature gets +1/+1 until end of turn. (Activate only during your upkeep and only once each turn.)", "Steeling Stance", "Instant", true), [
      { kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "pump", what: { ref: "each", filter: { types: ["creature"], control: "you" } }, power: { sign: 1, amount: one }, toughness: { sign: 1, amount: one }, duration: { duration: "end-of-turn" } }] }] },
      { ...activated(["W"]), effects: [{ steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: one, selectionId: "line:1:11" }, power: { sign: 1, amount: one }, toughness: { sign: 1, amount: one }, duration: { duration: "end-of-turn" } }] }] },
    ]);
  });
  it("preserves the source noun and every created token characteristic", () => {
    assert.deepEqual(parse("Forecast — {2}{W}{U}, Reveal this creature from your hand: Create a 1/1 white and blue Bird creature token with flying.", "Pride of the Clouds"), [{
      ...activated(["2", "W", "U"]), costs: [{ cost: "mana", symbols: ["2", "W", "U"] }, { cost: "reveal", what: { ref: "this", noun: "creature" }, from: hand }],
      effects: [{ steps: [{ effect: "create-token", count: one, token: { colors: ["white", "blue"], supertypes: [], types: ["creature"], subtypes: ["Bird"], keywords: [{ keyword: "flying" }], power: one, toughness: one } }] }],
    }]);
  });
  it("keeps the forecast target separate from a delayed trigger's damage event", () => {
    assert.deepEqual(parse("Whenever this creature deals damage, you gain that much life.\nForecast — {1}{W}, Reveal this card from your hand: Whenever target creature deals damage this turn, you gain that much life. (Activate only during your upkeep and only once each turn.)", "Paladin of Prahv", "Creature — Human Knight", true), [
      { kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: undefined, to: undefined }, effects: [{ steps: [{ effect: "gain-life", who: you, amount: { amount: "that-much" } }] }] },
      { ...activated(["1", "W"]), effects: [{ steps: [{ effect: "create-delayed-trigger", occurrence: "each", duration: { duration: "this-turn" }, trigger: { trigger: "deals-damage", what: { ref: "target", filter: { types: ["creature"] }, count: one, selectionId: "line:1:13" }, combat: undefined, to: undefined }, effects: [{ steps: [{ effect: "gain-life", who: you, amount: { amount: "that-much" } }] }], triggerTargets: "enclosing-ability", targets: "delayed-ability" }] }] },
    ]);
  });
  it("retains additional activation restrictions instead of replacing the keyword's intrinsic rules", () => {
    assert.deepEqual(parse("Forecast — {U}, Reveal this card from your hand: Draw a card. Activate only if you control a creature."), [{
      ...activated(["U"]), effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
      restriction: { restriction: "condition", condition: { condition: "control", who: you, what: { ref: "filter", filter: { types: ["creature"] }, count: one } } },
    }]);
    assert.deepEqual(parse("{U}, Reveal this card from your hand: Draw a card."), [{
      kind: "activated", abilityWord: undefined, restriction: undefined, costs: [{ cost: "mana", symbols: ["U"] }, revealCard], effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
    }]);
  });
  it("rejects malformed Forecast, unknown tails and mechanical Companion fallback", () => {
    for (const text of ["Forecast", "Forecast — Draw a card.", "Forecast — Flying", "Forecast — When this creature enters, draw a card.",
      "Forecast — {U}: Invent a mechanic.", "Forecast — {U}: Draw a card. Unsupported rules.",
      "Companion — Draw a card.", "Companion — Creatures you control get +1/+1.", "Companion — {U}: Draw a card.",
      "Companion — When this creature enters, draw a card.", "Companion — Flying", "Companion — Your starting deck contains only cards with even mana values."])
      assert.equal(parseOracleText(text).ok, false, text);
    assert.deepEqual(parse("Doctor's companion"), [{ kind: "keywords", keywords: [{ keyword: "doctor's companion" }] }]);
    assert.deepEqual(parse("Study — {U}: Draw a card."), [{ kind: "activated", abilityWord: "Study", restriction: undefined, costs: [{ cost: "mana", symbols: ["U"] }], effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }] }]);
  });
  it("does not certify Forecast's source-zone, upkeep, reveal and activation history bindings", () => {
    const result = parseOracleText("Forecast — {U}, Reveal this card from your hand: Draw a card.\nForecast — {U}, Reveal this card from your hand: Draw a card.", "Forecast fixture", { typeLine: "Creature" });
    assert.ok(result.ok);
    const graph = analyzeCardBindings({ identity: "forecast", source: { name: "Forecast fixture", layout: "normal", type_line: "Creature" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    const diagnostics = graph.diagnostics.filter(d => d.path.endsWith("/keyword"));
    assert.equal(diagnostics.length, 2);
    assert.ok(diagnostics.every(d => d.code === "unsupported-scope" && d.message.includes("reveal lifetime")));
    const abilities = graph.nodes.filter(n => n.node === "ability");
    assert.equal(abilities.length, 2);
    assert.notEqual(abilities[0].id, abilities[1].id);
  });
});
