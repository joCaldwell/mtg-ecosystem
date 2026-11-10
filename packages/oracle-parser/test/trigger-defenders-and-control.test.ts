import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import type { ObjectFilter, ObjectRef, Trigger } from "../src/ast.ts";

// Clauses are printed Oracle wording from the cached corpus (card named per
// case). Rules text: Comprehensive Rules 2026-09-25, cited per suite.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value }) as const;
const you = { player: "you" } as const;
const a = (filter: ObjectFilter): ObjectRef => ({ ref: "filter", filter, count: n(1) });

function trigger(text: string): Trigger {
  const c = new Cursor(lex(text));
  const result = parseTrigger(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens at ${c.pos}`);
  return result;
}

function unsupported(text: string, type = "Creature — Fish"): void {
  assert.equal(parseOracleText(`${text}, draw a card.`, "Fixture", { typeLine: type }).ok, false, text);
}

describe("damage to you or a permanent (CR 120.3, 603.2c)", () => {
  it("splits the recipients into separate occurrences", () => {
    // Elesh Norn; Flameblade Angel; Mangara's Equity.
    const source = a({ cls: "source", control: "opponent" });
    assert.deepEqual(trigger("Whenever a source an opponent controls deals damage to you or a permanent you control"), {
      trigger: "any-of", events: [
        { trigger: "deals-damage", what: source, combat: undefined, to: you },
        { trigger: "deals-damage", what: source, combat: undefined, to: a({ cls: "permanent", control: "you" }) },
      ],
    });
  });
  it("keeps the combat restriction on both recipients", () => {
    const creature = a({ types: ["creature"] });
    assert.deepEqual(trigger("Whenever a creature deals combat damage to you or a Wall you control"), {
      trigger: "any-of", events: [
        { trigger: "deals-damage", what: creature, combat: true, to: you },
        { trigger: "deals-damage", what: creature, combat: true, to: a({ subtypes: ["Wall"], control: "you" }) },
      ],
    });
  });
  it("rejects a third recipient", () => {
    unsupported("Whenever a creature deals damage to you or a creature you control or a player");
  });
});

describe("spells that have an Adventure (CR 715.1)", () => {
  it("is a filter test on the spell", () => {
    // Edgewall Innkeeper; Garenbrig Squire; Wandermare.
    assert.deepEqual(trigger("Whenever you cast a creature spell that has an Adventure"), {
      trigger: "cast", who: you, what: { types: ["creature"], cls: "spell", adventure: true },
    });
    assert.deepEqual(trigger("Whenever you cast a creature spell with an Adventure"), trigger("Whenever you cast a creature spell that has an Adventure"));
  });
  it("differs from the plain creature spell", () => {
    assert.notDeepEqual(trigger("Whenever you cast a creature spell that has an Adventure"), trigger("Whenever you cast a creature spell"));
  });
  it("rejects an alternative after the Adventure test", () => {
    unsupported("Whenever you cast a spell that has an Adventure or flying");
  });
});

describe("a player attacks collective defenders (CR 508.3e)", () => {
  const anOpponent = { player: "an-opponent" } as const;
  const walkers = a({ types: ["planeswalker"], control: "you" });
  it("one or more of your opponents is one occurrence per declaration", () => {
    // Jolene, the Plunder Queen.
    assert.deepEqual(trigger("Whenever a player attacks one or more of your opponents"), {
      trigger: "player-attacks", who: { player: "a-player" }, per: "declaration", whom: anOpponent, whomGrouping: "one-or-more",
    });
  });
  it("one of your opponents is the plain single-opponent attack", () => {
    // Combat Calligrapher; Breena, the Demagogue.
    assert.deepEqual(trigger("Whenever a player attacks one of your opponents"), {
      trigger: "player-attacks", who: { player: "a-player" }, per: "declaration", whom: anOpponent,
    });
  });
  it("one or more planeswalkers you control", () => {
    // Mila, Crafty Companion.
    assert.deepEqual(trigger("Whenever an opponent attacks one or more planeswalkers you control"), {
      trigger: "player-attacks", who: anOpponent, per: "declaration", whomPermanents: walkers, whomGrouping: "one-or-more",
    });
  });
  it("you and/or one or more planeswalkers you control", () => {
    // Cunning Rhetoric.
    assert.deepEqual(trigger("Whenever an opponent attacks you and/or one or more planeswalkers you control"), {
      trigger: "player-attacks", who: anOpponent, per: "declaration", whom: you, whomPermanents: walkers, whomGrouping: "one-or-more",
    });
  });
  it("rejects a bare 'and' (both required) and non-attackable permanents", () => {
    unsupported("Whenever an opponent attacks you and one or more planeswalkers you control");
    unsupported("Whenever an opponent attacks one or more creatures you control");
  });
});

describe("attacks a player alone (CR 506.5)", () => {
  it("is an alone attack of that player", () => {
    // Jiang Yanggu, Wildcrafter.
    assert.deepEqual(trigger("Whenever a creature you control attacks a player alone"), {
      trigger: "attacks", what: a({ types: ["creature"], control: "you" }), alone: true, whom: [{ player: "a-player" }],
    });
    assert.deepEqual(trigger("Whenever a creature you control attacks alone"), {
      trigger: "attacks", what: a({ types: ["creature"], control: "you" }), alone: true, whom: undefined,
    });
  });
});

describe("sacrificing the source or another match", () => {
  it("is two sacrifice events of the same player", () => {
    // Kingpin, Wilson Fisk; Esoteric Duplicator.
    assert.deepEqual(trigger("Whenever you sacrifice ~ or another creature"), {
      trigger: "any-of", events: [
        { trigger: "sacrifices", who: you, what: { ref: "self" } },
        { trigger: "sacrifices", who: you, what: { other: true, types: ["creature"] } },
      ],
    });
    assert.deepEqual(trigger("Whenever you sacrifice this artifact or another artifact"), {
      trigger: "any-of", events: [
        { trigger: "sacrifices", who: you, what: { ref: "this", noun: "artifact" } },
        { trigger: "sacrifices", who: you, what: { other: true, types: ["artifact"] } },
      ],
    });
  });
  it("rejects an alternative that is not 'another'", () => {
    unsupported("Whenever you sacrifice ~ or a creature");
  });
});

describe("control changes (CR 603.10d)", () => {
  it("losing control of the source", () => {
    // Duplicity; Gustha's Scepter.
    assert.deepEqual(trigger("When you lose control of this enchantment"), {
      trigger: "loses-control", who: you, what: { ref: "this", noun: "enchantment" },
    });
  });
  it("gaining control keeps the previous controller", () => {
    // Zidane, Tantalus Thief.
    assert.deepEqual(trigger("Whenever an opponent gains control of a permanent from you"), {
      trigger: "gains-control", who: { player: "an-opponent" }, what: a({ cls: "permanent" }), from: you,
    });
    assert.deepEqual(trigger("Whenever a player gains control of a permanent"), {
      trigger: "gains-control", who: { player: "a-player" }, what: a({ cls: "permanent" }),
    });
  });
  it("keeps the previous controller of 'from another player'", () => {
    // Risky Move.
    const event = trigger("When you gain control of this enchantment from another player");
    assert.equal(event.trigger, "gains-control");
    assert.ok("from" in event && event.from && event.from.player === "filtered");
  });
  it("rejects collective objects and losing control differently from gaining it", () => {
    unsupported("Whenever you lose control of one or more permanents");
    assert.notDeepEqual(trigger("When you lose control of this enchantment"), trigger("When you gain control of this enchantment"));
  });
});
