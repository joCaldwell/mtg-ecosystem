import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/index.ts";
import type { StaticEffect } from "../src/ast.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { lex } from "../src/lexer.ts";
import { parseStatic } from "../src/parser/statics.ts";

function one(text: string, name?: string): StaticEffect {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  const ability = result.abilities[0];
  assert.equal(ability.kind, "static");
  return ability.effect;
}

describe("conditional battlefield entry replacements", () => {
  // Card wording checked in .scryfall-cache/oracle-cards.json. Entry effects
  // are replacement effects under CR 614.1d and 614.12, not triggered abilities.
  it("retains the checkland subtype union and unless condition", () => {
    assert.deepEqual(one("This land enters tapped unless you control a Plains or an Island."), {
      static: "enters-tapped", what: { ref: "this", noun: "land" }, conditionKind: "unless",
      condition: { condition: "control", who: { player: "you" }, what: {
        ref: "filter", count: { amount: "fixed", value: 1 },
        filter: { anyOf: [{ subtypes: ["Plains"] }, { subtypes: ["Island"] }] },
      } },
    });
  });

  it("keeps slowland and fastland comparisons distinct, including other", () => {
    for (const [word, op] of [["more", "ge"], ["fewer", "le"]]) {
      assert.deepEqual(one(`This land enters tapped unless you control two or ${word} other lands.`), {
        static: "enters-tapped", what: { ref: "this", noun: "land" }, conditionKind: "unless",
        condition: { condition: "control-count", who: { player: "you" }, of: { other: true, types: ["land"] },
          comparison: { op, value: { amount: "fixed", value: 2 } } },
      });
    }
  });

  it("distinguishes Eddymurk Crab's if from land unless clauses", () => {
    assert.deepEqual(one("This creature enters tapped if it's not your turn."), {
      static: "enters-tapped", what: { ref: "this", noun: "creature" },
      condition: { condition: "not-your-turn" }, conditionKind: "if",
    });
    assert.deepEqual(one("Rivendell enters tapped unless you control a legendary creature.", "Rivendell"), {
      static: "enters-tapped", what: { ref: "self" }, conditionKind: "unless",
      condition: { condition: "control", who: { player: "you" }, what: {
        ref: "filter", count: { amount: "fixed", value: 1 }, filter: { supertypes: ["legendary"], types: ["creature"] },
      } },
    });
  });

  it("retains plural subjects and conditional counter counts", () => {
    assert.deepEqual(one("Creatures your opponents control enter tapped."), {
      static: "enters-tapped", what: { ref: "each", filter: { types: ["creature"], control: "opponent" } },
    });
    assert.deepEqual(one("This creature enters with two +1/+1 counters on it if it's your turn."), {
      static: "enters-with-counters", what: { ref: "this", noun: "creature" },
      counter: { type: "+1/+1" }, count: { amount: "fixed", value: 2 },
      conditionKind: "if", condition: { condition: "your-turn" },
    });
  });

  it("rejects incomplete or unrepresented entry clauses without leaking cursor state", () => {
    for (const text of ["This land enters tapped unless.",
      "This land enters tapped unless you control a Plains or an Island and a Swamp.",
      "This land enters tapped unless you revealed a Dragon card this way."]) {
      const c = new Cursor(lex(text));
      assert.equal(parseStatic(c), null, text);
      assert.equal(c.pos, 0, text);
      assert.equal(parseOracleText(text).ok, false, text);
    }
    // "tapped and with" is the same entry group as "tapped with"
    // (Noble's Purse, Sphere of the Suns; CR 614.1c-d, 614.12).
    const what = { ref: "this", noun: "creature" };
    const group = { static: "group", effects: [{ static: "enters-tapped", what },
      { static: "enters-with-counters", what, counter: { type: "+1/+1" }, count: { amount: "fixed", value: 3 } }] };
    assert.deepEqual(one("This creature enters tapped and with three +1/+1 counters on it."), group);
    assert.deepEqual(one("This creature enters tapped with three +1/+1 counters on it."), group);
    // A dangling conjunction, or one that hides a third clause, still fails.
    for (const text of ["This creature enters tapped and.", "This creature enters tapped and with flying.",
      "This creature enters tapped and with three +1/+1 counters on it and doesn't untap during your untap step."])
      assert.equal(parseOracleText(text, undefined, { typeLine: "Creature — Elf" }).ok, false, text);
  });
});

describe("continuous effect conditions and characteristic definitions", () => {
  it("keeps continuous multipliers on the affected modifier only", () => {
    assert.deepEqual(one("Equipped creature gets +1/+0 for each Gate you control and has vigilance and menace."), {
      static: "modify", what: { ref: "equipped", noun: "creature" }, condition: undefined, duration: undefined,
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 1 } },
        toughness: { sign: 1, amount: { amount: "fixed", value: 0 } }, forEach: { subtypes: ["Gate"], control: "you" } },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "vigilance" } },
        { gained: "keyword", keyword: { keyword: "menace" } }] }],
    });
  });

  it("binds continuous X expressions without changing them into resolution instructions", () => {
    assert.deepEqual(one("Equipped creature gets +X/+1, where X is the number of artifacts you control."), {
      static: "modify", what: { ref: "equipped", noun: "creature" }, condition: undefined, duration: undefined,
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: { amount: "x" } },
        toughness: { sign: 1, amount: { amount: "fixed", value: 1 } } }],
      bindings: [{ variable: "x", value: { amount: "count", of: { types: ["artifact"], control: "you" } } }],
    });
  });

  it("represents can't-unless restrictions as continuous abilities", () => {
    assert.deepEqual(one("This creature can't attack unless you control an artifact."), {
      static: "cant", what: { ref: "this", noun: "creature" }, action: "attack", conditionKind: "unless",
      condition: { condition: "control", who: { player: "you" }, what: {
        ref: "filter", filter: { types: ["artifact"] }, count: { amount: "fixed", value: 1 },
      } },
    });
  });

  it("preserves every independently changing condition on Tek", () => {
    const lands = ["Plains", "Island", "Swamp", "Mountain", "Forest"];
    const modifiers = [
      { modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 0 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 2 } } },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] },
      { modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 2 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 0 } } },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "first strike" } }] },
      { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }] },
    ].map((modifier, index) => ({ ...modifier, condition: { condition: "control", who: { player: "you" }, what: {
      ref: "filter", count: { amount: "fixed", value: 1 }, filter: { subtypes: [lands[index]] },
    } } }));
    assert.deepEqual(one("This creature gets +0/+2 as long as you control a Plains, has flying as long as you control an Island, gets +2/+0 as long as you control a Swamp, has first strike as long as you control a Mountain, and has trample as long as you control a Forest."), {
      static: "modify", what: { ref: "this", noun: "creature" }, modifiers, condition: undefined, duration: undefined,
    });
  });

  it("models continuous Otherwise branches separately from spell-resolution branches", () => {
    assert.deepEqual(one("As long as you control enchanted creature, it gets +2/+2. Otherwise, it can't block."), {
      static: "conditional", condition: { condition: "control", who: { player: "you" }, what: { ref: "enchanted", noun: "creature" } },
      effect: { static: "modify", what: { ref: "it" }, condition: undefined, duration: undefined, modifiers: [
        { modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 2 } },
          toughness: { sign: 1, amount: { amount: "fixed", value: 2 } } },
      ] }, otherwise: { static: "cant", what: { ref: "it" }, action: "block", condition: undefined },
    });
  });

  it("parses quoted token scaling as a continuous ability", () => {
    const result = parseOracleText('Create a 0/0 colorless Construct artifact creature token with "This token gets +1/+1 for each artifact you control."');
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    const ability = result.abilities[0];
    assert.equal(ability.kind, "spell");
    const effect = ability.effects[0].steps[0];
    assert.equal(effect.effect, "create-token");
    assert.ok(!("predefined" in effect.token));
    assert.ok(!("copyOf" in effect.token));
    assert.deepEqual(effect.token.abilities, [{ gained: "quoted", ability: { kind: "static", abilityWord: undefined, effect: {
      static: "modify", what: { ref: "this", noun: "token" }, condition: undefined, duration: undefined,
      modifiers: [{ modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 1 } },
        toughness: { sign: 1, amount: { amount: "fixed", value: 1 } }, forEach: { types: ["artifact"], control: "you" } }],
    } } }]);
  });

  it("grants shroud or hexproof to the player, not an object subtype named You", () => {
    for (const keyword of ["shroud", "hexproof"]) {
      assert.deepEqual(one(`You have ${keyword}.`), {
        static: "player-abilities", who: { player: "you" },
        abilities: [{ gained: "keyword", keyword: { keyword } }],
      });
    }
    assert.deepEqual(one("Each player has hexproof."), {
      static: "player-abilities", who: { player: "each-player" },
      abilities: [{ gained: "keyword", keyword: { keyword: "hexproof" } }],
    });
    assert.equal(parseOracleText("You have hexproof until end of turn.").ok, false);
  });

  it("preserves both an outer condition and a trailing condition", () => {
    assert.deepEqual(one("As long as it's your turn, this creature gets +1/+1 as long as you control an artifact."), {
      static: "conditional", condition: { condition: "your-turn" }, effect: {
        static: "modify", what: { ref: "this", noun: "creature" }, duration: undefined,
        modifiers: [{ modifier: "pt", power: { sign: 1, amount: { amount: "fixed", value: 1 } },
          toughness: { sign: 1, amount: { amount: "fixed", value: 1 } } }],
        condition: { condition: "control", who: { player: "you" }, what: {
          ref: "filter", filter: { types: ["artifact"] }, count: { amount: "fixed", value: 1 },
        } },
      },
    });
  });

  it("supports possessive this-object subjects without assigning another object's CDA", () => {
    assert.deepEqual(one("This creature's power and toughness are each equal to the number of artifacts you control."), {
      static: "cda-pt", what: { ref: "this", noun: "creature" }, stat: "both",
      value: { amount: "count", of: { types: ["artifact"], control: "you" } },
    });
    assert.equal(parseOracleText("Enchanted creature's power is equal to the number of artifacts you control.").ok, false);
    assert.deepEqual(one("As long as it's your turn, this creature's power is equal to the number of lands you control."), {
      static: "conditional", condition: { condition: "your-turn" }, effect: {
        static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "base-pt",
          power: { amount: "count", of: { types: ["land"], control: "you" } },
        }],
      },
    });
  });

  it("distinguishes colorless from all colors and rejects unrepresented riders", () => {
    assert.deepEqual(one("Ghostfire is colorless.", "Ghostfire"), {
      static: "cda-colors", what: { ref: "self" }, operation: "set", colors: [],
    });
    assert.deepEqual(one("Transguild Courier is all colors.", "Transguild Courier"), {
      static: "cda-colors", what: { ref: "self" }, operation: "set", colors: ["white", "blue", "black", "red", "green"],
    });
    for (const text of ["This creature is all colors. This ability doesn't affect its color identity."]) {
      assert.equal(parseOracleText(text).ok, false, text);
    }
    const colors = ["white", "blue", "black", "red", "green"];
    assert.deepEqual(JSON.parse(JSON.stringify(one("Each nonland permanent you control is all colors."))), {
      static: "modify", what: { ref: "each", filter: { nonTypes: ["land"], cls: "permanent", control: "you" } },
      modifiers: [{ modifier: "colors", operation: "set", colors }],
    });
    assert.deepEqual(one("As long as it's your turn, this creature is all colors."), {
      static: "conditional", condition: { condition: "your-turn" }, effect: {
        static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "colors", operation: "set", colors }],
      },
    });
  });

  it("rejects a dangling static-modifier conjunction", () => {
    const c = new Cursor(lex("Creatures you control get +1/+1 and."));
    assert.equal(parseStatic(c), null);
    assert.equal(c.pos, 0);
  });
});
