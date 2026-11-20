import assert from "node:assert/strict";
import { test } from "node:test";
import type { ObjectRef } from "../src/ast.ts";
import { lex, parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseBattlefieldEntryDetails } from "../src/parser/entry.ts";
import { parseTrigger } from "../src/parser/triggers.ts";

// The witness wording comes from the frozen Oracle records. The event
// distinctions use CR 106.12a, 122.2, 405.2, 508.3d, 603.4, 603.6a,
// 603.7, 603.10a, 605.1b, 608.2i, 609.7, and 614.1.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed", value });
const you = { player: "you" };
const battlefield = { zone: "battlefield" };
const filter = (match: object) => ({ ref: "filter", filter: match, count: fixed(1) });
const chosen = { choice: "chosen-value", category: "color", which: "chosen" };

function ability(text: string, typeLine: string) {
  const parsed = parseOracleText(text, "", { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0])) as (typeof parsed.abilities)[number];
}

function fails(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, "", { typeLine }).ok, false, text);
}

test("the printed self or other entry histories share the actual moving controller", () => {
  const text = "{T}: Create a 4/4 white and blue Golem artifact creature token. Activate only if this artifact or another artifact entered the battlefield under your control this turn.";
  const parsed = ability(text, "Artifact");
  assert.equal(parsed.kind, "activated");
  assert.deepEqual(parsed.restriction, { restriction: "condition", condition: { condition: "any-of", conditions: [
    { condition: "zone-change-history", event: "enters", subject: { match: "object", what: { ref: "this", noun: "artifact" } },
      to: battlefield, controllerAtMove: you, during: "this-turn" },
    { condition: "zone-change-history", event: "enters", subject: { match: "filter", filter: { other: true, types: ["artifact"] },
      comparison: { op: "ge", value: fixed(1) } }, to: battlefield, controllerAtMove: you, during: "this-turn" },
  ] } });
  fails(text.replace("another artifact entered", "another artifact departed"), "Artifact");
});

test("an intervening count filters only the participating attackers", () => {
  const text = "Whenever an opponent attacks with creatures, if two or more of those creatures are attacking you and/or planeswalkers you control, draw a card.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.trigger, { trigger: "player-attacks", who: { player: "an-opponent" },
    per: "declaration", with: { types: ["creature"] } });
  assert.deepEqual(parsed.condition, { condition: "count", comparison: { op: "ge", value: fixed(2) }, of: {
    fromAmong: { ref: "that", filter: { types: ["creature"] }, plural: true },
    attackingLookup: "current-or-last-known",
    attacking: [
      { defender: "player", who: you },
      { defender: "object", what: { ref: "each", filter: { types: ["planeswalker"], control: "you" } } },
    ],
  } });
  fails(text.replace("you and/or planeswalkers you control", "you and/or spells you control"), "Creature");
  fails(text.replace("an opponent attacks with creatures", "an artifact or creature card is put into a graveyard from anywhere"), "Creature");
});

test("threshold counts matching permanents rather than the counters they hold", () => {
  const text = "At the beginning of your end step, if two or more permanents you don't control have an aim counter on them, destroy one of those permanents at random.";
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.condition, { condition: "count", of: { cls: "permanent", control: "not-you", counterConstraints: [
    { counter: { type: "aim" }, comparison: { op: "ge", value: fixed(1) } },
  ] }, comparison: { op: "ge", value: fixed(2) } });
  assert.deepEqual(parsed.effects[0].steps[0], { effect: "destroy", what: {
    ref: "from-pool", pool: { ref: "that", filter: { cls: "permanent" }, plural: true }, count: fixed(1), random: true,
  } });
  fails(text.replace("on them", "on it"), "Enchantment");
});

test("mana addition from a land's ability and tapping for colored mana stay distinct", () => {
  const a = "Whenever a land's ability causes you to add one or more mana of the chosen color, add an additional one mana of that color.";
  const caged = ability(a, "Artifact");
  assert.equal(caged.kind, "triggered");
  assert.deepEqual(caged.trigger, {
    trigger: "adds-mana-from-ability", who: you, abilitySource: filter({ types: ["land"] }), color: chosen,
    quantity: { op: "ge", value: fixed(1) },
  });
  assert.deepEqual(caged.effects[0].steps, [
    { effect: "add-mana", who: you, mana: { mana: "trigger-event-color", amount: fixed(1) } },
  ]);

  const b = "Whenever a basic land is tapped for mana of the chosen color, its controller adds an additional one mana of that color.";
  const gauntlet = ability(b, "Artifact");
  assert.equal(gauntlet.kind, "triggered");
  assert.deepEqual(gauntlet.trigger, { trigger: "taps-for-mana", what: filter({ supertypes: ["basic"], types: ["land"] }), producingChosenColor: chosen });
  assert.deepEqual(gauntlet.effects[0].steps, [
    { effect: "add-mana", who: { player: "controller", of: { ref: "it" } }, mana: { mana: "trigger-event-color", amount: fixed(1) } },
  ]);
  fails(a.replace("of the chosen color", "of any color"), "Artifact");
  fails(b.replace("mana of the chosen color", "mana"), "Artifact");
});

test("stack placement matches both spells and abilities without requiring a cast", () => {
  const header = "Whenever a spell or ability is put onto the stack";
  const c = new Cursor(lex(header));
  const event = { trigger: "put-into-zone",
    what: filter({ allOf: [{ anyOf: [{ cls: "spell" }, { cls: "ability" }] }] }),
    to: { zone: "stack" },
  };
  assert.deepEqual(parseTrigger(c), event);
  assert.ok(c.done());
  const text = `${header}, if it has a single target, reselect its target at random.`;
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.trigger, event);
  assert.deepEqual(parsed.condition, { condition: "is-filter", what: { ref: "it" }, filter: { singleTarget: true } });
  assert.deepEqual(parsed.effects[0].steps, [{
    effect: "change-targets", what: { ref: "it" }, who: you, scope: "each",
    reselection: { method: "random", unit: "target-slot", whenUnavailable: "keep" },
  }]);
  fails("Whenever a land or ability is put onto the stack, draw a card.", "Enchantment");
  fails(`${header}, reselect its target at random.`, "Enchantment");
  fails("Whenever you cast a spell, if it has a single target, reselect its target at random.", "Enchantment");
});

test("damage source alternatives and recipient alternatives stay independent", () => {
  const text = "If a red or artifact source you control would deal damage to an opponent or a permanent an opponent controls, it deals that much damage plus 1 instead.";
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "static");
  assert.deepEqual(parsed.effect, { static: "replacement", replacement: {
    event: { event: "damage", kind: "all", source: filter({ anyOf: [{ colors: ["red"] }, { types: ["artifact"] }], control: "you" }),
      to: [{ player: "an-opponent" }, filter({ cls: "permanent", control: "opponent" })] },
    occurrence: "each", replace: { replacement: "modify-quantity", change: { operation: "add", amount: fixed(1) } },
  } });
  fails(text.replace("it deals that much damage", "another source deals that much damage"), "Enchantment");
});

test("replacement exclusion measures pre-move control and independent graveyard ownership", () => {
  const text = "If a card you didn't control would be put into an opponent's graveyard from anywhere, exile it instead.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "static");
  assert.deepEqual(parsed.effect, { static: "replacement", replacement: {
    event: { event: "move-zone", what: filter({ cls: "card" }), from: "anywhere",
      to: { zone: "graveyard", owner: "an-opponent" }, notControlledByBefore: you },
    occurrence: "each", replace: { replacement: "effects", effects: [
      { steps: [{ effect: "exile", what: { ref: "replacement-event-object", role: "affected" } }] },
    ] },
  } });
  fails(text.replace("didn't control", "didn't control at the beginning of your turn"), "Creature");
});

test("a next-cast replacement follows the watched spell through resolution", () => {
  const text = "II — When you next cast a creature spell this turn, that creature enters with an additional +1/+1 counter on it.";
  const parsed = ability(text, "Enchantment — Saga");
  assert.equal(parsed.kind, "chapter");
  assert.deepEqual(parsed.effects[0].steps, [{
    effect: "create-delayed-trigger", trigger: { trigger: "cast", who: you, what: { types: ["creature"], cls: "spell" } },
    occurrence: "next", duration: { duration: "this-turn" }, nextOnly: true, targets: "delayed-ability",
    effects: [{ steps: [{ effect: "create-replacement", replacement: {
      event: { event: "move-zone", from: { zone: "stack" }, to: battlefield, resolving: true,
        what: { ref: "trigger-event-object", event: "cast" } },
      occurrence: "next", replace: { replacement: "entry-modifiers", modifiers: {
        counters: [{ count: fixed(1), counter: { type: "+1/+1" }, additional: true }],
      } },
    } }] }],
  }]);
  fails(text.replace("a creature spell", "an artifact spell"), "Enchantment — Saga");
  fails(text.replace("that creature", "another creature"), "Enchantment — Saga");
});

test("death replacement damage source control is historical", () => {
  const text = "If a creature dealt damage this turn by a source you controlled would die, exile it instead.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "static");
  assert.equal(parsed.effect.static, "replacement");
  assert.deepEqual(parsed.effect.replacement.event, {
    event: "move-zone", what: filter({ types: ["creature"], damageHistory: {
      role: "recipient", window: { during: "this-turn" }, source: filter({ cls: "source", control: "you" }),
    } }), from: battlefield, to: { zone: "graveyard" },
  });
  fails(text.replace("this turn", "at some point"), "Creature");
});

test("one fewer death-time counter is an entry modifier for the same returning card", () => {
  const suffix = "with one fewer revival counter on it";
  const died: ObjectRef = { ref: "it" };
  const expected = { counters: [{ counter: { type: "revival" }, count: { amount: "minus",
    a: { amount: "counter-count-before-zone-change", of: died, event: "dies", counter: { type: "revival" } }, b: fixed(1),
  } }] };
  const c = new Cursor(lex(suffix));
  assert.deepEqual(parseBattlefieldEntryDetails(c, died), { entry: expected });
  assert.ok(c.done());
  const text = "When this creature dies, if it had a revival counter on it, return it to the battlefield with one fewer revival counter on it at the beginning of the next end step.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.condition, { condition: "is-filter", what: died,
    filter: { counterConstraints: [{ counter: { type: "revival" }, comparison: { op: "ge", value: fixed(1) } }] },
    time: "previous",
  });
  const delayed = parsed.effects[0].steps[0];
  assert.equal(delayed.effect, "create-delayed-trigger");
  assert.equal(delayed.occurrence, "next");
  assert.deepEqual(delayed.trigger, { trigger: "phase", phase: "end-step", whose: "each-player" });
  assert.deepEqual(delayed.effects[0].steps, [{ effect: "move-zone", what: died, to: battlefield, entry: expected }]);
  fails("Return it to the battlefield with one fewer revival counter on it.", "Instant");
  fails(text.replace("return it", "return another creature"), "Creature");
  fails(text.replace("on it at", "on them at"), "Creature");
});

test("leave-trigger damage quantity is totaled separately for each player", () => {
  const text = "When this creature leaves the battlefield, each player draws cards equal to the amount of damage dealt to this creature this turn by sources they controlled.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.effects[0].steps, [{ effect: "draw", who: { player: "each-player" }, amount: {
    amount: "damage-history-total", to: { ref: "this", noun: "creature" }, during: "this-turn",
    sourceControllerAtDamage: { player: "they" },
  } }]);
  fails(text.replace("sources they controlled", "sources they own"), "Creature");
  fails(text.replace("each player draws", "you draw"), "Creature");
});
