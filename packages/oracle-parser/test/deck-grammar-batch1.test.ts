import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Stage 4 deck-ranked grammar batch 1. Positives are exact frozen R5 Oracle
// text of the named cards; each production has negative neighbours that must
// keep failing. Rust owns meaning and rejects every new shape it does not
// lower (rework plan, Stage 4).
// CR 108.4, 118.12, 120.4a, 122.5, 201.5, 401.4, 508.1, 508.3a, 603.4,
// 607.2a, 607.2d, 608.2c, 608.2d, 614.12, 701.36a, 702.33d, 903.3:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" } as const;

function parsed(text: string, typeLine: string, name = "W1 fixture"): Ability[] {
  const result = parseOracleText(text, name, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines.filter(line => !line.ok)));
  return JSON.parse(JSON.stringify(result.abilities));
}
function rejected(text: string, typeLine: string, name = "W1 fixture"): void {
  assert.equal(parseOracleText(text, name, { typeLine }).ok, false, text);
}
const step = (abilities: Ability[], index = 0): unknown => {
  const ability = abilities[index] as { effects?: { steps: unknown[] }[] };
  return ability.effects?.[0]?.steps[0];
};

describe("object controllers described by player history and choices", () => {
  it("tests each object's own controller (Steel Hellkite)", () => {
    const [ability] = parsed("{X}: Destroy each nonland permanent with mana value X whose controller was dealt combat damage by this creature this turn. Activate only once each turn.",
      "Artifact Creature — Dragon");
    assert.deepEqual((step([ability]) as { what: unknown }).what, { ref: "each", filter: {
      nonTypes: ["land"], cls: "permanent",
      properties: [{ property: "mana-value", comparison: { op: "eq", value: { amount: "x" } } }],
      controllerMatches: { damageHistory: { role: "recipient", window: { during: "this-turn" }, damage: "combat", source: { ref: "this", noun: "creature" } } },
    } });
    // Only the passive recipient reading describes the controller.
    rejected("Destroy each creature whose controller dealt combat damage to you this turn.", "Sorcery");
    rejected("Destroy each creature whose controller has hexproof.", "Sorcery");
    rejected("Destroy each creature you control whose controller was dealt damage this turn.", "Sorcery");
  });

  it("reads a named-option choice and who chose it (Archangel of Strife, Seize the Spotlight)", () => {
    const abilities = parsed("Flying\nAs this creature enters, each player chooses war or peace.\nCreatures controlled by players who chose war get +3/+0.\nCreatures controlled by players who chose peace get +0/+3.",
      "Creature — Angel");
    assert.deepEqual(abilities[1], { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "creature" },
      effects: [{ steps: [{ effect: "choose-option", who: { player: "each-player" }, options: ["war", "peace"] }] }] } });
    assert.deepEqual((abilities[2] as { effect: { what: unknown } }).effect.what, { ref: "each", filter: {
      types: ["creature"], controllerMatches: { choseOption: { option: "war" } } } });
    const spotlight = parsed("Each opponent chooses fame or fortune. For each player who chose fame, gain control of a creature that player controls until end of turn. Untap those creatures and they gain haste until end of turn. For each player who chose fortune, you draw a card and create a Treasure token.",
      "Sorcery");
    assert.deepEqual(step(spotlight), { effect: "choose-option", who: { player: "each-opponent" }, options: ["fame", "fortune"] });
    // Subtypes, directions, modes and values keep their own productions.
    assert.notDeepEqual(step(parsed("When this creature enters, choose Island or Swamp.", "Creature — Bear")),
      { effect: "choose-option", who: you, options: ["island", "swamp"] });
    rejected("When this creature enters, choose war, peace, or famine.", "Creature — Bear");
    rejected("When this creature enters, choose war or war.", "Creature — Bear");
    rejected("Creatures controlled by players who chose war and peace get +3/+0.", "Creature — Angel");
  });
});

describe("counters, entries and amounts", () => {
  it("divides moved counters among chosen recipients (Forgotten Ancient)", () => {
    const [, upkeep] = parsed("Whenever a player casts a spell, you may put a +1/+1 counter on this creature.\nAt the beginning of your upkeep, you may move any number of +1/+1 counters from this creature onto other creatures.",
      "Creature — Elemental");
    assert.deepEqual(step([upkeep]), { effect: "move-counters", counter: { type: "+1/+1" }, count: { amount: "chosen", minimum: 0 },
      from: { ref: "this", noun: "creature" }, to: { ref: "each", filter: { other: true, types: ["creature"] } },
      split: "chosen-recipients", optional: true });
    // A fixed count, every counter, or an explicit "each" is not a division.
    rejected("Move two +1/+1 counters from this creature onto other creatures.", "Instant");
    rejected("Move all +1/+1 counters from this creature onto other creatures.", "Instant");
    rejected("Move any number of +1/+1 counters from this creature onto each other creature.", "Instant");
  });

  it("measures the entry counters after the as-enters instructions (Thief of Blood, Canker Abomination)", () => {
    const [, entry] = parsed("Flying\nAs this creature enters, remove all counters from all permanents. This creature enters with a +1/+1 counter on it for each counter removed this way.",
      "Creature — Vampire");
    assert.deepEqual(entry, { kind: "static", effect: { static: "as-enters", what: { ref: "this", noun: "creature" },
      effects: [{ steps: [{ effect: "remove-counters", counter: "any", count: "all", from: { ref: "all", filter: { cls: "permanent" } } }] }],
      entersWith: { static: "enters-with-counters", what: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: fixed(1),
        multiplier: { amount: "event-quantity", event: "counter-removal", counter: "any", scope: "this-way" } } } });
    assert.equal(parsed("As this creature enters, choose an opponent. This creature enters with a -1/-1 counter on it for each creature that player controls.",
      "Creature — Horror").length, 1);
    // The trailing entry must be the same object's, and unconditional.
    rejected("As this creature enters, choose an opponent. Target creature enters with a +1/+1 counter on it.", "Creature — Horror");
    rejected("As this creature enters, choose an opponent. This creature enters with a +1/+1 counter on it if that player has 10 or less life.", "Creature — Horror");
  });

  it("shares a trailing amount between two instructions of one actor (Lifeblood Hydra)", () => {
    const [, , dies] = parsed("Trample\nThis creature enters with X +1/+1 counters on it.\nWhen this creature dies, you gain life and draw cards equal to its power.",
      "Creature — Hydra");
    const power = { amount: "attribute", of: { ref: "it" }, attribute: "power" };
    assert.deepEqual((dies as { effects: unknown[] }).effects, [{ steps: [
      { effect: "gain-life", who: you, amount: power }, { effect: "draw", who: you, amount: power }] }]);
    rejected("When this creature dies, you gain life and draw two cards.", "Creature — Hydra");
    rejected("When this creature dies, you gain life and lose life equal to its power.", "Creature — Hydra");
    rejected("When this creature dies, you gain life, then draw cards equal to its power.", "Creature — Hydra");
  });

  it("reads the greatest mana value among your commanders (Tangleweave Armor)", () => {
    const abilities = parsed("Living weapon\nEquipped creature gets +X/+X, where X is the greatest mana value among your commanders.\nEquip {4}",
      "Artifact — Equipment");
    assert.deepEqual((abilities[1] as { effect: { bindings: unknown } }).effect.bindings, [{ variable: "x",
      value: { amount: "attribute-extremum", extremum: "maximum", attribute: "mana-value", among: { commander: true, own: "you" } } }]);
    rejected("Equipped creature gets +X/+X, where X is the greatest mana value among their commanders.", "Artifact — Equipment");
  });

  it("adds one target per kicker payment (Comet Storm, Strength of the Tajuru)", () => {
    const comet = parsed("Multikicker {1}\nChoose any target, then choose another target for each time this spell was kicked. Comet Storm deals X damage to each of them.",
      "Instant", "Comet Storm");
    const count = { amount: "plus", a: fixed(1), b: { amount: "kicker-count", of: { ref: "this", noun: "spell" } } };
    assert.deepEqual((step(comet, 1) as { what: unknown }).what, { ref: "any-target", count });
    const tajuru = parsed("Multikicker {1}\nChoose target creature, then choose another target creature for each time this spell was kicked. Put X +1/+1 counters on each of them.",
      "Instant");
    assert.deepEqual((step(tajuru, 1) as { what: { count: unknown } }).what.count, count);
    // The added targets restate the first description exactly.
    rejected("Multikicker {1}\nChoose target creature, then choose another target artifact for each time this spell was kicked. Destroy them.", "Instant");
    rejected("Multikicker {1}\nChoose up to one target creature, then choose another target creature for each time this spell was kicked. Destroy them.", "Instant");
    rejected("Multikicker {1}\nChoose any target, then choose another target for each creature you control. Testcard deals 1 damage to each of them.", "Instant");
  });
});

describe("conditions and riders", () => {
  it("tests the triggering declaration's defenders (Ever-Watching Threshold)", () => {
    const [ability] = parsed("Whenever an opponent attacks, if they attacked you and/or a planeswalker you control, draw a card.", "Enchantment");
    assert.deepEqual((ability as { condition: unknown }).condition, { condition: "declaration-attacked", who: { player: "they" }, defenders: [
      { defender: "player", who: you },
      { defender: "object", what: { ref: "filter", filter: { types: ["planeswalker"], control: "you" }, count: fixed(1) } },
    ] });
    // Without a declaration trigger the past tense names no period.
    rejected("Whenever a creature attacks, if they attacked you, draw a card.", "Enchantment");
    rejected("At the beginning of your upkeep, if an opponent attacked you, draw a card.", "Enchantment");
  });

  it("admits a tied mana-value extremum for one permanent type (Padeem, Consul of Innovation)", () => {
    const [, upkeep] = parsed("Artifacts you control have hexproof.\nAt the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the greatest mana value, draw a card.",
      "Legendary Creature — Vedalken Artificer");
    assert.deepEqual((upkeep as { condition: unknown }).condition, { condition: "control", who: you, what: { ref: "filter", filter: {
      types: ["artifact"], properties: [{ property: "mana-value", comparison: { op: "eq", value: {
        amount: "attribute-extremum", extremum: "maximum", attribute: "mana-value", among: { types: ["artifact"] } } } }] }, count: fixed(1) } });
    // Power belongs to creatures, and the restated extremum must match.
    rejected("At the beginning of your upkeep, if you control the artifact with the greatest power or tied for the greatest power, draw a card.", "Creature — Bear");
    rejected("At the beginning of your upkeep, if you control the artifact with the greatest mana value or tied for the least mana value, draw a card.", "Creature — Bear");
  });

  it("shares one subject and period across two history clauses (Laboratory Drudge)", () => {
    const [ability] = parsed("At the beginning of each end step, draw a card if you've cast a spell from a graveyard or activated an ability of a card in a graveyard this turn.",
      "Creature — Zombie Horror");
    const condition = (ability as { effects: { condition: { condition: string; conditions: { condition: string }[] } }[] }).effects[0].condition;
    assert.equal(condition.condition, "any-of");
    assert.deepEqual(condition.conditions.map(each => each.condition), ["cast-count", "player-action-history"]);
    rejected("At the beginning of each end step, draw a card if you've cast a spell from a graveyard or activated an ability of a card in a graveyard.", "Creature — Zombie Horror");
    rejected("At the beginning of each end step, draw a card if you've cast a spell from a graveyard or activated an ability of a card in a graveyard this turn and this combat.", "Creature — Zombie Horror");
  });

  it("restates the unable instruction (Xathrid Demon)", () => {
    const [, upkeep] = parsed("Flying, trample\nAt the beginning of your upkeep, sacrifice a creature other than this creature, then each opponent loses life equal to the sacrificed creature's power. If you can't sacrifice a creature, tap this creature and you lose 7 life.",
      "Creature — Demon");
    assert.deepEqual((upkeep as { effects: { condition?: unknown }[] }).effects[1].condition, { condition: "restated-action-unable", who: you,
      action: { action: "sacrifice", what: { ref: "filter", filter: { types: ["creature"] }, count: fixed(1) } } });
  });

  it("conditions an excess-damage redirection (Ram Through)", () => {
    const [ability] = parsed("Target creature you control deals damage equal to its power to target creature you don't control. If the creature you control has trample, excess damage is dealt to that creature's controller instead.",
      "Instant");
    assert.deepEqual((step([ability]) as { excessCondition: unknown }).excessCondition, { condition: "has-keyword",
      what: { ref: "that", filter: { types: ["creature"], control: "you" } }, keyword: "trample" });
    rejected("Target creature you control deals damage equal to its power to target creature you don't control. If you control an artifact, excess damage is dealt to that creature's controller instead.", "Instant");
  });

  it("reads players by their cast and attack history (Angelic Arbiter)", () => {
    const [, cast, attacked] = parsed("Flying\nEach opponent who cast a spell this turn can't attack with creatures.\nEach opponent who attacked with a creature this turn can't cast spells.",
      "Creature — Angel");
    assert.deepEqual((cast as { effect: unknown }).effect, { static: "player-combat-rule",
      who: { player: "filtered", base: { player: "each-opponent" }, filter: { castHistory: { what: { cls: "spell" }, during: "this-turn" } } },
      rule: { rule: "attack-limit", maximum: fixed(0) } });
    assert.deepEqual((attacked as { effect: { prohibition: { who: unknown } } }).effect.prohibition.who, { player: "filtered",
      base: { player: "each-opponent" }, filter: { attackHistory: { with: { types: ["creature"] }, during: "this-turn" } } });
    rejected("Each opponent who attacked with a land this turn can't cast spells.", "Creature — Angel");
    rejected("Each opponent who cast a spell can't attack with creatures.", "Creature — Angel");
    rejected("Each opponent can't attack with creatures this turn or with lands.", "Creature — Angel");
  });
});

describe("linked objects and placements", () => {
  it("names the creature a linked ability put onto the battlefield (Diabolic Servitude)", () => {
    const abilities = parsed("When this enchantment enters, return target creature card from your graveyard to the battlefield.\nWhen the creature put onto the battlefield with this enchantment dies, exile it and return this enchantment to its owner's hand.\nWhen this enchantment leaves the battlefield, exile the creature put onto the battlefield with this enchantment.",
      "Enchantment");
    const linked = { ref: "related", number: "singular", filter: { types: ["creature"] },
      relation: { relation: "linked-entry", source: { ref: "this", noun: "enchantment" } } };
    assert.deepEqual((abilities[1] as { trigger: { what: unknown } }).trigger.what, linked);
    assert.deepEqual((step(abilities, 2) as { what: unknown }).what, linked);
    rejected("When the creature put onto the battlefield with target artifact dies, draw a card.", "Enchantment");
  });

  it("copies the spell its own cast trigger names (Malicious Affliction)", () => {
    const [trigger] = parsed("Morbid — When you cast this spell, if a creature died this turn, you may copy Malicious Affliction and may choose a new target for the copy.\nDestroy target nonblack creature.",
      "Instant", "Malicious Affliction");
    assert.deepEqual((step([trigger]) as { what: unknown }).what, { ref: "this", noun: "spell" });
    // Outside the spell's own cast trigger its name is not a stack object.
    rejected("When this creature enters, you may copy Testcard.", "Creature — Bear", "Testcard");
    rejected("Whenever you cast a creature spell, you may copy Testcard.", "Creature — Bear", "Testcard");
  });

  it("enters the populated token tapped and attacking (Ghired, Conclave Exile)", () => {
    const [, attacks] = parsed("When Ghired enters, create a 4/4 green Rhino creature token with trample.\nWhenever Ghired attacks, populate. The token enters tapped and attacking.",
      "Legendary Creature — Human Shaman", "Ghired, Conclave Exile");
    assert.deepEqual(step([attacks]), { effect: "keyword-action", action: "populate", who: you, token: { tapped: true, attacking: true } });
    rejected("Whenever this creature attacks, proliferate. The token enters tapped and attacking.", "Creature — Bear");
    rejected("Whenever this creature attacks, populate. The tokens enter tapped and attacking.", "Creature — Bear");
  });

  it("returns the rest to the bottom of the looked-at library (Loot, Exuberant Explorer)", () => {
    const abilities = parsed("You may play an additional land on each of your turns.\n{4}{G}{G}, {T}: Look at the top six cards of your library. You may reveal a creature card with mana value less than or equal to the number of lands you control from among them and put it onto the battlefield. Put the rest on the bottom in a random order.",
      "Legendary Creature — Beast Noble");
    const effects = (abilities[1] as { effects: { steps: { what?: { filter?: unknown } }[] }[] }).effects;
    assert.deepEqual(effects[2].steps[0], { effect: "move-zone", what: { ref: "rest" },
      to: { zone: "library", owner: "your", position: "bottom" }, order: "random" });
    // The pool qualifies the revealed card, not the counted lands.
    const revealed = (effects[1].steps[0] as unknown as { steps: { what: { filter: { fromAmong?: unknown; properties: unknown } } }[] }).steps[0];
    assert.deepEqual(revealed.what.filter.fromAmong, { ref: "them" });
    assert.deepEqual(revealed.what.filter.properties, [{ property: "mana-value", comparison: { op: "le",
      value: { amount: "count", of: { types: ["land"], control: "you" } } } }]);
    // Without one earlier look the library is unstated.
    rejected("Put the rest on the bottom in a random order.", "Sorcery");
    rejected("Look at the top six cards of your library. Look at the top card of target player's library. Put the rest on the bottom in a random order.", "Sorcery");
  });

  it("groups you and that player as sacrificing actors (Infernal Offering)", () => {
    const abilities = parsed("Choose an opponent. You and that player each sacrifice a creature. Each player who sacrificed a creature this way draws two cards.\nChoose an opponent. Return a creature card from your graveyard to the battlefield, then that player returns a creature card from their graveyard to the battlefield.",
      "Sorcery");
    const sacrifice = (abilities[0] as { effects: { steps: { who: unknown }[] }[] }).effects[1].steps[0];
    assert.deepEqual(sacrifice.who, { player: "group", members: [you, { player: "that-player" }], multiplicity: "per-member-occurrence" });
    rejected("Choose an opponent. You and each opponent each sacrifice a creature.", "Sorcery");
  });
});
