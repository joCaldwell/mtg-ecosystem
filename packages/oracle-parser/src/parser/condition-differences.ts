// "… equal to the difference": the difference between the two quantities
// compared by the condition that governs the instruction ("If you have fewer
// than seven cards in hand, draw cards equal to the difference"; "if it had
// power greater than ~'s power, put a number of +1/+1 counters on ~ equal to
// the difference"). The printed comparison determines both operands, so the
// amount is that subtraction (larger minus smaller, as the comparison holds
// when the instruction is carried out, CR 603.4, 608.2c), not a contextual
// reference left for binding. Only a governing condition that compares one
// quantity of a single referent with another supplies it; anything else
// (no condition, a quantified player, a history or result test) blocks
// "the difference", so it fails instead of reaching an outer condition.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, PlayerRef, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type Difference = Extract<Amount, { amount: "minus" }>;

const scopes: (Difference | null)[] = [];

/** Parse `body` with "the difference" defined by `condition` (null: none).
 * `inherit` keeps the enclosing definition when there is no condition. */
export function withConditionDifference<T>(condition: Condition | undefined, body: () => T, inherit = false): T {
  if (!condition && inherit) return body();
  scopes.push(condition ? differenceOf(condition) : null);
  try { return body(); } finally { scopes.pop(); }
}

/** "the difference" inside an instruction governed by a comparison. */
export function parseConditionDifference(c: Cursor): Amount | null {
  const difference = scopes.at(-1);
  if (!difference) return null;
  return c.attempt(c => {
    if (!c.words("the", "difference")) return null;
    // "the difference between those results" is a dice amount.
    if (c.isWord("between")) return null;
    return structuredClone(difference);
  });
}

function handOwner(who: PlayerRef): ZoneRef["owner"] | null {
  if (who.player === "you") return "your";
  if (who.player === "that-player") return "that-player";
  if (who.player === "target-player" || who.player === "target-opponent") return who;
  return null;
}

/** The quantity measured for the condition's referent. */
function measured(condition: Condition): Amount | null {
  switch (condition.condition) {
    case "cards-in-hand": {
      const owner = handOwner(condition.who);
      return owner ? { amount: "count", of: { cls: "card", zone: { zone: "hand", owner } } } : null;
    }
    case "count": return { amount: "count", of: condition.of };
    case "life-total": return { amount: "life-total", of: condition.who };
    case "compare": return condition.value;
    case "is-filter": {
      const keys = Object.keys(condition.filter);
      const property = condition.filter.properties;
      if (keys.length !== 1 || property?.length !== 1) return null;
      if (!["it", "self", "this", "that", "target"].includes(condition.what.ref)) return null;
      return { amount: "attribute", of: condition.what, attribute: property[0].property };
    }
    default: return null;
  }
}

function differenceOf(condition: Condition): Difference | null {
  const comparison = condition.condition === "is-filter" ? condition.filter.properties?.[0]?.comparison
    : "comparison" in condition ? condition.comparison : undefined;
  if (!comparison || comparison.op === "eq") return null;
  const value = measured(condition);
  if (!value) return null;
  return comparison.op === "gt" || comparison.op === "ge"
    ? { amount: "minus", a: value, b: comparison.value }
    : { amount: "minus", a: comparison.value, b: value };
}
