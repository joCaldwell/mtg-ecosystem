// "You may pay {W} to end this effect." CR 116.2c: an effect can let a
// player end a continuous effect later by taking a special action. "This
// effect" is the lasting effect the same ability just created.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";

type EndAction = Extract<Effect, { effect: "end-effect-action" }>;

/** Lasting effects that end only when something ends them. */
const OPEN_ENDED = new Set<Effect["effect"]>(["lose-abilities", "modify-characteristics", "become", "gain-abilities"]);

function createsOpenEndedEffect(sentences: Sentence[]): boolean {
  return sentences.some(sentence => !sentence.condition && sentence.steps.some(step => OPEN_ENDED.has(step.effect) && !step.duration && !step.optional));
}

/** The complete sentence, including its period. Returns null when it does
 * not start here, false when there is no open-ended effect for it to end. */
export function parseEndEffectAction(c: Cursor, earlier: Sentence[]): EndAction | false | null {
  const start = c.pos;
  const costs = c.attempt(c => {
    if (!c.words("you", "may", "pay")) return null;
    const costs = parseCostList(c);
    if (!costs || !c.words("to", "end", "this", "effect")) return null;
    return c.punct(".") || c.done() ? costs : null;
  });
  if (!costs) return null;
  if (!createsOpenEndedEffect(earlier)) { c.pos = start; return false; }
  return { effect: "end-effect-action", who: { player: "you" }, costs, ends: "open-ended-effects-of-this-resolution" };
}
