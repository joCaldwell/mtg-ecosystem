// This pass resolves reference identities, not game execution or all semantic
// correctness. In particular, it never substitutes "last noun" for an
// ambiguous referent. Every unsupported scope remains a reported obligation.
// Rules: CR 109.5, 201.5, 400.7j, 601.2c, 608.2h/k, 107.3, 607.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Amount, Effect, GainedAbility, KeywordInstance, ObjectFilter, ObjectRef, PlayerRef, Sentence, Trigger } from "../ast.ts";
import { KEYWORD_SCOPES } from "./keyword-scopes.ts";
import { isPluralPlayerRef } from "../parser/refs.ts";
import { uniqueStackTriggerEvent } from "../parser/stack-copy.ts";
import type {
  BindingCardinality, BindingDiagnosticCode, BindingNode, BindingPhase, BindingSourceFace,
  CardBindingAnalysis, CardBindingInput, EngineFeature, ReferenceBinding, ReferenceExpression,
} from "./types.ts";

const RULES_URL = "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt";
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
interface Context {
  source: string; face: string; ability: string; you: string; phase: BindingPhase;
  objects: Candidate[]; players: Candidate[]; quantities: Candidate[]; x?: string;
  sourceMoved?: boolean;
  /** The object the source became in the public zone a zone-change trigger
   * saw it move to (CR 400.7e). */
  sourceSuccessor?: Candidate;
  /** The object of one lexical cast/activation occurrence, independent of
   * salient nouns in the surrounding text. Never derived from proximity. */
  triggerObject?: { event: "cast" | "activate"; candidate: Candidate };
  /** A scope whose capture/antecedent rules are not implemented. */
  barrier?: string;
  /** The object(s) an enclosing effect gives abilities to: a bound object or
   * group, an object it creates, or a player. */
  recipient?: { candidate: Candidate } | { created: true } | { unresolved: true };
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
/** Printed keyword parameters that are paid or chosen as the keyword's cost. */
const KEYWORD_PAYMENT_FIELDS = new Set(["cost", "additionalCosts", "materials", "costModifications", "restriction", "variableConstraints"]);
function child(path: string, part: string | number): string {
  return `${path}/${String(part).replace(/~/g, "~0").replace(/\//g, "~1")}`;
}
function cardCount(amount: Amount | undefined): BindingCardinality {
  return amount?.amount === "fixed" ? amount.value === 1 ? "one" : "many" : "unknown";
}
function objectCardinality(ref: ObjectRef): BindingCardinality {
  switch (ref.ref) {
    case "self": case "this": case "it": case "enchanted": case "equipped": case "fortified": case "haunted-creature": case "targeted-by": case "trigger-event-object": case "amassed-army": return "one";
    case "them": case "each": case "all": case "zone-cards": case "rest": case "vote-leaders": case "soulbond-pair": case "pile": return "many";
    case "related": case "copy-result": case "choice-result": return ref.number === "singular" ? "one" : "many";
    case "that": return ref.plural ? "many" : "one";
    case "target": case "filter": case "commander": case "library-cards": case "graveyard-cards": case "from-pool": return cardCount(ref.count);
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
function description(ref: ObjectRef): ObjectFilter | undefined {
  if ("filter" in ref) return ref.filter;
  // CR 115.4: "any target" is a creature, player, planeswalker or battle.
  if (ref.ref === "any-target") return { anyOf: [{ types: ["creature"] }, { types: ["planeswalker"] }, { types: ["battle"] }] } as ObjectFilter;
  // CR 301.5, 303.4: what an Equipment, Aura or Fortification is attached to.
  if ((ref.ref === "equipped" || ref.ref === "enchanted" || ref.ref === "fortified") && "noun" in ref && typeof ref.noun === "string"
    && ["creature", "artifact", "enchantment", "land", "planeswalker", "permanent"].includes(ref.noun))
    return ref.noun === "permanent" ? { cls: "permanent" } : { types: [ref.noun] } as ObjectFilter;
  if (ref.ref === "library-cards" || ref.ref === "graveyard-cards" || ref.ref === "zone-cards") return { cls: "card" };
  if (ref.ref === "this") {
    if (["card", "spell", "permanent", "ability"].includes(ref.noun)) return { cls: ref.noun as "card" | "spell" | "permanent" | "ability" };
    if (["creature", "artifact", "enchantment", "land", "planeswalker", "battle"].includes(ref.noun)) return { types: [ref.noun] };
  }
  return undefined;
}
function domain(expression: ReferenceExpression): Candidate["domain"] {
  if ("player" in expression || "implicit" in expression) return "player";
  if (!("ref" in expression)) return "number";
  return expression.ref === "any-target" || ("filter" in expression && (expression.filter.orPlayer || expression.filter.orOpponent)) ? "object-or-player" : "object";
}
function referenceCardinality(expression: ReferenceExpression): BindingCardinality {
  return "ref" in expression ? expression.ref === "affected-object" ? "one" : expression.ref === "those-counters" ? "many" : objectCardinality(expression)
    : "implicit" in expression || ("player" in expression && isPluralPlayerRef(expression)) ? "many" : "one";
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
  walkChildren(value: Record<string, unknown>, path: string, context: Context): void {
    for (const [key, entry] of Object.entries(value)) this.walk(entry, child(path, key), context);
  }
  walk(value: unknown, path: string, context: Context): void {
    if (Array.isArray(value)) { value.forEach((entry, index) => this.walk(entry, child(path, index), context)); return; }
    if (!isRecord(value)) return;
    if (value.recipient === "they") {
      this.unresolved({ recipient: "they" }, path, "unsupported-reference", "The ability recipient's pronoun needs an antecedent and a player/object domain; parser ordering cannot establish either.", ["109.1", "113.1"]);
      return;
    }
    if (value.ref === "affected-object") {
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
    if (value.static === "look-permission" || value.effect === "grant-look-permission"
      || value.static === "commander-eligibility" || value.static === "optional-untap"
      || value.static === "untap-during-step" || value.static === "untap-step-limit") {
      const barrier = "Persistent information queries, pregame commander eligibility, and turn-based untap choices require their own rule-specific binding scopes.";
      this.diagnostic("unsupported-scope", path, barrier, ["401.5", "502.3", "708.5", "903.3a", "113.6n"]);
      this.walkChildren(value, path, { ...context, barrier });
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
      const barrier = "An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.";
      this.diagnostic("unsupported-scope", path, barrier, ["119.3", "120.3", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // "For each evidence vote": the tally of the one vote held earlier in this ability.
    if (value.amount === "vote-count" && !(context.votes?.length === 1 && !context.barrier))
      this.diagnostic("unsupported-reference", path, "This vote count needs the one vote it tallies held earlier in the ability.", ["701.38"]);
    if (value.amount === "devotion")
      this.diagnostic("unsupported-number-binding", path, "Devotion needs the mana costs of the player's permanents evaluated at the CR 700.5a point in the layer system.", ["700.5", "700.5a"]);
    if (value.amount === "mana-spent") {
      const barrier = "Mana payment quantities need the exact originating spell's payment event, including additional costs and actual colors spent.";
      this.diagnostic("unsupported-scope", path, barrier, ["400.7d", "601.2f", "601.2h", "609.4b"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.condition === "controls-subset") {
      const barrier = "A controlled-subset existence predicate requires candidate, cardinality and pairwise characteristic scopes; it does not select or capture objects.";
      this.diagnostic("unsupported-scope", path, barrier, ["201.2b", "603.4", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    // Filtered-player counter predicates already use player()'s per-candidate
    // scope diagnostic; this covers standalone quantified conditions.
    if (value.condition === "player-counters") {
      const barrier = "A counter predicate needs its quantified player's candidate scope; counters must not be summed across opponents.";
      this.diagnostic("unsupported-scope", path, barrier, ["122.1", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.choice === "chosen-type")
      this.diagnostic("unsupported-reference", path, "This type needs a linked choice definition; its kind cannot be inferred from the filtered object's type.", ["205.3e", "607.2d"]);
    if (value.choice === "chosen-value")
      this.diagnostic("unsupported-reference", path, "This symbolic value requires the linked originating choice and its scope; a nearby choice is not proof of that association.", ["607.2d"]);
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
    if (["designation", "day-night", "completed-dungeon", "full-party", "turn-of", "player-comparison", "opponent-count"].includes(String(value.condition)))
      this.diagnostic("unsupported-scope", path, "Game and player designations, turn ownership and per-player comparisons need the participating player set and its quantifier bound.", ["102.1", "102.2", "102.3", "608.2h"]);
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
    if (value.other === true && !value.otherThan) {
      // "Other creatures you control", "another creature enters": in a
      // permanent's ability, before any other object is named, the excluded
      // object is the permanent itself. Elsewhere (a spell, or after a
      // trigger or instruction names an object) it may be that object.
      if (context.permanentSource && !context.mentionedObject && this.onlySource(context) && !context.barrier && !context.sourceMoved)
        this.bound({ ref: "self" }, child(path, "other"), context.source, ["201.5"]);
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
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (["move-zone", "put-counters", "transform", "convert", "attach", "unattach", "create-token"].includes(String(value.effect)) && value.who == null)
      this.player({ player: "you" }, child(path, "who"), context);
    if (value.cost === "tap-self" || value.cost === "untap-self")
      this.object({ ref: "self" }, child(path, "source"), context);
    if (typeof value.cost === "string") {
      this.player({ player: "you" }, child(path, "payer"), context);
      if (["choice", "collect-evidence", "waterbend", "forage", "blight"].includes(value.cost))
        this.diagnostic("unsupported-scope", path, "This cost has intrinsic payment choices and object selections requiring a distinct payment scope.", ["118", "701"]);
      if (value.cost === "mill" || (value.cost === "discard" && value.what === "hand"))
        this.player({ player: "you" }, child(path, "zone-owner"), context);
    }
    if (value.restriction === "timing" && value.turn != null) {
      if (value.turn === "your" || value.turn === "activator") this.player({ player: "you" }, child(path, "turn"), context);
      else this.diagnostic("unsupported-reference", child(path, "turn"), "This turn-owner relation needs its own scoped binding.", ["109.5"]);
    }
    if (value.activators === "opponents-of-source-controller" || value.activators === "any-player")
      this.diagnostic("unsupported-reference", path, "Activation permission requires a player set distinct from the ability's eventual activator.", ["602.2"]);
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
    const you = this.node({ id: this.id(path, "you"), path, node: "runtime-player", scope: context.ability, role: scope.role, rules: ["109.5", ...scope.rules] });
    this.node({ id: this.id(path, "keyword"), path, node: "keyword-ability", scope: context.ability, keyword: instance.keyword,
      abilities: scope.abilities, you, rules: scope.rules });
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
    else if (recipientKey) {
      const candidate = this.at(child(path, recipientKey));
      context.recipient = candidate ? { candidate } : { unresolved: true };
      // "Put a card an opponent owns from exile into that player's graveyard".
      const related = instruction && candidate && context.phase === "resolution" ? this.relationPlayer(candidate, child(path, "participant-0"), context) : undefined;
      if (related) context.players = combine(context.players, [related]);
      // "Creatures target opponent controls don't untap during their next untap step".
      if (instruction) context.players = combine(context.players, this.targetPlayersUnder(child(path, recipientKey), context));
    }
    for (const [entry, nested] of Object.entries(value)) if (!first.includes(entry)) this.walk(nested, child(path, entry), context);
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
        ? this.node({ id: this.id(path, "recipient"), path, node: "recipient", scope: context.ability, created: true, rules: ["111.1", "113.10"] })
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
    if (expression.ref === "card-extraction-group") context = { ...context, barrier: "Separate card-extraction slots require distinct allocation, shared-pool, branch-choice and aggregate result bindings." };
    this.walkChildren(expression as unknown as Record<string, unknown>, path, context);
    if ("groupConstraints" in expression && expression.groupConstraints?.length) {
      this.unresolved(expression, path, "unsupported-scope", "Collective selection restrictions need a selected-group scope and legality binding; per-candidate eligibility alone is insufficient.", ["201.2b", "601.2c", "608.2d"]);
      return undefined;
    }
    if (context.barrier) {
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
        this.unresolved(expression, path, "unsupported-reference", "This chosen object needs its exact announcement, resolution/search or linked-choice result; proximity does not establish the origin.", ["601.2c", "608.2d", "607.2d"]);
        return undefined;
      case "copy-result":
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
    this.walkChildren(expression as unknown as Record<string, unknown>, path, context);
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
      return this.antecedent(expression, path, strong.length ? strong : context.players, "unknown", ["608.2k"]);
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
    }
    this.node({ id, path, node: "selection", scope: context.ability, expression, cardinality,
      timing: targeted ? "stack-announcement" : context.phase,
      selection: targeted ? "target" : "evaluate-reference", rules: targeted ? ["601.2c"] : ["608.2h"] });
    const candidate = this.bound(expression, path, id, targeted ? ["601.2c"] : ["608.2h"]);
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
    const established = node?.node === "event-result" ? node.description
      : node?.node === "selection" && "ref" in node.expression ? description(node.expression)
      : "ref" in binding.expression && binding.expression.ref !== "affected-object" && binding.expression.ref !== "those-counters" ? description(binding.expression) : undefined;
    return { id: binding.target, cardinality, domain: domain(binding.expression), description: established };
  }
  event(path: string, context: Context, operation: string, inputs: Candidate[],
    extra: { optional?: true; decider?: string; repeated?: "for-each"; conditional?: true } = {}): string {
    return this.node({ id: this.id(path, "event"), path, node: "event", scope: context.ability,
      operation, inputs: inputs.map(input => input.id), availability: "when-performed", ...extra, rules: ["608.2c"] });
  }
  /** Branch contexts start from the antecedents established so far; nothing a
   * branch establishes is visible to a sibling or after the construct. */
  branch(context: Context): Context {
    return { ...context, objects: [...context.objects], players: [...context.players], quantities: [...context.quantities] };
  }
  /** CR 700.2a-c: modes are chosen as the spell or ability is put on the stack,
   * and a mode's targets exist only if it was chosen. Each mode binds in its own
   * branch; a later instruction cannot rely on a result of an unchosen mode. */
  modal(effect: Extract<Effect, { effect: "modal" }>, path: string, context: Context): void {
    const count = effect.count;
    if ("variable" in count) this.walk({ amount: count.variable === "x" ? "x" : "that-much" }, child(path, "count"), context);
    this.walk(effect.conditionalCount, child(path, "conditionalCount"), context);
    this.walk(effect.chooser, child(path, "chooser"), context);
    let sourceMoved = context.sourceMoved;
    effect.options.forEach((option, index) => {
      const at = child(child(path, "options"), index);
      const branch = this.branch(context);
      this.walk(option.additionalCost, child(at, "additionalCost"), { ...branch, phase: "payment" });
      this.sentences(option.effects, child(at, "effects"), branch);
      sourceMoved ||= branch.sourceMoved;
    });
    context.sourceMoved = sourceMoved;
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
    (effect.outcomes ?? []).forEach((outcome, index) => {
      const branch: Context = { ...this.branch(context), conditional: true };
      branch.objects = combine(branch.objects, announced);
      this.sentences(outcome.effects, child(child(child(path, "outcomes"), index), "effects"), branch);
      announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
      sourceMoved ||= branch.sourceMoved;
    });
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
    effect.table!.rows.forEach((row, index) => {
      const at = child(child(child(path, "table"), "rows"), index);
      const branch: Context = { ...this.branch(context), conditional: true };
      branch.objects = combine(branch.objects, announced);
      this.walk(row.range, child(at, "range"), branch);
      this.sentences(row.effects, child(at, "effects"), branch);
      announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
      sourceMoved ||= branch.sourceMoved;
    });
    context.sourceMoved = sourceMoved;
    context.objects = combine(context.objects, announced);
  }
  /** CR 608.2d: exactly one listed instruction is chosen while the effect is
   * applied. Options bind like modes; shared references are the same nodes. */
  choice(effect: Extract<Effect, { effect: "choice" }>, path: string, context: Context): void {
    this.walk(effect.who, child(path, "who"), context);
    const decider = this.at(child(path, "who"))?.id;
    let sourceMoved = context.sourceMoved;
    effect.options.forEach((option, index) => {
      const branch = { ...this.branch(context), ...(decider ? { decider } : {}) };
      this.effect(option, child(child(path, "options"), index), branch);
      sourceMoved ||= branch.sourceMoved;
    });
    context.sourceMoved = sourceMoved;
    context.objects = []; context.players = []; context.quantities = [];
  }
  /** Simultaneous instructions are one event: none sees another's results,
   * and all of their results are available afterwards. */
  simultaneous(effect: Extract<Effect, { effect: "simultaneous" }>, path: string, context: Context): void {
    const start = this.branch(context);
    const after = this.branch(context);
    effect.steps.forEach((step, index) => {
      const branch = this.branch(start);
      this.effect(step, child(child(path, "steps"), index), branch);
      after.objects = combine(after.objects, branch.objects);
      after.players = combine(after.players, branch.players);
      after.quantities = combine(after.quantities, branch.quantities);
      after.sourceMoved ||= branch.sourceMoved;
      after.departed = combine(after.departed ?? [], branch.departed ?? []);
    });
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
    return { ...this.branch(context), ability, phase: "resolution", decisions: undefined, previousDecisions: undefined, decider: undefined, conditional: undefined };
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
    identity: Extract<BindingNode, { node: "event-result" }>["identity"], description?: ObjectFilter): Candidate {
    const id = this.node({ id: this.id(path, result), path, node: "event-result", event, result, cardinality,
      availability: "if-produced", identity, ...(description ? { description } : {}), rules: identity === "zone-successors" ? ["400.7j"] : ["608.2h"] });
    return { id, cardinality, domain: result === "objects" ? "object" : result === "player" ? "player" : "number", description };
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
      if (linked && condition) this.node({ id: this.id(child(at, "condition"), "decision-link"), path: child(at, "condition"), node: "decision-link",
        scope: context.ability, condition: condition.condition as "you-do" | "you-dont", decision: linked, rules: ["608.2c", "608.2d"] });
      // An "unless" clause follows its instruction and may refer to that
      // instruction's objects ("Counter target spell unless its controller
      // pays {3}"), so it binds after the steps.
      const trailing = sentence.conditionKind === "unless" || sentence.conditionPosition === "after";
      context.linkedPath = linked;
      if (!trailing) this.walk(condition, child(at, "condition"), linked ? { ...context, linkedDecision: true } : context);
      if (sentence.afterConditional)
        this.diagnostic("unsupported-scope", at, "This sentence refers back to a conditional sentence; whether that condition governs it is not decided.", ["608.2c"]);
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
        context.sourceMoved = context.sourceMoved || branches.some(branch => branch.sourceMoved);
        context.mentionedObject = context.mentionedObject || branches.some(branch => branch.mentionedObject);
      } else {
        for (const [stepIndex, effect] of sentence.steps.entries())
          this.effect(effect, child(child(at, "steps"), stepIndex), context);
      }
      previousDecisions = decisions;
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
  effect(effect: Effect, path: string, context: Context): void {
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
    if (effect.effect === "gain-abilities" && "recipient" in effect.what) {
      this.walk(effect, path, { ...context, barrier: "An unresolved ability recipient cannot establish an object or player antecedent." });
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if (effect.effect === "for-each") {
      const barrier = "Quantified instructions require member references, original-ability target multiplicity, membership timing, and aggregated result bindings.";
      this.diagnostic("unsupported-scope", path, barrier, ["608.2e", "608.2f", "101.4", "115.3"]);
      this.walk(effect, path, { ...context, barrier });
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if ("who" in effect && effect.who?.player === "group") {
      const barrier = "Grouped actions preserve participant occurrences and shared-event multiplicity; result identities need explicit per-role aggregation.";
      this.diagnostic("unsupported-scope", path, barrier, ["101.4", "121.2c", "608.2e", "608.2f"]);
      this.walk(effect, path, { ...context, barrier });
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    const plain = !effect.optional && !effect.forEach && !effect.forEachVote;
    if (plain && effect.effect === "modal" && !effect.allowRepeat && !effect.unchosen && !effect.distinctTargetPlayers) { this.modal(effect, path, context); return; }
    if (plain && effect.effect === "choice") { this.choice(effect, path, context); return; }
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
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    if (effect.optional) this.beforeDecision.set(path, context.objects);
    this.walk(effect, path, context);
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
      case "destroy": case "counter": case "pump": case "set-pt":
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
      case "scan-library": case "grant-look-permission": case "self-replacement":
        this.diagnostic("unsupported-scope", path, "This instruction's event/result associations are not yet established.", ["608.2c"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "copy-stack-object": case "copy-card": case "choose-new-targets":
        this.diagnostic("unsupported-scope", path, "Stack-copy results and copy target-selection timing require their own binding scope.", ["707.10", "115.7"]);
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
      case "manifest": case "cloak":
        // The card is turned face down and put onto the battlefield: a new object (CR 400.7, 701.40, 701.58).
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        produces = "objects"; identity = "new-objects"; break;
      case "learn": case "open-attraction": case "assemble-contraption": case "incubate": case "create-role": case "amass":
      case "discover": case "endure": case "manifest-dread":
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
        : effect.effect === "create-token" ? { cls: "token", ...("types" in effect.token && effect.token.types.length ? { types: effect.token.types as NonNullable<ObjectFilter["types"]> } : {}) }
        : ["look-at", "mill", "draw", "reveal"].includes(effect.effect) ? { cls: "card" } : undefined;
      const result = this.eventResult(path, event, produces, cardinality, identity, established);
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
  triggerPattern(trigger: Trigger, path: string, context: Context): void {
    if (trigger.trigger === "counter-change") {
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
      if (!context.sourceMoved && branches.length > 1 && others.every(list => list.length === 1 && list[0].cardinality === "one")
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
    const match = { ...context, phase: "trigger-match" as const };
    this.walk(trigger, path, match);
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
    if (["dies", "leaves", "leaves-zone", "put-into-graveyard", "enters-or-leaves", "enters-or-dies"].includes(trigger.trigger)) {
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
  ability(ability: Ability, path: string, face: string, source: string): void {
    const id = this.id(path, "ability");
    const you = this.id(path, "you");
    const role = ability.kind === "activated" || ability.kind === "loyalty" ? "activator"
      : ability.kind === "triggered" || ability.kind === "chapter" || ability.kind === "visit" || ability.kind === "case-solve" ? "trigger-controller"
      : ability.kind === "spell" ? "spell-controller" : ability.kind === "additional-cost" ? "prospective-caster" : "static-source-controller";
    this.node({ id, path, node: "ability", source, face, abilityKind: ability.kind, you, rules: ["113", "109.5"] });
    this.node({ id: you, path, node: "runtime-player", scope: id, role, rules: ["109.5"] });
    // An unqualified "other" excludes the permanent with the ability, including
    // an object that was given the ability; never the "source" of a spell.
    const permanentSource = ability.kind !== "spell" &&
      (this.permanentFaces.has(face) || this.result.nodes.some(node => node.id === source && node.node === "recipient"));
    const context: Context = { source, face, ability: id, you, permanentSource, staticAbility: ability.kind === "static",
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
    const granted = this.result.nodes.some(node => node.id === source && node.node === "recipient");
    if (!granted && !JSON.stringify(ability).includes('"variable":"x"')) {
      const hasX = (value: unknown) => /"X"|"amount":"x"/.test(JSON.stringify(value ?? null));
      const entering = (ability.kind === "triggered" && ability.trigger.trigger === "enters" && "what" in ability.trigger
          && isRecord(ability.trigger.what) && ["self", "this"].includes(String(ability.trigger.what.ref)))
        || (ability.kind === "static" && ["enters-with-counters", "as-enters", "entry-choice"].includes(String(ability.effect.static)));
      const announcement = (ability.kind === "activated" && hasX(ability.costs)) || (ability.kind === "loyalty" && hasX(ability.cost)) ? "activation" as const
        : ability.kind === "spell" && this.xFaces.has(face) ? "cast" as const
        : entering && this.xFaces.has(face) && this.permanentFaces.has(face) ? "entering-spell" as const : undefined;
      if (announcement) context.announcedX = this.node({ id: this.id(path, "announced-x"), path, node: "announced-number", scope: id, chooser: you, announcement,
        rules: announcement === "activation" ? ["107.3a", "107.3k", "602.2b"] : announcement === "cast" ? ["107.3a", "601.2b"] : ["107.3m"] });
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
        this.walk(ability.kind === "activated" ? ability.costs : ability.cost,
          child(path, ability.kind === "activated" ? "costs" : "cost"), payment);
        this.walk(ability.restriction, child(path, "restriction"), payment);
        this.walk(ability.permission, child(path, "permission"), payment);
        this.walk(ability.costModifications, child(path, "costModifications"), payment);
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
          if (["sacrifice", "exile", "discard", "return", "move-zone"].includes(cost.cost)) context.departed = combine(context.departed ?? [], [selected]);
          else if (["tap-objects", "untap-objects", "exert", "reveal"].includes(cost.cost)) context.objects = combine(context.objects, [selected]);
        });
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      }
      case "triggered":
        this.trigger(ability.trigger, child(path, "trigger"), context);
        this.namingCondition(ability.condition, child(path, "condition"), context);
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      case "spell": {
        const previous = this.spellParagraph?.face === face ? this.spellParagraph.context : undefined;
        if (previous) Object.assign(context, { objects: previous.objects, players: previous.players, quantities: previous.quantities,
          departed: previous.departed, sourceMoved: previous.sourceMoved, mentionedObject: previous.mentionedObject, sourceNamed: previous.sourceNamed });
        this.sentences(ability.effects, child(path, "effects"), context);
        this.spellParagraph = { face, context };
        return;
      }
      case "chapter": this.sentences(ability.effects, child(path, "effects"), context); break;
      case "static":
        // "..., where X is ...": the static ability's own definition (CR 107.3c).
        if ("bindings" in ability.effect && Array.isArray(ability.effect.bindings)
          && !["entry-choice", "enters-with-counters", "enters-tapped", "as-enters"].includes(String(ability.effect.static)))
          this.definitions(ability.effect.bindings as Sentence["bindings"], child(child(path, "effect"), "bindings"), context);
        this.walk(ability.effect, child(path, "effect"), context);
        if (ability.effect.static === "replacement") this.diagnostic("unsupported-scope", path,
          "Replacement-event roles and captures need a separate scope.", ["614"]);
        break;
      case "keywords": ability.keywords.forEach((keyword, index) => this.keyword(keyword, child(child(path, "keywords"), index), context)); break;
      case "additional-cost": this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment" });
        this.walk(ability.costModifications, child(path, "costModifications"), { ...context, phase: "payment" }); break;
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
    let changed = true;
    while (changed) {
      changed = false;
      for (const node of this.result.nodes) {
        const dependent = node.node === "event" ? node.inputs.some(id => this.invalidNodes.has(id)) ||
            this.result.references.some(reference => reference.path.startsWith(`${node.path}/`) && reference.binding === "unresolved") ||
            this.result.diagnostics.some(diagnostic => diagnostic.path === node.path || diagnostic.path.startsWith(`${node.path}/`))
          : node.node === "event-result" ? this.invalidNodes.has(node.event)
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
      child(child(path, "abilities"), abilityIndex), face, source));
  }
  for (const face of input.faces) if (!Number.isInteger(face.sourceFaceIndex) || face.sourceFaceIndex < 0 || face.sourceFaceIndex >= faces.length)
    analyzer.diagnostic("face-mismatch", "/faces", "A supplied parse references a nonexistent source face.", []);
  return analyzer.finish();
}
