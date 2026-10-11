import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ObjectFilter, ObjectRef } from "../src/ast.ts";
import { buildCardNameCatalog } from "../src/card-names.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCardExtraction, parseObjectSelection } from "../src/parser/refs.ts";
import { parseCardExtractionGroup } from "../src/parser/card-selections.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Actual Oracle fixtures: Benefaction of Rhonas, Invent, Kaalia, Zenith
// Seeker, Krosan Verge, Alpine Houndmaster, Explore the Vastlands.
// Kaalia: a multitype card is assigned to just one extraction slot.
// https://magic.wizards.com/en/news/feature/core-set-2020-release-notes-2019-06-25
// Tragic Arrogance's reusable representative choices are deliberately separate:
// https://magic.wizards.com/en/news/feature/magic-origins-release-notes-2015-07-08
const n = (value: number) => ({ amount: "fixed", value } as const);
const card = (type: string): ObjectFilter => ({ types: [type], cls: "card" });
const catalog = buildCardNameCatalog([{ name: "Alpine Watchdog", layout: "normal" }, { name: "Igneous Cur", layout: "normal" }]);
const group = (filters: ObjectFilter[], source?: Extract<ObjectRef, { ref: "card-extraction-group" }>["source"], combine: "all" | "any-nonempty" = "any-nonempty"): ObjectRef => ({
  ref: "card-extraction-group", members: filters.map(filter => ({ filter, count: n(1) })), combine, allocation: "distinct-objects", ...(source ? { source } : {}),
});
function parsed(text: string) {
  const r = parseOracleText(text, undefined, { cardNames: catalog });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}
function selection(text: string): ObjectRef {
  const c = new Cursor(lex(text));
  const r = parseCardExtraction(c);
  assert.ok(r, c.errorMessage());
  assert.ok(c.done(), c.errorMessage());
  return JSON.parse(JSON.stringify(r));
}

describe("distinct card extraction slots", () => {
  it("keeps Benefaction's optional two-slot allocation and original pool complement", () => {
    assert.deepEqual(parsed("Reveal the top five cards of your library. You may put a creature card and/or an enchantment card from among them into your hand. Put the rest into your graveyard."), [{ kind: "spell", effects: [
      { steps: [{ effect: "reveal", who: { player: "you" }, what: { ref: "library-cards", position: "top", count: n(5), of: { zone: "library", owner: "your" } } }] },
      { steps: [{ effect: "move-zone", what: group([card("creature"), card("enchantment")], { kind: "pool", pool: { ref: "them" } }), to: { zone: "hand", owner: "your" }, optional: true }] },
      { steps: [{ effect: "move-zone", what: { ref: "rest" }, to: { zone: "graveyard", owner: "your" } }] },
    ] }]);
  });
  it("keeps Invent's per-slot printed count despite hidden-zone fail-to-find", () => {
    assert.deepEqual(parsed("Search your library for an instant card and/or a sorcery card, reveal them, put them into your hand, then shuffle."), [{ kind: "spell", effects: [{ steps: [
      { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: group([card("instant"), card("sorcery")]) },
      { effect: "reveal", who: { player: "you" }, what: { ref: "them" } },
      { effect: "move-zone", what: { ref: "them" }, to: { zone: "hand", owner: "your" } },
      { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } },
    ] }] }]);
    assert.deepEqual(selection("up to two creature cards and/or a land card from your graveyard"), { ref: "card-extraction-group", members: [
      { filter: card("creature"), count: n(2), upTo: true }, { filter: card("land"), count: n(1) },
    ], combine: "any-nonempty", allocation: "distinct-objects", source: { kind: "zone", zone: { zone: "graveyard", owner: "your" } } });
  });
  it("retains all three subtype allocations and a single shared pool in Kaalia", () => {
    const slots = group(["Angel", "Demon", "Dragon"].map(subtype => ({ subtypes: [subtype], cls: "card" })), { kind: "pool", pool: { ref: "them" } });
    assert.deepEqual(parsed("Look at the top six cards of your library. You may reveal an Angel card, a Demon card, and/or a Dragon card from among them and put them into your hand. Put the rest on the bottom of your library in a random order."), [{ kind: "spell", effects: [
      { steps: [{ effect: "look-at", who: { player: "you" }, what: { top: n(6), of: { zone: "library", owner: "your" } } }] },
      { steps: [{ effect: "optional-group", who: { player: "you" }, steps: [
        { effect: "reveal", who: { player: "you" }, what: slots }, { effect: "move-zone", what: { ref: "them" }, to: { zone: "hand", owner: "your" } },
      ] }] },
      { steps: [{ effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random" }] },
    ] }]);
  });
  it("keeps nested eligibility unions separate from independent slots", () => {
    assert.deepEqual(selection("a land card and/or an instant or sorcery card from among them"), group([
      card("land"), { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card" },
    ], { kind: "pool", pool: { ref: "them" } }));
    assert.deepEqual(selection("a creature card in your graveyard and/or a land card in your hand"), group([
      { ...card("creature"), zone: { zone: "graveyard", owner: "your" } }, { ...card("land"), zone: { zone: "hand", owner: "your" } },
    ]));
  });
  it("distinguishes conjunction from optional slot choice in Krosan Verge", () => {
    assert.deepEqual(parsed("{2}, {T}, Sacrifice this land: Search your library for a Forest card and a Plains card, put them onto the battlefield tapped, then shuffle."), [{ kind: "activated", costs: [
      { cost: "mana", symbols: ["2"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "land" } },
    ], effects: [{ steps: [
      { effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: group([{ subtypes: ["Forest"], cls: "card" }, { subtypes: ["Plains"], cls: "card" }], undefined, "all") },
      { effect: "move-zone", what: { ref: "them" }, to: { zone: "battlefield" }, tapped: true },
      { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: { player: "you" } } },
    ] }] }]);
  });
  it("keeps literal named cards as separate eligible slots", () => {
    const ast = parsed("Search your library for a card named Alpine Watchdog and/or a card named Igneous Cur.");
    assert.deepEqual(ast, [{ kind: "spell", effects: [{ steps: [{ effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] }, for: group([
      { cls: "card", name: { name: "literal", value: "Alpine Watchdog" } }, { cls: "card", name: { name: "literal", value: "Igneous Cur" } },
    ]) }] }] }]);
  });
  it("does not leak extraction allocation into target instances, costs, queries or reusable roles", () => {
    for (const text of [
      "Choose a creature and/or an enchantment.", "Choose a creature card and/or an enchantment card from among them.",
      "Sacrifice a creature and/or an enchantment: Draw a card.", "Discard a creature card and/or a land card: Draw a card.",
      "Whenever a creature card and/or a land card leaves your graveyard, draw a card.",
      "Return a creature card from your graveyard and/or a land card to your hand.",
      "Return a creature card and/or a land card from your graveyard to the battlefield from your hand.",
      "Return a creature card and a land card and/or an artifact card from your graveyard to your hand.",
      "Reveal a creature card and a land card with mana value 3 or less.",
      "Reveal a creature card and a land card with total mana value 3 or less.",
      "Reveal a creature card, a land card.", "Reveal a creature card and a land card, an artifact card.", "Reveal a creature card and/or another imaginary card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex("a creature card and/or target creature"));
    assert.equal(parseCardExtractionGroup(c, { parseSelection: parseObjectSelection }), null);
    assert.equal(c.pos, 0);
    const targeted = parseOracleText("Return target creature card and target land card to your hand.", undefined, { includeSourceProvenance: true });
    assert.ok(targeted.ok);
    const ids: string[] = [];
    const walk = (value: unknown) => { if (!value || typeof value !== "object") return; if ("selectionId" in value && typeof value.selectionId === "string") ids.push(value.selectionId); Object.values(value).forEach(walk); };
    walk(targeted.abilities);
    assert.equal(new Set(ids).size, 2);
    assert.ok(!JSON.stringify(targeted.abilities).includes("card-extraction-group"));
  });
  it("diagnoses separate allocation and result bindings rather than claiming complete references", () => {
    const r = parseOracleText("Search your library for an instant card and/or a sorcery card, reveal them.", "Invent", { includeSourceProvenance: true });
    const analysis = analyzeCardBindings({ identity: "extraction", source: { name: "Invent", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result: r }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(d => d.code === "unsupported-scope" && d.message.includes("card-extraction slots")), JSON.stringify(analysis));
  });
});
