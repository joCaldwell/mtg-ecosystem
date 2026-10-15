// Trigger events named by a player's action or by a game state, parsed after
// the When/Whenever opener. Each production consumes the complete event
// phrase or leaves the cursor untouched; no word is skipped.
// CR 603.2, 603.8, 700.13, 700.14, 701.18, 709.5h, 106.12.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectFilter, ObjectRef, PlayerRef, Trigger } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseDamageAmount } from "./trigger-events.ts";
import { parseCondition, parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if ("ref" in value && (value.ref === "target" || value.ref === "any-target")) return true;
  if ("player" in value && (value.player === "target-player" || value.player === "target-opponent")) return true;
  return Object.values(value).some(hasTarget);
}

function hasZoneQualifier(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasZoneQualifier) || !!filter.allOf?.some(hasZoneQualifier);
}

/** One identified object rather than a description of candidates. */
function parseSpecificObject(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    if (!c.isWord("this", "it") && c.peek()?.kind !== "selfref") return null;
    const what = parseObjectRef(c);
    return what && (what.ref === "this" || what.ref === "it" || what.ref === "self") ? what : null;
  });
}

/** "a land", "another land", "one or more Foods": the event's object slot.
 * The unresolved lexical "another" stays on the filter (see ObjectFilter). */
function parseEventObject(c: Cursor): { what: ObjectFilter | ObjectRef; grouping?: "one-or-more" } | null {
  return c.attempt(c => {
    const specific = parseSpecificObject(c);
    if (specific) return { what: specific };
    if (c.words("one", "or", "more")) {
      const filter = parseFilter(c);
      return filter && { what: filter, grouping: "one-or-more" as const };
    }
    if (c.word("a", "an") === null && !c.isWord("another")) return null;
    const filter = parseFilter(c);
    return filter && { what: filter };
  });
}

/** Events whose subject is the acting player, after that player was parsed. */
export function parsePlayerActionEvent(c: Cursor, who: PlayerRef): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.word("commit", "commits") !== null)
      return c.words("a", "crime") ? { trigger: "commits-crime", who } : null;
    if (c.word("expend", "expends") !== null) {
      const amount = c.number();
      return amount !== null && Number.isSafeInteger(amount) && amount >= 1 ? { trigger: "expends", who, amount } : null;
    }
    if (c.word("fully") !== null) {
      if (c.word("unlock", "unlocks") === null || c.word("a") === null) return null;
      const room = parseFilter(c);
      // CR 709.5i: only a permanent with a shared type line (a Room) can be fully unlocked.
      return room && room.subtypes?.length === 1 && room.subtypes[0] === "Room" && Object.keys(room).length === 1
        ? { trigger: "fully-unlocks", who, what: room } : null;
    }
    if (c.word("unlock", "unlocks") !== null)
      return c.words("this", "door") ? { trigger: "unlocks-door", who, door: "this" } : null;
    if (c.word("crank", "cranks") !== null) {
      const what = parseSpecificObject(c);
      return what?.ref === "this" && what.noun === "contraption" ? { trigger: "cranks", who, what } : null;
    }
    if (c.word("play", "plays") !== null) {
      const start = c.pos;
      const played = parseEventObject(c);
      // A collective play event has no rules meaning: lands and spells are
      // played one at a time (CR 116.2a, 601.2).
      if (!played || played.grouping) return null;
      if ("ref" in played.what || !hasZoneQualifier(played.what)) return { trigger: "plays", who, what: played.what };
      // "a land from exile": the origin of the play, not where the played
      // card is afterwards. Only one final "from <zone>" is lifted out.
      const words = c.tokens.slice(start, c.pos).flatMap(token => token.kind === "word" && !token.possessive ? [token.value] : []);
      const { zone, ...what } = played.what;
      if (!zone || hasZoneQualifier(what) || words.filter(word => word === "from").length !== 1 || words.includes("in")) return null;
      return { trigger: "plays", who, what, from: zone };
    }
    if (c.word("sacrifice", "sacrifices") !== null) {
      const sacrificed = parseEventObject(c);
      // The single indefinite form keeps its established production.
      if (!sacrificed || (!sacrificed.grouping && !("ref" in sacrificed.what))) return null;
      // "sacrifice ~ or another creature" is the keyword-event production's alternative.
      if ("ref" in sacrificed.what && c.isWord("or")) return null;
      return { trigger: "sacrifices", who, what: sacrificed.what, ...(sacrificed.grouping ? { grouping: sacrificed.grouping } : {}) };
    }
    if (c.word("tap", "taps") !== null) {
      const tapped = c.attempt((c): ObjectRef | null => {
        const specific = parseSpecificObject(c);
        if (specific) return specific;
        if (c.word("a", "an") === null) return null;
        const filter = parseFilter(c);
        return filter && { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
      });
      // Tapping a permanent by other means is a different event (CR 106.12).
      if (!tapped) return null;
      if (c.words("for", "mana")) return { trigger: "taps-for-mana", what: tapped, who };
      // "tap a land for {C}": tapped for mana that includes that type (CR 106.12a).
      const producing = c.attempt(c => c.word("for") !== null ? c.symbol() : null);
      return producing && /^[WUBRGC]$/.test(producing)
        ? { trigger: "taps-for-mana", what: tapped, who, producing: producing as "W" | "U" | "B" | "R" | "G" | "C" } : null;
    }
    return null;
  });
}

/** "you're dealt damage", "an opponent is dealt noncombat damage", "an
 * opponent is dealt 3 or more damage by a single source", "one or more
 * opponents are dealt combat damage". A further stated source is a
 * restriction and is left to fail. */
export function parsePlayerDamagedEvent(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    let who: PlayerRef | null = null;
    let grouping: "one-or-more" | undefined;
    if (c.word("you're") !== null) who = { player: "you" };
    else {
      // CR 603.2c: all the opponents dealt damage by one event are one occurrence.
      const collective = c.attempt((c): PlayerRef | null => {
        const one = c.words("one", "or", "more");
        const opponents = c.words("of", "your", "opponents") || (!one && c.words("your", "opponents")) || c.words("opponents");
        return opponents && c.word("are") !== null ? { player: "each-opponent" } : null;
      });
      if (collective) { who = collective; grouping = "one-or-more"; }
      else {
        who = parsePlayerRef(c);
        if (!who || c.word("is") === null) return null;
      }
    }
    if (c.word("dealt") === null) return null;
    const kind = c.word("combat", "noncombat");
    const amount = kind ? null : parseDamageAmount(c);
    if (c.word("damage") === null) return null;
    const single = amount && c.attempt(c => c.words("by", "a", "single", "source") ? true : null);
    if (grouping && (amount || single)) return null;
    return { trigger: "player-dealt-damage", who, ...(kind ? { combat: kind === "combat" } : {}),
      ...(amount ? { amount } : {}), ...(single ? { singleSource: true as const } : {}), ...(grouping ? { grouping } : {}) };
  });
}

/** Only present-state predicates can be state triggers; history conditions
 * and action outcomes describe events and stay with the event grammar. */
function isStatePredicate(condition: Condition): boolean {
  switch (condition.condition) {
    case "control": case "control-count": case "count": case "life-total": case "cards-in-hand": case "has-keyword":
      return true;
    case "not":
      return condition.of.condition === "control";
    case "is-filter":
      // "has no ice counters on it": a counter-count state of one object.
      return !condition.time && Object.keys(condition.filter).length === 1 && !!condition.filter.counterConstraints;
    default:
      return false;
  }
}

/** CR 603.8: "When you control no Swamps", "When there are no creatures on
 * the battlefield". The clause is the complete trigger condition. */
export function parseStateTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const condition = parseCondition(c);
    if (!condition || !isStatePredicate(condition) || hasTarget(condition)) return null;
    return c.isPunct(",") ? { trigger: "state", condition } : null;
  });
}
