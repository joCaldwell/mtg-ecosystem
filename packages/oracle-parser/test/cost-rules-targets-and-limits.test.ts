import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
function abilities(text: string, typeLine?: string): Ability[] {
  const parsed = parseOracleText(text, undefined, typeLine ? { typeLine } : {});
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  return parsed.abilities;
}
function rejects(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : {}).ok, false, text);
}
void fixed;

// Scryfall: Aerial Formation (Strive), Fireball, Hinata, Dawn-Crowned.
// CR 601.2c fixes the number of targets before 601.2f computes the total cost.
describe("cost adjustments counted per target", () => {
  const thisSpell = { ref: "this" as const, noun: "spell" };
  it("counts this spell's targets beyond the first", () => {
    assert.deepEqual(abilities("Strive — This spell costs {2}{U} more to cast for each target beyond the first.", "Instant"), [{
      kind: "static", abilityWord: "Strive", effect: { static: "cost-modification", scope: { action: "cast", spells: thisSpell },
        change: { direction: "increase", symbols: ["2", "U"] },
        multiplier: { amount: "target-count", of: thisSpell, beyond: 1 } },
    }]);
  });

  it("counts every target of each affected spell for a group adjustment", () => {
    assert.deepEqual(abilities("Spells your opponents cast cost {1} more to cast for each target."), [{
      kind: "static", abilityWord: undefined, effect: { static: "cost-modification",
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "each-opponent" } },
        change: { direction: "increase", symbols: ["1"] },
        multiplier: { amount: "target-count", of: { ref: "affected-object" } } },
    }]);
  });

  it("keeps an activated ability's own targets with that ability", () => {
    assert.deepEqual(abilities("{2}: Draw a card. This ability costs {1} more to activate for each target beyond the first."), [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "mana", symbols: ["2"] }],
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
      costModifications: [{ change: { direction: "increase", symbols: ["1"] },
        multiplier: { amount: "target-count", of: { ref: "this", noun: "ability" }, beyond: 1 } }],
      restriction: undefined,
    }]);
  });

  it("rejects target counts it cannot state", () => {
    rejects("This spell costs {1} more to cast for each target beyond the second.");
    rejects("This spell costs {1} more to cast for each target creature.");
    rejects("This spell costs 3 life more to cast for each target.");
    rejects("This spell costs {1} more to cast for each target beyond the first and each Elf you control.");
  });
});

// Scryfall: Oteclan Landmark, Paleontologist's Pick-Axe, Sunbird Standard,
// Eye of Ojer Taq, Throne of the Grim Captain, Ore-Rich Stalactite.
// CR 702.167a-b: the materials are part of the craft ability's cost.
describe("craft materials", () => {
  const craft = (text: string) => abilities(text, "Artifact");
  const mana = (...symbols: string[]) => [{ cost: "mana" as const, symbols }];
  it("reads a bare description as one object and keeps stated counts", () => {
    assert.deepEqual(craft("Craft with artifact {2}{W}"), [{ kind: "keywords", keywords: [{ keyword: "craft",
      materials: [{ count: fixed(1), filter: { types: ["artifact"] } }], cost: mana("2", "W") }] }]);
    assert.deepEqual(craft("Craft with six artifacts {4}"), [{ kind: "keywords", keywords: [{ keyword: "craft",
      materials: [{ count: fixed(6), filter: { types: ["artifact"] } }], cost: mana("4") }] }]);
  });

  it("keeps a minimum distinct from an exact number, with or without a description", () => {
    assert.deepEqual(craft("Craft with one or more creatures {5}"), [{ kind: "keywords", keywords: [{ keyword: "craft",
      materials: [{ count: { amount: "chosen", minimum: 1 }, filter: { types: ["creature"] } }], cost: mana("5") }] }]);
    assert.deepEqual(craft("Craft with one or more {5}"), [{ kind: "keywords", keywords: [{ keyword: "craft",
      materials: [{ count: { amount: "chosen", minimum: 1 }, filter: {} }], cost: mana("5") }] }]);
  });

  it("restricts a description of cards to cards", () => {
    assert.deepEqual(craft("Craft with four or more red instant and/or sorcery cards {3}{R}{R}"), [{ kind: "keywords", keywords: [{
      keyword: "craft", materials: [{ count: { amount: "chosen", minimum: 4 },
        filter: { colors: ["red"], allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card" } }],
      cost: mana("3", "R", "R") }] }]);
  });

  it("keeps a shared-characteristic requirement on the group and a list as separate objects", () => {
    assert.deepEqual(craft("Craft with two that share a card type {6}"), [{ kind: "keywords", keywords: [{ keyword: "craft",
      materials: [{ count: fixed(2), filter: {}, groupConstraints: [{ constraint: "shared-value", characteristic: "card-type" }] }],
      cost: mana("6") }] }]);
    assert.deepEqual(craft("Craft with a Dinosaur, a Merfolk, a Pirate, and a Vampire {4}"), [{ kind: "keywords", keywords: [{
      keyword: "craft", materials: ["Dinosaur", "Merfolk", "Pirate", "Vampire"].map(subtype => ({ count: fixed(1), filter: { subtypes: [subtype] } })),
      cost: mana("4") }] }]);
  });

  it("rejects materials without a cost, an unstated plural number, or an unknown description", () => {
    rejects("Craft with artifact", "Artifact");
    rejects("Craft with artifacts {2}", "Artifact");
    rejects("Craft {2}", "Artifact");
    rejects("Craft with two that share a color {6}", "Artifact");
    rejects("Craft with two creatures, a Pirate {4}", "Artifact");
    rejects("Craft with four or more nonlands with activated abilities {8}{U}", "Artifact");
  });
});

// Scryfall: Ghostly Prison, Baird, Steward of Argive, Sphere of Safety,
// Phyrexian Marauder, Brainwash, Heat Wave, Hipparion, War Tax, Nils.
// CR 508.1h / 509.1d: paid while declaring; 508.1d / 509.1c: never required.
describe("costs to attack or block", () => {
  const creatures = { ref: "each" as const, filter: { types: ["creature"] } };
  const theirController = { player: "controller" as const, of: { ref: "them" as const } };
  const itsController = { player: "controller" as const, of: { ref: "it" as const } };
  const you = { defender: "player" as const, who: { player: "you" as const } };
  const yourPlaneswalkers = { defender: "object" as const, what: { ref: "each" as const, filter: { types: ["planeswalker"], control: "you" as const } } };
  const rule = (text: string, typeLine = "Enchantment") => {
    const [ability] = abilities(text, typeLine);
    assert.equal(ability.kind, "static");
    return ability.kind === "static" ? ability.effect : null;
  };

  it("keeps a printed total that counts the declared attackers", () => {
    assert.deepEqual(rule("Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you."), {
      static: "combat-rule", what: creatures, rule: { rule: "declaration-cost", action: "attack", who: theirController,
        costs: [{ cost: "mana", symbols: ["2"] }], scope: "declaration",
        multiplier: { amount: "count", of: { types: ["creature"], controlledBy: { player: "they" }, attacking: [you] } },
        defenders: [you] },
    });
  });

  it("charges each affected creature for 'each of those creatures', with a defined X", () => {
    assert.deepEqual(rule("Creatures can't attack you or planeswalkers you control unless their controller pays {X} for each of those creatures, where X is the number of enchantments you control."), {
      static: "combat-rule", what: creatures, rule: { rule: "declaration-cost", action: "attack", who: theirController,
        costs: [{ cost: "mana", symbols: ["X"] }], scope: "each-affected-creature", defenders: [you, yourPlaneswalkers],
        bindings: [{ variable: "x", value: { amount: "count", of: { types: ["enchantment"], control: "you" } } }] },
    });
  });

  it("scales one creature's own cost and keeps the payer", () => {
    assert.deepEqual(rule("This creature can't attack unless you pay {1} for each +1/+1 counter on it.", "Artifact Creature — Construct"), {
      static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: { rule: "declaration-cost", action: "attack", who: { player: "you" },
        costs: [{ cost: "mana", symbols: ["1"] }], scope: "each-affected-creature",
        multiplier: { amount: "counter-count", counter: { type: "+1/+1" }, on: { ref: "it" } } },
    });
    assert.deepEqual(rule("Enchanted creature can't attack unless its controller pays {3}.", "Enchantment — Aura"), {
      static: "combat-rule", what: { ref: "enchanted", noun: "creature" }, rule: { rule: "declaration-cost", action: "attack", who: itsController,
        costs: [{ cost: "mana", symbols: ["3"] }], scope: "each-affected-creature" },
    });
    assert.deepEqual(rule("Each creature with one or more counters on it can't attack you unless its controller pays {X}, where X is the number of counters on that creature.", "Creature"), {
      static: "combat-rule", what: { ref: "each", filter: { types: ["creature"],
        counterConstraints: [{ counter: "any", comparison: { op: "ge", value: fixed(1) } }] } },
      rule: { rule: "declaration-cost", action: "attack", who: itsController, costs: [{ cost: "mana", symbols: ["X"] }],
        scope: "each-affected-creature", defenders: [you],
        bindings: [{ variable: "x", value: { amount: "counter-count", counter: "any", on: { ref: "that", filter: { types: ["creature"] } } } }] },
    });
  });

  it("keeps which attackers a blocking cost applies to and non-mana payments", () => {
    assert.deepEqual(rule("Nonblue creatures can't block creatures you control unless their controller pays 1 life for each blocking creature they control."), {
      static: "combat-rule", what: { ref: "each", filter: { nonColors: ["blue"], types: ["creature"] } }, rule: {
        rule: "declaration-cost", action: "block", who: theirController, costs: [{ cost: "pay-life", amount: fixed(1) }], scope: "declaration",
        multiplier: { amount: "count", of: { status: ["blocking"], types: ["creature"], controlledBy: { player: "they" } } },
        attackers: { ref: "each", filter: { types: ["creature"], control: "you" } } },
    });
    assert.deepEqual(rule("This creature can't block creatures with power 3 or greater unless you pay {1}.", "Creature — Horse"), {
      static: "combat-rule", what: { ref: "this", noun: "creature" }, rule: { rule: "declaration-cost", action: "block", who: { player: "you" },
        costs: [{ cost: "mana", symbols: ["1"] }], scope: "each-affected-creature",
        attackers: { ref: "each", filter: { types: ["creature"], properties: [{ property: "power", comparison: { op: "ge", value: fixed(3) } }] } } },
    });
  });

  it("gives a rule created on resolution its stated lifetime", () => {
    assert.deepEqual(abilities("{X}{U}: This turn, creatures can't attack unless their controller pays {X} for each attacking creature they control.", "Enchantment"), [{
      kind: "activated", abilityWord: undefined, costs: [{ cost: "mana", symbols: ["X", "U"] }], restriction: undefined,
      effects: [{ steps: [{ effect: "combat-rule", what: creatures, duration: { duration: "this-turn" }, rule: {
        rule: "declaration-cost", action: "attack", who: theirController, costs: [{ cost: "mana", symbols: ["X"] }], scope: "declaration",
        multiplier: { amount: "count", of: { status: ["attacking"], types: ["creature"], controlledBy: { player: "they" } } } } }] }],
    }]);
  });

  it("still reads a state test after unless as a condition, not a cost", () => {
    assert.deepEqual(rule("This creature can't attack unless you control an artifact.", "Creature"), {
      static: "cant", what: { ref: "this", noun: "creature" }, action: "attack", conditionKind: "unless",
      condition: { condition: "control", who: { player: "you" }, what: { ref: "filter", filter: { types: ["artifact"] }, count: fixed(1) } },
    });
  });

  it("rejects a shared cost with no stated basis or an unrelated count", () => {
    rejects("Creatures can't attack you unless their controller pays {2}.", "Enchantment");
    rejects("Creatures can't attack you unless their controller pays {2} for each Cleric on the battlefield.", "Enchantment");
    rejects("Creatures can't attack you unless you pay {2} for each of those creatures.", "Enchantment");
    rejects("This creature can't attack unless you pay {2} for each of those creatures.", "Creature");
    rejects("This creature can't attack unless their controller pays {2}.", "Creature");
    rejects("Green creatures can't attack unless their controller sacrifices a land of their choice for each green creature they control that's attacking.", "Enchantment");
  });
});

// Scryfall: Aethertide Whale, Peema Trailblazer, Electrosiphon, Harnessed
// Lightning, Pia Nalaar, Chief Mechanic, Confiscation Coup, Die Young.
// CR 107.14: {E} is one energy counter; a stated number makes it the unit.
describe("energy amounts", () => {
  const energy = { type: "energy" };
  const you = { player: "you" as const };
  const paid = { amount: "event-quantity" as const, event: "energy-paid" as const, scope: "this-way" as const };
  const steps = (text: string) => {
    const [ability] = abilities(text, "Sorcery");
    assert.equal(ability.kind, "spell");
    return ability.kind === "spell" ? ability.effects : [];
  };

  it("gets a worded, relative, or computed number of energy counters", () => {
    assert.deepEqual(steps("You get six {E}."), [{ steps: [{ effect: "get-counters", who: you, counter: energy, count: fixed(6) }] }]);
    assert.deepEqual(steps("You get that many {E}."), [{ steps: [{ effect: "get-counters", who: you, counter: energy, count: { amount: "that-much" } }] }]);
    assert.deepEqual(steps("Counter target spell. You get an amount of {E} equal to its mana value."), [
      { steps: [{ effect: "counter", what: { ref: "target", filter: { cls: "spell" }, count: fixed(1) } }] },
      { steps: [{ effect: "get-counters", who: you, counter: energy, count: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }] },
    ]);
  });

  it("keeps a chosen payment's minimum: any amount is zero or more, one or more is not", () => {
    assert.deepEqual(steps("You get {E}{E}{E}, then you may pay any amount of {E}."), [{ steps: [
      { effect: "get-counters", who: you, counter: energy, count: fixed(3) },
      { effect: "pay-cost", who: you, costs: [{ cost: "energy", amount: { amount: "chosen", minimum: 0 } }], optional: true },
    ] }]);
    assert.deepEqual(steps("You may pay one or more {E}. If you do, draw cards equal to the amount of {E} paid this way."), [
      { steps: [{ effect: "pay-cost", who: you, costs: [{ cost: "energy", amount: { amount: "chosen", minimum: 1 } }], optional: true }] },
      { steps: [{ effect: "draw", who: you, amount: paid }], condition: { condition: "you-do" }, conditionKind: "if" },
    ]);
  });

  it("pays a worded or computed number as a resolving payment and as an activation cost", () => {
    assert.deepEqual(steps("You may pay eight {E}. If you do, draw a card.")[0],
      { steps: [{ effect: "pay-cost", who: you, costs: [{ cost: "energy", amount: fixed(8) }], optional: true }] });
    assert.deepEqual(steps("You may pay an amount of {E} equal to that card's mana value. If you do, draw a card.")[0],
      { steps: [{ effect: "pay-cost", who: you, optional: true, costs: [{ cost: "energy",
        amount: { amount: "attribute", of: { ref: "that", filter: { cls: "card" } }, attribute: "mana-value" } }] }] });
    assert.deepEqual(abilities("{T}, Pay fifty {E}: Draw seven cards.", "Artifact"), [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "tap-self" }, { cost: "energy", amount: fixed(50) }], restriction: undefined,
      effects: [{ steps: [{ effect: "draw", who: you, amount: fixed(7) }] }] }]);
  });

  it("scales one instruction by the energy paid this way", () => {
    assert.deepEqual(steps("You may pay any amount of {E}. Target creature gets -1/-1 until end of turn for each {E} paid this way.")[1], {
      steps: [{ effect: "pump", what: { ref: "target", filter: { types: ["creature"] }, count: fixed(1) }, duration: { duration: "end-of-turn" },
        power: { sign: -1, amount: { amount: "product", factors: [fixed(1), paid] } },
        toughness: { sign: -1, amount: { amount: "product", factors: [fixed(1), paid] } } }] });
  });

  it("rejects energy wording whose number or event is not stated this way", () => {
    rejects("You get an amount of {E}.", "Sorcery");
    rejects("You may pay any amount of {E} or life. If you do, draw a card.", "Sorcery");
    rejects("You may pay one or more. If you do, draw a card.", "Sorcery");
    rejects("Draw cards equal to the amount of {E} paid this turn.", "Sorcery");
    rejects("Draw cards equal to the amount of {E} you have paid.", "Sorcery");
  });
});

// Scryfall: Savage Ventmaw, Fire Lord Ozai, Shizuko, Caller of Autumn, Klauth,
// Colossal Plow, Su-Chi Cave Guard, Sakiko, Mother of Summer.
// CR 106.4 / 500.5: unspent mana normally empties as each step and phase ends.
describe("mana kept past steps and phases", () => {
  const you = { player: "you" as const };
  const steps = (text: string) => {
    const [ability] = abilities(text, "Sorcery");
    assert.equal(ability.kind, "spell");
    return ability.kind === "spell" ? ability.effects : [];
  };

  it("keeps the produced mana until end of turn, on the instruction that adds it", () => {
    assert.deepEqual(steps("Add {R}{R}{R}. Until end of turn, you don't lose this mana as steps and phases end."), [{ steps: [
      { effect: "add-mana", who: you, mana: { mana: "fixed", symbols: ["R", "R", "R"] }, keptUntil: { duration: "end-of-turn" } }] }]);
    assert.deepEqual(steps("Add {W}{W}{W} and you gain 3 life. Until end of turn, you don't lose this mana as steps and phases end."), [{ steps: [
      { effect: "add-mana", who: you, mana: { mana: "fixed", symbols: ["W", "W", "W"] }, keptUntil: { duration: "end-of-turn" } },
      { effect: "gain-life", who: you, amount: fixed(3) }] }]);
  });

  it("keeps it only through combat when the text says so, and for the player who added it", () => {
    assert.deepEqual(steps("Add six {R}. Until end of combat, you don't lose this mana as steps end."), [{ steps: [
      { effect: "add-mana", who: you, mana: { mana: "fixed", symbols: ["R", "R", "R", "R", "R", "R"] }, keptUntil: { duration: "end-of-combat" } }] }]);
    assert.deepEqual(steps("Target player adds {G}{G}{G}. Until end of turn, they don't lose this mana as steps and phases end."), [{ steps: [
      { effect: "add-mana", who: { player: "target-player" }, mana: { mana: "fixed", symbols: ["G", "G", "G"] }, keptUntil: { duration: "end-of-turn" } }] }]);
  });

  it("follows a spending restriction on the same mana", () => {
    assert.deepEqual(steps("Add that much {G}. Spend this mana only to cast spells. Until end of turn, you don't lose this mana as steps and phases end."), [{ steps: [
      { effect: "add-mana", who: you, mana: { mana: "scaled-symbol", symbol: "G", amount: { amount: "that-much" } },
        restriction: { restriction: "only", uses: [{ use: "cast", filter: { cls: "spell" } }] }, keptUntil: { duration: "end-of-turn" } }] }]);
  });

  it("rejects a mismatched player, period, or mana that this instruction did not add", () => {
    rejects("Add {R}. Until end of turn, they don't lose this mana as steps and phases end.", "Sorcery");
    rejects("Add {R}. Until end of turn, you don't lose this mana as steps end.", "Sorcery");
    rejects("Add {R}. Until your next turn, you don't lose this mana as steps and phases end.", "Sorcery");
    rejects("Add {R}. Until end of combat, you don't lose this mana as steps and phases end.", "Sorcery");
    rejects("Draw a card. Until end of turn, you don't lose this mana as steps and phases end.", "Sorcery");
    rejects("Add {R}. Add {G}. Until end of turn, you don't lose this mana as steps and phases end. Until end of turn, you don't lose this mana as steps and phases end.", "Sorcery");
    rejects("Add six {R}{G}.", "Sorcery");
    rejects("Add six {2}.", "Sorcery");
  });
});

// Scryfall: Lair of the Hydra, Ludevic, Necrogenius, Benalish Commander,
// Thieving Skydiver, Shanna, Purifying Blade, Mind Grind, Soul Immolation.
// CR 107.3a/d/f: X is chosen by the controller; these sentences limit it.
describe("limits on a chosen X", () => {
  const notZero = [{ variable: "x" as const, not: 0 }];
  it("limits an activated ability's X before its activation rules", () => {
    assert.deepEqual(abilities("{X}{U}, Exile X creature cards from your graveyard: Draw X cards. X can't be 0. Activate only as a sorcery.", "Creature"), [{
      kind: "activated", abilityWord: undefined,
      costs: [{ cost: "mana", symbols: ["X", "U"] }, { cost: "exile", what: { ref: "filter", count: { amount: "x" },
        filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } } }],
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }] }],
      variableConstraints: notZero, restriction: { restriction: "sorcery" },
    }]);
  });

  it("limits the X of a keyword's cost", () => {
    assert.deepEqual(abilities("Suspend X—{X}{W}{W}. X can't be 0.", "Creature"), [{ kind: "keywords", keywords: [{ keyword: "suspend", amount: "x",
      cost: [{ cost: "mana", symbols: ["X", "W", "W"] }], variableConstraints: notZero }] }]);
    assert.deepEqual(abilities("Kicker {X}. X can't be 0.", "Creature"), [{ kind: "keywords", keywords: [{ keyword: "kicker",
      cost: [{ cost: "mana", symbols: ["X"] }], variableConstraints: notZero }] }]);
  });

  it("keeps a computed maximum, on a spell, a trigger, and an additional cost", () => {
    assert.deepEqual(abilities("Draw X cards. X can't be 0.", "Sorcery"), [{ kind: "spell", abilityWord: undefined,
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }] }], variableConstraints: notZero }]);
    const [trigger] = abilities("At the beginning of your end step, you may pay {X}. If you do, draw X cards. X can't be greater than the amount of life you gained this turn.", "Creature");
    assert.deepEqual(trigger.kind === "triggered" && trigger.variableConstraints, [{ variable: "x",
      maximum: { amount: "life-change", who: { player: "you" }, change: "gained", during: "this-turn" } }]);
    assert.deepEqual(abilities("As an additional cost to cast this spell, blight X. X can't be greater than the greatest toughness among creatures you control.", "Sorcery"), [{
      kind: "additional-cost", costs: [{ cost: "blight", amount: { amount: "x" } }],
      variableConstraints: [{ variable: "x", maximum: { amount: "attribute-extremum", extremum: "maximum", attribute: "toughness",
        among: { types: ["creature"], control: "you" } } }] }]);
  });

  it("rejects a limit with no X to limit, or in a place that has no chosen X", () => {
    rejects("{2}: Draw a card. X can't be 0.", "Creature");
    rejects("Draw a card. X can't be 0.", "Sorcery");
    rejects("Flashback {2}{U}. X can't be 0.", "Sorcery");
    rejects("Draw X cards. X can't be greater than X.", "Sorcery");
    rejects("Draw X cards. X must be 0.", "Sorcery");
    rejects("-X: Draw X cards. X can't be 0.", "Legendary Planeswalker — Jace");
  });
});

// Scryfall: Rosheen Meanderer, Cultivator Drone, Adarkar Unicorn, Unblinking
// Observer, Karfell Harbinger, Creeping Peeper, Tin Street Gossip, Freya
// Crescent, Quinjet Technician, Jegantha, the Wellspring.
// CR 106.6 with 116.2 (special actions), 107.3 / 107.4b-c (cost symbols).
describe("mana limited to special payments", () => {
  const restriction = (text: string) => {
    const [ability] = abilities(text, "Creature");
    const step = ability.kind === "activated" ? ability.effects[0].steps[0] : null;
    return step?.effect === "add-mana" ? step.restriction : null;
  };

  it("limits mana to costs containing a symbol, or forbids generic costs", () => {
    assert.deepEqual(restriction("{T}: Add {C}{C}{C}{C}. Spend this mana only on costs that contain {X}."),
      { restriction: "only", uses: [{ use: "pay-cost-containing", symbol: "X" }] });
    assert.deepEqual(restriction("{T}: Add {W}{U}{B}{R}{G}. This mana can't be spent to pay generic mana costs."),
      { restriction: "not", uses: [{ use: "pay-generic-costs" }] });
  });

  it("keeps every purpose of a comma list as an alternative", () => {
    assert.deepEqual(restriction("{T}: Add {C}. Spend this mana only to cast a colorless spell, activate an ability of a colorless permanent, or pay a cost that contains {C}."), {
      restriction: "only", uses: [{ use: "cast", filter: { colorless: true, cls: "spell" } },
        { use: "activate", source: { colorless: true, cls: "permanent" } }, { use: "pay-cost-containing", symbol: "C" }] });
    assert.deepEqual(restriction("{T}: Add {U}. Spend this mana only to cast an enchantment spell, unlock a door, or turn a permanent face up."), {
      restriction: "only", uses: [{ use: "cast", filter: { types: ["enchantment"], cls: "spell" } }, { use: "unlock-door" },
        { use: "turn-face-up", what: { cls: "permanent" } }] });
  });

  it("names keyword costs, special actions and keyword-defined abilities", () => {
    assert.deepEqual(restriction("{T}: Add {U}. Spend this mana only to pay cumulative upkeep costs."),
      { restriction: "only", uses: [{ use: "pay-keyword-cost", keyword: "cumulative upkeep" }] });
    assert.deepEqual(restriction("{T}: Add {U}. Spend this mana only to pay a disturb cost or cast an instant or sorcery spell."), {
      restriction: "only", uses: [{ use: "pay-keyword-cost", keyword: "disturb" },
        { use: "cast", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } }] });
    assert.deepEqual(restriction("{T}: Add {U}. Spend this mana only to foretell a card from your hand or cast an instant or sorcery spell."), {
      restriction: "only", uses: [{ use: "foretell" },
        { use: "cast", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } }] });
    assert.deepEqual(restriction("{T}: Add {R}{G}. Spend this mana only to cast face-down spells or to turn creatures face up."), {
      restriction: "only", uses: [{ use: "cast", filter: { status: ["face-down"], cls: "spell" } }, { use: "turn-face-up", what: { types: ["creature"] } }] });
    assert.deepEqual(restriction("{T}: Add {R}. Spend this mana only to cast an Equipment spell or activate an equip ability."), {
      restriction: "only", uses: [{ use: "cast", filter: { subtypes: ["Equipment"], cls: "spell" } }, { use: "activate", abilityKeyword: "equip" }] });
    assert.deepEqual(restriction("{T}: Add {R}{R}. Spend this mana only to activate power-up abilities."),
      { restriction: "only", uses: [{ use: "activate", abilityKeyword: "power-up" }] });
  });

  it("rejects purposes it cannot name", () => {
    rejects("{T}: Add {C}. Spend this mana only on costs that contain {2}.", "Creature");
    rejects("{T}: Add {C}. Spend this mana only on spells.", "Creature");
    rejects("{T}: Add {C}. Spend this mana only to pay a kicker cost.", "Creature");
    rejects("{T}: Add {C}. Spend this mana only to pay a mana cost to turn a manifested creature face up.", "Creature");
    rejects("{T}: Add {C}. Spend this mana only to activate flying abilities.", "Creature");
    rejects("{T}: Add {C}. Spend this mana only to cast a creature spell, unlock a door.", "Creature");
  });
});

// Scryfall: Lurrus of the Dream-Den, Kaheera, the Orphanguard, Keruga, the
// Macrosage, Lutri, the Spellchaser, Umori, the Collector.
// CR 702.139a-b: a condition on the starting deck, checked outside the game.
describe("companion deck conditions", () => {
  const companion = (text: string) => {
    const [ability] = abilities(text, "Legendary Creature — Cat Nightmare");
    assert.equal(ability.kind, "keywords");
    return ability.kind === "keywords" ? ability.keywords : [];
  };

  it("tests every matching deck card against the stated quality", () => {
    assert.deepEqual(companion("Companion — Each permanent card in your starting deck has mana value 2 or less."), [{ keyword: "companion",
      companion: { deck: "each", of: { cls: "card", permanent: true },
        matches: { properties: [{ property: "mana-value", comparison: { op: "le", value: fixed(2) } }] } } }]);
    assert.deepEqual(companion("Companion — Each creature card in your starting deck is a Cat, Elemental, Nightmare, Dinosaur, or Beast card."), [{
      keyword: "companion", companion: { deck: "each", of: { types: ["creature"], cls: "card" },
        matches: { allOf: [{ anyOf: ["Cat", "Elemental", "Nightmare", "Dinosaur", "Beast"].map(subtype => ({ subtypes: [subtype] })) }] } } }]);
  });

  it("reads 'contains only' as every card being one of the listed kinds", () => {
    assert.deepEqual(companion("Companion — Your starting deck contains only cards with mana value 3 or greater and land cards."), [{
      keyword: "companion", companion: { deck: "each", of: { cls: "card" }, matches: { anyOf: [
        { properties: [{ property: "mana-value", comparison: { op: "ge", value: fixed(3) } }] }, { types: ["land"] }] } } }]);
  });

  it("keeps conditions on the cards taken together as group constraints", () => {
    assert.deepEqual(companion("Companion — Each nonland card in your starting deck has a different name."), [{ keyword: "companion",
      companion: { deck: "group", of: { nonTypes: ["land"], cls: "card" }, constraint: { constraint: "pairwise-different", property: "name" } } }]);
    assert.deepEqual(companion("Companion — Each nonland card in your starting deck shares a card type."), [{ keyword: "companion",
      companion: { deck: "group", of: { nonTypes: ["land"], cls: "card" }, constraint: { constraint: "shared-value", characteristic: "card-type" } } }]);
  });

  it("rejects conditions it cannot state and decks other than the starting deck", () => {
    rejects("Companion — Each permanent card in your starting deck has an activated ability.", "Legendary Creature — Fox");
    // Gyruda's parity condition is now represented completely (ObjectFilter.parity).
    assert.deepEqual(companion("Companion — Your starting deck contains only cards with even mana values."), [{ keyword: "companion",
      companion: { deck: "each", of: { cls: "card" }, matches: { parity: { property: "mana-value", parity: "even" } } } }]);
    rejects("Companion — Your starting deck contains at least twenty cards more than the minimum deck size.", "Legendary Creature — Bird");
    rejects("Companion — Each creature card in your library is a Cat card.", "Legendary Creature — Cat");
    rejects("Companion — Each creature card in your starting deck is a card.", "Legendary Creature — Cat");
    rejects("Companion", "Legendary Creature — Cat");
  });
});

// Scryfall: Klaw, Master of Sound; Black Cat, Cunning Thief; Bloodsoaked
// Insight. CR 609.4b: the rider changes how a cost may be paid, nothing else.
describe("any-type mana payment stated after a play permission", () => {
  const permission = (text: string) => {
    const [ability] = abilities(text, "Sorcery");
    const step = ability.kind === "spell" ? ability.effects.at(-1)?.steps[0] : null;
    return step?.effect === "grant-play-permission" ? step : null;
  };
  const expected = { effect: "grant-play-permission", duration: { duration: "this-turn" },
    permission: { who: { player: "you" }, action: "play", what: { ref: "it" }, manaPayment: "any-type" } };

  it("folds the passive sentence into the permission, in each printed form", () => {
    for (const text of [
      "Exile the top card of your library. You may play it this turn. Mana of any type can be spent to cast a spell this way.",
      "Exile the top card of your library. You may play it this turn. Mana of any type can be spent to cast that spell.",
      "Exile the top card of your library. You may play it this turn. If you cast a spell this way, mana of any type can be spent to cast it.",
      "Exile the top card of your library. You may play it this turn, and mana of any type can be spent to cast it.",
    ]) assert.deepEqual(permission(text), expected, text);
  });

  it("rejects the sentence without a permission, twice, or for another purpose", () => {
    rejects("Draw a card. Mana of any type can be spent to cast a spell this way.", "Sorcery");
    rejects("Exile the top card of your library. You may play it this turn, and mana of any type can be spent to cast it. Mana of any type can be spent to cast that spell.", "Sorcery");
    rejects("Exile the top card of your library. You may play it this turn. Mana of any type can be spent to activate abilities this way.", "Sorcery");
    rejects("Exile the top card of your library. You may play it this turn. Mana of any color can be spent to cast that spell.", "Sorcery");
  });
});

// Scryfall: Inspired Fire, Heliod the Radiant Dawn, Gingerbehemoth, The
// Balrog, Delightful Discovery, Thrasta, Demilich, Damping Sphere, Licia,
// Avatar of Growth. CR 601.2f applies the multiplier; 608.2i: past actions
// are counted as they happened.
describe("cost adjustments counted from this turn's history", () => {
  const thisSpell = { ref: "this" as const, noun: "spell" };
  const multiplier = (text: string) => {
    const [ability] = abilities(text, "Creature");
    return ability.kind === "static" && ability.effect.static === "cost-modification" ? ability.effect.multiplier : null;
  };

  it("counts draws and sacrifices by the stated players, or by anyone when none is stated", () => {
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each card you've drawn this turn."),
      { amount: "player-action-count", who: { player: "you" }, action: { action: "draw" }, during: "this-turn" });
    assert.deepEqual(multiplier("Spells you cast cost {1} less to cast for each card your opponents have drawn this turn."),
      { amount: "player-action-count", who: { player: "each-opponent" }, action: { action: "draw" }, during: "this-turn" });
    assert.deepEqual(multiplier("This spell costs {2} less to cast for each Food you've sacrificed this turn."),
      { amount: "player-action-count", who: { player: "you" }, action: { action: "sacrifice", what: { subtypes: ["Food"] } }, during: "this-turn" });
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each permanent sacrificed this turn."),
      { amount: "player-action-count", who: "all-players-combined", action: { action: "sacrifice", what: { cls: "permanent" } }, during: "this-turn" });
  });

  it("counts spells cast, keeping 'other' and the caster", () => {
    assert.deepEqual(multiplier("This spell costs {3} less to cast for each other spell cast this turn."),
      { amount: "cast-count", who: "all-players-combined", what: { other: true, cls: "spell" }, during: "this-turn" });
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each spell your opponents have cast this turn."),
      { amount: "cast-count", who: { player: "each-opponent" }, what: { cls: "spell" }, during: "this-turn" });
    assert.deepEqual(abilities("Each spell a player casts costs {1} more to cast for each other spell that player has cast this turn.", "Artifact"), [{
      kind: "static", abilityWord: undefined, effect: { static: "cost-modification",
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "each-player" } },
        change: { direction: "increase", symbols: ["1"] },
        multiplier: { amount: "cast-count", who: { player: "that-player" }, what: { other: true, cls: "spell" }, during: "this-turn" } } }]);
  });

  it("uses life gained and the number of opponents as units", () => {
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each 1 life you gained this turn."),
      { amount: "life-change", who: { player: "you" }, change: "gained", during: "this-turn" });
    // Several players' changes are added together, as for other history counts.
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each 1 life your opponents have lost this turn."),
      { amount: "life-change", who: { player: "each-opponent" }, change: "lost", during: "this-turn" });
    assert.deepEqual(abilities("This spell costs {1} less to cast for each opponent you have.", "Creature"), [{ kind: "static", abilityWord: undefined,
      effect: { static: "cost-modification", scope: { action: "cast", spells: thisSpell }, change: { direction: "reduce", symbols: ["1"] },
        multiplier: { amount: "opponent-count", who: { player: "you" } } } }]);
  });

  it("rejects histories it cannot count faithfully", () => {
    // Formerly rejected: now counted (CR 702.29a cycling discards the card; CR 506.3 attacked players).
    assert.deepEqual(multiplier("This spell costs {2} less to cast for each card you've cycled or discarded this turn."),
      { amount: "player-action-count", who: { player: "you" }, action: { action: "discard", what: { cls: "card" } }, during: "this-turn" });
    assert.deepEqual(multiplier("This spell costs {1} less to cast for each opponent you attacked this turn."),
      { amount: "attacked-player-count", players: "opponents", which: { by: { player: "you" }, during: "this-turn" } });
    rejects("This spell costs {1} less to cast for each creature card you've drawn this turn.", "Creature");
    rejects("This spell costs {1} less to cast for each card you've drawn this game.", "Creature");
    rejects("This spell costs {1} less to cast for each card you've drawn.", "Creature");
  });
});

// Scryfall: Consume Spirit, Soul Burn, Crypt Rats, Emblazoned Golem,
// Imperiosaur, Myr Superion, Security Rhox. CR 601.2h / 602.2b: the limit
// applies while the cost is paid; 107.3 for the X part.
describe("limits on the mana that may pay a cost", () => {
  const thisSpell = { ref: "this" as const, noun: "spell" };
  it("limits the X of a spell to listed colors, or to colored mana with a per-color maximum", () => {
    assert.deepEqual(abilities("Spend only black and/or red mana on X.", "Sorcery"), [{ kind: "static", abilityWord: undefined, effect: {
      static: "payment-restriction", what: thisSpell, restriction: { part: "x", mana: { mana: "colors", colors: ["black", "red"] } } } }]);
    assert.deepEqual(abilities("Spend only colored mana on X. No more than one mana of each color may be spent this way.", "Artifact Creature — Golem"), [{
      kind: "static", abilityWord: undefined, effect: { static: "payment-restriction", what: thisSpell,
        restriction: { part: "x", mana: { mana: "colored" }, maximumPerColor: 1 } } }]);
  });

  it("limits a whole cost to mana from matching sources, also for an alternative cost", () => {
    assert.deepEqual(abilities("Spend only mana produced by basic lands to cast this spell.", "Creature — Dinosaur"), [{ kind: "static", abilityWord: undefined,
      effect: { static: "payment-restriction", what: thisSpell,
        restriction: { part: "total", mana: { mana: "produced-by", sources: { supertypes: ["basic"], types: ["land"] } } } } }]);
    assert.deepEqual(abilities("You may pay {R}{G} rather than pay this spell's mana cost. Spend only mana produced by Treasures to cast it this way.", "Creature — Rhino"), [{
      kind: "static", abilityWord: undefined, effect: { static: "alternative-cost", who: { player: "you" }, spells: thisSpell,
        costs: [{ cost: "mana", symbols: ["R", "G"] }],
        paymentRestriction: { part: "total", mana: { mana: "produced-by", sources: { subtypes: ["Treasure"] } } } } }]);
  });

  it("limits the X of an activated ability's cost", () => {
    assert.deepEqual(abilities("{X}, {T}: Draw X cards. Spend only red mana on X.", "Creature"), [{ kind: "activated", abilityWord: undefined,
      costs: [{ cost: "mana", symbols: ["X"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "x" } }] }],
      paymentRestriction: { part: "x", mana: { mana: "colors", colors: ["red"] } }, restriction: undefined }]);
  });

  it("rejects limits with no X cost, the wrong closing phrase, or an unknown kind of mana", () => {
    rejects("{2}, {T}: Draw a card. Spend only red mana on X.", "Creature");
    rejects("Whenever this creature attacks, draw X cards. Spend only red mana on X.", "Creature");
    rejects("Spend only mana produced by basic lands to cast it this way.", "Creature");
    rejects("Spend only mana produced by basic lands.", "Creature");
    rejects("Spend only snow mana on X.", "Sorcery");
    rejects("Spend only black mana on X. No more than one mana of each color may be spent this way.", "Sorcery");
    rejects("Spend only black or red mana on X.", "Sorcery");
  });
});
