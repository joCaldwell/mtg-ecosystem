# Project Multiverse: agent entry point

All future development is performed by AI agents. Optimize tools and instructions
for autonomous, reproducible execution and explicit agent handoffs. Josh supplies
product direction and decisions that require his input; routine implementation,
verification and coordination are agent responsibilities. For engine work follow
the [agent workflow](docs/game-engine-agent-workflow.md).

The packages are independent and have different tools and constraints. Establish
the target before work: a named package, path or unambiguous subject is enough.
If it is unclear, ask before doing anything. Read the target package's `AGENTS.md`
before editing its code; that manual is authoritative for its package.

| Package | Purpose | Manual |
| --- | --- | --- |
| deck-builder | Active standalone Scryfall/SQLite deck application | [manual](packages/deck-builder/AGENTS.md) |
| oracle-parser | Supported TypeScript AST/archive subset; broader parsing paused | [manual](packages/oracle-parser/AGENTS.md) |
| card-data | Generated compiled Card IR, not hand-edited | — |
| game-engine | Rust compiler, deterministic rules, finite Commander matches and sampled search | [manual](packages/game-engine/AGENTS.md) |
| game-server | Finite local Rust host and generated TypeScript SDK | [manual](packages/game-server/AGENTS.md) |

For engine/server onboarding, use [the current map](docs/game-engine-current.md)
and [development commands](docs/game-engine-development.md). Current development
includes IR25 candidate work; the accepted D2c-d release at `394301d` does **not**
cover it. Exact status belongs in that map and the linked source-qualified reports,
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
- Prefer clean design over compatibility shims. The standalone deck-builder
  does not consume Card IR or the engine and must not wait for them.
- Use package-specific npm/Cargo checks. Engine verification favors focused,
  substantially faster checks with explicit omissions; no automatic full matrix.
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
