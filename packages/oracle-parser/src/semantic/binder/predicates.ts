// Structural predicates over parsed AST values.
import type { Color, Cost, Effect, ObjectFilter, StaticEffect } from "../../ast.ts";

export const SYMBOLIC_AMOUNTS = new Set(["x", "that-number", "that-much", "replacement-event-quantity"]);
export type StackInstruction = Extract<Effect, { effect: "copy-stack-object" | "change-targets" | "choose-new-targets" | "change-text" }>;
export type StackObjectClass = "spell" | "ability" | "spell-or-ability";
export function stackClasses(filter: ObjectFilter | undefined): StackObjectClass | undefined {
  if (!filter || filter.orPlayer || filter.orOpponent) return;
  if (filter.cls === "spell" || filter.cls === "ability") return filter.cls;
  for (const clause of filter.allOf ?? []) {
    const classes = stackClasses(clause);
    if (classes) return classes;
  }
  if (!filter.anyOf?.length) return;
  const alternatives = filter.anyOf.map(stackClasses);
  if (alternatives.some(kind => !kind)) return;
  return alternatives.every(kind => kind === alternatives[0]) ? alternatives[0] : "spell-or-ability";
}
export type StaticRule = Extract<StaticEffect, { static: "prohibit" | "untap-during-step" | "untap-step-limit" }>;
/** The instruction's own number is a "that many" reference. */
export function echoes(effect: unknown): boolean {
  if (!isRecord(effect)) return false;
  return ["amount", "count"].some(key => isRecord(effect[key]) && JSON.stringify(effect[key]).includes('"amount":"that-much"'));
}
export function coloredSearchSlots(effect: Effect): Color[] | undefined {
  if (effect.effect !== "search" || Object.keys(effect).length !== 4 || effect.who?.player !== "you"
    || Object.keys(effect.who).length !== 1 || effect.zones.selection !== "all" || effect.zones.of.length !== 1
    || Object.keys(effect.zones).length !== 2) return;
  const [library] = effect.zones.of;
  const group = effect.for;
  if (library.zone !== "library" || library.owner !== "your" || Object.keys(library).length !== 2
    || group.ref !== "card-extraction-group" || group.combine !== "all" || group.allocation !== "distinct-objects"
    || Object.keys(group).length !== 4 || group.members.length < 2) return;
  const colors = new Set<Color>();
  for (const slot of group.members) {
    const { filter, count } = slot;
    if (Object.keys(slot).length !== 2 || count.amount !== "fixed" || count.value !== 1 || Object.keys(count).length !== 2
      || filter.cls !== "card" || Object.keys(filter).length !== 2 || filter.colors?.length !== 1) return;
    const [color] = filter.colors;
    if (!["white", "blue", "black", "red", "green"].includes(color) || colors.has(color)) return;
    colors.add(color);
  }
  return [...colors];
}
export function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
/** A keyword ability instance, as opposed to the other nodes that name a
 * keyword (conditions, triggers, keyword actions, keyword-labelled abilities). */
export function isKeywordInstance(value: Record<string, unknown>): boolean {
  return typeof value.keyword === "string" &&
    !["action", "condition", "trigger", "use", "static", "effect", "kind", "gained", "pretend"].some(key => key in value);
}
/** References that name a particular object a later "it" could mean: any
 * object a cost selects, or (in a static ability) a definite description. */
export const INDEFINITE_REFS = new Set(["filter", "each", "all", "zone-cards", "library-cards", "graveyard-cards", "commander", "from-pool"]);
export function namesObject(value: unknown, definiteOnly: boolean): boolean {
  if (Array.isArray(value)) return value.some(entry => namesObject(entry, definiteOnly));
  if (!isRecord(value)) return false;
  if (typeof value.ref === "string" && !["self", "this", "it", "them"].includes(value.ref) && !(definiteOnly && INDEFINITE_REFS.has(value.ref))) return true;
  return Object.values(value).some(entry => namesObject(entry, definiteOnly));
}
/** A constraint on a chosen X is not a printed definition of X (CR 107.3a, c). */
export function definesX(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(definesX);
  return isRecord(value) && ((value.variable === "x" && isRecord(value.value) && typeof value.value.amount === "string")
    || Object.values(value).some(definesX));
}
export function costHasX(value: unknown): boolean {
  return /"X"|"amount":"x"/.test(JSON.stringify(value ?? null));
}
export function soleObjectPayment(costs: Cost[]): number {
  const index = costs.findIndex(cost => (cost.cost === "sacrifice" || cost.cost === "exile") && cost.what.ref === "filter");
  return costs.every((cost, at) => at === index ||
    cost.cost === "mana" || cost.cost === "tap-self" || cost.cost === "untap-self" ||
    (cost.cost === "pay-life" && cost.amount.amount === "fixed")) ? index : -1;
}
export function containsEffect(value: unknown, effect: string): boolean {
  if (Array.isArray(value)) return value.some(entry => containsEffect(entry, effect));
  return isRecord(value) && (value.effect === effect || Object.values(value).some(entry => containsEffect(entry, effect)));
}
/** Printed keyword parameters that are paid or chosen as the keyword's cost. */
export const KEYWORD_PAYMENT_FIELDS = new Set(["cost", "additionalCosts", "materials", "costModifications", "restriction", "variableConstraints"]);
export const FACE_LAYOUTS = new Set(["transform", "modal_dfc", "adventure", "split", "flip", "reversible_card", "double_faced_token"]);
