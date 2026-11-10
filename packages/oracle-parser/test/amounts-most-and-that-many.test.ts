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

describe("players with the most of something (CR 107.1, 119.1)", () => {
  const mostControlled = (types: string[], among = { player: "each-player" }) => ({ op: "eq", value: { amount: "per-player-extremum",
    extremum: "maximum", among, count: { count: "controlled", of: { types } } } });
  // No Witnesses, Tectonic Hellion: every player tied for the most qualifies.
  it("reads 'each player who controls the most <objects>' as a per-player extremum", () => {
    const who = { player: "filtered", base: { player: "each-player" }, filter: { controls: { filter: { types: ["creature"] }, comparison: mostControlled(["creature"]) } } };
    assert.deepEqual(ability("Each player who controls the most creatures investigates. Then destroy all creatures."),
      { kind: "spell", effects: [{ steps: [{ effect: "keyword-action", action: "investigate", who }] },
        { steps: [{ effect: "destroy", what: { ref: "all", filter: { types: ["creature"] } } }] }] });
  });
  // Hooded Horror: one identified player compared with every player.
  it("reads '<player> controls the most creatures or is tied for the most' as a control-count condition", () => {
    assert.deepEqual(ability("This creature can't be blocked as long as defending player controls the most creatures or is tied for the most.", "Creature — Horror"),
      { kind: "static", effect: { static: "cant", what: { ref: "this", noun: "creature" }, action: "be-blocked",
        condition: { condition: "control-count", who: { player: "defending-player" }, of: { types: ["creature"] }, comparison: mostControlled(["creature"]) } } });
  });
  // The Black Gate; Galactus and The Master name the compared players.
  it("reads 'a player with the most life or tied for most life' and 'among your opponents'", () => {
    assert.deepEqual(ability("Choose a player with the most life or tied for most life. That player draws a card."),
      { kind: "spell", effects: [{ steps: [{ effect: "choose-object", who: { player: "you" }, what: { player: "filtered", base: { player: "a-player" },
        filter: { lifeTotal: { op: "eq", value: { amount: "life-total-extremum", extremum: "maximum", among: { player: "each-player" } } } } } }] },
        { steps: [{ effect: "draw", who: { player: "that-player" }, amount: n(1) }] }] });
    assert.deepEqual(ability("Choose an opponent with the most life among your opponents. That player draws a card."),
      { kind: "spell", effects: [{ steps: [{ effect: "choose-object", who: { player: "you" }, what: { player: "filtered", base: { player: "an-opponent" },
        filter: { lifeTotal: { op: "eq", value: { amount: "life-total-extremum", extremum: "maximum", among: { player: "each-opponent" } } } } } }] },
        { steps: [{ effect: "draw", who: { player: "that-player" }, amount: n(1) }] }] });
  });
  it("rejects superlatives whose compared players are not stated", () => {
    fails("Choose an opponent with the most life. That player draws a card.");
    // Without the tie clause, "the most" over every player is left unsupported.
    fails("Choose a player with the most life. That player draws a card.");
    fails("Each player with the most life draws a card.");
    fails("Each opponent who controls the most creatures draws a card.");
    fails("Each player who controls the most creatures you control draws a card.");
    fails("Each player with the most creatures draws a card.");
  });
});

describe("'that many' target counts (CR 601.2c, 603.12)", () => {
  // Cephalid Constable, Heroic Feast, Miasma Demon: the number of targets is
  // the contextual quantity, kept symbolic like every other "that many".
  it("uses the contextual quantity as the number of targets", () => {
    assert.deepEqual(ability("Whenever this creature deals combat damage to a player, return up to that many target permanents that player controls to their owner's hand.", "Creature — Cephalid"),
      { kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, combat: true, to: "any" },
        effects: [{ steps: [{ effect: "return-to-hand", what: { ref: "target", filter: { cls: "permanent", control: "that-player" }, count: { amount: "that-much" }, upTo: true }, to: "owner" }] }] });
    assert.deepEqual(ability("Whenever you gain life, choose up to that many target creatures you control. Put a +1/+1 counter on each of them.", "Enchantment"),
      { kind: "triggered", trigger: { trigger: "gains-life", who: { player: "you" } },
        effects: [{ steps: [{ effect: "declare-target", what: { ref: "target", filter: { types: ["creature"], control: "you" }, count: { amount: "that-much" }, upTo: true } }] },
          { steps: [{ effect: "put-counters", on: { ref: "them" }, counter: { type: "+1/+1" }, count: n(1) }] }] });
  });
  it("does not read 'that many' as a count of other objects", () => {
    fails("Destroy up to that many another target creatures.");
  });
});

describe("mana and life paid together or as alternatives (CR 118.1-118.3, 119.4)", () => {
  const mana = (...symbols: string[]) => ({ cost: "mana", symbols });
  const life = (value: number) => ({ cost: "pay-life", amount: n(value) });
  // Miara, Purgatory, Zoraline: both payments.
  it("reads 'pay {1} and 1 life' as both costs", () => {
    assert.deepEqual(ability("Whenever this creature attacks, you may pay {1} and 1 life. If you do, draw a card.", "Creature — Elf"),
      { kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
        effects: [{ steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [mana("1"), life(1)], optional: true }] },
          { steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }], condition: { condition: "you-do" }, conditionKind: "if" }] });
    // Mundungu
    assert.deepEqual(ability("{T}: Counter target spell unless its controller pays {1} and 1 life.", "Creature — Human Wizard"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "counter", what: { ref: "target", filter: { cls: "spell" }, count: n(1) } }],
        condition: { condition: "pays", who: { player: "controller", of: { ref: "it" } }, cost: [mana("1"), life(1)] }, conditionKind: "unless" }] });
    // Infernal Darkness
    assert.deepEqual(ability("Cumulative upkeep—Pay {B} and 1 life.", "Enchantment"),
      { kind: "keywords", keywords: [{ keyword: "cumulative upkeep", cost: [mana("B"), life(1)] }] });
  });
  // Erosion, Emberwilde Djinn: one of the payments, chosen by the payer.
  it("reads 'pays {1} or 1 life' as a cost choice", () => {
    assert.deepEqual(ability("At the beginning of your upkeep, destroy target land unless its controller pays {1} or 1 life.", "Enchantment"),
      { kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
        effects: [{ steps: [{ effect: "destroy", what: { ref: "target", filter: { types: ["land"] }, count: n(1) } }],
          condition: { condition: "pays", who: { player: "controller", of: { ref: "it" } }, cost: [{ cost: "choice", options: [[mana("1")], [life(1)]] }] }, conditionKind: "unless" }] });
  });
  it("leaves mixed and/or payment lists unsupported", () => {
    fails("At the beginning of your upkeep, you may pay {1} and 1 life or 3 life. If you do, draw a card.", "Enchantment");
    fails("At the beginning of your upkeep, you may pay {1} or 1 life and {2}. If you do, draw a card.", "Enchantment");
  });
});

describe("'where X is' after a payment or a mana combination (CR 107.3c)", () => {
  // Champions of Minas Tirith, Primordial Ooze; Burnt Offering, Xenagos.
  it("binds X for the instruction that pays {X}", () => {
    assert.deepEqual(ability("At the beginning of your upkeep, you may pay {X}, where X is the number of cards in your hand. If you do, draw X cards.", "Enchantment"),
      { kind: "triggered", trigger: { trigger: "phase", phase: "upkeep", whose: "your" },
        effects: [{ steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "mana", symbols: ["X"] }], optional: true }],
          bindings: [{ variable: "x", value: { amount: "count", of: { cls: "card", zone: { zone: "hand", owner: "your" } } } }] },
          { steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }], condition: { condition: "you-do" }, conditionKind: "if" }] });
  });
  it("binds X for a mana combination", () => {
    assert.deepEqual(ability("Add X mana in any combination of {R} and/or {G}, where X is the number of creatures you control.", "Instant"),
      { kind: "spell", effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "combination", amount: { amount: "x" }, options: ["R", "G"] } }],
        bindings: [{ variable: "x", value: { amount: "count", of: { types: ["creature"], control: "you" } } }] }] });
  });
  it("still rejects a comma list that does not continue", () => {
    fails("Add X mana in any combination of {R} and/or {G}, where.", "Instant");
    fails("At the beginning of your upkeep, you may pay {X}, where X is less than or equal to your life total. If you do, draw X cards.", "Enchantment");
  });
});

describe("bounded number choices and the chosen number (CR 107.1, 607.2d, 714.3b)", () => {
  // Expel the Interlopers, By Invitation Only, Shapeshifter, Talion.
  it("keeps the inclusive range of the choice and refers to the chosen number", () => {
    assert.deepEqual(ability("Choose a number between 0 and 10. Destroy all creatures with power greater than or equal to the chosen number."),
      { kind: "spell", effects: [{ steps: [{ effect: "choose-value", who: { player: "you" }, category: "number", minimum: 0, maximum: 10 }] },
        { steps: [{ effect: "destroy", what: { ref: "all", filter: { types: ["creature"], properties: [{ property: "power",
          comparison: { op: "ge", value: { amount: "chosen-number", which: "chosen" } } }] } } }] }] });
    assert.deepEqual(ability("~'s power is equal to the last chosen number and its toughness is equal to 7 minus that number.", "Creature — Shapeshifter"),
      { kind: "static", effect: { static: "group", effects: [
        { static: "cda-pt", what: { ref: "self" }, stat: "power", value: { amount: "chosen-number", which: "last-chosen" } },
        { static: "cda-pt", what: { ref: "self" }, stat: "toughness", value: { amount: "minus", a: n(7), b: { amount: "that-number" } } }] } });
  });
  it("reads 'a number greater than 0' as a lower bound", () => {
    assert.deepEqual(ability("Choose a number greater than 0. Target player mills that many cards."),
      { kind: "spell", effects: [{ steps: [{ effect: "choose-value", who: { player: "you" }, category: "number", minimum: 1 }] },
        { steps: [{ effect: "mill", who: { player: "target-player" }, amount: { amount: "that-much" } }] }] });
  });
  it("rejects malformed ranges", () => {
    fails("Choose a number between 10 and 0. Each player sacrifices that many creatures of their choice.");
    fails("Choose a number between 0 or 10. Each player sacrifices that many creatures of their choice.");
    fails("Choose a number less than 5. Each player sacrifices that many creatures of their choice.");
  });
});
