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

describe("protection and hexproof from each of an object's colors", () => {
  // Earnest Fellowship, Tam, Mindful First-Year, Samite Elder, Opal Titan.
  // CR 702.16i / 702.11g: the set is every color of the one named object.
  it("names the color set through its possessor", () => {
    assert.deepEqual(parsed("Each creature has protection from each of its colors.", "Enchantment"), [{ kind: "static", effect: {
      static: "modify", what: { ref: "each", filter: { types: ["creature"] } }, modifiers: [{ modifier: "abilities", abilities: [
        { gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "colors-of", what: { ref: "it" } }] } }] }],
    } }]);
    assert.deepEqual(parsed("Each other creature you control has hexproof from each of its colors.", "Creature — Gorgon"), [{ kind: "static", effect: {
      static: "modify", what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } }, modifiers: [{ modifier: "abilities", abilities: [
        { gained: "keyword", keyword: { keyword: "hexproof", from: [{ scope: "colors-of", what: { ref: "it" } }] } }] }],
    } }]);
    assert.deepEqual(parsed("{T}: Choose target permanent you control. Creatures you control gain protection from each of that permanent's colors until end of turn.", "Creature — Human Cleric"), [{
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [
        { steps: [{ effect: "declare-target", what: { ref: "target", filter: { cls: "permanent", control: "you" }, count: n(1) } }] },
        { steps: [{ effect: "gain-abilities", what: { ref: "each", filter: { types: ["creature"], control: "you" } }, abilities: [
          { gained: "keyword", keyword: { keyword: "protection", from: [{ scope: "colors-of", what: { ref: "that", filter: { cls: "permanent" } } }] } }],
          duration: { duration: "end-of-turn" } }] }],
    }]);
  });
  it("rejects plural possessors and other characteristic sets", () => {
    rejected("This creature has protection from each of those creatures' colors.", "Creature — Golem");
    rejected("This creature has protection from each of the exiled card's card types.", "Artifact Creature — Golem");
    rejected("This creature has protection from each of its colors and types.", "Creature — Golem");
  });
});

describe("tokens created with a linked ability's source", () => {
  // Dual Nature, Saproling Burst, Arcane Artisan, Tetravus, Tombstone Stairwell.
  // CR 607.2c: only tokens made by the linked ability printed on that object.
  const linked = (source: object) => ({ cls: "token", relation: { relation: "linked-creation", source } });
  it("keeps the link and its source on every quantifier", () => {
    assert.deepEqual(parsed("When this enchantment leaves the battlefield, exile all tokens created with this enchantment.", "Enchantment"), [{
      kind: "triggered", trigger: { trigger: "leaves", what: { ref: "this", noun: "enchantment" } },
      effects: [{ steps: [{ effect: "exile", what: { ref: "all", filter: linked({ ref: "this", noun: "enchantment" }) } }] }],
    }]);
    assert.deepEqual(parsed("When this enchantment leaves the battlefield, destroy all tokens created with this enchantment. They can't be regenerated.", "Enchantment"), [{
      kind: "triggered", trigger: { trigger: "leaves", what: { ref: "this", noun: "enchantment" } },
      effects: [{ steps: [{ effect: "destroy", what: { ref: "all", filter: linked({ ref: "this", noun: "enchantment" }) }, regenerationForbidden: true }] }],
    }]);
    assert.deepEqual(parsed("At the beginning of your upkeep, you may exile any number of tokens created with this creature. If you do, put that many +1/+1 counters on this creature.", "Artifact Creature — Construct"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" }, effects: [
        { steps: [{ effect: "exile", what: { ref: "filter", filter: linked({ ref: "this", noun: "creature" }), count: { amount: "chosen", minimum: 0 } }, optional: true }] },
        { steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: { amount: "that-much" } }],
          condition: { condition: "you-do" }, conditionKind: "if" }],
    }]);
  });
  it("does not read other creation wordings as the link", () => {
    rejected("When this enchantment leaves the battlefield, exile all tokens you created with this enchantment.", "Enchantment");
    rejected("When this enchantment leaves the battlefield, exile all tokens created with.", "Enchantment");
    rejected("When this enchantment leaves the battlefield, exile all tokens created by this enchantment.", "Enchantment");
  });
});

describe("quantified sets with a printed exception", () => {
  // Elspeth Tirel, Scourglass, Slash the Ranks, Ajani Unrelenting, Flame
  // Sweep, Whelming Wave, Cyclone Summoner, Slinn Voda. CR 608.2c.
  it("records single types, subtypes and tokens with the existing negative fields", () => {
    assert.deepEqual(parsed("Destroy all permanents except for artifacts and lands.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "all", filter: { cls: "permanent", nonTypes: ["artifact", "land"] } } }] }] }]);
    // Same node as "all other nonland, nontoken permanents".
    assert.deepEqual(parsed("Destroy all other permanents except for lands and tokens.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "all", filter: { other: true, cls: "permanent", nonTypes: ["land"], nonToken: true } } }] }] }]);
    assert.deepEqual(parsed("Destroy all other permanents except for artifacts, lands, and Phyrexians.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "all", filter: { other: true, cls: "permanent", nonTypes: ["artifact", "land"], nonSubtypes: ["Phyrexian"] } } }] }] }]);
  });
  it("keeps a compound exception whole", () => {
    assert.deepEqual(parsed("~ deals 2 damage to each creature except for creatures you control with flying.", "Instant"), [{ kind: "spell", effects: [{ steps: [
      { effect: "damage", source: { ref: "self" }, amount: n(2), to: { to: "object", ref: { ref: "each", filter: {
        types: ["creature"], noneOf: [{ types: ["creature"], control: "you", withAbility: ["flying"] }] } } } }] }] }]);
    assert.deepEqual(parsed("~ deals 4 damage to each creature except for tokens you control.", "Instant"), [{ kind: "spell", effects: [{ steps: [
      { effect: "damage", source: { ref: "self" }, amount: n(4), to: { to: "object", ref: { ref: "each", filter: {
        types: ["creature"], noneOf: [{ cls: "token", control: "you" }] } } } }] }] }]);
    assert.deepEqual(parsed("Destroy all creatures and planeswalkers except for commanders.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "all", filter: { allOf: [{ anyOf: [{ types: ["creature"] }, { types: ["planeswalker"] }] }], noneOf: [{ commander: true }] } } }] }] }]);
  });
  it("excludes the source itself and reads an exception placed after the destination", () => {
    assert.deepEqual(parsed("Destroy all creatures except for ~.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "all", filter: { types: ["creature"], otherThan: { ref: "self" } } } }] }] }]);
    assert.deepEqual(parsed("Return all creatures to their owners' hands except for Krakens, Leviathans, Octopuses, and Serpents.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "return-to-hand", what: { ref: "all", filter: { types: ["creature"], nonSubtypes: ["Kraken", "Leviathan", "Octopus", "Serpent"] } }, to: "owner" }] }] }]);
  });
  it("ends the exception list at the next instruction and rejects incomplete exceptions", () => {
    assert.deepEqual(parsed("Destroy each creature except for Walls and you gain 2 life.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "each", filter: { types: ["creature"], nonSubtypes: ["Wall"] } } },
      { effect: "gain-life", who: { player: "you" }, amount: n(2) }] }] }]);
    rejected("Destroy all creatures except for target creature.", "Sorcery");
    rejected("Destroy all other creatures except for ~.", "Sorcery");
    rejected("Destroy all creatures except lands.", "Sorcery");
    rejected("Destroy target creature except for Walls.", "Sorcery");
  });
});

describe("outlaws", () => {
  // Hellspur Posse Boss, Graywater's Fixer, Shoot the Sheriff, Double Down,
  // Mine Raider. CR 700.12: Assassin, Mercenary, Pirate, Rogue and/or
  // Warlock; CR 700.12a: the bare noun considers only outlaw permanents.
  const outlaw = { anyOf: ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"].map(subtype => ({ subtypes: [subtype] })) };
  it("reads the noun as an outlaw permanent", () => {
    assert.deepEqual(parsed("Other outlaws you control have haste.", "Creature — Human Rogue"), [{ kind: "static", effect: {
      static: "modify", what: { ref: "each", filter: { other: true, allOf: [outlaw], cls: "permanent", control: "you" } },
      modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }],
    } }]);
  });
  it("reads the adjective on the noun it modifies, and non-outlaw as none of the types", () => {
    assert.deepEqual(parsed("Whenever you cast an outlaw spell, copy that spell.", "Instant"), [{
      kind: "triggered", trigger: { trigger: "cast", who: { player: "you" }, what: { allOf: [outlaw], cls: "spell" } },
      effects: [{ steps: [{ effect: "copy-stack-object", what: { ref: "that", filter: { cls: "spell" } }, who: { player: "you" }, count: n(1) }] }],
    }]);
    assert.deepEqual(parsed("Destroy target non-outlaw creature.", "Instant"), [{ kind: "spell", effects: [{ steps: [
      { effect: "destroy", what: { ref: "target", filter: { nonSubtypes: ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"], types: ["creature"] }, count: n(1) } }] }] }]);
  });
  it("does not make a non-permanent class an outlaw noun", () => {
    rejected("Destroy target spell outlaw.", "Instant");
  });
});

describe("counters among a group of objects", () => {
  // Ferrafor, Young Yew, Kate Stewart, Dimension X Pizzasaur. CR 122.1: the
  // total of the counters on those objects, as "counters on <group>".
  it("is the same total as counters on the group", () => {
    const among = parsed("Draw cards equal to the number of +1/+1 counters among creatures you control.", "Sorcery");
    assert.deepEqual(among, [{ kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: {
      amount: "counter-count", counter: { type: "+1/+1" }, on: { ref: "each", filter: { types: ["creature"], control: "you" } } } }] }] }]);
    assert.deepEqual(parsed("Draw cards equal to the number of +1/+1 counters on creatures you control.", "Sorcery"), among);
    assert.deepEqual(parsed("Draw cards equal to the number of counters among creatures target player controls.", "Sorcery"), [{ kind: "spell", effects: [{ steps: [{
      effect: "draw", who: { player: "you" }, amount: { amount: "counter-count", counter: "any",
        on: { ref: "each", filter: { types: ["creature"], controlledBy: { player: "target-player" } } } } }] }] }]);
  });
  it("requires a group after among", () => {
    rejected("Draw cards equal to the number of counters among target creature.", "Sorcery");
    rejected("Draw cards equal to the number of counters among it.", "Sorcery");
  });
});

describe("either of an established group", () => {
  // Wicked Slumber, Call of the Death-Dweller, Tamiyo, Field Researcher,
  // Love on the Battlefield: one member, chosen as "one of them" is.
  it("selects one member as one of them does", () => {
    const one = parsed("Tap up to two target creatures. Put a stun counter on one of them.", "Instant");
    assert.deepEqual(parsed("Tap up to two target creatures. Put a stun counter on either of them.", "Instant"), one);
    assert.deepEqual(one, [{ kind: "spell", effects: [
      { steps: [{ effect: "tap", what: { ref: "target", filter: { types: ["creature"] }, count: n(2), upTo: true } }] },
      { steps: [{ effect: "put-counters", on: { ref: "from-pool", pool: { ref: "them" }, count: n(1) }, counter: { type: "stun" }, count: n(1) }] }] }]);
    assert.deepEqual(parsed("+1: Choose up to two target creatures. Until your next turn, whenever either of those creatures deals combat damage, you draw a card.", "Legendary Planeswalker — Tamiyo"), [{
      kind: "loyalty", cost: { sign: 1, amount: 1 }, effects: [
        { steps: [{ effect: "declare-target", what: { ref: "target", filter: { types: ["creature"] }, count: n(2), upTo: true } }] },
        { steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "deals-damage",
          what: { ref: "from-pool", pool: { ref: "that", filter: { types: ["creature"] }, plural: true }, count: n(1) }, combat: true },
          occurrence: "each", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }],
          targets: "delayed-ability", duration: { duration: "your-next-turn" } }] }],
    }]);
  });
  it("needs a group", () => {
    rejected("Tap target creature. Put a stun counter on either of it.", "Instant");
    rejected("Put a stun counter on either of target creature.", "Instant");
  });
});
