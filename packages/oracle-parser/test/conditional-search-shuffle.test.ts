import assert from "node:assert/strict";
import { it } from "node:test";
import { parseOracleText } from "../src/index.ts";

// Deicide current Oracle from frozen corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// Original explicit conditional scope:
// https://magic.wizards.com/en/news/feature/release-notes-2014-04-23
const parse = (text: string) => {
  const result = parseOracleText(text, undefined, { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result));
  const ability = result.abilities[0];
  assert.equal(ability.kind, "spell");
  if (ability.kind !== "spell") throw new Error("Expected spell");
  return JSON.parse(JSON.stringify(ability.effects));
};
const core = "Exile target enchantment. If the exiled card is a God card, search its controller's graveyard, hand, and library for any number of cards with the same name as that card and exile them";
it("keeps Deicide's shuffle under the original God condition, checked once", () => {
  const joined = parse(`${core}, then that player shuffles.`);
  const separated = parse(`${core}. Then that player shuffles.`);
  // The differing punctuation precedes no target declaration; provenance is unchanged.
  assert.deepEqual(separated, joined);
  assert.equal(separated.length, 2);
  assert.deepEqual(separated[1].condition, { condition: "is-filter", what: {
    ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" },
  }, filter: { subtypes: ["God"], cls: "card" } });
  assert.deepEqual(separated[1].steps.map((s: { effect: string }) => s.effect), ["search", "exile", "shuffle"]);
  assert.deepEqual(separated[1].steps[2], { effect: "shuffle", who: { player: "that-player" }, library: { zone: "library", owner: { player: "that-player" } } });
});
it("does not make other then instructions depend on a preceding condition", () => {
  const independent = parse("If you control a Forest, you gain 1 life. Then draw a card.");
  assert.equal(independent.length, 2);
  assert.equal(independent[1].condition, undefined);
  const counterbore = parse("Counter target spell. Search its controller's graveyard, hand, and library for all cards with the same name as that spell and exile them. Then that player shuffles.");
  assert.equal(counterbore.length, 3);
  assert.equal(counterbore[2].condition, undefined);
});
it("rejects a dependent shuffle when the conditional searched-owner scope is ambiguous", () => {
  for (const search of [
    "search your library for a card and exile it",
    "search your library and target opponent's library for a card and exile them",
  ]) {
    assert.equal(parseOracleText(`If you control a Forest, ${search}. Then that player shuffles.`).ok, false);
  }
});
