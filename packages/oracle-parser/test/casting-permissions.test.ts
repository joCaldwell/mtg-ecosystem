import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

// Wording from the pinned corpus: Chaotic Strike, Defiant Stand, Berserker's
// Frenzy, Reset, Siren's Call, Melee, Hewed Stone Retainers, Exploration,
// Storm Cauldron, Summer Bloom, Fastbond, Urban Evolution, Future Sight,
// Lantern of Insight, Telepathy, Bolas's Citadel, Intellect Devourer, Elsha of
// the Infinite, Mikey & Don, Edgar, Soar, Kaldring, Inside Information.
// CR 113.6e, 305.2, 307.5a, 401.5, 506.8, 601.3, 614.12:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: "you" };
const one = { amount: "fixed", value: 1 };
const thisSpell = { ref: "this", noun: "spell" };
const topOfLibrary = { zone: "library", owner: "your", position: "top" };
const lifeForManaValue = { cost: "pay", costs: [{ cost: "pay-life", amount: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }] };

function ability(text: string, typeLine: string) {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const rejected = (text: string, typeLine: string) => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const restriction = (text: string) => ability(text, "Instant");

describe("casting restrictions on the card itself", () => {
  it("keeps the current phase, the turn owner and combat bounds as separate parts of one window", () => {
    assert.deepEqual(restriction("Cast this spell only during combat after blockers are declared."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ during: "combat", after: "blockers-declared" }] } });
    // CR 506.8d: without "during combat" the bound concerns the turn's first combat.
    assert.deepEqual(restriction("Cast this spell only before blockers are declared."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ before: "blockers-declared" }] } });
    assert.deepEqual(restriction("Cast this spell only during combat on your turn before blockers are declared."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ during: "combat", turn: "your", before: "blockers-declared" }] } });
    assert.deepEqual(restriction("Cast this spell only during an opponent's turn, before attackers are declared."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ turn: "opponent", before: "attackers-declared" }] } });
    assert.deepEqual(restriction("Cast this spell only during an opponent's turn after their upkeep step."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ turn: "opponent", after: "upkeep-step" }] } });
    assert.deepEqual(restriction("Cast this spell only during your end step."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ turn: "your", during: "end-step" }] } });
  });

  it("represents alternative windows and a conjunctive condition", () => {
    assert.deepEqual(restriction("Cast this spell only before combat or during combat before blockers are declared."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ before: "combat" }, { during: "combat", before: "blockers-declared" }] } });
    assert.deepEqual(restriction("Cast this spell only during the declare attackers step and only if you've been attacked this step."), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, windows: [{ during: "declare-attackers" }],
      condition: { condition: "was-attacked", who: you, during: "this-step" } } });
    assert.deepEqual(ability("Cast this spell only if you've cast another spell this turn.", "Artifact Creature — Golem"), { kind: "static", effect: {
      static: "cast-restriction", what: thisSpell, condition: { condition: "cast-count", who: you, what: { other: true, cls: "spell" },
        comparison: { op: "ge", value: one }, during: "this-turn" } } });
  });

  it("rejects incomplete, repeated, or continued restrictions", () => {
    for (const text of [
      "Cast this spell only.",
      "Cast this spell only during combat and draw a card.",
      "Cast this spell only during combat during the declare blockers step.",
      "Cast this spell only on your turn.",
      "Cast this spell only after their upkeep step.",
      "Cast this spell only during combat before blockers are declared before attackers are declared.",
      "Cast this spell only during combat. Draw a card.",
      "Cast this spell only if you've been attacked this turn.",
    ]) rejected(text, "Instant");
  });
});

describe("land-play allowances", () => {
  it("stores a static allowance for each affected player", () => {
    assert.deepEqual(ability("You may play two additional lands on each of your turns.", "Legendary Creature — Human Monk"), { kind: "static", effect: {
      static: "additional-land-plays", allowance: { who: you, additional: { amount: "fixed", value: 2 } } } });
    assert.deepEqual(ability("Each player may play an additional land during each of their turns.", "Artifact"), { kind: "static", effect: {
      static: "additional-land-plays", allowance: { who: { player: "each-player" }, additional: one } } });
    assert.deepEqual(ability("You may play any number of lands on each of your turns.", "Enchantment"), { kind: "static", effect: {
      static: "additional-land-plays", allowance: { who: you, additional: "unlimited" } } });
  });

  it("creates a mandatory continuous effect with its printed lifetime when resolving", () => {
    assert.deepEqual(ability("Draw three cards. You may play an additional land this turn.", "Sorcery"), { kind: "spell", effects: [
      { steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 3 } }] },
      { steps: [{ effect: "additional-land-plays", allowance: { who: you, additional: one }, duration: { duration: "this-turn" } }] },
    ] });
    assert.deepEqual(ability("You may play up to three additional lands this turn.", "Sorcery"), { kind: "spell", effects: [
      { steps: [{ effect: "additional-land-plays", allowance: { who: you, additional: { amount: "fixed", value: 3 } }, duration: { duration: "this-turn" } }] },
    ] });
  });

  it("does not confuse the static and resolving wordings or drop a mismatched possessive", () => {
    rejected("You may play an additional land.", "Sorcery");
    rejected("You may play an additional land this turn.", "Enchantment");
    rejected("Each player may play an additional land on each of your turns.", "Enchantment");
    rejected("You may play an additional land on each of your turns and draw a card.", "Enchantment");
  });
});

describe("playing with hidden cards revealed", () => {
  it("records whose zone is revealed and which zone", () => {
    assert.deepEqual(ability("Play with the top card of your library revealed.", "Enchantment"), { kind: "static", effect: {
      static: "play-revealed", who: you, what: "library-top" } });
    assert.deepEqual(ability("Players play with the top card of their libraries revealed.", "Artifact"), { kind: "static", effect: {
      static: "play-revealed", who: { player: "each-player" }, what: "library-top" } });
    assert.deepEqual(ability("Your opponents play with their hands revealed.", "Enchantment"), { kind: "static", effect: {
      static: "play-revealed", who: { player: "each-opponent" }, what: "hand" } });
  });

  it("rejects a mismatched owner, another zone, or a trailing instruction", () => {
    rejected("Play with the top card of their library revealed.", "Enchantment");
    rejected("Players play with the top card of your library revealed.", "Enchantment");
    rejected("Play with your graveyard revealed.", "Enchantment");
    rejected("Play with the top card of your library revealed and draw a card.", "Enchantment");
  });
});

describe("sentences that qualify every use of a play permission", () => {
  const land = { static: "play-permission", permission: { who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } }, from: topOfLibrary } };

  it("folds a mandatory payment into the casting permission only", () => {
    assert.deepEqual(ability("You may play lands and cast spells from the top of your library. If you cast a spell this way, pay life equal to its mana value rather than pay its mana cost.", "Legendary Artifact"), {
      kind: "static", effect: { static: "group", effects: [land, { static: "play-permission", permission: {
        who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } }, from: topOfLibrary, alternativeCost: lifeForManaValue } }] } });
  });

  it("folds payment relaxation and flash timing into the same fields as the one-sentence wording", () => {
    const exiled = { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source: { ref: "this", noun: "creature" } } };
    assert.deepEqual(ability("You may play lands and cast spells from among cards exiled with this creature. If you cast a spell this way, you may spend mana as though it were mana of any color to cast it.", "Creature — Horror"), {
      kind: "static", effect: { static: "group", effects: [
        { static: "play-permission", permission: { who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } }, fromAmong: exiled } },
        { static: "play-permission", permission: { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } }, fromAmong: exiled, manaPayment: "any-color" } },
      ] } });
    assert.deepEqual(ability("You may cast noncreature spells from the top of your library. If you cast a spell this way, you may cast it as though it had flash.", "Legendary Creature — Djinn Monk"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast",
        what: { ref: "each", filter: { nonTypes: ["creature"], cls: "spell" } }, from: topOfLibrary, timing: "as-though-flash" } } });
  });

  it("keeps entry modifications of the resulting permanent as riders, with their own spell filter", () => {
    assert.deepEqual(ability("You may play lands and cast Mutant, Ninja, or Turtle spells from the top of your library. If you cast a creature spell this way, that creature enters with an additional +1/+1 counter on it.", "Legendary Creature — Mutant Ninja Turtle"), {
      kind: "static", effect: { static: "group", effects: [land, { static: "play-permission", permission: {
        who: you, action: "cast", what: { ref: "each", filter: { allOf: [{ anyOf: [{ subtypes: ["Mutant"] }, { subtypes: ["Ninja"] }, { subtypes: ["Turtle"] }] }], cls: "spell" } },
        from: topOfLibrary, riders: [{ rider: "entry", appliesTo: { types: ["creature"], cls: "spell" },
          counters: [{ count: one, counter: { type: "+1/+1" }, additional: true }] }] } }] } });
    assert.deepEqual(ability("Once during each of your turns, you may cast an artifact spell from your graveyard. If you cast a spell this way, that artifact enters tapped.", "Legendary Creature — Human Artificer Noble"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast",
        what: { ref: "filter", filter: { types: ["artifact"], cls: "spell" }, count: one }, from: { zone: "graveyard", owner: "your" },
        useLimit: { maximum: 1, period: "turn", during: "your" }, riders: [{ rider: "entry", tapped: true }] } } });
  });

  it("attaches an entry rider to a lasting resolving permission instead of testing it on resolution", () => {
    assert.deepEqual(ability("{T}: You may play target snow permanent card from your graveyard this turn. If you do, it enters tapped.", "Legendary Snow Artifact"), {
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "grant-play-permission", duration: { duration: "this-turn" }, permission: {
        who: you, action: "play", what: { ref: "target", filter: { supertypes: ["snow"], cls: "card", permanent: true }, count: one },
        from: { zone: "graveyard", owner: "your" }, riders: [{ rider: "entry", tapped: true }] } }] }] });
  });

  it("represents the off-timing sacrifice of a spell cast as though it had flash", () => {
    assert.deepEqual(ability("You may cast this spell as though it had flash. If you cast it any time a sorcery couldn't have been cast, the controller of the permanent it becomes sacrifices it at the beginning of the next cleanup step.", "Enchantment — Aura"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast", what: thisSpell, timing: "as-though-flash",
        riders: [{ rider: "cast-off-sorcery-timing", effects: [{ steps: [{ effect: "create-delayed-trigger",
          trigger: { trigger: "phase", phase: "cleanup", whose: "each-player" }, occurrence: "next", nextOnly: true, targets: "enclosing-ability",
          effects: [{ steps: [{ effect: "sacrifice", who: { player: "controller", of: { ref: "it" } }, what: { ref: "it" } }] }] }] }] }] } } });
  });

  it("rejects riders it cannot attach faithfully", () => {
    for (const text of [
      // The noun must be guaranteed by the permission or the rider's filter.
      "You may cast artifact spells from your graveyard. If you cast a spell this way, that creature enters tapped.",
      // A filtered rider cannot become a permission-wide payment rule.
      "You may cast spells from the top of your library. If you cast a creature spell this way, pay life equal to its mana value rather than pay its mana cost.",
      // Two alternative costs for one permission.
      "You may cast spells from your graveyard without paying their mana costs. If you cast a spell this way, pay life equal to its mana value rather than pay its mana cost.",
      "You may cast spells from the top of your library. If you cast a spell this way, draw a card.",
      "You may cast spells from the top of your library. If you cast it any time a sorcery couldn't have been cast, the controller of the permanent it becomes sacrifices it at the beginning of the next cleanup step.",
    ]) rejected(text, "Enchantment");
    // Formerly rejected here as "the pinned regression": a static permission
    // for this one card followed by "If you do, it enters with …" is now that
    // permission's entry rider (CR 614.1c, 614.12).
    assert.deepEqual(ability("You may cast this card from your graveyard. If you do, it enters with a finality counter on it.", "Creature — Zombie Warrior"), {
      kind: "static", effect: { static: "play-permission", permission: { who: you, action: "cast", what: { ref: "this", noun: "card" },
        from: { zone: "graveyard", owner: "your" }, riders: [{ rider: "entry", counters: [{ count: one, counter: { type: "finality" } }] }] } } });
    // Any other consequence of using a static permission stays rejected.
    rejected("You may cast this card from your graveyard. If you do, draw a card.", "Creature — Zombie Warrior");
    rejected("You may cast creature spells from your graveyard. If you do, it enters with a finality counter on it.", "Enchantment");
    // An outcome-dependent instruction after a lasting permission stays rejected.
    rejected("Exile the top card of your library. You may play that card this turn. If you do, draw a card.", "Sorcery");
    rejected("Exile the top card of your library. You may play that card this turn. If you don't, draw a card.", "Sorcery");
  });
});
