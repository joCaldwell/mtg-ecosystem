// "Reveal the first card you draw each turn. Whenever you reveal a creature
// card this way, draw a card.": a static ability linked to the triggered
// abilities printed after it in its paragraph (CR 603.11, 607.2a, 701.20a).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Sentence, StaticAbility, TriggeredAbility } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

const isWord = (token: Token | undefined, value: string) => token?.kind === "word" && !token.possessive && token.value === value;
const isPeriod = (token: Token | undefined) => token?.kind === "punct" && token.value === ".";

export function parseLinkedReveal(tokens: Token[], parseBody: (tokens: Token[]) => Sentence[] | null): Extract<Ability, { kind: "linked-triggers" }> | null {
  const c = new Cursor(tokens);
  const optional = c.words("you", "may");
  if (!c.words("reveal", "the", "first", "card", "you", "draw")) return null;
  const turns = c.words("on", "each", "of", "your", "turns") ? "your" as const : c.words("each", "turn") ? undefined : null;
  if (turns === null) return null;
  c.words("as", "you", "draw", "it");
  if (!c.punct(".")) return null;
  const statik: StaticAbility = { kind: "static", effect: { static: "reveal-first-draw", who: { player: "you" },
    ...(turns ? { turns } : {}), ...(optional ? { optional: true as const } : {}) } };
  const triggers: TriggeredAbility[] = [];
  let start = c.pos;
  while (start < tokens.length) {
    const header = new Cursor(tokens.slice(start));
    if (!header.words("whenever", "you", "reveal") || header.word("a", "an") === null) return null;
    const what = parseFilter(header);
    if (!what || what.cls !== "card" || !header.words("this", "way") || !header.punct(",")) return null;
    // The trigger's text runs to the period before the next linked trigger.
    let end = start + header.pos;
    let quoted = false;
    for (; end < tokens.length; end++) {
      const token = tokens[end];
      if (token.kind === "punct" && token.value === '"') quoted = !quoted;
      if (!quoted && isPeriod(token) && (end + 1 === tokens.length || isWord(tokens[end + 1], "whenever"))) break;
    }
    if (end >= tokens.length) return null;
    const effects = parseBody(tokens.slice(start + header.pos, end + 1));
    if (!effects) return null;
    triggers.push({ kind: "triggered", trigger: { trigger: "reveals-linked", who: { player: "you" }, what }, effects });
    start = end + 1;
  }
  return triggers.length ? { kind: "linked-triggers", static: statik, triggers } : null;
}
