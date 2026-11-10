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
const one = { amount: "fixed", value: 1 };

describe("tokens put onto the battlefield attacking a named player or blocking a named creature (CR 508.4, 509.4)", () => {
  it("records the named defending player", () => {
    const [ability] = parse("Whenever you attack a player, create a 1/1 white Glimmer enchantment creature token that's tapped and attacking that player.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "create-token", count: one,
      token: { colors: ["white"], supertypes: [], types: ["enchantment", "creature"], subtypes: ["Glimmer"], keywords: [], power: one, toughness: one,
        tapped: true, attacking: true, attackingDefender: { player: "that-player" } },
    });
  });
  it("keeps the unnamed form without a defender", () => {
    const [ability] = parse("Whenever ~ attacks, create a 1/1 white Cat Soldier creature token that's attacking.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "create-token" && "attacking" in step.token);
    assert.equal("attackingDefender" in step.token, false);
  });
  it("records the attacking creature a created blocker blocks", () => {
    const [ability] = parse("Create a 1/1 green Saproling creature token that's blocking target creature attacking you.", "Instant");
    assert.ok(ability.kind === "spell");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "create-token" && "blocking" in step.token);
    assert.deepEqual(step.token.blocking, { ref: "target", filter: { types: ["creature"], attacking: [{ defender: "player", who: { player: "you" } }] }, count: one });
  });
  it("records the creature a card put onto the battlefield blocks", () => {
    const [ability] = parse("Whenever this creature blocks a creature, you may put a creature card from your hand onto the battlefield blocking that creature.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "move-zone");
    assert.deepEqual(step.entry, { blocking: { ref: "that", filter: { types: ["creature"] } } });
  });
  it("rejects a blocker without a named creature, or one that also attacks", () => {
    rejected("Whenever ~ blocks a creature, create a 1/1 white Cat Soldier creature token that's blocking.");
    rejected("Create a tapped and blocking 1/1 green Saproling creature token.", "Instant");
    rejected("Whenever ~ attacks, create a 1/1 white Cat creature token that's attacking and blocking that creature.");
  });
});

describe("'The token(s) are goaded' after a creating instruction (CR 701.15a)", () => {
  it("names the created tokens like 'those tokens', with the stated duration", () => {
    const [ability] = parse("When this creature enters, if it's not a token, each opponent creates a token that's a copy of it. The tokens are goaded for the rest of the game.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[1].steps[0], {
      effect: "goad", what: { ref: "that", filter: { cls: "token" }, plural: true }, duration: { duration: "rest-of-game" },
    });
  });
  it("singular form", () => {
    const [ability] = parse("Whenever this creature deals combat damage to a player, that player creates a 1/1 red Pirate creature token. The token is goaded for the rest of the game.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[1].steps[0], {
      effect: "goad", what: { ref: "that", filter: { cls: "token" } }, duration: { duration: "rest-of-game" },
    });
  });
  it("still requires a stated duration and a goaded verb", () => {
    rejected("When this creature enters, each opponent creates a token that's a copy of it. The tokens are goaded.");
    rejected("When this creature enters, each opponent creates a token that's a copy of it. The tokens are tapped for the rest of the game.");
  });
});

describe("unlocking a door of a Room (CR 709.5f, 709.5c)", () => {
  const room = { subtypes: ["Room"], control: "you" };
  it("unlock a locked door of up to one target Room", () => {
    const [ability] = parse("Whenever this creature deals combat damage to a player, unlock a locked door of up to one target Room you control.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "unlock-door", what: { ref: "target", filter: room, count: one, upTo: true } }]);
  });
  it("is one alternative of a choice", () => {
    const [ability] = parse("When this creature enters, return an enchantment card from your graveyard to your hand or unlock a locked door of a Room you control.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "choice");
    assert.deepEqual(step.options[1], { effect: "unlock-door", what: { ref: "filter", filter: room, count: one } });
  });
  it("counts unlocked doors among Rooms", () => {
    const [ability] = parse("When you unlock this door, create an X/X blue Spirit creature token with flying, where X is the number of unlocked doors among Rooms you control.", "Enchantment — Room");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].bindings, [{ variable: "x", value: { amount: "unlocked-door-count", among: room } }]);
  });
  it("rejects non-Room targets and unlocking without a locked door", () => {
    rejected("When this creature enters, unlock a locked door of target creature you control.");
    rejected("When this creature enters, unlock a door of a Room you control.");
    rejected("When this creature enters, create an X/X blue Spirit creature token, where X is the number of unlocked doors among creatures you control.");
  });
});

describe("half amounts with their own stated rounding (CR 107.1a)", () => {
  const xHalf = (round: "up" | "down") => ({ amount: "half", of: { amount: "x" }, round });
  it("each damage recipient carries the rounding printed after its amount", () => {
    const [ability] = parse("~ deals half X damage, rounded down, to any target, and half X damage, rounded up, to you.", "Instant");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "simultaneous", steps: [
      { effect: "damage", source: { ref: "self" }, amount: xHalf("down"), to: { to: "any-target" } },
      { effect: "damage", source: { ref: "self" }, amount: xHalf("up"), to: { to: "player", ref: { player: "you" } } },
    ] }]);
  });
  it("half X counters that a player gets", () => {
    const [ability] = parse("You get half X rad counters, rounded up.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "get-counters", who: { player: "you" }, counter: { type: "rad" }, count: xHalf("up") }]);
  });
  it("rejects a half whose rounding is missing", () => {
    rejected("~ deals half X damage to any target and half X damage to you.", "Instant");
    rejected("~ deals half X damage, rounded down, to any target, and half X damage to you.", "Instant");
    rejected("You get half X rad counters.", "Sorcery");
  });
});

describe("exchange of control with shared permanent/card types (CR 110.4, 701.12)", () => {
  it("two targets that share a permanent type are a group constraint", () => {
    const [ability] = parse("Exchange control of two target permanents that share a permanent type.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [{ effect: "exchange-control", what: [{
      ref: "target", filter: { cls: "permanent" }, groupConstraints: [{ constraint: "shared-value", characteristic: "permanent-type" }], count: { amount: "fixed", value: 2 },
    }] }]);
  });
  it("a second target sharing one of the first target's listed types with it", () => {
    const [ability] = parse("Exchange control of target artifact or creature and another target permanent that shares one of those types with it.", "Sorcery");
    assert.ok(ability.kind === "spell");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "exchange-control");
    assert.deepEqual(step.what[1], { ref: "target", filter: { cls: "permanent", other: true, characteristicRelations: [
      { relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: { ref: "it" } }, among: "those-types" },
    ] }, count: one });
  });
  it("rejects the restricted-types wording without a compared object", () => {
    rejected("Exchange control of target artifact or creature and another target permanent that shares one of those types.", "Sorcery");
    rejected("Exchange control of target artifact or creature and another target permanent that doesn't share one of those types with it.", "Sorcery");
  });
});

describe("double-faced objects as a status of the filtered object (CR 712.1)", () => {
  it("a double-faced permanent being transformed", () => {
    const [ability] = parse("When this creature enters, transform up to one other target double-faced artifact you control.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "transform", who: { player: "you" },
      what: { ref: "target", filter: { status: ["double-faced"], types: ["artifact"], control: "you", other: true }, count: one, upTo: true },
    });
  });
  it("a card that is a land or a double-faced card", () => {
    const [ability] = parse("When this Siege enters, reveal the top card of your library. If it's a land or double-faced card, draw a card.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[1].condition, { condition: "is-filter", what: { ref: "it" }, filter: { anyOf: [{ types: ["land"] }, { status: ["double-faced"], cls: "card" }] } });
  });
});

describe("rules limited to a player's next turn or combat phase (CR 611.2, 500.1, 506.1)", () => {
  const itsController = { player: "controller", of: { ref: "it" } };
  it("can't attack during its controller's next turn", () => {
    const [ability] = parse("Whenever this creature blocks a creature, that creature can't attack during its controller's next turn.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "cant", what: { ref: "that", filter: { types: ["creature"] } }, action: "attack",
      duration: { duration: "during-next-turn-of", who: itsController },
    });
  });
  it("attacks during its controller's next combat phase if able", () => {
    const [ability] = parse("Whenever a land you control enters, target creature an opponent controls attacks during its controller's next combat phase if able.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "combat-rule",
      what: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: one },
      rule: { rule: "attack-requirement", frequency: "each-combat" },
      duration: { duration: "during-next-combat-of", who: itsController },
    });
  });
  it("names the creature it must attack", () => {
    const [ability] = parse("Whenever this creature attacks, up to one target creature an opponent controls attacks ~ during its controller's next turn if able.");
    assert.ok(ability.kind === "triggered");
    const step = ability.effects[0].steps[0];
    assert.ok(step.effect === "combat-rule");
    assert.deepEqual(step.rule, { rule: "attack-requirement", frequency: "each-combat", defender: { defender: "object", what: { ref: "self" } } });
    assert.deepEqual(step.duration, { duration: "during-next-turn-of", who: itsController });
  });
  it("rejects an unfinished period and a requirement without 'if able'", () => {
    rejected("Whenever this creature blocks a creature, that creature can't attack during its controller's turn.");
    rejected("Whenever a land you control enters, target creature an opponent controls attacks during its controller's next combat phase.");
  });
});
