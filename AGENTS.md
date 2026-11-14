# Project Multiverse: agent entry point

All future development is performed by AI agents. Optimize tools and instructions
for autonomous, reproducible execution and explicit agent handoffs. Josh supplies
product direction and decisions that require his input; routine implementation,
verification and coordination are agent responsibilities. For engine work follow
the [agent workflow](docs/game-engine-agent-workflow.md).

The packages have explicit boundaries and different tools and constraints. Establish
the target before work: a named package, path or unambiguous subject is enough.
If it is unclear, ask before doing anything. Read the target package's `AGENTS.md`
before editing its code; that manual is authoritative for its package.

| Package | Purpose | Manual |
| --- | --- | --- |
| multiverse | Product application, account UX, navigation, deck editor and game entry | [manual](packages/multiverse/AGENTS.md) |
| deck-builder | Deck capabilities, existing account/API backend and MCP | [manual](packages/deck-builder/AGENTS.md) |
| game-ui | Shared permitted game display and scoped styling | [manual](packages/game-ui/AGENTS.md) |
| game-client | Game session/decision adapters, qualified preview and standalone harness | [manual](packages/game-client/AGENTS.md) |
| oracle-parser | Supported TypeScript AST/archive subset; broader parsing paused | [manual](packages/oracle-parser/AGENTS.md) |
| card-data | Generated compiled Card IR, not hand-edited | — |
| game-engine | Rust compiler, deterministic rules, finite Commander matches and sampled search | [manual](packages/game-engine/AGENTS.md) |
| game-server | Finite local Rust host and generated TypeScript SDK | [manual](packages/game-server/AGENTS.md) |

For engine/server onboarding, use [the current map](docs/game-engine-current.md)
and [development commands](docs/game-engine-development.md). The active Card IR schema stays at 1 while development has no external
consumers; capabilities and exact build/catalog identity enforce supported behavior.
The accepted D2c-d release at `394301d` does **not** cover later engine increments. Exact status belongs in that map and the linked source-qualified reports,
not repeated release counts here. Old onboarding is preserved in the
[historical index](docs/history/engine/index.md).

## Shared constraints

- TypeScript owns ingestion/parser, applications and SDK; Rust owns semantic
  compilation, deterministic rules, native simulation/search and hosting. The
  language decision is complete; see [architecture](docs/architecture.md) and
  [language boundaries](docs/runtime-language-decision.md).
- Card names, Oracle text and legality come from exact card data, never model
  memory. Engine work uses frozen local source fixtures. Verify external formats
  and rules against their source documentation and cite it in code.
- Compile complete supported semantics or reject them explicitly. A wrong AST
  or silently incomplete rule is worse than rejection.
- Prefer clean design over compatibility shims. The deck library does not consume
  Card IR or the engine and must not wait for them. The unified shell hosts game
  presentation through public package interfaces; see [application boundaries](docs/multiverse-app.md). Preserve
  its public compatibility policy (v3 D19): never drop other users’ data; from
  public sign-up, MCP tools, shared URLs and token formats change additively or
  through deprecation. Internals may be rewritten freely.
- Use package-specific npm/Cargo checks. Engine verification favors focused,
  substantially faster checks with explicit omissions; no automatic full matrix.
- Autonomous engine commits use `git config --local commit.gpgsign false` so
  interactive 1Password signing cannot block checkpoints or publication. Follow
  the [agent workflow](docs/game-engine-agent-workflow.md).
- Commit and push coherent engine checkpoints after relevant focused checks.
  Preserve unrelated changes, historical source/evidence/worktrees, and failed
  results. Never force-push shared branches or merge to main incidentally.
- Keep downloaded/generated outputs out of Git: `generated/`, `.scryfall-cache/`,
  `target/`, `dist/`, and deck-builder `data/`/`.dev/`. Ignored is not disposable;
  consult [engine storage policy](docs/game-engine-storage.md) before cleanup.

Shared references: [project structure](docs/project-structure.md),
[decisions](docs/decisions.md), [glossary](docs/glossary.md),
[Scryfall integration](docs/scryfall-integration.md),
[agent design](docs/agent_design.md), [deck-builder spec](docs/deck-builder-spec.md),
[parser design](docs/oracle_parser.md), and [data schemas](docs/data_schemas.md).
