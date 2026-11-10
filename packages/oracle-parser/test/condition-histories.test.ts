import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Condition } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCondition } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Clauses taken from the cached Oracle corpus; card names are noted per test.
// CR 103.4, 118.12, 122.1f, 309.7, 603.4, 603.10a, 700.8c, 700.11, 700.13,
// 701.59c, 702.62b, 702.104b, 702.112b, 702.131c, 702.138b, 702.166b,
// 702.174k, 707.10b, 725, 726, 731:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;
const it_ = { ref: "it" } as const;
const atLeast = (value: number) => ({ op: "ge" as const, value: fixed(value) });
const not = (of: Condition): Condition => ({ condition: "not", of });

function condition(text: string): Condition {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.ok(parsed, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return parsed;
}
/** No complete condition: either no parse or unconsumed words remain. */
function rejected(text: string): void {
  const c = new Cursor(lex(text));
  const parsed = parseCondition(c);
  assert.equal(parsed !== null && c.done(), false, text);
}
function abilities(text: string, typeLine: string): Ability[] {
  const parsed = parseOracleText(text, "W1 fixture", { typeLine });
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities));
}

describe("designations as conditions", () => {
  it("keeps the holder and its quantifier (Throne Warden, Queen Marchesa, Archivist of Gondor)", () => {
    assert.deepEqual(condition("you're the monarch"), { condition: "designation", who: you, designation: "monarch" });
    assert.deepEqual(condition("an opponent is the monarch"), {
      condition: "designation", who: { player: "an-opponent" }, designation: "monarch",
    });
    assert.deepEqual(condition("there is no monarch"), not({ condition: "designation", who: { player: "a-player" }, designation: "monarch" }));
    assert.deepEqual(condition("you have the initiative"), { condition: "designation", who: you, designation: "initiative" });
    assert.deepEqual(condition("you don't have the city's blessing"), not({ condition: "designation", who: you, designation: "city's-blessing" }));
    for (const text of ["you become the monarch", "you have the monarch"]) rejected(text);
  });

  it("represents day/night, dungeon completion and a full party (Wolf Strike, Gloom Stalker, Nalia de'Arnise)", () => {
    assert.deepEqual(condition("it's night"), { condition: "day-night", state: "night" });
    assert.deepEqual(condition("it's neither day nor night"), { condition: "day-night", state: "neither" });
    assert.deepEqual(condition("you've completed a dungeon"), { condition: "completed-dungeon", who: you });
    assert.deepEqual(condition("you have a full party"), { condition: "full-party", who: you });
    for (const text of ["it's dusk", "you've completed a quest", "you have a party"]) rejected(text);
  });

  it("names another player's turn without touching the your-turn nodes (Glademuse, Discordant Spirit)", () => {
    assert.deepEqual(condition("it's not their turn"), not({ condition: "turn-of", who: { player: "they" } }));
    assert.deepEqual(condition("it's an opponent's turn"), { condition: "turn-of", who: { player: "an-opponent" } });
    assert.deepEqual(condition("it's not your turn"), { condition: "not-your-turn" });
  });

  it("maps poisoned, renowned and suspended to their rules definitions (Septic Rats, Consul's Lieutenant, Deep-Sea Kraken)", () => {
    assert.deepEqual(condition("defending player is poisoned"), {
      condition: "player-counters", who: { player: "defending-player" }, counter: { type: "poison" }, comparison: atLeast(1),
    });
    assert.deepEqual(condition("it's renowned"), { condition: "status", what: it_, status: "renowned" });
    assert.deepEqual(condition("this card is suspended"), { condition: "status", what: { ref: "this", noun: "card" }, status: "suspended" });
    rejected("each creature is renowned");
  });
});

describe("linked keyword decisions and ability history", () => {
  it("identifies the keyword and the printed object (Scrapshooter, Thunder Brute, Agatha's Champion, Earwig Squad, Uro)", () => {
    assert.deepEqual(condition("the gift was promised"), { condition: "keyword-choice", keyword: "gift" });
    assert.deepEqual(condition("tribute wasn't paid"), not({ condition: "keyword-choice", keyword: "tribute" }));
    assert.deepEqual(condition("evidence was collected"), { condition: "keyword-choice", keyword: "collect-evidence" });
    assert.deepEqual(condition("this spell was bargained"), { condition: "keyword-choice", keyword: "bargain", what: { ref: "this", noun: "spell" } });
    assert.deepEqual(condition("it's bargained"), { condition: "keyword-choice", keyword: "bargain", what: it_ });
    assert.deepEqual(condition("its prowl cost was paid"), { condition: "keyword-choice", keyword: "prowl", what: it_ });
    assert.deepEqual(condition("it escaped"), { condition: "keyword-choice", keyword: "escape", what: it_ });
    for (const text of ["the gift was given", "its kicker cost was paid", "a creature escaped"]) rejected(text);
  });

  it("names a printed alternative cost by its symbols (Devastating Mastery)", () => {
    assert.deepEqual(condition("the {2}{W}{W} cost was paid"), {
      condition: "alternative-cost-paid", cost: [{ cost: "mana", symbols: ["2", "W", "W"] }],
    });
    rejected("that cost was paid");
  });

  it("counts resolutions of the same ability, including abbreviated follow-ups (Belladonna Took)", () => {
    const ordinal = (...ordinals: number[]): Condition => ({ condition: "ability-resolution-ordinal", ordinals, during: "this-turn" });
    assert.deepEqual(condition("this is the first or second time this ability has resolved this turn"), ordinal(1, 2));
    assert.deepEqual(abilities("Whenever a token you control enters, you gain 1 life if this is the first time this ability has resolved this turn. If it's the second time, draw a card.", "Creature — Halfling"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "filter", filter: { cls: "token", control: "you" }, count: fixed(1) } },
      effects: [
        { steps: [{ effect: "gain-life", who: you, amount: fixed(1) }], condition: ordinal(1), conditionKind: "if", conditionPosition: "after" },
        { steps: [{ effect: "draw", who: you, amount: fixed(1) }], condition: ordinal(2), conditionKind: "if" },
      ],
    }]);
    // The abbreviation needs its complete clause earlier in the same ability.
    assert.equal(parseOracleText("When this creature enters, if it's the second time, draw a card.", "W1 fixture", { typeLine: "Creature — Bear" }).ok, false);
    rejected("this is the second time this ability has resolved this game");
  });

  it("counts activations with an explicit bound only (Dragon Whelp)", () => {
    assert.deepEqual(condition("this ability has been activated four or more times this turn"), {
      condition: "ability-activation-count", comparison: atLeast(4), during: "this-turn",
    });
    rejected("this ability has been activated four times this turn");
  });

  it("tests inability to perform the preceding mandatory action (Lord of the Pit)", () => {
    assert.deepEqual(abilities("At the beginning of your upkeep, sacrifice a creature. If you can't, you lose 2 life.", "Creature — Demon"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
      effects: [
        { steps: [{ effect: "sacrifice", who: you, what: { ref: "filter", filter: { types: ["creature"] }, count: fixed(1) } }] },
        { steps: [{ effect: "lose-life", who: you, amount: fixed(2) }],
          condition: { condition: "preceding-action", who: you, outcome: "unable" }, conditionKind: "if" },
      ],
    }]);
    // Xathrid Demon restates the action; that is not the bare CR 118.12 clause.
    assert.equal(parseOracleText("At the beginning of your upkeep, sacrifice a creature. If you can't sacrifice a creature, you lose 2 life.",
      "W1 fixture", { typeLine: "Creature — Demon" }).ok, false);
  });
});

describe("player comparisons and player history", () => {
  it("keeps both operands and their quantifiers (Land Tax, Akuta, Battle Angels of Tyr)", () => {
    // A fixed reference player reuses the ordinary count condition, compared
    // with that player's own measurement (see amounts-quantities.test.ts).
    assert.deepEqual(condition("an opponent controls more lands than you"), {
      condition: "control-count", who: { player: "an-opponent" }, of: { types: ["land"] },
      comparison: { op: "gt", value: { amount: "count", of: { types: ["land"], control: "you" } } },
    });
    assert.deepEqual(condition("you have more cards in hand than each opponent"), {
      condition: "player-comparison", who: you, quantity: { quantity: "cards-in-hand" }, op: "gt", than: { player: "each-opponent" },
    });
    assert.deepEqual(condition("that player has more life than each other player"), {
      condition: "player-comparison", who: { player: "that-player" }, quantity: { quantity: "life-total" }, op: "gt",
      than: { player: "each-other-player" },
    });
    assert.deepEqual(condition("you control fewer creatures than an opponent"), {
      condition: "player-comparison", who: you, quantity: { quantity: "controlled-count", of: { types: ["creature"] } },
      op: "lt", than: { player: "an-opponent" },
    });
    for (const text of ["an opponent controls more lands you control than you", "you have fewer life than an opponent"]) rejected(text);
  });

  it("counts the opponents' permanents together (Turbulent Fen, Kezzerdrix)", () => {
    assert.deepEqual(condition("your opponents control eight or more lands"), {
      condition: "count", of: { types: ["land"], control: "opponent" }, comparison: atLeast(8),
    });
    assert.deepEqual(condition("your opponents control no creatures"), {
      condition: "count", of: { types: ["creature"], control: "opponent" }, comparison: { op: "eq", value: fixed(0) },
    });
    rejected("your opponents control two lands");
  });

  it("records who did what and how often (Elanor Gardner, Lake-town Toymaker, Mercadian Atlas, Proctor of Potential)", () => {
    const history = (action: object, comparison = atLeast(1), who: object = you): Condition =>
      ({ condition: "player-action-history", who, action, comparison, during: "this-turn" }) as Condition;
    assert.deepEqual(condition("you sacrificed a Food this turn"), history({ action: "sacrifice", what: { subtypes: ["Food"] } }));
    assert.deepEqual(condition("you've drawn two or more cards this turn"), history({ action: "draw" }, atLeast(2)));
    assert.deepEqual(condition("you didn't play a land this turn"), not(history({ action: "play-land" })));
    assert.deepEqual(condition("an opponent discarded a card this turn"),
      history({ action: "discard", what: { cls: "card" } }, atLeast(1), { player: "an-opponent" }));
    assert.deepEqual(condition("you created a token this turn"), history({ action: "create-token", what: { cls: "token" } }));
    assert.deepEqual(condition("you've committed a crime this turn"), history({ action: "commit-crime" }));
    assert.deepEqual(condition("you've scried or surveilled this turn"), {
      condition: "any-of", conditions: [history({ action: "scry" }), history({ action: "surveil" })],
    });
    // A bare number has no stated bound; present tense offers an action.
    for (const text of ["you sacrificed two permanents this turn", "you discarded a creature this turn", "you sacrifice a Food this turn"]) rejected(text);
  });
});

describe("zone-change and life history wordings", () => {
  const enters = (filter: object, controllerAtMove: object = you): Condition => ({
    condition: "zone-change-history", event: "enters",
    subject: { match: "filter", filter, comparison: atLeast(1) }, to: { zone: "battlefield" }, controllerAtMove, during: "this-turn",
  }) as Condition;

  it("gives 'you had … enter' the node of the passive wording (Groundswell, Wary Farmer)", () => {
    assert.deepEqual(condition("you had a land enter the battlefield under your control this turn"), enters({ types: ["land"] }));
    assert.deepEqual(condition("you had a land enter the battlefield under your control this turn"),
      condition("a land entered the battlefield under your control this turn"));
    assert.deepEqual(condition("another creature entered the battlefield under your control this turn"), enters({ types: ["creature"], other: true }));
    rejected("you had a land enter the battlefield under their control this turn");
  });

  it("keeps control at the move and the zone left (Lulu, Primary Research, Cactuar)", () => {
    assert.deepEqual(condition("a permanent you controlled left the battlefield this turn"), {
      condition: "zone-change-history", event: "leaves-battlefield",
      subject: { match: "filter", filter: { cls: "permanent" }, comparison: atLeast(1) },
      from: { zone: "battlefield" }, controllerAtMove: you, during: "this-turn",
    });
    assert.deepEqual(condition("a card left your graveyard this turn"), {
      condition: "zone-change-history", event: "leaves-zone",
      subject: { match: "filter", filter: { cls: "card" }, comparison: atLeast(1) },
      from: { zone: "graveyard", owner: "your" }, during: "this-turn",
    });
    assert.deepEqual(condition("this creature didn't enter the battlefield this turn"), not({
      condition: "zone-change-history", event: "enters", subject: { match: "object", what: { ref: "this", noun: "creature" } },
      to: { zone: "battlefield" }, during: "this-turn",
    }));
    rejected("a card left your graveyard");
  });

  it("expands descended to its CR 700.11 definition (Ruin-Lurker Bat)", () => {
    assert.deepEqual(condition("you descended this turn"), condition("a permanent card was put into your graveyard from anywhere this turn"));
    assert.deepEqual(condition("you descended this turn"), {
      condition: "zone-change-history", event: "put-into-zone",
      subject: { match: "filter", filter: { cls: "card", permanent: true }, comparison: atLeast(1) },
      from: "anywhere", to: { zone: "graveyard", owner: "your" }, during: "this-turn",
    });
    rejected("you descended");
  });

  it("combines and negates life changes, and offsets the starting life total (Lunar Convocation, Luminarch Ascension, Angel of Destiny)", () => {
    const life = (change: "gained" | "lost"): Condition =>
      ({ condition: "life-change", who: you, change, comparison: { op: "gt", value: fixed(0) }, during: "this-turn" });
    assert.deepEqual(condition("you gained and lost life this turn"), { condition: "all-of", conditions: [life("gained"), life("lost")] });
    assert.deepEqual(condition("you didn't lose life this turn"), not(life("lost")));
    assert.deepEqual(condition("you have at least 15 life more than your starting life total"), {
      condition: "life-total", who: you,
      comparison: { op: "ge", value: { amount: "plus", a: { amount: "starting-life-total", of: you }, b: fixed(15) } },
    });
    assert.deepEqual(condition("your life total is less than your starting life total"), {
      condition: "compare", value: { amount: "life-total", of: you }, comparison: { op: "lt", value: { amount: "starting-life-total", of: you } },
    });
    rejected("you have 15 life more than your starting life total");
  });
});

describe("state predicates", () => {
  it("reads an article as existence, never exactly one (Murasa Behemoth, Windwright Mage, Dragonfly Swarm)", () => {
    const inGraveyard = (filter: object): Condition => ({
      condition: "count", of: { ...filter, cls: "card", zone: { zone: "graveyard", owner: "your" } }, comparison: atLeast(1),
    }) as Condition;
    assert.deepEqual(condition("there is a land card in your graveyard"), inGraveyard({ types: ["land"] }));
    assert.deepEqual(condition("an artifact card is in your graveyard"), inGraveyard({ types: ["artifact"] }));
    assert.deepEqual(condition("there's a Lesson card in your graveyard"), inGraveyard({ subtypes: ["Lesson"] }));
  });

  it("counts battlefield objects, counters on an object, and attackers (Pestilence, Budoka Pupil, Keldon Twilight)", () => {
    assert.deepEqual(condition("no creatures are on the battlefield"), condition("there are no creatures on the battlefield"));
    assert.deepEqual(condition("there are two or more ki counters on this creature"), condition("this creature has two or more ki counters on it"));
    assert.deepEqual(condition("there are two or more ki counters on this creature"), {
      condition: "is-filter", what: { ref: "this", noun: "creature" },
      filter: { counterConstraints: [{ counter: { type: "ki" }, comparison: atLeast(2) }] },
    });
    assert.deepEqual(condition("no creatures attacked this turn"), {
      condition: "compare", value: { amount: "combat-history-count", action: "attacked", during: "this-turn", distinct: "object-incarnations" },
      comparison: { op: "eq", value: fixed(0) },
    });
    rejected("there are two ki counters on this creature");
  });

  it("negates object predicates and accepts adjective-only ones (Gruff Triplets, Burnout, Coiling Rebirth, Shatterskull Charger)", () => {
    assert.deepEqual(condition("it isn't a token"), not({ condition: "is-filter", what: it_, filter: { cls: "token" } }));
    assert.deepEqual(condition("it's red"), { condition: "is-filter", what: it_, filter: { colors: ["red"] } });
    assert.deepEqual(condition("that creature isn't legendary"), not({
      condition: "is-filter", what: { ref: "that", filter: { types: ["creature"] } }, filter: { supertypes: ["legendary"] },
    }));
    assert.deepEqual(condition("this creature doesn't have a +1/+1 counter on it"), not({
      condition: "is-filter", what: { ref: "this", noun: "creature" },
      filter: { counterConstraints: [{ counter: { type: "+1/+1" }, comparison: atLeast(1) }] },
    }));
  });

  it("asks about an identified object's earlier existence (Boomerang Basics, Deathknell Berserker)", () => {
    assert.deepEqual(condition("you controlled that permanent"), {
      condition: "control", who: you, what: { ref: "that", filter: { cls: "permanent" } }, time: "previous",
    });
    assert.deepEqual(condition("its power was 3 or greater"), condition("it had power 3 or greater"));
    assert.deepEqual(condition("its power was 3 or greater"), {
      condition: "is-filter", what: it_, filter: { properties: [{ property: "power", comparison: atLeast(3) }] }, time: "previous",
    });
    rejected("you controlled a creature");
  });

  it("offers a payment to any player (Icy Prison)", () => {
    assert.deepEqual(condition("any player pays {3}"), {
      condition: "pays", who: { player: "a-player" }, cost: [{ cost: "mana", symbols: ["3"] }],
    });
    rejected("you pay {2}");
  });
});
