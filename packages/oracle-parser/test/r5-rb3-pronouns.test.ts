import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard } from "../src/semantic/resolution.ts";

// Real-card wording is from frozen corpus f8cb9da2. Rule references use
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function card(text: string, typeLine: string, name = "Fixture Card", modify?: (abilities: Ability[]) => void) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  modify?.(result.abilities);
  const input = { identity: "fixture", source: { name, layout: "normal", type_line: typeLine },
    faces: [{ sourceFaceIndex: 0, result }] };
  const binding = analyzeCardBindings(input);
  const resolution = resolveCard(input, binding);
  return { binding, resolution, blocks: resolution.faces[0].blocks! };
}

const refAt = (result: ReturnType<typeof card>, block: number, path: string) =>
  result.blocks[block].refs.find(ref => ref.path === path);

describe("Archive R5 pronoun antecedents", () => {
  it("uses the attachment for a singular static modifier and leaves an affected group unresolved", () => {
    const path = "/effect/modifiers/0/multiplier/among";
    const blessing = card("Enchant creature\nEnchanted creature gets +1/+1 for each of its colors.",
      "Enchantment — Aura", "Blessing of the Nephilim");
    assert.deepEqual(refAt(blessing, 1, path), { path, ref: { ref: "attached-to" } });

    const attached = "/effect/modifiers/0/forEach/attachedTo";
    const suit = card("Equipped creature gets +1/+1 for each Aura and Equipment attached to it and has ward {2}.\nEquip {2}",
      "Artifact — Equipment", "Thran Power Suit");
    assert.deepEqual(refAt(suit, 0, attached), { path: attached, ref: { ref: "attached-to" } });
    const group = card("Creatures you control get +1/+1 for each Aura and Equipment attached to them.", "Enchantment");
    assert.deepEqual(refAt(group, 0, attached), { path: attached, unresolved: "missing-antecedent" });
  });

  it("reads the tapped creature's controller for Lust for War, with a kind and a quotation guard", () => {
    const trigger = "Whenever enchanted creature becomes tapped, this Aura deals 3 damage to that creature's controller.";
    const path = "/effects/0/steps/0/to/ref/of";
    const lust = card(`Enchant creature\n${trigger}\nEnchanted creature attacks each combat if able.`,
      "Enchantment — Aura", "Lust for War");
    assert.deepEqual(refAt(lust, 1, path), { path, ref: { ref: "event-object", role: "becomes-tapped" } });
    assert.deepEqual(refAt(lust, 1, "/effects/0/steps/0/to/ref"),
      { path: "/effects/0/steps/0/to/ref", evaluated: "selection" });
    const land = card(`Enchant creature\n${trigger.replace("that creature", "that land")}`, "Enchantment — Aura");
    assert.deepEqual(refAt(land, 1, path), { path, unresolved: "missing-antecedent" });
    const quote = card(`Enchant creature\nEnchanted creature has "${trigger}"`, "Enchantment — Aura");
    const quoted = `/effect/modifiers/0/abilities/0/ability${path}`;
    assert.deepEqual(refAt(quote, 1, quoted), { path: quoted, unresolved: "missing-antecedent" });
  });

  it("uses the entering Dragon in both Scourge alternatives and rejects mixed roles or grouped entrants", () => {
    const text = "Flying\nWhenever this creature or another Dragon you control enters, it deals X damage to any target, where X is the number of Dragons you control.\n{R}: This creature gets +1/+0 until end of turn.";
    const analyze = (modify?: (abilities: Ability[]) => void) => card(text, "Creature — Dragon", "Scourge of Valkas", modify);
    const path = "/effects/0/steps/0/source";
    const scourge = analyze();
    assert.equal(scourge.resolution.complete, true);
    assert.deepEqual(refAt(scourge, 1, path), { path, ref: { ref: "event-object", role: "enters" } });
    const mixed = analyze(abilities => {
      const ability = abilities[1];
      assert.ok(ability.kind === "triggered" && ability.trigger.trigger === "any-of");
      ability.trigger.events = [ability.trigger.events[1], { trigger: "gains-life", who: { player: "an-opponent" } }];
    });
    assert.deepEqual(refAt(mixed, 1, path), { path, unresolved: "missing-antecedent" });
    const grouped = analyze(abilities => {
      const ability = abilities[1];
      assert.ok(ability.kind === "triggered" && ability.trigger.trigger === "any-of");
      ability.trigger.events[1].grouping = "one-or-more";
    });
    assert.deepEqual(refAt(grouped, 1, path), { path, unresolved: "missing-antecedent" });
  });

  it("uses Bile Blight's one target and preserves the ambiguity on Malamet Battle Glyph and Arena", () => {
    const text = "Target creature and all other creatures with the same name as that creature get -3/-3 until end of turn.";
    const path = "/effects/0/steps/0/steps/1/what/filter/characteristicRelations/0/with/what";
    assert.deepEqual(refAt(card(text, "Instant", "Bile Blight"), 0, path), { path, ref: { ref: "target", slot: 0 } });
    const glyph = card("Choose target creature you control and target creature you don't control. If the creature you control entered this turn, put a +1/+1 counter on it. Then those creatures fight each other.",
      "Sorcery", "Malamet Battle Glyph");
    const glyphPath = "/effects/1/steps/0/on";
    assert.deepEqual(refAt(glyph, 0, glyphPath), { path: glyphPath, unresolved: "ambiguous-antecedent" });
    const arena = card("{3}, {T}: Tap target creature you control and target creature of an opponent's choice they control. Those creatures fight each other.",
      "Land", "Arena");
    const arenaPath = "/effects/1/steps/0/what";
    assert.deepEqual(refAt(arena, 0, arenaPath), { path: arenaPath, unresolved: "ambiguous-antecedent" });
  });

  it("keeps the destroyed target's controller distinct from the searched land in Erode", () => {
    const text = "Destroy target creature or planeswalker. Its controller may search their library for a basic land card, put it onto the battlefield tapped, then shuffle.";
    const erode = card(text, "Instant", "Erode");
    assert.equal(erode.resolution.complete, true);
    const group = "/effects/1/steps/0";
    for (const field of ["steps/1/who/of", "steps/2/who/of", "steps/2/library/owner/of"]) {
      const path = `${group}/${field}`;
      assert.deepEqual(refAt(erode, 0, path), { path, ref: { ref: "target", slot: 0 } });
    }
    const path = `${group}/steps/1/what`;
    assert.deepEqual(refAt(erode, 0, path), { path, ref: { ref: "step-result", step: `${group}/steps/0/for`, role: "selection" } });
    for (const nearMiss of [text.replace("their library", "your library"), text.replace("target creature or planeswalker", "two target creatures")]) {
      const invalid = card(nearMiss, "Instant");
      const pronoun = `${group}/steps/1/who/of`;
      assert.deepEqual(refAt(invalid, 0, pronoun), { path: pronoun, unresolved: "ambiguous-antecedent" });
    }
  });

  it("counts the matching attackers from your declaration and leaves an opponent's declaration unresolved", () => {
    const text = "Whenever you attack with one or more legendary creatures, you gain that much life.";
    const path = "/effects/0/steps/0/amount";
    assert.deepEqual(refAt(card(text, "Enchantment"), 0, path),
      { path, ref: { ref: "step-result", step: "/trigger", role: "quantity" } });
    assert.deepEqual(refAt(card(text.replace("you attack", "an opponent attacks"), "Enchantment"), 0, path),
      { path, unresolved: "missing-antecedent" });
  });

  it("derives Nissa's owner from the exile successor and leaves a competing target unresolved", () => {
    const text = "Whenever a land you control enters, if you control seven or more lands, exile Nissa, then return her to the battlefield transformed under her owner's control.";
    const typeLine = "Legendary Creature — Elf Scout";
    const nissa = card(text, typeLine, "Nissa, Vastwood Seer");
    const group = "/effects/0/steps";
    const exile = { ref: "step-result", step: `${group}/0`, role: "objects" };
    assert.deepEqual(refAt(nissa, 0, `${group}/1/what`), { path: `${group}/1/what`, ref: exile });
    assert.deepEqual(refAt(nissa, 0, `${group}/1/controller`),
      { path: `${group}/1/controller`, derived: { relation: "owner", of: exile } });
    const competing = card(text.replace("exile Nissa", "exile target creature").replace("return her", "return it")
      .replace("her owner's", "its owner's"), typeLine, "Nissa, Vastwood Seer");
    const path = `${group}/1/controller`;
    assert.deepEqual(refAt(competing, 0, path), { path, unresolved: "missing-antecedent" });
  });

  it("preserves the residuals for bare damage recipients and counter sets", () => {
    const vortex = card("At the beginning of each player's upkeep, this enchantment deals 1 damage to them.", "Enchantment");
    assert.ok(vortex.binding.diagnostics.some(d => d.path.endsWith("/steps/0/to") && d.code === "unsupported-scope"));
    const iron = card("When this creature dies, if it had counters on it, put those counters on target creature you control.", "Artifact Creature — Construct");
    const path = "/effects/0/steps/0/counters";
    assert.deepEqual(refAt(iron, 0, path), { path, unresolved: "missing-antecedent" });
  });
});
