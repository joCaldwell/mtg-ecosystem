// Static abilities that determine a permanent's controller. CR 110.2 (default
// controller), 613.1b (layer 2), 614.1d/614.12a (entering under control).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

/** "You control enchanted creature." The affected object is the one this
 * source is attached to; other subjects would need their own affected-set
 * and timestamp treatment and are not accepted here. */
function parseAttachedControl(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("you", "control")) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "enchanted" && what.ref !== "equipped")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "control", who: { player: "you" }, what };
  });
}

/** "~ enters under the control of an opponent of your choice." */
function parseEntryControl(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this") || c.word("enters") === null) return null;
    c.words("the", "battlefield");
    if (!c.words("under", "the", "control", "of", "an", "opponent", "of", "your", "choice")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "enters-under-control", what, controller: { choice: "opponent", who: { player: "you" } } };
  });
}

export function parseControlStatic(c: Cursor): StaticEffect | null {
  return parseAttachedControl(c) ?? parseEntryControl(c);
}
