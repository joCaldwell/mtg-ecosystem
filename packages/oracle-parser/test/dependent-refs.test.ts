import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ParseCardResult } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseAmount, parseFilter, parseObjectRef, parsePlayerRef, parsePossessivePlayer, parseZoneRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle fixtures from Scryfall bulk SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60:
// Ionize, Mesmeric Orb, Curse of the Restless Dead, Brenard, Kindred Discovery.
// CR 109.4, 205.3e, 303.4b/e, 607.2d, 608.2c/h:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text, "test"));
  const result = parser(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return result;
}
function incomplete<T>(parser: (c: Cursor) => T | null, text: string): void {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result === null || !c.done(), `must reject: ${text}`);
  if (result === null) assert.equal(c.pos, 0, "null restores cursor");
}
function parsed(text: string, name = ""): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  if (!result.ok) throw new Error(text);
  return result;
}

describe("dependent object possession and enchanted players", () => {
  it("keeps controller, owner, attachment and definite object identities distinct", () => {
    assert.deepEqual(complete(parsePlayerRef, "that spell's controller"), { player: "controller", of: { ref: "that", filter: { cls: "spell" } } });
    assert.deepEqual(complete(parsePlayerRef, "that creature's owner"), { player: "owner", of: { ref: "that", filter: { types: ["creature"] } } });
    assert.deepEqual(complete(parsePlayerRef, "enchanted creature's controller"), { player: "controller", of: { ref: "enchanted", noun: "creature" } });
    assert.deepEqual(complete(parsePlayerRef, "equipped creature's owner"), { player: "owner", of: { ref: "equipped", noun: "creature" } });
    assert.deepEqual(complete(parsePlayerRef, "the sacrificed creature's controller"), {
      player: "controller", of: { ref: "related", filter: { types: ["creature"] }, relation: { relation: "event-result", event: "sacrifice", scope: "contextual" }, number: "singular" },
    });
    assert.deepEqual(complete(parsePlayerRef, "enchanted player"), { player: "enchanted-player" });
    incomplete(parseObjectRef, "enchanted player");
  });
  it("retains target provenance through possessive token cloning", () => {
    const target = { ref: "target", filter: { types: ["creature"] }, count: fixed(1), selectionId: "test:0" };
    assert.deepEqual(complete(parsePlayerRef, "target creature's controller"), { player: "controller", of: target });
    assert.deepEqual(complete(parseAmount, "target creature's toughness"), { amount: "attribute", of: target, attribute: "toughness" });
  });
  it("reuses possession for zones, attributes, life totals and step owners", () => {
    assert.deepEqual(complete(parseZoneRef, "enchanted player's graveyard"), { zone: "graveyard", owner: { player: "enchanted-player" } });
    assert.deepEqual(complete(parseZoneRef, "that creature's controller's library"), { zone: "library", owner: { player: "controller", of: { ref: "that", filter: { types: ["creature"] } } } });
    assert.deepEqual(complete(parsePossessivePlayer, "enchanted creature's controller's"), { player: "controller", of: { ref: "enchanted", noun: "creature" } });
    assert.deepEqual(complete(parseZoneRef, "its owner's hand"), { zone: "hand", owner: "its-owner" });
    assert.deepEqual(complete(parseAmount, "enchanted creature's power"), { amount: "attribute", of: { ref: "enchanted", noun: "creature" }, attribute: "power" });
    assert.deepEqual(complete(parseAmount, "that spell's controller's life total"), { amount: "life-total", of: { player: "controller", of: { ref: "that", filter: { cls: "spell" } } } });
  });
  it("does not mistake a controller relation for an enchanted status", () => {
    assert.deepEqual(complete(parseFilter, "creatures enchanted player controls"), { types: ["creature"], controlledBy: { player: "enchanted-player" } });
    assert.deepEqual(complete(parseFilter, "enchanted creatures"), { types: ["creature"], status: ["enchanted"] });
    assert.deepEqual(complete(parseFilter, "lands enchanted creature's controller controls"), { types: ["land"], controlledBy: { player: "controller", of: { ref: "enchanted", noun: "creature" } } });
  });
  it("keeps an attached subtype noun distinct from a plural status filter", () => {
    // Utopia Sprawl and the Genju cycle use singular basic-land subtype nouns.
    assert.deepEqual(complete(parseObjectRef, "enchanted Forest"), { ref: "enchanted", noun: "forest" });
    assert.deepEqual(complete(parseObjectRef, "enchanted Forests"), { ref: "each", filter: { status: ["enchanted"], subtypes: ["Forest"] } });
    assert.deepEqual(complete(parsePlayerRef, "enchanted Forest's controller"), { player: "controller", of: { ref: "enchanted", noun: "forest" } });
    assert.deepEqual(complete(parseAmount, "enchanted Forest's power"), { amount: "attribute", of: { ref: "enchanted", noun: "forest" }, attribute: "power" });
    incomplete(parseObjectRef, "enchanted Inventedtype");
  });
  it("rejects plural, missing, nested and malformed possessors without picking one", () => {
    for (const text of ["those creatures' controller", "that creatures' controller", "all creatures' controller", "two target creatures' controller", "enchanted player's controller", "target creature enchanted player's controller", "that creature's invented controller", "that spell's controllers"])
      incomplete(parsePlayerRef, text);
    for (const text of ["that spell's controller's invented zone", "enchanted player's invented zone"]) incomplete(parseZoneRef, text);
  });
  it("preserves Ionize's reference to the countered spell's controller", () => {
    assert.deepEqual(parsed("Counter target spell. Ionize deals 2 damage to that spell's controller.", "Ionize").abilities, [
      { kind: "spell", abilityWord: undefined, effects: [
        { steps: [{ effect: "counter", what: { ref: "target", filter: { cls: "spell" }, count: fixed(1) } }] },
        { steps: [{ effect: "damage", source: { ref: "self" }, amount: fixed(2), to: { to: "player", ref: { player: "controller", of: { ref: "that", filter: { cls: "spell" } } } } }] },
      ] },
    ]);
  });
  it("preserves Mesmeric Orb's event-dependent mill actor", () => {
    assert.deepEqual(parsed("Whenever a permanent becomes untapped, that permanent's controller mills a card.", "Mesmeric Orb").abilities, [
      { kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "becomes-untapped", what: { ref: "filter", filter: { cls: "permanent" }, count: fixed(1) } },
        effects: [{ steps: [{ effect: "mill", who: { player: "controller", of: { ref: "that", filter: { cls: "permanent" } } }, amount: fixed(1) }] }] },
    ]);
  });
  it("keeps the Curse's event controller distinct from the token creator", () => {
    assert.deepEqual(parsed("Whenever a land enchanted player controls enters, you create a 2/2 black Zombie creature token with decayed.").abilities, [
      { kind: "triggered", abilityWord: undefined, condition: undefined,
        trigger: { trigger: "enters", what: { ref: "filter", filter: { types: ["land"], controlledBy: { player: "enchanted-player" } }, count: fixed(1) } },
        effects: [{ steps: [{ effect: "create-token", count: fixed(1), token: { colors: ["black"], supertypes: [], types: ["creature"], subtypes: ["Zombie"], keywords: [{ keyword: "decayed" }], power: fixed(2), toughness: fixed(2) } }] }] },
    ]);
  });
});

describe("relative characteristic predicates and prior type choices", () => {
  it("conjoins Brenard's outer eligibility with Food-or-Golem alternatives", () => {
    assert.deepEqual(parsed("Each creature you control that's a Food or a Golem gets +2/+2 and has trample.").abilities, [
      { kind: "static", abilityWord: undefined, effect: { static: "modify", condition: undefined, duration: undefined, what: { ref: "each", filter: { types: ["creature"], control: "you", anyOf: [{ subtypes: ["Food"] }, { subtypes: ["Golem"] }] } },
        modifiers: [{ modifier: "pt", power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(2) } }, { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }] }],
      } },
    ]);
  });
  it("preserves conjunction and disjunction in adjective-only predicates", () => {
    assert.deepEqual(complete(parseFilter, "creatures that are red and green"), { types: ["creature"], colors: ["red", "green"] });
    assert.deepEqual(complete(parseFilter, "creature or planeswalker that's white or blue"), { allOf: [ { anyOf: [{ types: ["creature"] }, { types: ["planeswalker"] }] }, { allOf: [{ anyOf: [{ colors: ["white"] }, { colors: ["blue"] }] }] } ] });
    assert.deepEqual(complete(parseFilter, "creatures that are tapped and attacking"), { types: ["creature"], status: ["tapped", "attacking"] });
    assert.deepEqual(complete(parseFilter, "creatures that are enchanted or equipped"), { types: ["creature"], allOf: [{ anyOf: [{ status: ["enchanted"] }, { status: ["equipped"] }] }] });
    for (const text of ["white or blue", "creature that's mysterious", "creature that's a Food or a Golem you control", "creatures that are tapped and attacking or blocking", "creature that's a Food or", "creatures that aren't Foods"]) incomplete(parseFilter, text);
  });
  it("keeps mixed adjective-or-noun alternatives inside the relative predicate", () => {
    assert.deepEqual(complete(parseFilter, "card that's red or a Goblin"), { cls: "card", anyOf: [{ colors: ["red"] }, { subtypes: ["Goblin"] }] });
    assert.deepEqual(complete(parseFilter, "card that's legendary or a creature"), { cls: "card", anyOf: [{ supertypes: ["legendary"] }, { types: ["creature"] }] });
    assert.deepEqual(complete(parseFilter, "card that's a Goblin or red"), { cls: "card", anyOf: [{ subtypes: ["Goblin"] }, { colors: ["red"] }] });
    assert.deepEqual(complete(parseFilter, "creature card that's red or blue or an artifact"), { types: ["creature"], cls: "card", anyOf: [
      { allOf: [{ anyOf: [{ colors: ["red"] }, { colors: ["blue"] }] }] }, { types: ["artifact"] },
    ] });
    incomplete(parseFilter, "card that's red or a Goblin you control");
  });
  it("does not infer the chosen type category from the selected object's type", () => {
    assert.deepEqual(complete(parseFilter, "creatures you control of the chosen type"), { types: ["creature"], control: "you", chosenType: { choice: "chosen-type" } });
    assert.deepEqual(complete(parseFilter, "creature spells of the chosen card type"), { types: ["creature"], cls: "spell", chosenType: { choice: "chosen-type", kind: "card" } });
    assert.deepEqual(complete(parseFilter, "permanents of the chosen creature type"), { cls: "permanent", chosenType: { choice: "chosen-type", kind: "creature" } });
    assert.deepEqual(complete(parseFilter, "cards of the chosen type revealed this way"), { cls: "card", chosenType: { choice: "chosen-type" }, relation: { relation: "event-result", event: "reveal", scope: "this-way" } });
    for (const text of ["creatures of the chosen invented type", "creatures of the last chosen type", "creatures of a chosen type", "creatures of the chosen type and unknown"]) incomplete(parseFilter, text);
  });
  it("retains the choice and both chosen-type events in Kindred Discovery", () => {
    const what = { ref: "filter", filter: { types: ["creature"], control: "you", chosenType: { choice: "chosen-type" } }, count: fixed(1) };
    assert.deepEqual(parsed("As this enchantment enters, choose a creature type.\nWhenever a creature you control of the chosen type enters or attacks, draw a card.", "Kindred Discovery").abilities, [
      { kind: "static", abilityWord: undefined, effect: { static: "as-enters", what: { ref: "this", noun: "enchantment" }, effects: [{ steps: [{ effect: "choose-value", who: { player: "you" }, category: "creature-type" }] }] } },
      { kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "any-of", events: [{ trigger: "enters", what }, { trigger: "attacks", what, alone: undefined, whom: undefined }] }, effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }] },
    ]);
  });
  it("keeps attachment and linked-choice obligations unresolved in binding analysis", () => {
    for (const [text, needle] of [["Enchanted player draws a card.", "attachment"], ["Creatures of the chosen type get +1/+1.", "linked choice"]]) {
      const result = parsed(text, "Binding fixture");
      const graph = analyzeCardBindings({ identity: "fixture", source: { name: "Binding fixture", layout: "normal", type_line: "Enchantment — Aura" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(graph.fullyBound, false);
      assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference" && diagnostic.message.includes(needle)), JSON.stringify(graph));
    }
  });
});
