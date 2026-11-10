// Replacement effects that watch a player's keyword action or randomizer:
// mill (CR 701.17a), scry (701.22a), proliferate (701.34a), rolling dice
// (706.1, 706.6) and flipping coins (705.1).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, PlayerRef, ReplacementEvent, ReplacementInstruction } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseCount, parsePlayerRef } from "./refs.ts";

type ActionEvent = Extract<ReplacementEvent, { event: "mill" | "scry" | "proliferate" | "roll-dice" | "flip-coin" }>;

function parseAtLeast(c: Cursor): Comparison | null {
  return c.attempt(c => {
    const exactly = c.word("exactly") !== null;
    const value = parseCount(c);
    if (!value) return null;
    const op = c.words("or", "more") ? "ge" : "eq";
    if (exactly && op !== "eq") return null;
    return { op, value };
  });
}

/** The event after "<player> would". */
export function parseActionEvent(c: Cursor, who: PlayerRef): ActionEvent | null {
  return c.attempt((c): ActionEvent | null => {
    const action = c.word("mill", "scry") as "mill" | "scry" | null;
    if (action) {
      // "A number of cards" is any number; it restricts nothing.
      const any = c.words("a", "number", "of");
      const quantity = any ? null : parseAtLeast(c);
      if ((!any && !quantity) || c.word("card", "cards") === null) return null;
      return { event: action, who, ...(quantity ? { quantity } : {}) };
    }
    if (c.word("proliferate") !== null) return { event: "proliferate", who };
    if (c.word("roll") !== null) {
      const quantity = parseAtLeast(c);
      if (!quantity) return null;
      const planar = c.word("planar") !== null;
      if (c.word("die", "dice") === null) return null;
      return { event: "roll-dice", who, quantity, ...(planar ? { die: { die: "planar" } } : {}) };
    }
    // Each coin is flipped separately; "a coin" matches each flip (705.1).
    if (c.words("flip", "a", "coin")) return { event: "flip-coin", who };
    return null;
  });
}

function isActor(ref: PlayerRef, event: ActionEvent): boolean {
  return JSON.stringify(ref) === JSON.stringify(event.who) || ref.player === "they" || ref.player === "that-player";
}

/** The acting player of an instead-clause: printed, or the imperative
 * "you" when the event's own player is you. */
function parseActor(c: Cursor, event: ActionEvent, verb: string, thirdPerson: string): boolean {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    if (who) {
      if (!isActor(who, event)) return null;
      return c.word(who.player === "you" || who.player === "they" ? verb : thirdPerson) !== null ? true : null;
    }
    return event.who.player === "you" && c.word(verb) !== null ? true : null;
  }) === true;
}

/** "they mill twice that many cards", "scry that many cards plus one": the
 * same keyword action with a changed number of cards. */
function parseCardQuantity(c: Cursor, event: Extract<ActionEvent, { event: "mill" | "scry" }>): ReplacementInstruction["replace"] | null {
  return c.attempt((c): ReplacementInstruction["replace"] | null => {
    if (!parseActor(c, event, event.event, event.event === "mill" ? "mills" : "scries")) return null;
    let factor: number | null = null;
    const multiplier = c.word("twice", "three");
    if (multiplier === "twice") factor = 2;
    else if (multiplier === "three") { if (c.word("times") === null) return null; factor = 3; }
    if (!c.words("that", "many") || c.word("cards") === null) return null;
    if (factor !== null) return c.done() ? { replacement: "modify-quantity", change: { operation: "multiply", factor } } : null;
    // The unit precedes the addend: "that many cards plus four".
    if (c.word("plus") === null) return null;
    const amount = parseAmount(c);
    return amount && c.done() ? { replacement: "modify-quantity", change: { operation: "add", amount } } : null;
  });
}

/** "instead roll that many dice plus one and ignore the lowest roll",
 * "instead they roll that many dice plus one and you choose one of those
 * rolls to ignore", "instead flip two coins and ignore one". */
function parseRollAndIgnore(c: Cursor, event: Extract<ActionEvent, { event: "roll-dice" | "flip-coin" }>): ReplacementInstruction["replace"] | null {
  return c.attempt((c): ReplacementInstruction["replace"] | null => {
    const dice = event.event === "roll-dice";
    if (!parseActor(c, event, dice ? "roll" : "flip", dice ? "rolls" : "flips")) return null;
    let change: Extract<ReplacementInstruction["replace"], { replacement: "roll-and-ignore" }>["change"];
    if (dice) {
      if (!c.words("that", "many")) return null;
      if (event.die?.die === "planar" && c.word("planar") === null) return null;
      if (c.word("dice") === null || c.word("plus") === null) return null;
      const amount = parseAmount(c);
      if (!amount) return null;
      change = { operation: "add", amount };
    } else {
      const amount = parseCount(c);
      if (amount?.amount !== "fixed" || amount.value < 2 || c.word("coins") === null) return null;
      change = { operation: "set", amount };
    }
    if (c.word("and") === null) return null;
    let ignore: Extract<ReplacementInstruction["replace"], { replacement: "roll-and-ignore" }>["ignore"];
    if (c.words("ignore", "the", "lowest", "roll")) {
      if (!dice || event.die?.die === "planar") return null;
      ignore = { ignore: "lowest", count: 1 };
    } else if (c.words("ignore", "one")) {
      // The instruction's own actor makes the choice.
      ignore = { ignore: "chosen", who: event.who, count: 1 };
    } else {
      const who = parsePlayerRef(c);
      if (!who || who.player !== "you" || !c.words("choose", "one", "of", "those", "rolls", "to", "ignore") || !dice) return null;
      ignore = { ignore: "chosen", who, count: 1 };
    }
    return c.done() ? { replacement: "roll-and-ignore", change, ignore } : null;
  });
}

export function parseActionReplacement(c: Cursor, event: ReplacementEvent): ReplacementInstruction["replace"] | null {
  if (event.event === "mill" || event.event === "scry") return parseCardQuantity(c, event);
  if (event.event === "roll-dice" || event.event === "flip-coin") return parseRollAndIgnore(c, event);
  return null;
}
