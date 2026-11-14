# Project Multiverse: agent entry point

Development is performed by AI agents. Josh supplies product direction and
unresolved product decisions; agents own routine implementation, verification,
coordination and reviewable handoffs. Establish the target package/path/subject;
ask only if it cannot be inferred. Read its manual before editing code.

| Package | Owns | Manual |
| --- | --- | --- |
| multiverse | Product account/session UX, navigation, browser features, build and process composition | [manual](packages/multiverse/AGENTS.md) |
| deck-builder | Existing authentication/account backend, Actor/SQLite deck services, API, CLI and MCP | [manual](packages/deck-builder/AGENTS.md) |
| game-ui | Shared permitted table/card/inspection display with scoped styles and host React | [manual](packages/game-ui/AGENTS.md) |
| game-client | Game decision/session adapters, recovery, qualified preview and standalone harness | [manual](packages/game-client/AGENTS.md) |
| oracle-parser | Supported TypeScript AST/archive subset; broader parsing paused | [manual](packages/oracle-parser/AGENTS.md) |
| card-data | Generated compiled Card IR; no hand edits | [data contracts](docs/data_schemas.md) |
| game-engine | Rust compiler, deterministic rules, finite Commander simulation/search | [manual](packages/game-engine/AGENTS.md) |
| game-server | Finite local Rust host and generated TypeScript SDK | [manual](packages/game-server/AGENTS.md), [SDK manual](packages/game-server/sdk/AGENTS.md) |

## Shared constraints

- TypeScript owns ingestion/parser, applications and SDK; Rust owns semantic
  compilation, deterministic rules, native simulation/search and hosting. This
  decision is complete; see [architecture](docs/architecture.md).
- Use exact card data for names, Oracle text and legality, never model memory.
  Engine work uses frozen local source fixtures and pinned rules. Verify external
  formats/rules against their source documentation and cite it in code.
- Compile complete supported semantics or reject explicitly. Wrong or silently
  incomplete interpretation is worse than rejection.
- The deck library is independent of Card IR/engine. The product uses public
  backend `./host` and type-only `./contracts`; game display uses public package
  exports. Preserve the [application boundaries](docs/multiverse-app.md).
- Preserve privacy and public compatibility (deck spec v3 D19): never drop other
  users' data; public MCP tools, shared URLs and token formats evolve additively
  or through deprecation. Internals may be rewritten freely. Product sign-in
  does not grant a Rust seat. Never use the legacy solo playtester as game-client guidance.
- Use package-specific npm/Cargo checks. Engine work uses [focused verification](docs/game-engine-verification-workflow.md)
  with explicit omissions, no automatic full matrix. Active Card IR schema is 1;
  capabilities and exact build/catalog identity fence supported behavior.
- Follow the [agent workflow](docs/game-engine-agent-workflow.md). Configure
  `git config --local commit.gpgsign false` for autonomous engine checkpoints;
  commit and push coherent checked work. Never force-push shared branches or merge incidentally.
- Preserve unrelated changes, source/evidence/worktrees and meaningful failures.
  Downloaded/generated outputs (`generated/`, `.scryfall-cache/`, `target/`,
  `dist/`, backend `data/`/`.dev/`) stay ignored. Ignored is not disposable:
  read [storage policy](docs/game-engine-storage.md) before cleanup.

[README](README.md) is the documentation map. Engine/server work starts at the
[current map](docs/game-engine-current.md) and [development commands](docs/game-engine-development.md).
Accepted D2c-d `394301d` excludes later increments; [evidence](docs/game-engine-evidence.md)
records source boundaries. Update owning canonical sections instead of creating
per-task plans, progress logs or audit reports. Raw receipts belong in ignored or
external evidence storage, with locations and omissions in the handoff.
