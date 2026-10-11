import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { parseExileUntil } from "../src/parser/linked-exile.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const target: ObjectRef = { ref: "target", filter: { types: ["creature"] }, count: fixed(1) };
const source: ObjectRef = { ref: "this", noun: "enchantment" };
function linked(what: ObjectRef[], eventObject: ObjectRef = source): Effect {
  return { effect: "exile-until", what, until: { event: "leaves-battlefield", what: eventObject },
    return: { to: "previous-zone", battlefieldController: "owner" } };
}
function only(text: string): Ability {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return parsed.abilities[0];
}

// Banishing Light, Banisher Priest, Chained to the Rocks, and Calix's Oracle
// wording checked in cached Scryfall data. CR 610.3 defines the linked return.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("linked exile one-shot effects", () => {
  it("stores the return event and default owner control with the original move", () => {
    assert.deepEqual(only("When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield."), {
      kind: "triggered", abilityWord: undefined, condition: undefined,
      trigger: { trigger: "enters", what: source },
      effects: [{ steps: [linked([{ ref: "target", count: fixed(1),
        filter: { nonTypes: ["land"], cls: "permanent", control: "opponent" } }])] }],
    });
  });

  it("preserves a separate target whose departure causes the return", () => {
    const targetEnchantment: ObjectRef = { ref: "target", count: fixed(1),
      filter: { types: ["enchantment"], control: "you" } };
    assert.deepEqual(only("Exile target creature until target enchantment you control leaves the battlefield."), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [linked([target], targetEnchantment)] }],
    });
  });

  it("keeps the exile object list in one simultaneous linked move group", () => {
    const land: ObjectRef = { ref: "target", count: fixed(1), filter: { types: ["land"] } };
    assert.deepEqual(only("Exile target creature and target land until this enchantment leaves the battlefield."), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [linked([target, land])] }],
    });
  });

  it("keeps subsequent instructions outside the linked return", () => {
    assert.deepEqual(only("Exile target creature until this enchantment leaves the battlefield. You gain 2 life."), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [linked([target])] },
        { steps: [{ effect: "gain-life", who: { player: "you" }, amount: fixed(2) }] }],
    });
  });

  it("retains an explicit source zone for returning to that previous zone", () => {
    const from = { zone: "graveyard" as const, owner: "your" as const };
    const c = new Cursor(lex("until this enchantment leaves the battlefield."));
    assert.deepEqual(parseExileUntil(c, [target], from), { ...linked([target]), from });
    assert.equal(c.punct("."), true);
    assert.equal(c.done(), true);
  });

  it("does not invent linked returns for ordinary exile or separate triggers", () => {
    assert.deepEqual(only("Exile target creature."), {
      kind: "spell", abilityWord: undefined,
      effects: [{ steps: [{ effect: "exile", what: target }] }],
    });
    const parsed = parseOracleText("When this enchantment enters, exile target creature.\nWhen this enchantment leaves the battlefield, return the exiled card to the battlefield under its owner's control.");
    assert.equal(parsed.lines[0].ok, true);
    if (!parsed.lines[0].ok || parsed.lines[0].ability.kind !== "triggered") throw new Error("missing first trigger");
    assert.deepEqual(parsed.lines[0].ability.effects, [{ steps: [{ effect: "exile", what: target }] }]);
    assert.equal(parsed.lines.length, 2);
  });

  it("rejects unrepresented events instead of storing a meaningless duration", () => {
    for (const text of [
      "Exile target creature until end of turn.",
      "Until end of turn, exile target creature.",
      "Exile target creature until an opponent becomes the monarch.",
      "Exile target creature until this enchantment leaves the graveyard.",
      "Exile target creature until this enchantment leaves the battlefield or you lose the game.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });

  it("backtracks malformed suffixes and rejects empty move lists", () => {
    for (const text of ["until", "until this enchantment leaves", "until this enchantment leaves the graveyard"] ) {
      const c = new Cursor(lex(text));
      assert.equal(parseExileUntil(c, [target]), null, text);
      assert.equal(c.pos, 0, text);
    }
    const c = new Cursor(lex("until this enchantment leaves the battlefield"));
    assert.equal(parseExileUntil(c, []), null);
    assert.equal(c.pos, 0);
  });
});
