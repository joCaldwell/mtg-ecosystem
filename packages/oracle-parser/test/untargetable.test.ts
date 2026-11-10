import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Creature — Elf") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// CR 115.1, 601.2c, 602.2b: a "can't be the target of" restriction limits
// which spells and abilities can choose the object as a target. Hexproof and
// shroud are defined in the same words (702.11b-d, 702.18a) but the printed
// restriction is not that keyword.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("can't be the target of spells or abilities", () => {
  const thisCreature = { ref: "this", noun: "creature" };
  const spellsOrAbilities = { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }] };
  it("keeps the qualifiers of the spells (Fiendslayer Paladin, Tetsuo Umezawa, Shanna, Sisay's Legacy)", () => {
    assert.deepEqual(parsed("This creature can't be the target of black or red spells your opponents control.", "Fiendslayer Paladin", "Creature — Human Knight"),
      [{ kind: "static", effect: { static: "cant", what: thisCreature, action: { action: "be-targeted",
        by: { allOf: [{ anyOf: [{ colors: ["black"] }, { colors: ["red"] }] }], cls: "spell", control: "opponent" } } } }]);
    assert.deepEqual(parsed("~ can't be the target of Aura spells.", "Tetsuo Umezawa", "Legendary Creature — Human Archer"),
      [{ kind: "static", effect: { static: "cant", what: { ref: "self" }, action: { action: "be-targeted", by: { subtypes: ["Aura"], cls: "spell" } } } }]);
    assert.deepEqual(parsed("~ can't be the target of abilities your opponents control.", "Shanna, Sisay's Legacy", "Legendary Creature — Human Warrior"),
      [{ kind: "static", effect: { static: "cant", what: { ref: "self" }, action: { action: "be-targeted", by: { cls: "ability", control: "opponent" } } } }]);
  });
  it("keeps a spell quality apart from the abilities' source quality (Gaea's Revenge)", () => {
    assert.deepEqual(parsed("This creature can't be the target of nongreen spells or abilities from nongreen sources.", "Gaea's Revenge", "Creature — Elemental"),
      [{ kind: "static", effect: { static: "cant", what: thisCreature, action: { action: "be-targeted", by: { anyOf: [
        { nonColors: ["green"], cls: "spell" },
        { cls: "ability", abilitySource: { ref: "each", filter: { nonColors: ["green"], cls: "source" } } }] } } } }]);
  });
  it("applies to every matching object, with a condition, or alongside another modifier (Ground Seal, Lurker, Spectral Shield)", () => {
    assert.deepEqual(parsed("Cards in graveyards can't be the targets of spells or abilities.", "Ground Seal", "Enchantment"),
      [{ kind: "static", effect: { static: "cant", what: { ref: "each", filter: { cls: "card", zone: { zone: "graveyard" } } },
        action: { action: "be-targeted", by: spellsOrAbilities } } }]);
    assert.deepEqual(parsed("This creature can't be the target of spells unless it attacked or blocked this turn.", "Lurker", "Creature — Beast"),
      [{ kind: "static", effect: { static: "cant", what: thisCreature, action: { action: "be-targeted", by: { cls: "spell" } },
        condition: { condition: "any-of", conditions: [
          { condition: "combat-action-history", what: { ref: "it" }, action: "attacked", during: "this-turn" },
          { condition: "combat-action-history", what: { ref: "it" }, action: "blocked", during: "this-turn" }] }, conditionKind: "unless" } }]);
    assert.deepEqual(parsed("Enchanted creature gets +0/+2 and can't be the target of spells.", "Spectral Shield", "Enchantment — Aura"),
      [{ kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: n(0) }, toughness: { sign: 1, amount: n(2) } },
        { modifier: "cant", action: { action: "be-targeted", by: { cls: "spell" } } }] } }]);
  });
  it("is created for a duration by a resolving instruction (Vines of Vastwood)", () => {
    const [vines] = parsed("Target creature can't be the target of spells or abilities your opponents control this turn. If this spell was kicked, that creature gets +4/+4 until end of turn.", "Vines of Vastwood", "Instant");
    assert.deepEqual(vines.effects[0], { steps: [{ effect: "cant", what: { ref: "target", filter: { types: ["creature"] }, count: n(1) },
      action: { action: "be-targeted", by: { ...spellsOrAbilities, control: "opponent" } }, duration: { duration: "this-turn" } }] });
  });
  it("rejects qualifiers whose scope over spells and abilities is not stated", () => {
    fails("This creature can't be the target of black spells or abilities.");
    fails("This creature can't be the target of white spells or abilities your opponents control.");
    fails("This creature can't be the target of spells that can target only Walls or of abilities that can target only Walls.");
  });
});
