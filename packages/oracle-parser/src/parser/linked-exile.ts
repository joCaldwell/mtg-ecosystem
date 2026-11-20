// CR 610.3: "exile ... until [an event]" creates linked one-shot effects.
// The return is not a triggered ability, uses the object's previous zone, and
// must not exile anything if the return event has already occurred (610.3a/b).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef, Sentence, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCount, parseCounterSpec, parseObjectRef, parsePlayerRef } from "./refs.ts";

type ExileCounters = NonNullable<Extract<Effect, { effect: "exile" }>["counters"]>;

/** "Exile it with three time counters on it" (the wording of CR 702.62a):
 * the card has the counters as it arrives in exile, in the same action. Only
 * one definite card is supported, so "on it" can mean nothing else. */
export function parseExileCounters(c: Cursor, what: ObjectRef[], from?: ZoneRef): ExileCounters | null {
  return c.attempt(c => {
    if (what.length !== 1 || c.word("with") === null) return null;
    const [object] = what;
    if (object.ref === "filter" && object.count.amount === "fixed" && object.count.value === 1
      && object.filter.cls === "card" && !object.upTo && !object.filter.counterConstraints) {
      const zone = from ?? object.filter.zone;
      if (zone?.zone !== "hand" || zone.owner !== "your"
        || !c.words("a", "number", "of", "time", "counters", "on", "it", "equal", "to", "its", "mana", "value")) return null;
      // CR 122.1, 702.62a: derive the exile counters from the selected card.
      return [{ counter: { type: "time" }, count: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }];
    }
    if (!["self", "this", "it", "trigger-event-object"].includes(object.ref) && !(object.ref === "that" && !object.plural)) return null;
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
export function parseExileUntil(c: Cursor, what: ObjectRef[], from?: ZoneRef): Extract<Effect, { effect: "exile-until" }> | null {
  return c.attempt(c => {
    if (!what.length || c.word("until") === null) return null;
    const until = c.attempt(c => {
      // CR 725.2: the change of designation is an event, not a state test.
      const who = parsePlayerRef(c);
      return who?.player === "an-opponent" && c.words("becomes", "the", "monarch")
        ? { event: "becomes-monarch" as const, who } : null;
    }) ?? c.attempt(c => {
      const object = parseObjectRef(c);
      return object && c.words("leaves", "the", "battlefield")
        ? { event: "leaves-battlefield" as const, what: object } : null;
    });
    if (!until) return null;
    return {
      effect: "exile-until", what, ...(from ? { from } : {}),
      until,
      return: { to: "previous-zone", battlefieldController: "owner" },
    };
  });
}


/** Repeat the two instructions for the announced target, with that player
 * deciding whether to exile their own card. The following grant links both
 * exile actions, not just the opponent's (CR 608.2b–d, 400.7j, 702.62).
 * https://magic.wizards.com/en/news/feature/magic-the-gathering-doctor-who-release-notes */
export function parseRepeatedHandExile(c: Cursor, previous: Sentence | undefined): Sentence[] | null {
  return c.attempt(c => {
    if (!c.isWord("then") || !previous || previous.condition || previous.otherwise || previous.bindings ||
      previous.steps.length !== 2) return null;
    const [draw, exile] = previous.steps;
    if (draw.effect !== "draw" || draw.who.player !== "you" || draw.optional || draw.duration || draw.forEach ||
      draw.amount.amount !== "fixed" || draw.amount.value !== 2 ||
      exile.effect !== "exile" || !exile.optional || exile.decidedBy || exile.forEach || exile.duration ||
      exile.from || exile.faceDown || exile.pile || exile.who ||
      exile.what.ref !== "filter" || exile.what.upTo || exile.what.random || exile.what.groupConstraints ||
      exile.what.count.amount !== "fixed" || exile.what.count.value !== 1) return null;
    const { filter } = exile.what;
    if (filter.cls !== "card" || filter.nonTypes?.length !== 1 || filter.nonTypes[0] !== "land" ||
      filter.zone?.zone !== "hand" || filter.zone.owner !== "your" || Object.keys(filter).length !== 3 ||
      exile.counters?.length !== 1) return null;
    const [{ counter, count }] = exile.counters;
    if (counter.type !== "time" || count.amount !== "attribute" || count.attribute !== "mana-value" || count.of.ref !== "it") return null;
    c.word("then");
    const opponent = parsePlayerRef(c);
    if (opponent?.player !== "target-opponent" || opponent.count || opponent.upTo || opponent.random ||
      !c.words("does", "the", "same") || !c.punct(".") ||
      !c.words("cards", "exiled", "this", "way", "that", "don't", "have", "suspend", "gain", "suspend") ||
      (!c.punct(".") && !c.done())) return null;
    const who: PlayerRef = { player: "that-player" };
    return [
      { steps: [
        { ...draw, who: opponent },
        { ...exile, what: { ...exile.what, filter: { ...filter, zone: { zone: "hand", owner: who } } },
          who, decidedBy: who },
      ] },
      { steps: [
        { effect: "gain-abilities",
          what: { ref: "each", filter: { cls: "card", withoutAbility: ["suspend"],
            fromAmong: { ref: "related", number: "plural", filter: { cls: "card" },
              relation: { relation: "event-result", event: "exile", scope: "this-way" } } } },
          abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }] },
      ] },
    ];
  });
}
