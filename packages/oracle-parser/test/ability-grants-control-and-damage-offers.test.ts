import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings are from the cached
// Scryfall corpus; rules are the Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string, name?: string) {
  return parseOracleText(text, name, { typeLine });
}
function only(text: string, typeLine: string, name?: string): Ability {
  const parsed = parse(text, typeLine, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" };

describe("losing control of an object as a trigger event", () => {
  // Khârn the Betrayer; CR 603.10d.
  it("parses a player losing control of the ability's source", () => {
    assert.deepEqual(only("When you lose control of Khârn the Betrayer, draw two cards.", "Legendary Creature — Astartes Berserker", "Khârn the Betrayer"), {
      kind: "triggered", trigger: { trigger: "loses-control", who: you, what: { ref: "self" } },
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(2) }] }] });
  });

  // Ogre Geargrabber: the event of a delayed triggered ability (CR 603.7).
  it("parses a delayed trigger on losing control of a referenced object", () => {
    const ability = only("Whenever this creature attacks, gain control of target Equipment an opponent controls until end of turn. Attach it to this creature. When you lose control of that Equipment, unattach it.", "Creature — Ogre Warrior");
    assert.deepEqual(ability.kind === "triggered" && ability.effects[2], { steps: [{ effect: "create-delayed-trigger",
      trigger: { trigger: "loses-control", who: you, what: { ref: "that", filter: { subtypes: ["Equipment"] } } },
      occurrence: "next", effects: [{ steps: [{ effect: "unattach", what: { ref: "it" }, who: you }] }], targets: "delayed-ability" }] });
  });

  // Coffin Queen: either event ends the effect's delayed trigger.
  it("keeps a disjunction with an object event", () => {
    const ability = only("{2}{B}, {T}: Put target creature card from a graveyard onto the battlefield under your control. When this creature becomes untapped or you lose control of this creature, exile that creature.", "Creature — Zombie Wizard");
    const step = ability.kind === "activated" ? ability.effects[1].steps[0] : null;
    assert.deepEqual(step && "trigger" in step && step.trigger, { trigger: "any-of", events: [
      { trigger: "becomes-untapped", what: { ref: "this", noun: "creature" } },
      { trigger: "loses-control", who: you, what: { ref: "this", noun: "creature" } }] });
  });

  it("does not read losing life or an unfinished object as losing control", () => {
    rejects("When you lose control, draw two cards.", "Creature — Ogre");
    rejects("When you lose control of, draw two cards.", "Creature — Ogre");
  });
});

describe("any player may have the source deal damage to them; if no one does", () => {
  // Browbeat, Breaking Point. CR 118.12a: the same as "… unless any player has
  // ~ deal N damage to them".
  it("makes the fallback sentence conditional on nobody accepting the damage", () => {
    assert.deepEqual(only("Any player may have ~ deal 5 damage to them. If no one does, target player draws three cards.", "Sorcery"), {
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "target-player" }, amount: fixed(3) }],
        condition: { condition: "has-damage-dealt", who: { player: "a-player" }, source: { ref: "self" }, amount: fixed(5) },
        conditionKind: "unless" }] });
    assert.deepEqual(only("Any player may have ~ deal 6 damage to them. If no one does, destroy all creatures. Creatures destroyed this way can't be regenerated.", "Sorcery"), {
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "all", filter: { types: ["creature"] } }, regenerationForbidden: true }],
        condition: { condition: "has-damage-dealt", who: { player: "a-player" }, source: { ref: "self" }, amount: fixed(6) },
        conditionKind: "unless" }] });
  });

  it("rejects a fallback that does not test the offer, or another source", () => {
    rejects("Any player may have ~ deal 5 damage to them. Target player draws three cards.", "Sorcery");
    rejects("Any player may have ~ deal 5 damage to them. If no one does, if you control a creature, target player draws three cards.", "Sorcery");
    rejects("Any player may have target creature deal 5 damage to them. If no one does, target player draws three cards.", "Sorcery");
  });
});

describe("counter filters as relative clauses", () => {
  // Banewhip Punisher, Coiling Stalker, Grind. CR 122.1.
  const minusOne = { counter: { type: "-1/-1" }, comparison: { op: "ge", value: fixed(1) } };
  it("reads 'that has' as 'with' and 'that doesn't have' as 'without'", () => {
    assert.deepEqual(only("Destroy target creature that has a -1/-1 counter on it.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["creature"], counterConstraints: [minusOne] }, count: fixed(1) } }] }] });
    assert.deepEqual(only("Put a +1/+1 counter on target creature you control that doesn't have a +1/+1 counter on it.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "put-counters",
        on: { ref: "target", filter: { types: ["creature"], control: "you", counterConstraints: [{ counter: { type: "+1/+1" }, comparison: { op: "eq", value: fixed(0) } }] }, count: fixed(1) },
        counter: { type: "+1/+1" }, count: fixed(1) }] }] });
  });

  it("reads a bare plural -1/-1 kind as one or more counters", () => {
    assert.deepEqual(only("Exile any number of target creatures that have -1/-1 counters on them.", "Sorcery"), {
      kind: "spell", effects: [{ steps: [{ effect: "exile",
        what: { ref: "target", filter: { types: ["creature"], counterConstraints: [minusOne] }, count: { amount: "chosen", minimum: 0 } } }] }] });
  });

  it("does not read other relative 'has' clauses as counters", () => {
    rejects("Destroy target creature that has a -1/-1 counter on another creature.", "Instant");
    rejects("Destroy target creature that has an even number of counters on it.", "Instant");
  });
});
