import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. Rules text (2026-09-25):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 };

function abilities(text: string, typeLine = "Instant"): Ability[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
/** Sentences of a one-line instant. */
function spell(text: string, typeLine = "Instant"): Sentence[] {
  const [ability] = abilities(text, typeLine);
  assert.equal(ability.kind, "spell");
  return (ability as Extract<Ability, { kind: "spell" }>).effects;
}
function rejected(text: string, typeLine = "Instant"): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("serial color lists in token definitions (CR 111.4)", () => {
  it("keeps all three colors of a comma-separated list", () => {
    // Hazezon, Shaper of Sand.
    assert.deepEqual(spell("Create two 1/1 red, green, and white Sand Warrior creature tokens."), [{ steps: [{
      effect: "create-token", count: { amount: "fixed", value: 2 },
      token: { colors: ["red", "green", "white"], supertypes: [], types: ["creature"], subtypes: ["Sand", "Warrior"], keywords: [],
        power: one, toughness: one },
    }] }]);
  });
  it("rejects a comma list without its closing conjunction", () => {
    rejected("Create a 1/1 red, green Sand Warrior creature token.");
    rejected("Create a 1/1 red, Sand Warrior creature token.");
  });
});

describe("values and target qualifications fixed as the spell is cast or the ability activated (CR 601.2, 602.2)", () => {
  it("fixes a definition of X while casting", () => {
    // Jaws of Stone (card name replaced by ~ in normalization).
    assert.deepEqual(spell("~ deals X damage divided as you choose among any number of targets, where X is the number of Mountains you control as you cast this spell.", "Sorcery"), [{
      steps: [{ effect: "damage", source: { ref: "self" }, amount: { amount: "x" },
        to: { to: "divided", among: { ref: "any-target", count: { amount: "chosen", minimum: 0 } } } }],
      bindings: [{ variable: "x", value: { amount: "count", of: { subtypes: ["Mountain"], control: "you" } }, asOf: "casting" }],
    }]);
  });
  it("reads past-tense control at that moment as control", () => {
    // Lifestream's Blessing.
    assert.deepEqual(spell("Draw X cards, where X is the greatest power among creatures you controlled as you cast this spell.", "Instant"), [{
      steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }],
      bindings: [{ variable: "x", value: { amount: "attribute-extremum", extremum: "maximum", attribute: "power",
        among: { types: ["creature"], control: "you" } }, asOf: "casting" }],
    }]);
  });
  it("qualifies a target only at activation", () => {
    // Keeper of the Flame.
    const [ability] = abilities("{R}, {T}: Choose target opponent who has more life than you do as you activate this ability. This creature deals 2 damage to that player.", "Creature — Human Wizard");
    assert.deepEqual(ability, { kind: "activated", costs: [{ cost: "mana", symbols: ["R"] }, { cost: "tap-self" }], effects: [
      { steps: [{ effect: "declare-target", what: { player: "filtered", base: { player: "target-opponent" },
        filter: { lifeTotal: { op: "gt", value: { amount: "life-total", of: { player: "you" } } } } }, asOf: "activation" }] },
      { steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: { amount: "fixed", value: 2 },
        to: { to: "player", ref: { player: "that-player" } } }] },
    ] });
  });
  it("asks whether you controlled an object as you cast the spell", () => {
    // Faerie Fencing (second sentence).
    assert.deepEqual(spell("Target creature gets -3/-3 until end of turn if you controlled a Faerie as you cast this spell."), [{
      steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: one },
        power: { sign: -1, amount: { amount: "fixed", value: 3 } }, toughness: { sign: -1, amount: { amount: "fixed", value: 3 } },
        duration: { duration: "end-of-turn" } }],
      condition: { condition: "control", who: { player: "you" }, what: { ref: "filter", filter: { subtypes: ["Faerie"] }, count: one }, asOf: "casting" },
      conditionKind: "if", conditionPosition: "after",
    }]);
    rejected("Target creature gets -3/-3 until end of turn if you controlled a Faerie.");
  });
  it("rejects past-tense control without the moment, and the moment without a definition to own it", () => {
    rejected("Draw X cards, where X is the greatest power among creatures you controlled.");
    rejected("Draw a card for each creature you controlled as you cast this spell.");
    rejected("Choose target creature as you cast this spell. Destroy it.");
  });
});

describe("an empty library as an instruction condition", () => {
  it("reads 'your library has no cards in it' as a zero count of cards in it", () => {
    // Fblthp, Impossibly Lost (second sentence).
    assert.deepEqual(spell("If your library has no cards in it, you win the game.", "Sorcery"), [{
      steps: [{ effect: "win-game", who: { player: "you" } }],
      condition: { condition: "count", of: { cls: "card", zone: { zone: "library", owner: "your" } },
        comparison: { op: "eq", value: { amount: "fixed", value: 0 } } },
      conditionKind: "if",
    }]);
  });
  it("does not read other zones or a nonempty count this way", () => {
    rejected("If your graveyard has no cards in it, you win the game.", "Sorcery");
    rejected("If your library has two cards in it, you win the game.", "Sorcery");
  });
});

describe("nonattacking and nonblocking qualifiers (CR 506.4)", () => {
  it("negates the combat status", () => {
    // Alarum.
    assert.deepEqual(spell("Untap target nonattacking creature. It gets +1/+3 until end of turn.")[0], { steps: [{
      effect: "untap", what: { ref: "target", filter: { nonStatus: ["attacking"], types: ["creature"] }, count: one },
    }] });
    // Unlikely Alliance.
    const [ability] = abilities("{1}{W}: Target nonattacking, nonblocking creature gets +0/+2 until end of turn.", "Enchantment");
    assert.deepEqual(ability, { kind: "activated", costs: [{ cost: "mana", symbols: ["1", "W"] }], effects: [{ steps: [{
      effect: "pump", what: { ref: "target", filter: { nonStatus: ["attacking", "blocking"], types: ["creature"] }, count: one },
      power: { sign: 1, amount: { amount: "fixed", value: 0 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 2 } },
      duration: { duration: "end-of-turn" },
    }] }] });
  });
  it("does not invent other negated statuses", () => {
    rejected("Untap target nonuntapped creature.");
  });
});

describe("players losing all their counters (CR 122.1)", () => {
  it("keeps the counter kind, or any kind", () => {
    // Leeches.
    assert.deepEqual(spell("Target player loses all poison counters. ~ deals that much damage to that player.", "Sorcery"), [
      { steps: [{ effect: "lose-counters", who: { player: "target-player" }, counter: { type: "poison" }, count: "all" }] },
      { steps: [{ effect: "damage", source: { ref: "self" }, amount: { amount: "that-much" }, to: { to: "player", ref: { player: "that-player" } } }] },
    ]);
    // Final Act (fifth mode).
    assert.deepEqual(spell("Each opponent loses all counters.", "Sorcery"), [{ steps: [{
      effect: "lose-counters", who: { player: "each-opponent" }, counter: "any", count: "all",
    }] }]);
  });
  it("does not accept a gain, or a count other than all", () => {
    rejected("Each opponent gains all counters.", "Sorcery");
    rejected("Each opponent loses two poison counters.", "Sorcery");
  });
});

describe("counting coin flips won (CR 705.2)", () => {
  const won = { amount: "event-quantity", event: "coin-flip", result: "win", who: { player: "you" }, scope: "contextual" };
  it("scales a trailing instruction by the flips won", () => {
    // Crazed Firecat.
    const [ability] = abilities("When this creature enters, flip a coin until you lose a flip. Put a +1/+1 counter on this creature for each flip you won.", "Creature — Elemental Cat");
    assert.deepEqual(ability, { kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } }, effects: [
      { steps: [{ effect: "flip-coin", who: { player: "you" }, count: one, until: "lose" }] },
      { steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" },
        count: { amount: "product", factors: [one, won] } }] },
    ] });
  });
  it("scales an instruction after a leading unit", () => {
    // Goblin Traprunner.
    assert.deepEqual(spell("Flip three coins. For each flip you win, create a 1/1 red Goblin creature token.", "Sorcery")[1], { steps: [{
      effect: "create-token", count: { amount: "product", factors: [one, won] },
      token: { colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Goblin"], keywords: [], power: one, toughness: one },
    }] });
  });
  it("rejects a leading unit before an instruction it cannot scale, and lost flips", () => {
    rejected("Flip three coins. For each flip you win, destroy target creature.", "Sorcery");
    rejected("Flip three coins. For each flip you lose, create a 1/1 red Goblin creature token.", "Sorcery");
  });
});

describe("a leading unless-payment clause governs its whole sentence (CR 118.12)", () => {
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "your" };
  it("keeps every instruction of the sentence under the payment", () => {
    // Elder Spawn.
    const [ability] = abilities("At the beginning of your upkeep, unless you sacrifice an Island, sacrifice this creature and it deals 6 damage to you.", "Creature — Spawn");
    assert.deepEqual(ability, { kind: "triggered", trigger: upkeep, effects: [{
      steps: [
        { effect: "sacrifice", who: { player: "you" }, what: { ref: "this", noun: "creature" } },
        { effect: "damage", source: { ref: "it" }, amount: { amount: "fixed", value: 6 }, to: { to: "player", ref: { player: "you" } } },
      ],
      condition: { condition: "pays", who: { player: "you" }, cost: [{ cost: "sacrifice", what: { ref: "filter", filter: { subtypes: ["Island"] }, count: one } }] },
      conditionKind: "unless",
    }] });
  });
  it("records an opponent choosing what is sacrificed", () => {
    // Demonic Hordes.
    const [ability] = abilities("At the beginning of your upkeep, unless you pay {B}{B}{B}, tap this creature and sacrifice a land of an opponent's choice.", "Creature — Demon");
    assert.deepEqual(ability, { kind: "triggered", trigger: upkeep, effects: [{
      steps: [
        { effect: "tap", what: { ref: "this", noun: "creature" } },
        { effect: "sacrifice", who: { player: "you" }, what: { ref: "filter", filter: { types: ["land"] }, count: one }, chosenBy: { player: "an-opponent" } },
      ],
      condition: { condition: "pays", who: { player: "you" }, cost: [{ cost: "mana", symbols: ["B", "B", "B"] }] },
      conditionKind: "unless",
    }] });
  });
  it("rejects a second condition on the same sentence and a payment that runs into the instruction", () => {
    rejected("Unless you pay {1}, sacrifice a creature if you control an artifact.", "Sorcery");
    rejected("Unless you pay {1} draw a card.", "Sorcery");
  });
});
