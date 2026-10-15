import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// Oracle texts are complete abilities of cards in the fixed Scryfall corpus.
// CR 108.3: a card's owner never changes with its controller or zone, so an
// ownership qualifier is independent of "controls" and of the zone named:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("ownership qualifiers", () => {
  it("processes a card any opponent owns from exile (Mind Raker)", () => {
    assert.deepEqual(parsed("When this creature enters, you may put a card an opponent owns from exile into that player's graveyard. If you do, each opponent discards a card.", "Mind Raker", "Creature — Eldrazi Processor")[0].effects[0], { steps: [{
      effect: "move-zone", what: { ref: "filter", filter: { cls: "card", own: "opponent", zone: { zone: "exile" } }, count: n(1) },
      to: { zone: "graveyard", owner: "that-player" }, optional: true }] });
  });
  it("reads \"your opponents own\" as the same any-opponent ownership (Umbris, Fear Manifest)", () => {
    assert.deepEqual(parsed("Umbris gets +1/+1 for each card your opponents own in exile.", "Umbris", "Legendary Creature — Nightmare Horror")[0].effect.modifiers[0].forEach,
      { cls: "card", own: "opponent", zone: { zone: "exile" } });
    assert.deepEqual(parsed("When you cast this spell, you may put two cards your opponents own from exile into their owners' graveyards. If you do, counter target spell.", undefined, "Creature — Eldrazi Processor")[0].effects[0].steps[0], {
      effect: "move-zone", what: { ref: "filter", filter: { cls: "card", own: "opponent", zone: { zone: "exile" } }, count: n(2) },
      to: { zone: "graveyard", owner: "its-owner" }, optional: true });
  });
  it("keeps a specific owner as a player reference (Kaya, Orzhov Usurper; Oblivion Sower)", () => {
    assert.deepEqual(parsed("−5: Kaya deals damage to target player equal to the number of cards that player owns in exile and you gain that much life.", "Kaya", "Legendary Planeswalker — Kaya")[0].effects[0].steps[0].amount,
      { amount: "count", of: { cls: "card", ownedBy: { player: "that-player" }, zone: { zone: "exile" } } });
    assert.deepEqual(parsed("When you cast this spell, target opponent exiles the top four cards of their library, then you may put any number of land cards that player owns from exile onto the battlefield under your control.", "Oblivion Sower", "Creature — Eldrazi")[0].effects[0].steps[1], {
      effect: "move-zone", what: { ref: "filter", filter: { types: ["land"], cls: "card", ownedBy: { player: "that-player" }, zone: { zone: "exile" } }, count: { amount: "chosen", minimum: 0 } },
      to: { zone: "battlefield" }, controller: "you", optional: true });
  });
  it("stays distinct from control and from your own ownership", () => {
    assert.deepEqual(parsed("Destroy target artifact an opponent controls.", undefined, "Sorcery")[0].effects[0].steps[0].what.filter, { types: ["artifact"], control: "opponent" });
    assert.deepEqual(parsed("Exile target artifact an opponent owns.", undefined, "Sorcery")[0].effects[0].steps[0].what.filter, { types: ["artifact"], own: "opponent" });
    assert.deepEqual(parsed("Exile target artifact you own.", undefined, "Sorcery")[0].effects[0].steps[0].what.filter, { types: ["artifact"], own: "you" });
  });
  it("rejects coordinated or negated ownership it cannot state", () => {
    // "you both own and control" states both fields (Obelisk of Undoing).
    assert.match(JSON.stringify(parseOracleText("Exile target artifact you both own and control.", "Fixture", { typeLine: "Sorcery" })), /"own":"you","control":"you"/);
    fails("Exile target artifact an opponent doesn't own.");
    // "you control but don't own" is now stated by `control` + `own` together (statics-conditions).
  });
});
