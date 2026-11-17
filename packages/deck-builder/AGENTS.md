# Deck capabilities and existing account backend

Read the [specification](../../docs/deck-builder-spec.md) before changing behavior;
its section numbers remain stable for source comments. This package owns existing
authentication/accounts, Actor/SQLite capabilities, deck data, API, CLI and MCP.
Multiverse owns browser/account UX, build and process composition through public
`./host` and type-only `./contracts`; no cross-package private imports or second
account system. Interpretation belongs to external agents; no model calls/embeddings.

## Capability constraints

- HTTP and MCP pass an explicit verified `Actor` to the same services. Enforce
  tenant/owner/human checks below both doors, never only in routes/tools. Ownership
  is a relation, not an Actor kind. No default actor in tenant capabilities.
- `requireDeck`/`requireDeckOwner` return 404 for missing or unreadable targets,
  including writes; 403 is for forbidden actions on a readable target.
  `requireEditableDeck` includes ownership and lifecycle. Human-only actions also
  use `requireUi`; only the owner changes lifecycle, decides, imports or undoes.
- Agents edit Theorycrafted membership directly; Paper membership and all agent
  document/power-target changes need proposals. Archived is read-only until restored,
  except owner-controlled sharing. Editable rename/delete remain attributed.
- Membership writes use `applyChanges`: one atomic batch/revision/log entry.
  Slots/tags are organization, not membership. Primary purposes alone count toward
  targets; preserve secondary purposes and tags through parking, fork and undo.
  Slot/tag names cannot be card types/search prefixes; assignments stay deck-scoped.
- Card facts resolve exactly against installed data, never model memory/fuzzy
  matches. Full names win over face names; ambiguity requires Oracle ID. Retired
  cards remain resolvable. Images use stored URLs. Agent prose is linted on write.
  The server computes counts, deltas, checks and price estimates; do not duplicate them.
- Prices use `priceResolver(db, actor)` and whole cents with currency/store/date.
  Only UI changes source preferences. Public reads use viewer or explicit null
  (USD/lowest), never owner settings. Missing prices stay explicit. These are
  cheapest-playable-printing estimates, not valuations of owned copies.
- Have tags record persistent ownership with unknown quantities unless already
  known; removing a tag does not retract evidence. Source membership/Keep is not
  ownership or availability. See spec §§5.5 and 10.3 for correction/retry contracts.
- Agent writes retain principal/via/label and reach that principal's Activity.
  Draft amendment rights bind to verified token/client, not URL labels. Agents
  never delete individual memories or change owner-touched memories; supersede or
  retire them. Requested deck deletion also removes deck-scoped memories.
- Private reads, FTS, settings, focus, statistics, inbox, evidence and review queues
  are principal-scoped. `include_my_decks` is opt-in and means only that principal.
  Card-history lineage stops at unreadable/foreign-owner boundaries.
- Public viewers use slug-only `requireViewableDeck` and `publicDeckState` allowlist;
  numeric IDs remain owner-only. Visibility/publishing/log hiding are owner + UI.
  Never expose private tags, memories, proposals, ownership evidence or agent names.
- MCP is stateless; every request revalidates its bearer and every tool names its
  deck. Host/Origin validation precedes the SDK. Hosted mutating UI requests require
  configured Origin; forwarded headers require the explicit trusted-proxy setting.
  Hosted GET health alone is the minimal public exception; local health stays loopback.
- SSE is per principal; global job notifications reveal no tenant payload. Domain
  transactions include attribution and result construction. Retry keys, versions
  and response receipts follow spec §10.3; do not add transport-only write paths.
- Account export/deletion inventories must classify every table and grouped
  composite FK. Unknown tables fail closed. Preserve foreign content and spent
  invites with tombstone attribution; never delete by author/used_by. The last
  admin cannot delete itself. Never write new Activity after deletion.
- Use the shared reentrant SQLite gate per SQL query/transaction. Never reserve it
  across auth HTTP handlers, outbound I/O or open MCP streams. Auth SQL is generated
  offline and applied by ordered migrations; never run auth auto-migration.
- Verify external formats against their own sources and cite them in code.
  Tool descriptions/rendered text are public interfaces: inspect a real transcript
  after consequential changes, not just snapshots.

## Toolchain and commands

Use pinned Node through `sh scripts/engine-env.sh` from repository root. Install
locally with the package lock and `--workspaces=false --ignore-scripts`; never
replace the shared root installation. Node executes backend TypeScript directly
(`erasableSyntaxOnly`); no backend emit step or native dependency. Use the Database
adapter in `src/sqlite.ts`. Manifest/lock own dependency pins; new runtime
requirements need a stated PR reason. Existing personal tokens replace any need
for an auth api-key plugin. Auth CLI remains offline tooling.

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
container/runtime checks and Railway promotion. Never test against owner/live
hosted data, copy a live bare DB, promote a release or remove backup/evidence as
an incidental development step. Local mode is implicit user 1; hosted sessions
and MCP credentials retain the spec's distinct authorization boundaries.

## Focused test gate

From repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder run check --workspaces=false
```

Keep capability/auth permissions and tenant isolation, public allowlist privacy,
migration/content preservation, backup/restore, exact card resolution, membership/
undo, proposals, representative interop and a real MCP client smoke. Tests use
Node's runner. A representative gate is not exhaustive security acceptance.
Game runtime/DTOs live in `../game`; public `./application` supplies account/deck
authority and the existing SQL gate. Historical game migrations and deletion
handoffs remain here. Product route/inbox/architecture tests live in Multiverse; browser edits also run
its separate `check` and `build:web`, and shared presentation/session edits use
their own package gates. Prefer behavioral regressions over UI copy/CSS snapshots
or duplicate matrices. No native/SDK regeneration follows a backend-doc edit.

[Disposable browser tooling](browser-checks/README.md) owns integrated scripts and
source-backed actors/data. Plain-text Moxfield and Najeela printing fixtures are
still used there. Preserve failed results; [client evidence](../../docs/game-client-evidence.md)
retains exact accepted sources/omissions, not a current-HEAD claim.

## Related manuals

[Spec](../../docs/deck-builder-spec.md) owns behavior/compatibility;
[operations](OPERATIONS.md) owns operator procedures;
[MCP examples](../../docs/deck-builder-mcp-workflows.md) and the
[optional method skill](skill/commander-deck-building/SKILL.md) own agent workflows.
[Multiverse](../multiverse/AGENTS.md) owns browser changes and design constraints.
Update the owning section; do not append task reports, dependency inventories or
another copy of a contract here.
