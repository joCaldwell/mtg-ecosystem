// "each A and B <qualifier>" has one determiner and one trailing qualifier
// for both nouns ("each creature and planeswalker your opponents control").
// Reading it as "each A" plus an independent "B <qualifier>" silently drops
// the qualifier from A. Until the filter grammar builds that union under a
// singular "each", a coordinated list must reject the split (CR 608.2c:
// the complete instruction is read before applying its parts).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ObjectRef, PlayerRef } from "../ast.ts";
import type { Token } from "../lexer.ts";

/** Fields a prenominal head can state by itself; everything else is a
 * trailing qualifier whose scope the conjunction leaves undetermined. */
const HEAD_FIELDS = new Set<keyof ObjectFilter>([
  "types", "nonTypes", "cls", "subtypes", "nonSubtypes", "supertypes", "nonSupertypes", "colors", "nonColors",
  "colorless", "multicolored", "monocolored", "nonToken", "permanent", "commander", "status", "other",
]);

export function splitsSharedScope(previous: ObjectRef | PlayerRef, next: ObjectRef | PlayerRef, nextStart: Token | undefined): boolean {
  if (!("ref" in previous) || !("ref" in next) || previous.ref !== "each" || next.ref !== "each") return false;
  // An explicit second determiner begins its own noun phrase.
  if (nextStart?.kind === "word" && ["each", "all"].includes(nextStart.value)) return false;
  const before = previous.filter, after = next.filter;
  return (Object.keys(after) as (keyof ObjectFilter)[]).some(key =>
    !HEAD_FIELDS.has(key) && JSON.stringify(after[key]) !== JSON.stringify(before[key]));
}
