// "it becomes your choice of a 3/3 creature with flying, a 2/5 creature with
// vigilance, or a 0/12 creature with defender": one player chooses one of
// the printed characteristic sets as the effect is created (CR 608.2d); the
// chosen set is the only one applied. Each option is the same effect its
// "becomes <option>" wording alone would create.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";

const startsOption = (token: Token | undefined) =>
  token?.kind === "number" || (token?.kind === "word" && !token.possessive && (token.value === "a" || token.value === "an"));

/** `becomeOption` parses one complete "becomes <description>" for `what`. */
export function parseBecomeChoice(c: Cursor, what: ObjectRef,
  becomeOption: (option: Cursor, what: ObjectRef) => Effect | null): Effect | null {
  return c.attempt((c): Effect | null => {
    const verb = c.peek();
    if (verb?.kind !== "word" || verb.possessive || (verb.value !== "becomes" && verb.value !== "become")) return null;
    c.next();
    if (!c.words("your", "choice", "of")) return null;
    // The option list runs to the end of this clause.
    let end = c.pos;
    while (end < c.tokens.length && !(c.tokens[end].kind === "punct" && [".", ";"].includes((c.tokens[end] as { value: string }).value))) end++;
    const options: Token[][] = [];
    let current: Token[] = [];
    let sawOr = false;
    for (let i = c.pos; i < end; i++) {
      const token = c.tokens[i];
      const next = c.tokens[i + 1];
      if (token.kind === "punct" && token.value === "," ) {
        const after = next?.kind === "word" && next.value === "or" ? c.tokens[i + 2] : next;
        if (!startsOption(after)) return null;
        options.push(current); current = [];
        if (next?.kind === "word" && next.value === "or") { sawOr = true; i++; }
        continue;
      }
      if (token.kind === "word" && !token.possessive && token.value === "or" && startsOption(next)) {
        options.push(current); current = []; sawOr = true;
        continue;
      }
      current.push(token);
    }
    options.push(current);
    if (!sawOr || options.length < 2 || options.some(option => !option.length)) return null;
    // A trailing "in addition to its other types" could qualify only the
    // last option or every option; that scope is not decided here.
    if (options.some(option => option.some(token => token.kind === "word" && token.value === "addition"))) return null;
    const effects: Effect[] = [];
    for (const option of options) {
      const sub = new Cursor([verb, ...option]);
      const effect = becomeOption(sub, what);
      if (!effect || !sub.done()) return null;
      effects.push(effect);
    }
    c.pos = end;
    return { effect: "choice", who: { player: "you" }, options: effects };
  });
}
