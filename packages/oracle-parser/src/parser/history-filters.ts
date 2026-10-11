// Historical damage predicates on selected objects/players. CR 120.3, 120.7,
// 400.7 and 608.2h: actual damage, its source incarnation and its recipient
// cannot be reconstructed from damage marked or present-day life totals.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { DamageHistoryPredicate, ObjectFilter, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

interface ReferenceGrammar {
  parseRelationObject(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
}

/** CR508.4: declared attacking history survives the end of combat and is
 * not inferred from current attacking/tapped status. */
export function parseCombatActionHistoryPredicate(c: Cursor): ObjectFilter["combatHistory"] | null {
  return c.attempt(c => {
    if (c.word("that") === null) return null;
    c.word("has", "have");
    if (c.word("attacked") === null) return null;
    const during = c.words("this", "turn") ? "this-turn" : c.words("this", "combat") ? "this-combat" : null;
    return during ? { action: "attacked", during } : null;
  });
}

export function parseDamageHistoryPredicate(c: Cursor, grammar: ReferenceGrammar): DamageHistoryPredicate | null {
  return c.attempt(c => {
    const relative = c.word("that", "who") !== null;
    let passive = !relative;
    if (relative) {
      if (c.word("was", "were") !== null || c.words("has", "been") || c.words("have", "been")) passive = true;
      else c.word("has", "have");
    }
    if (c.word("dealt") === null) return null;
    const damage = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
    if (c.word("damage") === null) return null;
    let source: ObjectRef | undefined;
    let to: ObjectRef | PlayerRef | undefined;
    if (passive && c.word("by") !== null) {
      const operand = grammar.parseRelationObject(c);
      if (!operand) return null;
      source = operand;
    } else if (!passive && c.word("to") !== null) {
      const operand = grammar.parsePlayerRef(c) ?? grammar.parseRelationObject(c);
      if (!operand) return null;
      to = operand;
    }
    let window: DamageHistoryPredicate["window"];
    if (c.words("this", "turn")) window = { during: "this-turn" };
    else if (c.words("this", "combat")) window = { during: "this-combat" };
    else if (c.words("this", "way")) window = { event: "this-way" };
    else return c.fail("explicit damage-history period or event link");
    return passive
      ? { role: "recipient", window, ...(damage ? { damage } : {}), ...(source ? { source } : {}) }
      : { role: "source", window, ...(damage ? { damage } : {}), ...(to ? { to } : {}) };
  });
}
