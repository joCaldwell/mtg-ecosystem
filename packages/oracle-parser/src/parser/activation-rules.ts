// Rules that widen when abilities may be activated. CR 302.6 / 702.10c
// (abilities with {T} or {Q} of a creature that came under its controller's
// control this turn), 606.3 (one loyalty ability per permanent per turn),
// 609.4 ("as though" applies only to the stated action).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Duration, Effect, ObjectFilter, ObjectRef, PlayerRef, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef } from "./refs.ts";

function parseSources(c: Cursor): ObjectRef | null {
  const what = parseObjectRef(c);
  // The rule covers every matching permanent, or this one; it never selects.
  return what && ["each", "self", "this"].includes(what.ref) ? what : null;
}

function parseHaste(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("you", "may", "activate", "abilities", "of")) return null;
    const sources = parseSources(c);
    if (!sources || !c.words("as", "though")) return null;
    const plural = sources.ref === "each";
    if (!(plural ? c.words("those", "creatures") || c.word("they") !== null : c.word("it") !== null)) return null;
    if (!c.words("had", "haste")) return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete activation rule");
    return { static: "activation-as-though-haste", who: { player: "you" }, sources };
  });
}

interface LoyaltyLimit { who: PlayerRef; sources: ObjectRef; maximum: number; period: "each-turn" | "this-turn" }

function parseLoyaltyLimit(c: Cursor, optionalConsumed: boolean): LoyaltyLimit | null {
  return c.attempt((c): LoyaltyLimit | null => {
    if (!optionalConsumed && !c.words("you", "may")) return null;
    if (c.word("activate") === null) return null;
    c.word("the");
    if (!c.words("loyalty", "abilities", "of")) return null;
    const sources = parseSources(c);
    if (!sources || c.word("twice") === null) return null;
    const period = c.words("each", "turn") ? "each-turn" as const : c.words("this", "turn") ? "this-turn" as const : null;
    if (!period || !c.words("rather", "than", "only", "once")) return null;
    return { who: { player: "you" }, sources, maximum: 2, period };
  });
}

/** "You may activate equip abilities any time you could cast an instant",
 * "… loyalty abilities of ~ on any player's turn any time …" (CR 304.5). */
function parseInstantTiming(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("you", "may", "activate")) return null;
    // CR 702.6a: equip is an activated keyword ability.
    const abilities: ObjectFilter | null = c.words("equip", "abilities") ? { cls: "ability", abilityKeyword: "equip" }
      : c.words("its", "loyalty", "abilities") ? { cls: "ability", loyaltyAbility: true, abilitySource: { ref: "it" } }
      : c.attempt(c => { const filter = parseFilter(c); return filter?.cls === "ability" && filter.loyaltyAbility && filter.abilitySource ? filter : null; });
    if (!abilities) return null;
    const anyPlayersTurn = c.attempt(c => c.words("on", "any") && c.possessive("player") !== null && c.word("turn") !== null ? true : null) ?? false;
    if (!c.words("any", "time", "you", "could", "cast", "an", "instant")) return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete activation rule");
    return { static: "activation-timing", who: { player: "you" }, abilities, timing: "instant", ...(anyPlayersTurn ? { anyPlayersTurn: true as const } : {}) };
  });
}

/** Whole-line static rules of this module. */
export function parseActivationRule(c: Cursor): StaticEffect | null {
  return parseHaste(c) ?? parseInstantTiming(c) ?? c.attempt((c): StaticEffect | null => {
    const limit = parseLoyaltyLimit(c, false);
    if (!limit || limit.period !== "each-turn") return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete activation rule");
    return { static: "loyalty-activation-limit", who: limit.who, sources: limit.sources, maximum: limit.maximum };
  });
}

/** Resolving form: "… twice this turn rather than only once". The printed
 * period is the lifetime of the rule; creating it is not optional (CR 611.2a). */
export function parseLoyaltyLimitEffect(c: Cursor, options: { optionalConsumed?: boolean; leadingDuration?: Duration } = {}): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (options.leadingDuration) return null;
    const limit = parseLoyaltyLimit(c, options.optionalConsumed === true);
    if (!limit || limit.period !== "this-turn") return null;
    return [{ effect: "loyalty-activation-limit", who: limit.who, sources: limit.sources, maximum: limit.maximum, duration: { duration: "this-turn" } }];
  });
}
