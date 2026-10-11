import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseLookPermissionEffect, parsePersistentRule } from "../src/parser/persistent-rules.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Oracle wording: Lens of Clarity, Keeper of the Lens, Found Footage, Radha,
// Heart of Keld, Glowcap Lantern, Lumbering Laundry, The Belligerent, Giant
// Oyster, The Pandorica, and Teferi, Temporal Archmage in the pinned corpus.
// CR401.5,502.3,708.5,903.3a:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: "you" };
const oneCard = { amount: "fixed", value: 1 };
const top = { ref: "library-cards", position: "top", count: oneCard, of: { zone: "library", owner: "your" } };
const hidden = (control: string) => ({ ref: "each", filter: { status: ["face-down"], types: ["creature"], control } });
const libraryPermission = { who: you, what: [top], timing: "any-time", topLibraryChanges: "after-casting-activation-or-special-action" };
const hiddenPermission = { who: you, what: [hidden("not-you")], timing: "any-time" };
const lookStatic = { static: "look-permission", permission: libraryPermission };
const untilEnd = { duration: "end-of-turn" };
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}

describe("persistent information, deck, and untap rules", () => {
  it("stores private inspection of the changing library top as a persistent permission", () => {
    assert.deepEqual(one("You may look at the top card of your library any time."), { kind: "static", effect: lookStatic });
    assert.deepEqual(one("You may look at the top card of your library."), {
      kind: "spell", effects: [{ steps: [{ effect: "look-at", who: you,
        what: { top: oneCard, of: { zone: "library", owner: "your" } }, optional: true }] }],
    });
  });

  it("keeps face-down object inspection independent of library-top transition restrictions", () => {
    for (const [wording, control] of [["you don't control", "not-you"], ["your opponents control", "opponent"]]) {
      assert.deepEqual(one(`You may look at face-down creatures ${wording} any time.`), {
        kind: "static", effect: { static: "look-permission", permission: { who: you, what: [hidden(control)], timing: "any-time" } },
      });
    }
    assert.deepEqual(one("You may look at the top card of your library and at face-down creatures you don't control any time."), {
      kind: "static", effect: { static: "look-permission", permission: { ...libraryPermission, what: [top, hidden("not-you")] } },
    });
  });

  it("retains the independent land-play permission in Radha's coordinated static ability", () => {
    assert.deepEqual(one("You may look at the top card of your library any time, and you may play lands from the top of your library."), {
      kind: "static", effect: { static: "group", effects: [lookStatic, { static: "play-permission", permission: {
        who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } },
        from: { zone: "library", owner: "your", position: "top" },
      } }] },
    });
  });

  it("keeps outer static conditions on the information permission", () => {
    assert.deepEqual(one("As long as you control an artifact, you may look at the top card of your library any time."), {
      kind: "static", effect: { static: "conditional", condition: { condition: "control", who: you,
        what: { ref: "filter", count: oneCard, filter: { types: ["artifact"] } } }, effect: lookStatic },
    });
  });

  it("retains the source scope of quoted look and attack abilities on Glowcap Lantern", () => {
    assert.deepEqual(one('Equipped creature has "You may look at the top card of your library any time" and "Whenever this creature attacks, it explores."'), {
      kind: "static", effect: { static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [
        { modifier: "abilities", abilities: [
          { gained: "quoted", ability: { kind: "static", effect: lookStatic } },
          { gained: "quoted", ability: { kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
            effects: [{ steps: [{ effect: "explore", what: { ref: "it" } }] }] } },
        ] },
      ] },
    });
  });

  it("stores commander eligibility as a pregame rule attached to the source card", () => {
    assert.deepEqual(one("Teferi, Temporal Archmage can be your commander.", "Teferi, Temporal Archmage"), {
      kind: "static", effect: { static: "commander-eligibility", what: { ref: "self" }, during: "deck-construction" },
    });
    assert.deepEqual(one("This planeswalker can be your commander."), {
      kind: "static", effect: { static: "commander-eligibility", what: { ref: "this", noun: "planeswalker" }, during: "deck-construction" },
    });
  });

  it("preserves a choice during each normal untap step without creating an untap prohibition", () => {
    for (const noun of ["creature", "artifact", "land"]) {
      assert.deepEqual(one(`You may choose not to untap this ${noun} during your untap step.`), {
        kind: "static", effect: { static: "optional-untap", who: you, what: { ref: "this", noun }, step: { occurrence: "each", owner: you } },
      });
    }
    assert.deepEqual(one("You may choose not to untap The Pandorica during your untap step.", "The Pandorica"), {
      kind: "static", effect: { static: "optional-untap", who: you, what: { ref: "self" }, step: { occurrence: "each", owner: you } },
    });
    assert.deepEqual(one("You may untap this creature."), { kind: "spell", effects: [{ steps: [
      { effect: "untap", what: { ref: "this", noun: "creature" }, optional: true },
    ] }] });
  });

  it("creates a temporary look permission without an optional creation flag", () => {
    for (const text of [
      "Until end of turn, you may look at the top card of your library any time.",
      "You may look at the top card of your library any time until end of turn.",
    ]) assert.deepEqual(one(text), { kind: "spell", effects: [{ steps: [
      { effect: "grant-look-permission", permission: libraryPermission, duration: untilEnd },
    ] }] });
    assert.deepEqual(one("{2}: Until end of turn, you may look at face-down creatures you don't control any time."), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["2"] }], effects: [{ steps: [
        { effect: "grant-look-permission", permission: hiddenPermission, duration: untilEnd },
      ] }],
    });
  });

  it("keeps a temporary look/play/cast conjunction under the same printed duration", () => {
    assert.deepEqual(one("Until end of turn, you may look at the top card of your library any time, and you may play lands and cast spells from the top of your library."), {
      kind: "spell", effects: [{ steps: [
        { effect: "grant-look-permission", permission: libraryPermission, duration: untilEnd },
        { effect: "simultaneous", steps: [
          { effect: "grant-play-permission", permission: { who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } },
            from: { zone: "library", owner: "your", position: "top" } }, duration: untilEnd },
          { effect: "grant-play-permission", permission: { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } },
            from: { zone: "library", owner: "your", position: "top" } }, duration: untilEnd },
        ] },
      ] }] },
    );
  });

  it("rejects unsupported domains, altered step windows, targets, and unrepresented deck rules", () => {
    for (const text of [
      "You may look at the top two cards of your library any time.",
      "You may look at the bottom card of your library any time.",
      "You may look at a face-down creature you don't control any time.",
      "Target player may look at the top card of your library any time.",
      "You may look at the top card of your library any time except during combat.",
      "This planeswalker can be your commander until end of turn.",
      "Target creature can be your commander.",
      "This card can be your commander. If it is, your deck can contain any basic land cards and cards of any color identity that can create Food tokens.",
      "You may choose not to untap this creature during your next untap step.",
      "You may choose not to untap this creature during your upkeep.",
      "You may choose not to untap this creature during your untap step unless you pay {1}.",
      "You may choose not to untap target creature during your untap step.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });

  it("backtracks failed persistent and resolving parses completely", () => {
    for (const text of [
      "You may look at the top card of your library any time except during combat.",
      "This creature can be your commander and draw a card.",
      "You may choose not to untap this creature during your next untap step.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parsePersistentRule(c), null, text);
      assert.equal(c.pos, 0);
    }
    const c = new Cursor(lex("You may look at the top card of your library any time."));
    assert.equal(parseLookPermissionEffect(c), null);
    assert.equal(c.pos, 0);
  });

  it("keeps new rule scopes explicitly unbound", () => {
    for (const text of [
      "You may look at the top card of your library any time.",
      "Until end of turn, you may look at the top card of your library any time.",
      "This card can be your commander.",
      "You may choose not to untap this creature during your untap step.",
    ]) {
      const name = "Persistent fixture";
      const result = parseOracleText(text, name, { includeSourceProvenance: true });
      assert.equal(result.ok, true);
      const analysis = analyzeCardBindings({ identity: name, source: { name, layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(analysis.fullyBound, false);
      assert.ok(analysis.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"));
    }
  });
});
