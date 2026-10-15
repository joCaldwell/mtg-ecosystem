// "This creature enters with your choice of a flying counter or a hexproof
// counter on it", "… your choice of a +1/+1, first strike, or vigilance
// counter on it", "… your choice of two different counters on it from among
// menace, deathtouch, and lifelink". One entry replacement (CR 614.1c) whose
// choice is made before the permanent enters (614.12a); only the chosen
// options' counters are put on it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CounterSpec, StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCount, parseCounterSpec, parseObjectRef } from "./refs.ts";

type Option = { counter: CounterSpec; count: Amount };
const one: Amount = { amount: "fixed", value: 1 };

/** CR 122.1b: the keywords a keyword counter can be. A bare list item names a
 * counter kind only when it is one of these or a +N/+N kind. */
const KEYWORD_COUNTERS = ["flying", "first strike", "double strike", "deathtouch", "decayed", "exalted", "haste", "hexproof",
  "indestructible", "lifelink", "menace", "reach", "shadow", "trample", "vigilance"];

function bareKind(c: Cursor): CounterSpec | null {
  return c.attempt((c): CounterSpec | null => {
    const pt = c.attempt(c => {
      const spec = parseCounterSpec(c);
      return spec && /^[+-]\d+\/[+-]\d+$/.test(spec.type) ? spec : null;
    });
    if (pt) return pt;
    for (const kind of KEYWORD_COUNTERS) if (c.words(...kind.split(" "))) return { type: kind };
    return null;
  });
}

/** "a flying counter", "two charge counters". */
function fullOption(c: Cursor): Option | null {
  return c.attempt((c): Option | null => {
    const count = parseCount(c);
    const counter = count && parseCounterSpec(c);
    return counter && c.word("counter", "counters") !== null ? { counter, count } : null;
  });
}

/** Items of a printed list: "A or B", "A, B, or C", "A, B, and C". */
function list<T>(c: Cursor, item: (c: Cursor) => T | null, conjunction: "or" | "and"): T[] | null {
  return c.attempt(c => {
    const first = item(c);
    if (!first) return null;
    const items = [first];
    for (;;) {
      const comma = c.punct(",");
      const last = c.word(conjunction) !== null;
      if (!comma && !last) break;
      const next = item(c);
      if (!next) return null;
      items.push(next);
      if (last) return items.length === 2 || comma ? items : null;
    }
    return null;
  });
}

function parseOptions(c: Cursor): { count: number; options: Option[] } | null {
  return c.attempt(c => {
    const full = list(c, fullOption, "or");
    if (full) return c.words("on", "it") ? { count: 1, options: full } : null;
    // A shared noun: "a +1/+1, first strike, or vigilance counter".
    const shared = c.attempt(c => {
      if (c.word("a") === null) return null;
      const kinds = list(c, bareKind, "or");
      return kinds && c.word("counter") !== null && c.words("on", "it") ? kinds : null;
    });
    if (shared) return { count: 1, options: shared.map(counter => ({ counter, count: one })) };
    // "two different counters on it from among A, B, and C".
    return c.attempt(c => {
      const count = parseCount(c);
      if (count?.amount !== "fixed" || count.value < 2 || !c.words("different", "counters", "on", "it", "from", "among")) return null;
      const kinds = list(c, bareKind, "and");
      if (!kinds || kinds.length <= count.value || new Set(kinds.map(kind => kind.type)).size !== kinds.length) return null;
      return { count: count.value, options: kinds.map(counter => ({ counter, count: one })) };
    });
  });
}

/** After the entering subject and "enters" (or "escapes"): "with your choice
 * of … on it." Returns the static without any escape condition. */
export function parseEntryCounterChoice(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
    const verb = c.word("enters", "escapes");
    if (!verb || !c.words("with", "your", "choice", "of")) return null;
    const parsed = parseOptions(c);
    if (!parsed || parsed.options.length < 2) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "enters-with-counter-choice", what, who: { player: "you" }, ...parsed,
      // CR 702.138c: "escapes with" is "if this permanent escaped, it enters with".
      ...(verb === "escapes" ? { condition: { condition: "keyword-choice" as const, keyword: "escape" as const, what }, conditionKind: "if" as const } : {}) };
  });
}
