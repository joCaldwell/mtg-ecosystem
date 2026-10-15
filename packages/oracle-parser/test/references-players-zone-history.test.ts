import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
const you = { player: "you" } as const;
function parsed(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const entered = { event: "enters", to: { zone: "battlefield" }, during: "this-turn" };
const died = { event: "dies", from: { zone: "battlefield" }, to: { zone: "graveyard" }, during: "this-turn" };

// Oracle texts are complete cards from the fixed Scryfall corpus. The clauses
// look back at the object's own zone change (CR 608.2i); "dies" is
// battlefield to graveyard (700.4):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("objects described by their own zone change this turn", () => {
  it("targets a creature that entered this turn (Cathedral Acolyte's ability)", () => {
    assert.deepEqual(parsed("{T}: Put a +1/+1 counter on target creature that entered this turn.", undefined, "Creature — Human Cleric"), [{
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "put-counters",
        on: { ref: "target", filter: { types: ["creature"], zoneChangeHistory: entered }, count: n(1) }, counter: { type: "+1/+1" }, count: n(1) }] }],
    }]);
  });
  it("treats \"entered the battlefield\" alike and keeps the controller it entered under", () => {
    assert.deepEqual(parsed("Destroy all creatures that entered the battlefield this turn.", undefined, "Instant"),
      parsed("Destroy all creatures that entered this turn.", "Force of Despair", "Instant"));
    assert.deepEqual(parsed("{T}: Put a +1/+1 counter on each creature that entered the battlefield under your control this turn.", undefined, "Legendary Creature — Bird Cleric")[0].effects[0].steps[0].on,
      { ref: "each", filter: { types: ["creature"], zoneChangeHistory: { ...entered, controllerAtMove: you } } });
    // Present control is a different restriction from control on entering.
    assert.deepEqual(parsed("{1}, {T}: Put a +1/+1 counter on each creature you control that entered this turn.", "Raucous Entertainer", "Creature — Human Bard")[0].effects[0].steps[0].on,
      { ref: "each", filter: { types: ["creature"], control: "you", zoneChangeHistory: entered } });
  });
  it("counts the creatures that died, with or without your control (Canopy Stalker, Body Count)", () => {
    assert.deepEqual(parsed("When this creature dies, you gain 1 life for each creature that died this turn.", "Canopy Stalker", "Creature — Cat")[0].effects, [{ steps: [
      { effect: "gain-life", who: you, amount: n(1), forEach: { types: ["creature"], zoneChangeHistory: died } }] }]);
    assert.deepEqual(parsed("Draw a card for each creature that died under your control this turn.", "Body Count", "Sorcery"), [{ kind: "spell", effects: [{ steps: [
      { effect: "draw", who: you, amount: n(1), forEach: { types: ["creature"], zoneChangeHistory: { ...died, controllerAtMove: you } } }] }] }]);
    assert.deepEqual(parsed("Create an X/X black Horror creature token, where X is the number of creatures that died this turn.", "Spoils of Blood", "Instant")[0].effects[0].bindings,
      [{ variable: "x", value: { amount: "count", of: { types: ["creature"], zoneChangeHistory: died } } }]);
  });
  it("names the stated graveyard as the destination of \"put there\" (Grim Return)", () => {
    const graveyard = { zone: "graveyard", owner: "any" };
    assert.deepEqual(parsed("Choose target creature card in a graveyard that was put there from the battlefield this turn. Put that card onto the battlefield under your control.", "Grim Return", "Instant")[0].effects[0], { steps: [{
      effect: "declare-target", what: { ref: "target", count: n(1), filter: { types: ["creature"], cls: "card", zone: graveyard,
        zoneChangeHistory: { event: "put-into-zone", to: graveyard, from: { zone: "battlefield" }, during: "this-turn" } } } }] });
    const yours = { zone: "graveyard", owner: "your" };
    assert.deepEqual(parsed("When Night Nurse enters, choose target permanent card in your graveyard that was put there from anywhere this turn. Return it to your hand.", "Night Nurse", "Legendary Creature — Human Hero")[0].effects[0].steps[0].what.filter,
      { cls: "card", permanent: true, zone: yours, zoneChangeHistory: { event: "put-into-zone", to: yours, from: "anywhere", during: "this-turn" } });
  });
  it("links deaths within one resolution to that instruction (Mogg Infestation)", () => {
    assert.deepEqual(parsed("Destroy all creatures target player controls. For each creature that died this way, that player creates two 1/1 red Goblin creature tokens.", "Mogg Infestation", "Sorcery")[0].effects[1].steps[0].each,
      { ref: "each", filter: { types: ["creature"], relation: { relation: "event-result", event: "die", scope: "this-way" } } });
  });
  it("rejects windows, origins and antecedents it cannot state", () => {
    fails("Destroy target nonblack creature that entered since your last turn ended.");
    fails("Destroy target creature that entered.");
    fails("Draw a card for each creature that died.");
    fails("Choose target creature card that was put there from the battlefield this turn.");
    fails("Draw X cards, where X is the number of cards that were put into your graveyard from your hand or library this turn.");
    fails("Draw a card for each creature you controlled that died this turn.");
  });
});
