// Cleave (CR 702.148a-b): "If this spell's cleave cost was paid, change its
// text by removing all text found within square brackets in the spell's
// rules text." A bracketed paragraph therefore has two texts, and both are
// parsed: the paragraph with the bracketed words (cast normally) and the
// paragraph without them (cast for its cleave cost, a text-changing effect).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, SpellLine } from '../ast.ts';
import type { Token } from '../lexer.ts';

const isBracket = (token: Token, value: '[' | ']') => token.kind === 'punct' && token.value === value;

/** Only a spell with the cleave keyword gives square brackets this meaning. */
export function hasCleaveKeyword(lines: readonly string[]): boolean {
  return lines.some(line => /^cleave\s/i.test(line));
}

export function hasBrackets(tokens: readonly Token[]): boolean {
  return tokens.some(token => isBracket(token, '[') || isBracket(token, ']'));
}

/** Both readings must parse completely as spell text; neither is optional.
 * Brackets must pair up, hold at least one token, and never nest. */
export function parseCleaveParagraph(tokens: readonly Token[], parseLine: (tokens: Token[]) => Ability | null): SpellLine | null {
  const printed: Token[] = [], cleaved: Token[] = [];
  let open = false, spans = 0, length = 0;
  for (const token of tokens) {
    if (isBracket(token, '[')) {
      if (open) return null;
      open = true; length = 0;
    } else if (isBracket(token, ']')) {
      if (!open || length === 0) return null;
      open = false; spans++;
    } else {
      printed.push(token);
      if (open) length++; else cleaved.push(token);
    }
  }
  if (open || spans === 0 || cleaved.length === 0) return null;
  const full = parseLine(printed), reduced = parseLine(cleaved);
  if (full?.kind !== 'spell' || reduced?.kind !== 'spell' || full.abilityWord || reduced.abilityWord) return null;
  return { ...full, cleaved: reduced.effects };
}
