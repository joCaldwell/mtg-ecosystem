// Comma lists of independently headed descriptions: "spell, activated
// ability, or triggered ability", "enchantment, tapped artifact, or tapped
// creature". Each member keeps its own prenominal and postnominal qualifiers;
// the selected object matches any one member (CR 608.2c). A list must be
// closed by "or" / "and/or"; members never carry their own article, which
// would introduce a separate selection slot instead.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type ScopeKey = "zone" | "control" | "controlledBy" | "own" | "cast" | "fromAmong" | "relation" | "attachedTo" | "blocking" | "blockedBy" | "abilitySource";
const SCOPE_KEYS: ScopeKey[] = ["zone", "control", "controlledBy", "own", "cast", "fromAmong", "relation", "attachedTo", "blocking", "blockedBy", "abilitySource"];

export interface FilterListGrammar {
  /** One member, without consuming a following list continuation. */
  item: (c: Cursor) => ObjectFilter | null;
  scoped: (filter: ObjectFilter, key: ScopeKey) => boolean;
  /** The last member's selection pool, shared by the whole list. */
  hoist: (first: ObjectFilter, members: ObjectFilter[]) => { fromAmong: ObjectRef; members: ObjectFilter[] } | null;
}

/** After a complete first member: ", B, …, or Z" (at least one unjoined
 * middle member; the two-member form is the ordinary OR alternative). */
export function parseIndependentFilterList(c: Cursor, first: ObjectFilter, g: FilterListGrammar): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const members: ObjectFilter[] = [];
    for (;;) {
      if (!c.punct(",")) return null;
      const join = c.attempt(c => {
        if (c.word("or") !== null) return "or";
        return c.word("and") !== null && c.punct("/") && c.word("or") !== null ? "or" : null;
      });
      if (c.isWord("a", "an", "another", "other", "target", "each", "all", "any", "up")) return null;
      const member = g.item(c);
      if (!member) return null;
      members.push(member);
      if (join) break;
    }
    if (members.length < 2) return null;
    const pooled = g.hoist(first, members);
    if (pooled) {
      const listed = listUnion(first, pooled.members, g);
      return listed && { ...listed, fromAmong: pooled.fromAmong };
    }
    return listUnion(first, members, g);
  });
}

function listUnion(first: ObjectFilter, members: ObjectFilter[], g: FilterListGrammar): ObjectFilter | null {
  // A scope stated only on a later member ("…, or creature you control")
  // may restrict that member or the whole list; keep it unsupported, as
  // for the two-member alternative.
  for (const member of members)
    if (SCOPE_KEYS.some(key => !g.scoped(first, key) && g.scoped(member, key))) return null;
  const { other, ...term } = first;
  return { ...(other ? { other } : {}), anyOf: [term, ...members] };
}
