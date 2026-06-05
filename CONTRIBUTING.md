# Contributing to the MTG Ecosystem

Thank you for your interest in contributing to the MTG Ecosystem (Project Multiverse)! We are building a layered, agent-first digitalization of Magic: The Gathering from the ground up, starting with **Layer 0: Oracle Text Parser**.

Because the rules of MTG are incredibly complex and precise, we enforce high standards for code quality, documentation, and test coverage.

---

## 🛠️ Prerequisites

To set up and run the project, you need the following installed:

1. **Node.js** (v20.x or higher recommended) & **npm** (v10.x or higher)
2. **Java JDK** (v11 or higher) — Required by the ANTLR4 compiler tool to generate the parser code.
3. **Git**

---

## 🚀 Getting Started

Follow these steps to set up your local development environment:

1. **Clone the repository**:
   ```bash
   git clone https://github.com/josh/mtg-ecosystem.git
   cd mtg-ecosystem
   ```

2. **Install dependencies**:
   Installs root development tools and links packages using npm workspaces:
   ```bash
   npm install
   ```

3. **Generate the parser**:
   Compile the `.g4` grammar files into TypeScript:
   ```bash
   npm run generate-parser
   ```

4. **Run the test suite**:
   Ensure everything is compiled and working:
   ```bash
   npm test
   ```

---

## 🔮 Parser Development Workflow

When contributing changes to the Layer 0 parser (adding support for new keyword abilities, new effects, or spelling variations), follow this flow:

### 1. Update the ANTLR Grammar
The ANTLR grammar defines the MTG card language. It is split into two files under `packages/oracle-parser/grammar/`:
- `MTGLexer.g4`: Defines tokens (e.g., numbers, symbols like `{T}`, names like `destroy`, `target`).
- `MTGParser.g4`: Defines rules and structures (e.g., `activatedAbility`, `triggeredAbility`, `durationModifier`).

If you're adding support for a new mechanic, first define its syntactic pattern in `MTGParser.g4`.

### 2. Regenerate the Parser Runtimes
Once grammar files are modified, generate the TypeScript parser:
```bash
npm run generate-parser
```
This updates the files in `packages/oracle-parser/generated/`. **Do not edit these files directly**, as they are auto-generated and gitignored.

### 3. Update the TypeScript AST Types
Define your new AST node representation in `packages/oracle-parser/src/ast/types.ts`.
- Ensure nodes use discriminated union types (e.g., `kind: "keyword"` or `effect_type: "gain_life"`) for type safety.

### 4. Implement Visitor Logic
Modify `packages/oracle-parser/src/visitor/ASTBuilder.ts` to map the new ANTLR parse tree nodes to your typed TypeScript AST nodes.
- Extend the visitor class by overriding the generated `visit[RuleName]` methods.

### 5. Write Tests & Validate
Add test cases in `packages/oracle-parser/tests/parser.test.ts` with cards containing the new mechanic.
- Run tests: `npm test`
- If you're adding or changing AST formats, verify snapshot tests, and run:
  ```bash
  npm run test -- -u
  ```
  to update snapshots if the changes are intentional.

---

## 📡 Ingestion and Set Compilations

To run the full pipeline and test the parser against Scryfall card data:

1. **Ingest Scryfall bulk data**:
   Fetches the latest card definitions and saves them to a local cache folder (`.scryfall-cache/`):
   ```bash
   npm run ingest
   ```

2. **Compile Card IR**:
   Runs the normalization, parses all cards, and outputs per-set JSON files in `packages/card-data/sets/`:
   ```bash
   npm run build-ir
   ```

3. **Check Coverage**:
   Run the mechanic coverage script to see how many of the 27,000+ MTG cards are parsing without errors:
   ```bash
   npm run test packages/oracle-parser/tests/coverage.test.ts
   ```

---

## 🎨 Code Style and Naming Conventions

We rely on strict linting and formatting rules to keep the monorepo clean.

- **Coding Standard**: Strict TypeScript. Avoid using `any` or disabling type checks.
- **Naming Conventions**:
  - **Directories**: `kebab-case` (e.g., `oracle-parser`, `card-data`).
  - **TypeScript Files**: `camelCase` or `kebab-case` (e.g., `ASTBuilder.ts`, `ir-emitter.ts`).
  - **AST Nodes**: `PascalCase` (e.g., `TriggeredAbility`, `ModifyPT`).
  - **Grammar Files**: `PascalCase` starting with `MTG` (e.g., `MTGLexer.g4`, `MTGParser.g4`).
  - **JSON Set files**: `UPPERCASE` set code (e.g., `LEA.json`, `MH3.json`).
- **Formatting**: Run `npm run format` (Prettier) before committing.

---

## 📝 Pull Request Checklist

Before submitting a PR, make sure you do the following:

1. **Tests pass**: Run `npm test` and ensure all tests are green.
2. **Grammar is clean**: Ensure your ANTLR changes don't introduce grammar ambiguities or infinite recursions.
3. **No generated code committed**: Ensure files in `packages/oracle-parser/generated/` are not tracked by Git.
4. **Documentation updated**: If you added an architectural pattern, update [docs/architecture.md](docs/architecture.md) or [docs/oracle_parser.md](docs/oracle_parser.md) as necessary.
5. **No reminder text parsing**: Ensure reminder text is stripped in the normalizer before parsing — the parser must understand the keyword, not parse the parenthetical helper text.

---

## 📚 Documentation Reference

Ensure you familiarize yourself with the design documents in `docs/`:
- [Architecture Plan](docs/architecture.md)
- [Oracle Parser Design](docs/oracle_parser.md)
- [Project Structure Layout](docs/project-structure.md)
- [Glossary](docs/glossary.md)
