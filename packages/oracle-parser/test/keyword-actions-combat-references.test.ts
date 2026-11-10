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

describe("the blocking / attacking creature as a definite description (CR 506.4, 509.1g)", () => {
  it("refers to the identified blocking creature", () => {
    const [ability] = parse("Whenever this creature becomes blocked by a creature, the blocking creature gets -1/-1 until end of turn.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "pump", what: { ref: "that", filter: { status: ["blocking"], types: ["creature"] } },
      power: { sign: -1, amount: one }, toughness: { sign: -1, amount: one }, duration: { duration: "end-of-turn" },
    });
  });
  it("refers to the identified attacking creature", () => {
    const [ability] = parse("Whenever a creature blocks a creature with lesser power, destroy the attacking creature.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], {
      effect: "destroy", what: { ref: "that", filter: { status: ["attacking"], types: ["creature"] } },
    });
  });
  it("keeps other statuses out of the definite description", () => {
    rejected("Whenever a creature blocks, destroy the tapped creature.");
  });
});

describe("regeneration riders and reflexive regeneration (CR 701.19, 603.12)", () => {
  it("creates a reflexive trigger for 'When it regenerates this way'", () => {
    const [ability] = parse("{1}: Regenerate this creature. When it regenerates this way, put a -1/-1 counter on it.");
    assert.ok(ability.kind === "activated");
    assert.deepEqual(ability.effects, [
      { steps: [{ effect: "regenerate", what: { ref: "this", noun: "creature" } }] },
      { steps: [{ effect: "create-reflexive-trigger", event: { event: "regenerated", what: { ref: "it" } },
        effects: [{ steps: [{ effect: "put-counters", on: { ref: "it" }, counter: { type: "-1/-1" }, count: one }] }] }] },
    ]);
  });
  it("keeps the conditional follow-up of another player distinct", () => {
    const [ability] = parse("{1}: Choose target opponent. Regenerate this creature. When it regenerates this way, that player may draw a card.");
    assert.ok(ability.kind === "activated");
    const trigger = ability.effects[2].steps[0];
    assert.ok(trigger.effect === "create-reflexive-trigger" && trigger.event.event === "regenerated");
  });
  it("rejects a regeneration reflexive without 'this way'", () => {
    rejected("{1}: Regenerate this creature. When it regenerates, put a -1/-1 counter on it.");
  });
  it("applies 'Those creatures can't be regenerated' to a multi-object destruction of that type", () => {
    const [ability] = parse("Destroy all creatures except for ~. Those creatures can't be regenerated.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [{
      effect: "destroy", what: { ref: "all", filter: { types: ["creature"], otherThan: { ref: "self" } } }, regenerationForbidden: true,
    }]);
  });
  it("rejects the plural rider for another type or a single target", () => {
    rejected("Destroy all creatures. Those lands can't be regenerated.", "Sorcery");
    rejected("Destroy target creature. Those creatures can't be regenerated.", "Sorcery");
  });
});

describe("players finish voting (CR 701.38a)", () => {
  const you = { player: "you" };
  const opponents = (same: boolean) => ({ player: "filtered", base: { player: "each-opponent" }, filter: { votedFor: { as: you, same } } });
  it("reads each opponent's vote against yours", () => {
    const [ability] = parse("Whenever players finish voting, each opponent who voted for a choice you didn't vote for loses 2 life.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.trigger, { trigger: "players-finish-voting" });
    assert.deepEqual(ability.effects[0].steps[0], { effect: "lose-life", who: opponents(false), amount: { amount: "fixed", value: 2 } });
  });
  it("counts the opponents who voted with you", () => {
    const [ability] = parse("Whenever players finish voting, you scry X, where X is the number of opponents who voted for a choice you voted for.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].bindings, [{ variable: "x", value: { amount: "player-count", of: opponents(true) } }]);
  });
  it("keeps same and different choices distinct", () => {
    const same = parse("Whenever players finish voting, each opponent who voted for a choice you voted for loses 2 life.");
    const different = parse("Whenever players finish voting, each opponent who voted for a choice you didn't vote for loses 2 life.");
    assert.notDeepEqual(same, different);
  });
  it("rejects votes compared with another player or a different event", () => {
    rejected("Whenever players finish voting, each opponent who voted for a choice that player voted for loses 2 life.");
    rejected("Whenever players finish voting, each opponent who voted for a choice you voted against loses 2 life.");
  });
});

describe("objects that had counters put on them this way (CR 122.1, 608.2c)", () => {
  it("names the counter kind", () => {
    const [ability] = parse("Put a double strike counter on target creature, then goad each creature that had a double strike counter put on it this way.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps[1], {
      effect: "goad",
      what: { ref: "each", filter: { types: ["creature"], relation: { relation: "counters-put", counter: { type: "double strike" }, scope: "this-way" } } },
    });
  });
  it("takes a bare 'counters' or 'a counter' as any kind", () => {
    const [ability] = parse("Proliferate, then choose any number of permanents you control that had a counter put on them this way. Those permanents phase out.", "Sorcery");
    assert.ok(ability.kind === "spell");
    const choose = ability.effects[0].steps[1];
    assert.ok(choose.effect === "choose-object" && "filter" in choose.what && "cls" in choose.what.filter);
    assert.deepEqual(choose.what.filter.relation, { relation: "counters-put", counter: "any", scope: "this-way" });
    const [bare] = parse("Put two +1/+1 counters on target creature. Goad each creature that had counters put on it this way.", "Sorcery");
    assert.ok(bare.kind === "spell");
    const goad = bare.effects[1].steps[0];
    assert.ok(goad.effect === "goad" && goad.what.ref === "each" && "types" in goad.what.filter);
    assert.deepEqual(goad.what.filter.relation, { relation: "counters-put", counter: "any", scope: "this-way" });
  });
  it("rejects a different counting window or action", () => {
    rejected("Put two +1/+1 counters on target creature. Goad each creature that had counters put on it this turn.", "Sorcery");
    rejected("Put two +1/+1 counters on target creature. Goad each creature that had counters removed from it this way.", "Sorcery");
  });
});

describe("a coin flip made by a delayed ability keeps its outcomes (CR 705.2, 603.7)", () => {
  it("folds the outcome into the delayed ability's flip", () => {
    const [ability] = parse("When this creature enters, flip a coin at the beginning of the next end step. If you lose the flip, sacrifice ~.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects, [{ steps: [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" }, occurrence: "next", nextOnly: true,
      effects: [{ steps: [{ effect: "flip-coin", who: { player: "you" }, count: one,
        outcomes: [{ result: "lose", effects: [{ steps: [{ effect: "sacrifice", who: { player: "you" }, what: { ref: "self" } }] }] }] }] }],
      targets: "enclosing-ability",
    }] }]);
  });
  it("works at end of combat", () => {
    const [ability] = parse("Whenever this creature attacks or blocks, flip a coin at end of combat. If you lose the flip, an opponent gains control of this creature.");
    assert.ok(ability.kind === "triggered");
    const delayed = ability.effects[0].steps[0];
    assert.ok(delayed.effect === "create-delayed-trigger" && delayed.trigger.trigger === "phase" && delayed.trigger.phase === "end-of-combat");
    const flip = delayed.effects[0].steps[0];
    assert.ok(flip.effect === "flip-coin" && flip.outcomes?.[0].result === "lose");
  });
  it("still rejects an unrelated condition after a delayed flip", () => {
    rejected("When this creature enters, flip a coin at the beginning of the next end step. If you control a creature, draw a card.");
  });
});

describe("keyword actions performed by other players", () => {
  it("lets the player whose 'may' governs it investigate or discover", () => {
    const [investigate] = parse("When this creature enters, each opponent may investigate.");
    assert.ok(investigate.kind === "triggered");
    assert.deepEqual(investigate.effects[0].steps[0], {
      effect: "keyword-action", action: "investigate", who: { player: "each-opponent" }, optional: true, decidedBy: { player: "each-opponent" },
    });
    const [discover] = parse("When this creature enters, target opponent may discover 3.");
    assert.ok(discover.kind === "triggered");
    assert.deepEqual(discover.effects[0].steps[0], {
      effect: "discover", who: { player: "target-opponent" }, amount: { amount: "fixed", value: 3 }, optional: true, decidedBy: { player: "target-opponent" },
    });
  });
  it("keeps the controller as the actor of 'you may investigate'", () => {
    const [ability] = parse("When this creature enters, you may investigate.");
    assert.ok(ability.kind === "triggered");
    assert.deepEqual(ability.effects[0].steps[0], { effect: "keyword-action", action: "investigate", who: { player: "you" }, optional: true });
  });
  it("continues the previous actor with a subjectless conjugated verb", () => {
    const [ability] = parse("Target opponent scries 2, then investigates.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[0].steps, [
      { effect: "scry", amount: { amount: "fixed", value: 2 }, who: { player: "target-opponent" } },
      { effect: "keyword-action", action: "investigate", who: { player: "target-opponent" } },
    ]);
    rejected("You scry 2, then investigates.", "Sorcery");
  });
  it("lets another player discover and takes 'an additional time' as one more investigate", () => {
    const [ability] = parse("The owner of target creature shuffles it into their library. Then that player discovers X, where X is its mana value.", "Sorcery");
    assert.ok(ability.kind === "spell");
    assert.deepEqual(ability.effects[1].steps[0], { effect: "discover", who: { player: "that-player" }, amount: { amount: "x" } });
    const [again] = parse("Whenever you investigate for the first time each turn, investigate an additional time.");
    assert.ok(again.kind === "triggered");
    assert.deepEqual(again.effects[0].steps[0], { effect: "keyword-action", action: "investigate", who: { player: "you" } });
  });
});

describe("harness and infinity abilities (CR 701.64, 702.186b)", () => {
  it("harnesses the permanent itself", () => {
    const [ability] = parse("{5}{W}, {T}: Harness ~.", "Legendary Artifact — Infinity Stone");
    assert.ok(ability.kind === "activated");
    assert.deepEqual(ability.effects, [{ steps: [{ effect: "become-status", what: { ref: "self" }, status: "harnessed" }] }]);
  });
  it("rejects harnessing another object", () => {
    rejected("{5}{W}, {T}: Harness target creature.", "Legendary Artifact — Infinity Stone");
  });
  it("wraps the ability held while harnessed", () => {
    const [ability] = parse("∞ — At the beginning of your upkeep, draw a card.", "Legendary Artifact — Infinity Stone");
    assert.ok(ability.kind === "infinity");
    assert.equal(ability.ability.kind, "triggered");
  });
  it("rejects an infinity label with no ability", () => {
    rejected("∞ — ", "Legendary Artifact — Infinity Stone");
    rejected("∞ At the beginning of your upkeep, draw a card.", "Legendary Artifact — Infinity Stone");
  });
});
