import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { Ability, Effect, ObjectRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseObjectRef, parsePlayerRef, parseZoneRef } from "../src/parser/refs.ts";

// Target occurrences are syntactic selection identities, not chosen game
// objects. CR 601.2c permits separately specified targets to coincide while
// retaining their distinct selection requirements.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const creature = (selectionId: string): Extract<ObjectRef, { ref: "target" }> => ({
  ref: "target", filter: { types: ["creature"] }, count: one, selectionId,
});

function abilities(text: string): Ability[] {
  const result = parseOracleText(text, undefined, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return result.abilities;
}

function effects(text: string): Effect[] {
  const result = abilities(text);
  assert.equal(result.length, 1);
  assert.ok(result[0].kind === "spell");
  return result[0].effects.flatMap(sentence => sentence.steps);
}

describe("optional token provenance", () => {
  it("records every original token index and survives slices and clones", () => {
    const text = "{T}: Destroy target creature.";
    const raw = lex(text);
    const tokens = lex(text, "example");
    assert.deepEqual(tokens, raw.map((token, index) => ({ ...token, provenance: { sourceId: "example", index } })));
    const c = new Cursor(tokens.slice(3).map(token => ({ ...token })));
    assert.deepEqual(parseObjectRef(c), creature("example:3"));
    assert.equal(c.isPunct("."), true);
    assert.equal(raw.some(token => "provenance" in token), false);
  });

  it("leaves ordinary syntax ASTs unchanged unless explicitly enabled", () => {
    const result = parseOracleText("Destroy target creature.");
    assert.ok(result.ok);
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined,
      effects: [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["creature"] }, count: one } }] }],
    }]);
    assert.deepEqual(parseOracleText("Destroy target creature.", undefined, { includeSourceProvenance: false }), result);
  });

  it("does not mint a target identity for an untargeted choice or a reference", () => {
    for (const text of ["a creature", "each creature", "that creature", "it", "any number of creatures"]) {
      const c = new Cursor(lex(text, "example"));
      const result = parseObjectRef(c);
      assert.ok(result && c.done(), text);
      assert.equal("selectionId" in result, false, text);
    }
  });
});

describe("target selection provenance", () => {
  it("uses the target word after count prefixes and keeps chosen cardinality", () => {
    const cases: [string, ObjectRef][] = [
      ["up to two other target creatures", { ref: "target", filter: { types: ["creature"], other: true }, count: { amount: "fixed", value: 2 }, upTo: true, selectionId: "example:4" }],
      ["another target creature", { ...creature("example:1"), filter: { types: ["creature"], other: true } }],
      ["one or two target creatures", { ref: "target", filter: { types: ["creature"] }, count: { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: 2 } }, selectionId: "example:3" }],
      ["any number of other target creatures", { ref: "target", filter: { types: ["creature"], other: true }, count: { amount: "chosen", minimum: 0 }, selectionId: "example:4" }],
      ["any number of targets", { ref: "any-target", count: { amount: "chosen", minimum: 0 }, selectionId: "example:3" }],
      ["any target", { ref: "any-target", selectionId: "example:1" }],
    ];
    for (const [text, expected] of cases) {
      const c = new Cursor(lex(text, "example"));
      assert.deepEqual(parseObjectRef(c), expected, text);
      assert.equal(c.done(), true, text);
    }
  });

  it("retains one selector for an elided subject and distinguishes a repeated explicit target", () => {
    const common = {
      effect: "pump", what: creature("line:0:0"), power: { sign: 1, amount: one }, toughness: { sign: 1, amount: one },
    };
    const grant = { effect: "gain-abilities", what: creature("line:0:0"),
      abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }], duration: { duration: "end-of-turn" } };
    assert.deepEqual(effects("Target creature gets +1/+1 and gains flying until end of turn."), [
      { ...common, duration: { duration: "end-of-turn" } }, grant,
    ]);
    assert.deepEqual(effects("Target creature gets +1/+1 and target creature gains flying until end of turn."), [
      common, { ...grant, what: creature("line:0:9") },
    ]);
    assert.deepEqual(effects("Destroy target creature. Destroy target creature."), [
      { effect: "destroy", what: creature("line:0:1") }, { effect: "destroy", what: creature("line:0:5") },
    ]);
  });

  it("preserves object and targeted-controller identities in the same selector", () => {
    assert.deepEqual(effects("Destroy target creature target opponent controls."), [{
      effect: "destroy", what: { ref: "target", count: one, selectionId: "line:0:1",
        filter: { types: ["creature"], controlledBy: { player: "target-opponent", selectionId: "line:0:3" } } },
    }]);
    for (const [text, player] of [["target player", "target-player"], ["target opponent", "target-opponent"]]) {
      const c = new Cursor(lex(text, "example"));
      assert.deepEqual(parsePlayerRef(c), { player, selectionId: "example:0" });
      assert.equal(c.done(), true);
    }
    assert.deepEqual(effects("Target player draws a card."), [{
      effect: "draw", who: { player: "target-player", selectionId: "line:0:0" }, amount: one,
    }]);
  });

  it("keeps the identity through the compact any-target damage representation", () => {
    assert.deepEqual(effects("This creature deals 2 damage to any target."), [{
      effect: "damage", source: { ref: "this", noun: "creature" }, amount: { amount: "fixed", value: 2 },
      to: { to: "any-target", selectionId: "line:0:7" },
    }]);
  });

  it("preserves possessive target owners in zones, zone-card refs, and step restrictions", () => {
    const c = new Cursor(lex("top of target opponent's library", "example"));
    assert.deepEqual(parseZoneRef(c), { zone: "library", owner: { player: "target-opponent", selectionId: "example:2" }, position: "top" });
    assert.equal(c.done(), true);
    assert.deepEqual(effects("Look at the top card of target player's library."), [{
      effect: "look-at", who: { player: "you" }, what: { top: one, of: { zone: "library", owner: { player: "target-player", selectionId: "line:0:6" } } },
    }]);
    assert.deepEqual(effects("Exile target player's graveyard."), [{
      effect: "exile", what: { ref: "zone-cards", zone: "graveyard", owner: { player: "target-player", selectionId: "line:0:1" } },
    }]);
    assert.deepEqual(effects("Target creature doesn't untap during target opponent's next untap step."), [{
      effect: "untap-step-suppression", what: creature("line:0:0"), step: { occurrence: "next", owner: { player: "target-opponent", selectionId: "line:0:5" } },
    }]);
  });

  it("retains original normalized line indices across modal bullet grouping", () => {
    assert.deepEqual(abilities("Choose one —\n• Destroy target creature.\n• Exile target creature.\nDraw a card.\nDestroy target creature."), [
      { kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "modal", count: { exactly: 1 }, options: [
        { effects: [{ steps: [{ effect: "destroy", what: creature("line:1:2") }] }] },
        { effects: [{ steps: [{ effect: "exile", what: creature("line:2:2") }] }] },
      ] }] }] },
      { kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: one }] }] },
      { kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "destroy", what: creature("line:4:1") }] }] },
    ]);
  });

  it("retains target token indices through ability-word and activation prefix slices", () => {
    assert.deepEqual(abilities("Landfall — {T}: Destroy target creature."), [{
      kind: "activated", abilityWord: "Landfall", costs: [{ cost: "tap-self" }], restriction: undefined,
      effects: [{ steps: [{ effect: "destroy", what: creature("line:0:5") }] }],
    }]);
    assert.deepEqual(abilities("-2: Destroy target creature."), [{
      kind: "loyalty", cost: { sign: -1, amount: 2 },
      effects: [{ steps: [{ effect: "destroy", what: creature("line:0:4") }] }],
    }]);
  });

  it("keeps nested quoted-ability targets separate without restarting token numbering", () => {
    assert.deepEqual(effects('Target creature gains "{T}: Destroy target creature." until end of turn.'), [{
      effect: "gain-abilities", what: creature("line:0:0"), duration: { duration: "end-of-turn" },
      abilities: [{ gained: "quoted", ability: { kind: "activated", abilityWord: undefined, restriction: undefined,
        costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "destroy", what: creature("line:0:7") }] }],
      } }],
    }]);
  });
});
