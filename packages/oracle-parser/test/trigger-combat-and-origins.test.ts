import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import type { ObjectFilter, ObjectRef, Trigger } from "../src/ast.ts";

// Clauses are printed Oracle wording from the cached corpus (card named per
// case). Rules text: Comprehensive Rules 2026-09-25, cited per suite.
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

function unsupported(text: string, type = "Creature — Fish"): void {
  assert.equal(parseOracleText(`${text}, draw a card.`, "Fixture", { typeLine: type }).ok, false, text);
}

describe("attacks and isn't blocked (CR 509.3g)", () => {
  it("is a separate event from an attack", () => {
    // Ophidian; Cloak of Confusion; Stinkdrinker Bandit.
    assert.deepEqual(trigger("Whenever this creature attacks and isn't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: thisCreature,
    });
    assert.deepEqual(trigger("Whenever enchanted creature attacks and isn't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: { ref: "enchanted", noun: "creature" },
    });
    assert.deepEqual(trigger("Whenever a Rogue you control attacks and isn't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: a({ subtypes: ["Rogue"], control: "you" }),
    });
    assert.deepEqual(trigger("Whenever this creature attacks"), {
      trigger: "attacks", what: thisCreature, alone: undefined, whom: undefined,
    });
  });

  it("keeps the attacked player and a collective subject", () => {
    // Master of Cruelties; Coveted Jewel.
    assert.deepEqual(trigger("Whenever this creature attacks a player and isn't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: thisCreature, whom: [{ player: "a-player" }],
    });
    assert.deepEqual(trigger("Whenever one or more creatures an opponent controls attack you and aren't blocked"), {
      trigger: "attacks-and-isnt-blocked", what: a({ types: ["creature"], control: "opponent" }), whom: [you],
      grouping: "one-or-more",
    });
  });

  it("rejects a combat scope, 'alone' and a dangling conjunction", () => {
    unsupported("Whenever a creature attacks and isn't blocked this combat"); // Melee
    unsupported("Whenever this creature attacks alone and isn't blocked");
    unsupported("Whenever this creature attacks and isn't");
    unsupported("Whenever this creature attacks and is blocked");
  });
});

describe("plural phase triggers (CR 500.8, 505.1a)", () => {
  it("reads 'each of <player>'s <phases>' as that player's every such phase", () => {
    // Neheb, the Eternal; Carpet of Flowers; Shadow of the Second Sun; Ertai's Meddling.
    assert.deepEqual(trigger("At the beginning of each of your postcombat main phases"), {
      trigger: "phase", phase: "postcombat-main", whose: "your",
    });
    assert.deepEqual(trigger("At the beginning of each of your postcombat main phases"),
      trigger("At the beginning of your postcombat main phase"));
    assert.deepEqual(trigger("At the beginning of each of your main phases"), {
      trigger: "phase", phase: "main", whose: "your",
    });
    assert.deepEqual(trigger("At the beginning of each of enchanted player's postcombat main phases"), {
      trigger: "phase", phase: "postcombat-main", whose: { player: "enchanted-player" },
    });
    assert.deepEqual(trigger("At the beginning of each of that player's upkeeps"),
      trigger("At the beginning of that player's upkeep"));
  });

  it("rejects a singular phase, a missing owner and a quantified owner", () => {
    unsupported("At the beginning of each of your upkeep");
    unsupported("At the beginning of each of the upkeeps");
    unsupported("At the beginning of each of each player's upkeeps");
    unsupported("At the beginning of your upkeeps");
  });
});

describe("events caused by a spell or ability (CR 609.1)", () => {
  const opposing: ObjectFilter = { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }], control: "opponent" };

  it("keeps the causing spell or ability on the ordinary player event", () => {
    // Guerrilla Tactics; Spiritual Focus; Firesong and Sunspeaker.
    assert.deepEqual(trigger("When a spell or ability an opponent controls causes you to discard this card"), {
      trigger: "discards", who: you, what: { ref: "this", noun: "card" }, causedBy: opposing,
    });
    assert.deepEqual(trigger("Whenever a spell or ability an opponent controls causes you to discard a card"), {
      trigger: "discards", who: you, causedBy: opposing,
    });
    assert.deepEqual(trigger("Whenever a white instant or sorcery spell causes you to gain life"), {
      trigger: "gains-life", who: you,
      causedBy: { colors: ["white"], allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" },
    });
    // The uncaused event is a different, broader trigger.
    assert.deepEqual(trigger("When you discard this card"), {
      trigger: "discards", who: you, what: { ref: "this", noun: "card" },
    });
  });

  it("rejects causes that are not spells or abilities, other events and scoped wording", () => {
    unsupported("Whenever a creature causes you to discard a card");
    unsupported("Whenever a spell or ability an opponent controls causes you to draw a card");
    // Pure Intentions: a collective discard ("return those cards") in a delayed trigger.
    unsupported("Whenever a spell or ability an opponent controls causes you to discard cards this turn", "Instant");
    unsupported("Whenever a spell or ability an opponent controls causes you to discard cards");
    unsupported("Whenever a spell or ability an opponent controls causes you to");
    unsupported("Whenever a spell or ability an opponent controls causes target player to discard a card");
  });
});

describe("haunt (CR 702.55b)", () => {
  it("names the object this card's haunt ability targeted", () => {
    // Absolver Thrull; Cry of Contrition.
    assert.deepEqual(trigger("When this creature enters or the creature it haunts dies"), {
      trigger: "any-of", events: [
        { trigger: "enters", what: thisCreature },
        { trigger: "dies", what: { ref: "haunted-creature" } },
      ],
    });
    assert.deepEqual(trigger("When the creature this card haunts dies"), {
      trigger: "dies", what: { ref: "haunted-creature" },
    });
    // A description of creatures is a different subject.
    assert.deepEqual(trigger("When a creature dies"), { trigger: "dies", what: a({ types: ["creature"] }) });
  });

  it("rejects other haunting sources and incomplete phrases", () => {
    unsupported("When the creature this creature haunts dies");
    unsupported("When the creature it haunts");
    unsupported("When a creature it haunts dies");
    unsupported("When the creature haunts dies");
  });
});

describe("excluded origin zones (CR 601.2a, 603.6a)", () => {
  it("keeps 'from anywhere other than' as one excluded origin", () => {
    // Ghostly Pilferer; Rory Williams; The Lost and the Damned.
    assert.deepEqual(trigger("Whenever an opponent casts a spell from anywhere other than their hand"), {
      trigger: "cast", who: { player: "an-opponent" }, what: { cls: "spell" }, notFrom: { zone: "hand", owner: "their" },
    });
    assert.deepEqual(trigger("When you cast this spell from anywhere other than exile"), {
      trigger: "cast", who: you, what: { ref: "this", noun: "spell" }, notFrom: { zone: "exile", owner: undefined },
    });
    assert.deepEqual(trigger("Whenever a land you control enters from anywhere other than your hand or you cast a spell from anywhere other than your hand"), {
      trigger: "any-of", events: [
        { trigger: "enters", what: a({ types: ["land"], control: "you" }), notFrom: { zone: "hand", owner: "your" } },
        { trigger: "cast", who: you, what: { cls: "spell" }, notFrom: { zone: "hand", owner: "your" } },
      ],
    });
    // The positive origin stays a different field.
    assert.deepEqual(trigger("Whenever you cast a spell from your hand"), {
      trigger: "cast", who: you, what: { cls: "spell" }, from: { zone: "hand", owner: "your" },
    });
  });

  it("rejects copies, zone lists and an ambiguous shared scope", () => {
    unsupported("Whenever you cast or copy a spell from anywhere other than your hand");
    unsupported("Whenever you cast a spell from anywhere other than your hand or graveyard");
    unsupported("Whenever you cast a spell from anywhere other than");
    // Shadow of the Goblin: the origin may qualify the land play too.
    unsupported("Whenever you play a land or cast a spell from anywhere other than your hand");
  });
});

describe("player keyword-action and game events", () => {
  const opponent = { player: "an-opponent" } as const;

  it("names the completed action and its player (CR 701.34a, 309.7, 725.3, 104.3, 701.54d)", () => {
    // Ezuri, Stalker of Spheres; Varis, Silverymoon Ranger; Knights of the Black Rose; Blood Tyrant; Nazgûl.
    assert.deepEqual(trigger("Whenever you proliferate"), { trigger: "proliferates", who: you });
    assert.deepEqual(trigger("Whenever you complete a dungeon"), { trigger: "completes-dungeon", who: you });
    assert.deepEqual(trigger("Whenever an opponent becomes the monarch"), { trigger: "becomes-monarch", who: opponent });
    assert.deepEqual(trigger("Whenever you become the monarch"), { trigger: "becomes-monarch", who: you });
    assert.deepEqual(trigger("Whenever a player loses the game"), { trigger: "loses-game", who: { player: "a-player" } });
    assert.deepEqual(trigger("When enchanted player loses the game"), { trigger: "loses-game", who: { player: "enchanted-player" } });
    assert.deepEqual(trigger("Whenever the Ring tempts you"), { trigger: "ring-tempts", who: you });
    // Losing life is a different event from losing the game.
    assert.deepEqual(trigger("Whenever a player loses life"), { trigger: "loses-life", who: { player: "a-player" } });
  });

  it("limits library events to the acting player's own library (CR 701.23a, 701.24a)", () => {
    // Archivist of Oghma; Search Elemental; Cosi's Trickster.
    assert.deepEqual(trigger("Whenever an opponent searches their library"), { trigger: "searches-library", who: opponent });
    assert.deepEqual(trigger("Whenever you search your library"), { trigger: "searches-library", who: you });
    assert.deepEqual(trigger("Whenever an opponent shuffles their library"), { trigger: "shuffles-library", who: opponent });
    unsupported("Whenever you search their library");
    unsupported("Whenever an opponent searches your library");
    unsupported("Whenever an opponent shuffles their graveyard");
  });

  it("separates clashing from winning a clash and coin-flip results (CR 701.30d, 705.2)", () => {
    // Entangling Trap; Marvo, Deep Operative; Sylvan Echoes; Zndrsplt, Eye of Wisdom; Karplusan Minotaur.
    assert.deepEqual(trigger("Whenever you clash"), { trigger: "clashes", who: you });
    assert.deepEqual(trigger("Whenever you win a clash"), { trigger: "clashes", who: you, won: true });
    assert.deepEqual(trigger("Whenever you clash and win"), trigger("Whenever you win a clash"));
    assert.deepEqual(trigger("Whenever a player wins a coin flip"), { trigger: "coin-flip", who: { player: "a-player" }, result: "win" });
    assert.deepEqual(trigger("Whenever you lose a coin flip"), { trigger: "coin-flip", who: you, result: "lose" });
    unsupported("Whenever you lose a clash");
    unsupported("Whenever you flip a coin");
    unsupported("Whenever you win the game");
  });

  it("keeps the exerted permanent's description (CR 701.43a)", () => {
    // Vizier of the True.
    assert.deepEqual(trigger("Whenever you exert a creature"), { trigger: "exerts", who: you, what: a({ types: ["creature"] }) });
    unsupported("Whenever you exert");
    unsupported("Whenever you exert target creature");
  });

  it("rejects targeted and collective players and other Ring subjects", () => {
    unsupported("Whenever target player loses the game");
    unsupported("Whenever each opponent loses the game");
    unsupported("Whenever the Ring tempts an opponent");
    unsupported("Whenever the Ring tempts");
  });
});

describe("keyword-action events of permanents", () => {
  const creatureYouControl = a({ types: ["creature"], control: "you" });

  it("names the completed action (CR 701.44b, 701.50, 701.14, 702.100b, 702.149c)", () => {
    // Wildgrowth Walker; Glorious Purpose; Foe-Razer Regent; Renegade Krasis; Savior of Ollenbock.
    assert.deepEqual(trigger("Whenever a creature you control explores"), { trigger: "explores", what: creatureYouControl });
    assert.deepEqual(trigger("Whenever a creature you control connives"), { trigger: "connives", what: creatureYouControl });
    assert.deepEqual(trigger("Whenever a creature you control fights"), { trigger: "fights", what: creatureYouControl });
    assert.deepEqual(trigger("Whenever this creature evolves"), { trigger: "evolves", what: thisCreature });
    assert.deepEqual(trigger("Whenever this creature trains"), { trigger: "trains", what: thisCreature });
    // Nicanzil, Current Conductor: the explored card is a further restriction.
    unsupported("Whenever a creature you control explores a land card");
  });

  it("keeps phasing in and out apart from zone changes (CR 702.26b–d)", () => {
    // Warping Wurm; Ertai's Familiar; King of the Oathbreakers; The War Doctor.
    assert.deepEqual(trigger("Whenever this creature phases in"), { trigger: "phases-in", what: thisCreature });
    assert.deepEqual(trigger("When this creature phases out or leaves the battlefield"), {
      trigger: "any-of", events: [{ trigger: "phases-out", what: thisCreature }, { trigger: "leaves", what: thisCreature }],
    });
    assert.deepEqual(trigger("Whenever ~ or another Spirit you control phases in"), {
      trigger: "any-of", events: [
        { trigger: "phases-in", what: { ref: "self" } },
        { trigger: "phases-in", what: a({ other: true, subtypes: ["Spirit"], control: "you" }) },
      ],
    });
    assert.deepEqual(trigger("Whenever one or more other permanents phase out"), {
      trigger: "phases-out", what: a({ other: true, cls: "permanent" }), grouping: "one-or-more",
    });
    unsupported("Whenever this creature phases");
  });

  it("keeps the crewed, saddled, enlisted or mentored permanent (CR 702.122b, 702.171c, 702.154c, 702.134c)", () => {
    // Gearshift Ace; Guardian of New Benalia; Aegis of the Legion.
    assert.deepEqual(trigger("Whenever this creature crews a Vehicle"), {
      trigger: "crews", what: thisCreature, recipient: a({ subtypes: ["Vehicle"] }),
    });
    assert.deepEqual(trigger("Whenever this creature saddles a Mount"), {
      trigger: "saddles", what: thisCreature, recipient: a({ subtypes: ["Mount"] }),
    });
    assert.deepEqual(trigger("Whenever this creature enlists a creature"), {
      trigger: "enlists", what: thisCreature, recipient: a({ types: ["creature"] }),
    });
    assert.deepEqual(trigger("Whenever equipped creature mentors a creature"), {
      trigger: "mentors", what: { ref: "equipped", noun: "creature" }, recipient: a({ types: ["creature"] }),
    });
    unsupported("Whenever this creature crews a creature");
    unsupported("Whenever this creature crews");
    // Canyon Vaulter's main-phase window is asserted with the phase-window suite below.
    unsupported("Whenever this creature saddles a Mount or crews a Vehicle during your");
  });

  it("treats becoming crewed, saddled or plotted as transitions (CR 702.122e, 702.171b, 702.170a)", () => {
    // Mobilizer Mech; Mindlink Mech; Stubborn Burrowfiend; Aloe Alchemist.
    const vehicle: ObjectRef = { ref: "this", noun: "vehicle" };
    assert.deepEqual(trigger("Whenever this Vehicle becomes crewed"), { trigger: "becomes-status", what: vehicle, status: "crewed" });
    assert.deepEqual(trigger("Whenever this Vehicle becomes crewed for the first time each turn"), {
      trigger: "becomes-status", what: vehicle, status: "crewed", occurrence: { ordinal: 1, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever this creature becomes saddled for the first time each turn"), {
      trigger: "becomes-status", what: thisCreature, status: "saddled", occurrence: { ordinal: 1, period: "turn" },
    });
    assert.deepEqual(trigger("When this card becomes plotted"), {
      trigger: "becomes-status", what: { ref: "this", noun: "card" }, status: "plotted",
    });
    unsupported("Whenever this Vehicle becomes crewed by a Pilot");
  });
});

describe("attachment events (CR 701.3a, 603.10c)", () => {
  const equipment: ObjectRef = { ref: "this", noun: "equipment" };

  it("keeps both permanents and the direction", () => {
    // Enormous Energy Blade; Grafted Wargear; Bramble Elemental; Siona, Captain of the Pyleas.
    assert.deepEqual(trigger("Whenever this Equipment becomes attached to a creature"), {
      trigger: "becomes-attached", what: equipment, to: a({ types: ["creature"] }),
    });
    assert.deepEqual(trigger("Whenever this Equipment becomes unattached from a permanent"), {
      trigger: "becomes-unattached", what: equipment, from: a({ cls: "permanent" }),
    });
    assert.deepEqual(trigger("Whenever an Aura becomes attached to this creature"), {
      trigger: "becomes-attached", what: a({ subtypes: ["Aura"] }), to: thisCreature,
    });
    assert.deepEqual(trigger("Whenever an Aura you control becomes attached to a creature you control"), {
      trigger: "becomes-attached", what: a({ subtypes: ["Aura"], control: "you" }), to: a({ types: ["creature"], control: "you" }),
    });
  });

  it("rejects a missing partner and a targeted partner", () => {
    unsupported("Whenever this Equipment becomes unattached");
    unsupported("Whenever this Equipment becomes attached to target creature");
    unsupported("Whenever this Equipment becomes attached");
  });
});

describe("a player becoming a target (CR 115.1)", () => {
  const opposing: ObjectFilter = { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }], control: "opponent" };

  it("is the player counterpart of becomes-target", () => {
    // Amulet of Safekeeping; Dormant Gomazoa.
    assert.deepEqual(trigger("Whenever you become the target of a spell or ability an opponent controls"), {
      trigger: "player-becomes-target", who: you, of: opposing,
    });
    assert.deepEqual(trigger("Whenever you become the target of a spell"), {
      trigger: "player-becomes-target", who: you, of: { cls: "spell" },
    });
  });

  it("keeps the player and the permanent as separate occurrences", () => {
    // Leovold, Emissary of Trest; Reidane, God of the Worthy.
    assert.deepEqual(trigger("Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls"), {
      trigger: "any-of", events: [
        { trigger: "player-becomes-target", who: you, of: opposing },
        { trigger: "becomes-target", what: a({ cls: "permanent", control: "you" }), of: opposing },
      ],
    });
    assert.deepEqual(trigger("Whenever you or another permanent you control becomes the target of a spell or ability an opponent controls"), {
      trigger: "any-of", events: [
        { trigger: "player-becomes-target", who: you, of: opposing },
        { trigger: "becomes-target", what: a({ other: true, cls: "permanent", control: "you" }), of: opposing },
      ],
    });
    // Leyline of Combustion: one occurrence for the whole set of targets.
    unsupported("Whenever you and/or at least one permanent you control becomes the target of a spell or ability an opponent controls");
    unsupported("Whenever you become the target");
    unsupported("Whenever you or a permanent you control becomes the target of");
  });
});

describe("draw exception, energy, token creation and milling", () => {
  const opponent = { player: "an-opponent" } as const;

  it("excludes the first draw of each of that player's draw steps", () => {
    // Orcish Bowmasters; Xyris, the Writhing Storm.
    assert.deepEqual(trigger("Whenever an opponent draws a card except the first one they draw in each of their draw steps"), {
      trigger: "draws", who: opponent, except: "first-in-each-draw-step",
    });
    assert.deepEqual(trigger("Whenever you draw a card except the first one you draw in each of your draw steps"), {
      trigger: "draws", who: you, except: "first-in-each-draw-step",
    });
    assert.deepEqual(trigger("Whenever an opponent draws a card"), { trigger: "draws", who: opponent });
    unsupported("Whenever you draw a card except the first one they draw in each of their draw steps");
    unsupported("Whenever an opponent draws a card except the first one they draw in each of your draw steps");
    unsupported("Whenever an opponent draws a card except the first one");
  });

  it("reads getting {E} as energy counters put on the player (CR 107.14, 122.1)", () => {
    // Aether Revolt; Brotherhood Scribe.
    assert.deepEqual(trigger("Whenever you get one or more {E}"), {
      trigger: "counter-change", direction: "put", counter: { type: "energy" },
      on: { recipient: "player", who: you }, match: { counters: "one-or-more", recipients: "each" },
    });
    assert.deepEqual(trigger("Whenever you get one or more {E} during your turn"), {
      trigger: "counter-change", direction: "put", counter: { type: "energy" },
      on: { recipient: "player", who: you }, match: { counters: "one-or-more", recipients: "each" }, during: "your",
    });
    unsupported("Whenever you get {E}");
    unsupported("Whenever you get one or more {G}");
  });

  it("keeps the created token's description and grouping (CR 701.7a)", () => {
    // Rosie Cotton of South Lane; Staff of the Storyteller; Akim, the Soaring Wind; Mirkwood Bats.
    assert.deepEqual(trigger("Whenever you create a token"), { trigger: "creates-token", who: you, what: a({ cls: "token" }) });
    assert.deepEqual(trigger("Whenever you create one or more creature tokens"), {
      trigger: "creates-token", who: you, what: a({ types: ["creature"], cls: "token" }), grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever you create one or more tokens for the first time each turn"), {
      trigger: "creates-token", who: you, what: a({ cls: "token" }), grouping: "one-or-more",
      occurrence: { ordinal: 1, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever you create or sacrifice a token"), {
      trigger: "any-of", events: [
        { trigger: "creates-token", who: you, what: a({ cls: "token" }) },
        { trigger: "sacrifices", who: you, what: { cls: "token" } },
      ],
    });
    unsupported("Whenever you create a creature");
    unsupported("Whenever you create or sacrifice one or more tokens");
  });

  it("separates a named milling player from cards milled by anyone (CR 701.17a)", () => {
    // Glowing One; Zellix, Sanity Flayer; Mirelurk Queen; Lo and Li, Royal Advisors.
    const nonland = a({ nonTypes: ["land"], cls: "card" });
    assert.deepEqual(trigger("Whenever a player mills a nonland card"), { trigger: "mills", who: { player: "a-player" }, what: nonland });
    assert.deepEqual(trigger("Whenever a player mills one or more creature cards"), {
      trigger: "mills", who: { player: "a-player" }, what: a({ types: ["creature"], cls: "card" }), grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more nonland cards are milled"), {
      trigger: "mills", what: nonland, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever an opponent discards a card or mills one or more cards"), {
      trigger: "any-of", events: [
        { trigger: "discards", who: opponent },
        { trigger: "mills", who: opponent, what: a({ cls: "card" }), grouping: "one-or-more" },
      ],
    });
    unsupported("Whenever you mill a creature");
    unsupported("Whenever one or more creatures are milled");
  });
});

describe("day and night (CR 731.1a)", () => {
  it("names the designation the game changes to", () => {
    // Firmament Sage; The Weekly Princess.
    assert.deepEqual(trigger("Whenever day becomes night or night becomes day"), {
      trigger: "any-of", events: [{ trigger: "day-night-changes", to: "night" }, { trigger: "day-night-changes", to: "day" }],
    });
    assert.deepEqual(trigger("Whenever night becomes day"), { trigger: "day-night-changes", to: "day" });
    assert.deepEqual(trigger("Whenever day becomes night"), { trigger: "day-night-changes", to: "night" });
  });

  it("rejects becoming day from neither and incomplete pairs", () => {
    unsupported("Whenever it becomes day");
    unsupported("Whenever day becomes night or night becomes");
    unsupported("Whenever day becomes day");
  });
});

describe("further wordings of existing events", () => {
  const creature = a({ types: ["creature"] });

  it("reads passive exile and return as the zone moves they are (CR 603.6c)", () => {
    // Soulherder; Azorius Aethermage; Stormfront Riders.
    assert.equal(JSON.stringify(trigger("Whenever a creature is exiled from the battlefield")),
      JSON.stringify(trigger("Whenever a creature is put into exile from the battlefield")));
    assert.deepEqual(trigger("Whenever a creature is exiled from the battlefield"), {
      trigger: "put-into-zone", what: creature, from: { zone: "battlefield" }, to: { zone: "exile" },
    });
    assert.deepEqual(trigger("Whenever a permanent is returned to your hand"), {
      trigger: "put-into-zone", what: a({ cls: "permanent" }), to: { zone: "hand", owner: "your" },
    });
    assert.deepEqual(trigger("Whenever this creature or another creature is returned to your hand from the battlefield"), {
      trigger: "any-of", events: [
        { trigger: "put-into-zone", what: thisCreature, from: { zone: "battlefield" }, to: { zone: "hand", owner: "your" } },
        { trigger: "put-into-zone", what: a({ other: true, types: ["creature"] }), from: { zone: "battlefield" }, to: { zone: "hand", owner: "your" } },
      ],
    });
    unsupported("Whenever a creature is exiled");
    unsupported("Whenever one or more noncreature permanents are returned to hand"); // Tameshi: whose hand is unstated
    unsupported("Whenever a permanent is returned to your graveyard");
  });

  it("reads 'damage is dealt to <player>' as the player-dealt-damage event (CR 120.3)", () => {
    // Risona, Asari Commander; Vengeful Pharaoh.
    assert.deepEqual(trigger("Whenever combat damage is dealt to you"), trigger("Whenever you're dealt combat damage"));
    assert.deepEqual(trigger("Whenever combat damage is dealt to you or a planeswalker you control"), {
      trigger: "any-of", events: [
        { trigger: "player-dealt-damage", who: you, combat: true },
        { trigger: "is-dealt-damage", what: a({ types: ["planeswalker"], control: "you" }), combat: true },
      ],
    });
    unsupported("Whenever combat damage is dealt to target player");
    unsupported("Whenever combat damage is dealt");
  });

  it("counts 'in a turn' like 'each turn' and joins coordinated verbs", () => {
    // Vance's Blasting Cannons; Sneaky Snacker; Wax-Wane Witness; Drownyard Lurker.
    assert.deepEqual(trigger("Whenever you cast your third spell in a turn"), trigger("Whenever you cast your third spell each turn"));
    assert.deepEqual(trigger("When you draw your third card in a turn"), {
      trigger: "draws", who: you, occurrence: { ordinal: 3, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever you gain or lose life during your turn"), {
      trigger: "any-of", events: [{ trigger: "gains-life", who: you }, { trigger: "loses-life", who: you }], during: "your",
    });
    assert.deepEqual(trigger("When you cast or cycle ~"), {
      trigger: "any-of", events: [
        { trigger: "cast", who: you, what: { ref: "self" } }, { trigger: "cycle", who: you, what: { ref: "self" } },
      ],
    });
    unsupported("When you cast or cycle a spell");
    unsupported("Whenever you gain or lose");
    unsupported("Whenever you cast your third spell in a game");
  });
});

describe("collective status changes (CR 603.2c)", () => {
  const yours = a({ types: ["creature"], control: "you" });

  it("groups plural 'become' events of one-or-more subjects", () => {
    // Rewrite History; Hezrou; Professor Hojo.
    assert.deepEqual(trigger("Whenever one or more creatures you control become tapped"), {
      trigger: "becomes-tapped", what: yours, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more creatures you control become blocked"), {
      trigger: "becomes-blocked", what: yours, by: undefined, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more creatures you control become the target of an activated ability"), {
      trigger: "becomes-target", what: yours, of: { cls: "ability", abilityKind: "activated" }, grouping: "one-or-more",
    });
    // The singular event stays ungrouped.
    assert.deepEqual(trigger("Whenever a creature you control becomes tapped"), { trigger: "becomes-tapped", what: yours });
    unsupported("Whenever one or more creatures you control become");
    unsupported("Whenever one or more creatures you control become tapped or");
  });
});

describe("sized attack groups and collective damage recipients (CR 508.1, 603.2c)", () => {
  const yours = a({ types: ["creature"], control: "you" });

  it("keeps the minimum size of a group of attackers", () => {
    // Landroval, Horizon Witness; Argent Dais; Inniaz, the Gale Force.
    assert.deepEqual(trigger("Whenever two or more creatures you control attack a player"), {
      trigger: "attacks", what: yours, alone: undefined, whom: [{ player: "a-player" }],
      grouping: "one-or-more", groupSize: { op: "ge", value: n(2) },
    });
    assert.deepEqual(trigger("Whenever two or more creatures attack"), {
      trigger: "attacks", what: a({ types: ["creature"] }), alone: undefined, whom: undefined,
      grouping: "one-or-more", groupSize: { op: "ge", value: n(2) },
    });
    assert.deepEqual(trigger("Whenever three or more creatures you control with flying attack"), {
      trigger: "attacks", what: a({ types: ["creature"], control: "you", withAbility: ["flying"] }), alone: undefined, whom: undefined,
      grouping: "one-or-more", groupSize: { op: "ge", value: n(3) },
    });
    // "one or more" has no size.
    assert.deepEqual(trigger("Whenever one or more creatures you control attack a player"), {
      trigger: "attacks", what: yours, alone: undefined, whom: [{ player: "a-player" }], grouping: "one-or-more",
    });
    unsupported("Whenever two or more creatures you control die");
    unsupported("Whenever two or more creatures you control attack or block");
    unsupported("Whenever two or more attack");
  });

  it("groups the damaged recipients of one event", () => {
    // Contaminant Grafter; Briarbridge Patrol; Hordewing Skaab.
    assert.deepEqual(trigger("Whenever one or more creatures you control deal combat damage to one or more players"), {
      trigger: "deals-damage", what: yours, combat: true, to: "any", recipients: "one-or-more", grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever this creature deals damage to one or more creatures"), {
      trigger: "deals-damage", what: thisCreature, combat: undefined, to: a({ types: ["creature"] }), recipients: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever one or more Zombies you control deal combat damage to one or more of your opponents"), {
      trigger: "deals-damage", what: a({ subtypes: ["Zombie"], control: "you" }), combat: true,
      to: { player: "an-opponent" }, recipients: "one-or-more", grouping: "one-or-more",
    });
    // Per damaged player: a different trigger.
    assert.deepEqual(trigger("Whenever one or more creatures you control deal combat damage to a player"), {
      trigger: "deals-damage", what: yours, combat: true, to: "any", grouping: "one-or-more",
    });
    unsupported("Whenever this creature deals damage to one or more");
    unsupported("Whenever one or more Pirates you control deal damage to your opponents"); // Breeches: bare plural
  });
});

describe("events within a phase or step (CR 500.1)", () => {
  it("keeps the phase window apart from the turn restriction", () => {
    // Mongrel Pack; Dovin's Acuity; Canyon Vaulter; Portal Mage.
    assert.deepEqual(trigger("When this creature dies during combat"), {
      trigger: "dies", what: thisCreature, inPhase: { phase: "combat" },
    });
    assert.deepEqual(trigger("Whenever you cast an instant spell during your main phase"), {
      trigger: "cast", who: you, what: { types: ["instant"], cls: "spell" }, inPhase: { phase: "main", whose: "your" },
    });
    assert.deepEqual(trigger("Whenever this creature saddles a Mount or crews a Vehicle during your main phase"), {
      trigger: "any-of", events: [
        { trigger: "saddles", what: thisCreature, recipient: a({ subtypes: ["Mount"] }) },
        { trigger: "crews", what: thisCreature, recipient: a({ subtypes: ["Vehicle"] }) },
      ], inPhase: { phase: "main", whose: "your" },
    });
    assert.deepEqual(trigger("When this creature enters during the declare attackers step"), {
      trigger: "enters", what: thisCreature, inPhase: { phase: "declare-attackers" },
    });
    assert.deepEqual(trigger("Whenever you cast a spell during your turn"), {
      trigger: "cast", who: you, what: { cls: "spell" }, during: "your",
    });
  });

  it("rejects an unstated phase and an ambiguous scope over separate alternatives", () => {
    unsupported("Whenever you cast a spell during");
    unsupported("Whenever you play a land or cast a spell during combat");
    unsupported("Whenever you cast a spell during their turn");
  });
});
