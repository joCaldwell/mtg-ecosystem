# Local operations

Use Node 26 (`nvm use`, `.nvmrc`). The package declares Node >=24.12 for
stable native TypeScript stripping; `src/sqlite.ts` uses the renamed Node 26
`Database` class and the equivalent constructor on Node 24. All connections
use WAL, foreign keys and a 5-second SQLite busy timeout.

Run commands from `packages/deck-builder`. No build is needed for server
TypeScript. `npm start` builds the browser assets and starts the local server;
`npm stop`, `npm run status`, and `npm run logs` manage the laptop launcher.
Local mode (the default) binds `127.0.0.1` and builds implicit user 1 for UI
requests without a login. `DECKBUILDER_MODE=hosted` binds all interfaces and
requires `DECKBUILDER_PUBLIC_ORIGIN`, `BETTER_AUTH_SECRET` and provider
credentials. Cookie sessions authorize UI requests; personal tokens or OAuth
access tokens authorize MCP. Local mode is unchanged.

## Card refresh and scheduled jobs

The serving process schedules work; it never ingests bulk data itself.

- `node src/cli.ts refresh` (or `npm run refresh`) opens a separate database
  connection, refreshes oracle cards, then prices. `--force` bypasses cache
  freshness and the unchanged-price check. The Settings button enqueues the
  same child command without `--force`, returns immediately, and lets the app
  remain usable.
- Oracle downloads live in `DATA_DIR/.scryfall-cache/oracle-cards.json` and
  are reused for 24 hours. This cache is independent of the parser's cache.
  Default-printing prices stream from the download and are not kept on disk.
- Refresh runs on boot when card or price data is stale/missing and daily
  thereafter. A failed refresh retries one hour after its last run, still
  respecting the active-job and shared-lease checks. Backup runs at 02:00 in
  the server's local timezone; an overdue backup runs on the next scheduler
  tick (30 seconds). Only one job runs at
  a time; repeated queued requests coalesce.
- `GET /api/settings` and Settings show each job's last run, result, next run,
  active job and queue. Child stdout/stderr go to the server log. CLI jobs
  write the same persistent status. CLI-triggered runs become visible on
  the next Settings read; scheduled completions also notify the live stream.
- `meta['job:lock']` is an atomic SQLite lease shared by refresh and backup,
  including manually launched commands. Locks older than two hours are
  reclaimed; release only removes the holder's own lease. A failed command
  reports failure and releases its lease. If a process is killed, wait for
  lease expiry before retrying; do not delete a lock while a job is alive.
- Oracle JSON is parsed one object at a time and staged on disk, with
  savepoints every 500 cards. Malformed/duplicate/truncated input leaves live
  facts untouched before publishing. Fewer than 90% of the currently active
  cards is refused. Retired rows do not inflate future truncation thresholds.
  Validated facts publish in batches; names/FTS and the freshness marker are
  rebuilt together at the end. Readers can see a mix of old/new facts during
  publication; a failed publish is retryable and never deletes deck contents.
- Missing oracle IDs are retained with `retired=1`, including names and
  images, and remain exactly resolvable. Search excludes them unless the
  named deck already contains them (including its maybe list). Deck checks
  flag them for manual identity verification. Returning IDs become active.
- Prices load into `card_prices_staging` in batches, then swap with the live
  price table and its as-of timestamp in one short transaction. Failed
  staging leaves the old prices usable.

`GET /api/health` returns `{ ok, schema_version, cards, last_ingest }` without
credentials. Host/Origin protection still applies. It is a liveness/data-date
check, not proof that Scryfall or off-box storage is reachable.

## Consistent backups

```sh
npm run backup
# equivalent: node src/cli.ts backup
```

This uses Node's online `sqlite.backup()` API, including committed WAL
content, writes a temporary file, then renames it to
`DATA_DIR/backups/deck-builder-YYYY-MM-DD.db` (UTC date). A second backup on
the same UTC day replaces that day's snapshot. Keep the newest 14 dated
files, then optionally upload the new snapshot. Never copy only
`deck-builder.db` while the server is running: committed data may be in its
WAL sidecar.

At roughly 60 MB per snapshot, 14 local backups use about 840 MB (roughly
850 MB allowing for growth); stored image URLs will increase that as the
card database grows. Allow space for another snapshot while one is being
written, plus the live database, WAL and download cache. Retention remains
14 for all modes; operators who need less local disk use can prune older
local snapshots between runs. The next backup still keeps at most 14 dated
files. This does not change off-box bucket lifecycle policies.

To upload each snapshot to an S3-compatible bucket, configure all four:

| Variable | Purpose |
| --- | --- |
| `BACKUP_S3_ENDPOINT` | Bucket provider's S3 endpoint |
| `BACKUP_S3_BUCKET` | Existing bucket name |
| `BACKUP_S3_ACCESS_KEY` | Access key (secret) |
| `BACKUP_S3_SECRET_KEY` | Secret key (secret) |

The ops-only `@aws-sdk/client-s3` dependency uploads to
`backups/deck-builder-YYYY-MM-DD.db`, with region `auto` and path-style
addressing. Partial configuration or an upload failure fails the job while
keeping the local snapshot. Store credentials in the process environment,
never in source, commands copied into issues, or screenshots. Remote-object
retention is the bucket operator's lifecycle policy; local retention is 14.

## Restore drill

Restoring replaces the destination database. Take a current backup first,
then stop the server. Restore validates SQLite integrity, foreign keys and
the supported schema on a temporary online copy, reports the deck count,
checkpoints the stopped destination, and atomically renames the validated
copy into place. A running server or active refresh/backup blocks restore.
The database-specific `.server-lock` file is shared by startup and restore;
a dead PID is reclaimable. Restored job leases are cleared and interrupted
job statuses are marked failed. Tokens now live hashed inside SQLite: restoring
a snapshot restores its credential/revocation state too. Revoke credentials
again after an older restore when necessary; a once-shown secret cannot be
recovered from a backup.

```sh
npm run backup
npm stop
node src/cli.ts restore data/backups/deck-builder-2026-10-25.db
npm start
curl --fail http://127.0.0.1:8787/api/health
```

For a nondestructive rehearsal, point the restore at a separate destination:

```sh
DECKBUILDER_DB=/tmp/deck-builder-restore.db node src/cli.ts restore data/backups/deck-builder-2026-10-25.db
DECKBUILDER_DB=/tmp/deck-builder-restore.db DECKBUILDER_DATA=/tmp/deck-builder-drill DECKBUILDER_PORT=8799 node src/server.ts
curl --fail http://127.0.0.1:8799/api/health
```

Open `http://localhost:8799`, compare the deck count and a deck's contents,
then stop the drill process. Run only one server for a database, and stop
older versions that predate the process-lock mechanism before restoring.

## Environment and tokens

| Variable | Default / meaning |
| --- | --- |
| `DECKBUILDER_DATA` | Package `data/`; cache and backups live here |
| `DECKBUILDER_DB` | `DATA_DIR/deck-builder.db`; also determines server lock path |
| `DECKBUILDER_PORT` | `8787` |
| `DECKBUILDER_WEB_PORT` | `5173` in the dev script |
| `DECKBUILDER_TOKEN_FILE` | Optional private credential file for the stdio shim |
| `DECKBUILDER_MODE` | `local`; or `hosted` for sessions and OAuth |
| `DECKBUILDER_PUBLIC_ORIGIN` | Required in hosted mode, e.g. `https://decks.example` (no trailing slash) |
| `BACKUP_S3_*` | Optional off-box backup credentials above |

`DECKBUILDER_TOKEN` has been removed. Settings creates per-user tokens with
labels, shows each secret exactly once, lists prefix/last-used/revoked state,
and revokes individually. Newly minted secrets use `dbk_` plus 32 random
bytes in base64url. SQLite stores SHA-256 hashes only. Verification reads the
prefix, compares hashes with `timingSafeEqual`, and updates last-used at most
once a minute. Revocation takes effect on the next MCP request; other clients
keep working. Neither Settings reads nor server boot mint a token.

On **first local boot**, the server imports an existing `DATA_DIR/agent-token`
file into `agent_tokens` for user 1, labelled “Migrated local token”. It commits
the hash before renaming the file to `DATA_DIR/stdio-token` with mode 0600, and retries safely by hash after a crash.
The old secret remains valid, so existing Claude Code and Codex HTTP configs
keep working. Existing user-1 agent proposals/edits bind to that credential
within the import transaction, so their amendments continue to work too.
New drafts record the verified token id; changing a URL label grants no rights. This migration preserves the old secret format; new tokens use
`dbk_`. Back up before upgrading; never run the migration against a live file
for a rehearsal. Use an online SQLite backup to a temporary destination, copy
the legacy file privately there, and set both `DECKBUILDER_DB` and
`DECKBUILDER_DATA` to that copy.

In local mode, no-argument stdio clients read `DATA_DIR/stdio-token`, preserving
existing configurations on first v11 boot. After restoring a pre-v11 backup,
local boot re-imports that saved file into the newly migrated token table;
existing v11 revoked records are never reactivated. If neither file exists,
create a token in Settings and save it privately. Hosted Settings omits local
stdio/tunnel snippets.

The stdio shim cannot recover a secret from its hash. Create a token in
Settings, save the once-shown value in a private file (`chmod 600`), then use:

```sh
node /absolute/path/to/packages/deck-builder/src/mcp/stdio.ts --name my-agent --token-file /private/path/to/token
```

Secrets in `--token` argv are rejected. `--token-file` takes precedence over
`DECKBUILDER_TOKEN_FILE`, then local mode defaults to `DATA_DIR/stdio-token`.
Missing credentials
fail clearly; reads never create or rotate anything. Settings returns an
absolute shim path, uses placeholders until creation, and clears the secret
from its component after “I saved it” or leaving the page. No browser storage
holds it. The `/mcp/<label>` suffix remains attribution only, never a
principal selector or a way to become UI. Missing bearers receive a 401
resource-metadata challenge; hosted mode supplies metadata and JWKS verification.

Schema 11 creates `users` and assigns all old records to user 1 (`owner`,
`Owner`, admin, no auth id). Card data/jobs remain global; personal documents,
memories, settings, focus, inbox, review queue, activity and evidence are scoped
to that user. The migrations run as ordered steps within one `BEGIN IMMEDIATE` transaction,
re-reading the schema version under the write reservation so concurrent
server/CLI openers serialize. Foreign keys are checked before commit; failures
name the first five offending tables/rowids/parents and roll back all steps.
Unsupported databases are refused before changing journal/schema state.
Historical `owner (from v1)` attribution is UI; `migration` displays as
“System migration”, with the original compact author text retained. See spec
§17 for safe claim-code bootstrap and the hosted account lifecycle.

## Phase 0 verification (2026-10-25)

Automated coverage includes body/content-type/number validation, cross-deck
rulings, static 404s and cache headers, SSE heartbeat, image facts, truncated
streams, retirement, lock reclaim, backup/restore with WAL, retention, memory
scope/permissions, atomic accept-all and client HTTP errors. The live drill used an isolated copy, with the server on 8798:
`DECKBUILDER_DATA=<copy> npm run refresh -- --force` completed in 12.95 s,
35,134 active cards / 33,327 priced, child peak RSS 246.21 MiB. All 124 mixed
health/decks/search requests succeeded (max 6.55 ms). `npm run backup` while
serving, followed by `DECKBUILDER_DB=<copy>/restored.db node src/cli.ts restore
<copy>/backups/deck-builder-2026-10-25.db`, restored 19 decks; a server on 8799
served them and returned healthy status. The phase PR records browser checks.
S3 upload is covered against a local HTTP bucket stub; delivery to a real
bucket requires configured credentials and remains unverified.

## Hosted bootstrap, secrets and provider redirects

Every variable Phase 3 needs is listed here. The Node service reads the
process environment; `.env` is loaded only when explicitly requested with
`node --env-file=.env src/server.ts`. Never print `.env`, tokens, OAuth
responses, or provider credentials. Local `.env` is ignored and mode 0600.

| Variable | Secret? | Hosted setting |
| --- | --- | --- |
| `DECKBUILDER_MODE` | No | `hosted` |
| `DECKBUILDER_PUBLIC_ORIGIN` | No | Exact `https://<railway-domain>`, no slash |
| `DECKBUILDER_PORT` | No | `8787`; set Railway `PORT=8787`/target port to match |
| `DECKBUILDER_DATA` | No | `/data` (persistent volume) |
| `DECKBUILDER_DB` | No | Optional; defaults to `/data/deck-builder.db` |
| `DECKBUILDER_TRUST_PROXY` | No | `1` only behind the trusted Railway ingress |
| `DECKBUILDER_INVITES_REQUIRED` | No | Keep `1` for beta; only `0` opens signup |
| `BETTER_AUTH_SECRET` | Yes | Random stable value, at least 32 characters |
| `DECKBUILDER_CLAIM_CODE` | Yes | Random bootstrap code, remove after owner claim |
| `DISCORD_CLIENT_ID` | No | Discord application ID |
| `DISCORD_CLIENT_SECRET` | Yes | Discord application secret |
| `GOOGLE_CLIENT_ID` | No | Optional; Google requires both variables |
| `GOOGLE_CLIENT_SECRET` | Yes | Optional Google secret |
| `BACKUP_S3_ENDPOINT`, `BACKUP_S3_BUCKET` | No | Optional off-volume backup destination |
| `BACKUP_S3_ACCESS_KEY`, `BACKUP_S3_SECRET_KEY` | Yes | Required with the backup destination |

`DECKBUILDER_TOKEN_FILE` is a private-file path for a local stdio client, not
a service credential. `DECKBUILDER_WEB_PORT` is dev-only. No Sentry integration
is added in this phase. No Google provider button appears until both Google
credentials exist.

Set `DECKBUILDER_CLAIM_CODE` before exposing the service. Open `/signin`, enter
that code in **Invite or owner claim code**, and sign in with Discord. This
links user 1 and its decks to that identity. A stranger cannot claim it by
arriving first; if the claim variable is unset, only invited new users can
register. The comparison is timing-safe. Once user 1 has an auth_id, the code
is dead even if configured. The single success log says to remove it; remove
the environment variable and redeploy. Do not reset auth_id to reuse a code.
This phase generates a random local test code in ignored `.env`; Phase 3's
operator must set a different real service code.

Sign in as an admin and open Settings → Admin → Create invite. Copy the
unused code privately to the intended tester. They enter it on `/signin`;
registration consumes it once. Existing accounts sign in with an empty field.
Invited users have empty accounts and cannot see the owner's decks. A
colliding provider handle opens a first-run picker. The last admin cannot
delete their account because someone must remain to run jobs/manage invites.

Before using a new Railway domain, add this **exact redirect** in Discord's
Developer Portal → application → OAuth2 → Redirects:
`https://<railway-domain>/api/auth/callback/discord`. Keep the localhost
redirect `http://localhost:8787/api/auth/callback/discord` for isolated tests.
For Google, add `https://<railway-domain>/api/auth/callback/google` to its
OAuth client's authorized redirect URIs. Set the service public origin to
the same domain. A domain change changes the issuer/resource: reconnect OAuth
clients and sign in again rather than accepting tokens for the old audience.

Keep BETTER_AUTH_SECRET stable: it protects cookies and encrypts stored
provider tokens. Changing it invalidates those credentials.

HTTPS uses Secure, HttpOnly, SameSite=Lax cookies with no cross-domain cookie.
Loopback HTTP development deliberately omits Secure so the supplied Discord
callback works. TLS terminates at Railway; the public origin still stays
HTTPS. The application trusts forwarded host/protocol/IP only when
`DECKBUILDER_TRUST_PROXY=1`; configure exactly one ingress that overwrites
those headers. Do not expose a second untrusted route to the container.
Host and browser Origin must match the configured public origin. Auth uses a
fixed base URL, never a forwarded-header-selected issuer.

`GET /api/health` is unauthenticated and returns shared liveness/schema/card
freshness data. Through a proxy it accepts the configured Host or a trusted
forwarded public host/protocol. **Railway's native checker uses
`Host: healthcheck.railway.app`; our single-public-host rule rejects that
with 403.** Phase 3 must arrange a trusted health-check relay that rewrites
that Host to the public domain (or explicitly amend the host rule), and test
it before switching traffic. Do not silently widen the allowlist here. See
[Railway health-check hostname documentation](https://docs.railway.com/deployments/healthchecks#healthcheck-hostname).
The public path remains `/api/health`; deployment port must match `PORT`.

The service intentionally runs one replica: rate limits are in-memory and
reset at restart. `src/ratelimit.ts` is the canonical limit table. A future
replica rollout needs shared limit state and a shared transaction strategy.
CIMD transport uses Better Auth's public-address policy and pins one checked
DNS answer, preferring IPv4 to work without outbound IPv6. It validates all
answers and never follows redirects. Do not substitute ordinary fetch.

## Account lifecycle and auth schema

Settings → Account exports JSON (all owned tables plus Archidekt deck text)
and deletes after typed-handle confirmation. Export excludes credential
secrets. Deletion removes auth sessions/accounts, OAuth grants/codes, personal
tokens and all application rows, with no post-deletion activity. Local
backups expire in 14 days; the operator must configure remote bucket retention
and repeat deletions when restoring older backups.

Schema generation uses the pinned config and an **in-memory database**:
`npx auth@1.7.13 generate --config scripts/auth-schema.ts --output <temporary.sql> --yes`.
Compare its SQL to `src/migrations/auth-v12.sql`. Never generate against
production and never run `auth migrate`. The schema SQL is applied only by
our ordered v12 migration within the same BEGIN IMMEDIATE transaction.

Hosted request bodies are capped at 1 MiB and buffered before reserving the
shared database connection. Incomplete bodies time out after 10 seconds;
slow uploads do not hold up other users’ reads. Open MCP subscriptions release
the reservation after dispatch. Scheduler failure writes queue behind auth
transactions on this connection; refresh/backup workers keep separate connections.
