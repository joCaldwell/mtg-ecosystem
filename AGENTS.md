# 🤖 AI Agent Onboarding Manual (AGENTS.md)

Welcome! This document provides context, execution commands, and guidelines for AI coding assistants working in this repository.

---

## 🏗️ Project Context & Architecture
This is a **TypeScript monorepo** using npm workspaces. The current focus is **Layer 0 (Oracle Text Parser)**.
*   **Core Parser**: [packages/oracle-parser](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser) (defines ANTLR grammar, TypeScript AST types, and visitor walking logic).
*   **Shared Config**: [package.json](file:///home/josh/Code/mtg-ecosystem/package.json) (root), [tsconfig.base.json](file:///home/josh/Code/mtg-ecosystem/tsconfig.base.json).
*   **Bulk Dataset**: [.scryfall-cache/oracle-cards.json](file:///home/josh/Code/mtg-ecosystem/.scryfall-cache/oracle-cards.json) (created by ingest script).

---

## 🚀 Key Commands
*   `npm install` — Installs workspace dependencies and links packages.
*   `npm run ingest` — Downloads and caches Scryfall's unique cards JSON export.
*   `npm run generate-parser` — Compiles `.g4` grammars to TS under `generated/` (requires Java).
*   `npm run validate` — Runs the parser against all 33,000+ cards and reports statistics and error groupings.
*   `npm test` — Runs the Vitest test suite.
*   `npm run build` — Compiles TS workspaces to JS under their respective `dist/` folders.

---

## 🔄 Development Loop: Adding Grammar Support
When adding support for a new keyword, action, or trigger rule:
1.  **Analyze**: Run `npm run validate` to identify failing card examples and error patterns.
2.  **Rules Reference**: Consult the official Magic: The Gathering Comprehensive Rules (CR) or reliable wiki resources to understand the exact game mechanics, layers, and syntax requirements for the rule or action you are implementing. This ensures that the grammar structure is built with MTG's core rules engine logic in mind.
3.  **Lexer**: Add any new literal words as UPPERCASE tokens in [MTGLexer.g4](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/grammar/MTGLexer.g4).
4.  **Parser**: Add syntactic rules to the appropriate sub-parser file (e.g., [MTGEffectsParser.g4](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/grammar/MTGEffectsParser.g4)).
5.  **Compile**: Run `npm run generate-parser` to rebuild the parser runtime.
6.  **AST Types**: Define corresponding types in [src/ast/types.ts](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/src/ast/types.ts).
7.  **Visitor**: Extend visitor methods in [src/visitor/ASTBuilder.ts](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/src/visitor/ASTBuilder.ts) to construct your new AST nodes.
8.  **Test**: Add unit tests in [tests/parser.test.ts](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/tests/parser.test.ts) and verify them using `npm test`.
9.  **Re-validate**: Run `npm run validate` to verify the baseline success rate goes up!

---

## ⚠️ Coding Constraints & Rules
*   **Do NOT Commit Generated Parser Files**: All files in `packages/oracle-parser/generated/` are gitignored. Never add them to Git.
*   **No String Literals in Parser Grammars**: Because we use a split Lexer/Parser, you cannot use literal strings like `'exile'` or `'+'` inside any parser file. Declare them in [MTGLexer.g4](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/grammar/MTGLexer.g4) and use the token name.
*   **No Reminder Text Parsing**: Stripping parentheticals must happen in [normalize.ts](file:///home/josh/Code/mtg-ecosystem/packages/oracle-parser/src/ingest/normalize.ts). The parser must only process rules text.
*   **TypeScript Resolution**: Do not use suffix extensions (`.ts` or `.js`) on imports inside package source code, unless required. In scripts, use `.ts` for native ES Modules compatibility.
*   **Prioritize Clean Design over Backward Compatibility**: The parser is in early-stage development. Always prefer refactoring, rewriting, or replacing old grammar rules with cleaner, more efficient designs over maintaining backward compatibility or adding compatibility shims for sub-optimal rules.
