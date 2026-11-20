// This pass resolves reference identities, not game execution or all semantic
// correctness. In particular, it never substitutes "last noun" for an
// ambiguous referent. Every unsupported scope remains a reported obligation.
// Rules: CR 109.5, 201.5, 400.7j, 601.2c, 608.2h/k, 107.3, 607.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import { isDeepStrictEqual } from "node:util";
import type { Ability, Amount, CascadeExiledCards, CharacteristicRelation, Color, Condition, Cost, CostModification, Effect, EntryCopy, GainedAbility, KeywordInstance, LookPermission, ObjectFilter, ObjectRef, PlayerRef, PlayerDesignation, ReplacementInstruction, Sentence, StaticEffect, Trigger } from "../ast.ts";
import { KEYWORD_SCOPES } from "./keyword-scopes.ts";
import { isPluralPlayerRef } from "../parser/refs.ts";
import { uniqueStackTriggerEvent } from "../parser/stack-copy.ts";
import type {
  BindingCardinality, BindingDiagnosticCode, BindingNode, BindingPhase, BindingSourceFace,
  CardBindingAnalysis, CardBindingInput, EngineFeature, ReferenceBinding, ReferenceExpression,
} from "./types.ts";

const RULES_URL = "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt";
const ENTRY_CHOICE_RULES = ["400.7", "607.2d", "607.5a", "614.12a", "707.6"];
const DESIGNATION_RULES: Readonly<Record<PlayerDesignation, string>> = {
  monarch: "725.1",
  initiative: "726.1",
  "city's-blessing": "702.131c",
  "enduring-story": "702.195b",
};
// Double Major's complete spell paragraph. A retarget option, another copy
// exception or an enclosing choice would require different bindings.
const DIRECT_CREATURE_SPELL_COPY = {
  effect: "copy-stack-object", who: { player: "you" }, count: { amount: "fixed", value: 1 },
  what: { ref: "target", count: { amount: "fixed", value: 1 }, selectionId: "line:0:1",
    filter: { cls: "spell", types: ["creature"], control: "you" } },
  copyExceptions: [{ change: "types", types: { supertypes: { operation: "remove", types: ["legendary"] } } }],
} satisfies Effect;
// The singular match, the remainder and the exile successor have different
// identities. Only the complete sequence below establishes all three links.
const REVEAL_AND_CAST = [
  { steps: [
    { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: "your" } },
    { effect: "scan-library", who: { player: "you" }, operation: "reveal", library: { zone: "library", owner: "your" },
      until: { count: { amount: "fixed", value: 1 }, matching: { nonTypes: ["land"], cls: "card" } } },
  ] },
  { steps: [
    { effect: "exile", what: { ref: "that", filter: { cls: "card" } } },
    { effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random" },
  ] },
  { steps: [{ effect: "play-card", optional: true, instruction: { who: { player: "you" }, action: "cast",
    what: { ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    alternativeCost: { cost: "without-mana-cost" } } }] },
] as const satisfies Sentence[];
function isRevealAndCast(ability: Extract<Ability, { kind: "spell" }>): ability is { kind: "spell"; effects: typeof REVEAL_AND_CAST } {
  const { abilityWord, ...paragraph } = ability;
  return abilityWord === undefined && isDeepStrictEqual(paragraph, { kind: "spell", effects: REVEAL_AND_CAST });
}
// CR 607.2a: "the exiled card" in this resolution instruction refers to a
// different printed ability. Match its entire paragraph, including the cost
// and resolution condition, before supplying a cross-ability reference.
const HIDEAWAY_PLAY = {
  effect: "play-card", optional: true, instruction: {
    who: { player: "you" }, action: "play",
    what: { ref: "related", number: "singular", filter: { cls: "card" },
      relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    alternativeCost: { cost: "without-mana-cost" },
    condition: { condition: "compare", value: { amount: "attribute-sum", attribute: "power",
      among: { ref: "each", filter: { types: ["creature"], control: "you" } } },
      comparison: { op: "ge", value: { amount: "fixed", value: 10 } } },
  },
} as const satisfies Effect;
const HIDEAWAY_FOUR = { keyword: "hideaway", amount: 4 } as const satisfies KeywordInstance;

// Only these independent entry and mana paragraphs may accompany the pair.
// A second hideaway, another exile producer or a granted ability must never
// supply this link. Printed order is immaterial.
function hideawayProducer(abilities: Ability[], activation: Extract<Ability, { kind: "activated" }>): number | undefined {
  const { abilityWord, restriction, ...paragraph } = activation;
  if (abilityWord !== undefined || restriction !== undefined || !isDeepStrictEqual(paragraph, {
    kind: "activated", costs: [{ cost: "mana", symbols: ["G"] }, { cost: "tap-self" }],
    effects: [{ steps: [HIDEAWAY_PLAY] }],
  })) return undefined;
  let producer: number | undefined;
  for (const [index, ability] of abilities.entries()) {
    if (ability === activation) continue;
    if (isDeepStrictEqual(ability, { kind: "keywords", keywords: [HIDEAWAY_FOUR] }) && producer === undefined) {
      producer = index;
      continue;
    }
    if (ability.kind === "static") {
      const { abilityWord, ...entry } = ability;
      if (abilityWord === undefined && isDeepStrictEqual(entry, {
        kind: "static", effect: { static: "enters-tapped", what: { ref: "this", noun: "land" } },
      })) continue;
    }
    if (ability.kind === "activated") {
      const { abilityWord, restriction, ...mana } = ability;
      if (abilityWord === undefined && restriction === undefined && isDeepStrictEqual(mana, {
        kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "add-mana",
          who: { player: "you" }, mana: { mana: "fixed", symbols: ["G"] } }] }],
      })) continue;
    }
    return undefined;
  }
  return producer;
}
const SYMBOLIC_AMOUNTS = new Set(["x", "that-number", "that-much", "replacement-event-quantity"]);
interface Candidate { id: string; cardinality: BindingCardinality; domain: "object" | "object-or-player" | "player" | "number"; description?: ObjectFilter;
  /** The departed object this one became in its new zone (CR 400.7e, 400.7j). */
  successorOf?: string;
  /** Meant only by a reference that names its kind ("that spell"), never by
   * a bare pronoun: the spell or ability that targeted the trigger's object. */
  qualifiedOnly?: true;
  /** Meant only when nothing else is named: the card a zone-change trigger
   * saw an object become (CR 400.7e), or a discarded card. */
  triggerSuccessor?: true }
type ChoiceEffect = Extract<Effect, { effect: "choose-value" }>;
type ChoiceOrigin = { category: ChoiceEffect["category"]; path: string };
type ChoiceReference = Extract<ReferenceExpression, { choice: string }>;

function choiceCount(value: unknown): number {
  if (Array.isArray(value)) return value.reduce((count, entry) => count + choiceCount(entry), 0);
  if (!isRecord(value)) return 0;
  if (value.effect === "choose-value") return 1;
  return Object.values(value).reduce<number>((count, entry) => count + choiceCount(entry), 0);
}

/** A mandatory scalar choice with an unconditional value domain. */
function simpleChoice(effect: Effect | undefined): effect is ChoiceEffect {
  return effect?.effect === "choose-value" &&
    (Object.keys(effect).length === 3 ||
      (Object.keys(effect).length === 4 && effect.category === "card-type" && effect.within === "permanent-types")) &&
    effect.who.player === "you" && Object.keys(effect.who).length === 1 &&
    ["color", "card-type", "creature-type", "basic-land-type", "land-type", "parity", "number"].includes(effect.category);
}

function entryChoice(abilities: Ability[], path: string): ChoiceOrigin | undefined {
  // CR 607.1, 607.2d: link only an unambiguous directly printed producer.
  if (choiceCount(abilities) !== 1) return;
  for (const [index, ability] of abilities.entries()) {
    if (ability.kind !== "static" || ability.effect.static !== "as-enters") continue;
    const { what, effects } = ability.effect;
    if (!["self", "this"].includes(what.ref) || Object.keys(ability.effect).length !== 3 || effects.length !== 1) continue;
    const [sentence] = effects;
    if (Object.keys(sentence).length === 1 && sentence.steps.length === 1 && simpleChoice(sentence.steps[0]))
      return { category: sentence.steps[0].category, path: `${path}/${index}/effect/effects/0/steps/0` };
  }
}

function resolutionChoice(ability: Ability, path: string): ChoiceOrigin | undefined {
  if (ability.kind !== "spell" && ability.kind !== "activated" && ability.kind !== "triggered" && ability.kind !== "chapter") return;
  // A choice in another branch could supply the value. Do not substitute an earlier choice.
  if (choiceCount(ability) !== 1) return;
  for (const [index, sentence] of ability.effects.entries()) {
    if (sentence.condition || sentence.otherwise || sentence.afterConditional) continue;
    for (const [step, effect] of sentence.steps.entries())
      if (simpleChoice(effect)) return { category: effect.category, path: `${path}/effects/${index}/steps/${step}` };
  }
}

type QuantityUnit = Extract<BindingNode, { node: "quantity-replacement" }>["unit"];

function replacementPlayer(player: PlayerRef | undefined): boolean {
  return player !== undefined && ["you", "a-player", "an-opponent"].includes(player.player);
}

function replacementObject(object: ObjectRef | PlayerRef | undefined, excludeRecipient = false): boolean {
  if (!object || !("ref" in object)) return false;
  if (object.ref === "equipped") return object.noun === "creature";
  return object.ref === "filter" && object.count.amount === "fixed" && object.count.value === 1 &&
    Object.keys(object).length === 3 && Object.keys(object.filter).every(key => ["cls", "types", "supertypes", "control"].includes(key) ||
      (excludeRecipient && key === "otherThan" && isDeepStrictEqual(object.filter.otherThan, { ref: "replacement-event-object", role: "recipient" })));
}

function quantityUnit(event: ReplacementInstruction["event"]): QuantityUnit | undefined {
  switch (event.event) {
    case "create-token":
      if (event.match === "creation-event" && replacementPlayer(event.under) &&
          event.tokens?.cls === "token" && Object.keys(event.tokens).every(key => ["cls", "types"].includes(key)) &&
          event.quantity.value.amount === "fixed") return "token-specification";
      return;
    case "tap-for-mana":
      if (replacementObject(event.what) && (!event.by || replacementPlayer(event.by)) &&
          (!event.quantity || event.quantity.value.amount === "fixed")) return "mana-specification";
      return;
    case "damage":
      if (replacementObject(event.source, event.kind === "combat" && event.to?.length === 1 &&
          !("player" in event.to[0]) && replacementObject(event.to[0])) &&
          event.to?.every(ref => "player" in ref ? replacementPlayer(ref) : replacementObject(ref)) !== false &&
          (!event.quantity || event.quantity.value.amount === "fixed")) return "damage-source-and-recipient";
      return;
    case "gain-life": return replacementPlayer(event.who) ? "life-source-and-recipient" : undefined;
    case "put-counters":
      if (event.on.length > 0 && event.on.every(ref => replacementObject(ref)) && (!event.by || replacementPlayer(event.by)) &&
          event.quantity.value.amount === "fixed") return "counter-kind-and-recipient";
      return;
  }
}

type StackInstruction = Extract<Effect, { effect: "copy-stack-object" | "change-targets" | "choose-new-targets" | "change-text" }>;
type StackObjectClass = "spell" | "ability" | "spell-or-ability";
type StackCopyCapture = { path: string; event: string; result: Candidate; chooser: string; inlineRetarget: boolean };
type ScanCapture = { event: string; operation: "reveal" | "exile"; pool: Candidate; matches: Candidate };

function stackClasses(filter: ObjectFilter | undefined): StackObjectClass | undefined {
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

type ResolutionCountOperation = Extract<BindingNode, { node: "resolution-count" }>["operation"];

function chosenRange(amount: Amount | undefined): { minimum: number; maximum?: number } | undefined {
  if (amount?.amount !== "chosen" || !Number.isSafeInteger(amount.minimum) || amount.minimum < 0) return;
  if (amount.maximum === undefined) return { minimum: amount.minimum };
  if (amount.maximum.amount === "fixed" && Number.isSafeInteger(amount.maximum.value) &&
      amount.maximum.value >= amount.minimum) return { minimum: amount.minimum, maximum: amount.maximum.value };
}

function resolutionCount(effect: Effect, path: string): Context["resolutionCount"] {
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

type StaticRule = Extract<StaticEffect, { static: "prohibit" | "untap-during-step" | "untap-step-limit" }>;

type OptionalEntryCopy = EntryCopy & { model: Extract<EntryCopy["model"], { model: "choose" }>; optional: { who: PlayerRef } };
type LinkedExileOrigin = { ability: string; path: string; imprint: boolean; rule: "607.2a" | "607.2b" };

function ordinaryEntryCopy(effect: EntryCopy): effect is OptionalEntryCopy {
  const { what, model, optional, duration, changes, entryExceptions } = effect;
  if ((what.ref !== "self" && what.ref !== "this") || model.model !== "choose" || model.who.player !== "you" ||
      optional?.who.player !== "you" || model.filter.zoneChangeHistory ||
      (model.filter.zone && model.filter.zone.zone !== "battlefield") ||
      (duration && duration.duration !== "end-of-turn")) return false;
  if (entryExceptions?.some(exception => exception.count.amount !== "fixed")) return false;
  return !changes?.some(change => change.change !== "types" &&
    !(change.change === "abilities" && change.abilities.every(ability => isDeepStrictEqual(ability,
      { gained: "keyword", keyword: { keyword: "haste" } }))));
}

function seeksLinkedExile(value: unknown, imprint: boolean): boolean {
  if (Array.isArray(value)) return value.some(entry => seeksLinkedExile(entry, imprint));
  if (!isRecord(value)) return false;
  return value.relation === "linked-exile" ||
    (imprint && value.relation === "event-result" && value.event === "exile" && value.scope === "contextual") ||
    Object.values(value).some(entry => seeksLinkedExile(entry, imprint));
}

/** A per-object move altered to exile, with only the supported follow-up shape. */
function exileReplacementParts(replacement: ReplacementInstruction) {
  const { event, replace } = replacement;
  if (replacement.occurrence !== "each" || replacement.optional || replacement.condition ||
      event.event !== "move-zone" || !isDeepStrictEqual(event.from, { zone: "battlefield" }) ||
      !isDeepStrictEqual(event.to, { zone: "graveyard" }) || event.notControlledByBefore || event.resolving ||
      event.what.ref !== "filter" || event.what.count.amount !== "fixed" || event.what.count.value !== 1 ||
      !isDeepStrictEqual(event.what.filter.types, ["creature"]) ||
      Object.keys(event.what.filter).some(key => !["nonToken", "types", "control"].includes(key)) ||
      replace.replacement !== "effects" || replace.effects.length < 1 || replace.effects.length > 2 ||
      replace.effects.some(sentence => Object.keys(sentence).length !== 1)) return;
  const [exile, life] = replace.effects[0].steps;
  if (exile?.effect !== "exile" || Object.keys(exile).length !== 2 ||
      !isDeepStrictEqual(exile.what, { ref: "replacement-event-object", role: "affected" }) ||
      replace.effects[0].steps.length > 2) return;
  if (life && (life.effect !== "gain-life" || life.who.player !== "you" || life.amount.amount !== "fixed" ||
      Object.keys(life).length !== 3 || replace.effects.length !== 1)) return;

  const followup = replace.effects[1]?.steps[0];
  if (replace.effects.length === 2 && (replace.effects[1].steps.length !== 1 ||
      followup?.effect !== "create-reflexive-trigger" || Object.keys(followup).length !== 3 ||
      !isDeepStrictEqual(followup.event, { event: "preceding-action", who: { player: "you" }, performed: true }))) return;
  return { event, exile, life, followup };
}

function linkedExileOrigin(abilities: Ability[], siblings: string): LinkedExileOrigin | undefined {
  const possible = abilities.flatMap((ability, index) => containsEffect(ability, "exile") ||
    containsEffect(ability, "scan-library") || containsEffect(ability, "exile-until") ? [{ ability, index }] : []);
  if (possible.length !== 1) return;
  const [{ ability, index }] = possible;
  if (ability.kind === "static" && ability.effect.static === "replacement" &&
      exileReplacementParts(ability.effect.replacement) && abilities.some(other => other !== ability && seeksLinkedExile(other, false))) {
    return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effect/replacement/replace/effects/0/steps/0`,
      imprint: false, rule: "607.2b" };
  }
  if ((ability.kind !== "activated" && ability.kind !== "triggered") || ability.effects.length !== 1) return;
  const [sentence] = ability.effects;
  if (sentence.steps.length !== 1 || sentence.condition || sentence.otherwise || sentence.afterConditional) return;
  const [exile] = sentence.steps;
  if (exile.effect !== "exile" || exile.forEach || exile.forEachVote || exile.decidedBy ||
      (exile.what.ref !== "target" && exile.what.ref !== "filter") || objectCardinality(exile.what) !== "one") return;
  const imprint = ability.abilityWord === "Imprint";
  if (!abilities.some(entry => entry !== ability && seeksLinkedExile(entry, imprint))) return;
  return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effects/0/steps/0`, imprint, rule: "607.2a" };
}

interface Context {
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
  /** A scope whose capture/antecedent rules are not implemented. */
  barrier?: string;
  replacementSubjectPath?: string;
  exactEventObject?: { path: string; id: string; rules: string[] };
  exactEventQuantity?: { path: string; id: string };
  eventPredicate?: true;
  sourceExclusionPath?: string;
  sacrificedCost?: Candidate;
  stackCopies?: StackCopyCapture[];
  previousEffect?: string;
  targetLegalityQuery?: true;
  interveningIf?: true;
  resolutionCount?: { path: string; operation: ResolutionCountOperation };
  distributedCount?: boolean;
  exileResults?: { event: string; result: Candidate }[];
  uncertainExileOrigin?: boolean;
  uncertainStackOrigin?: boolean;
  characteristicSubject?: string;
  comparisonExclusion?: { path: string; subject: string };
  affectedMember?: Candidate;
  scans?: ScanCapture[];
  uncertainScanOrigin?: boolean;
  copiedCards?: Candidate[];
  uncertainCardCopyOrigin?: boolean;
  announcementMember?: string;
  definiteEventPlayer?: Candidate;
  measuredEvents?: { operation: "damage" | "lose-life"; event: string }[];
  uncertainMeasureOrigin?: boolean;
  triggerX?: string;
  linkedExile?: LinkedExileOrigin;
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

/** The instruction's own number is a "that many" reference. */
function echoes(effect: unknown): boolean {
  if (!isRecord(effect)) return false;
  return ["amount", "count"].some(key => isRecord(effect[key]) && JSON.stringify(effect[key]).includes('"amount":"that-much"'));
}
function coloredSearchSlots(effect: Effect): Color[] | undefined {
  if (effect.effect !== "search" || Object.keys(effect).length !== 4 || effect.who.player !== "you"
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
function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
/** A keyword ability instance, as opposed to the other nodes that name a
 * keyword (conditions, triggers, keyword actions, keyword-labelled abilities). */
function isKeywordInstance(value: Record<string, unknown>): boolean {
  return typeof value.keyword === "string" &&
    !["action", "condition", "trigger", "use", "static", "effect", "kind", "gained", "pretend"].some(key => key in value);
}
/** References that name a particular object a later "it" could mean: any
 * object a cost selects, or (in a static ability) a definite description. */
const INDEFINITE_REFS = new Set(["filter", "each", "all", "zone-cards", "library-cards", "graveyard-cards", "commander", "from-pool"]);
function namesObject(value: unknown, definiteOnly: boolean): boolean {
  if (Array.isArray(value)) return value.some(entry => namesObject(entry, definiteOnly));
  if (!isRecord(value)) return false;
  if (typeof value.ref === "string" && !["self", "this", "it", "them"].includes(value.ref) && !(definiteOnly && INDEFINITE_REFS.has(value.ref))) return true;
  return Object.values(value).some(entry => namesObject(entry, definiteOnly));
}
/** A constraint on a chosen X is not a printed definition of X (CR 107.3a, c). */
function definesX(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(definesX);
  return isRecord(value) && ((value.variable === "x" && isRecord(value.value) && typeof value.value.amount === "string")
    || Object.values(value).some(definesX));
}
function costHasX(value: unknown): boolean {
  return /"X"|"amount":"x"/.test(JSON.stringify(value ?? null));
}
function soleObjectPayment(costs: Cost[]): number {
  const index = costs.findIndex(cost => (cost.cost === "sacrifice" || cost.cost === "exile") && cost.what.ref === "filter");
  return costs.every((cost, at) => at === index ||
    cost.cost === "mana" || cost.cost === "tap-self" || cost.cost === "untap-self" ||
    (cost.cost === "pay-life" && cost.amount.amount === "fixed")) ? index : -1;
}
function containsEffect(value: unknown, effect: string): boolean {
  if (Array.isArray(value)) return value.some(entry => containsEffect(entry, effect));
  return isRecord(value) && (value.effect === effect || Object.values(value).some(entry => containsEffect(entry, effect)));
}

/** Printed keyword parameters that are paid or chosen as the keyword's cost. */
const KEYWORD_PAYMENT_FIELDS = new Set(["cost", "additionalCosts", "materials", "costModifications", "restriction", "variableConstraints"]);
const PROTECTION_FROM_EVERYTHING = { keyword: "protection", from: [{ scope: "everything" }] } satisfies KeywordInstance;
// Match the complete copy clause, independently of any other printed abilities.
// Different model filters, copy exceptions or entry modifiers need their own analysis.
const OPTIONAL_SAME_TURN_ENTRY_COPY = {
  static: "entry-copy", what: { ref: "this", noun: "creature" },
  model: { model: "choose", who: { player: "you" }, count: 1, when: "before-entry",
    filter: { cls: "permanent", zoneChangeHistory: { event: "enters", to: { zone: "battlefield" }, during: "this-turn" } } },
  optional: { who: { player: "you" } },
} as const satisfies StaticEffect;
function isSameTurnEntryCopy(ability: Ability): ability is { kind: "static"; effect: typeof OPTIONAL_SAME_TURN_ENTRY_COPY } {
  return ability.kind === "static" && ability.abilityWord === undefined && ability.effect.static === "entry-copy" &&
    isDeepStrictEqual(ability.effect, OPTIONAL_SAME_TURN_ENTRY_COPY);
}
const SELF_GRAVEYARD_REPLACEMENT = {
  static: "replacement",
  replacement: {
    event: { event: "move-zone", what: { ref: "self" }, to: { zone: "graveyard", owner: "any" }, from: "anywhere" },
    occurrence: "each",
    replace: { replacement: "effects", effects: [{ steps: [
      { effect: "reveal", who: { player: "you" }, what: { ref: "replacement-event-object", role: "affected" } },
      { effect: "shuffle-into-library", who: { player: "you" }, what: [{ ref: "replacement-event-object", role: "affected" }],
        library: { zone: "library", owner: "its-owner" } },
    ] }] },
  },
} as const satisfies StaticEffect;
function isSelfGraveyardReplacement(ability: Ability, cardAbilities: Ability[] | undefined): boolean {
  if (ability.kind !== "static" || cardAbilities?.length !== 2 || cardAbilities[1] !== ability) return false;
  const { abilityWord, ...paragraph } = ability;
  return abilityWord === undefined && isDeepStrictEqual(paragraph, { kind: "static", effect: SELF_GRAVEYARD_REPLACEMENT }) &&
    isDeepStrictEqual(cardAbilities[0], { kind: "keywords", keywords: [PROTECTION_FROM_EVERYTHING] });
}
function child(path: string, part: string | number): string {
  return `${path}/${String(part).replace(/~/g, "~0").replace(/\//g, "~1")}`;
}
function cardCount(amount: Amount | undefined): BindingCardinality {
  return amount?.amount === "fixed" ? amount.value === 1 ? "one" : "many" : "unknown";
}
function objectCardinality(ref: ObjectRef | CascadeExiledCards): BindingCardinality {
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
function combine(a: Candidate[], b: Candidate[]): Candidate[] {
  const distinct = new Map<string, Candidate>();
  for (const candidate of [...a, ...b]) if (!distinct.has(candidate.id)) distinct.set(candidate.id, candidate);
  return [...distinct.values()];
}
function unambiguousPluralRecipient(context: Context): Candidate | undefined {
  const candidates = combine(context.objects, context.players);
  const [recipient] = candidates;
  if (!context.barrier && candidates.length === 1 && recipient.domain === "object" &&
      recipient.cardinality === "many" && !recipient.qualifiedOnly) return recipient;
}
function description(ref: ObjectRef | CascadeExiledCards): ObjectFilter | undefined {
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
function domain(expression: ReferenceExpression): Candidate["domain"] {
  if ("player" in expression || "implicit" in expression) return "player";
  if ("choice" in expression) return "number"; // Values never become participant antecedents.
  if ("recipient" in expression) return "object"; // Only a proven cast-spell antecedent binds this form.
  if (!("ref" in expression)) return "number";
  return expression.ref === "any-target" || ("filter" in expression && (expression.filter.orPlayer || expression.filter.orOpponent)) ? "object-or-player" : "object";
}
function referenceCardinality(expression: ReferenceExpression): BindingCardinality {
  return "ref" in expression ? expression.ref === "affected-object" ? "one" : expression.ref === "those-counters" ? "many" : objectCardinality(expression)
    : "implicit" in expression || "recipient" in expression || ("player" in expression && isPluralPlayerRef(expression)) ? "many" : "one";
}
/** This is an antecedent-description check, not a current-characteristics
 * predicate. A later "that" never chooses a new object by its current type. */
const PERMANENT_TYPES = new Set(["artifact", "battle", "creature", "enchantment", "land", "planeswalker"]);
/** Every object the filter describes is a permanent (CR 110.4). */
function impliesPermanent(filter: ObjectFilter): boolean {
  const any = (filter as { anyOf?: ObjectFilter[] }).anyOf, all = (filter as { allOf?: ObjectFilter[] }).allOf;
  return filter.cls === "permanent" || filter.cls === "token"
    || (!filter.cls && !filter.zone && Array.isArray(filter.types) && filter.types.length > 0 && filter.types.every(type => PERMANENT_TYPES.has(type)))
    || (Array.isArray(any) && any.length > 0 && any.every(impliesPermanent)) || (Array.isArray(all) && all.some(impliesPermanent));
}
function describes(actual: ObjectFilter | undefined, required: ObjectFilter): boolean {
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
const FACE_LAYOUTS = new Set(["transform", "modal_dfc", "adventure", "split", "flip", "reversible_card", "double_faced_token"]);
class Analyzer {
  readonly result: CardBindingAnalysis;
  private readonly prefix: string;
  private readonly targets = new Map<string, { id: string; path: string; signature: string }>();
  private readonly invalidNodes = new Map<string, BindingDiagnosticCode>();
  /** Objects an activation cost selected ("Sacrifice a creature: …"). */
  readonly costObjects = new Set<string>();
  /** Event results that are the source itself ("When this creature enters"). */
  readonly sourceAliases = new Set<string>();
  /** The preceding paragraph of an instant or sorcery: its paragraphs are
   * followed in order as one spell (CR 113.3a, 608.2c). */
  spellParagraph?: { face: string; context: Context };
  /** Pronouns whose antecedent is fixed by the role they fill (a
   * self-replacement's participant), by path. */
  readonly roleAntecedents = new Map<string, Candidate>();
  /** Quantities of instructions whose amount is itself "that many". */
  readonly echoQuantities = new Set<string>();
  /** Players named only through an object's owner or controller. */
  readonly weakPlayers = new Set<string>();
  /** The antecedents just before each optional instruction, by path. */
  readonly beforeDecision = new Map<string, Candidate[]>();
  /** Each face's card types, as the source's established description. */
  readonly faceTypes = new Map<string, ObjectFilter>();
  /** The latest choice from a pool in each ability, for "the rest". */
  readonly poolSelections = new Map<string, { pool: Candidate; selected: Candidate; path: string }>();
  /** Kicker abilities printed on each face, with their costs (CR 702.33e). */
  readonly kickers = new Map<string, { id: string; you: string; costs: string[] }[]>();
  /** Faces with an X in the mana cost or an additional cost (CR 107.3a). */
  readonly xFaces = new Set<string>();
  /** Faces whose type line is a permanent type (CR 110.4). */
  readonly permanentFaces = new Set<string>();

  constructor(identity: string) {
    this.prefix = `card:${encodeURIComponent(identity)}`;
    this.result = { artifactKind: "oracle-reference-binding", schemaVersion: 1, executable: false, identity,
      rules: { edition: "2026-09-25", url: RULES_URL }, nodes: [], references: [], diagnostics: [],
      counts: { obligations: 0, bound: 0, unresolved: 0 }, referencesBound: false, engineFeatures: [], fullyBound: false };
  }
  id(path: string, role: string): string { return `${this.prefix}${path}#${role}`; }
  node(node: BindingNode): string { this.result.nodes.push(node); return node.id; }
  diagnostic(code: BindingDiagnosticCode, path: string, message: string, rules: string[]): void {
    if (!this.result.diagnostics.some(d => d.code === code && d.path === path))
      this.result.diagnostics.push({ code, path, message, rules });
  }
  /** The construct at `path` needs a rules-engine capability, not a binding;
   * its references are bound as anywhere else. Inside an unsupported scope
   * the scope's diagnostic already prevents certification. */
  engineFeature(feature: EngineFeature, path: string, message: string, rules: string[]): void {
    if (!this.result.engineFeatures.some(need => need.feature === feature && need.path === path))
      this.result.engineFeatures.push({ feature, path, message, rules });
  }
  unresolved(expression: ReferenceExpression, path: string, code: BindingDiagnosticCode,
    message: string, rules: string[], candidates: string[] = []): void {
    this.result.references.push({ path, expression, binding: "unresolved", code, candidates, rules });
    this.diagnostic(code, path, message, rules);
  }
  bound(expression: ReferenceExpression, path: string, target: string, rules: string[]): Candidate {
    this.result.references.push({ path, expression, binding: "bound", target, rules });
    return { id: target, cardinality: referenceCardinality(expression), domain: domain(expression),
      ...("ref" in expression && expression.ref !== "affected-object" && expression.ref !== "those-counters" ? { description: description(expression) } : {}) };
  }
  antecedent(expression: ReferenceExpression, path: string, candidates: Candidate[], desired: BindingCardinality,
    rules: string[]): Candidate | undefined {
    if (candidates.length !== 1) {
      this.unresolved(expression, path, candidates.length ? "ambiguous-antecedent" : "missing-antecedent",
        candidates.length ? "More than one eligible antecedent remains; no proximity heuristic is applied."
          : "No antecedent is established in this scope.", rules, candidates.map(candidate => candidate.id));
      return undefined;
    }
    const candidate = candidates[0];
    if (candidate.domain !== domain(expression)) {
      this.unresolved(expression, path, "unsupported-reference", "An object/player-union selection does not establish the kind of this pronoun.", rules, [candidate.id]);
      return undefined;
    }
    // A group of unknown size ("up to X cards") is what a plural reference
    // ("those cards", "them") names; a singular one needs exactly one.
    if (desired !== "unknown" && candidate.cardinality !== desired && !(desired === "many" && candidate.cardinality === "unknown")) {
      this.unresolved(expression, path, "cardinality-mismatch", "The antecedent's cardinality does not establish this reference.", rules, [candidate.id]);
      return undefined;
    }
    if ("ref" in expression && expression.ref === "that" && !describes(candidate.description, expression.filter)) {
      this.unresolved(expression, path, "unsupported-reference", "The antecedent's established description does not resolve this qualified reference.", rules, [candidate.id]);
      return undefined;
    }
    this.bound(expression, path, candidate.id, rules);
    return candidate;
  }
  chosenValue(expression: ChoiceReference, path: string, context: Context): void {
    const local = context.availableChoice;
    const origin = local ?? (context.sourceMoved ? undefined : context.entryChoice);
    const category = origin?.category;
    const matches = expression.choice === "chosen-value"
      ? expression.which === "chosen" && expression.category === category
      : expression.kind === undefined && category !== undefined &&
        ["card-type", "creature-type", "basic-land-type", "land-type"].includes(category);
    // CR 601.2c: a resolution choice is unavailable when targets are announced.
    // CR 607.5a: an entry choice is undefined if that linked choice never ran.
    if (origin && matches && !context.barrier && !(local && (context.announcementOperand || context.phase !== "resolution"))) {
      const rules = local ? ["607.1c", "607.2d", "608.2c"] : [...ENTRY_CHOICE_RULES, "113.7a", "608.2h"];
      this.bound(expression, path, this.id(origin.path, "value-choice"), rules);
    } else {
      this.unresolved(expression, path, "unsupported-reference", expression.choice === "chosen-type"
        ? "This type needs a linked choice definition; its kind cannot be inferred from the filtered object's type."
        : "This symbolic value requires the linked originating choice and its scope; a nearby choice is not proof of that association.",
        expression.choice === "chosen-type" ? ["205.3e", "607.2d"] : ["607.2d"]);
    }
  }
  walkChildren(value: Record<string, unknown>, path: string, context: Context): void {
    for (const [key, entry] of Object.entries(value)) this.walk(entry, child(path, key), context);
  }
  walk(value: unknown, path: string, context: Context): void {
    if (Array.isArray(value)) { value.forEach((entry, index) => this.walk(entry, child(path, index), context)); return; }
    if (!isRecord(value)) return;
    if (value.recipient === "they") {
      const recipient = unambiguousPluralRecipient(context);
      if (recipient) {
        this.bound({ recipient: "they" }, path, recipient.id, ["608.2c", "611.2c"]);
      } else this.unresolved({ recipient: "they" }, path, "unsupported-reference",
        "The ability recipient's pronoun needs an antecedent and a player/object domain; parser ordering cannot establish either.", ["109.1", "113.1"]);
      return;
    }
    if (value.ref === "affected-object") {
      if (context.affectedMember && !context.barrier) {
        this.bound({ ref: "affected-object" }, path, context.affectedMember.id, ["611.3a", "613.6"]);
        return;
      }
      this.unresolved({ ref: "affected-object" }, path, "unsupported-reference", "This exclusion requires a bound per-affected-object scope.", ["611.3"]);
      return;
    }
    if (value.ref === "those-counters") {
      // Counters are not objects (CR 122.1); the counters an earlier clause named need that clause's binding.
      this.unresolved({ ref: "those-counters" }, path, "missing-antecedent", "These counters need the earlier clause of the same ability that named them.", ["122.1", "608.2c"]);
      return;
    }
    // "A copy of that creature", "protection from each of that creature's
    // colors": its copiable values and colors as it last existed (CR 608.2h, 707.2).
    if (typeof value.ref === "string" && !context.lastKnown && (path.endsWith("/copyOf") || /\/from\/\d+\/what$/.test(path))) {
      context.lastKnown = true;
      try { this.object(value as ObjectRef, path, context); } finally { context.lastKnown = false; }
      return;
    }
    if (typeof value.ref === "string") { this.object(value as ObjectRef, path, context); return; }
    if (!context.lastKnown && ((isRecord(value.of) && (value.player === "controller" || value.player === "owner" || value.amount === "attribute"))
        || (value.amount === "counter-count" && isRecord(value.on))
        || (value.amount === "attribute-sum" && isRecord(value.among) && value.among.ref === "that" && value.among.plural === true)
        // "Put its counters on target creature": the counters it had (CR 608.2h).
        || (path.endsWith("/counters") && isRecord(value.of))
        // "If it was attacking", "if it had a counter on it": as it last existed.
        || (typeof value.condition === "string" && value.time === "previous"))) {
      context.lastKnown = true;
      try { this.walk(value, path, context); } finally { context.lastKnown = false; }
      return;
    }
    if (typeof value.player === "string") { this.player(value as PlayerRef, path, context); return; }
    if (value.to === "any-target") {
      this.object({ ref: "any-target", ...(typeof value.selectionId === "string" ? { selectionId: value.selectionId } : {}) }, path, context);
      return;
    }
    if (isKeywordInstance(value)) { this.otherObjectKeyword(value, path, context); return; }
    if (value.amount === "x" && (typeof value.keyword === "string" || typeof value.sign === "number")) {
      this.walk({ amount: "x" }, child(path, "amount"), context);
    } else if (typeof value.amount === "string" && SYMBOLIC_AMOUNTS.has(value.amount)) {
      const expression = value as Extract<Amount, { amount: "x" | "that-number" | "that-much" | "replacement-event-quantity" }>;
      if (context.barrier) this.unresolved(expression, path, "unsupported-scope", context.barrier, ["107.3"]);
      else if (expression.amount === "x" && context.x) this.bound(expression, path, context.x, ["107.3c"]);
      else if (expression.amount === "x" && context.triggerX) this.bound(expression, path, context.triggerX, ["701.37c"]);
      else if (expression.amount === "x" && context.announcedX) this.bound(expression, path, context.announcedX, ["107.3a"]);
      else if (expression.amount === "that-much") {
        // "Discard all the cards in your hand, then draw that many cards"; "You
        // may pay that much life. If you do, draw that many cards": the
        // quantity of the instruction this one follows in its sentence, or of
        // the decision "if you do" refers to.
        // An instruction whose own amount is "that many" repeats the number it
        // refers to ("you draw that many cards and lose that much life"); it
        // is not a new antecedent.
        let candidates = context.quantities.some(candidate => !this.echoQuantities.has(candidate.id))
          ? context.quantities.filter(candidate => !this.echoQuantities.has(candidate.id)) : context.quantities;
        const instruction = (candidate: Candidate) => candidate.id.slice(this.prefix.length).replace(/#[^#]*$/, "");
        const sentence = /^(.*\/effects\/\d+)\//.exec(path)?.[1];
        if (candidates.length > 1) {
          const linked = context.linkedPath ? candidates.filter(candidate => instruction(candidate).startsWith(context.linkedPath!)) : [];
          const same = sentence ? candidates.filter(candidate => instruction(candidate).startsWith(`${sentence}/`) && path.startsWith(`${sentence}/`)) : [];
          if (linked.length === 1) candidates = linked;
          else if (same.length) candidates = [same[same.length - 1]];
        }
        this.antecedent(expression, path, candidates, "unknown", ["608.2h", "608.2c"]);
      }
      else this.unresolved(expression, path, "unsupported-number-binding",
        "This number needs a rule-specific definition, choice, or capture; no default value is inferred.", ["107.3a", "107.3f", "107.3j"]);
      return;
    }
    if (typeof value.gained === "string") { this.gained(value as unknown as GainedAbility, path, context); return; }
    if (value.trigger === "phase-in-turn" || value.trigger === "phase-instance") {
      const barrier = "This delayed header needs its exact turn or phase occurrence linked, including future schedule changes and skipped occurrences.";
      this.diagnostic("unsupported-scope", path, barrier, ["500.7", "500.8", "603.7b", "614.10a"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.amount === "combat-history-count") {
      const barrier = "This cardinality requires a historical set of distinct declared attacker incarnations, including departed objects.";
      this.diagnostic("unsupported-scope", path, barrier, ["400.7", "508.4"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.amount === "cast-mana-value-sum" && (!context.spellAbility || context.sourceMoved)) {
      const barrier = "The excluded cast must be the resolving spell's own cast occurrence.";
      this.diagnostic("unsupported-scope", path, barrier, ["202.3e", "601.2i", "608.2i"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "as-cascade") {
      this.cascade(value as Extract<StaticEffect, { static: "as-cascade" }>, path, context);
      return;
    }
    if (value.static === "skip-time" && !context.barrier && context.phase === "continuous" &&
        isDeepStrictEqual(value.who, { player: "you" }) && isDeepStrictEqual(value.what, { kind: "step", step: "draw" })) {
      const rules = ["504.1", "611.3", "614.10", "614.10a"];
      this.player({ player: "you" }, child(path, "who"), context);
      this.node({ id: this.id(path, "skip-draw-step"), path, node: "skip-draw-step", scope: context.ability, who: context.you, rules });
      this.engineFeature("phase-scheduling", path, "Replace the controller's draw step each time it would begin while the static ability applies.", rules);
      return;
    }
    if (value.static === "skip-time" || value.effect === "skip-time") {
      const barrier = "Skipping requires exact upcoming occurrences, replacement consumption, and future-turn scope bindings.";
      this.diagnostic("unsupported-scope", path, barrier, ["500.11", "614.10"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "level-band") {
      const barrier = "A level symbol needs the source's level-counter range, battlefield evaluation, and granted-ability scopes bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["711.2", "711.4", "711.6"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "station-band") {
      const barrier = "A station symbol needs the source's charge-counter threshold, battlefield evaluation, printed power/toughness box, and granted-ability scopes bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["721.2", "721.2b", "721.4"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.effect === "scan-library") {
      const barrier = "A sequential library scan needs its actor, complete processed pool, matching subset, and per-card iteration scope bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["609.3", "701.13", "701.20"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // Shields watching future damage events: the engine applies them; their
    // printed references bind as anywhere else.
    if (value.static === "redirect-damage" || value.effect === "redirect-damage")
      this.engineFeature("damage-prevention", path, "Redirection needs the engine's future damage events, source-choice eligibility, budget consumption, and the new recipient's legality.", ["609.7", "614.9", "615.7", "615.8"]);
    if (value.static === "prevent-damage" || value.effect === "prevent-damage" || value.choice === "damage-source"
      || value.amount === "prevention-event-quantity" || value.condition === "damage-prevented")
      this.engineFeature("damage-prevention", path, "Prevention needs the engine's future damage events, source-choice eligibility, shield consumption, and prevented quantities.", ["609.7", "615.5", "615.7", "615.8", "615.10", "615.12"]);
    if (value.amount === "prevention-event-quantity" && value.quantity === "prevented" && !context.barrier &&
        context.exactEventQuantity?.path === path) {
      this.bound({ amount: "prevention-event-quantity", quantity: "prevented" }, path, context.exactEventQuantity.id, ["615.5", "615.12"]);
      return;
    }
    if (value.static === "prevent-damage" &&
        this.preventionOutcome((value as Extract<StaticEffect, { static: "prevent-damage" }>).prevention, child(path, "prevention"), context))
      return;
    if ((value.static === "prohibit" || value.static === "untap-during-step" || value.static === "untap-step-limit") &&
        this.staticRule(value as unknown as StaticRule, path, context)) return;
    if (value.static === "look-permission" && this.privateLibraryLook(value.permission as LookPermission, path, context)) return;
    if (value.static === "look-permission" || value.effect === "grant-look-permission"
      || value.static === "commander-eligibility" || value.static === "optional-untap"
      || value.static === "untap-during-step" || value.static === "untap-step-limit") {
      const barrier = "Persistent information queries, pregame commander eligibility, and turn-based untap choices require their own rule-specific binding scopes.";
      this.diagnostic("unsupported-scope", path, barrier, ["401.5", "502.3", "708.5", "903.3a", "113.6n"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.condition === "characteristic-relation" && !context.barrier) {
      const condition = value as Extract<Condition, { condition: "characteristic-relation" }>;
      const at = child(path, "what");
      this.walk(condition.what, at, { ...context, lastKnown: context.phase === "resolution" && !context.announcementOperand });
      const subject = this.at(at);
      if (subject?.domain === "object" && subject.cardinality === "one") {
        this.characteristicPredicate(condition.test, child(path, "test"), subject.id, context);
        return;
      }
      this.diagnostic("unsupported-scope", path, "The characteristic comparison requires one bound subject.", ["608.2h"]);
      this.walk(condition.test, child(path, "test"), { ...context, barrier: "The comparison has no bound subject." });
      return;
    }
    if (Array.isArray(value.characteristicRelations) && context.characteristicSubject && !context.barrier) {
      const { characteristicRelations, ...operand } = value;
      this.walk(operand, path, { ...context, characteristicSubject: undefined });
      for (const [index, relation] of characteristicRelations.entries())
        this.characteristicPredicate(relation as CharacteristicRelation, `${path}/characteristicRelations/${index}`,
          context.characteristicSubject, context);
      return;
    }
    if (value.characteristicRelations || value.condition === "characteristic-relation" || value.constraint === "shared-value") {
      const barrier = "Characteristic relations need subject, operand, quantified-set and common-value scopes bound before comparison.";
      this.diagnostic("unsupported-scope", path, barrier, ["201.2a", "201.2b", "201.2c", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "entry-copy") {
      const barrier = "Entry-copy needs separate entrant, model choice, copied characteristic, and entry-exception scopes before entry.";
      this.diagnostic("unsupported-scope", path, barrier, ["614.12", "707.2", "707.5", "707.9"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // A predefined token's abilities are fixed by CR 111.10 and name nothing
    // outside the token; entering attacking needs the token's own choices.
    if (value.attacking === true)
      this.diagnostic("unsupported-scope", path, "Entering-attacking choices require scopes belonging to the new token.", ["508.4"]);
    if (value.amount === "chosen")
      this.diagnostic("unsupported-number-binding", path, "This number needs a phase-specific choice identity and constraint scope.", ["107.3", "601.2b"]);
    if (value.amount === "dice-result" || value.condition === "dice-result")
      this.diagnostic("unsupported-scope", path, "A die result needs its exact roll occurrence, natural/final stage, retained-results set, and table/trigger scope bound.", ["706.2", "706.3", "706.6", "706.7"]);
    if (value.amount === "kicker-count")
      this.diagnostic("unsupported-number-binding", path, "This count requires the linked originating spell's recorded kicker payments.", ["400.7d", "702.33"]);
    if (value.amount === "life-change")
      this.engineFeature("event-history", path, "How much life a player gained or lost this turn needs the engine's life-change history.", ["119.3", "608.2i"]);
    else if (value.amount === "event-quantity") {
      if (this.boundEventAggregate(value as Extract<Amount, { amount: "event-quantity" }>, path, context)) return;
      const barrier = "An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.";
      this.diagnostic("unsupported-scope", path, barrier, ["119.3", "120.3", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // "For each evidence vote": the tally of the one vote held earlier in this ability.
    if (value.amount === "vote-count" && !(context.votes?.length === 1 && !context.barrier))
      this.diagnostic("unsupported-reference", path, "This vote count needs the one vote it tallies held earlier in the ability.", ["701.38"]);
    if (value.amount === "devotion") {
      const { who, colors } = value as Extract<Amount, { amount: "devotion" }>;
      const playerPath = child(path, "who");
      const colorPath = child(path, "colors");
      this.walk(who, playerPath, { ...context });
      this.walk(colors, colorPath, context);
      const player = this.at(playerPath);
      const rules = ["700.5", "700.5a"];
      if (!["you", "owner", "controller", "that-player"].includes(who.player) || player?.cardinality !== "one" ||
          (!Array.isArray(colors) && !this.at(colorPath))) {
        this.diagnostic("unsupported-number-binding", path, "Devotion requires the player and its color operands to be bound.", rules);
        return;
      }
      this.node({ id: this.id(path, "devotion"), path, node: "devotion",
        scope: context.ability, who: player.id, colors, rules });
      this.engineFeature("devotion", path,
        "Compute devotion after copy, control, and text-changing effects, including effects that modify devotion.", rules);
      return;
    }
    if (value.amount === "mana-spent") {
      const what = child(path, "what");
      if (!context.barrier && context.phase === "resolution" && context.triggerObject?.event === "cast" &&
          value.quantity === "total" && value.source === undefined &&
          isDeepStrictEqual(value.what, { ref: "that", filter: { cls: "spell" } })) {
        const rules = ["601.2f", "601.2h", "601.2i", "603.2c", "608.2i"];
        const occurrence = context.triggerObject.candidate.id;
        this.bound({ ref: "that", filter: { cls: "spell" } }, what, occurrence, rules);
        this.node({ id: this.id(path, "cast-mana-paid"), path, node: "cast-mana-paid",
          scope: context.ability, occurrence, measure: "total", rules });
        this.engineFeature("event-history", path, "Read the actual mana spent in the one recorded cast, including additional costs.", rules);
        return;
      }
      const barrier = "Mana payment quantities need the exact originating spell's payment event, including additional costs and actual colors spent.";
      this.diagnostic("unsupported-scope", path, barrier, ["400.7d", "601.2f", "601.2h", "609.4b"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.condition === "controls-subset" &&
        this.commonNameSubset(value as Extract<Condition, { condition: "controls-subset" }>, path, context)) return;
    if (value.condition === "controls-subset") {
      const barrier = "A controlled-subset existence predicate requires candidate, cardinality and pairwise characteristic scopes; it does not select or capture objects.";
      this.diagnostic("unsupported-scope", path, barrier, ["201.2b", "603.4", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // Filtered-player counter predicates already use player()'s per-candidate
    // scope diagnostic; this covers standalone quantified conditions.
    if (value.condition === "player-counters") {
      if (!context.barrier && context.phase === "resolution" && isRecord(value.who) &&
          (value.who.player === "target-player" || value.who.player === "target-opponent") && !value.who.count) {
        this.walkChildren(value, path, context);
        return;
      }
      const barrier = "A counter predicate needs its quantified player's candidate scope; counters must not be summed across opponents.";
      this.diagnostic("unsupported-scope", path, barrier, ["122.1", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.choice === "chosen-type" || value.choice === "chosen-value") {
      this.chosenValue(value as ChoiceReference, path, context);
      return;
    }
    if ((value.condition === "you-do" || value.condition === "you-dont") && !context.linkedDecision) {
      this.diagnostic("unsupported-reference", path, "Payment/performance conditions need an explicit preceding-action link.", ["118.12"]);
    }
    if (value.condition === "searched-zone")
      this.diagnostic("unsupported-reference", path, "This condition needs the specific search event, including a search that found no cards.", ["701.23b", "701.23d"]);
    if (value.condition === "put-action-history") {
      const barrier = "The named movement outcome needs its exact preceding instruction, actor, historical object incarnation, and actual destination; attempting an action or choosing to pay is insufficient.";
      this.diagnostic("unsupported-scope", path, barrier, ["118.12", "608.2c", "608.2i", "609.3"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.effect === "search" && isRecord(value.zones) &&
        (value.zones.selection !== "all" || Array.isArray(value.zones.of) && value.zones.of.length > 1))
      this.diagnostic("unsupported-scope", path, "Zone choices and found-card selection across multiple searched zones need one search-event scope.", ["701.23"]);
    if (value.condition === "pays") {
      // CR 118.12a: "unless [player] pays": that player pays, not the
      // ability's controller.
      this.walk(value.who, child(path, "who"), context);
      const payer = this.at(child(path, "who"));
      const payment: Context = payer && payer.domain === "player" && payer.cardinality === "one"
        ? { ...context, you: payer.id, phase: "payment" }
        : { ...context, barrier: "The optional payment event must bind its payer separately from the resolving ability's controller." };
      if (payment.barrier) this.diagnostic("unsupported-scope", path, payment.barrier, ["118.12a"]);
      // "Unless its controller pays {1} for each creature you control": the
      // count is in the ability's own voice, so its "you" is the ability's
      // controller (CR 109.5); only the payment is the payer's.
      for (const [key, entry] of Object.entries(value)) if (key !== "who" && key !== "condition")
        this.walk(entry, child(path, key), key === "multiplier" && !payment.barrier ? { ...context, phase: "payment" } : payment);
      // "Counter target spell unless its controller pays {1}. That player discards a card."
      if (payer && payer.domain === "player" && payer.cardinality === "one" && payer.id !== context.you) context.players = combine(context.players, [payer]);
      return;
    }
    // Questions about the game's history ("if you attacked this turn", "if
    // this spell was cast from a graveyard", "creatures that attacked this
    // turn"): the engine keeps that record. The objects and players they name
    // bind as anywhere else; a permanent's ability can ask about the spell it
    // was (CR 400.7d).
    // What happened is asked of the object as it was (CR 608.2h): "each
    // creature it blocked this combat" after it died.
    const history = value.combatHistory || value.damageHistory || ["attacked", "combat-action-history", "was-cast", "zone-change-history",
      "damage-history", "mana-spent-to-cast", "entered-from", "player-action-history", "was-attacked"].includes(String(value.condition));
    if (history && !context.lastKnown) {
      context.lastKnown = true;
      try { this.walk(value, path, context); } finally { context.lastKnown = false; }
      return;
    }
    if (value.combatHistory)
      this.engineFeature("event-history", child(path, "combatHistory"), "Combat history needs the engine's record of each candidate's declared attacks in the stated window.", ["508.4", "400.7", "608.2h"]);
    if (value.damageHistory)
      this.engineFeature("event-history", child(path, "damageHistory"), "Damage history needs the engine's record of damage events in the stated window.", ["120.3", "120.7", "400.7", "608.2h"]);
    if (["attacked", "combat-action-history", "was-cast", "zone-change-history", "damage-history", "mana-spent-to-cast", "entered-from",
      "player-action-history", "life-change", "was-attacked", "ability-resolution-ordinal", "ability-activation-count",
      "alternative-cost-paid", "additional-cost-paid", "keyword-choice"].includes(String(value.condition)))
      this.engineFeature("event-history", path, "This condition asks about earlier events or how this object was cast; the engine keeps that record.", ["603.4", "607.2", "608.2i", "400.7d"]);
    // Conditions about an earlier instruction of this ability ("if you won
    // the clash", "if [a player] did this way") need that instruction.
    if (["vote-result", "clash-result", "preceding-action"].includes(String(value.condition))) {
      const barrier = "This condition queries earlier events or a linked decision; the exact events, period, actor and source incarnation have not been bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["603.4", "607.2", "608.2i"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // State of the game or of players as a whole. Their player operands are
    // still walked, but the quantified comparison itself has no binding rule.
    const query = value.condition;
    if (!context.barrier && isRecord(value.who) && value.who.player === "you" &&
        (query === "designation" || query === "completed-dungeon" || query === "opponent-count" || query === "turn-of")) {
      const scoped = { ...context };
      this.walkChildren(value, path, scoped);
      const rules = query === "designation"
        ? [DESIGNATION_RULES[value.designation as PlayerDesignation], "109.5"]
        : query === "completed-dungeon" ? ["309.7", "608.2i"]
        : query === "opponent-count" ? ["102.2", "102.3", "800.4"] : ["102.1", "800.4j"];
      this.node({ id: this.id(path, "player-state"), path, node: "player-state-condition",
        scope: context.ability, who: context.you, query, rules });
      if (query === "completed-dungeon")
        this.engineFeature("event-history", path, "Whether the specified player has completed a dungeon this game.", rules);
      return;
    }
    if (["designation", "day-night", "completed-dungeon", "full-party", "turn-of", "player-comparison", "opponent-count"].includes(String(value.condition)))
      this.diagnostic("unsupported-scope", path, "Game and player designations, turn ownership and per-player comparisons need the participating player set and its quantifier bound.", ["102.1", "102.2", "102.3", "608.2h"]);
    if (value.static === "combat-rule" && this.attackPayment(value as unknown as Extract<StaticEffect, { static: "combat-rule" }>, path, context)) return;
    if (value.rule === "declaration-cost") {
      const barrier = "A cost to attack or block needs the declaring player, each affected creature's controller, and the per-declaration cost unit bound; its payer is not the ability's controller by default.";
      this.diagnostic("unsupported-scope", path, barrier, ["508.1h", "509.1d"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.trigger === "player-attacks") {
      const barrier = "Declared-attacker groups, attacked players, and per-attacker trigger participants have not been bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["508.3c", "508.3d", "508.3e"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.conditionScope === "each-affected-object") {
      const barrier = "A continuously evaluated condition needs a distinct affected-object binding for each match.";
      this.diagnostic("unsupported-scope", path, barrier, ["611.3"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.valueScope === "each-affected-object") {
      if (this.dynamicStaticValue(value, path, context)) return;
      const barrier = "A dynamic characteristic value needs a distinct affected-object binding for each match.";
      this.diagnostic("unsupported-scope", path, barrier, ["611.3", "613.6"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "prohibit" || value.effect === "prohibit" || value.static === "cast-limit" || value.effect === "cast-limit") {
      const barrier = "Dynamically prohibited actors, source-zone ownership, ability sources, and per-player cast-history totals need action-scope binding.";
      this.diagnostic("unsupported-scope", path, barrier, ["601.3", "602.1c", "611.2c", "615.12"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if ((value.action === "cast" || value.action === "play") && isRecord(value.who) && isRecord(value.what)
        && (isRecord(value.alternativeCost) || Array.isArray(value.additionalCosts))) {
      // The player who casts the spell pays its costs (CR 601.2f-h), whoever
      // controls the effect that lets them.
      this.walk(value.who, child(path, "who"), context);
      const caster = this.at(child(path, "who"));
      const payment: Context = caster?.domain === "player" && caster.cardinality === "one" ? { ...context, you: caster.id, phase: "payment" }
        : { ...context, barrier: "The casting player who pays these costs is not one bound player." };
      if (payment.barrier) this.diagnostic("unsupported-scope", path, payment.barrier, ["601.2f"]);
      for (const [key, entry] of Object.entries(value)) {
        if (key === "who") continue;
        this.walk(entry, child(path, key), key === "alternativeCost" || key === "additionalCosts" ? payment : context);
      }
      return;
    }
    if (value.static === "conditional" && isRecord(value.condition)) {
      // "As long as enchanted creature is black, it gets +1/+1": the
      // condition's subject is the effect's antecedent.
      const scoped = { ...context };
      const pronoun = (ref: unknown) => isRecord(ref) && (ref.ref === "it" || ref.ref === "them");
      // "Enchanted creature gets +1/+2 as long as it's white. Otherwise, it
      // gets -2/-1": the condition follows the subject it refers to. In a cost
      // change, that subject is the spell or ability whose cost changes ("Each
      // creature spell you cast costs {1} less if it has mutate").
      const effect = isRecord(value.effect) ? value.effect : undefined;
      const scope = effect && effect.static === "cost-modification" && isRecord(effect.scope) ? effect.scope : undefined;
      const subjectPath = effect && isRecord(effect.what) && !pronoun(effect.what) ? child(child(path, "effect"), "what")
        : scope && isRecord(scope.spells) ? child(child(child(path, "effect"), "scope"), "spells")
        : scope && isRecord(scope.abilities) ? child(child(child(path, "effect"), "scope"), "abilities") : undefined;
      if (pronoun(value.condition.what) && subjectPath) {
        this.walk(value.effect, child(path, "effect"), scoped);
        const subject = this.at(subjectPath);
        scoped.mentionedObject = true;
        if (subject?.domain === "object") scoped.objects = combine(scoped.objects, [subject]);
        for (const [key, entry] of Object.entries(value)) if (key !== "effect") this.walk(entry, child(path, key), scoped);
        return;
      }
      this.namingCondition(value.condition, child(path, "condition"), scoped);
      for (const [key, entry] of Object.entries(value)) if (key !== "condition") this.walk(entry, child(path, key), scoped);
      return;
    }
    if (["entry-choice", "enters-with-counters", "enters-tapped", "as-enters"].includes(String(value.static))) { this.entering(value, path, context); return; }
    if (isRecord(value.entry) && (value.entry.attachedTo !== undefined || value.entry.attacking !== undefined)) {
      const barrier = "The entering permanent's pre-entry controller, payment, attachment, and attack-choice scopes have not been bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["614.12a", "303.4f", "508.4"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.condition === "cast-count" && context.spellAbility && !context.sourceMoved && !context.barrier &&
        isRecord(value.who) && value.who.player === "you" && isRecord(value.what) && value.what.cls === "spell" &&
        value.what.other === true && value.during === "this-game" && isRecord(value.what.name) &&
        value.what.name.name === "literal") {
      const filter = child(path, "what");
      this.walkChildren(value, path, { ...context, castHistoryExclusionPath: filter });
      const rules = ["601.2i", "608.2i", "707.10"];
      this.node({ id: this.id(path, "cast-history-exclusion"), path, node: "cast-history-exclusion",
        scope: context.ability, caster: context.you, occurrence: context.source, rules });
      this.engineFeature("event-history", path, "Count matching cast occurrences other than this resolving spell's own cast.", rules);
      return;
    }
    if (value.other === true && !value.otherThan) {
      // "Other creatures you control", "another creature enters": in a
      // permanent's ability, before any other object is named, the excluded
      // object is the permanent itself. Elsewhere (a spell, or after a
      // trigger or instruction names an object) it may be that object.
      if (!context.barrier && context.castHistoryExclusionPath === path) {
        this.bound({ ref: "this", noun: "spell" }, child(path, "other"), context.source, ["601.2i", "608.2i"]);
      } else if (!context.barrier && context.comparisonExclusion && path.startsWith(context.comparisonExclusion.path) &&
          /^(?:\/anyOf\/\d+)*$/.test(path.slice(context.comparisonExclusion.path.length))) {
        this.bound({ ref: "affected-object" }, child(path, "other"), context.comparisonExclusion.subject, ["201.2a", "608.2b"]);
      } else if (!context.barrier && context.copyModelExclusion?.path === path) {
        this.bound(context.copyModelExclusion.reference, child(path, "other"), context.copyModelExclusion.model.id, ["611.2c", "707.2"]);
      } else if (context.permanentSource && !context.barrier && !context.sourceMoved &&
          (path === context.sourceExclusionPath || (!context.mentionedObject && this.onlySource(context))))
        this.bound({ ref: "self" }, child(path, "other"), context.source,
          path !== context.sourceExclusionPath ? ["201.5"]
            : context.phase === "replacement-match" ? ["201.5", "615.1a"] : ["201.5", "602.2b", "701.21a"]);
      else this.diagnostic("unsupported-reference", child(path, "other"), "The excluded antecedent of other/another is unresolved.", ["201.5"]);
    }
    // CR 702.33e: "kicked" refers to the kicker ability printed on the object;
    // with one kicker ability (or the kicker cost it names) that link is fixed.
    const kickers = value.condition === "kicked" ? (this.kickers.get(context.face) ?? []) : [];
    const kicker = value.condition === "kicked" && !context.barrier
      ? (isRecord(value) && Array.isArray(value.kickerCost)
        ? kickers.filter(entry => entry.costs.includes(JSON.stringify(value.kickerCost)))
        : kickers.length === 1 ? kickers : []) : [];
    if (kicker.length === 1) {
      // "If it was kicked": the object with the kicker ability (CR 702.33e).
      if (isRecord(value.what) && value.what.ref === "it" && !context.sourceMoved)
        this.roleAntecedents.set(child(path, "what"), { id: context.source, cardinality: "one", domain: "object" });
      this.node({ id: this.id(path, "linked-ability"), path, node: "linked-ability", scope: context.ability, condition: "kicked", ability: kicker[0].id, rules: ["702.33e", "607.2"] });
      // The kicker cost it names was paid by the player who cast the spell.
      if (isRecord(value) && Array.isArray(value.kickerCost)) {
        for (const [key, entry] of Object.entries(value)) this.walk(entry, child(path, key), key === "kickerCost" ? { ...context, you: kicker[0].you, phase: "payment" } : context);
        return;
      }
    }
    else if (value.relation === "linked-exile" && this.linkedExileFilter(value, path, context)) return;
    else if (value.relation === "event-result" && value.scope === "this-way" && this.thisWay(value, path, context)) { /* linked */ }
    else if (value.relation === "event-result" || value.relation === "linked-exile" || value.condition === "kicked")
      this.diagnostic("unsupported-reference", path, "This reference requires a specific event/payment or linked-ability association.", ["607"]);
    if (value.static === "alternative-cost" || value.static === "play-permission") {
      // CR 109.5: "you" on an object is its would-be controller while a player
      // is casting it, or its owner where it has no controller. A card's own
      // alternative cost ("rather than pay this spell's mana cost") is the
      // caster's; its own permission ("You may cast this card from your
      // graveyard") its owner's. A permanent's permission for other cards
      // ("You may play lands from your graveyard") is its controller's.
      const subject = value.static === "alternative-cost" ? value.spells : isRecord(value.permission) ? value.permission.what : undefined;
      const own = isRecord(subject) && (subject.ref === "self" || subject.ref === "this");
      if (own) {
        const role = value.static === "alternative-cost" ? "prospective-caster" as const : "card-owner" as const;
        const player = this.node({ id: this.id(path, role), path, node: "runtime-player", scope: context.ability, role, rules: ["109.5"] });
        this.walkChildren(value, path, { ...context, you: player, ...(role === "prospective-caster" ? { phase: "payment" as const } : {}) });
        return;
      }
      if (context.permanentSource) { this.walkChildren(value, path, context); return; }
    }
    if (["alternative-cost", "replacement", "play-permission", "play-permission-choice"].includes(String(value.static))) {
      const barrier = "This pre-entry or alternative-payment scope needs a would-be controller/caster binding.";
      this.diagnostic("unsupported-scope", path, barrier, ["109.5", "614.12"]);
      // CR 201.5, 614.1: the watched move precedes the replacement. Its
      // explicit source is known even while replacement-event captures are not.
      const event = value.static === "replacement" && isRecord(value.replacement) ? value.replacement.event : undefined;
      const replacementSubjectPath = !context.barrier && isRecord(event) && event.event === "move-zone"
        ? child(child(child(path, "replacement"), "event"), "what") : undefined;
      this.walkChildren(value, path, { ...context, barrier, replacementSubjectPath });
      return;
    }
    if (["move-zone", "put-counters", "transform", "convert", "attach", "unattach", "create-token"].includes(String(value.effect)) && value.who == null)
      this.player({ player: "you" }, child(path, "who"), context);
    if (value.cost === "tap-self" || value.cost === "untap-self")
      this.object({ ref: "self" }, child(path, "source"), context);
    if (typeof value.cost === "string") {
      this.player({ player: "you" }, child(path, "payer"), context);
      if (value.cost === "choice" && this.simplePaymentAlternatives(value as Extract<Cost, { cost: "choice" }>, path, context)) return;
      if (value.cost === "blight" && !context.barrier && context.phase === "payment" && isRecord(value.amount) &&
          value.amount.amount === "fixed" && typeof value.amount.value === "number") {
        const rules = ["118.11", "601.2h", "602.2b", "701.68a", "701.68b"];
        this.node({ id: this.id(path, "blight-payment"), path, node: "blight-payment", scope: context.ability,
          payer: context.you, count: value.amount.value, rules });
        context.mentionedObject = true;
        this.engineFeature("intrinsic-payment", path, "The payer chooses a creature they control for the blight payment.", rules);
      } else if (["choice", "collect-evidence", "waterbend", "forage", "blight"].includes(value.cost))
        this.diagnostic("unsupported-scope", path, "This cost has intrinsic payment choices and object selections requiring a distinct payment scope.", ["118", "701"]);
      if (value.cost === "mill" || (value.cost === "discard" && value.what === "hand"))
        this.player({ player: "you" }, child(path, "zone-owner"), context);
    }
    if (value.restriction === "timing" && value.turn != null) {
      if (value.turn === "your" || value.turn === "activator") this.player({ player: "you" }, child(path, "turn"), context);
      else this.diagnostic("unsupported-reference", child(path, "turn"), "This turn-owner relation needs its own scoped binding.", ["109.5"]);
    }
    if (value.activators === "opponents-of-source-controller" || value.activators === "any-player") {
      if (!context.barrier && context.phase === "payment" && context.permanentSource) {
        const rules = ["109.5", "602.2"];
        this.node({ id: this.id(path, "activation-permission"), path, node: "activation-permission", scope: context.ability,
          source: context.source, actors: value.activators, rules });
        this.engineFeature("activation-permission", path, "Check the printed eligible player set before the actual activator pays the cost.", rules);
      } else this.diagnostic("unsupported-reference", path, "Activation permission requires a player set distinct from the ability's eventual activator.", ["602.2"]);
    }
    if (value.trigger === "you-attack") this.player({ player: "you" }, child(path, "who"), context);
    if (value.condition === "cast-count" && value.who === "all-players-combined")
      this.diagnostic("unsupported-reference", child(path, "who"), "This aggregate requires the complete participating-player set.", ["109.5"]);
    if (value.condition === "your-turn" || value.condition === "not-your-turn" ||
        ["your-next-turn", "end-of-your-next-turn", "your-next-end-step", "your-next-upkeep"].includes(String(value.duration)))
      this.player({ player: "you" }, child(path, "whose"), context);
    if (Array.isArray(value.symbols)) value.symbols.forEach((symbol, index) => {
      if (symbol === "X") this.walk({ amount: "x" }, child(child(path, "symbols"), index), context);
    });
    // Filters have implicit player references as well as explicit PlayerRef
    // nodes. Record them instead of silently treating these strings as data.
    for (const key of ["control", "own", "owner", "whose", "during"] as const) {
      const token = value[key];
      if (token === "you" || token === "your") this.player({ player: "you" }, child(path, key), context);
      else if (token === "that-player" || token === "their") this.player({ player: token === "their" ? "they" : "that-player" }, child(path, key), context);
      // "An opponent controls", "your opponents' graveyards": relative to you.
      else if (token === "opponent" || token === "each-opponent" || token === "an-opponent") this.relativePlayers("opponents", child(path, key), context);
      // "You don't control": every other player, who in a team game need not be an opponent.
      else if (token === "not-you") this.relativePlayers("other-players", child(path, key), context);
      // "Its owner's", "each player's", "a graveyard" describe each candidate
      // itself or every player; they name no player outside it.
      else if (token === "its-owner" || token === "owner" || token === "each-player" || token === "any") { /* no outside reference */ }
      else if (token === "active-player") {
        this.diagnostic("unsupported-reference", child(path, key), "This implicit player relation requires its own scoped binding rule.", ["109.5"]);
      }
    }
    if (value.effect === "become" && isRecord(value.spec) && Object.keys(value.spec).length === 1 &&
        isRecord(value.spec.copyOf) && isRecord(value.what) && value.what.ref === "each" &&
        isRecord(value.what.filter) && value.what.filter.other === true && !context.barrier) {
      const modelPath = `${path}/spec/copyOf`;
      this.walk(value.spec.copyOf, modelPath, context);
      const model = this.at(modelPath);
      if (model?.domain === "object" && model.cardinality === "one") {
        const scoped = { ...context, copyModelExclusion: { path: `${path}/what/filter`, model,
          reference: value.spec.copyOf as ObjectRef } };
        this.walk(value.what, child(path, "what"), scoped);
        this.walk(value.duration, child(path, "duration"), scoped);
        this.engineFeature("characteristic-layers", path, "Set the affected group at resolution, excluding the bound copy model, and apply its copiable values.", ["611.2c", "707.2"]);
        return;
      }
    }
    this.walkWithRecipient(value, path, context);
    if (["sacrifice", "discard", "exile", "return"].includes(String(value.cost)) &&
        this.at(child(path, "what"))?.id === context.source) context.sourceMoved = true;
    if (value.controller === "you") this.player({ player: "you" }, child(path, "controller"), context);
    else if ((value.controller === "owner" || value.to === "owner") && isRecord(value.what)) {
      const input = this.at(child(path, "what"));
      const at = child(path, value.controller === "owner" ? "controller" : "to");
      const expression: PlayerRef = { player: "owner", of: { ref: "it" } };
      if (context.barrier || !input) this.unresolved(expression, at, "missing-antecedent",
        "The moved object's owner requires a bound object.", ["108.3"]);
      else {
        const owner = this.node({ id: this.id(at, "owner"), path: at, node: "derived-player", scope: context.ability,
          role: "owner", of: input.id, rules: ["108.3"] });
        this.bound(expression, at, owner, ["108.3"]);
        // "Return target permanent to its owner's hand. Then that player discards a card."
        if (input.cardinality === "one" && context.phase === "resolution") {
          this.weakPlayers.add(owner);
          context.players = combine(context.players, [{ id: owner, cardinality: "one", domain: "player" }]);
        }
      }
    } else if (value.owner === "each-affected-objects-controller" && path.endsWith("/step") && !context.barrier
        && this.at(`${path.slice(0, -"/step".length)}/what`)?.domain === "object") {
      // "Doesn't untap during its controller's untap step": the controller of
      // each object the effect applies to (CR 108.4, 502.3).
      const affected = this.at(`${path.slice(0, -"/step".length)}/what`)!;
      const at = child(path, "owner");
      const controller = this.node({ id: this.id(at, "controller"), path: at, node: "derived-player", scope: context.ability,
        role: "controller", of: affected.id, rules: ["108.4", "502.3"] });
      this.bound({ player: "controller", of: { ref: "it" } }, at, controller, ["108.4", "502.3"]);
    } else if (value.battlefieldController === "owner" && path.endsWith("/return") && !context.barrier
        && this.exiledUntil(path.slice(0, -"/return".length))) {
      // "Exile ... until this creature leaves the battlefield": each card
      // returns under its owner's control (CR 610.3c).
      const exiled = this.exiledUntil(path.slice(0, -"/return".length))!;
      const at = child(path, "battlefieldController");
      const owner = this.node({ id: this.id(at, "owner"), path: at, node: "derived-player", scope: context.ability,
        role: "owner", of: exiled.id, rules: ["108.3", "610.3c"] });
      this.bound({ player: "owner", of: { ref: "it" } }, at, owner, ["108.3", "610.3c"]);
    } else if (value.battlefieldController === "owner" || value.owner === "each-affected-objects-controller") {
      this.diagnostic("unsupported-reference", path, "This ownership rule requires a bound affected-object scope.", ["109.5"]);
    }
  }
  /** A keyword ability of the source, printed on its own line. Its printed
   * parameters belong to the ability the keyword represents (CR 702): "you"
   * and a cost's payer are that ability's player, not the line's. */
  keyword(instance: KeywordInstance, path: string, context: Context): void {
    const scope = KEYWORD_SCOPES[instance.keyword];
    if (!scope || "unsupported" in scope) {
      const barrier = scope?.unsupported ?? "This keyword's intrinsic ability scopes, targets, and payment roles are not modeled.";
      this.diagnostic("unsupported-scope", path, barrier, ["702"]);
      this.walkChildren(instance as unknown as Record<string, unknown>, path, { ...context, barrier });
      return;
    }
    const protectionFromEverything = isDeepStrictEqual(instance, PROTECTION_FROM_EVERYTHING);
    if (protectionFromEverything) this.engineFeature("protection-from-everything", path,
      "Apply protection against all sources for targeting, damage, attachments, and blocking.", ["702.16j"]);
    const you = this.node({ id: this.id(path, "you"), path, node: "runtime-player", scope: context.ability, role: scope.role, rules: ["109.5", ...scope.rules] });
    this.node({ id: this.id(path, "keyword"), path, node: "keyword-ability", scope: context.ability, keyword: instance.keyword,
      ...(protectionFromEverything ? { protectionFrom: "everything" as const } : {}),
      abilities: scope.abilities, you, rules: scope.rules });
    if (instance.keyword === "hideaway" && context.source === this.id("/source", "object")) {
      if (isDeepStrictEqual(instance, HIDEAWAY_FOUR)) {
        this.node({ id: this.id(path, "pool"), path, node: "hideaway-exiled-pool",
          keyword: this.id(path, "keyword"), source: context.source, key: "source-incarnation",
          libraryOwner: you, lookCount: 4, exile: "one-face-down", remainder: "bottom-random",
          lookPermission: "controller-of-exiling-permanent", membership: "still-in-exile", cardinality: "many",
          rules: ["400.7", "406.3", "607.2a", "607.3", "702.75a"] });
      }
      this.engineFeature("hideaway-resolution", path,
        "On entry, the trigger controller looks at the top N cards of their library, exiles one face down if possible and puts the rest on the bottom in random order. Track the cards exiled by this keyword on this incarnation and the granted and retained permissions to look.",
        ["400.7", "406.3", "607.2a", "609.3", "702.75a"]);
    }
    if (instance.keyword === "demonstrate")
      this.engineFeature("demonstrate", path,
        "On cast, the trigger controller may copy the spell. If they do, they choose an opponent who also copies it. Each may choose new targets for their own copy as it is put onto the stack.",
        ["702.144a", "707.10", "707.10c"]);
    const own: Context = { ...context, you, objects: [], players: [], quantities: [], x: undefined };
    this.definitions(instance.bindings, child(path, "bindings"), own);
    for (const [key, value] of Object.entries(instance)) {
      if (key === "keyword" || key === "bindings") continue;
      if (key === "amount") { if (value === "x") this.walk({ amount: "x" }, child(path, "amount"), own); continue; }
      this.walk(value, child(path, key), KEYWORD_PAYMENT_FIELDS.has(key) ? { ...own, phase: "payment" } : own);
    }
    instance.bindings?.forEach((binding, index) => this.walk(binding.value, child(child(child(path, "bindings"), index), "value"), own));
  }
  /** A keyword ability that belongs to another object: a token being created,
   * a characteristic an effect gives, or an ability a filter describes. With
   * no printed references it has nothing to bind; any reference in its
   * parameters would need that object's own ability scope. */
  otherObjectKeyword(instance: Record<string, unknown>, path: string, context: Context): void {
    const barrier = "A keyword ability of another object needs that object's own ability scope for the references in its parameters.";
    const references = this.result.references.length, diagnostics = this.result.diagnostics.length;
    for (const [key, value] of Object.entries(instance)) {
      if (key === "keyword") continue;
      if (key === "amount") { if (value === "x") this.walk({ amount: "x" }, child(path, "amount"), { ...context, barrier }); continue; }
      this.walk(value, child(path, key), { ...context, barrier });
    }
    if (this.result.references.length > references || this.result.diagnostics.length > diagnostics)
      this.diagnostic("unsupported-scope", path, barrier, ["113.10", "702"]);
  }
  /** A replacement effect that modifies how a permanent enters (CR 614.1c,
   * 614.12). For the source's own entry, "this" is the entering object and
   * "you" the player it enters under (CR 110.2a); choices and payments are
   * made before it enters (614.12a). Entry of other permanents is evaluated
   * for each of them by the static ability's controller. */
  entering(value: Record<string, unknown>, path: string, context: Context): void {
    const what = value.what;
    const self = isRecord(what) && (what.ref === "self" || what.ref === "this");
    const scope: Context = self ? { ...context, phase: "continuous",
      you: this.node({ id: this.id(path, "entering-controller"), path, node: "runtime-player", scope: context.ability, role: "entering-controller", rules: ["110.2a", "614.12"] }) } : context;
    const bindings = Array.isArray(value.bindings) ? value.bindings as Sentence["bindings"] : undefined;
    this.definitions(bindings, child(path, "bindings"), scope);
    for (const [key, entry] of Object.entries(value)) {
      if (key === "static" || key === "bindings") continue;
      if (key === "effects") { this.sentences(entry as Sentence[], child(path, key), scope); continue; }
      this.walk(entry, child(path, key), key === "costs" ? { ...scope, phase: "payment" } : scope);
    }
    bindings?.forEach((binding, index) => this.walk(binding.value, child(child(child(path, "bindings"), index), "value"), scope));
  }
  private simplePaymentAlternatives(cost: Extract<Cost, { cost: "choice" }>, path: string, context: Context): boolean {
    if (context.barrier || context.phase !== "payment" || cost.options.length < 2 ||
        !cost.options.every(option => option.length > 0 && option.every(part =>
          (part.cost === "mana" && !part.multiplier && !part.symbols.includes("X")) ||
          (part.cost === "pay-life" && part.amount.amount === "fixed")))) return false;
    this.walk(cost.options, child(path, "options"), context);
    const rules = ["118.1", "601.2b", "601.2h", "602.2b"];
    this.node({ id: this.id(path, "payment-alternatives"), path, node: "payment-alternatives", scope: context.ability,
      payer: context.you, options: cost.options, rules });
    this.engineFeature("intrinsic-payment", path, "Choose one alternative in the activation payment scope before paying it.", rules);
    return true;
  }

  private attackPayment(effect: Extract<StaticEffect, { static: "combat-rule" }>, path: string, context: Context): boolean {
    const { rule } = effect;
    if (context.barrier || context.phase !== "continuous" || rule.rule !== "declaration-cost" ||
        rule.action !== "attack" || rule.scope !== "declaration" || rule.condition ||
        !isDeepStrictEqual(rule.who, { player: "controller", of: { ref: "them" } }) ||
        effect.what.ref !== "each" || rule.multiplier?.amount !== "count" ||
        !rule.costs.every(cost => cost.cost === "mana" && !cost.multiplier)) return false;
    const group = this.object(effect.what, child(path, "what"), context);
    if (!group || group.domain !== "object") return false;
    const at = child(path, "rule");
    const rules = ["508.1a", "508.1b", "508.1h", "508.1i", "508.1j"];
    const payer = this.node({ id: this.id(at, "declaring-player"), path: at, node: "runtime-player",
      scope: context.ability, role: "declaring-player", rules });
    this.bound(rule.who, child(at, "who"), payer, rules);
    this.bound({ ref: "them" }, `${at}/who/of`, group.id, rules);
    this.walk(rule.costs, child(at, "costs"), { ...context, you: payer, phase: "payment" });
    this.walk(rule.multiplier, child(at, "multiplier"), { ...context, actor: { id: payer, domain: "player", cardinality: "one" } });
    this.walk(rule.defenders, child(at, "defenders"), context);
    this.node({ id: this.id(at, "attack-payment"), path: at, node: "attack-payment", scope: context.ability, payer, eligible: group.id, rule, rules });
    this.engineFeature("attack-payment", at, "Evaluate the costs once for the proposed attack declaration and charge its declaring player.", rules);
    return true;
  }

  private linkedExileFilter(filter: Record<string, unknown>, path: string, context: Context): boolean {
    const origin = context.linkedExile;
    if (context.barrier || context.sourceMoved || !origin || context.lexicalAbilityPath === origin.ability ||
        !isRecord(filter.source) || (filter.source.ref !== "self" && filter.source.ref !== "this")) return false;
    this.walkChildren(filter, path, context);
    const rules = ["400.7", origin.rule, "607.3", "607.5"];
    this.node({ id: this.id(path, "linked-exile-filter"), path, node: "linked-exile-filter", scope: context.ability,
      pool: this.id(origin.path, "linked-exile-pool"), rules });
    return true;
  }

  private characteristicPredicate(test: CharacteristicRelation, path: string, subject: string, context: Context): void {
    let inner = test;
    let at = path;
    while (inner.relation === "not") { inner = inner.of; at = child(at, "of"); }
    if (inner.relation === "shared-value" && inner.among) {
      this.diagnostic("unsupported-scope", at, "The comparison needs a bound preceding set of named types.", ["205.2", "608.2c"]);
      return;
    }
    const operand = inner.relation === "shared-value" ? inner.with : inner.from;
    const operandPath = child(at, inner.relation === "shared-value" ? "with" : "from");
    const knownTiming = context.targetLegalityQuery ? "target-legality" : context.interveningIf ? "intervening-if"
      : context.announcementOperand ? "stack-announcement" : context.phase;
    const rules = ["105.2", "201.2a", "201.2c", "205.2", "205.3", "608.2h", ...(context.targetLegalityQuery ? ["601.2c", "608.2b"] : []), ...(context.interveningIf ? ["603.4"] : [])];
    const scoped = { ...context, characteristicSubject: undefined };
    let binding: Extract<BindingNode, { node: "characteristic-predicate" }>["operand"];
    if (operand.operand === "matching-set") {
      const atFilter = child(operandPath, "filter");
      this.walk(operand.filter, atFilter, inner.relation === "shared-value"
        ? { ...scoped, comparisonExclusion: { path: atFilter, subject } } : scoped);
      binding = { kind: "matching-set", filter: operand.filter, quantifier: operand.quantifier };
    } else if (inner.relation === "shared-value" && inner.characteristic === "name" &&
        context.linkedExile?.imprint && context.lexicalAbilityPath !== context.linkedExile.ability &&
        !context.sourceMoved && operand.what.ref === "related" && isDeepStrictEqual(operand.what.filter, { cls: "card" }) &&
        operand.what.relation.relation === "event-result" && operand.what.relation.event === "exile" &&
        operand.what.relation.scope === "contextual") {
      const pool = this.id(context.linkedExile.path, "linked-exile-pool");
      this.bound(operand.what, child(operandPath, "what"), pool, ["607.2a", "607.3"]);
      binding = { kind: "linked-exile-names", pool, quantifier: "any" };
    } else {
      const whatPath = child(operandPath, "what");
      const sourceOperand = operand.what.ref === "self" || (operand.what.ref === "this" && operand.what.noun !== "ability");
      const lastKnown = knownTiming === "resolution" || knownTiming === "intervening-if" || (knownTiming === "target-legality" && sourceOperand);
      this.walk(operand.what, whatPath, { ...scoped, lastKnown });
      const target = this.at(whatPath);
      if (target?.domain !== "object" || target.cardinality !== "one") {
        this.diagnostic("unsupported-scope", operandPath, "The comparison operand needs one bound object.", ["608.2h"]);
        return;
      }
      binding = { kind: "object", object: target.id, information: lastKnown ? "current-or-last-known" : "current" };
    }
    this.node({ id: this.id(path, "characteristic-predicate"), path, node: "characteristic-predicate",
      scope: context.ability, subject, operand: binding, timing: knownTiming, test, rules });
    this.engineFeature("characteristic-predicate", path,
      "Evaluate the printed relation using the indicated characteristic set and quantifier at its binding time.", rules);
  }

  private commonNameSubset(condition: Extract<Condition, { condition: "controls-subset" }>, path: string, context: Context): boolean {
    const { who, of, minimum, groupConstraints } = condition;
    if (context.barrier || who.player !== "you" || minimum.amount !== "fixed" || minimum.value < 2 ||
        !isDeepStrictEqual(groupConstraints, [{ constraint: "shared-value", characteristic: "name" }])) return false;
    this.walk(who, child(path, "who"), context);
    this.walk(of, child(path, "of"), context);
    const rules = ["201.2a", "602.2b", "608.2h"];
    this.node({ id: this.id(path, "common-name-subset"), path, node: "common-name-subset",
      scope: context.ability, controller: context.you, filter: of, minimum: minimum.value, rules });
    this.engineFeature("characteristic-predicate", path, "Test for a qualifying group with a common name without selecting any of its members.", rules);
    return true;
  }

  private dynamicStaticValue(value: Record<string, unknown>, path: string, context: Context): boolean {
    const recipient = context.recipient;
    const multiplier = value.multiplier;
    if (context.barrier || context.phase !== "continuous" || value.modifier !== "pt" ||
        !recipient || !("candidate" in recipient) || recipient.candidate.domain !== "object" || !isRecord(multiplier)) return false;
    const countsSelf = multiplier.amount === "counter-count" && isRecord(multiplier.on) && multiplier.on.ref === "it";
    if (!countsSelf && multiplier.amount !== "distinct-count") return false;
    const group = recipient.candidate;
    const rules = ["611.3a", "613.1g", "613.6"];
    const member = this.node({ id: this.id(path, "affected-member"), path, node: "characteristic-subject",
      scope: context.ability, role: "affected-member", of: group.id, timing: "continuous", rules });
    const candidate: Candidate = { id: member, cardinality: "one", domain: "object", description: group.description };
    // In this value scope alone, "it" means the individual affected object.
    this.walkChildren(value, path, { ...context, objects: countsSelf ? [candidate] : context.objects,
      affectedMember: candidate, mentionedObject: true });
    this.engineFeature("characteristic-layers", path, "Reevaluate the modifier on each affected object in the appropriate layer.", rules);
    return true;
  }

  private staticRule(rule: StaticRule, path: string, context: Context): boolean {
    if (context.barrier || context.phase !== "continuous" || !context.permanentSource) return false;
    let rules: string[];
    const first = this.result.references.length;
    if (rule.static === "prohibit") {
      const prohibition = rule.prohibition;
      if (prohibition.prohibition === "prevent-damage" && !prohibition.by && !prohibition.to) {
        rules = ["101.2", "615.12"];
      } else if (prohibition.prohibition === "activate" && prohibition.abilities.ref === "each" &&
          prohibition.abilities.filter.cls === "ability" && prohibition.abilities.filter.abilityKind === "activated" &&
          prohibition.abilities.filter.abilitySource?.ref === "each" && !prohibition.who && !prohibition.when) {
        rules = ["101.2", "602.1c", "611.3"];
      } else if (prohibition.prohibition === "cast" && prohibition.who.player === "each-opponent" &&
          prohibition.what.ref === "each" && prohibition.what.filter.cls === "spell" &&
          prohibition.when?.during === "turn" && prohibition.when.whose.player === "you") {
        rules = ["101.2", "102.3", "601.3", "611.3"];
      } else return false;
      this.walk(prohibition, child(path, "prohibition"), context);
    } else if (rule.static === "untap-during-step") {
      const owner = rule.step.owner;
      if (!isDeepStrictEqual(owner, { player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "you" } } }) ||
          !["self", "this", "all", "each"].includes(rule.what.ref)) return false;
      rules = ["102.1", "502.3", "611.3a"];
      const at = `${path}/step/owner`;
      const others = this.node({ id: this.id(at, "other-players"), path: at, node: "derived-player",
        role: "other-players", scope: context.ability, of: context.you, rules });
      this.bound(owner, at, others, rules);
      this.object(rule.what, child(path, "what"), context);
    } else {
      if (rule.who.player !== "each-player" || rule.maximum.amount !== "fixed") return false;
      rules = ["502.3", "611.3a"];
      this.walkChildren(rule as unknown as Record<string, unknown>, path, context);
    }
    const operands = this.result.references.slice(first).flatMap(ref => ref.binding === "bound" ? [ref.target] : []);
    this.node({ id: this.id(path, "static-rule"), path, node: "static-rule", scope: context.ability, rule, operands, rules });
    this.engineFeature(rule.static === "prohibit" && rule.prohibition.prohibition === "prevent-damage" ? "damage-prevention" : "static-rule",
      path, "Apply the printed prohibition or untap rule to its bound subjects as the game action is evaluated.", rules);
    return true;
  }

  private additionalStackCopy(replacement: ReplacementInstruction, path: string, context: Context): boolean {
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

  private keepUnspentMana(replacement: ReplacementInstruction, path: string, context: Context): boolean {
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

  /** CR 401.5: the looked-at object is queried when the permission is used.
   * A newly exposed top card is hidden until the current action finishes. */
  privateLibraryLook(permission: LookPermission, path: string, context: Context): boolean {
    const [card] = permission.what;
    if (context.barrier || context.phase !== "continuous" || !context.permanentSource ||
        permission.who.player !== "you" || permission.timing !== "any-time" || permission.what.length !== 1 ||
        permission.topLibraryChanges !== "after-casting-activation-or-special-action" ||
        card.ref !== "library-cards" || card.position !== "top" || card.count.amount !== "fixed" ||
        card.count.value !== 1 || card.of?.zone !== "library" || card.of.owner !== "your" ||
        Object.keys(card).some(key => !["ref", "position", "count", "of"].includes(key))) return false;
    const at = child(path, "permission");
    this.walk(permission, at, context);
    const rules = ["109.5", "401.5", "601.2i", "602.2b"];
    this.node({ id: this.id(path, "private-library-look"), path, node: "private-library-look", scope: context.ability,
      viewer: context.you, libraryOwner: context.you, refresh: "after-announcement-or-special-action", rules });
    this.engineFeature("private-library-look", path,
      "Allow only the bound viewer to inspect the current top card, after any in-progress casting, activation, or special action finishes.", rules);
    return true;
  }
  private exileReplacement(replacement: ReplacementInstruction, path: string, context: Context): boolean {
    const parts = !context.barrier && context.permanentSource ? exileReplacementParts(replacement) : undefined;
    if (!parts) return false;

    const rules = ["400.6", "400.7", "614.1a", "614.5", "614.6", "616.1"];
    const proposal = this.id(path, "exile-replacement");
    const at = `${path}/event/what`;
    const match = this.object(parts.event.what, at, { ...context, phase: "replacement-match" });
    const exilePath = `${path}/replace/effects/0/steps/0`;
    const exileEvent = this.id(exilePath, "event");
    const object = this.node({ id: this.id(at, "affected"), path: at, node: "event-participant",
      scope: context.ability, event: proposal, role: "proposed-move-object", cardinality: "one", rules });
    const followup = parts.followup ? `${path}/replace/effects/1/steps/0` : undefined;
    this.node({ id: proposal, path, node: "exile-replacement", scope: context.ability, object, filter: match?.id,
      from: "battlefield", originalDestination: "graveyard", exileEvent,
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
      "Replace each matching proposed battlefield-to-graveyard move with exile and execute the printed follow-up.", rules);
    return true;
  }

  private preventionOutcome(prevention: Extract<StaticEffect, { static: "prevent-damage" }>["prevention"],
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
  quantityReplacement(replacement: ReplacementInstruction, path: string, context: Context): boolean {
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
  bindEntryCopy({ what, model, optional, duration, changes, entryExceptions, entry }: OptionalEntryCopy,
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
  selfGraveyardReplacement(path: string, context: Context): void {
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
  /** Every established object antecedent is the source itself. */
  onlySource(context: Context): boolean {
    return context.objects.every(object => object.id === context.source || this.sourceAliases.has(object.id));
  }
  /** Players defined relative to "you": your opponents (CR 102.2), or every
   * player other than you. */
  relativePlayers(relation: "opponents" | "other-players", path: string, context: Context): void {
    const expression = { implicit: relation } as const;
    if (context.barrier) { this.unresolved(expression, path, "unsupported-scope", context.barrier, ["102.2"]); return; }
    const id = this.node({ id: this.id(path, relation), path, node: "derived-player", scope: context.ability, role: relation, of: context.you, rules: ["102.2", "109.5"] });
    this.bound(expression, path, id, ["102.2"]);
  }
  /** The object or player the source permanent is attached to (CR 301.5f, 303.4m). */
  attachedTo(path: string, context: Context): string {
    const id = `${context.ability}/attached-to`;
    if (!this.result.nodes.some(node => node.id === id))
      this.node({ id, path, node: "derived-object", scope: context.ability, role: "attached-to", of: context.source, rules: ["301.5f", "303.4m"] });
    return id;
  }
  private walkDamageAmount(effect: Extract<Effect, { effect: "damage" }>, path: string, context: Context): boolean {
    if (context.barrier || effect.to.to !== "player") return false;
    const quantity = effect.amount.amount === "twice" ? effect.amount.of : effect.amount;
    if (quantity.amount !== "count" || !(quantity.of.control === "that-player" ||
        quantity.of.controlledBy?.player === "they" || quantity.of.controlledBy?.player === "that-player")) return false;
    const { ref } = effect.to;
    if (ref.player !== "defending-player" && ref.player !== "target-player" && ref.player !== "target-opponent" &&
        ref.player !== "each-player" && ref.player !== "each-opponent") return false;
    const at = child(path, "to");
    this.walk(effect.to, at, context);
    const recipient = this.at(child(at, "ref"));
    if (!recipient) return true; // The operand has its own diagnostic.
    const amount = child(path, "amount");
    if (recipient.cardinality === "one") this.walk(effect.amount, amount, { ...context, actor: recipient });
    else if (ref.player === "each-player" || ref.player === "each-opponent") {
      const rules = ["608.2f", "608.2h", "608.2k"];
      const member = this.node({ id: this.id(amount, "recipient-member"), path: amount, node: "recipient-member",
        scope: context.ability, group: recipient.id, use: "damage-amount", rules });
      this.walk(effect.amount, amount, { ...context, actor: { id: member, domain: "player", cardinality: "one" } });
      this.engineFeature("recipient-dependent-damage", path,
        "Calculate each recipient's scalar damage amount from that recipient before applying the simultaneous damage event.", rules);
    } else {
      this.diagnostic("unsupported-scope", amount, "The damage amount needs an explicit member of its recipient set.", ["608.2f", "608.2h"]);
      this.walk(effect.amount, amount, { ...context, barrier: "The damage amount needs an explicit member of its recipient set." });
    }
    return true;
  }

  /** Walk a node's children, binding `what` (or a player recipient) before
   * the abilities it is given, so a granted ability knows its recipient. */
  walkWithRecipient(value: Record<string, unknown>, path: string, context: Context): void {
    // An instruction's subject (its acting player, its damage source) comes
    // first and is an antecedent for the rest of the same instruction:
    // "Target creature's owner puts it on top", "Target player shuffles their
    // graveyard", "deals damage equal to its power". A damage source that has
    // left its zone acts with its last-known information (CR 113.7a).
    const instruction = typeof value.effect === "string";
    const recipientKey = isRecord(value.what) ? "what" : value.static === "player-abilities" && isRecord(value.who) ? "who" : undefined;
    const first = [...(instruction ? ["who", "source"].filter(key => isRecord(value[key])) : []), ...(recipientKey ? [recipientKey] : [])]
      .filter((key, index, keys) => keys.indexOf(key) === index);
    if (!first.length && value.effect !== "create-token") { this.walkChildren(value, path, context); return; }
    const saved = { recipient: context.recipient, objects: context.objects, actor: context.actor, subjects: context.subjects, named: context.named };
    const subjects: Candidate[] = [];
    for (const key of first) {
      if (key === "source") {
        const lastKnown = context.lastKnown;
        context.lastKnown = true;
        this.walk(value[key], child(path, key), context);
        context.lastKnown = lastKnown;
      } else this.walk(value[key], child(path, key), context);
      if (!instruction) continue;
      const candidate = this.at(child(path, key));
      // "They" never means "you", which the text calls "you".
      if (key === "who" && candidate?.domain === "player" && candidate.id !== context.you) context.actor = candidate;
      if (key === "who" && isRecord(value.who) && isRecord(value.who.of)) {
        const possessor = this.at(child(child(path, "who"), "of"));
        if (possessor?.domain === "object") subjects.push(possessor);
      }
      if (key === "source" && candidate?.domain === "object") subjects.push(candidate);
      if (subjects.length) {
        context.objects = combine(context.objects, subjects);
        context.subjects = [...subjects];
      }
    }
    if (value.effect === "create-token") context.recipient = { created: true };
    else if (value.effect === "get-emblem") {
      const player = this.at(child(path, "who"));
      if (!context.barrier && player?.domain === "player" && player.cardinality === "one") {
        context.recipient = { created: true, emblemController: player.id };
        this.engineFeature("emblem", path, "Create the emblem with its printed abilities in the command zone for the bound player.",
          ["113.6p", "114.1", "114.2", "114.4"]);
      } else context.recipient = { unresolved: true };
    } else if (recipientKey) {
      const candidate = this.at(child(path, recipientKey));
      context.recipient = candidate ? { candidate } : { unresolved: true };
      // "Put a card an opponent owns from exile into that player's graveyard".
      const related = instruction && candidate && context.phase === "resolution" ? this.relationPlayer(candidate, child(path, "participant-0"), context) : undefined;
      if (related) context.players = combine(context.players, [related]);
      // "Creatures target opponent controls don't untap during their next untap step".
      if (instruction) context.players = combine(context.players, this.targetPlayersUnder(child(path, recipientKey), context));
    }
    const damageAmount = value.effect === "damage" && this.walkDamageAmount(value as unknown as Extract<Effect, { effect: "damage" }>, path, context);
    const drawRange = value.effect === "draw" && !context.barrier && context.phase === "resolution" &&
      isRecord(value.amount) && (!value.decidedBy || isDeepStrictEqual(value.decidedBy, value.who))
      ? chosenRange(value.amount as Amount) : undefined;
    const drawer = drawRange?.maximum !== undefined ? this.at(child(path, "who")) : undefined;
    const chosenDraw = drawRange && drawer?.cardinality === "one";
    if (chosenDraw) {
      const amount = child(path, "amount");
      this.node({ id: this.id(amount, "resolution-count"), path: amount, node: "resolution-count",
        scope: context.ability, chooser: drawer.id, operation: "draw", ...drawRange, rules: ["121.2", "608.2d"] });
    }
    const tested = value.condition === "is-filter" ? this.at(child(path, "what")) : undefined;
    for (const [entry, nested] of Object.entries(value)) {
      if (first.includes(entry) || (chosenDraw && entry === "amount") ||
          (damageAmount && (entry === "to" || entry === "amount"))) continue;
      const scoped = entry === "filter" && tested?.domain === "object" && tested.cardinality === "one"
        ? { ...context, characteristicSubject: tested.id } : context;
      this.walk(nested, child(path, entry), scoped);
    }
    Object.assign(context, saved);
  }

  /** An ability an effect gives an object is that object's ability (CR 113.10,
   * 201.5a-b): its source is the recipient, its "you" the recipient's
   * controller (CR 301.5d, 303.4e). Each affected object has its own instance.
   * The granting effect's own decisions (a keyword choice, a protection
   * exception) stay in the granting scope. */
  gained(grant: GainedAbility, path: string, context: Context): void {
    if (grant.gained === "this-ability") return;
    if (grant.gained === "abilities-of") {
      const barrier = "Abilities copied from other objects need those objects' abilities and their reevaluation bound (CR 113.10, 611.3a).";
      this.diagnostic("unsupported-scope", path, barrier, ["113.10", "201.5b", "611.3a"]);
      this.walkChildren(grant as unknown as Record<string, unknown>, path, { ...context, barrier });
      return;
    }
    const exception = grant.gained === "keyword" ? grant.protectionAttachmentException : undefined;
    if (exception?.exception === "already-attached") {
      const barrier = "The exempted attachments are a snapshot taken as the protection effect begins; that moment is not bound.";
      this.diagnostic("unsupported-scope", child(path, "protectionAttachmentException"), barrier, ["702.16n", "702.16p"]);
      this.walk(exception, child(path, "protectionAttachmentException"), { ...context, barrier });
    } else if (exception) this.walk(exception, child(path, "protectionAttachmentException"), context);
    if (grant.gained === "keyword-choice") {
      this.walk(grant.who, child(path, "who"), context);
      this.walk(grant.count, child(path, "count"), context);
    }
    const keywords = grant.gained === "keyword" ? [[grant.keyword, child(path, "keyword")] as const]
      : grant.gained === "keyword-choice" ? grant.options.map((option, index) => [option, child(child(path, "options"), index)] as const) : [];
    const recipient = context.recipient;
    const source = recipient && "candidate" in recipient && recipient.candidate.domain === "object"
      ? this.node({ id: this.id(path, "recipient"), path, node: "recipient", scope: context.ability, of: recipient.candidate.id,
          each: recipient.candidate.cardinality !== "one", rules: ["113.10", "201.5a"] })
      : recipient && "created" in recipient
        ? this.node({ id: this.id(path, "recipient"), path, node: "recipient", scope: context.ability, created: true,
            ...(recipient.emblemController ? { emblem: { owner: recipient.emblemController, controller: recipient.emblemController, zone: "command" } } : {}),
            rules: recipient.emblemController ? ["114.1", "114.2", "114.4"] : ["111.1", "113.10"] })
        : undefined;
    if (!source) {
      // A player's keyword, or a recipient that is not bound: only a keyword
      // without references in its parameters is free of obligations.
      if (grant.gained !== "quoted") { keywords.forEach(([keyword, at]) => this.otherObjectKeyword(keyword as unknown as Record<string, unknown>, at, context)); return; }
      const barrier = "The ability's recipient is not a bound object, so its source and controller are unknown (CR 113.10, 201.5a).";
      this.diagnostic("unsupported-scope", path, barrier, ["113.10", "201.5a"]);
      this.walk(grant.ability, child(path, "ability"), { ...context, barrier });
      return;
    }
    if (grant.gained === "quoted") { this.ability(grant.ability, child(path, "ability"), context.face, source); return; }
    const ability = this.id(path, "ability"), you = this.id(path, "you");
    this.node({ id: ability, path, node: "ability", source, face: context.face, abilityKind: "keywords", you, rules: ["113", "113.10"] });
    this.node({ id: you, path, node: "runtime-player", scope: ability, role: "static-source-controller", rules: ["109.5", "301.5d", "303.4e"] });
    const granted: Context = { source, face: context.face, ability, you, phase: "continuous", objects: [], players: [], quantities: [] };
    keywords.forEach(([keyword, at]) => {
      // "Protection from the color of your choice": the qualities are fixed by
      // the granting effect as it applies (CR 608.2d, 608.2h), so they belong
      // to the granting scope, not to the recipient's ability.
      if (!keyword.from) { this.keyword(keyword, at, granted); return; }
      const { from, ...rest } = keyword;
      this.walk(from, child(at, "from"), context);
      this.keyword(rest as KeywordInstance, at, granted);
    });
    if (grant.gained === "keyword" && grant.costDefinition) this.walk(grant.costDefinition, child(path, "costDefinition"), { ...granted, phase: "payment" });
  }
  object(expression: ObjectRef, path: string, context: Context): Candidate | undefined {
    if (!context.barrier && context.exactEventObject?.path === path &&
        (expression.ref === "replacement-event-object" || expression.ref === "prevention-event-object"))
      return this.bound(expression, path, context.exactEventObject.id, context.exactEventObject.rules);
    if (expression.ref === "related" && expression.relation.relation === "event-result" &&
        expression.relation.event === "exile" && expression.filter.cls === "card" &&
        Object.keys(expression.filter).length === 1 && !context.barrier && !context.uncertainExileOrigin &&
        context.exileResults?.length === 1) {
      const [capture] = context.exileResults;
      const rules = ["400.7j", "608.2c"];
      const at = child(path, "relation");
      this.node({ id: this.id(at, "event-link"), path: at, node: "event-link", scope: context.ability,
        action: "exile", event: capture.event, rules });
      return this.antecedent(expression, path, [capture.result], objectCardinality(expression), rules);
    }
    if (!context.barrier && !context.uncertainScanOrigin && context.scans?.length === 1 &&
        expression.ref === "related" && expression.relation.relation === "event-result" &&
        expression.relation.event === context.scans[0].operation && expression.filter.cls === "card" &&
        Object.keys(expression.filter).length === 1 &&
        (expression.relation.event !== "exile" || !context.exileResults?.length)) {
      const { event, pool, matches } = context.scans[0];
      const rules = ["608.2c", expression.relation.event === "reveal" ? "701.20" : "400.7j"];
      const at = child(path, "relation");
      this.node({ id: this.id(at, "event-link"), path: at, node: "event-link", scope: context.ability,
        action: expression.relation.event, event, rules });
      return this.antecedent(expression, path, [expression.number === "plural" ? pool : matches], objectCardinality(expression), rules);
    }
    // A paid sacrifice and a later zone successor are different objects.
    // This contextual reference is supported only for information about the
    // uniquely identified original object (CR 400.7, 608.2h, 701.21a).
    if (!context.barrier && context.lastKnown && context.sacrificedCost &&
        expression.ref === "related" && expression.number === "singular" &&
        expression.relation.relation === "event-result" && expression.relation.event === "sacrifice" &&
        expression.relation.scope === "contextual" && describes(context.sacrificedCost.description, expression.filter)) {
      const rules = ["601.2h", "602.2b", "608.2h", "701.21a"];
      this.bound(expression, path, context.sacrificedCost.id, rules);
      return context.sacrificedCost;
    }
    if (expression.ref === "card-extraction-group") context = { ...context, barrier: "Separate card-extraction slots require distinct allocation, shared-pool, branch-choice and aggregate result bindings." };
    const target = expression.ref === "target" || expression.ref === "any-target";
    const operands = target ? { ...context, announcementOperand: true } : context;
    const count = this.boundTargetCount(expression, context) ?? this.boundResolutionCount(expression, path, context);
    for (const [key, value] of Object.entries(expression)) {
      if (key === "filter" && isRecord(value) && Array.isArray(value.characteristicRelations) &&
          !operands.barrier && (expression.ref === "target" || expression.ref === "each" ||
            expression.ref === "all" || expression.ref === "filter")) {
        const at = child(path, key);
        const timing = target || context.targetLegalityQuery ? "target-legality" : context.phase;
        const subject = this.node({ id: this.id(at, "filter-candidate"), path: at, node: "characteristic-subject",
          scope: context.ability, role: "evaluated-candidate", timing, rules: ["601.2c", "608.2b", "608.2h"] });
        this.walk(value, at, { ...operands, characteristicSubject: subject, ...(target ? { targetLegalityQuery: true as const } : {}) });
      } else if (key !== "count" || !count) this.walk(value, child(path, key), operands);
    }
    if ("groupConstraints" in expression && expression.groupConstraints?.length) {
      this.unresolved(expression, path, "unsupported-scope", "Collective selection restrictions need a selected-group scope and legality binding; per-candidate eligibility alone is insufficient.", ["201.2b", "601.2c", "608.2d"]);
      return undefined;
    }
    if (context.barrier && !(path === context.replacementSubjectPath && expression.ref === "self" && Object.keys(expression).length === 1)) {
      this.unresolved(expression, path, "unsupported-scope", context.barrier, ["201.5", "607"]);
      return undefined;
    }
    switch (expression.ref) {
      case "self": case "this":
        if (expression.ref === "this" && expression.noun === "ability") return this.bound(expression, path, context.ability, ["113.7"]);
        if (context.sourceMoved) {
          // Its owner, controller or characteristics come from last-known information (CR 608.2h).
          if (context.lastKnown) return this.bound(expression, path, context.source, ["201.5", "608.2h"]);
          // The card it became is what an instruction acts on ("return this
          // card to your hand"); questions about it use last-known information.
          if (context.sourceSuccessor && /\/what(\/\d+)?$/.test(path)) return this.bound(expression, path, context.sourceSuccessor.id, ["400.7e", "603.6c"]);
          this.unresolved(expression, path, "unsupported-reference", "After a source zone change, this reference needs an action-specific source/last-known/successor rule.", ["400.7", "608.2h"]);
          return undefined;
        }
        return this.bound(expression, path, context.source, ["201.5"]);
      case "it": case "them": case "that": {
        const role = this.roleAntecedents.get(path);
        if (role) return this.antecedent(expression, path, [role], objectCardinality(expression), ["614.15", "608.2c"]);
        // Older templating says "it" for the permanent with the ability ("{T}:
        // It deals 1 damage"). With no other object named in the ability, it
        // can mean nothing else.
        if (expression.ref === "it" && (context.permanentSource || context.staticAbility) && !context.mentionedObject && !context.spellPronoun && context.objects.length === 0
          && (!context.sourceMoved || context.lastKnown))
          return this.bound(expression, path, context.source, context.sourceMoved ? ["201.5", "113.7a", "608.2h"] : ["201.5"]);
        // CR 608.2h: information about an object that has left its zone comes
        // from its last-known information ("its controller", "its power").
        let candidates = context.lastKnown && context.departed?.length ? combine(context.objects, context.departed) : context.objects;
        // Asked for its information, an object a trigger saw move is known by
        // its last-known information, not as the object it became (CR 603.10a).
        // A card the effect put onto the battlefield is where it was expected
        // to be, so its current information answers (CR 608.2h).
        // "That card" still means the card it was.
        const onBattlefield = (candidate: Candidate) => candidate.description?.cls === "permanent"
          && !(expression.ref === "that" && expression.filter.cls === "card");
        if (context.lastKnown) candidates = candidates.filter(candidate => candidate.successorOf && onBattlefield(candidate)
          ? true : !candidates.some(other => other.successorOf === candidate.id && onBattlefield(other))
            && (!candidate.successorOf || !candidates.some(other => other.id === candidate.successorOf)));
        // Acted on, it is the object it became (CR 400.7e): "its owner shuffles it into their library".
        else candidates = candidates.filter(candidate => !candidates.some(other => other.successorOf === candidate.id));
        // A pronoun means the source (or the definite subject of its trigger),
        // or an object a cost chose, only when nothing else could be meant (as
        // for a bare "it" above): another object the ability introduces is the
        // antecedent. Those objects are named explicitly when they are meant.
        // "That creature" never means an object the text calls "this creature".
        if (expression.ref === "that") candidates = candidates.filter(candidate => candidate.id !== context.source && !this.sourceAliases.has(candidate.id));
        else candidates = candidates.filter(candidate => !candidate.qualifiedOnly);
        // "When a player casts a spell, ... counter that spell", "if at least
        // four mana was spent to cast it": the spell the trigger is about,
        // when nothing else is named (CR 603.2c).
        if (!candidates.length && context.triggerObject && (expression.ref === "that"
          ? describes(context.triggerObject.candidate.description, expression.filter) : context.spellPronoun)) candidates = [context.triggerObject.candidate];
        const named = expression.ref !== "that" && context.named
          && !candidates.some(candidate => !context.named!.ids.includes(candidate.id) && !context.named!.before.includes(candidate.id))
          ? context.named.ids : [];
        // The card a trigger saw an object become is, like the source, meant
        // only when the effect names nothing else.
        const fallback = (candidate: Candidate) => !named.includes(candidate.id) && (this.costObjects.has(candidate.id)
          || candidate.triggerSuccessor === true
          || (!context.sourceNamed && (candidate.id === context.source || this.sourceAliases.has(candidate.id))));
        // A possessive means its own instruction's subject: "This creature
        // deals damage equal to its power to that creature."
        const subject = context.subjects?.length === 1 ? context.subjects[0] : undefined;
        if (expression.ref === "it" && path.endsWith("/of") && subject && candidates.some(candidate => candidate.id === subject.id)) candidates = [subject];
        else {
          // "If that creature has power 4 or greater, it gains first strike":
          // a pronoun governed by a condition means the condition's subject,
          // even one that would otherwise be meant only as a last resort. An
          // object the governed instructions introduce after the condition is
          // meant instead ("if this creature is tapped, search for a card, put
          // it into your graveyard").
          const governing = context.conditionSubject;
          if (candidates.length > 1 && governing && candidates.some(candidate => candidate.id === governing.candidate.id)
            && (expression.ref !== "that" || describes(governing.candidate.description, expression.filter))
            && candidates.every(candidate => candidate.id === governing.candidate.id || governing.before.includes(candidate.id)))
            candidates = [governing.candidate];
          if (candidates.some(fallback) && candidates.some(candidate => !fallback(candidate))) candidates = candidates.filter(candidate => !fallback(candidate));
          // "That land" means an antecedent established as a land; only when
          // every candidate's kind is established (the source's by its type line).
          if (expression.ref === "that" && candidates.length > 1) {
            const established = (candidate: Candidate) => candidate.id === context.source || this.sourceAliases.has(candidate.id)
              ? this.faceTypes.get(context.face) : candidate.description;
            if (candidates.every(candidate => established(candidate))) {
              const agreeing = candidates.filter(candidate => describes(established(candidate), expression.filter));
              if (agreeing.length) candidates = agreeing;
            }
          }
        }
        return this.antecedent(expression, path, candidates, objectCardinality(expression), ["608.2k", "400.7j", ...(candidates !== context.objects ? ["608.2h"] : [])]);
      }
      case "trigger-event-object":
        if (context.triggerObject?.event === expression.event)
          return this.bound(expression, path, context.triggerObject.candidate.id, ["603.2c", "707.10"]);
        this.unresolved(expression, path, "missing-antecedent", "No unique matching stack-object trigger occurrence is established in this lexical ability.", ["603.2c", "707.10"]);
        return undefined;
      case "card-extraction-group":
        this.unresolved(expression, path, "unsupported-scope", "Separate card-extraction slots require distinct allocation, shared-pool, branch-choice and aggregate result bindings.", ["608.2d", "701.23b"]);
        return undefined;
      case "choice-result":
        if (expression.scope === "contextual" && expression.which === "chosen" &&
            !context.uncertainTargetOrigin && context.declaredTargets?.length === 1 &&
            describes(context.declaredTargets[0].description, expression.filter))
          return this.antecedent(expression, path, context.declaredTargets, objectCardinality(expression), ["601.2c", "608.2b", "608.2c"]);
        this.unresolved(expression, path, "unsupported-reference", "This chosen object needs its exact announcement, resolution/search or linked-choice result; proximity does not establish the origin.", ["601.2c", "608.2d", "607.2d"]);
        return undefined;
      case "copy-result":
        if (context.copiedCards?.length === 1 && !context.uncertainCardCopyOrigin &&
            /\/instruction\/(?:what|fromAmong)$/.test(path))
          return this.antecedent(expression, path, context.copiedCards, objectCardinality(expression), ["707.12", "707.12a"]);
        this.unresolved(expression, path, "unsupported-reference", "This printed copy reference needs a specific copy-result binding; it is not the most recent object.", ["707.10"]);
        return undefined;
      case "amassed-army":
        this.unresolved(expression, path, "unsupported-reference", "The amassed Army needs the Army creature chosen by the preceding amass.", ["701.47c"]);
        return undefined;
      case "vote-leaders":
        this.unresolved(expression, path, "unsupported-reference", "The objects with the most votes need the preceding vote's tally.", ["701.38"]);
        return undefined;
      case "soulbond-pair":
        this.unresolved(expression, path, "unsupported-reference", "Both members of a soulbond pair need the pairing relation of the referenced creature.", ["702.95b"]);
        return undefined;
      case "pile":
        this.unresolved(expression, path, "unsupported-reference", "The objects of a pile need the preceding separation and the pile choice.", ["700.3"]);
        return undefined;
      case "equipped": case "enchanted": case "fortified": {
        // CR 301.5f, 303.4m: whatever the permanent with this ability is attached to.
        if (context.sourceMoved) {
          this.unresolved(expression, path, "unsupported-reference", "After a source zone change, the attachment relation needs the source's last-known information.", ["400.7", "608.2h"]);
          return undefined;
        }
        return this.bound(expression, path, this.attachedTo(path, context), ["301.5f", "303.4m"]);
      }
      case "rest": {
        const scan = context.scans?.length === 1 && !context.uncertainScanOrigin ? context.scans[0] : undefined;
        const explicitPool = expression.of ? this.at(child(path, "of")) : undefined;
        if (scan && (explicitPool ? explicitPool.id === scan.pool.id :
            !expression.of && !this.poolSelections.has(context.ability) &&
              !context.objects.some(candidate => candidate.id !== scan.matches.id && candidate.cardinality !== "one"))) {
          const rules = ["608.2c", scan.operation === "reveal" ? "701.20" : "400.7j"];
          const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability,
            pool: scan.pool.id, excluding: scan.matches.id, rules });
          return this.bound(expression, path, id, rules);
        }

        // "Put one of them into your hand and the rest into your graveyard":
        // the pool the same sentence's selection chose from, less that choice.
        const sentence = /^(.*\/effects\/\d+)\//.exec(path)?.[1];
        const chosen = this.poolSelections.get(context.ability);
        const pool = expression.of ? this.at(child(path, "of")) : chosen?.pool;
        if (chosen && pool && sentence && chosen.path.startsWith(`${sentence}/`) && chosen.pool.id === pool.id) {
          const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability, pool: pool.id,
            excluding: chosen.selected.id, rules: ["608.2c"] });
          return this.bound(expression, path, id, ["608.2c"]);
        }
        this.unresolved(expression, path, "unsupported-reference", "This relation needs an attachment, event, pool, or linked-ability binding.", ["303.4", "301.5", "607"]);
        return undefined;
      }
      case "haunted-creature": case "targeted-by": case "related": case "replacement-event-object": case "prevention-event-object":
        this.unresolved(expression, path, "unsupported-reference", "This relation needs an attachment, event, pool, or linked-ability binding.", ["303.4", "301.5", "607"]);
        return undefined;
      case "target": case "any-target": return this.selection(expression, path, context, true);
      case "from-pool": {
        const selected = this.selection(expression, path, context, false);
        const pool = this.at(child(path, "pool"));
        if (selected && pool) this.poolSelections.set(context.ability, { pool, selected, path });
        return selected;
      }
      case "filter": case "each": case "all": case "library-cards": case "graveyard-cards": case "zone-cards": case "commander":
        return this.selection(expression, path, context, false);
      default: {
        const unreachable: never = expression;
        throw new Error(`Unhandled ObjectRef variant: ${JSON.stringify(unreachable)}`);
      }
    }
  }
  player(expression: PlayerRef, path: string, context: Context): Candidate | undefined {
    this.walkChildren(expression as unknown as Record<string, unknown>, path,
      expression.player === "target-player" || expression.player === "target-opponent" ? { ...context, announcementOperand: true } : context);
    if (context.barrier) {
      this.unresolved(expression, path, "unsupported-scope", context.barrier, ["109.5"]);
      return undefined;
    }
    if (expression.player === "filtered") {
      this.unresolved(expression, path, "unsupported-scope", "Player eligibility requires a per-candidate predicate scope; the base target or quantifier does not remove these restrictions.", ["115.1", "608.2c"]);
      return undefined;
    }
    if (expression.player === "you") return this.bound(expression, path, context.you, ["109.5"]);
    if (expression.player === "chosen-player") {
      this.unresolved(expression, path, "unsupported-reference", "The chosen player requires its exact announcement, resolution or linked pre-entry choice; no new selection is implied.", ["601.2c", "608.2d", "607.1c", "607.2d"]);
      return undefined;
    }
    if (expression.player === "the-player" && context.definiteEventPlayer &&
        context.players.every(player => player.id === context.definiteEventPlayer?.id || this.weakPlayers.has(player.id)))
      return this.bound(expression, path, context.definiteEventPlayer.id, ["106.12a", "102.1", "504.1", "603.2c"]);
    if (expression.player === "group" || expression.player === "the-player") {
      this.unresolved(expression, path, "unsupported-reference", expression.player === "group"
        ? "Grouped players require occurrence-preserving participant, event multiplicity and aggregate result bindings."
        : "This definite player requires its exact contextual event or choice role; proximity is not proof of identity.", ["608.2c", "608.2e", "608.2f"]);
      return undefined;
    }
    if (expression.player === "they" || expression.player === "that-player") {
      // Within an instruction, "they" is its acting player ("Target opponent
      // sacrifices a creature they control").
      if (context.actor) return this.bound(expression, path, context.actor.id, ["608.2k"]);
      // A player named only through an object ("its owner's hand", "a land an
      // opponent controls") is meant only when no player is named outright.
      const strong = context.players.filter(player => !this.weakPlayers.has(player.id));
      const found = strong.length ? strong : context.players;
      const owners = new Set<string>();
      const distinct = found.filter(player => {
        const node = this.result.nodes.find(candidate => candidate.id === player.id);
        const owned = node?.node === "derived-player" && node.role === "owner" ? node.of
          : node?.node === "selection" && "player" in node.expression && node.expression.player === "owner"
            ? this.at(child(node.path, "of"))?.id : undefined;
        if (!owned) return true;
        if (owners.has(owned)) return false;
        owners.add(owned);
        return true;
      });
      return this.antecedent(expression, path, distinct, "unknown", distinct.length < found.length ? ["108.3", "608.2k"] : ["608.2k"]);
    }
    if (expression.player === "that-player-or-controller") {
      this.unresolved(expression, path, "unsupported-reference", "This player depends on whether one earlier object-or-player antecedent is a player or an object; that shared antecedent must be bound first.", ["608.2c"]);
      return undefined;
    }
    if (expression.player === "enchanted-player") {
      // CR 303.4b, 303.4m: the player the Aura is attached to, not its controller.
      if (context.sourceMoved) {
        this.unresolved(expression, path, "unsupported-reference", "After a source zone change, the attachment relation needs the source's last-known information.", ["400.7", "608.2h"]);
        return undefined;
      }
      return this.bound(expression, path, this.attachedTo(path, context), ["303.4b", "303.4m"]);
    }
    if (expression.player === "defending-player" && !context.sourceMoved) {
      // CR 508.5: for an ability of an attacking creature, or one that names
      // an attacking creature, the player that creature is attacking. Only
      // one such creature can fix it; for several, each has its own (508.5a).
      const attacker = context.attacker ?? (context.attackingSource ? { id: context.source, cardinality: "one" as const, domain: "object" as const } : undefined);
      if (attacker?.cardinality === "one") {
        const id = this.id(path, "defending");
        this.node({ id, path, node: "derived-player", scope: context.ability, role: "defending", of: attacker.id, rules: ["506.2", "508.5"] });
        return this.bound(expression, path, id, ["508.5"]);
      }
    }
    if (expression.player === "replacement-event-player" || expression.player === "prevention-event-player" || expression.player === "defending-player") {
      this.unresolved(expression, path, "unsupported-reference", "This player requires a bound replacement or combat event.", ["508.1", "614"]);
      return undefined;
    }
    return this.selection(expression, path, context,
      expression.player === "target-player" || expression.player === "target-opponent");
  }
  boundTargetCount(expression: ObjectRef | PlayerRef, context: Context): ReturnType<typeof chosenRange> {
    if (!context.barrier && !context.distributedCount && context.phase === "resolution" && "ref" in expression &&
        expression.ref === "target" && expression.selectionId && !expression.chosenBy && !expression.random)
      return chosenRange(expression.count);
  }
  boundResolutionCount(expression: ObjectRef | PlayerRef, path: string, context: Context): ReturnType<typeof chosenRange> {
    if (!context.barrier && context.phase === "resolution" && context.resolutionCount?.path === path &&
        "ref" in expression && expression.ref === "filter") return chosenRange(expression.count);
  }
  selection(expression: ObjectRef | PlayerRef, path: string, context: Context, targeted: boolean): Candidate | undefined {
    if (targeted && context.phase !== "resolution") {
      this.unresolved(expression, path, "unsupported-scope", "Target declarations and target predicates outside a resolving ability need a specific announcement scope.", ["601.2c", "602.2b"]);
      return undefined;
    }
    const selectionId = "selectionId" in expression && typeof expression.selectionId === "string" ? expression.selectionId : undefined;
    const id = targeted && selectionId ? `${context.targetScope ?? context.ability}/targets/${encodeURIComponent(selectionId)}` : this.id(path, targeted ? "target" : "query");
    const cardinality = "ref" in expression ? objectCardinality(expression)
      : isPluralPlayerRef(expression) ? "many" : "one";
    if (targeted && !selectionId) {
      this.unresolved(expression, path, "missing-target-identity",
        "A target requires its parser-minted selection identity; structural equality and proximity cannot identify it.", ["601.2c"]);
      return undefined;
    }
    let countChoice: string | undefined;
    if (targeted) {
      const previous = this.targets.get(id);
      const signature = JSON.stringify(expression);
      if (previous) {
        if (previous.signature !== signature) {
          this.invalidNodes.set(id, "ambiguous-target-identity");
          this.unresolved(expression, path, "ambiguous-target-identity", "The same target identity carries conflicting selectors.", ["601.2c"], [id]);
          this.diagnostic("ambiguous-target-identity", previous.path, "The same target identity carries conflicting selectors.", ["601.2c"]);
          return undefined;
        }
        return this.bound(expression, path, id, ["601.2c"]);
      }
      this.targets.set(id, { id, path, signature });
      const count = this.boundTargetCount(expression, context);
      if (count) {
        const scope = context.targetScope ?? context.ability;
        const announcing = this.result.nodes.find(node => node.id === scope);
        if (announcing?.node === "ability") {
          const at = child(path, "count");
          countChoice = this.node({ id: `${id}/count`, path: at, node: "target-count", scope, chooser: announcing.you,
            ...count, rules: ["115.3", "601.2c", "602.2b", "603.3d"] });
        } else this.diagnostic("unsupported-number-binding", child(path, "count"),
          "The target count requires its announcing ability.", ["601.2c"]);
      }
    }
    const resolution = this.boundResolutionCount(expression, path, context);
    if (resolution && context.resolutionCount) {
      const at = child(path, "count");
      const operation = context.resolutionCount.operation;
      countChoice = this.node({ id: this.id(at, "resolution-count"), path: at, node: "resolution-count",
        scope: context.ability, chooser: context.you, ...resolution, operation,
        rules: operation === "search" ? ["608.2d", "701.23b"] : ["608.2c", "608.2d"] });
    }
    const predicateRules = context.eventPredicate ? ["603.2c"] : context.phase === "replacement-match" ? ["614.1a"] : undefined;
    const rules = targeted ? ["601.2c"] : predicateRules ?? ["608.2h"];
    this.node({ id, path, node: "selection", scope: context.ability, expression, cardinality,
      ...(context.directCreatureSpellCopy?.what === expression ? { chooser: context.you } : {}),
      ...(countChoice ? { countChoice } : {}),
      ...(targeted && context.announcementMember ? { perMember: context.announcementMember } : {}),
      timing: targeted ? "stack-announcement" : context.targetLegalityQuery ? "target-legality" : context.phase,
      selection: targeted ? "target" : resolution ? "resolution-choice" : predicateRules ? "event-participant-filter" : "evaluate-reference", rules });
    const candidate = this.bound(expression, path, id, rules);
    return { ...candidate, cardinality };
  }
  at(path: string): Candidate | undefined {
    let binding: ReferenceBinding | undefined;
    for (let index = this.result.references.length - 1; index >= 0; index--) {
      if (this.result.references[index].path === path) { binding = this.result.references[index]; break; }
    }
    if (binding?.binding !== "bound") return undefined;
    const node = this.result.nodes.find(node => node.id === binding.target);
    const cardinality = node && "cardinality" in node ? node.cardinality
      : referenceCardinality(binding.expression);
    const established: ObjectFilter | undefined = node?.node === "distinct-library-search" ? { cls: "card" }
      : node?.node === "event-result" ? node.description
      : node?.node === "selection" && "ref" in node.expression ? description(node.expression)
      : "ref" in binding.expression && binding.expression.ref !== "affected-object" && binding.expression.ref !== "those-counters" ? description(binding.expression) : undefined;
    return { id: binding.target, cardinality, domain: domain(binding.expression), description: established };
  }
  cascade(effect: Extract<StaticEffect, { static: "as-cascade" }>, path: string, context: Context): void {
    const choice = child(path, "choice");
    const controllerPath = child(path, "controller");
    const from = child(choice, "from");
    const controller = this.player(effect.controller, controllerPath, context);
    if (!controller) {
      this.unresolved(effect.choice.from, from, "unresolved-dependency",
        "The cascade occurrence requires the controller of the static ability.", ["109.5", "702.85b"]);
      return;
    }
    const occurrence = this.node({ id: this.id(path, "cascade"), path, node: "cascade-occurrence",
      scope: context.ability, controllerMatches: controller.id, rules: ["608.2c", "702.85b", "702.85c"] });
    const decider = this.node({ id: this.id(controllerPath, "cascade-controller"), path: controllerPath,
      node: "derived-player", scope: context.ability, role: "controller", of: occurrence, rules: ["608.2c", "702.85b"] });
    const pool = this.node({ id: this.id(from, "pool"), path: from, node: "cascade-exiled-pool", occurrence,
      rules: ["702.85a", "702.85b", "702.85c"] });
    const cards = this.bound(effect.choice.from, from, pool, ["702.85b"]);
    this.event(choice, context, "choose-and-put", [cards], { optional: effect.choice.optional, decider });
    this.engineFeature("cascade-resolution", path,
      "Offer the choice from this cascade's cards still in exile after exiling, before the cast decision or bottom placement.",
      ["608.2d", "702.85b"]);
  }
  event(path: string, context: Context, operation: string, inputs: Candidate[],
    extra: { optional?: true; decider?: string; repeated?: "for-each"; conditional?: true; condition?: string; requiresResult?: string } = {}): string {
    return this.node({ id: this.id(path, "event"), path, node: "event", scope: context.ability,
      operation, inputs: inputs.map(input => input.id), availability: "when-performed", ...extra, rules: ["608.2c"] });
  }
  /** Branch contexts start from the antecedents established so far; nothing a
   * branch establishes is visible to a sibling or after the construct. */
  branch(context: Context): Context {
    return { ...context, objects: [...context.objects], players: [...context.players], quantities: [...context.quantities] };
  }
  private blockBranchCaptures(context: Context, branches: Context[]): void {
    context.uncertainExileOrigin ||= branches.some(branch =>
      branch.uncertainExileOrigin || branch.exileResults !== context.exileResults);
    context.uncertainStackOrigin ||= branches.some(branch =>
      branch.uncertainStackOrigin || branch.stackCopies !== context.stackCopies);
    context.uncertainScanOrigin ||= branches.some(branch =>
      branch.uncertainScanOrigin || branch.scans !== context.scans);
    context.uncertainCardCopyOrigin ||= branches.some(branch =>
      branch.uncertainCardCopyOrigin || branch.copiedCards !== context.copiedCards);
    context.uncertainMeasureOrigin ||= branches.some(branch =>
      branch.uncertainMeasureOrigin || branch.measuredEvents !== context.measuredEvents);
    context.uncertainTargetOrigin ||= branches.some(branch =>
      branch.uncertainTargetOrigin || branch.declaredTargets !== context.declaredTargets);
  }
  /** CR 700.2a-c: modes are chosen as the spell or ability is put on the stack,
   * and a mode's targets exist only if it was chosen. Each mode binds in its own
   * branch; a later instruction cannot rely on a result of an unchosen mode. */
  modal(effect: Extract<Effect, { effect: "modal" }>, path: string, context: Context): void {
    const count = effect.count;
    if ("variable" in count) this.walk({ amount: count.variable === "x" ? "x" : "that-much" }, child(path, "count"), context);
    const announcement = { ...context, announcementOperand: true };
    this.walk(effect.conditionalCount, child(path, "conditionalCount"), announcement);
    this.walk(effect.chooser, child(path, "chooser"), announcement);
    let sourceMoved = context.sourceMoved;
    const branches: Context[] = [];
    effect.options.forEach((option, index) => {
      const at = child(child(path, "options"), index);
      const branch = this.branch(context);
      this.walk(option.additionalCost, child(at, "additionalCost"), { ...branch, phase: "payment" });
      this.sentences(option.effects, child(at, "effects"), branch);
      sourceMoved ||= branch.sourceMoved;
      branches.push(branch);
    });
    context.sourceMoved = sourceMoved;
    this.blockBranchCaptures(context, branches);
    context.objects = []; context.players = []; context.quantities = [];
  }
  /** Players targeted anywhere in the instruction at `path` ("target
   * player's library", "creatures target opponent controls"). */
  targetPlayersUnder(path: string, context: Context): Candidate[] {
    const found: Candidate[] = [];
    for (const reference of this.result.references) {
      if (reference.binding !== "bound" || !reference.path.startsWith(`${path}/`) && reference.path !== path) continue;
      const node = this.result.nodes.find(entry => entry.id === reference.target);
      if (node?.node === "selection" && node.timing === "stack-announcement" && "player" in node.expression && node.cardinality === "one" && node.id !== context.you)
        found.push({ id: node.id, cardinality: "one", domain: "player" });
    }
    return combine([], found);
  }
  /** The one object or group an "exile ... until" instruction at `path` exiled. */
  exiledUntil(path: string): Candidate | undefined {
    const what = this.at(`${path}/what/0`);
    return what?.domain === "object" && !this.at(`${path}/what/1`) ? what : undefined;
  }
  /** "Each permanent destroyed this way": the one earlier instruction of this
   * ability, in a branch visible from here, that performed that action. */
  thisWay(relation: Record<string, unknown>, path: string, context: Context): boolean {
    const actions: Record<string, string[]> = { exile: ["exile"], sacrifice: ["sacrifice"], discard: ["discard"], reveal: ["reveal"], destroy: ["destroy"],
      tap: ["tap"], return: ["return-to-hand"], counter: ["counter"], mill: ["mill"], draw: ["draw"], cast: ["play-card"] };
    const operations = actions[String(relation.event)];
    const root = /^(.*\/abilities\/\d+)\//.exec(path)?.[1];
    if (!operations || !root || context.barrier) return false;
    const branches = new Set(["options", "outcomes", "rows", "otherwise", "instead", "original"]);
    const visible = (eventPath: string) => {
      const a = eventPath.split("/"), b = path.split("/");
      let common = 0;
      while (common < a.length && common < b.length && a[common] === b[common]) common++;
      // A sibling mode, outcome or branch diverges right after its container.
      return !branches.has(a[common - 1]) && !a.slice(common).some(segment => branches.has(segment));
    };
    const events = this.result.nodes.filter(node => node.node === "event" && operations.includes(node.operation)
      && node.path.startsWith(`${root}/`) && visible(node.path));
    if (events.length !== 1) return false;
    this.node({ id: this.id(path, "event-link"), path, node: "event-link", scope: context.ability, action: String(relation.event), event: events[0].id, rules: ["608.2c"] });
    return true;
  }
  /** A target is announced with its spell or ability (CR 601.2c), whichever
   * branch of it names the target. */
  announced(candidate: Candidate): boolean {
    const node = this.result.nodes.find(entry => entry.id === candidate.id);
    return node?.node === "selection" && node.timing === "stack-announcement";
  }
  /** "Flip a coin. If you win the flip, [A]. If you lose the flip, [B]" (CR
   * 705): one flip, and the outcome's instructions as exclusive branches.
   * What a branch produces exists only if that outcome happened; a target
   * named in one outcome is still the ability's target in the next ("If you
   * lose the flip, it gets -0/-2"). */
  flipCoin(effect: Extract<Effect, { effect: "flip-coin" }>, path: string, context: Context): void {
    this.walk(effect.who, child(path, "who"), context);
    this.walk(effect.count, child(path, "count"), context);
    const flipper = this.at(child(path, "who"));
    this.event(path, context, "flip-coin", flipper ? [flipper] : [], context.conditional ? { conditional: true } : {});
    let announced: Candidate[] = [];
    let sourceMoved = context.sourceMoved;
    const branches: Context[] = [];
    (effect.outcomes ?? []).forEach((outcome, index) => {
      const branch: Context = { ...this.branch(context), conditional: true };
      branch.objects = combine(branch.objects, announced);
      this.sentences(outcome.effects, child(child(child(path, "outcomes"), index), "effects"), branch);
      announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
      sourceMoved ||= branch.sourceMoved;
      branches.push(branch);
    });
    this.blockBranchCaptures(context, branches);
    context.sourceMoved = sourceMoved;
    context.objects = combine(context.objects, announced);
  }
  /** "Roll a d20" with a results table (CR 706.3): one roll, then the row its
   * result falls in, as exclusive branches like a coin flip's outcomes.
   * Targets in any row are the ability's own (CR 706.3b). */
  dieTable(effect: Extract<Effect, { effect: "roll-dice" }>, path: string, context: Context): void {
    for (const key of ["who", "die", "count", "adjustments"] as const) this.walk(effect[key], child(path, key), context);
    const roller = this.at(child(path, "who"));
    this.event(path, context, "roll-dice", roller ? [roller] : [], context.conditional ? { conditional: true } : {});
    let announced: Candidate[] = [];
    let sourceMoved = context.sourceMoved;
    const branches: Context[] = [];
    effect.table!.rows.forEach((row, index) => {
      const at = child(child(child(path, "table"), "rows"), index);
      const branch: Context = { ...this.branch(context), conditional: true };
      branch.objects = combine(branch.objects, announced);
      this.walk(row.range, child(at, "range"), branch);
      this.sentences(row.effects, child(at, "effects"), branch);
      announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
      sourceMoved ||= branch.sourceMoved;
      branches.push(branch);
    });
    this.blockBranchCaptures(context, branches);
    context.sourceMoved = sourceMoved;
    context.objects = combine(context.objects, announced);
  }
  /** CR 608.2d: exactly one listed instruction is chosen while the effect is
   * applied. Options bind like modes; shared references are the same nodes. */
  choice(effect: Extract<Effect, { effect: "choice" }>, path: string, context: Context): void {
    this.walk(effect.who, child(path, "who"), context);
    const decider = this.at(child(path, "who"))?.id;
    let sourceMoved = context.sourceMoved;
    const branches: Context[] = [];
    effect.options.forEach((option, index) => {
      const branch = { ...this.branch(context), ...(decider ? { decider } : {}) };
      this.effect(option, child(child(path, "options"), index), branch);
      sourceMoved ||= branch.sourceMoved;
      branches.push(branch);
    });
    this.blockBranchCaptures(context, branches);
    context.sourceMoved = sourceMoved;
    context.objects = []; context.players = []; context.quantities = [];
  }
  /** Simultaneous instructions are one event: none sees another's results,
   * and all of their results are available afterwards. */
  simultaneous(effect: Extract<Effect, { effect: "simultaneous" }>, path: string, context: Context): void {
    const start = this.branch(context);
    const after = this.branch(context);
    const branches: Context[] = [];
    effect.steps.forEach((step, index) => {
      const branch = this.branch(start);
      this.effect(step, child(child(path, "steps"), index), branch);
      branches.push(branch);
      after.objects = combine(after.objects, branch.objects);
      after.players = combine(after.players, branch.players);
      after.quantities = combine(after.quantities, branch.quantities);
      after.sourceMoved ||= branch.sourceMoved;
      after.departed = combine(after.departed ?? [], branch.departed ?? []);
    });
    this.blockBranchCaptures(context, branches);
    Object.assign(context, { objects: after.objects, players: after.players, quantities: after.quantities, sourceMoved: after.sourceMoved, departed: after.departed });
  }
  /** A triggered ability created now (CR 603.7): its source is the creating
   * ability's source and its controller the player who controlled that ability
   * (603.7d-e). It may refer to objects the creating ability named; those are
   * the same objects (603.7c). What it establishes stays inside it. Targets the
   * creating ability chose belong to that ability's announcement. */
  nestedAbility(path: string, context: Context, kind: string, rules: string[]): Context {
    const ability = this.node({ id: this.id(path, kind), path, node: "ability", source: context.source, face: context.face,
      abilityKind: kind, you: context.you, rules });
    return { ...this.branch(context), ability, phase: "resolution", decisions: undefined, previousDecisions: undefined,
      decider: undefined, conditional: undefined, availableChoice: undefined, entryChoice: undefined, resolutionChoice: undefined,
      sacrificedCost: undefined, stackCopies: undefined, previousEffect: undefined, exileResults: undefined,
      uncertainExileOrigin: undefined, uncertainStackOrigin: undefined, scans: undefined, uncertainScanOrigin: undefined,
      copiedCards: undefined, uncertainCardCopyOrigin: undefined, announcementMember: undefined, definiteEventPlayer: undefined,
      measuredEvents: undefined, uncertainMeasureOrigin: undefined, triggerX: undefined,
      linkedExile: undefined, lexicalAbilityPath: undefined, delayedSchedule: kind === "delayed-triggered",
      declaredTargets: undefined, uncertainTargetOrigin: undefined };
  }
  delayedTrigger(effect: Extract<Effect, { effect: "create-delayed-trigger" }>, path: string, context: Context): void {
    const delayed = this.nestedAbility(path, context, "delayed-triggered", ["603.7"]);
    const enclosing = context.targetScope ?? context.ability;
    this.trigger(effect.trigger, child(path, "trigger"), effect.triggerTargets === "enclosing-ability" ? { ...delayed, targetScope: enclosing } : delayed);
    const body: Context = effect.targets === "enclosing-ability" ? { ...delayed, targetScope: enclosing } : delayed;
    this.walk(effect.condition, child(path, "condition"), body);
    this.sentences(effect.effects, child(path, "effects"), body);
  }
  /** "When you do, …" (CR 603.12): triggers on an action of this resolution and
   * otherwise follows the delayed-trigger rules. "When you do" names the one
   * optional instruction just decided. */
  reflexiveTrigger(effect: Extract<Effect, { effect: "create-reflexive-trigger" }>, path: string, context: Context): void {
    const event = effect.event;
    if (event.event === "preceding-action") {
      const decisions = context.decisions?.length ? context.decisions : context.previousDecisions ?? [];
      this.walk(event.who, child(child(path, "event"), "who"), context);
      if (decisions.length === 1) this.node({ id: this.id(child(path, "event"), "decision-link"), path: child(path, "event"), node: "decision-link",
        scope: context.ability, condition: event.performed ? "you-do" : "you-dont", decision: decisions[0], rules: ["603.12", "608.2d"] });
      else this.diagnostic("unsupported-reference", child(path, "event"), "Payment/performance conditions need an explicit preceding-action link.", ["603.12"]);
    } else if (event.event === "repeated-payment" || event.event === "damage-prevented") {
      const barrier = "This reflexive trigger needs the repeated payment or prevention it watches bound.";
      this.diagnostic("unsupported-scope", child(path, "event"), barrier, ["603.12a", "615.5"]);
      this.walk(event, child(path, "event"), { ...context, barrier });
    } else this.walk(event, child(path, "event"), context);
    const reflexive = this.nestedAbility(path, context, "reflexive-triggered", ["603.12"]);
    this.sentences(effect.effects, child(path, "effects"), reflexive);
  }
  /** "[Do A.] If [condition], [do B] instead" (CR 614.15): one of the two
   * happens. Each binds in its own branch from what came before; the
   * condition may refer to the original instruction's objects. Results of
   * either branch are not antecedents afterwards. */
  selfReplacement(effect: Extract<Effect, { effect: "self-replacement" }>, path: string, context: Context): void {
    const run = (instructions: Effect | Effect[] | { sequence: Sentence[] }, at: string): Context => {
      const branch = this.branch(context);
      if (Array.isArray(instructions)) instructions.forEach((instruction, index) => this.effect(instruction, child(at, index), branch));
      else if ("effect" in instructions) this.effect(instructions, at, branch);
      else this.sentences(instructions.sequence, child(at, "sequence"), branch);
      return branch;
    };
    const original = run(effect.original, child(path, "original"));
    // The condition decides whether the original instruction is replaced, so
    // it is checked before that instruction happens (CR 614.15): it sees the
    // original's participants, not what the instruction did to them.
    const participants = [...original.objects, ...(original.departed ?? [])]
      .filter(candidate => this.result.nodes.find(node => node.id === candidate.id)?.node !== "event-result");
    this.walk(effect.condition, child(path, "condition"), { ...this.branch(context), objects: combine(context.objects, participants), players: original.players });
    // The replacement's pronoun in a role the original instruction fills means
    // that participant: "Target creature gets +2/+2. If it's a Human, instead
    // it gets +3/+3"; "~ deals 2 damage to any target. If ..., it deals 3
    // damage instead."
    if ("effect" in effect.original && Array.isArray(effect.instead)) {
      const pronoun = (value: unknown) => isRecord(value) && (value.ref === "it" || value.ref === "them" || value.ref === "that");
      for (const [index, instruction] of effect.instead.entries()) {
        const at = child(child(path, "instead"), index);
        const instead = instruction as unknown as Record<string, unknown>, replaced = effect.original as unknown as Record<string, unknown>;
        for (const key of ["what", "source", "on", "from"]) {
          if (!pronoun(instead[key]) || !isRecord(replaced[key])) continue;
          const participant = this.at(child(child(path, "original"), key));
          if (participant) this.roleAntecedents.set(child(at, key), participant);
        }
        if (isRecord(instead.to) && pronoun(instead.to.ref) && isRecord(replaced.to) && isRecord(replaced.to.ref)) {
          const participant = this.at(child(child(child(path, "original"), "to"), "ref"));
          if (participant) this.roleAntecedents.set(child(child(at, "to"), "ref"), participant);
        }
      }
    }
    const instead = run(effect.instead, child(path, "instead"));
    this.blockBranchCaptures(context, [original, instead]);
    context.sourceMoved = context.sourceMoved || original.sourceMoved || instead.sourceMoved;
    context.mentionedObject = context.mentionedObject || original.mentionedObject || instead.mentionedObject;
    context.objects = []; context.players = []; context.quantities = [];
  }
  /** "[Player] pays [cost]" as an instruction (CR 118.12): the payer is that
   * player. Paying it is optional when printed with "may". */
  payCost(effect: Extract<Effect, { effect: "pay-cost" }>, path: string, context: Context): void {
    this.walk(effect.who, child(path, "who"), context);
    const payer = this.at(child(path, "who"));
    const decider = effect.optional ? (effect.decidedBy ? (this.walk(effect.decidedBy, child(path, "decidedBy"), context), this.at(child(path, "decidedBy"))?.id) : payer?.id) : undefined;
    if (!payer || payer.domain !== "player" || payer.cardinality !== "one") {
      const barrier = "The payment needs one bound paying player.";
      this.diagnostic("unsupported-scope", path, barrier, ["118.12"]);
      this.walk(effect.costs, child(path, "costs"), { ...context, barrier });
      return;
    }
    this.walk(effect.costs, child(path, "costs"), { ...context, you: payer.id, phase: "payment" });
    const event = this.event(path, context, "pay-cost", [payer], {
      ...(effect.optional ? { optional: true as const, ...(decider ? { decider } : {}) } : {}), ...(context.conditional ? { conditional: true as const } : {}) });
    if (effect.optional) context.decisions?.push(path);
    // "You may pay life equal to its power. If you do, put that many +1/+1 counters on it."
    if (effect.costs.length === 1 && (effect.costs[0].cost === "pay-life" || effect.costs[0].cost === "remove-counters"))
      context.quantities = combine(context.quantities, [this.eventResult(path, event, "quantity", "one", "measured-quantity")]);
  }
  /** "You may [A] and [B]": one decision covers the instructions, which are
   * then performed in order (CR 608.2c-d). Their results exist only if the
   * player chose to perform them. */
  optionalGroup(effect: Extract<Effect, { effect: "optional-group" }>, path: string, context: Context): void {
    this.walk(effect.who, child(path, "who"), context);
    const decider = this.at(child(path, "who"))?.id;
    const group: Context = { ...context, decider: decider ?? "unresolved" };
    effect.steps.forEach((step, index) => this.effect(step, child(child(path, "steps"), index), group));
    this.blockBranchCaptures(context, [group]);
    Object.assign(context, { objects: group.objects, players: group.players, quantities: group.quantities, sourceMoved: group.sourceMoved, sourceSuccessor: group.sourceSuccessor, x: group.x });
    context.decisions?.push(path);
  }
  /** The player one object is described by ("a land an opponent controls",
   * "target card from an opponent's graveyard"), who "that player" may then
   * mean: that object's controller or owner (CR 108.3, 404.2). */
  relationPlayer(object: Candidate, path: string, context: Context): Candidate | undefined {
    const filter = object.description;
    if (!filter || object.cardinality !== "one" || object.domain !== "object") return undefined;
    const role = filter.control === "opponent" ? "controller"
      : filter.own === "opponent" || filter.zone?.owner === "an-opponent" ? "owner" : undefined;
    if (!role) return undefined;
    const id = this.id(path, `${role}-of-described`);
    if (!this.result.nodes.some(node => node.id === id))
      this.node({ id, path, node: "derived-player", scope: context.ability, role, of: object.id, rules: role === "owner" ? ["108.3", "404.2"] : ["108.4"] });
    this.weakPlayers.add(id);
    return { id, cardinality: "one", domain: "player" };
  }
  eventResult(path: string, event: string, result: "objects" | "quantity" | "player", cardinality: BindingCardinality,
    identity: Extract<BindingNode, { node: "event-result" }>["identity"], description?: ObjectFilter, subsetOf?: string): Candidate {
    const id = this.node({ id: this.id(path, result), path, node: "event-result", event, result, cardinality,
      availability: "if-produced", identity, ...(description ? { description } : {}), ...(subsetOf ? { subsetOf } : {}),
      rules: identity === "zone-successors" ? ["400.7j"] : ["608.2h"] });
    return { id, cardinality, domain: result === "objects" ? "object" : result === "player" ? "player" : "number", description };
  }
  hideawayPlay(path: string, pool: string, context: Context): void {
    const at = child(path, "instruction");
    const { instruction } = HIDEAWAY_PLAY;
    const controller = this.bound(instruction.who, child(at, "who"), context.you, ["109.5"]);
    const cards = this.bound(instruction.what, child(at, "what"), pool, ["400.7", "607.2a", "607.3"]);
    this.walk(instruction.alternativeCost, child(at, "alternativeCost"), context);
    const conditionPath = child(at, "condition");
    this.walk(instruction.condition, conditionPath, context);
    const creatures = this.at(`${conditionPath}/value/among`)!;
    const condition = this.node({ id: this.id(conditionPath, "condition"), path: conditionPath,
      node: "total-power-condition", scope: context.ability, creatures: creatures.id,
      comparison: instruction.condition.comparison, timing: "resolution", rules: ["608.2h"] });
    this.event(path, context, HIDEAWAY_PLAY.effect, [cards, controller],
      { optional: true, decider: controller.id, conditional: true, condition });
    this.engineFeature("linked-exile-play", path,
      "During resolution, if the activator's creatures have total power at least ten, they may play each card still in this incarnation's linked exile pool. Apply land-play limits or cast without paying its mana cost, subject to legality.",
      ["113.7a", "305.2", "305.3", "406.3a", "607.2a", "607.3", "608.2g", "608.2h", "701.18b"]);
  }
  revealAndCast(effects: typeof REVEAL_AND_CAST, path: string, context: Context): void {
    const [shuffle, scan] = effects[0].steps;
    const [exile, bottom] = effects[1].steps;
    const [cast] = effects[2].steps;
    this.effect(shuffle, `${path}/0/steps/0`, context);

    const scanPath = `${path}/0/steps/1`;
    this.walkChildren(scan, scanPath, context);
    const controller = this.at(child(scanPath, "who"))!;
    const event = this.event(scanPath, context, scan.effect, [controller]);
    const pool = this.eventResult(scanPath, event, "objects", "many", "same-objects", { cls: "card" });
    const hit = this.eventResult(child(scanPath, "until"), event, "objects", "one", "same-objects", scan.until.matching, pool.id);
    context.objects = [hit];

    const exilePath = `${path}/1/steps/0`;
    this.effect(exile, exilePath, context);
    // "The rest" excludes the revealed library object, not its successor in exile.
    this.poolSelections.set(context.ability, { pool, selected: hit, path: exilePath });
    this.effect(bottom, `${path}/1/steps/1`, context);

    const castPath = `${path}/2/steps/0`;
    const instruction = child(castPath, "instruction");
    const successor = this.id(exilePath, "objects");
    this.walk(cast.instruction.who, child(instruction, "who"), context);
    const card = this.bound(cast.instruction.what, child(instruction, "what"), successor, ["400.7j", "608.2c"]);
    this.walk(cast.instruction.alternativeCost, child(instruction, "alternativeCost"), context);
    this.event(castPath, context, cast.effect, [card, controller], { optional: true, decider: controller.id, requiresResult: successor });
    this.engineFeature("reveal-and-cast", path,
      "Shuffle the controller's library, reveal through the first nonland or exhaustion, exile the match and put the other revealed cards on the bottom in random order. If the exile successor exists, the resolving spell's controller may cast that card during resolution without paying its mana cost if legal.",
      ["400.7j", "601.2", "608.2c", "608.2d", "608.2g", "609.3", "701.20", "701.24"]);
  }
  libraryTopMana(effect: Extract<Effect, { effect: "look-top-may-reveal-for-mana" }>, path: string, context: Context): void {
    this.walk(effect, path, context);
    const controller = this.at(child(path, "who"));
    const library = child(path, "library");
    const reveal = child(path, "reveal");
    const revealIf = child(path, "revealIf");
    const to = child(path, "to");
    const libraryOwner = this.at(child(library, "owner"));
    const handOwner = this.at(child(to, "owner"));
    if (!controller || !libraryOwner || !handOwner) return;

    const lookEvent = this.event(library, context, "look-at", combine([controller], [libraryOwner]));
    const looked = this.eventResult(library, lookEvent, "objects", "one", "same-objects", { cls: "card" });
    const revealEvent = this.event(reveal, context, "reveal", [controller, looked],
      { optional: true, decider: controller.id, conditional: true, requiresResult: looked.id });
    this.node({ id: this.id(revealIf, "eligibility"), path: revealIf, node: "colored-mana-symbol-count",
      scope: context.ability, card: looked.id, guardedEvent: revealEvent, comparison: effect.revealIf,
      rules: ["107.4e", "107.4f", "202.1", "608.2d"] });
    const revealed = this.eventResult(reveal, revealEvent, "objects", "one", "same-objects", { cls: "card" }, looked.id);
    this.event(child(path, "mana"), context, "add-mana", [controller, revealed],
      { conditional: true, requiresResult: revealed.id });
    this.event(to, context, "move-zone", [looked, handOwner], { requiresResult: looked.id });
    this.engineFeature(effect.effect, path,
      "Look privately at the top card, offer its public reveal only when eligible, and allocate the printed amount among its colors only after revealing it. Put the looked-at card into hand in either branch.",
      ["105.2", "106.1a", "107.4e", "107.4f", "400.1", "608.2c", "608.2d", "609.3", "701.20"]);
  }
  costModifications(modifications: CostModification[] | undefined, path: string, payment: Context): void {
    modifications?.forEach((modification, index) => {
      const at = child(path, index);
      const scoped = { ...payment };
      this.definitions(modification.bindings, child(at, "bindings"), scoped);
      this.walk(modification, at, scoped);
      if (scoped.x && this.result.references.some(reference => reference.binding === "bound" &&
          reference.target === scoped.x && reference.path.startsWith(`${at}/bindings/`))) {
        this.invalidNodes.set(scoped.x, "unresolved-dependency");
        this.diagnostic("unsupported-number-binding", child(at, "bindings"), "A payment definition cannot establish itself recursively.", ["107.3c"]);
      }
    });
  }
  definitions(bindings: Sentence["bindings"], path: string, context: Context): void {
    if (!bindings) return;
    for (const [index, binding] of bindings.entries()) {
      const at = child(path, index);
      if (context.x) {
        this.diagnostic("duplicate-number-definition", at, "X already has a definition in this scope; shadowing has not been established.", ["107.3i", "107.3k"]);
        continue;
      }
      context.x = this.node({ id: this.id(at, "number"), path: at, node: "defined-number", scope: context.ability,
        definition: binding.value, rules: ["107.3c"] });
    }
  }
  sentences(sentences: Sentence[], path: string, context: Context): void {
    let previousDecisions: string[] = [];
    let precedingAction: { event: string; action: "sacrifice" | "discard" } | undefined;
    for (const [index, sentence] of sentences.entries()) {
      const at = child(path, index);
      const decisions: string[] = [];
      context.decisions = decisions;
      context.previousDecisions = previousDecisions;
      this.definitions(sentence.bindings, child(at, "bindings"), context);
      // "If you do" / "If you don't" refers to the one optional instruction of
      // the preceding sentence (CR 608.2c, 608.2d).
      const condition = sentence.condition;
      const linked = condition && (condition.condition === "you-do" || condition.condition === "you-dont") && previousDecisions.length === 1
        ? previousDecisions[0] : undefined;
      const paid = !linked && condition?.condition === "you-do" ? precedingAction : undefined;
      if (paid) {
        const atCondition = child(at, "condition");
        const rules = ["118.11", "118.12", "608.2c", paid.action === "sacrifice" ? "701.21a" : "701.9a"];
        this.node({ id: this.id(atCondition, "action-cost-condition"), path: atCondition, node: "action-cost-condition",
          scope: context.ability, ...paid, rules });
        this.engineFeature("action-cost-condition", atCondition, "Check whether the preceding action was carried out as the cost of this instruction.", rules);
      }
      if (linked && condition) this.node({ id: this.id(child(at, "condition"), "decision-link"), path: child(at, "condition"), node: "decision-link",
        scope: context.ability, condition: condition.condition as "you-do" | "you-dont", decision: linked, rules: ["608.2c", "608.2d"] });
      // An "unless" clause follows its instruction and may refer to that
      // instruction's objects ("Counter target spell unless its controller
      // pays {3}"), so it binds after the steps.
      const trailing = sentence.conditionKind === "unless" || sentence.conditionPosition === "after";
      context.linkedPath = linked;
      if (!trailing) this.walk(condition, child(at, "condition"), linked || paid ? { ...context, linkedDecision: true } : context);
      if (sentence.afterConditional)
        this.diagnostic("unsupported-scope", at, "This sentence refers back to a conditional sentence; whether that condition governs it is not decided.", ["608.2c"]);
      if (condition || sentence.otherwise || sentence.afterConditional) {
        context.previousEffect = undefined;
      }
      if (condition || sentence.otherwise) {
        // The steps bind against what came before; what they establish exists
        // only if the condition held, so it is not an antecedent afterwards.
        // "If you discard a land card this way, put it ...": the objects that
        // instruction acted on.
        const link = this.result.nodes.find(node => node.node === "event-link" && node.path.startsWith(`${child(at, "condition")}/`));
        const linkedResult = link?.node === "event-link" ? this.result.nodes.find(node => node.node === "event-result" && node.event === link.event && node.result === "objects") : undefined;
        const conditionSubject = !trailing && condition && "what" in condition && isRecord(condition.what)
          && ["it", "that", "this", "self", "target", "equipped", "enchanted"].includes(String(condition.what.ref))
          ? this.at(child(child(at, "condition"), "what"))
          : linkedResult?.node === "event-result" ? { id: linkedResult.id, cardinality: linkedResult.cardinality, domain: "object" as const, description: linkedResult.description } : undefined;
        const branches = [sentence.steps, sentence.otherwise?.steps ?? []].map((steps, branchIndex) => {
          const branch: Context = { ...this.branch(context), conditional: true, decisions,
            players: combine(context.players, this.targetPlayersUnder(child(at, "condition"), context)),
            // "If you don't, ...": what the declined instruction would have produced does not exist.
            ...(linked && condition?.condition === "you-dont" && branchIndex === 0 && this.beforeDecision.has(linked)
              ? { objects: [...this.beforeDecision.get(linked)!] } : {}),
            ...(conditionSubject?.domain === "object" ? { conditionSubject: { candidate: conditionSubject,
              before: [...context.objects, ...(context.departed ?? [])].map(candidate => candidate.id) } } : {}) };
          const stepsPath = branchIndex === 0 ? child(at, "steps") : child(child(at, "otherwise"), "steps");
          steps.forEach((effect, stepIndex) => this.effect(effect, child(stepsPath, stepIndex), branch));
          return branch;
        });
        // A trailing "if" or "unless" is checked as its instruction would be
        // performed, before that instruction acts (CR 608.2c): it sees the
        // instruction's participants, not what the instruction produced.
        if (trailing) {
          const steps = branches[0];
          const participants = [...steps.objects, ...(steps.departed ?? [])]
            .filter(candidate => this.result.nodes.find(node => node.id === candidate.id)?.node !== "event-result");
          const checked: Context = { ...this.branch(context), objects: combine(context.objects, participants), players: steps.players,
            departed: steps.departed, conditional: true, decisions };
          this.walk(condition, child(at, "condition"), checked);
          steps.players = combine(steps.players, checked.players);
        }
        // Targets were announced with the spell or ability (CR 601.2c), and an
        // "unless" clause's payer and participants exist whether or not it is
        // paid; only what the instructions produce depends on the condition.
        const lasting = (candidate: Candidate) => {
          const node = this.result.nodes.find(entry => entry.id === candidate.id);
          return node?.node === "selection" ? trailing || node.timing === "stack-announcement"
            : node?.node === "derived-player" ? trailing : false;
        };
        for (const branch of branches) {
          context.objects = combine(context.objects, branch.objects.filter(lasting));
          context.players = combine(context.players, branch.players.filter(lasting));
        }
        this.blockBranchCaptures(context, branches);
        context.sourceMoved = context.sourceMoved || branches.some(branch => branch.sourceMoved);
        context.mentionedObject = context.mentionedObject || branches.some(branch => branch.mentionedObject);
      } else {
        for (const [stepIndex, effect] of sentence.steps.entries())
          this.effect(effect, child(child(at, "steps"), stepIndex), context);
      }
      previousDecisions = decisions;
      const [step] = sentence.steps;
      const actionPath = `${at}/steps/0`;
      precedingAction = !condition && !sentence.otherwise && !sentence.afterConditional &&
        sentence.steps.length === 1 && (step.effect === "sacrifice" || step.effect === "discard") && !step.optional && !step.forEach && !step.forEachVote &&
        step.who.player === "you" && (step.effect === "discard" || !step.chosenBy) &&
        step.what !== "hand" && step.what.ref === "filter" && objectCardinality(step.what) === "one" && this.at(child(actionPath, "what"))
        ? { event: this.id(actionPath, "event"), action: step.effect } : undefined;
      context.linkedPath = undefined;
      // A trailing definition may refer to a target established earlier in
      // the same sentence. Traverse it after the subjects have been bound.
      sentence.bindings?.forEach((binding, bindingIndex) =>
        this.walk(binding.value, child(child(child(at, "bindings"), bindingIndex), "value"), context));
      for (const reference of this.result.references) {
        if (reference.binding === "bound" && reference.target === context.x &&
            reference.path.startsWith(`${child(at, "bindings")}/`)) {
          this.diagnostic("unsupported-number-binding", reference.path, "A number definition cannot establish itself recursively.", ["107.3c"]);
          this.invalidNodes.set(reference.target, "unresolved-dependency");
        }
      }
    }
  }
  private boundEventAggregate(quantity: Extract<Amount, { amount: "event-quantity" }>, path: string, context: Context): boolean {
    if (context.barrier || context.uncertainMeasureOrigin || quantity.scope !== "this-way") return false;
    const damage = quantity.event === "damage" && quantity.measure === "dealt" && !quantity.by && !quantity.to;
    const life = quantity.event === "life-change" && quantity.change === "lost" && !quantity.who;
    if (!damage && !life) return false;
    const matches = context.measuredEvents?.filter(entry => entry.operation === (damage ? "damage" : "lose-life"));
    if (matches?.length !== 1) return false;
    const rules = [damage ? "120.4b" : "119.3", "608.2c", "608.2f", "608.2h"];
    this.node({ id: this.id(path, "event-aggregate"), path, node: "event-aggregate", scope: context.ability,
      event: matches[0].event, measure: damage ? "damage-dealt" : "life-lost", rules });
    this.engineFeature("event-aggregate", path, "Sum the actual outcomes of the bound instruction across its participants.", rules);
    return true;
  }

  private distinctAction(effect: Effect, path: string, context: Context): boolean {
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

  private groupedDraw(effect: Extract<Effect, { effect: "draw" }>, path: string, context: Context): boolean {
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

  private damageByEachSource(effect: Extract<Effect, { effect: "damage" }>, path: string, context: Context): boolean {
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

  private quantifiedInstruction(effect: Extract<Effect, { effect: "for-each" }>, path: string, context: Context): boolean {
    if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote ||
        effect.effects.length === 0 || Object.keys(effect.effects[0]).length !== 1) return false;
    const [first, second] = effect.effects[0].steps;
    const { each } = effect;
    const target = first?.effect === "exile-until" && first.what.length === 1 ? first.what[0]
      : first?.effect === "tap" || first?.effect === "destroy" ? first.what : undefined;
    const targetByPlayer = effect.effects.length === 1 && "player" in each && (each.player === "each-player" || each.player === "each-opponent") &&
      effect.effects[0].steps.length === 1 && target?.ref === "target" && target.upTo && objectCardinality(target) === "one" &&
      target.filter.control === "that-player" && !first?.optional && !first?.decidedBy && !first?.forEach && !first?.forEachVote;
    const tokenCopy = effect.effects.length === 1 && "ref" in each && each.ref === "each" && each.filter.cls === "token" &&
      effect.effects[0].steps.length === 1 && first?.effect === "create-token" &&
      "copyOf" in first.token && first.token.copyOf.ref === "that";
    const exileCopy = effect.effects.length === 1 && "ref" in each && each.ref === "each" && each.filter.cls === "card" &&
      each.filter.relation?.relation === "event-result" && each.filter.relation.event === "exile" &&
      each.filter.relation.scope === "this-way" && effect.effects[0].steps.length === 2 &&
      first?.effect === "copy-card" && second?.effect === "play-card" && second.instruction.what.ref === "copy-result" &&
      context.exileResults?.length === 1 && !context.uncertainExileOrigin;
    const onExiledCards = effect.effects.length === 2 && "ref" in each && each.ref === "each" &&
      isDeepStrictEqual(each.filter, { nonTypes: ["land"], cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" } }) &&
      effect.effects[0].steps.length === 1 && first?.effect === "put-counters" &&
      isDeepStrictEqual(first.on, { ref: "that", filter: { cls: "card" } }) &&
      isDeepStrictEqual(first.count, { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" }) &&
      !first.optional && !first.forEach && !first.forEachVote && !first.decidedBy &&
      effect.effects[1].steps.length === 1 && effect.effects[1].steps[0].effect === "gain-abilities" &&
      isDeepStrictEqual(effect.effects[1].condition,
        { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } }) &&
      effect.effects[1].conditionKind === "if" && !effect.effects[1].otherwise && !effect.effects[1].afterConditional &&
      context.exileResults?.length === 1 && !context.uncertainExileOrigin;
    if (!targetByPlayer && !tokenCopy && !exileCopy && !onExiledCards) return false;
    const where = child(path, "each");
    let domain: Candidate | undefined;
    const exile = exileCopy || onExiledCards ? context.exileResults?.[0] : undefined;
    if (onExiledCards && exile) {
      const rules = ["400.7j", "608.2c", "608.2f"];
      const id = this.node({ id: this.id(where, "nonland-exile-members"), path: where, node: "filtered-result-group",
        scope: context.ability, pool: exile.result.id, filter: { cls: "card", nonTypes: ["land"] }, cardinality: "many", rules });
      this.bound(each, where, id, rules);
      domain = { id, domain: "object", cardinality: "many", description: { cls: "card", nonTypes: ["land"] } };
    } else if (exile) {
      this.bound(each, where, exile.result.id, ["400.7j", "608.2c"]);
      domain = exile.result;
    } else {
      this.walk(each, where, context);
      domain = this.at(where);
    }
    if (!domain || domain.domain === "object-or-player" || domain.domain === "number") return false;
    const rules = targetByPlayer ? ["115.3", "601.2c", "603.3d", "608.2f"] : ["608.2c", "608.2f", "611.2c"];
    const memberId = this.node({ id: this.id(where, "instruction-member"), path: where, node: "instruction-member",
      scope: context.ability, group: domain.id, domain: domain.domain, rules });
    const member: Candidate = { id: memberId, cardinality: "one", domain: domain.domain, description: domain.description };
    this.node({ id: this.id(path, "group-instruction"), path, node: "group-instruction", scope: context.ability,
      execution: { kind: "for-each", domain: domain.id, member: member.id, membership: targetByPlayer ? "announcement" : "resolution",
        processing: effect.processing }, rules });
    const branch = this.branch(context);
    if (targetByPlayer) {
      branch.actor = member;
      branch.announcementMember = member.id;
    } else {
      branch.objects = [member];
      branch.departed = [];
      branch.mentionedObject = true;
      if (exile && exileCopy) branch.exileResults = [{ event: exile.event, result: member }];
    }
    this.sentences(effect.effects, child(path, "effects"), branch);
    this.blockBranchCaptures(context, [branch]);
    // No per-member selection or effect result is exported as a single
    // antecedent after the loop.
    context.objects = [];
    if (targetByPlayer && first?.effect === "tap" && target) {
      const selection = this.at(`${path}/effects/0/steps/0/what`);
      const groupRules = ["601.2c", "603.3d", "608.2b", "608.2f"];
      if (selection) {
        const pool = this.node({ id: this.id(path, "eligible-target-group"), path, node: "eligible-target-group",
          scope: context.ability, template: selection.id, member: member.id, cardinality: "many", rules: groupRules });
        context.objects = [{ id: pool, domain: "object", cardinality: "many", description: target.filter }];
      }
    }
    context.quantities = [];
    context.mentionedObject = true;
    this.engineFeature("group-instruction", path,
      "Remember which members were eligible at the stated time and process each printed instruction across those members.", rules);
    return true;
  }

  private libraryScan(effect: Extract<Effect, { effect: "scan-library" }>, path: string, context: Context): boolean {
    const { library, until, operation } = effect;
    if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote ||
        library.zone !== "library" || (library.owner !== "your" && library.owner !== "their") ||
        until.count.amount !== "fixed" || !Number.isSafeInteger(until.count.value) || until.count.value < 1) return false;
    this.walk(effect.who, child(path, "who"), context);
    const actor = this.at(child(path, "who"));
    if (actor?.domain !== "player" || actor.cardinality !== "one") return false;
    this.walk(library, child(path, "library"), { ...context, actor });
    const owner = this.at(`${path}/library/owner`);
    if (!owner || owner.id !== actor.id) {
      this.diagnostic("unsupported-scope", path, "A scan must bind the library of the player performing it.", ["608.2c"]);
      return true;
    }
    const rules = ["608.2c", "609.3", operation === "exile" ? "701.13" : "701.20"];
    const match = `${path}/until/matching`;
    const subject = this.node({ id: this.id(match, "scan-candidate"), path: match, node: "characteristic-subject",
      scope: context.ability, role: "evaluated-candidate", timing: "resolution", rules });
    this.walk(until.matching, match, { ...context, characteristicSubject: subject, actor });
    const event = this.event(path, context, `library-scan:${operation}`, [actor, owner],
      context.conditional ? { conditional: true } : {});
    const identity = operation === "exile" ? "zone-successors" as const : "same-objects" as const;
    const pool = this.eventResult(path, event, "objects", "many", identity, { cls: "card" });
    const matches = this.eventResult(child(path, "until"), event, "objects",
      until.count.value === 1 ? "one" : "many", identity, { cls: "card" }, pool.id);
    this.node({ id: this.id(path, "scan"), path, node: "library-scan", scope: context.ability, actor: actor.id,
      owner: owner.id, operation, filter: until.matching, count: until.count.value, pool: pool.id, matches: matches.id, event, rules });
    this.engineFeature("library-scan", path,
      "Process the library in order until the requested number of matches or exhaustion. Keep the complete pool distinct from the matches.", rules);
    context.scans = [...(context.scans ?? []), { event, operation, pool, matches }];
    if (operation === "exile") context.uncertainExileOrigin = true;
    context.objects = combine(context.objects, [matches]);
    context.mentionedObject = true;
    if (actor.id !== context.you) context.players = combine(context.players, [actor]);
    return true;
  }

  private copyCard(effect: Extract<Effect, { effect: "copy-card" }>, path: string, context: Context): boolean {
    if (context.barrier || context.decider || context.phase !== "resolution" || effect.forEach || effect.forEachVote ||
        effect.optional || effect.decidedBy || effect.who.player !== "you" ||
        effect.count.amount !== "fixed" || !Number.isSafeInteger(effect.count.value) || effect.count.value < 1 ||
        context.exileResults?.length !== 1 || context.uncertainExileOrigin ||
        (effect.what.ref !== "it" && effect.what.ref !== "that" && effect.what.ref !== "related")) return false;
    const card = this.object(effect.what, child(path, "what"), context);
    if (!card || card.id !== context.exileResults[0].result.id || card.cardinality !== "one") {
      this.diagnostic("unsupported-scope", path, "The copied card must be the one explicitly retained in exile.", ["707.12"]);
      return true;
    }
    const chooser = this.player(effect.who, child(path, "who"), context)!;
    const event = this.event(path, context, effect.effect, [card, chooser],
      context.conditional ? { conditional: true } : {});
    const result = this.eventResult(path, event, "objects", effect.count.value === 1 ? "one" : "many", "new-objects");
    const rules = ["112.2a", "608.2g", "704.5e", "707.12", "707.12a"];
    this.node({ id: this.id(path, "card-copy"), path, node: "card-copy", scope: context.ability,
      event, card: card.id, chooser: chooser.id, count: effect.count.value, result: result.id, rules });
    this.engineFeature("card-copy", path, "Create temporary copies in the original card's zone and offer any printed immediate casting choices.", rules);
    context.copiedCards = [...(context.copiedCards ?? []), result];
    context.objects = combine(context.objects, [result]);
    return true;
  }

  /** CR 115.7a–f, 707.10: copies retain the original choices and source.
   * Selecting new targets during copy creation is tied to the copy event. */
  stackInstruction(effect: StackInstruction, path: string, context: Context, precedingEffect?: string): boolean {
    if (context.barrier || context.phase !== "resolution" || effect.forEach || effect.forEachVote ||
        effect.decidedBy || effect.who.player !== "you" || effect === context.directCreatureSpellCopy) return false;
    const whatPath = child(path, "what");
    const whoPath = child(path, "who");
    const subject = effect.what;
    if (effect.effect === "choose-new-targets" && effect.timing === "copy-creation") {
      const [copy] = context.stackCopies ?? [];
      if (context.stackCopies?.length !== 1 || copy.path !== precedingEffect || context.uncertainStackOrigin || copy.inlineRetarget || copy.chooser !== context.you ||
          context.conditional || context.decider || !effect.optional || subject.ref !== "copy-result" ||
          copy.result.cardinality !== (subject.number === "singular" ? "one" : "many")) return false;
      const rules = ["115.7d", "115.7e", "115.7f", "707.10c"];
      this.player(effect.who, whoPath, context);
      this.bound(subject, whatPath, copy.result.id, rules);
      this.node({ id: this.id(path, "stack-instruction"), path, node: "stack-instruction", scope: context.ability,
        subject: copy.result.id, chooser: copy.chooser,
        action: { kind: "choose-new-targets", timing: "copy-creation", copyEvent: copy.event }, rules });
      context.stackCopies = [{ ...copy, inlineRetarget: true }];
      return true;
    }

    if (effect.effect === "change-text") {
      if (subject.ref !== "target" || objectCardinality(subject) !== "one" ||
          !effect.replacements.length || effect.optional || context.decider) return false;
      const candidate = this.object(subject, whatPath, context);
      if (!candidate) return false;
      this.player(effect.who, whoPath, context);
      const rules = ["608.2d", "611.2a", "612.1", "612.2", "613.1c"];
      this.node({ id: this.id(path, "text-choice"), path, node: "text-choice", scope: context.ability,
        subject: candidate.id, chooser: context.you, replacements: effect.replacements, rules });
      this.engineFeature("text-change", path, "Choose the printed word replacement and apply it only where the word has the chosen rules meaning.", rules);
      context.objects = combine(context.objects, [candidate]);
      return true;
    }

    const stack = subject.ref === "target" ? stackClasses(subject.filter)
      : subject.ref === "this" && subject.noun === "spell" && context.spellAbility ? "spell"
      : subject.ref === "trigger-event-object" && context.triggerObject?.event === subject.event
        ? subject.event === "cast" ? "spell" : "ability" : undefined;
    if (!stack || objectCardinality(subject) !== "one") return false;
    if (effect.effect === "change-targets" && effect.scope === "each") return false;
    if (effect.effect === "change-targets" && (effect.to || effect.newTarget)) return false;
    if (effect.effect === "copy-stack-object" && (effect.copyExceptions || effect.copyGrant ||
        effect.count.amount !== "fixed" || !Number.isSafeInteger(effect.count.value) || effect.count.value < 1 ||
        (effect.retarget && (effect.retarget.who.player !== "you" || !effect.retarget.optional)))) return false;

    const candidate = this.object(subject, whatPath, context);
    if (!candidate) return false;
    const chooser = this.player(effect.who, whoPath, context)!;
    if (effect.optional) {
      this.beforeDecision.set(path, context.objects);
      context.decisions?.push(path);
    }
    context.objects = combine(context.objects, [candidate]);
    const at = this.id(path, "stack-instruction");
    const scope = context.ability;
    const event = this.event(path, context, effect.effect, [candidate, chooser], {
      ...(effect.optional || context.decider ? { optional: true as const, decider: context.decider ?? chooser.id } : {}),
      ...(context.conditional ? { conditional: true as const } : {}),
    });
    if (effect.effect === "copy-stack-object" && effect.count.amount === "fixed") {
      const rules = ["707.10", "707.10b", "707.10c", "707.10f"];
      const count = effect.count.value;
      if (effect.retarget) this.player(effect.retarget.who, `${path}/retarget/who`, context);
      const result = this.eventResult(path, event, "objects", count === 1 ? "one" : "many", "new-objects",
        stack === "spell-or-ability" ? undefined : { cls: stack });
      this.node({ id: at, path, node: "stack-instruction", scope, subject: candidate.id, chooser: chooser.id,
        action: { kind: "copy", event, count, retarget: Boolean(effect.retarget) }, rules });
      // The copy is not the original. A later bare pronoun with both in
      // scope is ambiguous; an explicit copy-result has its own capture.
      context.objects = combine(context.objects, [result]);
      context.quantities = combine(context.quantities,
        [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")]);
      context.stackCopies = [...(context.stackCopies ?? []), { path, event, result, chooser: chooser.id, inlineRetarget: Boolean(effect.retarget) }];
      this.engineFeature("stack-instruction", path, "Create copies on the stack with inherited choices and the original ability source. Apply printed retargeting during creation.", rules);
    } else if (effect.effect === "change-targets" || effect.effect === "choose-new-targets") {
      const changeScope = effect.effect === "change-targets" ? effect.scope : undefined;
      if (changeScope === "each") return false;
      const rules = [changeScope === undefined ? "115.7d"
        : changeScope === "all" ? "115.7a" : changeScope === "one" ? "115.7b" : "115.7c", "115.7e", "115.7f"];
      this.node({ id: at, path, node: "stack-instruction", scope, subject: candidate.id, chooser: chooser.id,
        action: changeScope === undefined ? { kind: "choose-new-targets", timing: "resolution" }
          : { kind: "change-targets", scope: changeScope }, rules });
      this.engineFeature("stack-instruction", path, "Apply the printed target-change procedure to the bound stack object, preserving modes and division.", rules);
    }
    return true;
  }
  effect(effect: Effect, path: string, context: Context): void {
    const precedingEffect = context.previousEffect;
    context.previousEffect = path;
    if (effect.effect === "choose-object") context.uncertainTargetOrigin = true;
    if (this.distinctAction(effect, path, context)) return;
    if (effect.effect === "damage" && this.damageByEachSource(effect, path, context)) return;
    if (effect.effect === "draw" && this.groupedDraw(effect, path, context)) return;
    if (effect.effect === "for-each" && this.quantifiedInstruction(effect, path, context)) return;
    if (effect.effect === "scan-library" && this.libraryScan(effect, path, context)) return;
    if (effect.effect === "copy-card" && this.copyCard(effect, path, context)) return;
    if ((effect.effect === "copy-stack-object" || effect.effect === "change-targets" ||
        effect.effect === "choose-new-targets" || effect.effect === "change-text") &&
        this.stackInstruction(effect, path, context, precedingEffect)) return;
    if (effect.effect === "grant-cast-keyword") {
      const caster = this.player(effect.caster, child(path, "caster"), context);
      const from = child(path, "from");
      this.walk(effect.from, from, context);
      const owner = this.at(child(from, "owner"));
      const what = child(path, "what");
      if (!caster || !owner) {
        this.unresolved(effect.what, what, "unresolved-dependency",
          "The recipient spells require a bound caster and hand owner.", ["109.5", "400.1", "601.2a"]);
        return;
      }
      const grant = this.node({ id: this.id(path, "grant"), path, node: "cast-keyword-grant",
        scope: context.ability, created: "resolution", duration: effect.duration.duration, keyword: effect.keyword,
        rules: ["113.7a", "514.2", "603.3a", "611.2a", "611.2c", "611.2f"] });
      const spells = this.node({ id: this.id(what, "spells"), path: what, node: "future-cast-spells",
        grant, caster: caster.id, from: { zone: effect.from.zone, owner: owner.id }, timing: effect.timing,
        cardinality: "many", rules: ["601.2a", "601.2i", "611.2f"] });
      this.bound(effect.what, what, spells, ["601.2a", "611.2f", "702.85a"]);
      this.engineFeature("cast-keyword-grant", path,
        "Keep one source-independent grant per resolution and expire it during cleanup. Apply each grant to that controller's later completed casts from hand before collecting cast triggers.",
        ["109.5", "113.7a", "514.2", "601.2i", "603.3a", "611.2a", "611.2f", "702.85a", "702.85c"]);
      return;
    }
    if (effect.effect === "modify-characteristics") {
      // "Target land becomes a 3/3 creature": the engine applies the change in
      // the layer system with the affected set fixed as it begins (CR 611.2c,
      // 613). Its references bind as anywhere else, and what it changes is
      // still an antecedent ("It's still a land").
      this.engineFeature("characteristic-layers", path, "Characteristic changes need the engine's layer system, with the affected set and values fixed when the effect begins.", ["611.2c", "611.2d", "613.6", "604.3a"]);
      this.walk(effect, path, context);
      const changed = this.at(child(path, "what"));
      if (changed?.domain === "object") context.objects = combine(context.objects, [changed]);
      return;
    }
    if (effect.effect === "gain-abilities" && "recipient" in effect.what && !unambiguousPluralRecipient(context)) {
      this.walk(effect, path, { ...context, barrier: "An unresolved ability recipient cannot establish an object or player antecedent." });
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if (effect.effect === "for-each") {
      const barrier = "Quantified instructions require member references, original-ability target multiplicity, membership timing, and aggregated result bindings.";
      this.diagnostic("unsupported-scope", path, barrier, ["608.2e", "608.2f", "101.4", "115.3"]);
      this.walk(effect, path, { ...context, barrier });
      context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
      context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
      context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if ("who" in effect && effect.who?.player === "group") {
      const barrier = "Grouped actions preserve participant occurrences and shared-event multiplicity; result identities need explicit per-role aggregation.";
      this.diagnostic("unsupported-scope", path, barrier, ["101.4", "121.2c", "608.2e", "608.2f"]);
      this.walk(effect, path, { ...context, barrier });
      context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
      context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
      context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    const plain = !effect.optional && !effect.forEach && !effect.forEachVote;
    if (plain && effect.effect === "look-top-may-reveal-for-mana") { this.libraryTopMana(effect, path, context); return; }
    if (plain && effect.effect === "modal" && !effect.unchosen && !effect.distinctTargetPlayers &&
        (!effect.allowRepeat || ("exactly" in effect.count && !effect.conditionalCount && !effect.random && !effect.chooser && !effect.keyword))) {
      if (effect.allowRepeat) this.engineFeature("repeatable-modal-selection", path,
        "Each chosen occurrence needs its own announced targets and resolves in printed mode order.", ["700.2", "601.2b", "601.2c", "608.2c"]);
      this.modal(effect, path, context);
      return;
    }
    if (effect.effect === "choice" && (plain || (!context.barrier && effect.optional && !effect.decidedBy &&
        !effect.forEach && !effect.forEachVote && effect.who.player === "you"))) {
      if (effect.optional) {
        this.beforeDecision.set(path, context.objects);
        context.decisions?.push(path);
        this.event(path, context, "choose-one", [{ id: context.you, domain: "player", cardinality: "one" }],
          { optional: true, decider: context.you, ...(context.conditional ? { conditional: true as const } : {}) });
      }
      this.choice(effect, path, context);
      return;
    }
    if (plain && effect.effect === "flip-coin" && !effect.until && effect.outcomes?.length && effect.count.amount === "fixed" && effect.count.value === 1) {
      this.flipCoin(effect, path, context); return;
    }
    if (plain && effect.effect === "roll-dice" && effect.table && !effect.ignore && effect.count.amount === "fixed" && effect.count.value === 1
      && !JSON.stringify(effect.table).includes('"effect":"roll-again"')) {
      this.dieTable(effect, path, context); return;
    }
    if (plain && effect.effect === "simultaneous") { this.simultaneous(effect, path, context); return; }
    if (plain && effect.effect === "optional-group") { this.optionalGroup(effect, path, context); return; }
    if (effect.effect === "pay-cost" && !effect.times && !effect.forEach && !effect.forEachVote) { this.payCost(effect, path, context); return; }
    if (plain && effect.effect === "create-delayed-trigger") { this.delayedTrigger(effect, path, context); return; }
    if (plain && effect.effect === "create-reflexive-trigger") { this.reflexiveTrigger(effect, path, context); return; }
    if (plain && effect.effect === "self-replacement" && !effect.replacementChoice) { this.selfReplacement(effect, path, context); return; }
    if (["optional-group", "choice", "villainous-choice", "simultaneous", "modal", "create-delayed-trigger", "create-reflexive-trigger", "create-replacement", "self-replacement", "pay-cost", "roll-dice", "roll-again", "flip-coin"].includes(effect.effect) || effect.forEachVote) {
      const barrier = "Nested, optional, simultaneous, or delayed instruction scopes need explicit capture and merge rules.";
      this.diagnostic("unsupported-scope", path, barrier, ["603.7", "608.2c"]);
      this.walk(effect, path, { ...context, barrier });
      context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
      context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
      context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if (effect.optional) this.beforeDecision.set(path, context.objects);
    const castPool = !context.barrier && !context.decider && !context.uncertainExileOrigin &&
      context.exileResults?.length === 1 && precedingEffect &&
      context.exileResults[0].event === this.id(precedingEffect, "event") ? context.exileResults[0].result : undefined;
    const immediateCasts = effect.effect === "play-card" && effect.optional && !effect.forEach && !effect.forEachVote &&
      !effect.decidedBy && castPool?.cardinality === "many" &&
      effect.instruction.who.player === "you" && effect.instruction.action === "cast" &&
      effect.instruction.what.ref === "filter" && isDeepStrictEqual(effect.instruction.what.filter, { cls: "spell" }) &&
      isDeepStrictEqual(effect.instruction.what.count, { amount: "chosen", minimum: 0 }) &&
      isDeepStrictEqual(effect.instruction.alternativeCost, { cost: "without-mana-cost" }) &&
      isDeepStrictEqual(effect.instruction.fromAmong, { ref: "that", filter: { cls: "card" }, plural: true });
    const slots = plain && !context.barrier && context.phase === "resolution" ? coloredSearchSlots(effect) : undefined;
    if (slots && effect.effect === "search") {
      const selection = child(path, "for");
      this.walk(effect.who, child(path, "who"), context);
      this.walk(effect.zones, child(path, "zones"), context);
      const allocation = this.node({ node: "distinct-library-search", id: this.id(selection, "allocation"), path: selection,
        scope: context.ability, chooser: context.you, libraryOwner: context.you, slots, allocation: "distinct-objects",
        perSlot: "zero-or-one", cardinality: "many", rules: ["608.2d", "701.23b"] });
      this.bound(effect.for, selection, allocation, ["608.2d", "701.23b"]);
    } else {
      const count = immediateCasts ? { path: `${path}/instruction/what`, operation: "cast-from-exile" as const }
        : resolutionCount(effect, path);
      this.walk(effect, path, count || effect.effect === "distribute-counters"
        ? { ...context, resolutionCount: count, distributedCount: effect.effect === "distribute-counters" } : context);
    }
    // CR 608.2d: "[player] may" is decided while the instruction is applied;
    // declining it produces no results, as any impossible instruction would.
    const decider = effect.optional ? (effect.decidedBy ? this.at(child(path, "decidedBy"))?.id : context.you)
      : context.decider === "unresolved" ? undefined : context.decider;
    const objectPaths: string[] = [];
    const playerPaths: string[] = [];
    let produces: "objects" | "quantity" | undefined;
    let cardinality: BindingCardinality = "unknown";
    let identity: Extract<BindingNode, { node: "event-result" }>["identity"] = "same-objects";
    switch (effect.effect) {
      case "search": objectPaths.push(child(path, "for")); playerPaths.push(child(path, "who")); break;
      case "choose-object": objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
      case "separate-piles": objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
      // "Exile ... until this leaves the battlefield" (CR 610.3): an exile whose
      // return is a later, separate one-shot effect.
      case "exile-until":
        effect.what.forEach((_, index) => objectPaths.push(child(child(path, "what"), index)));
        if (effect.who) playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "zone-successors";
        break;
      case "exile": case "move-zone": case "return-to-hand":
        objectPaths.push(child(path, "what"));
        if (effect.who) playerPaths.push(child(path, "who"));
        // The public-zone successor is trackable under CR 400.7j. Following
        // a hidden-zone move needs additional rules and remains diagnosed.
        if (effect.effect === "exile" || (effect.effect === "move-zone" && ["battlefield", "graveyard", "exile", "stack", "command"].includes(effect.to.zone))) {
          produces = "objects"; identity = "zone-successors";
        }
        break;
      case "create-token": playerPaths.push(child(path, "who")); produces = "objects"; cardinality = cardCount(effect.count); identity = "new-objects"; break;
      case "draw": case "mill":
        playerPaths.push(child(path, "who")); produces = "objects"; cardinality = cardCount(effect.amount); identity = "new-objects"; break;
      case "gain-life": case "lose-life":
        playerPaths.push(child(path, "who")); produces = "quantity"; cardinality = "one"; identity = "measured-quantity"; break;
      case "damage":
        objectPaths.push(child(path, "source"));
        produces = "quantity"; cardinality = "one"; identity = "measured-quantity";
        // Damage's recipient is also a salient participant; treating its
        // source as an automatic "it" antecedent would be a false binding.
        if (effect.to.to === "object") objectPaths.push(child(child(path, "to"), "ref"));
        // "Deals 2 damage to target player. That player discards a card."
        else if (effect.to.to === "player") playerPaths.push(child(child(path, "to"), "ref"));
        else if (effect.to.to === "any-target") objectPaths.push(child(path, "to"));
        else if (effect.to.to === "divided") objectPaths.push(child(child(path, "to"), "among"));
        else if (effect.to.to === "them") this.diagnostic("unsupported-scope", child(path, "to"), "A bare \"them\" damage recipient needs its player-or-object antecedent bound.", ["109.1", "608.2c"]);
        else if (effect.to.to === "each") effect.to.refs.forEach((_ref, index) => objectPaths.push(child(child(child(path, "to"), "refs"), index)));
        break;
      case "fight": objectPaths.push(child(path, "a"), child(path, "b")); break;
      case "fight-each-other": objectPaths.push(child(path, "what")); break;
      case "look-at":
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        // "Look at the top three cards of your library": those cards (CR 401.5).
        if (!("ref" in effect.what)) { produces = "objects"; identity = "same-objects"; cardinality = cardCount(effect.what.top); }
        break;
      // A discarded card goes to its owner's graveyard, a public zone where
      // the rest of the effect can find it (CR 400.7j, 701.9a).
      case "discard":
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "zone-successors"; break;
      case "reveal": case "sacrifice": case "gain-control":
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
      case "tap": case "untap":
        objectPaths.push(child(path, "what"));
        if (effect.who) playerPaths.push(child(path, "who"));
        break;
      case "counter":
        objectPaths.push(child(path, "what"));
        if (effect.countered?.to.zone === "exile") {
          produces = "objects"; identity = "zone-successors";
          this.engineFeature("counter-destination", path,
            "Find only the public successor created by a successful counter and apply its printed entry state.", ["400.7j", "701.5"]);
        }
        break;
      case "destroy": case "pump": case "set-pt":
      case "transform": case "convert": case "phase-out": case "phase-in":
      case "flip": case "detain": case "suspect": case "unsuspect": case "prepare": case "unprepare": case "become-status":
      case "gain-abilities": case "lose-abilities": case "cant-have-abilities": case "untap-step-suppression":
      case "become": case "cant": case "remove-from-combat": case "become-blocked": case "combat-rule":
        objectPaths.push(child(path, "what")); break;
      case "turn-face":
        objectPaths.push(child(path, "what"));
        if (effect.who) playerPaths.push(child(path, "who"));
        break;
      case "attach": objectPaths.push(child(path, "what"), child(path, "to")); playerPaths.push(child(path, "who")); break;
      case "unattach": objectPaths.push(child(path, "what"), child(path, "from")); playerPaths.push(child(path, "who")); break;
      case "put-counters": objectPaths.push(child(path, "on")); break;
      case "put-sticker": objectPaths.push(child(path, "on")); playerPaths.push(child(path, "who")); break;
      case "remove-counters": objectPaths.push(child(path, "from")); break;
      case "add-mana": case "get-counters": case "lose-counters": case "get-emblem": case "pay-cost": case "choose-value": case "player-gain-abilities":
      case "shuffle": case "shuffle-pile": case "choose-pile": case "win-game": case "lose-game": case "lose-unspent-mana": playerPaths.push(child(path, "who")); break;
      case "declare-target": objectPaths.push(child(path, "what")); break;
      case "shuffle-into-library":
        // CR 701.24: the objects go into a hidden zone; their last-known information remains.
        effect.what.forEach((_ref, index) => objectPaths.push(child(child(path, "what"), index)));
        playerPaths.push(child(path, "who"));
        break;
      // Instructions whose only references are their printed participants;
      // they produce no objects a later pronoun could mean.
      case "double-counters": case "put-counters-of": objectPaths.push(child(path, "on")); break;
      case "switch-pt": case "multiply-pt": objectPaths.push(child(path, "what")); break;
      case "move-counters": objectPaths.push(child(path, "from"), child(path, "to")); break;
      case "exchange-control": effect.what.forEach((_, index) => objectPaths.push(child(child(path, "what"), index))); break;
      case "add-turns": case "set-life-total": case "double-life-total": case "double-mana-pool": playerPaths.push(child(path, "who")); break;
      case "add-phases": case "end-turn": case "end-combat": case "player-rule": case "additional-land-plays": case "modify-spells-cast": case "cost-modification": break;
      // A prevention or redirection shield (an engine feature, recorded where
      // it is walked) establishes no object a later pronoun could mean.
      case "prevent-damage": case "redirect-damage": break;
      // A permission names the card it lets a player play; the card stays
      // where it is (CR 601.2a moves it only when it is cast).
      case "grant-play-permission":
        objectPaths.push(child(child(path, "permission"), "what")); playerPaths.push(child(child(path, "permission"), "who")); break;
      // The card moves only if it is cast, and the effect can then find the
      // spell it became (CR 400.7h), so it remains the antecedent.
      case "play-card":
        objectPaths.push(child(child(path, "instruction"), "what")); playerPaths.push(child(child(path, "instruction"), "who")); break;
      case "prohibit": case "cast-limit": case "loyalty-activation-limit": case "control-player": case "repeat-process": case "end-effect-action": case "legend-rule-exemption": case "player-combat-rule": case "targeting-as-though":
      case "skip-time": case "set-day-night":
      case "distribute-counters":
      case "exchange-life-totals":
      case "exchange-values": case "exchange-zones": case "choose-random": case "choose-friend-or-foe":
      case "scan-library": case "grant-look-permission": case "self-replacement": case "look-top-may-reveal-for-mana":
        this.diagnostic("unsupported-scope", path, "This instruction's event/result associations are not yet established.", ["608.2c"]);
        if (effect.effect === "scan-library") context.uncertainScanOrigin = true;
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "copy-stack-object": case "copy-card": case "choose-new-targets":
        if (effect === context.directCreatureSpellCopy) {
          this.engineFeature("permanent-spell-copy", path,
            "Recheck the target, copy its spell decisions and copiable values with the printed exception, and resolve the copy as a token permanent.",
            ["601.2c", "608.2b", "707.9", "707.10", "707.10f"]);
          objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
          produces = "objects"; cardinality = "one"; identity = "new-objects";
          break;
        }
        this.diagnostic("unsupported-scope", path, "Stack-copy results and copy target-selection timing require their own binding scope.", ["707.10", "115.7"]);
        if (effect.effect !== "choose-new-targets") context.uncertainStackOrigin = true;
        if (effect.effect === "copy-card") context.uncertainCardCopyOrigin = true;
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "change-text":
        this.diagnostic("unsupported-scope", path, "Chosen words of a text-changing effect require their own binding scope.", ["612"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "change-targets":
        this.diagnostic("unsupported-scope", path, "Changed targets of another spell or ability require their own binding scope.", ["115.7"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      // CR 701, 725, 726: a keyword action's own choices (which creature
      // bolster picks, proliferate's recipients, support's targets) are fixed by
      // its rule, not printed references. Its printed participants bind
      // normally; an action that creates or reveals new objects adds them as a
      // result of unknown size, so a later pronoun is never resolved past them.
      case "airbend":
        // Airbend exiles its target (it may then be cast from exile).
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "zone-successors"; break;
      case "manifest-dread":
        // CR 701.62a, 701.40a: at most one card becomes a face-down
        // creature. The other looked-at cards never enter this result.
        playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "new-objects"; cardinality = "one"; break;
      case "manifest": case "cloak":
        // The card is turned face down and put onto the battlefield: a new object (CR 400.7, 701.40, 701.58).
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "new-objects"; break;
      case "learn": case "open-attraction": case "assemble-contraption": case "incubate": case "create-role": case "amass":
      case "discover": case "endure":
        if ("what" in effect) objectPaths.push(child(path, "what"));
        if ("attachedTo" in effect) objectPaths.push(child(path, "attachedTo"));
        if ("who" in effect && effect.who) playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "new-objects"; break;
      case "keyword-action":
        playerPaths.push(child(path, "who"));
        if (effect.action !== "proliferate") { produces = "objects"; identity = "new-objects"; }
        break;
      case "become-monarch": case "take-initiative": case "ring-tempts": case "forage": case "time-travel": case "planeswalk": case "chaos-ensues":
      case "behold": case "collect-evidence": case "fateseal": case "venture": case "earthbend": case "goad": case "unlock-door": case "lock-or-unlock-door": case "empower-jace": case "clash": case "vote":
      case "adapt": case "monstrosity": case "bolster": case "support": case "scry": case "surveil": case "blight": case "explore": case "connive": case "regenerate":
        if ("what" in effect) objectPaths.push(child(path, "what"));
        if ("who" in effect && effect.who) playerPaths.push(child(path, "who"));
        break;
      case "optional-group": case "choice": case "villainous-choice": case "simultaneous": case "modal": case "create-delayed-trigger": case "create-reflexive-trigger": case "create-replacement": case "roll-dice": case "roll-again": case "flip-coin": case "pay-cost":
        return; // Handled above; retained for exhaustiveness.
      default: {
        const unreachable: never = effect;
        throw new Error(`Unhandled effect variant: ${JSON.stringify(unreachable)}`);
      }
    }
    const objectParticipants = objectPaths.map(path => this.at(path)).filter((candidate): candidate is Candidate => candidate !== undefined);
    const objects = objectParticipants.filter(candidate => candidate.domain === "object" || candidate.domain === "object-or-player");
    const players = combine(objectParticipants.filter(candidate => candidate.domain === "player"),
      playerPaths.map(path => this.at(path)).filter((candidate): candidate is Candidate => candidate !== undefined));
    const event = this.event(path, context, effect.effect, [...objects, ...players], {
      ...(effect.optional || context.decider ? { optional: true as const, ...(decider ? { decider } : {}) } : {}),
      ...(effect.forEach ? { repeated: "for-each" as const } : {}), ...(context.conditional ? { conditional: true as const } : {}) });
    if (effect.effect === "exile" && effect.what.ref === "library-cards" &&
        (effect.what.of?.owner === "each-player" || effect.what.of?.owner === "each-opponent")) {
      const rules = ["400.1", "400.7j", "608.2f", "609.3"];
      this.engineFeature("multi-library-exile", path, "Process each eligible library separately and capture only cards actually moved to exile.", rules);
    }
    if (immediateCasts && castPool) {
      const limit = this.at(`${path}/instruction/what`);
      const pool = this.at(`${path}/instruction/fromAmong`);
      if (pool?.id === castPool.id && limit) {
        const rules = ["400.7j", "608.2d", "608.2g"];
        this.node({ id: this.id(path, "sequential-cast"), path, node: "sequential-cast", scope: context.ability,
          event, pool: pool.id, caster: context.you, limit: limit.id, rules });
        this.engineFeature("sequential-cast", path, "Let the caster cast eligible members one at a time during resolution and stop at any time.", rules);
      } else if (pool) {
        this.diagnostic("unsupported-scope", path, "The immediate casts need the exact captured exile set.", ["400.7j", "608.2g"]);
      }
    }
    if (effect.effect === "exile" && context.linkedExile?.path === path) {
      const rules = ["400.7", context.linkedExile.rule, "607.3", "607.5"];
      this.node({ id: this.id(path, "linked-exile-pool"), path, node: "linked-exile-pool", scope: context.ability,
        source: context.source, producer: context.ability, event, membership: "still-in-exile",
        identity: "source-incarnation", cardinality: "many", rules });
      this.engineFeature("linked-exile-pool", path, "Record only cards exiled by this linked ability instance and still in exile.", rules);
    }
    if (effect.effect === "become" && "copyOf" in effect.spec) {
      const model = this.at(`${path}/spec/copyOf`);
      if (model && model.cardinality === "one" && this.announced(model)) {
        context.objects = combine(context.objects, [model]);
        context.mentionedObject = true;
      }
    }
    if (effect.effect === "declare-target" && objects.length === 1 && this.announced(objects[0]))
      context.declaredTargets = [...(context.declaredTargets ?? []), objects[0]];
    if (effect.effect === "damage" || effect.effect === "lose-life")
      context.measuredEvents = [...(context.measuredEvents ?? []), { operation: effect.effect, event }];
    const preEntryChoice = context.entryChoice?.path === path;
    if (effect.effect === "choose-value" && !context.barrier && players[0]?.cardinality === "one" &&
        (preEntryChoice || (context.resolutionChoice?.path === path && context.phase === "resolution"))) {
      this.node({ id: this.id(path, "value-choice"), path, node: "value-choice", scope: context.ability,
        event, chooser: players[0].id, category: effect.category,
        ...(effect.category === "card-type" && effect.within === "permanent-types" ? { within: effect.within } : {}),
        lifetime: preEntryChoice ? { kind: "source-incarnation", source: context.source, lookup: "current-or-last-known" } : { kind: "resolution" },
        rules: preEntryChoice ? ENTRY_CHOICE_RULES : ["607.1c", "607.2d", "608.2d"] });
      if (!preEntryChoice) context.availableChoice = context.resolutionChoice;
    }
    if (effect.optional) context.decisions?.push(path);
    if (effect.effect === "vote") context.votes = [...(context.votes ?? []), event];
    if (objects.length || produces === "objects") context.mentionedObject = true;
    if (objects.some(object => object.id === context.source || this.sourceAliases.has(object.id))) context.sourceNamed = true;
    // Retain all distinct established participants. Replacing this set with
    // the most recently mentioned noun would silently guess an antecedent.
    if (objectPaths.length) context.objects = combine(context.objects, objects);
    if (playerPaths.length) context.players = combine(context.players, players.filter(player => player.id !== context.you));
    // "Search target player's library ... Then that player shuffles".
    context.players = combine(context.players, this.targetPlayersUnder(path, context));
    for (const [index, object] of objects.entries()) {
      const related = this.relationPlayer(object, child(path, `participant-${index}`), context);
      if (related) context.players = combine(context.players, [related]);
    }
    if (produces) {
      if (produces === "objects" && objects.length === 1 && identity !== "new-objects") cardinality = objects[0].cardinality;
      // "... for each Elf you control": how many objects result is counted on resolution.
      if (effect.forEach && produces === "objects") cardinality = "unknown";
      const perPlayer = players.some(player => player.cardinality !== "one");
      if (perPlayer) cardinality = "unknown";
      // What the result is known to be: a card in its new zone, a permanent on
      // the battlefield, a token, or cards from a library (CR 110.1, 111.1).
      const destination = effect.effect === "move-zone" ? effect.to.zone : undefined;
      const established: ObjectFilter | undefined = identity === "zone-successors"
        ? { ...(objects[0]?.description ?? {}), cls: destination === "battlefield" ? "permanent" : "card" }
        : effect.effect === "manifest-dread" ? { cls: "permanent", types: ["creature"] }
        : effect.effect === "create-token" ? { cls: "token", ...("types" in effect.token && effect.token.types.length ? { types: effect.token.types as NonNullable<ObjectFilter["types"]> } : {}) }
        : ["look-at", "mill", "draw", "reveal"].includes(effect.effect) ? { cls: "card" } : undefined;
      const result = this.eventResult(path, event, produces, cardinality, identity, established);
      if (effect.effect === "exile") {
        const fromPool = effect.what.ref === "from-pool" ? this.at(`${path}/what/pool`) : undefined;
        if (objects[0]?.description?.cls === "card" || fromPool?.description?.cls === "card")
          context.exileResults = [...(context.exileResults ?? []), { event, result }];
        else context.uncertainExileOrigin = true;
      } else if (effect.effect === "exile-until" || effect.effect === "airbend" ||
          (effect.effect === "move-zone" && effect.to.zone === "exile")) context.uncertainExileOrigin = true;
      // Each player's action is its own event (CR 101.4); a later reference to
      // their results would need a per-player aggregate. Only such a reference
      // is unresolved; the instruction itself is not.
      if (perPlayer) this.invalidNodes.set(result.id, "unsupported-scope");
      if (produces === "objects") {
        let successor = result;
        if (identity === "zone-successors") {
          // The successor stands for the moved object when the effect acts on
          // it again (CR 400.7j); asked about as it was, the moved object is
          // known by its last-known information (CR 608.2h).
          const moved = new Set(objects.map(object => object.id));
          if (context.sourceSuccessor && moved.has(context.sourceSuccessor.id) && moved.size === 1) context.sourceSuccessor = result;
          // A card that came from a library or hand has no information there
          // to look back at; the effect uses the moved card's (CR 608.2h).
          const hidden = effect.effect === "discard" || /"zone":"(library|hand)"|"top":/.test(JSON.stringify(this.result.nodes.find(node => node.id === objects[0]?.id) ?? {}));
          if (effect.effect === "discard") successor = { ...result, triggerSuccessor: true };
          if (!hidden) context.departed = combine(context.departed ?? [], objects.filter(object => object.domain === "object"));
          if (moved.size === 1 && !hidden) successor = { ...result, successorOf: objects[0].id };
          context.objects = context.objects.filter(object => !moved.has(object.id));
        }
        context.objects = combine(context.objects, [successor]);
      }
      else {
        if (echoes(effect)) this.echoQuantities.add(result.id);
        context.quantities = combine(context.quantities, [result]);
      }
    }
    // "That many" after an instruction is how many objects or counters it
    // affected ("Discard all the cards in your hand, then draw that many cards").
    if (["draw", "mill", "create-token", "put-counters", "remove-counters", "lose-counters"].includes(effect.effect)
      || (["discard", "sacrifice", "destroy", "exile", "move-zone", "return-to-hand", "shuffle-into-library"].includes(effect.effect)
        && objects.some(object => object.cardinality !== "one"))) {
      const quantity = this.eventResult(path, event, "quantity", "one", "measured-quantity");
      if (echoes(effect)) this.echoQuantities.add(quantity.id);
      if (players.some(player => player.cardinality !== "one")) this.invalidNodes.set(quantity.id, "unsupported-scope");
      context.quantities = combine(context.quantities, [quantity]);
    }
    if (["exile", "exile-until", "move-zone", "return-to-hand", "destroy", "counter", "sacrifice", "discard", "airbend", "manifest", "cloak", "shuffle-into-library"].includes(effect.effect)) {
      if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
      if (objects.some(object => object.id === context.sourceSuccessor?.id)) context.sourceSuccessor = undefined;
      else if (objects.some(object => object.id === context.source)) context.sourceSuccessor = undefined;
      if (identity !== "zone-successors") {
        // Hidden destinations and implicit/replaced zone moves need a
        // separate successor/LKI model. Never retain the old target as "it".
        const moved = new Set(objects.map(object => object.id));
        context.departed = combine(context.departed ?? [], context.objects.filter(object => moved.has(object.id)));
        context.objects = context.objects.filter(object => !moved.has(object.id));
      }
    }
  }
  trigger(trigger: Trigger, path: string, context: Context): void {
    this.triggerPattern(trigger, path, context);
    // After the trigger, "another" may mean the object it matched.
    // The source as the trigger's subject ("When this creature enters") is not another object.
    if (namesObject(trigger, false)) context.mentionedObject = true;
    // A cast or activation trigger is about a spell or ability; a bare "it"
    // means that, unless the trigger names this object too ("a spell that
    // targets this creature, put a +1/+1 counter on it").
    if (context.triggerObject && !/"ref":"(self|this)"/.test(JSON.stringify(trigger))) context.spellPronoun = true;
  }
  counterChange(trigger: Extract<Trigger, { trigger: "counter-change" }>, path: string, context: Context): boolean {
    const { on, match, by } = trigger;
    const recipients = on.recipient === "any-of" ? on.recipients : [on];
    if (context.barrier || !("recipients" in match) || match.recipients !== "each" ||
        (by && by.player !== "you") || !recipients.every(recipient => recipient.recipient === "object"
          ? replacementObject(recipient.what) || recipient.what.ref === "this" || recipient.what.ref === "self"
          : replacementPlayer(recipient.who))) return false;
    // CR 122.6a: the actor is independent of the recipient. Each matching
    // recipient gets its own occurrence; the count is of the actual change.
    const scope: Context = { ...context, phase: "trigger-match", eventPredicate: true };
    this.walk(trigger, path, scope);
    const candidates = recipients.flatMap((recipient, index) => {
      const base = on.recipient === "any-of" ? `${path}/on/recipients/${index}` : child(path, "on");
      const candidate = this.at(child(base, recipient.recipient === "object" ? "what" : "who"));
      return candidate ? [candidate] : [];
    });
    const actor = by ? this.at(child(path, "by")) : undefined;
    if (candidates.length !== recipients.length || (by && !actor)) return false;
    const event = this.event(path, context, "trigger:counter-change", [...candidates, ...(actor ? [actor] : [])]);
    const rules = ["122.2", "122.6", "122.6a", "603.2c", "603.10", "700.1"];
    this.node({ id: this.id(path, "counter-change"), path, node: "counter-change-match", scope: context.ability,
      event, pattern: trigger, recipients: candidates.map(candidate => candidate.id), ...(actor ? { actor: actor.id } : {}), rules });
    this.engineFeature("counter-change-event", path,
      "Match actual counter changes by kind, actor and recipient. Partition each event by recipient and apply the printed counter grouping.", rules);
    context.quantities = [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")];
    const [candidate] = candidates;
    if (on.recipient === "object") {
      const result = this.eventResult(path, event, "objects", "one", "same-objects", candidate.description);
      context.objects = [result];
      if (candidate.id === context.source) this.sourceAliases.add(result.id);
    } else if (on.recipient === "player" && candidate.id !== context.you) {
      context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
    } else if (on.recipient === "any-of") {
      // A mixed event does not make an object or player available in every outcome.
      context.mentionedObject = true;
    }
    return true;
  }
  triggerPattern(trigger: Trigger, path: string, context: Context): void {
    if (trigger.trigger === "player-attacks" && !context.barrier && trigger.per === "declaration" &&
        trigger.who.player === "you" && trigger.whom &&
        (trigger.whom.player === "a-player" || trigger.whom.player === "the-initiative-holder") &&
        !trigger.whomPermanents && !trigger.whomGrouping && !trigger.totalPower &&
        (!trigger.count || trigger.count.value.amount === "fixed")) {
      const match: Context = { ...context, phase: "trigger-match", eventPredicate: true };
      this.walkChildren(trigger as unknown as Record<string, unknown>, path, match);
      const actor = this.at(child(path, "who"));
      const defender = this.at(child(path, "whom"));
      if (actor && defender && defender.cardinality === "one") {
        const rules = ["508.1k", "508.3d", "603.2c", "726.1"];
        const event = this.event(path, context, "trigger:player-attacks", [actor, defender]);
        this.node({ id: this.id(path, "attack-declaration"), path, node: "attack-declaration", scope: context.ability,
          event, actor: actor.id, defender: defender.id, ...(trigger.with ? { filter: trigger.with } : {}),
          ...(trigger.count ? { count: trigger.count } : {}), rules });
        this.engineFeature("attack-declaration", path, "Match the one qualifying declaration, including its stated defender and attacker predicate.", rules);
        context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
        return;
      }
    }
    if (trigger.trigger === "counter-change") {
      if (this.counterChange(trigger, path, context)) return;
      const barrier = "Counter-change triggers need per-counter/per-recipient/group occurrence, placing/removing actor, and event quantity/recipient captures bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["122.2", "122.6", "122.7", "603.2c"]);
      this.walk(trigger, path, { ...context, phase: "trigger-match", barrier });
      return;
    }
    if (trigger.trigger === "event-gated") {
      const barrier = "Trigger event-time conditions need the matching event's participant, source snapshot, and branch-local gate bindings.";
      this.diagnostic("unsupported-scope", path, barrier, ["603.1", "603.4", "603.10"]);
      this.walk(trigger, path, { ...context, phase: "trigger-match", barrier });
      return;
    }
    if (trigger.trigger === "becomes-class-level") {
      if (!context.barrier && context.classLevel === trigger.level &&
          (trigger.what.ref === "self" || trigger.what.ref === "this")) {
        const what = this.object(trigger.what, child(path, "what"), { ...context, phase: "trigger-match" });
        if (what) {
          this.event(path, context, "trigger:becomes-class-level", [what]);
          return;
        }
      }
      const barrier = "A Class-level trigger needs the source incarnation's designation transition and granted-ability availability bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["716.2", "603.2"]);
      this.walk(trigger, path, { ...context, barrier });
      return;
    }
    if (trigger.trigger === "roll-dice") {
      const barrier = "Dice triggers need per-roll/group occurrence, ignored-result exclusion, natural/final stage, and result capture scopes.";
      this.diagnostic("unsupported-scope", path, barrier, ["706.2", "706.6", "706.7"]);
      this.walk(trigger, path, { ...context, barrier });
      return;
    }
    // A trigger pattern denotes the object/player of the matching occurrence,
    // not every object satisfying its filter at resolution (CR 603.2c).
    if (trigger.trigger === "any-of") {
      // One ability triggers on whichever printed event occurs (CR 603.2).
      // Each alternative binds on its own; what the effect may refer to is
      // what every alternative establishes alike ("Whenever this creature
      // attacks or blocks, ... it"). If any alternative moves the source, it
      // may have left its zone (CR 603.10a).
      const branches = trigger.events.map((event, index) => {
        const branch: Context = { ...context, objects: [], players: [], quantities: [], departed: [] };
        this.triggerPattern(event, child(child(path, "events"), index), branch);
        return { branch, subject: "what" in event && event.what ? this.at(child(child(child(path, "events"), index), "what")) : undefined,
          who: "who" in event && event.who ? this.at(child(child(child(path, "events"), index), "who")) : undefined };
      });
      context.sourceMoved = context.sourceMoved || branches.some(({ branch }) => branch.sourceMoved);
      const [first] = branches;
      // An alternative with another participant ("blocks or becomes blocked
      // by a creature") makes "that creature" a different object per event;
      // that needs a discriminated binding, so nothing is established.
      // "Blocks or becomes blocked by a creature": each alternative has one
      // other creature, described alike; "that creature" is the one the event
      // that occurred established.
      const others = branches.map(({ branch }) => branch.objects.filter(candidate => !this.sourceAliases.has(candidate.id)));
      if (!context.barrier && branches.length > 1 && branches.every(({ branch }) => branch.sourceSuccessor?.cardinality === "one")) {
        // Each disjunct moves this same source to a public zone. The
        // resulting object depends on which trigger event actually occurred.
        const rules = ["400.7e", "603.2", "603.6c"];
        const id = this.node({ id: this.id(path, "successors"), path, node: "alternatives", scope: context.ability,
          of: branches.map(({ branch }) => branch.sourceSuccessor!.id), rules });
        const successor: Candidate = { id, domain: "object", cardinality: "one", description: { cls: "card" },
          successorOf: context.source, triggerSuccessor: true };
        context.sourceSuccessor = successor;
        context.objects = [successor];
      } else if (!context.sourceMoved && branches.length > 1 && others.every(list => list.length === 1 && list[0].cardinality === "one")
        && others.every(list => JSON.stringify(list[0].description) === JSON.stringify(others[0][0].description))) {
        const id = this.node({ id: this.id(path, "alternatives"), path, node: "alternatives", scope: context.ability, of: others.map(list => list[0].id), rules: ["603.2"] });
        context.objects = [{ id, cardinality: "one", domain: "object", description: others[0][0].description }];
        context.mentionedObject = true;
      } else if (branches.some(({ branch }) => branch.objects.length > 1)) context.mentionedObject = true;
      else if (!context.sourceMoved && first.subject && branches.every(({ subject }) => subject?.id === first.subject!.id && subject.cardinality === "one"))
        context.objects = [first.subject];
      if (first.who && branches.every(({ who }) => who?.id === first.who!.id) && first.who.id !== context.you)
        context.players = [first.who];
      return;
    }
    if (trigger.trigger === "phase-in-turn" && context.delayedSchedule && !context.barrier &&
        isDeepStrictEqual(trigger.turn, { turn: "next" })) {
      const rules = ["500.7", "603.7b", "614.10a"];
      this.node({ id: this.id(path, "scheduled-phase"), path, node: "scheduled-phase",
        scope: context.ability, phase: trigger.phase, turn: "next", rules });
      this.engineFeature("phase-scheduling", path, "Anchor the delayed trigger to the stated phase of the next turn.", rules);
      return;
    }
    const match = { ...context, phase: "trigger-match" as const };
    if (!context.barrier && trigger.trigger === "cast" && trigger.from?.owner === "their" &&
        (trigger.who?.player === "a-player" || trigger.who?.player === "an-opponent")) {
      // The origin is the casting player's zone at the matched occurrence.
      // Another trigger or a later pronoun cannot supply this role (CR 603.2c).
      match.actor = this.player(trigger.who, child(path, "who"), { ...match, eventPredicate: true });
      for (const [key, value] of Object.entries(trigger)) if (key !== "who") this.walk(value, child(path, key), match);
    } else this.walk(trigger, path, match);
    const objects: Candidate[] = [];
    if ("what" in trigger && trigger.what && "ref" in trigger.what) {
      const what = this.at(child(path, "what"));
      if (what) objects.push(what);
    }
    const who = "who" in trigger ? this.at(child(path, "who")) : undefined;
    const recipient = "to" in trigger && trigger.to && trigger.to !== "any" ? this.at(child(path, "to")) : undefined;
    const event = this.event(path, context, `trigger:${trigger.trigger}`, [...objects, ...(who ? [who] : [])]);
    const stackEvent = uniqueStackTriggerEvent(trigger);
    const eventObject = objects.length ? this.eventResult(path, event, "objects",
      trigger.grouping === "one-or-more" ? "many" : "one", "same-objects", objects[0].description)
      : stackEvent ? this.eventResult(path, event, "objects", "one", "same-objects",
        stackEvent === "cast" ? { cls: "spell" } : { cls: "ability", abilityKind: "activated", manaAbility: false }) : undefined;
    // Preserve the older ordinary-pronoun policy. A filter-only cast trigger
    // establishes this explicit event role, not a generic latest-object alias.
    if (objects.length && eventObject) context.objects = [eventObject];
    // A trigger whose subject is the source or another definite description
    // ("equipped creature") names that object explicitly when it means it.
    if (eventObject && "what" in trigger && trigger.what && "ref" in trigger.what
      && ["self", "this", "equipped", "enchanted", "fortified"].includes(trigger.what.ref)) this.sourceAliases.add(eventObject.id);
    if (stackEvent && eventObject) context.triggerObject = { event: stackEvent, candidate: eventObject };
    if (["attacks", "attacks-and-isnt-blocked", "becomes-blocked"].includes(trigger.trigger) && eventObject?.cardinality === "one") context.attacker = eventObject;
    if (recipient) {
      if (recipient.domain === "player") context.players = combine(context.players, [recipient]);
      else context.objects = combine(context.objects, [recipient]);
    }
    // "Blocks a creature", "becomes blocked by a creature", "becomes unattached
    // from a permanent": the other object of the event.
    for (const key of ["by", "blocking", "from"] as const) {
      const other = key in trigger && isRecord((trigger as Record<string, unknown>)[key]) && "ref" in ((trigger as Record<string, unknown>)[key] as object)
        ? this.at(child(path, key)) : undefined;
      if (other?.domain === "object" && other.cardinality === "one") context.objects = combine(context.objects, [other]);
    }
    const related = !who && eventObject ? this.relationPlayer(eventObject, path, context) : undefined;
    if (related) context.players = combine(context.players, [related]);
    // "That player" never means the player the text calls "you".
    if (who && who.id !== context.you) context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
    // "deals damage to a player" (one occurrence per damaged player), "at the
    // beginning of each player's upkeep": "that player" is the event's player.
    else if ((trigger.trigger === "deals-damage" && trigger.to === "any" && !trigger.recipients)
      || (trigger.trigger === "phase" && (trigger.whose === "each-player" || trigger.whose === "each-opponent")))
      context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
    if ((trigger.trigger === "taps-for-mana" && who?.id !== context.you &&
        (trigger.who?.player === "a-player" || trigger.who?.player === "an-opponent")) ||
        (trigger.trigger === "phase" && trigger.phase === "draw-step" && trigger.whose === "each-player")) {
      if (context.players.length === 1 && context.players[0].cardinality === "one") context.definiteEventPlayer = context.players[0];
    }
    // "Becomes the target of a spell or ability ... counter that spell": the
    // spell or ability that targeted it.
    if (trigger.trigger === "becomes-target" && "of" in trigger && isRecord(trigger.of) && !("ref" in trigger.of))
      context.objects = combine(context.objects, [{ ...this.eventResult(child(path, "of"), event, "objects", "one", "same-objects", trigger.of as ObjectFilter), qualifiedOnly: true }]);
    // "Put into an opponent's graveyard", "returned to a player's hand": that
    // player is the card's owner (CR 400.3, 404.2).
    if (eventObject && eventObject.cardinality === "one" && "to" in trigger && isRecord(trigger.to) && "owner" in trigger.to && trigger.to.owner !== "your"
      && (trigger.to.owner === "an-opponent" || (isRecord(trigger.to.owner) && trigger.to.owner.player === "a-player"))) {
      const id = this.id(child(path, "to"), "owner-of-described");
      this.node({ id, path: child(path, "to"), node: "derived-player", scope: context.ability, role: "owner", of: eventObject.id, rules: ["108.3", "400.3"] });
      this.weakPlayers.add(id);
      context.players = combine(context.players, [{ id, cardinality: "one", domain: "player" }]);
    }
    // "At the beginning of enchanted player's upkeep, that player ...", "a
    // creature enchanted player controls": the player the trigger names.
    for (const at of [child(path, "whose"), child(child(child(path, "what"), "filter"), "controlledBy")]) {
      const named = this.at(at);
      if (named?.domain === "player" && named.cardinality === "one" && named.id !== context.you) context.players = combine(context.players, [named]);
    }
    if (["deals-damage", "is-dealt-damage", "player-dealt-damage", "gains-life", "loses-life", "pays-life"].includes(trigger.trigger))
      context.quantities = [this.eventResult(path, event, "quantity", "one", "measured-quantity")];
    // "Whenever one or more Dragons you control attack, draw that many cards":
    // how many objects the event had.
    else if ("grouping" in trigger && trigger.grouping === "one-or-more" && eventObject)
      context.quantities = [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")];
    const publicDeparture = trigger.trigger === "put-into-zone" &&
      isDeepStrictEqual(trigger.from, { zone: "battlefield" }) && trigger.to.zone === "exile";
    if (publicDeparture || ["dies", "leaves", "leaves-zone", "put-into-graveyard", "enters-or-leaves", "enters-or-dies"].includes(trigger.trigger)) {
      if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
      // The moved object is a new object (CR 400.7). The ability can find the
      // object it became in the public zone it first went to (CR 400.7e,
      // 603.6c): "When this creature dies, return it to its owner's hand".
      // Its last-known information answers "if it was a Human" or "its
      // power" (CR 603.10a, 608.2h).
      context.objects = [];
      if (eventObject) {
        context.departed = combine(context.departed ?? [], [eventObject]);
        if (trigger.trigger !== "enters-or-leaves" && trigger.trigger !== "enters-or-dies") {
          const id = this.node({ id: this.id(path, "successor"), path, node: "event-result", event, result: "objects",
            cardinality: eventObject.cardinality, availability: "if-produced", identity: "zone-successors",
            description: { ...(eventObject.description ?? {}), cls: "card" }, rules: ["400.7e", "603.6c"] });
          const successor: Candidate = { id, cardinality: eventObject.cardinality, domain: "object",
            description: { ...(eventObject.description ?? {}), cls: "card" }, successorOf: eventObject.id, triggerSuccessor: true };
          context.objects = [successor];
          if (context.sourceMoved && objects.some(object => object.id === context.source)) context.sourceSuccessor = successor;
        }
      }
    }
  }
  ability(ability: Ability, path: string, face: string, source: string, cardAbilities?: Ability[], classLevel?: number): void {
    const id = this.id(path, "ability");
    const you = this.id(path, "you");
    const selfGraveyardReplacement = isSelfGraveyardReplacement(ability, cardAbilities);
    const sameTurnEntryCopy = cardAbilities !== undefined && this.permanentFaces.has(face) &&
      this.faceTypes.get(face)?.types?.includes("creature") && isSameTurnEntryCopy(ability);
    const entryCopy = sameTurnEntryCopy ? ability.effect :
      this.permanentFaces.has(face) && ability.kind === "static" && ability.effect.static === "entry-copy" && ordinaryEntryCopy(ability.effect)
        ? ability.effect : undefined;
    const role = ability.kind === "activated" || ability.kind === "loyalty" ? "activator"
      : ability.kind === "triggered" || ability.kind === "chapter" || ability.kind === "visit" || ability.kind === "case-solve" ? "trigger-controller"
      : ability.kind === "spell" ? "spell-controller" : ability.kind === "additional-cost" ? "prospective-caster"
      : selfGraveyardReplacement ? "replacement-source-controller-or-owner"
      : entryCopy ? "entering-controller" : "static-source-controller";
    this.node({ id, path, node: "ability", source, face, abilityKind: ability.kind, you, rules: ["113", "109.5"] });
    this.node({ id: you, path, node: "runtime-player", scope: id, role,
      rules: selfGraveyardReplacement ? ["108.4a", "109.5"] : entryCopy ? ["109.5", "110.2", "614.12"] : ["109.5"] });
    // An unqualified "other" excludes the permanent with the ability, including
    // an object that was given the ability; never the "source" of a spell.
    const recipientSource = this.result.nodes.find(node => node.id === source);
    const granted = recipientSource?.node === "recipient";
    const permanentSource = ability.kind !== "spell" && !(granted && recipientSource.emblem) &&
      (this.permanentFaces.has(face) || granted);
    const context: Context = { source, face, ability: id, you, permanentSource, staticAbility: ability.kind === "static",
      entryChoice: cardAbilities && permanentSource ? entryChoice(cardAbilities, path.slice(0, path.lastIndexOf("/"))) : undefined,
      resolutionChoice: resolutionChoice(ability, path), lexicalAbilityPath: path, classLevel,
      linkedExile: cardAbilities && permanentSource ? linkedExileOrigin(cardAbilities, path.slice(0, path.lastIndexOf("/"))) : undefined,
      spellAbility: ability.kind === "spell",
      // "This creature can't attack unless defending player controls an
      // Island", "This creature can't be blocked as long as ...": a rule about
      // the creature's own attack (CR 508.5).
      attackingSource: ability.kind === "static" && ability.effect.static === "cant" && isRecord(ability.effect.what)
        && (ability.effect.what.ref === "this" || ability.effect.what.ref === "self")
        && ["attack", "be-blocked"].includes(String((ability.effect as { action?: unknown }).action)),
      phase: ability.kind === "static" || ability.kind === "keywords" || ability.kind === "class-level-bar" ? "continuous" : "resolution", objects: [], players: [], quantities: [] };
    if ((ability.kind === "activated" && namesObject(ability.costs, false)) || (ability.kind === "static" && namesObject(ability.effect, true)))
      context.mentionedObject = true;
    // X a player announced (CR 107.3a): for an activated ability with X in
    // its cost (107.3k), a spell with X in its costs, or a permanent's own
    // entry from such a spell (107.3m). Text that defines X wins (107.3c); a
    // granted ability has no announced X (107.3j).
    if (!granted && !definesX(ability)) {
      const entering = (ability.kind === "triggered" && ability.trigger.trigger === "enters" && "what" in ability.trigger
          && isRecord(ability.trigger.what) && ["self", "this"].includes(String(ability.trigger.what.ref)))
        || (ability.kind === "static" && ["enters-with-counters", "as-enters", "entry-choice"].includes(String(ability.effect.static)));
      const announcement = (ability.kind === "activated" && costHasX(ability.costs)) || (ability.kind === "loyalty" && costHasX(ability.cost)) ? "activation" as const
        : ability.kind === "spell" && this.xFaces.has(face) ? "cast" as const
        : entering && this.xFaces.has(face) && this.permanentFaces.has(face) ? "entering-spell" as const : undefined;
      const constraints = ability.kind === "activated" ? ability.variableConstraints : undefined;
      if (announcement && constraints?.some(constraint => !("not" in constraint))) {
        this.diagnostic("unsupported-number-binding", child(path, "variableConstraints"),
          "An announcement constraint needs its own earlier bound value.", ["107.3a", "601.2b"]);
      } else if (announcement) context.announcedX = this.node({ id: this.id(path, "announced-x"), path, node: "announced-number", scope: id, chooser: you, announcement,
        ...(constraints?.length ? { constraints } : {}), rules: announcement === "activation" ? ["107.3a", "107.3k", "602.2b"] : announcement === "cast" ? ["107.3a", "601.2b"] : ["107.3m"] });
    }
    switch (ability.kind) {
      case "activated": case "loyalty": {
        const payment = { ...context, phase: "payment" as const };
        if (ability.kind === "activated" && ability.keyword)
          this.diagnostic("unsupported-scope", child(path, "keyword"),
            ability.keyword === "forecast"
              ? "Forecast requires the source's hand/owner/upkeep bindings, activation history, and reveal lifetime."
              : "Intrinsic activated-keyword rules require the source incarnation's activation, attack, or entry history and payment bindings.",
            ability.keyword === "forecast" ? ["702.57"] : ["702"]);
        const costs = child(path, ability.kind === "activated" ? "costs" : "cost");
        const solePayment = ability.kind === "activated" ? soleObjectPayment(ability.costs) : -1;
        if (ability.kind === "activated" && ability.costs[solePayment]?.cost === "sacrifice")
          payment.sourceExclusionPath = `${costs}/${solePayment}/what/filter`;
        this.walk(ability.kind === "activated" ? ability.costs : ability.cost, costs, payment);
        this.walk(ability.restriction, child(path, "restriction"), payment);
        this.walk(ability.permission, child(path, "permission"), payment);
        this.costModifications(ability.costModifications, child(path, "costModifications"), payment);
        context.sourceMoved = payment.sourceMoved;
        // Objects the cost selected are the effect's antecedents ("Sacrifice a
        // creature: You gain life equal to its toughness"); a sacrificed,
        // exiled, discarded or returned one is known by its last-known
        // information (CR 608.2h).
        // "Remove all charge counters from this artifact: It deals that much
        // damage": how much a cost paid is known to the effect (CR 601.2h, 602.2b).
        if (ability.kind === "activated") ability.costs.forEach((cost, index) => {
          if (!["pay-life", "remove-counters", "discard", "exile", "sacrifice"].includes(cost.cost)) return;
          const at = child(child(path, "costs"), index);
          const event = this.event(at, payment, `cost:${cost.cost}`, []);
          context.quantities = combine(context.quantities, [this.eventResult(at, event, "quantity", "one", "measured-quantity")]);
        });
        if (ability.kind === "activated") ability.costs.forEach((cost, index) => {
          if (!("what" in cost) || !isRecord(cost.what)) return;
          const selected = this.at(child(child(child(path, "costs"), index), "what"));
          if (!selected || selected.id === context.source || selected.domain !== "object") return;
          this.costObjects.add(selected.id);
          if (index === solePayment && selected.cardinality === "one") {
            if (cost.cost === "sacrifice" && !containsEffect(ability.effects, "sacrifice")) context.sacrificedCost = selected;
            else if (cost.cost === "exile" && selected.description?.cls === "card") {
              const at = `${costs}/${index}`;
              const event = this.id(at, "event");
              const paid = this.result.nodes.find(node => node.id === event);
              if (paid?.node === "event") paid.inputs.push(selected.id);
              const result = this.eventResult(at, event, "objects", "one", "zone-successors", { cls: "card" });
              context.exileResults = [{ event, result }];
            }
          }
          if (["sacrifice", "exile", "discard", "return", "move-zone"].includes(cost.cost)) context.departed = combine(context.departed ?? [], [selected]);
          else if (["tap-objects", "untap-objects", "exert", "reveal"].includes(cost.cost)) context.objects = combine(context.objects, [selected]);
        });
        const producer = ability.kind === "activated" && ability.effects[0]?.steps[0]?.effect === "play-card"
          && cardAbilities && permanentSource
          ? hideawayProducer(cardAbilities, ability) : undefined;
        if (producer !== undefined) {
          const siblings = path.slice(0, path.lastIndexOf("/"));
          this.hideawayPlay(`${path}/effects/0/steps/0`, this.id(`${siblings}/${producer}/keywords/0`, "pool"), context);
          break;
        }
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      }
      case "triggered":
        this.trigger(ability.trigger, child(path, "trigger"), context);
        if (ability.trigger.trigger === "becomes-status" && ability.trigger.status === "monstrous" &&
            (ability.trigger.what.ref === "self" || ability.trigger.what.ref === "this") &&
            costHasX(ability.effects) && !definesX(ability)) {
          const at = child(path, "trigger");
          const event = this.id(at, "event");
          context.triggerX = this.eventResult(child(at, "monstrosity"), event, "quantity", "one", "measured-quantity").id;
          this.engineFeature("monstrosity-event", at, "Read X from the monstrosity action that made the triggering object monstrous.", ["701.37c"]);
        }
        context.interveningIf = true;
        this.namingCondition(ability.condition, child(path, "condition"), context);
        context.interveningIf = undefined;
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      case "spell": {
        const first = ability.effects[0]?.steps[0];
        if (cardAbilities?.length === 1 && first?.effect === "copy-stack-object") {
          const { abilityWord, ...paragraph } = ability;
          if (abilityWord === undefined && isDeepStrictEqual(paragraph, {
            kind: "spell", effects: [{ steps: [DIRECT_CREATURE_SPELL_COPY] }],
          })) context.directCreatureSpellCopy = first;
        }
        if (first?.effect === "shuffle" && cardAbilities?.length === 2 &&
            cardAbilities[1] === ability && isDeepStrictEqual(cardAbilities[0], { kind: "keywords", keywords: [{ keyword: "demonstrate" }] }) &&
            isRevealAndCast(ability)) {
          this.revealAndCast(ability.effects, child(path, "effects"), context);
          return;
        }
        const previous = this.spellParagraph?.face === face ? this.spellParagraph.context : undefined;
        if (previous) Object.assign(context, { objects: previous.objects, players: previous.players, quantities: previous.quantities,
          departed: previous.departed, sourceMoved: previous.sourceMoved, mentionedObject: previous.mentionedObject, sourceNamed: previous.sourceNamed,
          exileResults: previous.exileResults, uncertainExileOrigin: previous.uncertainExileOrigin,
          stackCopies: previous.stackCopies, previousEffect: previous.previousEffect, uncertainStackOrigin: previous.uncertainStackOrigin,
          scans: previous.scans, uncertainScanOrigin: previous.uncertainScanOrigin,
          copiedCards: previous.copiedCards, uncertainCardCopyOrigin: previous.uncertainCardCopyOrigin,
          measuredEvents: previous.measuredEvents, uncertainMeasureOrigin: previous.uncertainMeasureOrigin,
          declaredTargets: previous.declaredTargets, uncertainTargetOrigin: previous.uncertainTargetOrigin });
        this.sentences(ability.effects, child(path, "effects"), context);
        this.spellParagraph = { face, context };
        return;
      }
      case "chapter": this.sentences(ability.effects, child(path, "effects"), context); break;
      case "static":
        if (entryCopy) {
          this.bindEntryCopy(entryCopy, child(path, "effect"), context);
          break;
        }
        if (selfGraveyardReplacement) {
          this.selfGraveyardReplacement(`${path}/effect/replacement`, context);
          break;
        }
        if (ability.effect.static === "replacement") {
          const at = `${path}/effect/replacement`;
          if (this.quantityReplacement(ability.effect.replacement, at, context) ||
              this.keepUnspentMana(ability.effect.replacement, at, context) ||
              this.additionalStackCopy(ability.effect.replacement, at, context) ||
              this.exileReplacement(ability.effect.replacement, at, context)) break;
        }
        // "..., where X is ...": the static ability's own definition (CR 107.3c).
        if ("bindings" in ability.effect && Array.isArray(ability.effect.bindings)
          && !["entry-choice", "enters-with-counters", "enters-tapped", "as-enters"].includes(String(ability.effect.static)))
          this.definitions(ability.effect.bindings as Sentence["bindings"], child(child(path, "effect"), "bindings"), context);
        this.walk(ability.effect, child(path, "effect"), context);
        if (ability.effect.static === "replacement") this.diagnostic("unsupported-scope", path,
          "Replacement-event roles and captures need a separate scope.", ["614"]);
        break;
      case "keywords": ability.keywords.forEach((keyword, index) => this.keyword(keyword, child(child(path, "keywords"), index), context)); break;
      case "additional-cost": {
        const payment = { ...context, phase: "payment" as const };
        // Additional costs and the spell paragraph use one announced X on
        // the stack. Do not share it with activated or triggered abilities.
        const spells = cardAbilities?.flatMap((entry, index) => entry.kind === "spell" ? [{ entry, index }] : []);
        if (!ability.optional && !ability.variableConstraints?.length &&
            costHasX(ability.costs) && spells?.length === 1 && !definesX(spells[0].entry)) {
          const spellPath = `${path.slice(0, path.lastIndexOf("/"))}/${spells[0].index}`;
          payment.announcedX = this.id(spellPath, "announced-x");
        }
        this.walk(ability.costs, child(path, "costs"), payment);
        this.costModifications(ability.costModifications, child(path, "costModifications"), payment);
        break;
      }
      case "exert-on-attack": {
        // The linked "When you do" ability is a separate reflexive trigger
        // whose "it" is the exerted object; its capture is not bound here.
        const barrier = "A linked reflexive exert trigger needs the exert cost's capture and its own ability scope bound.";
        this.diagnostic("unsupported-scope", path, barrier, ["701.43d", "607.2h"]);
        this.walk(ability.what, child(path, "what"), { ...context, phase: "payment", barrier });
        this.sentences(ability.reflexive ?? [], child(path, "reflexive"), { ...context, barrier });
        break;
      }
      case "linked-triggers": {
        const barrier = "Linked triggered abilities need the static ability's reveal events bound as their \"this way\".";
        this.diagnostic("unsupported-scope", path, barrier, ["603.11", "607.2a"]);
        this.walk(ability.static, child(path, "static"), { ...context, barrier });
        this.walk(ability.triggers, child(path, "triggers"), { ...context, barrier });
        break;
      }
      case "visit":
        this.diagnostic("unsupported-scope", path, "A visit ability needs its roll-to-visit event and the printing's lit-up numbers bound; neither is in the Oracle text.", ["701.52", "702.159a", "717.1"]);
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      case "sticker":
        this.diagnostic("unsupported-scope", path, "A sticker's ability belongs to the object it is put on; that object and the ticket payment are not bound.", ["123.3c", "123.7"]);
        if (ability.sticker.sticker === "ability")
          this.walk(ability.sticker.ability, child(path, "sticker/ability"), { ...context, barrier: "Sticker ability scope is unsupported." });
        break;
      case "max-speed":
        this.diagnostic("unsupported-scope", path, "The nested ability's source and controller scope have not been bound.", ["702.179"]);
        this.walk(ability.ability, child(path, "ability"), { ...context, barrier: "Nested ability scope is unsupported." });
        break;
      case "infinity":
        this.diagnostic("unsupported-scope", path, "The nested ability applies only while the permanent is harnessed; that designation history is not bound.", ["702.186b", "701.64b"]);
        this.walk(ability.ability, child(path, "ability"), { ...context, barrier: "Nested ability scope is unsupported." });
        break;
      case "class-level-bar": {
        const metadata = this.result.nodes.find(node => node.id === face);
        if (metadata?.node === "face" && /[—-] .*\bClass\b/.test(metadata.typeLine ?? "") &&
            Number.isSafeInteger(ability.level) && ability.level >= 2 && ability.costs.length > 0 &&
            ability.abilities.every(nested => nested.kind === "static" || nested.kind === "triggered" || nested.kind === "activated")) {
          const rules = ["602.2b", "716.2a", "716.2b", "716.2d"];
          const payer = this.node({ id: this.id(path, "upgrade-activator"), path, node: "runtime-player",
            scope: id, role: "activator", rules });
          this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment", you: payer });
          const abilities = ability.abilities.map((nested, index) => {
            const at = `${path}/abilities/${index}`;
            this.ability(nested, at, face, source, undefined, ability.level);
            return this.id(at, "ability");
          });
          this.node({ id: this.id(path, "class-level"), path, node: "class-level", scope: id, source, payer,
            level: ability.level, requiresLevel: ability.level - 1, timing: "sorcery", abilities, rules });
          this.engineFeature("class-level", path, "Track the source incarnation's noncopiable level and cumulative abilities, and pay for the next level only as a sorcery.", rules);
          break;
        }
        const barrier = "A Class level bar needs intrinsic upgrade activation/payment, designation history, and cumulative granted-ability scopes bound.";
        this.diagnostic("unsupported-scope", path, barrier, ["716.2", "716.3"]);
        this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment", barrier });
        this.walk(ability.abilities, child(path, "abilities"), { ...context, barrier });
        break;
      }
      case "anchor-words": {
        const barrier = "Anchor-word abilities need the linked as-enters choice and their own ability scopes bound.";
        this.diagnostic("unsupported-scope", path, barrier, ["614.12c", "607.2m"]);
        this.walk(ability.options, child(path, "options"), { ...context, barrier });
        break;
      }
      case "case-solve": case "case-solved": {
        const barrier = "Case designation history and the distinct solve, continuous, trigger-time, or activation gate scopes are not bound.";
        this.diagnostic("unsupported-scope", path, barrier, ["719.3", "702.169"]);
        if (ability.kind === "case-solve") this.walk(ability.condition, child(path, "condition"), { ...context, barrier });
        else this.walk(ability.ability, child(path, "ability"), { ...context, barrier });
        break;
      }
      default: {
        const unreachable: never = ability;
        throw new Error(`Unhandled ability variant: ${JSON.stringify(unreachable)}`);
      }
    }
    this.spellParagraph = undefined;
  }
  /** An intervening "if" clause (CR 603.4) or a static ability's "as long as"
   * condition that names the source or what it is attached to makes that
   * object an antecedent of the effect: "if this creature has defender, it
   * loses defender", "if enchanted permanent is tapped, untap it". */
  namingCondition(condition: unknown, path: string, context: Context): void {
    const start = this.result.references.length;
    const before = context.objects.map(candidate => candidate.id);
    this.walk(condition, path, context);
    // Only the clause's subjects; not a possessive ("Drizzt's power") or a
    // filter's exclusion ("another snow permanent").
    const named = this.result.references.slice(start).flatMap(reference => {
      if (reference.binding !== "bound" || !("ref" in reference.expression)) return [];
      if (/\/(of|filter|other)(\/|$)/.test(reference.path.slice(path.length))) return [];
      const ref = reference.expression.ref;
      if (!(ref === "self" || (ref === "this" && reference.expression.noun !== "ability") || ref === "equipped" || ref === "enchanted" || ref === "fortified")) return [];
      const candidate = this.at(reference.path);
      return candidate?.domain === "object" ? [candidate] : [];
    });
    if (!named.length) return;
    context.objects = combine(context.objects, named);
    context.named = { ids: named.map(candidate => candidate.id), before };
    if (named.length === 1) context.conditionSubject = { candidate: named[0], before: [...before, ...(context.departed ?? []).map(candidate => candidate.id)] };
    if (named.some(candidate => candidate.id !== context.source)) context.mentionedObject = true;
  }
  finish(): CardBindingAnalysis {
    // A result derived from an ambiguous target cannot become a proven
    // antecedent simply because the next instruction says "it".
    const choiceIds = new Set(this.result.nodes.filter(node => node.node === "value-choice").map(node => node.id));
    for (const reference of this.result.references) {
      if (reference.binding === "bound" && "choice" in reference.expression &&
          typeof reference.expression.choice === "string" && !choiceIds.has(reference.target))
        this.invalidNodes.set(reference.target, "unresolved-dependency");
    }
    let changed = true;
    while (changed) {
      changed = false;
      for (const node of this.result.nodes) {
        const dependent = node.node === "event" ? node.inputs.some(id => this.invalidNodes.has(id)) ||
            this.result.references.some(reference => reference.path.startsWith(`${node.path}/`) && reference.binding === "unresolved") ||
            this.result.diagnostics.some(diagnostic => diagnostic.path === node.path || diagnostic.path.startsWith(`${node.path}/`))
          : node.node === "value-choice" ? this.invalidNodes.has(node.event) || this.invalidNodes.has(node.chooser)
          : node.node === "recipient-member" || node.node === "instruction-member" ? this.invalidNodes.has(node.group)
          : node.node === "characteristic-subject" && node.of ? this.invalidNodes.has(node.of)
          : node.node === "event-result" || node.node === "linked-exile-pool" ? this.invalidNodes.has(node.event)
          : node.node === "alternatives" ? node.of.some(id => this.invalidNodes.has(id))
          : node.node === "eligible-target-group" ? this.invalidNodes.has(node.template)
          : node.node === "filtered-result-group" ? this.invalidNodes.has(node.pool)
          : node.node === "derived-player" ? this.invalidNodes.has(node.of)
          : node.node === "selection" || node.node === "defined-number"
            ? this.result.references.some(reference => reference.path.startsWith(`${node.path}/`) &&
                (reference.binding === "unresolved" || this.invalidNodes.has(reference.target))) ||
              this.result.diagnostics.some(diagnostic => diagnostic.path.startsWith(`${node.path}/`))
            : false;
        if (dependent && !this.invalidNodes.has(node.id)) { this.invalidNodes.set(node.id, "unresolved-dependency"); changed = true; }
      }
    }
    this.result.references = this.result.references.map((reference): ReferenceBinding => {
      if (reference.binding !== "bound" || !this.invalidNodes.has(reference.target)) return reference;
      const code = this.invalidNodes.get(reference.target) ?? "unresolved-dependency";
      this.diagnostic(code, reference.path, "This reference depends on an unresolved identity or number definition.", ["608.2h"]);
      return { path: reference.path, expression: reference.expression, rules: reference.rules,
        binding: "unresolved", code, candidates: [reference.target] };
    });
    const bound = this.result.references.filter(reference => reference.binding === "bound").length;
    this.result.counts = { obligations: this.result.references.length, bound, unresolved: this.result.references.length - bound };
    this.result.referencesBound = this.result.counts.unresolved === 0 && this.result.diagnostics.length === 0;
    this.result.fullyBound = this.result.referencesBound && this.result.engineFeatures.length === 0;
    return this.result;
  }
}

function validFace(value: unknown): value is BindingSourceFace {
  return isRecord(value) && typeof value.name === "string" && value.name.length > 0 &&
    (value.type_line == null || typeof value.type_line === "string") &&
    (value.mana_cost == null || typeof value.mana_cost === "string");
}

export function analyzeCardBindings(input: CardBindingInput): CardBindingAnalysis {
  const analyzer = new Analyzer(typeof input.identity === "string" ? input.identity : "invalid-source-identity");
  if (typeof input.identity !== "string" || !input.identity || !Array.isArray(input.faces) || !isRecord(input.source) || !validFace(input.source) ||
      typeof input.source.layout !== "string" || !input.source.layout ||
      !(input.source.card_faces == null || (Array.isArray(input.source.card_faces) &&
        input.source.card_faces.length > 0 && input.source.card_faces.every(validFace))) ||
      (FACE_LAYOUTS.has(input.source.layout) && input.source.card_faces?.length !== 2)) {
    analyzer.diagnostic("invalid-source", "/source", "Missing or invalid source identity/layout/face metadata.", []);
    return analyzer.finish();
  }
  const source = analyzer.node({ id: analyzer.id("/source", "object"), path: "/source", node: "source",
    identity: input.identity, rules: ["201.5"] });
  const faces = input.source.card_faces ?? [input.source];
  for (const [index, metadata] of faces.entries()) {
    const path = `/faces/${index}`;
    const matches = input.faces.filter(face => face.sourceFaceIndex === index);
    if (matches.length !== 1) {
      analyzer.diagnostic("missing-face", path, "Exactly one parse result is required for every source face.", []);
      continue;
    }
    const result = matches[0].result;
    const mainTypes = (metadata.type_line ?? "").split(/—|\s-\s/u)[0];
    if (/\b(?:Artifact|Battle|Creature|Enchantment|Land|Planeswalker)\b/.test(mainTypes) && !/\b(?:Instant|Sorcery)\b/.test(mainTypes))
      analyzer.permanentFaces.add(analyzer.id(path, "face"));
    if (matches[0].result?.ok) matches[0].result.abilities.forEach((ability, abilityIndex) => {
      if (ability.kind !== "keywords") return;
      ability.keywords.forEach((keyword, keywordIndex) => {
        if (keyword.keyword !== "kicker" && keyword.keyword !== "multikicker") return;
        const list = analyzer.kickers.get(analyzer.id(path, "face")) ?? [];
        const at = `${path}/abilities/${abilityIndex}/keywords/${keywordIndex}`;
        const costs = "additionalCosts" in keyword && Array.isArray(keyword.additionalCosts) ? keyword.additionalCosts.map(cost => JSON.stringify(cost))
          : ["cost" in keyword ? JSON.stringify(keyword.cost) : ""];
        list.push({ id: analyzer.id(at, "keyword"), you: analyzer.id(at, "you"), costs });
        analyzer.kickers.set(analyzer.id(path, "face"), list);
      });
    });
    if (/\{X\}/.test(metadata.mana_cost ?? "") || (matches[0].result?.ok && matches[0].result.abilities.some(ability => ability.kind === "additional-cost" && /"X"|"amount":"x"/.test(JSON.stringify(ability.costs)))))
      analyzer.xFaces.add(analyzer.id(path, "face"));
    const types = mainTypes.toLowerCase().split(/\s+/).filter(word => ["artifact", "battle", "creature", "enchantment", "instant", "land", "planeswalker", "sorcery", "kindred"].includes(word));
    analyzer.faceTypes.set(analyzer.id(path, "face"), { types: types as NonNullable<ObjectFilter["types"]> });
    const face = analyzer.node({ id: analyzer.id(path, "face"), path, node: "face", source,
      sourceFaceIndex: index, name: metadata.name, layout: input.source.layout,
      ...(metadata.type_line != null ? { typeLine: metadata.type_line } : {}),
      ...(metadata.mana_cost != null ? { manaCost: metadata.mana_cost } : {}), rules: ["712"] });
    if (!result?.ok) {
      analyzer.diagnostic("unparsed-face", path, "A missing or failed structural parse cannot have complete reference binding.", []);
      continue;
    }
    if (result.name !== metadata.name) {
      analyzer.diagnostic("face-mismatch", path, "The parsed face name does not match source metadata.", []);
      continue;
    }
    result.abilities.forEach((ability, abilityIndex) => analyzer.ability(ability,
      child(child(path, "abilities"), abilityIndex), face, source, faces.length === 1 ? result.abilities : undefined));
  }
  for (const face of input.faces) if (!Number.isInteger(face.sourceFaceIndex) || face.sourceFaceIndex < 0 || face.sourceFaceIndex >= faces.length)
    analyzer.diagnostic("face-mismatch", "/faces", "A supplied parse references a nonexistent source face.", []);
  return analyzer.finish();
}
