// "If it's the second creature spell you cast this turn": the spell (or the
// permanent it became, CR 400.7d) is that ordinal among the matching spells
// its caster cast this turn. Spells are counted as they were cast (601.2i);
// copies are not cast (707.10).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

const ORDINALS = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];

export function parseCastOrdinal(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let what: ObjectRef;
    if (c.word("it's") !== null) what = { ref: "it" };
    else {
      const subject = g.parseObjectRef(c);
      if (!subject || !["it", "self", "this"].includes(subject.ref) || c.word("was", "is") === null) return null;
      what = subject;
    }
    if (c.word("the") === null) return null;
    const ordinal = ORDINALS.indexOf(c.word(...ORDINALS) ?? "") + 1;
    if (ordinal === 0) return null;
    // The described kind ends where its caster begins.
    let end = c.pos;
    while (end < c.tokens.length && !(c.tokens[end].kind === "word" && ["you", "you've"].includes((c.tokens[end] as { value: string }).value))) end++;
    const bounded = new Cursor(c.tokens.slice(c.pos, end));
    const of = g.parseFilter(bounded);
    if (!of || !bounded.done() || of.cls !== "spell" || Object.keys(of).some(key => !["cls", "types", "nonTypes", "subtypes", "colors"].includes(key))) return null;
    c.pos = end;
    if (!(c.words("you", "cast") || c.words("you've", "cast")) || !c.words("this", "turn")) return null;
    return { condition: "cast-ordinal", what, who: { player: "you" }, of, ordinal, during: "this-turn" };
  });
}
