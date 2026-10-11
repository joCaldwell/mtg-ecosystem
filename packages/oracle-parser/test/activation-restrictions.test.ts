import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseActivationRules, type ActivationRules } from "../src/parser/restrictions.ts";
import { parseOracleText } from "../src/index.ts";

// Activation clauses verified in cached Scryfall corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 602.2 and 602.5b–e:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function rules(text: string): ActivationRules {
  const c = new Cursor(lex(text));
  const result = parseActivationRules(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return result;
}
function failure(text: string): void {
  const c = new Cursor(lex(text));
  assert.equal(parseActivationRules(c), null, text);
  assert.equal(c.pos, 0, text);
}

describe("activation timing and usage restrictions", () => {
  it("retains sorcery and instant timing as different restrictions", () => {
    assert.deepEqual(rules("Activate only as a sorcery."), { restriction: { restriction: "sorcery" } });
    assert.deepEqual(rules("Activate only as an instant."), { restriction: { restriction: "instant" } });
    failure("Activate only as a creature.");
  });

  it("distinguishes turn limits from an object's lifetime limit", () => {
    // Duggan, Private Detective 0fbe3805-b59b-47fc-ab21-f05a4aa4cfe0:
    // 'Activate only once' is not reset every turn (CR 602.5b–c, 400.7).
    assert.deepEqual(rules("Activate only once each turn."), { restriction: { restriction: "once-each-turn" } });
    assert.deepEqual(rules("Activate only twice each turn."), {
      restriction: { restriction: "activation-limit", maximum: 2, period: "turn" },
    });
    assert.deepEqual(rules("Activate only once."), {
      restriction: { restriction: "activation-limit", maximum: 1, period: "object" },
    });
    failure("Activate only once each combat.");
    failure("Activate only once each.");
  });

  it("retains phase, turn owner, and cutoff independently", () => {
    // CR505.1b: a third main phase is postcombat but is not the second.
    assert.deepEqual(rules("Activate only during your second main phase."), {
      restriction: { restriction: "timing", turn: "your", phase: "second-main" },
    });
    assert.deepEqual(rules("Activate only during your postcombat main phase."), {
      restriction: { restriction: "timing", turn: "your", phase: "postcombat-main" },
    });
    // Humble Defector 1582e16d-4825-4cc5-b12c-449c45e4e449;
    // Eternal Dragon 04d8615c-3883-4251-9790-1d8a4a40e142;
    // Stern Marshal 00602959-e9a1-4706-a1cb-70156a6fc713.
    assert.deepEqual(rules("Activate only during your turn."), { restriction: { restriction: "timing", turn: "your" } });
    assert.deepEqual(rules("Activate only during your upkeep."), {
      restriction: { restriction: "timing", turn: "your", phase: "upkeep" },
    });
    assert.deepEqual(rules("Activate only during any upkeep step."), {
      restriction: { restriction: "timing", phase: "upkeep" },
    });
    assert.deepEqual(rules("Activate only during your turn, before attackers are declared."), {
      restriction: { restriction: "timing", turn: "your", before: "declare-attackers" },
    });
    assert.deepEqual(rules("Activate only before attackers are declared."), {
      restriction: { restriction: "timing", before: "declare-attackers" },
    });
    assert.deepEqual(rules("Activate only during the declare blockers step."), {
      restriction: { restriction: "timing", phase: "declare-blockers" },
    });
    assert.deepEqual(rules("Activate only during combat."), {
      restriction: { restriction: "timing", phase: "combat" },
    });
    failure("Activate only during your turn, after attackers are declared.");
    failure("Activate only during your next upkeep.");
    failure("Activate only during your turn except during combat.");
  });

  it("preserves every conjunct instead of choosing one restriction", () => {
    // Licia, Sanguine Tribune 57170e91-6ebe-4b14-bf56-96d8cf6a4d03.
    assert.deepEqual(rules("Activate only during your turn and only once each turn."), {
      restriction: { restriction: "all-of", restrictions: [
        { restriction: "timing", turn: "your" }, { restriction: "once-each-turn" },
      ] },
    });
    assert.deepEqual(rules("Activate only as a sorcery. Activate only once."), {
      restriction: { restriction: "all-of", restrictions: [
        { restriction: "sorcery" }, { restriction: "activation-limit", maximum: 1, period: "object" },
      ] },
    });
    failure("Activate only during your turn or only once each turn.");
    failure("Activate only as a sorcery and only.");
    failure("Activate only as a sorcery. Unknown rule.");
  });

  it("retains the actual activation condition", () => {
    // Tainted Isle 0222414f-98b5-458a-a0fd-831a66cd8b07.
    const swamp = { condition: "control", who: { player: "you" },
      what: { ref: "filter", filter: { subtypes: ["Swamp"] }, count: { amount: "fixed", value: 1 } } };
    assert.deepEqual(rules("Activate only if you control a Swamp."), {
      restriction: { restriction: "condition", condition: swamp },
    });
    assert.deepEqual(rules("Activate only if you control a Swamp and only once each turn."), {
      restriction: { restriction: "all-of", restrictions: [
        { restriction: "condition", condition: swamp }, { restriction: "once-each-turn" },
      ] },
    });
    assert.deepEqual(rules("Activate only if there are seven or more cards in your graveyard."), {
      restriction: { restriction: "condition", condition: {
        condition: "count", of: { cls: "card", zone: { zone: "graveyard", owner: "your" } },
        comparison: { op: "ge", value: { amount: "fixed", value: 7 } },
      } },
    });
    failure("Activate only if you control a creature that fought this turn.");
    failure("Activate only if you control a Swamp unless an opponent controls an Island.");
  });
});

describe("activation permissions are separate from restrictions", () => {
  it("retains who may activate together with their timing restriction", () => {
    // Flailing Manticore 044a169e-32ae-464a-a642-8d3409e82d9f and
    // Scandalmonger 2d98d8d8-e277-42c4-bf94-dd7d7f2047c3.
    assert.deepEqual(rules("Any player may activate this ability."), { permission: { activators: "any-player" } });
    assert.deepEqual(rules("Any player may activate this ability but only as a sorcery."), {
      permission: { activators: "any-player" }, restriction: { restriction: "sorcery" },
    });
    // Detention Vortex 5383a1c1-662f-46fe-a0e5-0f38efe1a37a.
    assert.deepEqual(rules("Only your opponents may activate this ability and only as a sorcery."), {
      permission: { activators: "opponents-of-source-controller" }, restriction: { restriction: "sorcery" },
    });
    failure("Any player may activate this ability or only as a sorcery.");
    failure("Any player may activate this ability. Only your opponents may activate this ability.");
  });

  it("preserves the activator's turn instead of using the source controller's turn", () => {
    // Mana Cache 1c09f2d1-413b-4a92-aa39-99c1a2106814 and
    // Well of Knowledge 99e2b36b-1fab-4b09-b371-d97a6854dfbc.
    assert.deepEqual(rules("Any player may activate this ability but only during their turn before the end step."), {
      permission: { activators: "any-player" }, restriction: { restriction: "timing", turn: "activator", before: "end-step" },
    });
    assert.deepEqual(rules("Any player may activate this ability but only during their draw step."), {
      permission: { activators: "any-player" }, restriction: { restriction: "timing", turn: "activator", phase: "draw-step" },
    });
    failure("Activate only during their turn.");
  });
});

describe("activation rules in complete abilities", () => {
  it("retains permission and restriction on an activated ability", () => {
    const result = parseOracleText("{3}: Destroy this Aura. Only your opponents may activate this ability and only as a sorcery.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "mana", symbols: ["3"] }],
      effects: [{ steps: [{ effect: "destroy", what: { ref: "this", noun: "aura" } }] }],
      permission: { activators: "opponents-of-source-controller" }, restriction: { restriction: "sorcery" },
    }]);
  });

  it("keeps the complete condition of a mana ability", () => {
    const result = parseOracleText("{T}: Add {U} or {B}. Activate only if you control a Swamp.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "choice", options: [{ mana: "fixed", symbols: ["U"] }, { mana: "fixed", symbols: ["B"] }] } }] }],
      restriction: { restriction: "condition", condition: { condition: "control", who: { player: "you" },
        what: { ref: "filter", filter: { subtypes: ["Swamp"] }, count: { amount: "fixed", value: 1 } } } },
    }]);
  });

  it("rejects activation rules on spell and triggered abilities", () => {
    for (const text of [
      "Draw a card. Any player may activate this ability.",
      "When this creature enters, draw a card. Activate only during your turn.",
      "When this creature enters, draw a card. Any player may activate this ability.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
