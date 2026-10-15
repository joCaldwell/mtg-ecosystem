// CR 614.9: redirection effects replace damage dealt to one battle, creature,
// planeswalker or player with the same damage dealt to another. They share
// the watched-damage grammar of prevention shields (615.7–615.9) but prevent
// nothing and never change the amount or the source.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { DamagePrevention, DamageRedirection, Duration, Effect, ObjectFilter, ObjectRef, PlayerRef, Sentence, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { damageHeader, parseDamageClause, pluralObject, type Match } from "./prevention.ts";
import { parseDuration, parseObjectRef, parsePlayerRef } from "./refs.ts";

interface Options { leadingDuration?: Duration }
type Watched = Pick<DamageRedirection, "by" | "to">;
const source: ObjectRef = { ref: "replacement-event-object", role: "source" };

function same(a: unknown, b: unknown): boolean { return JSON.stringify(a) === JSON.stringify(b); }

function matchesNoun(filter: ObjectFilter, ref: ObjectRef): boolean {
  if (Object.keys(filter).length !== 1) return false;
  if ("filter" in ref) {
    if (filter.cls) return ref.filter.cls === filter.cls;
    return Boolean(filter.types?.length && filter.types.every(type => ref.filter.types?.includes(type)));
  }
  if ("noun" in ref) return filter.cls === ref.noun || filter.types?.length === 1 && filter.types[0] === ref.noun;
  return false;
}

/** Each occurrence of the watched damage has one source and one recipient;
 * only a singular description can be the antecedent of "it" / "that …". */
function participants(watched: Watched): { ref: ObjectRef; role: "source" | "recipient" }[] {
  const result: { ref: ObjectRef; role: "source" | "recipient" }[] = [];
  if (watched.by && "ref" in watched.by && !pluralObject(watched.by)) result.push({ ref: watched.by, role: "source" });
  for (const ref of watched.to ?? []) if ("ref" in ref && !pluralObject(ref)) result.push({ ref, role: "recipient" });
  return result;
}

/** The subject of the active form ("it deals that damage to …") is the
 * source of the watched damage, never a newly selected object. */
function subjectIsSource(subject: ObjectRef, watched: Watched): boolean {
  const by = watched.by;
  if (!by) return false;
  if (subject.ref === "that" && !subject.plural && subject.filter.cls === "source" && Object.keys(subject.filter).length === 1) return true;
  if (!("ref" in by) || pluralObject(by)) return false;
  if (subject.ref === "it") return true;
  if (subject.ref === "that") return !subject.plural && matchesNoun(subject.filter, by);
  return same(subject, by);
}

/** Pronouns for a participant of the watched damage become event roles.
 * In the active form, "it" / "itself" continue the subject (the source).
 * A definite description matching no singular participant keeps its
 * symbolic outer antecedent; an ambiguous one is rejected. */
function bindRecipient(ref: ObjectRef | PlayerRef, watched: Watched, active: boolean): ObjectRef | PlayerRef | null {
  const object = (ref: ObjectRef): ObjectRef | null => {
    if (ref.ref === "them") return null;
    if (ref.ref === "it") {
      if (active) return source;
      const candidates = participants(watched);
      return candidates.length === 1 ? { ref: "replacement-event-object", role: candidates[0].role } : null;
    }
    if (ref.ref === "that") {
      if (ref.plural) return null;
      if (ref.filter.cls === "source" && Object.keys(ref.filter).length === 1) return watched.by ? source : null;
      const matching = participants(watched).filter(candidate => matchesNoun(ref.filter, candidate.ref));
      if (matching.length > 1) return null;
      return matching.length ? { ref: "replacement-event-object", role: matching[0].role } : ref;
    }
    return ref;
  };
  if ("ref" in ref) return object(ref);
  if (ref.player === "controller" || ref.player === "owner") {
    const of = object(ref.of);
    return of && { ...ref, of };
  }
  if (ref.player === "that-player" || ref.player === "the-player" || ref.player === "they") return null;
  return ref;
}

function parseNewRecipient(c: Cursor, watched: Watched, active: boolean): ObjectRef | PlayerRef | null {
  return c.attempt(c => {
    // "That source" needs the source the header introduced.
    const controller = c.attempt((c): PlayerRef | null =>
      c.word("that", "the") !== null && c.possessive("source") !== null && c.word("controller") !== null && watched.by
        ? { player: "controller", of: source } : null);
    if (controller) return controller;
    const ref: ObjectRef | PlayerRef | null = parsePlayerRef(c) ?? parseObjectRef(c);
    return ref && bindRecipient(ref, watched, active);
  });
}

function watchedDamage(match: Pick<DamagePrevention, "eventScope" | "kind" | "by" | "to">): Pick<DamageRedirection, "eventScope" | "kind" | "by" | "to"> {
  return { eventScope: match.eventScope, kind: match.kind, ...(match.by && { by: match.by }), ...(match.to && { to: match.to }) };
}

function parseCore(c: Cursor, options: Options): { redirection: DamageRedirection; duration?: Duration } | null {
  return c.attempt(c => {
    let duration = options.leadingDuration;
    if (!duration) duration = c.attempt(c => { const d = parseDuration(c); return d && c.punct(",") ? d : null; }) ?? undefined;
    const occurrence = c.words("the", "next", "time") ? "next" : c.word("if") !== null ? "each" : null;
    let redirection: DamageRedirection;
    if (occurrence) {
      let header: Match | null = null;
      for (let i = c.pos; i < c.tokens.length; i++) {
        const token = c.tokens[i];
        if (token.kind === "punct" && token.value === ".") break;
        if (token.kind !== "punct" || token.value !== ",") continue;
        const probe = new Cursor(c.tokens.slice(c.pos, i)), match = damageHeader(probe);
        if (match && probe.done()) { header = match; c.pos = i + 1; break; }
      }
      // A "while" condition on the watched event has no redirection wording.
      if (!header || header.condition || header.incomingAmount || duration && header.duration
        || occurrence === "each" && header.unmarkedPluralRecipients) return null;
      duration ??= header.duration;
      const watched = watchedDamage(header);
      if (occurrence === "next") watched.eventScope = { ...watched.eventScope, recipients: "simultaneous" };
      let optional: DamageRedirection["optional"];
      let active = false;
      if (!c.words("that", "damage", "is", "dealt", "to")) {
        const chooser = c.attempt(c => {
          const who = parsePlayerRef(c);
          return who && c.words("may", "have", "that", "damage", "dealt", "to") ? who : null;
        });
        if (chooser) optional = { who: chooser };
        else {
          const subject = parseObjectRef(c);
          if (!subject || !subjectIsSource(subject, watched) || !c.words("deals", "that", "damage", "to")) return null;
          active = true;
        }
      }
      const redirectTo = parseNewRecipient(c, watched, active);
      if (!redirectTo) return null;
      redirection = { ...watched, shield: { shield: "events", occurrence }, redirectTo, ...(optional && { optional }) };
    } else {
      const clause = parseDamageClause(c);
      if (!clause || duration && clause.duration || !c.words("is", "dealt", "to")) return null;
      duration ??= clause.duration;
      const { prevention } = clause;
      // Separate per-recipient budgets and "dealt to and dealt by" have no
      // printed redirection; reject rather than invent their meaning.
      if (prevention.involving || prevention.shield.shield === "damage-budget" && prevention.shield.allocation) return null;
      const watched = watchedDamage(prevention);
      const redirectTo = parseNewRecipient(c, watched, false);
      if (!redirectTo) return null;
      redirection = { ...watched, shield: prevention.shield.shield === "damage-budget"
        ? { shield: "damage-budget", amount: prevention.shield.amount }
        : { shield: "events", occurrence: prevention.shield.occurrence }, redirectTo };
    }
    // Damage is never redirected to the recipient it already has.
    if (redirection.to?.some(ref => same(ref, redirection.redirectTo))) return null;
    if (c.word("instead") === null) return null;
    if (!c.punct(".") && !c.done()) return null;
    return { redirection, ...(duration && { duration }) };
  });
}

/** Owns the whole sentence, before ordinary `if` parsing. A resolving effect
 * that redirects every matching event must print how long it lasts. */
export function parseRedirectionSentence(c: Cursor, options: Options = {}): Sentence | null {
  return c.attempt(c => {
    const parsed = parseCore(c, options);
    if (!parsed) return null;
    const { redirection, duration } = parsed;
    if (!duration && redirection.shield.shield === "events" && redirection.shield.occurrence === "each") return null;
    const effect: Effect = { effect: "redirect-damage", redirection, ...(duration && { duration }) };
    return { steps: [effect] };
  });
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasTarget);
  const object = value as Record<string, unknown>;
  if (object.ref === "target" || object.ref === "any-target" || object.player === "target-player" || object.player === "target-opponent") return true;
  return Object.values(object).some(hasTarget);
}

/** A static ability redirects every matching event while it functions; it
 * has no budget, no single "next" event, no target and no chosen source. */
export function parseStaticRedirection(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    const parsed = parseCore(c, {});
    if (!parsed || !c.done()) return null;
    const { redirection, duration } = parsed;
    if (duration && duration.duration !== "as-long-as" || redirection.shield.shield !== "events" || redirection.shield.occurrence !== "each"
      || hasTarget(redirection) || redirection.by && "choice" in redirection.by || !redirection.by && !redirection.to) return null;
    const effect: StaticEffect = { static: "redirect-damage", redirection };
    return duration?.duration === "as-long-as" ? { static: "conditional", condition: duration.condition, effect } : effect;
  });
}
