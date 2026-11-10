// "Creature cards in graveyards and libraries can't enter the battlefield":
// a "can't" effect takes precedence over whatever would put the object onto
// the battlefield (CR 101.2). The prohibited objects are the described set;
// zones are part of that description.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

export function parseEntryProhibition(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const what = parseObjectRef(c);
    if (!what || what.ref !== "each") return null;
    if (!c.words("can't", "enter", "the", "battlefield")) return null;
    c.punct(".");
    return c.done() ? { static: "prohibit", prohibition: { prohibition: "enter-battlefield", what } } : null;
  });
}
