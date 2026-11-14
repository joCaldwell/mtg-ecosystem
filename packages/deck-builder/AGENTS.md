# Deck capabilities and existing account backend

Read [the specification](../../docs/deck-builder-spec.md) before changing behavior.
Its section numbers remain stable for source comments. This package owns the
existing authentication/account implementation, Actor/SQLite capability services,
deck data, API, CLI and MCP. `multiverse` owns browser/account UX, navigation,
features, build and process composition, through public `./host` and type-only
`./contracts`. There is one account/database system and no migration from the move.
No model calls or embeddings; interpretation lives in the external agent.

## 🎯 The rules that explain most of the design

1.  **One capability layer, two front doors.** The UI's HTTP API and the MCP
    tools call the same service functions, each passing an `Actor` with a
    `user_id` principal, `via` (`ui` or `agent`) and attribution `label`.
    Ownership is a relation to the deck, never a kind of actor. Every
    permission and tenant filter is enforced *below* both doors.
    Nothing in `src/mcp/` or `src/app.ts` decides what an agent may do.
2.  **Lifecycle controls editing.** Agents edit Theorycrafted cards directly;
    Paper card changes and all brief/power-target edits require proposals.
    Archived is read-only for both actors; the owner alone changes lifecycle or
    restores an archive. Membership is enforced by `applyChanges`; all deck
    mutators use `requireEditableDeck` in `src/deck/service.ts`.
3.  **No invented card anywhere in the app.** Card references resolve exactly
    (`src/cards.ts`) or the whole call fails with suggestions; prose an agent
    stores is linted for `[[Card Name]]` on write. What an agent says in its
    own chat window is outside the app and is not covered.
4.  **If the app can compute it, nobody else derives it.** Counts, deltas and
    checks are returned with every read and every write.
5.  **Everything an agent writes is attributed and visible to its principal.**
    Store the author user, via and label beside the compact author string;
    UI attribution reads `handle via label`. Unapproved writes (memories,
    filing, Theorycrafted card edits) land in that user’s activity feed.
    A URL label grants no identity or UI privileges. Pending card/document
    proposals record `author_token_id`; amendment rights follow the verified
    credential, never its URL hint. Legacy import binds existing agent drafts
    to the migrated credential so outstanding local work stays editable.

---

Use Node 26 (`nvm use`; the package floor is >=24.12). The backend tsconfig enforces
`erasableSyntaxOnly`. Node runs the `.ts` files directly via type stripping — **there is no build
step for `src/`**. `node:sqlite` (built in, has FTS5) means no native deps. Use `Database`
from `src/sqlite.ts` (Node 26 rename; the same constructor on Node 24).
Connections have a 5-second busy timeout.
Runtime dependencies include `@modelcontextprotocol/server` (the official MCP
TypeScript SDK, v2 line) and `zod` (minimum 4.6.7 for the collection-validation
security patch). `@aws-sdk/client-s3` is the ops-only
exception for optional S3-compatible backup uploads; it is loaded only when
configured. A new runtime dependency needs a stated reason in the PR and an
entry here. Hosted authentication adds exact pins: `better-auth@1.7.13` for social
sign-in and cookie sessions; `@better-auth/mcp@1.7.13` for the OAuth 2.1
provider and RFC 9728 metadata; `@better-auth/cimd@1.7.13` for client
document registration; `@better-auth/oauth-provider@1.7.13` to pin the MCP
plugin’s provider peer to the documented version; `@better-auth/core@1.7.13`
for the canonical public-address policy used by the DNS-pinned CIMD
transport; `@better-auth/kysely-adapter@1.7.13` and `kysely@0.29.7`
for the official native dialect with query/transaction-scoped reservations; `jose@6.2.3` for JWKS JWT verification. The existing personal
tokens satisfy D7, so do not install the api-key plugin. No native deps.
The auth CLI is an offline schema-generation tool, not a runtime dependency. Prefer the platform and the existing set otherwise.


## 🚀 Commands

Run from this directory (`packages/deck-builder/`):

| Command | What it does |
| --- | --- |
| `npm run launch` | Start the app if needed and open it in the default browser |
| `npm start` | Build the UI and start the server in the background at :8787 |
| `npm stop` / `npm run restart` | Stop it / rebuild and restart it |
| `npm run status` / `npm run logs` | Is it up / last log lines (`-f` follows) |
| `npm run dev` | **Frontend loop**: API on :8787 + Vite HMR on :5173. Use :5173 |
| `npm run check` | Backend typecheck and tests; also run the Multiverse gate for browser changes |
| `npm test` | Tests only — `node --test`, **not** Vitest |
| `npm run seed:demo` | Create the sample deck if absent, through the first-run capability |
| `npm run refresh` | CLI job: stream cards, then prices. `-- --force` bypasses freshness; Settings only enqueues a child job |
| `npm run backup` | Online SQLite snapshot; 14 local files, optional S3 with 14 daily/6 monthly retention |
| `npm run restore -- <file>` | Verify and restore a snapshot while the server is stopped |
| `npm run search '<query>'` | Scryfall-syntax search from the CLI |
| `npm run mcp` | The stdio shim, for clients that only launch a command |

`DECKBUILDER_MODE` defaults to `local`: loopback, implicit UI user 1, no login.
`hosted` binds all interfaces and requires `DECKBUILDER_PUBLIC_ORIGIN`,
`BETTER_AUTH_SECRET` and a configured provider. UI APIs resolve Better Auth
sessions to `users.auth_id`; missing sessions return 401. Local mode exposes
no OAuth endpoints and never shows sign-in. `DECKBUILDER_PORT`,
`DECKBUILDER_DB` and `DECKBUILDER_DATA` override port, database and data folder.
`DECKBUILDER_TOKEN` is removed; credentials live hashed in `agent_tokens`.

See [OPERATIONS.md](OPERATIONS.md) for scheduled jobs, backup/restore drills,
job/process locks and environment variables. Never copy a live `.db` alone.
The Railway runbook there covers the private beta: repo-root Docker context,
production `release` versus staging `main`, one replica per environment,
claim-code removal, bucket restores and the Hobby native-backup limitation.
Hosted data is primary after go-live; laptop `data/` is development-only.
Operational code uses host-aware process locks, a 30-second deployment drain,
timestamped backups with 14 daily/6 monthly bucket retention and backup-age
status. `scripts/check-runtime.mjs` verifies the final image's imports and
exact pins; the Go context checker uses Moby's real ignore matcher. Production
code changes are promoted only by the owner, using an explicit verified SHA.
`scripts/docker-context-check` needs Go, runs outside the gate, and uses Moby's own matcher to verify Docker exclusions exactly.


## ⚠️ Constraints — these are load-bearing, not style

*   **Permissions live in the capability layer.** If a new capability must be
    deck-owner-only, call `requireDeckOwner(db, actor, deckId)` inside it,
    or `requireSelf` for a user-scoped record. `requireEditableDeck` includes
    the ownership check. Reads use `requireDeck(db, actor, deckId)` and return
    the caller role; unreadable records are 404 for reads and writes.
    PermissionError (403) is for forbidden actions on readable targets. Never guard only in a tool or route.
    Viewers (other users, their agents, anonymous) read a shared deck only by
    slug through `requireViewableDeck` and get only `publicDeckState`, an
    allow-list; every id-addressed capability stays the owner's (spec §18.3).
    Visibility, publishing and hiding log entries are owner + `requireUi`. Human-only actions additionally
    use `requireUi`: decision, undo, lifecycle, import, direct brief/target/profile
    edits. An agent acting for the deck owner still needs Paper proposals.
    Owners and agents may rename and delete editable decks. Only the owner
    classifies decks (`paper` / `theorycrafted` / `archived`, spec §5.6), since
    lifecycle governs permissions. Agent actions are attributed in Activity.
*   **A deck's contents change only through `applyChanges`.** It applies a
    batch atomically, bumps the revision once, and writes one log entry. Do
    not add a second write path to `deck_cards` for membership.
*   **Every tenant capability takes an explicit Actor.** Global card facts
    are shared; profile, global memories, settings/budgets, focus, stats, inbox,
    review queue, activity and ownership evidence are per principal. Never
    provide a default actor in a service. A parent/fork source must belong to
    the caller; lineage stops at unreadable boundaries.
*   **Cross-deck membership in search is off** unless the call sets
    `include_my_decks`; that always means the principal’s own decks.
*   **Card facts come from the database, never from model memory.** This
    applies to you: do not type a card name into code, a fixture, or a seed
    without checking it resolves.
*   **A price is the cheapest playable printing's, in whole cents, and is an
    estimate.** Decks hold cards, not printings, so nothing may present a
    price as what the owner's copy is worth. `priceResolver(db, actor)` supplies
    every personal card price, currency search, sourcing and proposal cost.
    Only UI may call `setPriceSource`; currency/store are per-user settings.
    Public reads use `priceResolver(db, viewer | null)`: null means
    USD/lowest without consulting owner settings. Wire amounts use `cents`
    plus currency/store/as_of metadata. Totals come
    from `estimateCost` and always report how many cards had no price.
*   **Retired cards stay resolvable.** Refresh never deletes a missing oracle ID.
    Hide it from general search, retain it in its deck context and flag its check.
    Images come from stored bulk URLs; never construct a Scryfall API image lookup.
*   **Exact name resolution only — never fuzzy.** A full-name match wins over
    a face-name match; what is still ambiguous needs an `oracle_id`.
*   **Agent prose is linted on write.** Reasons, memory text, brief and
    profile edits, shortlist notes: every `[[Card Name]]` must be a real card.
*   **Agents never delete individual memories.** Memories are superseded or
    retired and stay on disk. An agent cannot change a memory the owner wrote
    or edited (`owner_touched`). Deleting a deck, when requested, also deletes
    its deck-scoped memories. The `delete_deck` tool is marked destructive.
*   **The endpoint is stateless.** Every tool names its deck; nothing is kept
    between requests. Do not add per-connection state — the stdio shim and the
    2025-protocol fallback both rely on its absence.
*   **Host, Origin and token are checked in front of the SDK.** The SDK's
    handler validates none of them. `app.ts` applies the Host/Origin check to
    the UI API as well. Local UI requests build implicit user 1; hosted UI
    requests resolve the cookie session. Hosted mutating UI requests require
    the configured Origin. Forwarded headers are trusted only with
    `DECKBUILDER_TRUST_PROXY=1`. MCP resolves a per-user bearer on
    every request, with revoked/deleted-user credentials rejected immediately.
*   **Personal tokens are secrets shown once.** `dbk_` + 32 random bytes,
    SHA-256 at rest, prefix lookup and timing-safe hash comparison. Label,
    last-used (updated at most once a minute) and revoke live in Settings;
    token CRUD is UI-only. Local boot commits the legacy file’s hash for user
    1 before renaming the file to `stdio-token` (0600). The local stdio shim
    defaults to that file, or takes `--token-file` / `DECKBUILDER_TOKEN_FILE`;
    reading it never creates credentials.
*   **Streams are per user.** `notify(user_id, { kind, deck_id? })` reaches
    only that principal. Global job invalidations may notify all users and
    contain no tenant payload; HTTP refresh additionally requires admin.
*   **No model calls, including embeddings.** Memory search is FTS5 plus
    scope and card links.
*   **External formats get verified against their own documentation first.**
    The Archidekt research is cited in `src/deck/interop.ts`; the MCP
    behaviour in `src/mcp/http.ts`.
*   **Slots are optional.** Nothing — proposals included — may require one.
*   **Slots describe purpose; tags track sourcing and status.** `slot_id` is
    the optional primary purpose; `secondary_slot_ids` contains additional
    purposes. Targets count only primary copies. Secondary counts and total
    coverage are separate computed fields. A card cannot carry the same slot
    in both roles. Preserve both on parking, promotion, forks and undo.
*   **Slot and card type are independent axes.** `validateVocabName` rejects
    card types as slot or tag names in singular and plural.
*   **Tool text is an interface.** The MCP smoke test checks a real tool response.
    Change a description or a rendered line deliberately, then read a real
    transcript (spec §14, phase 6) before trusting it.


## Hosted accounts and environment

See spec §17 and OPERATIONS.md for the complete policy. `DISCORD_CLIENT_ID`
and `DISCORD_CLIENT_SECRET` enable Discord; `GOOGLE_CLIENT_ID` and
`GOOGLE_CLIENT_SECRET` together enable Google. `BETTER_AUTH_SECRET` is a
secret of at least 32 characters, generated with cryptographic randomness.
`DECKBUILDER_CLAIM_CODE` is a separate one-time owner bootstrap secret; it
must be at least 32 characters when set and only claims user 1 while auth_id is NULL. Remove it after the success log.
`DECKBUILDER_INVITES_REQUIRED` defaults to gated; only `0` disables the gate.
Hosted startup also requires non-secret `DECKBUILDER_OPERATOR_NAME` and
`DECKBUILDER_CONTACT_EMAIL`, served to legal pages from session bootstrap.
Secrets belong in the environment or ignored `.env`, never code or logs.
Use `node --env-file=.env ../multiverse/src/server.ts` from this package for explicit local env-file loading;
set DB/DATA to an ignored `.dev/` copy when testing hosted mode. Never migrate
or test against the owner's live data.

Better Auth schema comes from `npx auth@1.7.13 generate`, committed in
`src/migrations/auth-v12.sql` and executed by v12 in our migration transaction.
Never run its migrate command or runtime auto-migration. Native SQLite is
shared with the capability layer. Its official native dialect reserves the
connection per SQL query/transaction; service batches use the same reentrant
gate. Never hold that gate around an auth HTTP handler or outbound I/O.

Account lifecycle derives selectors from sqlite_master and grouped composite
foreign keys. Unknown tables fail closed unless explicitly global or linked
to a tenant. Add lifecycle coverage with any table. Export redacts credentials;
deletion removes owned records, OAuth state/grants and sessions without
writing new activity. Retain foreign content/spent invites with tombstone
user 0 attribution, never delete based on author/used_by. UI-only guards apply below HTTP. The last admin cannot
delete their account. OAuth drafts bind to `author_oauth_client_id` just as
personal-token drafts bind to `author_token_id`. URL labels never grant rights.

Hosted GET /api/health alone bypasses Host/Origin for Railway probes and
returns only ok/schema_version with no cookies/auth. Other hosted routes
keep the configured-origin boundary; local health remains loopback-only.
Local mode exposes neither legal pages nor their footer. OAuth disconnect
must remove refresh grants and preserve JWT issuance cutoffs on re-consent.
Handles reserve route words and may change only during the first-run picker.

Phase 4b adds human-only `createDemo` (src/demo.ts) and first-run voice
(src/preferences.ts). The sample is atomic, idempotent per user, has no global
memory/profile writes, and resolves all cards against installed data. The CLI
is a thin adapter and never replaces an existing sample. UI components call
only api/ clients, never fetch directly. TabletShell must call useWindowScroll;
keep the inherited phone and public contracts intact. See the product's
[design system](../multiverse/web/DESIGN.md) and
[architecture](../multiverse/web/ARCHITECTURE.md) for breakpoints and lazy-route checks.

## Focused test gate

Keep tests for tenant isolation and actor permissions, public-state privacy, migrations
and content preservation, backup/restore, card resolution, membership/undo, proposals,
import parsing, hosted sessions/headers and MCP smoke. Product architecture and
browser tests belong to Multiverse. Prefer a
small representative regression over matrices or UI copy/markup/CSS assertions.
Run `npm run check` before committing; browser regressions live separately in `browser-checks/`.

See [TESTING.md](TESTING.md) for current gate ownership, coverage and omissions.

## Related manuals

[Operations](OPERATIONS.md) owns environment, jobs, locks, online backup/restore,
claim lifecycle and release procedures. [Testing](TESTING.md) owns the backend gate;
[browser fixtures](browser-checks/README.md) exercise the product with disposable
actors/data. [MCP workflows](../../docs/deck-builder-mcp-workflows.md) and the
[optional skill](skill/commander-deck-building/SKILL.md) own client method/setup.
Browser edits use [Multiverse](../multiverse/AGENTS.md), never private backend imports.
Install locally with the package lock through pinned Node and `--workspaces=false`;
never overwrite the root shared installation.
