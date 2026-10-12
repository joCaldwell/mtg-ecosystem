// Rules-defined actions on existing objects. In particular, transformation
// and phasing do not exile/return the object, and attaching does not transfer
// control. CR 701.3, 701.27–28, 702.26:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef, parsePlayerRef } from "./refs.ts";

type Verb = "transform" | "convert" | "attach" | "unattach";
const verbs: Record<string, Verb> = {
  transform: "transform", transforms: "transform", convert: "convert", converts: "convert",
  attach: "attach", attaches: "attach", unattach: "unattach", unattaches: "unattach",
};

function parseEndpoint(c: Cursor): ObjectRef | PlayerRef | null {
  return parsePlayerRef(c) ?? parseObjectRef(c);
}

/** In "unattach X from Y", from describes the old attachment endpoint,
 * never a zone or selection pool. The local slice keeps refs.ts's separate
 * "a card from it" selection grammar from swallowing that endpoint. */
function parseUnattachment(c: Cursor, who: PlayerRef): Effect | null {
  return c.attempt(c => {
    const start = c.pos;
    let fromIndex = -1;
    for (let i = start; i < c.tokens.length; i++) {
      const token = c.tokens[i];
      if (token.kind === "punct") break;
      if (token.kind === "word" && !token.possessive && ["then", "and"].includes(token.value)) break;
      if (token.kind === "word" && !token.possessive && token.value === "from") { fromIndex = i; break; }
    }
    let what: ObjectRef | null;
    if (fromIndex < 0) what = parseObjectRef(c);
    else {
      const source = new Cursor(c.tokens.slice(start, fromIndex));
      what = parseObjectRef(source);
      if (!what || !source.done()) return null;
      c.pos = fromIndex;
    }
    if (!what) return null;
    if (c.word("from") === null) return { effect: "unattach", what, who };
    const from = parseEndpoint(c);
    return from && { effect: "unattach", what, from, who };
  });
}

function parsePlayerAction(c: Cursor, lastPlayer?: PlayerRef): Effect | null {
  return c.attempt(c => {
    const explicit = parsePlayerRef(c);
    const token = c.word(...Object.keys(verbs));
    if (!token) return null;
    const verb = verbs[token];
    // Match the enclosing effect grammar: bare imperative = you; an elided
    // conjugated verb retains its stated actor, including singular "they".
    const who = explicit ?? (token === verb && lastPlayer?.player !== "they" ? { player: "you" as const } : lastPlayer);
    if (!who) return c.fail("actor of object action");
    if (verb === "unattach") return parseUnattachment(c, who);
    const what = parseObjectRef(c);
    if (!what) return null;
    if (verb === "attach") {
      if (c.word("to") === null) return null;
      const to = parseEndpoint(c);
      return to && { effect: "attach", what, to, who };
    }
    // Meld uses a named resulting face and its own zone-changing action;
    // neither meld nor a generic "turn face up" is transformation syntax.
    if (c.word("into") !== null) return c.fail("transform/convert without named-result replacement");
    return { effect: verb, what, who };
  });
}

function parseObjectAction(c: Cursor, lastSubject?: ObjectRef): Effect[] | null {
  return c.attempt(c => {
    const first = parseObjectRef(c) ?? lastSubject;
    if (!first) return null;
    const change = c.word("transforms", "converts");
    if (change) return [{ effect: change === "transforms" ? "transform" : "convert", what: first }];
    // A coordinated subject ("this creature and that creature phase out") is
    // one simultaneous event over every listed object (CR 702.26).
    const subjects = [first];
    while (c.isWord("and")) {
      const more = c.attempt(c => c.word("and") !== null ? parseObjectRef(c) : null);
      if (!more) return null;
      subjects.push(more);
    }
    if (c.word("phase", "phases") === null) return null;
    const direction = c.word("out", "in");
    return direction ? subjects.map((what): Effect => ({ effect: direction === "out" ? "phase-out" : "phase-in", what })) : null;
  });
}

/** Resolving instruction only; recurring phasing or can't-phase-in rules are
 * static abilities and are deliberately not reclassified here. Null restores
 * the cursor, and this function never mutates the caller's elision context. */
export function parseObjectActions(c: Cursor, lastSubject?: ObjectRef, lastPlayer?: PlayerRef): Effect[] | null {
  return c.attempt(c => {
    const player = parsePlayerAction(c, lastPlayer);
    return player ? [player] : parseObjectAction(c, lastSubject);
  });
}
