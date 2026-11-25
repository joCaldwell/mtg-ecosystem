// Binding state: nodes, references, diagnostics and the dependency fixpoint.
import type { ObjectFilter } from "../../ast.ts";
import type { BindingCardinality, BindingDiagnosticCode, BindingNode, CardBindingAnalysis, EngineFeature, ReferenceBinding, ReferenceExpression } from "../types.ts";
import { RULES_URL, description, domain, referenceCardinality, type Candidate, type Context } from "./context.ts";
import * as antecedents from "./antecedents.ts";
import * as players from "./players.ts";
import * as numbers from "./numbers.ts";
import * as scopes from "./scopes.ts";
import * as effects from "./effects.ts";
import * as triggers from "./triggers.ts";
import * as abilities from "./abilities.ts";
import * as replacements from "./replacements.ts";
import * as specialStackCopies from "./special/stack-copies.ts";
import * as specialExileLinks from "./special/exile-links.ts";
import * as specialSearches from "./special/searches.ts";
import * as specialPlayers from "./special/players.ts";
import * as specialAbilities from "./special/abilities.ts";

/** The binder's methods live in the sibling modules as `this: Analyzer`
 * functions and are installed on the prototype below. */
type BinderMethods =
  typeof antecedents &
  typeof players &
  typeof numbers &
  typeof scopes &
  typeof effects &
  typeof triggers &
  typeof abilities &
  typeof replacements &
  typeof specialStackCopies &
  typeof specialExileLinks &
  typeof specialSearches &
  typeof specialPlayers &
  typeof specialAbilities;
export interface Analyzer extends BinderMethods {}
export class Analyzer {
  readonly result: CardBindingAnalysis;
  readonly prefix: string;
  readonly targets = new Map<string, { id: string; path: string; signature: string }>();
  readonly invalidNodes = new Map<string, BindingDiagnosticCode>();
  /** Objects an activation cost selected ("Sacrifice a creature: …"). */
  readonly costObjects = new Set<string>();
  readonly groupedTokenEvents = new Map<string, string>();
  readonly targetedPlayerGroups = new Set<string>();
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
  readonly beforeDecision = new Map<string, Pick<Context,
    "objects" | "departed" | "players" | "quantities" | "sourceMoved" | "sourceSuccessor" | "previousEffect" |
    "exileResults" | "uncertainExileOrigin" | "stackCopies" | "uncertainStackOrigin" | "scans" | "dieResults" | "uncertainDieOrigin" | "uncertainScanOrigin" | "uncertainRevealOrigin" |
    "copiedCards" | "linkedCopies" | "uncertainCardCopyOrigin" | "measuredEvents" | "uncertainMeasureOrigin" | "precedingSentence">>();
  /** Each face's card types, as the source's established description. */
  readonly faceTypes = new Map<string, ObjectFilter>();
  /** Each face ability's printed AST by its path ("/faces/0/abilities/1"),
   * for checks that walk printed instructions by path. */
  readonly abilityAsts = new Map<string, unknown>();
  /** The latest choice from a pool in each ability, for "the rest". */
  readonly poolSelections = new Map<string, { pool: Candidate; selected: Candidate; path: string }>();
  /** Kicker abilities printed on each face, with their costs (CR 702.33e). */
  readonly kickers = new Map<string, { id: string; you: string; costs: string[]; announcedX?: string }[]>();
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
  event(path: string, context: Context, operation: string, inputs: Candidate[],
    extra: { optional?: true; decider?: string; repeated?: "for-each"; conditional?: true; condition?: string; requiresResult?: string } = {}): string {
    return this.node({ id: this.id(path, "event"), path, node: "event", scope: context.ability,
      operation, inputs: inputs.map(input => input.id), availability: "when-performed", ...extra, rules: ["608.2c"] });
  }
  eventResult(path: string, event: string, result: "objects" | "quantity" | "player", cardinality: BindingCardinality,
    identity: Extract<BindingNode, { node: "event-result" }>["identity"], description?: ObjectFilter, subsetOf?: string): Candidate {
    const id = this.node({ id: this.id(path, result), path, node: "event-result", event, result, cardinality,
      availability: "if-produced", identity, ...(description ? { description } : {}), ...(subsetOf ? { subsetOf } : {}),
      rules: identity === "zone-successors" ? ["400.7j"] : ["608.2h"] });
    return { id, cardinality, domain: result === "objects" ? "object" : result === "player" ? "player" : "number", description };
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
          : node.node === "result-union" ? node.from.some(id => this.invalidNodes.has(id))
          // "The rest" is no better established than its pool or the pick it excludes.
          : node.node === "remainder" ? this.invalidNodes.has(node.pool) || this.invalidNodes.has(node.excluding)
          : node.node === "destroyed-permanents" ? this.invalidNodes.has(node.event)
          : node.node === "derived-player" ? this.invalidNodes.has(node.of)
          : node.node === "selection" || node.node === "defined-number"
            ? this.result.references.some(reference => reference.path.startsWith(`${node.path}/`) &&
                (reference.binding === "unresolved" || this.invalidNodes.has(reference.target))) ||
              this.result.diagnostics.some(diagnostic => diagnostic.path.startsWith(`${node.path}/`))
            : false;
        if (dependent && !this.invalidNodes.has(node.id)) { this.invalidNodes.set(node.id, "unresolved-dependency"); changed = true; }
      }
    }
    // A remainder that is not established needs no declined-choice handling.
    const unestablished = new Set(this.result.nodes.flatMap(node => node.node === "remainder" && this.invalidNodes.has(node.id) ? [node.path] : []));
    this.result.engineFeatures = this.result.engineFeatures.filter(need =>
      !(need.feature === "optional-look-remainder" && (unestablished.has(need.path) || unestablished.has(`${need.path}/what`))));
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
Object.assign(Analyzer.prototype,
  antecedents, players, numbers, scopes, effects, triggers, abilities, replacements,
  specialStackCopies, specialExileLinks, specialSearches, specialPlayers, specialAbilities);
