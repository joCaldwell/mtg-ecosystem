// packages/oracle-parser/src/index.ts
// Entry point for the oracle-parser library

import { ANTLRInputStream, CommonTokenStream, ANTLRErrorListener, RecognitionException, Recognizer } from "antlr4ts";
import { MTGLexer } from "../generated/MTGLexer";
import { MTGParser } from "../generated/MTGParser";

// Custom error listener to capture syntax errors programmatically
class ParserErrorListener implements ANTLRErrorListener<any> {
  public errors: string[] = [];

  syntaxError<T>(
    recognizer: Recognizer<T, any>,
    offendingSymbol: T | undefined,
    line: number,
    charPositionInLine: number,
    msg: string,
    e: RecognitionException | undefined
  ): void {
    this.errors.push(`line ${line}:${charPositionInLine} - ${msg}`);
  }
}

export interface ParseResult {
  success: boolean;
  errors: string[];
  tree?: any;
}

// Detailed parsing function that returns error logs
export function parseOracleTextDetails(text: string): ParseResult {
  const chars = new ANTLRInputStream(text);
  const lexer = new MTGLexer(chars);
  const tokens = new CommonTokenStream(lexer);
  const parser = new MTGParser(tokens);
  
  const listener = new ParserErrorListener();
  lexer.removeErrorListeners();
  parser.removeErrorListeners();
  parser.addErrorListener(listener);
  
  const tree = parser.card();
  
  return {
    success: listener.errors.length === 0,
    errors: listener.errors,
    tree: listener.errors.length === 0 ? tree : undefined
  };
}

// Simplified function that throws on errors (used in tests)
export function parseOracleText(text: string): any {
  const result = parseOracleTextDetails(text);
  if (!result.success) {
    throw new Error(`Syntax errors found: ${result.errors.join("; ")}`);
  }
  return result.tree;
}
