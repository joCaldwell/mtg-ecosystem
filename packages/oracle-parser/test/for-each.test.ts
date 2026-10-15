import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Effect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseForEach } from "../src/parser/for-each.ts";
import { parseSentences } from "../src/parser/effects.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Actual Scryfall wording: Dismantling Wave, Seasoned Pyromancer,
// Hollowhenge Overlord, Graveyard Glutton, Rhys the Redeemed, Fighting Chance.
// Action ordering: CR608.2e/f,101.4. Target declaration: CR601.2c,115.3.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const two = { amount: "fixed", value: 2 } as const;
const you = { player: "you" } as const;
const perInstruction = { effect: "for-each", processing: "instruction-wise" } as const;
function direct(text: string): Effect {
  const c = new Cursor(lex(text, "line:0"));
  const result = parseForEach(c, body => parseSentences(body, { modalOptions: [] }));
  assert.ok(result, c.errorMessage());
  assert.equal(c.punct("."), true);
  assert.equal(c.done(), true);
  return result;
}
function parsed(text: string, name = "Fixture", typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}

describe("explicit per-member instruction scopes", () => {
  it("preserves per-opponent targets under the original lexical declaration", () => {
    const effect = { ...perInstruction, each: { player: "each-opponent" }, effects: [{ steps: [{
      effect: "destroy", what: { ref: "target", count: one, upTo: true, selectionId: "line:0:8",
        filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["enchantment"] }] }], control: "that-player" } },
    }] }] };
    const text = "For each opponent, destroy up to one target artifact or enchantment that player controls.";
    assert.deepEqual(direct(text), effect);
    assert.deepEqual(parsed(text, "Dismantling Wave").abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [effect] }] }]);
  });
  it("processes each coordinated action across players instead of completing all actions player by player", () => {
    assert.deepEqual(direct("For each player, that player draws a card and discards a card."), {
      ...perInstruction, each: { player: "each-player" }, effects: [{ steps: [
        { effect: "draw", who: { player: "that-player" }, amount: one },
        { effect: "discard", who: { player: "that-player" }, what: { ref: "filter", filter: { cls: "card" }, count: one }, random: undefined },
      ] }],
    });
  });
  it("retains object membership and branch-local type alternatives", () => {
    assert.deepEqual(direct("For each creature you control that's a Wolf or a Werewolf, create a 2/2 green Wolf creature token."), {
      ...perInstruction, each: { ref: "each", filter: { types: ["creature"], control: "you", anyOf: [{ subtypes: ["Wolf"] }, { subtypes: ["Werewolf"] }] } },
      effects: [{ steps: [{ effect: "create-token", count: one, token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Wolf"], keywords: [], power: two, toughness: two } }] }],
    });
  });
  it("keeps the actual event-result domain rather than counting arbitrary current cards", () => {
    const text = "When this creature enters, discard two cards, then draw two cards. For each nonland card discarded this way, create a 1/1 red Elemental creature token.";
    assert.deepEqual(parsed(text, "Seasoned Pyromancer", "Creature — Human Shaman").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } }, effects: [
        { steps: [{ effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: two }, random: undefined }, { effect: "draw", who: you, amount: two }] },
        { steps: [{ ...perInstruction, each: { ref: "each", filter: { nonTypes: ["land"], cls: "card", relation: { relation: "event-result", event: "discard", scope: "this-way" } } }, effects: [{ steps: [{ effect: "create-token", count: one, token: { colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Elemental"], keywords: [], power: one, toughness: one } }] }] }] },
      ],
    }]);
  });
  it("preserves multiple actors, per-member optionality and filtered player domains", () => {
    assert.deepEqual(direct("For each creature card exiled this way, each opponent loses 1 life and you gain 1 life."), {
      ...perInstruction, each: { ref: "each", filter: { types: ["creature"], cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" } } }, effects: [{ steps: [
        { effect: "lose-life", who: { player: "each-opponent" }, amount: one }, { effect: "gain-life", who: you, amount: one },
      ] }],
    });
    assert.deepEqual(direct("For each creature token you control, you may create a token that's a copy of that creature."), {
      ...perInstruction, each: { ref: "each", filter: { types: ["creature"], cls: "token", control: "you" } }, effects: [{ steps: [{ effect: "create-token", count: one, token: { copyOf: { ref: "that", filter: { types: ["creature"] } } }, optional: true }] }],
    });
    assert.deepEqual(direct("For each opponent with three or more poison counters, you draw a card."), {
      ...perInstruction, each: { player: "filtered", base: { player: "each-opponent" }, filter: { counterConstraints: [{ counter: { type: "poison" }, comparison: { op: "ge", value: { amount: "fixed", value: 3 } } }] } },
      effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
    });
  });
  it("retains internal durations and separately parses an outer activation restriction", () => {
    const text = "{T}: For each creature you control, target creature gets +1/+1 until end of turn. Activate only as a sorcery.";
    assert.deepEqual(parsed(text, "Fixture", "Artifact").abilities, [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "tap-self" }], restriction: { restriction: "sorcery" }, effects: [{ steps: [{
        ...perInstruction, each: { ref: "each", filter: { types: ["creature"], control: "you" } }, effects: [{ steps: [{
          effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: one, selectionId: "line:0:8" },
          power: { sign: 1, amount: one }, toughness: { sign: 1, amount: one }, duration: { duration: "end-of-turn" },
        }] }],
      }] }],
    }]);
  });
  it("requires complete terminal bodies and rejects unrepresented member continuations or domains", () => {
    for (const text of ["For each creature, invent a mechanic.", "For each creature, draw a card. It gains flying until end of turn.",
      "For each blocking creature, flip a coin. If you win the flip, prevent all combat damage that would be dealt by that creature this turn.",
      "For each player, for each creature, draw a card.",
      "For each color, draw a card.", "For each kind of counter on this creature, draw a card.",
      "For each card type, draw a card.", "For each 1 damage prevented this way, draw a card.", "For each {B} in a cost, you may pay 2 life rather than pay that mana.",
      "For each player draw a card.", "For each target creature, draw a card.", "For each creature, draw a card. Activate never."])
      assert.equal(parseOracleText(text).ok, false, text);
    // Players who could not take the preceding action are now a represented
    // member domain (CR 118.12): PlayerFilter.precedingAction.
    assert.deepEqual(parsed("For each opponent who can't, draw a card.").abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
      ...perInstruction, each: { player: "filtered", base: { player: "each-opponent" }, filter: { precedingAction: "unable" } },
      effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
    }] }] }]);
    // A later sentence that cannot refer to the loop is a separate
    // instruction after it, so this former rejection is now structural.
    assert.deepEqual(parsed("For each opponent, draw a card. Draw a card.").abilities, [{ kind: "spell", abilityWord: undefined, effects: [
      { steps: [{ ...perInstruction, each: { player: "each-opponent" }, effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }] }] },
      { steps: [{ effect: "draw", who: you, amount: one }] },
    ] }]);
    // A plural reference to what every member
    // produced, when each member produces one object, is said once after
    // the loop; it is now a sibling sentence outside the loop.
    assert.deepEqual(parsed("For each artifact you control, create a token that's a copy of it. Those tokens gain haste.").abilities, [{ kind: "spell", abilityWord: undefined, effects: [
      { steps: [{ ...perInstruction, each: { ref: "each", filter: { types: ["artifact"], control: "you" } },
        effects: [{ steps: [{ effect: "create-token", count: one, token: { copyOf: { ref: "it" } } }] }] }] },
      { steps: [{ effect: "gain-abilities", what: { ref: "that", filter: { cls: "token" }, plural: true }, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] },
    ] }]);
    const c = new Cursor(lex("For each creature, unsupported."));
    assert.equal(parseForEach(c, body => parseSentences(body, { modalOptions: [] })), null);
    assert.equal(c.pos, 0);
  });
  it("does not inherit an outer actor or unique-stack-event proof into the body", () => {
    assert.equal(parseOracleText("That player draws a card, then for each creature, draws a card.").ok, false);
    assert.equal(parseOracleText("Whenever you cast an instant spell, for each opponent, copy it.").ok, false);
    assert.deepEqual(parsed("That player draws a card, then for each creature, draw a card.").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "draw", who: { player: "that-player" }, amount: one }, {
        ...perInstruction, each: { ref: "each", filter: { types: ["creature"] } }, effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
      }] }],
    }]);
  });
  it("marks member references, target multiplicity and output aggregation as unresolved", () => {
    const result = parsed("For each opponent, destroy up to one target creature that player controls.");
    const graph = analyzeCardBindings({ identity: "loop", source: { name: "Fixture", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope" && d.message.includes("membership timing")));
    assert.ok(graph.references.filter(r => JSON.stringify(r.expression).includes('"that-player"')).every(r => r.binding === "unresolved"));
  });
});
