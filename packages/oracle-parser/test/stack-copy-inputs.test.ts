import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect, ParseCardResult, Trigger } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseStackCopy, prepareStackCopyInput } from "../src/parser/stack-copy.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle corpus SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60:
// Melek, Thurid, Lucky Clover, Chandra Hope's Beacon, Rowan Kenrith,
// Jeong Jeong, Najal and The Mirari Conjecture distinguish these input scopes.
// CR 603.2c, 603.4, 605.3b, 707.10/b/c/f:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const cast: Trigger = { trigger: "cast", who: you, what: { cls: "spell" } };
const spell = { ref: "trigger-event-object", event: "cast" } as const;
const copied: Effect = { effect: "copy-stack-object", who: you, what: spell, count: one };
const retarget = { who: you, optional: true } as const;
const instantOrSorcery = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } as const;
function parsed(text: string, name = "Fixture"): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return result;
}
function graph(result: ParseCardResult) {
  return analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: "Enchantment" }, faces: [{ sourceFaceIndex: 0, result }] });
}

describe("proven first-instruction stack-copy inputs", () => {
  it("retains Melek's library casting restriction and exact event input", () => {
    assert.deepEqual(parsed("Whenever you cast an instant or sorcery spell from your library, copy it. You may choose new targets for the copy.", "Melek, Izzet Paragon").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "cast", who: you, what: instantOrSorcery, from: { zone: "library", owner: "your" } },
      effects: [{ steps: [{ ...copied, retarget }] }],
    }]);
  });
  it("preserves permanent spell copies without turning the operation into token creation", () => {
    assert.deepEqual(parsed("Flying, lifelink\nWhenever you cast a Pegasus, Unicorn, or Horse creature spell, copy it. (The copy becomes a token.)\nOther Pegasi, Unicorns, and Horses you control get +1/+1.", "Thurid, Mare of Destiny").abilities[1], {
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "cast", who: you, what: { allOf: [{ anyOf: [{ subtypes: ["Pegasus"] }, { subtypes: ["Unicorn"] }, { subtypes: ["Horse"] }] }], types: ["creature"], cls: "spell" } },
      effects: [{ steps: [copied] }],
    });
  });
  it("preserves the source Aura's enchanted card rather than any enchanted card", () => {
    const result = parsed("When you cast enchanted card, copy it.", "Don't Worry About It");
    assert.deepEqual(result.abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "cast", who: you, what: { ref: "enchanted", noun: "card" } },
      effects: [{ steps: [copied] }],
    }]);
    const binding = graph(parsed("When you cast enchanted card, copy it."));
    assert.equal(binding.fullyBound, false);
    assert.ok(binding.references.some(ref => "ref" in ref.expression && ref.expression.ref === "enchanted" && ref.binding === "unresolved"));
    assert.ok(binding.references.some(ref => "ref" in ref.expression && ref.expression.ref === "trigger-event-object" && ref.binding === "unresolved"));
    for (const [printed, what] of [
      ["this card", { ref: "this", noun: "card" }], ["that card", { ref: "that", filter: { cls: "card" } }],
      ["it", { ref: "it" }], ["equipped card", { ref: "equipped", noun: "card" }],
    ] as const) {
      const ability = parsed(`When you cast ${printed}, copy it.`).abilities[0];
      assert.ok(ability.kind === "triggered" && ability.trigger.trigger === "cast");
      assert.deepEqual(ability.trigger.what, what);
      assert.deepEqual(ability.effects, [{ steps: [copied] }]);
    }
  });
  it("preserves Adventure eligibility and once-per-turn trigger limits", () => {
    assert.deepEqual(parsed("Whenever you cast an Adventure instant or sorcery spell, copy it. You may choose new targets for the copy.", "Lucky Clover").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "cast", who: you, what: { subtypes: ["Adventure"], ...instantOrSorcery } },
      effects: [{ steps: [{ ...copied, retarget }] }],
    }]);
    assert.deepEqual(parsed("Whenever you cast an instant or sorcery spell, copy it. You may choose new targets for the copy. This ability triggers only once each turn.", "Chandra, Hope's Beacon").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "cast", who: you, what: instantOrSorcery },
      effects: [{ steps: [{ ...copied, retarget }] }], limit: { maximum: 1, period: "turn" },
    }]);
  });
  it("keeps optional copying and copy multiplicity separate from event cardinality", () => {
    assert.deepEqual(parsed("Whenever you cast a spell, you may copy it twice. You may choose new targets for the copies.").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined, trigger: cast,
      effects: [{ steps: [{ ...copied, count: { amount: "fixed", value: 2 }, optional: true, retarget }] }],
    }]);
  });
  it("requires explicit nonmana activation eligibility", () => {
    assert.deepEqual(parsed("Whenever you activate an ability that isn't a mana ability, copy it. You may choose new targets for the copy.").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "activate", who: you, what: { cls: "ability", manaAbility: false } },
      effects: [{ steps: [{ ...copied, what: { ref: "trigger-event-object", event: "activate" }, retarget }] }],
    }]);
    const neither = parsed("Whenever you activate an ability that isn't a mana ability or a loyalty ability, copy it.").abilities[0];
    assert.ok(neither.kind === "triggered" && neither.trigger.trigger === "activate");
    assert.deepEqual(neither.trigger.what, { cls: "ability", manaAbility: false, loyaltyAbility: false });
    for (const header of ["Whenever you activate an ability", "Whenever you activate a mana ability"])
      assert.equal(parseOracleText(`${header}, copy it.`).ok, false, header);
    assert.equal(prepareStackCopyInput(new Cursor(lex("copy it")), { trigger: "activate", who: you,
      what: { anyOf: [{ cls: "ability", manaAbility: false }, { cls: "ability", abilityKind: "activated" }] } }), undefined);
  });
  it("matches immutable token identity through slices and retries, never equal-looking later tokens", () => {
    const original = new Cursor(lex("copy it"));
    const proof = prepareStackCopyInput(original, cast);
    assert.ok(proof);
    assert.equal(original.pos, 0);
    assert.deepEqual(parseStackCopy(original, undefined, proof), copied);
    original.pos = 0;
    assert.deepEqual(parseStackCopy(original, undefined, proof), copied);
    assert.deepEqual(parseStackCopy(new Cursor(original.tokens.slice()), undefined, proof), copied);
    for (const cursor of [new Cursor(lex("copy it")), new Cursor(original.tokens.map(token => ({ ...token })))]) {
      assert.equal(parseStackCopy(cursor, undefined, proof), null);
      assert.equal(cursor.pos, 0);
    }
    const optional = new Cursor(lex("you may copy it"));
    const optionalProof = prepareStackCopyInput(optional, cast);
    optional.pos = 2; // Shared sentence code consumes this exact optional prefix.
    assert.deepEqual(parseStackCopy(optional, undefined, optionalProof), copied);
  });
  it("rejects competing occurrences, intervening clauses and preceding instructions", () => {
    for (const text of [
      "Whenever you cast a spell, draw a card. Copy it.",
      "Whenever you cast a spell, copy it. Copy it.",
      "Whenever you cast a spell, copy target spell and copy it.",
      "Whenever you cast a spell, if you control a creature, copy it.",
      "Whenever you cast a spell, you may pay {1}. If you do, copy it.",
      "Whenever you cast one or more spells, copy it.",
      "Whenever you cast a spell or activate an ability, copy it.",
      "Whenever a creature enters, copy it.",
      "Copy it.",
      "Counter target spell. Copy it.",
      "Exile target spell. Copy it.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    assert.equal(prepareStackCopyInput(new Cursor(lex("copy it")), { ...cast, grouping: "one-or-more" }), undefined);
    assert.equal(prepareStackCopyInput(new Cursor(lex("copy it")), { trigger: "cast", who: { player: "target-player" }, what: { cls: "spell" } }), undefined);
  });
  it("creates a fresh local delayed occurrence without borrowing its enclosing trigger", () => {
    assert.deepEqual(parsed("When you next cast an instant or sorcery spell this turn, copy it. You may choose new targets for the copy.").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "create-delayed-trigger",
        trigger: { trigger: "cast", who: you, what: instantOrSorcery }, occurrence: "next", nextOnly: true,
        duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [{ steps: [{ ...copied, retarget }] }],
      }] }],
    }]);
    const nested = parsed("Whenever you cast a spell, when you next cast a creature spell this turn, copy it.");
    const outer = nested.abilities[0];
    assert.equal(outer.kind, "triggered");
    if (outer.kind !== "triggered") throw new Error("expected trigger");
    assert.deepEqual(outer.effects, [{ steps: [{ effect: "create-delayed-trigger",
      trigger: { trigger: "cast", who: you, what: { types: ["creature"], cls: "spell" } },
      occurrence: "next", nextOnly: true, duration: { duration: "this-turn" }, targets: "delayed-ability", effects: [{ steps: [copied] }],
    }] }]);
    const binding = graph(nested);
    assert.ok(binding.references.some(ref => "ref" in ref.expression && ref.expression.ref === "trigger-event-object" && ref.binding === "unresolved"));
    for (const text of [
      "Whenever you cast a spell, copy it. At the beginning of the next end step, copy it.",
      "Whenever you cast a spell, at the beginning of the next end step, copy it.",
      "When you next cast a spell this turn, if you control a creature, copy it.",
      "Whenever you cast a spell, target creature gains \"{T}: Copy it.\" until end of turn.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it("binds the explicit input to its matching lexical event, while copy-result scopes remain unsupported", () => {
    const result = parsed("Whenever you cast a spell, copy it.");
    const binding = graph(result);
    const input = binding.references.find(ref => "ref" in ref.expression && ref.expression.ref === "trigger-event-object");
    assert.ok(input && input.binding === "bound", JSON.stringify(binding));
    const node = binding.nodes.find(node => node.id === input.target);
    assert.ok(node?.node === "event-result");
    assert.equal(node.cardinality, "one");
    assert.equal(node.identity, "same-objects");
    assert.deepEqual(node.description, { cls: "spell" });
    const event = binding.nodes.find(candidate => candidate.id === node.event);
    assert.ok(event?.node === "event" && event.operation === "trigger:cast");
    assert.equal(binding.fullyBound, false);
    assert.ok(binding.diagnostics.some(diagnostic => diagnostic.message.includes("Stack-copy results")));
    const mismatch = structuredClone(result);
    const ability = mismatch.abilities[0];
    if (ability.kind !== "triggered") throw new Error("expected trigger");
    ability.effects[0].steps = [{ ...copied, what: { ref: "trigger-event-object", event: "activate" } }];
    assert.ok(graph(mismatch).references.some(ref => "ref" in ref.expression && ref.expression.ref === "trigger-event-object" && ref.binding === "unresolved"));
  });
});
