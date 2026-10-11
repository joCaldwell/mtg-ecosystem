import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { CombatRule, ObjectFilter, ObjectRef, StaticEffect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseCombatEffect, parseCombatStatic } from "../src/parser/combat.ts";
import { parseDuration } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/index.ts";

// Exact card facts from cached Scryfall oracle corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 508.1c–d, 509.1a–c: legal restrictions, maximum satisfiable requirements,
// and additional blocking capacity must remain distinct.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const self = { ref: "self" } as const;
const thisCreature = { ref: "this", noun: "creature" } as const;
const each = (filter: ObjectFilter): ObjectRef => ({ ref: "each", filter });
function stat(text: string): StaticEffect {
  const c = new Cursor(lex(text));
  const result = parseCombatStatic(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return result;
}
function expected(rule: CombatRule, what: ObjectRef = thisCreature): StaticEffect {
  return { static: "combat-rule", what, rule };
}

describe("evasion restrictions preserve blocker eligibility and quantity", () => {
  it("distinguishes forbidden blockers from the only eligible blockers", () => {
    // Dauthi Horror 07c20708-c305-46e7-a98e-c99f54aeafec;
    // Zulaport Enforcer 0ab5fde8-e7db-4265-81c6-2a6488ba56c4.
    assert.deepEqual(stat("This creature can't be blocked by white creatures."), expected({
      rule: "blocker-filter", mode: "exclude", blockers: each({ colors: ["white"], types: ["creature"] }),
    }));
    assert.deepEqual(stat("This creature can't be blocked except by black creatures."), expected({
      rule: "blocker-filter", mode: "only", blockers: each({ colors: ["black"], types: ["creature"] }),
    }));
    assert.deepEqual(stat("This creature can't be blocked by Walls."), expected({ rule: "blocker-filter", mode: "exclude", blockers: each({ subtypes: ["Wall"] }) }));
  });

  it("keeps complete alternative categories and property restrictions", () => {
    // Doorman 01ea2130-e212-4532-b68b-2f940d53ef6e (ability body).
    assert.deepEqual(stat("This creature can't be blocked by creatures with power 2 or less and/or Walls."), expected({
      rule: "blocker-filter", mode: "exclude", blockers: each({ anyOf: [
        { types: ["creature"], properties: [{ property: "power", comparison: { op: "le", value: fixed(2) } }] },
        { subtypes: ["Wall"] },
      ] }),
    }));
    assert.deepEqual(stat("This creature can't be blocked except by artifact creatures or black creatures."), expected({
      rule: "blocker-filter", mode: "only", blockers: each({ anyOf: [
        { types: ["artifact", "creature"] }, { types: ["creature"], colors: ["black"] },
      ] }),
    }));
  });

  it("constrains nonempty blocker assignments without requiring a block", () => {
    // The Foretold Soldier 02a45719-ee4a-49c4-96ec-a140cd6e1547;
    // Troll of Khazad-dum 222fd9fc-7af4-4269-bbb9-cd7b8ac029bd.
    assert.deepEqual(stat("This creature can't be blocked by more than one creature."), expected({
      rule: "blocker-count", mode: "exclude", comparison: { op: "gt", value: fixed(1) },
    }));
    assert.deepEqual(stat("This creature can't be blocked except by three or more creatures."), expected({
      rule: "blocker-count", mode: "only", comparison: { op: "ge", value: fixed(3) },
    }));
    assert.deepEqual(stat("This creature can't be blocked by exactly two creatures."), expected({
      rule: "blocker-count", mode: "exclude", comparison: { op: "eq", value: fixed(2) },
    }));
  });

  it("does not reinterpret an indefinite eligible blocker as an exact blocker count", () => {
    assert.deepEqual(stat("This creature can't be blocked except by a creature with flying."), expected({
      rule: "blocker-filter", mode: "only", blockers: each({ types: ["creature"], withAbility: ["flying"] }),
    }));
    assert.deepEqual(stat("This creature can't be blocked except by an artifact creature."), expected({
      rule: "blocker-filter", mode: "only", blockers: each({ types: ["artifact", "creature"] }),
    }));
    assert.deepEqual(stat("A creature with flying can't block a creature with reach."), expected({
      rule: "blocking-filter", mode: "exclude", attackers: each({ types: ["creature"], withAbility: ["reach"] }),
    }, each({ types: ["creature"], withAbility: ["flying"] })));
    assert.deepEqual(stat("A creature with flying can block an additional creature each combat."), expected({
      rule: "block-capacity", capacity: { additional: fixed(1) },
    }, each({ types: ["creature"], withAbility: ["flying"] })));
  });
});

describe("blocking restrictions and capacities", () => {
  it("keeps the attacker filter distinct from the creatures subject to the restriction", () => {
    // Sidar Kondo of Jamuraa 331b6834-6bb4-40fa-b80b-22d67b34734f.
    assert.deepEqual(stat("Creatures your opponents control without flying or reach can't block creatures with power 2 or less."), expected({
      rule: "blocking-filter", mode: "exclude", attackers: each({ types: ["creature"], properties: [{ property: "power", comparison: { op: "le", value: fixed(2) } }] }),
    }, each({ types: ["creature"], control: "opponent", withoutAbility: ["flying", "reach"] })));
    // Welkin Tern 010c660a-a67b-48f0-a63a-15470eb89f69.
    assert.deepEqual(stat("This creature can block only creatures with flying."), expected({
      rule: "blocking-filter", mode: "only", attackers: each({ types: ["creature"], withAbility: ["flying"] }),
    }));
  });

  it("represents both directions of a shared block-or-be-blocked restriction", () => {
    // Sneaky Homunculus 0b76bb3b-1ada-4c78-bd13-3464371d4fa0.
    const other = each({ types: ["creature"], properties: [{ property: "power", comparison: { op: "ge", value: fixed(2) } }] });
    assert.deepEqual(stat("This creature can't block or be blocked by creatures with power 2 or greater."), expected({ rule: "all-of", rules: [
      { rule: "blocking-filter", mode: "exclude", attackers: other },
      { rule: "blocker-filter", mode: "exclude", blockers: other },
    ] }));
  });

  it("distinguishes additional capacity from an absolute or unlimited maximum", () => {
    // Selesnya Sagittars 027484b8-0df5-48b6-b69b-bb4e39c5ba81;
    // Watcher in the Web 232a4128-133e-446d-9f1c-dd3875094473;
    // Thoughtweft Trio 15fca8d6-8a86-4c24-9178-f0b6496518ab.
    assert.deepEqual(stat("This creature can block an additional creature each combat."), expected({ rule: "block-capacity", capacity: { additional: fixed(1) } }));
    assert.deepEqual(stat("This creature can block an additional seven creatures each combat."), expected({ rule: "block-capacity", capacity: { additional: fixed(7) } }));
    assert.deepEqual(stat("This creature can block only one creature each combat."), expected({ rule: "block-capacity", capacity: { maximum: fixed(1) } }));
    assert.deepEqual(stat("This creature can block any number of creatures."), expected({ rule: "block-capacity", capacity: { maximum: "unlimited" } }));
  });
});

describe("attack requirements, restrictions, and dynamic conditions", () => {
  it("preserves defender choice and requirements separately from forbidden defenders", () => {
    assert.deepEqual(stat("Each creature attacks a player other than you each combat if able."), expected({
      rule: "attack-requirement", frequency: "each-combat", defender: { defender: "player-other-than", who: { player: "you" } },
    }, each({ types: ["creature"] })));
    // Xantcha, Sleeper Agent 0f0f3712-8d13-41a5-b332-2ab34e48d79d.
    assert.deepEqual(stat("~ attacks each combat if able and can't attack its owner or planeswalkers its owner controls."), expected({ rule: "all-of", rules: [
      { rule: "attack-requirement", frequency: "each-combat" },
      { rule: "attack-restriction", defenders: [
        { defender: "player", who: { player: "owner", of: { ref: "it" } } },
        { defender: "object", what: each({ types: ["planeswalker"], controlledBy: { player: "owner", of: { ref: "it" } } }) },
      ] },
    ] }, self));
  });

  it("keeps a condition on the rule that it actually limits", () => {
    // Reckless Cohort 015b1006-decb-4d94-940e-bca550e0ad5b.
    assert.deepEqual(stat("This creature attacks each combat if able unless you control another Ally."), expected({
      rule: "attack-requirement", frequency: "each-combat", conditionKind: "unless", condition: {
        condition: "control", who: { player: "you" }, what: { ref: "filter", count: fixed(1), filter: { subtypes: ["Ally"], other: true } },
      },
    }));
    assert.deepEqual(stat("This creature can't be blocked by Walls as long as it's your turn."), expected({
      rule: "blocker-filter", mode: "exclude", blockers: each({ subtypes: ["Wall"] }), condition: { condition: "your-turn" }, conditionKind: "if",
    }));
    assert.deepEqual(stat("As long as it's your turn, this creature can't be blocked by Walls."), {
      static: "conditional", condition: { condition: "your-turn" }, effect: expected({ rule: "blocker-filter", mode: "exclude", blockers: each({ subtypes: ["Wall"] }) }),
    });
  });

  it("retains two different prohibited actions under a shared can't", () => {
    // Storm, Windrider 06e9287b-56d4-443e-97ba-b5dcdc4fa989.
    assert.deepEqual(stat("Creatures with flying can't attack you or block creatures you control."), expected({ rule: "all-of", rules: [
      { rule: "attack-restriction", defenders: [{ defender: "player", who: { player: "you" } }] },
      { rule: "blocking-filter", mode: "exclude", attackers: each({ types: ["creature"], control: "you" }) },
    ] }, each({ types: ["creature"], withAbility: ["flying"] })));
  });
});

describe("combat parser integration and rejection boundaries", () => {
  it("leaves temporary duration with the enclosing effect and keeps inherited subjects", () => {
    // Act of Heroism 25edb501-11b5-4617-8a65-2eb4869cccd5 (final clause).
    const c = new Cursor(lex("can block an additional creature this turn"));
    assert.deepEqual(parseCombatEffect(c, { ref: "it" }), [{ effect: "combat-rule", what: { ref: "it" }, rule: { rule: "block-capacity", capacity: { additional: fixed(1) } } }]);
    assert.deepEqual(parseDuration(c), { duration: "this-turn" });
    assert.equal(c.done(), true);
    // Valor Made Real 5bd1348c-51cc-4426-b995-8bb6d4a5bd2e.
    const result = parseOracleText("Target creature can block any number of creatures this turn.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{
      effect: "combat-rule", what: { ref: "target", filter: { types: ["creature"] }, count: fixed(1) },
      rule: { rule: "block-capacity", capacity: { maximum: "unlimited" } }, duration: { duration: "this-turn" },
    }] }] }]);
  });

  it("keeps existing simple static nodes unchanged and parses new detailed statics", () => {
    const legacy = parseOracleText("This creature attacks each combat if able.");
    assert.equal(legacy.ok, true);
    assert.deepEqual(legacy.abilities, [{ kind: "static", abilityWord: undefined, effect: { static: "must-attack", what: thisCreature } }]);
    const result = parseOracleText("This creature can't be blocked by more than one creature.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "static", abilityWord: undefined, effect: expected({ rule: "blocker-count", mode: "exclude", comparison: { op: "gt", value: fixed(1) } }) }]);
  });

  it("retains additional blocking capacity per Equipment attached to the source", () => {
    const result = parseOracleText("This creature can block an additional creature each combat for each Equipment attached to this creature.");
    assert.ok(result.ok);
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [{ kind: "static", effect: {
      static: "modify", what: thisCreature, modifiers: [{ modifier: "combat-rule",
        rule: { rule: "block-capacity", capacity: { additional: fixed(1) } },
        forEach: { subtypes: ["Equipment"], attachedTo: thisCreature },
      }],
    } }]);
  });

  it("backtracks and refuses incomplete or unrepresented combat rules", () => {
    for (const text of [
      "This creature can't be blocked except by.",
      "This creature can't be blocked by creatures with flavor text.",
      "This creature can't be blocked except by three or more creatures with flying.",
      "This creature can block an additional creature each combat for each Equipment attached to this creature.",
      "This creature attacks an opponent with the most life each combat if able.",
      "Target creature can block an additional creature this turn.",
      "This creature can't be blocked by Walls until end of turn.",
      "This creature can block only Imaginary creatures.",
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseCombatStatic(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    for (const text of [
      "This creature can't be blocked except by three or more creatures with flying.",
      "This creature can't be blocked by creatures with flavor text.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
