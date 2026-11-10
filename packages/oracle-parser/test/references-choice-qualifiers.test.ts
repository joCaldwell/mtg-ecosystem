import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";

const n = (value: number) => ({ amount: "fixed", value } as const);
function parsed(text: string, name?: string, typeLine = "Sorcery") {
  const result = parseOracleText(text, name, { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}
const fails = (text: string, typeLine = "Sorcery") => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

// Oracle texts are complete abilities of cards in the fixed Scryfall corpus.
// CR 701.21a: the sacrificing player chooses among the permanents they
// control that match the whole description, wherever "of their choice" is
// printed inside it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("qualifiers after \"of their choice\"", () => {
  it("keeps a keyword or mana-value test printed after the chooser (Clip Wings, Exile into Darkness)", () => {
    const flying = { effect: "sacrifice", who: { player: "each-opponent" },
      what: { ref: "filter", filter: { types: ["creature"], withAbility: ["flying"] }, count: n(1) }, chosenBy: { player: "each-opponent" } };
    assert.deepEqual(parsed("Each opponent sacrifices a creature of their choice with flying.", "Clip Wings", "Instant")[0].effects[0].steps, [flying]);
    // The same description with the chooser printed last is the same node.
    assert.deepEqual(parsed("Each opponent sacrifices a creature with flying of their choice.")[0].effects[0].steps, [flying]);
    assert.deepEqual(parsed("Target player sacrifices a creature of their choice with mana value 3 or less.", "Exile into Darkness")[0].effects[0].steps[0].what,
      { ref: "filter", filter: { types: ["creature"], properties: [{ property: "mana-value", comparison: { op: "le", value: n(3) } }] }, count: n(1) });
  });
  it("keeps a characteristic relation and a damage history (Martyr's Bond, Witch-king of Angmar)", () => {
    assert.deepEqual(parsed("Whenever this enchantment or another nonland permanent you control is put into a graveyard from the battlefield, each opponent sacrifices a permanent of their choice that shares a card type with it.",
      "Martyr's Bond", "Enchantment")[0].effects[0].steps, [{ effect: "sacrifice", who: { player: "each-opponent" },
      what: { ref: "filter", filter: { cls: "permanent", characteristicRelations: [{ relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: { ref: "it" } } }] }, count: n(1) },
      chosenBy: { player: "each-opponent" } }]);
    assert.deepEqual(parsed("Whenever one or more creatures deal combat damage to you, each opponent sacrifices a creature of their choice that dealt combat damage to you this turn. The Ring tempts you.",
      "Witch-king of Angmar", "Legendary Creature — Wraith Noble")[0].effects[0].steps[0].what,
      { ref: "filter", filter: { types: ["creature"], damageHistory: { role: "source", window: { during: "this-turn" }, damage: "combat", to: { player: "you" } } }, count: n(1) });
  });
  it("rejects a late qualifier over alternatives and one it cannot state", () => {
    fails("Each opponent sacrifices an artifact or creature of their choice with flying.");
    fails("That player sacrifices a creature of their choice that they controlled since the beginning of the turn.");
  });
});

// CR 608.2c/e: a loop's body is done for each member; a later sentence that
// speaks in the plural of what every member's single object produced is done
// once, after the loop (a sibling sentence), never per member.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("plural continuations after a for-each", () => {
  const loop = (each: object, step: object) => ({ effect: "for-each", each, effects: [{ steps: [step] }], processing: "instruction-wise" });
  const opponentsCreature = { ref: "target", filter: { types: ["creature"], control: "that-player" }, count: n(1), upTo: true };
  it("acts on those objects once (Hapatra, the Desert Frost)", () => {
    assert.deepEqual(parsed("When ~ enters, for each opponent, tap up to one target creature that player controls. Put a stun counter on each of those creatures.",
      "Hapatra, the Desert Frost", "Legendary Creature — Human Wizard")[0].effects, [
      { steps: [loop({ player: "each-opponent" }, { effect: "tap", what: opponentsCreature })] },
      { steps: [{ effect: "put-counters", on: { ref: "that", filter: { types: ["creature"] }, plural: true }, counter: { type: "stun" }, count: n(1) }] }]);
  });
  it("uses the chosen objects and an aggregate of this way's events (Ultimate Magic: Meteor, Unexplained Absence)", () => {
    const meteor = parsed("~ deals 7 damage to each creature. If this spell was cast from exile, for each opponent, choose an artifact or land that player controls. Destroy the chosen permanents.", "Ultimate Magic: Meteor")[0].effects;
    assert.deepEqual(meteor[1].steps, [loop({ player: "each-opponent" }, { effect: "choose-object", who: { player: "you" },
      what: { ref: "filter", filter: { allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["land"] }] }], control: "that-player" }, count: n(1) } })]);
    // The destroy may be governed by the "if" of the previous sentence.
    assert.deepEqual(meteor[2], { steps: [{ effect: "destroy", what: { ref: "choice-result", number: "plural", filter: { cls: "permanent" }, which: "chosen", scope: "contextual" } }],
      afterConditional: "scope-unresolved" });
    assert.deepEqual(parsed("For each player, exile up to one target nonland permanent that player controls. For each permanent exiled this way, its controller cloaks the top card of their library.", "Unexplained Absence")[0].effects[1],
      { steps: [loop({ ref: "each", filter: { cls: "permanent", relation: { relation: "event-result", event: "exile", scope: "this-way" } } },
        { effect: "cloak", who: { player: "controller", of: { ref: "it" } }, what: { ref: "library-cards", position: "top", count: n(1), of: { zone: "library", owner: "their" } } })] });
  });
  it("aggregates one token per member, then later sentences follow outside (Saheeli, the Gifted)", () => {
    assert.deepEqual(parsed("−7: For each artifact you control, create a token that's a copy of it. Those tokens gain haste. Exile those tokens at the beginning of the next end step.",
      "Saheeli, the Gifted", "Legendary Planeswalker — Saheeli")[0].effects, [
      { steps: [loop({ ref: "each", filter: { types: ["artifact"], control: "you" } }, { effect: "create-token", count: n(1), token: { copyOf: { ref: "it" } } })] },
      { steps: [{ effect: "gain-abilities", what: { ref: "that", filter: { cls: "token" }, plural: true }, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }] }] },
      { steps: [{ effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" }, occurrence: "next", nextOnly: true,
        effects: [{ steps: [{ effect: "exile", what: { ref: "that", filter: { cls: "token" }, plural: true } }] }], targets: "enclosing-ability" }] }]);
  });
  it("rejects continuations that could be per member or name something else", () => {
    // Several objects per member: "they" could be each member's own tokens.
    fails("For each opponent, create two 3/3 blue and red Elemental creature tokens with flying that attack that opponent this turn if able. They gain haste until end of turn.");
    // A singular reference to the member or its object.
    fails("For each opponent, tap up to one untapped creature you control. When you do, gain control of target creature that player controls.");
    fails("For each opponent, choose up to one target creature they control. That creature's owner puts it on the top of their library.");
    // "those tokens" names nothing the loop made.
    fails("For each opponent, tap up to one target creature that player controls. Sacrifice those tokens.");
    // The chosen objects of a loop that chose nothing.
    fails("For each opponent, destroy up to one target creature that player controls. Return the chosen creatures to their owners' hands.");
  });
});

// CR 707.10, 707.10c: the player who copies may choose new targets for the
// copy as it is created; another player's "may" is that player's decision.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("another player's optional copy with new targets", () => {
  const thisSpell = { ref: "this", noun: "spell" };
  it("keeps the copier as decider and retargeting chooser (Chain of Smog, Chain of Vapor)", () => {
    assert.deepEqual(parsed("Target player discards two cards. That player may copy this spell and may choose a new target for that copy.", "Chain of Smog")[0].effects[1],
      { steps: [{ effect: "copy-stack-object", what: thisSpell, who: { player: "that-player" }, count: n(1), optional: true, decidedBy: { player: "that-player" },
        retarget: { who: { player: "that-player" }, optional: true } }] });
    const vapor = parsed("Return target nonland permanent to its owner's hand. Then that permanent's controller may sacrifice a land of their choice. If the player does, they may copy this spell and may choose a new target for that copy.", "Chain of Vapor", "Instant")[0].effects;
    assert.deepEqual(vapor[2], { condition: { condition: "preceding-action", who: { player: "the-player" }, outcome: "performed" }, conditionKind: "if",
      steps: [{ effect: "copy-stack-object", what: thisSpell, who: { player: "they" }, count: n(1), optional: true, decidedBy: { player: "they" },
        retarget: { who: { player: "they" }, optional: true } }] });
  });
  it("keeps your own optional copy without a decider (Sevinne's Reclamation)", () => {
    assert.deepEqual(parsed("Return target permanent card with mana value 3 or less from your graveyard to the battlefield. If this spell was cast from a graveyard, you may copy this spell and may choose a new target for the copy.", "Sevinne's Reclamation")[0].effects[1].steps,
      [{ effect: "copy-stack-object", what: thisSpell, who: { player: "you" }, count: n(1), retarget: { who: { player: "you" }, optional: true }, optional: true }]);
  });
  it("rejects a retarget whose number disagrees with the copies made", () => {
    fails("Target player discards two cards. That player may copy this spell twice and may choose new targets for that copy.");
    fails("Target player discards two cards. That player may copy this spell and may choose new targets for the copies.");
  });
});
