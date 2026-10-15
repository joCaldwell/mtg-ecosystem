import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, PlayerRef, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. Rules text (2026-09-25):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };

function abilities(text: string, typeLine = "Instant"): Ability[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
/** Sentences of a one-line instant. */
function spell(text: string): Sentence[] {
  const [ability] = abilities(text);
  assert.equal(ability.kind, "spell");
  return (ability as Extract<Ability, { kind: "spell" }>).effects;
}
function rejected(text: string, typeLine = "Instant"): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("'the difference' of a governing comparison (CR 603.4, 608.2c)", () => {
  const yourHand = { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } };
  it("subtracts the smaller compared quantity from the larger (intervening if)", () => {
    // Damia, Sage of Stone.
    const condition = { condition: "cards-in-hand", who: you, comparison: { op: "lt", value: { amount: "fixed", value: 7 } } };
    assert.deepEqual(abilities("At the beginning of your upkeep, if you have fewer than seven cards in hand, draw cards equal to the difference.", "Creature — Gorgon"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, condition,
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "minus", a: { amount: "fixed", value: 7 }, b: yourHand } }] }],
    }]);
  });
  it("uses the comparison of the instruction's own sentence", () => {
    // Balance of Power.
    const opponent = { player: "target-opponent" };
    assert.deepEqual(spell("If target opponent has more cards in hand than you, draw cards equal to the difference."), [{
      condition: { condition: "cards-in-hand", who: opponent, comparison: { op: "gt", value: yourHand } }, conditionKind: "if",
      steps: [{ effect: "draw", who: you, amount: { amount: "minus",
        a: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: opponent } } }, b: yourHand } }],
    }]);
    // Skull Raid: a count of cards discarded this way.
    const discarded = { cls: "card", relation: { relation: "event-result", event: "discard", scope: "this-way" } };
    assert.deepEqual(spell("Target opponent discards two cards. If fewer than two cards were discarded this way, you draw cards equal to the difference.")[1], {
      condition: { condition: "count", of: discarded, comparison: { op: "lt", value: { amount: "fixed", value: 2 } } }, conditionKind: "if",
      steps: [{ effect: "draw", who: you, amount: { amount: "minus", a: { amount: "fixed", value: 2 }, b: { amount: "count", of: discarded } } }],
    });
  });
  it("compares an object's attribute", () => {
    // Drizzt Do'Urden.
    const [ability] = abilities("Whenever a creature dies, if it had power greater than ~'s power, put a number of +1/+1 counters on ~ equal to the difference.", "Legendary Creature — Elf Ranger");
    assert.equal(ability.kind, "triggered");
    assert.deepEqual((ability as Extract<Ability, { kind: "triggered" }>).effects, [{ steps: [{
      effect: "put-counters", on: { ref: "self" }, counter: { type: "+1/+1" },
      count: { amount: "minus", a: { amount: "attribute", of: { ref: "it" }, attribute: "power" }, b: { amount: "attribute", of: { ref: "self" }, attribute: "power" } },
    }] }]);
  });
  it("fails without a governing comparison", () => {
    rejected("Draw cards equal to the difference.");
    // The comparison governs only its own sentence.
    rejected("If you have fewer than seven cards in hand, draw a card. Draw cards equal to the difference.");
    // A sentence condition that compares nothing blocks the trigger's.
    rejected("At the beginning of your upkeep, if you have fewer than seven cards in hand, you may pay {1}. If you do, draw cards equal to the difference.", "Creature — Gorgon");
    // Quantified players have no single difference.
    rejected("If an opponent has more cards in hand than you, draw cards equal to the difference.");
    rejected("If you have exactly seven cards in hand, draw cards equal to the difference.");
  });
});

describe("'this emblem' inside an emblem's abilities (CR 114.1–114.4)", () => {
  const emblem = { ref: "this", noun: "emblem" };
  it("names the emblem as the damage source of its own ability", () => {
    // Chandra, Torch of Defiance.
    assert.deepEqual(abilities("−7: You get an emblem with \"Whenever you cast a spell, this emblem deals 5 damage to any target.\"", "Legendary Planeswalker — Chandra"), [{
      kind: "loyalty", cost: { sign: -1, amount: 7 },
      effects: [{ steps: [{ effect: "get-emblem", who: you, abilities: [{ gained: "quoted", ability: {
        kind: "triggered", trigger: { trigger: "cast", who: you, what: { cls: "spell" } },
        effects: [{ steps: [{ effect: "damage", source: emblem, amount: { amount: "fixed", value: 5 }, to: { to: "any-target" } }] }],
      } }] }] }],
    }]);
    // Chandra, Awakened Inferno: "you" is the emblem's owner and controller (114.2).
    const [inferno] = abilities("+2: Each opponent gets an emblem with \"At the beginning of your upkeep, this emblem deals 1 damage to you.\"", "Legendary Planeswalker — Chandra");
    assert.deepEqual((inferno as Extract<Ability, { kind: "loyalty" }>).effects, [{ steps: [{ effect: "get-emblem", who: { player: "each-opponent" }, abilities: [{ gained: "quoted", ability: {
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      effects: [{ steps: [{ effect: "damage", source: emblem, amount: { amount: "fixed", value: 1 }, to: { to: "player", ref: you } }] }],
    } }] }] }]);
  });
  it("is unknown outside an emblem", () => {
    rejected("Whenever you cast a spell, this emblem deals 5 damage to any target.", "Enchantment");
    rejected("This emblem deals 1 damage to any target.");
    rejected("Creatures you control have \"{T}: This emblem deals 1 damage to any target.\"", "Enchantment");
  });
});

describe("suspended cards (CR 702.62b)", () => {
  const suspended = { status: ["suspended"], cls: "card" };
  it("is a state of a card, an alternative to a permanent", () => {
    // Fury Charm, Timecrafting.
    assert.deepEqual(spell("Remove two time counters from target permanent or suspended card."), [{ steps: [{
      effect: "remove-counters", counter: { type: "time" }, count: { amount: "fixed", value: 2 },
      from: { ref: "target", filter: { anyOf: [{ cls: "permanent" }, suspended] }, count: { amount: "fixed", value: 1 } },
    }] }]);
    // Shivan Sand-Mage.
    assert.deepEqual(spell("Put two time counters on target permanent with a time counter on it or suspended card."), [{ steps: [{
      effect: "put-counters", counter: { type: "time" }, count: { amount: "fixed", value: 2 },
      on: { ref: "target", count: { amount: "fixed", value: 1 }, filter: { anyOf: [
        { cls: "permanent", counterConstraints: [{ counter: { type: "time" }, comparison: { op: "ge", value: { amount: "fixed", value: 1 } } }] },
        suspended] } },
    }] }]);
    // Venser's Diffusion.
    assert.deepEqual(spell("Return target nonland permanent or suspended card to its owner's hand."), [{ steps: [{
      effect: "return-to-hand", to: "owner",
      what: { ref: "target", filter: { anyOf: [{ nonTypes: ["land"], cls: "permanent" }, suspended] }, count: { amount: "fixed", value: 1 } },
    }] }]);
  });
  it("is only an adjective of cards", () => {
    rejected("Remove two time counters from target suspended permanent.");
    rejected("Return target nonland permanent or suspended creature to its owner's hand.");
  });
});
