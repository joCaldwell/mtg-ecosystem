// "The "legend rule" doesn't apply [to <permanents>]." CR 704.5j names the
// state-based action; an effect can exempt permanents from it (or switch it
// off altogether) for as long as the effect lasts.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Duration, Effect, ObjectRef, StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { isPluralObjectRef } from "./quantified-selections.ts";
import { parseDuration, parseObjectRef } from "./refs.ts";

/** Returns the exempt permanents, or "all" when the rule doesn't apply at
 * all. The exemption is a query over permanents, never a choice of one. */
function parseExemption(c: Cursor): { what?: ObjectRef } | null {
  return c.attempt(c => {
    if (c.word("the") === null || !c.punct('"') || !c.words("legend", "rule") || !c.punct('"')) return null;
    if (!c.words("doesn't", "apply")) return null;
    if (c.word("to") === null) return {};
    const what = parseObjectRef(c);
    if (!what || what.ref === "target" || !isPluralObjectRef(what)) return c.fail("permanents exempt from the legend rule");
    return { what };
  });
}

export function parseLegendRuleStatic(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const exemption = parseExemption(c);
    if (!exemption) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return null;
    return { static: "legend-rule-exemption", ...exemption };
  });
}

/** The resolving form needs its stated lifetime (CR 611.2a); without one the
 * sentence is a static ability. */
export function parseLegendRuleEffect(c: Cursor, leadingDuration?: Duration): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const exemption = parseExemption(c);
    if (!exemption) return null;
    const duration = leadingDuration ?? parseDuration(c);
    if (!duration) return c.fail("duration of a resolving legend-rule exemption");
    return [{ effect: "legend-rule-exemption", ...exemption, duration }];
  });
}
