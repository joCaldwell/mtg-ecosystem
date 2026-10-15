// Conditions about one particular earlier event of an identified object:
// how it entered the battlefield, what an instruction did "this way".
// CR 400.7, 603.4, 603.6a, 608.2c/h:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, ObjectRef, PlayerHistoryAction, PlayerRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

/** An already identified single object; a fresh "a creature" would be a
 * count of entries over some period instead. */
function parseIdentifiedObject(c: Cursor, g: ConditionGrammar): ObjectRef | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    if (!what || !["self", "this", "it", "that"].includes(what.ref)) return null;
    return what.ref === "that" && what.plural ? null : what;
  });
}

/** "It entered from your graveyard": no period is printed because the
 * question is about the entry that made the object what it is. */
function parseEnteredFrom(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const what = parseIdentifiedObject(c, g);
    if (!what || !c.words("entered", "from")) return null;
    const from = g.parseZoneRef(c);
    if (!from || from.position || from.zone === "battlefield" ||
        (["hand", "library", "graveyard"].includes(from.zone) && from.owner === undefined)) return null;
    // "Entered from X this turn" is a dated history, a different question.
    if (c.isWord("this", "last", "under", "since", "during")) return null;
    return { condition: "entered-from", what, from };
  });
}

/** "You put a counter on a creature this turn" (CR 122.6). Only "a" / "one
 * or more" is read: a larger number would not say whether it is counted on
 * one recipient or across all of them. */
function parseCountersPut(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let who: PlayerRef | null;
    let negate = false;
    if (c.word("you've") !== null) who = { player: "you" };
    else {
      who = g.parsePlayerRef(c);
      if (!who) return null;
      if (c.word("haven't", "hasn't", "didn't") !== null) negate = true;
      else c.word("have", "has");
    }
    if (c.word("put") === null) return null;
    let comparison: Comparison | null = null;
    if (c.word("a", "an") !== null) comparison = { op: "ge", value: { amount: "fixed", value: 1 } };
    else if (c.words("one", "or", "more")) comparison = { op: "ge", value: { amount: "fixed", value: 1 } };
    if (!comparison) return null;
    const counter = c.isWord("counter", "counters") ? "any" as const : g.parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null || c.word("on") === null) return null;
    type Recipient = Extract<PlayerHistoryAction, { action: "put-counters" }>["on"];
    let on: Recipient;
    const what = g.parseObjectRef(c);
    if (!what) return null;
    if (what.ref === "self" || what.ref === "this" || what.ref === "it" || (what.ref === "that" && !what.plural)) on = { match: "object", what };
    else if (what.ref === "filter" && what.count.amount === "fixed" && what.count.value === 1 && !what.upTo && !what.random && !what.groupConstraints)
      on = { match: "filter", filter: what.filter };
    else return null;
    const during = c.words("this", "turn") ? "this-turn" as const : c.words("last", "turn") ? "last-turn" as const : null;
    if (!during) return null;
    const history: Condition = { condition: "player-action-history", who, action: { action: "put-counters", counter, on }, comparison, during };
    return negate ? { condition: "not", of: history } : history;
  });
}

export function parseEventCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return parseEnteredFrom(c, g) ?? parseCountersPut(c, g);
}
