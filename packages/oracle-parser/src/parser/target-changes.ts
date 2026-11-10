// Changing the targets of a spell or ability on the stack. CR 115.7a–c
// distinguish "the target(s)" (all or none), "a target" (one) and "any
// targets" (any number); each differs from choosing new targets (115.7d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectFilter, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";

type ChangeTargets = Extract<Effect, { effect: "change-targets" }>;

/** Only spells and abilities have targets to change (CR 115.1). */
function isStackObjectFilter(filter: ObjectFilter): boolean {
  if (filter.cls === "spell" || filter.cls === "ability") return true;
  return !!filter.allOf?.some(part => !!part.anyOf?.length && part.anyOf.every(isStackObjectFilter));
}

/** The affected spell or ability must be an explicit reference; the acting
 * player is the controller of this instruction ("you"). A following "The
 * new target must be …" sentence restricts the same change (CR 608.2c) and
 * is consumed here, leaving the sentence's final period to the caller. */
export function parseChangeTargets(c: Cursor): ChangeTargets | null {
  return c.attempt((c): ChangeTargets | null => {
    if (c.word("change") === null) return null;
    const scope: ChangeTargets["scope"] | null = c.words("the", "target") || c.words("the", "targets") ? "all"
      : c.words("a", "target") ? "one" : c.words("any", "targets") ? "any" : null;
    if (!scope || c.word("of") === null) return null;
    const what = parseObjectRef(c);
    if (!what || !("filter" in what) || !isStackObjectFilter(what.filter)) return c.fail("spell or ability whose targets change");
    const to = c.attempt((c): ObjectRef | PlayerRef | null =>
      c.word("to") !== null ? c.attempt(parsePlayerRef) ?? parseObjectRef(c) : null);
    const newTarget = to ? null : c.attempt((c): ChangeTargets["newTarget"] | null => {
      if (!c.punct(".") || !c.words("the", "new", "target", "must", "be")) return null;
      if (c.words("a", "player")) return { kind: "player" };
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && { kind: "object", filter };
    });
    return { effect: "change-targets", what, who: { player: "you" }, scope,
      ...(to ? { to } : {}), ...(newTarget ? { newTarget } : {}) };
  });
}
