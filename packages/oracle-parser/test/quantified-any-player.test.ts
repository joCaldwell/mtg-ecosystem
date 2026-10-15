import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Amount, PlayerRef } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Oracle wordings checked in the frozen corpus;
// the cards are named with each construct.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
function rejected(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("any opponent may", () => {
  // Desecration Demon, Innocent Traveler, Vexing Devil, Distant Memories,
  // Sin Prodder. At most one opponent performs the offered instruction; the
  // follow-up asks whether a player did (CR 118.12, 608.2c).
  const anyOpponent: PlayerRef = { player: "any-one-of", among: { player: "each-opponent" } };
  const someoneDid = { condition: "preceding-action", who: { player: "a-player" }, outcome: "performed" };
  it("keeps the single performing opponent apart from each opponent", () => {
    assert.deepEqual(parsed("At the beginning of each combat, any opponent may sacrifice a creature of their choice. If a player does, tap this creature and put a +1/+1 counter on it.", "Creature — Demon"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "combat", whose: "each-player" },
      effects: [
        { steps: [{ effect: "sacrifice", who: anyOpponent, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: anyOpponent,
          optional: true, decidedBy: anyOpponent }] },
        { condition: someoneDid, conditionKind: "if", steps: [
          { effect: "tap", what: { ref: "this", noun: "creature" } },
          { effect: "put-counters", counter: { type: "+1/+1" }, count: n(1), on: { ref: "it" } },
        ] },
      ],
    }]);
  });
  it("reads 'If no one does' as no player having done it", () => {
    assert.deepEqual(parsed("At the beginning of your upkeep, any opponent may sacrifice a creature of their choice. If no one does, transform this creature.", "Creature — Human Peasant")[0].effects[1], {
      condition: { condition: "not", of: someoneDid }, conditionKind: "if",
      steps: [{ effect: "transform", what: { ref: "this", noun: "creature" }, who: { player: "you" } }],
    });
  });
  it("reads a damage offer as the has-damage-dealt condition, like 'unless' (CR 118.12a)", () => {
    // Vexing Devil, Longhorn Firebeast: the positive form of Browbeat's offer.
    assert.deepEqual(parsed("When this creature enters, any opponent may have it deal 4 damage to them. If a player does, sacrifice this creature.", "Creature — Devil"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "sacrifice", who: { player: "you" }, what: { ref: "this", noun: "creature" } }],
        condition: { condition: "has-damage-dealt", who: { player: "an-opponent" }, source: { ref: "it" }, amount: n(4) }, conditionKind: "if" }],
    }]);
    // Without its follow-up nothing says that one player's action settles it.
    rejected("When this creature enters, any opponent may have it deal 4 damage to them.", "Creature — Devil");
  });
  it("keeps the causative actor of a non-damage instruction", () => {
    assert.deepEqual(parsed("Search your library for a card, exile it, then shuffle. Any opponent may have you put that card into your hand. If no player does, you draw three cards.", "Sorcery")[0].effects.slice(1), [
      { steps: [{ effect: "move-zone", what: { ref: "that", filter: { cls: "card" } }, to: { zone: "hand", owner: "your" }, optional: true, decidedBy: anyOpponent }] },
      { condition: { condition: "not", of: someoneDid }, conditionKind: "if", steps: [{ effect: "draw", who: { player: "you" }, amount: n(3) }] },
    ]);
  });
  it("keeps 'any player may' and does not read 'each player may … If a player does' existentially", () => {
    // Brain Gorgers; Orzhov Advokist's follow-up is about each player who did.
    const anyPlayer: PlayerRef = { player: "any-one-of", among: { player: "each-player" } };
    assert.deepEqual(parsed("When you cast this spell, any player may sacrifice a creature of their choice. If a player does, counter ~.", "Creature — Zombie")[0].effects, [
      { steps: [{ effect: "sacrifice", who: anyPlayer, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: anyPlayer, optional: true, decidedBy: anyPlayer }] },
      { condition: someoneDid, conditionKind: "if", steps: [{ effect: "counter", what: { ref: "self" } }] },
    ]);
    rejected("At the beginning of your upkeep, each player may put two +1/+1 counters on a creature they control. If a player does, creatures that player controls can't attack you or planeswalkers you control until your next turn.", "Creature — Human Advisor");
  });
  it("still rejects a plural decider's 'them' and an instruction without 'may'", () => {
    rejected("When this creature enters, each opponent may have it deal 4 damage to them. If a player does, sacrifice this creature.", "Creature — Devil");
    rejected("When this creature enters, any opponent sacrifices a creature.", "Creature — Devil");
  });
});

describe("villainous choice", () => {
  // The Dalek Emperor, Dr. Eggman, Missy, Damocles Base. CR 701.55a-d: the
  // facing player chooses one option; all actions in it are performed.
  it("keeps the facing player and both complete options", () => {
    assert.deepEqual(parsed("At the beginning of combat on your turn, each opponent faces a villainous choice — That player sacrifices a creature of their choice, or you create a 3/3 black Dalek artifact creature token with menace.", "Legendary Artifact Creature — Dalek"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "combat", whose: "your" },
      effects: [{ steps: [{ effect: "villainous-choice", who: { player: "each-opponent" }, options: [
        [{ effect: "sacrifice", who: { player: "that-player" }, what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: { player: "that-player" } }],
        [{ effect: "create-token", count: n(1), token: { power: n(3), toughness: n(3), colors: ["black"], supertypes: [], types: ["artifact", "creature"], subtypes: ["Dalek"],
          keywords: [{ keyword: "menace" }] } }],
      ] }] }],
    }]);
  });
  it("keeps a multi-instruction option and an optional instruction inside one option", () => {
    assert.deepEqual(parsed("Whenever this creature deals combat damage to a player, that player faces a villainous choice — They sacrifice a nontoken creature of their choice, or they lose 2 life and you draw two cards.", "Creature — Dalek")[0].effects, [{
      steps: [{ effect: "villainous-choice", who: { player: "that-player" }, options: [
        [{ effect: "sacrifice", who: { player: "they" }, what: { ref: "filter", filter: { nonToken: true, types: ["creature"] }, count: n(1) }, chosenBy: { player: "they" } }],
        [{ effect: "lose-life", who: { player: "they" }, amount: n(2) }, { effect: "draw", who: { player: "you" }, amount: n(2) }],
      ] }],
    }]);
    assert.deepEqual(parsed("Draw three cards. Then target opponent faces a villainous choice — They discard three cards, or you may cast a spell from your hand without paying its mana cost.", "Sorcery")[0].effects[1], {
      steps: [{ effect: "villainous-choice", who: { player: "target-opponent" }, options: [
        [{ effect: "discard", who: { player: "they" }, what: { ref: "filter", filter: { cls: "card" }, count: n(3) } }],
        [{ effect: "play-card", optional: true, instruction: { who: { player: "you" }, action: "cast", what: { ref: "filter", filter: { cls: "spell" }, count: n(1) },
          from: { zone: "hand", owner: "your" }, alternativeCost: { cost: "without-mana-cost" } } }],
      ] }],
    });
  });
  it("rejects a choice without two complete options", () => {
    rejected("Each opponent faces a villainous choice — That player discards a card.", "Sorcery");
    rejected("Each opponent faces a villainous choice — That player discards a card, or.", "Sorcery");
    rejected("Each opponent faces a choice — That player discards a card, or you draw a card.", "Sorcery");
  });
});

describe("friend or foe", () => {
  // Virtus's Maneuver, Pir's Whim, Regna's Sanction, Zndrsplt's Judgment,
  // Khorvath's Fury: you label each player; the labeled groups then act.
  const label = (friendOrFoe: "friend" | "foe"): PlayerRef => ({ player: "filtered", base: { player: "each-player" }, filter: { friendOrFoe } });
  it("chooses the labels before the labeled players act", () => {
    assert.deepEqual(parsed("For each player, choose friend or foe. Each friend returns a creature card from their graveyard to their hand. Each foe sacrifices a creature of their choice.", "Sorcery"), [{
      kind: "spell", effects: [
        { steps: [{ effect: "choose-friend-or-foe", who: { player: "you" }, players: { player: "each-player" } }] },
        { steps: [{ effect: "return-to-hand", what: { ref: "filter", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "their" } }, count: n(1) },
          to: "their", who: label("friend") }] },
        { steps: [{ effect: "sacrifice", who: label("foe"), what: { ref: "filter", filter: { types: ["creature"] }, count: n(1) }, chosenBy: label("foe") }] },
      ],
    }]);
  });
  it("keeps a labeled group as a damage recipient", () => {
    assert.deepEqual(parsed("For each player, choose friend or foe. Each friend draws a card. ~ deals 2 damage to each foe.", "Sorcery")[0].effects[2], {
      steps: [{ effect: "damage", source: { ref: "self" }, amount: n(2), to: { to: "player", ref: label("foe") } }],
    });
  });
  it("rejects labels nobody chose", () => {
    rejected("Each friend draws a card.", "Sorcery");
    rejected("Each foe sacrifices a creature. For each player, choose friend or foe.", "Sorcery");
    rejected("For each opponent, choose friend or foe. Each foe sacrifices a creature.", "Sorcery");
  });
});

describe("draw another card", () => {
  // Imoen, Mystic Trickster; Hymn of the Faller: one more one-card draw,
  // like "draw an additional card" (CR 121.2).
  it("is an ordinary one-card draw", () => {
    assert.deepEqual(parsed("At the beginning of your end step, if you have the initiative, draw a card. Draw another card if you've completed a dungeon.", "Legendary Creature — Human Wizard")[0].effects[1], {
      steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }], condition: { condition: "completed-dungeon", who: { player: "you" } }, conditionKind: "if",
    });
  });
  it("does not read other 'another' phrases as one card", () => {
    rejected("Draw another two cards.", "Sorcery");
    rejected("Draw another.", "Sorcery");
  });
});

describe("with the same controller", () => {
  // Bioshift, Simic Guildmage, Fear, Fire, Foes!: the standard is left
  // unstated (the object named before it) and stays contextual.
  it("marks the second target's unstated controller standard", () => {
    assert.deepEqual(parsed("Move any number of +1/+1 counters from target creature onto another target creature with the same controller.", "Instant"), [{
      kind: "spell", effects: [{ steps: [{ effect: "move-counters", counter: { type: "+1/+1" }, count: { amount: "chosen", minimum: 0 },
        from: { ref: "target", filter: { types: ["creature"] }, count: n(1) },
        to: { ref: "target", filter: { types: ["creature"], implicitSameController: true, other: true }, count: n(1) } }] }],
    }]);
  });
  it("applies to a group as well", () => {
    assert.deepEqual(parsed("~ deals X damage to target creature and 1 damage to each other creature with the same controller.", "Sorcery")[0].effects[0].steps[0].steps[1], {
      effect: "damage", source: { ref: "self" }, amount: n(1), to: { to: "object", ref: { ref: "each", filter: { other: true, types: ["creature"], implicitSameController: true } } },
    });
  });
  it("does not accept a different relation", () => {
    rejected("Move a +1/+1 counter from target creature onto another target creature with the same owner.", "Instant");
    rejected("Move a +1/+1 counter from target creature onto another target creature with the same controller as you.", "Instant");
  });
});

describe("any player / each player as an existential condition and a unit", () => {
  // Knight of Grace: some player controls a black permanent.
  it("reads 'any player controls' existentially", () => {
    assert.deepEqual(parsed("This creature gets +1/+0 as long as any player controls a black permanent.", "Creature — Human Knight")[0].effect.condition, {
      condition: "control", who: { player: "a-player" }, what: { ref: "filter", filter: { colors: ["black"], cls: "permanent" }, count: n(1) },
    });
  });
  // Benediction of Moons: every player in the game counts once.
  it("counts players for a bare 'for each player'", () => {
    assert.deepEqual(parsed("You gain 1 life for each player.", "Sorcery")[0].effects[0].steps[0], {
      effect: "gain-life", who: { player: "you" }, amount: { amount: "product", factors: [n(1), { amount: "player-count", of: { player: "each-player" } }] },
    });
    rejected("You gain 1 life for each player and each creature.", "Sorcery");
  });
});

describe("kinds of counters among objects", () => {
  // Perrie, the Pulverizer; Storm of Forms. CR 122.1: each kind counts once.
  const kinds = { amount: "distinct-count", characteristic: "counter-kind", among: { ref: "each", filter: { cls: "permanent", control: "you" } } };
  it("counts the different kinds as one number", () => {
    assert.deepEqual(parsed("Whenever this creature attacks, target creature you control gets +X/+X until end of turn, where X is the number of different kinds of counters among permanents you control.", "Creature — Rhino")[0].effects[0].bindings, [
      { variable: "x", value: kinds }]);
  });
  it("makes that many copies", () => {
    assert.deepEqual(parsed("When you cast this spell, copy it for each kind of counter among permanents you control. You may choose new targets for the copies.", "Sorcery")[0].effects[0].steps[0], {
      effect: "copy-stack-object", what: { ref: "trigger-event-object", event: "cast" }, who: { player: "you" }, count: { amount: "product", factors: [n(1), kinds] },
      retarget: { who: { player: "you" }, optional: true },
    });
  });
  it("does not count a kind on one object as a unit of objects", () => {
    rejected("Draw a card for each kind of counter.", "Sorcery");
  });
});

describe("their next upkeep; {S} spent as a unit", () => {
  // Phytotitan: the delayed trigger waits for that player's next upkeep.
  it("keeps the pronoun owner of the next upkeep", () => {
    assert.deepEqual(parsed("When this creature dies, return it to the battlefield tapped under its owner's control at the beginning of their next upkeep.", "Creature — Plant Elemental")[0].effects[0].steps[0], {
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "upkeep", whose: { player: "they" } }, occurrence: "next", nextOnly: true,
      effects: [{ steps: [{ effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, tapped: true, controller: "owner" }] }], targets: "enclosing-ability",
    });
  });
  // Tundra Fumarole. CR 107.4h: {S} spent is mana from snow sources.
  it("scales a mana symbol by the snow mana spent", () => {
    assert.deepEqual(parsed("Add {C} for each {S} spent to cast this spell.", "Snow Sorcery")[0].effects[0].steps[0], {
      effect: "add-mana", who: { player: "you" }, mana: { mana: "scaled-symbol", symbol: "C", amount: { amount: "mana-spent", what: { ref: "this", noun: "spell" }, quantity: "snow" } },
    });
    rejected("Add {C} for each {G} spent to cast this spell.", "Snow Sorcery");
  });
});
