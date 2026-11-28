# Project Multiverse: agent entry point

AI agents perform development. Josh supplies product direction and unresolved
product decisions; agents own routine implementation, verification, coordination
and reviewable handoffs. Establish the target package/path/subject (ask only if it
cannot be inferred) and read its manual before editing code.

| Package | Owns | Manual |
| --- | --- | --- |
| multiverse | Product account/session UX, navigation, browser features, build and process composition | [manual](packages/multiverse/AGENTS.md) |
| deck-builder | Existing authentication/account backend, Actor/SQLite deck services, API, CLI and MCP | [manual](packages/deck-builder/AGENTS.md) |
| game-ui | Shared permitted table/card/inspection display with scoped styles and host React | [manual](packages/game-ui/AGENTS.md) |
| game | Game browser, launch/recovery coordination, native supervision and shared contracts | [manual](packages/game/AGENTS.md) |
| oracle-parser | Supported TypeScript AST/archive subset; broader parsing paused | [manual](packages/oracle-parser/AGENTS.md) |
| card-data | Generated compiled Card IR; no hand edits | [data contracts](docs/oracle_parser.md#data-contracts) |
| game-engine | Rust compiler, deterministic rules, finite Commander simulation/search | [manual](packages/game-engine/AGENTS.md) |
| game-server | Finite local Rust host and generated TypeScript SDK | [manual](packages/game-server/AGENTS.md), [SDK manual](packages/game-server/sdk/AGENTS.md) |

## Shared constraints

- TypeScript owns ingestion/parser, applications and SDK; Rust owns semantic
  compilation, deterministic rules, native simulation/search and hosting. This
  decision is complete ([architecture](README.md#system-architecture)).
- Use exact card data for names, Oracle text and legality, never model memory.
  Engine work uses frozen local source fixtures and pinned rules. Verify external
  formats/rules against their source documentation and cite it in code.
- Compile complete supported semantics or reject explicitly; wrong or silently
  incomplete interpretation is worse than rejection.
- The deck library is independent of Card IR/engine. The product uses public
  backend `./host` and type-only `./contracts`; game display uses public package
  exports ([application boundaries](packages/multiverse/web/ARCHITECTURE.md#application-boundaries)).
- Preserve privacy and public compatibility (deck spec v3 D19): never drop other
  users' data; public MCP tools, shared URLs and token formats evolve additively
  or through deprecation; internals may be rewritten freely. Product sign-in
  grants no Rust seat. Never use the legacy solo playtester as game-client guidance.
- Build first: the cheapest relevant compile check and one directly affected
  witness when useful. Defer broad qualification and extra reviews unless requested
  or needed to diagnose a failure; [focused verification](docs/game-engine-development.md#choose-focused-checks)
  covers evidence and omissions, and package manuals own commands. Active Card IR
  schema is 1; capabilities and exact build/catalog identity fence support.
- Follow the [agent workflow](docs/game-engine-development.md#enter-a-task). Configure
  `git config --local commit.gpgsign false` for autonomous engine checkpoints;
  commit and push coherent checkpoints. Never force-push shared branches or merge incidentally.
- Preserve unrelated changes, source/evidence/worktrees and meaningful failures.
  Downloaded/generated outputs (`generated/`, `.scryfall-cache/`, `target/`,
  `dist/`, backend `data/`/`.dev/`) stay ignored. Ignored is not disposable: read
  the [storage policy](docs/game-engine-storage.md) before cleanup.

[README](README.md) is the documentation map. Engine/server work starts at the
[current map](docs/game-engine-current.md) and [rework plan](docs/game-engine-rework.md), then [development](docs/game-engine-development.md)
(`check:engine`, `check:engine:gate`, `check:engine:tooling`), then
[mechanics](docs/game-engine-mechanics.md) ([board](docs/game-engine-board-mechanics.md)) and [Kernel IR](docs/game-engine-catalogs.md#kernel-ir).

## Documentation and handoff

Each contract has one canonical owner in the [README map](README.md#documentation-map).
Package manuals state local constraints and link that owner; examples explain use
without duplicating types, inventories or current status. Update the owner in
place; never create per-task plans, progress logs or audit reports. Completed
plans, task audits and chronological release logs belong in Git history. Evidence
guides retain exact source/acceptance boundaries and proof locations, not a
transcript of every check; raw receipts belong in ignored or external evidence
storage, with locations and omissions in the handoff. Preserve referenced raw evidence.

`npm run check:docs` (or `python3 scripts/check-docs.py`, no Node required) checks
all maintained `.md`, `.markdown` and `.mdx` files, including package manuals and
skills: at most **40 files, 60,000 total words and 9,000 words per file**, fenced
examples included. Run it locally before every PR; engine checks also run it.
Consolidate before adding a document; routine feature work must not raise these
ceilings, and any deliberate budget change must explain the durable ownership need
in its PR.

Use existing pinned tools through `sh scripts/engine-env.sh`; bootstrap only when
dependencies are missing/broken. Application packages use their local locks and
`--workspaces=false --ignore-scripts`, never a shared root reinstall. Run focused
owning checks and `git diff --check`. Publish a draft PR with exact checks,
omissions and overlap, marking work provisional when qualification is deferred.
