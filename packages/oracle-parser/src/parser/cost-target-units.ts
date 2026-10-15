// Cost changes that depend on what the adjusted spell or ability targets:
// "where X is the power of the creature it targets", "for each +1/+1 counter
// on the creature it targets", "for each color of the creature it targets".
// Targets are chosen before the total cost is determined (CR 601.2c, 601.2f),
// so the targeted object exists when the change is computed. Only "it"
// directly after a described object is read this way, and only here.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCounterSpec } from "./refs.ts";

type Source = Extract<ObjectRef, { ref: "targeted-by" }>["source"];

/** "the creature it targets" / "creature it targets" (article optional where the
 * caller already consumed it). */
function parseTargetedObject(c: Cursor, source: Source, article: boolean): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (article && c.word("the") === null) return null;
    // Not parseFilter: its relative clauses would consume "it".
    const type = c.word("creature", "artifact", "enchantment", "land", "planeswalker");
    return type && c.words("it", "targets") ? { ref: "targeted-by", source, filter: { types: [type] } } : null;
  });
}

/** After "where X is": a characteristic of the object the adjusted ability targets. */
export function parseTargetedValue(c: Cursor, source: Source): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("the") === null) return null;
    const attribute = c.word("power", "toughness") ?? (c.words("mana", "value") ? "mana-value" : null);
    if (!attribute || c.word("of") === null) return null;
    const of = parseTargetedObject(c, source, true);
    return of && { amount: "attribute", of, attribute: attribute as "power" | "toughness" | "mana-value" };
  });
}

/** After "for": "each" and a counter on, or a color of, the targeted object. */
export function parseTargetedUnit(c: Cursor, source: Source): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("each") === null) return null;
    if (c.words("color", "of")) {
      const among = parseTargetedObject(c, source, true);
      return among && { amount: "distinct-count", characteristic: "color", among };
    }
    // CR 601.2c, 601.2f: "for each creature it targets" counts the chosen
    // targets that are creatures; it is not a count of objects on the battlefield.
    const targeted = c.attempt((c): Amount | null => {
      const type = c.word("creature", "artifact", "enchantment", "land", "planeswalker");
      return type && c.words("it", "targets") ? { amount: "target-count", of: source, matching: { types: [type] } } : null;
    });
    if (targeted) return targeted;
    const counter = parseCounterSpec(c);
    if (!counter || c.word("counter") === null || c.word("on") === null) return null;
    const on = parseTargetedObject(c, source, true);
    return on && { amount: "counter-count", counter, on };
  });
}
