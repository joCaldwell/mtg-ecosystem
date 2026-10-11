import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect, ParseCardResult } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseChooseNewTargets, parseStackCopy } from "../src/parser/stack-copy.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Scryfall oracle corpus SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60:
// Abstruse Archaic, Lithoform Engine, Teach by Example, Meletis Charlatan,
// Founding the Third Path and Psionic Ritual establish different copy domains.
// CR 707.10/b/c/f, 707.12, 115.7d-f:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const two = { amount: "fixed", value: 2 } as const;
const you = { player: "you" } as const;
const thatSpell = { ref: "that", filter: { cls: "spell" } } as const;
const copy = { ref: "copy-result", number: "singular" } as const;
const copies = { ref: "copy-result", number: "plural" } as const;
function parsed(text: string, name = "Fixture"): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return result;
}
function effects(text: string): Effect[] {
  const result = parsed(text);
  assert.equal(result.abilities.length, 1);
  const ability = result.abilities[0];
  assert.equal(ability.kind, "spell");
  if (ability.kind !== "spell") throw new Error("expected spell");
  return ability.effects.flatMap(sentence => sentence.steps);
}
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text, "test"));
  const value = parser(c);
  assert.ok(value, c.errorMessage());
  assert.ok(c.done(), text);
  return value;
}

describe("copying spells and abilities on the stack", () => {
  it("preserves Abstruse Archaic's ability eligibility, actor, count and copy referent", () => {
    assert.deepEqual(parsed("{1}, {T}: Copy target activated or triggered ability you control from a colorless source. You may choose new targets for the copy.").abilities, [{
      kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }], effects: [
        { steps: [{ effect: "copy-stack-object", who: you, count: one, retarget: { who: you, optional: true }, what: {
          ref: "target", count: one, selectionId: "line:0:5", filter: {
            cls: "ability", anyOf: [{ abilityKind: "activated" }, { abilityKind: "triggered" }], control: "you",
            abilitySource: { ref: "filter", filter: { colorless: true, cls: "source" }, count: one },
          },
        } }] },
      ],
    }]);
  });
  it("stores Reverberate and Teach by Example's target choices before copy placement", () => {
    const filter = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" };
    assert.deepEqual(parsed("Copy target instant or sorcery spell. You may choose new targets for the copy.", "Reverberate").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "copy-stack-object", who: you, count: one,
        what: { ref: "target", count: one, selectionId: "line:0:1", filter }, retarget: { who: you, optional: true },
      }] }],
    }]);
    assert.deepEqual(parsed("When you next cast an instant or sorcery spell this turn, copy that spell. You may choose new targets for the copy.", "Teach by Example").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "cast", who: you, what: filter },
        occurrence: "next", nextOnly: true, duration: { duration: "this-turn" }, targets: "delayed-ability",
        effects: [{ steps: [{ effect: "copy-stack-object", who: you, what: thatSpell, count: one, retarget: { who: you, optional: true } }] }],
      }] }],
    }]);
  });
  it("distinguishes permanent spells from permanent objects and cards", () => {
    assert.deepEqual(effects("Copy target permanent spell you control."), [{ effect: "copy-stack-object", who: you, count: one,
      what: { ref: "target", filter: { cls: "spell", permanent: true, control: "you" }, count: one, selectionId: "line:0:1" },
    }]);
    for (const text of ["Copy target permanent.", "Copy target creature.", "Copy that card.", "Copy target spell in your graveyard.", "Copy target spell or permanent.", "Copy target activated or static ability.", "Copy target static ability."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("refuses to guess that an untyped copied object is on the stack", () => {
    // A card can also be copied in exile before that copy is cast (707.12).
    // Even a nearby spell mention is insufficient for a local parser to bind it.
    for (const text of ["Copy it.", "Copy them.", "Exile target instant or sorcery card from a graveyard and copy it.", "Exile target instant or sorcery card. Copy it. You may cast the copy.", "Counter target spell. Copy it."])
      assert.equal(parseOracleText(text).ok, false, text);
    assert.deepEqual(complete(parseStackCopy, "copy this spell"), { effect: "copy-stack-object", who: you, count: one, what: { ref: "this", noun: "spell" } });
    assert.deepEqual(complete(parseStackCopy, "copy that spell or ability"), { effect: "copy-stack-object", who: you, count: one,
      what: { ref: "that", filter: { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }] } },
    });
  });
  it("retains copy multiplicity independently of the number of source targets", () => {
    assert.deepEqual(effects("Copy two target spells three times."), [{ effect: "copy-stack-object", who: you, count: { amount: "fixed", value: 3 },
      what: { ref: "target", filter: { cls: "spell" }, count: two, selectionId: "line:0:2" },
    }]);
    assert.deepEqual(effects("Copy that spell twice. You may choose new targets for the copies."), [
      { effect: "copy-stack-object", who: you, what: thatSpell, count: two, retarget: { who: you, optional: true } },
    ]);
    assert.deepEqual(effects("Copy that spell X times."), [{ effect: "copy-stack-object", who: you, what: thatSpell, count: { amount: "x" } }]);
    assert.deepEqual(effects("Copy that spell for each creature you control."), [{ effect: "copy-stack-object", who: you, what: thatSpell, count: one, forEach: { types: ["creature"], control: "you" } }]);
    for (const text of ["Copy that spell twice except the copies are red.", "Copy that spell for each creature it could target.", "Copy that spell three.", "Copy that spell -1 times.", "Copy that spell until end of turn."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("choosing new targets retains actors, references and choice scope", () => {
  it("preserves an explicit player and dependent controller independently", () => {
    assert.deepEqual(effects("That player copies that spell. That player may choose new targets for the copy."), [
      { effect: "copy-stack-object", who: { player: "that-player" }, what: thatSpell, count: one, retarget: { who: { player: "that-player" }, optional: true } },
    ]);
    assert.deepEqual(effects("Its controller may choose new targets for it."), [{
      effect: "choose-new-targets", who: { player: "controller", of: { ref: "it" } }, what: { ref: "it" }, timing: "resolution", optional: true,
    }]);
    assert.deepEqual(complete(parseChooseNewTargets, "choose new targets for each copy"), { effect: "choose-new-targets", who: you, what: copies, timing: "copy-creation" });
    assert.deepEqual(complete(parseChooseNewTargets, "choose new targets for them"), { effect: "choose-new-targets", who: you, what: { ref: "them" }, timing: "resolution" });
    assert.deepEqual(effects("Choose new targets for target spell."), [{ effect: "choose-new-targets", who: you, timing: "resolution", what: {
      ref: "target", filter: { cls: "spell" }, count: one, selectionId: "line:0:4",
    } }]);
  });
  it("keeps an elided plural actor and a single optional coordinated group", () => {
    assert.deepEqual(effects("They copy that spell and choose new targets for the copy."), [
      { effect: "copy-stack-object", who: { player: "they" }, what: thatSpell, count: one },
      { effect: "choose-new-targets", who: { player: "they" }, what: copy, timing: "copy-creation" },
    ]);
    assert.deepEqual(effects("You may copy that spell and choose new targets for the copy."), [{ effect: "optional-group", who: you, steps: [
      { effect: "copy-stack-object", who: you, what: thatSpell, count: one },
      { effect: "choose-new-targets", who: you, what: copy, timing: "copy-creation" },
    ] }]);
    // An explicitly different actor's optional coordinated group has not
    // acquired the generic parser's separate "you may" group implementation.
    for (const text of ["That player may copy that spell and choose new targets for the copy.", "Copies that spell.", "Chooses new targets for the copy.", "Choose new targets for target creature.", "Choose new targets for the copy except its first target.", "Choose new targets for the copies with different modes."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
  it("requires an explicit matching copy dependency across delayed-body sentences", () => {
    const prefix = "When you next cast a spell this turn, ";
    const first = effects(prefix + "copy that spell. You may choose new targets for the copy.")[0];
    assert.equal(first.effect, "create-delayed-trigger");
    if (first.effect !== "create-delayed-trigger") throw new Error("expected delayed trigger");
    assert.deepEqual(first.effects, [
      { steps: [{ effect: "copy-stack-object", who: you, what: thatSpell, count: one, retarget: { who: you, optional: true } }] },
    ]);
    assert.ok(parseOracleText(prefix + "copy that spell twice. You may choose new targets for the copies.").ok);
    for (const body of ["copy that spell. You may choose new targets for it.", "copy that spell. You may choose new targets for target spell.", "copy that spell twice. You may choose new targets for the copy."])
      assert.equal(parseOracleText(prefix + body).ok, false, body);
  });
  it("retains explicit creation timing when multiple earlier copies prevent folding", () => {
    assert.deepEqual(effects("Copy that spell. Copy target spell. You may choose new targets for the copy."), [
      { effect: "copy-stack-object", who: you, what: thatSpell, count: one },
      { effect: "copy-stack-object", who: you, what: { ref: "target", count: one, filter: { cls: "spell" }, selectionId: "line:0:5" }, count: one },
      { effect: "choose-new-targets", who: you, what: copy, timing: "copy-creation", optional: true },
    ]);
  });
  it("keeps copy-result bindings unresolved rather than borrowing the last target", () => {
    const result = parsed("Copy target spell. You may choose new targets for the copy.");
    const graph = analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: "Instant" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.message.includes("Stack-copy results")), JSON.stringify(graph));
    const unresolvedResult = parsed("Choose new targets for the copy.");
    const unresolvedGraph = analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: "Instant" }, faces: [{ sourceFaceIndex: 0, result: unresolvedResult }] });
    assert.equal(unresolvedGraph.fullyBound, false);
    assert.ok(unresolvedGraph.references.some(r => "ref" in r.expression && r.expression.ref === "copy-result" && r.binding === "unresolved"));
  });
  it("backtracks rejected copy and target-choice productions", () => {
    for (const [parser, text] of [[parseStackCopy, "copy a creature"], [parseStackCopy, "copy it"], [parseChooseNewTargets, "choose new targets for a card"], [parseChooseNewTargets, "that player may choose new targets for it and draw"]] as const) {
      const c = new Cursor(lex(text));
      assert.equal(parser(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });
});
