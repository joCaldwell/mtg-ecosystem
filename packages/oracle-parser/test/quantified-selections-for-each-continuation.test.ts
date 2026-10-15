import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: "For each X, <instruction>." followed by sentences that cannot
// refer to the loop. Oracle wordings checked
// in the frozen corpus: Selvala's Enforcer, Storm Fleet Negotiator, Selvala,
// Explorer Returned, Moogles' Valor, Heartless Conscription, Saheeli, the
// Gifted, Sylvan Primordial, Culmination of Studies.
// CR 608.2c/e/f: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const revealedNonland = { ref: "each", filter: { nonTypes: ["land"], cls: "card", relation: { relation: "event-result", event: "reveal", scope: "this-way" } } };
const reveal = { steps: [{ effect: "reveal", who: { player: "each-player" },
  what: { ref: "library-cards", position: "top", count: n(1), of: { zone: "library", owner: "their" } } }] };
const everyoneDraws = { steps: [{ effect: "draw", who: { player: "each-player" }, amount: n(1) }] };
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("independent sentences after a for-each", () => {
  it("keeps the loop as its own sentence and the later instruction outside it", () => {
    assert.deepEqual(parsed("Each player reveals the top card of their library. For each nonland card revealed this way, put a +1/+1 counter on this creature. Then each player draws a card.", "Sorcery"), [{
      kind: "spell", effects: [reveal,
        { steps: [{ effect: "for-each", each: revealedNonland, processing: "instruction-wise",
          effects: [{ steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: n(1) }] }] }] },
        everyoneDraws] }]);
  });
  it("keeps a two-step body inside the loop", () => {
    assert.deepEqual(parsed("Parley — {T}: Each player reveals the top card of their library. For each nonland card revealed this way, add {G} and you gain 1 life. Then each player draws a card.", "Legendary Creature — Elf Scout"), [{
      kind: "activated", abilityWord: "Parley", costs: [{ cost: "tap-self" }], effects: [reveal,
        { steps: [{ effect: "for-each", each: revealedNonland, processing: "instruction-wise", effects: [{ steps: [
          { effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["G"] } },
          { effect: "gain-life", who: { player: "you" }, amount: n(1) }] }] }] },
        everyoneDraws] }]);
  });
  it("accepts a later sentence about a freshly described group", () => {
    const [ability] = parsed("For each creature you control, create a 1/2 white Moogle creature token with lifelink. Then creatures you control gain indestructible until end of turn.", "Instant");
    assert.equal(ability.effects.length, 2);
    assert.equal(ability.effects[0].steps[0].effect, "for-each");
    assert.deepEqual(ability.effects[1], { steps: [{ effect: "gain-abilities", what: { ref: "each", filter: { types: ["creature"], control: "you" } },
      abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }], duration: { duration: "end-of-turn" } }] });
  });
  it("still rejects later text that refers to members, results, body events or body outcomes", () => {
    // With one object per member, a plural
    // reference to all of them ("those tokens", "each creature destroyed
    // this way") is said once, after the loop: a sibling sentence.
    assert.deepEqual(parsed("For each artifact you control, create a token that's a copy of it. Those tokens gain haste.", "Sorcery")[0].effects[1],
      { steps: [{ effect: "gain-abilities", what: { ref: "that", filter: { cls: "token" }, plural: true }, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] });
    // The definite "the tokens" is the same plural
    // anaphor as "those tokens" (formerly asserted to fail only because the
    // definite form was unrepresented).
    assert.deepEqual(parsed("For each creature you control, create a 1/1 white Soldier creature token. Exile the tokens at the beginning of the next end step.", "Sorcery"),
      parsed("For each creature you control, create a 1/1 white Soldier creature token. Exile those tokens at the beginning of the next end step.", "Sorcery"));
    assert.deepEqual(parsed("For each opponent, destroy target creature that player controls. Draw a card for each creature destroyed this way.", "Sorcery")[0].effects, [
      { steps: [{ effect: "for-each", each: { player: "each-opponent" }, processing: "instruction-wise", effects: [{ steps: [{ effect: "destroy",
        what: { ref: "target", filter: { types: ["creature"], control: "that-player" }, count: n(1) } }] }] }] },
      { steps: [{ effect: "draw", who: { player: "you" }, amount: n(1), forEach: { types: ["creature"], relation: { relation: "event-result", event: "destroy", scope: "this-way" } } }] }]);
    fails("For each opponent, you may pay {1}. If you do, draw a card.", "Sorcery");
    fails("For each opponent, you may draw a card. Otherwise, you gain 1 life.", "Sorcery");
    fails("For each blocking creature, flip a coin. If you win the flip, draw a card.", "Sorcery");
    fails("For each creature you control, draw a card. Then put the rest on the bottom of your library.", "Sorcery");
    // A following loop over the same earlier event still names "this way".
    fails("Exile the top X cards of your library. For each land card exiled this way, create a Treasure token. For each blue card exiled this way, draw a card.", "Sorcery");
  });
});
