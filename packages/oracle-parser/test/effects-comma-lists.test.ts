import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. Rules text (2026-09-25):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const one = { amount: "fixed", value: 1 };

function abilities(text: string, typeLine = "Instant"): Ability[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
/** Sentences of a one-line instant. */
function spell(text: string, typeLine = "Instant"): Sentence[] {
  const [ability] = abilities(text, typeLine);
  assert.equal(ability.kind, "spell");
  return (ability as Extract<Ability, { kind: "spell" }>).effects;
}
function rejected(text: string, typeLine = "Instant"): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("comma lists of independently headed descriptions (CR 608.2c)", () => {
  const activated = { cls: "ability", abilityKind: "activated" };
  const triggered = { cls: "ability", abilityKind: "triggered" };
  it("unions a spell and two ability kinds", () => {
    // Disallow.
    assert.deepEqual(spell("Counter target spell, activated ability, or triggered ability."), [{ steps: [{
      effect: "counter", what: { ref: "target", filter: { anyOf: [{ cls: "spell" }, activated, triggered] }, count: one },
    }] }]);
    // Louisoix's Sacrifice: each member keeps its own prenominal qualifier.
    assert.deepEqual(spell("Counter target activated ability, triggered ability, or noncreature spell."), [{ steps: [{
      effect: "counter", what: { ref: "target", filter: { anyOf: [activated, triggered, { nonTypes: ["creature"], cls: "spell" }] }, count: one },
    }] }]);
  });
  it("keeps each member's adjectives and postnominal qualifiers to itself", () => {
    // Nahiri, the Harbinger.
    assert.deepEqual(spell("Exile target enchantment, tapped artifact, or tapped creature.", "Sorcery"), [{ steps: [{
      effect: "exile", what: { ref: "target", filter: { anyOf: [
        { types: ["enchantment"] }, { status: ["tapped"], types: ["artifact"] }, { status: ["tapped"], types: ["creature"] },
      ] }, count: one },
    }] }]);
    // Thraben Exorcism.
    assert.deepEqual(spell("Exile target Spirit, creature with disturb, or enchantment."), [{ steps: [{
      effect: "exile", what: { ref: "target", filter: { anyOf: [
        { subtypes: ["Spirit"] }, { types: ["creature"], withAbility: ["disturb"] }, { types: ["enchantment"] },
      ] }, count: one },
    }] }]);
  });
  it("rejects a scope stated only on a later member, an unclosed list, and repeated articles", () => {
    rejected("Exile target enchantment, tapped artifact, or creature you control.");
    rejected("Counter target spell, activated ability, triggered ability.");
    rejected("Counter target spell, an activated ability, or a triggered ability.");
  });
});

describe("comma lists of coordinated objects closed by 'and' (CR 608.2c)", () => {
  const target = (types: string[], upTo = false) => ({ ref: "target", filter: { types }, count: one, ...(upTo ? { upTo: true } : {}) });
  it("is one simultaneous instruction per object, like the two-object form", () => {
    // Decimate.
    assert.deepEqual(spell("Destroy target artifact, target creature, target enchantment, and target land.", "Sorcery"), [{ steps: [{
      effect: "simultaneous", steps: ["artifact", "creature", "enchantment", "land"].map(type => ({ effect: "destroy", what: target([type]) })),
    }] }]);
    // Convert to Slime.
    assert.deepEqual(spell("Destroy up to one target artifact, up to one target creature, and up to one target enchantment.", "Sorcery"), [{ steps: [{
      effect: "simultaneous", steps: ["artifact", "creature", "enchantment"].map(type => ({ effect: "destroy", what: target([type], true) })),
    }] }]);
  });
  it("shares only the closing graveyard of target cards", () => {
    // Rise from the Wreck-like wording (Reconstruct History).
    const card = (type: string) => ({ ref: "target", filter: { types: [type], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: one, upTo: true });
    assert.deepEqual(spell("Return up to one target artifact card, up to one target enchantment card, and up to one target planeswalker card from your graveyard to your hand.", "Sorcery"), [{ steps: [{
      effect: "simultaneous", steps: ["artifact", "enchantment", "planeswalker"].map(type => ({ effect: "return-to-hand", what: card(type), to: "your" })),
    }] }]);
  });
  it("rejects a comma list without its closing 'and'", () => {
    rejected("Destroy target artifact, target creature, target enchantment.", "Sorcery");
    rejected("Destroy target artifact, target creature, or target enchantment.", "Sorcery");
  });
});

describe("definite descriptions as anaphora (CR 608.2c)", () => {
  it("'the creature' is the same symbolic reference as 'that creature'", () => {
    // Mephitic Ooze.
    const text = (ref: string) => `Whenever this creature deals combat damage to a creature, destroy that creature. ${ref} can't be regenerated.`;
    const [definite] = abilities(text("The creature"), "Creature — Ooze");
    const [demonstrative] = abilities(text("That creature"), "Creature — Ooze");
    assert.deepEqual(definite, demonstrative);
    // Sneak Attack.
    assert.deepEqual(abilities("{R}: You may put a creature card from your hand onto the battlefield. That creature gains haste. Sacrifice the creature at the beginning of the next end step.", "Enchantment"),
      abilities("{R}: You may put a creature card from your hand onto the battlefield. That creature gains haste. Sacrifice that creature at the beginning of the next end step.", "Enchantment"));
  });
  it("keeps number and plain qualifiers", () => {
    // Nacatl War-Pride.
    const [ability] = abilities("Whenever this creature attacks, create X tokens that are copies of it and that are tapped and attacking, where X is the number of creatures defending player controls. Exile the tokens at the beginning of the next end step.", "Creature — Cat Warrior");
    assert.deepEqual(JSON.stringify(ability).includes('"what":{"ref":"that","filter":{"cls":"token"},"plural":true}'), true);
    // Outmuscle's Adamant line.
    assert.deepEqual(spell("If at least three green mana was spent to cast this spell, the creature you control gains indestructible until end of turn.", "Sorcery")[0].steps, [{
      effect: "gain-abilities", what: { ref: "that", filter: { types: ["creature"], control: "you" } },
      abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }], duration: { duration: "end-of-turn" },
    }]);
  });
  it("does not turn selections or relations into anaphora", () => {
    rejected("Destroy the creature with the greatest power.", "Sorcery");
    rejected("Destroy the creature it's attached to.", "Sorcery");
    rejected("Destroy the tapped creature.", "Sorcery");
  });
});

describe("several targets declared by 'choose', fighting each other (CR 601.2c, 701.14a)", () => {
  const yours = { ref: "target", filter: { types: ["creature"], control: "you" }, count: one };
  const theirs = { ref: "target", filter: { types: ["creature"], control: "not-you" }, count: one };
  it("declares each target and fights the pair", () => {
    // Malamet Battle Glyph.
    assert.deepEqual(spell("Choose target creature you control and target creature you don't control. If the creature you control entered this turn, put a +1/+1 counter on it. Then those creatures fight each other.", "Sorcery"), [
      { steps: [{ effect: "simultaneous", steps: [{ effect: "declare-target", what: yours }, { effect: "declare-target", what: theirs }] }] },
      { steps: [{ effect: "put-counters", on: { ref: "it" }, counter: { type: "+1/+1" }, count: one }],
        condition: { condition: "zone-change-history", event: "enters", subject: { match: "object", what: { ref: "that", filter: { types: ["creature"], control: "you" } } }, to: { zone: "battlefield" }, during: "this-turn" },
        conditionKind: "if" },
      { steps: [{ effect: "fight-each-other", what: { ref: "that", filter: { types: ["creature"] }, plural: true } }], afterConditional: "scope-unresolved" },
    ]);
  });
  it("needs a plural pair, and only targets form a declared list", () => {
    rejected("Choose target creature you control. Then that creature fights each other.", "Sorcery");
    rejected("Choose target creature you control and a creature you don't control.", "Sorcery");
  });
});

describe("an additional beginning phase (CR 500.8, 501.1)", () => {
  it("is an added phase of its own kind", () => {
    // Sphinx of the Second Sun.
    assert.deepEqual(abilities("At the beginning of each of your postcombat main phases, there is an additional beginning phase after this phase.", "Creature — Sphinx"), [{
      kind: "triggered", trigger: { trigger: "phase", phase: "postcombat-main", whose: "your" },
      effects: [{ steps: [{ effect: "add-phases", after: { phase: "current" }, sequence: [{ phase: "beginning", count: one }] }] }],
    }]);
  });
  it("still rejects steps or unknown phases named as phases", () => {
    rejected("At the beginning of each of your postcombat main phases, there is an additional upkeep phase after this phase.", "Creature — Sphinx");
    rejected("At the beginning of each of your postcombat main phases, there is an additional ending phase after this phase.", "Creature — Sphinx");
  });
});

describe("'with that spell's mana value' (CR 202.3)", () => {
  it("is the equality constraint stated with 'equal to'", () => {
    // Celestial Kirin.
    const type = "Legendary Creature — Kirin Spirit";
    assert.deepEqual(abilities("Whenever you cast a Spirit or Arcane spell, destroy all permanents with that spell's mana value.", type),
      abilities("Whenever you cast a Spirit or Arcane spell, destroy all permanents with mana value equal to that spell's mana value.", type));
  });
  it("needs a single possessed object", () => {
    rejected("Destroy all permanents with those spells' mana value.", "Sorcery");
  });
});
