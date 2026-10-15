// Persistent permissions do not instruct a resolving ability to perform an
// action. CR 401.5/708.5 (private information), 502.3/4 (turn-based untapping),
// and 903.3a/113.6n (pregame commander eligibility).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type { Duration, Effect, LookPermission, ObjectRef, PlayerRef, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parsePlayPermissionStatic } from "./permissions.ts";
import { parseCastingRule } from "./casting-rules.ts";
import { parseManaSpendingRule } from "./mana-spending-rules.ts";
import { parseActivationRule } from "./activation-rules.ts";
import { parseTargetingStatic } from "./targeting-rules.ts";
import { parseDuration, parseObjectRef, parsePlayerRef } from "./refs.ts";

interface LookOptions {
  optionalConsumed?: boolean;
  leadingDuration?: Duration;
}

function parseLookDomain(c: Cursor): ObjectRef | null {
  return c.attempt((c) => {
    const what = parseObjectRef(c);
    if (!what) return null;
    if (what.ref === "library-cards" && what.position === "top" && what.count.amount === "fixed"
      && what.count.value === 1 && what.of.zone === "library") return what;
    // A plural query is reevaluated when the permission is used; it is not
    // a resolving choice of one creature or a snapshot of today's set.
    if ((what.ref === "each" || what.ref === "all") && what.filter.status?.includes("face-down")) return what;
    return null;
  });
}

function parseLook(c: Cursor, options: LookOptions): { permission: LookPermission; duration?: Duration } | null {
  return c.attempt((c) => {
    const leadingDuration = options.leadingDuration ?? c.attempt((c) => {
      const duration = parseDuration(c);
      return duration && c.punct(",") ? duration : null;
    });
    const who: PlayerRef | null = options.optionalConsumed ? { player: "you" } : c.attempt((c) => {
      const who = parsePlayerRef(c);
      return who && c.word("may") !== null ? who : null;
    });
    if (!who || !c.words("look", "at")) return null;
    const first = parseLookDomain(c);
    if (!first) return null;
    const what = [first];
    while (c.attempt((c) => {
      if (!c.words("and", "at")) return null;
      const next = parseLookDomain(c);
      if (!next) return null;
      what.push(next);
      return true;
    })) { /* Lens of Clarity: one permission, two distinct information domains. */ }
    if (!c.words("any", "time") && !c.words("at", "any", "time")) return null;
    const duration = leadingDuration ?? parseDuration(c) ?? undefined;
    const permission: LookPermission = { who, what, timing: "any-time",
      ...(what.some(ref => ref.ref === "library-cards")
        ? { topLibraryChanges: "after-casting-activation-or-special-action" as const } : {}) };
    return { permission, ...(duration ? { duration } : {}) };
  });
}

/** Requires an explicit lifetime for a resolving permission, and explicit
 * "any time" to distinguish it from an immediate optional look instruction.
 * The enclosing "may" permits future inspection; it does not make creation
 * of this continuous effect optional (CR 611.2a/c). */
export function parseLookPermissionEffect(c: Cursor, options: LookOptions = {}): Effect[] | null {
  const exiled = parseExiledCardLook(c, options);
  if (exiled) return exiled;
  return c.attempt((c) => {
    const result = parseLook(c, options);
    if (!result?.duration) return null;
    return [{ effect: "grant-look-permission", permission: result.permission, duration: result.duration }];
  });
}

/** "You may look at those cards for as long as they remain exiled." CR 406.3:
 * looking at a card exiled face down needs an effect's permission. The lasting
 * period, not "any time", distinguishes it from an immediate look; as with
 * "look at and play", each card's permission ends when that card leaves exile.
 * The looked-at reference must be the same card(s) the period names. */
function parseExiledCardLook(c: Cursor, options: LookOptions): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (options.leadingDuration) return null;
    if (!options.optionalConsumed && !c.words("you", "may")) return null;
    if (!c.words("look", "at")) return null;
    const what = parseObjectRef(c);
    if (!what) return null;
    const number = what.ref === "it" ? "singular" : what.ref === "them" ? "plural"
      : what.ref === "that" && what.filter.cls === "card" && Object.keys(what.filter).length === 1 ? (what.plural ? "plural" : "singular")
      : null;
    if (!number) return null;
    const duration = parseDuration(c);
    if (duration?.duration !== "as-long-as" || duration.condition.condition !== "remains" || duration.condition.zone.zone !== "exile") return null;
    if (duration.condition.what.ref !== (number === "singular" ? "it" : "them")) return null;
    return [{ effect: "grant-look-permission", permission: { who: { player: "you" }, what: [what], timing: "any-time" },
      duration, durationScope: "each-permitted-card" }];
  });
}

function parseStaticLook(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const result = parseLook(c, {});
    if (!result || result.duration) return null;
    // An indefinite selected player needs a separate scope; a static rule
    // cannot manufacture a chosen player or an announcement target.
    if (!["you", "each-player", "each-opponent", "enchanted-player"].includes(result.permission.who.player)) return null;
    const effect: StaticEffect = { static: "look-permission", permission: result.permission };
    const play = c.attempt((c) => {
      c.punct(",");
      if (c.word("and") === null) return null;
      return parsePlayPermissionStatic(c);
    });
    c.punct(".");
    if (!c.done()) return null;
    return play ? { static: "group", effects: [effect, play] } : effect;
  });
}

function parseCommanderEligibility(c: Cursor): StaticEffect | null {
  return c.attempt((c) => {
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this") || !c.words("can", "be", "your", "commander")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "commander-eligibility", what, during: "deck-construction" };
  });
}

function parseOptionalUntap(c: Cursor): StaticEffect | null {
  return c.attempt((c) => {
    if (!c.words("you", "may", "choose", "not", "to", "untap")) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this") || !c.words("during", "your", "untap", "step")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "optional-untap", who: { player: "you" }, what,
      step: { occurrence: "each", owner: { player: "you" } } };
  });
}

/** "All damage is dealt [to you] as though its source had wither." CR 609.4,
 * 120.3b/d: only the results of that damage change; no source gains the
 * ability, so nothing else that looks for wither or infect sees it. */
function parseDamagePretense(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("all", "damage", "is", "dealt")) return null;
    let to: PlayerRef | undefined;
    if (c.word("to") !== null) {
      const who = parsePlayerRef(c);
      if (!who) return null;
      to = who;
    }
    if (!c.words("as", "though", "its", "source", "had")) return null;
    const keyword = c.word("wither", "infect") as "wither" | "infect" | null;
    if (!keyword) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "damage-as-though", keyword, ...(to ? { to } : {}) };
  });
}

/** Complete persistent clauses only: a following unsupported deck rule or
 * action must fail the ability rather than be silently discarded. */
export function parsePersistentRule(c: Cursor): StaticEffect | null {
  return parseStaticLook(c) ?? parseCommanderEligibility(c) ?? parseOptionalUntap(c) ?? parseCastingRule(c) ?? parseManaSpendingRule(c) ?? parseActivationRule(c) ?? parseTargetingStatic(c) ?? parseDamagePretense(c);
}
