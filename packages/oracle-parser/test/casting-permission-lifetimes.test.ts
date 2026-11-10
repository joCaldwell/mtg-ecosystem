import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 601.3, 609.4,
// 614.1c/614.12, 702.103a, 702.140a, 702.152a, 702.185a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Enchantment"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const you = { player: "you" } as const;
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const thisCard = { ref: "this", noun: "card" } as const;
const yourGraveyard = { zone: "graveyard", owner: "your" } as const;

describe("permissions every player has", () => {
  it("gives 'any player may' to each player, with cost and timing parts", () => {
    // Tidal Barracuda; Aluren.
    assert.deepEqual(ability("Any player may cast spells as though they had flash.", "Creature — Fish"), {
      kind: "static", effect: { static: "play-permission", permission: { who: { player: "each-player" }, action: "cast",
        what: { ref: "each", filter: { cls: "spell" } }, timing: "as-though-flash" } },
    });
    assert.deepEqual(ability("Any player may cast creature spells with mana value 3 or less without paying their mana costs and as though they had flash."), {
      kind: "static", effect: { static: "play-permission", permission: { who: { player: "each-player" }, action: "cast",
        what: { ref: "each", filter: { types: ["creature"], cls: "spell",
          properties: [{ property: "mana-value", comparison: { op: "le", value: fixed(3) } }] } },
        alternativeCost: { cost: "without-mana-cost" }, timing: "as-though-flash" } },
    });
  });

  it("does not read other quantified players as that permission", () => {
    fails("Any opponent may cast spells as though they had flash.");
    fails("Any player may cast spells.");
  });
});

describe("casting a card with one of its own keyword abilities", () => {
  it("records the keyword that supplies the alternative cost", () => {
    // Tenacious Underdog; Brokkos, Apex of Forever; Detective's Phoenix; Timeline Culler.
    for (const keyword of ["blitz", "mutate", "bestow", "warp"]) {
      assert.deepEqual(ability(`You may cast this card from your graveyard using its ${keyword} ability.`, "Creature — Human Warrior"), {
        kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast", what: thisCard, from: yourGraveyard, usingKeyword: keyword } },
      });
    }
  });

  it("rejects other keywords, other cards and a second alternative cost", () => {
    fails("You may cast this card from your graveyard using its flying ability.", "Creature — Bird");
    fails("You may cast this card from your graveyard using its blitz.", "Creature — Human Warrior");
    fails("You may cast creature spells from your graveyard using its blitz ability.");
    fails("You may cast this card from your graveyard without paying its mana cost using its blitz ability.", "Creature — Human Warrior");
  });
});

describe("entry riders of a static permission for this card (CR 614.1c, 614.12)", () => {
  it("attaches 'If you do, it enters with …' to the permission", () => {
    // Worldheart Phoenix; Undead Sprinter.
    assert.deepEqual(ability("You may cast this card from your graveyard by paying {W}{U}{B}{R}{G} rather than paying its mana cost. If you do, it enters with two +1/+1 counters on it.", "Creature — Phoenix"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast", what: thisCard, from: yourGraveyard,
        alternativeCost: { cost: "pay", costs: [{ cost: "mana", symbols: ["W", "U", "B", "R", "G"] }] },
        riders: [{ rider: "entry", counters: [{ count: fixed(2), counter: { type: "+1/+1" } }] }] } },
    });
    const sprinter = ability("You may cast this card from your graveyard if a non-Zombie creature died this turn. If you do, this creature enters with a +1/+1 counter on it.", "Creature — Zombie");
    assert.equal(sprinter.kind, "static");
    const permission = sprinter.kind === "static" && sprinter.effect.static === "play-permission" ? sprinter.effect.permission : undefined;
    assert.deepEqual(permission?.riders, [{ rider: "entry", counters: [{ count: fixed(1), counter: { type: "+1/+1" } }] }]);
    assert.equal(permission?.condition?.condition, "zone-change-history");
  });

  it("rejects the rider when 'it' is not this card or the consequence is not an entry modifier", () => {
    fails("You may cast creature spells from your graveyard. If you do, it enters with a finality counter on it.");
    // The card cast this way gains the ability.
    assert.deepEqual(ability("You may cast this card from your graveyard. If you do, it gains haste.", "Creature — Zombie"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast", what: thisCard, from: yourGraveyard,
        riders: [{ rider: "gain-abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] } },
    });
    fails("You may cast this card from your graveyard. If you do, that artifact enters with a finality counter on it.", "Creature — Zombie");
    fails("You may cast this card from your graveyard. If you do, it enters with a finality counter on it and is a Vampire in addition to its other types.", "Creature — Zombie");
  });
});

describe("a permission for the next matching spell (CR 611.2a)", () => {
  const grant = (permission: object) => ({ effect: "grant-play-permission", permission, duration: { duration: "this-turn" } });

  it("applies to one spell, the next one, whatever it needed", () => {
    // Quicken; World War Hulk (chapter I).
    assert.deepEqual(ability("The next sorcery spell you cast this turn can be cast as though it had flash.", "Instant"), {
      kind: "spell", effects: [{ steps: [grant({ who: you, action: "cast", what: { ref: "each", filter: { types: ["sorcery"], cls: "spell" } },
        occurrence: { next: true }, timing: "as-though-flash" })] }],
    });
    assert.deepEqual(ability("I — The next red or green creature spell you cast this turn can be cast without paying its mana cost.", "Enchantment — Saga"), {
      kind: "chapter", chapters: [1], effects: [{ steps: [grant({ who: you, action: "cast",
        what: { ref: "each", filter: { allOf: [{ anyOf: [{ colors: ["red"] }, { colors: ["green"] }] }], types: ["creature"], cls: "spell" } },
        occurrence: { next: true }, alternativeCost: { cost: "without-mana-cost" } })] }],
    });
  });

  it("keeps a linked type choice wherever it is printed, and playing apart from casting", () => {
    // Progenitor's Icon; Eye of Ojer Taq; Scout's Warning.
    const chosen = { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell", chosenType: { choice: "chosen-type" } } }, occurrence: { next: true } };
    assert.deepEqual(ability("{T}: The next spell of the chosen type you cast this turn can be cast as though it had flash.", "Artifact"), {
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [grant({ ...chosen, timing: "as-though-flash" })] }],
    });
    assert.deepEqual(ability("{T}: The next spell you cast this turn of the chosen type can be cast without paying its mana cost.", "Artifact"), {
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [grant({ ...chosen, alternativeCost: { cost: "without-mana-cost" } })] }],
    });
    assert.deepEqual(ability("The next creature card you play this turn can be played as though it had flash.", "Instant"), {
      kind: "spell", effects: [{ steps: [grant({ who: you, action: "play", what: { ref: "each", filter: { types: ["creature"], cls: "card" } },
        occurrence: { next: true }, timing: "as-though-flash" })] }],
    });
  });

  it("rejects other periods, mismatched verbs and unrepresented consequences", () => {
    fails("The next sorcery spell you cast can be cast as though it had flash.", "Instant");
    fails("The next creature card you play this turn can be cast as though it had flash.", "Instant");
    fails("The next creature card you cast this turn can be cast as though it had flash.", "Instant");
    fails("The next sorcery spell you cast this turn can be cast as though it had haste.", "Instant");
    fails("The next sorcery spell an opponent casts this turn can be cast as though it had flash.", "Instant");
    // Savage Summoning's two riders are represented (asserted below with the
    // other permission riders); other consequences for that spell are not.
    fails("The next creature spell you cast this turn can be cast as though it had flash. That creature gains haste.", "Instant");
    fails("The next creature spell you cast this turn can be cast as though it had flash. That spell costs {1} less to cast.", "Instant");
  });
});

describe("cards a cast instruction left behind (CR 608.2c)", () => {
  const moved = (text: string, typeLine: string, sentence: number) => {
    const parsed = ability(text, typeLine);
    const effects = parsed.kind === "spell" || parsed.kind === "triggered" || parsed.kind === "activated" ? parsed.effects : [];
    return effects[sentence].steps;
  };

  it("narrows an event-result pool to the cards not cast this way", () => {
    // Collected Conjuring; Epic Experiment; Treasure Keeper.
    assert.deepEqual(moved("Exile the top six cards of your library. You may cast up to two sorcery spells with mana value 3 or less from among them without paying their mana costs. Put the exiled cards not cast this way on the bottom of your library in a random order.", "Sorcery", 2), [{
      effect: "move-zone", order: "random", to: { zone: "library", owner: "your", position: "bottom" },
      what: { ref: "related", number: "plural", filter: { cls: "card", notCastThisWay: true },
        relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    }]);
    assert.deepEqual(moved("Exile the top X cards of your library. You may cast instant and sorcery spells with mana value X or less from among them without paying their mana costs. Then put all cards exiled this way that weren't cast into your graveyard.", "Sorcery", 2), [{
      effect: "move-zone", to: { zone: "graveyard", owner: "your" },
      what: { ref: "all", filter: { cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" }, notCastThisWay: true } },
    }]);
    assert.deepEqual(moved("When this creature dies, reveal cards from the top of your library until you reveal a nonland card with mana value 3 or less. You may cast that card without paying its mana cost. Put all revealed cards not cast this way on the bottom of your library in a random order.", "Artifact Creature — Construct", 2), [{
      effect: "move-zone", order: "random", to: { zone: "library", owner: "your", position: "bottom" },
      what: { ref: "related", number: "plural", filter: { cls: "card", notCastThisWay: true },
        relation: { relation: "event-result", event: "reveal", scope: "contextual" } },
    }]);
  });

  it("keeps the acting player and a coordinated type list's plural number", () => {
    // Dazzling Sphinx; Muse Vortex.
    assert.deepEqual(moved("Whenever this creature deals combat damage to a player, that player exiles cards from the top of their library until they exile an instant or sorcery card. You may cast that card without paying its mana cost. Then that player puts the exiled cards that weren't cast this way on the bottom of their library in a random order.", "Creature — Sphinx", 2), [{
      effect: "move-zone", order: "random", who: { player: "that-player" }, to: { zone: "library", owner: "their", position: "bottom" },
      what: { ref: "related", number: "plural", filter: { cls: "card", notCastThisWay: true },
        relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    }]);
    const vortex = moved("Exile the top X cards of your library. You may cast an instant or sorcery spell with mana value X or less from among them without paying its mana cost. Then put the exiled instant and sorcery cards that weren't cast this way into your hand and the rest on the bottom of your library in a random order.", "Sorcery", 2);
    assert.deepEqual(vortex, [{ effect: "simultaneous", steps: [
      { effect: "move-zone", to: { zone: "hand", owner: "your" },
        what: { ref: "related", number: "plural",
          filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card", notCastThisWay: true },
          relation: { relation: "event-result", event: "exile", scope: "contextual" } } },
      { effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random" },
    ] }]);
  });

  it("does not turn cards currently in exile into an event result", () => {
    // Livio, Oathsworn Sentinel: "all exiled cards with aegis counters".
    // Now a state description (cards in exile with aegis counters), never an event result.
    assert.deepEqual(ability("{2}{W}, {T}: Return all exiled cards with aegis counters on them to the battlefield under their owners' control.", "Legendary Creature — Human Knight"),
      { kind: "activated", costs: [{ cost: "mana", symbols: ["2", "W"] }, { cost: "tap-self" }], effects: [{ steps: [{ effect: "move-zone",
        what: { ref: "all", filter: { zone: { zone: "exile" }, cls: "card",
          counterConstraints: [{ counter: { type: "aegis" }, comparison: { op: "ge", value: fixed(1) } }] } },
        to: { zone: "battlefield" }, controller: "owner" }] }] });
    fails("Put all revealed card not cast this way on the bottom of your library in a random order.", "Sorcery");
    fails("Put the exiled cards that weren't played this way on the bottom of your library in a random order.", "Sorcery");
  });
});

describe("permission lifetimes that end on an event (CR 611.2a)", () => {
  const steps = (text: string, typeLine: string, sentence: number) => {
    const parsed = ability(text, typeLine);
    const effects = parsed.kind === "spell" || parsed.kind === "triggered" || parsed.kind === "activated" ? parsed.effects : [];
    return effects[sentence].steps;
  };
  const thatCard = { ref: "that", filter: { cls: "card" } };

  it("ends a grant when this card is cast from exile", () => {
    // Emrakul, the Exigent Doom.
    const emrakul = steps("{3}, Exile this card from your hand: Target land gains \"{T}: Add {C}{C}\" until this card is cast from exile. You may cast this card for as long as it remains exiled.", "Legendary Creature — Eldrazi", 0);
    assert.deepEqual(emrakul[0].duration, { duration: "until-cast", what: thisCard, from: { zone: "exile" } });
    assert.equal(emrakul[0].effect, "gain-abilities");
  });

  it("ends a play permission when the same object exiles another card", () => {
    // Furious Rise.
    assert.deepEqual(steps("At the beginning of your end step, if you control a creature with power 4 or greater, exile the top card of your library. You may play that card until you exile another card with this enchantment.", "Enchantment", 1), [{
      effect: "grant-play-permission", permission: { who: you, action: "play", what: thatCard },
      duration: { duration: "until-next-linked-exile", who: you, source: { ref: "this", noun: "enchantment" } },
    }]);
  });

  it("reads 'until the beginning of your next upkeep' as the existing upkeep boundary", () => {
    // Elkin Bottle.
    assert.deepEqual(steps("{3}, {T}: Exile the top card of your library. Until the beginning of your next upkeep, you may play that card.", "Artifact", 1), [{
      effect: "grant-play-permission", permission: { who: you, action: "play", what: thatCard }, duration: { duration: "your-next-upkeep" },
    }]);
  });

  it("keeps 'during any turn you attacked with …' as a recurring condition of an unending permission", () => {
    // Neyali, Suns' Vanguard; Robber of the Rich.
    assert.deepEqual(steps("Whenever one or more tokens you control attack a player, exile the top card of your library. During any turn you attacked with a token, you may play that card.", "Legendary Creature — Human Rebel", 1), [{
      effect: "grant-play-permission", duration: { duration: "rest-of-game" },
      permission: { who: you, action: "play", what: thatCard,
        condition: { condition: "attacked", who: you, during: "this-turn", with: { cls: "token" } } },
    }]);
    assert.deepEqual(steps("Whenever this creature attacks, if defending player has more cards in hand than you, exile the top card of their library. During any turn you attacked with a Rogue, you may cast that card and you may spend mana as though it were mana of any color to cast that spell.", "Creature — Human Archer Rogue", 1), [{
      effect: "grant-play-permission", duration: { duration: "rest-of-game" },
      permission: { who: you, action: "cast", what: thatCard, manaPayment: "any-color",
        condition: { condition: "attacked", who: you, during: "this-turn", with: { subtypes: ["Rogue"] } } },
    }]);
  });

  it("rejects turn conditions and event ends it cannot state", () => {
    // Goblin Researcher: a specific attacker is not an attacked-with filter.
    fails("When this creature enters, exile the top card of your library. During any turn you attacked with this creature, you may play that card.", "Creature — Goblin Wizard");
    // Boros Strike-Captain: a count of attackers is represented by the attacked condition's `count`.
    assert.deepEqual(steps("Exile the top card of your library. During any turn you attacked with three or more creatures, you may play that card.", "Sorcery", 0), [{
      effect: "exile", what: { ref: "library-cards", position: "top", count: fixed(1), of: { zone: "library", owner: "your" } } }]);
    assert.deepEqual(steps("Exile the top card of your library. During any turn you attacked with three or more creatures, you may play that card.", "Sorcery", 1), [{
      effect: "grant-play-permission", duration: { duration: "rest-of-game" },
      permission: { who: you, action: "play", what: thatCard, condition: { condition: "attacked", who: you, during: "this-turn",
        with: { types: ["creature"] }, count: { op: "ge", value: fixed(3) } } } }]);
    fails("Exile the top card of your library. During any turn you gained life, you may play that card.", "Sorcery");
    fails("Exile the top card of your library. You may play that card until you exile another card.", "Sorcery");
    fails("Exile the top card of your library. You may play that card until an opponent exiles another card with this artifact.", "Artifact");
    fails("{3}: Target land gains \"{T}: Add {C}{C}\" until target creature is cast from exile.", "Artifact");
  });
});

describe("demonstrative event-result pools", () => {
  it("reads 'those exiled cards' as the same pool as 'the exiled cards'", () => {
    // Dream Pillager.
    const parsed = ability("Whenever this creature deals combat damage to a player, exile that many cards from the top of your library. Until end of turn, you may cast spells from among those exiled cards.", "Creature — Dragon");
    assert.deepEqual(parsed.kind === "triggered" && parsed.effects[1].steps, [{
      effect: "grant-play-permission", duration: { duration: "end-of-turn" },
      permission: { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } },
        fromAmong: { ref: "related", number: "plural", filter: { cls: "card" },
          relation: { relation: "event-result", event: "exile", scope: "contextual" } } },
    }]);
  });

  it("rejects a demonstrative whose number disagrees with its noun", () => {
    fails("Exile the top five cards of your library. Until end of turn, you may cast spells from among those exiled card.", "Sorcery");
    fails("Exile the top five cards of your library. Until end of turn, you may cast spells from among that exiled cards.", "Sorcery");
  });
});

describe("abilities of the spell cast with a permission (CR 601.2f, 608.2c)", () => {
  it("folds 'That spell has …' into the permission instead of granting to a spell on the stack", () => {
    // Durnan of the Yawning Portal (audit defect: formerly a separate gain-abilities step).
    const durnan = ability("Whenever Test Card attacks, look at the top four cards of your library. You may exile a creature card from among them. Put the rest on the bottom of your library in any order. For as long as that card remains exiled, you may cast it. That spell has undaunted.", "Legendary Creature — Human Warrior");
    assert.equal(durnan.kind, "triggered");
    const effects = durnan.kind === "triggered" ? durnan.effects : [];
    assert.equal(effects.length, 4);
    assert.deepEqual(effects[3].steps, [{
      effect: "grant-play-permission",
      permission: { who: you, action: "cast", what: { ref: "it" }, spellAbilities: [{ keyword: "undaunted" }] },
      duration: { duration: "as-long-as", condition: { condition: "remains", what: { ref: "that", filter: { cls: "card" } }, zone: { zone: "exile" } } },
    }]);
  });

  it("never accepts such a sentence as a free-standing grant", () => {
    // "gains", a plural permission, and an immediate cast are not folded.
    fails("Exile the top card of your library. You may cast it this turn. That spell gains haste.", "Sorcery");
    fails("Exile the top two cards of your library. You may cast spells from among them this turn. That spell has undaunted.", "Sorcery");
    fails("Exile the top two cards of your library. You may cast spells from among them this turn. Those spells have undaunted.", "Sorcery");
    fails("Exile the top card of your library. You may cast it. That spell has undaunted.", "Sorcery");
    fails("Exile the top card of your library. You may cast it this turn. That spell has undaunted. Draw a card.", "Sorcery");
  });
});

describe("'If you don't cast it' after an immediate optional cast (CR 608.2c)", () => {
  it("tests whether the offered card was cast, whatever kept it from being cast", () => {
    // Descendants' Path: the cast itself is conditional, so this is not the
    // declined-option test of a bare "If you don't".
    const path = ability("At the beginning of your upkeep, reveal the top card of your library. If it's a creature card that shares a creature type with a creature you control, you may cast it without paying its mana cost. If you don't cast it, put it on the bottom of your library.");
    assert.equal(path.kind, "triggered");
    const effects = path.kind === "triggered" ? path.effects : [];
    assert.equal(effects.length, 3);
    assert.deepEqual(effects[1].steps, [{ effect: "play-card", optional: true,
      instruction: { who: you, action: "cast", what: { ref: "it" }, alternativeCost: { cost: "without-mana-cost" } } }]);
    assert.deepEqual(effects[2], {
      condition: { condition: "not-cast", who: you, what: { ref: "it" }, scope: "preceding-instruction" }, conditionKind: "if",
      steps: [{ effect: "move-zone", what: { ref: "it" }, to: { zone: "library", owner: "your", position: "bottom" } }],
    });
  });

  it("keeps the bare 'If you don't' node and accepts 'this way' as the same scope", () => {
    const bare = ability("Reveal the top card of your library. You may cast it. If you don't, draw a card.", "Sorcery");
    assert.deepEqual(bare.kind === "spell" && bare.effects[2].condition, { condition: "you-dont" });
    const scoped = ability("Reveal the top card of your library. You may cast it. If you don't cast it this way, draw a card.", "Sorcery");
    assert.deepEqual(scoped.kind === "spell" && scoped.effects[2].condition,
      { condition: "not-cast", who: you, what: { ref: "it" }, scope: "preceding-instruction" });
  });

  it("rejects the wording without a directly preceding optional cast by you", () => {
    fails("You may draw a card. If you don't cast it, draw a card.", "Sorcery");
    fails("Reveal the top card of your library. You may play it. If you don't cast it, draw a card.", "Sorcery");
    fails("Reveal the top card of your library. Cast it. If you don't cast it, draw a card.", "Sorcery");
    fails("Reveal the top card of your library. You may cast it, then draw a card. If you don't cast it, draw a card.", "Sorcery");
    fails("Exile the top card of your library. You may cast it this turn. If you don't cast it, draw a card.", "Sorcery");
  });
});

describe("triggers and riders tied to using a permission (CR 603.1, 607.2)", () => {
  const steps = (text: string, typeLine: string, sentence: number) => {
    const parsed = ability(text, typeLine);
    const effects = parsed.kind === "spell" || parsed.kind === "triggered" || parsed.kind === "activated" ? parsed.effects : [];
    return effects[sentence].steps;
  };

  it("links 'When you play a card this way' to the lasting permission", () => {
    // Fires of Mount Doom.
    assert.deepEqual(steps("{2}{R}: Exile the top card of your library. You may play that card this turn. When you play a card this way, Test Card deals 2 damage to each player.", "Legendary Enchantment", 1), [{
      effect: "grant-play-permission", duration: { duration: "this-turn" },
      permission: { who: you, action: "play", what: { ref: "that", filter: { cls: "card" } }, riders: [{ rider: "use-trigger", event: "play",
        effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: fixed(2), to: { to: "player", ref: { player: "each-player" } } }] }] }] },
    }]);
  });

  it("distinguishes casting from playing and keeps the spell restriction", () => {
    // The Ruinous Powers (play permission, cast trigger); Galea, Kindler of Hope (static).
    const powers = steps("At the beginning of your upkeep, choose an opponent at random. Exile the top card of that player's library. Until end of turn, you may play that card and you may spend mana as though it were mana of any color to cast it. When you cast a spell this way, its owner loses life equal to its mana value.", "Enchantment", 2);
    assert.deepEqual(powers, [{ effect: "grant-play-permission", duration: { duration: "end-of-turn" },
      permission: { who: you, action: "play", what: { ref: "that", filter: { cls: "card" } }, manaPayment: "any-color",
        riders: [{ rider: "use-trigger", event: "cast", effects: [{ steps: [{ effect: "lose-life", who: { player: "owner", of: { ref: "it" } },
          amount: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }] }] }] } }]);
    const galea = ability("You may cast Aura and Equipment spells from the top of your library. When you cast an Equipment spell this way, it gains \"When this Equipment enters, attach it to target creature you control.\"", "Legendary Creature — Elf Knight");
    const permission = galea.kind === "static" && galea.effect.static === "play-permission" ? galea.effect.permission : undefined;
    assert.equal(permission?.riders?.length, 1);
    const rider = permission?.riders?.[0];
    assert.equal(rider?.rider, "use-trigger");
    assert.deepEqual(rider?.rider === "use-trigger" && { event: rider.event, appliesTo: rider.appliesTo, effect: rider.effects[0].steps[0].effect },
      { event: "cast", appliesTo: { subtypes: ["Equipment"], cls: "spell" }, effect: "gain-abilities" });
  });

  it("gives the next-spell permission its uncounterable and entry riders", () => {
    // Savage Summoning.
    assert.deepEqual(steps("The next creature spell you cast this turn can be cast as though it had flash. That spell can't be countered. That creature enters with an additional +1/+1 counter on it.", "Instant", 0), [{
      effect: "grant-play-permission", duration: { duration: "this-turn" },
      permission: { who: you, action: "cast", what: { ref: "each", filter: { types: ["creature"], cls: "spell" } }, occurrence: { next: true },
        timing: "as-though-flash", riders: [{ rider: "cant-be-countered" },
          { rider: "entry", counters: [{ count: fixed(1), counter: { type: "+1/+1" }, additional: true }] }] },
    }]);
  });

  it("rejects triggers and riders it cannot tie to the permission", () => {
    // A play trigger needs a play permission; a typed entry rider needs that type.
    fails("Exile the top card of your library. You may cast that card this turn. When you play a card this way, draw a card.", "Sorcery");
    fails("Exile the top card of your library. You may play that card this turn. When you play a land card this way, draw a card.", "Sorcery");
    fails("Exile the top card of your library. You may play that card this turn. When you play a card this way, draw a card. You gain 1 life.", "Sorcery");
    fails("The next sorcery spell you cast this turn can be cast as though it had flash. That creature enters with an additional +1/+1 counter on it.", "Instant");
    fails("The next creature spell you cast this turn can be cast as though it had flash. That spell can't be countered. That spell can't be countered.", "Instant");
    // After a lasting permission "that spell" is never a free-standing instruction.
    fails("Exile the top card of your library. You may cast it this turn. That spell can't be countered.", "Sorcery");
    // Apple of Eden: a separate "Until end of turn, whenever you play a land or cast a spell this way" ability.
    fails("Exile the top card of your library. You may play that card this turn. Until end of turn, whenever you play a land or cast a spell this way, its owner draws a card.", "Sorcery");
  });
});
