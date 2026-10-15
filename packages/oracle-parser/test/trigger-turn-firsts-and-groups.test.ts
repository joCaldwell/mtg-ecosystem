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

describe("enchanted opponent (CR 303.4, 702.5)", () => {
  it("is the player the Aura enchants, as 'enchanted player' is", () => {
    // Psychic Possession; Tenuous Truce; Overencumbered.
    assert.deepEqual(trigger("Whenever enchanted opponent draws a card"), trigger("Whenever enchanted player draws a card"));
    assert.deepEqual(trigger("Whenever enchanted opponent draws a card"), { trigger: "draws", who: { player: "enchanted-player" } });
    assert.deepEqual(trigger("At the beginning of enchanted opponent's end step"),
      { trigger: "phase", phase: "end-step", whose: { player: "enchanted-player" } });
  });
  it("accepts the turn of the enchanted player", () => {
    assert.deepEqual(trigger("At the beginning of combat on enchanted opponent's turn"),
      { trigger: "phase", phase: "combat", whose: { player: "enchanted-player" } });
  });
  it("rejects an unrelated qualifier", () => {
    unsupported("Whenever enchanted opponent's friend draws a card");
  });
});

describe("first occurrence during each of a player's own turns (CR 603.2, 500.1)", () => {
  it("is an occurrence ordinal plus a turn restriction", () => {
    // Cat Collector.
    assert.deepEqual(trigger("Whenever you gain life for the first time during each of your turns"), {
      trigger: "gains-life", who: you, occurrence: { ordinal: 1, period: "turn" }, during: "your",
    });
  });
  it("counts each other player separately in that player's own turn", () => {
    // Valgavoth, Harrower of Souls.
    assert.deepEqual(trigger("Whenever an opponent loses life for the first time during each of their turns"), {
      trigger: "loses-life", who: { player: "an-opponent" },
      occurrence: { ordinal: 1, period: "turn", per: "player" }, during: "event-player",
    });
  });
  it("reads 'your first spell during each of your turns' the same way", () => {
    // Rashmi and Ragavan.
    assert.deepEqual(trigger("Whenever you cast your first spell during each of your turns"), {
      trigger: "cast", who: you, what: { cls: "spell" }, occurrence: { ordinal: 1, period: "turn" }, during: "your",
    });
    assert.deepEqual(trigger("Whenever you cast your first spell each turn"), {
      trigger: "cast", who: you, what: { cls: "spell" }, occurrence: { ordinal: 1, period: "turn" },
    });
  });
  it("rejects a mismatched possessive", () => {
    unsupported("Whenever you cast your first spell during each of their turns");
    unsupported("Whenever you gain life for the first time during each of their turns");
    unsupported("Whenever an opponent loses life for the first time during each of your turns");
  });
});

describe("investigating (CR 701.16a)", () => {
  it("is a player keyword-action event, with the first-time occurrence", () => {
    // Erdwal Illuminator.
    assert.deepEqual(trigger("Whenever you investigate for the first time each turn"),
      { trigger: "investigates", who: you, occurrence: { ordinal: 1, period: "turn" } });
  });
});

describe("a player puts an object onto the battlefield (CR 302.2)", () => {
  it("names the player performing the put, with the object description", () => {
    // Overburden; Nature's Wrath.
    assert.deepEqual(trigger("Whenever a player puts a nontoken creature onto the battlefield"), {
      trigger: "puts-onto-battlefield", who: { player: "a-player" }, what: a({ nonToken: true, types: ["creature"] }),
    });
    assert.deepEqual(trigger("Whenever a player puts a Swamp or black permanent onto the battlefield"), {
      trigger: "puts-onto-battlefield", who: { player: "a-player" },
      what: a({ anyOf: [{ subtypes: ["Swamp"] }, { colors: ["black"], cls: "permanent" }] }),
    });
  });
  it("is a different event from entering under a player's control", () => {
    assert.notDeepEqual(trigger("Whenever a player puts a Swamp onto the battlefield"), trigger("Whenever a Swamp enters"));
  });
  it("rejects other destinations and targeted players", () => {
    unsupported("Whenever a player puts a Swamp into a graveyard");
    unsupported("Whenever target player puts a Swamp onto the battlefield");
    unsupported("Whenever a player puts a Swamp onto the battlefield tapped");
  });
});

describe("base power and toughness in a filter (CR 208.4b)", () => {
  const base = (property: "power" | "toughness", value: number) =>
    ({ property: property === "power" ? "base-power" : "base-toughness", comparison: { op: "eq", value: n(value) } }) as const;
  it("tests the base value, not the current value", () => {
    // Rapid Augmenter; Primo, the Unbounded.
    assert.deepEqual(trigger("Whenever another creature you control with base power 1 enters"), {
      trigger: "enters", what: a({ other: true, types: ["creature"], control: "you", properties: [base("power", 1)] }),
    });
    assert.deepEqual(trigger("Whenever one or more creatures you control with base power 0 deal combat damage to a player"), {
      trigger: "deals-damage", what: a({ types: ["creature"], control: "you", properties: [base("power", 0)] }),
      combat: true, to: "any", grouping: "one-or-more",
    });
    const current = trigger("Whenever another creature you control with power 1 enters");
    assert.notDeepEqual(current, trigger("Whenever another creature you control with base power 1 enters"));
  });
  it("applies both base values for 'base power and toughness N/N'", () => {
    // Duskana, the Rage Mother; Bess, Soul Nourisher.
    assert.deepEqual(trigger("Whenever a creature you control with base power and toughness 2/2 attacks"), {
      trigger: "attacks", what: a({ types: ["creature"], control: "you", properties: [base("power", 2), base("toughness", 2)] }),
      alone: undefined, whom: undefined,
    });
  });
  it("rejects base power and toughness without a value pair", () => {
    unsupported("Whenever a creature you control with base power and toughness 2 attacks");
    unsupported("Whenever a creature you control with base mana value 2 attacks");
  });
});

describe("blocking or being blocked by a group (CR 509.3a-e)", () => {
  const black = a({ colors: ["black"], types: ["creature"] });
  const self = { ref: "self" } as const;
  it("is one occurrence for the matching group that blocks or is blocked", () => {
    // Rashka the Slayer; Hedron Blade.
    assert.deepEqual(trigger("Whenever ~ blocks one or more black creatures"),
      { trigger: "blocks", what: self, blocking: black, blockingGrouping: "one-or-more" });
    assert.deepEqual(trigger("Whenever equipped creature becomes blocked by one or more colorless creatures"), {
      trigger: "becomes-blocked", what: { ref: "equipped", noun: "creature" },
      by: a({ colorless: true, types: ["creature"] }), byGrouping: "one-or-more",
    });
  });
  it("keeps the required number of creatures (CR 509.3e)", () => {
    // Lairwatch Giant; Seifer, Balamb Rival.
    assert.deepEqual(trigger("Whenever this creature blocks two or more creatures"), {
      trigger: "blocks", what: { ref: "this", noun: "creature" }, blocking: a({ types: ["creature"] }),
      blockingGrouping: "one-or-more", blockingSize: { op: "ge", value: n(2) },
    });
    assert.deepEqual(trigger("Whenever a creature attacking one of your opponents becomes blocked by two or more creatures"), {
      trigger: "becomes-blocked",
      what: a({ types: ["creature"], attacking: [{ defender: "player", who: { player: "an-opponent" } }] }),
      by: a({ types: ["creature"] }), byGrouping: "one-or-more", bySize: { op: "ge", value: n(2) },
    });
  });
  it("applies the group to both sides of 'blocks or becomes blocked by'", () => {
    // Serra Inquisitors; Godsend.
    assert.deepEqual(trigger("Whenever this creature blocks or becomes blocked by one or more black creatures"), {
      trigger: "any-of", events: [
        { trigger: "blocks", what: { ref: "this", noun: "creature" }, blocking: black, blockingGrouping: "one-or-more" },
        { trigger: "becomes-blocked", what: { ref: "this", noun: "creature" }, by: black, byGrouping: "one-or-more" },
      ],
    });
  });
  it("differs from the per-creature wording", () => {
    assert.notDeepEqual(trigger("Whenever ~ blocks one or more black creatures"), trigger("Whenever ~ blocks a black creature"));
  });
  it("rejects a group the grammar does not model", () => {
    unsupported("Whenever ~ blocks one or more");
    unsupported("Whenever a creature becomes blocked by six or more creatures");
  });
});

describe("tapped for a specified type of mana, fortified land (CR 106.12a, 301.6)", () => {
  it("records the type of mana tapped for", () => {
    // Forsaken Monument; Ultima, Origin of Oblivion.
    assert.deepEqual(trigger("Whenever you tap a permanent for {C}"),
      { trigger: "taps-for-mana", what: a({ cls: "permanent" }), who: you, producing: "C" });
    assert.deepEqual(trigger("Whenever you tap a land for {C}"),
      { trigger: "taps-for-mana", what: a({ types: ["land"] }), who: you, producing: "C" });
    assert.deepEqual(trigger("Whenever a land is tapped for {G}"),
      { trigger: "taps-for-mana", what: a({ types: ["land"] }), producing: "G" });
  });
  it("differs from tapping for any mana", () => {
    assert.notDeepEqual(trigger("Whenever you tap a land for {C}"), trigger("Whenever you tap a land for mana"));
  });
  it("rejects a non-mana symbol", () => {
    unsupported("Whenever you tap a land for {2}");
    unsupported("Whenever you tap a land for {X}");
  });
  it("names the land a Fortification is attached to", () => {
    // Darksteel Garrison; C.A.M.P.
    assert.deepEqual(trigger("Whenever fortified land becomes tapped"),
      { trigger: "becomes-tapped", what: { ref: "fortified", noun: "land" } });
    assert.deepEqual(trigger("Whenever fortified land is tapped for mana"),
      { trigger: "taps-for-mana", what: { ref: "fortified", noun: "land" } });
    unsupported("Whenever fortified creature becomes tapped");
  });
});

describe("activating a keyword ability (CR 702.107, 702.128, 702.129)", () => {
  it("names the keyword and, for 'this creature's', the source", () => {
    // Herald of Anafenza.
    assert.deepEqual(trigger("Whenever you activate this creature's outlast ability"), {
      trigger: "activate", who: you,
      what: { cls: "ability", abilityKeyword: "outlast", abilitySource: { ref: "this", noun: "creature" } },
    });
  });
  it("accepts either of two keywords", () => {
    // Vizier of the Anointed.
    assert.deepEqual(trigger("Whenever you activate an eternalize or embalm ability"), {
      trigger: "activate", who: you,
      what: { cls: "ability", anyOf: [{ abilityKeyword: "eternalize" }, { abilityKeyword: "embalm" }] },
    });
    assert.deepEqual(trigger("Whenever you activate an eternalize ability"),
      { trigger: "activate", who: you, what: { cls: "ability", abilityKeyword: "eternalize" } });
  });
  it("rejects an ability of another source without a keyword", () => {
    unsupported("Whenever you activate this creature's cleverness ability");
    unsupported("Whenever you activate an eternalize or ability");
  });
});

describe("abilities with or without {T} in their activation cost (CR 107.5)", () => {
  const artifact = a({ types: ["artifact"] });
  it("is a property of the activated ability", () => {
    // Haunting Wind; Powerleech.
    assert.deepEqual(trigger("Whenever a player activates an artifact's ability without {T} in its activation cost"), {
      trigger: "activate", who: { player: "a-player" },
      what: { cls: "ability", abilitySource: artifact, tapSymbolInCost: false },
    });
    assert.deepEqual(trigger("Whenever an opponent activates an artifact's ability without {T} in its activation cost"), {
      trigger: "activate", who: { player: "an-opponent" },
      what: { cls: "ability", abilitySource: artifact, tapSymbolInCost: false },
    });
  });
  it("reads the 'ability of' wording the same way, with or without {T}", () => {
    // Artifact Possession; Imprison.
    assert.deepEqual(trigger("Whenever a player activates an ability of enchanted artifact without {T} in its activation cost"), {
      trigger: "activate", who: { player: "a-player" },
      what: { cls: "ability", abilitySource: { ref: "enchanted", noun: "artifact" }, tapSymbolInCost: false },
    });
    assert.deepEqual(trigger("Whenever a player activates an ability of enchanted creature with {T} in its activation cost that isn't a mana ability"), {
      trigger: "activate", who: { player: "a-player" },
      what: { cls: "ability", abilitySource: { ref: "enchanted", noun: "creature" }, tapSymbolInCost: true, manaAbility: false },
    });
  });
  it("combines with a becomes-tapped alternative", () => {
    // Powerleech.
    assert.deepEqual(trigger("Whenever an artifact becomes tapped or a player activates an artifact's ability without {T} in its activation cost"), {
      trigger: "any-of", events: [
        { trigger: "becomes-tapped", what: artifact },
        { trigger: "activate", who: { player: "a-player" }, what: { cls: "ability", abilitySource: artifact, tapSymbolInCost: false } },
      ],
    });
  });
  it("rejects other symbols and an unfinished cost phrase", () => {
    unsupported("Whenever a player activates an artifact's ability without {Q} in its activation cost");
    unsupported("Whenever a player activates an artifact's ability without {T} in its cost");
  });
});

describe("regressions for neighbouring wordings", () => {
  it("keeps 'has base power N' an effect on the subject, not a filter", () => {
    // Singing Tree.
    const result = parseOracleText("{T}: Target attacking creature has base power 0 until end of turn.", "Fixture", { typeLine: "Creature — Plant" });
    assert.equal(result.ok, true);
  });
  it("keeps 'you put a sticker on' its own event", () => {
    // Unlawful Entry.
    assert.equal(trigger("Whenever you put an art sticker on a creature").trigger, "puts-sticker");
  });
});

describe("attacking with creatures of a total power (CR 508.1, 603.2c)", () => {
  it("compares the summed power of one declaration of attackers", () => {
    // Ugin's Mastery; Desert Were-Worm.
    assert.deepEqual(trigger("Whenever you attack with creatures with total power 6 or greater"), {
      trigger: "player-attacks", who: you, per: "declaration", with: { types: ["creature"] },
      totalPower: { op: "ge", value: n(6) },
    });
    assert.deepEqual(trigger("Whenever you attack with creatures with total power 12 or greater for the first time each turn"), {
      trigger: "player-attacks", who: you, per: "declaration", with: { types: ["creature"] },
      totalPower: { op: "ge", value: n(12) }, occurrence: { ordinal: 1, period: "turn" },
    });
  });
  it("rejects other totals and a bare plural", () => {
    unsupported("Whenever you attack with creatures with total toughness 6 or greater");
    unsupported("Whenever you attack with creatures with total power");
  });
});

describe("transforming into a described permanent (CR 701.27)", () => {
  it("tests the permanent as it is after transforming", () => {
    // Cult of the Waxing Moon; Norn's Inquisitor.
    assert.deepEqual(trigger("Whenever a permanent you control transforms into a non-Human creature"), {
      trigger: "transforms", what: a({ cls: "permanent", control: "you" }),
      into: { filter: { nonSubtypes: ["Human"], types: ["creature"] } },
    });
    assert.deepEqual(trigger("Whenever a permanent you control transforms into a Phyrexian"), {
      trigger: "transforms", what: a({ cls: "permanent", control: "you" }), into: { filter: { subtypes: ["Phyrexian"] } },
    });
  });
  it("keeps the printed-face wording distinct", () => {
    assert.deepEqual(trigger("Whenever this creature transforms into ~"),
      { trigger: "transforms", what: { ref: "this", noun: "creature" }, into: { face: "source-face" } });
    unsupported("Whenever a permanent you control transforms into");
  });
});

describe("a noncreature-or-X spell (CR 205.2)", () => {
  it("is either test with the shared head noun", () => {
    // Firespitter Whelp; Harnesser of Storms.
    assert.deepEqual(trigger("Whenever you cast a noncreature or Dragon spell"), {
      trigger: "cast", who: you,
      what: { allOf: [{ anyOf: [{ nonTypes: ["creature"] }, { subtypes: ["Dragon"] }] }], cls: "spell" },
    });
    assert.deepEqual(trigger("Whenever you cast a noncreature or artifact spell"), {
      trigger: "cast", who: you,
      what: { allOf: [{ anyOf: [{ nonTypes: ["creature"] }, { types: ["artifact"] }] }], cls: "spell" },
    });
  });
  it("leaves the plain forms alone", () => {
    assert.deepEqual(trigger("Whenever you cast a noncreature spell"),
      { trigger: "cast", who: you, what: { nonTypes: ["creature"], cls: "spell" } });
    unsupported("Whenever you cast a noncreature or spell");
  });
});

describe("a player's graveyard or hand as a destination (CR 400.1)", () => {
  const permanent = a({ cls: "permanent" });
  it("names the player whose zone it is", () => {
    // Liability; Warped Devotion.
    assert.deepEqual(trigger("Whenever a nontoken permanent is put into a player's graveyard from the battlefield"), {
      trigger: "put-into-graveyard", what: a({ nonToken: true, cls: "permanent" }), from: "battlefield",
      to: { zone: "graveyard", owner: { player: "a-player" } },
    });
    assert.deepEqual(trigger("Whenever a permanent is returned to a player's hand"), {
      trigger: "put-into-zone", what: permanent, to: { zone: "hand", owner: { player: "a-player" } },
    });
  });
  it("differs from an unowned 'a graveyard' and from your own", () => {
    assert.notDeepEqual(trigger("Whenever a permanent is returned to a player's hand"), trigger("Whenever a permanent is returned to your hand"));
    unsupported("Whenever a permanent is returned to a player's pocket");
  });
});

describe("a spell or ability causes a player to shuffle (CR 609.1, 701.24)", () => {
  const cause = { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }] } satisfies ObjectFilter;
  it("is the shuffle event restricted to a matching cause", () => {
    // Psychogenic Probe.
    assert.deepEqual(trigger("Whenever a spell or ability causes a player to shuffle their library"),
      { trigger: "shuffles-library", who: { player: "a-player" }, causedBy: cause });
  });
  it("accepts the controller of the cause as the shuffling player", () => {
    // Widespread Panic.
    assert.deepEqual(trigger("Whenever a spell or ability causes its controller to shuffle their library"),
      { trigger: "shuffles-library", who: { player: "controller", of: { ref: "it" } }, causedBy: cause });
  });
  it("rejects another player's library and an uncaused shuffle", () => {
    unsupported("Whenever a spell or ability causes a player to shuffle your library");
    unsupported("Whenever a spell or ability causes a player to shuffle a library");
    assert.equal("causedBy" in trigger("Whenever you shuffle your library"), false);
  });
});

describe("attacking a planeswalker or a player or planeswalker (CR 506.3, 508.3)", () => {
  const withCreatures = { with: { types: ["creature"] }, count: { op: "ge", value: n(1) } } as const;
  it("treats each defender attacked as its own occurrence", () => {
    // Oath of Kaya.
    assert.deepEqual(trigger("Whenever an opponent attacks a planeswalker you control with one or more creatures"), {
      trigger: "player-attacks", who: { player: "an-opponent" }, per: "declaration",
      whomPermanents: a({ types: ["planeswalker"], control: "you" }), ...withCreatures,
    });
    // Rigo, Streetwise Mentor.
    assert.deepEqual(trigger("Whenever you attack a player or planeswalker with one or more creatures with power 1 or less"), {
      trigger: "player-attacks", who: you, per: "declaration", whom: { player: "a-player" },
      whomPermanents: a({ types: ["planeswalker"] }),
      with: { types: ["creature"], properties: [{ property: "power", comparison: { op: "le", value: n(1) } }] },
      count: { op: "ge", value: n(1) },
    });
  });
  it("differs from the collective and/or wording and rejects non-defenders", () => {
    assert.notDeepEqual(trigger("Whenever an opponent attacks a planeswalker you control"),
      trigger("Whenever an opponent attacks one or more planeswalkers you control"));
    unsupported("Whenever an opponent attacks a creature you control");
  });
});

describe("power greater than its base power (CR 208.4b)", () => {
  const above = { property: "power", comparison: { op: "gt", value: { amount: "attribute", of: { ref: "it" }, attribute: "base-power" } } } as const;
  it("compares the candidate's power with its own base power", () => {
    // Ms. Marvel, Elastic Ally.
    assert.deepEqual(trigger("Whenever a creature you control with power greater than its base power deals combat damage to a player"), {
      trigger: "deals-damage", what: a({ types: ["creature"], control: "you", properties: [above] }), combat: true, to: "any",
    });
  });
  it("reads 'each with' like 'with' for a group", () => {
    // Kutzil, Malamet Exemplar.
    assert.deepEqual(trigger("Whenever one or more creatures you control each with power greater than its base power deal combat damage to a player"), {
      trigger: "deals-damage", what: a({ types: ["creature"], control: "you", properties: [above] }), combat: true, to: "any",
      grouping: "one-or-more",
    });
  });
  it("is distinct from comparing with the current power", () => {
    assert.notDeepEqual(trigger("Whenever a creature you control with power greater than its toughness deals combat damage to a player"),
      trigger("Whenever a creature you control with power greater than its base power deals combat damage to a player"));
    unsupported("Whenever a creature you control with power greater than its base deals combat damage to a player");
  });
});

describe("an ability is activated (passive; CR 602.2)", () => {
  it("is the activation by any player", () => {
    // Illusionist's Bracers; Battlemage's Bracers.
    assert.deepEqual(trigger("Whenever an ability of equipped creature is activated"),
      trigger("Whenever a player activates an ability of equipped creature"));
    assert.deepEqual(trigger("Whenever an ability of equipped creature is activated"), {
      trigger: "activate", who: { player: "a-player" },
      what: { cls: "ability", abilitySource: { ref: "equipped", noun: "creature" } },
    });
  });
  it("rejects a non-ability subject", () => {
    unsupported("Whenever a creature is activated");
  });
});
