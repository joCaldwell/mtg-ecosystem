// How the reference graph accounts for each printed AST step.
//
// Every `Effect` step is one of two things. A structural step organizes other
// instructions and is accounted for by the scope it opens; its member steps,
// branches or nested abilities carry their own events. Every other step is a
// printed instruction: when the binder records it, it records one `event` node
// at the step's own path whose `operation` is the step's AST discriminant
// (`Effect["effect"]`). Results, decisions and other side events may sit at the
// same path under their own operation names; they never replace the step's
// event. A step the binder cannot account for is a diagnostic, not a renamed
// or missing event.
//
// This table is the only list of structural steps. Consumers (the Rust
// composition lowerer checks archived graphs against the AST) read it here
// rather than keeping their own exemptions.
// CR 608.2, 608.2c, 603.7, 603.12, 614.1, 700.2, 101.4:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect } from "../ast.ts";

/** Every AST step discriminant. */
export type StepEffect = Effect["effect"];

/** The scope a structural step opens. */
export type StructuralScope =
  /** CR 700.2, 101.4, 608.2c: modes, simultaneous parts or one optional group;
   * each member step is bound and evented separately. */
  | "member-steps"
  /** CR 603.7, 603.12: a delayed or reflexive triggered ability with its own
   * ability scope and effects. */
  | "nested-ability"
  /** CR 608.2c: instructions performed once per member of a group. */
  | "iteration"
  /** CR 614.1: a replacement of a printed or upcoming event; the replaced and
   * replacing events are bound where they are spelled. */
  | "replacement"
  /** CR 611.2, 613.1: a continuous effect on characteristics; its operands are
   * bound as references, and no event happens when it is created. */
  | "continuous-effect";

export const STRUCTURAL_STEPS = {
  "modal": "member-steps",
  "simultaneous": "member-steps",
  "optional-group": "member-steps",
  "create-delayed-trigger": "nested-ability",
  "create-reflexive-trigger": "nested-ability",
  "for-each": "iteration",
  "self-replacement": "replacement",
  "create-replacement": "replacement",
  "modify-characteristics": "continuous-effect",
} as const satisfies Partial<Record<StepEffect, StructuralScope>>;

export type StructuralStep = keyof typeof STRUCTURAL_STEPS;
/** A printed instruction step: its event carries this discriminant. */
export type InstructionStep = Exclude<StepEffect, StructuralStep>;

export type StepObligation =
  | { kind: "event"; operation: InstructionStep }
  | { kind: "scope"; scope: StructuralScope };

export function isStructuralStep(effect: StepEffect): effect is StructuralStep {
  return Object.hasOwn(STRUCTURAL_STEPS, effect);
}

/** What the reference graph must hold at a step's own path. */
export function stepObligation(effect: StepEffect): StepObligation {
  return isStructuralStep(effect)
    ? { kind: "scope", scope: STRUCTURAL_STEPS[effect] }
    : { kind: "event", operation: effect };
}

/** The event operation the binder records for a printed instruction step. */
export function stepEventOperation(effect: InstructionStep): InstructionStep {
  return effect;
}
