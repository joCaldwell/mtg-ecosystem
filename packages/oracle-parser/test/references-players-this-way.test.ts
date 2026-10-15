import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const you = { player: "you" } as const;
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const result = (event: string, extra: object = {}) => ({ relation: "event-result", event, scope: "this-way", ...extra });

// Oracle texts are complete cards from the fixed Scryfall corpus.
// "This way" links a later clause to what an earlier instruction of the same
// spell or ability actually did (CR 608.2c, 609.3), looking back at the
// objects as that event happened to them (608.2h-i):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("objects tapped, returned, countered or drawn this way", () => {
  it("counts what Harmony of Nature tapped, not creatures that are tapped", () => {
    assert.deepEqual(parsed("Tap any number of untapped creatures you control. You gain 4 life for each creature tapped this way.", "Harmony of Nature", "Sorcery"), [{ kind: "spell", effects: [
      { steps: [{ effect: "tap", what: { ref: "filter", filter: { status: ["untapped"], types: ["creature"], control: "you" }, count: { amount: "chosen", minimum: 0 } } }] },
      { steps: [{ effect: "gain-life", who: you, amount: n(4), forEach: { types: ["creature"], relation: result("tap") } }] },
    ] }]);
    // The bare adjective remains the current status.
    assert.deepEqual(parsed("You gain 4 life for each tapped creature you control.", undefined, "Sorcery")[0].effects[0].steps[0].forEach,
      { status: ["tapped"], types: ["creature"], control: "you" });
  });
  it("measures Barrel Down Sokenzan's returned Mountains", () => {
    assert.deepEqual(parsed("Sweep — Return any number of Mountains you control to their owner's hand. Barrel Down Sokenzan deals damage to target creature equal to twice the number of Mountains returned this way.", "Barrel Down Sokenzan", "Instant — Arcane")[0].effects[1].steps[0].amount,
      { amount: "twice", of: { amount: "count", of: { subtypes: ["Mountain"], relation: result("return") } } });
  });
  it("draws for each spell Swift Silence countered", () => {
    assert.deepEqual(parsed("Counter all other spells. Draw a card for each spell countered this way.", "Swift Silence", "Instant")[0].effects[1], {
      steps: [{ effect: "draw", who: you, amount: n(1), forEach: { cls: "spell", relation: result("counter") } }],
    });
  });
  it("discards for each card Ancient Excavation drew", () => {
    assert.deepEqual(parsed("Draw cards equal to the number of cards in your hand, then discard a card for each card drawn this way.", "Ancient Excavation", "Instant")[0].effects[0].steps[1], {
      effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: n(1) }, forEach: { cls: "card", relation: result("draw") },
    });
  });
  it("requires the explicit link for these participles", () => {
    fails("Draw a card for each spell countered.");
    fails("Draw a card for each Mountain returned this turn.");
    fails("You gain 4 life for each creature tapped that way.");
  });
});

describe("outcome conditions linked by \"this way\"", () => {
  const atLeast = (value: number) => ({ op: "ge", value: n(value) });
  const sentence = (text: string, typeLine: string, index = 1) => parsed(text, undefined, typeLine)[0].effects[index];
  it("tests what Deathgorge Scavenger's optional exile actually exiled", () => {
    assert.deepEqual(parsed("Whenever this creature enters or attacks, you may exile target card from a graveyard. If a creature card is exiled this way, you gain 2 life. If a noncreature card is exiled this way, this creature gets +1/+1 until end of turn.", "Deathgorge Scavenger", "Creature — Dinosaur")[0].effects, [
      { steps: [{ effect: "exile", what: { ref: "target", filter: { cls: "card", zone: { zone: "graveyard", owner: "any" } }, count: n(1) }, optional: true }] },
      { steps: [{ effect: "gain-life", who: you, amount: n(2) }], conditionKind: "if",
        condition: { condition: "count", of: { types: ["creature"], cls: "card", relation: result("exile") }, comparison: atLeast(1) } },
      { steps: [{ effect: "pump", what: { ref: "this", noun: "creature" }, power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) }, duration: { duration: "end-of-turn" } }], conditionKind: "if",
        condition: { condition: "count", of: { nonTypes: ["creature"], cls: "card", relation: result("exile") }, comparison: atLeast(1) } },
    ]);
  });
  it("treats present and past tense as the same test", () => {
    const was = sentence("Exile target card from a graveyard. If a creature card was exiled this way, you gain 2 life.", "Sorcery");
    const is = sentence("Exile target card from a graveyard. If a creature card is exiled this way, you gain 2 life.", "Sorcery");
    assert.deepEqual(was, is);
  });
  it("restricts Garruk's test to what you sacrificed", () => {
    assert.deepEqual(sentence("Each player sacrifices a creature. If you sacrificed a creature this way, draw a card.", "Sorcery").condition,
      { condition: "count", of: { types: ["creature"], relation: result("sacrifice", { who: you }) }, comparison: atLeast(1) });
    // Without an actor, anyone's sacrifice counts.
    assert.deepEqual(sentence("Each player sacrifices a creature. If a Goblin is sacrificed this way, draw a card.", "Sorcery").condition,
      { condition: "count", of: { subtypes: ["Goblin"], relation: result("sacrifice") }, comparison: atLeast(1) });
  });
  it("keeps another player's present-tense discard and its card filter", () => {
    assert.deepEqual(sentence("{1}, {T}: Target player draws a card, then discards a card. If that player discards an artifact card this way, untap this creature.", "Creature — Vedalken Wizard").condition,
      { condition: "count", of: { types: ["artifact"], cls: "card", relation: result("discard", { who: { player: "that-player" } }) }, comparison: atLeast(1) });
  });
  it("negates Trade Route Envoy's draw outcome", () => {
    assert.deepEqual(sentence("When this creature enters, draw a card if you control a creature with a counter on it. If you don't draw a card this way, put a +1/+1 counter on this creature.", "Creature — Dog Soldier").condition,
      { condition: "not", of: { condition: "count", of: { cls: "card", relation: result("draw", { who: you }) }, comparison: atLeast(1) } });
  });
  it("reads a bare number as that many or more and keeps explicit bounds", () => {
    assert.deepEqual(parsed("Tap six untapped creatures you control: Return The Cinematic Phoenix from your graveyard to the battlefield. If you tapped six legendary creatures this way, you win the game.", "The Cinematic Phoenix", "Legendary Creature — Phoenix")[0].effects[1].condition,
      { condition: "count", of: { supertypes: ["legendary"], types: ["creature"], relation: result("tap", { who: you }) }, comparison: atLeast(6) });
    assert.deepEqual(sentence("Target opponent discards two cards. If fewer than two cards were discarded this way, you draw a card.", "Sorcery").condition,
      { condition: "count", of: { cls: "card", relation: result("discard") }, comparison: { op: "lt", value: n(2) } });
    assert.deepEqual(sentence("Target opponent discards two cards. If no cards were discarded this way, you draw a card.", "Sorcery").condition,
      { condition: "count", of: { cls: "card", relation: result("discard") }, comparison: { op: "eq", value: n(0) } });
  });
  it("tests one identified object against the result", () => {
    assert.deepEqual(sentence("At the beginning of your upkeep, destroy this creature unless you pay {3}{B}{B}{B}. If this creature is destroyed this way, it deals 7 damage to you.", "Creature — Horror").condition,
      { condition: "is-filter", what: { ref: "this", noun: "creature" }, filter: { relation: result("destroy") } });
    // Dying is the move to the graveyard, which a destruction may not cause.
    assert.deepEqual(sentence("Destroy target creature. If that creature dies this way, you gain 2 life.", "Sorcery").condition,
      { condition: "is-filter", what: { ref: "that", filter: { types: ["creature"] } }, filter: { relation: result("die") } });
    assert.deepEqual(sentence("Destroy target creature. If a white creature dies this way, you gain 2 life.", "Sorcery").condition,
      { condition: "count", of: { colors: ["white"], types: ["creature"], relation: result("die") }, comparison: atLeast(1) });
  });
  it("uses the damage history of Marauding Raptor's recipient", () => {
    assert.deepEqual(sentence("Whenever another creature you control enters, this creature deals 2 damage to it. If a Dinosaur is dealt damage this way, this creature gets +2/+0 until end of turn.", "Creature — Dinosaur").condition,
      { condition: "count", of: { subtypes: ["Dinosaur"], damageHistory: { role: "recipient", window: { event: "this-way" } } }, comparison: atLeast(1) });
  });
  it("rejects outcomes it cannot state completely", () => {
    // Players are not objects; a destination or a shared characteristic is a further restriction.
    fails("Lightning Blast deals 2 damage to any target. If a player is dealt damage this way, you draw a card.");
    fails("Return target creature card from your graveyard to your hand. If a creature card was returned to your hand this way, draw a card.");
    fails("Target player mills two cards. If two cards that share a color were milled this way, draw a card.");
    // The destination of a passive "is put into <zone> this way" is now the zone-change relation's own.
    assert.deepEqual(parsed("Destroy target creature. If a creature card is put into a graveyard this way, draw a card.", undefined, "Sorcery")[0].effects[1], {
      steps: [{ effect: "draw", who: you, amount: n(1) }], conditionKind: "if",
      condition: { condition: "count", of: { types: ["creature"], cls: "card", relation: { relation: "zone-change", to: { zone: "graveyard", owner: "any" }, scope: "this-way" } },
        comparison: { op: "ge", value: n(1) } } });
    // Outside resolving instructions there is no earlier instruction to look back at.
    fails("This land enters tapped unless you revealed a Dragon card this way.", "Land");
    // No link, no outcome test.
    fails("Exile target card from a graveyard. If a creature card is exiled, you gain 2 life.");
    fails("Exile target card from a graveyard. If a creature card is exiled that way, you gain 2 life.");
  });
});

describe("players selected by what they did this way", () => {
  const eachPlayer = { player: "each-player" } as const;
  const who = (base: object, of: object, comparison: object) => ({ player: "filtered", base, filter: { eventResults: { of, comparison } } });
  const none = { op: "eq", value: n(0) };
  const some = { op: "ge", value: n(1) };
  it("punishes the players who discarded no creature card (Strongarm Tactics)", () => {
    assert.deepEqual(parsed("Each player discards a card. Then each player who didn't discard a creature card this way loses 4 life.", "Strongarm Tactics", "Sorcery")[0].effects[1], { steps: [{
      effect: "lose-life", who: who(eachPlayer, { types: ["creature"], cls: "card", relation: result("discard") }, none), amount: n(4) }] });
  });
  it("rewards the players who did draw (Kwain, Itinerant Meddler)", () => {
    assert.deepEqual(parsed("{T}: Each player may draw a card, then each player who drew a card this way gains 1 life.", "Kwain, Itinerant Meddler", "Legendary Creature — Rabbit Wizard")[0].effects[0].steps, [
      { effect: "draw", who: eachPlayer, amount: n(1), optional: true, decidedBy: eachPlayer },
      { effect: "gain-life", who: who(eachPlayer, { cls: "card", relation: result("draw") }, some), amount: n(1) },
    ]);
  });
  it("uses the selection as a damage recipient (Impulsive Destruction)", () => {
    assert.deepEqual(parsed("Each player may sacrifice an artifact or enchantment. Impulsive Destruction deals 3 damage to each player who didn't sacrifice a permanent this way.", "Impulsive Destruction", "Instant")[0].effects[1].steps[0].to,
      { to: "player", ref: who(eachPlayer, { cls: "permanent", relation: result("sacrifice") }, none) });
  });
  it("rejects a negated number, a missing link and other verbs", () => {
    fails("Each opponent discards two cards. Then each opponent who didn't discard two nonland cards this way loses 3 life.");
    fails("Each player discards a card. Then each player who didn't discard a creature card loses 4 life.");
    fails("Each player discards a card. Then each player who kept a creature card this way loses 4 life.");
  });
});

// CR 603.12: "when [something happens] this way" creates a reflexive
// triggered ability; it is checked once against what the earlier instruction
// of the same resolution did, and its targets are chosen when it triggers.
describe("reflexive triggers on what was done this way", () => {
  const discarded = (filter: object) => ({ event: "event-result", comparison: { op: "ge", value: n(1) },
    of: { ...filter, cls: "card", relation: result("discard", { who: you }) } });
  it("triggers Evie Frye's reflexive ability only for a discarded creature card", () => {
    assert.deepEqual(parsed("{1}, {T}: Draw a card, then discard a card. When you discard a creature card this way, target creature you control can't be blocked this turn.", "Evie Frye", "Legendary Creature — Human Assassin")[0].effects, [
      { steps: [{ effect: "draw", who: you, amount: n(1) }, { effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: n(1) } }] },
      { steps: [{ effect: "create-reflexive-trigger", event: discarded({ types: ["creature"] }), effects: [{ steps: [{
        effect: "cant", what: { ref: "target", filter: { types: ["creature"], control: "you" }, count: n(1) }, action: "be-blocked", duration: { duration: "this-turn" } }] }] }] },
    ]);
  });
  it("keeps \"one or more\" as the event's quantity (Ravenous Rotbelly)", () => {
    assert.deepEqual(parsed("When this creature enters, you may sacrifice up to three Zombies. When you sacrifice one or more Zombies this way, each opponent sacrifices that many creatures of their choice.", "Ravenous Rotbelly", "Creature — Zombie Horror")[0].effects[1].steps[0].event,
      { event: "event-result", of: { subtypes: ["Zombie"], relation: result("sacrifice", { who: you }) }, comparison: { op: "ge", value: n(1) } });
  });
  it("differs from the bare reflexive \"when you do\" and from a conditional instruction", () => {
    assert.deepEqual(parsed("You may discard a card. When you do, draw a card.", undefined, "Sorcery")[0].effects[1].steps[0].event,
      { event: "preceding-action", who: you, performed: true });
    assert.equal(parsed("Discard a card. If you discard a creature card this way, draw a card.", undefined, "Sorcery")[0].effects[1].condition.condition, "count");
  });
  it("rejects negated, unlinked and state-like \"when\" clauses", () => {
    fails("Discard a card. When you don't discard a creature card this way, draw a card.");
    fails("Discard a card. When a creature card is discarded this way, draw a card.");
  });
});
