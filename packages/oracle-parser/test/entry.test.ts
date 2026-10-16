import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseBattlefieldEntryDetails } from "../src/parser/entry.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
function ability(text: string, name = "Entry fixture"): Ability {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  if (!result.ok || result.abilities.length !== 1) throw new Error("Expected one complete ability");
  return result.abilities[0];
}
function effects(text: string): Effect[] {
  const parsed = ability(text);
  assert.equal(parsed.kind, "spell");
  if (parsed.kind !== "spell") throw new Error("Expected spell");
  return parsed.effects.flatMap(sentence => sentence.steps);
}

// Card examples verified against the cached Scryfall corpus. The resulting
// nodes modify entry, never perform later counter/attach actions: CR 122.6,
// 303.4f–h, 508.4, 614.12a.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("battlefield-entry replacement choices", () => {
  it("stores the pay-life choice and declined entry state together", () => {
    // Stomping Ground / Witch-Blessed Meadow.
    for (const amount of [2, 3]) assert.deepEqual(ability(`As this land enters, you may pay ${amount} life. If you don't, it enters tapped.`), {
      kind: "static", abilityWord: undefined, effect: { static: "entry-choice", what: { ref: "this", noun: "land" }, who: { player: "you" },
        costs: [{ cost: "pay-life", amount: fixed(amount) }], declined: { tapped: true } },
    });
  });

  it("retains the revealed type alternatives and owned source zone (Game Trail)", () => {
    assert.deepEqual(ability("As this land enters, you may reveal a Mountain or Forest card from your hand. If you don't, this land enters tapped."), {
      kind: "static", abilityWord: undefined, effect: { static: "entry-choice", what: { ref: "this", noun: "land" }, who: { player: "you" },
        costs: [{ cost: "reveal", what: { ref: "filter", count: fixed(1), filter: {
          allOf: [{ anyOf: [{ subtypes: ["Mountain"] }, { subtypes: ["Forest"] }] }], cls: "card", zone: { zone: "hand", owner: "your" },
        } } }], declined: { tapped: true } },
    });
  });

  it("rejects unmatched objects, altered alternatives, and additional unsupported riders", () => {
    for (const text of [
      "As this land enters, you may pay 2 life. If you don't, target land enters tapped.",
      "As this land enters, you may pay 2 life. If you do, it enters tapped.",
      "As this land enters, you may pay 2 life. If you don't, it enters tapped. Invented rider.",
    ]) assert.equal(parseOracleText(text, "Fixture").ok, false, text);
  });

  it("retains entry counter multipliers, additional quantities, and source exclusions", () => {
    // Dawn of a New Age / Hamlet Vanguard / Aeve, Progenitor Ooze.
    assert.deepEqual(ability("This enchantment enters with a hope counter on it for each creature you control."), {
      kind: "static", abilityWord: undefined, effect: { static: "enters-with-counters", what: { ref: "this", noun: "enchantment" },
        count: fixed(1), counter: { type: "hope" }, multiplier: { amount: "count", of: { types: ["creature"], control: "you" } } },
    });
    assert.deepEqual(ability("This creature enters with two +1/+1 counters on it for each other nontoken Human you control."), {
      kind: "static", abilityWord: undefined, effect: { static: "enters-with-counters", what: { ref: "this", noun: "creature" },
        count: fixed(2), counter: { type: "+1/+1" }, multiplier: { amount: "count", of: {
          otherThan: { ref: "self" }, nonToken: true, subtypes: ["Human"], control: "you",
        } } },
    });
    const counters = ability("This creature enters with an additional +1/+1 counter on it for each loyalty counter on planeswalkers you control.");
    assert.deepEqual(counters, { kind: "static", abilityWord: undefined, effect: { static: "enters-with-counters",
      what: { ref: "this", noun: "creature" }, count: fixed(1), additional: true, counter: { type: "+1/+1" },
      multiplier: { amount: "counter-count", counter: { type: "loyalty" }, on: { ref: "each", filter: { types: ["planeswalker"], control: "you" } } },
    } });
  });

  it("keeps dynamic counter totals and historical multikicker counts symbolic", () => {
    // Rhizome Lurcher / Everflowing Chalice.
    assert.deepEqual(ability("This creature enters with a number of +1/+1 counters on it equal to the number of creature cards in your graveyard."), {
      kind: "static", abilityWord: undefined, effect: { static: "enters-with-counters", what: { ref: "this", noun: "creature" },
        counter: { type: "+1/+1" }, count: { amount: "count", of: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } } },
    });
    assert.deepEqual(ability("This artifact enters with a charge counter on it for each time it was kicked."), {
      kind: "static", abilityWord: undefined, effect: { static: "enters-with-counters", what: { ref: "this", noun: "artifact" },
        counter: { type: "charge" }, count: fixed(1), multiplier: { amount: "kicker-count", of: { ref: "this", noun: "artifact" } } },
    });
  });

  it("stores a kicked condition on the entry replacement (Myriad Construct)", () => {
    assert.deepEqual(ability("If this creature was kicked, it enters with a +1/+1 counter on it for each nonbasic land your opponents control."), {
      kind: "static", abilityWord: undefined, effect: { static: "conditional", condition: { condition: "kicked",
        what: { ref: "this", noun: "creature" }, comparison: { op: "ge", value: fixed(1) } }, effect: {
        static: "enters-with-counters", what: { ref: "it" }, counter: { type: "+1/+1" }, count: fixed(1),
        multiplier: { amount: "count", of: { nonSupertypes: ["basic"], types: ["land"], control: "opponent" } },
      } },
    });
  });
});

describe("same-event battlefield entry modifications", () => {
  it("places counters during the zone move (Persist and Daydream)", () => {
    assert.deepEqual(effects("Return target nonlegendary creature card from your graveyard to the battlefield with a -1/-1 counter on it."), [{
      effect: "move-zone", what: { ref: "target", count: fixed(1), filter: {
        nonSupertypes: ["legendary"], types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" },
      } }, to: { zone: "battlefield" }, entry: { counters: [{ count: fixed(1), counter: { type: "-1/-1" } }] },
    }]);
    const blink = effects("Exile target creature you control, then return that card to the battlefield under its owner's control with a +1/+1 counter on it.");
    assert.equal(blink.length, 2);
    assert.deepEqual(blink[1], { effect: "move-zone", what: { ref: "that", filter: { cls: "card" } },
      to: { zone: "battlefield" }, controller: "owner", entry: { counters: [{ count: fixed(1), counter: { type: "+1/+1" } }] } });
  });

  it("keeps tapped state, additional counters, and multiple counter kinds on one event", () => {
    const cursors = [
      ["tapped with two +1/+1 counters on it", { tapped: true, entry: { counters: [{ count: fixed(2), counter: { type: "+1/+1" } }] } }],
      ["with two additional +1/+1 counters on it", { entry: { counters: [{ count: fixed(2), counter: { type: "+1/+1" }, additional: true }] } }],
      ["under its owner's control with a vigilance counter and a lifelink counter on it", { controller: "owner", entry: { counters: [
        { count: fixed(1), counter: { type: "vigilance" } }, { count: fixed(1), counter: { type: "lifelink" } },
      ] } }],
    ] as const;
    for (const [text, expected] of cursors) {
      const cursor = new Cursor(lex(text));
      assert.deepEqual(parseBattlefieldEntryDetails(cursor), expected);
      assert.equal(cursor.done(), true);
    }
  });

  it("retains entering-attacking defender choice separately from named defenders", () => {
    assert.deepEqual(effects("Put a creature card from your hand onto the battlefield tapped and attacking."), [{
      effect: "move-zone", what: { ref: "filter", filter: { types: ["creature"], cls: "card", zone: { zone: "hand", owner: "your" } }, count: fixed(1) },
      to: { zone: "battlefield" }, tapped: true, entry: { attacking: { choice: "entering-controller" } },
    }]);
    const cursor = new Cursor(lex("tapped and attacking that player"));
    assert.deepEqual(parseBattlefieldEntryDetails(cursor), { tapped: true, entry: { attacking: { defender: { player: "that-player" } } } });
    assert.equal(cursor.done(), true);
  });

  it("keeps entering attached as part of the move, without a later attach action", () => {
    assert.deepEqual(effects("Return this card from your graveyard to the battlefield attached to that creature."), [{
      effect: "move-zone", what: { ref: "this", noun: "card" }, from: { zone: "graveyard", owner: "your" }, to: { zone: "battlefield" },
      entry: { attachedTo: { ref: "that", filter: { types: ["creature"] } } },
    }]);
    const plural = new Cursor(lex("attached to creatures you control"));
    assert.equal(parseBattlefieldEntryDetails(plural), null);
    assert.equal(plural.pos, 0);
  });

  it("backtracks malformed or duplicate modifiers and leaves later instruction separators", () => {
    for (const text of ["with a +1/+1 counter", "tapped tapped", "with a stun counter on it with a shield counter on it", "attached to", "attacking each opponent"]) {
      const cursor = new Cursor(lex(text));
      assert.equal(parseBattlefieldEntryDetails(cursor), null, text);
      assert.equal(cursor.pos, 0, text);
    }
    const cursor = new Cursor(lex("tapped and draw a card"));
    assert.deepEqual(parseBattlefieldEntryDetails(cursor), { tapped: true });
    assert.equal(cursor.isWord("and"), true);
    for (const text of [
      "Return target creature card from your graveyard to the battlefield from your hand.",
      "Put a creature card from your hand onto the battlefield from your graveyard.",
    ]) assert.equal(parseOracleText(text, "Fixture").ok, false, text);
  });

  it("binds the source's own entry choices to the player it enters under (CR 110.2a, 614.12a)", () => {
    const result = parseOracleText("As this land enters, you may pay 2 life. If you don't, it enters tapped.", "Fixture", { includeSourceProvenance: true });
    const graph = analyzeCardBindings({ identity: "entry", source: { name: "Fixture", layout: "normal", type_line: "Land" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    const payer = graph.references.find(ref => ref.path.endsWith("/costs/0/payer"));
    assert.ok(payer && payer.binding === "bound");
    assert.ok(graph.nodes.some(node => node.id === payer.target && node.node === "runtime-player" && node.role === "entering-controller"));
  });

  it("keeps entering attached or attacking explicitly unresolved", () => {
    for (const text of ["Return target Aura card from your graveyard to the battlefield attached to a creature you control.",
      "Put a creature card from your hand onto the battlefield tapped and attacking."]) {
      const result = parseOracleText(text, "Fixture", { includeSourceProvenance: true });
      assert.equal(result.ok, true);
      const graph = analyzeCardBindings({ identity: "entry", source: { name: "Fixture", layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(graph.fullyBound, false);
      assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope"));
    }
  });
});
