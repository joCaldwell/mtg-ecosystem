import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Every expected value is the complete
// AST of the line. Rule numbers refer to the Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const ability = (text: string, typeLine = "Sorcery") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};
const fails = (text: string, typeLine = "Sorcery") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.ok, false, `${text} must not be accepted`);
};

describe("mana of one chosen color (CR 106.1, 608.2c, 700.5)", () => {
  const chosen = { choice: "chosen-value", category: "color", which: "chosen" };
  const chooseColor = { steps: [{ effect: "choose-value", who: { player: "you" }, category: "color" }] };
  // Nyx Lotus, Nykthos, Shrine to Nyx.
  it("reads 'an amount of mana of that color equal to your devotion to that color'", () => {
    assert.deepEqual(ability("{T}: Choose a color. Add an amount of mana of that color equal to your devotion to that color.", "Legendary Artifact"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [chooseColor,
        { steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "chosen-color",
          amount: { amount: "devotion", who: { player: "you" }, colors: chosen }, choice: chosen } }] }] });
  });
  // Three Tree City.
  it("reads 'an amount of mana of that color equal to the number of …'", () => {
    assert.deepEqual(ability("{T}: Choose a color. Add an amount of mana of that color equal to the number of creatures you control.", "Land"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [chooseColor,
        { steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "chosen-color",
          amount: { amount: "count", of: { types: ["creature"], control: "you" } }, choice: chosen } }] }] });
  });
  it("reads 'that many mana of that color' after one color choice", () => {
    assert.deepEqual(ability("{T}: Choose a color. Add three mana of that color.", "Artifact"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [chooseColor,
        { steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "chosen-color", amount: { amount: "fixed", value: 3 }, choice: chosen } }] }] });
  });
  it("rejects 'that color' without exactly one earlier color choice", () => {
    fails("{T}: Add an amount of mana of that color equal to your devotion to that color.", "Artifact");
    fails("{T}: Choose two colors. Add three mana of that color.", "Artifact");
    fails("{T}: Choose a color. Add an amount of mana of that color.", "Artifact");
  });
});

describe("a life total that can't change (CR 119.5, 119.7, 119.8)", () => {
  const you = { player: "you" };
  // Platinum Emperion.
  it("reads 'Your life total can't change.' as both life-change prohibitions", () => {
    assert.deepEqual(ability("Your life total can't change.", "Artifact Creature — Golem"),
      { kind: "static", effect: { static: "group", effects: [
        { static: "player-rule", rule: { who: you, rule: "cant-gain-life" } },
        { static: "player-rule", rule: { who: you, rule: "cant-lose-life" } }] } });
  });
  // Teferi's Protection.
  it("shares a leading duration with a coordinated player ability gain", () => {
    const duration = { duration: "your-next-turn" };
    assert.deepEqual(ability("Until your next turn, your life total can't change and you gain protection from everything.", "Instant"),
      { kind: "spell", effects: [{ steps: [
        { effect: "simultaneous", steps: [
          { effect: "player-rule", rule: { who: you, rule: "cant-gain-life" }, duration },
          { effect: "player-rule", rule: { who: you, rule: "cant-lose-life" }, duration }] },
        { effect: "player-gain-abilities", who: you, abilities: [{ gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "everything" }] } }], duration }] }] });
  });
  it("rejects a targeted static subject and other life-total predicates", () => {
    fails("Target player's life total can't change.", "Artifact");
    fails("Your life total can't increase.", "Artifact");
  });
});

describe("an unless-payment of printed mana plus scaled mana (CR 118.12a)", () => {
  const counter = (filter: object) => ({ effect: "counter", what: { ref: "target", filter, count: { amount: "fixed", value: 1 } } });
  const payer = { player: "controller", of: { ref: "it" } };
  // Spell Stutter.
  it("reads 'pays {2} plus an additional {1} for each <object>' as one two-part payment", () => {
    assert.deepEqual(ability("Counter target spell unless its controller pays {2} plus an additional {1} for each Faerie you control.", "Instant"),
      { kind: "spell", effects: [{ steps: [counter({ cls: "spell" })], conditionKind: "unless", condition: { condition: "pays", who: payer,
        cost: [{ cost: "mana", symbols: ["2"] }, { cost: "mana", symbols: ["1"], multiplier: { amount: "count", of: { subtypes: ["Faerie"], control: "you" } } }] } }] });
  });
  // Concerted Defense.
  it("reads 'for each creature in your party' as the party size", () => {
    assert.deepEqual(ability("Counter target noncreature spell unless its controller pays {1} plus an additional {1} for each creature in your party.", "Instant"),
      { kind: "spell", effects: [{ steps: [counter({ nonTypes: ["creature"], cls: "spell" })], conditionKind: "unless", condition: { condition: "pays", who: payer,
        cost: [{ cost: "mana", symbols: ["1"] }, { cost: "mana", symbols: ["1"], multiplier: { amount: "party-size", who: { player: "you" } } }] } }] });
    assert.deepEqual(ability("Counter target spell unless its controller pays {1} for each creature in your party.", "Instant"),
      { kind: "spell", effects: [{ steps: [counter({ cls: "spell" })], conditionKind: "unless", condition: { condition: "pays", who: payer,
        cost: [{ cost: "mana", symbols: ["1"] }], multiplier: { amount: "party-size", who: { player: "you" } } } }] });
  });
  it("rejects an additional payment without its unit or with X", () => {
    fails("Counter target spell unless its controller pays {2} plus an additional {1}.", "Instant");
    fails("Counter target spell unless its controller pays {2} plus an additional {X} for each Faerie you control.", "Instant");
  });
});

describe("limits on X printed as their own paragraph (CR 107.3a, 107.3f)", () => {
  const constraints = (list: object[]) => ({ kind: "static", effect: { static: "variable-constraints", what: { ref: "this", noun: "spell" }, constraints: list } });
  // Ertai's Meddling, Winter's Chill, Open the Way.
  it("reads 'X can't be 0.' and 'X can't be greater than <amount>.' as casting rules of the spell", () => {
    assert.deepEqual(ability("X can't be 0.", "Instant"), constraints([{ variable: "x", not: 0 }]));
    assert.deepEqual(ability("X can't be greater than the number of snow lands you control.", "Instant"),
      constraints([{ variable: "x", maximum: { amount: "count", of: { supertypes: ["snow"], types: ["land"], control: "you" } } }]));
    assert.deepEqual(ability("X can't be greater than the number of players in the game.", "Sorcery"),
      constraints([{ variable: "x", maximum: { amount: "player-count", of: { player: "each-player" } } }]));
  });
  it("rejects a constraint followed by other text or an X maximum", () => {
    fails("X can't be 0. Draw a card.", "Instant");
    fails("X can't be greater than X.", "Instant");
    fails("X can't be less than 2.", "Instant");
  });
});

describe("entering with twice X / an additional X counters (CR 614.1c, 107.3)", () => {
  const plus = { type: "+1/+1" };
  // Banquet Guests, Primo, the Unbounded.
  it("reads 'enters with twice X +1/+1 counters on it'", () => {
    assert.deepEqual(ability("This creature enters with twice X +1/+1 counters on it.", "Creature — Halfling"),
      { kind: "static", effect: { static: "enters-with-counters", what: { ref: "this", noun: "creature" }, counter: plus,
        count: { amount: "twice", of: { amount: "x" } } } });
  });
  // Bloodspore Thrinax.
  it("reads 'enters with an additional X +1/+1 counters on it, where X is …'", () => {
    assert.deepEqual(ability("Each other creature you control enters with an additional X +1/+1 counters on it, where X is the number of +1/+1 counters on this creature.", "Creature — Lizard"),
      { kind: "static", effect: { static: "enters-with-counters", what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } },
        counter: plus, count: { amount: "x" }, additional: true,
        bindings: [{ variable: "x", value: { amount: "counter-count", counter: plus, on: { ref: "this", noun: "creature" } } }] } });
  });
  it("rejects other doubled or additional leads", () => {
    fails("This creature enters with twice two +1/+1 counters on it.", "Creature — Elf");
    fails("This creature enters with twice an additional X +1/+1 counters on it.", "Creature — Elf");
  });
});

describe("players getting a computed number of counters (CR 122.1)", () => {
  // Infesting Radroach, Etali, Primal Conqueror.
  it("reads 'they get that many rad counters' with the contextual quantity", () => {
    assert.deepEqual(ability("Whenever this creature deals combat damage to a player, they get that many rad counters.", "Creature — Insect"),
      { kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true, to: "any" },
        effects: [{ steps: [{ effect: "get-counters", who: { player: "they" }, counter: { type: "rad" }, count: { amount: "that-much" } }] }] });
  });
  // Feral Ghoul.
  it("reads 'gets a number of rad counters equal to <amount>'", () => {
    assert.deepEqual(ability("When this creature dies, each opponent gets a number of rad counters equal to its power.", "Creature — Zombie"),
      { kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } },
        effects: [{ steps: [{ effect: "get-counters", who: { player: "each-opponent" }, counter: { type: "rad" },
          count: { amount: "attribute", of: { ref: "it" }, attribute: "power" } }] }] });
  });
  it("rejects 'a number of' without its amount", () => {
    fails("When this creature dies, each opponent gets a number of rad counters.", "Creature — Zombie");
    fails("When this creature dies, each opponent gets a number of that many rad counters.", "Creature — Zombie");
  });
});

describe("the life paid as this permanent entered (CR 607.2g)", () => {
  // Minion of the Wastes.
  it("reads a characteristic-defining P/T equal to the life paid as it entered", () => {
    assert.deepEqual(ability("This creature's power and toughness are each equal to the life paid as it entered.", "Creature — Minion"),
      { kind: "static", effect: { static: "cda-pt", what: { ref: "this", noun: "creature" }, stat: "both",
        value: { amount: "entry-payment", of: { ref: "it" }, paid: "life" } } });
  });
  // Phyrexian Processor.
  it("reads 'where X is the life paid as this artifact entered'", () => {
    assert.deepEqual(ability("{4}, {T}: Create an X/X black Phyrexian Minion creature token, where X is the life paid as this artifact entered.", "Artifact"),
      { kind: "activated", costs: [{ cost: "mana", symbols: ["4"] }, { cost: "tap-self" }], effects: [{ steps: [{ effect: "create-token",
        count: { amount: "fixed", value: 1 }, token: { colors: ["black"], supertypes: [], types: ["creature"], subtypes: ["Phyrexian", "Minion"], keywords: [],
          power: { amount: "x" }, toughness: { amount: "x" } } }],
        bindings: [{ variable: "x", value: { amount: "entry-payment", of: { ref: "this", noun: "artifact" }, paid: "life" } }] }] });
  });
  it("rejects another object's entry or another moment", () => {
    fails("{4}, {T}: Create an X/X black Phyrexian Minion creature token, where X is the life paid as target creature entered.", "Artifact");
    fails("{4}, {T}: Create an X/X black Phyrexian Minion creature token, where X is the life paid as this artifact attacked.", "Artifact");
  });
});
