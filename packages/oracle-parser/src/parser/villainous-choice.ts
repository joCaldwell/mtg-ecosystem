// "[A player] faces a villainous choice — [option A], or [option B]."
// CR 701.55a: that player chooses one option, then all actions in the chosen
// option are performed; 701.55b-d: an impossible option may be chosen, and
// several facing players do this one at a time in APNAP order.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

const isPunct = (c: Cursor, index: number, value: string): boolean => {
  const token = c.tokens[index];
  return token?.kind === "punct" && token.value === value;
};
const isWord = (c: Cursor, index: number, value: string): boolean => {
  const token = c.tokens[index];
  return token?.kind === "word" && !token.possessive && token.value === value;
};

/** The options run to the end of the sentence and are separated by the one
 * ", or" at which both halves are complete instruction lists; a sentence
 * with several such divisions is ambiguous and fails. Each option is parsed
 * in a fresh context: its subjects and pronouns are its own. */
export function parseVillainousChoice(
  c: Cursor, parsePlayer: (c: Cursor) => PlayerRef | null, parseOption: (option: Cursor) => Effect[] | null,
): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = parsePlayer(c);
    if (!who || c.word("faces") === null || !c.words("a", "villainous", "choice") || !c.punct("—")) return null;
    const start = c.pos;
    let end = start;
    let quoted = false;
    for (; end < c.tokens.length; end++) {
      if (isPunct(c, end, '"')) quoted = !quoted;
      if (!quoted && isPunct(c, end, ".")) break;
    }
    if (quoted) return null;
    const parse = (from: number, to: number): Effect[] | null => {
      const option = new Cursor(c.tokens.slice(from, to));
      const effects = parseOption(option);
      return effects && option.done() ? effects : null;
    };
    const divisions: [Effect[], Effect[]][] = [];
    for (let comma = start + 1; comma < end - 2; comma++) {
      if (!isPunct(c, comma, ",") || !isWord(c, comma + 1, "or")) continue;
      const first = parse(start, comma);
      const second = first && parse(comma + 2, end);
      if (first && second) divisions.push([first, second]);
    }
    if (divisions.length !== 1) return c.fail("one division into two villainous-choice options");
    c.pos = end;
    return [{ effect: "villainous-choice", who, options: divisions[0] }];
  });
}
