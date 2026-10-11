// Battlefield-entry changes apply to the entering event itself (CR 614.1c–d,
// 614.12, 122.6, 303.4f–h, 508.4), never later counter/attachment instructions.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, BattlefieldEntryModifiers, Condition, Effect, ObjectFilter, ObjectRef, PlayerRef, StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { isPluralPlayerRef, parseCondition, parseCount, parseCounterSpec, parseEqualTo, parseObjectRef, parsePlayerRef, parseZoneRef } from "./refs.ts";
import { bindOtherExclusion } from "./other-exclusion.ts";
import { parseEligibleCardRef } from "./permissions.ts";
import { parseAggregateCount } from "./aggregates.ts";

type EntryDetails = Pick<Extract<Effect, { effect: "move-zone" }>, "tapped" | "controller" | "from" | "entry">;
type EntryCounter = NonNullable<BattlefieldEntryModifiers["counters"]>[number];

/** A source zone absorbed by the object selector is still an explicit source
 * declaration; a second trailing source must not override or conjoin it. */
export function hasExplicitSourceZone(ref: ObjectRef): boolean {
  if (ref.ref === "card-extraction-group") return ref.source?.kind === "zone"
    || ref.members.some(member => hasExplicitSourceZone({ ref: "filter", ...member }));
  const filterHasZone = (filter: ObjectFilter): boolean => Boolean(filter.zone ||
    filter.allOf?.some(filterHasZone) || filter.anyOf?.some(filterHasZone));
  if ("filter" in ref && filterHasZone(ref.filter)) return true;
  if (ref.ref === "library-cards" || ref.ref === "zone-cards") return true;
  return ref.ref === "from-pool" && hasExplicitSourceZone(ref.pool);
}

function counter(c: Cursor): EntryCounter | null {
  return c.attempt(c => {
    const count = parseCount(c);
    if (!count) return null;
    const additional = c.word("additional") !== null;
    const counter = parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    return { count, counter, ...(additional ? { additional: true } : {}) };
  });
}

function counters(c: Cursor): EntryCounter[] | null {
  return c.attempt(c => {
    if (c.word("with") === null) return null;
    const first = counter(c);
    if (!first) return null;
    const results = [first];
    while (true) {
      const next = c.attempt(c => {
        c.punct(",");
        if (c.word("and") === null) return null;
        return counter(c);
      });
      if (!next) break;
      results.push(next);
    }
    if (c.word("on") === null || c.word("it", "them") === null) return null;
    return results;
  });
}

function singular(ref: ObjectRef | PlayerRef): boolean {
  if ("player" in ref) return !isPluralPlayerRef(ref);
  if (["self", "this", "it", "enchanted", "equipped"].includes(ref.ref)) return true;
  if (ref.ref === "that") return !ref.plural;
  if (ref.ref === "related") return ref.number === "singular";
  return "count" in ref && ref.count?.amount === "fixed" && ref.count.value === 1 && !("upTo" in ref && ref.upTo);
}

/** Called immediately after a battlefield destination. Does not consume the
 * separator of a following instruction. Every modifier belongs to one move. */
export function parseBattlefieldEntryDetails(c: Cursor): EntryDetails | null {
  return c.attempt(c => {
    const details: EntryDetails = {};
    const entry: BattlefieldEntryModifiers = {};
    for (;;) {
      const joined = c.attempt(c => c.word("and") !== null && c.isWord("tapped", "attacking", "attached", "with") ? true : null);
      if (c.word("tapped") !== null) {
        if (details.tapped) return null;
        details.tapped = true;
      } else if (c.word("under") !== null) {
        if (details.controller) return null;
        if (c.word("your") !== null) details.controller = "you";
        else if (c.word("its", "their") !== null && c.possessive("owner", "owners") !== null) details.controller = "owner";
        else return null;
        if (c.word("control") === null) return null;
      } else if (c.word("from") !== null) {
        if (details.from) return null;
        const from = parseZoneRef(c);
        if (!from) return null;
        details.from = from;
      } else if (c.isWord("with")) {
        if (entry.counters) return null;
        const parsed = counters(c);
        if (!parsed) return null;
        entry.counters = parsed;
      } else if (c.word("attached") !== null) {
        if (entry.attachedTo || c.word("to") === null) return null;
        const to = parsePlayerRef(c) ?? parseObjectRef(c);
        // Plural sets require allocation per Aura/Equipment; they are not a
        // single common attachment destination (e.g. Storm Herald).
        if (!to || !singular(to)) return null;
        entry.attachedTo = to;
      } else if (c.word("attacking") !== null) {
        if (entry.attacking) return null;
        const defender = parsePlayerRef(c) ?? parseObjectRef(c);
        if (defender && !singular(defender)) return null;
        entry.attacking = defender ? { defender } : { choice: "entering-controller" };
      } else {
        if (joined) return null;
        break;
      }
    }
    return { ...details, ...(Object.keys(entry).length ? { entry } : {}) };
  });
}

/** CR 614.12a: pay/reveal choices happen before entering. The declined branch
 * changes the entry event, and cannot be represented as a resolving tap. */
export function parseEntryChoice(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    if (c.word("as") === null) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this") || c.word("enters") === null) return null;
    c.words("the", "battlefield");
    if (!c.punct(",") || !c.words("you", "may")) return null;
    const costs = parseCostList(c);
    if (!costs || !costs.every(cost => cost.cost === "pay-life" || cost.cost === "reveal") || !c.punct(".")) return null;
    if (!c.words("if", "you", "don't") || !c.punct(",")) return null;
    const repeated = parseObjectRef(c);
    if (!repeated || !(repeated.ref === "it" || JSON.stringify(repeated) === JSON.stringify(what))) return null;
    if (c.word("enters") === null) return null;
    c.words("the", "battlefield");
    if (c.word("tapped") === null) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "entry-choice", what, who: { player: "you" }, costs, declined: { tapped: true } };
  });
}

function multiplier(c: Cursor, subject: ObjectRef): Amount | null {
  return c.attempt(c => {
    const aggregate = c.attempt(c => c.word("each") !== null
      ? parseAggregateCount(c, { parseObjectRef }, { singular: true }) : null);
    const kicks = aggregate ? null : c.attempt((c): Amount | null => {
      if (!c.words("each", "time")) return null;
      const of = c.word("it", "he", "she") !== null ? subject : parseObjectRef(c);
      return of && c.words("was", "kicked") ? { amount: "kicker-count", of } : null;
    });
    const counterCount = aggregate || kicks ? null : c.attempt((c): Amount | null => {
      if (c.word("each") === null || c.isWord("different", "kind", "type", "color")) return null;
      const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null || c.word("on", "among") === null) return null;
      const on = parseObjectRef(c);
      return on ? { amount: "counter-count", counter, on } : null;
    });
    let result: Amount;
    if (aggregate || kicks || counterCount) result = (aggregate ?? kicks ?? counterCount)!;
    else {
      const ref = parseEligibleCardRef(c, { distributive: true });
      if (ref?.ref !== "each") return null;
      const filter = bindOtherExclusion(ref.filter, subject);
      if (!filter) return null;
      result = { amount: "count", of: filter };
    }
    if (c.attempt(c => c.word("and") !== null && c.isWord("each") ? true : null)) {
      const next = multiplier(c, subject);
      if (!next) return null;
      return { amount: "plus", a: result, b: next };
    }
    return result;
  });
}

/** Entry quantities are evaluated as the permanent would enter (614.12),
 * including originating-spell kicker history (400.7d, 702.33). */
export function parseStaticEntryCounters(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    const what = parseObjectRef(c);
    if (!what || c.word("enters", "enter") === null) return null;
    c.words("the", "battlefield");
    if (c.word("with") === null) return null;
    const dynamic = c.words("a", "number", "of");
    let count = dynamic ? null : parseCount(c);
    if (!dynamic && !count) return null;
    const additional = c.word("additional") !== null;
    const counter = parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    if (c.word("on") !== null && c.word("it", "him", "her", "them") === null) return null;
    if (dynamic) count = parseEqualTo(c);
    if (!count) return null;
    const repeated = c.word("for") !== null ? multiplier(c, what) : undefined;
    if (repeated === null) return null;
    const conditionKind = c.word("if", "unless") as "if" | "unless" | null;
    const condition: Condition | null = conditionKind ? parseCondition(c) : null;
    if (conditionKind && !condition) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return null;
    return { static: "enters-with-counters", what, counter, count,
      ...(additional ? { additional: true } : {}), ...(repeated ? { multiplier: repeated } : {}),
      ...(condition && conditionKind ? { condition, conditionKind } : {}) };
  });
}
