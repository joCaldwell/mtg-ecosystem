// Turn-step untapping restrictions, distinct from an unconditional "can't
// untap" instruction. CR 502.3, 611.2a and 611.3 govern the affected action,
// durations, and the difference between resolution-created and static effects.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, StaticEffect, UntapStepWindow } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { isTargetedPlayerRef, parseCondition, parseDuration, parseObjectRef, targetSelectionProvenance } from "./refs.ts";

function parseStepOwner(c: Cursor): UntapStepWindow["owner"] | null {
  return c.attempt(c => {
    const affectedController = c.attempt(c => {
      if (c.word("its", "their") === null || c.possessive("controller", "controllers") === null) return null;
      return "each-affected-objects-controller" as const;
    });
    if (affectedController) return affectedController;
    if (c.attempt(c => c.word("your") !== null && c.possessive("opponents") !== null ? true : null))
      return { player: "each-opponent" };
    if (c.word("your") !== null) return { player: "you" };
    if (c.word("their") !== null) return { player: "they" };
    if (c.attempt(c => c.word("that", "the") !== null && c.possessive("player") !== null ? true : null))
      return { player: "that-player" };
    const target = c.peek();
    if (c.attempt(c => c.word("target") !== null && c.possessive("player") !== null ? true : null))
      return { player: "target-player", ...targetSelectionProvenance(target) };
    if (c.attempt(c => c.word("target") !== null && c.possessive("opponent") !== null ? true : null))
      return { player: "target-opponent", ...targetSelectionProvenance(target) };
    if (c.attempt(c => c.word("each") !== null && c.possessive("player") !== null ? true : null))
      return { player: "each-player" };
    if (c.attempt(c => c.word("each") !== null && c.possessive("opponent") !== null ? true : null))
      return { player: "each-opponent" };
    return c.fail("untap-step owner");
  });
}

function parseSuppression(c: Cursor, previousSubject?: ObjectRef): { what: ObjectRef; step: UntapStepWindow } | null {
  return c.attempt(c => {
    const what = parseObjectRef(c) ?? previousSubject;
    if (!what || c.word("doesn't", "don't", "can't") === null || !c.words("untap", "during")) return null;
    const owner = parseStepOwner(c);
    if (!owner) return null;
    const occurrence = c.word("next") !== null ? "next" : "each";
    if (c.word("untap") === null || c.word("step", "steps") === null) return null;
    return { what, step: { occurrence, owner } };
  });
}

/** Complete static clause: repeated suppression follows the represented step
 * owner. A "next" occurrence is a resolution-created effect, not a static
 * instruction to skip an indefinitely moving "next" step. */
export function parseStaticUntapSuppression(c: Cursor, subject?: ObjectRef): StaticEffect | null {
  return c.attempt(c => {
    // A coordinated clause ("~ enters tapped and doesn't untap …") supplies
    // its explicit preceding subject; nothing is inferred from context.
    const result = parseSuppression(c, subject);
    if (!result || result.what.ref === "target" || result.step.occurrence === "next") return null;
    if (typeof result.step.owner !== "string" &&
        isTargetedPlayerRef(result.step.owner)) return null;
    let effect: StaticEffect = { static: "untap-step-suppression", ...result };
    const qualifier = c.word("if", "unless");
    if (qualifier) {
      const condition = parseCondition(c);
      if (!condition) return null;
      effect = { static: "conditional", effect,
        condition: qualifier === "unless" ? { condition: "not", of: condition } : condition };
    } else {
      const duration = parseDuration(c);
      if (duration) {
        if (duration.duration !== "as-long-as") return null;
        effect = { static: "conditional", effect, condition: duration.condition };
      }
    }
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return c.fail("end of static untap restriction");
    return effect;
  });
}

/** Parse a resolving instruction. The enclosing effect grammar retains any
 * following duration/condition. The symbolic "they" player and the explicit
 * controllers of affected objects are deliberately different references. */
export function parseUntapSuppressionEffect(c: Cursor, previousSubject?: ObjectRef): Effect[] | null {
  return c.attempt(c => {
    const result = parseSuppression(c, previousSubject);
    return result ? [{ effect: "untap-step-suppression", ...result }] : null;
  });
}
