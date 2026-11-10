// CR 702.139: "Companion—[Condition]" functions outside the game. The
// condition describes the owner's starting deck (702.139b) and is checked
// before the game begins; it is never a condition tested during play.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CompanionCondition, KeywordInstance, ObjectFilter } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

/** A predicate after "has", read through the equivalent "card with ..." filter. */
function parseHasPredicate(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const has = c.tokens[c.pos - 1];
    if (has?.kind !== "word") return null;
    const filterCursor = new Cursor([{ ...has, value: "card", raw: "card" }, { ...has, value: "with", raw: "with" }, ...c.tokens.slice(c.pos)]);
    const filter = parseFilter(filterCursor);
    if (!filter || filterCursor.pos <= 2 || filter.cls !== "card") return null;
    c.pos += filterCursor.pos - 2;
    const { cls: _cls, ...qualities } = filter;
    return Object.keys(qualities).length ? qualities : null;
  });
}

function parseCondition(c: Cursor): CompanionCondition | null {
  return c.attempt((c): CompanionCondition | null => {
    if (c.word("each") !== null) {
      const of = parseFilter(c);
      if (!of || of.cls !== "card" || of.zone || !c.words("in", "your", "starting", "deck")) return null;
      if (c.words("has", "a", "different", "name")) return { deck: "group", of, constraint: { constraint: "pairwise-different", property: "name" } };
      if (c.words("shares", "a", "card", "type")) return { deck: "group", of, constraint: { constraint: "shared-value", characteristic: "card-type" } };
      if (c.word("has") !== null) {
        const matches = parseHasPredicate(c);
        return matches && { deck: "each", of, matches };
      }
      if (c.word("is") === null || c.word("a", "an") === null) return null;
      const matches = parseFilter(c);
      if (!matches || matches.cls !== "card" || matches.zone) return null;
      const { cls: _cls, ...qualities } = matches;
      return Object.keys(qualities).length ? { deck: "each", of, matches: qualities } : null;
    }
    if (!c.words("your", "starting", "deck", "contains", "only")) return null;
    const kinds: ObjectFilter[] = [];
    for (;;) {
      const kind = parseFilter(c);
      if (!kind || kind.cls !== "card" || kind.zone) return null;
      const { cls: _cls, ...qualities } = kind;
      if (!Object.keys(qualities).length) return null;
      kinds.push(qualities);
      if (c.word("and") === null) break;
    }
    return { deck: "each", of: { cls: "card" }, matches: kinds.length === 1 ? kinds[0] : { anyOf: kinds } };
  });
}

export function parseCompanion(c: Cursor): KeywordInstance | null {
  return c.attempt((c): KeywordInstance | null => {
    if (c.word("companion") === null || !c.punct("—")) return null;
    const companion = parseCondition(c);
    return companion && { keyword: "companion", companion };
  });
}
