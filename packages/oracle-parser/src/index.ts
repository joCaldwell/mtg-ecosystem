// @mtg-ecosystem/oracle-parser — public API.
//
// Pipeline: normalize (lines) → lex (tokens) → parse (typed AST).
// See AGENTS.md for the development loop and design rules.

export { parseOracleText } from "./parser/index.ts";
export { normalizeOracleText } from "./normalize.ts";
export { lex, LexError, type Token } from "./lexer.ts";
export * from "./ast.ts";
