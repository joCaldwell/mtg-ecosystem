import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parsePlayerRef, parsePossessivePlayer, parseZoneRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const chosen = (which: "chosen" | "last-chosen" = "chosen") => ({ player: "chosen-player", choice: { choice: "chosen-value", category: "player", which } } as const);
const you = { player: "you" } as const;
function parsed(text: string, name?: string) {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
// Complete Oracle text from the fixed Scryfall corpus. CR607.1c/607.2d:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("singular chosen-player references", () => {
  it("preserves Stuffy Doll's pre-entry choice, damage recipient and self-damage activation", () => {
    assert.deepEqual(parsed("Indestructible\nAs this creature enters, choose a player.\nWhenever this creature is dealt damage, it deals that much damage to the chosen player.\n{T}: This creature deals 1 damage to itself.", "Stuffy Doll"), [
      { kind: "keywords", keywords: [{ keyword: "indestructible" }] },
      { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "creature" }, effects: [{ steps: [{ effect: "choose-object", who: you, what: { player: "a-player" } }] }] } },
      { kind: "triggered", trigger: { trigger: "is-dealt-damage", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{ effect: "damage", source: { ref: "it" }, amount: { amount: "that-much" }, to: { to: "player", ref: chosen() } }] }] },
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(1), to: { to: "object", ref: { ref: "it" } } }] }] },
    ]);
  });
  it("retains The Rack's chosen upkeep owner and quantity definition", () => {
    assert.deepEqual(parsed("As this artifact enters, choose an opponent.\nAt the beginning of the chosen player's upkeep, this artifact deals X damage to that player, where X is 3 minus the number of cards in their hand.", "The Rack"), [
      { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "artifact" }, effects: [{ steps: [{ effect: "choose-object", who: you, what: { player: "an-opponent" } }] }] } },
      { kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: chosen() }, effects: [{
        steps: [{ effect: "damage", source: { ref: "this", noun: "artifact" }, amount: { amount: "x" }, to: { to: "player", ref: { player: "that-player" } } }],
        bindings: [{ variable: "x", value: { amount: "minus", a: n(3), b: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "their" } } } } }],
      }] },
    ]);
  });
  it("retains a chosen player's owned zone in Nyxathid's continuous quantity", () => {
    assert.deepEqual(parsed("As this creature enters, choose an opponent.\nThis creature gets -1/-1 for each card in the chosen player's hand.", "Nyxathid"), [
      { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "creature" }, effects: [{ steps: [{ effect: "choose-object", who: you, what: { player: "an-opponent" } }] }] } },
      { kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "pt", power: { sign: -1, amount: n(1) }, toughness: { sign: -1, amount: n(1) },
        forEach: { cls: "card", zone: { zone: "hand", owner: chosen() } },
      }] } },
    ]);
  });
  it("keeps Haunt the Network's announced target separate from its later chosen-player reference", () => {
    const text = "Choose target opponent. Create two 1/1 colorless Thopter artifact creature tokens with flying. Then the chosen player loses X life and you gain X life, where X is the number of artifacts you control.";
    assert.deepEqual(parsed(text, "Haunt the Network"), [{ kind: "spell", effects: [
      { steps: [{ effect: "declare-target", what: { player: "target-opponent", selectionId: "line:0:1" } }] },
      { steps: [{ effect: "create-token", count: n(2), token: { colors: [], supertypes: [], types: ["artifact", "creature"], subtypes: ["Thopter"], keywords: [{ keyword: "flying" }], power: n(1), toughness: n(1) } }] },
      { steps: [{ effect: "lose-life", who: chosen(), amount: { amount: "x" } }, { effect: "gain-life", who: you, amount: { amount: "x" } }],
        bindings: [{ variable: "x", value: { amount: "count", of: { types: ["artifact"], control: "you" } } }] },
    ] }]);
    const result = parseOracleText(text, "Haunt the Network", { includeSourceProvenance: true });
    const graph = analyzeCardBindings({ identity: "haunt-fixture", source: { name: "Haunt the Network", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    const reference = graph.references.find(r => "player" in r.expression && r.expression.player === "chosen-player");
    assert.equal(reference?.binding, "unresolved");
    assert.equal(graph.nodes.filter(n => n.node === "selection" && n.selection === "target").length, 1);
  });
  it("retains recency and possessives without inventing plural or counterpart choice semantics", () => {
    const player = new Cursor(lex("the last chosen player"));
    assert.deepEqual(parsePlayerRef(player), chosen("last-chosen"));
    assert.ok(player.done());
    const possessive = new Cursor(lex("the last chosen player's"));
    assert.deepEqual(parsePossessivePlayer(possessive), chosen("last-chosen"));
    assert.ok(possessive.done());
    const zone = new Cursor(lex("the last chosen player's graveyard"));
    assert.deepEqual(parseZoneRef(zone), { zone: "graveyard", owner: chosen("last-chosen") });
    assert.ok(zone.done());
    for (const text of ["The chosen players draw a card.", "The other chosen player draws a card.", "One of the chosen players draws a card.", "The first chosen player draws a card."])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});
