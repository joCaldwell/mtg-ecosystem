// "If … this way" outcome tests. They ask what an earlier instruction of the
// same spell or ability actually did, after replacement effects and
// impossibility (CR 608.2c, 609.3, 614.6), never what it was told to do and
// never the current game state (608.2h–i). An actor restricts the result to
// what that player did ("if you sacrificed a creature this way" after "each
// player sacrifices").
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, ObjectFilter, ObjectRef, ObjectRelation, PlayerFilter, PlayerRef, ZoneRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";
import { wordNumber } from "../vocab.ts";
import { parseZoneChangeRelation } from "./zone-history.ts";

type ResultEvent = Extract<ObjectRelation, { relation: "event-result" }>["event"];

const PARTICIPLES: Record<string, ResultEvent> = {
  exiled: "exile", sacrificed: "sacrifice", discarded: "discard", revealed: "reveal", destroyed: "destroy",
  tapped: "tap", returned: "return", countered: "counter", milled: "mill", drawn: "draw",
};
/** Finite verbs of the acting player: [event, base, third person, past]. */
const ACTIVE: [ResultEvent, string, string, string][] = [
  ["exile", "exile", "exiles", "exiled"], ["sacrifice", "sacrifice", "sacrifices", "sacrificed"],
  ["discard", "discard", "discards", "discarded"], ["reveal", "reveal", "reveals", "revealed"],
  ["destroy", "destroy", "destroys", "destroyed"], ["tap", "tap", "taps", "tapped"],
  ["return", "return", "returns", "returned"], ["counter", "counter", "counters", "countered"],
  ["mill", "mill", "mills", "milled"], ["draw", "draw", "draws", "drew"],
];

const atLeastOne: Comparison = { op: "ge", value: { amount: "fixed", value: 1 } };

function hasRelation(filter: ObjectFilter): boolean {
  return !!filter.relation || !!filter.damageHistory || !!filter.allOf?.some(hasRelation) || !!filter.anyOf?.some(hasRelation);
}

/** "A creature card" is one or more; a bare number is that many or more, as
 * in the other history counts; "no" and explicit bounds are kept exactly. */
type SubjectGrammar = Pick<ConditionGrammar, "parseComparison" | "parseFilter" | "parseObjectRef">;

function parseQuantifier(c: Cursor, g: SubjectGrammar, barePlural: boolean): Comparison | null {
  return c.attempt((c): Comparison | null => {
    if (c.word("a", "an") !== null) return atLeastOne;
    if (c.word("no") !== null) return { op: "eq", value: { amount: "fixed", value: 0 } };
    const first = c.peek();
    const numeric = first?.kind === "number" || (first?.kind === "word" && !first.possessive &&
      (wordNumber(first.value) !== undefined || ["exactly", "at", "fewer", "more", "less"].includes(first.value)));
    if (!numeric) return barePlural ? atLeastOne : null;
    const exact = c.isWord("exactly");
    const comparison = g.parseComparison(c);
    if (!comparison) return null;
    return comparison.op === "eq" && !exact ? { op: "ge", value: comparison.value } : comparison;
  });
}

/** "a permanent's ability": an ability whose source matches the plain
 * description; a stack object of any other source (a spell, a card in a
 * graveyard) does not qualify (CR 113.7, 113.8). */
function parsePossessedAbility(c: Cursor, g: SubjectGrammar): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("a", "an") === null) return null;
    let offset = 0;
    while (c.peek(offset)?.kind === "word" && !(c.peek(offset) as { possessive?: boolean }).possessive) offset++;
    const head = c.peek(offset);
    if (head?.kind !== "word" || !head.possessive) return null;
    const phrase = new Cursor([...c.tokens.slice(c.pos, c.pos + offset), { ...head, possessive: false }]);
    const source = g.parseFilter(phrase);
    if (!source || !phrase.done() || hasRelation(source) || source.fromAmong) return null;
    c.pos += offset + 1;
    if (c.word("ability") === null) return null;
    return { cls: "ability", abilitySource: { ref: "filter", filter: source, count: { amount: "fixed", value: 1 } } };
  });
}

type Subject = { match: "object"; what: ObjectRef } | { match: "filter"; filter: ObjectFilter; comparison: Comparison };

function parseSubject(c: Cursor, g: SubjectGrammar, barePlural = false): Subject | null {
  return c.attempt((c): Subject | null => {
    const object = c.attempt(c => {
      if (!c.isWord("this", "that", "it") && c.peek()?.kind !== "selfref") return null;
      const what = g.parseObjectRef(c);
      return what && (what.ref === "self" || what.ref === "this" || what.ref === "it" || (what.ref === "that" && !what.plural)) ? what : null;
    });
    if (object) return { match: "object", what: object };
    const ability = parsePossessedAbility(c, g);
    if (ability) return { match: "filter", filter: ability, comparison: atLeastOne };
    const comparison = parseQuantifier(c, g, barePlural);
    const filter = comparison && g.parseFilter(c);
    // A second event link or pool on the counted objects needs its own scope.
    if (!filter || !comparison || hasRelation(filter) || filter.fromAmong) return null;
    return { match: "filter", filter, comparison };
  });
}

function outcome(subject: Subject, link: Pick<ObjectFilter, "relation" | "damageHistory">): Condition {
  return subject.match === "object"
    ? { condition: "is-filter", what: subject.what, filter: link }
    : { condition: "count", of: { ...subject.filter, ...link }, comparison: subject.comparison };
}

/** "[subject] is exiled / was destroyed / dies / is dealt damage this way". */
function parsePassiveOutcome(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const subject = parseSubject(c, g);
    if (!subject) return null;
    // "a creature you controlled was destroyed this way": control as the
    // event happened to it, which the event-linked filter already describes.
    const pastControl = subject.match === "filter" && !subject.filter.control && !subject.filter.controlledBy ? c.attempt(c => {
      const who = g.parsePlayerRef(c);
      return who && c.word("controlled") !== null && c.isWord("was", "were") ? who : null;
    }) : null;
    if (pastControl && subject.match === "filter")
      subject.filter = { ...subject.filter, ...(pastControl.player === "you" ? { control: "you" as const } : { controlledBy: pastControl }) };
    let link: Pick<ObjectFilter, "relation" | "damageHistory">;
    if (c.word("dies", "die", "died") !== null) link = { relation: { relation: "event-result", event: "die", scope: "this-way" } };
    else {
      if (c.word("is", "was", "are", "were") === null) return null;
      if (c.isWord("put")) {
        // "is put into a graveyard / onto the battlefield this way": the move
        // the earlier instruction actually made (CR 400.7, 608.2c, 614.6).
        const relation = parseZoneChangeRelation(c, g.parseZoneRef);
        return relation && relation.scope === "this-way" && !relation.period ? outcome(subject, { relation }) : null;
      }
      if (c.word("dealt") !== null) {
        // CR 120.3: damage actually dealt to the object, not damage marked.
        const damage = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
        if (c.word("damage") === null) return null;
        link = { damageHistory: { role: "recipient", window: { event: "this-way" }, ...(damage ? { damage } : {}) } };
      } else {
        const participle = c.anyWord();
        const event = participle && PARTICIPLES[participle.value];
        if (!event) return null;
        link = { relation: { relation: "event-result", event, scope: "this-way" } };
      }
    }
    return c.words("this", "way") ? outcome(subject, link) : null;
  });
}

/** "you sacrificed a creature / that player discards an artifact card / you
 * didn't draw cards this way": the stated player performed the event. */
function parseActiveOutcome(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    // A present-tense action in an unless clause is an offered payment.
    if (g.purpose === "unless") return null;
    const who: PlayerRef | null = g.parsePlayerRef(c);
    if (!who) return null;
    const negate = c.word("didn't", "don't", "doesn't") !== null;
    const verb = c.anyWord();
    const entry = verb && ACTIVE.find(([, base, third, past]) => negate ? verb.value === base : [base, third, past].includes(verb.value));
    if (!entry) return null;
    const subject = parseSubject(c, g, true);
    if (!subject || subject.match !== "filter" || !c.words("this", "way")) return null;
    const result = outcome(subject, { relation: { relation: "event-result", event: entry[0], scope: "this-way", who } });
    return negate ? { condition: "not", of: result } : result;
  });
}

/** "excess damage was dealt [to that creature] this way": the earlier damage
 * event exceeded lethal damage, loyalty or defense (CR 120.4a, 120.10). */
function parseExcessDamageOutcome(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("excess", "damage") || c.word("was", "is") === null || c.word("dealt") === null) return null;
    let to: ObjectRef | undefined;
    if (c.word("to") !== null) {
      const what = g.parseObjectRef(c);
      if (!what || !["self", "this", "it", "that"].includes(what.ref) || (what.ref === "that" && what.plural)) return null;
      to = what;
    }
    if (!c.words("this", "way")) return null;
    return { condition: "compare", value: { amount: "event-quantity", event: "damage", measure: "excess", ...(to ? { to } : {}), scope: "this-way" }, comparison: atLeastOne };
  });
}

export function parseEventOutcome(c: Cursor, g: ConditionGrammar): Condition | null {
  return g.outcomes ? parsePassiveOutcome(c, g) ?? parseActiveOutcome(c, g) ?? parseExcessDamageOutcome(c, g) : null;
}

/** "who discarded a card this way" / "who didn't discard a nonland card this
 * way" after a quantified player: each candidate is tested on what that
 * player did. Only the negation of "a/an" is a count of zero; a negated
 * number ("who didn't discard two cards") has no single reading here. */
export function parseEventResultPlayerQualifier(c: Cursor, g: SubjectGrammar): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    if (c.word("who") === null) return null;
    const negate = c.word("didn't", "don't", "doesn't") !== null;
    const verb = c.anyWord();
    const entry = verb && ACTIVE.find(([, base, third, past]) => negate ? verb.value === base : [base, third, past].includes(verb.value));
    if (!entry) return null;
    const article = c.isWord("a", "an");
    const subject = parseSubject(c, g, true);
    if (!subject || subject.match !== "filter" || !c.words("this", "way") || (negate && !article)) return null;
    const comparison: Comparison = negate ? { op: "eq", value: { amount: "fixed", value: 0 } } : subject.comparison;
    return { eventResults: { of: { ...subject.filter, relation: { relation: "event-result", event: entry[0], scope: "this-way" } }, comparison } };
  });
}

/** "When you discard a nonland card this way," / "When you sacrifice one or
 * more Zombies this way,": the event of a reflexive triggered ability (CR
 * 603.12). Called after "when"; a negated outcome is not an event. */
export function parseReflexiveOutcome(c: Cursor, g: SubjectGrammar & Pick<ConditionGrammar, "parsePlayerRef">): { of: ObjectFilter; comparison: Comparison } | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    const verb = who && c.anyWord();
    const entry = verb && ACTIVE.find(([, base, third]) => verb.value === base || verb.value === third);
    if (!who || !entry) return null;
    const subject = parseSubject(c, g);
    if (!subject || subject.match !== "filter" || !c.words("this", "way")) return null;
    return { of: { ...subject.filter, relation: { relation: "event-result", event: entry[0], scope: "this-way", who } }, comparison: subject.comparison };
  });
}

/** "who searched their library this way" after a quantified player: whether
 * each candidate searched (CR 701.23a), not whether they found a card. */
export function parseSearchedPlayerQualifier(c: Cursor, parseZoneRef: (c: Cursor) => ZoneRef | null): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    if (c.word("who") === null || c.word("searched") === null) return null;
    const zone = parseZoneRef(c);
    if (zone?.zone !== "library" || zone.owner !== "their" || zone.position || !c.words("this", "way")) return null;
    return { searched: { zone, scope: "this-way" } };
  });
}
