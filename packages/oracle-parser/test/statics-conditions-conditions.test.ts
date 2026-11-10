import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Condition } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Conditions. Clauses are taken from the
// cached Oracle corpus; card names are noted per test.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;
const atLeast = (value: number) => ({ op: "ge" as const, value: fixed(value) });

function condition(text: string): Condition {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.ok(parsed, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return JSON.parse(JSON.stringify(parsed));
}
/** No complete condition: either no parse or unconsumed words remain. */
function rejected(text: string): void {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.equal(parsed !== null && c.done(), false, text);
}
function abilities(text: string, typeLine: string, name = "W2 fixture"): Ability[] {
  const parsed = parseOracleText(text, name, { typeLine });
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function fullyBound(text: string, typeLine: string): boolean {
  const result = parseOracleText(text, "Test", { typeLine, includeSourceProvenance: true });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return analyzeCardBindings({ identity: "test", source: { name: "Test", layout: "normal", type_line: typeLine },
    faces: [{ sourceFaceIndex: 0, result }] }).fullyBound;
}

describe("number of opponents (CR 102.2-102.3)", () => {
  it("counts players, not their permanents (Spire Garden, Luxury Suite)", () => {
    assert.deepEqual(condition("you have two or more opponents"), { condition: "opponent-count", who: you, comparison: atLeast(2) });
    assert.deepEqual(abilities("This land enters tapped unless you have two or more opponents.", "Land"), [{
      kind: "static", effect: { static: "enters-tapped", what: { ref: "this", noun: "land" },
        condition: { condition: "opponent-count", who: you, comparison: atLeast(2) }, conditionKind: "unless" },
    }]);
    assert.deepEqual(condition("that player has exactly one opponent"), {
      condition: "opponent-count", who: { player: "that-player" }, comparison: { op: "eq", value: fixed(1) },
    });
  });

  it("rejects a bare number and other things a player can have", () => {
    rejected("you have two opponents");
    rejected("you have two or more opponents' creatures");
    rejected("you have two or more teammates");
  });
});

describe("condition nodes stay visible to reference analysis", () => {
  it("never reports history, designation or player-count conditions as fully bound", () => {
    for (const clause of ["you're the monarch", "it's day", "you've completed a dungeon", "you have a full party",
      "you have the city's blessing", "you've drawn two or more cards this turn", "you gained life this turn",
      "you've committed a crime this turn", "you control more creatures than each other player",
      "you have two or more opponents", "this is the second time this ability has resolved this turn",
      "this ability has been activated four or more times this turn"])
      assert.equal(fullyBound(`At the beginning of your upkeep, if ${clause}, you gain 1 life.`, "Enchantment"), false, clause);
  });

  it("leaves plain state queries of the controller unaffected", () => {
    assert.equal(fullyBound("At the beginning of your upkeep, if you have ten or more life, you gain 1 life.", "Enchantment"), true);
  });
});

describe("the previous turn as a lookback window (CR 608.2i)", () => {
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "each-player" };
  it("looks back at life changes of the stated player (First Response, Feast on the Fallen)", () => {
    assert.deepEqual(condition("you lost life last turn"), {
      condition: "life-change", who: you, change: "lost", comparison: { op: "gt", value: fixed(0) }, during: "last-turn",
    });
    assert.deepEqual(condition("an opponent lost life last turn"), {
      condition: "life-change", who: { player: "an-opponent" }, change: "lost", comparison: { op: "gt", value: fixed(0) }, during: "last-turn",
    });
    assert.deepEqual(abilities("At the beginning of each upkeep, if you lost life last turn, create a 1/1 white Soldier creature token.", "Enchantment")[0], {
      kind: "triggered", trigger: upkeep,
      condition: { condition: "life-change", who: you, change: "lost", comparison: { op: "gt", value: fixed(0) }, during: "last-turn" },
      effects: [{ steps: [{ effect: "create-token", count: fixed(1), token: {
        power: fixed(1), toughness: fixed(1), colors: ["white"], supertypes: [], types: ["creature"], subtypes: ["Soldier"], keywords: [],
      } }] }],
    });
  });

  it("looks back at entries and damage of the previous turn (Ephara, Command the Stage)", () => {
    assert.deepEqual(condition("you had another creature enter the battlefield under your control last turn"), {
      condition: "zone-change-history", event: "enters",
      subject: { match: "filter", filter: { types: ["creature"], other: true }, comparison: atLeast(1) },
      to: { zone: "battlefield" }, controllerAtMove: you, during: "last-turn",
    });
    assert.deepEqual(condition("an opponent was dealt noncombat damage last turn"), {
      condition: "damage-history", comparison: { op: "gt", value: fixed(0) }, quantity: "positive-event", damage: "noncombat",
      to: { player: "an-opponent" }, during: "last-turn",
    });
  });

  it("does not read a player's own last turn, or a since-window, as the previous turn", () => {
    rejected("you lost life during your last turn");
    rejected("it was dealt damage since your last turn");
    // actions-b: now represented for declared combat actions (formerly rejected).
    assert.deepEqual(condition("it attacked during its controller's last turn"), {
      condition: "combat-action-history", what: { ref: "it" }, action: "attacked",
      during: { lastTurnOf: { player: "controller", of: { ref: "it" } } },
    });
    rejected("you lost life last");
    rejected("a creature died last combat");
  });
});

describe("the zone a permanent entered from (CR 400.7, 603.6a)", () => {
  const yourGraveyard = { zone: "graveyard", owner: "your" };
  it("keeps the entry origin apart from where its spell was cast (Archfiend's Vessel, Prized Amalgam)", () => {
    assert.deepEqual(condition("it entered from your graveyard"), { condition: "entered-from", what: { ref: "it" }, from: yourGraveyard });
    assert.deepEqual(condition("it entered from your graveyard or you cast it from your graveyard"), {
      condition: "any-of", conditions: [
        { condition: "entered-from", what: { ref: "it" }, from: yourGraveyard },
        { condition: "was-cast", what: { ref: "it" }, by: you, from: yourGraveyard },
      ],
    });
    // Breathless Knight: any player's graveyard, and the entering creature by description.
    assert.deepEqual(condition("that creature entered from a graveyard or you cast it from a graveyard"), {
      condition: "any-of", conditions: [
        { condition: "entered-from", what: { ref: "that", filter: { types: ["creature"] } }, from: { zone: "graveyard", owner: "any" } },
        { condition: "was-cast", what: { ref: "it" }, by: you, from: { zone: "graveyard", owner: "any" } },
      ],
    });
    assert.deepEqual(condition("it entered from exile"), { condition: "entered-from", what: { ref: "it" }, from: { zone: "exile" } });
  });

  it("rejects unidentified or quantified subjects, dated entries and missing owners", () => {
    rejected("a creature entered from your graveyard");
    rejected("one or more of them entered from a graveyard");
    rejected("they entered from a graveyard");
    // A dated entry is the turn-history node, not the origin of this object.
    assert.equal(condition("it entered from your graveyard this turn").condition, "zone-change-history");
    rejected("it entered from graveyard");
    rejected("it entered from the battlefield");
    rejected("it entered from");
  });

  it("does not report an entry origin as fully bound", () => {
    assert.equal(fullyBound("When this creature enters, if it entered from your graveyard, you gain 1 life.", "Creature — Zombie"), false);
  });
});

describe("counters a player put this turn (CR 122.6)", () => {
  const history = (counter: object | "any", on: object, during = "this-turn") => ({
    condition: "player-action-history", who: you, action: { action: "put-counters", counter, on }, comparison: atLeast(1), during,
  });
  it("keeps the counter kind and the recipient (Lasting Tarfire, Fractal Tender, Sigardian Paladin, Beast)", () => {
    assert.deepEqual(condition("you put a counter on a creature this turn"),
      history("any", { match: "filter", filter: { types: ["creature"] } }));
    assert.deepEqual(condition("you put a counter on this creature this turn"),
      history("any", { match: "object", what: { ref: "this", noun: "creature" } }));
    assert.deepEqual(condition("you've put one or more +1/+1 counters on a creature this turn"),
      history({ type: "+1/+1" }, { match: "filter", filter: { types: ["creature"] } }));
    assert.deepEqual(condition("you've put one or more +1/+1 counters on ~ this turn"),
      history({ type: "+1/+1" }, { match: "object", what: { ref: "self" } }));
    assert.deepEqual(condition("you didn't put a counter on a creature this turn"),
      { condition: "not", of: history("any", { match: "filter", filter: { types: ["creature"] } }) });
  });

  it("rejects larger numbers, undated or passive forms and non-counter objects", () => {
    rejected("you put two counters on a creature this turn");
    rejected("you've put three or more +1/+1 counters on a creature this turn");
    rejected("you put a counter on a creature");
    rejected("you put a counter on each creature this turn");
    rejected("you put a counter on target creature this turn");
    rejected("a +1/+1 counter was put on a permanent under your control this turn");
  });
});

describe("a source that hasn't dealt damage yet (CR 400.7)", () => {
  it("looks at the whole existence of that object (Palladia-Mors, Karakyk Guardian)", () => {
    assert.deepEqual(condition("it hasn't dealt damage yet"), { condition: "not", of: {
      condition: "damage-history", comparison: { op: "gt", value: fixed(0) }, quantity: "positive-event",
      source: { ref: "it" }, during: "object-lifetime",
    } });
    assert.deepEqual(abilities("This creature has hexproof as long as it hasn't dealt damage yet.", "Creature — Dragon"), [{
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "hexproof" } }] }],
        condition: { condition: "not", of: { condition: "damage-history", comparison: { op: "gt", value: fixed(0) },
          quantity: "positive-event", source: { ref: "it" }, during: "object-lifetime" } } },
    }]);
  });

  it("rejects the same words without a complete, negative, unqualified question", () => {
    rejected("it hasn't dealt damage");
    rejected("it dealt damage yet");
    rejected("it hasn't dealt combat damage yet");
    rejected("it hasn't dealt damage to a player yet");
    rejected("it hasn't dealt damage yet this turn");
    rejected("it hasn't been dealt damage yet");
  });
});
