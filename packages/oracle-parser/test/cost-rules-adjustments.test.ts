import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

const you = { player: "you" as const };
const thisSpell = { action: "cast" as const, spells: { ref: "this" as const, noun: "spell" as const } };
const reduceOne = { direction: "reduce" as const, symbols: ["1"] };
const controlOne = (filter: object) => ({ condition: "control", who: you, what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } });
function abilities(text: string, typeLine?: string): Ability[] {
  const parsed = parseOracleText(text, undefined, typeLine ? { typeLine } : {});
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function rejects(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : {}).ok, false, text);
}

// Scryfall: Valiant Changeling. CR 205.3, 702.73a (creature types), 601.2f.
describe("creature types and permanent types among objects", () => {
  it("counts creature types and caps the reduction", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast for each creature type among creatures you control. This effect can't reduce the amount of mana this spell costs by more than {5}.", "Creature — Shapeshifter"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } },
        change: { direction: "reduce", symbols: ["1"] },
        multiplier: { amount: "distinct-count", characteristic: "creature-type", among: { ref: "each", filter: { types: ["creature"], control: "you" } } },
        maximumReduction: { amount: "fixed", value: 5 } } }]);
  });

  it("rejects a cap that is not a number of mana, and an increase with a reduction cap", () => {
    rejects("This spell costs {1} less to cast. This effect can't reduce the amount of mana this spell costs by more than {R}.", "Creature — Shapeshifter");
    rejects("This spell costs {1} more to cast. This effect can't reduce the amount of mana this spell costs by more than {5}.", "Creature — Shapeshifter");
  });

  // Scryfall: Matzalantli, the Great Door. CR 110.4.
  it("counts permanent types in an activation condition", () => {
    const [ability] = abilities("{4}, {T}: Transform ~. Activate only if there are four or more permanent types among cards in your graveyard.", "Artifact");
    assert.deepEqual((ability as { restriction: unknown }).restriction, { restriction: "condition", condition: { condition: "compare",
      value: { amount: "distinct-count", characteristic: "permanent-type", among: { ref: "each", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } } } },
      comparison: { op: "ge", value: { amount: "fixed", value: 4 } } } });
  });
});

// Scryfall: Assassin's Ink, Geistlight Snare, Dargo, the Shipwrecker. CR 601.2f.
describe("several adjustments of one spell stated together", () => {
  const group = [{ kind: "static", effect: { static: "group", effects: [
    { static: "conditional", condition: controlOne({ types: ["artifact"] }), effect: { static: "cost-modification", scope: thisSpell, change: reduceOne } },
    { static: "conditional", condition: controlOne({ types: ["enchantment"] }), effect: { static: "cost-modification", scope: thisSpell, change: reduceOne } },
  ] } }];

  it("coordinates two reductions with their own conditions", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast if you control an artifact and {1} less to cast if you control an enchantment.", "Instant"), group);
  });

  it("reads 'It also costs' as a second, independent adjustment of the same spell", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast if you control an artifact. It also costs {1} less to cast if you control an enchantment.", "Instant"), group);
  });

  it("keeps a coordinated pair of additional-cost reductions as separate modifications", () => {
    const [ability] = abilities("As an additional cost to cast this spell, you may sacrifice any number of artifacts and/or creatures. This spell costs {2} less to cast for each permanent sacrificed this way and {2} less to cast for each other artifact or creature you've sacrificed this turn.", "Legendary Creature — Giant Pirate");
    const mods = (ability as { costModifications: { change: unknown; multiplier: { amount: string } }[] }).costModifications;
    assert.equal(mods.length, 2);
    assert.deepEqual(mods.map(mod => mod.change), [{ direction: "reduce", symbols: ["2"] }, { direction: "reduce", symbols: ["2"] }]);
    assert.deepEqual(mods.map(mod => mod.multiplier.amount), ["count", "player-action-count"]);
  });

  it("does not accept a second sentence that is not a cost adjustment of the same spell", () => {
    rejects("This spell costs {1} less to cast if you control an artifact. It also costs you 2 life.", "Instant");
    rejects("This spell costs {1} less to cast if you control an artifact. It also costs {1} less to activate if you control an enchantment.", "Instant");
    rejects("Spells you cast cost {1} less to cast if you control an artifact. It also costs {1} less to cast if you control an enchantment.", "Enchantment");
  });
});

// Scryfall: Kaza, Roil Chaser; Maelstrom Muse. CR 608.2h.
describe("where-X definition evaluated as the ability resolves", () => {
  const nextSpell = { action: "cast", spells: { ref: "each", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } },
    caster: you, occurrence: { next: true } };
  const effect = (value: object) => ({ kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "cost-modification", scope: nextSpell,
    change: { direction: "reduce", symbols: ["X"] }, duration: { duration: "this-turn" } }], bindings: [{ variable: "x", value }] }] });

  it("reads 'as this ability resolves' as the resolving binding itself", () => {
    const text = (tail: string) => `{T}: The next instant or sorcery spell you cast this turn costs {X} less to cast, where X is the number of Wizards you control${tail}.`;
    const value = { amount: "count", of: { subtypes: ["Wizard"], control: "you" } };
    assert.deepEqual(abilities(text(" as this ability resolves"), "Creature — Human Wizard"), [effect(value)]);
    assert.deepEqual(abilities(text(""), "Creature — Human Wizard"), [effect(value)]);
  });

  it("keeps an evaluation time other than resolution on the binding", () => {
    const [ability] = abilities("{T}: The next instant or sorcery spell you cast this turn costs {X} less to cast, where X is the number of Wizards you control as you activate this ability.", "Creature — Human Wizard");
    assert.match(JSON.stringify(ability), /"bindings":\[\{"variable":"x","value":\{"amount":"count".*"asOf":"activation"\}\]/);
  });
});

// Scryfall: Battlefield Thaumaturge. CR 601.2c, 601.2f.
describe("cost change for each target of a stated kind", () => {
  it("counts only the chosen targets that are creatures", () => {
    assert.deepEqual(abilities("Each instant and sorcery spell you cast costs {1} less to cast for each creature it targets.", "Creature — Human Wizard"), [{
      kind: "static", effect: { static: "cost-modification",
        scope: { action: "cast", spells: { ref: "each", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } }, caster: you },
        change: reduceOne, multiplier: { amount: "target-count", of: { ref: "affected-object" }, matching: { types: ["creature"] } } } }]);
  });
});

// Scryfall: Trap Runner. CR 509.1.
describe("activation timing relative to blockers being declared", () => {
  it("allows activation only in combat after blockers are declared", () => {
    const [ability] = abilities("{T}: Target unblocked attacking creature becomes blocked. Activate only during combat after blockers are declared.", "Creature — Human");
    assert.deepEqual((ability as { restriction: unknown }).restriction, { restriction: "timing", phase: "combat", after: "declare-blockers" });
  });

  it("allows activation only before blockers are declared", () => {
    const [ability] = abilities("{T}: You gain 1 life. Activate only before blockers are declared.", "Creature — Human");
    assert.deepEqual((ability as { restriction: unknown }).restriction, { restriction: "timing", before: "declare-blockers" });
  });

  it("does not attach 'after blockers are declared' to a bare phase it does not follow", () => {
    rejects("{T}: You gain 1 life. Activate only during your upkeep after blockers are declared.", "Creature — Human");
  });
});

// Scryfall: Nature's Chosen, Sawback Manticore. CR 506.4 (attacking, blocking), 502.3 (tapped).
describe("coordinated statuses in an activation condition", () => {
  const restriction = (text: string) => (abilities(text, "Creature — Human")[0] as { restriction: unknown }).restriction;
  const self = { ref: "this", noun: "creature" };

  it("is either of two statuses", () => {
    assert.deepEqual(restriction("{1}: You gain 1 life. Activate only if this creature is attacking or blocking."), { restriction: "condition", condition: {
      condition: "any-of", conditions: [{ condition: "status", what: self, status: "attacking" }, { condition: "status", what: self, status: "blocking" }] } });
  });

  it("is a color and a status together and keeps the following restriction", () => {
    assert.deepEqual(restriction("{1}: You gain 1 life. Activate only if this creature is white and untapped and only once each turn."), { restriction: "all-of", restrictions: [
      { restriction: "condition", condition: { condition: "all-of", conditions: [{ condition: "is-filter", what: self, filter: { colors: ["white"] } },
        { condition: "status", what: self, status: "untapped" }] } }, { restriction: "once-each-turn" }] });
  });

  it("does not mix connectors or distribute a negation over 'and'", () => {
    rejects("{1}: You gain 1 life. Activate only if this creature isn't attacking and blocking.", "Creature — Human");
    rejects("{1}: You gain 1 life. Activate only if this creature is tapped or attacking and blocking.", "Creature — Human");
  });
});

// Scryfall: Suppression Field, Anointed Peacekeeper, Tithe Taker. CR 605.1a (mana abilities), 602.1.
describe("activation cost increases that exempt mana abilities", () => {
  const nonMana = { ref: "each", filter: { cls: "ability", manaAbility: false } };

  it("excludes mana abilities from all activated abilities", () => {
    assert.deepEqual(abilities("Activated abilities cost {2} more to activate unless they're mana abilities.", "Artifact"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "activate", abilities: nonMana }, change: { direction: "increase", symbols: ["2"] } } }]);
  });

  it("keeps the source restriction beside the exemption", () => {
    const [ability] = abilities("Activated abilities of sources with the chosen name cost {2} more to activate unless they're mana abilities.", "Creature — Human");
    const effect = (ability as { effect: { scope: { source: unknown; abilities: unknown } } }).effect;
    assert.deepEqual(effect.scope.abilities, nonMana);
    assert.ok(effect.scope.source);
  });

  it("applies the exemption only to the abilities half of a coordinated pair", () => {
    const [ability] = abilities("During your turn, spells your opponents cast cost {1} more to cast and abilities your opponents activate cost {1} more to activate unless they're mana abilities.", "Creature — Human");
    const effects = (ability as { effect: { static: string; effects: { condition: unknown; effect: { scope: { action: string; abilities?: unknown } } }[] } }).effect.effects;
    assert.equal(effects.length, 2);
    assert.deepEqual(effects.map(e => e.condition), [{ condition: "your-turn" }, { condition: "your-turn" }]);
    assert.deepEqual(effects.map(e => e.effect.scope.action), ["cast", "activate"]);
    assert.equal(effects[0].effect.scope.abilities, undefined);
    assert.deepEqual(effects[1].effect.scope.abilities, nonMana);
  });

  it("rejects the exemption on a cast cost and on a keyword cost", () => {
    rejects("Spells cost {2} more to cast unless they're mana abilities.", "Artifact");
    rejects("Equip costs {2} more unless they're mana abilities.", "Artifact");
    rejects("Abilities that aren't mana abilities cost {2} more to activate unless they're mana abilities.", "Artifact");
  });
});

// Scryfall: Lumbering Megasloth. CR 122.1.
describe("cost change for each counter among a group", () => {
  it("counts counters on players and on permanents", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast for each counter among players and permanents.", "Creature — Sloth"), [{
      kind: "static", effect: { static: "cost-modification", scope: thisSpell, change: reduceOne,
        multiplier: { amount: "plus", a: { amount: "counter-count", counter: "any", on: { player: "each-player" }, total: true },
          b: { amount: "counter-count", counter: "any", on: { ref: "each", filter: { cls: "permanent" } } } } } }]);
  });

  it("counts one kind of counter over a described group", () => {
    const [ability] = abilities("This spell costs {1} less to cast for each +1/+1 counter among creatures you control.", "Instant");
    assert.deepEqual((ability as { effect: { multiplier: unknown } }).effect.multiplier, { amount: "counter-count", counter: { type: "+1/+1" },
      on: { ref: "each", filter: { types: ["creature"], control: "you" } } });
  });
});

// Scryfall: Jace, Reality Sculptor. CR 122.1; hyphenated number words.
describe("counters among a group in a condition", () => {
  it("compares the total over the group and reads hyphenated number words", () => {
    const [ability] = abilities("{T}: You gain 1 life. Activate only if there are twenty-five or more loyalty counters among Jaces you control.", "Artifact");
    assert.deepEqual((ability as { restriction: unknown }).restriction, { restriction: "condition", condition: { condition: "compare",
      value: { amount: "counter-count", counter: { type: "loyalty" }, on: { ref: "each", filter: { subtypes: ["Jace"], control: "you" } } },
      comparison: { op: "ge", value: { amount: "fixed", value: 25 } } } });
  });

  it("rejects a malformed hyphenated number", () => {
    rejects("{T}: You gain 1 life. Activate only if there are twenty-twenty or more loyalty counters among Jaces you control.", "Artifact");
    rejects("{T}: You gain 1 life. Activate only if there are ten-five or more loyalty counters among Jaces you control.", "Artifact");
  });
});

// Scryfall: Myth Unbound, Opal Palace, Study Hall. CR 903.8 (commander tax counts each commander's casts).
describe("times a commander has been cast from the command zone", () => {
  const timesCast = (what: object) => ({ amount: "times-cast", what, from: { zone: "command" }, during: "this-game" });

  it("reduces each commander's cost by its own cast count", () => {
    assert.deepEqual(abilities("Your commander costs {1} less to cast for each time it's been cast from the command zone this game.", "Enchantment"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "commander", owner: you, count: { amount: "fixed", value: 1 } } },
        change: reduceOne, multiplier: timesCast({ ref: "affected-object" }) } }]);
  });

  it("scries that many when the mana is spent on the commander", () => {
    const [ability] = abilities("{1}, {T}: Add one mana of any color. When you spend this mana to cast your commander, scry X, where X is the number of times it's been cast from the command zone this game.", "Artifact");
    const rider = (ability as { effects: { steps: { riders: unknown[] }[] }[] }).effects[0].steps[0].riders[0];
    assert.deepEqual(rider, { rider: "delayed-trigger", spentOn: [{ use: "cast-commander", whose: you }], effects: [{
      steps: [{ effect: "scry", amount: { amount: "x" } }], bindings: [{ variable: "x", value: timesCast({ ref: "it" }) }] }] });
  });

  it("gives the commander counters equal to that count when 'you spend this mana'", () => {
    const [ability] = abilities("{1}, {T}: Add one mana of any color in your commander's color identity. If you spend this mana to cast your commander, it enters with a number of additional +1/+1 counters on it equal to the number of times it's been cast from the command zone this game.", "Land");
    const rider = (ability as { effects: { steps: { riders: unknown[] }[] }[] }).effects[0].steps[0].riders[0];
    assert.deepEqual(rider, { rider: "entry", mana: "that", spentOn: [{ use: "cast-commander", whose: you }], effect: {
      static: "enters-with-counters", what: { ref: "it" }, counter: { type: "+1/+1" }, count: timesCast({ ref: "it" }), additional: true } });
  });

  it("rejects other zones and other periods", () => {
    rejects("Your commander costs {1} less to cast for each time it's been cast from your hand this game.", "Enchantment");
    rejects("Your commander costs {1} less to cast for each time it's been cast from the command zone this turn.", "Enchantment");
  });
});

// Scryfall: Keys to the House, Marina Vendrell. CR 709.5f–g.
describe("lock or unlock a door of a Room", () => {
  const room = { ref: "target", filter: { subtypes: ["Room"], control: "you" }, count: { amount: "fixed", value: 1 } };

  it("keeps the choice between locking and unlocking and the sorcery restriction", () => {
    assert.deepEqual(abilities("{T}: Lock or unlock a door of target Room you control. Activate only as a sorcery.", "Legendary Creature — Human"), [{
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "lock-or-unlock-door", what: room }] }], restriction: { restriction: "sorcery" } }]);
  });

  it("stays distinct from unlocking only", () => {
    const [ability] = abilities("{T}: Unlock a locked door of target Room you control.", "Artifact");
    assert.deepEqual((ability as { effects: { steps: unknown[] }[] }).effects[0].steps, [{ effect: "unlock-door", what: room }]);
  });

  it("rejects locking alone and a non-Room permanent", () => {
    rejects("{T}: Lock a door of target Room you control.", "Artifact");
    rejects("{T}: Lock or unlock a door of target artifact you control.", "Artifact");
  });
});

// Scryfall: Power Artifact. CR 602.1, 118.7.
describe("an attached permanent's activated abilities cost less", () => {
  it("scopes the reduction to abilities of the enchanted artifact", () => {
    assert.deepEqual(abilities("Enchanted artifact's activated abilities cost {2} less to activate. This effect can't reduce the mana in that cost to less than one mana.", "Enchantment — Aura"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "activate", source: { ref: "enchanted", noun: "artifact" }, abilities: { ref: "each", filter: { cls: "ability" } } },
        change: { direction: "reduce", symbols: ["2"] }, minimumTotalMana: { amount: "fixed", value: 1 } } }]);
  });

  it("still rejects an unqualified possessive source", () => {
    rejects("This artifact's activated abilities cost {2} less to activate.", "Artifact");
  });
});

// Scryfall: Hierophant Bio-Titan. CR 118.8, 601.2f.
describe("cost reduction for each counter removed by the additional cost", () => {
  it("counts the counters removed as this spell's additional cost", () => {
    const [ability] = abilities("As an additional cost to cast this spell, you may remove any number of +1/+1 counters from among creatures you control. This spell costs {2} less to cast for each counter removed this way.", "Creature — Tyranid");
    assert.deepEqual((ability as { costModifications: unknown[] }).costModifications, [{ change: { direction: "reduce", symbols: ["2"] },
      multiplier: { amount: "event-quantity", event: "counter-removal", counter: "any", scope: "this-way" } }]);
  });

  it("is not a count without a preceding removal", () => {
    rejects("This spell costs {2} less to cast for each counter removed this way.", "Creature — Tyranid");
  });
});

// Scryfall: Iron Man, Bleeding Edge; Donal, Herald of Wings; The Clone Saga. CR 707.9a–d, 707.10, 603.2h.
describe("copying a spell with characteristic exceptions", () => {
  const cast = (what: object) => ({ trigger: "cast", who: you, what });
  const copy = (copyExceptions: object[]) => ({ effect: "copy-stack-object", what: { ref: "trigger-event-object", event: "cast" }, who: you,
    count: { amount: "fixed", value: 1 }, copyExceptions, optional: true });

  it("removes the legendary supertype from the copy", () => {
    assert.deepEqual(abilities("Whenever you cast an artifact spell, you may copy it, except the copy isn't legendary. Do this only once each turn.", "Legendary Creature — Human Hero"), [{
      kind: "triggered", trigger: cast({ types: ["artifact"], cls: "spell" }),
      effects: [{ steps: [copy([{ change: "types", types: { supertypes: { operation: "remove", types: ["legendary"] } } }])] }],
      actionLimit: { maximum: 1, period: "turn" } }]);
  });

  it("adds a type and sets power and toughness on the copy", () => {
    const [ability] = abilities("Whenever you cast a nonlegendary creature spell with flying, you may copy it, except the copy is a 1/1 Spirit in addition to its other types. Do this only once each turn.", "Legendary Creature — Human Wizard");
    const step = (ability as { effects: { steps: unknown[] }[] }).effects[0].steps[0];
    assert.deepEqual(step, copy([{ change: "power-toughness", power: { amount: "fixed", value: 1 }, toughness: { amount: "fixed", value: 1 } },
      { change: "types", types: { subtypes: [{ family: "creature", operation: "add", types: ["Spirit"] }] } }]));
  });

  it("rejects a copy exception that gains an ability or names no subject", () => {
    rejects("Whenever you cast an artifact spell, you may copy it, except the copy has flying.", "Legendary Creature — Human Hero");
    rejects("Whenever you cast an artifact spell, you may copy it, except it isn't legendary.", "Legendary Creature — Human Hero");
    rejects("Whenever you cast an artifact spell, you may copy it, except the copy isn't legendary and draws a card.", "Legendary Creature — Human Hero");
  });
});

// Scryfall: Shadow of Mortality. CR 608.2c, 119.
describe("cost reduction by the difference a condition compares", () => {
  it("defines X as starting life total minus current life total", () => {
    assert.deepEqual(abilities("If your life total is less than your starting life total, this spell costs {X} less to cast, where X is the difference.", "Creature — Avatar"), [{
      kind: "static", effect: { static: "conditional",
        condition: { condition: "compare", value: { amount: "life-total", of: you }, comparison: { op: "lt", value: { amount: "starting-life-total", of: you } } },
        effect: { static: "cost-modification", scope: thisSpell, change: { direction: "reduce", symbols: ["X"] },
          bindings: [{ variable: "x", value: { amount: "minus", a: { amount: "starting-life-total", of: you }, b: { amount: "life-total", of: you } } }] } } }]);
  });

  it("does not define 'the difference' without a comparing condition", () => {
    rejects("This spell costs {X} less to cast, where X is the difference.", "Creature — Avatar");
  });
});

// Scryfall: Blaster Hulk, Izzet Generatorium. CR 107.14.
describe("energy paid or lost this turn", () => {
  const spent = { amount: "energy-paid-or-lost", who: you, during: "this-turn" };

  it("reduces cost for each energy counter paid or lost", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast for each {E} (energy counter) you've paid or lost this turn.", "Creature — Robot"), [{
      kind: "static", effect: { static: "cost-modification", scope: thisSpell, change: reduceOne, multiplier: spent } }]);
  });

  it("compares the energy paid or lost in an activation condition", () => {
    const [ability] = abilities("{T}: Draw a card. Activate only if you've paid or lost four or more {E} this turn.", "Artifact");
    assert.deepEqual((ability as { restriction: unknown }).restriction, { restriction: "condition", condition: { condition: "compare", value: spent,
      comparison: { op: "ge", value: { amount: "fixed", value: 4 } } } });
  });

  it("rejects paying only, another player, or another period", () => {
    rejects("{T}: Draw a card. Activate only if you've paid four or more {E} this turn.", "Artifact");
    rejects("{T}: Draw a card. Activate only if you've paid or lost four or more {E} this game.", "Artifact");
    rejects("This spell costs {1} less to cast for each {E} you've paid or lost this game.", "Creature — Robot");
  });
});
