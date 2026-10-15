import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Comprehensive Rules (https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
function parse(text: string, typeLine = "Creature — Test"): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function rejected(text: string, typeLine = "Creature — Test"): void {
  assert.equal(parseOracleText(text, "Test", { typeLine }).ok, false, text);
}

describe("fully unlocking a Room (CR 709.5i)", () => {
  it("parses 'whenever you fully unlock a Room'", () => {
    const [ability] = parse("Whenever you fully unlock a Room, draw a card.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.trigger, { trigger: "fully-unlocks", who: { player: "you" }, what: { subtypes: ["Room"] } });
  });
  it("rejects other objects and a partial unlock wording", () => {
    rejected("Whenever you fully unlock a creature, draw a card.");
    rejected("Whenever you fully unlock a Room you control, draw a card.");
    rejected("Whenever you fully unlock, draw a card.");
  });
});

describe("excess damage dealt this way (CR 120.4a, 120.10)", () => {
  it("tests the excess damage of the earlier damage event and binds 'that excess damage'", () => {
    const [ability] = parse("Target creature you control deals damage equal to its power to another target creature. If excess damage was dealt this way, discover X, where X is that excess damage.", "Sorcery");
    assert.ok(ability.kind === "spell");
    const step = ability.effects[1];
    assert.deepEqual(step.condition, { condition: "compare", value: { amount: "event-quantity", event: "damage", measure: "excess", scope: "this-way" },
      comparison: { op: "ge", value: { amount: "fixed", value: 1 } } });
    assert.deepEqual(step.bindings, [{ variable: "x", value: { amount: "event-quantity", event: "damage", measure: "excess", scope: "contextual" } }]);
  });
  it("keeps the stated recipient", () => {
    const [ability] = parse("~ deals 2 damage to target creature. If excess damage was dealt to that creature this way, investigate.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[1].condition, { condition: "compare", value: { amount: "event-quantity", event: "damage", measure: "excess",
      to: { ref: "that", filter: { types: ["creature"] } }, scope: "this-way" }, comparison: { op: "ge", value: { amount: "fixed", value: 1 } } });
  });
  it("rejects excess damage outside a this-way outcome", () => {
    rejected("~ deals 2 damage to target creature. If excess damage was dealt, investigate.", "Sorcery");
    rejected("~ deals 2 damage to target creature. If excess damage was dealt to target creature this way, investigate.", "Sorcery");
    rejected("~ deals 2 damage to target creature. Discover X, where X is that excess damage dealt last turn.", "Sorcery");
  });
});

describe("planeswalk and chaos ensues (CR 701.31, 311.7)", () => {
  it("parses an optional planeswalk", () => {
    const [ability] = parse("You may planeswalk.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects, [{ steps: [{ effect: "planeswalk", who: { player: "you" }, optional: true }] }]);
  });
  it("parses a vote between planeswalk and chaos with its results", () => {
    const [ability] = parse("Starting with you, each player votes for planeswalk or chaos. If planeswalk gets more votes, planeswalk. If chaos gets more votes or the vote is tied, chaos ensues.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects, [
      { steps: [{ effect: "vote", starting: { player: "you" }, choices: { choices: "named", names: ["planeswalk", "chaos"] } }] },
      { steps: [{ effect: "planeswalk", who: { player: "you" } }], condition: { condition: "vote-result", choice: "planeswalk", result: "more" }, conditionKind: "if" },
      { steps: [{ effect: "chaos-ensues" }], condition: { condition: "vote-result", choice: "chaos", result: "more-or-tied" }, conditionKind: "if" },
    ]);
  });
  it("rejects a chaos result that no vote offered", () => {
    rejected("If chaos gets more votes, chaos ensues.", "Sorcery");
    rejected("Chaos ensues for target creature.", "Sorcery");
  });
});

describe("the amassed Army (CR 701.47c)", () => {
  it("refers to the Army chosen by the preceding amass", () => {
    const [ability] = parse("Amass Zombies 2, then the Army you amassed deals damage equal to its power to each non-Army creature.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps[1], { effect: "damage", source: { ref: "amassed-army" }, amount: { amount: "attribute", of: { ref: "it" }, attribute: "power" },
      to: { to: "object", ref: { ref: "each", filter: { nonSubtypes: ["Army"], types: ["creature"] } } } });
  });
  it("reads the amassed Army's power and attaches to it", () => {
    const [ability] = parse("Amass Orcs 3, then target player mills X cards, where X is the amassed Army's power.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].bindings, [{ variable: "x", value: { amount: "attribute", of: { ref: "amassed-army" }, attribute: "power" } }]);
    const [equipment] = parse("When this Equipment enters, amass Goblins 1, then attach this Equipment to the amassed Army.", "Artifact — Equipment");
    assert.ok(equipment.kind === "triggered");
    assert.deepEqual(equipment.effects[0].steps[1], { effect: "attach", what: { ref: "this", noun: "equipment" }, to: { ref: "amassed-army" }, who: { player: "you" } });
  });
  it("rejects other 'amassed' wordings", () => {
    rejected("Amass Orcs 3, then the amassed creature deals damage equal to its power to each creature.", "Sorcery");
    rejected("Amass Orcs 3, then the Army you controlled deals damage equal to its power to each creature.", "Sorcery");
  });
});

describe("scry/surveil by another player or alongside another instruction (CR 701.22a, 701.25a)", () => {
  it("keeps the may-governing player as the scrying player", () => {
    const [ability] = parse("When this creature enters, each opponent may scry 1.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "scry", amount: { amount: "fixed", value: 1 }, who: { player: "each-opponent" }, optional: true, decidedBy: { player: "each-opponent" } }]);
  });
  it("separates 'and you scry' from the damage recipients", () => {
    const [ability] = parse("When this creature enters, it deals 2 damage to each opponent and you scry 2.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps, [
      { effect: "damage", source: { ref: "it" }, amount: { amount: "fixed", value: 2 }, to: { to: "player", ref: { player: "each-opponent" } } },
      { effect: "scry", amount: { amount: "fixed", value: 2 } },
    ]);
  });
  it("still rejects a may-scry without an explicit counter", () => {
    rejected("When this creature enters, each opponent may scry.");
  });
});

describe("cards looked at while scrying (CR 701.22a)", () => {
  const looked = { amount: "event-quantity", event: "scry-looked-at", scope: "this-way" };
  it("reads the number of cards the scry looked at", () => {
    const [ability] = parse("Whenever you scry, tap up to X target nonland permanents, where X is the number of cards looked at while scrying this way.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].bindings, [{ variable: "x", value: looked }]);
  });
  it("scales a per-card effect", () => {
    const [ability] = parse("Whenever you scry, ~ gets +1/+1 until end of turn for each card looked at while scrying this way.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "pump");
    assert.deepEqual(step.power, { sign: 1, amount: { amount: "product", factors: [{ amount: "fixed", value: 1 }, looked] } });
  });
  it("rejects other looked-at wordings", () => {
    rejected("Whenever you scry, draw a card for each card looked at while surveilling this way.");
    rejected("Whenever you scry, draw a card for each card looked at this way.");
  });
});

describe("contracted goaded state (CR 701.15b)", () => {
  it("parses \"It's goaded for the rest of the game\" and \"They're goaded ...\"", () => {
    const [ability] = parse("Gain control of target creature. It's goaded for the rest of the game.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[1].steps, [{ effect: "goad", what: { ref: "it" }, duration: { duration: "rest-of-game" } }]);
    const [them] = parse("Gain control of all creatures target player controls. They're goaded for the rest of the game.", "Sorcery");
    assert.ok(them.kind === "spell");
    assert.deepEqual(them.effects[1].steps, [{ effect: "goad", what: { ref: "them" }, duration: { duration: "rest-of-game" } }]);
  });
  it("rejects a goaded state with no duration", () => {
    rejected("Gain control of target creature. It's goaded.", "Sorcery");
  });
});

describe("a 'where X is' definition followed by a later instruction", () => {
  it("keeps the definition and appends the 'then' instruction to the same sentence", () => {
    const [ability] = parse("Scry X, where X is the greatest mana value among permanents you control, then draw three cards.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects, [{
      steps: [{ effect: "scry", amount: { amount: "x" } }, { effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 3 } }],
      bindings: [{ variable: "x", value: { amount: "attribute-extremum", extremum: "maximum", attribute: "mana-value", among: { cls: "permanent", control: "you" } } }],
    }]);
  });
  it("still rejects a definition followed by something other than 'then'", () => {
    rejected("Scry X, where X is the number of lands you control, draw three cards.", "Sorcery");
  });
});

describe("snow mana spent to cast this spell (CR 107.4h, 609.4b)", () => {
  it("counts the snow mana among the mana actually spent", () => {
    const [ability] = parse("Scry X, where X is the amount of {S} spent to cast this spell, then draw three cards.", "Snow Instant");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].bindings, [{ variable: "x", value: { amount: "mana-spent", what: { ref: "this", noun: "spell" }, quantity: "snow" } }]);
  });
  it("keeps total mana spent distinct and rejects other symbols", () => {
    const [ability] = parse("Scry X, where X is the amount of mana spent to cast this spell.", "Instant");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].bindings?.[0].value, { amount: "mana-spent", what: { ref: "this", noun: "spell" }, quantity: "total" });
    rejected("Scry X, where X is the amount of {R} spent to cast this spell.", "Instant");
  });
});

describe("Contraptions assembling and the difference between two dice", () => {
  const trigger = (body: string) => parse(`Whenever you crank this Contraption, ${body}`, "Artifact — Contraption");
  it("parses 'it assembles a Contraption'", () => {
    const [ability] = trigger("it assembles a Contraption.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "assemble-contraption", by: { ref: "it" }, count: { amount: "fixed", value: 1 } }]);
  });
  it("assembles a number of Contraptions equal to the difference between two dice results", () => {
    const [ability] = trigger("roll two six-sided dice. This Contraption assembles a number of Contraptions equal to the difference between those results.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[1].steps, [{ effect: "assemble-contraption", by: { ref: "this", noun: "contraption" },
      count: { amount: "dice-result", roll: { roll: "contextual" }, measure: "difference" } }]);
  });
  it("rejects assembling anything but Contraptions and other differences", () => {
    rejected("Whenever you crank this Contraption, it assembles a creature.", "Artifact — Contraption");
    rejected("Whenever you crank this Contraption, roll two six-sided dice. You gain life equal to the difference between the results.", "Artifact — Contraption");
  });
});

describe("reflexive 'when it connives this way' (CR 701.50a, 603.12)", () => {
  it("creates a reflexive trigger on the connive instruction", () => {
    const [ability] = parse("When this creature enters, it connives. When it connives this way, return up to one target nonland permanent to its owner's hand.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[1].steps[0];
    assert.ok(step.effect === "create-reflexive-trigger");
    assert.deepEqual(step.event, { event: "connived", what: { ref: "it" } });
  });
  it("rejects other verbs and plural subjects", () => {
    rejected("When this creature enters, it connives. When it explores this way, draw a card.");
    rejected("When this creature enters, it connives. When they connive this way, draw a card.");
  });
});
