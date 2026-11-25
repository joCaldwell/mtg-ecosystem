// Single-template recognisers: whole-ability shapes.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Effect, ObjectRef, Sentence } from "../../../ast.ts";
import type { Analyzer } from "../analyzer.ts";
import { child, combine, impliesPermanent, objectCardinality, type Context } from "../context.ts";

/** CR 608.2d–f and 701.14b. Capture the original creature set
 * before creation, then pair only actual tokens with distinct creatures. */
export function fightCreatedTokens(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || sentences.length !== 2 ||
      sentences.some(sentence => Object.keys(sentence).length !== 1 || sentence.steps.length !== 1)) return false;
  const [[loop], [fight]] = sentences.map(sentence => sentence.steps);
  const creatures = { ref: "each", filter: { types: ["creature"], control: "opponent" } } satisfies ObjectRef;
  const tokenRef = { ref: "that", plural: true, filter: { cls: "token" } } as const;
  const opponentRef = { ref: "that", plural: true, filter: { types: ["creature"] } } satisfies ObjectRef;
  if (loop.effect !== "for-each" || loop.effects.length !== 1 || Object.keys(loop.effects[0]).length !== 1 ||
      loop.effects[0].steps.length !== 1 ||
      !isDeepStrictEqual(loop, { effect: "for-each", each: creatures, effects: loop.effects, processing: "instruction-wise" }) ||
      !isDeepStrictEqual(fight, { effect: "fight", a: tokenRef, b: opponentRef,
        pairing: { kind: "distinct-pairs", who: { player: "you" }, inputs: "preceding-token-loop", unmatched: "omit" },
      })) return false;
  const [create] = loop.effects[0].steps;
  if (create.effect !== "create-token" || !("types" in create.token) || !create.token.types.includes("creature") ||
      !isDeepStrictEqual(create, { effect: "create-token", count: { amount: "fixed", value: 1 }, token: create.token })) return false;
  const at = `${path}/0/steps/0`;
  const group = this.object(creatures, child(at, "each"), context);
  if (!group) return true;
  const template = `${at}/effects/0/steps/0`;
  this.effect(create, template, this.branch(context));
  const rules = ["101.4", "608.2d", "608.2e", "608.2f", "609.3", "701.14a", "701.14b"];
  const member = this.node({ id: this.id(at, "creature-member"), path: at, node: "instruction-member",
    scope: context.ability, group: group.id, domain: "object", rules });
  const event = this.id(template, "event");
  this.node({ id: this.id(at, "group-instruction"), path: at, node: "group-instruction", scope: context.ability, event,
    execution: { kind: "for-each", domain: group.id, member, membership: "resolution", processing: "instruction-wise" }, rules });
  const tokens = this.eventResult(child(template, "batch"), event, "objects", "many", "new-objects", { cls: "token" });
  const next = `${path}/1/steps/0`;
  this.bound(tokenRef, child(next, "a"), tokens.id, rules);
  this.bound(opponentRef, child(next, "b"), group.id, rules);
  const chooser = this.bound({ player: "you" }, `${next}/pairing/who`, context.you, rules);
  const fightEvent = this.event(next, context, "fight", [tokens, group, chooser]);
  this.node({ id: this.id(next, "token-fight-pairing"), path: next, node: "token-fight-pairing", scope: context.ability,
    event: fightEvent, creation: event, tokens: tokens.id, originals: group.id, chooser: chooser.id,
    pairing: "distinct-one-to-one-omit-unmatched", combatants: "same-live-creatures", order: "simultaneous", rules });
  this.engineFeature("group-instruction", next,
    "Create tokens for the snapshot count, pair actual token results with distinct original creatures, and fight simultaneously only with pairs still eligible.", rules);
  context.objects = [];
  context.quantities = [];
  context.mentionedObject = true;
  return true;
}

export function numericRoll(this: Analyzer, effect: Extract<Effect, { effect: "roll-dice" }>, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || Object.keys(effect).length !== 4 ||
      effect.die.die !== "numeric" || !Number.isSafeInteger(effect.die.sides) || effect.die.sides < 2 ||
      !isDeepStrictEqual(effect.count, { amount: "fixed", value: 1 })) return false;
  const roller = this.player(effect.who, child(path, "who"), context);
  if (!roller || roller.cardinality !== "one") return false;
  const rules = ["706.1", "706.2", "706.3", "706.6"];
  const event = this.event(path, context, effect.effect, [roller], context.conditional ? { conditional: true } : {});
  const result = this.eventResult(path, event, "quantity", "one", "measured-quantity");
  this.node({ id: this.id(path, "numeric-roll"), path, node: "numeric-roll", scope: context.ability, event,
    roller: roller.id, sides: effect.die.sides, result: result.id, stage: "final-kept-result", rules });
  context.dieResults = [...(context.dieResults ?? []), result];
  this.engineFeature("numeric-roll", path, "Resolve a single numeric roll using the final result after applicable modifiers and ignored rolls.", rules);
  return true;
}

export function randomAuraAttachment(this: Analyzer, effect: Extract<Effect, { effect: "attach" }>, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || !context.permanentSource || context.sourceMoved ||
      Object.keys(effect).length !== 4 || effect.who?.player !== "you" ||
      !isDeepStrictEqual(effect.what, { ref: "this", noun: "aura" }) ||
      !isDeepStrictEqual(effect.to, { player: "filtered", base: { player: "an-opponent" },
        filter: { otherThan: { player: "enchanted-player" } }, random: true })) return false;
  const rules = ["102.2", "303.4", "608.2d", "701.3a", "701.3b"];
  const who = this.player(effect.who, child(path, "who"), context);
  const aura = this.object(effect.what, child(path, "what"), context);
  const to = child(path, "to");
  const base = this.player({ player: "an-opponent" }, child(to, "base"), context);
  const excluded = this.player({ player: "enchanted-player" }, `${to}/filter/otherThan`, context);
  if (!who || !aura || !base || !excluded) return true;
  const choice = this.node({ id: this.id(to, "random-aura-attachment"), path: to, node: "random-aura-attachment",
    scope: context.ability, aura: aura.id, opponentOf: who.id, exclude: excluded.id, event: this.id(path, "event"),
    cardinality: "one", rules });
  const recipient = this.bound(effect.to, to, choice, rules);
  this.event(path, context, effect.effect, [aura, who, recipient], context.conditional ? { conditional: true } : {});
  this.engineFeature("random-aura-attachment", path, "Choose another valid opponent at random without targeting and preserve the existing attachment when no choice exists.", rules);
  context.objects = combine(context.objects, [aura]);
  context.players = [];
  return true;
}

export function repeatHandPlacement(this: Analyzer, effect: Extract<Effect, { effect: "repeat-process" }>, path: string, context: Context,
  precedingEffect?: string): boolean {
  if (context.barrier || context.decider || effect.times || effect.optional || effect.decidedBy || effect.forEach ||
      effect.forEachVote || context.performedRepeat?.path !== path || !precedingEffect || !context.precedingSentence) return false;
  const { path: previous, steps } = context.precedingSentence;
  if (steps.length !== 1 || context.performedRepeat.decision !== `${previous}/steps/0`) return false;
  const [entry] = steps;
  const draw = this.result.nodes.find(node => node.id === this.id(precedingEffect, "event"));
  if (entry.effect !== "move-zone" || !entry.optional || entry.decidedBy || entry.who || !entry.tapped ||
      entry.forEach || entry.forEachVote || entry.to.zone !== "battlefield" ||
      entry.what.ref !== "filter" || objectCardinality(entry.what) !== "one" ||
      !isDeepStrictEqual(entry.what.filter, { types: ["land"], cls: "card", zone: { zone: "hand", owner: "your" } }) ||
      draw?.node !== "event" || draw.operation !== "draw" || draw.scope !== context.ability || draw.optional ||
      !isDeepStrictEqual(draw.inputs, [context.you])) return false;
  const rules = ["118.12", "608.2c", "608.2d", "609.3"];
  this.node({ id: this.id(path, "repeat-hand-placement"), path, node: "repeat-hand-placement", scope: context.ability,
    place: this.id(context.performedRepeat.decision, "event"), draw: draw.id, continueWhen: "placement-performed", rules });
  this.engineFeature("repeat-hand-placement", path, "Offer the same placement choice anew, drawing and repeating only when it is performed.", rules);
  return true;
}

/** CR 508.3c and 603.4: both checks count qualifying members of this
 * declaration; creatures put onto the battlefield attacking are distinct. */
export function attackDeclarationGuard(this: Analyzer, ability: Extract<Ability, { kind: "triggered" }>, path: string, context: Context): boolean {
  const { trigger, condition } = ability;
  const attackers = { ref: "that", filter: { types: ["creature"] }, plural: true } as const;
  const attacking = [
    { defender: "player", who: { player: "you" } },
    { defender: "object", what: { ref: "each", filter: { types: ["planeswalker"], control: "you" } } },
  ];
  if (context.barrier ||
      !isDeepStrictEqual(trigger, { trigger: "player-attacks", who: { player: "an-opponent" }, per: "declaration", with: { types: ["creature"] } }) ||
      condition?.condition !== "count" || condition.comparison.op !== "ge" || condition.comparison.value.amount !== "fixed" ||
      condition.comparison.value.value < 1 ||
      !isDeepStrictEqual(condition.of, { fromAmong: attackers, attackingLookup: "current-or-last-known", attacking })) return false;
  const at = child(path, "trigger");
  const actor = this.player({ player: "an-opponent" }, child(at, "who"), { ...context, phase: "trigger-match", eventPredicate: true });
  if (!actor) return false;
  const rules = ["506.4", "508.3c", "603.4", "608.2h"];
  const event = this.event(at, context, "trigger:player-attacks", [actor]);
  const group = this.eventResult(at, event, "objects", "many", "same-objects", { types: ["creature"] });
  this.node({ id: this.id(at, "attack-declaration"), path: at, node: "attack-declaration", scope: context.ability,
    event, actor: actor.id, attackers: group.id, filter: { types: ["creature"] }, rules });
  const conditionPath = child(path, "condition");
  this.bound({ ref: "that", filter: { types: ["creature"] }, plural: true }, `${conditionPath}/of/fromAmong`, group.id, rules);
  this.walk(attacking, `${conditionPath}/of/attacking`, { ...context, interveningIf: true });
  const planeswalkers = this.at(`${conditionPath}/of/attacking/1/what`);
  if (!planeswalkers) {
    this.diagnostic("unsupported-scope", conditionPath, "The declaration count requires the printed set of defending planeswalkers.", rules);
    return true;
  }
  this.node({ id: this.id(conditionPath, "attack-declaration-count"), path: conditionPath, node: "attack-declaration-count",
    scope: context.ability, attackers: group.id, defendingPlayer: context.you, defendingPlaneswalkers: planeswalkers.id,
    minimum: condition.comparison.value.value, lookup: "current-or-last-known", timing: "trigger-check-and-resolution", rules });
  this.engineFeature("attack-declaration", at,
    "Count only members of this declaration attacking the printed defenders, using current combat information or the departed creature's last known combat information, at both checks.", rules);
  context.players = [this.eventResult(at, event, "player", "one", "same-objects")];
  this.sentences(ability.effects, child(path, "effects"), context);
  return true;
}

/** CR 603.3d and 608.2b: the entrant's controller announces the
 * target. At resolution, test its restrictions again, then exchange the
 * same live battlefield objects only if both transfers can happen. */
export function entrantExchange(this: Analyzer, ability: Extract<Ability, { kind: "triggered" }>, path: string, context: Context): boolean {
  const { trigger, effects } = ability;
  const entrant = context.objects[0];
  const entry = { ref: "trigger-event-object", event: "enter" } as const;
  const controller = { player: "controller", of: entry } as const;
  const [declare] = effects[0]?.steps ?? [];
  const [exchange] = effects[1]?.steps ?? [];
  if (context.barrier || ability.condition || trigger.trigger !== "enters" ||
      trigger.grouping || trigger.what.ref !== "filter" || objectCardinality(trigger.what) !== "one" ||
      !impliesPermanent(trigger.what.filter) || context.objects.length !== 1 || entrant.cardinality !== "one" ||
      effects.length !== 2 || effects.some(sentence => Object.keys(sentence).length !== 1 || sentence.steps.length !== 1) ||
      declare?.effect !== "declare-target" || Object.keys(declare).length !== 2 ||
      !("ref" in declare.what) || declare.what.ref !== "target" ||
      !declare.what.selectionId || !isDeepStrictEqual({ ...declare.what, selectionId: undefined }, {
        ref: "target", count: { amount: "fixed", value: 1 }, chosenBy: controller, selectionId: undefined,
        filter: { cls: "permanent", controlledBy: { player: "filtered", base: { player: "each-player" }, filter: { otherThan: controller } },
          characteristicRelations: [{ relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: entry } }] },
      }) || !isDeepStrictEqual(exchange, { effect: "exchange-control", what: [
        entry, { ref: "choice-result", number: "singular", filter: { cls: "permanent" }, which: "chosen", scope: "contextual" },
      ] })) return false;

  this.sentences(effects, child(path, "effects"), { ...context, exchangeEntrant: entrant });
  const targetPath = `${path}/effects/0/steps/0/what`;
  const target = this.at(targetPath);
  const chooser = this.at(child(targetPath, "chosenBy"));
  const at = `${path}/effects/1/steps/0`;
  const affected = this.at(`${at}/what/1`);
  if (!target || !chooser || affected?.id !== target.id) {
    this.diagnostic("unsupported-scope", at, "The exchange needs the exact target announced by the entrant's controller.", ["603.3d", "701.12"]);
    return true;
  }
  const rules = ["603.2c", "603.3a", "603.3d", "608.2b", "608.2h", "701.12a", "701.12b", "702.11b"];
  this.node({ id: this.id(at, "entrant-control-exchange"), path: at, node: "entrant-control-exchange",
    scope: context.ability, entrant: entrant.id, chooser: chooser.id, target: target.id, event: this.id(at, "event"),
    targetRestrictions: "recheck-at-resolution", execution: "both-live-and-different-controllers", rules });
  this.engineFeature("entrant-control-exchange", at,
    "The entrant's controller announces a distinct-controller permanent sharing a card type. Recheck legality at resolution and exchange only when both transfers can occur.", rules);
  return true;
}

export function countedPermanentPool(this: Analyzer, ability: Extract<Ability, { kind: "triggered" }>, path: string, context: Context): void {
  const { condition, effects } = ability;
  if (context.barrier || context.objects.length || context.departed?.length || condition?.condition !== "count" ||
      condition.comparison.op !== "ge" || condition.comparison.value.amount !== "fixed" ||
      condition.of.cls !== "permanent" || !condition.of.counterConstraints?.length ||
      Object.keys(condition.of).some(key => !["cls", "control", "counterConstraints"].includes(key)) ||
      effects.length !== 1 || effects[0].steps.length !== 1 || Object.keys(effects[0]).length !== 1) return;
  const [destroy] = effects[0].steps;
  if (destroy.effect !== "destroy" || Object.keys(destroy).length !== 2 ||
      destroy.what.ref !== "from-pool" || !destroy.what.random || objectCardinality(destroy.what) !== "one" ||
      !isDeepStrictEqual(destroy.what.pool, { ref: "that", filter: { cls: "permanent" }, plural: true })) return;
  const at = `${path}/condition/of`;
  const rules = ["122.1", "603.4", "608.2d", "608.2h"];
  const id = this.node({ id: this.id(at, "intervening-pool"), path: at, node: "intervening-condition-pool",
    scope: context.ability, filter: condition.of, cardinality: "many", timing: "trigger-check-and-resolution", rules });
  context.objects = [{ id, domain: "object", cardinality: "many", description: condition.of }];
  this.engineFeature("intervening-condition-pool", at,
    "Recompute the current set at each intervening-if check and sample the one random choice from the resolving set.", rules);
}
