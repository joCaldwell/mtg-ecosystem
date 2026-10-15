/**
 * "[Ability text about keyword K]. The same is true for K1, K2, and K3."
 * (Majestic Myriarch, Urborg Scavengers, Odric, Lunarch Marshal): the first
 * sentence is a template; the second repeats it once for each listed keyword,
 * with K replaced everywhere it appears. The printed ability therefore says
 * exactly what the separately written abilities would say, and is represented
 * that way: a static ability becomes a group of the per-keyword static
 * effects, a triggered ability resolves the per-keyword sentences in order.
 *
 * Only parameterless keywords are accepted. "Landwalk" and "protection" name
 * keyword families whose instances would have to be matched per quality
 * (CR 702.14, 702.16); those lines keep failing.
 */
import type { Ability } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseKeywordLine } from "./keywords.ts";

const isWord = (token: Token | undefined, value: string) => token?.kind === "word" && !token.possessive && token.value === value;
const isPunct = (token: Token | undefined, value: string) => token?.kind === "punct" && token.value === value;

/** The keyword name a token sequence spells, when it is exactly one
 * parameterless keyword. */
function simpleKeyword(tokens: Token[]): string | null {
  if (tokens.length === 0 || tokens.some(token => token.kind !== "word" || token.possessive)) return null;
  const c = new Cursor(tokens);
  const keywords = parseKeywordLine(c);
  if (!keywords || keywords.length !== 1 || !c.done()) return null;
  const [keyword] = keywords;
  return Object.keys(keyword).length === 1 ? keyword.keyword : null;
}

function splitList(tokens: Token[]): Token[][] | null {
  const items: Token[][] = [[]];
  for (const token of tokens) {
    if (isPunct(token, ",")) items.push([]);
    else items[items.length - 1].push(token);
  }
  const last = items[items.length - 1];
  if (items.length === 1) {
    // "K1 and K2"
    const and = last.findIndex(token => isWord(token, "and"));
    if (and < 0) return null;
    return [last.slice(0, and), last.slice(and + 1)];
  }
  if (!isWord(last[0], "and")) return null;
  items[items.length - 1] = last.slice(1);
  return items;
}

function sameSequence(a: Token[], b: Token[]): boolean {
  return a.length === b.length && a.every((token, i) => token.kind === "word" && isWord(b[i], token.value));
}

function substitute(tokens: Token[], from: Token[], to: Token[]): Token[] {
  const out: Token[] = [];
  for (let i = 0; i < tokens.length;) {
    if (sameSequence(from, tokens.slice(i, i + from.length))) {
      out.push(...to);
      i += from.length;
    } else out.push(tokens[i++]);
  }
  return out;
}

export function parseSameIsTrueFor(tokens: Token[], parseLine: (tokens: Token[]) => Ability | null): Ability | null {
  const period = tokens.findIndex(token => isPunct(token, "."));
  if (period < 0) return null;
  const tail = tokens.slice(period + 1);
  const header = ["the", "same", "is", "true", "for"];
  if (!header.every((word, i) => isWord(tail[i], word)) || !isPunct(tail.at(-1), ".")) return null;
  const items = splitList(tail.slice(header.length, -1));
  if (!items || items.length < 2) return null;
  const names = items.map(simpleKeyword);
  if (names.some(name => name === null) || new Set(names).size !== names.length) return null;

  const template = tokens.slice(0, period + 1);
  // The template keyword is the one keyword the first sentence repeats.
  const candidates: { tokens: Token[]; name: string }[] = [];
  for (let i = 0; i < template.length; i++) {
    for (const length of [1, 2]) {
      const sequence = template.slice(i, i + length);
      const name = sequence.length === length ? simpleKeyword(sequence) : null;
      if (!name || candidates.some(candidate => sameSequence(candidate.tokens, sequence))) continue;
      const occurrences = template.filter((_, j) => sameSequence(sequence, template.slice(j, j + length))).length;
      if (occurrences >= 2) candidates.push({ tokens: sequence, name });
    }
  }
  if (candidates.length !== 1 || names.includes(candidates[0].name)) return null;
  const from = candidates[0];

  const first = parseLine(template);
  if (!first || (first.kind !== "static" && first.kind !== "triggered")) return null;
  const expected = JSON.stringify(first);
  const variants: Ability[] = [];
  for (const [index, item] of items.entries()) {
    const variant = parseLine(substitute(template, from.tokens, item));
    if (!variant || variant.kind !== first.kind) return null;
    // The substitution must change nothing but the keyword itself (keyword
    // nodes and `withAbility` filter entries are the keyword's name).
    const restored = JSON.stringify(variant).replaceAll(JSON.stringify(names[index]), JSON.stringify(from.name));
    if (restored !== expected) return null;
    variants.push(variant);
  }
  if (first.kind === "static") return { ...first, effect: { static: "group", effects: [first.effect,
    ...variants.map(variant => variant.kind === "static" ? variant.effect : first.effect)] } };
  if (first.kind === "triggered") return { ...first, effects: [...first.effects,
    ...variants.flatMap(variant => variant.kind === "triggered" ? variant.effects : [])] };
  return null;
}
