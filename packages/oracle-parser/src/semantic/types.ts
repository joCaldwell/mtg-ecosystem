import type { Amount, ObjectFilter, ObjectRef, ParseCardResult, PlayerRef } from "../ast.ts";
import type { KeywordAbilityKind } from "./keyword-scopes.ts";

/** Structural source input: callers may pass the complete Scryfall record.
 * Fields actually consumed by the analysis are also checked at runtime. */
export interface BindingSourceFace {
  name: string;
  type_line?: string | null;
  mana_cost?: string | null;
}
export interface BindingSourceCard extends BindingSourceFace {
  layout: string;
  card_faces?: BindingSourceFace[] | null;
}
export interface CardBindingInput {
  identity: string;
  source: BindingSourceCard;
  faces: { sourceFaceIndex: number; result?: ParseCardResult }[];
}

export type BindingPhase = "payment" | "resolution" | "continuous" | "trigger-match";
export type BindingCardinality = "one" | "many" | "unknown";
export type SymbolicAmount = Extract<Amount, { amount: "x" | "that-number" | "that-much" | "replacement-event-quantity" }>;
/** `implicit` is a player relation printed inside a filter ("an opponent
 * controls", "you don't control"). */
export type ReferenceExpression = ObjectRef | { ref: "affected-object" } | { ref: "those-counters" } | { recipient: "they" } | PlayerRef | SymbolicAmount
  | { implicit: "opponents" | "other-players" };

interface NodeBase { id: string; path: string; rules: string[] }
export type BindingNode = NodeBase & (
  | { node: "source"; identity: string }
  | { node: "face"; source: string; sourceFaceIndex: number; name: string; typeLine?: string; manaCost?: string; layout: string }
  | { node: "ability"; source: string; face: string; abilityKind: string; you: string }
  | { node: "runtime-player"; scope: string; role: "static-source-controller" | "activator" | "spell-controller" | "trigger-controller" | "prospective-caster"
      | "special-action-player" | "targeting-player" | "card-owner" | "entering-controller" }
  /** One keyword instance of this ability line, with the kinds of ability it
   * represents (CR 702); `you` is the player its printed parameters refer to. */
  | { node: "keyword-ability"; scope: string; keyword: string; abilities: KeywordAbilityKind[]; you: string }
  /** "opponents" and "other-players" are the players relative to `of` (a "you");
   * "defending" is the player the attacking creature `of` attacks (CR 508.5). */
  | { node: "derived-player"; scope: string; role: "owner" | "controller" | "opponents" | "other-players" | "defending"; of: string }
  /** The object or player the source permanent is attached to (CR 301.5f, 303.4m). */
  | { node: "derived-object"; scope: string; role: "attached-to"; of: string }
  /** The source of an ability an effect grants: each object of the bound
   * group `of` (`each`), or the object the effect creates (CR 113.10). */
  | { node: "recipient"; scope: string; of?: string; each?: boolean; created?: true }
  | { node: "selection"; scope: string; expression: ObjectRef | PlayerRef; cardinality: BindingCardinality;
      timing: BindingPhase | "stack-announcement"; selection: "target" | "evaluate-reference" }
  /** `optional`: performed only if `decider` chooses to (CR 608.2d); a missing
   * decider is unresolved. `repeated`: scaled by a "for each" count. */
  | { node: "event"; scope: string; operation: string; inputs: string[]; availability: "when-performed";
      optional?: true; decider?: string; repeated?: "for-each"; conditional?: true }
  /** "If you do" / "If you don't": the optional instruction at path `decision`
   * of the preceding sentence (CR 608.2c-d). */
  | { node: "decision-link"; scope: string; condition: "you-do" | "you-dont"; decision: string }
  /** "If it was kicked": the kicker ability printed on the same object the
   * condition refers to (CR 702.33e, 607.2). */
  | { node: "linked-ability"; scope: string; condition: "kicked"; ability: string }
  /** "Destroyed this way", "milled this way": the objects the one earlier
   * instruction `event` of the same ability acted on (CR 608.2c). */
  | { node: "event-link"; scope: string; action: string; event: string }
  /** "The rest": the objects of `pool` that the selection `excluding` did
   * not take (CR 608.2c). */
  | { node: "remainder"; scope: string; pool: string; excluding: string }
  /** Whichever of `of` the event that occurred established: one per
   * alternative trigger event ("blocks or becomes blocked by a creature",
   * CR 603.2). */
  | { node: "alternatives"; scope: string; of: string[] }
  | { node: "event-result"; event: string; result: "objects" | "quantity" | "player"; cardinality: BindingCardinality;
      description?: ObjectFilter;
      /** Result sets can be empty when an instruction is impossible or replaced. */
      availability: "if-produced"; identity: "same-objects" | "zone-successors" | "new-objects" | "measured-quantity" }
  | { node: "defined-number"; scope: string; definition: Amount }
  /** X chosen and announced by `chooser` as the spell was cast or the ability
   * activated (CR 107.3a, 107.3k), or, for a permanent's own entry, as the
   * spell it was cast (CR 107.3m). */
  | { node: "announced-number"; scope: string; chooser: string; announcement: "cast" | "activation" | "entering-spell" }
);

export type ReferenceBinding = {
  path: string;
  expression: ReferenceExpression;
  rules: string[];
} & (
  | { binding: "bound"; target: string }
  | { binding: "unresolved"; code: BindingDiagnosticCode; candidates: string[] }
);

export type BindingDiagnosticCode =
  | "invalid-source" | "missing-face" | "unparsed-face" | "face-mismatch"
  | "missing-antecedent" | "ambiguous-antecedent" | "cardinality-mismatch"
  | "missing-target-identity" | "ambiguous-target-identity" | "unsupported-scope" | "unsupported-reference"
  | "unsupported-number-binding" | "duplicate-number-definition" | "unresolved-dependency";

export interface BindingDiagnostic {
  code: BindingDiagnosticCode;
  path: string;
  message: string;
  rules: string[];
}

/** A rules-engine capability a card needs that is not reference binding: its
 * references resolve, but executing it needs the engine to provide this.
 * - event-history: a record of what happened ("if you attacked this turn",
 *   "if this spell was cast from a graveyard", "life you gained this turn"),
 *   including the spell a permanent was (CR 400.7d, 608.2i).
 * - damage-prevention: shields and redirections watching future damage
 *   events (CR 609.7, 614.9, 615).
 * - characteristic-layers: characteristic changes applied in the layer system,
 *   with the affected set fixed when the effect begins (CR 611.2c, 613). */
export type EngineFeature = "event-history" | "damage-prevention" | "characteristic-layers";
export interface EngineFeatureNeed {
  feature: EngineFeature;
  path: string;
  message: string;
  rules: string[];
}

/** Reference analysis is neither full semantic validation nor engine support.
 * referencesBound: every reference inspected here resolves, with no
 * diagnostics. engineFeatures: what the rules engine must provide beyond that.
 * fullyBound: references bound and no engine feature needed. Unknown scope
 * behavior is diagnosed and prevents both, even with zero refs. */
export interface CardBindingAnalysis {
  artifactKind: "oracle-reference-binding";
  schemaVersion: 1;
  executable: false;
  identity: string;
  rules: { edition: "2026-09-25"; url: string };
  nodes: BindingNode[];
  references: ReferenceBinding[];
  diagnostics: BindingDiagnostic[];
  counts: { obligations: number; bound: number; unresolved: number };
  referencesBound: boolean;
  engineFeatures: EngineFeatureNeed[];
  fullyBound: boolean;
}
