import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { CardPlayInstruction, Effect, StaticEffect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { looksLikeStaticPlayPermission, parsePermissionSpellAbilities, parsePlayInstruction, parsePlayPermissionStatic } from "../src/parser/permissions.ts";

// Wording checked against the cached Scryfall oracle-cards.json. Named cases
// below are real card text; malformed/unsupported negative pairs are synthetic.
// CR 118.9d, 305.2–3, 601.2, 608.2g, 609.4b, 611.2a, 701.18:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

const you = { player: "you" } as const;
const card = { ref: "this", noun: "card" } as const;
const thatCard = { ref: "that", filter: { cls: "card" } } as const;
const graveyard = { zone: "graveyard", owner: "your" } as const;
const waived = { cost: "without-mana-cost" } as const;

function staticPermission(text: string): StaticEffect {
  const c = new Cursor(lex(text));
  const result = parsePlayPermissionStatic(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true);
  return result;
}

function effect(text: string): Effect {
  const c = new Cursor(lex(text));
  const result = parsePlayInstruction(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(result.length, 1);
  c.punct(".");
  assert.equal(c.done(), true, c.errorMessage());
  return result[0];
}

function permission(fields: Omit<CardPlayInstruction, "who">): CardPlayInstruction {
  return { who: you, ...fields };
}

describe("abilities granted to spells cast under a permission", () => {
  it("grants Chiss-Goria's affinity during the eventual cast instead of attack resolution", () => {
    // Complete Oracle text from cached Scryfall data, including the source's
    // separate affinity ability. CR 608.2c modifies the earlier permission;
    // the cast spell has its new affinity before cost determination (601.2f).
    const text = "Affinity for artifacts (This spell costs {1} less to cast for each artifact you control.)\nFlying, haste\nWhenever Chiss-Goria attacks, exile the top five cards of your library. You may cast an artifact spell from among them this turn. If you do, it has affinity for artifacts.";
    const result = parseOracleText(text, "Chiss-Goria, Forge Tyrant");
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [
      { kind: "keywords", keywords: [{ keyword: "affinity", filter: { types: ["artifact"] } }] },
      { kind: "keywords", keywords: [{ keyword: "flying" }, { keyword: "haste" }] },
      { kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } }, effects: [
        { steps: [{ effect: "exile", what: { ref: "library-cards", position: "top", count: { amount: "fixed", value: 5 }, of: { zone: "library", owner: "your" } } }] },
        { steps: [{ effect: "grant-play-permission", duration: { duration: "this-turn" }, permission: {
          who: you, action: "cast", what: { ref: "filter", filter: { types: ["artifact"], cls: "spell" }, count: { amount: "fixed", value: 1 } },
          fromAmong: { ref: "them" }, useLimit: { maximum: { amount: "fixed", value: 1 }, period: "permission" },
          spellAbilities: [{ keyword: "affinity", filter: { types: ["artifact"] } }],
        } }] },
      ] },
    ]);
  });

  it("preserves a complete keyword list without mutating the input permission", () => {
    const original = permission({ action: "cast", what: thatCard });
    const c = new Cursor(lex("If you do, it has convoke and cascade."));
    assert.deepEqual(parsePermissionSpellAbilities(c, original), { ...original, spellAbilities: [{ keyword: "convoke" }, { keyword: "cascade" }] });
    assert.ok(c.done());
    assert.equal(original.spellAbilities, undefined);
  });

  it("rejects unrepresented or ambiguous permission continuations and rewinds", () => {
    const original = permission({ action: "cast", what: thatCard });
    for (const text of [
      "If you don't, it has affinity for artifacts.",
      "If you do, they have affinity for artifacts.",
      "If you do, it has affinity for artifacts until end of turn.",
      "If you do, it has affinity for artifacts if you control a Forest.",
      'If you do, it has "{T}: Add {G}."',
      "If you do, it has affinity for artifacts and draw a card.",
      "If you do, it has affinity for artifacts. Draw a card.",
      "If you do, it has an unrecognized ability.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parsePermissionSpellAbilities(c, original), null, text);
      assert.equal(c.pos, 0, text);
    }
    for (const value of [
      { ...original, action: "play" as const },
      { ...original, who: { player: "target-opponent" as const } },
      { ...original, what: { ref: "that" as const, filter: { cls: "card" as const }, plural: true as const } },
      { ...original, spellAbilities: [{ keyword: "haste" }] },
    ]) {
      const c = new Cursor(lex("If you do, it has affinity for artifacts."));
      assert.equal(parsePermissionSpellAbilities(c, value), null);
      assert.equal(c.pos, 0);
    }
  });

  it("prevents dependent duration-permission riders from becoming immediate effects", () => {
    for (const text of [
      "You may cast that card this turn. If you do, draw a card.",
      "You may cast that card this turn. If you don't, draw a card.",
      "You may cast that card this turn. If you do, it has affinity for artifacts. Draw a card.",
      "You may play that card this turn. If you do, it has affinity for artifacts.",
      "Until end of turn, you may play lands and cast spells from your graveyard. If you do, it has affinity for artifacts.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("cast/play instruction and permission identity", () => {
  it("casts now without waiving any additional costs (Dazzling Sphinx)", () => {
    assert.deepEqual(effect("You may cast that card without paying its mana cost."), {
      effect: "play-card", instruction: permission({ action: "cast", what: thatCard, alternativeCost: waived }), optional: true,
    });
    assert.deepEqual(effect("Cast it without paying its mana cost."), {
      effect: "play-card", instruction: permission({ action: "cast", what: { ref: "it" }, alternativeCost: waived }),
    });
  });

  it("keeps play distinct from cast and does not make a granted permission optional", () => {
    for (const action of ["play", "cast"] as const) {
      assert.deepEqual(effect(`You may ${action} that card this turn.`), {
        effect: "grant-play-permission", permission: permission({ action, what: thatCard }), duration: { duration: "this-turn" },
      });
    }
    // Knacksaw Clique: creation of the permission occurs during resolution;
    // using it later still observes land-play limits and normal spell timing.
    assert.deepEqual(effect("Until end of turn, you may play that card."), {
      effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }), duration: { duration: "end-of-turn" },
    });
  });

  it("distinguishes start/end of the next turn and the next end step", () => {
    for (const [words, duration] of [
      ["Until your next turn", "your-next-turn"],
      ["Until the end of your next turn", "end-of-your-next-turn"],
      ["Until your next end step", "your-next-end-step"],
    ] as const) {
      assert.deepEqual(effect(`${words}, you may play that card.`), {
        effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }), duration: { duration },
      });
    }
  });

  it("supports the enclosing optional/duration parser without changing meaning", () => {
    const c = new Cursor(lex("cast it without paying its mana cost"));
    assert.deepEqual(parsePlayInstruction(c, { optionalConsumed: true }), [{
      effect: "play-card", instruction: permission({ action: "cast", what: { ref: "it" }, alternativeCost: waived }), optional: true,
    }]);
    assert.equal(c.done(), true);
    const d = new Cursor(lex("play that card"));
    assert.deepEqual(parsePlayInstruction(d, { optionalConsumed: true, leadingDuration: { duration: "end-of-turn" } }), [{
      effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }), duration: { duration: "end-of-turn" },
    }]);
  });

  it("retains the actor, even when an owner receives the permission", () => {
    assert.deepEqual(effect("Its owner may cast it without paying its mana cost."), {
      effect: "play-card", instruction: { who: { player: "owner", of: { ref: "it" } }, action: "cast", what: { ref: "it" }, alternativeCost: waived }, optional: true,
      // The owner, not you, decides whether their optional cast happens.
      decidedBy: { player: "owner", of: { ref: "it" } },
    });
    assert.deepEqual(effect("That player may play that card this turn."), {
      effect: "grant-play-permission", permission: { who: { player: "that-player" }, action: "play", what: thatCard }, duration: { duration: "this-turn" },
    });
  });

  it("keeps target selection and source-zone ownership (Memory Plunder)", () => {
    assert.deepEqual(effect("You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost."), {
      effect: "play-card", optional: true, instruction: permission({ action: "cast",
        what: { ref: "target", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card" }, count: { amount: "fixed", value: 1 } },
        from: { zone: "graveyard", owner: "an-opponent" }, alternativeCost: waived }),
    });
  });

  it("bounds a counted permission across its whole lifetime (Chainer)", () => {
    // Official rulings: no creature is chosen while the ability resolves;
    // each resolved ability gives one independently usable permission.
    // https://magic.wizards.com/en/news/feature/commander-2019-edition-release-notes-2019-08-09
    assert.deepEqual(effect("You may cast a creature spell from your graveyard this turn."), {
      effect: "grant-play-permission", permission: permission({ action: "cast",
        what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { types: ["creature"], cls: "spell" } },
        from: graveyard, useLimit: { maximum: { amount: "fixed", value: 1 }, period: "permission" } }),
      duration: { duration: "this-turn" },
    });
    assert.deepEqual(effect("You may cast creature spells from your graveyard this turn."), {
      effect: "grant-play-permission", permission: permission({ action: "cast",
        what: { ref: "each", filter: { types: ["creature"], cls: "spell" } }, from: graveyard }),
      duration: { duration: "this-turn" },
    });
  });

  it("keeps a candidate pool separate from spell characteristics (Muse Vortex)", () => {
    assert.deepEqual(effect("You may cast an instant or sorcery spell with mana value X or less from among them without paying its mana cost."), {
      effect: "play-card", optional: true, instruction: permission({ action: "cast",
        what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: {
          allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell",
          properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "x" } } }],
        } }, fromAmong: { ref: "them" }, alternativeCost: waived }),
    });
  });

  it("preserves an explicitly symbolic exiled-card antecedent", () => {
    assert.deepEqual(effect("You may cast the exiled card without paying its mana cost."), {
      effect: "play-card", optional: true, instruction: permission({ action: "cast", alternativeCost: waived,
        what: { ref: "related", number: "singular", filter: { cls: "card" },
          relation: { relation: "event-result", event: "exile", scope: "contextual" } } }),
    });
    assert.deepEqual(effect("You may cast those cards for as long as they remain exiled."), {
      effect: "grant-play-permission", permission: permission({ action: "cast", what: { ...thatCard, plural: true } }),
      durationScope: "each-permitted-card",
      duration: { duration: "as-long-as", condition: { condition: "remains", what: { ref: "them" }, zone: { zone: "exile" } } },
    });
    assert.deepEqual(effect("You may play that card for as long as it remains exiled."), {
      effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }),
      durationScope: "each-permitted-card",
      duration: { duration: "as-long-as", condition: { condition: "remains", what: { ref: "it" }, zone: { zone: "exile" } } },
    });
    assert.deepEqual(effect("You may play that card for as long as you control this creature."), {
      effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }),
      duration: { duration: "as-long-as", condition: { condition: "control", who: you, what: { ref: "this", noun: "creature" } } },
    });
  });

  it("keeps an instruction-selected spell separate from a static eligible set (Sram's Expertise)", () => {
    const text = "You may cast a spell with mana value 3 or less from your hand without paying its mana cost.";
    assert.equal(parsePlayPermissionStatic(new Cursor(lex(text))), null);
    assert.deepEqual(effect(text), {
      effect: "play-card", optional: true, instruction: permission({ action: "cast",
        what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { cls: "spell", properties: [{ property: "mana-value", comparison: { op: "le", value: { amount: "fixed", value: 3 } } }] } },
        from: { zone: "hand", owner: "your" }, alternativeCost: waived }),
    });
  });
});

describe("static casting permissions and their costs", () => {
  it("preserves a graveyard casting condition (Gravecrawler)", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard as long as you control a Zombie."), {
      static: "play-permission", permission: permission({ action: "cast", what: card, from: graveyard,
        condition: { condition: "control", who: you, what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { subtypes: ["Zombie"] } } } }),
    });
  });

  it("keeps normal casting costs and mana-cost waivers different (Skaab Ruinator/Omniscience)", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard."), {
      static: "play-permission", permission: permission({ action: "cast", what: card, from: graveyard }),
    });
    assert.deepEqual(staticPermission("You may cast spells from your hand without paying their mana costs."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } },
        from: { zone: "hand", owner: "your" }, alternativeCost: waived }),
    });
  });

  it("preserves source alternatives without conflating them with intersected zones", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard or from exile."), {
      static: "play-permission", permission: permission({ action: "cast", what: card,
        from: { anyOf: [graveyard, { zone: "exile", owner: undefined }] } }),
    });
    // One with the Multiverse: one total use, from either permitted source.
    assert.deepEqual(staticPermission("Once during each of your turns, you may cast a spell from your hand or the top of your library without paying its mana cost."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { cls: "spell" } },
        from: { anyOf: [{ zone: "hand", owner: "your" }, { zone: "library", owner: "your", position: "top" }] }, alternativeCost: waived,
        useLimit: { maximum: 1, period: "turn", during: "your" } }),
    });
  });

  it("does not turn land-play permissions into instructions to put lands on the battlefield", () => {
    assert.deepEqual(staticPermission("You may play lands from your graveyard."), {
      static: "play-permission", permission: permission({ action: "play", what: { ref: "each", filter: { types: ["land"] } }, from: graveyard }),
    });
    assert.deepEqual(staticPermission("You may play lands from the top of your library."), {
      static: "play-permission", permission: permission({ action: "play", what: { ref: "each", filter: { types: ["land"] } }, from: { zone: "library", owner: "your", position: "top" } }),
    });
  });

  it("preserves an additional life-and-discard payment (Demonic Embrace)", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard by paying 3 life and discarding a card in addition to paying its other costs."), {
      static: "play-permission", permission: permission({ action: "cast", what: card, from: graveyard, additionalCosts: [
        { cost: "pay-life", amount: { amount: "fixed", value: 3 } },
        { cost: "discard", what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { cls: "card" } } },
      ] }),
    });
  });

  it("preserves an alternative mana-and-exile payment (Squee, Dubious Monarch)", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard by paying {3}{R} and exiling four other cards from your graveyard rather than paying its mana cost."), {
      static: "play-permission", permission: permission({ action: "cast", what: card, from: graveyard, alternativeCost: { cost: "pay", costs: [
        { cost: "mana", symbols: ["3", "R"] },
        { cost: "exile", what: { ref: "filter", count: { amount: "fixed", value: 4 }, filter: { other: true, cls: "card", zone: graveyard } } },
      ] } }),
    });
  });

  it("retains counters distributed among payment objects (Quilled Greatwurm)", () => {
    assert.deepEqual(staticPermission("You may cast this card from your graveyard by removing six counters from among creatures you control in addition to paying its other costs."), {
      static: "play-permission", permission: permission({ action: "cast", what: card, from: graveyard, additionalCosts: [{
        cost: "remove-counters", counter: "any", count: { amount: "fixed", value: 6 }, distributed: true,
        from: { ref: "each", filter: { types: ["creature"], control: "you" } },
      }] }),
    });
  });

  it("distinguishes once on your turns from once on any turn", () => {
    // Karador's complete permission; second clause tests the generic period.
    for (const [words, during] of [["Once during each of your turns", "your"], ["Once each turn", "any"]] as const) {
      assert.deepEqual(staticPermission(`${words}, you may cast a creature spell from your graveyard.`), {
        static: "play-permission", permission: permission({ action: "cast", what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { types: ["creature"], cls: "spell" } }, from: graveyard,
          useLimit: { maximum: 1, period: "turn", during } }),
      });
    }
  });

  it("represents life equal to mana value as a life cost (Access Maze)", () => {
    assert.deepEqual(staticPermission("Once during each of your turns, you may cast a spell from your hand by paying life equal to its mana value rather than paying its mana cost."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "filter", count: { amount: "fixed", value: 1 }, filter: { cls: "spell" } },
        from: { zone: "hand", owner: "your" }, useLimit: { maximum: 1, period: "turn", during: "your" },
        alternativeCost: { cost: "pay", costs: [{ cost: "pay-life", amount: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }] } }),
    });
  });

  it("changes timing without granting a new source zone (Yeva/Rout)", () => {
    assert.deepEqual(staticPermission("You may cast green creature spells as though they had flash."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { colors: ["green"], types: ["creature"], cls: "spell" } }, timing: "as-though-flash" }),
    });
    assert.deepEqual(staticPermission("You may cast this spell as though it had flash if you pay {2} more to cast it."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "this", noun: "spell" }, timing: "as-though-flash", additionalCosts: [{ cost: "mana", symbols: ["2"] }] }),
    });
  });

  it("treats plural eligible categories as alternatives (Melek/Nalia/Renari)", () => {
    assert.deepEqual(staticPermission("You may cast instant and sorcery spells from the top of your library."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: {
        allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell",
      } }, from: { zone: "library", owner: "your", position: "top" } }),
    });
    assert.deepEqual(staticPermission("You may cast Cleric, Rogue, Warrior, and Wizard spells from the top of your library."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: {
        allOf: [{ anyOf: ["Cleric", "Rogue", "Warrior", "Wizard"].map(subtype => ({ subtypes: [subtype] })) }], cls: "spell",
      } }, from: { zone: "library", owner: "your", position: "top" } }),
    });
    assert.deepEqual(staticPermission("You may cast Dragon spells and artifact spells as though they had flash."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: {
        anyOf: [{ subtypes: ["Dragon"], cls: "spell" }, { types: ["artifact"], cls: "spell" }],
      } }, timing: "as-though-flash" }),
    });
  });

  it("keeps adjacent types and conjunctive colors as intersections", () => {
    assert.deepEqual(staticPermission("You may cast green and blue artifact creature spells as though they had flash."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: {
        colors: ["green", "blue"], types: ["artifact", "creature"], cls: "spell",
      } }, timing: "as-though-flash" }),
    });
  });

  it("shares the source across explicit land and spell permissions (Future Sight)", () => {
    const from = { zone: "library", owner: "your", position: "top" } as const;
    assert.deepEqual(staticPermission("You may play lands and cast spells from the top of your library."), {
      static: "group", effects: [
        { static: "play-permission", permission: permission({ action: "play", what: { ref: "each", filter: { types: ["land"] } }, from }) },
        { static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } }, from }) },
      ],
    });
    // A later duration applies to both permissions and is not interpreted as
    // an immediate play-land instruction followed by a lasting cast grant.
    const c = new Cursor(lex("You may play lands and cast spells from your graveyard this turn."));
    assert.deepEqual(parsePlayInstruction(c), [
      { effect: "grant-play-permission", permission: permission({ action: "play", what: { ref: "each", filter: { types: ["land"] } }, from: graveyard }), duration: { duration: "this-turn" } },
      { effect: "grant-play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } }, from: graveyard }), duration: { duration: "this-turn" } },
    ]);
  });

  it("keeps linked exile pools and exile-event periods distinct from permission durations", () => {
    const source = { ref: "this", noun: "creature" } as const;
    assert.deepEqual(staticPermission("You may cast spells from among cards exiled with this creature."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } },
        fromAmong: { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source } } }),
    });
    assert.deepEqual(staticPermission("You may cast spells from among cards exiled with this creature this turn without paying their mana costs."), {
      static: "play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } }, alternativeCost: waived,
        fromAmong: { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source, period: "this-turn" } } }),
    });
    assert.deepEqual(staticPermission("You may play cards exiled with this enchantment."), {
      static: "play-permission", permission: permission({ action: "play",
        what: { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source: { ref: "this", noun: "enchantment" } } } }),
    });
  });

  it("preserves spending mana as any color vs any type", () => {
    for (const [rider, manaPayment] of [
      ["you may spend mana as though it were mana of any color to cast it", "any-color"],
      ["mana of any type can be spent to cast that spell", "any-type"],
    ] as const) {
      assert.deepEqual(effect(`You may cast that card this turn, and ${rider}.`), {
        effect: "grant-play-permission", permission: permission({ action: "cast", what: thatCard, manaPayment }), duration: { duration: "this-turn" },
      });
    }
  });
});

describe("casting grammar refuses unsupported detail", () => {
  it("rewinds unsupported costs, zone unions, face changes, and durations", () => {
    for (const text of [
      "You may cast it without paying any costs.",
      "You may cast it from your graveyard and/or hand.",
      "You may cast it face down as a 2/2 creature without paying its mana cost.",
      "You may cast it transformed without paying its mana cost.",
      "You may cast it using its mutate ability.",
      "You may cast it by discarding a card.",
      "You may cast it as though it were a copy of another card.",
      "You may play that card until your third turn.",
      "You may cast it without paying its mana cost without paying its mana cost.",
      "You may play that card for as long as they remain exiled.",
      "You may play those cards for as long as it remains exiled.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parsePlayInstruction(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("never accepts only a prefix of a static permission", () => {
    for (const text of [
      "You may cast this card from your graveyard, but not from anywhere else.",
      "You may cast this spell as though it had flash if you pay {2} more to cast it for each opponent.",
      "You may cast this card from your graveyard as long as you control a Zombie and if.",
      "You may play an additional land on each of your turns.",
      "Once each turn, you may play lands and cast spells from your graveyard.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parsePlayPermissionStatic(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    // Formerly in the list above only because nothing could represent the
    // rider. "If you do, it enters with …" after a permission for this one
    // card is now the permission's entry rider (CR 614.1c, 614.12), so the
    // complete wording is asserted structurally (Hundred-Battle Veteran).
    assert.deepEqual(staticPermission("You may cast this card from your graveyard. If you do, it enters with a finality counter on it."), {
      static: "play-permission", permission: { who: you, action: "cast", what: card, from: graveyard,
        riders: [{ rider: "entry", counters: [{ count: { amount: "fixed", value: 1 }, counter: { type: "finality" } }] }] },
    });
  });

  it("prevents unsupported static riders from falling through as immediate actions", () => {
    const text = "You may cast this card from your graveyard. If you do, draw a card.";
    const c = new Cursor(lex(text));
    assert.equal(looksLikeStaticPlayPermission(c), true);
    assert.equal(c.pos, 0);
    assert.equal(parseOracleText(text).ok, false);
    const result = parseOracleText(`{T}: ${text}`);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined, restriction: undefined, costs: [{ cost: "tap-self" }], effects: [
      { steps: [{ effect: "play-card", instruction: permission({ action: "cast", what: card, from: graveyard }), optional: true }] },
      { condition: { condition: "you-do" }, conditionKind: "if", steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] },
    ] }]);
  });

  it("integrates a leading duration before choosing the effect kind", () => {
    const result = parseOracleText("Until end of turn, you may play that card.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
      effect: "grant-play-permission", permission: permission({ action: "play", what: thatCard }), duration: { duration: "end-of-turn" },
    }] }] }]);
  });

  it("does not make the simultaneous creation of two permissions optional", () => {
    const result = parseOracleText("Until end of turn, you may play lands and cast spells from your graveyard.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
      effect: "simultaneous", steps: [
        { effect: "grant-play-permission", permission: permission({ action: "play", what: { ref: "each", filter: { types: ["land"] } }, from: graveyard }), duration: { duration: "end-of-turn" } },
        { effect: "grant-play-permission", permission: permission({ action: "cast", what: { ref: "each", filter: { cls: "spell" } }, from: graveyard }), duration: { duration: "end-of-turn" } },
      ],
    }] }] }]);
  });
});
