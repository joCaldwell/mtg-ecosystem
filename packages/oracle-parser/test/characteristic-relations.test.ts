import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseFilter, parseObjectRef, parseObjectSelection } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
const n = (value: number) => ({ amount: "fixed", value } as const);
const you = { player: "you" } as const;
function parsed(text: string, name?: string) {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function filter(text: string) {
  const c = new Cursor(lex(text)); const result = parseFilter(c);
  assert.ok(result); assert.ok(c.done(), text); return result;
}
const shared = (characteristic: string, what: object) => ({ relation: "shared-value", characteristic, with: { operand: "object", what } });
// Complete source fixtures checked in frozen Scryfall oracle_cards. CR201.2a/c
// multiple and absent names, CR105.2/205.2/3 characteristic sets:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("characteristic comparisons retain both sides and their scope", () => {
  it("keeps Harness the Storm's eligible graveyard card separate from the triggering spell", () => {
    assert.deepEqual(parsed("Whenever you cast an instant or sorcery spell from your hand, you may cast target card with the same name as that spell from your graveyard. (You still pay its costs.)", "Harness the Storm"), [{
      kind: "triggered", trigger: { trigger: "cast", who: you, what: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" }, from: { zone: "hand", owner: "your" } },
      effects: [{ steps: [{ effect: "play-card", instruction: { who: you, action: "cast", what: { ref: "target", filter: { cls: "card", characteristicRelations: [shared("name", { ref: "that", filter: { cls: "spell" } })] }, count: n(1), selectionId: "line:0:15" }, from: { zone: "graveyard", owner: "your" } }, optional: true }] }],
    }]);
  });
  it("retains Pack Hunt's one target inside the search comparison, count and all follow-up instructions", () => {
    assert.deepEqual(parsed("Search your library for up to three cards with the same name as target creature, reveal them, put them into your hand, then shuffle.", "Pack Hunt"), [{ kind: "spell", effects: [{ steps: [
      { effect: "search", who: you, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { cls: "card", characteristicRelations: [shared("name", { ref: "target", filter: { types: ["creature"] }, count: n(1), selectionId: "line:0:13" })] }, count: n(3), upTo: true } },
      { effect: "reveal", who: you, what: { ref: "them" } }, { effect: "move-zone", what: { ref: "them" }, to: { zone: "hand", owner: "your" } },
      { effect: "shuffle", who: you, library: { zone: "library", owner: you } },
    ] }] }]);
  });
  it("retains Pyroclast Consul's kinship condition, optional reveal and subsequent payment condition", () => {
    assert.deepEqual(parsed("Kinship — At the beginning of your upkeep, you may look at the top card of your library. If it shares a creature type with this creature, you may reveal it. If you do, this creature deals 2 damage to each creature.", "Pyroclast Consul"), [{ kind: "triggered", abilityWord: "Kinship", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, effects: [
      { steps: [{ effect: "look-at", who: you, what: { top: n(1), of: { zone: "library", owner: "your" } }, optional: true }] },
      { steps: [{ effect: "reveal", who: you, what: { ref: "it" }, optional: true }], condition: { condition: "characteristic-relation", what: { ref: "it" }, test: shared("creature-type", { ref: "this", noun: "creature" }) }, conditionKind: "if" },
      { steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(2), to: { to: "object", ref: { ref: "each", filter: { types: ["creature"] } } } }], condition: { condition: "you-do" }, conditionKind: "if" },
    ] }]);
  });
  it("represents existential and universal query operands without creating selections", () => {
    assert.deepEqual(filter("creature that shares a color with a creature you control"), { types: ["creature"], characteristicRelations: [{ relation: "shared-value", characteristic: "color", with: { operand: "matching-set", filter: { types: ["creature"], control: "you" }, quantifier: "any" } }] });
    assert.deepEqual(filter("Aura card with a different name than each Aura you control"), { subtypes: ["Aura"], cls: "card", characteristicRelations: [{ relation: "different-name", from: { operand: "matching-set", filter: { subtypes: ["Aura"], control: "you" }, quantifier: "every" } }] });
  });
  it("distinguishes different-name from the negation of a common name for nameless candidates", () => {
    const positive = filter("card with the same name as that card");
    assert.deepEqual(positive.characteristicRelations, [shared("name", { ref: "that", filter: { cls: "card" } })]);
    assert.deepEqual(filter("card that doesn't have the same name as that card").characteristicRelations, [{ relation: "not", of: positive.characteristicRelations![0] }]);
    assert.deepEqual(filter("card with a different name than that card").characteristicRelations, [{ relation: "different-name", from: { operand: "object", what: { ref: "that", filter: { cls: "card" } } } }]);
  });
  it("uses explicit attribute equality for scalar properties instead of comparing opaque descriptions", () => {
    assert.deepEqual(filter("creature with the same power as this creature"), { types: ["creature"], properties: [{ property: "power", comparison: { op: "eq", value: { amount: "attribute", attribute: "power", of: { ref: "this", noun: "creature" } } } }] });
    const result = parsed("Whenever an opponent casts a spell, you may reveal the top card of your library. If you do, counter that spell if it has the same mana value as the revealed card.", "Counterbalance");
    assert.deepEqual(result[0].effects[1].condition, { condition: "all-of", conditions: [{ condition: "you-do" }, { condition: "compare", value: { amount: "attribute", attribute: "mana-value", of: { ref: "it" } }, comparison: { op: "eq", value: { amount: "attribute", attribute: "mana-value", of: { ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "reveal", scope: "contextual" } } } } }] });
  });
  it("keeps Illuminated Folio's common color on the whole reveal payment group", () => {
    assert.deepEqual(parsed("{1}, {T}, Reveal two cards from your hand that share a color: Draw a card.", "Illuminated Folio"), [{ kind: "activated", costs: [
      { cost: "mana", symbols: ["1"] }, { cost: "tap-self" }, { cost: "reveal", what: { ref: "filter", filter: { cls: "card", zone: { zone: "hand", owner: "your" } }, groupConstraints: [{ constraint: "shared-value", characteristic: "color" }], count: n(2) } },
    ], effects: [{ steps: [{ effect: "draw", who: you, amount: n(1) }] }] }]);
  });
  it("retains Myriad Landscape's shared land type, optional selection count, tapped entry and sacrificed source", () => {
    assert.deepEqual(parsed("This land enters tapped.\n{T}: Add {C}.\n{2}, {T}, Sacrifice this land: Search your library for up to two basic land cards that share a land type, put them onto the battlefield tapped, then shuffle.", "Myriad Landscape"), [
      { kind: "static", effect: { static: "enters-tapped", what: { ref: "this", noun: "land" } } },
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "add-mana", who: you, mana: { mana: "fixed", symbols: ["C"] } }] }] },
      { kind: "activated", costs: [{ cost: "mana", symbols: ["2"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "land" } }], effects: [{ steps: [
        { effect: "search", who: you, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: { ref: "filter", filter: { supertypes: ["basic"], types: ["land"], cls: "card" }, groupConstraints: [{ constraint: "shared-value", characteristic: "land-type" }], count: n(2), upTo: true } },
        { effect: "move-zone", what: { ref: "them" }, to: { zone: "battlefield" }, tapped: true }, { effect: "shuffle", who: you, library: { zone: "library", owner: you } },
      ] }] },
    ]);
  });
  it("requires opt-in group selection and does not erase unsupported quantifiers or operand scope", () => {
    const text = "two creatures that share a creature type";
    const selected = new Cursor(lex(text)); const result = parseObjectSelection(selected);
    assert.ok(selected.done()); assert.ok(result && "groupConstraints" in result); assert.deepEqual(result.groupConstraints, [{ constraint: "shared-value", characteristic: "creature-type" }]);
    const ordinary = new Cursor(lex(text)); parseObjectRef(ordinary); assert.equal(ordinary.done(), false);
    for (const text of [
      "Whenever two creatures that share a creature type attack, draw a card.",
      "Destroy each creature that share a creature type.",
      "Destroy target creature that shares all its creature types with this creature.",
      "Destroy target creature that shares a color with two creatures you control.",
      "Destroy target creature that shares a color with a creature you control and with flying.",
      "Destroy target creature with the same colors as this creature.",
      "Destroy target creature that shares a color with that creature with flying.",
      "Destroy target creature with the same name as them.",
      "Destroy target creature with the same name.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it("retains target provenance while keeping relational binding obligations unresolved", () => {
    const text = "Destroy target creature with the same name as target creature.";
    const result = parseOracleText(text, "Relation fixture", { includeSourceProvenance: true }); assert.ok(result.ok);
    const graph = analyzeCardBindings({ identity: "relation-fixture", source: { name: "Relation fixture", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.message.includes("Characteristic relations")));
    assert.equal(new Set(JSON.stringify(result).match(/line:0:\d+/g)).size, 2);
  });
});
