// Trigger condition parsing: the clause between "When/Whenever/At" and the
// comma that starts the effect text.

import type { Comparison, ObjectFilter, ObjectRef, PhaseName, PlayerRef, Trigger, TriggerLimit, TurnOwner, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseCount, parseComparison, parseFilter, parseObjectRef, parsePlayerRef, parsePossessivePlayer, parseZoneRef } from "./refs.ts";
import { parseDiceTrigger } from "./dice.ts";
import { parseTriggerGate } from "./trigger-gates.ts";
import { parseCounterChangeTrigger } from "./counter-events.ts";

/** Subject of an event trigger: "~", "a creature", "another Elf you control" … */
type TriggerSubject = { what: ObjectRef; grouping?: "one-or-more" };

function parseTriggerSubject(c: Cursor): TriggerSubject | null {
  return c.attempt((c): TriggerSubject | null => {
    // An attachment reference identifies the object attached to this source;
    // it is not the set of all equipped/enchanted creatures (CR 301.5, 303.4).
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.isWord("equipped", "enchanted")) {
      const what = parseObjectRef(c);
      return what && { what };
    }
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

/** "At the beginning of <owner> <phase>" / "of <phase> on <owner> turn" */
function parsePhaseTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.words("end", "of", "combat")) return { trigger: "phase", phase: "end-of-combat", whose: "each-player" };
    if (!c.words("the", "beginning", "of")) return null;

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

    const phase = parsePhase(c);
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
    }
    return { trigger: "phase", phase, whose: whose ?? "each-player" };
  });
}

function parseOrdinal(c: Cursor): number | null {
  const ordinals = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
  const word = c.word(...ordinals);
  return word === null ? null : ordinals.indexOf(word) + 1;
}

function parseOccurrencePeriod(c: Cursor): { during?: TurnOwner } | null {
  return c.attempt(c => {
    if (c.words("each", "turn")) return {};
    if (c.words("during", "each") && c.possessive("opponent") !== null && c.word("turn") !== null)
      return { during: "each-opponent" };
    return null;
  });
}

function hasZoneQualifier(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasZoneQualifier) || !!filter.allOf?.some(hasZoneQualifier);
}

/** CR 508.3c–e: a single-creature qualification is per attacker; an explicit
 * count groups the qualifying creatures in one declaration of attackers. */
function parsePlayerAttack(c: Cursor): Trigger | null {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    if (!who || c.word("attack", "attacks") === null) return null;
    const whom = c.attempt(parsePlayerRef);
    let withFilter: ObjectFilter | undefined;
    let count: Comparison | undefined;
    let per: "attacker" | "declaration" = "declaration";
    if (c.word("with") !== null) {
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
    if (who.player === "you" && !whom && !withFilter) return { trigger: "you-attack" };
    return { trigger: "player-attacks", who, per, ...(whom ? { whom } : {}), ...(withFilter ? { with: withFilter } : {}), ...(count ? { count } : {}) };
  });
}

/** CR 109.2: an unqualified creature/artifact/etc description denotes a
 * permanent. A generic card or source-name reference does not prove origin. */
function isPermanentReference(what: ObjectRef): boolean {
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

function parseSingleTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const dice = parseDiceTrigger(c, { parseAmount, parseCount, parsePlayerRef });
    if (dice) return dice;
    const at = c.isWord("at");
    const opener = c.word("when", "whenever", "at");
    if (opener === null) return null;
    if (at || opener === "at") return parsePhaseTrigger(c);
    const counters = parseCounterChangeTrigger(c);
    if (counters) return counters;

    // "Whenever you cast a(n) <filter> spell"
    const youCast = c.attempt((c): Trigger | null => {
      const who = parsePlayerRef(c);
      if (!who) return null;
      const action = c.word("cast", "casts", "copy", "copies");
      if (action === null) return null;
      const copyToo = c.attempt(c => c.word("or") !== null && c.word("copy", "copies") !== null ? true : null);
      if (copyToo && action !== "cast" && action !== "casts") return null;
      const specific = c.attempt(c => {
        const what = parseObjectRef(c);
        // CR 303.4b, 400.7: "enchanted card" denotes this Aura's attached
        // card, not any card with enchanted status (Don't Worry About It).
        // Preserve singular dependent identities before the generic filter.
        return what && (what.ref === "self" || what.ref === "it"
          || ((what.ref === "this" || what.ref === "enchanted" || what.ref === "equipped") && (what.noun === "spell" || what.noun === "card"))
          || (what.ref === "that" && !what.plural && (what.filter.cls === "spell" || what.filter.cls === "card"))) ? what : null;
      });
      if (specific) {
        if (copyToo || (action !== "cast" && action !== "casts")) return null;
        let from: ZoneRef | undefined;
        if (c.word("from") !== null) {
          const origin = parseZoneRef(c);
          if (!origin) return null;
          from = origin;
        }
        return { trigger: "cast", who, what: specific, ...(from ? { from } : {}) };
      }
      let ordinal: number | null = null;
      if (c.word("your") !== null) {
        if (who.player !== "you" || copyToo) return null;
        ordinal = parseOrdinal(c);
        if (ordinal === null) return null;
      } else c.word("a", "an");
      const filterStart = c.pos;
      const filter = parseFilter(c);
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
      const event: Trigger = action === "cast" || action === "casts"
        ? { trigger: "cast", who, what, ...(from ? { from } : {}) }
        : { trigger: "copy", who, what };
      if (ordinal !== null) {
        const period = parseOccurrencePeriod(c);
        if (!period) return null;
        return { ...event, occurrence: { ordinal, period: "turn" }, ...period };
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
        const p = parsePlayerRef(c);
        if (p) who = p;
      }
      if (!who) return null;
      if (c.word("activate", "activates") === null) return null;
      c.word("a", "an");
      const what = parseFilter(c);
      if (!what) return null;
      return { trigger: "activate", who, what };
    });
    if (activate) return activate;

    const playerAttack = parsePlayerAttack(c);
    if (playerAttack) return playerAttack;

    // Player life/draw/discard triggers.
    const playerEvent = c.attempt((c): Trigger | null => {
      const who = parsePlayerRef(c);
      if (!who) return null;
      if (c.word("cycle", "cycles") !== null) {
        const discardToo = c.words("or", "discard");
        const card = parseTriggerSubject(c);
        if (!card || card.grouping) return null;
        const what = card.what;
        return discardToo ? { trigger: "any-of", events: [
          { trigger: "cycle", who, what }, { trigger: "discards", who, what },
        ] } : { trigger: "cycle", who, what };
      }
      if (c.word("gain", "gains") !== null && c.word("life") !== null)
        return { trigger: "gains-life", who };
      if (c.word("lose", "loses") !== null && c.word("life") !== null)
        return { trigger: "loses-life", who };
      if (c.word("draw", "draws") !== null) {
        if (c.words("a", "card")) return { trigger: "draws", who };
        if (who.player !== "you" || c.word("your") === null) return null;
        const ordinal = parseOrdinal(c);
        if (ordinal === null || c.word("card") === null) return null;
        const period = parseOccurrencePeriod(c);
        return period && { trigger: "draws", who, occurrence: { ordinal, period: "turn" }, ...period };
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
    for (const { what: subject, grouping } of subjects) {
      c.pos = eventStart;
      const event = parseSubjectEvents(c, subject);
      if (!event) return null;
      events.push(grouping ? { ...event, grouping } : event);
    }
    return events.length === 1 ? events[0] : { trigger: "any-of", events };
  });
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
      let controller: PlayerRef | undefined;
      let from: ZoneRef | undefined;
      for (;;) {
        if (!controller && c.words("under", "your", "control")) controller = { player: "you" };
        else if (!from && c.word("from") !== null) {
          const zone = parseZoneRef(c);
          if (!zone) return null;
          from = zone;
        } else break;
      }
      return { trigger: "enters", what: subject, ...(controller ? { controller } : {}), ...(from ? { from } : {}) };
    }
    if (c.word("leaves", "leave") !== null) {
      const from = parseZoneRef(c);
      if (from && from.zone !== "battlefield") return { trigger: "leaves-zone", what: subject, from };
      if (from && Object.keys(from).length !== 1) return null;
      return { trigger: "leaves", what: subject };
    }
    if (c.word("dies", "die") !== null) return { trigger: "dies", what: subject };
    if (c.attempt((c) => (c.word("is", "are") !== null && c.words("put", "into") ? true : null))) {
      const to = parseZoneRef(c);
      if (to?.zone !== "graveyard") return null;
      if (c.word("from") === null)
        return isPermanentReference(subject) ? { trigger: "put-into-graveyard", what: subject, from: "battlefield", to } : null;
      const source = c.word("anywhere") !== null ? "anywhere" : parseZoneRef(c);
      if (!source) return c.fail("graveyard event source");
      const from = source === "anywhere" ? source : source.zone === "battlefield" && !source.owner ? "battlefield" : source;
      return { trigger: "put-into-graveyard", what: subject, from, to };
    }

    if (c.word("attacks", "attack") !== null) {
      const alone = c.word("alone") !== null;
      // optional "you or a planeswalker you control" tail
      const whom: (PlayerRef | ObjectRef)[] = [];
      const first = c.attempt(parsePlayerRef) ?? c.attempt(parseObjectRef);
      if (first) {
        whom.push(first);
        while (c.word("or") !== null) {
          const nxt = c.attempt(parsePlayerRef) ?? c.attempt(parseObjectRef);
          if (!nxt) return null;
          whom.push(nxt);
        }
      }
      return { trigger: "attacks", what: subject, alone: alone || undefined, whom: whom.length ? whom : undefined };
    }
    if (c.word("blocks", "block") !== null) {
      if (c.words("or", "becomes", "blocked")) {
        if (c.word("by") !== null) {
          const other = parseTriggerSubject(c);
          if (!other || other.grouping) return null;
          return { trigger: "any-of", events: [
            { trigger: "blocks", what: subject, blocking: other.what },
            { trigger: "becomes-blocked", what: subject, by: other.what },
          ] };
        }
        return { trigger: "blocks", what: subject, orBecomesBlocked: true };
      }
      const blocking = c.attempt(parseObjectRef);
      return { trigger: "blocks", what: subject,
        ...(blocking ? { blocking } : {}) };
    }
    if (c.words("becomes", "blocked")) {
      const by = c.attempt((c): ObjectRef | null => {
        if (c.word("by") === null) return null;
        c.word("a", "an");
        const filter = parseFilter(c);
        return filter && { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
      });
      return { trigger: "becomes-blocked", what: subject, by: by ?? undefined };
    }
    if (c.words("becomes", "tapped")) return { trigger: "becomes-tapped", what: subject };
    if (c.words("becomes", "untapped")) return { trigger: "becomes-untapped", what: subject };
    // Class designation changes are transitions, not battlefield entry or
    // level-counter changes (CR716.2a/b/d).
    if (c.words("becomes", "level")) {
      const level = c.number();
      return level !== null && Number.isSafeInteger(level) && level >= 1
        ? { trigger: "becomes-class-level", what: subject, level } : null;
    }
    const status = c.attempt(c => c.word("becomes", "become") !== null ? c.word("monstrous", "renowned") : null);
    // CR 603.2e, 701.37b, 702.112b: these are transitions, not persistent
    // states and not synonyms for getting +1/+1 counters.
    if (status === "monstrous" || status === "renowned") return { trigger: "becomes-status", what: subject, status };
    if (c.words("is", "turned", "face", "up")) return { trigger: "turned-face-up", what: subject };
    if (c.word("mutates") !== null) return { trigger: "mutates", what: subject };
    if (c.word("transforms") !== null) {
      if (c.word("into") === null) return { trigger: "transforms", what: subject };
      // The normalized name identifies this printed face, not an ObjectRef
      // that follows whichever face the permanent currently has (CR 712).
      if (!c.selfref()) return null;
      return { trigger: "transforms", what: subject, into: { face: "source-face" } };
    }
    if (c.attempt((c) => (c.words("becomes", "the", "target", "of") ? true : null))) {
      c.word("a", "an");
      const of = parseFilter(c);
      if (!of) return null;
      return { trigger: "becomes-target", what: subject, of };
    }
    if (c.attempt((c) => (c.words("is", "dealt", "damage") ? true : null)))
      return { trigger: "is-dealt-damage", what: subject };
    if (c.word("deals", "deal") !== null) {
      const combat = c.word("combat") !== null;
      if (c.word("damage") === null) return null;
      let to: "any" | PlayerRef | ObjectRef | undefined;
      if (c.word("to") !== null) {
        if (c.attempt((c) => (c.words("a", "player") ? true : null))) to = "any";
        else {
          const p = c.attempt(parsePlayerRef);
          if (p) to = p;
          else {
            const o = parseObjectRef(c);
            if (!o) return null;
            to = o;
          }
        }
      }
      return { trigger: "deals-damage", what: subject, combat: combat || undefined, to };
    }
    if (c.attempt((c) => (c.words("is", "tapped", "for", "mana") ? true : null)))
      return { trigger: "taps-for-mana", what: subject };

    return c.fail("trigger event");
  });
}

function parseQualifiedTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const first = parseSingleTrigger(c);
    if (!first) return null;
    let result = first;
    const occurrence = c.attempt(c => {
      if (!c.words("for", "the")) return null;
      const ordinal = parseOrdinal(c);
      if (!ordinal || c.word("time") === null || !c.words("each", "turn")) return null;
      return { ordinal, period: "turn" as const };
    });
    if (occurrence) {
      if (result.occurrence) return c.fail("single trigger event ordinal");
      // Counter history over a variable recipient set may be per recipient
      // or global. The current ordinal only models one fixed source's
      // matching occurrences (Axgard Artisan/U.S.S. Enterprise-D).
      if (result.trigger === "counter-change" && (result.on.recipient !== "object"
        || result.on.what.ref !== "self" && result.on.what.ref !== "this"))
        return c.fail("source-specific counter-event ordinal");
      result = { ...result, occurrence };
    }
    const during = c.attempt((c): TurnOwner | null => {
      if (c.word("during", "on") === null) return null;
      if (c.words("your", "turn")) return "your";
      if (c.word("each") !== null && c.possessive("opponent") !== null && c.word("turn") !== null) return "each-opponent";
      return null;
    });
    if (during) {
      if (result.during) return c.fail("single trigger turn restriction");
      result = { ...result, during };
    }
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
