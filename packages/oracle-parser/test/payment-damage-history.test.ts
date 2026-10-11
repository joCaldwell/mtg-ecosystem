import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Condition, Trigger } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Card clauses checked in cached Scryfall corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 106.3, 107.4h, 109.2, 120.3–4/120.8, 508.3c–e, 603.6c,
// 608.2i, 609.4b: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const comparison = (op: "eq" | "ge" | "gt", value: number) => ({ op, value: fixed(value) });
const you = { player: "you" } as const;
const itself = { ref: "it" } as const;
const creature = { ref: "this", noun: "creature" } as const;
const spell = { ref: "this", noun: "spell" } as const;
function condition(text: string): Condition {
  const cursor = new Cursor(lex(text));
  const parsed = parseCondition(cursor);
  assert.ok(parsed, `${text}: ${cursor.errorMessage()}`);
  assert.equal(cursor.done(), true, text);
  return parsed;
}
function abilities(text: string): Ability[] {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function withDraw(trigger: Trigger): Ability[] {
  return [{ kind: "triggered", trigger, effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) }] }] }];
}

describe("actual damage history", () => {
  it("distinguishes one positive event from a summed damage threshold", () => {
    // Lightning Phoenix and Spinerock Knoll use accumulated damage to one opponent.
    assert.deepEqual(condition("an opponent was dealt 7 or more damage this turn"), {
      condition: "damage-history", to: { player: "an-opponent" }, comparison: comparison("ge", 7),
      quantity: "total", during: "this-turn",
    });
    // Wall of Resistance: damage need not remain marked, nor cause life loss.
    assert.deepEqual(condition("this creature was dealt damage this turn"), {
      condition: "damage-history", to: creature, comparison: comparison("gt", 0),
      quantity: "positive-event", during: "this-turn",
    });
    assert.deepEqual(condition("you were dealt 4 or more damage this turn"), {
      condition: "damage-history", to: you, comparison: comparison("ge", 4), quantity: "total", during: "this-turn",
    });
  });

  it("retains active source, recipient, combat type, and negation", () => {
    // Vessel of the All-Consuming, Wave of Rats, Thirsting Axe.
    assert.deepEqual(condition("it has dealt 10 or more damage to that player this turn"), {
      condition: "damage-history", source: itself, to: { player: "that-player" },
      comparison: comparison("ge", 10), quantity: "total", during: "this-turn",
    });
    assert.deepEqual(condition("it dealt combat damage to a player this turn"), {
      condition: "damage-history", source: itself, to: { player: "a-player" }, damage: "combat",
      comparison: comparison("gt", 0), quantity: "positive-event", during: "this-turn",
    });
    assert.deepEqual(condition("equipped creature didn't deal combat damage to a creature this turn"), {
      condition: "not", of: { condition: "damage-history", source: { ref: "equipped", noun: "creature" },
        to: { ref: "filter", filter: { types: ["creature"] }, count: fixed(1) }, damage: "combat",
        comparison: comparison("gt", 0), quantity: "positive-event", during: "this-turn" },
    });
    assert.deepEqual(condition("it was dealt noncombat damage this combat"), {
      condition: "damage-history", to: itself, damage: "noncombat", comparison: comparison("gt", 0),
      quantity: "positive-event", during: "this-combat",
    });
  });

  it("retains source qualities in either historical phrase order", () => {
    // Lost Monarch of Ifnir: the source must have been a Zombie then.
    const expected: Condition = { condition: "damage-history", to: { player: "a-player" },
      source: { ref: "filter", filter: { subtypes: ["Zombie"] }, count: fixed(1) },
      damage: "combat", quantity: "positive-event", comparison: comparison("gt", 0), during: "this-turn" };
    assert.deepEqual(condition("a player was dealt combat damage by a Zombie this turn"), expected);
    assert.deepEqual(condition("a player was dealt combat damage this turn by a Zombie"), expected);
    assert.deepEqual(condition("4 or more damage was dealt to it this turn"), {
      condition: "damage-history", to: itself, quantity: "total", comparison: comparison("ge", 4), during: "this-turn",
    });
  });

  it("keeps a historical check as the trigger's intervening condition", () => {
    // Whirling Dervish's complete triggered ability.
    assert.deepEqual(abilities("At the beginning of each end step, if this creature dealt damage to an opponent this turn, put a +1/+1 counter on it."), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      condition: { condition: "damage-history", source: creature, to: { player: "an-opponent" },
        quantity: "positive-event", comparison: comparison("gt", 0), during: "this-turn" },
      effects: [{ steps: [{ effect: "put-counters", counter: { type: "+1/+1" }, count: fixed(1), on: itself }] }],
    }]);
  });

  it("rejects unrepresented periods, collective totals, and event-result scopes", () => {
    for (const clause of [
      "it was dealt damage", "it was dealt damage since your last turn", "it was dealt excess damage this turn",
      "each opponent was dealt 7 or more damage this turn", "all opponents were dealt combat damage this turn",
      "you were dealt damage by two or more creatures this turn",
      "red sources you controlled dealt 4 or more damage this turn", "it deals damage to you this way",
    ]) assert.equal(parseOracleText(`If ${clause}, draw a card.`).ok, false, clause);
  });
});

describe("mana actually spent to cast", () => {
  it("keeps colored, colorless, and total thresholds separate", () => {
    // Adamant (Rally for the Throne), Void Mirror, and Blazing Bomb.
    for (const [clause, mana, op, value] of [
      ["at least three white mana was spent to cast this spell", "white", "ge", 3],
      ["at least three colorless mana was spent to cast this spell", "colorless", "ge", 3],
      ["no colored mana was spent to cast this spell", "colored", "eq", 0],
      ["four or more mana was spent to cast this spell", "any", "ge", 4],
      ["no mana was spent to cast this spell", "any", "eq", 0],
    ] as const) assert.deepEqual(condition(clause), {
      condition: "mana-spent-to-cast", what: spell, mana, comparison: comparison(op, value),
    });
  });

  it("expands distinct type symbols into conjunctive minima and preserves multiplicity", () => {
    // Mythos of Brokkos: both colors matter, regardless of the mana cost paid.
    assert.deepEqual(condition("{U}{B} was spent to cast this spell"), {
      condition: "all-of", conditions: [
        { condition: "mana-spent-to-cast", what: spell, mana: "blue", comparison: comparison("ge", 1) },
        { condition: "mana-spent-to-cast", what: spell, mana: "black", comparison: comparison("ge", 1) },
      ],
    });
    assert.deepEqual(condition("{R}{R} was spent to cast it"), {
      condition: "mana-spent-to-cast", what: itself, mana: "red", comparison: comparison("ge", 2),
    });
    assert.deepEqual(condition("{S} was spent to cast it"), {
      condition: "mana-spent-to-cast", what: itself, mana: "snow", comparison: comparison("ge", 1),
    });
  });

  it("retains the mana source's historical subtype and complete conditional effects", () => {
    // Smaug, Wicked Worm; a Treasure need not still exist when the check occurs.
    assert.deepEqual(condition("mana from a Treasure was spent to cast it"), {
      condition: "mana-spent-to-cast", what: itself, mana: "any", source: { subtypes: ["Treasure"] },
      comparison: comparison("gt", 0),
    });
    assert.deepEqual(abilities("Adamant — If at least three black mana was spent to cast this spell, mill four cards."), [{
      kind: "spell", abilityWord: "Adamant", effects: [{
        condition: { condition: "mana-spent-to-cast", what: spell, mana: "black", comparison: comparison("ge", 3) },
        conditionKind: "if", steps: [{ effect: "mill", who: you, amount: fixed(4) }],
      }],
    }]);
  });

  it("rejects ambiguous payment allocations and unknown mana types", () => {
    for (const clause of [
      "{W/U} was spent to cast it", "{B/P} was spent to cast it", "{2} was spent to cast it",
      "{S}{U} was spent to cast it", "{S} of any of that spell's colors was spent to cast it",
      "at least three purple mana was spent to cast it", "mana from two or more Treasures was spent to cast it",
      "mana was spent to activate it", "mana would be spent to cast it",
    ]) assert.equal(parseOracleText(`If ${clause}, draw a card.`).ok, false, clause);
  });
});

describe("keyword predicates", () => {
  it("supports closed keyword names including multiword and parameterized families", () => {
    assert.deepEqual(condition("it has mutate"), { condition: "has-keyword", what: itself, keyword: "mutate" });
    assert.deepEqual(condition("that creature has first strike"), {
      condition: "has-keyword", what: { ref: "that", filter: { types: ["creature"] } }, keyword: "first strike",
    });
    assert.deepEqual(condition("it doesn't have suspend"), { condition: "not", of: {
      condition: "has-keyword", what: itself, keyword: "suspend",
    } });
    // Pollywog Symbiote.
    assert.deepEqual(abilities("Whenever you cast a creature spell, if it has mutate, draw a card, then discard a card."), [{
      kind: "triggered", trigger: { trigger: "cast", who: you, what: { types: ["creature"], cls: "spell" } },
      condition: { condition: "has-keyword", what: itself, keyword: "mutate" },
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(1) },
        { effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: fixed(1) } }] }],
    }]);
  });

  it("does not reinterpret keyword counters, statuses, parameters, or unknown names", () => {
    for (const clause of ["it has imaginarywalk", "it has modified", "it has ward {2}", "it had flying", "it has flying and unknownwalk"])
      assert.equal(parseOracleText(`If ${clause}, draw a card.`).ok, false, clause);
    assert.equal(condition("it has a flying counter on it").condition, "is-filter");
  });
});

describe("continuous conditional modifiers", () => {
  it("keeps Shadow the Hedgehog's mana condition local to each affected spell", () => {
    assert.deepEqual(abilities("Chaos Control — Each spell you cast has split second if mana from an artifact was spent to cast it."), [{
      kind: "static", abilityWord: "Chaos Control", effect: {
        static: "modify", what: { ref: "each", filter: { cls: "spell", cast: { by: you } } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "split second" } }] }],
        condition: { condition: "mana-spent-to-cast", what: itself, mana: "any", comparison: comparison("gt", 0),
          source: { types: ["artifact"] } },
        conditionScope: "each-affected-object",
      },
    }]);
  });

  it("preserves separately guarded coordinated modifiers and a shared final guard", () => {
    const flying = { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }] };
    const haste = { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] };
    const has = (keyword: string) => ({ condition: "has-keyword", what: itself, keyword });
    assert.deepEqual(abilities("Each creature has flying if it has trample and has haste if it has vigilance."), [{
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"] } },
        modifiers: [
          { ...flying, condition: has("trample"), conditionScope: "each-affected-object" },
          { ...haste, condition: has("vigilance"), conditionScope: "each-affected-object" },
        ],
      },
    }]);
    assert.deepEqual(abilities("Each creature has flying and has haste if it has vigilance."), [{
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"] } },
        modifiers: [flying, haste], condition: has("vigilance"), conditionScope: "each-affected-object",
      },
    }]);
  });

  it("rejects unsupported static branches without converting them into a resolving spell", () => {
    for (const text of [
      "Each creature has flying if it has trample. Otherwise, each creature has haste.",
      "Each creature has flying if it was dealt damage this turn. Otherwise, each creature has haste.",
      "Each spell you cast has split second if mana from an artifact was spent to cast it. Otherwise, each spell you cast has flash.",
      "Each creature has flying if it was cast during an unknown phase.",
      // Relic Axe: this is a conditional continuous bonus, not a resolving
      // self-replacement (CR 611.3, 614.15). Its static variant is unsupported.
      "Equipped creature gets +1/+1. If it's a Warrior, it gets +2/+1 instead.",
      "Creatures you control get +1/+1. If you control a Warrior, they get +2/+1 instead.",
      // A later sentence's duration cannot turn the opening continuous bonus
      // into a resolving pump with an unstated duration.
      "Equipped creature gets +1/+1. It gains flying until end of turn. If it's a Warrior, it gains haste instead.",
      // Nor can a lifetime inside a separately granted activated ability.
      'Equipped creature gets +1/+1 and has "{T}: It gets +1/+1 until end of turn." If it\'s a Warrior, it gets +2/+1 instead.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    // A targeted grant with an explicit duration still belongs to resolution.
    assert.deepEqual(abilities("Target creature has flying until end of turn if it has trample."), [{
      kind: "spell", effects: [{ steps: [{ effect: "gain-abilities", what: {
        ref: "target", filter: { types: ["creature"] }, count: fixed(1),
      }, abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }], duration: { duration: "end-of-turn" } }],
      condition: { condition: "has-keyword", what: itself, keyword: "trample" }, conditionKind: "if" }],
    }]);
    assert.deepEqual(abilities("Equipped creature gets +1/+1."), [{ kind: "static", effect: {
      static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [{ modifier: "pt",
        power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) } }],
    } }]);
    for (const [subject, what] of [
      ["Target creature", { ref: "target", filter: { types: ["creature"] }, count: fixed(1) }],
      ["Equipped creature", { ref: "equipped", noun: "creature" }],
    ] as const) assert.deepEqual(abilities(`${subject} gets +1/+1 until end of turn. If it's a Warrior, it gets +2/+1 until end of turn instead.`), [{
      kind: "spell", effects: [{ steps: [{ effect: "self-replacement", original: {
        effect: "pump", what, power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) },
        duration: { duration: "end-of-turn" },
      }, condition: { condition: "is-filter", what: itself, filter: { subtypes: ["Warrior"] } },
      instead: [{ effect: "pump", what: itself, power: { sign: 1, amount: fixed(2) }, toughness: { sign: 1, amount: fixed(1) },
        duration: { duration: "end-of-turn" } }], targetPolicy: "retain-original" }] }],
    }]);
  });
});

describe("attack groups and implied battlefield departures", () => {
  it("distinguishes per-attacker and grouped declarations", () => {
    assert.deepEqual(abilities("Whenever you attack with a creature, draw a card."), withDraw({
      trigger: "player-attacks", who: you, per: "attacker", with: { types: ["creature"] },
    }));
    // Military Intelligence; Alluring Suitor's exact count uses the same grammar.
    assert.deepEqual(abilities("Whenever you attack with two or more creatures, draw a card."), withDraw({
      trigger: "player-attacks", who: you, per: "declaration", with: { types: ["creature"] }, count: comparison("ge", 2),
    }));
    assert.deepEqual(abilities("Whenever you attack with exactly two creatures, draw a card."), withDraw({
      trigger: "player-attacks", who: you, per: "declaration", with: { types: ["creature"] }, count: comparison("eq", 2),
    }));
  });

  it("keeps attacked players and attacker restrictions on the same declaration", () => {
    // Akiri, Fearless Voyager.
    assert.deepEqual(abilities("Whenever you attack a player with one or more equipped creatures, draw a card."), withDraw({
      trigger: "player-attacks", who: you, whom: { player: "a-player" }, per: "declaration",
      with: { status: ["equipped"], types: ["creature"] }, count: comparison("ge", 1),
    }));
    assert.deepEqual(abilities("Whenever an opponent attacks you, draw a card."), withDraw({
      trigger: "player-attacks", who: { player: "an-opponent" }, whom: you, per: "declaration",
    }));
    assert.deepEqual(abilities("Whenever you attack, draw a card."), withDraw({ trigger: "you-attack" }));
  });

  it("infers battlefield departure only from explicit permanent descriptions", () => {
    // Viridian Harvest.
    assert.deepEqual(abilities("When enchanted artifact is put into a graveyard, you gain 6 life."), [{
      kind: "triggered", trigger: { trigger: "put-into-graveyard", what: { ref: "enchanted", noun: "artifact" },
        from: "battlefield", to: { zone: "graveyard", owner: "any" } },
      effects: [{ steps: [{ effect: "gain-life", who: you, amount: fixed(6) }] }],
    }]);
    const card = { ref: "filter" as const, filter: { cls: "card" as const }, count: fixed(1) };
    assert.deepEqual(abilities("Whenever a card is put into a graveyard from anywhere, draw a card."), withDraw({
      trigger: "put-into-graveyard", what: card, from: "anywhere", to: { zone: "graveyard", owner: "any" },
    }));
    for (const text of [
      "Whenever a card is put into a graveyard, draw a card.",
      "Whenever a creature card is put into a graveyard, draw a card.",
      "Whenever you attack with two creatures and a planeswalker, draw a card.",
      "Whenever you attack with creatures with total power 6 or greater, draw a card.",
      "Whenever you attack a player or planeswalker, draw a card.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("new historical and attack scopes remain uncertified", () => {
  it("diagnoses the missing event-specific binding analysis", () => {
    for (const text of [
      "If an opponent was dealt damage this turn, draw a card.",
      "If {B} was spent to cast this spell, draw a card.",
      "Whenever you attack with one or more creatures, draw a card.",
      "Each spell you cast has split second if mana from an artifact was spent to cast it.",
    ]) {
      const parsed = parseOracleText(text, "History fixture", { includeSourceProvenance: true });
      assert.ok(parsed.ok);
      const result = analyzeCardBindings({ identity: "history", source: { name: "History fixture", layout: "normal" },
        faces: [{ sourceFaceIndex: 0, result: parsed }] });
      assert.equal(result.fullyBound, false);
      assert.ok(result.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"), text);
    }
  });
});
