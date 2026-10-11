import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parsePlayerGroupActor } from "../src/parser/player-actors.ts";
import { parsePlayerRef, parsePossessivePlayer, parseZoneRef } from "../src/parser/refs.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const you = { player: "you" } as const;
const contextual = { player: "the-player" } as const;
const group = (second: PlayerRef) => ({ player: "group", members: [you, second], multiplicity: "per-member-occurrence" } as const);
function parsed(text: string, name?: string) {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

// Oracle facts are the complete named card text from the fixed Scryfall
// corpus, except the explicitly marked Angel of Destiny trigger fragment.
// Group actions retain CR101.4/121.2c/608.2e-f ordering rather than expanding
// into sequential effects or deduplicating players. Secret Rendezvous keeps
// its one target, so CR608.2b applies to the whole spell:
// https://media.wizards.com/2021/downloads/STX_Release_Notes/EN_MTGSTX_FAQ_04122021.pdf
describe("occurrence-preserving grouped player actions", () => {
  it("keeps Secret Rendezvous as one draw instruction with one target occurrence", () => {
    assert.deepEqual(parsed("You and target opponent each draw three cards.", "Secret Rendezvous"), [{ kind: "spell", effects: [{ steps: [{
      effect: "draw", who: group({ player: "target-opponent", selectionId: "line:0:2" }), amount: n(3),
    }] }] }]);
  });
  it("preserves an elided plural actor but resets the next sentence and explicit subjects", () => {
    const actor = group({ player: "target-opponent", selectionId: "line:0:2" });
    assert.deepEqual(parsed("You and target opponent each draw a card and lose 1 life. Draw a card."), [{ kind: "spell", effects: [
      { steps: [{ effect: "draw", who: actor, amount: n(1) }, { effect: "lose-life", who: actor, amount: n(1) }] },
      { steps: [{ effect: "draw", who: you, amount: n(1) }] },
    ] }]);
    assert.deepEqual(parsed("You and target opponent each draw a card, then you gain 2 life."), [{ kind: "spell", effects: [{ steps: [
      { effect: "draw", who: actor, amount: n(1) }, { effect: "gain-life", who: you, amount: n(2) },
    ] }] }]);
  });
  it("retains Bloodroot Apothecary's token creators, target and separate poison trigger", () => {
    const text = "Toxic 2 (Players dealt combat damage by this creature also get two poison counters. A player with ten or more poison counters loses the game.)\nWhen this creature enters, you and target opponent each create a Treasure token.\nWhenever an opponent sacrifices a noncreature token, that player gets two poison counters.";
    assert.deepEqual(parsed(text, "Bloodroot Apothecary"), [
      { kind: "keywords", keywords: [{ keyword: "toxic", amount: 2 }] },
      { kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{
        effect: "create-token", count: n(1), token: { predefined: "Treasure" }, who: group({ player: "target-opponent", selectionId: "line:1:7" }),
      }] }] },
      { kind: "triggered", trigger: { trigger: "sacrifices", who: { player: "an-opponent" }, what: { nonTypes: ["creature"], cls: "token" } }, effects: [{ steps: [{
        effect: "get-counters", who: { player: "that-player" }, counter: { type: "poison" }, count: n(2),
      }] }] },
    ]);
  });
  it("keeps coincident Angel of Destiny participant roles in one life-gain action", () => {
    // Trigger fragment: the official ruling requires twice the life as ONE
    // life-gain event if redirected combat damage is dealt to you.
    // https://magic.wizards.com/en/news/feature/zendikar-rising-release-notes-2020-09-10
    assert.deepEqual(parsed("Whenever a creature you control deals combat damage to a player, you and that player each gain that much life."), [{
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: n(1) }, combat: true, to: "any" },
      effects: [{ steps: [{ effect: "gain-life", who: group({ player: "that-player" }), amount: { amount: "that-much" } }] }],
    }]);
  });
  it("binds Alms Collector's second participant to the prospective draw actor", () => {
    assert.deepEqual(parsed("Flash\nIf an opponent would draw two or more cards, instead you and that player each draw a card.", "Alms Collector"), [
      { kind: "keywords", keywords: [{ keyword: "flash" }] },
      { kind: "static", effect: { static: "replacement", replacement: {
        event: { event: "draw", who: { player: "an-opponent" }, quantity: { op: "ge", value: n(2) } }, occurrence: "each",
        replace: { replacement: "effects", effects: [{ steps: [{ effect: "draw", who: group({ player: "replacement-event-player", role: "actor" }), amount: n(1) }] }] },
      } } },
    ]);
    const replacement = parsed("If an opponent would draw two or more cards, instead you and that player each create a Treasure token.");
    assert.deepEqual(replacement[0].effect.replacement.replace.effects, [{ steps: [{ effect: "create-token", count: n(1), token: { predefined: "Treasure" },
      who: group({ player: "replacement-event-player", role: "actor" }),
    }] }]);
  });
  it("preserves Sylvan Offering's separate resolution choices rather than making targets", () => {
    // Either the same or different opponents may be selected; these are
    // separate resolution choices, not target declarations.
    // https://magic.wizards.com/en/news/feature/release-notes-2014-11-10
    const choose = { steps: [{ effect: "choose-object", who: you, what: { player: "an-opponent" } }] };
    const common = { colors: ["green"], supertypes: [], types: ["creature"], keywords: [] };
    assert.deepEqual(parsed("Choose an opponent. You and that player each create an X/X green Treefolk creature token.\nChoose an opponent. You and that player each create X 1/1 green Elf Warrior creature tokens.", "Sylvan Offering"), [
      { kind: "spell", effects: [choose, { steps: [{ effect: "create-token", count: n(1), token: { ...common, subtypes: ["Treefolk"], power: { amount: "x" }, toughness: { amount: "x" } }, who: group({ player: "that-player" }) }] }] },
      { kind: "spell", effects: [choose, { steps: [{ effect: "create-token", count: { amount: "x" }, token: { ...common, subtypes: ["Elf", "Warrior"], power: n(1), toughness: n(1) }, who: group({ player: "that-player" }) }] }] },
    ]);
  });
  it("does not leak group actors into queries, targets or unrepresented distributive actions", () => {
    for (const text of [
      "You and target opponent each draw a card and sacrifice a creature.",
      "You and target opponent each reveal the top card of your library.",
      "You and target opponent may each draw a card.",
      "You and target opponent each may draw a card.",
      "You and target player each draw a card.",
      "You and that player each draws a card.",
      "You and that player draw a card.",
      "Whenever you and that player each draw a card, gain 1 life.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex("you and that player each draw a card"));
    assert.deepEqual(parsePlayerRef(c), you);
    assert.equal(c.pos, 1, "general PlayerRef grammar must not consume actor groups");
    const failed = new Cursor(lex("you and that player draw a card"));
    assert.equal(parsePlayerGroupActor(failed, parsePlayerRef), null);
    assert.equal(failed.pos, 0);
  });
});

describe("definite contextual player references", () => {
  it("retains Quiet Speculation's target library, destination and contextual shuffle actor", () => {
    assert.deepEqual(parsed("Search target player's library for up to three cards with flashback and put them into that player's graveyard. Then the player shuffles.", "Quiet Speculation"), [{ kind: "spell", effects: [
      { steps: [
        { effect: "search", who: you, zones: { selection: "all", of: [{ zone: "library", owner: { player: "target-player", selectionId: "line:0:1" } }] },
          for: { ref: "filter", filter: { cls: "card", withAbility: ["flashback"] }, count: n(3), upTo: true } },
        { effect: "move-zone", what: { ref: "them" }, to: { zone: "graveyard", owner: "that-player" } },
      ] },
      { steps: [{ effect: "shuffle", who: contextual, library: { zone: "library", owner: contextual } }] },
    ] }]);
  });
  it("parses explicit possessives without erasing quantified or relative player descriptors", () => {
    const owner = new Cursor(lex("the player's"));
    assert.deepEqual(parsePossessivePlayer(owner), contextual);
    assert.ok(owner.done());
    const zone = new Cursor(lex("the player's graveyard"));
    assert.deepEqual(parseZoneRef(zone), { zone: "graveyard", owner: contextual });
    assert.ok(zone.done());
    for (const text of ["the player with the most life", "the player who controls the most creatures", "the player to your right", "the player whose turn it is"])
      assert.equal(parsePlayerRef(new Cursor(lex(text))), null, text);
  });
  it("diagnoses group result scope and definite-player identity instead of inventing antecedents", () => {
    for (const text of ["You and target opponent each create a 1/1 white Soldier creature token. Sacrifice it.", "Target player draws a card. The player draws a card.", "The player draws a card."]) {
      const name = "Player binding fixture";
      const result = parseOracleText(text, name, { includeSourceProvenance: true });
      assert.ok(result.ok, JSON.stringify(result.lines));
      const graph = analyzeCardBindings({ identity: "player-fixture", source: { name, layout: "normal", type_line: "Sorcery" }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.diagnostics.some(d => /Grouped|definite player/.test(d.message)), JSON.stringify(graph.diagnostics));
      const it = graph.references.find(r => r.expression && "ref" in r.expression && r.expression.ref === "it");
      if (it) assert.equal(it.binding, "unresolved");
    }
  });
});
