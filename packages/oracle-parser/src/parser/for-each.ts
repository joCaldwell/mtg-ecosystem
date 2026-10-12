// Explicit resolution instructions for each member of a printed set.
// CR 608.2e/f, 101.4: each action spans the members before the next action;
// this is not a sequential JavaScript-style loop over whole instruction lists.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, parseObjectRef, parsePlayerRef } from "./refs.ts";

function containsLoop(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(containsLoop);
  if ("effect" in value && value.effect === "for-each") return true;
  return Object.values(value).some(containsLoop);
}

/** The callback must use a fresh resolution context: enclosing actor elision,
 * stack-copy input proof and dice-table scope do not enter this body.
 * Initially require a terminal single body sentence. Singular continuations
 * can remain per-member while plural ones can aggregate outputs; accepting
 * either without its scope would lose meaning (Fighting Chance / Saheeli).
 * Activation restrictions are handled separately by the outer ability. */
export function parseForEach(c: Cursor, parseBody: (body: Cursor) => Sentence[] | null): Effect | null {
  return c.attempt(c => {
    if (c.word("for") === null || !c.isWord("each")) return null;
    const following = c.peek(1);
    const establishedGroup = following?.kind === "word" && !following.possessive && following.value === "of";
    const each = c.attempt(c => {
      const player = parsePlayerRef(c);
      return player && isPluralPlayerRef(player) ? player : null;
    }) ?? c.attempt(c => {
      const object = parseObjectRef(c);
      return object?.ref === "each" ? object : null;
    });
    if (!each || !c.punct(",")) return null;

    const start = c.pos;
    let end = start;
    let quoted = false;
    for (; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "punct" && token.value === '"') quoted = !quoted;
      if (!quoted && token.kind === "punct" && token.value === ".") break;
    }
    if (quoted || end === start) return null;
    const after = c.tokens[end + 1];
    if (after && !(after.kind === "word" && !after.possessive && after.value === "activate"))
      return c.fail("a fully scoped continuation after quantified instructions");
    const body = new Cursor(c.tokens.slice(start, end));
    const effects = parseBody(body);
    if (!body.done() || effects?.length !== 1 || containsLoop(effects)) return null;
    // CR 608.2f (Soulfire Eruption example): a dependent action sequence for
    // each member of an established group is completed member by member when
    // it cannot be simultaneous. That is not instruction-wise processing, so
    // only a single action is accepted for "For each of <group>, ...".
    if (establishedGroup && effects[0].steps.length !== 1)
      return c.fail("a single action for each member of an established group");
    c.pos = end;
    return { effect: "for-each", each, effects, processing: "instruction-wise" };
  });
}
