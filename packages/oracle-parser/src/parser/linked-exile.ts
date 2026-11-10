// CR 610.3: "exile ... until [an event]" creates linked one-shot effects.
// The return is not a triggered ability, uses the object's previous zone, and
// must not exile anything if the return event has already occurred (610.3a/b).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
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

/** "Exile that card with three time counters on it" / "exile target nonland
 * card from your graveyard with two time counters on it": the reference
 * grammar reads the tail as a counter requirement on the card. A card that
 * is about to be exiled from a zone other than the battlefield has no
 * counters (CR 122.2), so an exact count printed there is this exile's own
 * entry modifier. Returns the corrected reference and its counters. */
export function liftCardExileCounters(what: ObjectRef[]): { what: ObjectRef[]; counters: ExileCounters } | null {
  if (what.length !== 1) return null;
  const ref = what[0];
  if ((ref.ref !== "that" || ref.plural) && !((ref.ref === "target" || ref.ref === "filter") && ref.count.amount === "fixed" && ref.count.value === 1 && !ref.upTo)) return null;
  const { counterConstraints, ...filter } = ref.filter;
  if (filter.cls !== "card" || !counterConstraints || counterConstraints.length !== 1) return null;
  if (filter.zone && (filter.zone.zone === "battlefield" || filter.zone.zone === "exile")) return null;
  const [{ counter, comparison }] = counterConstraints;
  if (counter === "any" || comparison.op !== "eq") return null;
  return { what: [{ ...ref, filter }], counters: [{ counter, count: comparison.value }] };
}

/** "Exile that card with a dream counter on it": the reference grammar
 * would read "with a dream counter on it" as a requirement on the card,
 * which liftCardExileCounters cannot tell apart from "one or more". A
 * definite card off the battlefield has no counters to test (CR 122.2), so
 * the tail is this exile's own counters. */
export function parseDefiniteCardExileCounters(c: Cursor): { what: ObjectRef[]; counters: ExileCounters } | null {
  return c.attempt(c => {
    let at = c.pos;
    while (at < c.tokens.length && !(c.tokens[at].kind === "word" && (c.tokens[at] as { value: string }).value === "with")) at++;
    if (at >= c.tokens.length) return null;
    const head = new Cursor(c.tokens.slice(c.pos, at));
    const ref = parseObjectRef(head);
    if (!ref || !head.done() || ref.ref !== "that" || ref.plural || ref.filter.cls !== "card" || ref.filter.zone || ref.filter.counterConstraints) return null;
    c.pos = at;
    const counters = parseExileCounters(c, [ref]);
    return counters && { what: [ref], counters };
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
