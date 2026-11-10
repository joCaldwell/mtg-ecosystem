import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus.
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
const whileExiled = (what: { ref: "it" } | { ref: "them" }) =>
  ({ duration: "as-long-as", condition: { condition: "remains", what, zone: { zone: "exile" } } }) as const;

describe("looking at face-down exiled cards while they stay exiled (CR 406.3)", () => {
  it("grants a look permission for the exiled cards, each until it leaves exile", () => {
    // Jester's Scepter.
    assert.deepEqual(ability("When this artifact enters, exile the top five cards of target player's library face down. You may look at those cards for as long as they remain exiled.", "Artifact"), {
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "artifact" } },
      effects: [
        { steps: [{ effect: "exile", faceDown: true,
          what: { ref: "library-cards", position: "top", count: { amount: "fixed", value: 5 }, of: { zone: "library", owner: { player: "target-player" } } } }] },
        { steps: [{ effect: "grant-look-permission", permission: { who: you, what: [thoseCards], timing: "any-time" },
          duration: whileExiled({ ref: "them" }), durationScope: "each-permitted-card" }] },
      ],
    });
  });

  it("accepts a singular pronoun for one exiled card", () => {
    // Gustha's Scepter.
    assert.deepEqual(ability("{T}: Exile a card from your hand face down. You may look at it for as long as it remains exiled.", "Artifact"), {
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [
        { steps: [{ effect: "exile", faceDown: true,
          what: { ref: "filter", filter: { cls: "card", zone: { zone: "hand", owner: "your" } }, count: { amount: "fixed", value: 1 } } }] },
        { steps: [{ effect: "grant-look-permission", permission: { who: you, what: [{ ref: "it" }], timing: "any-time" },
          duration: whileExiled({ ref: "it" }), durationScope: "each-permitted-card" }] },
      ],
    });
  });

  it("needs the period to name the same exiled card(s)", () => {
    fails("Exile target card from a graveyard. You may look at those cards for as long as it remains exiled.");
    fails("Exile target creature. You may look at that creature for as long as it remains exiled.");
    fails("Exile the top card of your library face down. You may look at that card until end of turn.");
    fails("Exile the top card of your library face down. Until end of turn, you may look at that card for as long as it remains exiled.");
  });
});

describe("the top cards of the library just shuffled, with the library unstated", () => {
  const shuffle = { effect: "shuffle", who: you, library: { zone: "library", owner: "your" } } as const;
  const top = (value: number) => ({ ref: "library-cards", position: "top", count: { amount: "fixed", value }, of: { zone: "library", owner: "your" } }) as const;
  it("reads the elided library as the one the same sentence shuffled", () => {
    // Urza, Lord High Artificer.
    assert.deepEqual(ability("{5}: Shuffle your library, then exile the top card. Until end of turn, you may play that card without paying its mana cost.", "Legendary Creature — Human Artificer"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["5"] }],
      effects: [
        { steps: [shuffle, { effect: "exile", what: top(1) }] },
        { steps: [{ effect: "grant-play-permission", duration: { duration: "end-of-turn" },
          permission: { who: you, action: "play", what: { ref: "that", filter: { cls: "card" } }, alternativeCost: { cost: "without-mana-cost" } } }] },
      ],
    });
    // Hazoret's Undying Fury (first sentence).
    assert.deepEqual(ability("Shuffle your library, then exile the top four cards."), { kind: "spell", effects: [{ steps: [shuffle, { effect: "exile", what: top(4) }] }] });
    // Unexpected Results (first sentence).
    assert.deepEqual(ability("Shuffle your library, then reveal the top card."),
      { kind: "spell", effects: [{ steps: [shuffle, { effect: "reveal", who: you, what: top(1) }] }] });
  });

  it("is the same instruction as the printed library", () => {
    assert.deepEqual(ability("Shuffle your library, then exile the top four cards."), ability("Shuffle your library, then exile the top four cards of your library."));
  });

  it("needs the shuffle of your own library right before it", () => {
    fails("Exile the top card. You may play that card this turn.");
    fails("Target player shuffles their library, then exiles the top card.");
    fails("Shuffle your library. Exile the top card.");
    fails("Shuffle your library, then exile the top cards.");
  });
});
