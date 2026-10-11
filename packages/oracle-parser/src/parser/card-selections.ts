import type { CardExtractionSlot, ObjectFilter, ObjectRef } from "../ast.ts";
import { wordNumber } from "../vocab.ts";
import { Cursor } from "./cursor.ts";

type Group = Extract<ObjectRef, { ref: "card-extraction-group" }>;
interface Grammar { parseSelection(c: Cursor): ObjectRef | null }

function slot(ref: ObjectRef | null): CardExtractionSlot | null {
  if (ref?.ref !== "filter" || ref.filter.cls !== "card") return null;
  const { ref: _ref, ...selection } = ref;
  return selection;
}

function startsSlot(c: Cursor): boolean {
  const t = c.peek();
  return t?.kind === "number" || t?.kind === "word" && !t.possessive
    && (["a", "an", "another", "x", "up"].includes(t.value) || wordNumber(t.value) !== undefined);
}

function separator(c: Cursor): { combine?: Group["combine"] } | null {
  return c.attempt(c => {
    const comma = c.punct(",");
    let combine: Group["combine"] | undefined;
    if (c.word("and") !== null) {
      if (c.punct("/")) {
        if (c.word("or") === null) return null;
        combine = "any-nonempty";
      } else combine = "all";
    }
    if ((!comma && !combine) || !startsSlot(c)) return null;
    return { ...(combine ? { combine } : {}) };
  });
}

function nestedSource(filter: ObjectFilter): boolean {
  return [...(filter.anyOf ?? []), ...(filter.allOf ?? [])].some(f => !!f.zone || !!f.fromAmong || nestedSource(f));
}

/** Once separate counted card slots are recognized, malformed or ambiguous
 * coordination must fail as a whole instead of falling back to an unrelated
 * ordinary effect-list parse. */
export function parseCardExtraction(c: Cursor, g: Grammar): ObjectRef | null {
  const grouped = parseCardExtractionGroup(c, g);
  if (grouped) return grouped;
  for (let end = c.pos; end < c.tokens.length; end++) {
    const token = c.tokens[end];
    if (token?.kind === "punct" && [".", ";", ":", '"'].includes(token.value)) break;
    const tail = new Cursor(c.tokens.slice(end));
    if (!separator(tail)) continue;
    const head = new Cursor(c.tokens.slice(c.pos, end));
    if (slot(g.parseSelection(head)) && head.done()) return null;
  }
  return g.parseSelection(c);
}

function hasAmbiguousTrailingModifier(filter: ObjectFilter): boolean {
  const nominal = new Set([
    "cls", "types", "subtypes", "supertypes", "nonTypes", "nonSubtypes", "nonSupertypes",
    "colors", "nonColors", "colorless", "multicolored", "monocolored", "nonToken", "status",
    "commander", "permanent", "other", "name", "zone", "fromAmong", "anyOf", "allOf",
  ]);
  return Object.keys(filter).some(key => !nominal.has(key))
    || [...(filter.anyOf ?? []), ...(filter.allOf ?? [])].some(hasAmbiguousTrailingModifier);
}

/** Card extraction uses separate allocations. Kaalia's ruling explicitly
 * assigns a multitype card to one slot; Tragic Arrogance's reusable permanent
 * representatives are a different operation and are not parsed here.
 * https://magic.wizards.com/en/news/feature/core-set-2020-release-notes-2019-06-25
 * https://magic.wizards.com/en/news/feature/magic-origins-release-notes-2015-07-08
 * CR608.2c/d, 701.23b (hidden-zone fail-to-find does not change printed counts):
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseCardExtractionGroup(c: Cursor, g: Grammar): Group | null {
  return c.attempt(c => {
    if (!startsSlot(c)) return null;
    const members: CardExtractionSlot[] = [];
    let combine: Group["combine"] | undefined;
    for (;;) {
      const start = c.pos;
      let next: { member: CardExtractionSlot; end: number; combine?: Group["combine"] } | undefined;
      for (let end = start; end < c.tokens.length; end++) {
        const token = c.tokens[end];
        if (token?.kind === "punct" && [".", ";", ":", '"'].includes(token.value)) break;
        const tail = new Cursor(c.tokens.slice(end));
        const join = separator(tail);
        if (!join) continue;
        const head = new Cursor(c.tokens.slice(start, end));
        const member = slot(g.parseSelection(head));
        if (!member || !head.done()) continue;
        next = { member, end: end + tail.pos, ...join };
        break;
      }
      if (!next) break;
      if (combine && !next.combine) return null; // no dangling comma-only tail after a completed conjunction
      if (next.combine) {
        if (combine && combine !== next.combine) return null;
        combine = next.combine;
      }
      members.push(next.member);
      c.pos = next.end;
    }
    if (!members.length || !combine) return null;
    const last = slot(g.parseSelection(c));
    if (!last || last.groupConstraints?.length || hasAmbiguousTrailingModifier(last.filter)) return null;
    members.push(last);
    if (members.some(member => nestedSource(member.filter) || member.filter.zone && member.filter.fromAmong)) return null;
    const sources = members.map(member => !!member.filter.zone || !!member.filter.fromAmong);
    let source: Group["source"];
    if (sources.at(-1) && sources.slice(0, -1).every(present => !present)) {
      // The one trailing source scopes the full grammatical coordinated NP.
      const { zone, fromAmong, ...filter } = last.filter;
      source = zone ? { kind: "zone", zone } : { kind: "pool", pool: fromAmong! };
      last.filter = filter;
    } else if (sources.some(Boolean) && !sources.every(Boolean)) {
      return null; // neither erase branch sources nor invent a shared source
    }
    return { ref: "card-extraction-group", members, combine, allocation: "distinct-objects", ...(source ? { source } : {}) };
  });
}
