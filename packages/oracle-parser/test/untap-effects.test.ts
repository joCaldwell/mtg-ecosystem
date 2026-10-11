import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ObjectRef, UntapStepWindow } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseStaticUntapSuppression, parseUntapSuppressionEffect } from "../src/parser/untap.ts";

function only(text: string): Ability {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return parsed.abilities[0];
}
const controllers = "each-affected-objects-controller" as const;
const creature: ObjectRef = { ref: "this", noun: "creature" };

// Oracle examples are checked against the cached Scryfall bulk corpus: Choke,
// Back to Basics, Waterknot, Walking Dream, Frost Breath, and Blinding Beam.
// CR 502.3/611.2 distinguish turn-based untapping from untapping by an effect.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("untap-step restrictions", () => {
  it("retains each affected permanent's controller for recurring restrictions", () => {
    for (const [text, what] of [
      ["Islands don't untap during their controllers' untap steps.", { ref: "each", filter: { subtypes: ["Island"] } }],
      ["Nonbasic lands don't untap during their controllers' untap steps.", { ref: "each", filter: { types: ["land"], nonSupertypes: ["basic"] } }],
      ["Enchanted creature doesn't untap during its controller's untap step.", { ref: "enchanted", noun: "creature" }],
    ] as const) {
      assert.deepEqual(only(text), {
        kind: "static", abilityWord: undefined,
        effect: { static: "untap-step-suppression", what, step: { occurrence: "each", owner: controllers } },
      });
    }
  });

  it("keeps your step distinct from the affected permanent's controller's step", () => {
    assert.deepEqual(only("This creature doesn't untap during your untap step."), {
      kind: "static", abilityWord: undefined,
      effect: { static: "untap-step-suppression", what: creature,
        step: { occurrence: "each", owner: { player: "you" } } },
    });
    assert.deepEqual(only("This creature can't untap."), {
      kind: "static", abilityWord: undefined,
      effect: { static: "cant", what: creature, action: "untap", condition: undefined },
    });
  });

  it("retains conditions on a recurring static suppression", () => {
    assert.deepEqual(only("This creature doesn't untap during your untap step if an opponent controls two or more creatures."), {
      kind: "static", abilityWord: undefined,
      effect: {
        static: "conditional",
        effect: { static: "untap-step-suppression", what: creature,
          step: { occurrence: "each", owner: { player: "you" } } },
        condition: { condition: "control-count", who: { player: "an-opponent" },
          of: { types: ["creature"] }, comparison: { op: "ge", value: { amount: "fixed", value: 2 } } },
      },
    });
  });

  it("does not turn one upcoming step into a recurring static ability", () => {
    const text = "Those creatures don't untap during their controller's next untap step.";
    const c = new Cursor(lex(text));
    assert.equal(parseStaticUntapSuppression(c), null);
    assert.equal(c.pos, 0);
    assert.deepEqual(only(text), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [{ effect: "untap-step-suppression", what: { ref: "that", filter: { types: ["creature"] }, plural: true },
        step: { occurrence: "next", owner: controllers } }] }],
    });
  });

  it("preserves explicit player owners separately from object controllers", () => {
    const cases: [string, UntapStepWindow["owner"]][] = [
      ["their", { player: "they" }], ["that player's", { player: "that-player" }],
      ["your", { player: "you" }], ["target player's", { player: "target-player" }],
      ["their controllers'", controllers], ["their controller's", controllers],
    ];
    for (const [ownerText, owner] of cases) {
      const c = new Cursor(lex(`They don't untap during ${ownerText} next untap step.`));
      assert.deepEqual(parseUntapSuppressionEffect(c), [{ effect: "untap-step-suppression", what: { ref: "them" },
        step: { occurrence: "next", owner } }]);
      assert.equal(c.punct("."), true);
      assert.equal(c.done(), true);
    }
  });

  it("retains the tap action and suppression as separate sentences", () => {
    assert.deepEqual(only("Tap up to two target creatures. Those creatures don't untap during their controller's next untap step."), {
      kind: "spell", abilityWord: undefined,
      effects: [
        { steps: [{ effect: "tap", what: { ref: "target", count: { amount: "fixed", value: 2 }, upTo: true, filter: { types: ["creature"] } } }] },
        { steps: [{ effect: "untap-step-suppression", what: { ref: "that", filter: { types: ["creature"] }, plural: true },
          step: { occurrence: "next", owner: controllers } }] },
      ],
    });
  });

  it("retains an enclosing duration instead of making suppression permanent", () => {
    assert.deepEqual(only("Until end of turn, creatures don't untap during their controllers' untap steps."), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [{ effect: "untap-step-suppression", what: { ref: "each", filter: { types: ["creature"] } },
        step: { occurrence: "each", owner: controllers }, duration: { duration: "end-of-turn" } }] }],
    });
  });

  it("backtracks malformed steps and refuses unrepresented step qualifiers", () => {
    for (const text of [
      "This creature doesn't untap during its controller's next.",
      "This creature doesn't untap during untap step.",
      "This creature doesn't untap during your draw step.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseUntapSuppressionEffect(c), null, text);
      assert.equal(c.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
    for (const text of [
      "This creature doesn't untap during your second untap step.",
      "This creature doesn't untap during your untap step unless invented condition.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
