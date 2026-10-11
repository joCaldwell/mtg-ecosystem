import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect, ObjectRef, Sentence } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseSentences, type EffectContext } from "../src/parser/effects.ts";
import { parseDelayedPrefix, parseDelayedSuffix } from "../src/parser/delayed.ts";
import { parseOracleText } from "../src/index.ts";

// CR 603.3d, 603.7a–c, 603.12:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Oracle facts checked against Scryfall oracle corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
const you = { player: "you" } as const;
const itRef: ObjectRef = { ref: "it" };
const draw: Effect = { effect: "draw", who: you, amount: { amount: "fixed", value: 1 } };
const sacrifice: Effect = { effect: "sacrifice", who: you, what: itRef };

function prefix(text: string): Effect[] {
  const c = new Cursor(lex(text));
  const result = parseDelayedPrefix(c, { modalOptions: [] }, parseSentences);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return result;
}
function full(text: string): Sentence[] {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  const ability = result.abilities[0];
  assert.ok(ability.kind === "spell" || ability.kind === "activated" || ability.kind === "triggered");
  return ability.effects;
}

describe("delayed effects preserve timing and scope", () => {
  it("schedules a suffix action without executing it", () => {
    // Planebound Accomplice 01076e04-3106-4968-8ef4-b26fc3786699.
    const c = new Cursor(lex("at the beginning of the next end step"));
    assert.deepEqual(parseDelayedSuffix(c, [sacrifice]), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      effects: [{ steps: [sacrifice] }], occurrence: "next", nextOnly: true, targets: "enclosing-ability",
    }]);
    assert.equal(c.done(), true);
  });

  it("retains the specified turn owner and optional choice inside the future body", () => {
    const c = new Cursor(lex("at the beginning of your next end step"));
    assert.deepEqual(parseDelayedSuffix(c, [{ ...sacrifice, optional: true }]), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      effects: [{ steps: [{ ...sacrifice, optional: true }] }], occurrence: "next", nextOnly: true, targets: "enclosing-ability",
    }]);
  });

  it("keeps every object of a single instruction in the delayed body", () => {
    const targetCreature: ObjectRef = { ref: "target", count: { amount: "fixed", value: 1 }, filter: { types: ["creature"] } };
    const targetLand: ObjectRef = { ref: "target", count: { amount: "fixed", value: 1 }, filter: { types: ["land"] } };
    const effects: Effect[] = [{ effect: "destroy", what: targetCreature }, { effect: "destroy", what: targetLand }];
    const c = new Cursor(lex("at end of combat"));
    assert.deepEqual(parseDelayedSuffix(c, effects), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-of-combat", whose: "each-player" },
      effects: [{ steps: effects }], occurrence: "next", targets: "enclosing-ability",
    }]);
  });

  it("parses prefix bodies and keeps their later sentences within the future ability", () => {
    assert.deepEqual(prefix("At the beginning of your next end step, sacrifice it. If you do, draw a card."), [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [
        { steps: [sacrifice] },
        { steps: [draw], condition: { condition: "you-do" }, conditionKind: "if" },
      ],
    }]);
  });

  it("distinguishes reflexive triggering from a conditional effect", () => {
    assert.deepEqual(prefix("When you do, draw a card."), [{
      effect: "create-reflexive-trigger", event: { event: "preceding-action", who: you, performed: true },
      effects: [{ steps: [draw] }],
    }]);
    assert.deepEqual(prefix("When you don't, draw a card."), [{
      effect: "create-reflexive-trigger", event: { event: "preceding-action", who: you, performed: false },
      effects: [{ steps: [draw] }],
    }]);
  });

  it("rolls back malformed prefixes and isolates nested effect context", () => {
    const outerSubject: ObjectRef = { ref: "this", noun: "creature" };
    const ctx: EffectContext = { modalOptions: [], lastSubject: outerSubject };
    const c = new Cursor(lex("At the beginning of the next end step, nonsense."));
    assert.equal(parseDelayedPrefix(c, ctx, (nested, childCtx) => {
      nested.next();
      childCtx.lastSubject = itRef;
      return null;
    }), null);
    assert.equal(c.pos, 0);
    assert.deepEqual(ctx, { modalOptions: [], lastSubject: outerSubject });
    for (const text of [
      "At the beginning of the next turn's unsupported step, draw a card.",
      "At the beginning of your next, draw a card.",
      "When you do draw a card.",
      "When you do, draw a card. Unsupported instruction.",
    ]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseDelayedPrefix(cursor, ctx, parseSentences), null, text);
      assert.equal(cursor.pos, 0, text);
      assert.equal(ctx.lastSubject, outerSubject, text);
    }
  });

  it("rejects unsupported suffix scope rather than moving a dependent action to now", () => {
    for (const text of [
      "at the beginning of your next end step if it has mana value 3 or less",
      "at the beginning of the next end step, then draw a card.",
      "at end of combat. If you do, draw a card.",
      "at the beginning of the next turn's upkeep unexpectedly",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedSuffix(c, [sacrifice]), null, text);
      assert.equal(c.pos, 0, text);
    }
  });
});

describe("delayed effects in full ability parsing", () => {
  it("delays only the instruction before the suffix", () => {
    assert.deepEqual(full("Draw a card, then sacrifice it at the beginning of the next end step."), [{ steps: [draw, {
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      effects: [{ steps: [sacrifice] }], occurrence: "next", nextOnly: true, targets: "enclosing-ability",
    }] }]);
  });

  it("retains target selection by the enclosing activated ability", () => {
    // Silent Assassin 05b07f1f-42e1-41fb-8858-5e4a311c4ed9.
    const what: ObjectRef = { ref: "target", filter: { status: ["blocking"], types: ["creature"] }, count: { amount: "fixed", value: 1 } };
    assert.deepEqual(full("{3}{B}: Destroy target blocking creature at end of combat."), [{ steps: [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-of-combat", whose: "each-player" },
      effects: [{ steps: [{ effect: "destroy", what }] }], occurrence: "next", targets: "enclosing-ability",
    }] }]);
  });

  it("makes the optional choice at the delayed resolution", () => {
    assert.deepEqual(full("You may sacrifice it at the beginning of the next end step."), [{ steps: [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      effects: [{ steps: [{ ...sacrifice, optional: true }] }], occurrence: "next", nextOnly: true, targets: "enclosing-ability",
    }] }]);
  });

  it("creates a reflexive ability following the optional action", () => {
    assert.deepEqual(full("You may sacrifice a creature. When you do, draw a card."), [
      { steps: [{ effect: "sacrifice", who: you, what: { ref: "filter", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } }, optional: true }] },
      { steps: [{ effect: "create-reflexive-trigger", event: { event: "preceding-action", who: you, performed: true }, effects: [{ steps: [draw] }] }] },
    ]);
  });

  it("keeps activation restrictions outside a delayed body", () => {
    const result = parseOracleText("{1}: Draw a card. At the beginning of the next end step, sacrifice it. Activate only as a sorcery.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "mana", symbols: ["1"] }],
      restriction: { restriction: "sorcery" }, effects: [
        { steps: [draw] },
        { steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
          occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [{ steps: [sacrifice] }] }] },
      ],
    }]);
  });
});
