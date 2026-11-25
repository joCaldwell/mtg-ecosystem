// Single-template recognisers: per-player choices and grouped actions.
import { isDeepStrictEqual } from "node:util";
import type { Effect, Sentence } from "../../../ast.ts";
import { isPluralPlayerRef } from "../../../parser/refs.ts";
import type { Analyzer } from "../analyzer.ts";
import { chosenRange } from "../choices.ts";
import { child, combine, impliesPermanent, objectCardinality, type Candidate, type Context } from "../context.ts";

/** CR 118.12a and 608.2e–f: each opponent separately decides whether to
 * pay the fixed discard, and loss applies only to the nonpayers. */
export function opponentDiscardOrLife(this: Analyzer, sentence: Sentence, path: string, context: Context): boolean {
  const [loss] = sentence.steps;
  const condition = sentence.condition;
  if (context.barrier || context.decider || context.phase !== "resolution" || sentence.conditionKind !== "unless" ||
      Object.keys(sentence).some(key => !["steps", "condition", "conditionKind"].includes(key)) ||
      sentence.steps.length !== 1 || loss.effect !== "lose-life" || Object.keys(loss).length !== 3 ||
      loss.amount.amount !== "fixed" || !isDeepStrictEqual(loss.who, { player: "each-opponent" }) ||
      condition?.condition !== "pays" || !isDeepStrictEqual(condition.who, { player: "they" }) ||
      Object.keys(condition).length !== 3 || condition.cost.length !== 1) return false;
  const [cost] = condition.cost;
  if (cost.cost !== "discard" || cost.what === "hand" || cost.what.ref !== "filter" || cost.what.count.amount !== "fixed" ||
      !Number.isSafeInteger(cost.what.count.value) || cost.what.count.value < 1 ||
      !isDeepStrictEqual(cost, { cost: "discard", what: { ref: "filter", filter: { cls: "card" }, count: cost.what.count } })) return false;
  const at = `${path}/steps/0`;
  const opponents = this.player(loss.who, child(at, "who"), context);
  if (!opponents) return false;
  const rules = ["101.4", "101.4a", "118.11", "118.12", "118.12a", "608.2e", "608.2f", "701.9a"];
  const conditionPath = child(path, "condition");
  const id = this.node({ id: this.id(conditionPath, "paying-opponent"), path: conditionPath,
    node: "instruction-member", scope: context.ability, group: opponents.id, domain: "player", rules });
  const member: Candidate = { id, cardinality: "one", domain: "player" };
  this.walk(condition, conditionPath, { ...context, actor: member });
  const chooser = this.at(`${conditionPath}/cost/0/what`);
  if (!chooser) {
    this.diagnostic("unsupported-scope", conditionPath, "Each opponent's discard must have a bound payment selection.", rules);
    return true;
  }
  const payment = this.event(conditionPath, context, "pay-discard", [member, chooser],
    { optional: true, decider: member.id, ...(context.conditional ? { conditional: true } : {}) });
  const event = this.event(at, context, "lose-life", [member], { conditional: true });
  this.node({ id: this.id(path, "opponent-discard-or-life"), path, node: "opponent-discard-or-life",
    scope: context.ability, opponents: opponents.id, payer: member.id, payment,
    loss: event, cards: cost.what.count.value, life: loss.amount.value, lossWhen: "did-not-pay",
    order: "apnap-private-choices-then-discards-then-losses", rules });
  this.engineFeature("multiplayer-choice", path,
    "For each opponent, offer the printed discard in APNAP order, then apply the payment and life-loss instructions to their respective participants.", rules);
  context.objects = context.objects.map(candidate => ({ ...candidate, qualifiedOnly: true }));
  context.departed = context.departed?.map(candidate => ({ ...candidate, qualifiedOnly: true }));
  context.players = [];
  context.definiteEventPlayer = undefined;
  context.quantities = [];
  context.uncertainMeasureOrigin = true;
  context.mentionedObject = true;
  return true;
}

/** Compare to the exact permanent paid by the controller, then track
 * each opponent's own optional payment separately. CR 118.12, 608.2e–h. */
export function matchingSacrifice(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (sentences.length !== 3 || sentences.some(sentence => sentence.steps.length !== 1) ||
      Object.keys(sentences[0]).length !== 1 || Object.keys(sentences[1]).length !== 3 || Object.keys(sentences[2]).length !== 1 ||
      sentences[1].conditionKind !== "if" || !isDeepStrictEqual(sentences[1].condition, { condition: "you-do" }) ||
      sentences[1].otherwise || sentences[1].afterConditional) return false;
  const [[payment], [matching], [outcome]] = sentences.map(sentence => sentence.steps);
  const you = { player: "you" } as const;
  const opponents = { player: "each-opponent" } as const;
  const ref = { ref: "it" } as const;
  const test = { relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: ref } } as const;
  const nonPayers = { player: "filtered", base: opponents, filter: { precedingAction: "not-performed" } } as const;
  if (payment.effect !== "sacrifice" || payment.what.ref !== "filter" || objectCardinality(payment.what) !== "one" ||
      !impliesPermanent(payment.what.filter) || !isDeepStrictEqual(payment, { effect: "sacrifice", who: you, what: payment.what, optional: true }) ||
      matching.effect !== "sacrifice" || !isDeepStrictEqual(matching, { effect: "sacrifice", who: opponents,
        what: { ref: "filter", filter: { cls: "permanent", characteristicRelations: [test] }, count: { amount: "fixed", value: 1 } },
        chosenBy: opponents, decidedBy: opponents, optional: true,
      }) || outcome.effect !== "for-each" || outcome.effects.length !== 1 || Object.keys(outcome.effects[0]).length !== 1 ||
      !isDeepStrictEqual(outcome, { effect: "for-each", each: nonPayers, processing: "instruction-wise", effects: outcome.effects })) return false;
  const [loss, draw] = outcome.effects[0].steps;
  if (outcome.effects[0].steps.length !== 2 || loss.effect !== "lose-life" || draw.effect !== "draw" ||
      loss.amount.amount !== "fixed" || draw.amount.amount !== "fixed" ||
      !isDeepStrictEqual(loss, { effect: "lose-life", who: { player: "that-player" }, amount: loss.amount }) ||
      !isDeepStrictEqual(draw, { effect: "draw", who: you, amount: draw.amount })) return false;
  const at = `${path}/0/steps/0`;
  this.effect(payment, at, context);
  const source = this.at(child(at, "what"));
  if (!source) return true;
  const rules = ["101.4", "118.11", "118.12", "121.2", "608.2e", "608.2f", "608.2h", "701.21a"];
  const guard = `${path}/1/condition`;
  this.node({ id: this.id(guard, "decision-link"), path: guard, node: "decision-link", scope: context.ability,
    condition: "you-do", decision: at, rules });
  const choice = `${path}/1/steps/0`;
  const group = this.player(opponents, child(choice, "who"), context);
  if (!group) return true;
  const payer = this.node({ id: this.id(choice, "sacrifice-member"), path: choice, node: "instruction-member",
    scope: context.ability, group: group.id, domain: "player", rules });
  this.bound(opponents, child(choice, "chosenBy"), payer, rules);
  this.bound(opponents, child(choice, "decidedBy"), payer, rules);
  const actor: Candidate = { id: payer, domain: "player", cardinality: "one" };
  const what = child(choice, "what");
  const selected = this.object(matching.what, what, { ...context, actor,
    boundOperand: { path: `${what}/filter/characteristicRelations/0/with/what`, candidate: source, rules } });
  if (!selected) return true;
  const each = this.event(choice, context, "sacrifice", [selected, actor], { optional: true, decider: payer, conditional: true });
  const next = `${path}/2/steps/0`;
  const result = this.node({ id: this.id(next, "nonpaying-opponents"), path: next, node: "optional-action-participants",
    scope: context.ability, event: each, group: group.id, outcome: "did-not-pay", activation: this.id(at, "event"),
    cardinality: "many", rules });
  this.bound(opponents, `${next}/each/base`, group.id, rules);
  this.bound(nonPayers, child(next, "each"), result, rules);
  const member = this.node({ id: this.id(next, "nonpayer-member"), path: next, node: "instruction-member",
    scope: context.ability, group: result, domain: "player", rules });
  const lossPath = `${next}/effects/0/steps/0`;
  const drawPath = `${next}/effects/0/steps/1`;
  const receiver = this.bound({ player: "that-player" }, child(lossPath, "who"), member, rules);
  const drawer = this.bound(you, child(drawPath, "who"), context.you, rules);
  const lose = this.event(lossPath, context, "lose-life", [receiver], { conditional: true });
  const cards = this.event(drawPath, context, "draw", [drawer, receiver], { conditional: true });
  this.node({ id: this.id(next, "optional-matching-sacrifice"), path: next, node: "optional-matching-sacrifice",
    scope: context.ability, controllerPayment: this.id(at, "event"), original: source.id, opponents: group.id,
    opponentPayment: each, nonpayers: result, lose, life: loss.amount.value, draw: cards, cards: draw.amount.value,
    requireOriginalPayment: true, rules });
  this.engineFeature("multiplayer-choice", next,
    "Only after the controller pays, each opponent may sacrifice a permanent sharing a type with the paid permanent. Apply loss and draw a card per nonpayer.", rules);
  context.objects = [];
  context.departed = [];
  context.players = [];
  context.quantities = [];
  return true;
}

export function distinctAction(this: Analyzer, effect: Effect, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote || effect.decidedBy) return false;
  if (effect.effect === "exchange-values") {
    if (effect.a.value !== "life-total" || effect.b.value !== "toughness") return false;
    this.walk(effect.a, child(path, "a"), context);
    this.walk(effect.b, child(path, "b"), context);
    const player = this.at(`${path}/a/of`);
    const object = this.at(`${path}/b/of`);
    if (player?.domain !== "player" || object?.domain !== "object" ||
        player.cardinality !== "one" || object.cardinality !== "one") return false;
    const rules = ["609.3", "701.12g", "613.4b"];
    const event = this.event(path, context, effect.effect, [player, object], context.conditional ? { conditional: true } : {});
    this.node({ id: this.id(path, "value-exchange"), path, node: "value-exchange", scope: context.ability,
      subjects: [player.id, object.id], event, operation: "life-and-toughness", rules });
    this.engineFeature("value-exchange", path, "Exchange the two measured values only if both changes can be made, using the stated life and toughness rules.", rules);
    context.players = combine(context.players, [player]);
    context.objects = combine(context.objects, [object]);
    return true;
  }
  if (effect.effect === "targeting-as-though" && effect.lacking === "hexproof" && effect.by?.player === "you" &&
      effect.what.every(ref => "player" in ref ? ref.player === "each-opponent" : ref.ref === "each" || ref.ref === "this" || ref.ref === "self")) {
    // CR 611.2c: rules effects evaluate their applicability as the rule is used.
    this.walk(effect.what, child(path, "what"), { ...context, targetLegalityQuery: true });
    this.walk(effect.by, child(path, "by"), context);
    this.walk(effect.duration, child(path, "duration"), context);
    const affected = effect.what.flatMap((_, index) => {
      const member = this.at(`${path}/what/${index}`);
      return member ? [member.id] : [];
    });
    if (affected.length !== effect.what.length) return false;
    const rules = ["611.2a", "611.2c", "702.11b", "702.11c", "702.11e"];
    this.node({ id: this.id(path, "targeting-permission"), path, node: "targeting-permission", scope: context.ability,
      controller: context.you, affected, duration: effect.duration, evaluation: "each-target-legality-check", rules });
    this.engineFeature("targeting-permission", path, "Ignore only the printed targeting restriction for the bound controller's spells and abilities during the stated duration.", rules);
    return true;
  }
  return false;
}

export function groupedDraw(this: Analyzer, effect: Extract<Effect, { effect: "draw" }>, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote ||
      effect.who.player !== "group" || effect.amount.amount !== "fixed" ||
      !effect.who.members.every(player => player.player === "you" ||
        ((player.player === "target-opponent" || player.player === "target-player") && !isPluralPlayerRef(player)))) return false;
  const players = effect.who.members.flatMap((ref, index) => {
    const player = this.player(ref, `${path}/who/members/${index}`, context);
    return player ? [player] : [];
  });
  if (players.length !== effect.who.members.length) return false;
  const rules = ["101.4", "121.2c", "608.2e"];
  const event = this.event(path, context, effect.effect, players, context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(path, "group-instruction"), path, node: "group-instruction", scope: context.ability, event,
    execution: { kind: "draw", players: players.map(player => player.id), amount: effect.amount.value, order: "apnap" }, rules });
  // The group is not one player and the draw is not one player's card.
  // A later reference to an aggregate draw result needs a separate rule.
  const drawn = this.eventResult(path, event, "objects", "unknown", "new-objects", { cls: "card" });
  this.invalidNodes.set(drawn.id, "unsupported-scope");
  context.objects = combine(context.objects, [drawn]);
  context.players = combine(context.players, players.filter(player => player.id !== context.you));
  context.mentionedObject = true;
  this.engineFeature("group-instruction", path, "Draw for each printed participant occurrence in APNAP order.", rules);
  return true;
}

export function commanderManaRider(this: Analyzer, effect: Extract<Effect, { effect: "add-mana" }>, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || effect.who.player !== "you" ||
      !isDeepStrictEqual(effect.mana, { mana: "commander-color", amount: { amount: "fixed", value: 1 }, whose: { player: "you" } }) ||
      effect.riders?.length !== 1) return false;
  const [rider] = effect.riders;
  if (rider.rider !== "delayed-trigger" || rider.spentOn.length !== 1 ||
      !isDeepStrictEqual(rider.spentOn[0], { use: "cast", filter: { types: ["creature"], cls: "spell",
        characteristicRelations: [{ relation: "shared-value", characteristic: "creature-type", with: {
          operand: "object", what: { ref: "commander", owner: { player: "you" }, count: { amount: "fixed", value: 1 } },
        } }],
      } }) || !isDeepStrictEqual(rider.effects, [{ steps: [{ effect: "scry", amount: { amount: "fixed", value: 1 } }] }]))
    return false;
  const rules = ["106.6a", "205.3m", "601.2h", "601.2i", "603.7e", "702.124c", "702.124e", "903.3e", "903.4"];
  const actor = this.player(effect.who, child(path, "who"), context);
  if (!actor) return true;
  this.walk(effect.mana, child(path, "mana"), context);
  const event = this.event(path, context, effect.effect, [actor], context.conditional ? { conditional: true } : {});
  const at = `${path}/riders/0`;
  const commander = `${at}/spentOn/0/filter/characteristicRelations/0/with/what`;
  const commandSet = this.node({ id: this.id(commander, "commander-set"), path: commander,
    node: "commander-set", scope: context.ability, owner: actor.id, cardinality: "many", rules });
  this.bound({ player: "you" }, child(commander, "owner"), actor.id, rules);
  this.bound({ ref: "commander", owner: { player: "you" }, count: { amount: "fixed", value: 1 } }, commander, commandSet, rules);
  const delayed = this.nestedAbility(at, context, "delayed-triggered", rules);
  this.node({ id: this.id(at, "commander-mana-rider"), path: at, node: "commander-mana-rider",
    scope: context.ability, manaEvent: event, commanderSet: commandSet, trigger: delayed.ability,
    comparison: "shares-creature-type-with-any", evaluate: "when-mana-spent-on-cast", occurrence: "each-mana-produced", rules });
  this.sentences(rider.effects, child(at, "effects"), delayed);
  this.engineFeature("commander-mana-rider", path,
    "Track the produced mana and test the paid-for creature spell against either designated commander.", rules);
  return true;
}

/** CR 603.10a and 608.2i: group actual damage by the source
 * controller at damage time and retain the departed recipient incarnation. */
export function historicalDamageDraw(this: Analyzer, effect: Extract<Effect, { effect: "draw" }>, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || !context.sourceMoved || !context.ownLeaveEvent ||
      context.decider || Object.keys(effect).length !== 3 ||
      !isDeepStrictEqual(effect.who, { player: "each-player" }) ||
      !isDeepStrictEqual(effect.amount, { amount: "damage-history-total", to: { ref: "self" }, during: "this-turn",
        sourceControllerAtDamage: { player: "they" } })) return false;
  const players = this.player(effect.who, child(path, "who"), context);
  if (!players) return false;
  const rules = ["120.4b", "121.2c", "121.2d", "400.7", "603.10a", "608.2i"];
  const amount = child(path, "amount");
  const member = this.node({ id: this.id(amount, "draw-member"), path: amount, node: "instruction-member",
    scope: context.ability, group: players.id, domain: "player", rules });
  this.bound({ ref: "self" }, child(amount, "to"), context.source, rules);
  this.bound({ player: "they" }, child(amount, "sourceControllerAtDamage"), member, rules);
  const event = this.event(path, context, "draw", [players], context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(amount, "damage-history-draw"), path: amount, node: "damage-history-draw", scope: context.ability,
    event, player: member, target: context.source, departure: context.ownLeaveEvent, window: "this-turn",
    sourceController: "at-damage", order: "active-player-then-turn-order", rules });
  context.quantities = [];
  this.engineFeature("event-history", amount,
    "Total actual damage to this departed battlefield incarnation by the damage source's controller at the time of damage, separately for each drawing player.", rules);
  return true;
}

export function multiplayerChoices(this: Analyzer, effect: Extract<Effect, { effect: "choose-object" }>, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || context.decider || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || (effect.who.player !== "each-player" && effect.who.player !== "each-opponent") ||
      (effect.startingWith && effect.startingWith.player !== "you")) return false;
  const what = effect.what;
  if (!("ref" in what)) return false;
  const eachType = effect.who.player === "each-opponent" && !effect.startingWith && what.ref === "each" &&
    isDeepStrictEqual(effect.perPermanentType, { skipMissing: true, sameObjectForMultipleTypes: true }) &&
    isDeepStrictEqual(what.filter, { cls: "permanent", controlledBy: { player: "they" } });
  const ownNonlands = !effect.perPermanentType && !effect.startingWith && effect.who.player === "each-player" &&
    what.ref === "filter" && !what.random && !what.groupConstraints && what.count.amount === "fixed" &&
    isDeepStrictEqual(what.filter, { cls: "permanent", nonTypes: ["land"], controlledBy: { player: "they" } });
  const amongOpponents = !effect.perPermanentType && effect.startingWith?.player === "you" &&
    effect.who.player === "each-player" && what.ref === "filter" && !what.random && !what.groupConstraints &&
    isDeepStrictEqual(what.count, { amount: "fixed", value: 1 }) && what.upTo && what.filter.cls === "permanent" &&
    Object.keys(what.filter).every(key => ["cls", "properties", "fromAmong"].includes(key)) &&
    isDeepStrictEqual(what.filter.fromAmong, { ref: "each", filter: { cls: "permanent", control: "opponent" } });
  if (!eachType && !ownNonlands && !amongOpponents) return false;

  const rules = ["101.4", "110.4", "608.2d", "608.2e", "609.3"];
  const group = this.player(effect.who, child(path, "who"), context);
  if (!group) return true;
  const memberId = this.node({ id: this.id(path, "choice-member"), path, node: "instruction-member",
    scope: context.ability, group: group.id, domain: "player", rules });
  const member: Candidate = { id: memberId, domain: "player", cardinality: "one" };
  const selector = this.object(what, child(path, "what"), { ...context, actor: member });
  if (!selector) return true;
  const first = effect.startingWith ? this.player(effect.startingWith, child(path, "startingWith"), context) : undefined;
  const event = this.event(path, context, effect.effect, [member, selector], context.conditional ? { conditional: true } : {});
  const chosen = this.eventResult(path, event, "objects", "many", "same-objects", { cls: "permanent" });
  this.node({ id: this.id(path, "multiplayer-choice"), path, node: "multiplayer-choice", scope: context.ability, event,
    group: group.id, member: member.id, selector: selector.id, result: chosen.id,
    ...(first ? { startWith: first.id } : {}), choose: eachType ? "one-per-permanent-type" : "printed-count", rules });
  this.engineFeature("multiplayer-choice", path,
    "Make choices in the specified player order and remember each player's selections and their union.", rules);
  context.objects = combine(context.objects, [chosen]);
  context.groupChoice = { path, group, member, result: chosen, eachType, ownNonlands };
  context.quantities = [];
  context.uncertainMeasureOrigin = true;
  return true;
}

export function afterMultiplayerChoices(this: Analyzer, effect: Effect, path: string, context: Context, precedingEffect?: string): boolean {
  const choice = context.groupChoice;
  if (context.barrier || context.decider || context.phase !== "resolution" || !choice || choice.path !== precedingEffect ||
      effect.optional || effect.decidedBy || effect.forEach || effect.forEachVote) return false;
  const rules = ["101.4", "608.2c", "608.2d", "608.2f"];
  if (choice.ownNonlands && effect.effect === "destroy" && isDeepStrictEqual(effect.what, {
    ref: "all", filter: { cls: "permanent", nonTypes: ["land"], other: true },
  })) {
    const what = child(path, "what");
    const unchosen = this.node({ id: this.id(what, "unchosen-permanents"), path: what, node: "unchosen-permanents",
      scope: context.ability, chosen: choice.result.id, filter: { cls: "permanent", nonTypes: ["land"] },
      exclude: "all-player-choices", cardinality: "many", rules });
    this.bound({ ref: "them" }, `${what}/filter/other`, choice.result.id, rules);
    const selection = this.bound(effect.what, what, unchosen, rules);
    this.event(path, context, effect.effect, [selection], context.conditional ? { conditional: true } : {});
  } else if (choice.eachType && effect.effect === "sacrifice" &&
      effect.who.player === "each-opponent" && !effect.chosenBy && effect.what.ref === "rest" && isDeepStrictEqual(effect.what, {
        ref: "rest", exclude: "own-preceding-choices", of: { ref: "each", filter: { cls: "permanent", controlledBy: { player: "they" } } },
      })) {
    const what = child(path, "what");
    if (!effect.what.of) return false;
    this.bound(effect.who, child(path, "who"), choice.group.id, rules);
    this.object(effect.what.of, child(what, "of"), { ...context, actor: choice.member });
    const unchosen = this.node({ id: this.id(what, "unchosen-permanents"), path: what, node: "unchosen-permanents",
      scope: context.ability, chosen: choice.result.id, member: choice.member.id,
      filter: { cls: "permanent", controlledBy: { player: "they" } }, exclude: "own-choices", cardinality: "many", rules });
    const selection = this.bound(effect.what, what, unchosen, rules);
    this.event(path, context, effect.effect, [choice.member, selection], context.conditional ? { conditional: true } : {});
  } else return false;
  this.engineFeature("multiplayer-choice", path,
    "Apply the action to the matching permanents outside the precisely bound preceding selections.", rules);
  context.objects = [];
  context.groupChoice = undefined;
  context.quantities = [];
  context.uncertainMeasureOrigin = true;
  context.mentionedObject = true;
  return true;
}

export function targetedPlayerAction(this: Analyzer, effect: Effect, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || context.decider || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || (effect.effect !== "lose-life" && effect.effect !== "sacrifice"))
    return false;
  const who = effect.who;
  if ((who.player !== "target-player" && who.player !== "target-opponent") || !who.selectionId ||
      !chosenRange(who.count) || chosenRange(who.count)?.maximum === 1) return false;
  if (effect.effect === "lose-life" && effect.amount.amount !== "fixed") return false;
  if (effect.effect === "sacrifice" &&
      (effect.what.ref !== "filter" || objectCardinality(effect.what) !== "one" || effect.what.random ||
        (effect.chosenBy && !isDeepStrictEqual(effect.chosenBy, who)))) return false;
  const rules = ["101.4", "115.3", "601.2c", "602.2b", "608.2b", "608.2f",
    effect.effect === "sacrifice" ? "701.21a" : "119.3"];
  const group = this.player(who, child(path, "who"), { ...context, targetPlayerGroup: true });
  if (!group) return true;
  const memberId = this.node({ id: this.id(path, "targeted-player-member"), path, node: "instruction-member",
    scope: context.ability, group: group.id, domain: "player", rules });
  const member: Candidate = { id: memberId, domain: "player", cardinality: "one" };
  const operands: Candidate[] = [member];
  if (effect.effect === "sacrifice") {
    const what = this.object(effect.what, child(path, "what"), { ...context, actor: member });
    if (what) operands.push(what);
    if (effect.chosenBy) this.bound(effect.chosenBy, child(path, "chosenBy"), memberId, rules);
  }
  const event = this.event(path, context, effect.effect, operands, context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(path, "group-instruction"), path, node: "group-instruction", scope: context.ability, event,
    execution: { kind: "for-each", domain: group.id, member: memberId, membership: "announcement", processing: "instruction-wise" }, rules });
  this.engineFeature("group-instruction", path,
    "Apply this instruction to every still-legal member of the announced player group, separately binding their choices.", rules);
  this.targetedPlayerGroups.add(group.id);
  context.players = combine(context.players, [group]);
  if (effect.effect === "lose-life") {
    context.measuredEvents = [...(context.measuredEvents ?? []), { operation: "lose-life", event }];
    const amounts = this.eventResult(path, event, "quantity", "unknown", "measured-quantity");
    this.invalidNodes.set(amounts.id, "unsupported-scope");
    context.quantities = combine(context.quantities, [amounts]);
  }
  return true;
}

export function relativePlayerAction(this: Analyzer, effect: Effect, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || effect.optional || context.decider ||
      effect.decidedBy || effect.forEach || effect.forEachVote ||
      (effect.effect !== "sacrifice" && effect.effect !== "create-token" && effect.effect !== "untap") ||
      effect.who?.player !== "filtered" || effect.who.random) return false;
  const { who } = effect;
  const attacked = context.attackedTriggerPlayer;
  const againstAttacked = attacked && !context.actor &&
    context.players.filter(player => !this.weakPlayers.has(player.id)).every(player => player.id === attacked.id) &&
    who.base.player === "each-opponent" &&
    isDeepStrictEqual(who.filter, { attackingPlayer: { player: "that-player" } }) &&
    (effect.effect === "create-token" && effect.count.amount === "fixed" ||
      effect.effect === "untap" && effect.what.ref === "all" && effect.what.filter.controlledBy?.player === "they" &&
        !effect.what.filter.control);
  const otherPlayers = effect.effect === "sacrifice" && who.base.player === "each-player" &&
    isDeepStrictEqual(who.filter, { other: true }) && isDeepStrictEqual(effect.chosenBy, who) &&
    effect.what.ref === "filter" && objectCardinality(effect.what) === "one";
  if (!againstAttacked && !otherPlayers) return false;

  const rules = againstAttacked ? ["102.2", "508.1k", "508.3b", "608.2f", "805.10b"]
    : ["101.4", "109.5", "608.2f", "701.21a"];
  const at = child(path, "who");
  const base = this.player(who.base, child(at, "base"), context);
  if (!base) return true;
  const filter = againstAttacked ? { kind: "attacking" as const, player: attacked!.id }
    : { kind: "other-than" as const, player: context.you };
  const group = this.node({ id: this.id(at, "relative-players"), path: at, node: "relative-player-group",
    scope: context.ability, base: base.id, filter, cardinality: "many", rules });
  const member = this.node({ id: this.id(at, "instruction-member"), path: at, node: "instruction-member",
    scope: context.ability, group, domain: "player", rules });
  this.bound(who, at, member, rules);
  if (againstAttacked) {
    this.bound({ player: "that-player" }, `${at}/filter/attackingPlayer`, attacked!.id, rules);
  } else if (effect.effect === "sacrifice" && effect.chosenBy) {
    this.bound({ player: "you" }, `${at}/filter/other`, context.you, rules);
    this.bound(who.base, `${path}/chosenBy/base`, base.id, rules);
    this.bound({ player: "you" }, `${path}/chosenBy/filter/other`, context.you, rules);
    this.bound(effect.chosenBy, child(path, "chosenBy"), member, rules);
  }
  const scope: Context = { ...this.branch(context), actor: { id: member, domain: "player", cardinality: "one" } };
  for (const [key, value] of Object.entries(effect)) {
    if (key !== "who" && key !== "chosenBy") this.walk(value, child(path, key), scope);
  }
  const object = this.at(child(path, "what"));
  const event = this.event(path, scope, effect.effect, [scope.actor!, ...(object ? [object] : [])],
    context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(path, "group-instruction"), path, node: "group-instruction", scope: context.ability, event,
    execution: { kind: "for-each", domain: group, member, membership: "resolution", processing: "instruction-wise" }, rules });
  this.engineFeature("group-instruction", path,
    "Evaluate the specified player membership and apply the instruction independently to each player, preserving simultaneous outcomes.", rules);

  // The action did not select a single player or object for a later pronoun.
  context.objects = [];
  context.players = [];
  context.mentionedObject = true;
  return true;
}

export function damageByEachSource(this: Analyzer, effect: Extract<Effect, { effect: "damage" }>, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || !context.spellAbility || context.conditional || context.decider ||
      effect.optional || effect.forEach || effect.forEachVote || effect.decidedBy || context.mentionedObject ||
      context.objects.length || context.departed?.length || effect.source.ref !== "each" ||
      effect.to.to !== "object" || !isDeepStrictEqual(effect.to.ref, { ref: "it" }) ||
      effect.amount.amount !== "attribute" || effect.amount.attribute !== "power" ||
      !isDeepStrictEqual(effect.amount.of, { ref: "it" })) return false;
  const sourcePath = child(path, "source");
  const group = this.object(effect.source, sourcePath, context);
  if (!group) return true;
  const rules = ["109.2", "120.4b", "608.2f", "608.2h"];
  const member = this.node({ id: this.id(sourcePath, "damage-member"), path: sourcePath,
    node: "instruction-member", scope: context.ability, group: group.id, domain: "object", rules });
  const amountPath = child(path, "amount");
  this.bound(effect.amount.of, child(amountPath, "of"), member, rules);
  this.bound(effect.to.ref, `${path}/to/ref`, member, rules);
  const event = this.event(path, context, effect.effect, [group]);
  this.node({ id: this.id(path, "source-dependent-damage"), path, node: "source-dependent-damage", scope: context.ability,
    event, group: group.id, sourceAndRecipient: member, characteristic: "power", timing: "simultaneous", rules });
  context.objects = [group];
  context.mentionedObject = true;
  context.measuredEvents = [...(context.measuredEvents ?? []), { operation: "damage", event }];
  this.engineFeature("source-dependent-damage", path, "Read each creature's power and deal its component of the simultaneous damage event to that creature.", rules);
  return true;
}
