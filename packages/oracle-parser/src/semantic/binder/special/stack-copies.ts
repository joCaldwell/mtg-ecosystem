// Single-template recognisers: stack copies and retargeting.
import { isDeepStrictEqual } from "node:util";
import type { Effect, Sentence } from "../../../ast.ts";
import type { Analyzer } from "../analyzer.ts";
import { child, objectCardinality, type Context } from "../context.ts";
import type { StackInstruction } from "../predicates.ts";

/** In this whole template the target is the object named by the first
 * sentence's continuation. The second sentence explicitly names the
 * different copy and applies at copy creation (CR 707.10c).
 * https://magic.wizards.com/en/news/feature/war-spark-release-notes-2019-04-19 */
export function copyThenReturnOriginal(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || sentences.length !== 2 || sentences.some(sentence => Object.keys(sentence).length !== 1) ||
      sentences[0].steps.length !== 2 || sentences[1].steps.length !== 1) return false;
  const [copy, bounce] = sentences[0].steps;
  const [retarget] = sentences[1].steps;
  const you = { player: "you" } as const;
  if (copy.effect !== "copy-stack-object" || !isDeepStrictEqual(copy, {
        effect: "copy-stack-object", what: copy.what, who: you, count: { amount: "fixed", value: 1 },
      }) || copy.what.ref !== "target" || !copy.what.selectionId ||
      !isDeepStrictEqual({ ...copy.what, selectionId: undefined }, {
        ref: "target", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" },
        count: { amount: "fixed", value: 1 }, selectionId: undefined,
      }) || !isDeepStrictEqual(bounce, { effect: "return-to-hand", what: { ref: "it" }, to: "owner" }) ||
      !isDeepStrictEqual(retarget, { effect: "choose-new-targets",
        who: you, what: { ref: "copy-result", number: "singular" }, timing: "copy-creation", optional: true,
      })) return false;
  const first = `${path}/0/steps/0`;
  this.effect(copy, first, context);
  const original = this.at(child(first, "what"));
  const capture = context.stackCopies?.length === 1 ? context.stackCopies[0] : undefined;
  const second = `${path}/0/steps/1`;
  const rules = ["101.2", "112.2", "400.7", "608.2b", "608.2c", "609.3", "704.5e", "707.10", "707.10c"];
  if (!original || !capture || original.id === capture.result.id || capture.path !== first) {
    this.diagnostic("unsupported-scope", second, "The return needs the named original spell and a separate copy operation.", rules);
    return true;
  }
  const scoped: Context = { ...context, precedingSentence: { path: child(path, 0), steps: sentences[0].steps },
    boundOperand: { path: child(second, "what"), candidate: original, rules } };
  this.effect(bounce, second, scoped);
  const third = `${path}/1/steps/0`;
  this.effect(retarget, third, scoped);
  const owner = this.at(child(second, "to"));
  if (!owner) return true;
  this.node({ id: this.id(second, "copy-return-original"), path: second, node: "copy-return-original",
    scope: context.ability, original: original.id, copy: capture.result.id, copyEvent: capture.event,
    returnEvent: this.id(second, "event"), owner: owner.id, targetOption: this.id(third, "stack-instruction"),
    whenNoCopy: "return-original", rules });
  this.engineFeature("stack-instruction", second,
    "Create the copy with its own target choices, then return the original spell to its owner's hand, even if the copy could not be created.", rules);
  context.objects = [];
  context.quantities = [];
  context.stackCopies = undefined;
  context.uncertainStackOrigin = true;
  return true;
}

/** CR 115.9c counts distinct target objects, while CR 115.7e–f
 * still applies to each slot of each stack incarnation. */
export function copyAndReselect(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || sentences.length !== 2 || sentences.some(sentence => Object.keys(sentence).length !== 1) ||
      sentences[0].steps.length !== 1 || sentences[1].steps.length !== 2) return false;
  const [gain] = sentences[0].steps;
  const [copy, change] = sentences[1].steps;
  const you = { player: "you" } as const;
  const spell = { ref: "that", filter: { cls: "spell" } } as const;
  const copied = { ref: "copy-result", number: "singular" } as const;
  const controlled = { ref: "each", filter: { cls: "permanent", controlledBy: you } } as const;
  if (gain.effect !== "gain-control" || !isDeepStrictEqual(gain, { effect: "gain-control", who: you, what: gain.what }) ||
      gain.what.ref !== "target" || !gain.what.selectionId ||
      !isDeepStrictEqual({ ...gain.what, selectionId: undefined }, { ref: "target", selectionId: undefined,
        count: { amount: "fixed", value: 1 }, filter: { cls: "spell", targetsOnly: {
          ref: "filter", filter: { cls: "permanent", orPlayer: true }, count: { amount: "fixed", value: 1 },
        } },
      }) || !isDeepStrictEqual(copy, { effect: "copy-stack-object", what: spell, who: you, count: { amount: "fixed", value: 1 } }) ||
      !isDeepStrictEqual(change, { effect: "change-targets", what: spell, also: [copied], who: you, scope: "each",
        reselection: { method: "random", unit: "target-slot", whenUnavailable: "keep", excluding: [you, controlled] },
      })) return false;
  const from = `${path}/0/steps/0`;
  this.effect(gain, from, context);
  const original = this.at(child(from, "what"));
  if (!original || !this.announced(original)) return true;
  const copyPath = `${path}/1/steps/0`;
  const rules = ["115.7e", "115.7f", "115.8", "115.9b", "115.9c", "608.2b", "609.3", "707.10", "707.10c"];
  const chooser = this.bound(you, child(copyPath, "who"), context.you, rules);
  this.bound(spell, child(copyPath, "what"), original.id, rules);
  const copyEvent = this.event(copyPath, context, "copy-stack-object", [original, chooser]);
  const result = this.eventResult(copyPath, copyEvent, "objects", "one", "new-objects", { cls: "spell" });
  this.node({ id: this.id(copyPath, "stack-instruction"), path: copyPath, node: "stack-instruction",
    scope: context.ability, subject: original.id, chooser: chooser.id,
    action: { kind: "copy", event: copyEvent, count: 1, retarget: false }, rules });
  const changePath = `${path}/1/steps/1`;
  this.bound(spell, child(changePath, "what"), original.id, rules);
  this.bound(copied, `${changePath}/also/0`, result.id, rules);
  this.bound(you, child(changePath, "who"), chooser.id, rules);
  this.bound(you, `${changePath}/reselection/excluding/0`, chooser.id, rules);
  const permanents = this.object(controlled, `${changePath}/reselection/excluding/1`, context);
  if (!permanents) return true;
  const event = this.event(changePath, context, "change-targets", [original, result, chooser, permanents]);
  this.node({ id: this.id(changePath, "paired-random-reselection"), path: changePath, node: "paired-random-reselection",
    scope: context.ability, event, original: original.id, copy: result.id, chooser: chooser.id,
    exclude: [chooser.id, permanents.id], unit: "target-slot", whenUnavailable: "keep", whenNoCopy: "reselect-original", rules });
  this.engineFeature("stack-instruction", changePath,
    "Copy the announced spell, then reselect every slot of the original and copy using each stack object's current controller and legal target set. Exclude the stated player and permanents they control.", rules);
  context.objects = [];
  context.quantities = [];
  context.uncertainStackOrigin = true;
  return true;
}

/** CR 405.3 and 707.10c: each opponent makes their own copy and
 * retargeting decisions. Only actual participants increase the later count. */
export function copiesForOpponents(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || sentences.length !== 3 ||
      sentences.some(sentence => Object.keys(sentence).length !== 1 || sentence.steps.length !== 1)) return false;
  const [[target], [copies], [repeat]] = sentences.map(sentence => sentence.steps);
  const opponents = { player: "each-opponent" } as const;
  const spell = { ref: "that", filter: { cls: "spell" } } as const;
  const you = { player: "you" } as const;
  const participants = { player: "filtered", base: opponents, filter: { precedingAction: "performed" } } as const;
  if (target.effect !== "declare-target" || !("ref" in target.what) || target.what.ref !== "target" ||
      !target.what.selectionId || !isDeepStrictEqual({ ...target.what, selectionId: undefined }, {
        ref: "target", selectionId: undefined, count: { amount: "fixed", value: 1 },
        filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" },
      }) || !isDeepStrictEqual(copies, { effect: "copy-stack-object", what: spell, who: opponents,
        count: { amount: "fixed", value: 1 }, optional: true, decidedBy: opponents, retarget: { who: opponents, optional: true },
      }) || !isDeepStrictEqual(repeat, { effect: "copy-stack-object", who: you, what: spell,
        count: { amount: "plus", a: { amount: "fixed", value: 1 }, b: { amount: "player-count", of: participants } },
        retarget: { who: you, optional: true },
      })) return false;
  const targetPath = `${path}/0/steps/0`;
  this.effect(target, targetPath, context);
  const original = this.at(child(targetPath, "what"));
  if (!original || !this.announced(original)) return true;
  const at = `${path}/1/steps/0`;
  const rules = ["101.4", "405.3", "608.2d", "707.10", "707.10c"];
  const group = this.player(opponents, child(at, "who"), context);
  if (!group) return true;
  const member = this.node({ id: this.id(at, "copying-opponent"), path: at, node: "instruction-member",
    scope: context.ability, group: group.id, domain: "player", rules });
  this.bound(spell, child(at, "what"), original.id, rules);
  this.bound(opponents, child(at, "decidedBy"), member, rules);
  this.bound(opponents, `${at}/retarget/who`, member, rules);
  const opponentEvent = this.event(at, context, "copy-stack-object", [original, group], { optional: true, decider: member });
  const performed = this.eventResult(at, opponentEvent, "player", "many", "same-objects");
  const follow = `${path}/2/steps/0`;
  this.bound(spell, child(follow, "what"), original.id, rules);
  const creator = this.bound(you, child(follow, "who"), context.you, rules);
  this.bound(you, `${follow}/retarget/who`, context.you, rules);
  this.bound(opponents, `${follow}/count/b/of/base`, group.id, rules);
  this.bound(participants, `${follow}/count/b/of`, performed.id, rules);
  const final = this.event(follow, context, "copy-stack-object", [original, creator, performed]);
  this.node({ id: this.id(follow, "opponent-copy-count"), path: follow, node: "opponent-copy-count",
    scope: context.ability, original: original.id, opponents: group.id, member, opponentEvent, participants: performed.id,
    final, controller: creator.id, extra: 1, order: "apnap-opponents-then-controller",
    opponentPlacement: "before-next-opponent", retarget: "optional-per-copy", rules });
  this.engineFeature("stack-instruction", follow,
    "Give each opponent their own copy and retargeting choices in order. Then make one copy for the controller plus one per opponent who copied, each with its own retargeting choice.", rules);
  context.objects = [];
  context.quantities = [];
  context.uncertainStackOrigin = true;
  return true;
}

/** The optional retargeting is defined only for the produced copy, so
 * entering or leaving the later guard cannot expose it after a decline.
 * CR 707.10c fixes retargeting at creation, before the separate grant. */
export function grantOnStackCopies(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (context.triggerObject?.event !== "cast" || context.castActors?.caster.id !== context.you ||
      !isDeepStrictEqual(context.castActors.filter, {
        allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell", singleTarget: true,
      }) || sentences.length !== 3 ||
      sentences.some(sentence => sentence.steps.length !== 1)) return false;
  const [[copy], [gain], [retarget]] = sentences.map(sentence => sentence.steps);
  const you = { player: "you" } as const;
  const wither = [{ gained: "keyword", keyword: { keyword: "wither" } }] as const;
  const both = { ref: "that", filter: { cls: "spell" }, plural: true } as const;
  const copied = { ref: "copy-result", number: "singular" } as const;
  if (Object.keys(sentences[0]).length !== 1 || Object.keys(sentences[1]).length !== 3 ||
      sentences[1].condition?.condition !== "you-do" || sentences[1].conditionKind !== "if" ||
      sentences[1].otherwise || sentences[1].afterConditional ||
      Object.keys(sentences[2]).length !== 2 || sentences[2].afterConditional !== "scope-unresolved" ||
      !isDeepStrictEqual(copy, { effect: "copy-stack-object", what: { ref: "trigger-event-object", event: "cast" },
        who: you, count: { amount: "fixed", value: 1 }, optional: true }) ||
      !isDeepStrictEqual(gain, { effect: "gain-abilities", what: both, abilities: wither }) ||
      !isDeepStrictEqual(retarget, { effect: "choose-new-targets", who: you, what: copied, timing: "copy-creation", optional: true })) return false;
  const at = `${path}/0/steps/0`;
  this.effect(copy, at, context);
  const capture = context.stackCopies?.length === 1 ? context.stackCopies[0] : undefined;
  const original = this.at(child(at, "what"));
  if (!capture || capture.path !== at || !original) {
    const barrier = "The grant and retargeting must use the copy produced by this optional instruction.";
    this.diagnostic("unsupported-scope", at, barrier, ["118.12", "707.10"]);
    sentences.slice(1).forEach((sentence, index) => this.walk(sentence, child(path, index + 1), { ...context, barrier }));
    return true;
  }
  const rules = ["115.7d", "115.9a", "118.12", "400.7", "608.2c", "608.2h", "611.2c", "702.80", "707.10", "707.10c"];
  const affected = `${path}/1/steps/0`;
  const retargetPath = `${path}/2/steps/0`;
  const union = this.node({ id: this.id(affected, "copy-and-original"), path: affected, node: "result-union", scope: context.ability,
    from: [original.id, capture.result.id], cardinality: "many", rules });
  const group = this.bound(both, child(affected, "what"), union, rules);
  const event = this.event(affected, context, "gain-abilities", [group], { conditional: true, requiresResult: capture.result.id });
  const condition = `${path}/1/condition`;
  this.node({ id: this.id(condition, "decision-link"), path: condition, node: "decision-link", scope: context.ability,
    condition: "you-do", decision: at, rules });
  this.bound(copied, child(retargetPath, "what"), capture.result.id, rules);
  this.bound(you, child(retargetPath, "who"), context.you, rules);
  const choose = this.node({ id: this.id(retargetPath, "stack-instruction"), path: retargetPath, node: "stack-instruction",
    scope: context.ability, subject: capture.result.id, chooser: context.you,
    action: { kind: "choose-new-targets", timing: "copy-creation", copyEvent: capture.event }, rules });
  this.node({ id: this.id(affected, "contingent-copy-grant"), path: affected, node: "contingent-copy-grant",
    scope: context.ability, event, copy: capture.result.id, chosenTargets: choose, affected: union, keyword: "wither",
    copyFrom: "current-or-last-known", grantTo: "still-on-stack", rules });
  this.engineFeature("stack-instruction", affected,
    "After making the optional copy with its legal target choices, grant wither to that copy and the original spell.", rules);
  context.objects = [];
  context.quantities = [];
  context.stackCopies = undefined;
  context.uncertainStackOrigin = true;
  return true;
}

/** CR 607.3 and 707.12a. For a copied imprint trigger, the casting
 * decision is individual even though the copy choice covers the pool.
 * https://magic.wizards.com/en/news/feature/kaladesh-release-notes-2016-09-16 */
export function copyLinkedCards(this: Analyzer, effect: Extract<Effect, { effect: "copy-card" }>, path: string, context: Context): boolean {
  const origin = context.linkedExile;
  if (context.barrier || context.phase !== "resolution" || context.sourceMoved || context.decider || context.conditional ||
      !origin || effect.decidedBy || effect.forEach || effect.forEachVote ||
      !isDeepStrictEqual(effect.count, { amount: "fixed", value: 1 })) return false;
  const imprint = origin.imprint && origin.ability !== context.lexicalAbilityPath && effect.optional && effect.who.player === "you" &&
    isDeepStrictEqual(effect.what, { ref: "related", number: "singular", filter: { cls: "card" },
      relation: { relation: "event-result", event: "exile", scope: "contextual" } });
  const caster = context.castActors?.caster;
  const explicit = !effect.optional && effect.who.player === "that-player" && origin.rule === "607.1c" && caster &&
    context.players.every(player => player.id === caster.id || this.weakPlayers.has(player.id)) &&
    effect.what.ref === "each" && effect.what.filter.relation?.relation === "linked-exile" &&
    (effect.what.filter.relation.source.ref === "this" || effect.what.filter.relation.source.ref === "self");
  if (!imprint && !explicit) return false;

  const pool = this.id(origin.path, "linked-exile-pool");
  const rules = [origin.rule, "607.2a", "607.3", "704.5e", "707.12", "707.12a"];
  const what = child(path, "what");
  const cards = imprint ? this.bound(effect.what, what, pool, rules) : this.object(effect.what, what, context);
  const chooser = this.player(effect.who, child(path, "who"), context);
  if (!cards || !chooser || (explicit && chooser.id !== caster?.id)) return true;
  if (effect.optional) {
    this.rememberDecision(path, context);
    context.decisions?.push(path);
  }
  const event = this.event(path, context, effect.effect, [chooser, cards], effect.optional ? { optional: true, decider: chooser.id } : {});
  const result = this.eventResult(path, event, "objects", "many", "new-objects", { cls: "card" });
  this.node({ id: this.id(path, "linked-card-copies"), path, node: "linked-card-copies", scope: context.ability,
    pool, eligible: cards.id, event, caster: chooser.id, result: result.id, rules });
  context.linkedCopies = { path, result, caster: chooser, optional: Boolean(effect.optional) };
  context.uncertainCardCopyOrigin = true;
  this.engineFeature("linked-card-copies", path,
    "Copy each eligible card still in the source incarnation's linked exile pool.", rules);
  return true;
}

export function castLinkedCopies(this: Analyzer, effect: Extract<Effect, { effect: "play-card" }>, path: string, context: Context,
  precedingEffect?: string): boolean {
  const copy = context.linkedCopies;
  if (context.barrier || context.phase !== "resolution" || context.decider || !copy || !effect.optional ||
      effect.forEach || effect.forEachVote ||
      (copy.optional ? context.linkedPath !== copy.path : precedingEffect !== copy.path) ||
      effect.instruction.action !== "cast" || effect.instruction.what.ref !== "copy-result" ||
      effect.instruction.what.number !== (copy.optional ? "singular" : "plural") ||
      !isDeepStrictEqual(effect.instruction.alternativeCost, { cost: "without-mana-cost" }) ||
      Object.keys(effect.instruction).some(key => !["action", "what", "who", "alternativeCost"].includes(key))) return false;
  const at = child(path, "instruction");
  const chooser = this.player(effect.instruction.who, child(at, "who"), context);
  if (!chooser || chooser.id !== copy.caster.id) return false;
  if (effect.decidedBy) {
    const decider = this.player(effect.decidedBy, child(path, "decidedBy"), context);
    if (decider?.id !== chooser.id) return false;
  }
  const rules = ["608.2g", "704.5e", "707.12", "707.12a"];
  this.bound(effect.instruction.what, child(at, "what"), copy.result.id, rules);
  const event = this.event(path, context, effect.effect, [chooser, copy.result],
    { optional: true, decider: chooser.id, requiresResult: copy.result.id, ...(context.conditional ? { conditional: true as const } : {}) });
  this.node({ id: this.id(path, "cast-linked-copies"), path, node: "cast-linked-copies", scope: context.ability,
    event, copies: copy.result.id, caster: chooser.id, choice: "optional-per-copy", rules });
  this.engineFeature("linked-card-copies", path, "Offer the same player a separate immediate cast choice for each copy.", rules);
  return true;
}

export function specializedStackInstruction(this: Analyzer, effect: StackInstruction, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || context.decider || context.conditional ||
      effect.optional || effect.decidedBy || effect.forEach || effect.forEachVote) return false;
  if (effect.effect === "change-targets" && context.stackPlacement && context.singleTargetStackGuard && context.lexicalAbilityPath &&
      effect.scope === "each" && !effect.also && !effect.to && !effect.newTarget && effect.who.player === "you" &&
      isDeepStrictEqual(effect.what, { ref: "it" }) &&
      isDeepStrictEqual(effect.reselection, { method: "random", unit: "target-slot", whenUnavailable: "keep" })) {
    const guard = this.at(`${context.lexicalAbilityPath}/condition/what`);
    if (guard?.id !== context.stackPlacement.id || context.objects.some(object => object.id !== guard.id)) return false;
    const rules = ["115.7e", "115.7f", "115.8", "603.4"];
    const subject = this.bound(effect.what, child(path, "what"), guard.id, rules);
    const chooser = this.player(effect.who, child(path, "who"), context);
    if (!chooser) return true;
    const event = this.event(path, context, effect.effect, [subject, chooser]);
    this.node({ id: this.id(path, "random-target-reselection"), path, node: "random-target-reselection",
      scope: context.ability, event, subject: subject.id, guard: "single-target-intervening-if", rules });
    this.engineFeature("stack-instruction", path, "Reselect the one slot among all legal options after rechecking the intervening condition.", rules);
    return true;
  }
  if (effect.effect !== "copy-stack-object" || !isDeepStrictEqual(effect.what, { ref: "trigger-event-object", event: "cast" }) ||
      context.triggerObject?.event !== "cast" || !context.castActors || effect.copyExceptions || effect.copyGrant ||
      !isDeepStrictEqual(effect.count, { amount: "fixed", value: 1 })) return false;
  const { caster, filter } = context.castActors;
  if (context.players.some(player => player.id !== caster.id && !this.weakPlayers.has(player.id))) return false;
  const otherPlayers = effect.who.player === "filtered" && effect.who.base.player === "each-player" && !effect.who.random &&
    isDeepStrictEqual(effect.who.filter, { otherThan: { player: "that-player" } }) &&
    !effect.fixedTargets && effect.retarget?.optional && isDeepStrictEqual(effect.retarget.who, effect.who);
  const fixed = effect.fixedTargets;
  const target = filter.targetsOnly;
  const alternateTargets = !effect.retarget && effect.who.player === "that-player" && fixed &&
    target && "ref" in target && target.ref === "filter" && objectCardinality(target) === "one" && isDeepStrictEqual(target.filter, fixed.filter) &&
    Object.keys(fixed.filter).length === 1 && fixed.filter.subtypes?.length === 1;
  if (!otherPlayers && !alternateTargets) return false;

  const subject = this.object(effect.what, child(path, "what"), context);
  if (!subject) return true;
  const rules = ["603.2c", "707.10", otherPlayers ? "707.10c" : "707.10d", ...(otherPlayers ? ["101.4", "405.3"] : ["115.9c"])];
  if (alternateTargets && fixed) {
    this.bound(effect.who, child(path, "who"), caster.id, rules);
    const event = this.event(path, context, effect.effect, [subject, caster]);
    this.node({ id: this.id(path, "alternate-target-copy"), path, node: "alternate-target-copy", scope: context.ability,
      event, subject: subject.id, caster: caster.id, targetPolicy: fixed, rules });
  } else if (otherPlayers && effect.who.player === "filtered" && effect.retarget) {
    const at = child(path, "who");
    const everyone = this.player(effect.who.base, child(at, "base"), context);
    if (!everyone) return true;
    const group = this.node({ id: this.id(at, "relative-players"), path: at, node: "relative-player-group",
      scope: context.ability, base: everyone.id, filter: { kind: "other-than", player: caster.id }, cardinality: "many", rules });
    const member = this.node({ id: this.id(at, "instruction-member"), path: at, node: "instruction-member",
      scope: context.ability, group, domain: "player", rules });
    this.bound({ player: "that-player" }, `${at}/filter/otherThan`, caster.id, rules);
    this.bound({ player: "that-player" }, `${path}/retarget/who/filter/otherThan`, caster.id, rules);
    this.bound(effect.who.base, `${path}/retarget/who/base`, everyone.id, rules);
    this.bound(effect.who, at, member, rules);
    this.bound(effect.retarget.who, `${path}/retarget/who`, member, rules);
    const event = this.event(path, context, effect.effect, [subject, { id: member, cardinality: "one", domain: "player" }]);
    this.node({ id: this.id(path, "per-player-copies"), path, node: "per-player-copies",
      scope: context.ability, event, subject: subject.id, group, member, retarget: "optional-per-copy", rules });
  }
  this.engineFeature("stack-instruction", path, "Create copies only for the bound eligible choices and preserve the printed target policy.", rules);
  context.uncertainStackOrigin = true;
  context.objects = [];
  context.mentionedObject = true;
  return true;
}
