import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, CombatRule, ObjectFilter, ObjectRef, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 609.4 ("as
// though" applies only to the stated action), 702.3b, 302.6/702.10b,
// 509.1a–b, 510.1a–c.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed" as const, value });
const thisCreature = { ref: "this", noun: "creature" } as const;
const each = (filter: ObjectFilter): ObjectRef => ({ ref: "each", filter });
const you = { player: "you" } as const;

function line(text: string, typeLine = "Creature — Wall"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  // JSON form: the AST as a consumer serializes it (no undefined fields).
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const combatStatic = (rule: CombatRule, what: ObjectRef = thisCreature): Ability =>
  ({ kind: "static", effect: { static: "combat-rule", what, rule } });

const withoutDefender: CombatRule = { rule: "as-though", action: "attack",
  pretense: { pretend: "lacks-keyword", of: "affected-creature", keyword: { keyword: "defender" } } };
const withHaste: CombatRule = { rule: "as-though", action: "attack",
  pretense: { pretend: "has-keyword", of: "affected-creature", keyword: { keyword: "haste" } } };

describe("attacking as though without defender or with haste (CR 609.4, 702.3b, 302.6)", () => {
  it("states the pretense as a rule of the affected creatures", () => {
    // Animate Wall, High Alert, Instill Energy.
    assert.deepEqual(ability("Enchanted Wall can attack as though it didn't have defender.", "Enchantment — Aura"),
      combatStatic(withoutDefender, { ref: "enchanted", noun: "wall" }));
    assert.deepEqual(ability("Creatures you control can attack as though they didn't have defender.", "Enchantment"),
      combatStatic(withoutDefender, each({ types: ["creature"], control: "you" })));
    assert.deepEqual(ability("Enchanted creature can attack as though it had haste.", "Enchantment — Aura"),
      combatStatic(withHaste, { ref: "enchanted", noun: "creature" }));
  });

  it("keeps leading, trailing and unless conditions", () => {
    // Pillar of War, Ogre Jailbreaker, Chaos Lord.
    assert.deepEqual(ability("As long as this creature is enchanted, it can attack as though it didn't have defender."), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "status", what: thisCreature, status: "enchanted" },
        effect: { static: "combat-rule", what: { ref: "it" }, rule: withoutDefender } },
    });
    assert.deepEqual(ability("This creature can attack as though it didn't have defender as long as you control a Gate."), combatStatic({
      ...withoutDefender, conditionKind: "if",
      condition: { condition: "control", who: you, what: { ref: "filter", filter: { subtypes: ["Gate"] }, count: fixed(1) } },
    }));
    assert.deepEqual(ability("This creature can attack as though it had haste unless it entered this turn.", "Creature — Human"), combatStatic({
      ...withHaste, conditionKind: "unless",
      condition: { condition: "zone-change-history", event: "enters", subject: { match: "object", what: { ref: "it" } },
        to: { zone: "battlefield" }, during: "this-turn" },
    }));
  });

  it("carries the printed period of a resolving effect as its duration", () => {
    // Mirror Wall; Nivix Cyclops.
    assert.deepEqual(ability("{W}: This creature can attack this turn as though it didn't have defender."), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["W"] }],
      effects: [{ steps: [{ effect: "combat-rule", what: thisCreature, rule: withoutDefender, duration: { duration: "this-turn" } }] }],
    });
    const cyclops = ability("Whenever you cast an instant or sorcery spell, this creature gets +3/+0 until end of turn and can attack this turn as though it didn't have defender.", "Creature — Cyclops");
    assert.equal(cyclops.kind, "triggered");
    assert.deepEqual(cyclops.kind === "triggered" && cyclops.effects[0].steps[1],
      { effect: "combat-rule", what: thisCreature, rule: withoutDefender, duration: { duration: "this-turn" } });
  });

  it("joins the pretense with other modifiers of the same creature", () => {
    // Colossus of Akros; Arcades, the Strategist.
    assert.deepEqual(ability("As long as this creature is monstrous, it has trample and can attack as though it didn't have defender.", "Artifact Creature — Golem"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "status", what: thisCreature, status: "monstrous" },
        effect: { static: "modify", what: { ref: "it" }, modifiers: [
          { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "trample" } }] },
          { modifier: "combat-rule", rule: withoutDefender },
        ] } },
    });
    assert.deepEqual(ability("Each creature you control with defender assigns combat damage equal to its toughness rather than its power and can attack as though it didn't have defender.", "Legendary Creature — Elder Dragon"),
      combatStatic({ rule: "all-of", rules: [{ rule: "damage-assignment-value", value: "toughness" }, withoutDefender] },
        each({ types: ["creature"], control: "you", withAbility: ["defender"] })));
  });

  it("limits the pretense to the printed defenders", () => {
    // Frenzied Saddlebrute.
    assert.deepEqual(ability("All creatures can attack your opponents and planeswalkers your opponents control as though those creatures had haste.", "Creature — Orc Warrior"),
      combatStatic({ ...withHaste, defenders: [
        { defender: "player", who: { player: "each-opponent" } },
        { defender: "object", what: each({ types: ["planeswalker"], control: "opponent" }) },
      ] }, { ref: "all", filter: { types: ["creature"] } }));
  });

  it("rejects pretenses that attacking does not consult and unparsed defenders", () => {
    fails("This creature can attack as though it had flying.");
    fails("This creature can attack as though it didn't have haste.");
    fails("This creature can attack as though it had defender.");
    fails("This creature can attack as though it didn't have defender and.");
    // Weathered Sentinels: the player description is not represented.
    fails("This creature can attack players who attacked you during their last turn as though it didn't have defender.");
  });
});

describe("blocking and being blocked as though (CR 609.4, 509.1a–b)", () => {
  it("distinguishes a pretense about the blocker from one about the blocked creatures", () => {
    // Aetherflame Wall vs. Heartwood Dryad; Dragon Hunter.
    const shadowed = each({ types: ["creature"], withAbility: ["shadow"] });
    assert.deepEqual(ability("This creature can block creatures with shadow as though they didn't have shadow."), combatStatic({
      rule: "as-though", action: "block", attackers: shadowed,
      pretense: { pretend: "lacks-keyword", of: "blocked-creature", keyword: { keyword: "shadow" } },
    }));
    assert.deepEqual(ability("This creature can block creatures with shadow as though it had shadow."), combatStatic({
      rule: "as-though", action: "block", attackers: shadowed,
      pretense: { pretend: "has-keyword", of: "affected-creature", keyword: { keyword: "shadow" } },
    }));
    assert.deepEqual(ability("This creature can block Dragons as though it had reach."), combatStatic({
      rule: "as-though", action: "block", attackers: each({ subtypes: ["Dragon"] }),
      pretense: { pretend: "has-keyword", of: "affected-creature", keyword: { keyword: "reach" } },
    }));
  });

  it("resolves 'those abilities' only to the abilities the printed filter names", () => {
    // Street Savvy; Staff of the Ages.
    const savvy = ability("Enchanted creature gets +0/+2 and can block creatures with landwalk abilities as though they didn't have those abilities.", "Enchantment — Aura");
    assert.deepEqual(savvy.kind === "static" && savvy.effect.static === "modify" && savvy.effect.modifiers[1], {
      modifier: "combat-rule", rule: { rule: "as-though", action: "block",
        attackers: each({ types: ["creature"], withAbility: ["landwalk"] }),
        pretense: { pretend: "lacks-keyword", of: "blocked-creature", keyword: { keyword: "landwalk" } } },
    });
    assert.deepEqual(ability("Creatures with landwalk abilities can be blocked as though they didn't have those abilities.", "Artifact"), combatStatic({
      rule: "as-though", action: "be-blocked",
      pretense: { pretend: "lacks-keyword", of: "affected-creature", keyword: { keyword: "landwalk" } },
    }, each({ types: ["creature"], withAbility: ["landwalk"] })));
    fails("Creatures with flying can be blocked as though they didn't have those abilities.", "Artifact");
    fails("This creature can block creatures with flying as though they didn't have those abilities.");
  });

  it("represents blocking as though untapped", () => {
    // Masako the Humorless.
    assert.deepEqual(ability("Tapped creatures you control can block as though they were untapped.", "Legendary Creature — Human Advisor"),
      combatStatic({ rule: "as-though", action: "block", pretense: { pretend: "untapped", of: "affected-creature" } },
        each({ status: ["tapped"], types: ["creature"], control: "you" })));
  });

  it("rejects an ambiguous or meaningless pretense", () => {
    // "they" after blocked creatures must be those creatures losing the
    // very ability their filter requires; "it" must be the blocker.
    fails("This creature can block creatures with shadow as though they had shadow.");
    fails("This creature can block creatures with shadow as though they didn't have flying.");
    fails("This creature can block creatures with shadow as though it didn't have shadow.");
    fails("This creature can be blocked as though it had flying.");
    fails("This creature can block as though it had haste.");
    fails("This creature can block creatures with shadow as though they were untapped.");
  });
});

describe("combat damage assigned from another characteristic (CR 510.1a)", () => {
  it("names the characteristic used instead of power", () => {
    // Doran, the Siege Tower; Narod, the Beige Flower; Ancient Lumberknot.
    assert.deepEqual(ability("Each creature assigns combat damage equal to its toughness rather than its power.", "Legendary Creature — Treefolk Shaman"),
      combatStatic({ rule: "damage-assignment-value", value: "toughness" }, each({ types: ["creature"] })));
    assert.deepEqual(ability("Each creature assigns combat damage equal to its mana value rather than its power.", "Legendary Creature — Plant Rogue"),
      combatStatic({ rule: "damage-assignment-value", value: "mana-value" }, each({ types: ["creature"] })));
    assert.deepEqual(ability("Each creature you control with toughness greater than its power assigns combat damage equal to its toughness rather than its power.", "Creature — Treefolk"),
      combatStatic({ rule: "damage-assignment-value", value: "toughness" }, each({ types: ["creature"], control: "you",
        properties: [{ property: "toughness", comparison: { op: "gt", value: { amount: "attribute", of: { ref: "it" }, attribute: "power" } } }] })));
  });

  it("is a modifier beside a P/T change and a resolving effect with its duration", () => {
    // Gauntlets of Light; Plagon, Lord of the Beach; Bill the Pony.
    const gauntlets = ability("Enchanted creature gets +0/+2 and assigns combat damage equal to its toughness rather than its power.", "Enchantment — Aura");
    assert.deepEqual(gauntlets.kind === "static" && gauntlets.effect.static === "modify" && gauntlets.effect.modifiers[1],
      { modifier: "combat-rule", rule: { rule: "damage-assignment-value", value: "toughness" } });
    const target: ObjectRef = { ref: "target", filter: { types: ["creature"], control: "you" }, count: fixed(1) };
    assert.deepEqual(ability("{W/U}: Target creature you control assigns combat damage equal to its toughness rather than its power this turn.", "Legendary Creature — Starfish Wizard"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["W/U"] }],
      effects: [{ steps: [{ effect: "combat-rule", what: target, rule: { rule: "damage-assignment-value", value: "toughness" }, duration: { duration: "this-turn" } }] }],
    });
    const bill = ability("Sacrifice a Food: Until end of turn, target creature you control assigns combat damage equal to its toughness rather than its power.", "Legendary Creature — Horse");
    assert.deepEqual(bill.kind === "activated" && bill.effects[0].steps,
      [{ effect: "combat-rule", what: target, rule: { rule: "damage-assignment-value", value: "toughness" }, duration: { duration: "end-of-turn" } }]);
  });

  it("rejects other values and mismatched possessives", () => {
    fails("Each creature assigns combat damage equal to its toughness.", "Enchantment");
    fails("Each creature assigns combat damage equal to its power rather than its toughness.", "Enchantment");
    fails("Each creature assigns combat damage equal to its toughness rather than their power.", "Enchantment");
    // Loot, the Anomaly: a different counterfactual, not represented.
    fails("If ~'s power is negative, it assigns combat damage as though its power were positive.", "Legendary Creature — Beast");
  });
});

describe("assigning combat damage as though not blocked (CR 510.1b–c, 609.4)", () => {
  const unblocked = (chooser?: { player: "you" }): CombatRule => ({ rule: "assign-as-unblocked", ...(chooser ? { chooser } : {}) });

  it("keeps the player who decides", () => {
    // Thorn Elemental; Imaryll, Elfhame Elite; Indomitable Might.
    assert.deepEqual(ability("You may have this creature assign its combat damage as though it weren't blocked.", "Creature — Elemental"),
      combatStatic(unblocked(you)));
    assert.deepEqual(ability("You may have ~ assign its combat damage as though it weren't blocked.", "Legendary Creature — Elf"),
      combatStatic(unblocked(you), { ref: "self" }));
    const enchanted = { ref: "enchanted", noun: "creature" } as const;
    assert.deepEqual(ability("Enchanted creature's controller may have it assign its combat damage as though it weren't blocked.", "Enchantment — Aura"),
      combatStatic({ rule: "assign-as-unblocked", chooser: { player: "controller", of: enchanted } }, enchanted));
  });

  it("states the per-creature permission of the 'for each' wording", () => {
    // Invasion of Ikoria's back face; Siege Behemoth.
    assert.deepEqual(ability("For each non-Human creature you control, you may have that creature assign its combat damage as though it weren't blocked.", "Legendary Creature — Dinosaur"),
      combatStatic(unblocked(you), each({ nonSubtypes: ["Human"], types: ["creature"], control: "you" })));
    assert.deepEqual(ability("As long as this creature is attacking, for each creature you control, you may have that creature assign its combat damage as though it weren't blocked.", "Creature — Beast"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "status", what: thisCreature, status: "attacking" },
        effect: { static: "combat-rule", what: each({ types: ["creature"], control: "you" }), rule: unblocked(you) } },
    });
  });

  it("creates a lasting permission, not an optional resolution action", () => {
    // Gurzigost; Predatory Focus. CR 611.2a: "may" describes the later
    // assignment, exactly as in the static wording.
    assert.deepEqual(ability("{G}{G}, Discard a card: You may have this creature assign its combat damage this turn as though it weren't blocked.", "Creature — Beast"), {
      kind: "activated",
      costs: [{ cost: "mana", symbols: ["G", "G"] }, { cost: "discard", what: { ref: "filter", filter: { cls: "card" }, count: fixed(1) } }],
      effects: [{ steps: [{ effect: "combat-rule", what: thisCreature, rule: unblocked(you), duration: { duration: "this-turn" } }] }],
    });
    assert.deepEqual(ability("You may have creatures you control assign their combat damage this turn as though they weren't blocked.", "Sorcery"), {
      kind: "spell",
      effects: [{ steps: [{ effect: "combat-rule", what: each({ types: ["creature"], control: "you" }), rule: unblocked(you), duration: { duration: "this-turn" } }] }],
    });
  });

  it("is the quoted ability of a grant", () => {
    // Proud Wildbonder.
    const grant = ability("Creatures you control with trample have \"You may have this creature assign its combat damage as though it weren't blocked.\"", "Creature — Human Warrior");
    assert.deepEqual(grant.kind === "static" && grant.effect.static === "modify" && grant.effect.modifiers, [{
      modifier: "abilities", abilities: [{ gained: "quoted", ability: { kind: "static",
        effect: { static: "combat-rule", what: thisCreature, rule: unblocked(you) } } }],
    }]);
  });

  it("rejects number disagreement and other damage", () => {
    fails("You may have this creature assign their combat damage as though it weren't blocked.");
    fails("You may have this creature assign its combat damage as though they weren't blocked.");
    fails("You may have this creature assign its damage as though it weren't blocked.");
    fails("You may have this creature assign its combat damage as though it were blocked.");
    fails("For each creature you control, you may have this creature assign its combat damage as though it weren't blocked.");
  });
});
