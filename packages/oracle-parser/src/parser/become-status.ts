// "[It] becomes plotted / foretold / saddled": a card or permanent gains a
// rules designation from an effect, not by its keyword's special action.
// CR 702.170c-d (plotted card in exile), 702.143d (a card in exile becomes
// foretold; it gets a foretell cost only if the effect gives one), 702.171b
// (saddled lasts until end of turn or until the permanent leaves):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

export function parseBecomeStatusAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    // CR 701.64a: "Harness ~" — only a permanent can be harnessed.
    if (c.word("harness") !== null) {
      const self = parseObjectRef(c);
      return self && (self.ref === "self" || self.ref === "this") ? { effect: "become-status", what: self, status: "harnessed" } : null;
    }
    const what = parseObjectRef(c);
    if (!what || c.word("becomes", "become") === null) return null;
    const status = c.word("plotted", "foretold", "saddled");
    if (status !== "plotted" && status !== "foretold" && status !== "saddled") return null;
    // 702.171b: the printed "until end of turn" restates the designation's
    // own lifetime; any other duration would be a different effect.
    if (status === "saddled" && c.isWord("until") && !c.words("until", "end", "of", "turn")) return null;
    return { effect: "become-status", what, status };
  });
}
