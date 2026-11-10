import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: several printed subjects of one resolving pump or grant.
// Oracle wordings checked in the frozen corpus:
// Wojek Siren, Surge of Zeal, Incite Hysteria, Alandra, Sky Dreamer,
// Razorgrass Invoker, Lazotep Plating, Wand of Orcus, Sapling Nursery.
// CR 109.1, 113.1, 608.2c, 611.2c:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const endOfTurn = { duration: "end-of-turn" };
const plus = (value: number) => ({ sign: 1, amount: n(value) });
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("coordinated subjects of one resolving modifier", () => {
  const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) };
  const sharingColor = { ref: "each", filter: { other: true, types: ["creature"],
    characteristicRelations: [{ relation: "shared-value", characteristic: "color", with: { operand: "object", what: { ref: "it" } } }] } };
  it("pumps the target and every other creature sharing a color with it at once", () => {
    assert.deepEqual(parsed("Radiance — Target creature and each other creature that shares a color with it get +1/+1 until end of turn.", "Instant"), [{
      kind: "spell", abilityWord: "Radiance", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "pump", what: target, power: plus(1), toughness: plus(1), duration: endOfTurn },
        { effect: "pump", what: sharingColor, power: plus(1), toughness: plus(1), duration: endOfTurn },
      ] }] }] }]);
  });
  it("grants the same abilities to each subject with the shared duration", () => {
    const haste = [{ gained: "keyword", keyword: { keyword: "haste" } }];
    assert.deepEqual(parsed("Radiance — Target creature and each other creature that shares a color with it gain haste until end of turn.", "Instant"), [{
      kind: "spell", abilityWord: "Radiance", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "gain-abilities", what: target, abilities: haste, duration: endOfTurn },
        { effect: "gain-abilities", what: sharingColor, abilities: haste, duration: endOfTurn },
      ] }] }] }]);
  });
  it("keeps 'up to one other target' as its own optional target beside the source", () => {
    assert.deepEqual(parsed("{8}: This creature and up to one other target creature each get +3/+3 until end of turn.", "Creature — Human Druid"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["8"] }], effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "pump", what: { ref: "this", noun: "creature" }, power: plus(3), toughness: plus(3), duration: endOfTurn },
        { effect: "pump", what: { ref: "target", filter: { types: ["creature"], other: true }, count: n(1), upTo: true }, power: plus(3), toughness: plus(3), duration: endOfTurn },
      ] }] }] }]);
  });
  it("keeps a player and that player's permanents as different kinds of recipient", () => {
    const hexproof = [{ gained: "keyword", keyword: { keyword: "hexproof" } }];
    assert.deepEqual(parsed("You and permanents you control gain hexproof until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "player-gain-abilities", who: { player: "you" }, abilities: hexproof, duration: endOfTurn },
        { effect: "gain-abilities", what: { ref: "each", filter: { cls: "permanent", control: "you" } }, abilities: hexproof, duration: endOfTurn },
      ] }] }] }]);
  });
  it("rejects a bare plural whose qualifier may also belong to the previous noun, players getting P/T, and a later elided verb", () => {
    // "Treefolk" does not show its number; beside the visibly plural "Forests"
    // the filter grammar forms the plural union, so
    // the list never splits off an unqualified first noun.
    assert.deepEqual(parsed("Treefolk and Forests you control gain indestructible until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities",
        what: { ref: "each", filter: { allOf: [{ anyOf: [{ subtypes: ["Treefolk"] }, { subtypes: ["Forest"] }] }], control: "you" } },
        abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }], duration: endOfTurn }] }] }]);
    // One color choice shared by the player and the permanents.
    fails("You and each permanent you control gain protection from the color of your choice until end of turn.", "Instant");
    fails("You and creatures you control get +1/+1 until end of turn.", "Instant");
    fails("Target creature and target land get +1/+1 and gain haste until end of turn.", "Instant");
  });
  it("leaves a plural union, a single subject and a player group action to their own productions", () => {
    assert.deepEqual(parsed("Other Plants and Elves you control get +2/+0 until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "pump",
        what: { ref: "each", filter: { other: true, allOf: [{ anyOf: [{ subtypes: ["Plant"] }, { subtypes: ["Elf"] }] }], control: "you" } },
        power: plus(2), toughness: plus(0), duration: endOfTurn }] }] }]);
    assert.deepEqual(parsed("Target creature gets +1/+1 and gains haste until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [
        { effect: "pump", what: target, power: plus(1), toughness: plus(1), duration: endOfTurn },
        { effect: "gain-abilities", what: target, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }], duration: endOfTurn },
      ] }] }]);
  });
});

// Static counterpart. Oracle wordings checked in the frozen corpus: Sigarda,
// Heron's Grace, Serra's Emissary, Captain America, Super-Soldier, Nighthowler,
// Dion, Bahamut's Dominant, Timber Protector. CR 113.1b, 611.3a.
describe("coordinated subjects of one static ability", () => {
  const hexproof = [{ gained: "keyword", keyword: { keyword: "hexproof" } }];
  it("gives a player and that player's permanents separate continuous effects in one group", () => {
    assert.deepEqual(parsed("You and Humans you control have hexproof.", "Legendary Creature — Angel"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "player-abilities", who: { player: "you" }, abilities: hexproof },
        { static: "modify", what: { ref: "each", filter: { subtypes: ["Human"], control: "you" } }, modifiers: [{ modifier: "abilities", abilities: hexproof }] },
      ] } }]);
  });
  it("keeps a leading condition around the whole group and the lexical 'other'", () => {
    assert.deepEqual(parsed("As long as ~ has a shield counter on it, you and other Heroes you control have hexproof.", "Legendary Creature — Human Soldier Hero"), [{
      kind: "static", effect: { static: "conditional",
        condition: { condition: "is-filter", what: { ref: "self" }, filter: { counterConstraints: [{ counter: { type: "shield" }, comparison: { op: "ge", value: n(1) } }] } },
        effect: { static: "group", effects: [
          { static: "player-abilities", who: { player: "you" }, abilities: hexproof },
          { static: "modify", what: { ref: "each", filter: { other: true, subtypes: ["Hero"], control: "you" } }, modifiers: [{ modifier: "abilities", abilities: hexproof }] },
        ] } } }]);
  });
  it("modifies the source and the other listed objects alike", () => {
    const flying = [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] }];
    assert.deepEqual(parsed("~ and other Knights you control have flying.", "Legendary Creature — Human Noble Knight"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "modify", what: { ref: "self" }, modifiers: flying },
        { static: "modify", what: { ref: "each", filter: { other: true, subtypes: ["Knight"], control: "you" } }, modifiers: flying },
      ] } }]);
  });
  it("rejects power/toughness for a player, a split shared qualifier and a trailing condition", () => {
    fails("You and Humans you control get +1/+1.", "Legendary Creature — Angel");
    // One plural union, not a split shared qualifier.
    assert.deepEqual(parsed("Other Treefolk and Forests you control have indestructible.", "Creature — Treefolk Warrior"), [{
      kind: "static", effect: { static: "modify",
        what: { ref: "each", filter: { other: true, allOf: [{ anyOf: [{ subtypes: ["Treefolk"] }, { subtypes: ["Forest"] }] }], control: "you" } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }] }] } }]);
    fails("~ and other Knights you control have flying as long as you control a Dragon.", "Legendary Creature — Human Noble Knight");
  });
});

// A modifier applies once to each object in the union of its subjects (CR
// 611.2c, 611.3a). Oracle wordings checked in the frozen corpus: Death Baron,
// Caterwauling Boggart, Nighthowler, Alandra, Sky Dreamer, Fated Clash.
describe("subject lists never modify one object through two subjects", () => {
  it("turns filter subjects into one union filter in a single effect", () => {
    assert.deepEqual(parsed("Skeletons you control and other Zombies you control get +1/+1 and have deathtouch.", "Creature — Zombie Wizard"), [{
      kind: "static", effect: { static: "modify",
        what: { ref: "each", filter: { anyOf: [{ subtypes: ["Skeleton"], control: "you" }, { other: true, subtypes: ["Zombie"], control: "you" }] } },
        modifiers: [{ modifier: "pt", power: plus(1), toughness: plus(1) },
          { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "deathtouch" } }] }] } }]);
    assert.deepEqual(parsed("Goblins you control and Elementals you control gain haste until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities",
        what: { ref: "each", filter: { anyOf: [{ subtypes: ["Goblin"], control: "you" }, { subtypes: ["Elemental"], control: "you" }] } },
        abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }], duration: endOfTurn }] }] }]);
  });
  it("treats a floating 'each' as the same per-subject modifier for a source and what it is attached to", () => {
    // CR 303.4d: an Aura can't enchant itself, so the two subjects differ.
    const x = { sign: 1, amount: { amount: "x" } };
    const bindings = [{ variable: "x", value: { amount: "count", of: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "all-players" } } } }];
    assert.deepEqual(parsed("This creature and enchanted creature each get +X/+X, where X is the number of creature cards in all graveyards.", "Enchantment Creature — Horror"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "pt", power: x, toughness: x }], bindings },
        { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [{ modifier: "pt", power: x, toughness: x }], bindings },
      ] } }]);
  });
  it("rejects subjects that could name the same object, which would need a union reference", () => {
    // Alandra could itself be a Drake; two effects would pump it twice.
    fails("Whenever you draw your fifth card each turn, ~ and Drakes you control each get +X/+X until end of turn, where X is the number of cards in your hand.", "Legendary Creature — Merfolk Wizard");
    fails("~ and Drakes you control each get +1/+1.", "Legendary Creature — Merfolk Wizard");
    fails("Whenever equipped creature attacks, it and Zombies you control gain deathtouch until end of turn.", "Artifact — Equipment");
    fails("Target creature and target artifact each gain indestructible until end of turn.", "Instant");
  });
});

// Audit fix (Tribal Forcemage): an elided subject repeats the subject node,
// and a subject with a fresh choice would then print one choice twice.
describe("a fresh choice in a subject is never duplicated by subject elision", () => {
  const choice = { choice: "fresh-choice", category: "creature-type", who: { player: "you" }, when: "instruction-applies" };
  it("keeps the single-verb instruction with its one choice", () => {
    assert.deepEqual(parsed("Creatures of the creature type of your choice get +2/+2 until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "pump", what: { ref: "each", filter: { types: ["creature"], valueOfChoice: choice } },
        power: plus(2), toughness: plus(2), duration: endOfTurn }] }] }]);
  });
  it("rejects a second coordinated verb that would need the same choice again", () => {
    fails("When this creature is turned face up, creatures of the creature type of your choice get +2/+2 and gain trample until end of turn.", "Creature — Elf Wizard");
    fails("Creatures of the creature type of your choice gain trample and get +2/+2 until end of turn.", "Instant");
  });
  it("still shares an ordinary subject between coordinated verbs", () => {
    const elves = { ref: "each", filter: { subtypes: ["Elf"], control: "you" } };
    assert.deepEqual(parsed("Elves you control get +2/+2 and gain trample until end of turn.", "Instant"), [{
      kind: "spell", effects: [{ steps: [
        { effect: "pump", what: elves, power: plus(2), toughness: plus(2), duration: endOfTurn },
        { effect: "gain-abilities", what: elves, abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }], duration: endOfTurn },
      ] }] }]);
  });
});
