import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseSentences } from "../src/parser/effects.ts";
import { parsePreventionSentence } from "../src/parser/prevention.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle wording checked against the pinned Scryfall corpus. CR609.7,
// 615.5/7/8/10/11/12 and 700.1 control scope, timing, and event grouping:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// WotC Phantom Nantuko ruling (2022-12-08): simultaneous sources remove
// only one counter. Cover of Winter (2006-07-15): each creature's damage
// to several recipients shares one prevention quantity.
const n = (value: number) => ({ amount: "fixed", value });
const you = { player: "you" };
const self = { ref: "this", noun: "creature" };
const source = { ref: "filter", filter: { cls: "source" }, count: n(1) };
const recipient = { ref: "prevention-event-object", role: "recipient" };
const incoming = { amount: "prevention-event-quantity", quantity: "incoming" };
const prevented = { amount: "prevention-event-quantity", quantity: "prevented" };
const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
const events = (prevent: unknown, occurrence = "each") => ({ shield: "events", occurrence, prevent });
const scope = (sources = "each", recipients = "each") => ({ sources, recipients });
const chosen = (filter: unknown = { cls: "source" }) => ({ choice: "damage-source", who: you, filter,
  when: "effect-created", universe: "damage-sources", recheck: "when-damage-would-be-dealt" });
const staticAbility = (prevention: unknown) => ({ kind: "static", effect: { static: "prevent-damage", prevention } });
const spell = (prevention: unknown, duration: unknown = { duration: "this-turn" }) => ({ kind: "spell", effects: [{ steps: [{ effect: "prevent-damage", prevention, duration }] }] });
function one(text: string) {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe("prospective damage prevention", () => {
  it("stores recurring per-source reductions and all-but quantities", () => {
    assert.deepEqual(one("If a source would deal damage to you, prevent 1 of that damage."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "up-to", amount: n(1) }), kind: "all", by: source, to: [you],
    }));
    assert.deepEqual(one("If a source would deal damage to another Dinosaur you control, prevent all but 1 of that damage."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "all-but", amount: n(1) }), kind: "all", by: source,
      to: [{ ref: "filter", filter: { other: true, subtypes: ["Dinosaur"], control: "you" }, count: n(1) }],
    }));
  });
  it("retains incoming thresholds, recipient alternatives, and explicit half rounding", () => {
    assert.deepEqual(one("If a source would deal 3 or less damage to this creature, prevent that damage."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "all" }), kind: "all", by: source, incomingAmount: { op: "le", value: n(3) }, to: [self],
    }));
    assert.deepEqual(one("If a source would deal damage to you or a permanent you control, prevent half that damage, rounded up."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "half", round: "up" }), kind: "all", by: source,
      to: [you, { ref: "filter", filter: { cls: "permanent", control: "you" }, count: n(1) }],
    }));
  });
  it("aggregates simultaneous incoming sources for Phantom creatures, unlike Nine Lives", () => {
    assert.deepEqual(one("If damage would be dealt to this creature, prevent that damage. Remove a +1/+1 counter from this creature."), staticAbility({
      eventScope: scope("simultaneous"), shield: events({ prevent: "all" }), kind: "all", to: [self],
      after: [{ steps: [{ effect: "remove-counters", counter: { type: "+1/+1" }, count: n(1), from: self }] }],
    }));
    assert.deepEqual(one("If a source would deal damage to you, prevent that damage and put an incarnation counter on this enchantment."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "all" }), kind: "all", by: source, to: [you],
      after: [{ steps: [{ effect: "put-counters", on: { ref: "this", noun: "enchantment" }, counter: { type: "incarnation" }, count: n(1) }] }],
    }));
  });
  it("keeps a counter condition and removes incoming rather than actually prevented damage", () => {
    assert.deepEqual(one("If damage would be dealt to this creature while it has a +1/+1 counter on it, prevent that damage and remove that many +1/+1 counters from it."), staticAbility({
      eventScope: scope("simultaneous"), shield: events({ prevent: "all" }), kind: "all", to: [self],
      condition: { condition: "is-filter", what: recipient, filter: { counterConstraints: [{ counter: { type: "+1/+1" }, comparison: { op: "ge", value: n(1) } }] } },
      after: [{ steps: [{ effect: "remove-counters", counter: { type: "+1/+1" }, count: incoming, from: recipient }] }],
    }));
  });
  it("distinguishes incoming damage from actual prevention under unpreventable damage", () => {
    assert.deepEqual(one("If damage would be dealt to you, prevent that damage and mill twice that many cards."), staticAbility({
      eventScope: scope("simultaneous"), shield: events({ prevent: "all" }), kind: "all", to: [you],
      after: [{ steps: [{ effect: "mill", who: you, amount: { amount: "twice", of: incoming } }] }],
    }));
    assert.deepEqual(one("If noncombat damage would be dealt to you, prevent that damage. You gain life equal to the damage prevented this way."), staticAbility({
      eventScope: scope("simultaneous"), shield: events({ prevent: "all" }), kind: "noncombat", to: [you],
      after: [{ steps: [{ effect: "gain-life", who: you, amount: prevented }] }],
    }));
  });
  it("retains source choices as CR609.7 choices and target declarations as targets", () => {
    assert.deepEqual(one("The next time a red source of your choice would deal damage to you this turn, prevent that damage."), spell({
      eventScope: scope("each", "simultaneous"), shield: events({ prevent: "all" }, "next"), kind: "all", by: chosen({ colors: ["red"], cls: "source" }), to: [you],
    }));
    assert.deepEqual(one("The next time target creature would deal damage this turn, prevent that damage. You gain life equal to the damage prevented this way."), spell({
      eventScope: scope("each", "simultaneous"), shield: events({ prevent: "all" }, "next"), kind: "all", by: target,
      after: [{ steps: [{ effect: "gain-life", who: you, amount: prevented }] }],
    }));
  });
  it("retains chosen-color and keyword source restrictions and downward rounding", () => {
    assert.deepEqual(one("The next time a source of your choice of the chosen color would deal damage to you this turn, prevent that damage."), spell({
      eventScope: scope("each", "simultaneous"), shield: events({ prevent: "all" }, "next"), kind: "all", by: chosen({ cls: "source", chosenColor: { choice: "chosen-value", category: "color", which: "chosen" } }), to: [you],
    }));
    assert.deepEqual(one("The next time a creature of your choice with shadow would deal damage to you this turn, prevent half that damage, rounded down."), spell({
      eventScope: scope("each", "simultaneous"), shield: events({ prevent: "half", round: "down" }, "next"), kind: "all", by: chosen({ types: ["creature"], withAbility: ["shadow"] }), to: [you],
    }));
  });
  it("preserves one shared finite budget and a conditional rider targeting during casting", () => {
    assert.deepEqual(one("Prevent the next X damage that would be dealt to you and/or permanents you control this turn. If damage is prevented this way, this spell deals that much damage to any target."), spell({
      eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: { amount: "x" } }, kind: "all",
      to: [you, { ref: "each", filter: { cls: "permanent", control: "you" } }],
      after: [{ condition: { condition: "damage-prevented" }, conditionKind: "if", steps: [{ effect: "damage", source: { ref: "this", noun: "spell" }, amount: prevented, to: { to: "any-target" } }] }],
    }));
    assert.deepEqual(one("Prevent the next 1 damage that would be dealt to each creature and each player this turn."), spell({
      eventScope: scope("simultaneous", "each"), shield: { shield: "damage-budget", amount: n(1), allocation: "per-recipient" }, kind: "all",
      to: [{ ref: "each", filter: { types: ["creature"] } }, { player: "each-player" }],
    }));
  });
  it("scales one counter-placement instruction by actual prevented damage", () => {
    assert.deepEqual(one("Prevent the next 3 damage that would be dealt to target creature this turn. For each 1 damage prevented this way, put a +1/+1 counter on that creature."), spell({
      eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: n(3) }, kind: "all", to: [target],
      after: [{ steps: [{ effect: "put-counters", on: recipient, counter: { type: "+1/+1" }, count: prevented }] }],
    }));
    assert.deepEqual(one("If noncombat damage would be dealt to this creature, prevent that damage. Put a +1/+1 counter on this creature for each 1 damage prevented this way."), staticAbility({
      eventScope: scope("simultaneous"), shield: events({ prevent: "all" }), kind: "noncombat", to: [self],
      after: [{ steps: [{ effect: "put-counters", on: self, counter: { type: "+1/+1" }, count: prevented }] }],
    }));
  });
  it("preserves grouped recipients and recurring X bindings (Cover of Winter)", () => {
    assert.deepEqual(one("If a creature would deal combat damage to you and/or one or more creatures you control, prevent X of that damage, where X is the number of age counters on this enchantment."), staticAbility({
      eventScope: scope("each", "simultaneous"), shield: events({ prevent: "up-to", amount: { amount: "x" } }), kind: "combat",
      by: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, to: [you, { ref: "each", filter: { types: ["creature"], control: "you" } }],
      bindings: [{ variable: "x", when: "event-application", value: { amount: "counter-count", counter: { type: "age" }, on: { ref: "this", noun: "enchantment" } } }],
    }));
    assert.deepEqual(one("Prevent the next X damage that would be dealt to target creature this turn, where X is the number of Clerics on the battlefield."), spell({
      eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: { amount: "x" } }, kind: "all", to: [target],
      bindings: [{ variable: "x", when: "effect-created", value: { amount: "count", of: { subtypes: ["Cleric"], zone: { zone: "battlefield" } } } }],
    }));
    assert.deepEqual(one("Until end of turn, if a source would deal damage to you, prevent X of that damage, where X is the number of creatures you control."), spell({
      eventScope: scope(), shield: events({ prevent: "up-to", amount: { amount: "x" } }), kind: "all", by: source, to: [you],
      bindings: [{ variable: "x", when: "effect-created", value: { amount: "count", of: { types: ["creature"], control: "you" } } }],
    }, { duration: "end-of-turn" }));
  });
  it("stores application-time optional prevention separately from optional rider actions", () => {
    assert.deepEqual(one("If a source would deal damage to a player, you may prevent X of that damage, where X is the number of Clerics you control."), staticAbility({
      eventScope: scope(), shield: events({ prevent: "up-to", amount: { amount: "x" } }), kind: "all", by: source, to: [{ player: "a-player" }],
      optional: { who: you }, bindings: [{ variable: "x", when: "event-application", value: { amount: "count", of: { subtypes: ["Cleric"], control: "you" } } }],
    }));
  });
  it("preserves leading durations and separates an immediate following draw", () => {
    const prevention = { eventScope: scope("each", "simultaneous"), shield: events({ prevent: "all" }, "next"), kind: "all", by: target };
    assert.deepEqual(one("Until end of turn, the next time target creature would deal damage, prevent that damage."), spell(prevention, { duration: "end-of-turn" }));
    assert.deepEqual(one("The next time target creature would deal damage this turn, prevent that damage. Draw a card."), {
      kind: "spell", effects: [{ steps: [{ effect: "prevent-damage", prevention, duration: { duration: "this-turn" } }] }, { steps: [{ effect: "draw", who: you, amount: n(1) }] }],
    });
  });
  it("retains one established permanent-or-player target domain in a kicked shield", () => {
    const original = { effect: "prevent-damage", prevention: { eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: n(2) }, kind: "all", to: [{ ref: "any-target" }] }, duration: { duration: "this-turn" } };
    const replacement = { effect: "prevent-damage", prevention: { eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: n(4) }, kind: "all", to: [{ ref: "that", filter: { cls: "permanent", orPlayer: true } }] }, duration: { duration: "this-turn" } };
    assert.deepEqual(one("Prevent the next 2 damage that would be dealt to any target this turn. If this spell was kicked, prevent the next 4 damage that would be dealt to that permanent or player this turn instead."), {
      kind: "spell", effects: [{ steps: [{ effect: "self-replacement", original, condition: { condition: "kicked", what: { ref: "this", noun: "spell" }, comparison: { op: "ge", value: n(1) } }, instead: [replacement], targetPolicy: "retain-original" }] }],
    });
  });
  it("keeps a player-or-planeswalker target as one alternative target domain", () => {
    assert.deepEqual(one("Prevent the next 1 damage that would be dealt to target player or planeswalker this turn."), spell({
      eventScope: scope("simultaneous", "simultaneous"), shield: { shield: "damage-budget", amount: n(1) }, kind: "all",
      to: [{ ref: "target", filter: { orPlayer: true, types: ["planeswalker"] }, count: n(1) }],
    }));
  });
  it("rejects unsupported delayed riders, ambiguous event objects, quantities, and rounding", () => {
    for (const text of [
      "If a source would deal damage to you, prevent half that damage.",
      "If two sources would deal damage to you, prevent that damage.",
      "If a source would deal damage to two creatures, prevent that damage.",
      "If two creatures would be dealt damage, prevent that damage.",
      "If creatures would be dealt damage, prevent that damage.",
      "If creatures you control would deal damage, prevent that damage and draw a card.",
      "If a source would deal damage to creatures you control, prevent 1 of that damage.",
      "If damage would be dealt to one or more creatures you control, prevent that damage and put a +1/+1 counter on it.",
      "If damage would be dealt to one or more players, prevent that damage and that player draws a card.",
      "If a creature would deal damage to a creature, prevent that damage and put a +1/+1 counter on it.",
      "If damage would be dealt to this creature, prevent that damage and create a 1/1 white Soldier creature token, then put a +1/+1 counter on it.",
      "If damage would be dealt to this creature, prevent that damage and remove a +1/+1 counter from it, then create that many 1/1 white Soldier creature tokens.",
      "Prevent the next 3 damage that would be dealt to target creature this turn. For each 2 damage prevented this way, put a +1/+1 counter on that creature.",
      "The next time a source of your choice would deal damage to you this turn this turn, prevent that damage.",
    ]) {
      assert.equal(parseOracleText(text).ok, false, text);
      const c = new Cursor(lex(text));
      assert.equal(parsePreventionSentence(c, { modalOptions: [] }, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
    }
    // The triggered ability each prevention creates is
    // now represented (CR 615.5, 603.12; formerly asserted to fail).
    assert.deepEqual(one("If damage would be dealt to this creature, prevent that damage. When damage is prevented this way, this creature deals that much damage to any other target."),
      staticAbility({ shield: { shield: "events", occurrence: "each", prevent: { prevent: "all" } }, kind: "all", eventScope: scope("simultaneous", "each"),
        to: [{ ref: "this", noun: "creature" }], after: [{ steps: [{ effect: "create-reflexive-trigger", event: { event: "damage-prevented" }, effects: [{ steps: [{
          effect: "damage", source: { ref: "this", noun: "creature" }, amount: { amount: "prevention-event-quantity", quantity: "prevented" },
          to: { to: "object", ref: { ref: "any-target", other: true } } }] }] }] }] }));
  });
  it("does not certify future event references or source choice scopes as bound", () => {
    const name = "Prevention fixture";
    const result = parseOracleText("The next time a source of your choice would deal damage to any target this turn, prevent that damage.", name, { includeSourceProvenance: true });
    assert.equal(result.ok, true);
    const analysis = analyzeCardBindings({ identity: "prevention-fixture", source: { name, layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    // The chooser and target bind; the shield over future damage is the engine's.
    assert.equal(analysis.fullyBound, false);
    assert.equal(analysis.referencesBound, true, JSON.stringify(analysis.diagnostics));
    assert.ok(analysis.engineFeatures.some(need => need.feature === "damage-prevention"));
  });
});
