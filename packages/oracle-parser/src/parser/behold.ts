// Behold (CR 701.4): cost, imperative and "was beheld" condition forms.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, Cost, Effect, ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseObjectSelection } from "./refs.ts";

/** Filter keys that describe a quality shared by a card in hand and a
 * permanent on the battlefield. Zone and controller are part of the action. */
const QUALITY_KEYS = new Set(["types", "subtypes", "supertypes", "nonTypes", "nonSubtypes", "colors", "nonColors", "colorless", "multicolored", "monocolored"]);

/** "a Dragon", "an Elf", "three Elementals", "a Gamma creature": the [quality]
 * of CR 701.4a and how many objects are beheld. */
export function parseBeholdQuality(c: Cursor): { quality: ObjectFilter; count: Amount } | null {
  return c.attempt(c => {
    const what = parseObjectSelection(c);
    if (!what || what.ref !== "filter" || !what.count) return null;
    if (Object.keys(what.filter).some(key => !QUALITY_KEYS.has(key))) return null;
    return { quality: what.filter, count: what.count };
  });
}

/** "behold a Dragon" / "behold a Kithkin and exile it" as a cost. The
 * beheld card or permanent is the object that is exiled. */
export function parseBeholdCost(c: Cursor): Cost | null {
  return c.attempt((c): Cost | null => {
    if (c.word("behold") === null) return null;
    const behold = parseBeholdQuality(c);
    if (!behold) return null;
    const exile = c.attempt(c => (c.words("and", "exile", "it") ? true : null));
    return { cost: "behold", ...behold, ...(exile ? { exile: true as const } : {}) };
  });
}

/** CR 701.4a imperative; the actor is the ability's controller. */
export function parseBeholdEffect(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (c.word("behold") === null) return null;
    const behold = parseBeholdQuality(c);
    return behold ? [{ effect: "behold", who: { player: "you" }, ...behold }] : null;
  });
}

/** CR 701.4b: "if a Dragon was beheld". */
export function parseBeheldCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    const behold = parseBeholdQuality(c);
    if (!behold || behold.count.amount !== "fixed" || behold.count.value !== 1) return null;
    return c.words("was", "beheld") ? { condition: "beheld", quality: behold.quality } : null;
  });
}
