// Imperative actions that belong to no larger family: removal from combat,
// kind-agnostic or "up to" counter removal, and the blight/recruit keyword
// actions. CR 506.4, 122.1, 701.68, 701.70:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCount, parseCounterSpec, parseObjectRef } from "./refs.ts";

export interface MiscEffectGrammar {
  /** Imperative verbs act for you; conjugated ones keep the stated actor. */
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
}

/** "from combat" names the combat relationship being ended, never a zone or
 * a selection pool. Slice the object phrase so refs.ts's separate
 * "a card from <pool>" grammar cannot swallow it. */
function parseRemoveFromCombat(c: Cursor): Effect | null {
  return c.attempt(c => {
    if (c.word("remove") === null) return null;
    const start = c.pos;
    let from = -1;
    for (let i = start; i + 1 < c.tokens.length; i++) {
      const token = c.tokens[i], next = c.tokens[i + 1];
      if (token.kind === "punct") break;
      if (token.kind === "word" && !token.possessive && token.value === "from" &&
          next.kind === "word" && !next.possessive && next.value === "combat") { from = i; break; }
    }
    if (from <= start) return null;
    const subject = new Cursor(c.tokens.slice(start, from));
    const what = parseObjectRef(subject);
    if (!what || !subject.done()) return c.fail("object removed from combat");
    c.pos = from + 2;
    return { effect: "remove-from-combat", what };
  });
}

/** Only the forms the kind-specific production rejects: an unnamed counter
 * (the remover chooses among the kinds present) and/or an "up to" count.
 * The count is a plain number; arithmetic stays with the typed production. */
function parseRemoveCounters(c: Cursor): Effect | null {
  return c.attempt(c => {
    if (c.word("remove") === null) return null;
    const upTo = c.words("up", "to");
    const count: Amount | "all" | null = !upTo && c.word("all") !== null ? "all" : parseCount(c);
    if (!count) return null;
    const counter = c.isWord("counter", "counters") ? "any" as const : parseCounterSpec(c);
    if (!counter || (!upTo && counter !== "any")) return null;
    if (c.word("counter", "counters") === null || c.word("from") === null) return null;
    const from: ObjectRef | null = parseObjectRef(c);
    return from && { effect: "remove-counters", counter, count, from, ...(upTo ? { upTo: true as const } : {}) };
  });
}

export function parseMiscEffect(c: Cursor, g: MiscEffectGrammar): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const combat = parseRemoveFromCombat(c);
    if (combat) return [combat];
    const counters = parseRemoveCounters(c);
    if (counters) return [counters];
    const blight = c.attempt((c): Effect | null => {
      const who = g.actor(c, "blight", "blights");
      const amount = who && parseCount(c);
      return who && amount ? { effect: "blight", who, amount } : null;
    });
    if (blight) return [blight];
    const recruit = c.attempt((c): Effect | null => {
      const who = g.actor(c, "recruit", "recruits");
      return who && { effect: "keyword-action", action: "recruit", who };
    });
    return recruit ? [recruit] : null;
  });
}
