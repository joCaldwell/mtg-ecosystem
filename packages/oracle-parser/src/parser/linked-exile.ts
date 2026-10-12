// CR 610.3: "exile ... until [an event]" creates linked one-shot effects.
// The return is not a triggered ability, uses the object's previous zone, and
// must not exile anything if the return event has already occurred (610.3a/b).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCount, parseCounterSpec, parseObjectRef } from "./refs.ts";

type ExileCounters = NonNullable<Extract<Effect, { effect: "exile" }>["counters"]>;

/** "Exile it with three time counters on it" (the wording of CR 702.62a):
 * the card has the counters as it arrives in exile, in the same action. Only
 * one definite card is supported, so "on it" can mean nothing else. */
export function parseExileCounters(c: Cursor, what: ObjectRef[]): ExileCounters | null {
  return c.attempt(c => {
    if (what.length !== 1 || !["self", "this", "it"].includes(what[0].ref) && !(what[0].ref === "that" && !what[0].plural)) return null;
    if (c.word("with") === null) return null;
    const count = parseCount(c);
    const counter = count && parseCounterSpec(c);
    if (!count || !counter || c.word("counter", "counters") === null || !c.words("on", "it")) return null;
    return [{ counter, count }];
  });
}

/** Called after the complete exile object list and optional source zone.
 * All objects belong to one simultaneous move and linked return group.
 * Other event forms fail until their semantics have dedicated representation. */
export function parseExileUntil(c: Cursor, what: ObjectRef[], from?: ZoneRef): Effect | null {
  return c.attempt(c => {
    if (!what.length || c.word("until") === null) return null;
    const eventObject = parseObjectRef(c);
    if (!eventObject || !c.words("leaves", "the", "battlefield")) return null;
    return {
      effect: "exile-until", what, ...(from ? { from } : {}),
      until: { event: "leaves-battlefield", what: eventObject },
      return: { to: "previous-zone", battlefieldController: "owner" },
    };
  });
}
