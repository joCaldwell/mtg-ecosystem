// Rules-defined, historical and event-result quantities. Each production
// keeps what is measured, whose, and over which period or event; none of them
// is an object count. CR 103.4, 107.1–107.3, 119.1–119.4, 120.3–120.4,
// 120.10, 608.2h and 700.5.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Color, EventQuantity, ObjectRef, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { COLORS } from "../vocab.ts";

export interface QuantityGrammar {
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parsePossessivePlayer(c: Cursor): PlayerRef | null;
  isPluralPlayerRef(ref: PlayerRef): boolean;
}

type LifeChange = "gained" | "lost";
type EventAmount = Extract<Amount, { amount: "event-quantity" }>;

function color(c: Cursor): Color | null {
  return c.attempt(c => {
    const word = c.anyWord();
    return word && COLORS.has(word.value) ? word.value as Color : null;
  });
}

/** CR 700.5 defines devotion only to one color or to a pair of colors.
 * Faction names, "hybrid", and a contextual "that color" are not colors here
 * and stay rejected instead of becoming an unqualified symbol count. */
function parseDevotion(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const who = g.parsePossessivePlayer(c);
    if (!who || g.isPluralPlayerRef(who) || !c.words("devotion", "to")) return null;
    const first = color(c);
    if (!first) return c.fail("color");
    const second = c.attempt(c => c.word("and") !== null ? color(c) : null);
    if (second === first) return null;
    return { amount: "devotion", who, colors: second ? [first, second] : [first] };
  });
}

function parseStartingLifeTotal(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const of = g.parsePossessivePlayer(c);
    if (!of || g.isPluralPlayerRef(of) || !c.words("starting", "life", "total")) return null;
    return { amount: "starting-life-total", of };
  });
}

/** An event quantity without a printed scope refers to an event established
 * by its context. Reject a following clause that would give it a different
 * scope ("this turn", "when it entered", "so far", "by sources ...") rather
 * than silently dropping that scope. */
function endsQuantity(c: Cursor): boolean {
  return c.done() || c.peek()?.kind === "punct" || c.isWord("rounded", "plus", "minus", "until", "to");
}

/** Perfect and simple past name the same completed events. */
function parseLifeSubject(c: Cursor, g: QuantityGrammar): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.word("you've") !== null) return { player: "you" };
    if (c.word("they've") !== null) return { player: "they" };
    const who = g.parsePlayerRef(c);
    // A quantified group needs an explicit per-player or combined reading.
    if (!who || g.isPluralPlayerRef(who)) return null;
    c.word("have", "has");
    return who;
  });
}

/** Tail after "life": who changed it, in which direction, and the period or
 * event. Life paid or lost to damage is life lost (CR 119.2–119.4). */
function parseLifeTail(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const passive = c.word("gained", "lost") as LifeChange | null;
    if (passive) {
      if (c.words("this", "way")) return { amount: "event-quantity", event: "life-change", change: passive, scope: "this-way" };
      if (c.words("by", "all", "players", "this", "turn"))
        return { amount: "life-change", who: "all-players-combined", change: passive, during: "this-turn" };
      return null;
    }
    const who = parseLifeSubject(c, g);
    const change = who && c.word("gained", "lost") as LifeChange | null;
    if (!who || !change) return null;
    if (c.words("this", "turn")) return { amount: "life-change", who, change, during: "this-turn" };
    if (c.words("this", "way")) return { amount: "event-quantity", event: "life-change", change, who, scope: "this-way" };
    return endsQuantity(c) ? { amount: "event-quantity", event: "life-change", change, who, scope: "contextual" } : null;
  });
}

function parseDamageParticipant(c: Cursor, g: QuantityGrammar): ObjectRef | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    // Only an already identified object can be a participant of an earlier
    // event; a fresh quantified description would be a different query.
    return what && ["self", "this", "it", "them", "that", "enchanted", "equipped"].includes(what.ref) ? what : null;
  });
}

/** Tail after "[excess] damage": active "<source> dealt [to <recipient>]" or
 * passive "dealt [to <recipient>]", then the event scope. Damage that was
 * prevented or replaced was not dealt (CR 120.3, 615.6). */
function parseDamageTail(c: Cursor, g: QuantityGrammar, measure: "dealt" | "excess"): EventAmount | null {
  return c.attempt((c): EventAmount | null => {
    type Damage = Extract<EventQuantity, { event: "damage" }>;
    let by: ObjectRef | undefined;
    if (c.word("dealt") === null) {
      const source = parseDamageParticipant(c, g);
      if (!source || c.word("dealt") === null) return null;
      by = source;
    }
    let to: Damage["to"];
    if (c.word("to") !== null) {
      const player = c.attempt(c => {
        const who = g.parsePlayerRef(c);
        return who && !g.isPluralPlayerRef(who) ? who : null;
      });
      const recipient = player ?? parseDamageParticipant(c, g);
      if (!recipient) return null;
      to = recipient;
    }
    const scope = c.words("this", "way") ? "this-way" : endsQuantity(c) ? "contextual" : null;
    return scope && { amount: "event-quantity", event: "damage", measure, ...(by ? { by } : {}), ...(to ? { to } : {}), scope };
  });
}

/** Definite quantity in amount position: "the [total] [amount of] life …" or
 * "the [amount of] [excess] damage …". */
function parseDefiniteQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("the") === null) return null;
    // Every quantity here already is the total of its matching events.
    c.word("total");
    c.words("amount", "of");
    if (c.word("life") !== null) return parseLifeTail(c, g);
    const excess = c.word("excess") !== null;
    if (c.word("damage") === null) return null;
    return parseDamageTail(c, g, excess ? "excess" : "dealt");
  });
}

/** Unit after "for each": "1 life you lost", "1 life lost this way". The
 * resulting amount multiplies one instruction's quantity; it never repeats
 * the instruction as separate events. */
export function parseUnitQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.number() !== 1 || c.word("life") === null) return null;
    return parseLifeTail(c, g);
  });
}

export function parseQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return parseDevotion(c, g) ?? parseStartingLifeTotal(c, g) ?? parseDefiniteQuantity(c, g);
}
