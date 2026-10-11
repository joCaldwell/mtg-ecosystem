// Explicit named-action outcome predicates. A failed/declined move and a
// search that found no cards must remain distinguishable from choosing to pay
// a resolution cost (CR118.12, 608.2c/i, 609.3):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Risen Reef can put a nonland or a declined land into hand:
// https://magic.wizards.com/en/news/feature/core-set-2020-release-notes-2019-06-25
// Inscribed Tablet's earlier land selection is mandatory if one is found:
// https://media.wizards.com/2022/downloads/DMU_Release_Notes/EN_MTGDMU_ReleaseNotes_20220825_Final.pdf
import type { Condition, ObjectFilter, PutActionSubject } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if ("selectionId" in value || "ref" in value && value.ref === "target" ||
      "player" in value && ["target-player", "target-opponent"].includes(String(value.player))) return true;
  return Object.values(value).some(hasTarget);
}
function hasZone(filter: ObjectFilter): boolean {
  return !!filter.zone || !!filter.anyOf?.some(hasZone) || !!filter.allOf?.some(hasZone);
}
function subject(c: Cursor, g: ConditionGrammar): PutActionSubject | null {
  return c.attempt(c => {
    const definite = c.attempt(c => c.words("the", "card") && c.done()
      ? { match: "definite-description" as const, filter: { cls: "card" as const } } : null);
    if (definite) return definite;
    const object = c.attempt(c => {
      const what = g.parseObjectRef(c);
      return what && c.done() && ["self", "this", "it", "them", "that", "related", "choice-result", "enchanted", "equipped"].includes(what.ref)
        && !hasTarget(what) ? { match: "object" as const, what } : null;
    });
    if (object) return object;
    if (c.word("a", "an") === null) return null;
    const filter = g.parseFilter(c);
    if (!filter || filter.cls !== "card" || !c.done() || hasZone(filter) || hasTarget(filter)) return null;
    return { match: "filter", filter, comparison: { op: "ge", value: { amount: "fixed", value: 1 } } };
  });
}

export function parsePutActionOutcome(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who || hasTarget(who)) return null;
    const presentNegative = c.word("don't", "doesn't") !== null || c.words("do", "not") || c.words("does", "not");
    const pastNegative = !presentNegative && (c.word("didn't", "haven't", "hasn't") !== null || c.words("did", "not") || c.words("have", "not") || c.words("has", "not"));
    const negate = presentNegative || pastNegative;
    if (!negate) c.word("did", "have", "has");
    if (c.word("put", "puts") === null) return null;
    const start = c.pos;
    // A bounded subject keeps the movement destination out of its filter.
    // Tokens keep their original positions; no replacement text is lexed.
    const boundary = c.tokens.slice(start).findIndex(t => t.kind === "word" && !t.possessive && ["into", "onto", "on"].includes(t.value));
    if (boundary < 1) return null;
    const parsed = subject(new Cursor(c.tokens.slice(start, start + boundary)), g);
    if (!parsed) return null;
    c.pos = start + boundary + 1;
    const to = g.parseZoneRef(c);
    if (!to || hasTarget(to) || ["hand", "library", "graveyard"].includes(to.zone) && to.owner === undefined) return null;
    const explicit = c.words("this", "way");
    // Without explicit linkage, only the printed negative fallback form is
    // supported. Present-tense unless clauses offer actions, not history.
    if (!explicit && (!negate || g.purpose === "unless")) return null;
    const result: Condition = { condition: "put-action-history", who, subject: parsed, to, scope: explicit ? "this-way" : "contextual" };
    return negate ? { condition: "not", of: result } : result;
  });
}
