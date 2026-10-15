// "If this artifact has five or more charge counters on it, remove all of
// them from it": counters are not objects (CR 122.1), so "them" can only be
// the counters the governing condition just counted — every counter of that
// kind on that permanent. The condition is required to be exactly one
// counter-kind threshold on one object, printed directly before this
// instruction in the same sentence; anything else keeps the line failing.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, CounterSpec, Effect, ObjectRef } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";

const isWord = (token: Token | undefined, ...values: string[]) =>
  token?.kind === "word" && !token.possessive && values.includes(token.value);
const isPunct = (token: Token | undefined, value: string) => token?.kind === "punct" && token.value === value;

/** The counter kind and holder of "if <object> has N or more <kind> counters
 * on it," ending at `end` (the comma before the instruction). */
function governingCounters(tokens: Token[], end: number,
  parseCondition: (c: Cursor) => Condition | null): { counter: CounterSpec; on: ObjectRef } | null {
  let start = end - 1;
  while (start >= 0 && !isWord(tokens[start], "if") && !isPunct(tokens[start], ",") && !isPunct(tokens[start], ".")) start--;
  if (start < 0 || !isWord(tokens[start], "if")) return null;
  // The conditional opens its sentence or the effect after a trigger.
  const before = tokens[start - 1];
  if (before && !isPunct(before, ".") && !isPunct(before, ",") && !isWord(before, "then")) return null;
  if (isWord(before, "then") && tokens[start - 2] && !isPunct(tokens[start - 2], ".")) return null;
  const body = new Cursor(tokens.slice(start + 1, end));
  const condition = parseCondition(body);
  if (!condition || !body.done() || condition.condition !== "is-filter") return null;
  const constraints = condition.filter.counterConstraints;
  if (Object.keys(condition.filter).length !== 1 || constraints?.length !== 1 || constraints[0].counter === "any") return null;
  return { counter: constraints[0].counter, on: condition.what };
}

/** "remove all of them [from it]" / "remove them all [from it]" / "remove
 * those counters [from it]" (the same counters, named again), read after
 * the verb. `prefix` is the number of tokens between the condition's comma
 * and the verb ("you may" = 2). */
export function parseRemoveCountedCounters(c: Cursor, verb: number, parseCondition: (c: Cursor) => Condition | null,
  parseObjectRef: (c: Cursor) => ObjectRef | null): Effect | null {
  return c.attempt((c): Effect | null => {
    if (!c.words("all", "of", "them") && !c.words("them", "all") && !c.words("those", "counters")) return null;
    let comma = verb - 1;
    if (isWord(c.tokens[comma], "may")) comma -= 2;
    if (!isPunct(c.tokens[comma], ",")) return null;
    const governing = governingCounters(c.tokens, comma, parseCondition);
    if (!governing) return null;
    const from = c.attempt(c => c.word("from") !== null ? parseObjectRef(c) : null);
    if (from && JSON.stringify(from) !== JSON.stringify(governing.on) && !(from.ref === "it" && governing.on.ref !== "it")) return null;
    return { effect: "remove-counters", counter: governing.counter, count: "all", from: from ?? governing.on };
  });
}
