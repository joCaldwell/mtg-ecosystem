// "Repeat this process": "this process" is an anaphor for instructions
// printed earlier in the same ability; its extent stays contextual.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCount } from "./refs.ts";

/** "repeat this process [once | twice | N more times | any number of
 * times]". Any other continuation ("until …", "except …", "for an
 * enchantment") changes what is repeated and is not read here. */
export function parseRepeatProcess(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (!c.words("repeat", "this", "process")) return null;
    let times: Amount | "any-number" | undefined;
    if (c.word("once") !== null) times = { amount: "fixed", value: 1 };
    else if (c.word("twice") !== null) times = { amount: "fixed", value: 2 };
    else if (c.words("any", "number", "of", "times")) times = "any-number";
    else {
      const counted = c.attempt((c): Amount | null => {
        const count = parseCount(c);
        return count && c.words("more", "times") ? count : null;
      });
      if (counted) times = counted;
    }
    if (!c.done() && !c.isPunct(".") && !c.isPunct(",")) return null;
    return [{ effect: "repeat-process", process: "contextual", ...(times ? { times } : {}) }];
  });
}
