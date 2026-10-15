// Regressions found by independent audits of new parses.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

function only(text: string, typeLine?: string): Ability {
  const parsed = parseOracleText(text, undefined, { typeLine });
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0])) as Ability;
}
const fails = (text: string, typeLine?: string) => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
const one = { amount: "fixed", value: 1 };

describe("audit fixes: qualifier scope", () => {
  it("applies a creature-only test to the creature alternative of bare card types (Broken Wings)", () => {
    assert.deepEqual(only("Destroy target artifact, enchantment, or creature with flying.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "destroy", what: { ref: "target", count: one, filter: { allOf: [{ anyOf: [
        { types: ["artifact"] }, { types: ["enchantment"] }, { types: ["creature"], withAbility: ["flying"] }] }] } } }] }],
    });
    assert.deepEqual(only("Exile target artifact, enchantment, or creature with power 4 or greater.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "exile", what: { ref: "target", count: one, filter: { allOf: [{ anyOf: [
        { types: ["artifact"] }, { types: ["enchantment"] },
        { types: ["creature"], properties: [{ property: "power", comparison: { op: "ge", value: { amount: "fixed", value: 4 } } }] }] }] } } }] }],
    });
  });
  it("keeps shared qualifiers shared", () => {
    // Mana value and control describe every alternative; a shared head noun does too.
    const shared = only("Destroy target artifact, enchantment, or creature with mana value 3 or less.", "Instant");
    assert.equal(JSON.stringify(shared).includes('"anyOf":[{"types":["artifact"]},{"types":["enchantment"]},{"types":["creature"]}]'), true);
    const controlled = only("Destroy target artifact or creature an opponent controls.", "Instant");
    assert.equal(JSON.stringify(controlled).includes('"anyOf":[{"types":["artifact"]},{"types":["creature"]}]'), true);
  });
  it("marks a per-object as-long-as condition on a plural subject (Arcades Sabboth)", () => {
    const ability = only("Each untapped creature you control gets +0/+2 as long as it's not attacking.", "Legendary Creature — Elder Dragon");
    assert.equal(ability.kind === "static" && ability.effect.static === "modify" && ability.effect.conditionScope, "each-affected-object");
  });
});

describe("audit fixes: references", () => {
  it("tests player counters on a player, not on an object set (Vexing Radgull)", () => {
    const ability = only("Whenever this creature deals combat damage to a player, that player gets two rad counters if they don't have any rad counters. Otherwise, proliferate.", "Creature — Bird Mutant");
    assert.equal(ability.kind, "triggered");
    assert.deepEqual(ability.kind === "triggered" && ability.effects[0].condition, { condition: "not", of: { condition: "player-counters",
      who: { player: "they" }, counter: { type: "rad" }, comparison: { op: "ge", value: one } } });
    fails("Whenever this creature deals combat damage to a player, that player draws a card if they don't have any +1/+1 counters.", "Creature — Bird");
  });
  it("never moves a selection's zone onto a counted object (Puca's Covenant)", () => {
    fails("Return target permanent card with mana value less than or equal to the number of counters on that creature from your graveyard to your hand.", "Sorcery");
  });
  it("never reads a token copy's attachment as a description of the original (Arna Kennerüd)", () => {
    fails("Create a token that's a copy of that permanent attached to that creature.", "Sorcery");
  });
  it("keeps the printed possessive of an ability word", () => {
    const ability = only("Council's dilemma — When this creature enters, draw a card.", "Creature — Human");
    assert.equal(ability.kind === "triggered" && ability.abilityWord, "Council's dilemma");
    const plural = only("Scions' Secretary — When this creature enters, draw a card.", "Creature — Human");
    assert.equal(plural.kind === "triggered" && plural.abilityWord, "Scions' Secretary");
  });
});

describe("audit fixes: anaphora and damage recipients", () => {
  it("limits every coordinated immediate play to the stated source (Gix, Yawgmoth Praetor)", () => {
    const ability = only("You may play lands and cast spells from among cards exiled this way without paying their mana costs.", "Sorcery");
    const text = JSON.stringify(ability);
    assert.equal(text.match(/"fromAmong"/g)?.length, 2, text);
  });
  it("does not carry the previous sentence's actor into an Otherwise branch (Fraying Line)", () => {
    const ability = only("At the beginning of each player's upkeep, that player may pay {2}. If they do, they put a rope counter on a creature they control. Otherwise, exile this artifact.", "Artifact");
    assert.equal(ability.kind, "triggered");
    assert.deepEqual(ability.kind === "triggered" && ability.effects[1].otherwise?.steps, [{ effect: "exile", what: { ref: "this", noun: "artifact" } }]);
  });
});

describe("audit fixes: conditional continuations", () => {
  const all = (text: string, typeLine: string) => {
    const parsed = parseOracleText(text, undefined, { typeLine });
    assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
    if (!parsed.ok) throw new Error(text);
    return JSON.parse(JSON.stringify(parsed.abilities)) as Ability[];
  };
  it("leaves a bare 'them' damage recipient unresolved between player and objects (Vexing Arcanix)", () => {
    const [ability] = all("At the beginning of each player's upkeep, this enchantment deals 2 damage to them.", "Enchantment");
    assert.equal(ability.kind, "triggered");
    assert.deepEqual(ability.kind === "triggered" && ability.effects[0].steps[0], {
      effect: "damage", source: { ref: "this", noun: "enchantment" }, amount: { amount: "fixed", value: 2 }, to: { to: "them" } });
  });
  it("marks a continuation after a conditional instruction as scope-unresolved, never silently unconditional (Hunting Wilds)", () => {
    const [ability] = all("Untap all Forests you control if you control a Bear. They become 3/3 green creatures with haste that are still lands.", "Sorcery");
    assert.equal(ability.kind === "spell" && ability.effects[1]?.afterConditional, "scope-unresolved");
    const [kicked] = all("If this spell was kicked, untap all lands tapped this way. They gain haste until end of turn.", "Sorcery");
    assert.equal(kicked.kind === "spell" && kicked.effects[1].afterConditional, "scope-unresolved");
    // Any pronoun-led continuation after a conditional sentence is left unresolved, even one whose
    // antecedent is older than that sentence; the parser asserts neither reading.
    const [earlier] = all("Destroy target creature. If you control a Wizard, draw a card. Its controller loses 2 life.", "Sorcery");
    assert.equal(earlier.kind === "spell" && earlier.effects[2].afterConditional, "scope-unresolved");
    // No back-reference, or only "the rest": plainly unconditional.
    const [plain] = all("Look at the top three cards of your library. If you control a Wizard, draw a card. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.equal(plain.kind === "spell" && plain.effects.some(sentence => sentence.afterConditional !== undefined), false);
  });
  it("never moves the outer selection's pool onto a compared object (Radagast the Brown)", () => {
    fails("You may reveal a creature card that doesn't share a creature type with a creature you control from among those cards and put it into your hand.", "Sorcery");
  });
});

