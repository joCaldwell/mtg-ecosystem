import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, PlayerRef, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. CR 506.4, 608.2c/d, 613.4c, 701.68, 701.70:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };
const n = (value: number) => ({ amount: "fixed", value }) as const;
const it_: ObjectRef = { ref: "it" };
const self: ObjectRef = { ref: "this", noun: "creature" };
const target = (filter: object): ObjectRef => ({ ref: "target", filter, count: n(1) });
const targetCreature = target({ types: ["creature"] });
const targetPermanent = target({ cls: "permanent" });
const eot = { duration: "end-of-turn" } as const;
const pump = (what: ObjectRef, p: number, t: number): Effect => ({ effect: "pump", what,
  power: { sign: p < 0 ? -1 : 1, amount: n(Math.abs(p)) }, toughness: { sign: t < 0 ? -1 : 1, amount: n(Math.abs(t)) } });
const youDo = { condition: { condition: "you-do" }, conditionKind: "if" } as const;

function abilities(text: string, typeLine = "Instant"): Ability[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
/** Sentences of a one-line instant. */
function spell(text: string): Sentence[] {
  const [ability] = abilities(text);
  assert.equal(ability.kind, "spell");
  return (ability as Extract<Ability, { kind: "spell" }>).effects;
}
function rejected(text: string, typeLine = "Instant"): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("choice between alternative instructions (CR 608.2d)", () => {
  it("offers one shared target to tap or to untap, under one optional decision", () => {
    // Twiddle-style wording; Psychic Puppetry.
    assert.deepEqual(spell("You may tap or untap target permanent."), [{ steps: [{
      effect: "choice", who: you, optional: true,
      options: [{ effect: "tap", what: targetPermanent }, { effect: "untap", what: targetPermanent }],
    }] }]);
    // A plain tap, a plain untap, and independent optional steps stay distinct.
    assert.deepEqual(spell("You may tap target permanent."), [{ steps: [{ effect: "tap", what: targetPermanent, optional: true }] }]);
    rejected("You may tap or untap target creature and target land.");
    rejected("You may tap or destroy target creature.");
  });

  it("keeps the shared subject and duration on both power/toughness alternatives", () => {
    // Brightling.
    assert.deepEqual(abilities("{1}: This creature gets +1/-1 or -1/+1 until end of turn.", "Creature — Shapeshifter"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1"] }],
      effects: [{ steps: [{ effect: "choice", who: you, duration: eot,
        options: [{ ...pump(self, 1, -1), duration: eot }, { ...pump(self, -1, 1), duration: eot }] }] }],
    }]);
  });

  it("coordinates complete instructions of one actor and keeps the following outcome test", () => {
    // Vision of Love.
    const card: ObjectRef = { ref: "filter", filter: { cls: "card" }, count: n(1) };
    assert.deepEqual(spell("You may sacrifice an artifact or discard a card. If you do, draw two cards."), [
      { steps: [{ effect: "choice", who: you, optional: true, options: [
        { effect: "sacrifice", who: you, what: { ref: "filter", filter: { types: ["artifact"] }, count: n(1) } },
        { effect: "discard", who: you, what: card }] }] },
      { steps: [{ effect: "draw", who: you, amount: n(2) }], ...youDo },
    ]);
    // "and" is two instructions under one decision, not a choice between them.
    assert.deepEqual(spell("You may sacrifice an artifact and discard a card."), [{ steps: [{ effect: "optional-group", who: you, steps: [
      { effect: "sacrifice", who: you, what: { ref: "filter", filter: { types: ["artifact"] }, count: n(1) } },
      { effect: "discard", who: you, what: card }] }] }]);
    // Different actors, or a trailing per-object rider of unclear scope.
    rejected("Sacrifice an artifact or target player discards a card.");
    rejected("Sacrifice an artifact or draw a card for each creature you control.");
    rejected("Draw a card or target creature gains flying until end of turn.");
  });

  it("creates one of the listed tokens, or all of them at once", () => {
    const food: Effect = { effect: "create-token", count: n(1), token: { predefined: "Food" } };
    const treasure: Effect = { effect: "create-token", count: n(1), token: { predefined: "Treasure" } };
    // Ant-Man's Army.
    assert.deepEqual(spell("Create a Food token or a Treasure token."), [{ steps: [{ effect: "choice", who: you, options: [food, treasure] }] }]);
    assert.deepEqual(spell("Create a Food token and a Treasure token."), [{ steps: [{ effect: "simultaneous", steps: [food, treasure] }] }]);
    const creature = (subtype: string, size: number): Effect => ({ effect: "create-token", count: n(1), token: {
      colors: ["green"], supertypes: [], types: ["creature"], subtypes: [subtype], keywords: [], power: n(size), toughness: n(size) } });
    // Bestial Menace.
    assert.deepEqual(spell("Create a 1/1 green Snake creature token, a 2/2 green Wolf creature token, and a 3/3 green Elephant creature token."),
      [{ steps: [{ effect: "simultaneous", steps: [creature("Snake", 1), creature("Wolf", 2), creature("Elephant", 3)] }] }]);
    rejected("Create a Food token, a Treasure token.");
    rejected("Create a Food token and a Treasure token or a Clue token.");
  });
});

describe("combat removal, counter removal and keyword actions", () => {
  it("removes from combat without tapping, and shares one optional decision with the untap", () => {
    // Gustcloak Runner.
    assert.deepEqual(abilities("Whenever this creature becomes blocked, you may untap it and remove it from combat.", "Creature — Human Soldier"), [{
      kind: "triggered", trigger: { trigger: "becomes-blocked", what: self },
      effects: [{ steps: [{ effect: "optional-group", who: you, steps: [{ effect: "untap", what: it_ }, { effect: "remove-from-combat", what: it_ }] }] }],
    }]);
    // Hollowhenge Spirit.
    assert.deepEqual(spell("Remove target attacking or blocking creature from combat."), [{ steps: [{ effect: "remove-from-combat",
      what: target({ allOf: [{ anyOf: [{ status: ["attacking"] }, { status: ["blocking"] }] }], types: ["creature"] }) }] }]);
    rejected("Remove target creature from the battlefield.");
    rejected("Remove from combat.");
  });

  it("distinguishes any-kind, up-to and all-counter removal from a named kind", () => {
    // Thrull Parasite / Render Inert / Vampire Hexmage / Gremlin Mine.
    assert.deepEqual(spell("Remove a counter from target permanent."), [{ steps: [{ effect: "remove-counters", counter: "any", count: n(1), from: targetPermanent }] }]);
    assert.deepEqual(spell("Remove up to five counters from target permanent."), [{ steps: [{ effect: "remove-counters", counter: "any", count: n(5), from: targetPermanent, upTo: true }] }]);
    assert.deepEqual(spell("Remove all counters from target permanent."), [{ steps: [{ effect: "remove-counters", counter: "any", count: "all", from: targetPermanent }] }]);
    assert.deepEqual(spell("Remove up to four charge counters from target permanent."), [{ steps: [{ effect: "remove-counters", counter: { type: "charge" }, count: n(4), from: targetPermanent, upTo: true }] }]);
    assert.deepEqual(spell("Remove two charge counters from target permanent."), [{ steps: [{ effect: "remove-counters", counter: { type: "charge" }, count: n(2), from: targetPermanent }] }]);
    rejected("Remove up to all counters from target permanent.");
    rejected("Remove a counter from.");
  });

  it("represents blight with its actor and amount, and recruit as a closed keyword action", () => {
    // Blighted Blackthorn.
    assert.deepEqual(spell("You may blight 2. If you do, you draw a card and lose 1 life."), [
      { steps: [{ effect: "blight", who: you, amount: n(2), optional: true }] },
      { steps: [{ effect: "draw", who: you, amount: n(1) }, { effect: "lose-life", who: you, amount: n(1) }], ...youDo },
    ]);
    // High Perfect Morcant.
    assert.deepEqual(spell("Each opponent blights 1."), [{ steps: [{ effect: "blight", who: { player: "each-opponent" }, amount: n(1) }] }]);
    // Esgaroth Garrison.
    assert.deepEqual(spell("Recruit."), [{ steps: [{ effect: "keyword-action", action: "recruit", who: you }] }]);
    rejected("Blight.");
    rejected("Recruit 2.");
  });
});

describe("coordinated subjects and riders", () => {
  it("keeps the stated subject as the source of a coordinated damage verb", () => {
    // Electric Eel.
    assert.deepEqual(abilities("{R}{R}: This creature gets +2/+0 until end of turn and deals 1 damage to you.", "Creature — Fish"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["R", "R"] }],
      effects: [{ steps: [{ ...pump(self, 2, 0), duration: eot },
        { effect: "damage", source: self, amount: n(1), to: { to: "player", ref: you } }] }],
    }]);
    rejected("Draw a card and deals 1 damage to you.");
  });

  it("starts a new instruction at a pronoun subject instead of extending the object list", () => {
    // Kashi-Tribe Warriors.
    const that: ObjectRef = { ref: "that", filter: { types: ["creature"] } };
    assert.deepEqual(spell("Tap that creature and it doesn't untap during its controller's next untap step."), [{ steps: [
      { effect: "tap", what: that },
      { effect: "untap-step-suppression", what: it_, step: { occurrence: "next", owner: "each-affected-objects-controller" } }] }]);
    // An object list is still one instruction.
    assert.deepEqual(spell("Tap that creature and target land."), [{ steps: [{ effect: "simultaneous", steps: [
      { effect: "tap", what: that }, { effect: "tap", what: target({ types: ["land"] }) }] }] }]);
  });

  it("folds an in-sentence regeneration rider into the destruction it modifies", () => {
    // Knight of the Mists.
    assert.deepEqual(spell("Destroy target creature and it can't be regenerated."), [{ steps: [{ effect: "destroy", what: targetCreature, regenerationForbidden: true }] }]);
    rejected("Tap target creature and it can't be regenerated.");
    rejected("Destroy target creature and it can't be regenerated this turn.");
  });

  it("treats additional draws and modifiers as ordinary cumulative instructions", () => {
    // Howling Mine's body; Desert's Due.
    assert.deepEqual(spell("Target player draws an additional card."), spell("Target player draws a card."));
    assert.deepEqual(spell("Target creature gets -2/-2 until end of turn. It gets an additional -1/-1 until end of turn for each Desert you control."), [
      { steps: [{ ...pump(targetCreature, -2, -2), duration: eot }] },
      { steps: [{ ...pump(it_, -1, -1), duration: eot, forEach: { subtypes: ["Desert"], control: "you" } }] },
    ]);
    rejected("Target creature gains an additional flying until end of turn.");
  });

  it("binds X defined after a trailing unless-payment, and accepts a comma after Then", () => {
    // Rethink.
    assert.deepEqual(spell("Counter target spell unless its controller pays {X}, where X is its mana value."), [{
      steps: [{ effect: "counter", what: target({ cls: "spell" }) }],
      condition: { condition: "pays", who: { player: "controller", of: it_ }, cost: [{ cost: "mana", symbols: ["X"] }] },
      conditionKind: "unless",
      bindings: [{ variable: "x", value: { amount: "attribute", of: it_, attribute: "mana-value" } }],
    }]);
    rejected("Counter target spell unless its controller pays {X}, where X is.");
    // Avacyn and Griselbrand.
    assert.deepEqual(spell("Draw a card. Then, draw a card."), spell("Draw a card. Then draw a card."));
    rejected("Draw a card. Then,");
  });
});
