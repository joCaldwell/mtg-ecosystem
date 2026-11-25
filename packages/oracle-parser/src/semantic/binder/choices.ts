// Value-choice and resolution-count origins.
import type { Ability, Amount, Effect, Sentence } from "../../ast.ts";
import type { BindingNode, ReferenceExpression } from "../types.ts";
import { child, type Context } from "./context.ts";
import { isRecord } from "./predicates.ts";

export type ChoiceEffect = Extract<Effect, { effect: "choose-value" }>;
export type ChoiceOrigin = { category: ChoiceEffect["category"]; path: string };
export type ChoiceReference = Extract<ReferenceExpression, { choice: string }>;
export function choiceCount(value: unknown): number {
  if (Array.isArray(value)) return value.reduce((count, entry) => count + choiceCount(entry), 0);
  if (!isRecord(value)) return 0;
  if (value.effect === "choose-value") return 1;
  return Object.values(value).reduce<number>((count, entry) => count + choiceCount(entry), 0);
}
/** A mandatory scalar choice with an unconditional value domain. */
export function simpleChoice(effect: Effect | undefined): effect is ChoiceEffect {
  return effect?.effect === "choose-value" &&
    (Object.keys(effect).length === 3 ||
      (Object.keys(effect).length === 4 && (
        (effect.category === "card-type" && effect.within === "permanent-types") ||
        (effect.category === "type" && effect.options.length > 0 && effect.options.every(option => option.type === "card-type"))
      ))) &&
    effect.who.player === "you" && Object.keys(effect.who).length === 1 &&
    ["color", "card-type", "creature-type", "basic-land-type", "land-type", "parity", "number", "type"].includes(effect.category);
}
export function entryChoice(abilities: Ability[], path: string): ChoiceOrigin | undefined {
  // CR 607.1, 607.2d: link only an unambiguous directly printed producer.
  if (choiceCount(abilities) !== 1) return;
  for (const [index, ability] of abilities.entries()) {
    if (ability.kind !== "static" || ability.effect.static !== "as-enters") continue;
    const { what, effects } = ability.effect;
    if (!["self", "this"].includes(what.ref) || Object.keys(ability.effect).length !== 3 || effects.length !== 1) continue;
    const [sentence] = effects;
    if (Object.keys(sentence).length === 1 && sentence.steps.length === 1 && simpleChoice(sentence.steps[0]))
      return { category: sentence.steps[0].category === "type" ? "card-type" : sentence.steps[0].category, path: `${path}/${index}/effect/effects/0/steps/0` };
  }
}
export function resolutionChoice(ability: Ability, path: string): ChoiceOrigin | undefined {
  if (ability.kind !== "spell" && ability.kind !== "activated" && ability.kind !== "triggered" && ability.kind !== "chapter") return;
  if (choiceCount(ability) !== 1) return;
  return choiceInSentences(ability.effects, child(path, "effects"));
}
export function choiceInSentences(effects: Sentence[], path: string): ChoiceOrigin | undefined {
  // A choice in another branch could supply the value. Do not substitute an earlier choice.
  if (choiceCount(effects) !== 1) return;
  for (const [index, sentence] of effects.entries()) {
    if (sentence.condition || sentence.otherwise || sentence.afterConditional) continue;
    for (const [step, effect] of sentence.steps.entries())
      if (simpleChoice(effect)) return { category: effect.category === "type" ? "card-type" : effect.category,
        path: `${path}/${index}/steps/${step}` };
  }
}
export type ResolutionCountOperation = Extract<BindingNode, { node: "resolution-count" }>["operation"];
export function chosenRange(amount: Amount | undefined): { minimum: number; maximum?: number } | undefined {
  if (amount?.amount !== "chosen" || !Number.isSafeInteger(amount.minimum) || amount.minimum < 0) return;
  if (amount.maximum === undefined) return { minimum: amount.minimum };
  if (amount.maximum.amount === "fixed" && Number.isSafeInteger(amount.maximum.value) &&
      amount.maximum.value >= amount.minimum) return { minimum: amount.minimum, maximum: amount.maximum.value };
}
/** CR 601.2c-d: "distribute N counters among one or two target creatures".
 * The target count is announced with the targets and the fixed total is
 * divided among them at the same time, each target receiving at least one, so
 * the count can never exceed the total. Any other division (any number of
 * targets, an X or measured total, an untargeted set, an optional or repeated
 * instruction) has no announcement scope here and stays unsupported. */
export function announcedDivision(effect: Effect): { total: number } | undefined {
  if (effect.effect !== "distribute-counters" || effect.optional || effect.forEach || effect.forEachVote || effect.decidedBy) return;
  const { among, count } = effect;
  if (among.ref !== "target" || !among.selectionId || among.upTo || among.chosenBy || among.random || among.groupConstraints?.length) return;
  const range = chosenRange(among.count);
  if (count.amount !== "fixed" || !range || range.minimum < 1 || range.maximum === undefined || range.maximum > count.value) return;
  return { total: count.value };
}
export function resolutionCount(effect: Effect, path: string): Context["resolutionCount"] {
  if (effect.forEach || effect.forEachVote || effect.decidedBy) return;
  if (effect.effect === "discard" && !effect.random && effect.who.player === "you" &&
      effect.what !== "hand" && effect.what.ref === "filter" && effect.what.filter.cls === "card")
    return { path: child(path, "what"), operation: "discard" };
  if (effect.effect === "sacrifice" && !effect.chosenBy && effect.who.player === "you" && effect.what.ref === "filter")
    return { path: child(path, "what"), operation: "sacrifice" };
  if (effect.effect === "move-zone" && (!effect.who || effect.who.player === "you") &&
      effect.to.zone === "battlefield" && !effect.order && !effect.from &&
      effect.what.ref === "filter" && effect.what.filter.cls === "card" &&
      effect.what.filter.zone?.zone === "hand" && effect.what.filter.zone.owner === "your")
    return { path: child(path, "what"), operation: "move-from-hand" };
  if (effect.effect === "search" && effect.who.player === "you" && effect.zones.selection === "all" &&
      effect.zones.of.length === 1 && effect.zones.of[0].zone === "library" && effect.zones.of[0].owner === "your" &&
      effect.for.ref === "filter" && effect.for.filter.cls === "card" && !effect.for.groupConstraints?.length)
    return { path: child(path, "for"), operation: "search" };
}
