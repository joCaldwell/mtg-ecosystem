// This pass resolves reference identities, not game execution or all semantic
// correctness. In particular, it never substitutes "last noun" for an
// ambiguous referent. Every unsupported scope remains a reported obligation.
// Rules: CR 109.5, 201.5, 400.7j, 601.2c, 608.2h/k, 107.3, 607.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Amount, Effect, ObjectFilter, ObjectRef, PlayerRef, Sentence, Trigger } from "../ast.ts";
import { isPluralPlayerRef } from "../parser/refs.ts";
import { uniqueStackTriggerEvent } from "../parser/stack-copy.ts";
import type {
  BindingCardinality, BindingDiagnosticCode, BindingNode, BindingPhase, BindingSourceFace,
  CardBindingAnalysis, CardBindingInput, ReferenceBinding, ReferenceExpression,
} from "./types.ts";

const RULES_URL = "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt";
const SYMBOLIC_AMOUNTS = new Set(["x", "that-number", "that-much", "replacement-event-quantity"]);
interface Candidate { id: string; cardinality: BindingCardinality; domain: "object" | "object-or-player" | "player" | "number"; description?: ObjectFilter }
interface Context {
  source: string; face: string; ability: string; you: string; phase: BindingPhase;
  objects: Candidate[]; players: Candidate[]; quantities: Candidate[]; x?: string;
  sourceMoved?: boolean;
  /** The object of one lexical cast/activation occurrence, independent of
   * salient nouns in the surrounding text. Never derived from proximity. */
  triggerObject?: { event: "cast" | "activate"; candidate: Candidate };
  /** A scope whose capture/antecedent rules are not implemented. */
  barrier?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function child(path: string, part: string | number): string {
  return `${path}/${String(part).replace(/~/g, "~0").replace(/\//g, "~1")}`;
}
function cardCount(amount: Amount | undefined): BindingCardinality {
  return amount?.amount === "fixed" ? amount.value === 1 ? "one" : "many" : "unknown";
}
function objectCardinality(ref: ObjectRef): BindingCardinality {
  switch (ref.ref) {
    case "self": case "this": case "it": case "enchanted": case "equipped": case "trigger-event-object": return "one";
    case "them": case "each": case "all": case "zone-cards": case "rest": case "vote-leaders": return "many";
    case "related": case "copy-result": case "choice-result": return ref.number === "singular" ? "one" : "many";
    case "that": return ref.plural ? "many" : "one";
    case "target": case "filter": case "commander": case "library-cards": case "from-pool": return cardCount(ref.count);
    case "any-target": return ref.count ? cardCount(ref.count) : "one";
    case "replacement-event-object": case "prevention-event-object": case "card-extraction-group": return "unknown";
  }
}
function combine(a: Candidate[], b: Candidate[]): Candidate[] {
  return [...new Map([...a, ...b].map(candidate => [candidate.id, candidate])).values()];
}
function description(ref: ObjectRef): ObjectFilter | undefined {
  if ("filter" in ref) return ref.filter;
  if (ref.ref === "this") {
    if (["card", "spell", "permanent", "ability"].includes(ref.noun)) return { cls: ref.noun as "card" | "spell" | "permanent" | "ability" };
    if (["creature", "artifact", "enchantment", "land", "planeswalker", "battle"].includes(ref.noun)) return { types: [ref.noun] };
  }
  return undefined;
}
function domain(expression: ReferenceExpression): Candidate["domain"] {
  if ("player" in expression) return "player";
  if (!("ref" in expression)) return "number";
  return expression.ref === "any-target" || ("filter" in expression && (expression.filter.orPlayer || expression.filter.orOpponent)) ? "object-or-player" : "object";
}
function referenceCardinality(expression: ReferenceExpression): BindingCardinality {
  return "ref" in expression ? expression.ref === "affected-object" ? "one" : objectCardinality(expression)
    : "player" in expression && isPluralPlayerRef(expression) ? "many" : "one";
}
/** This is an antecedent-description check, not a current-characteristics
 * predicate. A later "that" never chooses a new object by its current type. */
function describes(actual: ObjectFilter | undefined, required: ObjectFilter): boolean {
  if (!actual) return Object.keys(required).length === 0;
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

  constructor(identity: string) {
    this.prefix = `card:${encodeURIComponent(identity)}`;
    this.result = { artifactKind: "oracle-reference-binding", schemaVersion: 1, executable: false, identity,
      rules: { edition: "2026-09-25", url: RULES_URL }, nodes: [], references: [], diagnostics: [],
      counts: { obligations: 0, bound: 0, unresolved: 0 }, fullyBound: false };
  }
  id(path: string, role: string): string { return `${this.prefix}${path}#${role}`; }
  node(node: BindingNode): string { this.result.nodes.push(node); return node.id; }
  diagnostic(code: BindingDiagnosticCode, path: string, message: string, rules: string[]): void {
    if (!this.result.diagnostics.some(d => d.code === code && d.path === path))
      this.result.diagnostics.push({ code, path, message, rules });
  }
  unresolved(expression: ReferenceExpression, path: string, code: BindingDiagnosticCode,
    message: string, rules: string[], candidates: string[] = []): void {
    this.result.references.push({ path, expression, binding: "unresolved", code, candidates, rules });
    this.diagnostic(code, path, message, rules);
  }
  bound(expression: ReferenceExpression, path: string, target: string, rules: string[]): Candidate {
    this.result.references.push({ path, expression, binding: "bound", target, rules });
    return { id: target, cardinality: referenceCardinality(expression), domain: domain(expression),
      ...("ref" in expression && expression.ref !== "affected-object" ? { description: description(expression) } : {}) };
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
    if (desired !== "unknown" && candidate.cardinality !== desired) {
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
    if (typeof value.ref === "string") { this.object(value as ObjectRef, path, context); return; }
    if (typeof value.player === "string") { this.player(value as PlayerRef, path, context); return; }
    if (value.to === "any-target") {
      this.object({ ref: "any-target", ...(typeof value.selectionId === "string" ? { selectionId: value.selectionId } : {}) }, path, context);
      return;
    }
    if (typeof value.keyword === "string" && typeof value.action !== "string") {
      const barrier = "An atomic keyword needs its intrinsic ability scopes, targets, and payment roles expanded before reference completeness can be established.";
      this.diagnostic("unsupported-scope", path, barrier, ["702"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.amount === "x" && (typeof value.keyword === "string" || typeof value.sign === "number")) {
      this.walk({ amount: "x" }, child(path, "amount"), context);
    } else if (typeof value.amount === "string" && SYMBOLIC_AMOUNTS.has(value.amount)) {
      const expression = value as Extract<Amount, { amount: "x" | "that-number" | "that-much" | "replacement-event-quantity" }>;
      if (context.barrier) this.unresolved(expression, path, "unsupported-scope", context.barrier, ["107.3"]);
      else if (expression.amount === "x" && context.x) this.bound(expression, path, context.x, ["107.3c"]);
      else if (expression.amount === "that-much") this.antecedent(expression, path,
        context.quantities, "unknown", ["608.2h"]);
      else this.unresolved(expression, path, "unsupported-number-binding",
        "This number needs a rule-specific definition, choice, or capture; no default value is inferred.", ["107.3a", "107.3f", "107.3j"]);
      return;
    }
    if (value.gained === "quoted" || value.gained === "keyword" || value.gained === "keyword-choice" || value.gained === "abilities-of") {
      const barrier = "Granted-ability source rebinding and captures require a separate scope (CR 201.5a/b).";
      this.diagnostic("unsupported-scope", path, barrier, ["201.5a", "201.5b", "107.3j"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
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
    if (value.effect === "scan-library") {
      const barrier = "A sequential library scan needs its actor, complete processed pool, matching subset, and per-card iteration scope bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["609.3", "701.13", "701.20"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "prevent-damage" || value.effect === "prevent-damage" || value.choice === "damage-source"
      || value.amount === "prevention-event-quantity" || value.condition === "damage-prevented") {
      const barrier = "Prevention needs exact future damage events, source-choice eligibility, shield consumption, and actual prevented-quantity scopes bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["609.7", "615.5", "615.7", "615.8", "615.10", "615.12"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.static === "look-permission" || value.effect === "grant-look-permission"
      || value.static === "commander-eligibility" || value.static === "optional-untap") {
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
    if (typeof value.predefined === "string" || value.attacking === true)
      this.diagnostic("unsupported-scope", path, "Predefined token abilities and entering-attacking choices require scopes belonging to the new token.", ["111.10", "508.4"]);
    if (value.amount === "chosen")
      this.diagnostic("unsupported-number-binding", path, "This number needs a phase-specific choice identity and constraint scope.", ["107.3", "601.2b"]);
    if (value.amount === "dice-result" || value.condition === "dice-result")
      this.diagnostic("unsupported-scope", path, "A die result needs its exact roll occurrence, natural/final stage, retained-results set, and table/trigger scope bound.", ["706.2", "706.3", "706.6", "706.7"]);
    if (value.amount === "kicker-count")
      this.diagnostic("unsupported-number-binding", path, "This count requires the linked originating spell's recorded kicker payments.", ["400.7d", "702.33"]);
    if (value.amount === "event-quantity" || value.amount === "life-change") {
      const barrier = "An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.";
      this.diagnostic("unsupported-scope", path, barrier, ["119.3", "120.3", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
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
    if (value.condition === "you-do" || value.condition === "you-dont") {
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
      const barrier = "The optional payment event must bind its payer separately from the resolving ability's controller.";
      this.diagnostic("unsupported-scope", path, barrier, ["118.12a"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.combatHistory) {
      const barrier = "Combat history requires each candidate's declared-attacker event and incarnation within the stated turn/combat window.";
      this.diagnostic("unsupported-scope", child(path, "combatHistory"), barrier, ["508.4", "400.7", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.damageHistory) {
      const barrier = "A damage-history filter needs a per-candidate event role, historical counterpart incarnation, and event-window binding.";
      this.diagnostic("unsupported-scope", child(path, "damageHistory"), barrier, ["120.3", "120.7", "400.7", "608.2h"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (["attacked", "combat-action-history", "was-cast", "zone-change-history", "damage-history", "mana-spent-to-cast"].includes(String(value.condition))) {
      const barrier = "Historical event, phase, and source-incarnation associations have not been bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["608.2i", "400.7d"]);
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
    if (["entry-choice", "enters-with-counters", "enters-tapped"].includes(String(value.static)) || isRecord(value.entry)) {
      const barrier = "The entering permanent's pre-entry controller, payment, attachment, and attack-choice scopes have not been bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["614.12a", "303.4f", "508.4"]);
      this.walkChildren(value, path, { ...context, barrier });
      return;
    }
    if (value.other === true && !value.otherThan)
      this.diagnostic("unsupported-reference", child(path, "other"), "The excluded antecedent of other/another is unresolved.", ["201.5"]);
    if (value.relation === "event-result" || value.relation === "linked-exile" || value.condition === "kicked")
      this.diagnostic("unsupported-reference", path, "This reference requires a specific event/payment or linked-ability association.", ["607"]);
    if (["alternative-cost", "as-enters", "replacement", "play-permission"].includes(String(value.static))) {
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
      else if (["its-owner", "owner", "opponent", "each-player", "each-opponent", "active-player", "not-you", "any", "an-opponent"].includes(String(token))) {
        this.diagnostic("unsupported-reference", child(path, key), "This implicit player relation requires its own scoped binding rule.", ["109.5"]);
      }
    }
    this.walkChildren(value, path, context);
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
      }
    } else if (value.battlefieldController === "owner" || value.owner === "each-affected-objects-controller") {
      this.diagnostic("unsupported-reference", path, "This ownership rule requires a bound affected-object scope.", ["109.5"]);
    }
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
          this.unresolved(expression, path, "unsupported-reference", "After a source zone change, this reference needs an action-specific source/last-known/successor rule.", ["400.7", "608.2h"]);
          return undefined;
        }
        return this.bound(expression, path, context.source, ["201.5"]);
      case "it": case "them": case "that":
        return this.antecedent(expression, path, context.objects, objectCardinality(expression), ["608.2k", "400.7j"]);
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
      case "vote-leaders":
        this.unresolved(expression, path, "unsupported-reference", "The objects with the most votes need the preceding vote's tally.", ["701.38"]);
        return undefined;
      case "equipped": case "enchanted": case "related": case "rest": case "replacement-event-object": case "prevention-event-object":
        this.unresolved(expression, path, "unsupported-reference", "This relation needs an attachment, event, pool, or linked-ability binding.", ["303.4", "301.5", "607"]);
        return undefined;
      case "target": case "any-target": return this.selection(expression, path, context, true);
      case "filter": case "each": case "all": case "library-cards": case "zone-cards": case "commander": case "from-pool":
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
    if (expression.player === "they" || expression.player === "that-player")
      return this.antecedent(expression, path, context.players, "unknown", ["608.2k"]);
    if (expression.player === "that-player-or-controller") {
      this.unresolved(expression, path, "unsupported-reference", "This player depends on whether one earlier object-or-player antecedent is a player or an object; that shared antecedent must be bound first.", ["608.2c"]);
      return undefined;
    }
    if (expression.player === "enchanted-player") {
      this.unresolved(expression, path, "unsupported-reference", "This player requires the source Aura's attachment relation, independently of its controller.", ["303.4b", "303.4e"]);
      return undefined;
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
    const id = targeted && selectionId ? `${context.ability}/targets/${encodeURIComponent(selectionId)}` : this.id(path, targeted ? "target" : "query");
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
      : "ref" in binding.expression && binding.expression.ref !== "affected-object" ? description(binding.expression) : undefined;
    return { id: binding.target, cardinality, domain: domain(binding.expression), description: established };
  }
  event(path: string, context: Context, operation: string, inputs: Candidate[]): string {
    return this.node({ id: this.id(path, "event"), path, node: "event", scope: context.ability,
      operation, inputs: inputs.map(input => input.id), availability: "when-performed", rules: ["608.2c"] });
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
    for (const [index, sentence] of sentences.entries()) {
      const at = child(path, index);
      this.definitions(sentence.bindings, child(at, "bindings"), context);
      this.walk(sentence.condition, child(at, "condition"), context);
      if (sentence.condition || sentence.otherwise) {
        const barrier = "Conditional branch dominance and result merging are not established by this pass.";
        this.diagnostic("unsupported-scope", at, barrier, ["608.2c"]);
        this.walk(sentence.steps, child(at, "steps"), { ...context, barrier });
        this.walk(sentence.otherwise, child(at, "otherwise"), { ...context, barrier });
        context.objects = []; context.players = []; context.quantities = [];
      } else {
        for (const [stepIndex, effect] of sentence.steps.entries())
          this.effect(effect, child(child(at, "steps"), stepIndex), context);
      }
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
      const barrier = "Resolving characteristic changes need one affected-object snapshot across layers and direct value snapshots distinct from granted ability calculations.";
      this.diagnostic("unsupported-scope", path, barrier, ["611.2c", "611.2d", "613.6", "604.3a"]);
      this.walk(effect, path, { ...context, barrier });
      context.objects = []; context.players = []; context.quantities = [];
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
    if (["optional-group", "choice", "simultaneous", "modal", "create-delayed-trigger", "create-reflexive-trigger", "create-replacement", "self-replacement", "shuffle-into-library", "pay-cost", "roll-dice", "roll-again", "flip-coin"].includes(effect.effect) || effect.optional || effect.forEach || effect.forEachVote) {
      const barrier = "Nested, optional, simultaneous, or delayed instruction scopes need explicit capture and merge rules.";
      this.diagnostic("unsupported-scope", path, barrier, ["603.7", "608.2c"]);
      this.walk(effect, path, { ...context, barrier });
      context.objects = []; context.players = []; context.quantities = [];
      return;
    }
    this.walk(effect, path, context);
    const objectPaths: string[] = [];
    const playerPaths: string[] = [];
    let produces: "objects" | "quantity" | undefined;
    let cardinality: BindingCardinality = "unknown";
    let identity: Extract<BindingNode, { node: "event-result" }>["identity"] = "same-objects";
    switch (effect.effect) {
      case "search": objectPaths.push(child(path, "for")); playerPaths.push(child(path, "who")); break;
      case "choose-object": objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
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
        if (effect.to.to === "object" || effect.to.to === "player") objectPaths.push(child(child(path, "to"), "ref"));
        else if (effect.to.to === "any-target") objectPaths.push(child(path, "to"));
        else if (effect.to.to === "divided") objectPaths.push(child(child(path, "to"), "among"));
        else if (effect.to.to === "each") effect.to.refs.forEach((_ref, index) => objectPaths.push(child(child(child(path, "to"), "refs"), index)));
        break;
      case "fight": objectPaths.push(child(path, "a"), child(path, "b")); break;
      case "reveal": case "look-at": case "discard": case "sacrifice": case "gain-control":
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
      case "tap": case "untap":
        objectPaths.push(child(path, "what"));
        if (effect.who) playerPaths.push(child(path, "who"));
        break;
      case "destroy": case "counter": case "pump": case "set-pt":
      case "transform": case "convert": case "phase-out": case "phase-in":
      case "flip": case "detain": case "suspect": case "unsuspect":
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
      case "remove-counters": objectPaths.push(child(path, "from")); break;
      case "add-mana": case "get-counters": case "get-emblem": case "pay-cost": case "choose-value": case "player-gain-abilities":
      case "shuffle": case "win-game": case "lose-game": playerPaths.push(child(path, "who")); break;
      case "declare-target": objectPaths.push(child(path, "what")); break;
      case "prohibit": case "cast-limit": case "cost-modification":
      case "skip-time": case "add-turns": case "add-phases": case "end-turn": case "end-combat": case "set-day-night":
      case "double-counters": case "move-counters": case "distribute-counters": case "multiply-pt": case "switch-pt":
      case "double-life-total": case "double-mana-pool": case "exchange-control": case "exchange-life-totals":
      case "exchange-values": case "exchange-zones": case "choose-random":
      case "exile-until": case "scan-library": case "play-card": case "grant-play-permission": case "grant-look-permission": case "additional-land-plays": case "prevent-damage": case "shuffle-into-library": case "self-replacement":
        this.diagnostic("unsupported-scope", path, "This instruction's event/result associations are not yet established.", ["608.2c"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "copy-stack-object": case "choose-new-targets":
        this.diagnostic("unsupported-scope", path, "Stack-copy results and copy target-selection timing require their own binding scope.", ["707.10", "115.7"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "become-monarch": case "take-initiative": case "ring-tempts": case "learn": case "forage": case "time-travel": case "open-attraction":
      case "venture": case "incubate": case "earthbend": case "airbend": case "goad": case "endure": case "empower-jace": case "clash": case "vote": case "create-role":
      case "amass": case "adapt": case "monstrosity": case "bolster": case "support": case "scry": case "surveil":
      case "keyword-action": case "blight": case "discover": case "explore": case "connive": case "regenerate":
      case "manifest": case "cloak": case "manifest-dread":
        // Keyword actions have internal selections/results not visible in
        // this AST. Do not let later pronouns bind across them by accident.
        this.diagnostic("unsupported-scope", path, "This action's implicit runtime selections need a rule-specific binding model.", ["701"]);
        context.objects = []; context.players = []; context.quantities = [];
        return;
      case "optional-group": case "choice": case "simultaneous": case "modal": case "create-delayed-trigger": case "create-reflexive-trigger": case "create-replacement": case "roll-dice": case "roll-again": case "flip-coin":
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
    const event = this.event(path, context, effect.effect, [...objects, ...players]);
    // Retain all distinct established participants. Replacing this set with
    // the most recently mentioned noun would silently guess an antecedent.
    if (objectPaths.length) context.objects = combine(context.objects, objects);
    if (playerPaths.length) context.players = combine(context.players, players.filter(player => player.id !== context.you));
    if (produces) {
      if (produces === "objects" && objects.length === 1) cardinality = objects[0].cardinality;
      if (players.some(player => player.cardinality !== "one")) {
        cardinality = "unknown";
        this.diagnostic("unsupported-scope", path, "Per-player results require separate event instances and aggregate/result scope.", ["608.2c"]);
      }
      const result = this.eventResult(path, event, produces, cardinality, identity,
        identity === "zone-successors" ? { ...(objects[0]?.description ?? {}), cls: "card" } : undefined);
      if (produces === "objects") {
        if (identity === "zone-successors") {
          const moved = new Set(objects.map(object => object.id));
          context.objects = context.objects.filter(object => !moved.has(object.id));
        }
        context.objects = combine(context.objects, [result]);
      }
      else context.quantities = combine(context.quantities, [result]);
    }
    if (["draw", "mill", "create-token", "put-counters", "remove-counters"].includes(effect.effect))
      context.quantities = combine(context.quantities, [this.eventResult(path, event, "quantity", "one", "measured-quantity")]);
    if (["exile", "move-zone", "return-to-hand", "destroy", "counter", "sacrifice", "discard"].includes(effect.effect)) {
      if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
      if (identity !== "zone-successors") {
        // Hidden destinations and implicit/replaced zone moves need a
        // separate successor/LKI model. Never retain the old target as "it".
        const moved = new Set(objects.map(object => object.id));
        context.objects = context.objects.filter(object => !moved.has(object.id));
      }
    }
  }
  trigger(trigger: Trigger, path: string, context: Context): void {
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
      const barrier = "Alternative trigger events require a discriminated event-parameter binding.";
      this.diagnostic("unsupported-scope", path, barrier, ["603.2c"]);
      this.walk(trigger, path, { ...context, barrier });
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
    if (stackEvent && eventObject) context.triggerObject = { event: stackEvent, candidate: eventObject };
    if (recipient) {
      if (recipient.domain === "player") context.players = combine(context.players, [recipient]);
      else context.objects = combine(context.objects, [recipient]);
    }
    if (who) context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
    if (["deals-damage", "is-dealt-damage", "gains-life", "loses-life"].includes(trigger.trigger))
      context.quantities = [this.eventResult(path, event, "quantity", "one", "measured-quantity")];
    if (["dies", "leaves", "leaves-zone", "put-into-graveyard", "enters-or-leaves", "enters-or-dies"].includes(trigger.trigger)) {
      if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
      context.objects = [];
      this.diagnostic("unsupported-scope", path, "Zone-change triggers need action-specific last-known object and successor roles.", ["400.7e", "608.2h"]);
    }
  }
  ability(ability: Ability, path: string, face: string, source: string): void {
    const id = this.id(path, "ability");
    const you = this.id(path, "you");
    const role = ability.kind === "activated" || ability.kind === "loyalty" ? "activator"
      : ability.kind === "triggered" || ability.kind === "chapter" || ability.kind === "case-solve" ? "trigger-controller"
      : ability.kind === "spell" ? "spell-controller" : ability.kind === "additional-cost" ? "prospective-caster" : "static-source-controller";
    this.node({ id, path, node: "ability", source, face, abilityKind: ability.kind, you, rules: ["113", "109.5"] });
    this.node({ id: you, path, node: "runtime-player", scope: id, role, rules: ["109.5"] });
    const context: Context = { source, face, ability: id, you,
      phase: ability.kind === "static" || ability.kind === "keywords" || ability.kind === "class-level-bar" ? "continuous" : "resolution", objects: [], players: [], quantities: [] };
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
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      }
      case "triggered":
        this.trigger(ability.trigger, child(path, "trigger"), context);
        this.walk(ability.condition, child(path, "condition"), context);
        this.sentences(ability.effects, child(path, "effects"), context);
        break;
      case "spell": case "chapter": this.sentences(ability.effects, child(path, "effects"), context); break;
      case "static":
        this.walk(ability.effect, child(path, "effect"), context);
        if (ability.effect.static === "replacement") this.diagnostic("unsupported-scope", path,
          "Replacement-event roles and captures need a separate scope.", ["614"]);
        break;
      case "keywords": this.walk(ability.keywords, child(path, "keywords"), context); break;
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
      case "max-speed":
        this.diagnostic("unsupported-scope", path, "The nested ability's source and controller scope have not been bound.", ["702.179"]);
        this.walk(ability.ability, child(path, "ability"), { ...context, barrier: "Nested ability scope is unsupported." });
        break;
      case "class-level-bar": {
        const barrier = "A Class level bar needs intrinsic upgrade activation/payment, designation history, and cumulative granted-ability scopes bound.";
        this.diagnostic("unsupported-scope", path, barrier, ["716.2", "716.3"]);
        this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment", barrier });
        this.walk(ability.abilities, child(path, "abilities"), { ...context, barrier });
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
    this.result.fullyBound = this.result.counts.unresolved === 0 && this.result.diagnostics.length === 0;
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
