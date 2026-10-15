import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 406.3, 406.3b
// (looking at and casting face-down exiled cards), 611.2a, 601.2.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Sorcery"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const you = { player: "you" } as const;
const thoseCards = { ref: "that", filter: { cls: "card" }, plural: true } as const;
const thatCard = { ref: "that", filter: { cls: "card" } } as const;
const whileExiled = { duration: "as-long-as", condition: { condition: "remains", what: { ref: "them" }, zone: { zone: "exile" } } } as const;

describe("looking at and playing the same exiled cards (CR 406.3, 406.3b)", () => {
  it("grants both permissions for the same cards and period", () => {
    // Decadent Dragon // Expensive Taste.
    assert.deepEqual(ability("Exile the top two cards of target opponent's library face down. You may look at and play those cards for as long as they remain exiled."), {
      kind: "spell", effects: [
        { steps: [{ effect: "exile", faceDown: true,
          what: { ref: "library-cards", position: "top", count: { amount: "fixed", value: 2 }, of: { zone: "library", owner: { player: "target-opponent" } } } }] },
        { steps: [{ effect: "simultaneous", steps: [
          { effect: "grant-look-permission", permission: { who: you, what: [thoseCards], timing: "any-time" }, duration: whileExiled, durationScope: "each-permitted-card" },
          { effect: "grant-play-permission", permission: { who: you, action: "play", what: thoseCards }, duration: whileExiled, durationScope: "each-permitted-card" },
        ] }] },
      ],
    });
  });

  it("keeps the shared duration and the casting payment rider", () => {
    // Headliner Scarlett.
    assert.deepEqual(ability("You may look at and play that card this turn."), {
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "grant-look-permission", permission: { who: you, what: [thatCard], timing: "any-time" }, duration: { duration: "this-turn" } },
        { effect: "grant-play-permission", permission: { who: you, action: "play", what: thatCard }, duration: { duration: "this-turn" } },
      ] }] }],
    });
    // Ixhel, Scion of Atraxa.
    assert.deepEqual(ability("You may look at and play those cards for as long as they remain exiled, and you may spend mana as though it were mana of any color to cast those spells."), {
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "grant-look-permission", permission: { who: you, what: [thoseCards], timing: "any-time" }, duration: whileExiled, durationScope: "each-permitted-card" },
        { effect: "grant-play-permission", permission: { who: you, action: "play", what: thoseCards, manaPayment: "any-color" }, duration: whileExiled, durationScope: "each-permitted-card" },
      ] }] }],
    });
  });

  it("needs a lasting period and the same referenced cards", () => {
    // Without a period this would be an immediate look and play.
    fails("You may look at and play that card.");
    fails("You may look at and play cards exiled with this artifact this turn.");
  });
});

describe("permissions over cards from among an exiled group, each while it stays exiled", () => {
  const exiledCards = { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } } as const;
  it("ends each card's permission when that card leaves exile", () => {
    // Cosima, God of the Voyage.
    assert.deepEqual(ability("You may play lands from among those cards for as long as they remain exiled."), {
      kind: "spell", effects: [{ steps: [{ effect: "grant-play-permission", duration: whileExiled, durationScope: "each-permitted-card",
        permission: { who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } }, fromAmong: thoseCards } }] }],
    });
    // Shadow of the Enemy.
    assert.deepEqual(ability("You may cast spells from among those cards for as long as they remain exiled, and mana of any type can be spent to cast them."), {
      kind: "spell", effects: [{ steps: [{ effect: "grant-play-permission", duration: whileExiled, durationScope: "each-permitted-card",
        permission: { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } }, fromAmong: thoseCards, manaPayment: "any-type" } }] }],
    });
  });

  it("shares the per-card lifetime between coordinated plays", () => {
    // Elder Brain.
    assert.deepEqual(ability("You may play lands and cast spells from among the exiled cards for as long as they remain exiled. If you cast a spell this way, you may spend mana as though it were mana of any color to cast it."), {
      kind: "spell", effects: [{ steps: [{ effect: "simultaneous", steps: [
        { effect: "grant-play-permission", duration: whileExiled, durationScope: "each-permitted-card",
          permission: { who: you, action: "play", what: { ref: "each", filter: { types: ["land"] } }, fromAmong: exiledCards } },
        { effect: "grant-play-permission", duration: whileExiled, durationScope: "each-permitted-card",
          permission: { who: you, action: "cast", what: { ref: "each", filter: { cls: "spell" } }, fromAmong: exiledCards, manaPayment: "any-color" } },
      ] }] }],
    });
  });

  it("does not take a zone-described class as the exiled group", () => {
    fails("You may cast spells from among cards in your graveyard for as long as they remain exiled.");
  });
});
