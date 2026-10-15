// Trigger condition parsing: the clause between "When/Whenever/At" and the
// comma that starts the effect text.

import type { Comparison, ObjectFilter, ObjectRef, PhaseName, PlayerRef, Trigger, TriggerLimit, TurnOwner, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, parseAbilityHead, parseAmount, parseCount, parseComparison, parseFilter, parseObjectRef, parsePlayerRef, parsePossessiveObject, parsePossessivePlayer, parseZoneRef } from "./refs.ts";
import { parseDiceTrigger } from "./dice.ts";
import { parseTriggerGate } from "./trigger-gates.ts";
import { parseCounterChangeTrigger } from "./counter-events.ts";
import { parsePlayerActionEvent, parsePlayerDamagedEvent, parseStateTrigger } from "./trigger-actions.ts";
import { findNotFirstTail, isSingleOtherPlayer, parseCausedEvent, parseDamageAmount, parseGameEvent, parseMostLifePlayer, parseNotFirstTail, parseObjectDealtDamage, parseTriggerPlayer, parseObjectKeywordEvent, parsePlayerKeywordEvent } from "./trigger-events.ts";

/** Subject of an event trigger: "~", "a creature", "another Elf you control" … */
type TriggerSubject = { what: ObjectRef; grouping?: "one-or-more"; groupSize?: Comparison };

function parseTriggerSubject(c: Cursor): TriggerSubject | null {
  return c.attempt((c): TriggerSubject | null => {
    // An attachment reference identifies the object attached to this source;
    // it is not the set of all equipped/enchanted creatures (CR 301.5, 303.4).
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.isWord("equipped", "enchanted")) {
      const what = parseObjectRef(c);
      return what && { what };
    }
    // CR 702.55b defines this phrase: the object this card's haunt ability targeted.
    if (c.attempt(c => c.words("the", "creature") && (c.word("it") !== null || c.words("this", "card")) && c.word("haunts") !== null ? true : null))
      return { what: { ref: "haunted-creature" } };
    if (c.word("a", "an") !== null) {
      const filter = parseFilter(c);
      if (!filter) return null;
      return { what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } };
    }
    if (c.attempt((c) => (c.words("one", "or", "more") ? true : null))) {
      const filter = parseFilter(c);
      if (!filter) return null;
      return { what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } }, grouping: "one-or-more" };
    }
    // "two or more creatures you control": the same collective occurrence
    // with a minimum size (CR 603.2c).
    const minimum = c.attempt(c => { const n = c.word("two", "three", "four", "five"); return n && c.words("or", "more") ? n : null; });
    if (minimum) {
      const filter = parseFilter(c);
      if (!filter) return null;
      return { what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } }, grouping: "one-or-more",
        groupSize: { op: "ge", value: { amount: "fixed", value: ["two", "three", "four", "five"].indexOf(minimum) + 2 } } };
    }
    // Bare filter subject ("another creature you control dies") is one object
    // per event, not "each" — so classify it as a filter ref, not via
    // parseObjectRef's bare-plural fallback.
    const bare = c.attempt(parseFilter);
    if (bare) return { what: { ref: "filter", filter: bare, count: { amount: "fixed", value: 1 } } };
    const what = parseObjectRef(c);
    return what && { what };
  });
}

function parsePhase(c: Cursor): PhaseName | null {
  return c.attempt((c): PhaseName | null => {
    if (c.words("upkeep")) return "upkeep";
    if (c.words("draw", "step")) return "draw-step";
    if (c.words("untap", "step")) return "untap-step";
    if (c.words("end", "step")) return "end-step";
    if (c.words("end", "of", "combat")) return "end-of-combat";
    if (c.words("combat", "damage", "step")) return "combat-damage";
    if (c.words("declare", "attackers", "step")) return "declare-attackers";
    if (c.words("declare", "blockers", "step")) return "declare-blockers";
    if (c.words("cleanup", "step")) return "cleanup";
    if (c.words("first", "main", "phase") || c.words("precombat", "main", "phase"))
      return "precombat-main";
    // CR505.1b distinguishes the second main phase from additional
    // postcombat main phases granted by effects (505.1a).
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.words("second", "main", "phase")) return "second-main";
    if (c.words("postcombat", "main", "phase")) return "postcombat-main";
    if (c.words("main", "phase")) return "main";
    if (c.words("combat")) return "combat";
    // "Next" is a delayed-event restriction (CR 603.7b), not a synonym for
    // every occurrence. delayed.ts represents it in its separate context.
    if (c.words("turn")) return "turn";
    return c.fail("phase");
  });
}

function parsePluralPhase(c: Cursor): PhaseName | null {
  return c.attempt((c): PhaseName | null => {
    if (c.words("upkeeps")) return "upkeep";
    if (c.words("draw", "steps")) return "draw-step";
    if (c.words("end", "steps")) return "end-step";
    if (c.words("postcombat", "main", "phases")) return "postcombat-main";
    if (c.words("main", "phases")) return "main";
    return c.fail("plural phase");
  });
}

/** "At the beginning of <owner> <phase>" / "of <phase> on <owner> turn" */
function parsePhaseTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.words("end", "of", "combat")) return { trigger: "phase", phase: "end-of-combat", whose: "each-player" };
    if (!c.words("the", "beginning", "of")) return null;

    // "each of your postcombat main phases": every such phase of that one
    // player's turns, the occurrences the singular possessive form denotes
    // (CR 500.8, 505.1a: a turn can have several of one phase).
    const eachOf = c.attempt((c): Trigger | null => {
      if (!c.words("each", "of")) return null;
      const owner: TurnOwner | PlayerRef | null = c.word("your") !== null ? "your"
        : c.attempt(c => c.word("that") !== null && c.possessive("player") !== null ? "that-player" as const : null)
          ?? parsePossessivePlayer(c);
      if (owner && typeof owner !== "string" && (owner.player === "each-player" || owner.player === "each-opponent")) return null;
      const phase = owner && parsePluralPhase(c);
      return phase && owner ? { trigger: "phase", phase, whose: owner } : null;
    });
    if (eachOf) return eachOf;

    // "of your upkeep" / "of each player's upkeep" / "of each opponent's upkeep"
    let whose: TurnOwner | PlayerRef | undefined;
    if (c.word("your") !== null) whose = "your";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("player") !== null ? true : null)))
      whose = "each-player";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("opponent") !== null ? true : null)))
      whose = "each-opponent";
    else if (c.attempt((c) => (c.word("that") !== null && c.possessive("player") !== null ? true : null)))
      whose = "that-player";
    else if (c.attempt((c) => (c.word("an") !== null && c.possessive("opponent") !== null ? true : null)))
      whose = "opponent";
    else {
      const owner = c.attempt(parsePossessivePlayer);
      if (owner) whose = owner;
      else if (c.word("each") !== null) whose = "each-player"; // "each upkeep", "each end step"
      else c.word("the");
    }

    // CR 507: the beginning of combat step, the same event as "beginning of combat".
    const phase = c.words("combat", "step") ? "combat" as const : parsePhase(c);
    if (!phase) return null;

    // The possessive can follow the phase as an explicit player relation:
    // "the upkeep of enchanted creature's controller" (Takklemaggot).
    if (!whose && c.word("of") !== null) {
      const owner = parsePlayerRef(c);
      if (!owner) return null;
      whose = owner;
    }

    // "… on your turn" / "on each player's turn"
    if (!whose) {
      if (c.attempt((c) => (c.words("on", "your", "turn") ? true : null))) whose = "your";
      else if (
        c.attempt((c) =>
          c.word("on") !== null && c.word("each") !== null && c.possessive("player") !== null && c.word("turn") !== null
            ? true
            : null,
        )
      )
        whose = "each-player";
      // "on enchanted opponent's turn": the turn of the player this Aura enchants.
      else if (c.attempt(c => c.word("on") !== null && c.word("enchanted") !== null && c.possessive("player", "opponent") !== null && c.word("turn") !== null ? true : null))
        whose = { player: "enchanted-player" };
    }
    return { trigger: "phase", phase, whose: whose ?? "each-player" };
  });
}

function parseOrdinal(c: Cursor): number | null {
  const ordinals = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
  const word = c.word(...ordinals);
  return word === null ? null : ordinals.indexOf(word) + 1;
}

function parseOccurrencePeriod(c: Cursor, who?: PlayerRef): { during?: TurnOwner } | null {
  return c.attempt(c => {
    if (c.words("each", "turn") || c.words("in", "a", "turn")) return {};
    // "their second spell during their turn": the performing player's own turn.
    if (who && isSingularOtherPlayer(who) && c.words("during", "their", "turn")) return { during: "event-player" };
    // "your first spell during each of your turns": the first one in each of
    // your own turns (the counted period is still the turn).
    if (who?.player === "you" && c.words("during", "each", "of", "your", "turns")) return { during: "your" };
    if (who && isSingularOtherPlayer(who) && c.words("during", "each", "of", "their", "turns")) return { during: "event-player" };
    if (c.words("during", "each") && c.possessive("opponent") !== null && c.word("turn") !== null)
      return { during: "each-opponent" };
    return null;
  });
}

/** A non-collective player other than "you": the antecedent of a following
 * "their" is the one player performing each matching event. */
function isSingularOtherPlayer(who: PlayerRef): boolean {
  return isSingleOtherPlayer(who);
}

/** "from anywhere other than your hand": one excluded origin zone. */
function parseExcludedOrigin(c: Cursor): ZoneRef | null {
  return c.attempt(c => c.words("from", "anywhere", "other", "than") ? parseZoneRef(c) : null);
}

function hasZoneQualifier(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasZoneQualifier) || !!filter.allOf?.some(hasZoneQualifier);
}

/** The defenders of a player's attack, collectively: "one or more of your
 * opponents", "one or more planeswalkers you control", "you and/or one or more
 * planeswalkers you control". The ability triggers once if the declaration
 * attacks any of them (CR 508.3e; "and/or" accepts either or both). */
function parseAttackedParties(c: Cursor): { whom?: PlayerRef; whomPermanents?: ObjectRef } | null {
  return c.attempt((c): { whom?: PlayerRef; whomPermanents?: ObjectRef } | null => {
    if (c.words("one", "or", "more", "of", "your", "opponents")) return { whom: { player: "an-opponent" } };
    const permanents = (c: Cursor): ObjectRef | null => c.attempt((c): ObjectRef | null => {
      if (!c.words("one", "or", "more")) return null;
      const filter = parseFilter(c);
      // Only a planeswalker or battle can be attacked besides a player (CR 506.3).
      return filter && !filter.orPlayer && filter.types?.length && filter.types.every(type => type === "planeswalker" || type === "battle")
        ? { ref: "filter", filter, count: { amount: "fixed", value: 1 } } : null;
    });
    const only = permanents(c);
    if (only) return { whomPermanents: only };
    const player = parseEventPlayer(c);
    if (!player || player.player === "filtered" || isPluralPlayerRef(player)) return null;
    if (!c.words("and") || !c.punct("/") || c.word("or") === null) return null;
    const second = permanents(c);
    return second && { whom: player, whomPermanents: second };
  });
}

/** CR 508.3c–e: a single-creature qualification is per attacker; an explicit
 * count groups the qualifying creatures in one declaration of attackers. */
function parsePlayerAttack(c: Cursor): Trigger | null {
  return c.attempt(c => {
    const who = parseTriggerPlayer(c);
    if (!who || c.word("attack", "attacks") === null) return null;
    const parties = c.attempt(parseAttackedParties);
    // "attacks a planeswalker you control" / "attack a player or planeswalker":
    // each such defender attacked is its own occurrence (CR 508.3a, 508.3e);
    // only a planeswalker or battle can be attacked besides a player (CR 506.3).
    const defender = parties ? null : c.attempt((c): { whom?: PlayerRef; whomPermanents: ObjectRef } | null => {
      const withPlayer = c.words("a", "player", "or");
      if (!withPlayer && c.word("a") === null) return null;
      const filter = parseFilter(c);
      return filter && !filter.orPlayer && filter.types?.length && filter.types.every(type => type === "planeswalker" || type === "battle")
        ? { ...(withPlayer ? { whom: { player: "a-player" as const } } : {}), whomPermanents: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } } : null;
    });
    const whom = parties ? parties.whom : defender ? defender.whom : c.attempt(parseEventPlayer) ?? undefined;
    const attacked = parties ? { ...(parties.whomPermanents ? { whomPermanents: parties.whomPermanents } : {}), whomGrouping: "one-or-more" as const }
      : defender ? { whomPermanents: defender.whomPermanents } : {};
    let withFilter: ObjectFilter | undefined;
    let count: Comparison | undefined;
    let totalPower: Comparison | undefined;
    let per: "attacker" | "declaration" = "declaration";
    // "attacks with creatures with total power 6 or greater" (CR 508.1).
    const total = c.attempt(c => c.words("with", "creatures", "with", "total", "power") ? parseComparison(c) : null);
    if (total) { withFilter = { types: ["creature"] }; totalPower = total; }
    else if (c.word("with") !== null) {
      // CR 508.3c, 903.3: "your commander" is the commander card you own.
      if (who.player === "you" && c.words("your", "commander"))
        return { trigger: "player-attacks", who, per: "attacker", ...(whom ? { whom } : {}), with: { commander: true, own: "you" } };
      if (c.word("a", "an") !== null) per = "attacker";
      else {
        const bound = c.words("at", "least") ? "ge" : c.words("at", "most") ? "le" : null;
        const value = parseComparison(c);
        if (!value) return null;
        count = bound ? { op: bound, value: value.value } : value;
      }
      const filter = parseFilter(c);
      if (!filter) return null;
      withFilter = filter;
    }
    if (who.player === "you" && !whom && !parties && !withFilter) return { trigger: "you-attack" };
    return { trigger: "player-attacks", who, per, ...(whom ? { whom } : {}), ...attacked, ...(withFilter ? { with: withFilter } : {}), ...(count ? { count } : {}), ...(totalPower ? { totalPower } : {}) };
  });
}

/** CR 109.2: an unqualified creature/artifact/etc description denotes a
 * permanent. A generic card or source-name reference does not prove origin. */
function isPermanentReference(what: ObjectRef): boolean {
  // CR 303.4, 205.3i: "enchanted Forest" is the land this Aura is attached to.
  if (what.ref === "enchanted" && ["plains", "island", "swamp", "mountain", "forest"].includes(what.noun)) return true;
  if (what.ref === "this" || what.ref === "enchanted" || what.ref === "equipped")
    return what.noun !== undefined && ["creature", "artifact", "land", "enchantment", "permanent", "token", "planeswalker", "battle"].includes(what.noun);
  if (what.ref !== "filter" && what.ref !== "that") return false;
  const permanent = (filter: ObjectFilter): boolean => {
    if (filter.zone || filter.cls === "card" || filter.cls === "spell" || filter.cls === "ability") return false;
    if (filter.anyOf?.length) return filter.anyOf.every(permanent);
    return filter.cls === "permanent" || filter.cls === "token" ||
      !!filter.types?.some(type => ["creature", "artifact", "land", "enchantment", "planeswalker", "battle"].includes(type)) ||
      !!filter.subtypes?.length;
  };
  return permanent(what.filter);
}

/** "you play a land or cast a spell", "you cast a black spell or a Swamp
 * you control enters": an "or" left over after one complete event starts a
 * further complete event of the same introduced clause. It either repeats
 * the first event's player subject or has its own subject (CR 603.2). */
function parseSingleTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const start = c.pos;
    const complete = parseSingleEvent(c);
    const first = complete ?? parseEventBeforeAlternative(c);
    if (!first) return null;
    const events = [first];
    const probe = new Cursor(c.tokens.slice(start));
    const introduced = probe.word("when", "whenever") !== null;
    const subject = introduced && parseTriggerPlayer(probe) ? c.tokens.slice(start + 1, start + probe.pos) : null;
    while (introduced && c.isWord("or")) {
      const rest = c.tokens.slice(c.pos + 1);
      const next = c.attempt((c): Trigger | null => {
        for (const prefix of subject ? [subject, []] : [[]]) {
          const tokens = [c.tokens[start], ...prefix, ...rest];
          const alternative = new Cursor(tokens);
          const event = parseSingleEvent(alternative);
          if (!event || alternative.pos <= 1 + prefix.length) continue;
          // "you play a land or cast a spell from anywhere other than your
          // hand": the excluded origin may qualify both actions.
          if (prefix.length && "notFrom" in event) return c.fail("unambiguous origin scope for alternative events");
          c.pos += alternative.pos - prefix.length;
          return event;
        }
        return null;
      });
      if (!next) break;
      events.push(next);
    }
    // An event cut at an "or" stands only with the alternative after it.
    if (events.length === 1) return complete;
    // A trailing ordinal, turn or while qualifier could belong to the last
    // alternative or to all of them; that scope is not guessed.
    if (c.isWord("for", "during", "on", "while")) return c.fail("unambiguous qualifier scope for alternative events");
    // Likewise one trailing "from anywhere other than …" after alternatives
    // that state no origin of their own ("play a land or cast a spell from …").
    const last = events.at(-1)!;
    if (last.trigger === "cast" && last.notFrom && events.slice(0, -1).some(event => !("from" in event && event.from) && !("notFrom" in event && event.notFrom)))
      return c.fail("unambiguous origin scope for alternative events");
    return { trigger: "any-of", events: events.flatMap(event =>
      event.trigger === "any-of" && Object.keys(event).length === 2 ? event.events : [event]) };
  });
}

/** A noun phrase directly before the connecting "or" can otherwise absorb
 * the next event's subject as a filter alternative ("a black spell or a
 * Swamp you control enters"). The first event must end exactly at that "or";
 * the caller still has to parse everything after it as complete events. */
function parseEventBeforeAlternative(c: Cursor): Trigger | null {
  if (!c.isWord("when", "whenever")) return null;
  for (let end = c.pos + 1; end < c.tokens.length; end++) {
    const token = c.tokens[end];
    if (token.kind === "punct") return null;
    if (token.kind !== "word" || token.possessive || token.value !== "or") continue;
    const clause = new Cursor(c.tokens.slice(c.pos, end));
    const event = parseSingleEvent(clause);
    if (event && clause.done()) {
      c.pos = end;
      return event;
    }
  }
  return null;
}

function parseSingleEvent(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const dice = parseDiceTrigger(c, { parseAmount, parseCount, parsePlayerRef });
    if (dice) return dice;
    const at = c.isWord("at");
    const opener = c.word("when", "whenever", "at");
    if (opener === null) return null;
    if (at || opener === "at") return parsePhaseTrigger(c);
    const caused = parseCausedEvent(c, parseSingleEvent) ?? parseGameEvent(c);
    if (caused) return caused;
    const counters = parseCounterChangeTrigger(c);
    if (counters) return counters;
    const state = parseStateTrigger(c);
    if (state) return state;
    const damaged = parsePlayerDamagedEvent(c);
    if (damaged) return damaged;

    // "Whenever you cast a(n) <filter> spell"
    const youCast = c.attempt((c): Trigger | null => {
      const who = parseTriggerPlayer(c);
      if (!who) return null;
      const action = c.word("cast", "casts", "copy", "copies");
      if (action === null) return null;
      const copyToo = c.attempt(c => c.word("or") !== null && c.word("copy", "copies") !== null ? true : null);
      if (copyToo && action !== "cast" && action !== "casts") return null;
      // "cast or cycle ~" (CR 702.29): two ways this one card is used.
      const cycleToo = !copyToo && (action === "cast" || action === "casts")
        && !!c.attempt(c => c.word("or") !== null && c.word("cycle", "cycles") !== null ? true : null);
      const specific = c.attempt(c => {
        const what = parseObjectRef(c);
        // CR 303.4b, 400.7: "enchanted card" denotes this Aura's attached
        // card, not any card with enchanted status (Don't Worry About It).
        // Preserve singular dependent identities before the generic filter.
        // CR 903.3, 601.2: "cast your commander" is casting that very card.
        return what && (what.ref === "self" || what.ref === "it" || what.ref === "commander"
          || ((what.ref === "this" || what.ref === "enchanted" || what.ref === "equipped") && (what.noun === "spell" || what.noun === "card"))
          || (what.ref === "that" && !what.plural && (what.filter.cls === "spell" || what.filter.cls === "card"))) ? what : null;
      });
      if (cycleToo) return specific && !c.isWord("from") ? { trigger: "any-of", events: [
        { trigger: "cast", who, what: specific }, { trigger: "cycle", who, what: specific },
      ] } : null;
      if (specific) {
        if (copyToo || (action !== "cast" && action !== "casts")) return null;
        let from: ZoneRef | undefined;
        const notFrom = parseExcludedOrigin(c);
        if (!notFrom && c.word("from") !== null) {
          const origin = parseZoneRef(c);
          if (!origin) return null;
          from = origin;
        }
        return { trigger: "cast", who, what: specific, ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}) };
      }
      let ordinal: number | null = null;
      let perPlayer = false;
      if (c.word("your") !== null) {
        if (who.player !== "you" || copyToo) return null;
        ordinal = parseOrdinal(c);
        if (ordinal === null) return null;
      } else if (c.word("their") !== null) {
        // "their second spell": each matching player's own count.
        if (!isSingularOtherPlayer(who) || copyToo) return null;
        ordinal = parseOrdinal(c);
        if (ordinal === null) return null;
        perPlayer = true;
      } else c.word("a", "an");
      const filterStart = c.pos;
      // "a spell other than your first spell each turn": the filter must stop
      // before that tail instead of reading "other than …" as its own qualifier.
      const tail = ordinal === null && action !== "copy" && action !== "copies" && !copyToo ? findNotFirstTail(c) : null;
      let filter: ObjectFilter | null;
      if (tail !== null) {
        const bounded = new Cursor(c.tokens.slice(0, tail));
        bounded.pos = c.pos;
        filter = parseFilter(bounded);
        if (!filter || bounded.pos !== tail) return null;
        c.pos = tail;
      } else filter = parseFilter(c);
      if (!filter) return null;
      let what = filter;
      let from: ZoneRef | undefined;
      if (hasZoneQualifier(filter)) {
        // A cast spell is already on the stack (CR 601.2a, 601.2i). Its
        // origin is event history, not its current zone. Only an unambiguous
        // final "from <zone>" can be lifted out of this filter production.
        if (copyToo || (action !== "cast" && action !== "casts") || !filter.zone
          || filter.anyOf?.some(hasZoneQualifier) || filter.allOf?.some(hasZoneQualifier)) return null;
        const tokens = c.tokens.slice(filterStart, c.pos);
        const origins = tokens.flatMap((token, index) => token.kind === "word" && !token.possessive && token.value === "from" ? [index] : []);
        if (origins.length !== 1) return null;
        const originCursor = new Cursor(tokens.slice(origins[0] + 1));
        const origin = parseZoneRef(originCursor);
        if (!origin || !originCursor.done() || JSON.stringify(origin) !== JSON.stringify(filter.zone)) return null;
        const { zone, ...rest } = filter;
        what = rest;
        from = zone;
      }
      const notFrom = !from && !copyToo && (action === "cast" || action === "casts") ? parseExcludedOrigin(c) : null;
      // CR 601.2g: mana spent on the spell that this very source produced.
      const usingManaFrom = !from && !notFrom && !copyToo && (action === "cast" || action === "casts")
        ? c.attempt((c): ObjectRef | null => {
          if (!c.words("using", "mana", "produced", "by")) return null;
          const source = parseObjectRef(c);
          return source && (source.ref === "self" || source.ref === "this") ? source : null;
        }) : null;
      const event: Trigger = action === "cast" || action === "casts"
        ? { trigger: "cast", who, what, ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}), ...(usingManaFrom ? { usingManaFrom } : {}) }
        : { trigger: "copy", who, what };
      if (tail !== null) {
        const rest = parseNotFirstTail(c, who, filter);
        return rest && { ...event, ...rest } as Trigger;
      }
      if (ordinal !== null) {
        const period = parseOccurrencePeriod(c, who);
        if (!period) return null;
        return { ...event, occurrence: { ordinal, period: "turn", ...(perPlayer ? { per: "player" as const } : {}) }, ...period };
      }
      if (copyToo) return { trigger: "any-of", events: [
        { trigger: "cast", who, what }, { trigger: "copy", who, what },
      ] };
      return event;
    });
    if (youCast) return youCast;

    // "Whenever you activate an ability …"
    const activate = c.attempt((c): Trigger | null => {
      let who: PlayerRef | null = null;
      if (c.word("you") !== null) who = { player: "you" };
      else {
        const p = parseTriggerPlayer(c);
        if (p) who = p;
      }
      if (!who) return null;
      if (c.word("activate", "activates") === null) return null;
      // "this creature's outlast ability": that keyword's ability of that very source.
      const own = c.attempt((c): ObjectFilter | null => {
        const abilitySource = parsePossessiveObject(c);
        const head = abilitySource && parseAbilityHead(c);
        return head && head.abilityKeyword ? { ...head, abilitySource } : null;
      });
      if (own) return { trigger: "activate", who, what: own };
      // "an artifact's ability without {T} in its activation cost": an ability
      // of a source matching the possessive noun phrase.
      const possessed = c.attempt((c): ObjectFilter | null => {
        if (c.word("a", "an") === null) return null;
        let offset = 0;
        while (c.peek(offset)?.kind === "word" && !(c.peek(offset) as { possessive?: boolean }).possessive) offset++;
        const head = c.peek(offset);
        if (head?.kind !== "word" || !head.possessive) return null;
        const phrase = new Cursor([...c.tokens.slice(c.pos, c.pos + offset), { ...head, possessive: false }]);
        const source = parseFilter(phrase);
        if (!source || !phrase.done() || !c.tokens[c.pos + offset + 1]) return null;
        c.pos += offset + 1;
        if (c.word("ability", "abilities") === null) return null;
        const tapSymbolInCost = c.attempt(c => {
          const preposition = c.word("with", "without");
          return preposition && c.symbol() === "T" && c.words("in", "its", "activation", "cost") ? preposition === "with" : null;
        });
        return { cls: "ability", abilitySource: { ref: "filter", filter: source, count: { amount: "fixed", value: 1 } },
          ...(tapSymbolInCost !== null ? { tapSymbolInCost } : {}) };
      });
      if (possessed) return { trigger: "activate", who, what: possessed };
      c.word("a", "an");
      const what = parseFilter(c);
      if (!what) return null;
      return { trigger: "activate", who, what };
    });
    if (activate) return activate;

    // "an ability of equipped creature is activated": by any player, the same
    // event as "a player activates an ability of equipped creature".
    const activated = c.attempt((c): Trigger | null => {
      if (c.word("a", "an") === null) return null;
      const what = parseFilter(c);
      return what && what.cls === "ability" && c.words("is", "activated") ? { trigger: "activate", who: { player: "a-player" }, what } : null;
    });
    if (activated) return activated;

    const playerAttack = parsePlayerAttack(c);
    if (playerAttack) return playerAttack;

    // Player life/draw/discard triggers.
    const playerEvent = c.attempt((c): Trigger | null => {
      const who = parseTriggerPlayer(c);
      if (!who) return null;
      const action = parsePlayerActionEvent(c, who) ?? parsePlayerKeywordEvent(c, who);
      if (action) return action;
      if (c.word("cycle", "cycles") !== null) {
        const discardToo = c.words("or", "discard");
        const card = parseTriggerSubject(c);
        if (!card || card.grouping) return null;
        const what = card.what;
        return discardToo ? { trigger: "any-of", events: [
          { trigger: "cycle", who, what }, { trigger: "discards", who, what },
        ] } : { trigger: "cycle", who, what };
      }
      // "gain or lose life": either change of that player's life total.
      if (c.attempt(c => c.word("gain", "gains") !== null && c.word("or") !== null && c.word("lose", "loses") !== null && c.word("life") !== null ? true : null))
        return { trigger: "any-of", events: [{ trigger: "gains-life", who }, { trigger: "loses-life", who }] };
      // CR 603.10d: losing control of an object looks back in time.
      const lostControl = c.attempt((c): Trigger | null => {
        if (c.word("lose", "loses") === null || !c.words("control", "of")) return null;
        const what = parseObjectRef(c);
        return what && { trigger: "loses-control", who, what };
      });
      if (lostControl) return lostControl;
      if (c.word("gain", "gains") !== null && c.word("life") !== null)
        return { trigger: "gains-life", who };
      if (c.word("lose", "loses") !== null && c.word("life") !== null)
        return { trigger: "loses-life", who };
      if (c.word("draw", "draws") !== null) {
        if (c.words("a", "card")) {
          if (c.word("except") === null) return { trigger: "draws", who };
          // Same wording and meaning as the draw replacement exception.
          const pronoun = c.words("the", "first", "one") ? c.word("you", "they") : null;
          if (!pronoun || (pronoun === "you") !== (who.player === "you") || (pronoun === "they" && !isSingularOtherPlayer(who))) return null;
          if (!c.words("draw", "in", "each", "of") || c.word(pronoun === "you" ? "your" : "their") === null || !c.words("draw", "steps")) return null;
          return { trigger: "draws", who, except: "first-in-each-draw-step" };
        }
        const perPlayer = isSingularOtherPlayer(who) && c.word("their") !== null;
        if (!perPlayer && (who.player !== "you" || c.word("your") === null)) return null;
        const ordinal = parseOrdinal(c);
        if (ordinal === null || c.word("card") === null) return null;
        const period = parseOccurrencePeriod(c, who);
        return period && { trigger: "draws", who, occurrence: { ordinal, period: "turn", ...(perPlayer ? { per: "player" as const } : {}) }, ...period };
      }
      if (c.word("discard", "discards") !== null) {
        const subject = parseTriggerSubject(c);
        if (!subject) return null;
        // Preserve the old unqualified node while retaining every restriction
        // and one-or-more grouping on the expanded discard grammar.
        const simpleCard = subject.what.ref === "filter" && !subject.grouping
          && Object.keys(subject.what.filter).length === 1 && subject.what.filter.cls === "card";
        return { trigger: "discards", who,
          ...(!simpleCard ? { what: subject.what } : {}),
          ...(subject.grouping ? { grouping: subject.grouping } : {}) };
      }
      if (c.word("sacrifice", "sacrifices") !== null) {
        c.word("a", "an");
        const what = parseFilter(c);
        if (!what) return null;
        return { trigger: "sacrifices", who, what };
      }
      if (c.word("scry", "scries") !== null) return { trigger: "scries", who };
      if (c.word("surveil", "surveils") !== null) return { trigger: "surveils", who };
      return null;
    });
    if (playerEvent) return playerEvent;

    // Object-subject triggers.
    const parsedSubject = parseTriggerSubject(c);
    if (!parsedSubject) return c.fail("trigger subject");
    // "~ and at least one other creature attack": one attack declaration
    // containing the subject and the stated number of further attackers.
    const alongside = c.attempt(c => {
      if (parsedSubject.grouping || !isSingleIdentifiedObject(parsedSubject.what) || c.word("and") === null || !c.isWord("at")) return null;
      const count = parseComparison(c);
      const filter = count && count.op === "ge" ? parseFilter(c) : null;
      return filter && count && c.word("attack") !== null ? { filter, count } : null;
    });
    if (alongside) return { trigger: "attacks", what: parsedSubject.what, alone: undefined, whom: undefined, alongside };
    const subjects = [parsedSubject];
    while (c.word("or") !== null) {
      const next = parseTriggerSubject(c);
      if (!next) return null;
      // A union of singular subjects differs from one collective event. Do
      // not guess scope where the two syntaxes are mixed (CR 603.2c, 700.1).
      if (next.grouping || parsedSubject.grouping) return c.fail("ungrouped alternative subject");
      subjects.push(next);
    }
    const eventStart = c.pos;
    const events: Trigger[] = [];
    for (const { what: subject, grouping, groupSize } of subjects) {
      c.pos = eventStart;
      const event = parseSubjectEvents(c, subject);
      if (!event) return null;
      // A sized group is printed only for attacks (declared together, CR 508.1).
      if (groupSize && event.trigger !== "attacks") return c.fail("attack by a sized group");
      events.push(grouping ? { ...event, grouping, ...(groupSize ? { groupSize } : {}) } : event);
    }
    return events.length === 1 ? events[0] : { trigger: "any-of", events };
  });
}

/** A player participant of an event. "One of your opponents" is each single
 * opponent the event happens to, the same participant as "an opponent". */
function parseEventPlayer(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.words("one", "of", "your", "opponents")) return { player: "an-opponent" };
    return parseMostLifePlayer(c) ?? parsePlayerRef(c);
  });
}

function isSingleIdentifiedObject(what: ObjectRef): boolean {
  return what.ref === "self" || what.ref === "this" || what.ref === "equipped" || what.ref === "enchanted";
}

function parseSubjectEvents(c: Cursor, subject: ObjectRef): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const first = parseObjectEvent(c, subject);
    if (!first) return null;
    const events = [first];
    while (c.word("or") !== null) {
      const next = parseObjectEvent(c, subject);
      if (!next) return null;
      events.push(next);
    }
    return events.length === 1 ? first : { trigger: "any-of", events };
  });
}

/** CR 603.1, 603.2c: preserve distinct event alternatives and their qualifiers.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseObjectEvent(c: Cursor, subject: ObjectRef): Trigger | null {
  return c.attempt((c): Trigger | null => {

    // "enters (the battlefield)?" — possibly "or leaves" / "or attacks" / "or dies".
    // Plural verb forms cover collective subjects ("one or more creatures enter").
    if (c.word("enters", "enter") !== null) {
      c.words("the", "battlefield");
      const untapped = c.word("untapped") !== null;
      // "enters tapped" / "enters transformed": the entry event itself (CR 603.6a, 712.14a).
      const tapped = !untapped && c.word("tapped") !== null;
      const transformed = !untapped && !tapped && c.word("transformed") !== null;
      let controller: PlayerRef | undefined;
      let from: ZoneRef | undefined;
      let notFrom: ZoneRef | null = null;
      for (;;) {
        if (!controller && c.words("under", "your", "control")) controller = { player: "you" };
        else if (!from && !notFrom && (notFrom = parseExcludedOrigin(c))) continue;
        else if (!from && !notFrom && c.word("from") !== null) {
          const zone = parseZoneRef(c);
          if (!zone) return null;
          from = zone;
        } else break;
      }
      return { trigger: "enters", what: subject, ...(untapped ? { untapped } : {}), ...(tapped ? { tapped } : {}), ...(transformed ? { transformed } : {}), ...(controller ? { controller } : {}), ...(from ? { from } : {}), ...(notFrom ? { notFrom } : {}) };
    }
    if (c.word("leaves", "leave") !== null) {
      const from = parseZoneRef(c);
      if (from && from.zone !== "battlefield") return { trigger: "leaves-zone", what: subject, from };
      if (from && Object.keys(from).length !== 1) return null;
      if (from && c.words("without", "dying")) return { trigger: "leaves", what: subject, withoutDying: true };
      return { trigger: "leaves", what: subject };
    }
    if (c.word("dies", "die") !== null) return { trigger: "dies", what: subject };
    if (c.attempt((c) => (c.word("is", "are") !== null && c.words("put", "into") ? true : null))) {
      const to = parseZoneRef(c);
      if (to && to.zone !== "graveyard" && to.zone !== "battlefield" && to.zone !== "stack" && !to.position) {
        // Exile, a library, a hand or the command zone. The printed origin
        // is kept as printed; none is supplied when it is omitted.
        if (c.word("from") === null) return { trigger: "put-into-zone", what: subject, to };
        // "from your library and/or your graveyard" / "from graveyards and/or the battlefield".
        const fromAnyOf = c.attempt((c): ZoneRef[] | null => {
          const zone = (c: Cursor): ZoneRef | null => c.attempt(c => c.word("graveyards") !== null ? { zone: "graveyard" } : parseZoneRef(c));
          const first = zone(c);
          if (!first || !c.word("and") || !c.punct("/") || c.word("or") === null) return null;
          const second = zone(c);
          return second && !(first.zone === second.zone && first.owner === second.owner) ? [first, second] : null;
        });
        if (fromAnyOf) return { trigger: "put-into-zone", what: subject, fromAnyOf, to };
        const origin = c.word("anywhere") !== null ? "anywhere" : parseZoneRef(c);
        if (!origin || c.isWord("other")) return c.fail("zone event source");
        return { trigger: "put-into-zone", what: subject, from: origin, to };
      }
      if (to?.zone !== "graveyard") return null;
      if (c.word("from") === null)
        return isPermanentReference(subject) ? { trigger: "put-into-graveyard", what: subject, from: "battlefield", to } : null;
      const source = c.word("anywhere") !== null ? "anywhere" : parseZoneRef(c);
      if (!source) return c.fail("graveyard event source");
      const from = source === "anywhere" ? source : source.zone === "battlefield" && !source.owner ? "battlefield" : source;
      if (from === "anywhere" && c.words("other", "than")) {
        const notFrom = parseZoneRef(c);
        return notFrom && { trigger: "put-into-graveyard", what: subject, from, to, notFrom };
      }
      return { trigger: "put-into-graveyard", what: subject, from, to };
    }

    if (c.word("attacks", "attack") !== null) {
      let alone = c.word("alone") !== null;
      // optional "you or a planeswalker you control" tail
      const whom: (PlayerRef | ObjectRef)[] = [];
      const first = c.attempt(parseEventPlayer) ?? c.attempt(parseObjectRef);
      if (first) {
        whom.push(first);
        while (c.word("or") !== null) {
          const nxt = c.attempt(parseEventPlayer) ?? c.attempt(parseObjectRef);
          if (!nxt) return null;
          whom.push(nxt);
        }
      }
      // "attacks a player alone": it is the only declared attacker (CR 506.5)
      // and it attacks that player.
      if (!alone && whom.length && c.word("alone") !== null) alone = true;
      // CR 509.3g: a declare-blockers event of its own, not a qualified attack.
      if (c.isWord("and") && c.attempt(c => c.word("and") !== null && c.word("isn't", "aren't") !== null && c.word("blocked") !== null ? true : null))
        return alone ? c.fail("unblocked-attack event without 'alone'") : { trigger: "attacks-and-isnt-blocked", what: subject, ...(whom.length ? { whom } : {}) };
      return { trigger: "attacks", what: subject, alone: alone || undefined, whom: whom.length ? whom : undefined };
    }
    if (c.word("exploits", "exploit") !== null) {
      // CR 702.110b: the sacrificed creature, described by the printed filter.
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && { trigger: "exploits", what: subject, exploited: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } };
    }
    if (c.word("blocks", "block") !== null) {
      if (c.words("or", "becomes", "blocked")) {
        if (c.word("by") !== null) {
          const other = parseTriggerSubject(c);
          if (!other) return null;
          // "by one or more black creatures": each side is one occurrence per
          // declaration of the matching group (CR 509.3e).
          return { trigger: "any-of", events: [
            { trigger: "blocks", what: subject, blocking: other.what,
              ...(other.grouping ? { blockingGrouping: other.grouping } : {}), ...(other.groupSize ? { blockingSize: other.groupSize } : {}) },
            { trigger: "becomes-blocked", what: subject, by: other.what,
              ...(other.grouping ? { byGrouping: other.grouping } : {}), ...(other.groupSize ? { bySize: other.groupSize } : {}) },
          ] };
        }
        return { trigger: "blocks", what: subject, orBecomesBlocked: true };
      }
      // "blocks one or more black creatures" / "blocks two or more creatures".
      const group = c.attempt(c => { const g = parseTriggerSubject(c); return g?.grouping ? g : null; });
      if (group) return { trigger: "blocks", what: subject, blocking: group.what, blockingGrouping: "one-or-more",
        ...(group.groupSize ? { blockingSize: group.groupSize } : {}) };
      const blocking = c.attempt(parseObjectRef);
      return { trigger: "blocks", what: subject,
        ...(blocking ? { blocking } : {}) };
    }
    if (c.words("becomes", "blocked") || c.words("become", "blocked")) {
      const group = c.attempt(c => { const g = c.word("by") !== null ? parseTriggerSubject(c) : null; return g?.grouping ? g : null; });
      if (group) return { trigger: "becomes-blocked", what: subject, by: group.what, byGrouping: "one-or-more",
        ...(group.groupSize ? { bySize: group.groupSize } : {}) };
      const by = c.attempt((c): ObjectRef | null => {
        if (c.word("by") === null) return null;
        c.word("a", "an");
        const filter = parseFilter(c);
        return filter && { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
      });
      return { trigger: "becomes-blocked", what: subject, by: by ?? undefined };
    }
    if (c.words("becomes", "tapped") || c.words("become", "tapped")) return { trigger: "becomes-tapped", what: subject };
    if (c.words("becomes", "untapped") || c.words("become", "untapped")) return { trigger: "becomes-untapped", what: subject };
    // Class designation changes are transitions, not battlefield entry or
    // level-counter changes (CR716.2a/b/d).
    if (c.words("becomes", "level")) {
      const level = c.number();
      return level !== null && Number.isSafeInteger(level) && level >= 1
        ? { trigger: "becomes-class-level", what: subject, level } : null;
    }
    const status = c.attempt(c => c.word("becomes", "become") !== null ? c.word("monstrous", "renowned", "crewed", "saddled", "plotted") : null);
    // CR 603.2e, 701.37b, 702.112b: these are transitions, not persistent
    // states and not synonyms for getting +1/+1 counters.
    if (status === "monstrous" || status === "renowned" || status === "crewed" || status === "saddled" || status === "plotted")
      return { trigger: "becomes-status", what: subject, status };
    if (c.words("is", "turned", "face", "up")) return { trigger: "turned-face-up", what: subject };
    if (c.word("mutates") !== null) return { trigger: "mutates", what: subject };
    if (c.word("transforms") !== null) {
      if (c.word("into") === null) return { trigger: "transforms", what: subject };
      // The normalized name identifies this printed face, not an ObjectRef
      // that follows whichever face the permanent currently has (CR 712).
      if (!c.selfref()) {
        // "transforms into a non-Human creature": its characteristics afterwards.
        if (c.word("a", "an") === null) return null;
        const filter = parseFilter(c);
        return filter ? { trigger: "transforms", what: subject, into: { filter } } : null;
      }
      return { trigger: "transforms", what: subject, into: { face: "source-face" } };
    }
    if (c.attempt((c) => (c.words("becomes", "the", "target", "of") || c.words("become", "the", "target", "of") ? true : null))) {
      c.word("a", "an");
      const of = parseFilter(c);
      if (!of) return null;
      return { trigger: "becomes-target", what: subject, of };
    }
    if (c.attempt((c) => (c.words("is", "dealt", "damage") ? true : null)))
      return { trigger: "is-dealt-damage", what: subject };
    const dealtMore = parseObjectDealtDamage(c, subject);
    if (dealtMore) return dealtMore;
    const dealtKind = c.attempt(c => c.word("is", "are") !== null && c.word("dealt") !== null
      ? c.attempt(c => { const kind = c.word("combat", "noncombat"); return kind && c.word("damage") !== null ? kind : null; }) : null);
    if (dealtKind) return { trigger: "is-dealt-damage", what: subject, combat: dealtKind === "combat" };
    if (c.word("deals", "deal") !== null) {
      const kind = c.word("combat", "noncombat");
      const combat = kind === "combat";
      const amount = kind ? null : parseDamageAmount(c);
      if (c.word("damage") === null) return null;
      let to: "any" | PlayerRef | ObjectRef | undefined;
      if (c.word("to") !== null) {
        // "a player or planeswalker": each damaged player or such permanent.
        const union = c.attempt((c): ObjectRef | null => {
          const start = c.pos;
          if (!c.words("a", "player", "or")) return null;
          c.pos = start;
          const recipient = parseObjectRef(c);
          return recipient?.ref === "filter" && recipient.filter.orPlayer && recipient.filter.types?.length === 1
            && Object.keys(recipient.filter).length === 2 ? recipient : null;
        });
        // "one or more players / of your opponents / creatures": all the
        // matching recipients of the event are one occurrence.
        const collective = c.attempt((c): "any" | PlayerRef | ObjectRef | null => {
          if (!c.words("one", "or", "more")) return null;
          if (c.word("players") !== null) return "any";
          if (c.words("of", "your", "opponents")) return { player: "an-opponent" };
          const filter = parseFilter(c);
          return filter && { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
        });
        if (collective) return { trigger: "deals-damage", what: subject, combat: kind === "noncombat" ? false : combat || undefined, to: collective, recipients: "one-or-more", ...(amount ? { amount } : {}) };
        // "a player who controls more lands than you": a qualified player.
        const qualified = union ? null : c.attempt((c): PlayerRef | null => {
          const who = parseEventPlayer(c);
          return who?.player === "filtered" ? who : null;
        });
        if (union) to = union;
        else if (qualified) to = qualified;
        else if (c.attempt((c) => (c.words("a", "player") ? true : null))) to = "any";
        else {
          const p = c.attempt(parseEventPlayer) ?? c.attempt(parseTriggerPlayer);
          if (p) to = p;
          else {
            const o = parseObjectRef(c);
            if (!o) return null;
            to = o;
          }
        }
      }
      // "damage to you or a permanent you control": each recipient is its own
      // occurrence (CR 120.3, 603.2c), the same split as the passive wording.
      if (to && typeof to === "object" && "player" in to && to.player === "you") {
        const permanent = c.attempt((c): ObjectRef | null => {
          if (c.word("or") === null || c.word("a", "an") === null) return null;
          const filter = parseFilter(c);
          return filter && !filter.orPlayer && !c.isWord("or") ? { ref: "filter", filter, count: { amount: "fixed", value: 1 } } : null;
        });
        if (permanent) {
          const base = { what: subject, combat: kind === "noncombat" ? false : combat || undefined, ...(amount ? { amount } : {}) };
          return { trigger: "any-of", events: [{ trigger: "deals-damage", ...base, to }, { trigger: "deals-damage", ...base, to: permanent }] };
        }
      }
      return { trigger: "deals-damage", what: subject, combat: kind === "noncombat" ? false : combat || undefined, to, ...(amount ? { amount } : {}) };
    }
    if (c.attempt((c) => (c.words("is", "tapped", "for", "mana") ? true : null)))
      return { trigger: "taps-for-mana", what: subject };
    const producing = c.attempt(c => c.words("is", "tapped", "for") ? c.symbol() : null);
    if (producing && /^[WUBRGC]$/.test(producing))
      return { trigger: "taps-for-mana", what: subject, producing: producing as "W" | "U" | "B" | "R" | "G" | "C" };

    const keywordEvent = parseObjectKeywordEvent(c, subject);
    if (keywordEvent) return keywordEvent;

    return c.fail("trigger event");
  });
}

function parseQualifiedTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const first = parseSingleTrigger(c);
    if (!first) return null;
    let result = first;
    let ownTurn: TurnOwner | undefined;
    const occurrence = c.attempt(c => {
      if (!c.words("for", "the")) return null;
      const ordinal = parseOrdinal(c);
      if (!ordinal || c.word("time") === null) return null;
      if (c.words("each", "turn")) return { ordinal, period: "turn" as const };
      // "for the first time during each of your turns": the first occurrence
      // in each turn of that player, which is also a turn restriction.
      // "their" counts each performing player separately, in their own turn.
      if (!c.words("during", "each", "of")) return null;
      if (c.word("your") !== null) {
        if (!("who" in result) || result.who?.player !== "you" || !c.word("turns")) return null;
        ownTurn = "your";
        return { ordinal, period: "turn" as const };
      }
      if (c.word("their") === null || !c.word("turns") || !("who" in result) || !result.who || !isSingularOtherPlayer(result.who)) return null;
      ownTurn = "event-player";
      return { ordinal, period: "turn" as const, per: "player" as const };
    });
    if (occurrence) {
      if (result.occurrence) return c.fail("single trigger event ordinal");
      // Counter history over a variable recipient set may be per recipient
      // or global. The current ordinal only models one fixed source's
      // matching occurrences (Axgard Artisan/U.S.S. Enterprise-D).
      if (result.trigger === "counter-change" && (result.on.recipient !== "object"
        || result.on.what.ref !== "self" && result.on.what.ref !== "this"))
        return c.fail("source-specific counter-event ordinal");
      result = { ...result, occurrence, ...(ownTurn ? { during: ownTurn } : {}) };
    }
    const during = c.attempt((c): TurnOwner | null => {
      if (c.word("during", "on") === null) return null;
      if (c.words("your", "turn")) return "your";
      if (c.word("each") !== null && c.possessive("opponent") !== null && c.word("turn") !== null) return "each-opponent";
      if (c.word("an") !== null && c.possessive("opponent") !== null && c.word("turn") !== null) return "opponent";
      // "during their turn": the turn of the one other player performing the event.
      if (c.words("their", "turn") && "who" in result && result.who !== undefined && isSingularOtherPlayer(result.who)) return "event-player";
      return null;
    });
    if (during) {
      if (result.during) return c.fail("single trigger turn restriction");
      result = { ...result, during };
    }
    // "during combat" / "during your main phase" / "during the declare attackers step".
    const inPhase = c.attempt(c => {
      if (c.word("during") === null) return null;
      const whose = c.word("your") !== null ? "your" as const : (c.word("the"), undefined);
      const phase = parsePhase(c);
      return phase && phase !== "turn" ? { phase, ...(whose ? { whose } : {}) } : null;
    });
    if (inPhase) result = { ...result, inPhase };
    if (c.isWord("while")) return parseTriggerGate(c, result);
    return result;
  });
}

/** Multiple independently introduced trigger clauses describe alternatives,
 * e.g. "When this enters and whenever you cast ..." (CR 603.1). Qualifiers
 * belong to their own introduced clause rather than all sibling events. */
export function parseTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const first = parseQualifiedTrigger(c);
    if (!first) return null;
    const events = [first];
    while (c.word("and") !== null) {
      const next = parseQualifiedTrigger(c);
      if (!next) return null;
      events.push(next);
    }
    return events.length === 1 ? first : { trigger: "any-of", events };
  });
}

/** Instructions on the ability, not resolution actions or event ordinals.
 * "Do this only once" uses the different action-history rule in CR 603.2h.
 * CR 603.1: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseTriggerLimit(c: Cursor): TriggerLimit | null {
  return c.attempt(c => {
    if (!c.words("this", "ability", "triggers", "only")) return null;
    const number = c.word("once", "twice");
    if (!number) return null;
    const maximum = number === "once" ? 1 : 2;
    const period = c.words("each", "turn") ? "turn" : "ability-instance";
    c.punct(".");
    return c.done() ? { maximum, period } : null;
  });
}
