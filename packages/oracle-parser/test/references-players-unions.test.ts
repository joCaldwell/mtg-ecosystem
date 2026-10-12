import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parsePlayerRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// Oracle texts are the complete named cards from the fixed Scryfall corpus.
// CR 102.2–3 (opponents), 115.1 (one target whose legal set mixes players and
// objects), 608.2c (later text is read against the earlier target):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("opponent-or-object target unions", () => {
  const opponentOrPlaneswalker = { ref: "target", filter: { orOpponent: true, types: ["planeswalker"] }, count: n(1) };
  it("keeps Burning Fields' single target restricted to opponents on the player side", () => {
    assert.deepEqual(parsed("Burning Fields deals 5 damage to target opponent or planeswalker.", "Burning Fields", "Sorcery"), [{ kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "self" }, amount: n(5), to: { to: "object", ref: opponentOrPlaneswalker },
    }] }] }]);
  });
  it("differs from the any-player union of Blightning's target", () => {
    assert.deepEqual(parsed("Blightning deals 3 damage to target player or planeswalker. That player or that planeswalker's controller discards two cards.", "Blightning", "Sorcery"), [{ kind: "spell", effects: [
      { steps: [{ effect: "damage", source: { ref: "self" }, amount: n(3), to: { to: "object", ref: { ref: "target", filter: { orPlayer: true, types: ["planeswalker"] }, count: n(1) } } }] },
      { steps: [{ effect: "discard", who: { player: "that-player-or-controller", of: { ref: "that", filter: { types: ["planeswalker"] } } },
        what: { ref: "filter", filter: { cls: "card" }, count: n(2) } }] },
    ] }]);
  });
  it("retains Ayara's battle alternative and Flesh Reaver's event recipient", () => {
    assert.deepEqual(parsed("{T}, Sacrifice another creature or artifact: Ayara deals X damage to target opponent or battle and you gain X life, where X is the sacrificed permanent's mana value.", "Ayara", "Legendary Creature — Elf Noble")[0].effects[0].steps[0].to,
      { to: "object", ref: { ref: "target", filter: { orOpponent: true, types: ["battle"] }, count: n(1) } });
    assert.deepEqual(parsed("Whenever this creature deals damage to a creature or opponent, this creature deals that much damage to you.", "Flesh Reaver", "Creature — Phyrexian Horror"), [{
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, to: { ref: "filter", filter: { types: ["creature"], orOpponent: true }, count: n(1) } },
      effects: [{ steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: { amount: "that-much" }, to: { to: "player", ref: { player: "you" } } }] }],
    }]);
  });
  it("classifies the union as an object-or-player reference domain", () => {
    const text = "Burning Fields deals 5 damage to target opponent or planeswalker.";
    const result = parseOracleText(text, "Burning Fields", { includeSourceProvenance: true, typeLine: "Sorcery" });
    const graph = analyzeCardBindings({ identity: "burning-fields-fixture", source: { name: "Burning Fields", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.nodes.filter(node => node.node === "selection" && node.selection === "target").length, 1);
  });
  it("rejects unions that are not one object class plus one player class", () => {
    fails("Burning Fields deals 5 damage to target player or opponent.");
    fails("Burning Fields deals 5 damage to target creature, opponent, or planeswalker.");
    // A player subject is not an object-or-player selection.
    fails("Target opponent or planeswalker loses 3 life.");
  });
});

describe("player implied by one object-or-player antecedent", () => {
  const dependent = { player: "that-player-or-controller", of: { ref: "that", filter: { types: ["planeswalker"] } } };
  it("uses the same player inside Flame Wave's controller-scoped recipients", () => {
    assert.deepEqual(parsed("Flame Wave deals 4 damage to target player or planeswalker and each creature that player or that planeswalker's controller controls.", "Flame Wave", "Sorcery"), [{ kind: "spell", effects: [{ steps: [{
      effect: "damage", source: { ref: "self" }, amount: n(4), to: { to: "each", refs: [
        { ref: "target", filter: { orPlayer: true, types: ["planeswalker"] }, count: n(1) },
        { ref: "each", filter: { types: ["creature"], controlledBy: dependent } },
      ] },
    }] }] }]);
  });
  it("keeps Chandra's Fury's two quantities as one simultaneous instruction", () => {
    assert.deepEqual(parsed("Chandra's Fury deals 4 damage to target player or planeswalker and 1 damage to each creature that player or that planeswalker's controller controls.", "Chandra's Fury", "Instant"), [{ kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
      { effect: "damage", source: { ref: "self" }, amount: n(4), to: { to: "object", ref: { ref: "target", filter: { orPlayer: true, types: ["planeswalker"] }, count: n(1) } } },
      { effect: "damage", source: { ref: "self" }, amount: n(1), to: { to: "object", ref: { ref: "each", filter: { types: ["creature"], controlledBy: dependent } } } },
    ] }] }] }]);
  });
  it("gives the reversed Acorn Catapult printing the same node and the earlier any-target its own anaphor", () => {
    assert.deepEqual(parsed("{1}, {T}: This artifact deals 1 damage to any target. That permanent's controller or that player creates a 1/1 green Squirrel creature token.", "Acorn Catapult", "Artifact"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }], effects: [
        { steps: [{ effect: "damage", source: { ref: "this", noun: "artifact" }, amount: n(1), to: { to: "any-target" } }] },
        { steps: [{ effect: "create-token", count: n(1), who: { player: "that-player-or-controller", of: { ref: "that", filter: { cls: "permanent" } } },
          token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Squirrel"], keywords: [], power: n(1), toughness: n(1) } }] },
      ] }]);
    assert.deepEqual(parsed("Choose any target. Draw three cards, then discard a card. Blast of Genius deals damage equal to the discarded card's mana value to that permanent or player.", "Blast of Genius", "Sorcery")[0].effects[2].steps[0].to,
      { to: "object", ref: { ref: "that", filter: { cls: "permanent", orPlayer: true } } });
    // Only the anaphor is supported; a fresh selection keeps failing, and
    // "that permanent or player" never widens other class-or-player unions.
    fails("Blast of Genius deals 3 damage to target permanent or player.");
    fails("Choose any target. Blast of Genius deals 3 damage to that spell or player.");
  });
  it("is one unresolved reference, never a pair of players or a plural antecedent", () => {
    const player = new Cursor(lex("that player or that planeswalker's controller"));
    assert.deepEqual(parsePlayerRef(player), dependent);
    assert.ok(player.done());
    // Without the object's controller the OR does not belong to the player.
    const bare = new Cursor(lex("that player or that planeswalker"));
    assert.deepEqual(parsePlayerRef(bare), { player: "that-player" });
    assert.equal(bare.pos, 2);
    const plural = new Cursor(lex("that player or those planeswalkers' controller"));
    assert.deepEqual(parsePlayerRef(plural), { player: "that-player" });
    assert.equal(plural.pos, 2);
    fails("Blightning deals 3 damage to target player or planeswalker. That player or that planeswalker's owner discards two cards.");
    fails("Blightning deals 3 damage to target player or planeswalker. That player or target planeswalker's controller discards two cards.");
  });
});
