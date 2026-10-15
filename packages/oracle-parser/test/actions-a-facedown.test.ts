import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Face-down piles: CR 406.4, 701.24a, 701.40, 701.58 (https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
function parse(text: string, typeLine: string): Ability[] {
  const result = parseOracleText(text, "Test", { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function steps(text: string, typeLine: string): Effect[] {
  const [ability] = parse(text, typeLine);
  assert.ok(ability.kind === "triggered" || ability.kind === "spell");
  return ability.effects.flatMap(sentence => sentence.steps);
}
function rejected(text: string, typeLine: string): void {
  assert.equal(parseOracleText(text, "Test", { typeLine }).ok, false, text);
}

describe("face-down piles", () => {
  it("plain face-down exile is not a pile", () => {
    const [exile] = steps("Exile all creature cards from target player's graveyard face down.", "Sorcery");
    assert.equal((exile as { pile?: true }).pile, undefined);
  });
  it("rejects shuffling a pile it cannot keep together with the rest of the instruction", () => {
    rejected("Exile the top two cards of your library in a face-down pile, shuffle that pile and put it back on top of your library.", "Sorcery");
    rejected("Shuffle that pile.", "Sorcery");
  });
});

describe("face-down piles (shuffle and manifest)", () => {
  it("exiles into one face-down pile, shuffles that pile, then manifests those cards", () => {
    assert.deepEqual(steps("Exile all creature cards from target player's graveyard in a face-down pile, shuffle that pile, then manifest those cards.", "Sorcery"), [
      { effect: "exile", what: { ref: "all", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: { player: "target-player" } } } }, faceDown: true, pile: true },
      { effect: "shuffle-pile", who: { player: "you" } },
      { effect: "manifest", who: { player: "you" }, what: { ref: "that", filter: { cls: "card" }, plural: true } },
    ]);
  });
});
