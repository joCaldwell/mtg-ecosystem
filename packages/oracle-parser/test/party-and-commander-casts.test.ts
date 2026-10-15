import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const enters = { trigger: "enters", what: { ref: "this", noun: "creature" } };

// CR 700.8a-b: the game computes the number of creatures in a player's party
// (zero to four, the highest possible result); nobody chooses the members.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("the number of creatures in your party", () => {
  const party = { amount: "party-size", who: { player: "you" } };
  it("is a quantity after \"the number of\" (Cascade Seer, Archpriest of Iona)", () => {
    assert.deepEqual(parsed("When this creature enters, scry X, where X is the number of creatures in your party.", "Cascade Seer", "Creature — Merfolk Wizard"),
      [{ kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "scry", amount: { amount: "x" } }], bindings: [{ variable: "x", value: party }] }] }]);
    assert.deepEqual(parsed("~'s power is equal to the number of creatures in your party.", "Archpriest of Iona", "Creature — Human Cleric"),
      [{ kind: "static", effect: { static: "cda-pt", what: { ref: "self" }, stat: "power", value: party } }]);
  });
  it("is a unit after \"for each\" (Shepherd of Heroes, Ardent Electromancer, Emeria Captain)", () => {
    assert.deepEqual(parsed("When this creature enters, you gain 2 life for each creature in your party.", "Shepherd of Heroes", "Creature — Angel Cleric"),
      [{ kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: { amount: "product", factors: [n(2), party] } }] }] }]);
    assert.deepEqual(parsed("When this creature enters, add {R} for each creature in your party.", "Ardent Electromancer", "Creature — Human Wizard"),
      [{ kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "scaled-symbol", symbol: "R", amount: party } }] }] }]);
    assert.deepEqual(parsed("When this creature enters, put a +1/+1 counter on it for each creature in your party.", "Emeria Captain", "Creature — Angel Warrior"),
      [{ kind: "triggered", trigger: enters, effects: [{ steps: [{ effect: "put-counters", on: { ref: "it" }, counter: { type: "+1/+1" }, count: { amount: "product", factors: [n(1), party] } }] }] }]);
  });
  it("rejects a party member type or another player's party", () => {
    fails("You gain 1 life for each Cleric in your party.");
    fails("Draw a card for each creature in their party.");
  });
});

// CR 903.8: casts of a commander from the command zone are counted (that is
// what commander tax counts); the unit is a cast, not mana paid.
describe("for each time you've cast your commander from the command zone this game", () => {
  const casts = { amount: "cast-count", who: { player: "you" }, what: { cls: "spell", commander: true, own: "you" }, during: "this-game", from: { zone: "command" } };
  it("multiplies the copies of a spell (Echo Storm)", () => {
    assert.deepEqual(parsed("When you cast this spell, copy it for each time you've cast your commander from the command zone this game. You may choose new targets for the copies.", "Echo Storm"),
      [{ kind: "triggered", trigger: { trigger: "cast", who: { player: "you" }, what: { ref: "this", noun: "spell" } }, effects: [{ steps: [{
        effect: "copy-stack-object", what: { ref: "trigger-event-object", event: "cast" }, who: { player: "you" },
        count: { amount: "product", factors: [n(1), casts] }, retarget: { who: { player: "you" }, optional: true } }] }] }]);
  });
  it("multiplies entry counters and a static pump (The Swarmlord, Vanguard of the Restless)", () => {
    assert.deepEqual(parsed("Rapid Regeneration — ~ enters with two +1/+1 counters on it for each time you've cast your commander from the command zone this game.", "The Swarmlord", "Legendary Creature — Tyranid"),
      [{ kind: "static", abilityWord: "Rapid Regeneration", effect: { static: "enters-with-counters", what: { ref: "self" }, counter: { type: "+1/+1" }, count: n(2), multiplier: casts } }]);
    assert.deepEqual(parsed("Spirits you control get +1/+1 for each time you've cast your commander from the command zone this game.", "Vanguard of the Restless", "Creature — Spirit Knight"),
      [{ kind: "static", effect: { static: "modify", what: { ref: "each", filter: { subtypes: ["Spirit"], control: "you" } }, modifiers: [{
        modifier: "pt", power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) }, multiplier: casts, valueScope: "each-affected-object" }] } }]);
  });
  it("rejects the count without its zone or period", () => {
    fails("When you cast this spell, copy it for each time you've cast your commander this game.");
    fails("When you cast this spell, copy it for each time you've cast your commander from the command zone.");
  });
});
