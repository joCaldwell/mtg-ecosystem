import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { ObjectFilter, ObjectRef, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { bindOtherExclusion } from "../src/parser/other-exclusion.ts";
import { hasUnboundStaticOther, parseStatic } from "../src/parser/statics.ts";

function one(text: string): StaticEffect {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  const ability = result.abilities[0];
  assert.equal(ability.kind, "static");
  return ability.effect;
}

function boost(what: ObjectRef, forEach: ObjectFilter, size = 1): StaticEffect {
  return { static: "modify", what, condition: undefined, duration: undefined, modifiers: [{
    modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: size } },
    toughness: { sign: 1, amount: { amount: "fixed", value: size } }, forEach,
  }] };
}

describe("continuous other-object exclusions bind the actual antecedent", () => {
  // Wording from .scryfall-cache/oracle-cards.json. CR 611.3a continuously
  // recalculates these bonuses. The official Sliver Legion ruling confirms
  // that it continues affecting other Slivers after losing its own type:
  // https://api.scryfall.com/cards/87545415-2e27-4841-a3af-7653af2ba6d3/rulings
  it("binds Sliver Legion's count separately to each affected Sliver", () => {
    assert.deepEqual(one("All Sliver creatures get +1/+1 for each other Sliver on the battlefield."), boost(
      { ref: "all", filter: { subtypes: ["Sliver"], types: ["creature"] } },
      { subtypes: ["Sliver"], zone: { zone: "battlefield" }, otherThan: { ref: "affected-object" } },
    ));
  });

  it("excludes Bravado's enchanted creature, not the Aura", () => {
    assert.deepEqual(one("Enchanted creature gets +1/+1 for each other creature you control."), boost(
      { ref: "enchanted", noun: "creature" },
      { types: ["creature"], control: "you", otherThan: { ref: "affected-object" } },
    ));
  });

  it("excludes Ancestral Mask itself even if the enchanted creature is an enchantment", () => {
    assert.deepEqual(one("Enchanted creature gets +2/+2 for each other enchantment on the battlefield."), boost(
      { ref: "enchanted", noun: "creature" },
      { types: ["enchantment"], zone: { zone: "battlefield" }, otherThan: { ref: "self" } }, 2,
    ));
  });

  it("keeps self exclusion independent of whether the source still matches the counted subtype", () => {
    assert.deepEqual(one("This creature gets +1/+1 for each other Rat you control."), boost(
      { ref: "this", noun: "creature" },
      { subtypes: ["Rat"], control: "you", otherThan: { ref: "self" } },
    ));
  });

  it("retains count qualifiers while binding a repeated nominal (Heroes' Podium)", () => {
    assert.deepEqual(one("Each legendary creature you control gets +1/+1 for each other legendary creature you control."), boost(
      { ref: "each", filter: { supertypes: ["legendary"], types: ["creature"], control: "you" } },
      { supertypes: ["legendary"], types: ["creature"], control: "you", otherThan: { ref: "affected-object" } },
    ));
  });

  it("uses the affected artifact when enchanted, and the Equipment source when counting other Equipment", () => {
    assert.deepEqual(bindOtherExclusion({ other: true, types: ["artifact"] }, { ref: "enchanted", noun: "artifact" }), {
      types: ["artifact"], otherThan: { ref: "affected-object" },
    });
    assert.deepEqual(bindOtherExclusion({ other: true, subtypes: ["Equipment"] }, { ref: "equipped", noun: "creature" }), {
      subtypes: ["Equipment"], otherThan: { ref: "self" },
    });
  });

  it("does not mutate filters reused in another binding context", () => {
    const filter: ObjectFilter = { other: true, types: ["creature"] };
    const affected = bindOtherExclusion(filter, { ref: "enchanted", noun: "creature" });
    const source = bindOtherExclusion(filter, { ref: "this", noun: "creature" });
    assert.deepEqual(filter, { other: true, types: ["creature"] });
    assert.deepEqual(affected, { types: ["creature"], otherThan: { ref: "affected-object" } });
    assert.deepEqual(source, { types: ["creature"], otherThan: { ref: "self" } });
  });

  it("rejects unrelated noun comparisons instead of choosing a convenient source", () => {
    for (const text of [
      "Creatures you control get +1/+1 for each other enchantment on the battlefield.",
      "All Goblins get +1/+1 for each other Sliver on the battlefield.",
      "Enchanted creature gets +1/+1 for each other artifact you control.",
      "Equipped creature gets +1/+1 for each other enchantment you control.",
      "It gets +1/+1 for each other creature you control.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseStatic(c), null, text);
      assert.equal(c.pos, 0, text);
      assert.equal(hasUnboundStaticOther(c), true, text);
      assert.equal(c.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
  });
});
