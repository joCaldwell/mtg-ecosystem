import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { ParseCardResult } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseAmount, parseFilter, parseObjectRef, parsePlayerRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// Scryfall corpus SHA256 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60:
// Blood Cultist, Fatal Blow, Retaliate, Wicked Akuba, Anger of the Gods,
// Ezuri, Claw of Progress, and Mizzix of the Izmagnus.
// CR 120.3/7, 122.1, 400.7, 608.2h:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 } as const;
const you = { player: "you" } as const;
const source = { ref: "this", noun: "creature" } as const;
const turn = { during: "this-turn" } as const;
const experience = { amount: "counter-count", counter: { type: "experience" }, on: you } as const;
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text, "test"));
  const result = parser(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.ok(c.done(), text);
  return result;
}
function incomplete<T>(parser: (c: Cursor) => T | null, text: string): void {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result === null || !c.done(), `must reject: ${text}`);
  if (result === null) assert.equal(c.pos, 0);
}
function parsed(text: string, name = "Fixture"): Extract<ParseCardResult, { ok: true }> {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  return result;
}

describe("damage history on objects and players", () => {
  it("distinguishes who dealt damage from who received it", () => {
    for (const text of ["creature dealt damage this turn", "creature that was dealt damage this turn", "creature that has been dealt damage this turn"])
      assert.deepEqual(complete(parseFilter, text), { types: ["creature"], damageHistory: { role: "recipient", window: turn } });
    for (const text of ["creatures that dealt damage this turn", "creatures that have dealt damage this turn"])
      assert.deepEqual(complete(parseFilter, text), { types: ["creature"], damageHistory: { role: "source", window: turn } });
    assert.deepEqual(complete(parseFilter, "creatures dealt combat damage by this creature this combat"), {
      types: ["creature"], damageHistory: { role: "recipient", source, damage: "combat", window: { during: "this-combat" } },
    });
    assert.deepEqual(complete(parseFilter, "creatures that dealt noncombat damage to target player this turn"), {
      types: ["creature"], damageHistory: { role: "source", to: { player: "target-player", selectionId: "test:6" }, damage: "noncombat", window: turn },
    });
  });
  it("retains same-instruction event links separately from turn-wide history", () => {
    assert.deepEqual(complete(parseFilter, "creatures dealt damage this way"), { types: ["creature"], damageHistory: { role: "recipient", window: { event: "this-way" } } });
    assert.deepEqual(complete(parseFilter, "creatures that dealt damage this turn and that were dealt damage this turn"), {
      types: ["creature"], damageHistory: { role: "source", window: turn }, allOf: [{ damageHistory: { role: "recipient", window: turn } }],
    });
    assert.deepEqual(complete(parseObjectRef, "target creature or planeswalker an opponent controls that was dealt damage this turn"), {
      ref: "target", count: one, selectionId: "test:0", filter: {
        allOf: [{ anyOf: [{ types: ["creature"] }, { types: ["planeswalker"] }] }], control: "opponent",
        damageHistory: { role: "recipient", window: turn },
      },
    });
  });
  it("does not invent historical quantities, universal sources, or missing time windows", () => {
    for (const text of [
      "creatures dealt damage", "creatures that dealt damage last turn", "creatures dealt 3 damage this turn",
      "creatures dealt damage by three Pirates this turn", "creatures dealt damage by each creature this turn",
      "creatures dealt damage by up to one target creature this turn", "creatures dealt damage this turn by a source you controlled",
      "creatures that didn't deal damage this turn", "creatures that were dealt excess damage this turn",
      "creatures dealt damage by a creature and with flying this turn", "creatures dealt damage by a source you controlled this turn",
    ]) incomplete(parseFilter, text);
    incomplete(parsePlayerRef, "target player who dealt damage this turn");
  });
  it("preserves Blood Cultist's historical recipient in its dies event", () => {
    assert.deepEqual(parsed("Whenever a creature dealt damage by this creature this turn dies, put a +1/+1 counter on this creature.", "Blood Cultist").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "dies", what: { ref: "filter", count: one, filter: { types: ["creature"], damageHistory: { role: "recipient", source, window: turn } } } },
      effects: [{ steps: [{ effect: "put-counters", count: one, counter: { type: "+1/+1" }, on: source }] }],
    }]);
  });
  it("keeps Fatal Blow's target history and regeneration prohibition together", () => {
    assert.deepEqual(parsed("Destroy target creature that was dealt damage this turn. It can't be regenerated.", "Fatal Blow").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "destroy", regenerationForbidden: true,
        what: { ref: "target", count: one, selectionId: "line:0:1", filter: { types: ["creature"], damageHistory: { role: "recipient", window: turn } } },
      }] }],
    }]);
    assert.deepEqual(parsed("Destroy all creatures that dealt damage to you this turn.", "Retaliate").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "destroy", what: { ref: "all", filter: {
        types: ["creature"], damageHistory: { role: "source", to: you, window: turn },
      } } }] }],
    }]);
  });
  it("preserves Wicked Akuba's target-player identity under recipient eligibility", () => {
    assert.deepEqual(parsed("{B}: Target player dealt damage by this creature this turn loses 1 life.", "Wicked Akuba").abilities, [{
      kind: "activated", abilityWord: undefined, restriction: undefined, costs: [{ cost: "mana", symbols: ["B"] }],
      effects: [{ steps: [{ effect: "lose-life", amount: one, who: { player: "filtered", base: { player: "target-player", selectionId: "line:0:2" }, filter: {
        damageHistory: { role: "recipient", source, window: turn },
      } } }] }],
    }]);
  });
  it("does not confuse the earlier damage event with a later replacement's lifetime", () => {
    assert.deepEqual(parsed("If a creature dealt damage this way would die this turn, exile it instead.").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "create-replacement", duration: { duration: "this-turn" }, replacement: {
        event: { event: "move-zone", what: { ref: "filter", count: one, filter: { types: ["creature"], damageHistory: { role: "recipient", window: { event: "this-way" } } } }, from: { zone: "battlefield" }, to: { zone: "graveyard" } },
        occurrence: "each", replace: { replacement: "effects", effects: [{ steps: [{ effect: "exile", what: { ref: "replacement-event-object", role: "affected" } }] }] },
      } }] }],
    }]);
  });
  it("applies indefinite historical prohibitions to every qualifying recipient", () => {
    // CR 611.2c/614.9: a damage redirection can make more than one creature
    // receive the original instruction's damage. "A creature ... can't" is
    // a generic rule, not a choice of one member of that damaged set.
    const recipients = { ref: "each", filter: { types: ["creature"], damageHistory: { role: "recipient", window: { event: "this-way" } } } } as const;
    assert.deepEqual(parsed("Incinerate deals 3 damage to any target. A creature dealt damage this way can't be regenerated this turn.", "Incinerate").abilities, [{
      kind: "spell", abilityWord: undefined, effects: [
        { steps: [{ effect: "damage", source: { ref: "self" }, amount: { amount: "fixed", value: 3 }, to: { to: "any-target", selectionId: "line:0:6" } }] },
        { steps: [{ effect: "cant", what: recipients, action: "be-regenerated", duration: { duration: "this-turn" } }] },
      ],
    }]);
    assert.deepEqual(parsed("{2}{R}: This creature deals 1 damage to any target. A creature dealt damage this way can't block this turn.", "Ballista Wielder").abilities, [{
      kind: "activated", abilityWord: undefined, restriction: undefined, costs: [{ cost: "mana", symbols: ["2", "R"] }], effects: [
        { steps: [{ effect: "damage", source, amount: one, to: { to: "any-target", selectionId: "line:0:10" } }] },
        { steps: [{ effect: "cant", what: recipients, action: "block", duration: { duration: "this-turn" } }] },
      ],
    }]);
  });
  it("binds a history filter's references and leaves its event record to the engine", () => {
    const result = parsed("Destroy target creature dealt damage by this creature this turn.");
    const graph = analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.equal(graph.referencesBound, true, JSON.stringify(graph.diagnostics));
    assert.ok(graph.engineFeatures.some(need => need.feature === "event-history" && need.path.endsWith("/damageHistory")), JSON.stringify(graph.engineFeatures));
    assert.ok(graph.references.some(r => "ref" in r.expression && r.expression.ref === "target" && r.expression.selectionId === "line:0:1"));
  });
});

describe("counters held by players as numeric expressions", () => {
  it("preserves counter kind and dependent/targeted player identities", () => {
    assert.deepEqual(complete(parseAmount, "the number of experience counters you have"), experience);
    assert.deepEqual(complete(parseAmount, "the number of poison counters that player has"), { amount: "counter-count", counter: { type: "poison" }, on: { player: "that-player" } });
    assert.deepEqual(complete(parseAmount, "the number of experience counters they have"), { amount: "counter-count", counter: { type: "experience" }, on: { player: "they" } });
    assert.deepEqual(complete(parseAmount, "the number of counters target opponent has"), { amount: "counter-count", counter: "any", on: { player: "target-opponent", selectionId: "test:4" } });
    assert.deepEqual(complete(parseAmount, "the number of experience counters on you"), experience);
    for (const text of ["the number of experience counters you had", "the number of experience counters you have this turn", "the number of different counters you have", "the number of kinds of counters you have", "the number of experience counters all players have", "the number of experience counters each player has", "the number of experience counters each opponent has"])
      incomplete(parseAmount, text);
  });
  it("retains Ezuri's X definition and Mizzix's numeric comparison", () => {
    assert.deepEqual(parsed("At the beginning of combat on your turn, put X +1/+1 counters on another target creature you control, where X is the number of experience counters you have.", "Ezuri, Claw of Progress").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "phase", phase: "combat", whose: "your" },
      effects: [{ steps: [{ effect: "put-counters", on: { ref: "target", count: one, selectionId: "line:0:19", filter: { types: ["creature"], control: "you", other: true } }, count: { amount: "x" }, counter: { type: "+1/+1" } }], bindings: [{ variable: "x", value: experience }] }],
    }]);
    assert.deepEqual(parsed("Whenever you cast an instant or sorcery spell with mana value greater than the number of experience counters you have, you get an experience counter.", "Mizzix of the Izmagnus").abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined, trigger: { trigger: "cast", who: you, what: {
        allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell", properties: [{ property: "mana-value", comparison: { op: "gt", value: experience } }],
      } }, effects: [{ steps: [{ effect: "get-counters", who: you, count: one, counter: { type: "experience" } }] }],
    }]);
  });
});
