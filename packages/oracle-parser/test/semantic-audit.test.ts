import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { normalizeOracleText } from "../src/normalize.ts";
import { canonicalSubtypeName } from "../src/vocab.ts";
import { parseOracleText } from "../src/index.ts";

describe("subtype identity survives parsing", () => {
  it("preserves singular names ending in s and canonical internal capitalization", () => {
    for (const name of ["Plains", "Aurochs", "Astartes", "Custodes", "Cyclops", "Fungus",
      "Homunculus", "Locus", "Nautilus", "Octopus", "Pegasus", "Platypus", "Thalakos",
      "Walrus", "Bolas", "Calix", "Assembly-Worker", "Power-Plant", "Time Lord"]) {
      assert.equal(canonicalSubtypeName(name), name);
      const result = parseOracleText(`Destroy target ${name}.`);
      assert.equal(result.ok, true, JSON.stringify(result.lines));
      assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
        effect: "destroy", what: { ref: "target", count: { amount: "fixed", value: 1 }, filter: { subtypes: [name] } },
      }] }] }]);
    }
  });

  it("resolves regular and irregular plurals against actual subtypes", () => {
    for (const [plural, singular] of [["Elves", "Elf"], ["Dwarves", "Dwarf"], ["Wolves", "Wolf"],
      ["Werewolves", "Werewolf"], ["Faeries", "Faerie"], ["Heroes", "Hero"], ["Oxen", "Ox"],
      ["Mice", "Mouse"], ["Fungi", "Fungus"], ["Homunculi", "Homunculus"], ["Pegasi", "Pegasus"],
      ["Octopuses", "Octopus"], ["Allies", "Ally"], ["Armies", "Army"], ["Time Lords", "Time Lord"]]) {
      assert.equal(canonicalSubtypeName(plural), singular);
    }
  });

  it("does not invent subtypes for unimplemented statuses or unresolved names", () => {
    for (const unknown of ["They", "Sharuum", "Modified", "Commander", "Goaded", "Fortified", "Dinosaurish"]) {
      assert.equal(canonicalSubtypeName(unknown), undefined);
    }
    // Cached Scryfall text: Sharuum the Hegemon uses a non-comma short name;
    // rejecting an unresolved name is required until its binding is supported.
    for (const text of [
      "When Sharuum enters, you may return target artifact card from your graveyard to the battlefield.",
      "Fortified land has indestructible.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });

  it("stores Puppet Conjurer's token subtype without removing its final letter", () => {
    // Scryfall oracle-cards cache, Puppet Conjurer (actual Oracle line).
    const result = parseOracleText("{U}, {T}: Create a 0/1 blue Homunculus artifact creature token.", "Puppet Conjurer");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "mana", symbols: ["U"] }, { cost: "tap-self" }], restriction: undefined,
      effects: [{ steps: [{ effect: "create-token", count: { amount: "fixed", value: 1 }, token: {
        colors: ["blue"], supertypes: [], types: ["artifact", "creature"], subtypes: ["Homunculus"],
        keywords: [], power: { amount: "fixed", value: 0 }, toughness: { amount: "fixed", value: 1 },
      } }] }],
    }]);
  });
});

describe("normalization cannot erase meaningful input", () => {
  it("preserves ability boundaries inside parentheticals and avoids joining words", () => {
    assert.deepEqual(normalizeOracleText("Flying (Reminder\ncontinued.)Haste"), ["Flying", "Haste"]);
    assert.deepEqual(normalizeOracleText("fly(reminder)ing"), ["fly ing"]);
    assert.equal(parseOracleText("fly(reminder)ing").ok, false);
  });

  it("removes complete nested reminders and rejects unmatched delimiters", () => {
    assert.deepEqual(normalizeOracleText("Flying (Reminder (nested reminder).)\nHaste"), ["Flying", "Haste"]);
    assert.equal(parseOracleText("Flying (unsupported trailing instruction").ok, false);
    assert.equal(parseOracleText("Flying)").ok, false);
  });

  it("canonicalizes names consistently and respects Unicode word boundaries", () => {
    assert.deepEqual(normalizeOracleText("Urza’s Bauble enters tapped.", "Urza’s Bauble"), ["~ enters tapped."]);
    assert.deepEqual(normalizeOracleText("Sab-Sunen enters tapped.", "Sab-Sunen, Luxa Embodied"), ["~ enters tapped."]);
    assert.deepEqual(normalizeOracleText("ÆAuro Auroé Auro2 Auro_ ~ Auro", "Auro"), ["ÆAuro Auroé Auro2 Auro_ ~ ~"]);
  });
});
