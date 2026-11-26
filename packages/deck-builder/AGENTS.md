# Deck capabilities and existing account backend

Read the [specification](../../docs/deck-builder-spec.md) before changing behavior;
its section numbers remain stable for source comments. This package owns existing
authentication/accounts, Actor/SQLite capabilities, deck data, API, CLI and MCP.
Multiverse owns browser/account UX, build and process composition through public
`./host` and type-only `./contracts`; no cross-package private imports or second
account system. Interpretation belongs to external agents; no model calls/embeddings.

## Capability constraints

Where to enforce the spec's contracts:

- HTTP and MCP pass an explicit verified `Actor` to the same services. Enforce
  tenant/owner/human checks below both doors, never only in routes/tools; no
  default actor in tenant capabilities. `requireDeck`/`requireDeckOwner` return 404
  for missing or unreadable targets (including writes), 403 for forbidden actions on
  readable ones. `requireEditableDeck` includes ownership and lifecycle; human-only
  actions also use `requireUi`.
- Membership writes use `applyChanges`: one atomic batch/revision/log entry.
  Organization (slots/tags) is not membership (spec §5.4).
- Resolve card facts exactly against installed data (spec §4.3); lint agent prose on
  write. The server computes counts, deltas, checks and price estimates
  (`priceResolver(db, actor)`, spec §4.4); never duplicate them elsewhere.
- Public viewers go through slug-only `requireViewableDeck` and the
  `publicDeckState` allowlist (spec §18.3); never trim an owner composite.
- Private reads, FTS, settings, focus, statistics, inbox, evidence and review queues
  are principal-scoped; card-history lineage stops at unreadable/foreign owners.
- MCP is stateless and revalidates its bearer per request; Host/Origin validation
  precedes the SDK; forwarded headers require the explicit trusted-proxy setting.
- Domain transactions include attribution and result construction; retry keys and
  versions follow spec §10.3. Do not add transport-only write paths.
- Account export/deletion inventories classify every table and grouped composite FK;
  unknown tables fail closed; never delete by author/used_by (spec §17.5).
- Use the shared reentrant SQLite gate per query/transaction, never across auth HTTP
  handlers, outbound I/O or open MCP streams. Auth SQL is generated offline and
  applied by ordered migrations; never run auth auto-migration.
- Verify external formats against their sources and cite them in code. Tool
  descriptions/rendered text are public interfaces: inspect a real transcript after
  consequential changes, not just snapshots.

## Toolchain and commands

Use pinned Node through `sh scripts/engine-env.sh` from repository root. Install
locally with the package lock and `--workspaces=false --ignore-scripts`; never
replace the shared root installation. Node executes backend TypeScript directly
(`erasableSyntaxOnly`); no emit step or native dependency. Use the Database adapter
in `src/sqlite.ts`. New runtime dependencies need a stated PR reason; personal
tokens replace any auth api-key plugin.

Commands below run from `packages/deck-builder` under the pinned launcher:

| Command | Purpose |
| --- | --- |
| `npm run launch` / `npm start` | Build/start the product; launch also opens browser |
| `npm stop` / `npm run restart` / `npm run status` / `npm run logs` | Local product process manager |
| `npm run dev` | API :8787 and product HMR :5173 |
| `npm run check` / `npm test` | Backend typecheck + Node tests / tests only |
| `npm run seed:demo` | Human-only idempotent sample capability |
| `npm run refresh -- [--force]` | CLI ingestion/prices; Settings only enqueues |
| `npm run backup` / `npm run restore -- FILE` | Online snapshot / stopped-server restore |
| `npm run search -- QUERY` / `npm run mcp` | Scryfall-syntax CLI search / stdio shim |

[Operations](OPERATIONS.md) owns environment/secrets, jobs/locks, backup/restore,
container checks and Railway promotion. Never test against owner/live hosted data,
copy a live bare DB, promote a release or remove backup/evidence as an incidental
development step.

## Focused test gate

From repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder run check --workspaces=false
```

Keep capability/auth permissions and tenant isolation, public allowlist privacy,
migration/content preservation, backup/restore, exact card resolution, membership/
undo, proposals, representative interop and a real MCP client smoke. A
representative gate is not exhaustive security acceptance. Prefer behavioral
regressions over UI copy/CSS snapshots or duplicate matrices.

Game runtime/DTOs live in `../game`; public `./application` supplies account/deck
authority and the SQL gate, while historical game migrations and deletion handoffs
remain here. Product route/inbox/architecture tests live in Multiverse; browser
edits also run its `check` and `build:web`, and shared presentation/session edits
use their own package gates. No native/SDK regeneration follows a backend-doc edit.

[Disposable browser tooling](browser-checks/README.md) owns integrated scripts and
source-backed actors/data. Preserve failed results; [client evidence](../../docs/game-client.md#reproducibility-and-current-acceptance)
retains exact accepted sources/omissions, not a current-HEAD claim.

## Related manuals

[MCP examples](../../docs/deck-builder-mcp-workflows.md) and the
[optional method skill](skill/commander-deck-building/SKILL.md) own agent workflows;
[Multiverse](../multiverse/AGENTS.md) owns browser changes. Update the owning
section; do not append task reports, dependency inventories or contract copies here.
