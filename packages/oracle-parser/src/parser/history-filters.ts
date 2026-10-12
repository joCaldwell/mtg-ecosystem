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
export function parseCombatActionHistoryPredicate(c: Cursor): { action: "attacked"; during: "this-turn" | "this-combat" } | null {
  return c.attempt(c => {
    if (c.word("that") === null) return null;
    c.word("has", "have");
    if (c.word("attacked") === null) return null;
    const during = c.words("this", "turn") ? "this-turn" : c.words("this", "combat") ? "this-combat" : null;
    return during ? { action: "attacked", during } : null;
  });
}

/** CR 509.1g-h, 509.4: a creature blocked if it was declared as a blocker
 * or an effect made it block; an attacker was blocked once it became a
 * blocked creature. A creature put onto the battlefield blocking never
 * blocked. An operand restricts the historical counterpart of that block;
 * it selects nothing. Not having attacked is the absence of a declaration
 * as an attacker in the window (CR 508.4). */
export function parseBlockHistoryQualifier(c: Cursor, grammar: ReferenceGrammar): ObjectFilter | null {
  type During = "this-turn" | "this-combat";
  const window = (c: Cursor): During | null =>
    c.words("this", "turn") ? "this-turn" : c.words("this", "combat") ? "this-combat" : null;
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("that") === null) {
      // "creatures it blocked this turn": the candidates were blocked by it.
      if (!c.isWord("it", "this", "that") && c.peek()?.kind !== "selfref") return null;
      const blocker = grammar.parseRelationObject(c);
      if (!blocker || c.word("blocked") === null) return null;
      const during = window(c);
      return during ? { combatHistory: { action: "was-blocked", during, blocker } } : null;
    }
    if (c.word("didn't") !== null) {
      if (c.word("attack") === null) return null;
      const during = window(c);
      return during ? { combatHistory: { action: "attacked", during, negated: true } } : null;
    }
    const passive = (c: Cursor): true | null => c.word("was", "were") !== null && c.word("blocked") !== null ? true : null;
    const operand = (c: Cursor): ObjectRef | null | undefined =>
      c.word("by") === null ? undefined : grammar.parseRelationObject(c);
    if (c.attempt(passive)) {
      const blocker = operand(c);
      const during = blocker === null ? null : window(c);
      return during ? { combatHistory: { action: "was-blocked", during, ...(blocker ? { blocker } : {}) } } : null;
    }
    if (c.word("blocked") === null) return null;
    if (c.attempt(c => c.word("or") !== null ? passive(c) : null)) {
      // "blocked or was blocked by X": X is the counterpart in either role.
      const other = operand(c);
      const during = other === null ? null : window(c);
      return during ? { anyOf: [
        { combatHistory: { action: "blocked", during, ...(other ? { attacker: other } : {}) } },
        { combatHistory: { action: "was-blocked", during, ...(other ? { blocker: other } : {}) } },
      ] } : null;
    }
    const attacker = c.attempt(grammar.parseRelationObject);
    const during = window(c);
    return during ? { combatHistory: { action: "blocked", during, ...(attacker ? { attacker } : {}) } } : null;
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
