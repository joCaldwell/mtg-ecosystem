import assert from "node:assert/strict";
import { test } from "node:test";
import { parseOracleText } from "../src/index.ts";

const you = { player: "you" };
const thatPlayer = { player: "that-player" };
const one = { amount: "fixed", value: 1 };
const two = { amount: "fixed", value: 2 };
const castSpell = { ref: "trigger-event-object", event: "cast" };
const thatSpell = { ref: "that", filter: { cls: "spell" } };
const randomSlots = { method: "random", unit: "target-slot", whenUnavailable: "keep" };
const free = { cost: "without-mana-cost" };
const exileResult = { relation: "event-result", event: "exile", scope: "this-way" };

function parsed(text: string, typeLine: string) {
  const result = parseOracleText(text, "Test Card", { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines.filter(line => !line.ok)));
  const [ability] = result.abilities;
  assert.ok("effects" in ability);
  return ability;
}

function rejects(text: string, typeLine: string) {
  assert.equal(parseOracleText(text, "Test Card", { typeLine }).ok, false, text);
}

test("copies a spell once for each other legal Golem target, replacing all its target slots", () => {
  const text = "Whenever a player casts an instant or sorcery spell that targets only a single Golem, that player copies that spell for each other Golem that spell could target. Each copy targets a different one of those Golems.";
  assert.deepEqual(parsed(text, "Artifact Creature — Golem").effects, [{ steps: [{
    effect: "copy-stack-object", what: castSpell, who: thatPlayer, count: one,
    fixedTargets: { kind: "one-per-legal-target", filter: { subtypes: ["Golem"] },
      exclude: "original-target", allTargetSlots: true, order: "copy-controller" },
  }] }]);
  rejects(text.replace("those Golems", "those Goblins"), "Artifact Creature — Golem");
  rejects(text.replace("only a single Golem", "only a single Goblin"), "Artifact Creature — Golem");
});

test("allows each non-caster to retarget their own independently created copy", () => {
  const text = "Whenever a player casts an instant or sorcery spell, each other player copies that spell. Each of those players may choose new targets for their copy.";
  const others = { player: "filtered", base: { player: "each-player" }, filter: { otherThan: thatPlayer } };
  assert.deepEqual(parsed(text, "Enchantment").effects, [{ steps: [{
    effect: "copy-stack-object", what: castSpell, who: others, count: one,
    retarget: { who: others, optional: true },
  }] }]);
  rejects(text.replace("their copy", "the original spell"), "Enchantment");
});

test("counts opponents who made their optional copy, regardless of whether they retargeted it", () => {
  const text = "Tempting offer — Choose target instant or sorcery spell. Each opponent may copy that spell and may choose new targets for the copy they control. You copy that spell once plus an additional time for each opponent who copied the spell this way. You may choose new targets for the copies you control.";
  const eachOpponent = { player: "each-opponent" };
  const [declaration, offer, additional] = parsed(text, "Instant").effects;
  assert.equal(declaration.steps[0].effect, "declare-target");
  assert.deepEqual(offer.steps, [{
    effect: "copy-stack-object", what: thatSpell, who: eachOpponent, count: one,
    optional: true, decidedBy: eachOpponent, retarget: { who: eachOpponent, optional: true },
  }]);
  assert.deepEqual(additional.steps, [{
    effect: "copy-stack-object", what: thatSpell, who: you,
    count: { amount: "plus", a: one, b: { amount: "player-count",
      of: { player: "filtered", base: eachOpponent, filter: { precedingAction: "performed" } } } },
    retarget: { who: you, optional: true },
  }]);
  rejects(text.replace("copied the spell this way", "chose new targets this way"), "Instant");
});

test("allows the triggering caster a separate free cast of each copy of a linked exiled card", () => {
  const text = "Whenever a player casts an instant or sorcery card, exile it. Then that player copies each instant or sorcery card exiled with this enchantment. For each copy, the player may cast the copy without paying its mana cost.";
  const [, copy, permit] = parsed(text, "Enchantment").effects;
  assert.deepEqual(copy.steps, [{
    effect: "copy-card", who: thatPlayer, count: one,
    what: { ref: "each", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }],
      cls: "card", relation: { relation: "linked-exile", source: { ref: "this", noun: "enchantment" } } } },
  }]);
  assert.deepEqual(permit.steps, [{
    effect: "play-card", optional: true, decidedBy: thatPlayer,
    instruction: { who: thatPlayer, action: "cast", what: { ref: "copy-result", number: "plural" }, alternativeCost: free },
  }]);
  rejects(text.replace("Then that player copies each instant or sorcery card exiled with this enchantment. ", ""), "Enchantment");
});

test("separates the immediate land placement from the permission limited by nonland card type", () => {
  const text = "Exile the top eight cards of your library. You may put a land card from among them onto the battlefield. Until end of turn, for each nonland card type, you may cast a spell of that type from among the exiled cards without paying its mana cost.";
  const [, placement, permission] = parsed(text, "Sorcery").effects;
  assert.deepEqual(placement.steps, [{
    effect: "move-zone",
    what: { ref: "filter", filter: { types: ["land"], cls: "card", fromAmong: { ref: "them" } }, count: one },
    to: { zone: "battlefield" }, optional: true,
  }]);
  assert.deepEqual(permission.steps, [{
    effect: "grant-play-permission", duration: { duration: "end-of-turn" },
    permission: { who: you, action: "cast",
      fromAmong: { ref: "related", number: "plural", filter: { cls: "card" },
        relation: { relation: "event-result", event: "exile", scope: "contextual" } },
      what: { ref: "filter", filter: { cls: "spell" }, count: one },
      alternativeCost: free, useLimit: { maximum: 1, period: "permission", per: "nonland-card-type" } },
  }]);
  rejects(text.replace("card type", "creature type"), "Sorcery");
});

test("randomly reselects all slots for the spell and its copy subject to the same exclusions", () => {
  const text = "Gain control of target spell that targets only a single permanent or player. Copy it, then reselect the targets at random for the spell and the copy. The new targets can't be you or a permanent you control.";
  const [control, randomize] = parsed(text, "Instant").effects;
  const [gain] = control.steps;
  assert.ok(gain.effect === "gain-control" && gain.what.ref === "target");
  assert.ok(gain.what.selectionId);
  assert.deepEqual(gain.what.filter.targetsOnly, { ref: "filter", filter: { cls: "permanent", orPlayer: true }, count: one });
  assert.deepEqual(randomize.steps, [
    { effect: "copy-stack-object", what: thatSpell, who: you, count: one },
    { effect: "change-targets", what: thatSpell, also: [{ ref: "copy-result", number: "singular" }], who: you, scope: "each",
      reselection: { ...randomSlots, excluding: [you, { ref: "each", filter: { cls: "permanent", controlledBy: you } }] } },
  ]);
  rejects(text.replace("for the spell and the copy", "for the copy"), "Instant");
  rejects(text.replace("or a permanent you control", "or a permanent you own"), "Instant");
});

test("reselects the single stack target only in the guarded stack-trigger scope", () => {
  const text = "Whenever a spell or ability is put onto the stack, if it has a single target, reselect its target at random.";
  const ability = parsed(text, "Enchantment");
  assert.ok(ability.kind === "triggered");
  assert.deepEqual(ability.condition, { condition: "is-filter", what: { ref: "it" }, filter: { singleTarget: true } });
  assert.deepEqual(ability.effects, [{ steps: [{
    effect: "change-targets", what: { ref: "it" }, who: you, scope: "each", reselection: randomSlots,
  }] }]);
  rejects(text.replace("if it has a single target, ", ""), "Enchantment");
  rejects(text.replace("reselect", "copy that spell. Reselect"), "Enchantment");
  rejects("Reselect its target at random.", "Instant");
});

test("reselects the defender of the already attacking target creature", () => {
  const text = "When this artifact enters during the declare attackers step, you may reselect which player or permanent target attacking creature is attacking.";
  const ability = parsed(text, "Artifact");
  assert.ok(ability.kind === "triggered");
  assert.deepEqual(ability.trigger, { trigger: "enters", what: { ref: "this", noun: "artifact" }, inPhase: { phase: "declare-attackers" } });
  const [redirect] = ability.effects[0].steps;
  assert.ok(redirect.effect === "combat-rule" && redirect.what.ref === "target");
  assert.ok(redirect.what.selectionId);
  assert.deepEqual(redirect.what.filter, { status: ["attacking"], types: ["creature"] });
  assert.deepEqual(redirect.rule, { rule: "reselect-attack-defender", candidates: "player-or-permanent" });
  assert.equal(redirect.optional, true);
  rejects(text.replace("target attacking creature", "target creature"), "Artifact");
});

test("uses the same targeted creature pool in the repeating delayed trigger", () => {
  const text = "I, II — For each opponent, tap up to one target creature that player controls. Goad those creatures. Until your next turn, whenever any of those creatures deals combat damage to a player, draw a card.";
  const ability = parsed(text, "Enchantment — Saga");
  assert.ok(ability.kind === "chapter");
  assert.deepEqual(ability.chapters, [1, 2]);
  const [targeting, goad, delayed] = ability.effects;
  const loop = targeting.steps[0];
  assert.ok(loop.effect === "for-each" && loop.effects[0].steps[0].effect === "tap");
  assert.deepEqual(loop.each, { player: "each-opponent" });
  const pool = { ref: "that", filter: { types: ["creature"] }, plural: true };
  assert.deepEqual(goad.steps, [{ effect: "goad", what: pool }]);
  assert.deepEqual(delayed.steps, [{
    effect: "create-delayed-trigger", occurrence: "each", duration: { duration: "your-next-turn" }, targets: "delayed-ability",
    trigger: { trigger: "deals-damage", what: { ref: "from-pool", pool, count: one }, combat: true, to: "any" },
    effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }],
  }]);
  rejects(text.replace("Goad those creatures. ", ""), "Enchantment — Saga");
  rejects(text.replace("any of those creatures", "any of those permanents"), "Enchantment — Saga");
});

test("repeats drawing and optional exile for the target with a grant over both exile results", () => {
  const text = "Draw two cards, then you may exile a nonland card from your hand with a number of time counters on it equal to its mana value. Then target opponent does the same. Cards exiled this way that don't have suspend gain suspend.";
  const [yourInstructions, opponentInstructions, grant] = parsed(text, "Sorcery").effects;
  const [, yourExile] = yourInstructions.steps;
  const [draw, exile] = opponentInstructions.steps;
  assert.ok(draw.effect === "draw" && draw.who.player === "target-opponent");
  assert.ok(draw.who.selectionId);
  assert.deepEqual(draw.amount, two);
  assert.ok(yourExile.effect === "exile" && exile.effect === "exile" && exile.what.ref === "filter");
  assert.deepEqual(exile.what.filter.zone, { zone: "hand", owner: thatPlayer });
  assert.deepEqual(exile.decidedBy, thatPlayer);
  assert.deepEqual(exile.counters, yourExile.counters);
  assert.equal(exile.optional, true);
  assert.deepEqual(grant.steps, [{
    effect: "gain-abilities", what: { ref: "each", filter: { cls: "card", withoutAbility: ["suspend"],
      fromAmong: { ref: "related", number: "plural", filter: { cls: "card" }, relation: exileResult } } },
    abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }],
  }]);
  rejects(text.replace("Then target opponent", "Then each opponent"), "Sorcery");
  rejects(text.replace("you may exile", "exile"), "Sorcery");
  rejects(`Exile the top card of your library. ${text}`, "Sorcery");
});

test("puts counters on every nonland exile result and conditionally gives that same member suspend", () => {
  const text = "I — Exile the top five cards of your library. For each nonland card exiled this way, put a number of time counters on that card equal to its mana value. If it doesn't have suspend, it gains suspend.";
  const [, each] = parsed(text, "Enchantment — Saga").effects;
  assert.deepEqual(each.steps, [{
    effect: "for-each", processing: "instruction-wise",
    each: { ref: "each", filter: { nonTypes: ["land"], cls: "card", relation: exileResult } },
    effects: [
      { steps: [{ effect: "put-counters", on: { ref: "that", filter: { cls: "card" } },
        count: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" }, counter: { type: "time" } }] },
      { condition: { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } },
        conditionKind: "if", steps: [{ effect: "gain-abilities", what: { ref: "it" },
          abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }] }] },
    ],
  }]);
  rejects(text.replace("put a number of time counters", "put a number of stun counters"), "Enchantment — Saga");
});


test("the entrant's controller announces the target, then those two permanents exchange control", () => {
  const text = "Whenever an artifact, creature, or enchantment enters, its controller chooses target permanent another player controls that shares a card type with it. Exchange control of those permanents.";
  const ability = parsed(text, "Enchantment");
  assert.ok(ability.kind === "triggered");
  assert.deepEqual(ability.trigger, { trigger: "enters",
    what: { ref: "filter", count: one, filter: { allOf: [{ anyOf: [
      { types: ["artifact"] }, { types: ["creature"] }, { types: ["enchantment"] },
    ] }] } } });
  const [declaration, exchange] = ability.effects;
  const [choose] = declaration.steps;
  assert.ok(choose.effect === "declare-target" && "ref" in choose.what && choose.what.ref === "target");
  const { selectionId, ...target } = choose.what;
  assert.ok(selectionId);
  const entrant = { ref: "trigger-event-object", event: "enter" };
  const controller = { player: "controller", of: entrant };
  assert.deepEqual(target, { ref: "target", count: one, chosenBy: controller,
    filter: { cls: "permanent", controlledBy: { player: "filtered", base: { player: "each-player" },
      filter: { otherThan: controller } },
      characteristicRelations: [{ relation: "shared-value", characteristic: "card-type",
        with: { operand: "object", what: entrant } }] } });
  assert.deepEqual(exchange.steps, [{ effect: "exchange-control", what: [
    entrant, { ref: "choice-result", number: "singular", filter: { cls: "permanent" }, which: "chosen", scope: "contextual" },
  ] }]);
  rejects(text.replace("an artifact, creature, or enchantment enters", "one or more creatures enter"), "Enchantment");
  rejects(text.replace("another player", "an opponent"), "Enchantment");
});

test("a selected player pair imposes reciprocal attacker requirements through your next turn", () => {
  const text = "II — Choose two target players. Until your next turn, each creature they control attacks the other chosen player each combat if able.";
  const ability = parsed(text, "Enchantment — Saga");
  assert.ok(ability.kind === "chapter");
  const [selection, attack] = ability.effects;
  const [choose] = selection.steps;
  assert.ok(choose.effect === "declare-target" && "player" in choose.what && choose.what.player === "target-player");
  assert.ok(choose.what.selectionId);
  assert.deepEqual(choose.what.count, two);
  const pair = { player: "those-players" };
  assert.deepEqual(attack.steps, [{
    effect: "combat-rule", what: { ref: "each", filter: { types: ["creature"], controlledBy: pair } },
    rule: { rule: "attack-requirement", frequency: "each-combat",
      defender: { defender: "other-player-in-pair", pair, comparedTo: "attacker-controller" } },
    duration: { duration: "your-next-turn" },
  }]);
  rejects(text.replace("two target players", "target player"), "Enchantment — Saga");
  rejects(text.replace("two target players", "two target opponents"), "Enchantment — Saga");
});
