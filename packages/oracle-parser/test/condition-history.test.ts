import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Condition } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Actual clauses checked in cached Scryfall corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 400.7d, 505.1, 508.1/508.4, 509.1, 603.4, 608.2i, 700.4:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;
const itself = { ref: "it" } as const;
const creature = { ref: "this", noun: "creature" } as const;
const spell = { ref: "this", noun: "spell" } as const;
const atLeast = (value: number) => ({ op: "ge" as const, value: fixed(value) });

function condition(text: string): Condition {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.ok(parsed, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return parsed;
}
function abilities(text: string, name = "History fixture"): Ability[] {
  const parsed = parseOracleText(text, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities));
}

describe("historical casting conditions", () => {
  it("retains a specific originating spell, actor, and casting origin", () => {
    // Marina Vendrell's Grimoire, Wedding Ring, Patched Plaything.
    assert.deepEqual(condition("you cast it"), { condition: "was-cast", what: itself, by: you });
    assert.deepEqual(condition("it was cast"), { condition: "was-cast", what: itself });
    assert.deepEqual(condition("you cast it from your hand"), {
      condition: "was-cast", what: itself, by: you, from: { zone: "hand", owner: "your" },
    });
    assert.deepEqual(condition("this spell was cast from a graveyard"), {
      condition: "was-cast", what: spell, from: { zone: "graveyard", owner: "any" },
    });
    assert.deepEqual(condition("it wasn't cast by you"), { condition: "not", of: {
      condition: "was-cast", what: itself, by: you,
    } });
  });

  it("tests the casting phase rather than the current phase (Sphinx's Insight)", () => {
    const castMain: Condition = { condition: "was-cast", what: spell, by: you, when: { phase: "main", whose: you } };
    assert.deepEqual(abilities("Addendum — If you cast this spell during your main phase, you gain 2 life."), [{
      kind: "spell", abilityWord: "Addendum", effects: [{ condition: castMain, conditionKind: "if",
        steps: [{ effect: "gain-life", who: you, amount: fixed(2) }],
      }],
    }]);
    assert.deepEqual(condition("this spell was cast during your precombat main phase"), {
      condition: "was-cast", what: spell, when: { phase: "precombat-main", whose: you },
    });
    assert.deepEqual(condition("you cast this spell during your second main phase"), {
      condition: "was-cast", what: spell, by: you, when: { phase: "second-main", whose: you },
    });
    assert.deepEqual(condition("you cast this spell during your postcombat main phase"), {
      condition: "was-cast", what: spell, by: you, when: { phase: "postcombat-main", whose: you },
    });
  });

  it("keeps the intervening originating-spell check on the trigger", () => {
    assert.deepEqual(abilities("When Marina Vendrell's Grimoire enters, if you cast it, draw five cards.", "Marina Vendrell's Grimoire"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "self" } },
      condition: { condition: "was-cast", what: itself, by: you },
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(5) }] }],
    }]);
  });

  it("does not reinterpret an optional casting action as a historical predicate", () => {
    // Animate Spell, fd2861c3-bab3-452c-8e71-87475339a850 in the cached corpus.
    // CR 118.12a: the action after 'unless' is offered now, not a past event.
    assert.equal(parseOracleText(
      "When Animate Spell leaves the battlefield, enchanted card's owner sacrifices it unless they cast it.",
      "Animate Spell",
    ).ok, false);
    assert.equal(parseOracleText("Sacrifice this creature unless you cast it.").ok, false);
    for (const join of ["and", "or"]) for (const [first, second] of [
      ["you cast it", "you control a Forest"], ["you control a Forest", "you cast it"],
    ]) assert.equal(parseOracleText(`Sacrifice this creature unless ${first} ${join} ${second}.`).ok, false);

    // Explicit historical tense remains a condition, with the same source identity.
    for (const [clause, expected] of [
      ["you have cast it", { condition: "was-cast", what: itself, by: you }],
      ["it was cast", { condition: "was-cast", what: itself }],
      ["you cast a spell this turn", { condition: "cast-count", who: you,
        what: { cls: "spell" }, comparison: atLeast(1), during: "this-turn" }],
    ] as const) assert.deepEqual(abilities(`Sacrifice this creature unless ${clause}.`), [{
      kind: "spell", effects: [{ steps: [{ effect: "sacrifice", who: you, what: creature }],
        condition: expected, conditionKind: "unless" }],
    }]);
  });
});

describe("event headers retain specific source and phase-owner identities", () => {
  it("retains the enchanted player in a Curse's upkeep trigger", () => {
    assert.deepEqual(abilities("At the beginning of enchanted player's upkeep, that player mills two cards."), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: { player: "enchanted-player" } },
      effects: [{ steps: [{ effect: "mill", who: { player: "that-player" }, amount: fixed(2) }] }],
    }]);
  });

  it("retains an attached object's controller before or after the phase", () => {
    const expected = [{ kind: "triggered", trigger: { trigger: "phase", phase: "upkeep",
      whose: { player: "controller", of: { ref: "enchanted", noun: "creature" } } },
      effects: [{ steps: [{ effect: "put-counters", counter: { type: "-0/-1" }, count: fixed(1),
        on: { ref: "that", filter: { types: ["creature"] } } }] }],
    }];
    // Takklemaggot; the equivalent possessive order preserves the same owner.
    assert.deepEqual(abilities("At the beginning of the upkeep of enchanted creature's controller, put a -0/-1 counter on that creature."), expected);
    assert.deepEqual(abilities("At the beginning of enchanted creature's controller's upkeep, put a -0/-1 counter on that creature."), expected);
  });

  it("keeps source-spell cast origins distinct from broad spell categories", () => {
    assert.deepEqual(abilities("When you cast this spell from your hand, draw a card."), [{
      kind: "triggered", trigger: { trigger: "cast", who: you, what: spell, from: { zone: "hand", owner: "your" } },
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) }] }],
    }]);
    for (const text of [
      "When you cast or copy this spell, draw a card.", "When you copy this spell from your hand, draw a card.",
      "At the beginning of enchanted player's unknown phase, draw a card.",
      "At the beginning of the upkeep of enchanted creatures' controllers, draw a card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("declared combat history", () => {
  it("preserves raid and historical attackers' characteristics", () => {
    assert.deepEqual(condition("you attacked this turn"), { condition: "attacked", who: you, during: "this-turn" });
    assert.deepEqual(condition("you haven't attacked this turn"), { condition: "not", of: {
      condition: "attacked", who: you, during: "this-turn",
    } });
    assert.deepEqual(condition("you attacked with a Hero this turn"), {
      condition: "attacked", who: you, during: "this-turn", with: { subtypes: ["Hero"] },
    });
    // Storm Fleet Spy: raid is an intervening condition, not trigger grouping.
    assert.deepEqual(abilities("Raid — When this creature enters, if you attacked this turn, draw a card."), [{
      kind: "triggered", abilityWord: "Raid", trigger: { trigger: "enters", what: creature },
      condition: { condition: "attacked", who: you, during: "this-turn" },
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) }] }],
    }]);
  });

  it("retains action alternatives and turn versus combat scope (Clockwork Avian)", () => {
    const history: Condition = { condition: "any-of", conditions: [
      { condition: "combat-action-history", what: creature, action: "attacked", during: "this-combat" },
      { condition: "combat-action-history", what: creature, action: "blocked", during: "this-combat" },
    ] };
    assert.deepEqual(abilities("At end of combat, if this creature attacked or blocked this combat, remove a +1/+0 counter from it."), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "end-of-combat", whose: "each-player" }, condition: history,
      effects: [{ steps: [{ effect: "remove-counters", counter: { type: "+1/+0" }, count: fixed(1), from: itself }] }],
    }]);
    assert.deepEqual(condition("it attacked and blocked this turn"), { condition: "all-of", conditions: [
      { condition: "combat-action-history", what: itself, action: "attacked", during: "this-turn" },
      { condition: "combat-action-history", what: itself, action: "blocked", during: "this-turn" },
    ] });
    assert.notDeepEqual(condition("it is attacking"), condition("it attacked this turn"));
  });
});

describe("zone-change history", () => {
  it("represents morbid as battlefield to graveyard, not any graveyard move", () => {
    const died: Condition = { condition: "zone-change-history", event: "dies",
      subject: { match: "filter", filter: { types: ["creature"] }, comparison: atLeast(1) },
      from: { zone: "battlefield" }, to: { zone: "graveyard" }, during: "this-turn",
    };
    assert.deepEqual(condition("a creature died this turn"), died);
    // Malicious Affliction's cast trigger identifies the actual source spell.
    assert.deepEqual(abilities("When you cast this spell, if a creature died this turn, draw a card."), [{
      kind: "triggered", trigger: { trigger: "cast", who: you, what: spell }, condition: died,
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) }] }],
    }]);
    assert.deepEqual(condition("no creatures died this turn"), { ...died,
      subject: { match: "filter", filter: { types: ["creature"] }, comparison: { op: "eq", value: fixed(0) } },
    });
  });

  it("retains control at the departure event (Airdrop Aeronauts)", () => {
    const revolt: Condition = { condition: "zone-change-history", event: "leaves-battlefield",
      subject: { match: "filter", filter: { cls: "permanent" }, comparison: atLeast(1) },
      from: { zone: "battlefield" }, controllerAtMove: you, during: "this-turn",
    };
    assert.deepEqual(abilities("Revolt — When this creature enters, if a permanent left the battlefield under your control this turn, you gain 5 life."), [{
      kind: "triggered", abilityWord: "Revolt", trigger: { trigger: "enters", what: creature }, condition: revolt,
      effects: [{ steps: [{ effect: "gain-life", who: you, amount: fixed(5) }] }],
    }]);
  });

  it("preserves entry counts and a specifically referenced entering object", () => {
    // Malcator, Purity Overseer and Samut, Vizier of Naktamun.
    assert.deepEqual(condition("three or more artifacts entered the battlefield under your control this turn"), {
      condition: "zone-change-history", event: "enters",
      subject: { match: "filter", filter: { types: ["artifact"] }, comparison: atLeast(3) },
      to: { zone: "battlefield" }, controllerAtMove: you, during: "this-turn",
    });
    assert.deepEqual(condition("that creature entered this turn"), {
      condition: "zone-change-history", event: "enters",
      subject: { match: "object", what: { ref: "that", filter: { types: ["creature"] } } },
      to: { zone: "battlefield" }, during: "this-turn",
    });
  });

  it("distinguishes any origin and destination ownership (Macabre Reconstruction)", () => {
    assert.deepEqual(condition("a creature card was put into your graveyard from anywhere this turn"), {
      condition: "zone-change-history", event: "put-into-zone",
      subject: { match: "filter", filter: { types: ["creature"], cls: "card" }, comparison: atLeast(1) },
      from: "anywhere", to: { zone: "graveyard", owner: "your" }, during: "this-turn",
    });
    assert.deepEqual(condition("exactly two creatures died under your control this turn"), {
      condition: "zone-change-history", event: "dies",
      subject: { match: "filter", filter: { types: ["creature"] }, comparison: { op: "eq", value: fixed(2) } },
      from: { zone: "battlefield" }, to: { zone: "graveyard" }, controllerAtMove: you, during: "this-turn",
    });
  });

  it("keeps complete historical alternatives separate (Avengers Assemble!)", () => {
    assert.deepEqual(condition("you attacked with a Hero this turn or a Hero entered the battlefield under your control this turn"), {
      condition: "any-of", conditions: [
        { condition: "attacked", who: you, with: { subtypes: ["Hero"] }, during: "this-turn" },
        { condition: "zone-change-history", event: "enters",
          subject: { match: "filter", filter: { subtypes: ["Hero"] }, comparison: atLeast(1) },
          to: { zone: "battlefield" }, controllerAtMove: you, during: "this-turn" },
      ],
    });
  });
});

describe("historical limits and binding honesty", () => {
  it("rejects unsupported quantities, lookback windows, and paid-cost methods", () => {
    for (const text of [
      "you attacked with two or more creatures this turn", "you attacked with creatures with total power 6 or greater this combat",
      "it attacked during its controller's last turn", "it attacked or blocked since your last upkeep",
      "it blocked or was blocked by a blue creature this turn", "it didn't attack and block this turn",
      "a creature died", "a creature died last turn", "it entered the graveyard this turn",
      "this spell was cast using teamwork", "this spell was cast during an unknown phase",
      "you cast a spell from your hand and from your graveyard this turn",
      "you cast a spell from your hand and in your graveyard this turn",
      "you cast a spell in your hand this turn",
      "you cast it from your hand under an opponent's control", "a permanent left the battlefield under your influence this turn",
    ]) assert.equal(parseOracleText(`If ${text}, draw a card.`).ok, false, text);
    for (const text of ["you cast it from", "this spell was cast during your", "a creature was put into"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseCondition(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("does not certify unimplemented historical identity and lookback bindings", () => {
    for (const text of [
      "When this creature enters, if you attacked this turn, draw a card.",
      "When this creature enters, if you cast it, draw a card.",
      "At end of combat, if this creature attacked this combat, draw a card.",
      "When this creature enters, if a creature died this turn, draw a card.",
    ]) {
      const parsed = parseOracleText(text, "History fixture", { includeSourceProvenance: true });
      assert.equal(parsed.ok, true, text);
      const graph = analyzeCardBindings({ identity: "history", source: { name: "History fixture", layout: "normal" },
        faces: [{ sourceFaceIndex: 0, result: parsed }] });
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"), text);
    }
  });
});
