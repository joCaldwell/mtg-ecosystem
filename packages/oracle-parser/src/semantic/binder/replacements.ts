// Replacement and prevention effects.
import { isDeepStrictEqual } from "node:util";
import type { Effect, ReplacementInstruction, StaticEffect } from "../../ast.ts";
import type { BindingNode } from "../types.ts";
import type { Analyzer } from "./analyzer.ts";
import { child, combine, objectCardinality, type Candidate, type Context } from "./context.ts";
import { stackClasses } from "./predicates.ts";
import { exileReplacementParts, quantityUnit, simpleExileBody, singleGraveyardExile } from "./replacement-shapes.ts";
import { SELF_GRAVEYARD_REPLACEMENT, type OptionalEntryCopy } from "./templates.ts";

export function additionalStackCopy(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const { event, replace } = replacement;
  if (context.barrier || !context.permanentSource || replacement.optional || replacement.condition ||
      replacement.occurrence !== "each" || event.event !== "copy-spell" || event.who.player !== "you" ||
      !isDeepStrictEqual(event.quantity, { op: "ge", value: { amount: "fixed", value: 1 } }) ||
      replace.replacement !== "add-spell-copy") return false;
  const rules = ["614.1a", "614.5", "707.10", "707.10c"];
  this.player(event.who, `${path}/event/who`, context);
  this.node({ id: this.id(path, "additional-stack-copy"), path, node: "additional-stack-copy",
    scope: context.ability, actor: context.you, mayRetarget: Boolean(replace.mayRetargetAdditional), rules });
  this.engineFeature("stack-instruction", path, "Add one copy to each qualifying copy event and apply the additional copy's own retarget option.", rules);
  return true;
}

export function keepUnspentMana(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const { event, replace } = replacement;
  if (context.barrier || !context.permanentSource || replacement.optional || replacement.condition ||
      replacement.occurrence !== "each" || event.event !== "lose-unspent-mana" || event.who.player !== "you" ||
      replace.replacement !== "keep-mana" || !["W", "U", "B", "R", "G", "C"].includes(replace.as.symbol)) return false;
  const at = `${path}/event/who`;
  this.walk(event.who, at, { ...context, phase: "replacement-match" });
  const rules = ["106.4", "106.6", "500.5", "614.5", "614.6"];
  this.node({ id: this.id(path, "keep-unspent-mana"), path, node: "keep-unspent-mana", scope: context.ability,
    player: context.you, symbol: replace.as.symbol, rules });
  this.engineFeature("keep-unspent-mana", path, "Replace the loss of the bound player's unspent mana with a change of type, preserving each mana's properties.", rules);
  return true;
}

function topLevelReplacement(context: Context): boolean {
  return !context.barrier && context.lexicalAbilityPath !== undefined &&
    /^\/faces\/\d+\/abilities\/\d+$/.test(context.lexicalAbilityPath) &&
    !context.conditional && !context.decider;
}

function singleSentenceStep(path: string, context: Context): boolean {
  const ability = context.lexicalAbilityPath;
  return ability !== undefined && path.startsWith(ability) && /^\/effects\/\d+\/steps\/0$/.test(path.slice(ability.length));
}

function bindDamageCounterRecipient(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const { event, replace } = replacement;
  const one = { amount: "fixed", value: 1 };
  const on = { ref: "replacement-event-object", role: "recipient" };
  if (!isDeepStrictEqual(replacement, { occurrence: "each", event: {
    event: "damage", kind: "noncombat",
    source: { ref: "filter", filter: { control: "you" }, count: one },
    to: [{ ref: "filter", filter: { types: ["creature"], control: "opponent" }, count: one }],
  }, replace: { replacement: "effects", effects: [{ steps: [{
    effect: "put-counters", counter: { type: "-1/-1" }, count: { amount: "replacement-event-quantity" }, on,
  }] }] } }) || replace.replacement !== "effects") return false;

  // CR 120.4b, 614.6: the counters use the sole creature recipient of the
  // proposed damage. The quantity modifier graph has no operand for the
  // original amount, which therefore remains an unresolved reference.
  const rules = ["120.4b", "122.6", "614.1a", "614.6", "614.7a"];
  const eventPath = child(path, "event");
  const matching: Context = { ...context, phase: "replacement-match" };
  this.walk(event, eventPath, matching);
  const at = `${eventPath}/to/0`;
  const source = this.at(child(eventPath, "source"));
  const match = this.at(at);
  if (!source || !match) return true;
  const proposal = this.event(eventPath, matching, "proposed-damage", [source, match]);
  const recipient = this.node({ id: this.id(at, "recipient"), path: at, node: "event-participant",
    scope: context.ability, event: proposal, role: "proposed-damage-recipient", cardinality: "one", rules });
  const counterPath = `${path}/replace/effects/0/steps/0`;
  this.effect(replace.effects[0].steps[0], counterPath, { ...context, phase: "resolution",
    exactEventObject: { path: child(counterPath, "on"), id: recipient, rules } });
  return true;
}

function bindSinglePlayerReplacement(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const { event, replace } = replacement;
  if (!isDeepStrictEqual(replacement, { occurrence: "each", event, replace })) return false;
  const you = { player: "you" };
  const eventPath = child(path, "event");
  const matching: Context = { ...context, phase: "replacement-match" };
  if (isDeepStrictEqual(replace, { replacement: "skip" }) &&
      (["a-player", "an-opponent"].some(player =>
        isDeepStrictEqual(event, { event: "begin-time", who: { player }, what: { kind: "turn" }, extra: true })) ||
        isDeepStrictEqual(event, { event: "draw", who: { player: "a-player" }, quantity: { op: "eq", value: { amount: "fixed", value: 1 } } }))) {
    this.walk(event, eventPath, matching);
    if (event.event === "begin-time")
      this.engineFeature("phase-scheduling", path, "Skip each matching extra turn before it starts.", ["500.7", "614.1b", "614.10", "614.10a"]);
    else this.engineFeature("static-rule", path, "Replace each qualifying individual draw with no action.", ["121.2", "614.1b", "614.10"]);
    return true;
  }
  if (isDeepStrictEqual(event, { event: "proliferate", who: you }) && replace.replacement === "effects" &&
      isDeepStrictEqual(replace, { replacement: "effects", effects: [{ steps: [{
        effect: "keyword-action", action: "proliferate", who: you, times: { amount: "fixed", value: 2 },
      }] }] })) {
    this.walk(event, eventPath, matching);
    this.effect(replace.effects[0].steps[0], `${path}/replace/effects/0/steps/0`, { ...context, phase: "resolution" });
    this.engineFeature("static-rule", path, "Replace each qualifying proliferate event with two separate proliferations by the same player.", ["614.5", "614.6", "701.34"]);
    return true;
  }
  return false;
}

export function exileReplacement(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const parts = !context.barrier && context.permanentSource ? exileReplacementParts(replacement) : undefined;
  if (!parts) {
    if (!topLevelReplacement(context) || context.phase !== "continuous" || !context.permanentSource) return false;
    const move = singleGraveyardExile(replacement);
    if (move) {
      const rules = ["400.6", "400.7", "614.1a", "614.5", "614.6", "616.1"];
      const matching: Context = { ...context, phase: "replacement-match" };
      const eventPath = child(path, "event");
      const at = child(eventPath, "what");
      const match = this.object(move.event.what, at, matching);
      if (!match) return true;
      this.walk(move.event.to, child(eventPath, "to"), matching);
      const exilePath = `${path}/replace/effects/0/steps/0`;
      const proposal = this.id(path, "exile-replacement");
      const object = this.node({ id: this.id(at, "affected"), path: at, node: "event-participant",
        scope: context.ability, event: proposal, role: "proposed-move-object", cardinality: "one", rules });
      this.node({ id: proposal, path, node: "exile-replacement", scope: context.ability, object, filter: match.id,
        from: move.from, originalDestination: "graveyard", exileEvent: this.id(exilePath, "event"),
        graveyardOwner: this.at(`${eventPath}/to/owner`)?.id, rules });
      this.effect(move.exile, exilePath, { ...context, phase: "resolution",
        exactEventObject: { path: child(exilePath, "what"), id: object, rules } });
      this.engineFeature("zone-replacement", path, "Redirect each matching proposed graveyard move to exile, with any prescribed counter.", rules);
      return true;
    }
    return bindDamageCounterRecipient.call(this, replacement, path, context) ||
      bindSinglePlayerReplacement.call(this, replacement, path, context);
  }

  const rules = ["400.6", "400.7", "614.1a", "614.5", "614.6", "616.1",
    ...(parts.opponentGraveyard ? ["102.2", "108.4", "108.4a", "400.3", "607.2b"] : []),
    ...(parts.damageHistory ? ["109.5", "120.4b", "608.2i"] : [])];
  const proposal = this.id(path, "exile-replacement");
  const at = `${path}/event/what`;
  const matching: Context = { ...context, phase: "replacement-match" };
  const match = this.object(parts.event.what, at, matching);
  if (parts.opponentGraveyard) {
    this.walk(parts.event.to, `${path}/event/to`, matching);
    this.walk(parts.event.notControlledByBefore, `${path}/event/notControlledByBefore`, matching);
  }
  const exilePath = `${path}/replace/effects/0/steps/0`;
  const exileEvent = this.id(exilePath, "event");
  const object = this.node({ id: this.id(at, "affected"), path: at, node: "event-participant",
    scope: context.ability, event: proposal, role: "proposed-move-object", cardinality: "one", rules });
  const followup = parts.followup ? `${path}/replace/effects/1/steps/0` : undefined;
  this.node({ id: proposal, path, node: "exile-replacement", scope: context.ability, object, filter: match?.id,
    from: parts.opponentGraveyard ? "anywhere" : "battlefield", originalDestination: "graveyard", exileEvent,
    ...(parts.opponentGraveyard ? { control: { differentFrom: context.you, information: "before-move" as const },
      graveyardOwner: this.at(`${path}/event/to/owner`)?.id } : {}),
    ...(parts.damageHistory ? { damageHistory: { controller: context.you, sourceController: "at-damage" as const,
      recipient: object, window: "this-turn" as const } } : {}),
    ...(followup ? { reflexiveTrigger: this.id(followup, "reflexive-triggered") } : {}), rules });
  const execution: Context = { ...context, phase: "resolution",
    exactEventObject: { path: child(exilePath, "what"), id: object, rules } };
  this.effect(parts.exile, exilePath, execution);
  if (parts.life) this.effect(parts.life, `${path}/replace/effects/0/steps/1`, execution);
  if (parts.followup?.effect === "create-reflexive-trigger" && parts.followup.event.event === "preceding-action" && followup) {
    const triggerRules = ["603.12", "614.6"];
    this.walk(parts.followup.event.who, `${followup}/event/who`, execution);
    this.node({ id: this.id(followup, "performed-exile"), path: `${followup}/event`, node: "performed-action-trigger",
      scope: context.ability, event: exileEvent, outcome: "actually-exiled", rules: triggerRules });
    const trigger = this.nestedAbility(followup, execution, "reflexive-triggered", triggerRules);
    this.sentences(parts.followup.effects, child(followup, "effects"), trigger);
  }
  this.engineFeature("zone-replacement", path,
    parts.opponentGraveyard ? "Replace each proposed move of a card into an opponent's graveyard if this ability's controller did not control that card before the move."
      : "Replace each matching proposed battlefield-to-graveyard move with exile and execute the printed follow-up.", rules);
  return true;
}

/** Each successful replacement application creates its own return
 * trigger, with all card components of that one permanent. CR 603.7c,
 * 614.6, 712.21c and 730.3c. */
export function delayedExileReturn(this: Analyzer, effect: Extract<Effect, { effect: "create-replacement" }>, path: string, context: Context): boolean {
  const { event, replace, occurrence, condition, optional } = effect.replacement;
  const previous = { ref: "replacement-event-object", role: "affected" } as const;
  const successor = { ref: "related", filter: { cls: "card" },
    relation: { relation: "replacement-exile-results" }, number: "plural" } as const;
  const filter = { ref: "filter", filter: { cls: "permanent", control: "you" }, count: { amount: "fixed", value: 1 } } as const;
  const header = { trigger: "phase", phase: "end-step", whose: "each-player" } as const;
  if (context.barrier || context.phase !== "resolution" || context.decider || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || condition || optional || occurrence !== "each" ||
      !isDeepStrictEqual(effect.duration, { duration: "this-turn" }) ||
      event.event !== "move-zone" || !isDeepStrictEqual(event, { event: "move-zone", what: filter,
        from: { zone: "battlefield" }, to: { zone: "graveyard", owner: "any" } }) ||
      replace.replacement !== "effects" || !isDeepStrictEqual(replace.effects, [{ steps: [
        { effect: "exile", what: previous },
        { effect: "create-delayed-trigger", trigger: header, occurrence: "next", nextOnly: true,
          targets: "enclosing-ability", effects: [{ steps: [{ effect: "move-zone", what: successor,
            to: { zone: "battlefield" }, controller: "owner" }] }] },
      ] }])) return false;
  const rules = ["400.7", "603.7a", "603.7c", "603.7d", "614.5", "614.6", "712.21c", "730.3c"];
  const eventPath = `${path}/replacement/event`;
  const matching: Context = { ...context, phase: "replacement-match" };
  const match = this.object(filter, child(eventPath, "what"), matching);
  if (!match) return true;
  this.walk(event.to, child(eventPath, "to"), matching);
  this.walk(effect.duration, child(path, "duration"), context);
  const created = this.event(path, context, "create-replacement", [], context.conditional ? { conditional: true } : {});
  const ruleId = this.id(path, "move-to-exile");
  const at = `${path}/replacement/replace/effects/0/steps/0`;
  const subject = this.node({ id: this.id(at, "affected"), path: child(at, "what"), node: "event-participant",
    scope: context.ability, event: ruleId, role: "proposed-move-object", cardinality: "one", rules });
  const original = this.bound(previous, child(at, "what"), subject, rules);
  const exile = this.event(at, context, "exile", [original]);
  const cards = this.eventResult(at, exile, "objects", "many", "zone-successors", { cls: "card" });
  this.node({ id: ruleId, path, node: "captured-move-replacement", scope: context.ability, created, match: event, exile,
    tracked: { kind: "matching-before-move", filter: match.id }, duration: "this-turn", rules });
  const delayedPath = `${path}/replacement/replace/effects/0/steps/1`;
  const occurrenceScope = this.nestedAbility(delayedPath, context, "delayed-triggered", rules);
  this.trigger(header, child(delayedPath, "trigger"), occurrenceScope);
  const move = `${delayedPath}/effects/0/steps/0`;
  const { id } = cards;
  const returned = this.bound(successor, child(move, "what"), id, rules);
  const returnEvent = this.event(move, occurrenceScope, "move-zone", [returned], { requiresResult: id });
  this.node({ id: this.id(delayedPath, "replacement-return"), path: delayedPath, node: "replacement-return",
    scope: context.ability, exile, objects: id, delayed: occurrenceScope.ability, returnEvent,
    schedule: "per-exiled-permanent-at-next-end-step", membership: "exact-exile-card-incarnations",
    controller: "each-card-owner", rules });
  this.engineFeature("zone-replacement", path,
    "Replace each qualifying move independently. For each actually exiled permanent, schedule one return trigger tracking only its actual card components still in exile.", rules);
  return true;
}

/** CR 120.4b, 400.7, 608.2c: capture the prior damage step's one
 * creature recipient, or the directly named target where damage is immaterial. */
function captureDamageRecipient(this: Analyzer, effect: Extract<Effect, { effect: "create-replacement" }>,
  prior: Effect | undefined, path: string, context: Context, previous: string | undefined): string | undefined {
  const { event, replace } = effect.replacement;
  if (!topLevelReplacement(context) || !context.spellAbility || !previous || prior?.effect !== "damage" ||
      !isDeepStrictEqual(prior, { effect: "damage", source: { ref: "self" }, amount: prior.amount, to: prior.to }) ||
      prior.amount.amount !== "fixed" || prior.amount.value < 1 || event.event !== "move-zone" || !simpleExileBody(replace) ||
      !isDeepStrictEqual(effect, { effect: "create-replacement", duration: { duration: "this-turn" }, replacement: {
        occurrence: "each", event: { event: "move-zone", what: event.what, from: { zone: "battlefield" }, to: { zone: "graveyard" } }, replace,
      } }) || path !== `${context.lexicalAbilityPath}/effects/1/steps/0` ||
      previous !== `${context.lexicalAbilityPath}/effects/0/steps/0`) return;

  const creatures = { types: ["creature"] };
  const creatureOrPlaneswalker = { allOf: [{ anyOf: [creatures, { types: ["planeswalker"] }] }] };
  const to = prior.to;
  const ref = to.to === "object" ? to.ref : undefined;
  const target = ref?.ref === "target" && ref.selectionId &&
    isDeepStrictEqual(to, { to: "object", ref: { ref: "target", filter: ref.filter, count: { amount: "fixed", value: 1 }, selectionId: ref.selectionId } }) &&
    [creatures, creatureOrPlaneswalker].some(filter => isDeepStrictEqual(ref.filter, filter)) ? ref : undefined;
  const anyTarget = to.to === "any-target" && to.selectionId &&
    isDeepStrictEqual(to, { to: "any-target", selectionId: to.selectionId });
  if (!target && !anyTarget) return;
  const participant = this.at(`${previous}/to${target ? "/ref" : ""}`);
  const damage = this.result.nodes.find(node => node.id === this.id(previous, "event"));
  if (!participant || participant.cardinality !== "one" ||
      (target ? participant.domain !== "object" : participant.domain !== "object-or-player") ||
      damage?.node !== "event" || damage.operation !== "damage" || damage.scope !== context.ability ||
      damage.optional || damage.conditional || damage.repeated ||
      !isDeepStrictEqual(damage.inputs, [context.source, participant.id])) return;

  if (target && isDeepStrictEqual(event.what, { ref: "that", filter: target.filter })) return participant.id;
  if (!isDeepStrictEqual(event.what, { ref: "filter", count: { amount: "fixed", value: 1 }, filter: {
    ...creatures, damageHistory: { role: "recipient", window: { event: "this-way" } },
  } })) return;
  this.engineFeature("event-history", previous, "Track the exact creature incarnation dealt positive damage by this instruction.", ["120.4b", "614.7a", "400.7"]);
  return this.eventResult(previous, damage.id, "objects", "one", "same-objects", creatures).id;
}

function captureDelayedExileSubject(this: Analyzer, effect: Extract<Effect, { effect: "create-replacement" }>,
  path: string, previous: Effect | undefined, previousPath: string | undefined, context: Context): string | undefined {
  const { replace } = effect.replacement;
  if (!topLevelReplacement(context) || !singleSentenceStep(path, context) || !previousPath ||
      !singleSentenceStep(previousPath, context) || !simpleExileBody(replace) ||
      !isDeepStrictEqual(effect, { effect: "create-replacement", replacement: { occurrence: "each", replace,
        event: { event: "move-zone", what: { ref: "it" }, from: { zone: "battlefield" } },
      } }) || !isDeepStrictEqual(previous, {
        effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
        occurrence: "next", nextOnly: true, targets: "enclosing-ability", effects: [{ steps: [{ effect: "exile", what: { ref: "it" } }] }],
      }) || context.objects.length !== 1) return;
  const [subject] = context.objects;
  const delayedSubject = this.at(`${previousPath}/effects/0/steps/0/what`);
  const origin = this.result.nodes.find(node => node.id === subject.id);
  if (subject.domain !== "object" || subject.cardinality !== "one" || delayedSubject?.id !== subject.id ||
      origin?.node !== "event-result" || origin.identity !== "zone-successors" ||
      origin.description?.cls !== "permanent" || !isDeepStrictEqual(origin.description.types, ["creature"]) ||
      !singleSentenceStep(origin.path, context)) return;
  const move = this.result.nodes.find(node => node.id === origin.event);
  if (move?.node === "event" && move.scope === context.ability && move.operation === "move-zone" &&
      !move.optional && !move.conditional && !move.repeated) return subject.id;
}

export function moveToExileRule(this: Analyzer, effect: Extract<Effect, { effect: "create-replacement" }>, path: string,
  context: Context, precedingEffect?: string): boolean {
  const { event, replace, occurrence, condition, optional } = effect.replacement;
  if (context.barrier || context.phase !== "resolution" || context.conditional || context.decider ||
      effect.optional || effect.decidedBy || effect.forEach || effect.forEachVote ||
      condition || optional || occurrence !== "each" || event.event !== "move-zone" || event.notControlledByBefore ||
      replace.replacement !== "effects" || replace.effects.length < 1 || replace.effects.length > 2 ||
      replace.effects[0].steps.length !== 1 || Object.keys(replace.effects[0]).length !== 1) return false;
  const [exile] = replace.effects[0].steps;
  if (exile.effect !== "exile" || Object.keys(exile).some(key => !["effect", "what", "counters"].includes(key)) ||
      !isDeepStrictEqual(exile.what, { ref: "replacement-event-object", role: "affected" })) return false;
  const suspend = replace.effects[1];
  if (suspend && (!event.resolving || !isDeepStrictEqual(suspend, {
    steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }] }],
    condition: { condition: "not", of: { condition: "has-keyword", what: { ref: "related", number: "singular",
      filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } }, keyword: "suspend" } },
    conditionKind: "if",
  }))) return false;

  const at = child(path, "replacement");
  const eventPath = child(at, "event");
  const whatPath = child(eventPath, "what");
  const prior = context.precedingSentence?.steps.length === 1 && precedingEffect === `${context.precedingSentence.path}/steps/0`
    ? context.precedingSentence.steps[0] : undefined;
  const priorEvent = prior && precedingEffect ? this.result.nodes.find(node => node.id === this.id(precedingEffect, "event")) : undefined;
  const rules = ["400.6", "400.7h", "400.7j", "614.1a", "614.5", "614.6"];
  let tracked: Extract<BindingNode, { node: "captured-move-replacement" }>["tracked"];
  let capturedThisTurn = false;

  if (event.from === undefined && isDeepStrictEqual(event.to, { zone: "graveyard", owner: "your" }) &&
      !event.resolving && isDeepStrictEqual(event.what, { ref: "that", filter: { cls: "spell" } }) &&
      prior?.effect === "play-card" && prior.optional && !prior.decidedBy && !prior.forEach && !prior.forEachVote &&
      prior.instruction.who.player === "you" && prior.instruction.action === "cast" && prior.instruction.from && "zone" in prior.instruction.from && prior.instruction.from.zone === "graveyard" &&
      prior.instruction.what.ref === "filter" && objectCardinality(prior.instruction.what) === "one" &&
      priorEvent?.node === "event" && priorEvent.scope === context.ability && precedingEffect) {
    const cast = this.eventResult(precedingEffect, priorEvent.id, "objects", "one", "zone-successors", { cls: "spell" });
    tracked = { kind: "one-object", object: cast.id };
    this.bound(event.what, whatPath, cast.id, rules);
  } else if (event.from === undefined && event.resolving && context.castActors?.caster.id === context.you &&
      context.triggerObject?.event === "cast" && context.objects.length === 0 && !context.departed?.length &&
      context.lexicalAbilityPath !== undefined && path === `${context.lexicalAbilityPath}/effects/0/steps/0` &&
      isDeepStrictEqual(event.to, { zone: "graveyard", owner: "your" }) &&
      isDeepStrictEqual(event.what, { ref: "that", filter: { cls: "card" } })) {
    const cast = context.triggerObject.candidate.id;
    tracked = { kind: "one-object", object: cast };
    this.bound(event.what, whatPath, cast, rules);
  } else if (event.from === undefined && !event.resolving && isDeepStrictEqual(event.to, { zone: "graveyard", owner: "any" }) &&
      isDeepStrictEqual(event.what, { ref: "filter", count: { amount: "fixed", value: 1 }, filter: {
        cls: "spell", relation: { relation: "event-result", event: "cast", scope: "this-way" },
      } }) && prior?.effect === "grant-play-permission" &&
      prior.permission.action === "cast" && prior.permission.from && "zone" in prior.permission.from && prior.permission.from.zone === "graveyard" && prior.permission.who.player === "you" &&
      priorEvent?.node === "event" && priorEvent.scope === context.ability && !priorEvent.conditional) {
    const member = this.node({ id: this.id(whatPath, "cast-by-permission"), path: whatPath, node: "event-participant",
      scope: context.ability, event: priorEvent.id, role: "spell-cast-by-permission", cardinality: "one", rules });
    tracked = { kind: "cast-permission", grant: priorEvent.id, member };
    this.bound(event.what, whatPath, member, rules);
    // The grant describes possible future spells; it did not produce a
    // stack object to compete with a subsequent move's actual successor.
    const spellQuery = precedingEffect ? this.at(`${precedingEffect}/permission/what`) : undefined;
    context.objects = context.objects.filter(candidate => candidate.id !== spellQuery?.id);
  } else if (!event.to && !event.resolving && isDeepStrictEqual(event.from, { zone: "battlefield" }) &&
      isDeepStrictEqual(event.what, { ref: "that", filter: { types: ["creature"] } }) &&
      context.objects.length === 1 && context.objects[0].cardinality === "one" && context.objects[0].description?.cls === "permanent" &&
      context.objects[0].description.types?.includes("creature") &&
      this.result.nodes.some(node => node.node === "event-result" && node.identity === "zone-successors" && node.id === context.objects[0].id)) {
    const permanent = context.objects[0].id;
    tracked = { kind: "one-object", object: permanent };
    this.bound(event.what, whatPath, permanent, rules);
  } else {
    const object = captureDamageRecipient.call(this, effect, prior, path, context, precedingEffect) ??
      captureDelayedExileSubject.call(this, effect, path, prior, precedingEffect, context);
    if (!object) return false;
    tracked = { kind: "one-object", object };
    capturedThisTurn = effect.duration?.duration === "this-turn";
    this.bound(event.what, whatPath, object, [...rules, "608.2c"]);
  }

  this.walk(event.from, child(eventPath, "from"), context);
  this.walk(event.to, child(eventPath, "to"), context);
  const exilePath = `${at}/replace/effects/0/steps/0`;
  const ruleId = this.id(path, "move-to-exile");
  const affected = this.node({ id: this.id(exilePath, "affected"), path: child(exilePath, "what"),
    node: "event-participant", scope: context.ability, event: ruleId, role: "proposed-move-object", cardinality: "one", rules });
  this.node({ id: ruleId, path, node: "captured-move-replacement", scope: context.ability,
    created: this.event(path, context, effect.effect, []), tracked, match: event, exile: this.id(exilePath, "event"),
    ...(capturedThisTurn ? { duration: "this-turn" as const } : {}), rules });
  const execution = { ...context, objects: [], players: [], quantities: [], departed: [], exileResults: undefined,
    exactEventObject: { path: child(exilePath, "what"), id: affected, rules } };
  this.effect(exile, exilePath, execution);
  if (suspend) {
    const result = this.result.nodes.find(node => node.id === this.id(exilePath, "objects"));
    if (result?.node === "event-result") {
      const card: Candidate = { id: result.id, domain: "object", cardinality: "one", description: { cls: "card" } };
      const atSuspend = `${at}/replace/effects/1`;
      this.walk(suspend.condition, child(atSuspend, "condition"), { ...execution, objects: [card],
        exileResults: [{ event: result.event, result: card }], uncertainExileOrigin: false });
      this.effect(suspend.steps[0], `${atSuspend}/steps/0`, { ...execution, objects: [card], conditional: true });
    }
  }
  this.engineFeature("zone-replacement", path,
    "Install the exile replacement only for the specified object incarnation or spells cast by the bound permission.", rules);
  return true;
}

/** CR 118.9 and 601.2a,b,f. The alternative cost uses the spell as
 * proposed on the stack under this permission, including its chosen face. */
export function linkedExileLifeCost(this: Analyzer, permission: Extract<StaticEffect, { static: "play-permission" }>["permission"],
  path: string, context: Context): boolean {
  const origin = context.linkedExile;
  const { what, alternativeCost, ...rest } = permission;
  if (context.barrier || !context.permanentSource || context.phase !== "continuous" ||
      origin?.rule !== "607.2b" || origin.ability === context.lexicalAbilityPath ||
      !isDeepStrictEqual(rest, { who: { player: "you" }, action: "play", condition: { condition: "your-turn" } }) ||
      what.ref !== "related" || what.number !== "plural" || !isDeepStrictEqual(what.filter, { cls: "card" }) ||
      what.relation.relation !== "linked-exile" || !["self", "this"].includes(what.relation.source.ref) ||
      !isDeepStrictEqual(alternativeCost, { cost: "pay", costs: [
        { cost: "pay-life", amount: { amount: "attribute", attribute: "mana-value", of: { ref: "it" } } },
      ] })) return false;
  const at = child(path, "permission");
  const rules = ["107.3b", "118.9", "202.3", "202.3d", "202.3e", "305.1", "601.2a", "601.2b", "601.2f", "607.2b"];
  const pool = this.id(origin.path, "linked-exile-pool");
  const binding = this.id(path, "linked-exile-life-cost");
  const operand = `${at}/alternativeCost/costs/0/amount/of`;
  const spell = this.node({ id: this.id(operand, "proposed-spell"), path: operand, node: "event-participant",
    scope: context.ability, event: binding, role: "proposed-permission-spell", cardinality: "one", rules });
  this.walk({ ...rest, what }, at, context);
  this.walk(alternativeCost, child(at, "alternativeCost"), { ...context, phase: "payment",
    boundOperand: { path: operand, candidate: { id: spell, domain: "object", cardinality: "one" }, rules } });
  this.node({ id: binding, path, node: "linked-exile-life-cost", scope: context.ability,
    pool, player: context.you, spell, availability: "during-permitted-players-turn", timing: "cost-determination",
    payment: "life-equal-to-proposed-spell-mana-value", landPlay: "normal-rules", rules });
  this.engineFeature("linked-exile-play", path,
    "Play cards from the linked exile pool using normal timing. Casting under this permission requires the printed life cost for the proposed spell and any other applicable costs.", rules);
  return true;
}

/** CR 120.4b and 615.1a. Each proposed source-recipient pair is checked
 * against the predicate, using the departed source's last known characteristics if needed. */
export function preventionPair(this: Analyzer, prevention: Extract<StaticEffect, { static: "prevent-damage" }>["prevention"],
  path: string, context: Context): boolean {
  const source = { ref: "prevention-event-object", role: "source" } as const;
  const recipient = { ref: "prevention-event-object", role: "recipient" } as const;
  const singleCreature = { ref: "filter", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } };
  const test = { relation: "shared-value", characteristic: "color", with: { operand: "object", what: recipient } } as const;
  if (context.barrier || !context.permanentSource || context.phase !== "continuous" ||
      !isDeepStrictEqual(prevention, {
        kind: "all", eventScope: { sources: "each", recipients: "each" },
        shield: { shield: "events", occurrence: "each", prevent: { prevent: "all" } },
        by: { ...singleCreature, filter: { ...singleCreature.filter, otherThan: recipient } },
        to: [singleCreature], condition: { condition: "characteristic-relation", what: source, test },
      })) return false;
  const rules = ["105.2", "113.7a", "120.4b", "608.2h", "609.7c", "615.1a"];
  const event = this.id(path, "damage-prevention-pair");
  const atSource = `${path}/condition/what`;
  const atRecipient = `${path}/to/0`;
  const proposedSource = this.node({ id: this.id(atSource, "source"), path: atSource, node: "event-participant",
    scope: context.ability, event, role: "proposed-damage-source", cardinality: "one", rules });
  const proposedRecipient = this.node({ id: this.id(atRecipient, "recipient"), path: atRecipient, node: "event-participant",
    scope: context.ability, event, role: "proposed-damage-recipient", cardinality: "one", rules });
  const matching: Context = { ...context, phase: "replacement-match" };
  this.walk(prevention.to?.[0], atRecipient, matching);
  this.walk(prevention.by, child(path, "by"), { ...matching,
    exactEventObject: { path: `${path}/by/filter/otherThan`, id: proposedRecipient, rules } });
  this.bound(source, atSource, proposedSource, rules);
  const predicate = `${path}/condition/test`;
  this.characteristicPredicate(test, predicate, proposedSource, { ...matching,
    exactEventObject: { path: `${predicate}/with/what`, id: proposedRecipient, rules } });
  this.node({ id: event, path, node: "damage-prevention-pair", scope: context.ability,
    source: proposedSource, recipient: proposedRecipient, predicate: this.id(predicate, "characteristic-predicate"),
    sourceInformation: "current-or-last-known", sourceFilter: this.at(child(path, "by"))?.id, recipientFilter: this.at(atRecipient)?.id, rules });
  return true;
}

export function preventionOutcome(this: Analyzer, prevention: Extract<StaticEffect, { static: "prevent-damage" }>["prevention"],
  path: string, context: Context): boolean {
  const { shield, eventScope, kind, to, after } = prevention;
  if (context.barrier || !context.permanentSource || context.phase !== "continuous" ||
      Object.keys(prevention).some(key => !["shield", "eventScope", "kind", "to", "after"].includes(key)) ||
      !isDeepStrictEqual(shield, { shield: "events", occurrence: "each", prevent: { prevent: "all" } }) ||
      eventScope.sources !== "simultaneous" || eventScope.recipients !== "each" || to?.length !== 1 ||
      !("ref" in to[0]) || to[0].ref !== "filter" || objectCardinality(to[0]) !== "one" ||
      after?.length !== 1 || after[0].steps.length !== 1 || Object.keys(after[0]).length !== 1) return false;
  const [step] = after[0].steps;
  if (step.effect !== "put-counters" || Object.keys(step).some(key => !["effect", "on", "counter", "count"].includes(key)) ||
      !isDeepStrictEqual(step.on, { ref: "prevention-event-object", role: "recipient" }) ||
      !isDeepStrictEqual(step.count, { amount: "prevention-event-quantity", quantity: "prevented" })) return false;

  const rules = ["120.4b", "615.1a", "615.5", "615.12"];
  this.walk(to[0], `${path}/to/0`, { ...context, phase: "replacement-match", sourceExclusionPath: `${path}/to/0/filter` });
  const filter = this.at(`${path}/to/0`);
  if (!filter) return true;
  const afterPath = `${path}/after/0/steps/0`;
  const preventionId = this.id(path, "prevention");
  const recipient = this.node({ id: this.id(child(afterPath, "on"), "recipient"), path: child(afterPath, "on"),
    node: "event-participant", scope: context.ability, event: preventionId,
    role: "prevented-damage-recipient", cardinality: "one", rules });
  const prevented = this.eventResult(child(afterPath, "count"), preventionId, "quantity", "one", "measured-quantity");
  const followup = this.id(afterPath, "event");
  this.node({ id: preventionId, path, node: "prevention-outcome", scope: context.ability, targetFilter: filter.id,
    kind, prevented: prevented.id, recipient, followup, rules });
  this.effect(step, afterPath, { ...context, phase: "resolution",
    exactEventObject: { path: child(afterPath, "on"), id: recipient, rules },
    exactEventQuantity: { path: child(afterPath, "count"), id: prevented.id } });
  return true;
}

/** CR 614.5–7, 616.1: match the proposed event and modify its quantities.
 * The pattern's participants are predicates on that event, not selections. */
export function quantityReplacement(this: Analyzer, replacement: ReplacementInstruction, path: string, context: Context): boolean {
  const { event, replace } = replacement;
  if (context.barrier || !context.permanentSource || replace.replacement !== "modify-quantity" ||
      replacement.occurrence !== "each" || replacement.optional || replacement.condition) return false;
  const { change } = replace;
  if (change.operation === "multiply" ? !Number.isSafeInteger(change.factor) || change.factor < 1
    : change.operation === "half" || change.amount.amount !== "fixed" || change.amount.value < 0) return false;
  const unit = quantityUnit(event);
  if (!unit || ((unit === "mana-specification" || unit === "token-specification") && change.operation !== "multiply")) return false;

  const scope: Context = { ...context, phase: "replacement-match" };
  const firstReference = this.result.references.length;
  if (event.event === "damage" && event.source?.ref === "filter" && event.source.filter.otherThan) {
    const at = `${path}/event/source/filter/otherThan`;
    const recipient = this.node({ id: this.id(at, "proposed-damage-recipient"), path: at, node: "event-participant",
      scope: context.ability, event: this.id(path, "quantity-replacement"), role: "proposed-damage-recipient",
      cardinality: "one", rules: ["120.4b", "614.7a"] });
    scope.exactEventObject = { path: at, id: recipient, rules: ["120.4b", "614.7a"] };
  }
  this.walk(event, child(path, "event"), scope);
  const operands = this.result.references.slice(firstReference)
    .flatMap(reference => reference.binding === "bound" ? [reference.target] : []);
  const rules = ["614.1a", "614.5", "614.6", "616.1",
    ...unit === "damage-source-and-recipient" ? ["120.4b", "614.7a"] : unit === "mana-specification" ? ["106.6a", "106.12b"]
      : unit === "counter-kind-and-recipient" ? ["122.6", "614.16"]
        : unit === "token-specification" ? ["111.2", "614.16"] : ["119.9", "119.10"]];
  this.node({ id: this.id(path, "quantity-replacement"), path, node: "quantity-replacement", scope: context.ability,
    source: context.source, match: event, change, unit, operands, rules });
  this.engineFeature("quantity-replacement", path,
    "Preserve the proposed event's participants and specifications, including mana restrictions and additional effects. Recheck applicable replacements and apply each at most once per event.", rules);
  return true;
}
/** CR 614.12a: choose before the proposed entrant is on the battlefield.
 * CR 707.2, 707.5: apply the model's copiable values to that entry. */
export function bindEntryCopy(this: Analyzer, { what, model, optional, duration, changes, entryExceptions, entry }: OptionalEntryCopy,
  path: string, context: Context): void {
  const { source, ability: scope, you } = context;
  const entrantPath = child(path, "what");
  const proposal = this.node({ id: this.id(entrantPath, "proposal"), path: entrantPath, node: "battlefield-entry-proposal",
    scope, object: source, to: "battlefield", controller: you, rules: ["110.2", "614.12"] });
  this.bound(what, entrantPath, proposal, ["201.5", "614.12"]);
  const modelPath = child(path, "model");
  const choice = this.node({ id: this.id(modelPath, "choice"), path: modelPath, node: "entry-copy-model-choice",
    scope, proposal, chooser: you, timing: model.when, selection: "non-target", count: "zero-or-one",
    zone: "battlefield", filter: model.filter,
    ...(model.filter.zoneChangeHistory ? { historyIdentity: "current-incarnation" as const } : {}),
    rules: ["110.1", "115.10a", "400.7", "614.12a"] });
  this.bound(model.who, child(modelPath, "who"), you, ["110.2", "614.12a"]);
  this.bound(optional.who, `${path}/optional/who`, you, ["110.2", "614.12a"]);
  if (!model.filter.zoneChangeHistory) this.walk(model.filter, `${modelPath}/filter`, context);
  this.walk(entryExceptions, child(path, "entryExceptions"), context);
  this.node({ id: this.id(path, "replacement"), path, node: "entry-copy-replacement", scope,
    entry: proposal, model: choice, timing: "as-enters", values: "model-copiable-values",
    onDecline: "leave-entry-unchanged", ...(changes ? { changes } : {}), ...(duration ? { duration } : {}),
    ...(entryExceptions ? { exceptions: entryExceptions } : {}), ...(entry ? { entryState: entry } : {}),
    rules: ["614.12", "707.2", "707.2b", "707.5", ...(changes || entryExceptions ? ["707.9"] : [])] });
  this.engineFeature("entry-copy-replacement", path,
    model.filter.zoneChangeHistory
      ? "Track entry turn for current battlefield incarnations, offer the optional choice before entry, and apply the chosen model's copiable values during entry replacement processing."
      : "Offer the battlefield model choice before entry and apply the stated copy duration, characteristics, and entry exceptions.",
    ["400.7", "614.12", "614.12a", "616.1c", "707.2", "707.5"]);
}
/** CR 400.6 and 614.6: both instructions see the affected object of the
 * proposed move. Revealing it leaves it in its original zone (CR 701.20b). */
export function selfGraveyardReplacement(this: Analyzer, path: string, context: Context): void {
  const { source, ability: scope, you } = context;
  const { event, occurrence, replace } = SELF_GRAVEYARD_REPLACEMENT.replacement;
  const [reveal, shuffle] = replace.effects[0].steps;
  const eventPath = child(path, "event");
  const replacementPath = child(path, "replace");
  const revealPath = `${replacementPath}/effects/0/steps/0`;
  const shufflePath = `${replacementPath}/effects/0/steps/1`;
  const ownerPath = `${shufflePath}/library/owner`;
  const owner = this.node({ id: this.id(ownerPath, "owner"), path: ownerPath, node: "derived-player", scope, role: "owner",
    of: source, rules: ["108.3", "400.3"] });
  const proposal = this.node({ id: this.id(eventPath, "proposal"), path: eventPath, node: "zone-change-proposal", scope,
    object: source, from: event.from, to: { zone: event.to.zone, owner }, rules: ["400.3", "400.6", "614.1a"] });
  this.bound(event.what, child(eventPath, "what"), source, ["201.5", "400.6"]);
  this.bound(reveal.who, child(revealPath, "who"), you, ["108.4a", "109.5"]);
  this.bound(reveal.what, child(revealPath, "what"), source, ["201.5", "614.6", "701.20b"]);
  this.bound(shuffle.who, child(shufflePath, "who"), you, ["108.4a", "109.5"]);
  this.bound(shuffle.what[0], `${shufflePath}/what/0`, source, ["614.6", "701.20b", "701.24c"]);
  this.bound({ player: "owner", of: shuffle.what[0] }, ownerPath, owner, ["108.3", "400.3"]);
  this.node({ id: this.id(replacementPath, "replacement"), path: replacementPath, node: "reveal-and-shuffle-replacement", scope,
    replaces: proposal, occurrence, reveal: { actor: you, object: source },
    shuffleInto: { actor: you, object: source, library: { zone: shuffle.library.zone, owner } },
    rules: ["614.6", "701.20a", "701.20b", "701.24a", "701.24c"] });
  this.engineFeature("graveyard-reveal-and-shuffle", path,
    "Apply the replacement to a proposed graveyard move and shuffle the owner's library even if another effect redirects the library move.",
    ["400.6", "614.5", "614.6", "616.1", "701.24c"]);
}
export function choiceOfCopyCount(this: Analyzer, effect: Extract<Effect, { effect: "self-replacement" }>, path: string, context: Context): boolean {
  if (context.barrier || context.conditional || context.decider || context.uncertainStackOrigin || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || effect.replacementChoice || effect.targetPolicy !== "retain-original" ||
      !("effect" in effect.original) || effect.original.effect !== "copy-stack-object" ||
      !Array.isArray(effect.instead) || effect.instead.length !== 1 || effect.instead[0].effect !== "copy-stack-object") return false;
  const original = effect.original;
  const instead = effect.instead[0];
  const directSubject = original.what.ref === "target" && stackClasses(original.what.filter) === "spell" ||
    original.what.ref === "trigger-event-object" && original.what.event === "cast" && context.triggerObject?.event === "cast";
  if (!directSubject || !isDeepStrictEqual(instead.what, { ref: "that", filter: { cls: "spell" } }) ||
      ![original, instead].every(copy => copy.who.player === "you" && copy.count.amount === "fixed" &&
        Number.isSafeInteger(copy.count.value) && copy.count.value >= 1 && Object.keys(copy).length === 4) ||
      !(isDeepStrictEqual(effect.condition, { condition: "completed-dungeon", who: { player: "you" } }) ||
        context.spellAbility && effect.condition.condition === "was-cast" &&
        isDeepStrictEqual(effect.condition.what, { ref: "this", noun: "spell" }) && effect.condition.from?.zone === "graveyard"))
    return false;
  if (original.count.amount !== "fixed" || instead.count.amount !== "fixed" || objectCardinality(original.what) !== "one") return false;

  const rules = ["614.15", "707.10", "707.10c"];
  const originalPath = child(path, "original");
  const alternatePath = `${path}/instead/0`;
  const subject = this.object(original.what, child(originalPath, "what"), context);
  if (!subject) return true;
  const actor = this.player(original.who, child(originalPath, "who"), context);
  if (!actor) return true;
  if ([...context.objects, ...(context.departed ?? [])].some(candidate => candidate.id !== subject.id &&
      candidate.id !== context.source && (candidate.description === undefined || stackClasses(candidate.description)))) {
    this.diagnostic("unsupported-scope", path, "The replacement copy has a competing spell antecedent.", ["614.15", "707.10"]);
    return true;
  }
  this.bound(instead.what, child(alternatePath, "what"), subject.id, rules);
  this.bound(instead.who, child(alternatePath, "who"), actor.id, rules);
  this.walk(effect.condition, child(path, "condition"), context);
  const event = this.event(path, context, "conditional-copy-count", [subject, actor]);
  const copies = this.eventResult(path, event, "objects", original.count.value === 1 && instead.count.value === 1 ? "one" : "many",
    "new-objects", { cls: "spell" });
  this.node({ id: this.id(path, "conditional-copy-count"), path, node: "conditional-copy-count",
    scope: context.ability, event, subject: subject.id, condition: effect.condition,
    counts: { original: original.count.value, replacement: instead.count.value }, rules });
  this.engineFeature("stack-instruction", path, "Evaluate the self-replacement before making either group of copies and preserve the original subject and its choices.", rules);
  this.engineFeature("stack-instruction", originalPath,
    "Create copies on the stack with inherited choices and the original ability source. Apply printed retargeting during creation.",
    ["707.10", "707.10b", "707.10c", "707.10f"]);
  context.objects = combine(context.objects, [subject, copies]);
  context.stackCopies = [...(context.stackCopies ?? []), { path, event, result: copies, chooser: actor.id, inlineRetarget: false }];
  return true;
}
