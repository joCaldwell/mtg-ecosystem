// Where a card lies in its graveyard: "this card is in your graveyard with a
// creature card directly above it", "... with three or more creature cards
// above it". CR 404.2: cards are put on top of a graveyard and its order is
// kept, so what is above a card is determined, never chosen.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, ObjectFilter } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import type { Cursor } from "./cursor.ts";

/** A plain card description: card types, subtypes, colors only. */
function cardDescription(filter: ObjectFilter | null): ObjectFilter | null {
  if (!filter || filter.cls !== "card") return null;
  const { cls: _cls, ...rest } = filter;
  return Object.keys(rest).every(key => ["types", "nonTypes", "subtypes", "colors"].includes(key)) ? rest : null;
}

export function parseGraveyardPosition(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const what = g.parseObjectRef(c);
    if (!what || !["self", "this", "it"].includes(what.ref)) return null;
    if (c.word("is") === null || c.word("in") === null) return null;
    const graveyard = g.parseZoneRef(c);
    // One identified graveyard; "a graveyard" has no single order to consult.
    if (!graveyard || graveyard.zone !== "graveyard" || graveyard.position || graveyard.owner !== "your") return null;
    if (c.word("with") === null) return null;
    let comparison: Comparison | null = null;
    if (c.word("a", "an") === null) {
      comparison = g.parseComparison(c);
      if (!comparison) return null;
    }
    // The card description ends at "directly above" / "above".
    let end = c.pos;
    while (c.tokens[end]?.kind === "word" && !["directly", "above"].includes((c.tokens[end] as { value: string }).value)) end++;
    const noun = c.tokens[end - 1];
    if (end === c.pos || noun?.kind !== "word") return null;
    const plural = noun.value === "cards";
    if (plural === !comparison) return null;
    const filter = g.parseFilter(c);
    const described = cardDescription(filter);
    if (!described || c.pos !== end) return null;
    const directly = c.word("directly") !== null;
    if (c.word("above") === null || c.word("it") === null) return null;
    // "directly above" is the one adjacent card; a count looks at all above it.
    if (directly === !!comparison) return null;
    return { condition: "graveyard-position", what, graveyard,
      above: directly ? { match: "directly", filter: described } : { match: "count", filter: described, comparison: comparison! } };
  });
}
