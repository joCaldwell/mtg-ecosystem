import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { GainedAbility, ProtectionAttachmentException, ProtectionScope, StaticModifier } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseProtectionAttachmentException } from "../src/parser/protection.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Cached Oracle text: Blue Ward, Spectra Ward, Tattoo Ward, Floating Shield,
// Benevolent Blessing. CR 702.16n/p exempt only the specified protection
// instance's attachment SBA; new attachments/targeting retain protection.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const creature = { ref: "enchanted" as const, noun: "creature" };
const aura = { ref: "this" as const, noun: "aura" };
const sourceAura: ProtectionAttachmentException = { exception: "source-aura", aura };
const blue: ProtectionScope = { scope: "color", color: "blue" };
const chosen: ProtectionScope = { scope: "chosen-value", value: { choice: "chosen-value", category: "color", which: "chosen" } };
const protection = (from: ProtectionScope, exception?: ProtectionAttachmentException): GainedAbility => ({
  gained: "keyword", keyword: { keyword: "protection", from: [from] }, ...(exception ? { protectionAttachmentException: exception } : {}),
});
function abilities(from: ProtectionScope, exception?: ProtectionAttachmentException): StaticModifier {
  return { modifier: "abilities", abilities: [protection(from, exception)] };
}
function parse(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function expectStatic(text: string, modifiers: StaticModifier[]) {
  assert.deepEqual(parse(text), [{ kind: "static", effect: { static: "modify", what: creature, modifiers } }]);
}

describe("instance-local protection attachment exceptions", () => {
  it("keeps the granting Aura in its own scope, not the enchanted creature's", () => {
    expectStatic("Enchanted creature has protection from blue. This effect doesn't remove this Aura.", [abilities(blue, sourceAura)]);
    expectStatic("Enchanted creature has protection from the chosen color. This effect doesn't remove this Aura.", [abilities(chosen, sourceAura)]);
    expectStatic("Enchanted creature has protection from blue.", [abilities(blue)]);
  });
  it("scopes the all-Auras exception to the protection grant, not the PT modifier", () => {
    const signed = (value: number) => ({ sign: 1 as const, amount: { amount: "fixed" as const, value } });
    expectStatic("Enchanted creature gets +2/+2 and has protection from each color. This effect doesn't remove Auras.", [
      { modifier: "pt", power: signed(2), toughness: signed(2) }, abilities({ scope: "all-colors" }, { exception: "auras" }),
    ]);
    expectStatic("Enchanted creature gets +1/+1 and has protection from enchantments. This effect doesn't remove this Aura.", [
      { modifier: "pt", power: signed(1), toughness: signed(1) }, abilities({ scope: "filter", filter: { types: ["enchantment"] } }, sourceAura),
    ]);
  });
  it("retains Benevolent Blessing's attachment snapshot and controller filter", () => {
    expectStatic("Enchanted creature has protection from the chosen color. This effect doesn't remove Auras and Equipment you control that are already attached to it.", [abilities(chosen, {
      exception: "already-attached", filter: { anyOf: [{ subtypes: ["Aura"] }, { subtypes: ["Equipment"] }], control: "you" },
      to: creature, snapshot: "when-protection-effect-begins",
    })]);
  });
  it("does not copy the exception to a separate protection grant or activation", () => {
    assert.deepEqual(parse("Enchanted creature has protection from the chosen color. This effect doesn't remove this Aura.\nSacrifice this Aura: Target creature gains protection from the chosen color until end of turn."), [
      { kind: "static", effect: { static: "modify", what: creature, modifiers: [abilities(chosen, sourceAura)] } },
      { kind: "activated", costs: [{ cost: "sacrifice", what: aura }], effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } }, abilities: [protection(chosen)], duration: { duration: "end-of-turn" } }] }] },
    ]);
  });
  it("rejects unsupported subjects, conflicting grants, changed timing and extra clauses", () => {
    for (const text of [
      "This creature has protection from blue. This effect doesn't remove this Aura.",
      "Enchanted creature has flying. This effect doesn't remove this Aura.",
      "Enchanted creature has protection from blue and protection from red. This effect doesn't remove this Aura.",
      "Enchanted creature has protection from blue. This effect doesn't remove Equipment.",
      "Enchanted creature has protection from blue. This effect doesn't remove Auras and Equipment you control attached to it.",
      "Enchanted creature has protection from blue. This effect doesn't remove Auras and Equipment your opponents control that are already attached to it.",
      "Enchanted creature has protection from blue. This effect doesn't remove Auras and Equipment you control that become attached to it.",
      "Enchanted creature has protection from blue. This effect doesn't remove this Aura and it can be targeted.",
      "Enchanted creature has protection from blue. This effect doesn't remove this Aura. Draw a card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const input = [abilities(blue)];
    const snapshot = JSON.stringify(input);
    const cursor = new Cursor(lex(". This effect doesn't remove Auras and Equipment you control."));
    assert.equal(parseProtectionAttachmentException(cursor, creature, input), null);
    assert.equal(cursor.pos, 0);
    assert.equal(JSON.stringify(input), snapshot);
  });
  it("leaves grant-source and snapshot relations explicitly unbound", () => {
    const text = "Enchanted creature has protection from blue. This effect doesn't remove Auras and Equipment you control that are already attached to it.";
    const result = parseOracleText(text, "Test Aura", { includeSourceProvenance: true });
    assert.ok(result.ok);
    const analysis = analyzeCardBindings({ identity: "test-aura", source: { name: "Test Aura", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"));
  });
});
