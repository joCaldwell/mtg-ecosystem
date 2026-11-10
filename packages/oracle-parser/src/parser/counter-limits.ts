// "This ability can't cause the total number of +1/+0 counters on this creature
// to be greater than four." limits the counters the ability's preceding
// "put up to X counters" instruction adds (Clockwork Beast): the player may
// choose only as many as keep the object at or below the stated total.
// The sentence is consumed only when it follows exactly that instruction.
import type { Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseCounterSpec, parseObjectRef } from "./refs.ts";

/** true: folded into the preceding put-counters step; false: not this
 * sentence; null: this sentence, but it has no unambiguous instruction to limit. */
export function foldCounterTotalLimit(c: Cursor, previous: Sentence | undefined): boolean | null {
  const start = c.pos;
  if (!c.words("this", "ability", "can't", "cause", "the", "total", "number", "of")) return false;
  const parsed = c.attempt(c => {
    const counter = parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null || c.word("on") === null) return null;
    const on = parseObjectRef(c);
    if (!on || !c.words("to", "be", "greater", "than")) return null;
    const maximum = parseAmount(c);
    if (!maximum) return null;
    c.punct(".");
    return { counter, on, maximum };
  });
  const step = previous?.steps.length === 1 && !previous.condition && !previous.otherwise ? previous.steps[0] : undefined;
  if (!parsed || step?.effect !== "put-counters" || step.maxTotal || step.optional || step.forEach ||
      JSON.stringify(step.counter) !== JSON.stringify(parsed.counter) || JSON.stringify(step.on) !== JSON.stringify(parsed.on)) {
    c.pos = start;
    return null;
  }
  step.maxTotal = parsed.maximum;
  return true;
}
