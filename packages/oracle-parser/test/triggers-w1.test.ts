import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import type { ObjectFilter, ObjectRef, Trigger } from "../src/ast.ts";

// Clauses are printed Oracle wording from the cached corpus (card named per
// case). Rules: CR 106.12, 508.1, 603.2, 603.6, 603.8, 700.4, 700.6, 700.13,
// 700.14, 701.18, 702.33d, 702.110b, 709.5h.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value }) as const;
const you = { player: "you" } as const;
const thisCreature: ObjectRef = { ref: "this", noun: "creature" };
const a = (filter: ObjectFilter): ObjectRef => ({ ref: "filter", filter, count: n(1) });

function trigger(text: string): Trigger {
  const c = new Cursor(lex(text));
  const result = parseTrigger(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens at ${c.pos}`);
  return result;
}

/** A state trigger must be the complete clause before the comma. */
function lineTrigger(text: string): Trigger {
  const result = parseOracleText(`${text}, sacrifice this creature.`, "Fixture", { typeLine: "Creature — Fish" });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const ability = result.abilities[0];
  assert.equal(ability.kind, "triggered");
  return (ability as Extract<typeof ability, { kind: "triggered" }>).trigger;
}

function unsupported(text: string): void {
  assert.equal(parseOracleText(`${text}, draw a card.`).ok, false, text);
}

describe("state triggers", () => {
  it("represents a present game state as the whole trigger condition", () => {
    // Dandân; Drop of Honey; Dark Depths.
    assert.deepEqual(lineTrigger("When you control no Islands"), {
      trigger: "state", condition: { condition: "control-count", who: you, of: { subtypes: ["Island"] }, comparison: { op: "eq", value: n(0) } },
    });
    assert.deepEqual(lineTrigger("When there are no creatures on the battlefield"), {
      trigger: "state", condition: { condition: "count", of: { types: ["creature"], zone: { zone: "battlefield" } }, comparison: { op: "eq", value: n(0) } },
    });
    assert.deepEqual(lineTrigger("When ~ has no ice counters on it"), {
      trigger: "state", condition: { condition: "is-filter", what: { ref: "self" },
        filter: { counterConstraints: [{ counter: { type: "ice" }, comparison: { op: "eq", value: n(0) } }] } },
    });
  });

  it("rejects history, targets and trailing words", () => {
    unsupported("When you attacked this turn");
    unsupported("When you control target creature");
    unsupported("When you control no Islands this turn");
  });
});

describe("player action events", () => {
  it("names crimes, expending, doors and cranking", () => {
    assert.deepEqual(trigger("Whenever you commit a crime"), { trigger: "commits-crime", who: you });
    assert.deepEqual(trigger("Whenever you expend 4"), { trigger: "expends", who: you, amount: 4 });
    assert.deepEqual(trigger("When you unlock this door"), { trigger: "unlocks-door", who: you, door: "this" });
    assert.deepEqual(trigger("Whenever you crank this Contraption"), {
      trigger: "cranks", who: you, what: { ref: "this", noun: "contraption" },
    });
    unsupported("Whenever you commit a felony");
    unsupported("Whenever you expend X");
    unsupported("When you unlock a door");
    unsupported("Whenever you crank a Contraption");
  });

  it("distinguishes playing a land or card from casting, and keeps the origin of the play", () => {
    // Horn of Greed; City of Traitors; Prosper, Tome-Bound.
    assert.deepEqual(trigger("Whenever a player plays a land"), { trigger: "plays", who: { player: "a-player" }, what: { types: ["land"] } });
    assert.deepEqual(trigger("When you play another land"), { trigger: "plays", who: you, what: { other: true, types: ["land"] } });
    assert.deepEqual(trigger("Whenever you play a card from exile"), {
      trigger: "plays", who: you, what: { cls: "card" }, from: { zone: "exile", owner: undefined },
    });
    unsupported("Whenever you play one or more lands");
  });

  it("records who taps a permanent for mana", () => {
    // Mana Flare family: Manabarbs; Crypt Ghast; Forbidden Orchard.
    assert.deepEqual(trigger("Whenever a player taps a land for mana"), {
      trigger: "taps-for-mana", what: a({ types: ["land"] }), who: { player: "a-player" },
    });
    assert.deepEqual(trigger("Whenever you tap this land for mana"), {
      trigger: "taps-for-mana", what: { ref: "this", noun: "land" }, who: you,
    });
    assert.deepEqual(trigger("Whenever a land is tapped for mana"), { trigger: "taps-for-mana", what: a({ types: ["land"] }) });
    unsupported("Whenever you tap a land");
    unsupported("Whenever you tap a land for {C}");
  });

  it("accepts an identified or collective sacrificed object", () => {
    // Ordeal of Thassa; Blood Hypnotist.
    assert.deepEqual(trigger("When you sacrifice this Aura"), { trigger: "sacrifices", who: you, what: { ref: "this", noun: "aura" } });
    assert.deepEqual(trigger("Whenever you sacrifice one or more Blood tokens"), {
      trigger: "sacrifices", who: you, what: { subtypes: ["Blood"], cls: "token" }, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever you sacrifice a creature"), { trigger: "sacrifices", who: you, what: { types: ["creature"] } });
  });

  it("keeps a damaged player distinct from a per-source damage event", () => {
    // Sun Droplet; Chandra's Spitfire.
    assert.deepEqual(trigger("Whenever you're dealt damage"), { trigger: "player-dealt-damage", who: you });
    assert.deepEqual(trigger("Whenever an opponent is dealt noncombat damage"), {
      trigger: "player-dealt-damage", who: { player: "an-opponent" }, combat: false,
    });
    unsupported("Whenever an opponent is dealt damage by a red spell");
    unsupported("Whenever you're dealt 3 or more damage");
  });
});

describe("object event qualifiers", () => {
  it("represents exploiting with the sacrificed creature's description", () => {
    // Sidisi, Undead Vizier; Henry Wu, InGen Geneticist.
    assert.deepEqual(trigger("When ~ exploits a creature"), {
      trigger: "exploits", what: { ref: "self" }, exploited: a({ types: ["creature"] }),
    });
    assert.deepEqual(trigger("Whenever a creature you control exploits a non-Human creature"), {
      trigger: "exploits", what: a({ types: ["creature"], control: "you" }), exploited: a({ nonSubtypes: ["Human"], types: ["creature"] }),
    });
    unsupported("When this creature exploits");
  });

  it("keeps entry state, departure exclusions and non-graveyard destinations", () => {
    // Mystic Sanctuary; Imperial Cosmographer; God-Eternal Kefnet; Reyhan.
    assert.deepEqual(trigger("When this land enters untapped"), { trigger: "enters", what: { ref: "this", noun: "land" }, untapped: true });
    assert.deepEqual(trigger("Whenever another creature you control leaves the battlefield without dying"), {
      trigger: "leaves", what: a({ other: true, types: ["creature"], control: "you" }), withoutDying: true,
    });
    assert.deepEqual(trigger("When ~ dies or is put into exile from the battlefield"), {
      trigger: "any-of", events: [
        { trigger: "dies", what: { ref: "self" } },
        { trigger: "put-into-zone", what: { ref: "self" }, from: { zone: "battlefield" }, to: { zone: "exile", owner: undefined } },
      ],
    });
    assert.deepEqual(trigger("Whenever a creature you control is put into the command zone"), {
      trigger: "put-into-zone", what: a({ types: ["creature"], control: "you" }), to: { zone: "command" },
    });
    unsupported("When this land enters tapped");
    unsupported("When this creature leaves your graveyard without dying");
    unsupported("When ~ is put into exile from anywhere other than the battlefield");
    unsupported("When ~ is put on top of your library");
  });

  it("excludes one origin from a graveyard event and treats an enchanted basic land as a permanent", () => {
    // Disa the Restless; Genju of the Cedars.
    assert.deepEqual(trigger("Whenever a Lhurgoyf permanent card is put into your graveyard from anywhere other than the battlefield"), {
      trigger: "put-into-graveyard", what: a({ subtypes: ["Lhurgoyf"], cls: "card", permanent: true }), from: "anywhere",
      to: { zone: "graveyard", owner: "your" }, notFrom: { zone: "battlefield" },
    });
    assert.deepEqual(trigger("When enchanted Forest is put into a graveyard"), {
      trigger: "put-into-graveyard", what: { ref: "enchanted", noun: "forest" }, from: "battlefield", to: { zone: "graveyard", owner: "any" },
    });
    unsupported("When this card is put into a graveyard");
  });

  it("requires further attackers in the same declaration", () => {
    // Battalion (Boros Elite); Enkira, Hostile Scavenger.
    assert.deepEqual(trigger("Whenever this creature and at least two other creatures attack"), {
      trigger: "attacks", what: thisCreature, alone: undefined, whom: undefined,
      alongside: { filter: { other: true, types: ["creature"] }, count: { op: "ge", value: n(2) } },
    });
    assert.deepEqual(trigger("Whenever ~ and at least two Zombies attack"), {
      trigger: "attacks", what: { ref: "self" }, alone: undefined, whom: undefined,
      alongside: { filter: { subtypes: ["Zombie"] }, count: { op: "ge", value: n(2) } },
    });
    unsupported("Whenever this creature and at most two other creatures attack");
    unsupported("Whenever a creature and at least two other creatures attack");
    unsupported("Whenever this creature and at least two other creatures block");
  });

  it("separates combat, noncombat and unrestricted damage and unions a player with a permanent type", () => {
    // Chandra's Incinerator; Wall of Essence; Dreadhorde Butcher; Park Heights Maverick.
    const source = a({ cls: "source", control: "you" });
    assert.deepEqual(trigger("Whenever a source you control deals noncombat damage to an opponent"), {
      trigger: "deals-damage", what: source, combat: false, to: { player: "an-opponent" },
    });
    assert.deepEqual(trigger("Whenever a source you control deals damage to an opponent"), {
      trigger: "deals-damage", what: source, combat: undefined, to: { player: "an-opponent" },
    });
    assert.deepEqual(trigger("Whenever this creature is dealt combat damage"), { trigger: "is-dealt-damage", what: thisCreature, combat: true });
    assert.deepEqual(trigger("Whenever this creature is dealt damage"), { trigger: "is-dealt-damage", what: thisCreature });
    assert.deepEqual(trigger("Whenever this creature deals combat damage to a player or planeswalker"), {
      trigger: "deals-damage", what: thisCreature, combat: true, to: a({ orPlayer: true, types: ["planeswalker"] }),
    });
    assert.deepEqual(trigger("Whenever this creature deals combat damage to a player or dies"), {
      trigger: "any-of", events: [
        { trigger: "deals-damage", what: thisCreature, combat: true, to: "any" },
        { trigger: "dies", what: thisCreature },
      ],
    });
    unsupported("Whenever this creature deals combat damage to a player or planeswalker you control");
  });

  it("reads one of your opponents as each single opponent participant", () => {
    // Calculating Lich; Edric, Spymaster of Trest.
    assert.deepEqual(trigger("Whenever a creature attacks one of your opponents"), {
      trigger: "attacks", what: a({ types: ["creature"] }), alone: undefined, whom: [{ player: "an-opponent" }],
    });
    assert.deepEqual(trigger("Whenever a creature deals combat damage to one of your opponents"), {
      trigger: "deals-damage", what: a({ types: ["creature"] }), combat: true, to: { player: "an-opponent" },
    });
    unsupported("Whenever a creature attacks two of your opponents");
  });
});

describe("ordinals, turn qualifiers and alternative events", () => {
  it("counts their Nth event separately for each player", () => {
    // Faerie Mastermind; Lotho, Corrupt Shirriff; Esper Sentinel.
    assert.deepEqual(trigger("Whenever an opponent draws their second card each turn"), {
      trigger: "draws", who: { player: "an-opponent" }, occurrence: { ordinal: 2, period: "turn", per: "player" },
    });
    assert.deepEqual(trigger("Whenever a player casts their second spell each turn"), {
      trigger: "cast", who: { player: "a-player" }, what: { cls: "spell" }, occurrence: { ordinal: 2, period: "turn", per: "player" },
    });
    assert.deepEqual(trigger("Whenever you cast your second spell each turn"), {
      trigger: "cast", who: you, what: { cls: "spell" }, occurrence: { ordinal: 2, period: "turn" },
    });
    unsupported("Whenever you cast their second spell each turn");
    unsupported("Whenever each opponent casts their second spell each turn");
    unsupported("Whenever an opponent casts their second spell");
  });

  it("restricts an event to an opponent's turn", () => {
    // Brineborn Cutthroat.
    assert.deepEqual(trigger("Whenever you cast a spell during an opponent's turn"), {
      trigger: "cast", who: you, what: { cls: "spell" }, during: "opponent",
    });
    unsupported("Whenever you cast a spell during an opponent's upkeep");
  });

  it("joins complete events after or, sharing a player subject or starting a new one", () => {
    // Flubs, the Fool; Staff of the Death Magus; Matoya, Archon Elder; Dreadhound.
    assert.deepEqual(trigger("Whenever you play a land or cast a spell"), {
      trigger: "any-of", events: [
        { trigger: "plays", who: you, what: { types: ["land"] } },
        { trigger: "cast", who: you, what: { cls: "spell" } },
      ],
    });
    assert.deepEqual(trigger("Whenever you cast a black spell or a Swamp you control enters"), {
      trigger: "any-of", events: [
        { trigger: "cast", who: you, what: { colors: ["black"], cls: "spell" } },
        { trigger: "enters", what: a({ subtypes: ["Swamp"], control: "you" }) },
      ],
    });
    assert.deepEqual(trigger("Whenever you scry or surveil"), {
      trigger: "any-of", events: [{ trigger: "scries", who: you }, { trigger: "surveils", who: you }],
    });
    assert.deepEqual(trigger("Whenever a creature dies or a creature card is put into a graveyard from a library"), {
      trigger: "any-of", events: [
        { trigger: "dies", what: a({ types: ["creature"] }) },
        { trigger: "put-into-graveyard", what: a({ types: ["creature"], cls: "card" }),
          from: { zone: "library", owner: "any" }, to: { zone: "graveyard", owner: "any" } },
      ],
    });
  });

  it("does not guess the scope of a qualifier after alternatives or accept a dangling or", () => {
    unsupported("Whenever you scry or surveil during your turn");
    unsupported("Whenever you play a land or cast a spell for the first time each turn");
    unsupported("Whenever you cast a black spell or a Swamp you control");
    const c = new Cursor(lex("Whenever you play a land or"));
    assert.ok(parseTrigger(c) === null || !c.done());
  });
});

describe("spell and ability descriptions used by cast and activate triggers", () => {
  const cast = (what: ObjectFilter): Trigger => ({ trigger: "cast", who: you, what });
  it("expands historic and keeps kicked, {X}, ownership and keyword tests", () => {
    // Jhoira, Weatherlight Captain; Roost of Drakes; Nev, the Practical Dean;
    // Tasha, the Witch Queen; Joyful Stormsculptor; Battlegate Mimic.
    assert.deepEqual(trigger("Whenever you cast a historic spell"), cast({
      allOf: [{ anyOf: [{ supertypes: ["legendary"] }, { types: ["artifact"] }, { subtypes: ["Saga"] }] }], cls: "spell",
    }));
    assert.deepEqual(trigger("Whenever you cast a kicked spell"), cast({ kicked: true, cls: "spell" }));
    assert.deepEqual(trigger("Whenever you cast a spell with {X} in its mana cost"), cast({ cls: "spell", manaCostContains: ["X"] }));
    assert.deepEqual(trigger("Whenever you cast a spell you don't own"), cast({ cls: "spell", own: "not-you" }));
    assert.deepEqual(trigger("Whenever you cast a spell that has convoke"), cast({ cls: "spell", withAbility: ["convoke"] }));
    assert.deepEqual(trigger("Whenever you cast a spell with convoke"), cast({ cls: "spell", withAbility: ["convoke"] }));
    assert.deepEqual(trigger("Whenever you cast a spell that's both red and white"), cast({ cls: "spell", colors: ["red", "white"] }));
  });

  it("names an activated ability by its keyword", () => {
    // Rangers' Refueler; Frenzied Raider.
    assert.deepEqual(trigger("Whenever you activate an exhaust ability"), {
      trigger: "activate", who: you, what: { cls: "ability", abilityKeyword: "exhaust" },
    });
    assert.deepEqual(trigger("Whenever you activate a boast ability"), {
      trigger: "activate", who: you, what: { cls: "ability", abilityKeyword: "boast" },
    });
  });

  it("rejects near-miss descriptions", () => {
    unsupported("Whenever you cast a spell that's both red or white");
    unsupported("Whenever you cast a spell that's both red");
    unsupported("Whenever you cast a spell with {2} in its mana cost");
    unsupported("Whenever you cast a spell with {X} in its text");
    unsupported("Whenever you cast a kicked");
    unsupported("Whenever you activate a flying ability");
    unsupported("Whenever you cast a spell that has an Adventure");
    // The relative "that has" keyword test is limited to spell descriptions.
    assert.equal(parseOracleText("Put a time counter on each of those cards that has suspend.", "Fixture", { typeLine: "Sorcery" }).ok, false);
  });
});
