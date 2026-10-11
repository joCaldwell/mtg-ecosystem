import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parseCondition, parseFilter, parseObjectRef, parseZoneRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/index.ts";

// Verified in Scryfall oracle corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 607.1–2 (linked abilities), 400.7 (zone-change objects), 608.2h
// (last known information):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
function complete<T>(parser: (c: Cursor) => T | null, text: string): T {
  const c = new Cursor(lex(text));
  const result = parser(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens`);
  return result;
}
const contextual = (event: "exile" | "sacrifice" | "discard" | "reveal" | "destroy") => ({ relation: "event-result" as const, event, scope: "contextual" as const });

describe("related references preserve their operation and scope", () => {
  it("keeps singular and plural definite event results as symbolic antecedents", () => {
    assert.deepEqual(complete(parseObjectRef, "the exiled card"), {
      ref: "related", number: "singular", filter: { cls: "card" }, relation: contextual("exile"),
    });
    assert.deepEqual(complete(parseObjectRef, "the revealed cards"), {
      ref: "related", number: "plural", filter: { cls: "card" }, relation: contextual("reveal"),
    });
    assert.deepEqual(complete(parseObjectRef, "the discarded creature card"), {
      ref: "related", number: "singular", filter: { types: ["creature"], cls: "card" }, relation: contextual("discard"),
    });
    assert.deepEqual(complete(parseObjectRef, "the sacrificed Goblins"), {
      ref: "related", number: "plural", filter: { subtypes: ["Goblin"] }, relation: contextual("sacrifice"),
    });
  });

  it("separates explicit source links from this-way results and retains the actor", () => {
    assert.deepEqual(complete(parseObjectRef, "the cards exiled with it"), {
      ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source: { ref: "it" } },
    });
    assert.deepEqual(complete(parseObjectRef, "creature cards exiled this way"), {
      ref: "related", number: "plural", filter: { types: ["creature"], cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "this-way" },
    });
    assert.deepEqual(complete(parseObjectRef, "the card they exiled this way"), {
      ref: "related", number: "singular", filter: { cls: "card" }, relation: {
        relation: "event-result", event: "exile", scope: "this-way", who: { player: "they" },
      },
    });
  });

  it("retains event-period constraints on linked sets rather than effect durations", () => {
    assert.deepEqual(complete(parseObjectRef, "cards exiled with this creature this turn"), {
      ref: "related", number: "plural", filter: { cls: "card" }, relation: {
        relation: "linked-exile", source: { ref: "this", noun: "creature" }, period: "this-turn",
      },
    });
    assert.deepEqual(complete(parseFilter, "artifact cards exiled this way this turn"), {
      types: ["artifact"], cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way", period: "this-turn" },
    });
  });

  it("counts the qualifying event results rather than all cards in exile", () => {
    // Grey Host Reinforcements 053dff47-25df-424b-8328-3ee00739b4fd.
    assert.deepEqual(complete(parseAmount, "the number of creature cards exiled this way"), {
      amount: "count", of: { types: ["creature"], cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" } },
    });
  });

  it("retains possessive attributes of the sacrificed or exiled object", () => {
    // Bushmeat Poacher 0287d541-3f73-4c8b-9a77-87f99898758d.
    assert.deepEqual(complete(parseAmount, "the sacrificed creature's toughness"), {
      amount: "attribute", attribute: "toughness", of: {
        ref: "related", number: "singular", filter: { types: ["creature"] }, relation: contextual("sacrifice"),
      },
    });
    assert.deepEqual(complete(parseAmount, "the exiled card's mana value"), {
      amount: "attribute", attribute: "mana-value", of: {
        ref: "related", number: "singular", filter: { cls: "card" }, relation: contextual("exile"),
      },
    });
  });

  it("parses a complete cross-ability linked return without replacing it with it", () => {
    // Slithery Stalker 0c31edd0-a471-4994-8209-b0a7370fc27b.
    const text = "When this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control.";
    const result = parseOracleText(text);
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "leaves", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{
        effect: "move-zone", what: { ref: "related", number: "singular", filter: { cls: "card" }, relation: contextual("exile") },
        to: { zone: "battlefield" }, controller: "owner",
      }] }],
    }]);
  });

  it("parses linked sacrifice-cost value without losing the operation", () => {
    const result = parseOracleText("{1}, {T}, Sacrifice another creature: You gain life equal to the sacrificed creature's toughness. Draw a card.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }, { cost: "sacrifice", what: {
        ref: "filter", filter: { types: ["creature"], other: true }, count: fixed(1),
      } }], effects: [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: {
        amount: "attribute", attribute: "toughness", of: { ref: "related", number: "singular", filter: { types: ["creature"] }, relation: contextual("sacrifice") },
      } }] }, { steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
    }]);
  });

  it("rejects unknown relations, unrepresented temporal clauses, and missing linked sources", () => {
    for (const text of ["the imaginary card", "the cards exiled with", "the cards exiled with it last turn", "the cards sacrificed with it", "exiled cards", "the cards exiled this way by an opponent"]) {
      const c = new Cursor(lex(text));
      const result = parseObjectRef(c);
      assert.ok(result === null || !c.done(), text);
      if (result === null) assert.equal(c.pos, 0);
    }
  });
});

describe("selection pools and personal-zone references", () => {
  it("keeps from-among constraints separate from the selected objects", () => {
    assert.deepEqual(complete(parseObjectRef, "a Human card from among them"), {
      ref: "filter", count: fixed(1), filter: { subtypes: ["Human"], cls: "card", fromAmong: { ref: "them" } },
    });
    // Collected Company 0bbcb563-09b3-499d-a9cc-3a6d7501dd81.
    assert.deepEqual(complete(parseObjectRef, "up to two creature cards with mana value 3 or less from among them"), {
      ref: "filter", count: fixed(2), upTo: true, filter: {
        types: ["creature"], cls: "card", properties: [{ property: "mana-value", comparison: { op: "le", value: fixed(3) } }],
        fromAmong: { ref: "them" },
      },
    });
  });

  it("retains an implicit or explicit complement pool and an all-card selection", () => {
    assert.deepEqual(complete(parseObjectRef, "the rest"), { ref: "rest" });
    assert.deepEqual(complete(parseObjectRef, "the rest of those cards"), {
      ref: "rest", of: { ref: "that", filter: { cls: "card" }, plural: true },
    });
    assert.deepEqual(complete(parseObjectRef, "all the cards in your graveyard"), {
      ref: "all", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } },
    });
  });

  it("retains the personal zone's player, including targeted and quantified players", () => {
    for (const [phrase, player] of [["target player's", "target-player"], ["each opponent's", "each-opponent"], ["your", "you"], ["their", "they"]] as const) {
      assert.deepEqual(complete(parseObjectRef, `${phrase} graveyard`), { ref: "zone-cards", zone: "graveyard", owner: { player } });
    }
    // Bojuka Bog 04b7362d-0490-4cb0-b5d7-2a7732f659ce.
    const result = parseOracleText("When this land enters, exile target player's graveyard.");
    assert.equal(result.ok, true, JSON.stringify(result.lines));
    assert.deepEqual(result.abilities, [{ kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "enters", what: { ref: "this", noun: "land" } }, effects: [{ steps: [{
        effect: "exile", what: { ref: "zone-cards", zone: "graveyard", owner: { player: "target-player" } },
      }] }],
    }]);
  });

  it("keeps commander designation independent of creature type and unique identity", () => {
    // CR 903.3d, 702.124e. Road of Return 0007c283-5b7a-4c00-9ca1-b455c8dff8c3.
    assert.deepEqual(complete(parseObjectRef, "your commander"), { ref: "commander", owner: { player: "you" }, count: fixed(1) });
    assert.deepEqual(complete(parseFilter, "commanders you control"), { commander: true, control: "you" });
    assert.deepEqual(complete(parseCondition, "you control a commander"), {
      condition: "control", who: { player: "you" }, what: { ref: "filter", count: fixed(1), filter: { commander: true } },
    });
    assert.deepEqual(complete(parseZoneRef, "the command zone"), { zone: "command" });
  });
});
