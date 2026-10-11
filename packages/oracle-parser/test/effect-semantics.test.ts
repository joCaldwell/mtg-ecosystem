import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

const n = (value: number) => ({ amount: "fixed", value });
const you = { player: "you" };
function one(text: string): Ability {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0])) as Ability;
}
function steps(text: string): Effect[] {
  const ability = one(text);
  assert.equal(ability.kind, "spell");
  return ability.effects[0].steps;
}

describe("effect meaning and scope", () => {
  it("keeps predefined tokens distinct from explicit tokens with that subtype", () => {
    // CR 111.10; Brackish Blunder 4ad34f85-a185-458a-a7a1-eb60f7efce5b.
    assert.deepEqual(steps("Create a Map token."), [{ effect: "create-token", count: n(1), token: { predefined: "Map" } }]);
    assert.deepEqual(steps("Create a colorless Treasure artifact token."), [{
      effect: "create-token", count: n(1), token: { colors: [], supertypes: [], types: ["artifact"], subtypes: ["Treasure"], keywords: [] },
    }]);
    assert.equal(parseOracleText("Create a Madeup token.").ok, false);
  });
  it("retains quoted token abilities", () => {
    assert.deepEqual(steps('Create a 1/1 colorless Eldrazi Scion creature token with "Sacrifice this token: Add {C}.".'), [{
      effect: "create-token", count: n(1), token: {
        power: n(1), toughness: n(1), colors: [], supertypes: [], types: ["creature"], subtypes: ["Eldrazi", "Scion"], keywords: [],
        abilities: [{ gained: "quoted", ability: { kind: "activated", costs: [{ cost: "sacrifice", what: { ref: "this", noun: "token" } }], effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["C"] } }] }] } }],
      },
    }]);
  });
  it("retains permanent-card qualification and previous object state", () => {
    // Lion Sash 50330e48-db74-4c5f-a0bc-f9a8607e8f31.
    assert.deepEqual(one("If it was a permanent card, draw a card."), { kind: "spell", effects: [{
      condition: { condition: "is-filter", what: { ref: "it" }, filter: { cls: "card", permanent: true }, time: "previous" },
      conditionKind: "if", steps: [{ effect: "draw", who: you, amount: n(1) }],
    }] });
    assert.notDeepEqual(one("If it was tapped, draw a card."), one("If it is tapped, draw a card."));
  });
  it("preserves one optional choice across coordinated actions", () => {
    const draw = { effect: "draw", who: you, amount: n(1) };
    const discard = { effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: n(1) } };
    assert.deepEqual(steps("You may draw a card and discard a card."), [{ effect: "optional-group", who: you, steps: [draw, discard] }]);
    assert.deepEqual(steps("You may draw a card and you may discard a card."), [{ ...draw, optional: true }, { ...discard, optional: true }]);
  });
  it("distinguishes simultaneous object instructions from sequential ones", () => {
    const destroy = (type: string) => ({ effect: "destroy", what: { ref: "target", filter: { types: [type] }, count: n(1) } });
    assert.deepEqual(steps("Destroy target artifact and target creature."), [{ effect: "simultaneous", steps: [destroy("artifact"), destroy("creature")] }]);
    assert.deepEqual(steps("Destroy target artifact, then destroy target creature."), [destroy("artifact"), destroy("creature")]);
  });
  it("keeps unspecified characteristics unchanged when objects become something", () => {
    // Indigo Faerie 10ae611f-e56d-47c5-8b6e-d38d95208bba.
    assert.deepEqual(steps("Target permanent becomes blue in addition to its other colors until end of turn."), [{
      effect: "become", what: { ref: "target", filter: { cls: "permanent" }, count: n(1) },
      spec: { colors: ["blue"], inAddition: "colors" }, duration: { duration: "end-of-turn" },
    }]);
    assert.equal(parseOracleText("Target permanent becomes blue in addition to its other bananas.").ok, false);
    assert.deepEqual(steps("It becomes a Mutant in addition to its other types."), [{ effect: "become", what: { ref: "it" }, spec: { subtypes: ["Mutant"], inAddition: "types" } }]);
  });
  it("stores explicit variable definitions rather than accepting unbound trailing text", () => {
    assert.deepEqual(one("Draw X cards, where X is that creature's power."), { kind: "spell", effects: [{
      steps: [{ effect: "draw", who: you, amount: { amount: "x" } }],
      bindings: [{ variable: "x", value: { amount: "attribute", of: { ref: "that", filter: { types: ["creature"] } }, attribute: "power" } }],
    }] });
    assert.equal(parseOracleText("Draw X cards, where X is arbitrary prose.").ok, false);
  });
  it("keeps chapter symbols and rules-bearing prefixes out of cosmetic words", () => {
    const effects = [{ steps: [{ effect: "draw", who: you, amount: n(1) }] }];
    assert.deepEqual(one("I, II — Draw a card."), { kind: "chapter", chapters: [1, 2], effects });
    assert.deepEqual(one("Max speed — Flying"), { kind: "max-speed", ability: { kind: "keywords", keywords: [{ keyword: "flying" }] } });
    assert.deepEqual(one("Boast — {1}: Draw a card."), { kind: "activated", keyword: "boast", costs: [{ cost: "mana", symbols: ["1"] }], effects });
    assert.deepEqual(one("Ward—Sacrifice a creature."), { kind: "keywords", keywords: [{ keyword: "ward", cost: [{ cost: "sacrifice", what: { ref: "filter", count: n(1), filter: { types: ["creature"] } } }] }] });
  });
  it("keeps singular other objects and typed keyword actions", () => {
    assert.deepEqual(steps("Sacrifice another creature."), [{ effect: "sacrifice", who: you, what: { ref: "filter", count: n(1), filter: { types: ["creature"], other: true } } }]);
    assert.deepEqual(steps("Amass Zombies 2."), [{ effect: "amass", subtype: "Zombie", amount: n(2) }]);
    assert.deepEqual(steps("Investigate."), [{ effect: "keyword-action", action: "investigate", who: you }]);
    assert.equal(parseOracleText("Amass Madeups 2.").ok, false);
  });
  it("retains a player's elided actions and resets the actor at sentence boundaries", () => {
    const opponent = { player: "target-opponent" };
    assert.deepEqual(steps("Target opponent draws a card and loses 2 life."), [
      { effect: "draw", who: opponent, amount: n(1) },
      { effect: "lose-life", who: opponent, amount: n(2) },
    ]);
    assert.deepEqual(one("Target opponent draws a card. Draw a card."), { kind: "spell", effects: [
      { steps: [{ effect: "draw", who: opponent, amount: n(1) }] },
      { steps: [{ effect: "draw", who: you, amount: n(1) }] },
    ] });
    assert.equal(parseOracleText("Draws a card.").ok, false);
  });
  it("preserves library position, owner, ordering, and concrete source zones", () => {
    assert.deepEqual(steps("Exile the top two cards of your library."), [{ effect: "exile", what: {
      ref: "library-cards", position: "top", count: n(2), of: { zone: "library", owner: "your" },
    } }]);
    assert.deepEqual(steps("Put those cards on the bottom of your library in a random order."), [{ effect: "move-zone",
      what: { ref: "that", filter: { cls: "card" }, plural: true }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random",
    }]);
    assert.deepEqual(steps("Return this card from your graveyard to your hand."), [{ effect: "return-to-hand", what: { ref: "this", noun: "card" }, to: "your", from: { zone: "graveyard", owner: "your" } }]);
    assert.equal(parseOracleText("Put it into your graveyard tapped.").ok, false);
  });
  it("rejects overwritten or unattached conditional branches", () => {
    assert.equal(parseOracleText("Draw a card. Otherwise, gain 2 life.").ok, false);
    assert.equal(parseOracleText("If you control a creature, draw a card. Otherwise, gain 2 life. Otherwise, gain 3 life.").ok, false);
    assert.equal(parseOracleText("Draw a card, then gain 2 life unless an opponent pays {2}.").ok, false);
    const ability = one("If you control a creature, draw a card unless an opponent pays {2}.");
    assert.equal(ability.kind, "spell");
    assert.deepEqual(ability.effects[0].condition, { condition: "all-of", conditions: [
      { condition: "control", who: you, what: { ref: "filter", count: n(1), filter: { types: ["creature"] } } },
      { condition: "not", of: { condition: "pays", who: { player: "an-opponent" }, cost: [{ cost: "mana", symbols: ["2"] }] } },
    ] });
  });
  it("preserves copy exceptions as copiable characteristics", () => {
    // Saheeli's Artistry 0038e74a-b950-47ff-8853-dba6a3b31091.
    const copyOf = { ref: "target", count: n(1), filter: { types: ["creature"] } };
    assert.deepEqual(steps("Create a token that's a copy of target creature, except it's an artifact in addition to its other types."), [{
      effect: "create-token", count: n(1), token: { copyOf, modifications: [{ modification: "characteristics", spec: { types: ["artifact"], inAddition: "types" } }] },
    }]);
    assert.deepEqual(steps("Create a token that's a copy of target creature, except it's not legendary."), [{
      effect: "create-token", count: n(1), token: { copyOf, modifications: [{ modification: "not-legendary" }] },
    }]);
    assert.deepEqual(steps("Create a token that's a copy of target creature, except the token is 1/1."), [{
      effect: "create-token", count: n(1), token: { copyOf, modifications: [{ modification: "characteristics", spec: { power: n(1), toughness: n(1) } }] },
    }]);
    assert.equal(parseOracleText("Create a token that's a copy of target creature, except it's special.").ok, false);
  });
  it("preserves mode labels and permission to repeat modes", () => {
    assert.deepEqual(steps("Choose one —\n• Cure Wounds — You gain 2 life.\n• Dispel Magic — Destroy target enchantment."), [{
      effect: "modal", count: { exactly: 1 }, options: [
        { label: "Cure Wounds", effects: [{ steps: [{ effect: "gain-life", who: you, amount: n(2) }] }] },
        { label: "Dispel Magic", effects: [{ steps: [{ effect: "destroy", what: { ref: "target", count: n(1), filter: { types: ["enchantment"] } } }] }] },
      ],
    }]);
    const option = { effects: [{ steps: [{ effect: "draw", who: you, amount: n(1) }] }] };
    assert.deepEqual(steps("Choose three. You may choose the same mode more than once.\n• Draw a card."), [{ effect: "modal", count: { exactly: 3 }, options: [option], allowRepeat: true }]);
    assert.equal(parseOracleText("Choose three —\n• Draw a card.").ok, false);
    assert.deepEqual(steps("Choose X. You may choose the same mode more than once.\n• Draw a card."), [{ effect: "modal", count: { variable: "x" }, options: [option], allowRepeat: true }]);
  });
});
