// Anchor words (CR 614.12c, 607.2m): "As this enchantment enters, choose
// Khans or Dragons." followed by "• Khans — [ability]" bullets. The bullets
// are abilities the permanent has while their word is the chosen one; they
// are not modes of a spell or ability (700.2).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

type AnchorWords = Extract<Ability, { kind: "anchor-words" }>;

/** The header must offer exactly the printed bullet labels, in order, and
 * every bullet must be one complete permanent ability. A bullet that only
 * parses as resolving spell text is not an ability of a permanent. */
export function parseAnchorWords(header: Token[], bullets: Token[][], parseAbility: (tokens: Token[]) => Ability | null): AnchorWords | null {
  const c = new Cursor(header);
  if (c.word("as") === null) return null;
  const what = parseObjectRef(c);
  if (!what || (what.ref !== "self" && what.ref !== "this") || c.word("enters") === null || !c.punct(",") || c.word("choose") === null) return null;
  const words: string[] = [];
  for (;;) {
    const word = c.anyWord();
    if (!word) return null;
    words.push(word.raw);
    if (c.word("or") === null) break;
  }
  if (!c.punct(".") || !c.done() || words.length !== 2 || words[0] === words[1] || bullets.length !== words.length) return null;
  const options: AnchorWords["options"] = [];
  for (const [index, tokens] of bullets.entries()) {
    const [label, dash] = tokens;
    if (label?.kind !== "word" || label.possessive || label.raw !== words[index] || dash?.kind !== "punct" || dash.value !== "—") return null;
    const ability = parseAbility(tokens.slice(2));
    if (!ability || (ability.kind !== "static" && ability.kind !== "triggered" && ability.kind !== "activated")) return null;
    options.push({ word: words[index], ability });
  }
  return { kind: "anchor-words", what, who: { player: "you" }, options };
}
