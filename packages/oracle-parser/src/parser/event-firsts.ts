// "If it's the first time counters have been put on that creature this
// turn": the event that triggered the ability is the first such event of the
// period (an intervening-if clause, CR 603.4). The event is described with
// the trigger vocabulary; its object is the printed reference.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, CounterSpec, ObjectRef, Trigger } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import type { Cursor } from "./cursor.ts";

function parseEventObject(c: Cursor, g: ConditionGrammar): ObjectRef | null {
  return c.attempt(c => {
    const what = g.parseObjectRef(c);
    return what && (what.ref === "it" || (what.ref === "that" && !what.plural)) ? what : null;
  });
}

export function parseFirstEvent(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("it's", "the", "first", "time")) return null;
    const event = c.attempt((c): Trigger | null => {
      // CR 122.6: counters put on the object, including as it enters.
      const counter: CounterSpec | "any" | null = c.isWord("counters") ? "any" : g.parseCounterSpec(c);
      if (!counter || !c.words("counters", "have", "been", "put", "on")) return null;
      const what = parseEventObject(c, g);
      return what && { trigger: "counter-change", direction: "put", counter, on: { recipient: "object", what },
        match: { counters: "one-or-more", recipients: "each" } };
    }) ?? c.attempt((c): Trigger | null => {
      // CR 701.26a: becoming tapped, for any reason.
      const what = parseEventObject(c, g);
      return what && c.words("has", "become", "tapped") ? { trigger: "becomes-tapped", what } : null;
    });
    return event && c.words("this", "turn") ? { condition: "first-event", event, during: "this-turn" } : null;
  });
}
