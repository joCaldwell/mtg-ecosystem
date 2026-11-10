import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { GainedAbility, ProtectionScope } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseKeyword } from "../src/parser/keywords.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle: Mother of Runes, Pale Wayfarer, Wishmonger, Pippin Guard of the
// Citadel, Seht's Tiger, Samite Elder, Empty-Shrine Kannushi, Pledge of Loyalty.
// A fresh choice is made while the granting instruction applies (608.2d).
// Colors of a specified set: protection from each such color (702.16i), with
// resolving/static information evaluation distinguished by 608.2h/611.3a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed" as const, value });
const creature = { ref: "target" as const, filter: { types: ["creature"] }, count: n(1) };
const eot = { duration: "end-of-turn" as const };
const ability = (scope: ProtectionScope): GainedAbility => ({ gained: "keyword", keyword: { keyword: "protection", from: [scope] } });
const fresh: ProtectionScope = { scope: "fresh-choice", category: "color", who: { player: "you" }, when: "granting-effect-applies" };
function parse(text: string) {
  const result = parseOracleText(text);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe("protection choices when granting effects apply", () => {
  it("retains a fresh choice separately from target choice, prior linked color and fixed colors", () => {
    assert.deepEqual(parse("{T}: Target creature you control gains protection from the color of your choice until end of turn."), [{
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "gain-abilities", what: { ...creature, filter: { types: ["creature"], control: "you" } }, abilities: [ability(fresh)], duration: eot }] }],
    }]);
    const result = parse("Target creature gains protection from the chosen color and from red until end of turn.");
    assert.deepEqual(result, [{ kind: "spell", effects: [{ steps: [{ effect: "gain-abilities", what: creature, abilities: [{ gained: "keyword", keyword: { keyword: "protection", from: [
      { scope: "chosen-value", value: { choice: "chosen-value", category: "color", which: "chosen" } }, { scope: "color", color: "red" },
    ] } }], duration: eot }] }] }]);
  });
  it("preserves the affected creature controller as chooser independently of activation permission", () => {
    const controllerChoice: ProtectionScope = { scope: "fresh-choice", category: "color", who: { player: "controller", of: { ref: "it" } }, when: "granting-effect-applies" };
    assert.deepEqual(parse("{2}: Target creature gains protection from the color of its controller's choice until end of turn. Any player may activate this ability."), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2"] }], permission: { activators: "any-player" },
      effects: [{ steps: [{ effect: "gain-abilities", what: creature, abilities: [ability(controllerChoice)], duration: eot }] }],
    }]);
    assert.deepEqual(parse("{2}{W}{W}, {Q}: Target creature gains protection from the color of its controller's choice until end of turn."), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2", "W", "W"] }, { cost: "untap-self" }],
      effects: [{ steps: [{ effect: "gain-abilities", what: creature, abilities: [ability(controllerChoice)], duration: eot }] }],
    }]);
  });
  it("retains card-type choices and grants protection to players", () => {
    assert.deepEqual(parse("Another target creature you control gains protection from the card type of your choice until end of turn."), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities", what: { ...creature, filter: { types: ["creature"], control: "you", other: true } },
        abilities: [ability({ ...fresh, category: "card-type" })], duration: eot }] }],
    }]);
    assert.deepEqual(parse("When this creature enters, you gain protection from the color of your choice until end of turn."), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "player-gain-abilities", who: { player: "you" }, abilities: [ability(fresh)], duration: eot }] }],
    }]);
  });
  it("does not invent a resolution for an intrinsic or continuous static choice", () => {
    for (const text of ["Protection from the color of your choice", "Enchanted creature has protection from the color of your choice.",
      "Creatures you control have protection from the color of your choice.", "You have protection from the color of your choice.",
      "This creature gets +1/+1 and has protection from the color of your choice.",
      "Create a 1/1 white Soldier creature token with protection from the color of your choice.",
      "You get an emblem with protection from the color of your choice.",
      "Target creature loses protection from the color of your choice until end of turn.",
      "This creature gains protection from the colors of your choice until end of turn.",
      "This creature gains protection from the creature type of your choice until end of turn.",
      "This creature gains protection from the color of each player's choice until end of turn."])
      assert.equal(parseOracleText(text).ok, false, text);
    const cursor = new Cursor(lex("protection from the color of your choice"));
    assert.equal(parseKeyword(cursor), null);
    assert.equal(cursor.pos, 0);
    assert.deepEqual(parse("Enchanted creature has \"{T}: This creature gains protection from the color of your choice until end of turn.\"."), [{
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted", ability: {
        kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "gain-abilities", what: { ref: "this", noun: "creature" }, abilities: [ability(fresh)], duration: eot }] }],
      } }] }] },
    }]);
  });
  it("preserves explicit player grants and does not steal an elided plural object subject", () => {
    const what = { ref: "each", filter: { types: ["creature"], control: "you" } };
    assert.deepEqual(parse("Creatures you control get +1/+1 and gain protection from the color of your choice until end of turn."), [{ kind: "spell", effects: [{ steps: [
      { effect: "pump", what, power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) }, duration: eot },
      { effect: "gain-abilities", what, abilities: [ability(fresh)], duration: eot },
    ] }] }]);
    assert.deepEqual(parse("You may gain protection from the color of your choice until end of turn."), [{ kind: "spell", effects: [{ steps: [
      { effect: "player-gain-abilities", who: { player: "you" }, abilities: [ability(fresh)], duration: eot, optional: true },
    ] }] }]);
    assert.deepEqual(parse("Until end of turn, target player gains protection from blue."), [{ kind: "spell", effects: [{ steps: [
      { effect: "player-gain-abilities", who: { player: "target-player" }, abilities: [ability({ scope: "color", color: "blue" })], duration: eot },
    ] }] }]);
  });
  it("retains dynamic colors in a static grant and a target-derived color set in a resolving grant", () => {
    const colors: ProtectionScope = { scope: "colors-of", what: { ref: "each", filter: { cls: "permanent", control: "you" } } };
    assert.deepEqual(parse("This creature has protection from the colors of permanents you control."), [{ kind: "static", effect: {
      static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [ability(colors)] }],
    } }]);
    assert.deepEqual(parse("Enchanted creature has protection from the colors of permanents you control. This effect doesn't remove this Aura."), [{ kind: "static", effect: {
      static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [{ ...ability(colors), protectionAttachmentException: { exception: "source-aura", aura: { ref: "this", noun: "aura" } } }] }],
    } }]);
    assert.deepEqual(parse("{T}: Creatures you control gain protection from the colors of target permanent you control until end of turn."), [{ kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{
      effect: "gain-abilities", what: { ref: "each", filter: { types: ["creature"], control: "you" } }, abilities: [ability({ scope: "colors-of", what: { ref: "target", filter: { cls: "permanent", control: "you" }, count: n(1) } })], duration: eot,
    }] }] }]);
  });
  it("binds a granted protection's choice and derived colors in the granting scope", () => {
    for (const text of ["Target creature gains protection from the color of your choice until end of turn.",
      "This creature has protection from the colors of permanents you control."]) {
      const result = parseOracleText(text, "Protection fixture", { includeSourceProvenance: true });
      assert.ok(result.ok);
      const binding = analyzeCardBindings({ identity: "protection-fixture", source: { name: "Protection fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(binding.fullyBound, true, JSON.stringify(binding.diagnostics));
      // The choice and the colors are fixed by the granting effect (CR 608.2d, 608.2h), so
      // their "you" is the granting ability's player, not the recipient's ability's.
      const line = binding.nodes.find(node => node.node === "ability" && node.path === "/faces/0/abilities/0");
      assert.ok(line && line.node === "ability");
      const player = binding.references.find(ref => /\/from\/0\/(who|what\/filter\/control)$/.test(ref.path));
      assert.ok(player && player.binding === "bound" && player.target === line.you, JSON.stringify(player));
    }
  });
});
