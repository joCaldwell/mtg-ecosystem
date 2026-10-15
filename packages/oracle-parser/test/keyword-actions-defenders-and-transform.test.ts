import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Comprehensive Rules (https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
function parse(text: string, typeLine = "Creature — Test"): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function rejected(text: string, typeLine = "Creature — Test"): void {
  assert.equal(parseOracleText(text, "Test", { typeLine }).ok, false, text);
}

const attackers = { player: "filtered", base: { player: "each-opponent" }, filter: { attackingPlayer: { player: "that-player" } } };

describe("each opponent attacking that player (CR 506.2, 508.1b)", () => {
  it("restates the preceding instruction for those players with 'does the same'", () => {
    const [ability] = parse("Whenever enchanted player is attacked, you draw a card. Each opponent attacking that player does the same.", "Enchantment — Aura Curse");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects, [
      { steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] },
      { steps: [{ effect: "draw", who: attackers, amount: { amount: "fixed", value: 1 } }] },
    ]);
  });
  it("accepts the player filter on an explicit instruction", () => {
    const [ability] = parse("Whenever enchanted player is attacked, untap all nonland permanents you control. Each opponent attacking that player untaps all nonland permanents they control.", "Enchantment — Aura Curse");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[1].steps[0], {
      effect: "untap", who: attackers,
      what: { ref: "all", filter: { nonTypes: ["land"], cls: "permanent", controlledBy: { player: "they" } } },
    });
  });
  it("rejects 'does the same' without a restatable preceding instruction", () => {
    rejected("Whenever enchanted player is attacked, you may draw a card. Each opponent attacking that player does the same.", "Enchantment — Aura Curse");
    rejected("Whenever enchanted player is attacked, target creature gets +1/+1 until end of turn. Each opponent attacking that player does the same.", "Enchantment — Aura Curse");
    rejected("Each opponent attacking that player does the same.", "Enchantment — Aura Curse");
  });
});

describe("as this permanent transforms into ~ (CR 701.27e, 614.1c)", () => {
  it("is a static applied while transforming, with the whole body kept", () => {
    const [ability] = parse("As this creature transforms into ~, choose an opponent.");
    assert.deepEqual(ability, {
      kind: "static",
      effect: {
        static: "as-transforms", what: { ref: "this", noun: "creature" }, into: { face: "source-face" },
        effects: [{ steps: [{ effect: "choose-object", who: { player: "you" }, what: { player: "an-opponent" } }] }],
      },
    });
  });
  it("keeps an attachment instruction", () => {
    const [ability] = parse("As this permanent transforms into ~, attach it to a player.", "Enchantment — Aura Curse");
    assert.ok(ability.kind === "static" && ability.effect.static === "as-transforms");
    assert.deepEqual(ability.effect.effects, [{ steps: [{ effect: "attach", what: { ref: "it" }, to: { player: "a-player" }, who: { player: "you" } }] }]);
  });
  it("rejects wordings that do not name the face or have no body", () => {
    rejected("As this creature transforms, choose an opponent.");
    rejected("As this creature transforms into, choose an opponent.");
    rejected("As this creature transforms into ~.");
  });
});
