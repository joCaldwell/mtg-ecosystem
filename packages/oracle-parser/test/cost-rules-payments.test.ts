import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

const you = { player: "you" as const };
function abilities(text: string, typeLine?: string): Ability[] {
  const parsed = parseOracleText(text, undefined, typeLine ? { typeLine } : {});
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function rejects(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : {}).ok, false, text);
}

// Scryfall: Intrepid Adversary, Tranquil Frillback, Primal Adversary. CR 603.12 (reflexive triggers).
describe("repeated optional payments", () => {
  it("pays any number of times and counts the payments in the reflexive trigger", () => {
    assert.deepEqual(abilities("When this creature enters, you may pay {1}{W} any number of times. When you pay this cost one or more times, put that many valor counters on this creature.", "Creature — Human"), [{
      kind: "triggered", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      effects: [
        { steps: [{ effect: "pay-cost", who: you, costs: [{ cost: "mana", symbols: ["1", "W"] }], times: "any-number", optional: true }] },
        { steps: [{ effect: "create-reflexive-trigger", event: { event: "repeated-payment", who: you }, effects: [
          { steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "valor" }, count: { amount: "that-much" } }] }] }] },
      ] }]);
  });

  it("limits the number of payments and lets the number choose modes", () => {
    const [ability] = abilities("When this creature enters, you may pay {G} up to three times. When you pay this cost one or more times, choose up to that many —\n• Destroy target artifact or enchantment.\n• You gain 4 life.", "Creature — Dinosaur");
    assert.deepEqual(JSON.parse(JSON.stringify(ability)).effects[0], { steps: [{ effect: "pay-cost", who: you, costs: [{ cost: "mana", symbols: ["G"] }],
      times: { upTo: { amount: "fixed", value: 3 } }, optional: true }] });
    const reflexive = (ability as { effects: { steps: { effect: string; event: unknown; effects: { steps: { count: unknown }[] }[] }[] }[] }).effects[1].steps[0];
    assert.deepEqual(reflexive.event, { event: "repeated-payment", who: you });
    assert.deepEqual(reflexive.effects[0].steps[0].count, { variable: "that-many", upTo: true });
  });

  it("selects up to that many targets of the reflexive trigger", () => {
    const [ability] = abilities("When this creature enters, you may pay {1}{U} any number of times. When you pay this cost one or more times, put that many +1/+1 counters on this creature, then up to that many other target artifacts, creatures, and/or enchantments phase out.", "Creature — Spirit");
    const steps = (ability as { effects: { steps: { effects?: { steps: unknown[] }[] }[] }[] }).effects[1].steps[0].effects![0].steps;
    assert.deepEqual(steps[1], { effect: "phase-out", what: { ref: "target", count: { amount: "that-much" }, upTo: true,
      filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] }] }], other: true } } });
  });

  it("keeps 'that many' targets without 'up to' and an unstated repetition failing", () => {
    rejects("When this creature enters, exile that many target creatures.", "Creature — Elf");
    rejects("When this creature enters, you may pay {1}{W} any number of times. When you pay this cost, put that many valor counters on this creature.", "Creature — Human");
  });
});

// Scryfall: Defiler of Vigor, Defiler of Dreams. CR 601.2b, 601.2f, 118.7b.
describe("optional additional cost granted to spells, with a conditional reduction", () => {
  const spells = { ref: "each" as const, filter: { colors: ["green"], cls: "spell", permanent: true } };
  const scope = { action: "cast", spells, caster: you };
  const life = [{ cost: "pay-life", amount: { amount: "fixed", value: 2 } }];

  it("offers the cost to the caster and reduces only that color when it was paid", () => {
    assert.deepEqual(abilities("As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to cast if you paid life this way. This effect reduces only the amount of green mana you pay.", "Creature — Phyrexian Elk"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "spell-additional-cost", scope, costs: life, optional: true },
        { static: "conditional", condition: { condition: "additional-cost-paid", cost: life },
          effect: { static: "cost-modification", scope, change: { direction: "reduce", symbols: ["G"] }, onlyColor: "green" } },
      ] } }]);
  });

  it("rejects a reduction whose color differs from the restricted color or the reduced symbol", () => {
    rejects("As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to cast if you paid life this way. This effect reduces only the amount of blue mana you pay.", "Creature — Elk");
    rejects("As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to cast if you paid life this way.", "Creature — Elk");
    rejects("As an additional cost to cast green permanent spells, you may pay 2 life. Those spells cost {G} less to cast.", "Creature — Elk");
  });
});

// Scryfall: Visions of Glory. CR 702.34a, 118.9, 601.2f, 903.3.
describe("flashback cost reduction", () => {
  it("adjusts only a cast made with flashback", () => {
    assert.deepEqual(abilities("Flashback {8}{W}{W}. This spell costs {X} less to cast this way, where X is the greatest mana value of a commander you own on the battlefield or in the command zone.", "Sorcery"), [{
      kind: "keywords", keywords: [{ keyword: "flashback", cost: [{ cost: "mana", symbols: ["8", "W", "W"] }],
        costModifications: [{ change: { direction: "reduce", symbols: ["X"] }, bindings: [{ variable: "x", value: {
          amount: "attribute-extremum", extremum: "maximum", attribute: "mana-value", among: { commander: true, own: "you",
            allOf: [{ anyOf: [{ zone: { zone: "battlefield" } }, { zone: { zone: "command" } }] }] } } }] }] }] }]);
  });

  it("requires 'this way' and a keyword that supplies its own cost", () => {
    rejects("Flashback {8}{W}{W}. This spell costs {X} less to cast, where X is the greatest mana value of a commander you own on the battlefield or in the command zone.", "Sorcery");
    rejects("Flying. This spell costs {1} less to cast this way.", "Creature — Bird");
  });
});

describe("zone alternatives in a filter", () => {
  it("joins two zones with 'or' as membership of either", () => {
    assert.deepEqual(abilities("Draw a card for each commander you own on the battlefield or in the command zone.", "Sorcery"), [{
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 },
        forEach: { commander: true, own: "you", allOf: [{ anyOf: [{ zone: { zone: "battlefield" } }, { zone: { zone: "command" } }] }] } }] }] }]);
  });
});

// Scryfall: Clockwork Beast. CR 122.1, 602.5b.
describe("counter total limit on a chosen number of counters", () => {
  it("caps the total number of counters an 'up to X' ability can put", () => {
    assert.deepEqual(abilities("{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than seven. Activate only during your upkeep.", "Artifact Creature — Beast"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["X"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+0" }, count: { amount: "x" },
        upTo: true, maxTotal: { amount: "fixed", value: 7 } }] }],
      restriction: { restriction: "timing", turn: "your", phase: "upkeep" } }]);
  });

  it("keeps 'up to' counters without a limit and rejects a limit on another counter or object", () => {
    assert.deepEqual(abilities("{T}: Put up to two +1/+1 counters on this creature.", "Creature — Elf"), [{
      kind: "activated", costs: [{ cost: "tap-self" }],
      effects: [{ steps: [{ effect: "put-counters", on: { ref: "this", noun: "creature" }, counter: { type: "+1/+1" }, count: { amount: "fixed", value: 2 }, upTo: true }] }] }]);
    rejects("{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+1 counters on this creature to be greater than four.", "Creature — Elf");
    rejects("{X}, {T}: Put up to X +1/+0 counters on target creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than four.", "Creature — Elf");
    rejects("{T}: Draw a card. This ability can't cause the total number of +1/+0 counters on this creature to be greater than four.", "Creature — Elf");
  });
});

// Scryfall: Clockwork Servant, Henge Walker. CR 207.2c (adamant), 609.4b.
describe("adamant: mana of the same color", () => {
  it("tests at least three mana of one color spent to cast it", () => {
    assert.deepEqual(abilities("Adamant — When this creature enters, if at least three mana of the same color was spent to cast it, draw a card.", "Artifact Creature — Gnome"), [{
      kind: "triggered", abilityWord: "Adamant", trigger: { trigger: "enters", what: { ref: "this", noun: "creature" } },
      condition: { condition: "mana-spent-to-cast", what: { ref: "it" }, mana: "same-color", comparison: { op: "ge", value: { amount: "fixed", value: 3 } } },
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] }] }]);
  });
});

// Scryfall: Elite Spellbinder, Soul Partition, Urianger Augurelt. CR 601.2f, 701.18.
describe("cost change of spells cast with a play permission", () => {
  const permission = { who: { player: "owner", of: { ref: "it" } }, action: "play", what: { ref: "it" } };
  const duration = { duration: "as-long-as", condition: { condition: "remains", what: { ref: "that", filter: { cls: "card" } }, zone: { zone: "exile" } } };

  it("raises the cost of a spell cast this way, whoever casts it", () => {
    const [ability] = abilities("Exile target nonland permanent. For as long as that card remains exiled, its owner may play it. A spell cast this way costs {2} more to cast.", "Instant");
    assert.deepEqual((ability as { effects: unknown[] }).effects[1], { steps: [{ effect: "grant-play-permission", duration,
      permission: { ...permission, riders: [{ rider: "cost-change", change: { direction: "increase", symbols: ["2"] } }] } }] });
  });

  it("keeps the caster restriction of 'cast by an opponent this way'", () => {
    const [ability] = abilities("Exile target nonland permanent. For as long as that card remains exiled, its owner may play it. A spell cast by an opponent this way costs {2} more to cast.", "Instant");
    assert.deepEqual((ability as { effects: unknown[] }).effects[1], { steps: [{ effect: "grant-play-permission", duration,
      permission: { ...permission, riders: [{ rider: "cost-change", change: { direction: "increase", symbols: ["2"] }, caster: { player: "an-opponent" } }] } }] });
  });

  it("reduces the cost of spells the player casts with an activated permission", () => {
    assert.deepEqual(abilities("{T}: Until end of turn, you may play cards exiled with ~. Spells you cast this way cost {2} less to cast.", "Legendary Creature — Elf"), [{
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "grant-play-permission", duration: { duration: "end-of-turn" },
        permission: { who: you, action: "play", what: { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source: { ref: "self" } } },
          riders: [{ rider: "cost-change", change: { direction: "reduce", symbols: ["2"] }, caster: you }] } }] }] }]);
  });

  it("does not accept a cost change that names another way of casting", () => {
    rejects("Exile target nonland permanent. For as long as that card remains exiled, its owner may play it. A spell cast this turn costs {2} more to cast.", "Instant");
    rejects("Exile target nonland permanent. A spell cast this way costs {2} more to cast.", "Instant");
  });
});

// Scryfall: Dark Knight's Greatsword, Shredder's Armor. CR 602.5b, 702.6a.
describe("equip with a non-mana cost and an activation restriction", () => {
  it("keeps the restriction after a dash cost", () => {
    assert.deepEqual(abilities("Equip—Pay 3 life. Activate only once each turn.", "Artifact — Equipment"), [{
      kind: "keywords", keywords: [{ keyword: "equip", cost: [{ cost: "pay-life", amount: { amount: "fixed", value: 3 } }], restriction: { restriction: "once-each-turn" } }] }]);
  });

  it("still ends a plain dash cost at the period and rejects an unknown second sentence", () => {
    assert.deepEqual(abilities("Equip—Pay 3 life.", "Artifact — Equipment"), [{
      kind: "keywords", keywords: [{ keyword: "equip", cost: [{ cost: "pay-life", amount: { amount: "fixed", value: 3 } }] }] }]);
    rejects("Equip—Pay 3 life. Draw a card.", "Artifact — Equipment");
  });
});

// Scryfall: Minas Tirith, Ojer Taq, Windbrisk Heights, Deepway Navigator. CR 508.1a.
describe("attacked with a number of creatures", () => {
  const count = { op: "ge", value: { amount: "fixed", value: 2 } };
  it("counts the creatures attacked with in an activation restriction", () => {
    assert.deepEqual(abilities("{1}{W}, {T}: Draw a card. Activate only if you attacked with two or more creatures this turn.", "Land"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1", "W"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] }],
      restriction: { restriction: "condition", condition: { condition: "attacked", who: you, during: "this-turn", with: { types: ["creature"] }, count } } }]);
  });

  it("applies to a creature type and rejects an article or negation with a count", () => {
    assert.deepEqual(abilities("As long as you attacked with two or more Merfolk this turn, Merfolk you control get +1/+0.", "Creature — Merfolk"), [{
      kind: "static", effect: { static: "conditional", condition: { condition: "attacked", who: you, during: "this-turn", with: { subtypes: ["Merfolk"] }, count },
        effect: { static: "modify", what: { ref: "each", filter: { subtypes: ["Merfolk"], control: "you" } }, modifiers: [{ modifier: "pt",
          power: { sign: 1, amount: { amount: "fixed", value: 1 } }, toughness: { sign: 1, amount: { amount: "fixed", value: 0 } } }] } } }]);
    rejects("{1}{W}, {T}: Draw a card. Activate only if you attacked with a or more creatures this turn.", "Land");
    rejects("{1}{W}, {T}: Draw a card. Activate only if you didn't attack with two or more creatures this turn.", "Land");
  });
});

// Scryfall: Endless Atlas, Synchronized Eviction. CR 201.2a, 205.3m, 608.2c.
describe("control a number of objects sharing a characteristic value", () => {
  it("requires three lands with the same name", () => {
    assert.deepEqual(abilities("{2}, {T}: Draw a card. Activate only if you control three or more lands with the same name.", "Artifact"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] }],
      restriction: { restriction: "condition", condition: { condition: "controls-subset", who: you, of: { types: ["land"] },
        minimum: { amount: "fixed", value: 3 }, groupConstraints: [{ constraint: "shared-value", characteristic: "name" }] } } }]);
  });

  it("requires two creatures that share a creature type", () => {
    assert.deepEqual(abilities("This spell costs {2} less to cast if you control at least two creatures that share a creature type.", "Instant"), [{
      kind: "static", effect: { static: "conditional", condition: { condition: "controls-subset", who: you, of: { types: ["creature"] },
        minimum: { amount: "fixed", value: 2 }, groupConstraints: [{ constraint: "shared-value", characteristic: "creature-type" }] },
        effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } }, change: { direction: "reduce", symbols: ["2"] } } } }]);
  });

  it("keeps bare exact counts and targeted selections out of the subset condition", () => {
    rejects("Destroy target creature with the same name.", "Instant");
    rejects("{2}, {T}: Draw a card. Activate only if you control three lands with the same name.", "Artifact");
  });
});

// Scryfall: Serra Bestiary, Katabatic Winds. CR 107.5, 602.1.
describe("activated abilities with {T} in their costs can't be activated", () => {
  it("restricts the enchanted creature's abilities that have the tap symbol", () => {
    assert.deepEqual(abilities("Enchanted creature can't attack or block, and its activated abilities with {T} in their costs can't be activated.", "Enchantment — Aura"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "cant", what: { ref: "enchanted", noun: "creature" }, action: "attack-or-block" },
        { static: "prohibit", prohibition: { prohibition: "activate", abilities: { ref: "each", filter: {
          cls: "ability", abilityKind: "activated", abilitySource: { ref: "enchanted", noun: "creature" }, tapInCost: true } } } },
      ] } }]);
  });

  it("does not accept another symbol or a different cost location", () => {
    rejects("Enchanted creature can't attack or block, and its activated abilities with {Q} in their costs can't be activated.", "Enchantment — Aura");
    rejects("Enchanted creature can't attack or block, and its activated abilities with {T} in their text can't be activated.", "Enchantment — Aura");
  });
});

// Scryfall: Tornado, Skeleton Scavengers. CR 118.1, 122.1.
describe("scaled payments", () => {
  it("pays life once for each counter", () => {
    assert.deepEqual(abilities("{2}{G}, Pay 3 life for each velocity counter on this enchantment: Draw a card.", "Enchantment"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2", "G"] },
        { cost: "pay-life", amount: { amount: "fixed", value: 3 }, multiplier: { amount: "counter-count", counter: { type: "velocity" }, on: { ref: "this", noun: "enchantment" } } }],
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] }] }]);
  });

  it("pays mana once for each counter and rejects a count of targets", () => {
    assert.deepEqual(abilities("Pay {1} for each +1/+1 counter on this creature: Draw a card.", "Creature — Skeleton"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1"], multiplier: { amount: "counter-count", counter: { type: "+1/+1" }, on: { ref: "this", noun: "creature" } } }],
      effects: [{ steps: [{ effect: "draw", who: you, amount: { amount: "fixed", value: 1 } }] }] }]);
    rejects("Pay {1} for each target: Draw a card.", "Creature — Skeleton");
  });
});
