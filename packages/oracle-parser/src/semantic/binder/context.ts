// Binding contexts, antecedent candidates and reference-shape helpers.
import { isDeepStrictEqual } from "node:util";
import type { Amount, CascadeExiledCards, Effect, ObjectFilter, ObjectRef, PlayerDesignation } from "../../ast.ts";
import { isPluralPlayerRef } from "../../parser/refs.ts";
import type { BindingCardinality, BindingPhase, ReferenceExpression } from "../types.ts";
import { type ChoiceOrigin, type ResolutionCountOperation } from "./choices.ts";
import type { LinkedExileOrigin } from "./replacement-shapes.ts";

export const RULES_URL = "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt";
export const ENTRY_CHOICE_RULES = ["400.7", "607.2d", "607.5a", "614.12a", "707.6"];
export const DESIGNATION_RULES: Readonly<Record<PlayerDesignation, string>> = {
  monarch: "725.1",
  initiative: "726.1",
  "city's-blessing": "702.131c",
  "enduring-story": "702.195b",
};
export interface Candidate { id: string; cardinality: BindingCardinality; domain: "object" | "object-or-player" | "player" | "number"; description?: ObjectFilter;
  /** The departed object this one became in its new zone (CR 400.7e, 400.7j). */
  successorOf?: string;
  /** Meant only by a reference that names its kind ("that spell"), never by
   * a bare pronoun: the spell or ability that targeted the trigger's object. */
  qualifiedOnly?: true;
  /** Meant only when nothing else is named: the card a zone-change trigger
   * saw an object become (CR 400.7e), or a discarded card. */
  triggerSuccessor?: true }
export type StackCopyCapture = { path: string; event: string; result: Candidate; chooser: string; inlineRetarget: boolean };
export type ScanCapture = { event: string; operation: "reveal" | "exile"; pool: Candidate; matches: Candidate };
export interface Context {
  source: string; face: string; ability: string; you: string; phase: BindingPhase;
  objects: Candidate[]; players: Candidate[]; quantities: Candidate[]; x?: string;
  spellAbility?: boolean;
  directCreatureSpellCopy?: Extract<Effect, { effect: "copy-stack-object" }>;
  sourceMoved?: boolean;
  /** The object the source became in the public zone a zone-change trigger
   * saw it move to (CR 400.7e). */
  sourceSuccessor?: Candidate;
  /** The object of one lexical cast/activation occurrence, independent of
   * salient nouns in the surrounding text. Never derived from proximity. */
  triggerObject?: { event: "cast" | "activate"; candidate: Candidate };
  exchangeEntrant?: Candidate;
  quotedFightSide?: { paths: string[]; source: string };
  ownLeaveEvent?: string;
  exileCounterFromSelected?: string;
  castActors?: { caster: Candidate; filter: ObjectFilter };
  stackPlacement?: Candidate;
  singleTargetStackGuard?: true;
  firstUpkeep?: { scope: string; occurrence: string; player: Candidate };
  /** A scope whose capture/antecedent rules are not implemented. */
  barrier?: string;
  replacementSubjectPath?: string;
  exactEventObject?: { path: string; id: string; rules: string[] };
  exactEventQuantity?: { path: string; id: string };
  eventPredicate?: true;
  sourceExclusionPath?: string;
  sacrificedCost?: Candidate;
  enteringKickerX?: string;
  zoneAlternatives?: { kind: "dies" | "leaves" | "enters"; event: string; previous?: string; successor?: string }[];
  graveyardExclusion?: { filterPath: string; view: string };
  stackCopies?: StackCopyCapture[];
  previousEffect?: string;
  precedingSentence?: { path: string; steps: Effect[] };
  directCardChoice?: { path: string; result: Candidate };
  lookedCardChoice?: { path: string; pool: Candidate; excluding: string };
  performedRepeat?: { decision: string; path: string };
  groupChoice?: { path: string; group: Candidate; member: Candidate; result: Candidate; eachType: boolean; ownNonlands: boolean };
  explicitExiledCast?: { path: string; result: Candidate };
  boundOperand?: { path: string; candidate: Candidate; rules: string[] };
  targetLegalityQuery?: true;
  interveningIf?: true;
  resolutionCount?: { path: string; operation: ResolutionCountOperation };
  /** Inside a counter division with no announcement scope (CR 601.2d). */
  unannouncedDivision?: boolean;
  exileResults?: { event: string; result: Candidate; faceDown?: true }[];
  uncertainExileOrigin?: boolean;
  uncertainStackOrigin?: boolean;
  characteristicSubject?: string;
  comparisonExclusion?: { path: string; subject: string };
  affectedMember?: Candidate;
  scans?: ScanCapture[];
  dieResults?: Candidate[];
  uncertainDieOrigin?: boolean;
  uncertainScanOrigin?: boolean;
  uncertainRevealOrigin?: boolean;
  copiedCards?: Candidate[];
  linkedCopies?: { path: string; result: Candidate; caster: Candidate; optional: boolean };
  uncertainCardCopyOrigin?: boolean;
  announcementMember?: string;
  targetPlayerGroup?: true;
  definiteEventPlayer?: Candidate;
  attackedTriggerPlayer?: Candidate;
  measuredEvents?: { operation: "damage" | "lose-life"; event: string }[];
  uncertainMeasureOrigin?: boolean;
  triggerX?: string;
  lifeGain?: { player: string; quantity: string };
  resolutionPaymentX?: { path: string; id: string };
  linkedExile?: LinkedExileOrigin;
  countedLinkedPool?: Candidate;
  namedVote?: { path: string; event: string; names: string[] };
  unexportedSecretChoices?: true;
  weightedMode?: string;
  lexicalAbilityPath?: string;
  delayedSchedule?: boolean;
  declaredTargets?: Candidate[];
  uncertainTargetOrigin?: boolean;
  copyModelExclusion?: { path: string; model: Candidate; reference: ObjectRef };
  castHistoryExclusionPath?: string;
  classLevel?: number;
  entryChoice?: ChoiceOrigin;
  resolutionChoice?: ChoiceOrigin;
  availableChoice?: ChoiceOrigin;
  announcementOperand?: boolean;
  /** The object(s) an enclosing effect gives abilities to: a bound object or
   * group, an object it creates, or a player. */
  recipient?: { candidate: Candidate } | { created: true; emblemController?: string } | { unresolved: true };
  /** The source is a permanent (or an object that receives an ability), so an
   * unqualified "other" excludes it. */
  permanentSource?: boolean;
  /** A static ability: its bare "it" means the object with the ability when
   * no other object is named, whether a permanent or a spell ("This spell
   * costs {2} less to cast if it targets a tapped creature"). */
  staticAbility?: boolean;
  /** A cast or activation trigger that does not name this object: a bare
   * "it" with no other antecedent is the spell or ability it is about. */
  spellPronoun?: boolean;
  /** An earlier trigger subject or instruction of this ability names an object. */
  mentionedObject?: boolean;
  /** An instruction of this ability names the source as a participant
   * ("unattach Elbrus"), so it is a real antecedent, not a fallback. */
  sourceNamed?: boolean;
  /** Objects an earlier instruction moved out of their zone; their last-known
   * information is still available (CR 608.2h). */
  departed?: Candidate[];
  /** The ability that announces targets named here, when it is not the
   * ability being bound (a delayed trigger whose targets its creator chose). */
  targetScope?: string;
  /** The one attacking creature an attack trigger names (CR 508.5). */
  attacker?: Candidate;
  /** The ability belongs to a creature that is the attacking creature its
   * "defending player" refers to: its own attack trigger or attack rule. */
  attackingSource?: boolean;
  /** The acting player of the instruction being walked, when not "you". */
  actor?: Candidate;
  /** Objects the instruction being walked names as its subject (its damage
   * source, the object whose owner or controller acts). */
  subjects?: Candidate[];
  /** Objects an intervening "if" clause names as its subject, and the objects
   * established before it. A bare pronoun may mean a named object, but not
   * past an object the effect introduces later. */
  named?: { ids: string[]; before: string[] };
  /** Votes held earlier in this ability (CR 701.38). */
  votes?: string[];
  /** Optional instructions decided in the preceding sentence. */
  previousDecisions?: string[];
  /** Paths of the optional instructions decided in the current sentence. */
  decisions?: string[];
  /** X as announced for this spell or ability, when its text does not define X. */
  announcedX?: string;
  /** The object the condition governing the instructions being walked is
   * about ("If that creature has power 4 or greater, it gains ..."). */
  conditionSubject?: { candidate: Candidate; before: string[] };
  /** The optional instruction an "If you do" sentence being walked refers to. */
  linkedPath?: string;
  /** The condition being walked is "if you do"/"if you don't", linked to one decision. */
  linkedDecision?: boolean;
  /** Instructions performed only if a sentence's condition holds. */
  conditional?: boolean;
  /** The reference being walked asks for an object's information (its
   * controller, owner, or a characteristic), so last-known information may answer it. */
  lastKnown?: boolean;
  /** The player deciding whether the enclosing optional instructions are
   * performed (CR 608.2d); "unresolved" when that player is not bound. */
  decider?: string;
}
export function child(path: string, part: string | number): string {
  return `${path}/${String(part).replace(/~/g, "~0").replace(/\//g, "~1")}`;
}
export function cardCount(amount: Amount | undefined): BindingCardinality {
  return amount?.amount === "fixed" ? amount.value === 1 ? "one" : "many" : "unknown";
}
export function objectCardinality(ref: ObjectRef | CascadeExiledCards): BindingCardinality {
  switch (ref.ref) {
    case "self": case "this": case "it": case "enchanted": case "equipped": case "fortified": case "haunted-creature": case "targeted-by": case "trigger-event-object": case "amassed-army": return "one";
    case "them": case "each": case "all": case "zone-cards": case "cascade-exiled-cards": case "rest": case "vote-leaders": case "soulbond-pair": case "pile": return "many";
    case "related": case "copy-result": case "choice-result": return ref.number === "singular" ? "one" : "many";
    case "that": return ref.plural ? "many" : "one";
    case "library-cards":
      return ref.of?.owner === "each-player" || ref.of?.owner === "each-opponent" ? "many" : cardCount(ref.count);
    case "target": case "filter": case "commander": case "graveyard-cards": case "from-pool": return cardCount(ref.count);
    case "any-target": return ref.count ? cardCount(ref.count) : "one";
    case "replacement-event-object": case "prevention-event-object": case "card-extraction-group": return "unknown";
  }
}
/** Distinct candidates in order; the first description of each is kept. */
export function combine(a: Candidate[], b: Candidate[]): Candidate[] {
  const distinct = new Map<string, Candidate>();
  for (const candidate of [...a, ...b]) if (!distinct.has(candidate.id)) distinct.set(candidate.id, candidate);
  return [...distinct.values()];
}
export function unambiguousPluralRecipient(context: Context): Candidate | undefined {
  const candidates = combine(context.objects, context.players);
  const [recipient] = candidates;
  if (!context.barrier && candidates.length === 1 && recipient.domain === "object" &&
      recipient.cardinality === "many" && !recipient.qualifiedOnly) return recipient;
}
export function description(ref: ObjectRef | CascadeExiledCards): ObjectFilter | undefined {
  if ("filter" in ref) return ref.filter;
  // CR 115.4: "any target" is a creature, player, planeswalker or battle.
  if (ref.ref === "any-target") return { anyOf: [{ types: ["creature"] }, { types: ["planeswalker"] }, { types: ["battle"] }] } as ObjectFilter;
  // CR 301.5, 303.4: what an Equipment, Aura or Fortification is attached to.
  if ((ref.ref === "equipped" || ref.ref === "enchanted" || ref.ref === "fortified") && "noun" in ref && typeof ref.noun === "string"
    && ["creature", "artifact", "enchantment", "land", "planeswalker", "permanent"].includes(ref.noun))
    return ref.noun === "permanent" ? { cls: "permanent" } : { types: [ref.noun] } as ObjectFilter;
  if (ref.ref === "library-cards" || ref.ref === "graveyard-cards" || ref.ref === "zone-cards" || ref.ref === "cascade-exiled-cards") return { cls: "card" };
  if (ref.ref === "this") {
    if (["card", "spell", "permanent", "ability"].includes(ref.noun)) return { cls: ref.noun as "card" | "spell" | "permanent" | "ability" };
    if (["creature", "artifact", "enchantment", "land", "planeswalker", "battle"].includes(ref.noun)) return { types: [ref.noun] };
  }
  return undefined;
}
export function domain(expression: ReferenceExpression): Candidate["domain"] {
  if ("player" in expression || "implicit" in expression) return "player";
  if ("choice" in expression) return "number"; // Values never become participant antecedents.
  if ("recipient" in expression) return "object"; // Only a proven cast-spell antecedent binds this form.
  if (!("ref" in expression)) return "number";
  return expression.ref === "any-target" || ("filter" in expression && (expression.filter.orPlayer || expression.filter.orOpponent)) ? "object-or-player" : "object";
}
export function referenceCardinality(expression: ReferenceExpression): BindingCardinality {
  return "ref" in expression ? expression.ref === "affected-object" ? "one" : expression.ref === "those-counters" ? "many" : objectCardinality(expression)
    : "implicit" in expression || "recipient" in expression || ("player" in expression && isPluralPlayerRef(expression)) ? "many" : "one";
}
/** This is an antecedent-description check, not a current-characteristics
 * predicate. A later "that" never chooses a new object by its current type. */
export const PERMANENT_TYPES = new Set(["artifact", "battle", "creature", "enchantment", "land", "planeswalker"]);
/** Every object the filter describes is a permanent (CR 110.4). */
export function impliesPermanent(filter: ObjectFilter): boolean {
  const any = (filter as { anyOf?: ObjectFilter[] }).anyOf, all = (filter as { allOf?: ObjectFilter[] }).allOf;
  return filter.cls === "permanent" || filter.cls === "token"
    || (!filter.cls && !filter.zone && Array.isArray(filter.types) && filter.types.length > 0 && filter.types.every(type => PERMANENT_TYPES.has(type)))
    || (Array.isArray(any) && any.length > 0 && any.every(impliesPermanent)) || (Array.isArray(all) && all.some(impliesPermanent));
}
export function describes(actual: ObjectFilter | undefined, required: ObjectFilter): boolean {
  if (!actual) return Object.keys(required).length === 0;
  if (isDeepStrictEqual(actual, required)) return true;
  if (actual.anyOf?.length) {
    const { anyOf, ...common } = actual;
    return describes(common, required) || anyOf.every(clause => describes(clause, required));
  }
  if (required.anyOf?.length || required.allOf?.length) {
    const { anyOf, allOf, ...common } = required;
    return describes(actual, common) && (!anyOf || anyOf.some(clause => describes(actual, clause))) &&
      (!allOf || allOf.every(clause => describes(actual, clause)));
  }
  if (actual.allOf?.some(clause => describes(clause, required))) return true;
  // "That permanent", "that card": what the established description implies.
  if (Object.keys(required).length === 1 && required.cls === "permanent") return impliesPermanent(actual);
  if (Object.keys(required).length === 1 && required.cls === "card") return actual.cls === "card";
  // CR 109.2: "creature" without "card" or "spell" is a permanent on the
  // battlefield; a card in a graveyard or a spell is not "that creature".
  if (!required.cls && (required.types?.length || required.subtypes?.length) && (actual.cls === "card" || actual.cls === "spell")) return false;
  return Object.entries(required).every(([key, value]) => {
    const known = actual[key as keyof ObjectFilter];
    return Array.isArray(value) && Array.isArray(known)
      ? value.every(item => known.some(candidate => JSON.stringify(candidate) === JSON.stringify(item)))
      : JSON.stringify(value) === JSON.stringify(known);
  });
}
