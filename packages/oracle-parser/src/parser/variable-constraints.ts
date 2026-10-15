// Printed limits on a chosen X: "X can't be 0." / "X can't be greater than
// [amount]." CR 107.3a/d/f: the controller chooses X while casting,
// activating, taking the special action, or resolving; these sentences limit
// that choice and are never instructions of the resolving ability.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { VariableConstraint } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount } from "./refs.ts";

function parseConstraint(c: Cursor): VariableConstraint | null {
  return c.attempt((c): VariableConstraint | null => {
    if (!c.words("x", "can't", "be")) return null;
    let constraint: VariableConstraint;
    if (c.words("greater", "than")) {
      const maximum = parseAmount(c);
      if (!maximum || maximum.amount === "x") return null;
      constraint = { variable: "x", maximum };
    } else {
      const value = c.number();
      if (value === null) return null;
      constraint = { variable: "x", not: value };
    }
    return c.punct(".") || c.done() ? constraint : null;
  });
}

/** Whole sentences only; the caller owns what may follow them. */
export function parseVariableConstraints(c: Cursor): VariableConstraint[] | null {
  const constraints: VariableConstraint[] = [];
  for (;;) {
    const next = parseConstraint(c);
    if (!next) break;
    constraints.push(next);
  }
  return constraints.length ? constraints : null;
}

/** A constraint needs an X of the same ability to limit: an {X} symbol in
 * a cost or an X amount in its text. */
export function mentionsX(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(mentionsX);
  if (!value || typeof value !== "object") return value === "X" || value === "x";
  return Object.values(value).some(mentionsX);
}
