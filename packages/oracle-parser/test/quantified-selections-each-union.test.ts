import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, ObjectFilter } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Construct: one singular "each" over two coordinated nouns with a shared
// trailing qualifier. Oracle wordings checked
// in the frozen corpus: Powder Keg, Goblin Chainwhirler, Immolating Gyre,
// Chandra, Bold Pyromancer, Ajani, Strength of the Pride, Calamity of the
// Titans, Champion of the Flame, Wyleth, Great Divide Guide, Hemlock Vial,
// Miles Morales, Splatter Technique.
// CR 608.2c: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const either = (...filters: ObjectFilter[]): ObjectFilter => ({ allOf: [{ anyOf: filters }] });
const creatureOrPlaneswalker = either({ types: ["creature"] }, { types: ["planeswalker"] });
const auraOrEquipment = either({ subtypes: ["Aura"] }, { subtypes: ["Equipment"] });
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function fails(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("each A and B <qualifier> is one union under one determiner", () => {
  it("applies the trailing qualifier to both nouns", () => {
    assert.deepEqual(parsed("~ deals 1 damage to each creature and planeswalker your opponents control.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: n(1),
        to: { to: "object", ref: { ref: "each", filter: { ...creatureOrPlaneswalker, control: "opponent" } } } }] }] }]);
    assert.deepEqual(parsed("{T}, Sacrifice this artifact: Destroy each artifact and creature with mana value equal to the number of fuse counters on this artifact.", "Artifact"), [{
      kind: "activated", costs: [{ cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "artifact" } }],
      effects: [{ steps: [{ effect: "destroy", what: { ref: "each", filter: { ...either({ types: ["artifact"] }, { types: ["creature"] }),
        properties: [{ property: "mana-value", comparison: { op: "eq",
          value: { amount: "counter-count", counter: { type: "fuse" }, on: { ref: "this", noun: "artifact" } } } }] } } }] }] }]);
  });
  it("keeps a preceding player recipient separate from the union and its controller pronoun symbolic", () => {
    assert.deepEqual(parsed("When this creature enters, it deals 1 damage to each opponent and each creature and planeswalker they control.", "Creature — Goblin Warrior"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "damage", source: { ref: "it" }, amount: n(1), to: { to: "each", refs: [
        { player: "each-opponent" },
        { ref: "each", filter: { ...creatureOrPlaneswalker, controlledBy: { player: "they" } } },
      ] } }] }] }]);
  });
  it("is the same union in a 'for each' rider, a static multiplier and a leading for-each", () => {
    assert.deepEqual(parsed("Whenever ~ attacks, draw a card for each Aura and Equipment attached to it.", "Legendary Creature — Human Warrior"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1), forEach: { ...auraOrEquipment, attachedTo: { ref: "it" } } }] }] }]);
    assert.deepEqual(parsed("This creature gets +2/+2 for each Aura and Equipment attached to it.", "Creature — Human Warrior"), [{
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "pt",
        power: { sign: 1, amount: n(2) }, toughness: { sign: 1, amount: n(2) }, forEach: { ...auraOrEquipment, attachedTo: { ref: "it" } } }] } }]);
    assert.deepEqual(parsed("For each Aura and Equipment you control, you may attach it to a creature you control.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "for-each", each: { ref: "each", filter: { ...auraOrEquipment, control: "you" } },
        effects: [{ steps: [{ effect: "attach", what: { ref: "it" }, to: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: n(1) },
          who: { player: "you" }, optional: true }] }], processing: "instruction-wise" }] }] }]);
  });
  it("mixes a card type with a subtype and keeps a leading 'other' on the whole union", () => {
    assert.deepEqual(parsed("Each land and Ally you control has flying.", "Creature — Human Scout Ally"), [{
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { ...either({ types: ["land"] }, { subtypes: ["Ally"] }), control: "you" } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] }] } }]);
    assert.deepEqual(parsed("Destroy each other artifact and creature you control.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "destroy",
        what: { ref: "each", filter: { other: true, ...either({ types: ["artifact"] }, { types: ["creature"] }), control: "you" } } }] }] }]);
  });
  it("uses the same union without a qualifier (one object that is both is one member)", () => {
    assert.deepEqual(parsed("~ deals 4 damage to each creature and planeswalker.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: n(4),
        to: { to: "object", ref: { ref: "each", filter: creatureOrPlaneswalker } } }] }] }]);
  });
  it("keeps two determiners as two groups", () => {
    assert.deepEqual(parsed("~ deals 1 damage to each creature and each player.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: n(1),
        to: { to: "each", refs: [{ ref: "each", filter: { types: ["creature"] } }, { player: "each-player" }] } }] }] }]);
  });
  it("still rejects a prenominal modifier on either noun whose scope the conjunction leaves open", () => {
    fails("{B}, {T}, Sacrifice this artifact: Each equipped creature and Equipment you control gains deathtouch until end of turn.", "Artifact");
    fails("Destroy each Spider and legendary creature you control.", "Sorcery");
    fails("Destroy each nonblack creature and planeswalker you control.", "Sorcery");
  });
  it("does not read 'A and B' as a union without the singular each", () => {
    assert.deepEqual(parsed("Destroy target artifact and target creature you control.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "destroy", what: { ref: "target", filter: { types: ["artifact"] }, count: n(1) } },
        { effect: "destroy", what: { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(1) } },
      ] }] }] }]);
    fails("Sacrifice an artifact and creature you control.", "Sorcery");
  });
});
