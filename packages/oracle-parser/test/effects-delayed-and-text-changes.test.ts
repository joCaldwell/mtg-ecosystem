import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, PlayerRef, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. Rules text (2026-09-25):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };
const n = (value: number) => ({ amount: "fixed", value }) as const;
const target = (filter: object): ObjectRef => ({ ref: "target", filter, count: n(1) });
const thisCreature: ObjectRef = { ref: "this", noun: "creature" };
const spellOrAbility = { allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }] };

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

describe("explicit-next delayed triggers created by a spell line (CR 603.7b)", () => {
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "your" } as const;
  it("keeps the Pact payment and its lose-the-game consequence inside one next-upkeep trigger", () => {
    // Pact of Negation, Slaughter Pact, Summoner's Pact, Intervention Pact, Pact of the Titan.
    const loses: Effect = { effect: "lose-game", who: you };
    assert.deepEqual(spell("At the beginning of your next upkeep, pay {3}{U}{U}. If you don't, you lose the game."), [{ steps: [{
      effect: "create-delayed-trigger", trigger: upkeep, occurrence: "next", nextOnly: true, targets: "delayed-ability",
      effects: [
        { steps: [{ effect: "pay-cost", who: you, costs: [{ cost: "mana", symbols: ["3", "U", "U"] }] }] },
        { steps: [loses], condition: { condition: "you-dont" }, conditionKind: "if" },
      ],
    }] }]);
    // Glorious End.
    assert.deepEqual(spell("At the beginning of your next end step, you lose the game."), [{ steps: [{
      effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      occurrence: "next", nextOnly: true, targets: "delayed-ability", effects: [{ steps: [loses] }],
    }] }]);
  });
  it("does not turn recurring or permanent-printed phase text into a one-shot delayed trigger", () => {
    // Without "next" the same words are an ordinary recurring trigger.
    assert.deepEqual(abilities("At the beginning of your upkeep, you lose the game.", "Enchantment"), [{
      kind: "triggered", trigger: upkeep, effects: [{ steps: [{ effect: "lose-game", who: you }] }],
    }]);
    // A permanent has no resolving spell line that could create the trigger.
    rejected("At the beginning of your next upkeep, pay {3}{U}{U}. If you don't, you lose the game.", "Enchantment");
    rejected("At the beginning of your first upkeep, you lose the game.");
    rejected("At the beginning of next turn, you lose the game.");
  });
});

describe("changing the targets of a spell or ability (CR 115.7a-c, 115.9c)", () => {
  it("changes the single target of a single-target spell, all or nothing", () => {
    // Deflection, Misdirection, Swerve, Shunt, Ricochet Trap.
    assert.deepEqual(spell("Change the target of target spell with a single target."), [{ steps: [{
      effect: "change-targets", what: target({ cls: "spell", singleTarget: true }), who: you, scope: "all",
    }] }]);
    // Bolt Bend, Redirect Lightning; Reroute names only activated abilities.
    assert.deepEqual(spell("Change the target of target spell or ability with a single target."), [{ steps: [{
      effect: "change-targets", what: target({ ...spellOrAbility, singleTarget: true }), who: you, scope: "all",
    }] }]);
    assert.deepEqual(spell("Change the target of target activated ability with a single target."), [{ steps: [{
      effect: "change-targets", what: target({ cls: "ability", abilityKind: "activated", singleTarget: true }), who: you, scope: "all",
    }] }]);
    // Divert: the affected spell's controller may pay to stop the change.
    assert.deepEqual(spell("Change the target of target spell with a single target unless that spell's controller pays {2}."), [{
      steps: [{ effect: "change-targets", what: target({ cls: "spell", singleTarget: true }), who: you, scope: "all" }],
      condition: { condition: "pays", who: { player: "controller", of: { ref: "that", filter: { cls: "spell" } } }, cost: [{ cost: "mana", symbols: ["2"] }] },
      conditionKind: "unless",
    }]);
  });
  it("distinguishes the target(s), a target and any targets, and keeps optionality", () => {
    // Goblin Flectomancer: every target or none (115.7a).
    const instantOrSorcery = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" };
    assert.deepEqual(spell("You may change the targets of target instant or sorcery spell."), [{ steps: [{
      effect: "change-targets", what: target(instantOrSorcery), who: you, scope: "all", optional: true,
    }] }]);
    // Sideswipe: any number of them (115.7c).
    assert.deepEqual(spell("You may change any targets of target Arcane spell."), [{ steps: [{
      effect: "change-targets", what: target({ subtypes: ["Arcane"], cls: "spell" }), who: you, scope: "any", optional: true,
    }] }]);
    // Spellskite: one of them, and only to this creature (115.7b).
    assert.deepEqual(abilities("{U/P}: Change a target of target spell or ability to this creature.", "Artifact Creature — Phyrexian Horror"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["U/P"] }], effects: [{ steps: [{
        effect: "change-targets", what: target(spellOrAbility), who: you, scope: "one", to: thisCreature,
      }] }],
    }]);
  });
  it("keeps the targets-only restriction and the new-target requirement", () => {
    // Rebound.
    assert.deepEqual(spell("Change the target of target spell that targets only a player. The new target must be a player."), [{ steps: [{
      effect: "change-targets", what: target({ cls: "spell", targetsOnly: { player: "a-player" } }), who: you, scope: "all",
      newTarget: { kind: "player" },
    }] }]);
    // Silver Wyvern.
    assert.deepEqual(abilities("{U}: Change the target of target spell or ability that targets only this creature. The new target must be a creature.", "Creature — Drake"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["U"] }], effects: [{ steps: [{
        effect: "change-targets", what: target({ ...spellOrAbility, targetsOnly: thisCreature }), who: you, scope: "all",
        newTarget: { kind: "object", filter: { types: ["creature"] } },
      }] }],
    }]);
    // Muck Drubb: "a single creature" restates the one-target rule of 115.9c.
    assert.deepEqual(abilities("When this creature enters, change the target of target spell that targets only a single creature to this creature.", "Creature — Beast"), [{
      kind: "triggered", trigger: { trigger: "enters", what: thisCreature }, effects: [{ steps: [{
        effect: "change-targets", what: target({ cls: "spell", targetsOnly: { ref: "filter", filter: { types: ["creature"] }, count: n(1) } }),
        who: you, scope: "all", to: thisCreature,
      }] }],
    }]);
  });
  it("stays distinct from choosing new targets and rejects objects without targets", () => {
    assert.deepEqual(spell("You may choose new targets for target spell."), [{ steps: [{
      effect: "choose-new-targets", who: you, what: target({ cls: "spell" }), timing: "resolution", optional: true,
    }] }]);
    // "that targets" (any of its targets) is a different filter from "targets only".
    assert.deepEqual(spell("Counter target spell that targets a player."), [{ steps: [{
      effect: "counter", what: target({ cls: "spell", targets: { player: "a-player" } }),
    }] }]);
    rejected("Change the target of target creature.");
    rejected("Change target of target spell.");
    rejected("Destroy target creature with a single target.");
    // Meddle's conditional retarget has no representation yet.
    rejected("If target spell has only one target and that target is a creature, change that spell's target to another creature.");
  });
});

describe("text-changing effects (CR 612, 613.1c)", () => {
  const spellOrPermanent = target({ allOf: [{ anyOf: [{ cls: "spell" }, { cls: "permanent" }] }] });
  it("replaces one chosen word of a category with another, indefinitely or for a duration", () => {
    // Sleight of Mind, Alter Reality, Glamerdye.
    assert.deepEqual(spell("Change the text of target spell or permanent by replacing all instances of one color word with another."), [{ steps: [{
      effect: "change-text", what: spellOrPermanent, who: you, replacements: [{ category: "color-word" }],
    }] }]);
    // Magical Hack.
    assert.deepEqual(spell("Change the text of target spell or permanent by replacing all instances of one basic land type with another."), [{ steps: [{
      effect: "change-text", what: spellOrPermanent, who: you, replacements: [{ category: "basic-land-type" }],
    }] }]);
    // Mind Bend offers one of two replacements; Whim of Volrath adds a duration.
    const either = [{ category: "color-word" }, { category: "basic-land-type" }];
    assert.deepEqual(spell("Change the text of target permanent by replacing all instances of one color word with another or one basic land type with another."), [{ steps: [{
      effect: "change-text", what: target({ cls: "permanent" }), who: you, replacements: either,
    }] }]);
    assert.deepEqual(spell("Change the text of target permanent by replacing all instances of one color word with another or one basic land type with another until end of turn."), [{ steps: [{
      effect: "change-text", what: target({ cls: "permanent" }), who: you, replacements: either, duration: { duration: "end-of-turn" },
    }] }]);
  });
  it("keeps a printed new word and an excluded new word", () => {
    // Artificial Evolution.
    assert.deepEqual(spell("Change the text of target spell or permanent by replacing all instances of one creature type with another. The new creature type can't be Wall."), [{ steps: [{
      effect: "change-text", what: spellOrPermanent, who: you, replacements: [{ category: "creature-type", excluding: ["Wall"] }],
    }] }]);
    // New Blood.
    assert.deepEqual(spell("Gain control of target creature. Change the text of that creature by replacing all instances of one creature type with Vampire."), [
      { steps: [{ effect: "gain-control", who: you, what: target({ types: ["creature"] }) }] },
      { steps: [{ effect: "change-text", what: { ref: "that", filter: { types: ["creature"] } }, who: you,
        replacements: [{ category: "creature-type", to: "Vampire" }] }] },
    ]);
  });
  it("rejects incomplete or unknown replacements", () => {
    rejected("Change the text of target permanent by replacing all instances of one color word.");
    rejected("Change the text of target permanent by replacing all instances of one color word with another or one creature type.");
    rejected("Change the text of target permanent by replacing all instances of one card name with another.");
    rejected("Change the text of target permanent by replacing all instances of one creature type with Notatype.");
    // The exclusion sentence belongs only to a chosen creature type.
    rejected("Change the text of target permanent by replacing all instances of one color word with another. The new creature type can't be Wall.");
  });
});

describe("effects on the next spell a player casts (CR 611.2f)", () => {
  const thisTurn = { duration: "this-turn" } as const;
  const nextSpell = (filter: object, modifier: object): Effect => ({ effect: "modify-spells-cast",
    scope: { action: "cast", spells: { ref: "each", filter }, caster: you, occurrence: { next: true } },
    modifiers: [modifier], duration: thisTurn } as Effect);
  const has = (keyword: object) => ({ modifier: "abilities", abilities: [{ gained: "keyword", keyword }] });
  it("gives the next matching spell a keyword or makes it uncounterable", () => {
    // Wand of the Worldsoul / Flockchaser Phantom.
    assert.deepEqual(spell("The next spell you cast this turn has convoke."), [{ steps: [nextSpell({ cls: "spell" }, has({ keyword: "convoke" }))] }]);
    // Saheeli, the Gifted / Don & Raph, Hard Science.
    assert.deepEqual(spell("The next noncreature spell you cast this turn has affinity for artifacts."), [{ steps: [
      nextSpell({ nonTypes: ["creature"], cls: "spell" }, has({ keyword: "affinity", filter: { types: ["artifact"] } })),
    ] }]);
    // Overmaster (first sentence), Insist, Mistrise Village.
    assert.deepEqual(spell("The next instant or sorcery spell you cast this turn can't be countered. Draw a card."), [
      { steps: [nextSpell({ allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" }, { modifier: "cant", action: "be-countered" })] },
      { steps: [{ effect: "draw", who: you, amount: n(1) }] },
    ]);
  });
  it("creates one pending effect per named next spell", () => {
    // Smoldering Stagecoach.
    assert.deepEqual(abilities("Whenever this Vehicle attacks, the next instant spell and the next sorcery spell you cast this turn each have cascade.", "Artifact — Vehicle"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "vehicle" } }, effects: [{ steps: [{ effect: "simultaneous", steps: [
        nextSpell({ types: ["instant"], cls: "spell" }, has({ keyword: "cascade" })),
        nextSpell({ types: ["sorcery"], cls: "spell" }, has({ keyword: "cascade" })),
      ] }] }],
    }]);
  });
  it("uses the same pending cast scope as the existing next-spell cost reduction", () => {
    assert.deepEqual(spell("The next spell you cast this turn costs {2} less to cast."), [{ steps: [{
      effect: "cost-modification", scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: you, occurrence: { next: true } },
      change: { direction: "reduce", symbols: ["2"] }, duration: thisTurn,
    }] }]);
  });
  it("rejects a missing window, a mismatched 'each', and non-keyword grants", () => {
    rejected("The next spell you cast has epic.");
    rejected("The next spell you cast this turn each have cascade.");
    rejected("The next instant spell and the next sorcery spell you cast this turn has cascade.");
    rejected("The next spell you cast this turn has \"When you cast this spell, draw a card.\"");
    rejected("The next creature you cast this turn has haste.");
  });
});

describe("anchor words (CR 614.12c, 607.2m)", () => {
  const upkeep = { trigger: "phase", phase: "upkeep", whose: "your" } as const;
  const thisEnchantment: ObjectRef = { ref: "this", noun: "enchantment" };
  it("keeps each bulleted ability under its anchor word, not as a mode", () => {
    // Palace Siege.
    assert.deepEqual(abilities("As this enchantment enters, choose Khans or Dragons.\n• Khans — At the beginning of your upkeep, return target creature card from your graveyard to your hand.\n• Dragons — At the beginning of your upkeep, each opponent loses 2 life and you gain 2 life.", "Enchantment"), [{
      kind: "anchor-words", what: thisEnchantment, who: you, options: [
        { word: "Khans", ability: { kind: "triggered", trigger: upkeep, effects: [{ steps: [{ effect: "return-to-hand", to: "your",
          what: target({ types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }) }] }] } },
        { word: "Dragons", ability: { kind: "triggered", trigger: upkeep, effects: [{ steps: [
          { effect: "lose-life", who: { player: "each-opponent" }, amount: n(2) }, { effect: "gain-life", who: you, amount: n(2) },
        ] }] } },
      ],
    }]);
    // Frostcliff Siege: a static ability under an anchor word; any permanent type.
    assert.deepEqual(abilities("As this creature enters, choose Jeskai or Temur.\n• Jeskai — At the beginning of your upkeep, draw a card.\n• Temur — Creatures you control have haste.", "Creature — Elf"), [{
      kind: "anchor-words", what: thisCreature, who: you, options: [
        { word: "Jeskai", ability: { kind: "triggered", trigger: upkeep, effects: [{ steps: [{ effect: "draw", who: you, amount: n(1) }] }] } },
        { word: "Temur", ability: { kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "you" } },
          modifiers: [{ modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] } } },
      ],
    }]);
  });
  it("leaves an as-enters value choice without bullets alone", () => {
    // Roots of Life: a chosen land type, no anchor words.
    assert.deepEqual(abilities("As this enchantment enters, choose Island or Swamp.", "Enchantment"), [{ kind: "static", effect: {
      static: "as-enters", what: thisEnchantment, effects: [{ steps: [{ effect: "choose-value", who: you, category: "type",
        options: [{ type: "subtype", value: "Island" }, { type: "subtype", value: "Swamp" }] }] }],
    } }]);
  });
  it("rejects mismatched labels, missing bullets, spell text and unparsed bullets", () => {
    const header = "As this enchantment enters, choose Khans or Dragons.";
    const khans = "• Khans — At the beginning of your upkeep, draw a card.";
    const dragons = "• Dragons — At the beginning of your upkeep, you gain 2 life.";
    assert.equal(abilities(`${header}\n${khans}\n${dragons}`, "Enchantment")[0].kind, "anchor-words");
    rejected(`${header}\n${dragons}\n${khans}`, "Enchantment");
    rejected(`${header}\n${khans}`, "Enchantment");
    rejected(`${header}\n${khans}\n• Mardu — At the beginning of your upkeep, you gain 2 life.`, "Enchantment");
    rejected(`${header}\n${khans}\n• Dragons — You gain 2 life.`, "Enchantment");
    rejected(`${header}\n${khans}\n• Dragons — At the beginning of your upkeep, frobnicate.`, "Enchantment");
    rejected(`${header}\n${khans}\n• At the beginning of your upkeep, you gain 2 life.`, "Enchantment");
    rejected("As this enchantment enters, choose Khans, Dragons, or Mardu.\n" + khans + "\n" + dragons + "\n• Mardu — At the beginning of your upkeep, you gain 2 life.", "Enchantment");
  });
});

describe("kicker and devour history quantities (CR 702.33, 702.82b)", () => {
  const it_: ObjectRef = { ref: "it" };
  const enters = { trigger: "enters", what: thisCreature } as const;
  const kicks = { amount: "kicker-count", of: it_ } as const;
  const body = (text: string) => {
    const [ability] = abilities(text, "Creature — Elemental");
    assert.equal(ability.kind, "triggered");
    assert.deepEqual((ability as Extract<Ability, { kind: "triggered" }>).trigger, enters);
    return (ability as Extract<Ability, { kind: "triggered" }>).effects;
  };
  it("scales one instruction by the number of times the object was kicked", () => {
    // Lightkeeper of Emeria.
    assert.deepEqual(body("When this creature enters, you gain 2 life for each time it was kicked."), [{ steps: [{
      effect: "gain-life", who: you, amount: { amount: "product", factors: [n(2), kicks] },
    }] }]);
    // Bloodhusk Ritualist: one discard of that many cards.
    assert.deepEqual(body("When this creature enters, target opponent discards a card for each time it was kicked."), [{ steps: [{
      effect: "discard", who: { player: "target-opponent" },
      what: { ref: "filter", filter: { cls: "card" }, count: { amount: "product", factors: [n(1), kicks] } },
    }] }]);
    // Deathforge Shaman: the same quantity in amount position.
    assert.deepEqual(body("When this creature enters, it deals damage to any target equal to twice the number of times it was kicked."), [{ steps: [{
      effect: "damage", source: it_, amount: { amount: "twice", of: kicks }, to: { to: "any-target" },
    }] }]);
  });
  it("counts the permanents the object devoured as a relation on the counted filter", () => {
    const devoured = { relation: "devoured", by: it_ };
    // Skullmulcher.
    assert.deepEqual(body("When this creature enters, draw a card for each creature it devoured."), [{ steps: [{
      effect: "draw", who: you, amount: n(1), forEach: { types: ["creature"], relation: devoured },
    }] }]);
    // Voracious Dragon.
    assert.deepEqual(body("When this creature enters, it deals damage to any target equal to twice the number of Goblins it devoured."), [{ steps: [{
      effect: "damage", source: it_, amount: { amount: "twice", of: { amount: "count", of: { subtypes: ["Goblin"], relation: devoured } } },
      to: { to: "any-target" },
    }] }]);
  });
  it("rejects kicks or devouring attributed to something not yet identified", () => {
    rejected("When this creature enters, you gain 2 life for each time a creature was kicked.", "Creature — Elemental");
    rejected("When this creature enters, you gain 2 life for each time it was cast.", "Creature — Elemental");
    rejected("When this creature enters, draw a card for each creature target creature devoured.", "Creature — Elemental");
    rejected("When this creature enters, draw a card for each creature it devoured this turn.", "Creature — Elemental");
  });
});

describe("unpreventable damage rider (CR 608.2c, 615.12)", () => {
  const selfRef: ObjectRef = { ref: "self" };
  const targetCreature = target({ types: ["creature"] });
  it("marks the damage of the instruction the rider follows", () => {
    // Pinpoint Avalanche, Combust.
    assert.deepEqual(spell("~ deals 4 damage to target creature. The damage can't be prevented."), [{ steps: [{
      effect: "damage", source: selfRef, amount: n(4), to: { to: "object", ref: targetCreature }, unpreventable: true,
    }] }]);
    // The coordinated clause form used by Urza's Rage's replacement.
    assert.deepEqual(spell("~ deals 10 damage to target creature and the damage can't be prevented."), [{ steps: [{
      effect: "damage", source: selfRef, amount: n(10), to: { to: "object", ref: targetCreature }, unpreventable: true,
    }] }]);
    // Without the rider the same instruction is ordinary damage.
    assert.deepEqual(spell("~ deals 4 damage to target creature."), [{ steps: [{
      effect: "damage", source: selfRef, amount: n(4), to: { to: "object", ref: targetCreature },
    }] }]);
  });
  it("keeps the rider on the replacing instruction of a kicker self-replacement", () => {
    // Urza's Rage.
    const [sentence] = spell("~ deals 3 damage to any target. If this spell was kicked, instead it deals 10 damage to that permanent or player and the damage can't be prevented.");
    const [step] = sentence.steps;
    assert.equal(step.effect, "self-replacement");
    const replacement = step as Extract<Effect, { effect: "self-replacement" }>;
    assert.deepEqual(replacement.original, { effect: "damage", source: selfRef, amount: n(3), to: { to: "any-target" } });
    assert.deepEqual(replacement.instead, [{ effect: "damage", source: { ref: "it" }, amount: n(10),
      to: { to: "object", ref: { ref: "that", filter: { cls: "permanent", orPlayer: true } } }, unpreventable: true }]);
  });
  it("stays distinct from the turn-long rule and needs a damage instruction to attach to", () => {
    // Flaring Pain's rule concerns all damage this turn: a prohibition, not a rider.
    assert.deepEqual(spell("Damage can't be prevented this turn."), [{ steps: [{
      effect: "prohibit", prohibition: { prohibition: "prevent-damage", kind: "all" }, duration: { duration: "this-turn" },
    }] }]);
    rejected("Draw a card. The damage can't be prevented.");
    rejected("~ deals 4 damage to target creature and you gain 4 life. The damage can't be prevented.");
    rejected("Destroy target creature and the damage can't be prevented.");
    rejected("The damage can't be prevented.");
    // Banefire's conditional rider on another paragraph's damage is not represented.
    rejected("If X is 5 or more, this spell can't be countered and the damage can't be prevented.", "Sorcery");
  });
});

describe("modal reflexive triggers (CR 603.12, 700.2b)", () => {
  const draw: Effect = { effect: "draw", who: you, amount: n(1) };
  const treasure: Effect = { effect: "create-token", count: n(1), token: { predefined: "Treasure" } };
  it("gives the bullets to the reflexive ability that the header creates", () => {
    // Gorbag of Minas Morgul (trigger simplified to an attack trigger).
    assert.deepEqual(abilities("Whenever this creature attacks, you may sacrifice it. When you do, choose one —\n• Draw a card.\n• Create a Treasure token.", "Creature — Orc"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: thisCreature }, effects: [
        { steps: [{ effect: "sacrifice", who: you, what: { ref: "it" }, optional: true }] },
        { steps: [{ effect: "create-reflexive-trigger", event: { event: "preceding-action", who: you, performed: true }, effects: [{ steps: [{
          effect: "modal", count: { exactly: 1 }, options: [{ effects: [{ steps: [draw] }] }, { effects: [{ steps: [treasure] }] }],
        }] }] }] },
      ],
    }]);
  });
  it("still rejects bullets that no modal instruction consumes", () => {
    rejected("Whenever this creature attacks, you may sacrifice it. When you do, draw a card.\n• Draw a card.\n• Create a Treasure token.", "Creature — Orc");
    rejected("Whenever this creature attacks, draw a card.\n• Draw a card.\n• Create a Treasure token.", "Creature — Orc");
  });
});

describe("modal header riders (CR 601.2b, 700.2a-b)", () => {
  const drawTwo = (who: PlayerRef): Effect => ({ effect: "draw", who, amount: n(2) });
  const destroyArtifact: Effect = { effect: "destroy", what: target({ types: ["artifact"] }) };
  const targetPlayer: PlayerRef = { player: "target-player" };
  const modal = (text: string) => {
    const [sentence] = spell(text);
    assert.equal(sentence.steps.length, 1);
    return sentence.steps[0];
  };
  const controlsWizard = { condition: "control", who: you, what: { ref: "filter", filter: { subtypes: ["Wizard"] }, count: n(1) } };
  it("keeps a conditional larger mode count, optional or mandatory", () => {
    // Flame of Anor: three modes, "you may choose two instead".
    assert.deepEqual(modal("Choose one. If you control a Wizard as you cast this spell, you may choose two instead.\n• Target player draws two cards.\n• Destroy target artifact.\n• You draw two cards."), {
      effect: "modal", count: { exactly: 1 }, conditionalCount: { condition: controlsWizard, count: { min: 1, max: 2 } },
      options: [{ effects: [{ steps: [drawTwo(targetPlayer)] }] }, { effects: [{ steps: [destroyArtifact] }] }, { effects: [{ steps: [drawTwo(you)] }] }],
    });
    // "choose both instead" (Prophetic Titan's wording) requires both modes.
    assert.deepEqual(modal("Choose one. If you control a Wizard, choose both instead.\n• Target player draws two cards.\n• Destroy target artifact."), {
      effect: "modal", count: { exactly: 1 }, conditionalCount: { condition: controlsWizard, count: { exactly: 2 } },
      options: [{ effects: [{ steps: [drawTwo(targetPlayer)] }] }, { effects: [{ steps: [destroyArtifact] }] }],
    });
    // The unconditional header is unchanged.
    assert.deepEqual(modal("Choose one —\n• Target player draws two cards.\n• Destroy target artifact."), {
      effect: "modal", count: { exactly: 1 },
      options: [{ effects: [{ steps: [drawTwo(targetPlayer)] }] }, { effects: [{ steps: [destroyArtifact] }] }],
    });
  });
  it("records that each chosen mode must target a different player", () => {
    // Donnie & April, Adorkable Duo (second mode simplified).
    assert.deepEqual(modal("Choose one or both. Each mode must target a different player.\n• Target player draws two cards.\n• Target opponent draws two cards."), {
      effect: "modal", count: { min: 1, max: 2 }, distinctTargetPlayers: true,
      options: [{ effects: [{ steps: [drawTwo(targetPlayer)] }] }, { effects: [{ steps: [drawTwo({ player: "target-opponent" })] }] }],
    });
  });
  it("rejects malformed upgrades and modes without a targeted player", () => {
    const two = "\n• Target player draws two cards.\n• Destroy target artifact.";
    rejected("Choose one. If you control a Wizard, you may choose both instead." + two + "\n• You draw two cards.");
    rejected("Choose one. If you control a Wizard, you may choose two." + two);
    rejected("Choose two. If you control a Wizard, choose one instead." + two);
    rejected("Choose one. If you control a Wizard, choose three instead." + two);
    rejected("Choose one. If you frobnicate, choose both instead." + two);
    rejected("Choose one or both. Each mode must target a different player." + two);
    rejected("Choose one or both. Each mode must target a different creature.\n• Target player draws two cards.\n• Target opponent draws two cards.");
  });
});

describe("one trailing duration over coordinated modifiers (CR 611.2a)", () => {
  const eot = { duration: "end-of-turn" } as const;
  const pump = (what: ObjectRef, p: number, t: number): Effect => ({ effect: "pump", what,
    power: { sign: 1, amount: n(p) }, toughness: { sign: 1, amount: n(t) } });
  const attacking = (color: string): ObjectRef => ({ ref: "each", filter: { status: ["attacking"], colors: [color], types: ["creature"] } } as ObjectRef);
  const targetCreature = target({ types: ["creature"] });
  it("reaches modifiers that each state their own subject", () => {
    // Agrus Kos, Wojek Veteran.
    assert.deepEqual(spell("Attacking red creatures get +2/+0 and attacking white creatures get +0/+2 until end of turn."), [{ steps: [
      { ...pump(attacking("red"), 2, 0), duration: eot }, { ...pump(attacking("white"), 0, 2), duration: eot },
    ] }]);
  });
  it("reaches every member of a comma list with an elided subject", () => {
    // Traitor's Clutch.
    assert.deepEqual(spell("Target creature gets +1/+0, becomes black, and gains shadow until end of turn."), [{ steps: [
      { ...pump(targetCreature, 1, 0), duration: eot },
      { effect: "become", what: targetCreature, spec: { colors: ["black"] }, duration: eot },
      { effect: "gain-abilities", what: targetCreature, abilities: [{ gained: "keyword", keyword: { keyword: "shadow" } }], duration: eot },
    ] }]);
  });
  it("does not lend a duration to one-shot actions or across a stated duration", () => {
    assert.deepEqual(spell("Tap target creature and it gains flying until end of turn."), [{ steps: [
      { effect: "tap", what: targetCreature },
      { effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }], duration: eot },
    ] }]);
    const endOfCombat = { duration: "end-of-combat" } as const;
    assert.deepEqual(spell("Attacking red creatures get +2/+0 until end of combat and attacking white creatures get +0/+2 until end of turn."), [{ steps: [
      { ...pump(attacking("red"), 2, 0), duration: endOfCombat }, { ...pump(attacking("white"), 0, 2), duration: eot },
    ] }]);
  });
});

describe("continuations of a reflexive ability's body (CR 603.12)", () => {
  const eot = { duration: "end-of-turn" } as const;
  const attack = { trigger: "attacks", what: thisCreature } as const;
  const discard: Effect = { effect: "discard", who: you, what: { ref: "filter", filter: { cls: "card" }, count: n(1) }, optional: true };
  const reflexive = (effects: Sentence[]): Sentence => ({ steps: [{ effect: "create-reflexive-trigger",
    event: { event: "preceding-action", who: you, performed: true }, effects }] });
  const trample = [{ gained: "keyword" as const, keyword: { keyword: "trample" } }];
  it("keeps a sentence that names the reflexive ability's own target inside that ability", () => {
    // Inti, Seneschal of the Sun (trigger simplified); Spined Tyrranax says "That creature".
    const counter: Effect = { effect: "put-counters", on: target({ types: ["creature"] }), counter: { type: "+1/+1" }, count: n(1) };
    assert.deepEqual(abilities("Whenever this creature attacks, you may discard a card. When you do, put a +1/+1 counter on target creature. It gains trample until end of turn.", "Creature — Elf"), [{
      kind: "triggered", trigger: attack, effects: [{ steps: [discard] }, reflexive([
        { steps: [counter] },
        { steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: trample, duration: eot }] },
      ])],
    }]);
    assert.deepEqual(abilities("Whenever this creature attacks, you may discard a card. When you do, put a +1/+1 counter on target creature. That creature gains trample until end of turn.", "Creature — Elf"), [{
      kind: "triggered", trigger: attack, effects: [{ steps: [discard] }, reflexive([
        { steps: [counter] },
        { steps: [{ effect: "gain-abilities", what: { ref: "that", filter: { types: ["creature"] } }, abilities: trample, duration: eot }] },
      ])],
    }]);
  });
  it("rejects following sentences with no proven dependency on the reflexive body", () => {
    const head = "Whenever this creature attacks, you may discard a card. When you do, ";
    // Nothing introduced by the body sentence.
    rejected(head + "you gain 3 life. It gains trample until end of turn.", "Creature — Elf");
    // A fresh target, or no anaphor at all.
    rejected(head + "put a +1/+1 counter on target creature. Target creature gains trample until end of turn.", "Creature — Elf");
    rejected(head + "put a +1/+1 counter on target creature. You gain 3 life.", "Creature — Elf");
    // Earthbender Ascension: an intervening "if" would have to govern both sentences.
    rejected(head + "if you control a Wizard, put a +1/+1 counter on target creature. It gains trample until end of turn.", "Creature — Elf");
    // A delayed (not reflexive) body keeps its stricter rule: Time to Feed.
    rejected("Choose target creature an opponent controls. When that creature dies this turn, you gain 3 life. Target creature you control fights that creature.", "Sorcery");
  });
});
