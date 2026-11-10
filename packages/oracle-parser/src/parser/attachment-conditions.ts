// "[this Equipment] is attached to a creature", "an Equipment named X is
// attached to a creature you control", "two or more Equipment are attached
// to it": the current attachment relation (CR 301.5, 303.4, 701.3). A
// definite subject is tested itself; an indefinite or numeric subject asks
// how many matching objects are attached, never one chosen object.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, ObjectFilter, ObjectRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

interface AttachmentGrammar {
  parseComparison(c: Cursor): Comparison | null;
  parseFilter(c: Cursor): ObjectFilter | null;
  parseObjectRef(c: Cursor): ObjectRef | null;
  parseRelation(c: Cursor): ObjectFilter | null;
}

const definite = (what: ObjectRef) => ["self", "this", "it", "enchanted", "equipped"].includes(what.ref)
  || (what.ref === "that" && !what.plural);

function attachment(c: Cursor, g: AttachmentGrammar): ObjectFilter["attachedTo"] | null {
  if (!c.isWord("attached")) return null;
  const relation = g.parseRelation(c);
  return relation && Object.keys(relation).join() === "attachedTo" ? relation.attachedTo : null;
}

export function parseAttachmentCondition(c: Cursor, g: AttachmentGrammar): Condition | null {
  const counted = c.attempt((c): Condition | null => {
    // "An Equipment … is attached" is existential, not exactly one.
    const comparison: Comparison | null = c.isWord("another") || c.word("a", "an") !== null
      ? { op: "ge", value: { amount: "fixed", value: 1 } } : g.parseComparison(c);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || of.attachedTo || c.word("is", "are") === null) return null;
    const attachedTo = attachment(c, g);
    return attachedTo ? { condition: "count", of: { ...of, attachedTo }, comparison } : null;
  });
  if (counted) return counted;
  // "this creature is enchanted by exactly two Auras": how many matching
  // Auras are attached to it (CR 303.4); every one of them must be an Aura.
  const enchanted = c.attempt((c): Condition | null => {
    const what = g.parseObjectRef(c);
    if (!what || !definite(what) || !c.words("is", "enchanted", "by")) return null;
    const comparison: Comparison | null = c.word("a", "an") !== null ? { op: "ge", value: { amount: "fixed", value: 1 } } : g.parseComparison(c);
    const of = comparison && g.parseFilter(c);
    if (!of || !comparison || of.attachedTo || of.subtypes?.length !== 1 || of.subtypes[0] !== "Aura") return null;
    return { condition: "count", of: { ...of, attachedTo: what }, comparison };
  });
  if (enchanted) return enchanted;
  return c.attempt((c): Condition | null => {
    const what = g.parseObjectRef(c);
    if (!what || !definite(what) || c.word("is") === null) return null;
    const attachedTo = attachment(c, g);
    return attachedTo ? { condition: "is-filter", what, filter: { attachedTo } } : null;
  });
}
