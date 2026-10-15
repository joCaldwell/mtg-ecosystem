import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";

const n = (value: number) => ({ amount: "fixed", value });
const all = { shield: "events", occurrence: "each", prevent: { prevent: "all" } };
const you = { player: "you" };
const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
function one(text: string) {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const spell = (step: unknown) => ({ kind: "spell", effects: [{ steps: [step] }] });

describe("prevention shields", () => {
  it("keeps a static prevention ability permanent and distinguishes damage kinds", () => {
    // Candletrap and Blessed Sanctuary, checked against the cached corpus.
    assert.deepEqual(one("Prevent all combat damage that would be dealt by enchanted creature."), {
      kind: "static", effect: { static: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: all, kind: "combat", by: { ref: "enchanted", noun: "creature" } } },
    });
    assert.deepEqual(one("Prevent all noncombat damage that would be dealt to you and creatures you control."), {
      kind: "static", effect: { static: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: all, kind: "noncombat", to: [you, { ref: "each", filter: { types: ["creature"], control: "you" } }] } },
    });
    assert.deepEqual(one("Prevent all combat damage that would be dealt this turn."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: all, kind: "combat" }, duration: { duration: "this-turn" },
    }));
  });
  it("retains finite shields and separate per-creature allocations", () => {
    assert.deepEqual(one("{T}: Prevent the next 1 damage that would be dealt to any target this turn."), {
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{
        effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: { shield: "damage-budget", amount: n(1) }, kind: "all", to: [{ ref: "any-target" }] }, duration: { duration: "this-turn" },
      }] }],
    });
    assert.deepEqual(one("Prevent the next 3 damage that would be dealt to each creature this turn."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "each" }, shield: { shield: "damage-budget", amount: n(3), allocation: "per-recipient" }, kind: "all", to: [{ ref: "each", filter: { types: ["creature"] } }] }, duration: { duration: "this-turn" },
    }));
    assert.deepEqual(one("Prevent the next 3 damage that would be dealt to you."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: { shield: "damage-budget", amount: n(3) }, kind: "all", to: [you] },
    }));
    assert.deepEqual(one("Prevent the next 3 damage that would be dealt to each player this turn."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "each" }, shield: { shield: "damage-budget", amount: n(3), allocation: "per-recipient" }, kind: "all", to: [{ player: "each-player" }] }, duration: { duration: "this-turn" },
    }));
  });
  it("distinguishes to-and-by from a source restricted to a recipient", () => {
    assert.deepEqual(one("Prevent all combat damage that would be dealt to and dealt by target creature this turn."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: all, kind: "combat", involving: target }, duration: { duration: "this-turn" },
    }));
    assert.deepEqual(one("Prevent all damage that would be dealt to you this turn by target creature."), spell({
      effect: "prevent-damage", prevention: { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: all, kind: "all", to: [you], by: target }, duration: { duration: "this-turn" },
    }));
    for (const text of ["Prevent all damage that would be dealt to you except by Dragons.", "Prevent the next time damage is dealt to you.", "Prevent all combat damage that would be dealt by target creature this turn this turn."]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("player counters and explicit choices", () => {
  it("stores symbol counters on the correct player and rejects malformed kinds", () => {
    assert.deepEqual(one("You get {E}{E}."), spell({ effect: "get-counters", who: you, counter: { type: "energy" }, count: n(2) }));
    assert.deepEqual(one("Target player gets {TK}."), spell({ effect: "get-counters", who: { player: "target-player" }, counter: { type: "ticket" }, count: n(1) }));
    assert.deepEqual(one("Each opponent gets two poison counters."), spell({ effect: "get-counters", who: { player: "each-opponent" }, counter: { type: "poison" }, count: n(2) }));
    for (const text of ["You get {E}{TK}.", "You get {G}.", "Put one or three charge counters on target creature."]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it("keeps enter-as choices, resolving choices, and target declarations distinct", () => {
    assert.deepEqual(one("As this enchantment enters, choose a creature type."), {
      kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "enchantment" }, effects: [{ steps: [{ effect: "choose-value", who: you, category: "creature-type" }] }] },
    });
    assert.deepEqual(one("Choose target creature."), spell({ effect: "declare-target", what: target }));
    assert.deepEqual(one("Choose a creature."), spell({ effect: "choose-object", who: you, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } }));
    // The excluded value is now represented (CR 205.3e), so this former rejection is a positive assertion.
    assert.deepEqual(one("As this enchantment enters, choose a creature type other than Elf."), {
      kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "enchantment" },
        effects: [{ steps: [{ effect: "choose-value", who: you, category: "creature-type", excluding: ["Elf"] }] }] },
    });
    assert.equal(parseOracleText("Choose a number between one and ten.").ok, false);
  });
  it("retains optional causative actions and complete emblem abilities", () => {
    assert.deepEqual(one("You may have this creature deal 2 damage to target player."), spell({
      effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(2), to: { to: "player", ref: { player: "target-player" } }, optional: true,
    }));
    assert.deepEqual(one('You get an emblem with "You have hexproof."'), spell({
      effect: "get-emblem", who: you, abilities: [{ gained: "quoted", ability: { kind: "static", effect: {
        static: "player-abilities", who: you, abilities: [{ gained: "keyword", keyword: { keyword: "hexproof" } }],
      } } }],
    }));
    assert.equal(parseOracleText('You get an emblem with "Anything happens."').ok, false);
  });
});
