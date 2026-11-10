// "all creatures except for <those>": a quantified set minus the objects a
// printed exception describes. CR 608.2c reads the instruction as written;
// the exception narrows the set and is never a second instruction.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** One exception as the existing negative field with the same meaning; a
 * compound description ("tokens you control") has none and stays whole. */
function exclude(filter: ObjectFilter, item: ObjectFilter): void {
  const keys = Object.keys(item);
  if (keys.length === 1 && item.types?.length === 1) (filter.nonTypes ??= []).push(item.types[0]);
  else if (keys.length === 1 && item.subtypes?.length === 1) (filter.nonSubtypes ??= []).push(item.subtypes[0]);
  else if (keys.length === 1 && item.cls === "token") filter.nonToken = true;
  else (filter.noneOf ??= []).push(item);
}

/** The members of a plain noun union ("artifacts, lands, and Phyrexians"). */
function unionMembers(item: ObjectFilter): ObjectFilter[] {
  const keys = Object.keys(item);
  const inner = keys.length === 1 && item.allOf?.length === 1 ? item.allOf[0] : item;
  return Object.keys(inner).length === 1 && inner.anyOf ? inner.anyOf : [item];
}

/** "<quantified filter> except for ~" / "except for A, B, and C". Every
 * listed description is excluded: the result matches `filter` and none of
 * them. Returns null (cursor restored) when no complete exception follows. */
export function parseExceptFor(c: Cursor, filter: ObjectFilter, parseFilter: (c: Cursor) => ObjectFilter | null): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (!c.words("except", "for")) return null;
    const result = structuredClone(filter);
    if (c.selfref()) {
      if (result.other || result.otherThan) return c.fail("a single exclusion of the source");
      result.otherThan = { ref: "self" };
      return result;
    }
    const first = parseFilter(c);
    if (!first) return null;
    const items = unionMembers(first);
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        if (c.word("and") === null && !comma) return null;
        return parseFilter(c);
      });
      if (!next) break;
      items.push(...unionMembers(next));
    }
    for (const item of items) exclude(result, item);
    return result;
  });
}

const PLAIN_KEYS = new Set(["types", "subtypes", "nonTypes", "nonSubtypes", "supertypes", "nonSupertypes", "colors", "nonColors", "cls"]);

/** Postfix "other than <description>[, <description>…]" on a plural or
 * indefinite selection ("cards other than basic land cards"): a qualifier
 * excluding every listed description, exactly like "except for". Identity
 * references ("other than ~") are the caller's `otherThan`, not this. */
export function parseOtherThanDescriptions(c: Cursor, parseFilter: (c: Cursor) => ObjectFilter | null): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (!c.words("other", "than")) return null;
    const article = () => c.word("a", "an");
    article();
    const first = parseFilter(c);
    if (!first) return null;
    const items = unionMembers(first);
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        if (c.word("and") === null && !comma) return null;
        article();
        return parseFilter(c);
      });
      if (!next) break;
      items.push(...unionMembers(next));
    }
    // A trailing "from <zone>" phrase ("…other than a basic land card from
    // it") is read once, after the last description, and qualifies the whole
    // selection: the excluded kind never carries a position of its own.
    const result: ObjectFilter = {};
    const last = items.at(-1)!;
    for (const key of ["zone", "fromAmong"] as const) {
      if (last[key] === undefined) continue;
      if (items.some(item => item !== last && item[key] !== undefined)) return c.fail("one position for the whole selection");
      (result as Record<string, unknown>)[key] = last[key];
      delete last[key];
    }
    if (!items.every(item => Object.keys(item).every(key => PLAIN_KEYS.has(key)))) return c.fail("excluded kind without a scope of its own");
    for (const item of items) exclude(result, item);
    return result;
  });
}
