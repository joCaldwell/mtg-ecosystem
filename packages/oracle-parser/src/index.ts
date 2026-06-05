// packages/oracle-parser/src/index.ts
// Entry point for the oracle-parser library

import { ANTLRInputStream, CommonTokenStream } from "antlr4ts";
import { MTGLexer } from "../generated/MTGLexer.js";
import { MTGParser } from "../generated/MTGParser.js";

export function parseOracleText(text: string): any {
  const chars = new ANTLRInputStream(text);
  const lexer = new MTGLexer(chars);
  const tokens = new CommonTokenStream(lexer);
  const parser = new MTGParser(tokens);
  
  // Call the entry rule 'card'
  return parser.card();
}
