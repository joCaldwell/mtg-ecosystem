import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Sorcery"): ParsedLine {
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
const one = { amount: "fixed", value: 1 } as const;
const yourHand = { zone: "hand", owner: "your" } as const;
const free = { cost: "without-mana-cost" } as const;
const thatDamage = { amount: "event-quantity", event: "damage", measure: "dealt", scope: "contextual" } as const;
const atMostThatDamage = [{ property: "mana-value", comparison: { op: "le", value: thatDamage } }] as const;

describe("a cast permission's qualifier printed after its source zone", () => {
  it("restricts the same spells, keeping adjective alternatives on the one head noun", () => {
    // Glamdring.
    assert.deepEqual(ability("Whenever equipped creature deals combat damage to a player, you may cast an instant or sorcery spell from your hand with mana value less than or equal to that damage without paying its mana cost.", "Artifact — Equipment"), {
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "equipped", noun: "creature" }, combat: true, to: "any" },
      effects: [{ steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast",
        what: { ref: "filter", count: one, filter: { cls: "spell", allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }],
          properties: atMostThatDamage } },
        from: yourHand, alternativeCost: free } }] }],
    });
  });

  it("applies to a plain spell description", () => {
    // Aisha of Sparks and Smoke.
    assert.deepEqual(ability("Whenever this creature deals combat damage, you may cast a sorcery spell from your hand with mana value less than or equal to that damage without paying its mana cost.", "Creature — Human"), {
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true },
      effects: [{ steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast",
        what: { ref: "filter", count: one, filter: { types: ["sorcery"], cls: "spell", properties: atMostThatDamage } },
        from: yourHand, alternativeCost: free } }] }],
    });
  });

  it("rejects a damage quantity given a different scope", () => {
    fails("Whenever this creature deals combat damage, you may cast a sorcery spell from your hand with mana value less than or equal to that damage this turn without paying its mana cost.", "Creature — Human");
  });
});

describe("an inclusive comparison with an unstated standard (CR 202.3)", () => {
  it("keeps 'equal or lesser mana value' symbolic and inclusive", () => {
    // Reinterpret.
    assert.deepEqual(ability("Counter target spell. You may cast a spell with equal or lesser mana value from your hand without paying its mana cost.", "Instant"), {
      kind: "spell", effects: [
        { steps: [{ effect: "counter", what: { ref: "target", filter: { cls: "spell" }, count: one } }] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast",
          what: { ref: "filter", count: one, filter: { cls: "spell", implicitComparison: { attribute: "mana-value", op: "le" } } },
          from: yourHand, alternativeCost: free } }] },
      ],
    });
  });

  it("states the standard when 'than' names it, and keeps strict comparisons strict", () => {
    assert.deepEqual(ability("Destroy target creature with equal or greater power than this creature.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "target", count: one, filter: { types: ["creature"],
        properties: [{ property: "power", comparison: { op: "ge", value: { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" } } }] } } }] }],
    });
    assert.deepEqual(ability("Destroy target creature with lesser power.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "target", count: one,
        filter: { types: ["creature"], implicitComparison: { attribute: "power", op: "lt" } } } }] }],
    });
    fails("Destroy target creature with equal power.", "Instant");
  });
});

describe("a choice between playing a land and casting a spell", () => {
  const land = { ref: "filter", filter: { types: ["land"] }, count: one } as const;
  const spell = { ref: "filter", filter: { cls: "spell" }, count: one } as const;
  const exiledWithThis = { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "linked-exile", source: { ref: "this", noun: "permanent" } } } as const;
  const onceOnYourTurns = { maximum: 1, period: "turn", during: "your" } as const;

  it("shares one per-turn use between the actions and distributes the final source (static)", () => {
    // Hauken's Insight.
    assert.deepEqual(ability("Once during each of your turns, you may play a land or cast a spell from among the cards exiled with this permanent without paying its mana cost.", "Legendary Artifact"), {
      kind: "static", effect: { static: "play-permission-choice", useLimit: onceOnYourTurns, permissions: [
        { who: you, action: "play", what: land, fromAmong: exiledWithThis },
        { who: you, action: "cast", what: spell, fromAmong: exiledWithThis, alternativeCost: free },
      ] },
    });
  });

  it("keeps each action's own source and qualifiers", () => {
    const graveyard = { zone: "graveyard", owner: "your" } as const;
    assert.deepEqual(ability("Once each turn, you may play a land from your graveyard or cast a permanent spell with mana value 3 or less from your graveyard.", "Creature — Angel"), {
      kind: "static", effect: { static: "play-permission-choice", useLimit: { maximum: 1, period: "turn", during: "any" }, permissions: [
        { who: you, action: "play", what: land, from: graveyard },
        { who: you, action: "cast", from: graveyard, what: { ref: "filter", count: one, filter: { cls: "spell", permanent: true,
          properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "fixed", value: 3 } } }] } } },
      ] },
    });
  });

  it("is a one-time choice of at most one action while resolving (CR 608.2d)", () => {
    const top = { zone: "library", owner: "your", position: "top" } as const;
    assert.deepEqual(ability("Whenever this creature deals combat damage to a player, you may play a land from the top of your library or cast a spell with mana value 3 or less from the top of your library without paying its mana cost.", "Creature — Elf"), {
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true, to: "any" },
      effects: [{ steps: [{ effect: "choice", who: you, optional: true, options: [
        { effect: "play-card", instruction: { who: you, action: "play", what: land, from: top } },
        { effect: "play-card", instruction: { who: you, action: "cast", from: top, alternativeCost: free, what: { ref: "filter", count: one,
          filter: { cls: "spell", properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "fixed", value: 3 } } }] } } } },
      ] }] }],
    });
  });

  it("rejects a choice without a shared limit, and a lasting grant of a choice", () => {
    // Without a use limit "or" between two permissions is not represented.
    fails("You may play a land or cast a spell from your graveyard.", "Enchantment");
    fails("Until end of turn, you may play a land or cast a spell from among cards exiled with this creature.", "Instant");
  });
});

describe("a permission rider granting abilities to the card used (CR 400.7a)", () => {
  it("grants a keyword for a stated duration to matching spells cast this way", () => {
    // Thundermane Dragon (second sentence of its line).
    const creatureSpell = { types: ["creature"], cls: "spell" } as const;
    assert.deepEqual(ability("You may cast creature spells from the top of your library. If you cast a creature spell this way, it gains haste until end of turn.", "Creature — Dragon"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast",
        what: { ref: "each", filter: creatureSpell }, from: { zone: "library", owner: "your", position: "top" },
        riders: [{ rider: "gain-abilities", appliesTo: creatureSpell, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }],
          duration: { duration: "end-of-turn" } }] } },
    });
  });

  it("after a choice, 'If you do, it gains' is about the one card used, whichever action", () => {
    // Serra Paragon, with a keyword in place of its quoted ability.
    const graveyard = { zone: "graveyard", owner: "your" } as const;
    const rider = { rider: "gain-abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] } as const;
    assert.deepEqual(ability("Once during each of your turns, you may play a land from your graveyard or cast a permanent spell from your graveyard. If you do, it gains haste.", "Creature — Angel"), {
      kind: "static", effect: { static: "play-permission-choice", useLimit: { maximum: 1, period: "turn", during: "your" }, permissions: [
        { who: you, action: "play", what: { ref: "filter", filter: { types: ["land"] }, count: one }, from: graveyard, riders: [rider] },
        { who: you, action: "cast", what: { ref: "filter", filter: { cls: "spell", permanent: true }, count: one }, from: graveyard, riders: [rider] },
      ] },
    });
  });

  it("rejects 'If you do' after an unlimited permission for many cards, and a named subject", () => {
    fails("You may cast creature spells from your graveyard. If you do, it gains haste.", "Enchantment");
    fails("You may cast creature spells from the top of your library. If you cast a creature spell this way, that spell gains haste until end of turn.", "Enchantment");
  });
});

describe("casting a card as an Adventure (CR 715.3)", () => {
  it("records that the card may be cast only as its Adventure", () => {
    // Mosswood Dreadknight.
    assert.deepEqual(ability("When this creature dies, you may cast it from your graveyard as an Adventure until the end of your next turn.", "Creature — Human Knight"), {
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [{ effect: "grant-play-permission",
        permission: { who: you, action: "cast", what: { ref: "it" }, from: { zone: "graveyard", owner: "your" }, asAdventure: true },
        duration: { duration: "end-of-your-next-turn" } }] }],
    });
  });

  it("rejects playing (rather than casting) as an Adventure", () => {
    fails("When this creature dies, you may play it from your graveyard as an Adventure until the end of your next turn.", "Creature — Human Knight");
  });
});

describe("a play permission usable during any turn an action happened", () => {
  it("accepts a player-action history of the current turn as the recurring condition", () => {
    // Summon: Brynhildr, chapter I.
    assert.deepEqual(ability("I — Exile the top card of your library. During any turn you put a lore counter on this Saga, you may play that card.", "Enchantment Creature — Saga Knight"), {
      kind: "chapter", chapters: [1], effects: [
        { steps: [{ effect: "exile", what: { ref: "library-cards", position: "top", count: one, of: { zone: "library", owner: "your" } } }] },
        { steps: [{ effect: "grant-play-permission", duration: { duration: "rest-of-game" }, permission: { who: you, action: "play",
          what: { ref: "that", filter: { cls: "card" } },
          condition: { condition: "player-action-history", who: you, during: "this-turn", comparison: { op: "ge", value: one },
            action: { action: "put-counters", counter: { type: "lore" }, on: { match: "object", what: { ref: "this", noun: "saga" } } } } } }] },
      ],
    });
  });
});

describe("a mana-payment rider resumed by a pronoun (CR 609.4b)", () => {
  it("reads 'they' as the permitted player", () => {
    // Curse of Hospitality.
    const controller = { player: "controller", of: { ref: "that", filter: { types: ["creature"] } } } as const;
    assert.deepEqual(ability("Whenever a creature deals combat damage to enchanted player, that player exiles the top card of their library. Until end of turn, that creature's controller may play that card and they may spend mana as though it were mana of any color to cast that spell.", "Enchantment — Aura Curse"), {
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "filter", filter: { types: ["creature"] }, count: one }, combat: true, to: { player: "enchanted-player" } },
      effects: [
        { steps: [{ effect: "exile", who: { player: "that-player" },
          what: { ref: "library-cards", position: "top", count: one, of: { zone: "library", owner: "their" } } }] },
        { steps: [{ effect: "grant-play-permission", duration: { duration: "end-of-turn" },
          permission: { who: controller, action: "play", what: { ref: "that", filter: { cls: "card" } }, manaPayment: "any-color" } }] },
      ],
    });
  });

  it("does not let 'they' name a player other than you", () => {
    fails("Exile the top card of target player's library. Until end of turn, you may play that card and they may spend mana as though it were mana of any color to cast that spell.");
  });
});

describe("a damage-assignment rule limited to your turn (CR 510.1a)", () => {
  it("wraps the combat rule in the your-turn condition", () => {
    // Baldin, Century Herdmaster.
    assert.deepEqual(ability("During your turn, each creature assigns combat damage equal to its toughness rather than its power.", "Legendary Creature — Dwarf"), {
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "combat-rule", what: { ref: "each", filter: { types: ["creature"] } }, rule: { rule: "damage-assignment-value", value: "toughness" } } },
    });
  });

  it("keeps other combat rules out of the turn wrapper", () => {
    fails("During your turn, creatures you control can't be blocked.", "Legendary Creature — Dwarf");
  });
});
