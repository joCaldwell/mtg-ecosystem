// Rules-defined, historical and event-result quantities. Each production
// keeps what is measured, whose, and over which period or event; none of them
// is an object count. CR 103.4, 107.1–107.3, 119.1–119.4, 120.3–120.4,
// 120.10, 608.2h and 700.5.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ChosenValueRef, Color, EventQuantity, ObjectRef, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { COLORS } from "../vocab.ts";

export interface QuantityGrammar {
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parsePossessivePlayer(c: Cursor): PlayerRef | null;
  isPluralPlayerRef(ref: PlayerRef): boolean;
  parseChosenColor?(c: Cursor): ChosenValueRef<"color"> | null;
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
 * Faction names and "hybrid" are not colors here and stay rejected instead of
 * becoming an unqualified symbol count. "That color" / "the chosen color" is
 * the one color an earlier instruction of this ability chose (608.2c). */
function parseDevotion(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const who = g.parsePossessivePlayer(c);
    if (!who || g.isPluralPlayerRef(who) || !c.words("devotion", "to")) return null;
    const chosen = g.parseChosenColor?.(c);
    if (chosen) return { amount: "devotion", who, colors: chosen };
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
    // "the life paid as it entered" (CR 607.2g).
    const entry = c.attempt((c): Amount | null => {
      if (!c.words("paid", "as")) return null;
      const of = g.parseObjectRef(c);
      return of && (of.ref === "self" || of.ref === "this" || of.ref === "it") && c.word("entered") !== null
        ? { amount: "entry-payment", of, paid: "life" } : null;
    });
    if (entry) return entry;
    const passive = c.word("gained", "lost") as LifeChange | null;
    if (passive) {
      if (c.words("this", "way")) return { amount: "event-quantity", event: "life-change", change: passive, scope: "this-way" };
      if (c.words("by", "all", "players", "this", "turn"))
        return { amount: "life-change", who: "all-players-combined", change: passive, during: "this-turn" };
      return null;
    }
    // "Your opponents have lost this turn": all of their life changes over
    // the turn added together, like the other history totals.
    const opponents = c.attempt(c => c.words("your", "opponents", "have") ? true : null);
    if (opponents) {
      const change = c.word("gained", "lost") as LifeChange | null;
      return change && c.words("this", "turn") ? { amount: "life-change", who: { player: "each-opponent" }, change, during: "this-turn" } : null;
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

/** "{E} paid this way": the energy counters actually paid in the payment the
 * same instruction sequence offered (CR 107.14, 608.2h). */
function parseEnergyPaid(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const symbol = c.peek();
    if (symbol?.kind !== "symbol" || symbol.value !== "E") return null;
    c.pos++;
    return c.words("paid", "this", "way") ? { amount: "event-quantity", event: "energy-paid", scope: "this-way" } : null;
  });
}

/** "mana [that player] paid this way": the mana actually paid in the stated
 * payment of the same instruction sequence (CR 118.1, 608.2h), summed over
 * every payer unless one player is named. */
function parseManaPaid(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("mana") === null) return null;
    const who = c.isWord("paid") ? null : parseLifeSubject(c, g);
    if (!c.words("paid", "this", "way")) return null;
    return { amount: "event-quantity", event: "mana-paid", ...(who ? { who } : {}), scope: "this-way" };
  });
}

/** Definite quantity in amount position: "the [total] [amount of] life …" or
 * "the [amount of] [excess] damage …". */
function parseDefiniteQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const start = c.pos;
    // "that excess damage" has no printed scope; its antecedent event is the
    // one an earlier clause of the sentence tested (CR 120.4a, 120.10).
    if (c.words("that", "excess", "damage") && endsQuantity(c)) return { amount: "event-quantity", event: "damage", measure: "excess", scope: "contextual" };
    // "that damage": likewise the damage dealt in that event (CR 120.3).
    // A following "from"/"without" belongs to the enclosing description
    // ("… with mana value less than or equal to that damage from your hand").
    if (c.words("that", "damage") && (endsQuantity(c) || c.isWord("from", "without")))
      return { amount: "event-quantity", event: "damage", measure: "dealt", scope: "contextual" };
    c.pos = start;
    if (c.word("the") === null) return null;
    // Every quantity here already is the total of its matching events.
    c.word("total");
    const amountOf = c.words("amount", "of");
    const energy = amountOf ? parseEnergyPaid(c) : null;
    if (energy) return energy;
    const mana = amountOf ? parseManaPaid(c, g) : null;
    if (mana) return mana;
    if (c.word("life") !== null) return parseLifeTail(c, g);
    const excess = c.word("excess") !== null;
    if (c.word("damage") === null) return null;
    return parseDamageTail(c, g, excess ? "excess" : "dealt");
  });
}

/** CR 700.8a: "creature(s) in your party" (after "the number of" / "for
 * each"). The game computes the size of the party, from zero to four, taking
 * the highest result (700.8b); it is not a count of the Clerics, Rogues,
 * Warriors and Wizards the player controls, and no creatures are chosen. */
export function parsePartySize(c: Cursor, unit: "singular" | "plural"): Amount | null {
  return c.attempt((c): Amount | null =>
    c.words(unit === "singular" ? "creature" : "creatures", "in", "your", "party") ? { amount: "party-size", who: { player: "you" } } : null);
}

/** Unit after "for each": "1 life you lost", "1 life lost this way". The
 * resulting amount multiplies one instruction's quantity; it never repeats
 * the instruction as separate events. */
export function parseUnitQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return c.attempt((c): Amount | null => {
    const energy = parseEnergyPaid(c);
    if (energy) return energy;
    const party = parsePartySize(c, "singular");
    if (party) return party;
    // CR 701.22a: each card the scry action looked at.
    if (c.words("card", "looked", "at", "while", "scrying", "this", "way")) return { amount: "event-quantity", event: "scry-looked-at", scope: "this-way" };
    // CR 705.2: "flip you won" / "flip you win" counts the flips the flipper
    // won in the coin-flip instruction this refers back to.
    if (c.words("flip", "you", "won") || c.words("flip", "you", "win"))
      return { amount: "event-quantity", event: "coin-flip", result: "win", who: { player: "you" }, scope: "contextual" };
    if (c.number() !== 1 || c.word("life") === null) return null;
    return parseLifeTail(c, g);
  });
}

export function parseQuantity(c: Cursor, g: QuantityGrammar): Amount | null {
  return parseDevotion(c, g) ?? parseStartingLifeTotal(c, g) ?? parseDefiniteQuantity(c, g);
}
