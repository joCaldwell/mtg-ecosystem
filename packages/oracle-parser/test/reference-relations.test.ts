import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect, ParseCardResult } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { isPluralPlayerRef, isTargetedPlayerRef, parseFilter, parseObjectRef, parsePlayerRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle fixtures: Tinybones, Trinket Thief; Fractured Identity; Stifle;
// Talon of Pain; Abstruse Archaic; Greater Werewolf; Urborg Panther.
// Scryfall bulk SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 109.1/4, 113.3/7, 115.1, 120.1, 303.4, 509.1g-h, 605.1, 606.1:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const creature = { ref: "this", noun: "creature" } as const;
const handSize = { op: "eq", value: fixed(0) } as const;
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text, "test"));
  const value = parser(c);
  assert.ok(value, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return value;
}
function incomplete<T>(parser: (c: Cursor) => T | null, text: string): void {
  const c = new Cursor(lex(text));
  const value = parser(c);
  assert.ok(value === null || !c.done(), `must reject: ${text}`);
  if (value === null) assert.equal(c.pos, 0, "failed grammar restores cursor");
}
function parsed(text: string): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, "Fixture", { includeSourceProvenance: true });
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

describe("player eligibility retains quantifiers and target identity", () => {
  it("preserves hand, life, and controlled-object predicates with their comparisons", () => {
    assert.deepEqual(complete(parsePlayerRef, "each opponent with no cards in hand"), {
      player: "filtered", base: { player: "each-opponent" }, filter: { handSize },
    });
    assert.deepEqual(complete(parsePlayerRef, "target player with three or fewer cards in hand and with ten or more life"), {
      player: "filtered", base: { player: "target-player", selectionId: "test:0" },
      filter: { handSize: { op: "le", value: fixed(3) }, lifeTotal: { op: "ge", value: fixed(10) } },
    });
    for (const [text, comparison] of [
      ["each player who controls a land", { op: "ge", value: fixed(1) }],
      ["each player who controls no lands", { op: "eq", value: fixed(0) }],
      ["each player who controls exactly eight lands", { op: "eq", value: fixed(8) }],
      ["each player who controls eight or more lands", { op: "ge", value: fixed(8) }],
    ] as const) assert.deepEqual(complete(parsePlayerRef, text), {
      player: "filtered", base: { player: "each-player" }, filter: { controls: { filter: { types: ["land"] }, comparison } },
    });
    assert.deepEqual(complete(parsePlayerRef, "each opponent with ten or more life and with twenty or less life"), {
      player: "filtered", base: { player: "each-opponent" }, filter: {
        lifeTotal: { op: "ge", value: fixed(10) }, allOf: [{ lifeTotal: { op: "le", value: fixed(20) } }],
      },
    });
  });
  it("keeps explicit exclusions dependent on the correct object's controller", () => {
    assert.deepEqual(complete(parsePlayerRef, "each player other than its controller"), {
      player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "controller", of: { ref: "it" } } },
    });
    for (const text of ["each player other than target player with no cards in hand", "each player who controls eight lands", "each opponent who didn't discard a card this way", "each player with the most life", "each player with no cards in their graveyard"])
      incomplete(parsePlayerRef, text);
  });
  it("distinguishes a declared restricted target from a resolution choice", () => {
    assert.deepEqual(effects("Choose target player with no cards in hand."), [{ effect: "declare-target", what: {
      player: "filtered", base: { player: "target-player", selectionId: "line:0:1" }, filter: { handSize },
    } }]);
    assert.deepEqual(effects("Choose a player with no cards in hand."), [{ effect: "choose-object", who: { player: "you" }, what: {
      player: "filtered", base: { player: "a-player" }, filter: { handSize },
    } }]);
    const each = complete(parsePlayerRef, "each opponent with no cards in hand");
    const target = complete(parsePlayerRef, "target opponent with no cards in hand");
    assert.equal(isPluralPlayerRef(each), true);
    assert.equal(isTargetedPlayerRef(each), false);
    assert.equal(isPluralPlayerRef(target), false);
    assert.equal(isTargetedPlayerRef(target), true);
  });
  it("retains Tinybones' per-opponent eligibility on the life-loss action", () => {
    assert.deepEqual(parsed("{4}{B}{B}: Each opponent with no cards in hand loses 10 life.").abilities, [{
      kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "mana", symbols: ["4", "B", "B"] }],
      effects: [{ steps: [{ effect: "lose-life", who: { player: "filtered", base: { player: "each-opponent" }, filter: { handSize } }, amount: fixed(10) }] }],
    }]);
  });
  it("preserves Fractured Identity's excluded controller and each token creator", () => {
    assert.deepEqual(effects("Exile target nonland permanent. Each player other than its controller creates a token that's a copy of it."), [
      { effect: "exile", what: { ref: "target", filter: { nonTypes: ["land"], cls: "permanent" }, count: fixed(1), selectionId: "line:0:1" } },
      { effect: "create-token", count: fixed(1), token: { copyOf: { ref: "it" } }, who: { player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "controller", of: { ref: "it" } } } } },
    ]);
  });
  it("does not claim that base-target binding resolves candidate-player predicates", () => {
    const result = parsed("Target player with no cards in hand draws a card.");
    const graph = analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope" && d.message.includes("per-candidate")), JSON.stringify(graph));
    assert.ok(graph.nodes.some(n => n.node === "selection" && n.selection === "target" && "player" in n.expression && n.expression.player === "target-player" && n.expression.selectionId === "line:0:0"));
  });
});

describe("source roles and ability identities", () => {
  it("does not narrow a damage source to permanents", () => {
    assert.deepEqual(complete(parseObjectRef, "a noncreature source you control"), { ref: "filter", filter: { nonTypes: ["creature"], cls: "source", control: "you" }, count: fixed(1) });
    assert.deepEqual(complete(parseObjectRef, "a source you control other than this artifact"), { ref: "filter", filter: { cls: "source", control: "you", otherThan: { ref: "this", noun: "artifact" } }, count: fixed(1) });
    assert.deepEqual(complete(parseFilter, "creature spell source"), { types: ["creature"], cls: "source", allOf: [{ cls: "spell" }] });
    incomplete(parseObjectRef, "a source of your choice");
  });
  it("keeps ability category unions separate from source and mana constraints", () => {
    const kinds = [{ abilityKind: "activated" }, { abilityKind: "triggered" }];
    assert.deepEqual(effects("Counter target activated or triggered ability."), [{ effect: "counter", what: {
      ref: "target", filter: { cls: "ability", anyOf: kinds }, count: fixed(1), selectionId: "line:0:1",
    } }]);
    assert.deepEqual(complete(parseFilter, "activated and triggered abilities"), { cls: "ability", anyOf: kinds });
    assert.deepEqual(complete(parseFilter, "activated ability or triggered ability"), { anyOf: [{ cls: "ability", abilityKind: "activated" }, { cls: "ability", abilityKind: "triggered" }] });
    assert.deepEqual(complete(parseFilter, "nonmana activated abilities"), { cls: "ability", manaAbility: false, abilityKind: "activated" });
    assert.deepEqual(complete(parseFilter, "loyalty abilities"), { cls: "ability", loyaltyAbility: true });
    assert.deepEqual(complete(parseFilter, "ability that isn't a mana ability"), { cls: "ability", manaAbility: false });
    assert.deepEqual(complete(parseFilter, "abilities that aren't mana abilities or loyalty abilities"), { cls: "ability", manaAbility: false, loyaltyAbility: false });
    assert.deepEqual(complete(parseFilter, "ability that isn't a mana ability or a loyalty ability"), { cls: "ability", manaAbility: false, loyaltyAbility: false });
    assert.deepEqual(complete(parseFilter, "activated abilities of artifacts"), { cls: "ability", abilityKind: "activated", abilitySource: { ref: "each", filter: { types: ["artifact"] } } });
    for (const text of ["abilities that aren't mana abilities or triggered abilities", "abilities that aren't mana abilities or loyalty abilities or triggered abilities", "activated and triggered ability", "activated or triggered and static abilities", "invented abilities", "abilities from a creature", "abilities of creatures and you control"])
      incomplete(parseFilter, text);
  });
  it("preserves the controlled ability independently from its colorless source", () => {
    assert.deepEqual(complete(parseObjectRef, "target activated or triggered ability you control from a colorless source"), {
      ref: "target", count: fixed(1), selectionId: "test:0", filter: {
        cls: "ability", anyOf: [{ abilityKind: "activated" }, { abilityKind: "triggered" }], control: "you",
        abilitySource: { ref: "filter", filter: { colorless: true, cls: "source" }, count: fixed(1) },
      },
    });
    assert.deepEqual(complete(parseFilter, "abilities of creatures you control"), { cls: "ability", abilitySource: { ref: "each", filter: { types: ["creature"], control: "you" } } });
  });
});

describe("directed attachment, blocking, and identity exclusions", () => {
  it("retains attachment operands and any target declaration inside them", () => {
    assert.deepEqual(complete(parseFilter, "Equipment attached to target creature"), { subtypes: ["Equipment"], attachedTo: { ref: "target", filter: { types: ["creature"] }, count: fixed(1), selectionId: "test:3" } });
    assert.deepEqual(complete(parseFilter, "Auras attached to enchanted player"), { subtypes: ["Aura"], attachedTo: { player: "enchanted-player" } });
    assert.deepEqual(complete(parseFilter, "Aura or Equipment attached to it"), { allOf: [{ anyOf: [{ subtypes: ["Aura"] }, { subtypes: ["Equipment"] }] }], attachedTo: { ref: "it" } });
    assert.equal(parseOracleText("Creatures can block an additional creature each combat for each Equipment attached to target creature.").ok, false);
  });
  it("distinguishes blocking from blocked-by and keeps either-direction unions", () => {
    assert.deepEqual(complete(parseFilter, "creatures blocking this creature"), { types: ["creature"], blocking: creature });
    assert.deepEqual(complete(parseFilter, "creatures blocked by this creature"), { types: ["creature"], blockedBy: creature });
    assert.deepEqual(complete(parseFilter, "creatures blocking or blocked by this creature"), { types: ["creature"], anyOf: [{ blocking: creature }, { blockedBy: creature }] });
    assert.deepEqual(complete(parseFilter, "creatures it's blocking"), { types: ["creature"], blockedBy: { ref: "it" } });
    assert.deepEqual(complete(parseFilter, "creatures this creature is blocking"), { types: ["creature"], blockedBy: creature });
    assert.deepEqual(complete(parseFilter, "creature blocked by creatures you control"), { types: ["creature"], blockedBy: { ref: "each", filter: { types: ["creature"], control: "you" } } });
    assert.deepEqual(complete(parseFilter, "creature blocking a red and green creature"), { types: ["creature"], blocking: { ref: "filter", filter: { colors: ["red", "green"], types: ["creature"] }, count: fixed(1) } });
    assert.deepEqual(effects("Destroy target creature blocking it."), [{ effect: "destroy", what: { ref: "target", filter: { types: ["creature"], blocking: { ref: "it" } }, count: fixed(1), selectionId: "line:0:1" } }]);
  });
  it("keeps lexical other and explicit identity exclusions separately scoped", () => {
    const otherThan = { ref: "that", filter: { types: ["creature"] } };
    assert.deepEqual(complete(parseFilter, "creature other than that creature"), { types: ["creature"], otherThan });
    assert.deepEqual(complete(parseFilter, "another creature other than that creature"), { other: true, types: ["creature"], allOf: [{ otherThan }] });
    for (const text of ["creatures blocked by two creatures", "creatures blocked by all creatures", "creatures blocked by each creature", "creatures attached to up to one target creature", "abilities of two creatures", "creatures other than a creature", "creatures other than Goblins", "creatures other than those creatures", "creature blocking a creature and blocked by that creature", "Equipment attached to a creature and attached to this creature", "creature blocking or blocked by", "creatures blocked by or blocking a creature", "creatures with another Aura attached to them"])
      incomplete(parseFilter, text);
  });
});
