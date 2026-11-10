import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseOracleText } from "../src/parser/index.ts";

// Every expected value is the complete
// AST of the line. Rule numbers refer to the Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const ability = (text: string, typeLine = "Sorcery") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};
const fails = (text: string, typeLine = "Sorcery") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.ok, false, `${text} must not be accepted`);
};
const n = (value: number) => ({ amount: "fixed", value });
const pt = (power: number, toughness: number) => ({ modifier: "pt",
  power: { sign: power < 0 ? -1 : 1, amount: n(Math.abs(power)) }, toughness: { sign: toughness < 0 ? -1 : 1, amount: n(Math.abs(toughness)) } });

describe("static additional power/toughness (CR 613.4c)", () => {
  // Heavy Mattock, Patriarch's Desire: "an additional" modification is one
  // more cumulative modifier; the condition stays on this ability alone.
  it("keeps the modifier and its own condition", () => {
    assert.deepEqual(ability("As long as equipped creature is a Human, it gets an additional +1/+0.", "Artifact — Equipment"),
      { kind: "static", effect: { static: "conditional",
        condition: { condition: "is-filter", what: { ref: "equipped", noun: "creature" }, filter: { subtypes: ["Human"] } },
        effect: { static: "modify", what: { ref: "it" }, modifiers: [pt(1, 0)] } } });
    assert.deepEqual(ability("Threshold — Enchanted creature gets an additional +2/-2 as long as there are seven or more cards in your graveyard.", "Enchantment — Aura"),
      { kind: "static", abilityWord: "Threshold", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [pt(2, -2)],
        condition: { condition: "count", of: { cls: "card", zone: { zone: "graveyard", owner: "your" } }, comparison: { op: "ge", value: n(7) } } } });
  });
  it("does not accept other 'additional' static wordings through it", () => {
    fails("Enchanted creature gets an additional.", "Enchantment — Aura");
    fails("Enchanted creature has an additional flying.", "Enchantment — Aura");
  });
});

describe("pooled counts", () => {
  // Multani, Maro-Sorcerer; Realm Seekers (CR 400.1: one hand per player).
  it("counts the cards of all players' hands as one set", () => {
    const value = { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "all-players" } } };
    assert.deepEqual(ability("~'s power and toughness are each equal to the total number of cards in all players' hands.", "Creature — Elemental"),
      { kind: "static", effect: { static: "cda-pt", what: { ref: "self" }, stat: "both", value } });
    assert.deepEqual(ability("~'s power and toughness are each equal to the number of cards in all hands.", "Creature — Elemental"),
      { kind: "static", effect: { static: "cda-pt", what: { ref: "self" }, stat: "both", value } });
  });
  it("still rejects totals over players' counters and unknown pools", () => {
    fails("This creature gets +X/+X, where X is the total number of rad counters among players.", "Creature — Elemental");
    fails("~'s power and toughness are each equal to the total number of.", "Creature — Elemental");
  });
});

describe("payments stated as a quantity (CR 118.1-118.3, 119.4)", () => {
  const anyAmount = { amount: "chosen", minimum: 0 };
  // Necrodominance, Leyline Tyrant, Liege of the Hollows, Madame Null.
  it("pays a chosen or computed amount of life or mana", () => {
    assert.deepEqual(ability("At the beginning of your end step, you may pay any amount of life. If you do, draw that many cards.", "Legendary Enchantment"),
      { kind: "triggered", trigger: { trigger: "phase", phase: "end-step", whose: "your" }, effects: [
        { steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "pay-life", amount: anyAmount }], optional: true }] },
        { steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "that-much" } }], condition: { condition: "you-do" }, conditionKind: "if" }] });
    assert.deepEqual(ability("When this creature dies, you may pay any amount of {R}. When you do, it deals that much damage to any target.", "Creature — Dragon"),
      { kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } }, effects: [
        { steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "mana-amount", amount: anyAmount, unit: "R" }], optional: true }] },
        { steps: [{ effect: "create-reflexive-trigger", event: { event: "preceding-action", who: { player: "you" }, performed: true },
          effects: [{ steps: [{ effect: "damage", source: { ref: "it" }, amount: { amount: "that-much" }, to: { to: "any-target" } }] }] }] }] });
    assert.deepEqual(ability("Whenever another creature you control enters, you may pay life equal to its power. If you do, put that many +1/+1 counters on it.", "Legendary Creature — Demon"),
      { kind: "triggered", trigger: { trigger: "enters", what: { ref: "filter", filter: { other: true, types: ["creature"], control: "you" }, count: n(1) } }, effects: [
        { steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "pay-life", amount: { amount: "attribute", of: { ref: "it" }, attribute: "power" } }], optional: true }] },
        { steps: [{ effect: "put-counters", on: { ref: "it" }, counter: { type: "+1/+1" }, count: { amount: "that-much" } }], condition: { condition: "you-do" }, conditionKind: "if" }] });
    assert.deepEqual(ability("Counter target spell unless its controller pays mana equal to the greatest power among creatures you control.", "Instant"),
      { kind: "spell", effects: [{ steps: [{ effect: "counter", what: { ref: "target", filter: { cls: "spell" }, count: n(1) } }],
        condition: { condition: "pays", who: { player: "controller", of: { ref: "it" } }, cost: [{ cost: "mana-amount",
          amount: { amount: "attribute-extremum", extremum: "maximum", attribute: "power", among: { types: ["creature"], control: "you" } } }] },
        conditionKind: "unless" }] });
  });
  it("measures the mana each payer actually paid", () => {
    assert.deepEqual(ability("When this creature dies, each player may pay any amount of mana. Then each player creates a number of 1/1 green Squirrel creature tokens equal to the amount of mana they paid this way.", "Creature — Spirit"),
      { kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "creature" } }, effects: [
        { steps: [{ effect: "pay-cost", who: { player: "each-player" }, costs: [{ cost: "mana-amount", amount: anyAmount }], optional: true, decidedBy: { player: "each-player" } }] },
        { steps: [{ effect: "create-token", count: { amount: "event-quantity", event: "mana-paid", who: { player: "they" }, scope: "this-way" },
          token: { colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Squirrel"], keywords: [], power: n(1), toughness: n(1) },
          who: { player: "each-player" } }] }] });
  });
  it("rejects payments of other things or of non-unit symbols", () => {
    fails("When this creature dies, you may pay any amount of cards.", "Creature — Spirit");
    fails("When this creature dies, you may pay any amount of {T}.", "Creature — Spirit");
    fails("When this creature dies, you may pay any amount of {2}.", "Creature — Spirit");
    fails("When this creature dies, you may pay life equal.", "Creature — Spirit");
  });
});

describe("declaration limits (CR 508.1c, 509.1b, 802.3a, 802.4b)", () => {
  const limit = (rule: object) => ({ kind: "static", effect: { static: "player-combat-rule", who: { player: "each-player" }, rule } });
  // Silent Arbiter, Crawlspace, The Eternal Wanderer, Mirri, Weatherlight Duelist.
  it("limits the number of attackers or blockers in each declaration", () => {
    assert.deepEqual(ability("No more than one creature can attack each combat.", "Artifact Creature — Construct"), limit({ rule: "attack-limit", maximum: n(1) }));
    assert.deepEqual(ability("No more than two creatures can block each combat.", "World Enchantment"), limit({ rule: "block-limit", maximum: n(2) }));
    assert.deepEqual(ability("No more than two creatures can attack you each combat.", "Artifact"),
      limit({ rule: "attack-limit", maximum: n(2), defenders: [{ defender: "player", who: { player: "you" } }] }));
    assert.deepEqual(ability("No more than one creature can attack ~ each combat.", "Legendary Planeswalker — Ajani"),
      limit({ rule: "attack-limit", maximum: n(1), defenders: [{ defender: "object", what: { ref: "self" } }] }));
    assert.deepEqual(ability("As long as ~ is tapped, no more than one creature can attack you each combat.", "Legendary Creature — Cat Warrior"),
      { kind: "static", effect: { static: "conditional", condition: { condition: "status", what: { ref: "self" }, status: "tapped" },
        effect: limit({ rule: "attack-limit", maximum: n(1), defenders: [{ defender: "player", who: { player: "you" } }] }).effect } });
  });
  it("rejects limits without the period or on other actions", () => {
    fails("No more than one creature can attack.", "Enchantment");
    fails("No more than one creature can block you each combat.", "Enchantment");
    fails("No more than one creature can attack each turn.", "Enchantment");
    fails("No more than one spell can be cast each combat.", "Enchantment");
  });
});

describe("target counts at the start of a sentence (CR 115.1, 601.2c)", () => {
  // Wave of Indifference, Dwarven Song. "X" opening a sentence is a number,
  // not the chapter numeral X (CR 714.2), which is followed by "," or "—".
  it("reads X and 'one or more' as numbers of targets", () => {
    assert.deepEqual(ability("X target creatures can't block this turn."),
      { kind: "spell", effects: [{ steps: [{ effect: "cant", what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "x" } },
        action: "block", duration: { duration: "this-turn" } }] }] });
    assert.deepEqual(ability("One or more target creatures become red until end of turn.", "Instant"),
      { kind: "spell", effects: [{ steps: [{ effect: "become", what: { ref: "target", filter: { types: ["creature"] }, count: { amount: "chosen", minimum: 1 } },
        spec: { colors: ["red"] }, duration: { duration: "end-of-turn" } }] }] });
  });
  it("keeps chapter numerals and rejects an untargeted 'one or more' subject", () => {
    const draw = [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }];
    assert.deepEqual(ability("X — Draw a card.", "Enchantment — Saga"), { kind: "chapter", chapters: [10], effects: draw });
    assert.deepEqual(ability("I, II — Draw a card.", "Enchantment — Saga"), { kind: "chapter", chapters: [1, 2], effects: draw });
    fails("One or more creatures become red until end of turn.", "Instant");
  });
});
