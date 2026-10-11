// @mtg-ecosystem/oracle-parser — public API.
//
// Pipeline: normalize (lines) → lex (tokens) → parse (typed AST).
// See AGENTS.md for the development loop and design rules.

export { parseOracleText, type ParseContext } from "./parser/index.ts";
export { normalizeOracleText } from "./normalize.ts";
export { lex, LexError, type Token } from "./lexer.ts";
export { buildCardNameCatalog, CardNameCatalog, type CardNameSource, type CatalogNameKind, type CatalogNameSource, type CatalogNameMatch } from "./card-names.ts";
export * from "./ast.ts";
export * from "./semantic/index.ts";
