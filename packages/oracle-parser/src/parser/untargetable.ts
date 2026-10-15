// "Can't be the target(s) of [spells and/or abilities]": which spells and
// abilities can't choose the object as a target (CR 115.1, 601.2c, 602.2b).
// Hexproof and shroud are defined in these words (702.11b-d, 702.18a), but
// the printed restriction is not that keyword.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CantAction, ObjectFilter } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

/** A qualifier printed before "spells or abilities" belongs to the spells
 * only when the abilities have their own ("black spells or abilities from
 * black sources"); one that follows both ("spells or abilities your
 * opponents control") qualifies both. Only these readings are accepted. */
function isWholeSourceFilter(filter: ObjectFilter): boolean {
  if (filter.cls === "spell" || filter.cls === "ability") return !filter.anyOf;
  const { allOf, control, ...rest } = filter;
  const branches = allOf?.length === 1 && Object.keys(allOf[0]).length === 1 ? allOf[0].anyOf : undefined;
  return Object.keys(rest).length === 0 && (control === undefined || control === "opponent") && !!branches && branches.length === 2 &&
    JSON.stringify(branches) === JSON.stringify([{ cls: "spell" }, { cls: "ability" }]);
}

function atBoundary(c: Cursor): boolean {
  const next = c.peek();
  return !next || next.kind === "punct" && next.value === "." ||
    next.kind === "word" && !next.possessive && ["and", "unless", "this", "until", "as"].includes(next.value);
}

function parseSources(c: Cursor): ObjectFilter | null {
  const whole = c.attempt(c => {
    const filter = parseFilter(c);
    return filter && isWholeSourceFilter(filter) && atBoundary(c) ? filter : null;
  });
  if (whole) return whole;
  // "<spells> or <abilities>", each with its own qualifiers.
  return c.attempt((c): ObjectFilter | null => {
    const split = c.tokens.slice(c.pos).findIndex((t, i, all) => t.kind === "word" && t.value === "or" &&
      all[i + 1]?.kind === "word" && (all[i + 1] as { value: string }).value === "abilities");
    if (split < 1) return null;
    const left = new Cursor(c.tokens.slice(c.pos, c.pos + split));
    const spells = parseFilter(left);
    if (!spells || spells.cls !== "spell" || spells.anyOf || !left.done()) return null;
    c.pos += split + 1;
    const abilities = parseFilter(c);
    // Only the printed "abilities from [quality] sources" counterpart of a
    // qualified spell (cf. 702.11d) has an unambiguous scope.
    if (!abilities || abilities.cls !== "ability" || !abilities.abilitySource || abilities.anyOf || !atBoundary(c)) return null;
    return { anyOf: [spells, abilities] };
  });
}

/** After "can't": "be the target of …" / "be the targets of …". */
export function parseUntargetable(c: Cursor): CantAction | null {
  return c.attempt((c): CantAction | null => {
    if (!c.words("be", "the") || c.word("target", "targets") === null || c.word("of") === null) return null;
    const by = parseSources(c);
    return by && { action: "be-targeted", by };
  });
}
