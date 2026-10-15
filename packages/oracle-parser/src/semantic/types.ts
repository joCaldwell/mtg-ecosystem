import type { Amount, ObjectFilter, ObjectRef, ParseCardResult, PlayerRef } from "../ast.ts";

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
export type ReferenceExpression = ObjectRef | { ref: "affected-object" } | { ref: "those-counters" } | { recipient: "they" } | PlayerRef | SymbolicAmount;

interface NodeBase { id: string; path: string; rules: string[] }
export type BindingNode = NodeBase & (
  | { node: "source"; identity: string }
  | { node: "face"; source: string; sourceFaceIndex: number; name: string; typeLine?: string; manaCost?: string; layout: string }
  | { node: "ability"; source: string; face: string; abilityKind: string; you: string }
  | { node: "runtime-player"; scope: string; role: "static-source-controller" | "activator" | "spell-controller" | "trigger-controller" | "prospective-caster" }
  | { node: "derived-player"; scope: string; role: "owner" | "controller"; of: string }
  | { node: "selection"; scope: string; expression: ObjectRef | PlayerRef; cardinality: BindingCardinality;
      timing: BindingPhase | "stack-announcement"; selection: "target" | "evaluate-reference" }
  | { node: "event"; scope: string; operation: string; inputs: string[]; availability: "when-performed" }
  | { node: "event-result"; event: string; result: "objects" | "quantity" | "player"; cardinality: BindingCardinality;
      description?: ObjectFilter;
      /** Result sets can be empty when an instruction is impossible or replaced. */
      availability: "if-produced"; identity: "same-objects" | "zone-successors" | "new-objects" | "measured-quantity" }
  | { node: "defined-number"; scope: string; definition: Amount }
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

/** Reference analysis is neither full semantic validation nor engine support.
 * fullyBound describes only the reference obligations inspected here. Unknown
 * scope behavior is diagnosed and prevents fullyBound, even with zero refs. */
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
  fullyBound: boolean;
}
