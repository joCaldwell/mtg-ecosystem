import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ObjectFilter } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseFilter, parseObjectSelection } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Oracle facts: Circuitous Route, Map the Frontier, Reach the Horizon,
// Brightglass Gearhulk, Guardian of the Great Door, Sky Hussar, Nashi,
// Henrika, Infernal Seer. CR608.2c and target occurrence identity CR115.3:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value } as const);
const types = (...names: string[]) => ({ anyOf: names.map(type => ({ types: [type] })) });
function filter(text: string): ObjectFilter {
  const c = new Cursor(lex(text));
  const f = parseFilter(c);
  assert.ok(f, `${text}: ${c.errorMessage()}`);
  assert.ok(c.done(), `${text}: ${c.errorMessage()}`);
  return JSON.parse(JSON.stringify(f));
}
function parsed(text: string, provenance = false) {
  const r = parseOracleText(text, undefined, { includeSourceProvenance: provenance });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}

describe("candidate unions retain their single enclosing quantity", () => {
  it("keeps basic local to the land branch, with one total search count", () => {
    assert.deepEqual(parsed("Search your library for up to two basic land cards and/or Gate cards, put them onto the battlefield tapped, then shuffle."), [{ kind: "spell", effects: [{ steps: [
      { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: {
        ref: "filter", filter: { cls: "card", anyOf: [{ supertypes: ["basic"], types: ["land"] }, { subtypes: ["Gate"] }] }, count: n(2), upTo: true,
      } },
      { effect: "move-zone", what: { ref: "them" }, to: { zone: "battlefield" }, tapped: true },
      { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } },
    ] }] }]);
    const c = new Cursor(lex("up to two basic land cards and/or Town cards with different names"));
    assert.deepEqual(parseObjectSelection(c), { ref: "filter", count: n(2), upTo: true, filter: {
      cls: "card", anyOf: [{ supertypes: ["basic"], types: ["land"] }, { subtypes: ["Town"] }],
    }, groupConstraints: [{ constraint: "pairwise-different", property: "name" }] });
    assert.ok(c.done());
  });
  it("shares trailing card eligibility across all members of a nominal list", () => {
    assert.deepEqual(filter("artifact, creature, and/or enchantment cards with mana value 1 or less"), {
      allOf: [types("artifact", "creature", "enchantment")], cls: "card", properties: [{ property: "mana-value", comparison: { op: "le", value: n(1) } }],
    });
    assert.deepEqual(filter("basic land cards and/or Desert cards from your graveyard"), {
      cls: "card", anyOf: [{ supertypes: ["basic"], types: ["land"] }, { subtypes: ["Desert"] }], zone: { zone: "graveyard", owner: "your" },
    });
    assert.deepEqual(filter("artifact cards, creature cards, and/or enchantment cards"), { cls: "card", anyOf: [
      { types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] },
    ] });
  });
  it("keeps a comma-and singular nominal enumeration as one eligibility union", () => {
    // Complete Oracle text of Toil to Renown. A tapped artifact creature is
    // counted once, and the final singular land is part of the same union.
    assert.deepEqual(parsed("You gain 1 life for each tapped artifact, creature, and land you control."), [{ kind: "spell", effects: [{ steps: [{
      effect: "gain-life", who: { player: "you" }, amount: n(1),
      forEach: { status: ["tapped"], allOf: [types("artifact", "creature", "land")], control: "you" },
    }] }] }]);
  });
  it("does not collapse Release's separately quantified sacrifices into one union", () => {
    // Complete Release face. Each repeated article is a separate instruction
    // role; treating the entire list as one count-one union loses sacrifices.
    const text = "Each player sacrifices an artifact, a creature, an enchantment, a land, and a planeswalker of their choice.\nFuse (You may cast one or both halves of this card from your hand.)";
    const result = parseOracleText(text, "Release", { layout: "split", typeLine: "Sorcery" });
    assert.equal(result.ok, false);
    assert.equal(result.lines[0]?.ok, false);
    assert.equal(result.lines[1]?.ok, true);
    const c = new Cursor(lex("an artifact, a creature, and a land"));
    parseObjectSelection(c);
    assert.equal(c.done(), false, "repeated articles must not become a predicate union");
  });
  it("preserves Lovisa's repeated-article OR inside an explicit relative predicate", () => {
    assert.deepEqual(parsed("Each creature that's a Barbarian, a Warrior, or a Berserker gets +2/+2 and has haste."), [{ kind: "static", effect: {
      static: "modify", what: { ref: "each", filter: { types: ["creature"], anyOf: [
        { allOf: [{ anyOf: [{ subtypes: ["Barbarian"] }, { subtypes: ["Warrior"] }] }] },
        { subtypes: ["Berserker"] },
      ] } }, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: n(2) }, toughness: { sign: 1, amount: n(2) } },
        { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] },
      ],
    } }]);
    for (const text of [
      "Each creature that's a Barbarian, a Warrior, and a Berserker gets +2/+2 and has haste.",
      "Each creature that's a Barbarian, a Warrior gets +2/+2 and has haste.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it("preserves one target-word identity for any number of eligible types", () => {
    assert.deepEqual(parsed("Tap X target artifacts, creatures, and/or lands. You lose X life.", true), [{ kind: "spell", effects: [
      { steps: [{ effect: "tap", what: { ref: "target", count: { amount: "x" }, selectionId: "line:0:2", filter: { allOf: [types("artifact", "creature", "land")] } } }] },
      { steps: [{ effect: "lose-life", who: { player: "you" }, amount: { amount: "x" } }] },
    ] }]);
  });
  it("preserves shared untapped and control restrictions in a single payment", () => {
    assert.deepEqual(parsed("Tap four untapped artifacts, creatures, and/or lands you control: Draw a card."), [{ kind: "activated", costs: [{ cost: "tap-objects", what: {
      ref: "filter", count: n(4), filter: { status: ["untapped"], allOf: [types("artifact", "creature", "land")], control: "you" },
    } }], effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] }]);
  });
  it("distinguishes color/status union from ordinary conjunction", () => {
    assert.deepEqual(filter("untapped white and/or blue creatures you control"), { status: ["untapped"], allOf: [{ anyOf: [{ colors: ["white"] }, { colors: ["blue"] }] }], types: ["creature"], control: "you" });
    assert.deepEqual(filter("white and blue creatures"), { colors: ["white", "blue"], types: ["creature"] });
    assert.deepEqual(filter("goaded and/or suspected creatures"), { allOf: [{ anyOf: [{ status: ["goaded"] }, { status: ["suspected"] }] }], types: ["creature"] });
    assert.deepEqual(filter("goaded and suspected creatures"), { status: ["goaded", "suspected"], types: ["creature"] });
  });
  it("distinguishes mixed characteristic unions and ability predicates", () => {
    assert.deepEqual(filter("legendary and/or enchantment cards from among them"), { cls: "card", anyOf: [{ supertypes: ["legendary"] }, { types: ["enchantment"] }], fromAmong: { ref: "them" } });
    assert.deepEqual(filter("legendary enchantment cards"), { supertypes: ["legendary"], types: ["enchantment"], cls: "card" });
    assert.deepEqual(filter("creatures with flying, deathtouch, and/or lifelink"), { types: ["creature"], anyOf: [{ withAbility: ["flying"] }, { withAbility: ["deathtouch"] }, { withAbility: ["lifelink"] }] });
    assert.deepEqual(filter("creatures with flying and lifelink"), { types: ["creature"], withAbility: ["flying", "lifelink"] });
  });
  it("does not turn repeated singular slots or ambiguous modifiers into one union", () => {
    for (const text of [
      "Choose a creature card and/or an enchantment card from your graveyard.",
      "Destroy target nonlegendary creature and/or artifact creature you control.",
      "Destroy target artifact and/or enchantment creature.",
      "Destroy target white and blue or red creature.",
      "Destroy target creature with flying and lifelink or haste.",
      "Search your library for two basic land cards and/or, then shuffle.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
