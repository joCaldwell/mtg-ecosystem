import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: counters as the unit of a "for each" rider.
// Oracle wordings checked in the frozen corpus:
// Orochi Hatchery, Canopy Crawler, Hooded Hydra, Gyre Sage, Everflowing
// Chalice, Bottomless Vault, Coalition Relic, Vish Kal, Shrine of Limitless
// Power, Essence Bottle, Azula, Ruthless Firebender.
// CR 122.1 (counters are not objects), 608.2h/i, 602.2b:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const times = (unit: Amount, base = 1): Amount => ({ amount: "product", factors: [n(base), unit] });
function parsed(text: string, typeLine?: string) {
  const result = parseOracleText(text, undefined, typeLine ? { typeLine } : undefined);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string, typeLine?: string) {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : undefined).ok, false, text);
}

describe("for each [kind] counter on an object or player", () => {
  const charge: Amount = { amount: "counter-count", counter: { type: "charge" }, on: { ref: "this", noun: "artifact" } };
  it("multiplies the token count by that object's counter count", () => {
    assert.deepEqual(parsed("{5}, {T}: Create a 1/1 green Snake creature token for each charge counter on this artifact.", "Artifact"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["5"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "create-token", count: times(charge),
        token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Snake"], keywords: [], power: n(1), toughness: n(1) } }] }] }]);
  });
  it("scales both halves of a pump and keeps its duration", () => {
    const counters: Amount = { amount: "counter-count", counter: { type: "+1/+1" }, on: { ref: "this", noun: "creature" } };
    assert.deepEqual(parsed("{T}: Target creature gets +1/+1 until end of turn for each +1/+1 counter on this creature.", "Creature — Insect"), [{
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: n(1) },
        power: { sign: 1, amount: times(counters) }, toughness: { sign: 1, amount: times(counters) }, duration: { duration: "end-of-turn" } }] }] }]);
  });
  it("keeps a pronoun holder symbolic and multiplies a printed base quantity", () => {
    assert.deepEqual(parsed("When this creature dies, you gain 2 life for each -1/-1 counter on it.", "Creature — Horror"), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "gain-life", who: { player: "you" },
        amount: times({ amount: "counter-count", counter: { type: "-1/-1" }, on: { ref: "it" } }, 2) }] }] }]);
  });
  it("is the same production as 'an amount of {G} equal to' for one mana symbol", () => {
    const viaForEach = parsed("{T}: Add {C} for each charge counter on this artifact.", "Artifact");
    assert.deepEqual(viaForEach, [{ kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "scaled-symbol", symbol: "C", amount: charge } }] }] }]);
    assert.deepEqual(viaForEach, parsed("{T}: Add an amount of {C} equal to the number of charge counters on this artifact.", "Artifact"));
  });
  it("turns 'discards a card for each' into one discard of that many cards", () => {
    assert.deepEqual(parsed("{4}, {T}, Sacrifice this artifact: Target player discards a card for each charge counter on this artifact.", "Artifact"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "artifact" } }],
      effects: [{ steps: [{ effect: "discard", who: { player: "target-player" },
        what: { ref: "filter", filter: { cls: "card" }, count: times(charge) } }] }] }]);
  });
  it("counts a single player's own counters", () => {
    const experience: Amount = { amount: "counter-count", counter: { type: "experience" }, on: { player: "you" } };
    assert.deepEqual(parsed("{2}{B}: Until end of turn, ~ gets +1/+1 for each experience counter you have and gains menace.", "Legendary Creature — Human"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2", "B"] }],
      effects: [{ steps: [
        { effect: "pump", what: { ref: "self" }, power: { sign: 1, amount: times(experience) }, toughness: { sign: 1, amount: times(experience) }, duration: { duration: "end-of-turn" } },
        { effect: "gain-abilities", what: { ref: "self" }, abilities: [{ gained: "keyword", keyword: { keyword: "menace" } }], duration: { duration: "end-of-turn" } },
      ] }] }]);
  });
  it("still counts objects, not counters, when the unit is an object with a counter", () => {
    assert.deepEqual(parsed("Draw a card for each creature you control with a +1/+1 counter on it.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1),
        forEach: { types: ["creature"], control: "you", counterConstraints: [{ counter: { type: "+1/+1" }, comparison: { op: "ge", value: n(1) } }] } }] }] }]);
  });
  it("rejects kinds of counters, groups of counters, summed players and unscalable instructions", () => {
    fails("When this creature enters, draw a card for each kind of counter on target permanent.", "Creature — Elf");
    fails("{T}: Draw a card for each five counters on this artifact.", "Artifact");
    // Collective possession is an explicit total over the opponents.
    assert.deepEqual(parsed("At the beginning of your upkeep, create a 1/1 green Phyrexian Insect creature token with infect for each poison counter your opponents have.", "Creature — Phyrexian"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, effects: [{ steps: [{ effect: "create-token",
        count: times({ amount: "counter-count", counter: { type: "poison" }, on: { player: "each-opponent" }, total: true }),
        token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Phyrexian", "Insect"], keywords: [{ keyword: "infect" }], power: n(1), toughness: n(1) } }] }] }]);
    fails("At the beginning of your end step, create a 1/1 green Insect creature token for each +1/+1 counter you've put on creatures under your control this turn.", "Creature — Insect");
    // Counters on several holders have no stated summed or per-holder reading.
    fails("{T}: Draw a card for each poison counter on each opponent.", "Artifact");
    fails("{T}: Draw a card for each +1/+1 counter on creatures you control.", "Artifact");
    // Two symbols per counter have no scaled production; destroying has no quantity.
    fails("{T}: Add {C}{C} for each charge counter on this artifact.", "Artifact");
    fails("{T}: Destroy target creature for each charge counter on this artifact.", "Artifact");
  });
});

describe("for each [kind] counter removed this way", () => {
  const removed = (type: string): Amount => ({ amount: "event-quantity", event: "counter-removal", counter: { type }, scope: "this-way" });
  it("refers to a removal made as a cost of the same ability", () => {
    assert.deepEqual(parsed("{T}, Remove any number of storage counters from this land: Add {B} for each storage counter removed this way.", "Land"), [{
      kind: "activated",
      costs: [{ cost: "tap-self" }, { cost: "remove-counters", counter: { type: "storage" }, count: { amount: "chosen", minimum: 0 }, from: { ref: "this", noun: "land" } }],
      effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "scaled-symbol", symbol: "B", amount: removed("storage") } }] }] }]);
  });
  it("refers to a removal made by the preceding instruction and scales a symbolic production", () => {
    assert.deepEqual(parsed("At the beginning of your first main phase, remove all charge counters from this artifact. Add one mana of any color for each charge counter removed this way.", "Artifact"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "precombat-main", whose: "your" },
      effects: [
        { steps: [{ effect: "remove-counters", counter: { type: "charge" }, count: "all", from: { ref: "this", noun: "artifact" } }] },
        { steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "any-color", amount: times(removed("charge")) } }] },
      ] }]);
  });
  it("is an event quantity, never the current counter count", () => {
    const [ability] = parsed("{T}, Remove all elixir counters from this artifact: You gain 2 life for each elixir counter removed this way.", "Artifact");
    assert.deepEqual(ability.effects, [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: times(removed("elixir"), 2) }] }]);
    const [current] = parsed("{T}: You gain 2 life for each elixir counter on this artifact.", "Artifact");
    assert.deepEqual(current.effects, [{ steps: [{ effect: "gain-life", who: { player: "you" },
      amount: times({ amount: "counter-count", counter: { type: "elixir" }, on: { ref: "this", noun: "artifact" } }, 2) }] }]);
  });
  it("has the same meaning in amount position", () => {
    const [ability] = parsed("{T}, Remove any number of stun counters from this artifact: Draw cards equal to the number of stun counters removed this way.", "Artifact");
    assert.deepEqual(ability.effects, [{ steps: [{ effect: "draw", who: { player: "you" }, amount: removed("stun") }] }]);
  });
  it("rejects a group size and a different event scope", () => {
    fails("Remove all +1/+1 counters from this creature: Draw a card for each five counters removed this way.", "Creature — Human Wizard");
    fails("{T}: Draw a card for each charge counter removed this turn.", "Artifact");
  });
});
