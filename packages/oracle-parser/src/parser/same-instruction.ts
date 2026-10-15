import type { Effect, PlayerRef, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isTargetedPlayerRef, parsePlayerRef } from "./refs.ts";

/** Effects whose only participant is the player who performs them, so the
 * same instruction can be restated for another actor by changing `who`. */
const ACTOR_ONLY = new Set<Effect["effect"]>(["draw", "gain-life", "lose-life", "create-token", "mill", "scry", "surveil"]);

/** "Each opponent attacking that player does the same." (CR 608.2c): the
 * named players perform the directly preceding instruction themselves. Only
 * a lone, unconditional, untargeted instruction of its controller is
 * restated; anything richer (choices, pronouns, riders) has no honest copy
 * and the sentence stays unparsed. */
export function parseDoesTheSame(c: Cursor, previous: Sentence | undefined): Sentence | null {
  return c.attempt((c): Sentence | null => {
    if (!previous || previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 1) return null;
    const step = previous.steps[0];
    const actor = (step as { who?: PlayerRef }).who;
    if (!ACTOR_ONLY.has(step.effect) || (actor && actor.player !== "you")) return null;
    if (step.optional || step.forEach || step.decidedBy || /"(it|them|they|that-player|that-much|the-player)"|"target/.test(JSON.stringify(step))) return null;
    const who = parsePlayerRef(c);
    if (!who || who.player === "you" || isActorPronoun(who) || isTargetedPlayerRef(who)) return null;
    if (!c.words("does", "the", "same") || !c.punct(".") && !c.done()) return null;
    return { steps: [{ ...step, who } as Effect] };
  });
}

function isActorPronoun(who: PlayerRef): boolean {
  return who.player === "they" || who.player === "that-player" || who.player === "the-player";
}
