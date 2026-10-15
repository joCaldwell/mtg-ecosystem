// A token created with a printed name ('create "Smaug", a legendary …') may
// refer to itself by that name inside the abilities it is created with. CR
// 201.5: text that refers to the object it's on by name means just that
// object, which is what "this token" says; the creating card's own name (~)
// keeps its separate meaning (201.5a).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import { lex, type Token } from "../lexer.ts";

/** Every occurrence of the name (exact printed words; the last may be
 * possessive) becomes the words "this token". Names that are not plain words
 * are left alone, so their use keeps failing. */
export function ownNameAsThisToken(tokens: Token[], name: string): Token[] {
  let words: Token[];
  try { words = lex(name); } catch { return tokens; }
  if (!words.length || words.some(t => t.kind !== "word" || t.possessive)) return tokens;
  const out: Token[] = [];
  for (let i = 0; i < tokens.length;) {
    const match = words.every((w, k) => {
      const t = tokens[i + k];
      return t?.kind === "word" && w.kind === "word" && t.raw === w.raw && (k === words.length - 1 || !t.possessive);
    });
    if (!match) { out.push(tokens[i++]); continue; }
    const first = tokens[i], last = tokens[i + words.length - 1];
    const possessive = last.kind === "word" && last.possessive;
    out.push({ ...(first.provenance ? { provenance: first.provenance } : {}), kind: "word", value: "this", raw: "this", possessive: false });
    out.push({ ...(last.provenance ? { provenance: last.provenance } : {}), kind: "word", value: "token", raw: "token", possessive,
      ...(possessive && last.kind === "word" && last.possessiveSuffix ? { possessiveSuffix: last.possessiveSuffix } : {}) });
    i += words.length;
  }
  return out;
}
