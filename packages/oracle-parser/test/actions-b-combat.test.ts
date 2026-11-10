import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// CR 510.1, 611.2a: "assigns no combat damage" is a continuous effect on the
// creature for its stated period.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function ability(text: string, typeLine = "Creature — Test"): Ability | null {
  const line = parseOracleText(text, "Test", { typeLine }).lines[0];
  return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
}

describe("assigns no combat damage", () => {
  it("is a combat rule with the printed period", () => {
    assert.deepEqual(ability("Whenever this creature attacks and isn't blocked, it assigns no combat damage this turn and defending player loses 2 life."), {
      kind: "triggered",
      trigger: { trigger: "attacks-and-isnt-blocked", what: { ref: "this", noun: "creature" } },
      effects: [{ steps: [
        { effect: "combat-rule", what: { ref: "it" }, rule: { rule: "assigns-no-combat-damage" }, duration: { duration: "this-turn" } },
        { effect: "lose-life", who: { player: "defending-player" }, amount: { amount: "fixed", value: 2 } },
      ] }],
    });
    const combat = ability("Whenever this creature attacks and isn't blocked, this creature assigns no combat damage this combat.");
    assert.deepEqual(combat && combat.kind === "triggered" && combat.effects[0].steps, [
      { effect: "combat-rule", what: { ref: "this", noun: "creature" }, rule: { rule: "assigns-no-combat-damage" }, duration: { duration: "this-combat" } },
    ]);
  });
  it("does not accept a partial restriction", () => {
    assert.equal(ability("Whenever this creature attacks and isn't blocked, it assigns no combat damage to players this turn."), null);
    assert.equal(ability("Whenever this creature attacks and isn't blocked, it assigns no combat damage."), null);
  });
});

describe("can't attack/block unless companions also do", () => {
  const rule = (text: string) => {
    const line = parseOracleText(text, "Test", { typeLine: "Creature — Test" }).lines[0];
    return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
  };
  it("counts other creatures declared in the same declaration", () => {
    assert.deepEqual(rule("This creature can't attack unless at least two other creatures attack."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: {
        rule: "declared-with", action: "attack", others: { types: ["creature"], other: true }, minimum: { amount: "fixed", value: 2 } } },
    });
    assert.deepEqual(rule("This creature can't block unless a black or green creature also blocks."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: {
        rule: "declared-with", action: "block", others: { allOf: [{ anyOf: [{ colors: ["black"] }, { colors: ["green"] }] }], types: ["creature"], other: true },
        minimum: { amount: "fixed", value: 1 } } },
    });
  });
  it("does not accept wording that could include the creature itself or mismatch the action", () => {
    assert.equal(rule("This creature can't attack unless a creature attacks."), null);
    assert.equal(rule("This creature can't attack unless a creature also blocks."), null);
  });
});

describe("attack requirements that depend on another attacker", () => {
  const rule = (text: string) => {
    const line = parseOracleText(text, "Test", { typeLine: "Creature — Test" }).lines[0];
    return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
  };
  it("applies only to declarations that include the named attacker", () => {
    assert.deepEqual(rule("If this creature attacks, all creatures you control attack if able."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "all", filter: { types: ["creature"], control: "you" } }, rule: {
        rule: "attack-requirement", frequency: "each-combat", whenAttacking: { ref: "this", noun: "creature" } } },
    });
    assert.deepEqual(rule("If a creature you control attacks, this creature also attacks if able."), {
      kind: "static", effect: { static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: {
        rule: "attack-requirement", frequency: "each-combat", whenAttacking: { ref: "each", filter: { types: ["creature"], control: "you" } } } },
    });
  });
  it("is not the unconditional requirement, and keeps its whole shape", () => {
    assert.equal(rule("If this creature attacks, all creatures you control attack."), null);
    assert.equal(rule("If this creature attacks, all creatures you control attack this turn if able."), null);
  });
});

describe("counts of attacked players", () => {
  const parse = (text: string, typeLine = "Creature — Test") => {
    const line = parseOracleText(text, "Test", { typeLine }).lines[0];
    return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
  };
  it("counts players being attacked, not creatures", () => {
    assert.deepEqual(parse("Attacking creatures get +X/+0 until end of turn, where X is the number of players being attacked.", "Instant"), {
      kind: "spell", effects: [{ steps: [{
        effect: "pump", what: { ref: "each", filter: { status: ["attacking"], types: ["creature"] } },
        power: { sign: 1, amount: { amount: "x" } }, toughness: { sign: 1, amount: { amount: "fixed", value: 0 } },
        duration: { duration: "end-of-turn" } }],
      bindings: [{ variable: "x", value: { amount: "attacked-player-count", players: "players", which: { status: "being-attacked" } } }] }],
    });
  });
  it("counts opponents attacked in an explicit period", () => {
    const ast = parse("When this creature enters, create a number of 2/2 white Knight creature tokens equal to the number of opponents you attacked this turn.");
    assert.deepEqual(ast.effects[0].steps[0].count,
      { amount: "attacked-player-count", players: "opponents", which: { by: { player: "you" }, during: "this-turn" } });
  });
  it("requires an explicit period for 'you attacked'", () => {
    assert.equal(parse("When this creature enters, create a Treasure token for each opponent you attacked."), null);
  });
});

describe("attacked during a player's last turn", () => {
  const parse = (text: string) => {
    const line = parseOracleText(text, "Test", { typeLine: "Creature — Test" }).lines[0];
    return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
  };
  it("names the player whose last turn it was", () => {
    assert.deepEqual(parse("This creature can't attack if it attacked during your last turn."), {
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "cant", action: "attack" }],
        condition: { condition: "combat-action-history", what: { ref: "it" }, action: "attacked", during: { lastTurnOf: { player: "you" } } },
        conditionScope: "each-affected-object" },
    });
  });
  it("does not accept a bare last turn or an unnamed owner", () => {
    assert.equal(parse("This creature can't attack if it attacked during the last turn."), null);
    assert.equal(parse("This creature can't attack if it attacked during last turn."), null);
  });
});

describe("for each blocker beyond the first", () => {
  const parse = (text: string) => {
    const line = parseOracleText(text, "Test", { typeLine: "Creature — Test" }).lines[0];
    return line && line.ok === true ? JSON.parse(JSON.stringify(line.ability)) : null;
  };
  it("does not count the first blocker", () => {
    const ast = parse("Whenever this creature becomes blocked, it gets -1/-1 until end of turn for each creature blocking it beyond the first.");
    assert.deepEqual(ast.effects[0].steps[0].forEach, { types: ["creature"], blocking: { ref: "it" } });
    assert.equal(ast.effects[0].steps[0].forEachBeyond, 1);
    const plain = parse("Whenever this creature becomes blocked, it gets -1/-1 until end of turn for each creature blocking it.");
    assert.equal(plain.effects[0].steps[0].forEachBeyond, undefined);
  });
  it("rejects other 'beyond' counts", () => {
    assert.equal(parse("Whenever this creature becomes blocked, it gets -1/-1 until end of turn for each creature blocking it beyond the second."), null);
  });
});
