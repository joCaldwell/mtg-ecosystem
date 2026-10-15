import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Oracle wordings checked in the frozen corpus;
// the cards are named with each construct.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function rejected(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("a target chosen by another player", () => {
  // Evangelize, Mausoleum Turnkey, Volcanic Offering, Ley Line, Cuombajj
  // Witches. CR 601.2c, 603.3d: still a target of the spell or ability,
  // announced as it is put on the stack, but by the stated player.
  it("keeps an opponent as the chooser and the filter that follows the choice", () => {
    assert.deepEqual(parsed("Gain control of target creature of an opponent's choice they control.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-control", who: { player: "you" }, what: {
        ref: "target", filter: { types: ["creature"], controlledBy: { player: "they" } }, count: n(1), chosenBy: { player: "an-opponent" },
      } }] }],
    }]);
    assert.deepEqual(parsed("When this creature enters, return target creature card of an opponent's choice from your graveyard to your hand.", "Creature — Human Cleric"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "return-to-hand", what: {
        ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: n(1), chosenBy: { player: "an-opponent" },
      }, to: "your" }] }],
    }]);
  });
  it("distinguishes the opponent's target from your own in one sentence", () => {
    const land = { nonSupertypes: ["basic"], types: ["land"], control: "not-you" };
    assert.deepEqual(parsed("Destroy target nonbasic land you don't control and target nonbasic land of an opponent's choice you don't control.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "destroy", what: { ref: "target", filter: land, count: n(1) } },
        { effect: "destroy", what: { ref: "target", filter: land, count: n(1), chosenBy: { player: "an-opponent" } } },
      ] }] }],
    }]);
  });
  it("keeps 'their' symbolic and the deciding player separate", () => {
    assert.deepEqual(parsed("At the beginning of each player's upkeep, that player may put a +1/+1 counter on target creature of their choice.", "Enchantment"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "each-player" },
      effects: [{ steps: [{ effect: "put-counters", on: { ref: "target", filter: { types: ["creature"] }, count: n(1), chosenBy: { player: "they" } },
        counter: { type: "+1/+1" }, count: n(1), who: { player: "that-player" }, optional: true, decidedBy: { player: "that-player" } }] }],
    }]);
  });
  it("keeps the chooser of any target as an object recipient", () => {
    assert.deepEqual(parsed("{T}: This creature deals 1 damage to any target and 1 damage to any target of an opponent's choice.", "Creature — Human Wizard"), [{
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(1), to: { to: "any-target" } },
        { effect: "damage", source: { ref: "this", noun: "creature" }, amount: n(1), to: { to: "object", ref: { ref: "any-target", chosenBy: { player: "an-opponent" } } } },
      ] }] }],
    }]);
  });
  it("rejects choosers that are not one stated player", () => {
    rejected("Destroy target creature of your choice.", "Instant");
    rejected("Destroy target creature of each opponent's choice.", "Instant");
  });
});

describe("your choice of one printed counter kind or token", () => {
  // Assaultron Dominator, T-45 Power Armor, Vivien, Monsters' Advocate,
  // Invoke the Ancients; The Third Doctor, Transmutation Font. CR 608.2d:
  // one option is chosen as the instruction is carried out.
  const put = (on: object, type: string) => ({ effect: "put-counters", on, counter: { type }, count: n(1) });
  it("shares the final noun across the listed kinds", () => {
    assert.deepEqual(parsed("At the beginning of combat on your turn, put your choice of a +1/+1, first strike, vigilance, or menace counter on ~.", "Legendary Creature — Time Lord Doctor"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "combat", whose: "your" },
      effects: [{ steps: [{ effect: "choice", who: { player: "you" }, options: ["+1/+1", "first strike", "vigilance", "menace"].map(type => put({ ref: "self" }, type)) }] }],
    }]);
  });
  it("repeats the noun with each option", () => {
    assert.deepEqual(parsed("Put your choice of a reach counter, a vigilance counter, or a trample counter on target creature.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "choice", who: { player: "you" }, options: ["reach", "vigilance", "trample"].map(type =>
        put({ ref: "target", filter: { types: ["creature"] }, count: n(1) }, type)) }] }],
    }]);
  });
  it("is the same choice for an explicit 'your choice of' token list", () => {
    assert.deepEqual(parsed("{T}: Create your choice of a Blood token, a Clue token, or a Food token.", "Artifact"),
      parsed("{T}: Create a Blood token, a Clue token, or a Food token.", "Artifact"));
  });
  it("rejects mixed or non-alternative lists", () => {
    rejected("Put your choice of a reach counter, vigilance, or a trample counter on target creature.", "Instant");
    rejected("Put your choice of a reach counter and a trample counter on target creature.", "Instant");
    rejected("Put your choice of a reach or a reach counter on target creature.", "Instant");
    rejected("{T}: Create your choice of a Blood token and a Clue token.", "Artifact");
  });
});

describe("removing all of the counters a condition counted", () => {
  // Lightning Coils, Ludevic's Test Subject, Replicating Ring, Sarulf, Realm
  // Eater, Strixhaven Stadium. CR 122.1: counters are not objects, so "them"
  // is every counter of the counted kind on the counted permanent.
  const threshold = (what: object, type: string, value: number) =>
    ({ condition: "is-filter", what, filter: { counterConstraints: [{ counter: { type }, comparison: { op: "ge", value: n(value) } }] } });
  it("takes the kind and holder from the conditional that opens the sentence", () => {
    assert.deepEqual(parsed("{1}{U}: Put a hatchling counter on this creature. Then if there are five or more hatchling counters on it, remove all of them and transform it.", "Creature — Human Horror"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1", "U"] }], effects: [
        { steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "hatchling" }, count: n(1) }] },
        { steps: [
          { effect: "remove-counters", counter: { type: "hatchling" }, count: "all", from: { ref: "it" } },
          { effect: "transform", what: { ref: "it" }, who: { player: "you" } },
        ], condition: threshold({ ref: "it" }, "hatchling", 5), conditionKind: "if" },
      ],
    }]);
    assert.deepEqual(parsed("Whenever a creature you control deals combat damage to an opponent, put a point counter on this artifact. Then if it has ten or more point counters on it, remove them all and that player loses the game.", "Artifact"),
      parsed("Whenever a creature you control deals combat damage to an opponent, put a point counter on this artifact. Then if it has ten or more point counters on it, remove all point counters from it and that player loses the game.", "Artifact"));
  });
  it("reads an intervening-if threshold and keeps the optional removal", () => {
    assert.deepEqual(parsed("At the beginning of your upkeep, if ~ has one or more +1/+1 counters on it, you may remove all of them.", "Legendary Creature — Wolf"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, condition: threshold({ ref: "self" }, "+1/+1", 1),
      effects: [{ steps: [{ effect: "remove-counters", counter: { type: "+1/+1" }, count: "all", from: { ref: "self" }, optional: true }] }],
    }]);
    assert.deepEqual(parsed("At the beginning of your upkeep, if this artifact has five or more charge counters on it, remove all of them from it.", "Artifact"),
      parsed("At the beginning of your upkeep, if this artifact has five or more charge counters on it, remove all charge counters from it.", "Artifact"));
  });
  it("rejects 'them' without a single counted counter kind directly before it", () => {
    rejected("Put a charge counter on this artifact. Then remove all of them.", "Artifact");
    rejected("{T}: If this artifact has five or more counters on it, remove all of them.", "Artifact");
    rejected("{T}: If you control five or more artifacts, remove all of them.", "Artifact");
    rejected("{T}: If this artifact has five or more charge counters on it, draw a card and remove all of them.", "Artifact");
  });
});

describe("the other creature of an event", () => {
  // Venom, Venomous Fangs, Charisma: one definite event participant,
  // contrasted with the other one; the contrast is not resolved here.
  it("is a definite reference carrying the unresolved contrast", () => {
    assert.deepEqual(parsed("Whenever enchanted creature deals damage to a creature, destroy the other creature.", "Enchantment — Aura"), [{
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "enchanted", noun: "creature" }, to: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } },
      effects: [{ steps: [{ effect: "destroy", what: { ref: "that", filter: { types: ["creature"], other: true } } }] }],
    }]);
  });
  it("differs from 'that creature' and rejects qualified or plural forms", () => {
    assert.notDeepEqual(parsed("Whenever enchanted creature deals damage to a creature, destroy the other creature.", "Enchantment — Aura"),
      parsed("Whenever enchanted creature deals damage to a creature, destroy that creature.", "Enchantment — Aura"));
    rejected("Whenever enchanted creature deals damage to a creature, destroy the other creature with flying.", "Enchantment — Aura");
    rejected("Whenever enchanted creature deals damage to a creature, destroy the other creature you control.", "Enchantment — Aura");
    rejected("Whenever enchanted creature deals damage to a creature, destroy the other creatures.", "Enchantment — Aura");
  });
});

describe("each of / one of your opponents", () => {
  // Absolute Virtue, Courageous Resolve, Aurelia, the Law Above, Everything
  // Pizza, Lynde, Cheerful Tormentor, Martial Impetus.
  it("is the same player reference as 'each opponent' / 'an opponent'", () => {
    assert.deepEqual(parsed("You have protection from each of your opponents.", "Legendary Creature — Avatar"),
      parsed("You have protection from each opponent.", "Legendary Creature — Avatar"));
    assert.deepEqual(parsed("Each of your opponents discards a card.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "discard", who: { player: "each-opponent" }, what: { ref: "filter", filter: { cls: "card" }, count: n(1) } }] }],
    }]);
    assert.deepEqual(parsed("At the beginning of your upkeep, you may attach a Curse attached to you to one of your opponents. If you do, draw two cards.", "Legendary Creature — Human Warlock"),
      parsed("At the beginning of your upkeep, you may attach a Curse attached to you to an opponent. If you do, draw two cards.", "Legendary Creature — Human Warlock"));
  });
  it("does not read other possessors' opponents", () => {
    rejected("Each of their opponents discards a card.", "Sorcery");
    rejected("Each of your opponent discards a card.", "Sorcery");
  });
});

describe("starting with you, each player …", () => {
  // Alliance of Arms, Minds Aglow, Shared Trauma (join forces), The Horus
  // Heresy, Druid of Purification, The Legend of Yangchen. CR 101.4: the
  // players choose one after another in turn order, here from you.
  it("orders the choices of one instruction carried out by each player", () => {
    assert.deepEqual(parsed("Starting with you, each player may pay any amount of mana. Each player draws X cards, where X is the total amount of mana paid this way.", "Sorcery"), [{
      kind: "spell", effects: [
        { steps: [{ effect: "pay-cost", who: { player: "each-player" }, costs: [{ cost: "mana-amount", amount: { amount: "chosen", minimum: 0 } }],
          optional: true, decidedBy: { player: "each-player" }, startingWith: { player: "you" } }] },
        { steps: [{ effect: "draw", who: { player: "each-player" }, amount: { amount: "x" } }],
          bindings: [{ variable: "x", value: { amount: "event-quantity", event: "mana-paid", scope: "this-way" } }] },
      ],
    }]);
  });
  it("keeps every object chosen this way as the event result of the choice", () => {
    assert.deepEqual(parsed("Starting with you, each player chooses a creature. Destroy each creature chosen this way.", "Sorcery"), [{
      kind: "spell", effects: [
        { steps: [{ effect: "choose-object", who: { player: "each-player" }, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, startingWith: { player: "you" } }] },
        { steps: [{ effect: "destroy", what: { ref: "each", filter: { types: ["creature"], relation: { relation: "event-result", event: "choose", scope: "this-way" } } } }] },
      ],
    }]);
  });
  it("rejects an order for a single player's instruction or a different first player", () => {
    rejected("Starting with you, draw a card.", "Sorcery");
    rejected("Starting with you, target player draws a card.", "Sorcery");
    rejected("Starting with target opponent, each player draws a card.", "Sorcery");
    rejected("Starting with you, each player chooses a creature, then draws a card.", "Sorcery");
    rejected("Destroy each creature chosen.", "Sorcery");
  });
});
