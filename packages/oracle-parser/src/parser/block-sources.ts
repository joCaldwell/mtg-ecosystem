// The exact token lines one parsed ability block was built from, recorded
// only when the parse carries source provenance. The span certificate
// (semantic/spans.ts) attributes every one of these tokens to an AST field.
import type { Ability, ParsedLine } from "../ast.ts";
import type { Token } from "../lexer.ts";

export interface BlockSourceLine {
  text: string;
  tokens: Token[];
  /** "header": the block's own line. "option": a modal bullet whose tokens
   * (after its bullet mark) are one option of the block's modal step.
   * "paragraph": a following paragraph merged into this block. */
  role: "header" | "option" | "paragraph";
}
export interface BlockSource {
  lines: BlockSourceLine[];
  /** For a merged paragraph: the block's ability before the merge. */
  premerge?: Ability;
}

const sources = new WeakMap<ParsedLine, BlockSource>();

export function registerBlockSource(line: ParsedLine, source: BlockSource): ParsedLine {
  if (line.ok && source.lines.every(l => l.tokens.every(token => token.provenance))) sources.set(line, source);
  return line;
}

export function blockSource(line: ParsedLine): BlockSource | undefined {
  return sources.get(line);
}
