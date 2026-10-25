# Deck-builder operations

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

Local `GET /api/health` returns `{ ok, schema_version, cards, last_ingest }` without
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
| `DECKBUILDER_CLAIM_CODE` | Yes | Random bootstrap code, at least 32 characters; remove after owner claim |
| `DECKBUILDER_OPERATOR_NAME` | No | Required public operator name for hosted legal pages |
| `DECKBUILDER_CONTACT_EMAIL` | No | Required public contact email for hosted legal pages |
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
host/protocol headers. Rate-limit IP uses Railway’s documented `X-Real-IP`,
falling back to the rightmost XFF hop, never the spoofable leftmost entry.
Phase 3 must confirm those headers on the live service (including IPv6); see
[Railway network specs](https://docs.railway.com/networking/public-networking/specs-and-limits).
Do not expose a second untrusted route to the container.
Host and browser Origin must match the configured public origin. Auth uses a
fixed base URL, never a forwarded-header-selected issuer.

Hosted `GET /api/health` bypasses Host/Origin and authentication, returns only
`{ok,schema_version}`, sets no cookies and reveals no card/tenant/session facts.
This explicit exception supports Railway's `Host: healthcheck.railway.app`;
use `/api/health` directly as the Railway health-check path, with no relay.
Every other path keeps the public-origin rule. Local health retains its
loopback checks and existing payload. Deployment port must match `PORT`.
See [Railway health-check hostname documentation](https://docs.railway.com/deployments/healthchecks#healthcheck-hostname).

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
tokens and all owned application rows, with no post-deletion activity.
Other tenants’ content and spent invite records stay; attribution is replaced
with non-login tombstone user 0 and identifying author labels are cleared. Local
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

## OAuth review hardening

Web connectors use CIMD; anonymous DCR accepts only loopback HTTP or native
custom-scheme callbacks. Claude.ai publishes its client document at
https://claude.ai/oauth/mcp-oauth-client-metadata (redirect
https://claude.ai/api/mcp/auth_callback); ChatGPT publishes
https://chatgpt.com/oauth/client.json (redirect
https://chatgpt.com/connector_platform_oauth_redirect). Both documents were
verified directly from their hosts. ChatGPT's documented CIMD flow is at
https://developers.openai.com/plugins/build/auth#oauth-flow. Choose CIMD in
the connector setup; no HTTPS DCR redirect allowlist is needed. Consent shows
the identity and redirect hosts, plus an unverified-name notice for DCR and
local-app impersonation warning for loopback callbacks.

The DB reservation is taken by the pinned native Kysely dialect per query
or transaction. Never wrap the auth handler or outbound DNS/provider fetches
in it. The public metadata fetcher has four slots, a five-second deadline
including DNS waiting, a 64 KiB body cap and 30-second/128-entry public cache.
Concurrent UI reads remain available during slow CIMD/provider requests.


Hosted startup requires public `DECKBUILDER_OPERATOR_NAME` and
`DECKBUILDER_CONTACT_EMAIL`; set them in Railway/environment, not source.
Do not put personal contact details in the repository. Local mode needs
neither and exposes no legal pages/footer. Set a cryptographically random
claim code of at least 32 characters; shorter configured codes fail startup.
If the owner's Discord identity first registers as an invited/open user,
it cannot later claim user 1: delete that new account first, then sign in
with the owner claim code. Never manually rewrite auth_id.

Settings → Agent connection → Connected OAuth apps → Disconnect removes the
grant and refresh tokens, and invalidates existing JWTs even after a later
re-consent. Tokens minted within the same second as disconnect may need a
retry the next second. Handles can only be changed in the first-run picker.
Anonymous native client registrations with no grants expire after one day;
cleanup runs at most hourly on auth requests. Refresh rotation permits a
five-second retry grace. Auth/session/linking endpoints unused by the product (including generic HTTP
get-session; the product uses /api/session)
are disabled. Hashed assets are immutable for a year; HTML/auth/API responses
are no-store. Rate buckets are isolated by kind, IPv6 shares a /64, claim
attempts use 5/minute/IP, and OAuth token exchange uses 240/minute/client.


## Per-store prices (v3 Phase 1b)

Schema v13 migrates old USD quotes to `(oracle_id, tcgplayer)` without
changing cents, printing or finish. Preferences are per-user `price_currency`
and `price_store` keys (absent means USD/lowest). Production applies the ordered
migration on deploy. A missing/old price_stores_version marker makes the
scheduler enqueue refresh at boot and report it due immediately, even when
card/pricing timestamps and the previous successful run are fresh. Failed
refreshes retain the hourly retry. The refresh downloads default-cards even
if its timestamp matches: `price_stores_version` is set only with the successful
atomic swap. Later refreshes retain the normal publication freshness check.
Both store reductions stream together; the staging transactions still hold at
most 500 rows. Failed loads preserve both stores and the previous date.

Settings reports counts for the principal's selected store. To audit all stores:
`SELECT store, currency, count(*) FROM card_prices JOIN cards USING(oracle_id) GROUP BY store, currency;`.
Scryfall is the only source; no credentials, vendor API, exchange rates or new
cache/download artifact are needed. Lowest compares only the chosen currency.

Phase 1b acceptance used an online read-only-source backup of the owner's live
DB into this worktree's ignored `.dev/`, never the live file for migrations or
refresh. A real forced refresh at Scryfall publication 2026-10-25T21:05:44.018+00:00
priced 33,327 database cards at TCGplayer and 33,092 at Cardmarket (34,039 and
33,601 total store rows, including oracle IDs outside the card catalogue).
Every old USD per-card quote and all 18 original deck totals matched exactly.
Peak sampled JS heap was 83.46 MiB; Linux `/proc/self/status` VmHWM was
236.52 MiB RSS, below Phase 0's recorded 246.21 MiB. This runner's getrusage
maxRSS inherits a parent high-water value, so it is not used for that comparison.

User 2's EUR/Cardmarket choice was saved through the rendered Settings form on
5181 against an ignored acceptance harness using createApp's existing actor seam
(API 8791). The seeded 76-card deck showed €244.27, Cardmarket, 2026-10-25;
its selected card panel showed the matching EUR amount and date. Desktop and
720 px screenshots were inspected; browser console had no runtime errors. The
390 px shell remains cramped pending Phase 4a's phone view. Real-data agent
start_session/get_deck and eur<=1 search text were read; user 1 still showed
TCGplayer/USD. The harness, backup, screenshots and transcripts stay ignored.


Phase 1b review regression checks use the official MCP client against createApp
in both protocol eras: price-bearing responses have exactly one source/date
line, while exports retain their exact importable text and ruling/memory tools
have no source line. The three reported negation queries on the same backup
match pre-phase results by oracle ID: `-usd>5` 32,791; `t:dragon -usd<1` 153;
`t:dragon (usd>100 or -usd>0)` 34. No live database was opened for these checks.
The scheduler regression migrates a populated v12 fixture and checks missing,
old and current price-store markers one hour after a successful refresh.

## Railway private beta: topology and releases

The beta uses project **deck-builder** (`2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8`),
service **deck-builder** (`94e7ffb1-12ce-4fd0-a8cc-afad081d9881`).
Never run project mutations against another linked project.

| Environment | Source branch | Public origin | Volume / bucket |
| --- | --- | --- | --- |
| production | `release` | `https://deck-builder-production-00d7.up.railway.app` | `production-data`, 5000 MB at `/data`; `backups-production` |
| staging | `main` | `https://deck-builder-staging.up.railway.app` | `staging-data`, 5000 MB at `/data`; `backups-staging` |

Exactly **one replica per environment**, no sleep/serverless: SQLite has one
writer and rate limiting is in memory. The web process schedules refresh and
backup children against the same volume. From go-live, **production is the
primary database**; the owner's unchanged laptop data becomes development
input. Never redeploy a local database over hosted user data. From the first
invited account, migrations must preserve/backfill all user content (D19).

`release` was created at **3e1798d**, the merge commit of Phase 2 PR #5,
before Phase 3. Initial provisioning uploads the Phase 3 container/logging
assets alongside that predecessor's product code because the predecessor
has no Dockerfile. The owner must merge Phase 3 and promote its merge commit
before relying on push builds. This bootstrap does not advance `release` to
unmerged feature commits. Subsequent production releases come only from
`release`; parallel 1b/4a migrations land on `main`/staging first.

From the repository root, build exactly:

```sh
docker build -f packages/deck-builder/Dockerfile -t deck-builder .
```

The Dockerfile uses `node:26-slim`, the root npm lockfile with a selected
workspace, a Vite build stage, production dependencies only in the runtime,
and `node src/server.ts` as PID 1. No server TypeScript compilation. The
allowlist `.dockerignore` excludes `.env`, all database/token/cache files,
tests, `.git`, other packages and downloaded assets. Docker's `HEALTHCHECK`
uses `/api/health`; Railway separately checks that path on target port 8787.
The local Docker daemon was unavailable during this phase; Railway performs
and verifies the actual Docker build from the same repo-root context.

The image defaults to `USER node`. Railway requires `RAILWAY_RUN_UID=0` for
its root-owned volume; the entrypoint prepares only the data directory,
then `exec setpriv --reuid=node --regid=node --init-groups ...`. Node and
all scheduler children run as UID/GID 1000; SSH operator commands run as
root, so use the same `setpriv` prefix for database commands. Do not leave
a root-running Node service. [Railway volume permissions](https://docs.railway.com/volumes/reference)
explain why Dockerfile ownership alone does not prepare a mounted volume.

Infrastructure is reviewable in `.railway/railway.ts` (CLI 5.63.1, isolated
`railway@3.16.0` authoring dependency in `.railway/package-lock.json`). Install
with `npm ci --prefix .railway`. It chooses `release`/`main` from the linked
environment, retains secrets using `preserve()`, and never stores identities,
credentials or generated domains in the authoring file. Each environment
has a separately scoped graph. Live setup uses scoped CLI/API config edits;
plan the resulting graph before changing it:

```sh
railway link --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --service deck-builder
railway config plan
railway link --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder
railway config plan
```

Review every plan, especially deletes. Do not use `config pull
--include-variables`: it writes non-sealed secret values to source. Claim
codes are bootstrap state: production's code is deliberately absent from
the desired graph after claiming; staging preserves its independent code.
Config-as-code does not express billing alerts or native backup schedules;
those use the CLI/API below.

The owner promotes after staging smoke tests and a fresh production backup:

```sh
git fetch origin
git push origin main:release
# Or a specific staging-verified commit:
git push origin <commit-sha>:release
railway deployment list --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder --json
curl --fail https://deck-builder-production-00d7.up.railway.app/api/health
```

Use an up-to-date local `main` for the first form. Follow the exact resulting
deployment ID to `SUCCESS`; upload/build acceptance alone is not success.
Volumes imply brief deployment downtime. Do not bypass staging for a schema
change. Secrets, backups and monitoring are independent of Git promotion.

## Logs, health and monitoring

Request and MCP tool logs are one JSON object per line with `ts`, `level`,
`req_id`, resolved `user_id`, `via`, `label`, `method`, `path` or `tool`,
`status` and elapsed `ms`. `X-Request-ID` correlates the two records. Requests
log once on finish/close; open SSE requests log when closed (499 if aborted).
Errors include stack frames, excluding the message because provider/validation
messages can contain credentials or submitted text. Query strings, headers,
cookies, request bodies, tool arguments/results and raw IPs are never logged.
Dynamic URL segments are replaced with placeholders; MCP paths log `/mcp`.
Hosted child output records only stream/byte counts, plus job completion;
Settings and `meta` retain job status. Better Auth's own logger stays disabled.
For example, grepping Railway's structured output:

```sh
railway logs --environment production --service deck-builder --lines 200 --json | jq 'select(.user_id == 1 or (.message // "" | contains("user_id")))'
```

Railway can parse application JSON into log fields; use its `user_id` filter
or inspect `.message` if a CLI version keeps the inner JSON as text. Request
IDs are generated by the app, not trusted from the caller. Logs preserve
actor attribution without granting any capability.

`GET /api/health` remains public and returns exactly `{ok,schema_version}`.
It ignores Host/Origin only for that GET path, never resolves a session, and
sets no cookie. Railway probes send `Host: healthcheck.railway.app`; successful
platform deployments plus an internal request with that Host verify the
exception. All other routes retain their configured Host/Origin boundary.

Set up an external monitor in UptimeRobot (no credentials supplied to the
agent): Add New Monitor → HTTP(s) → Friendly Name `deck-builder production`
→ URL `https://deck-builder-production-00d7.up.railway.app/api/health`
→ interval 5 minutes → select the owner's alert contact → Create Monitor.
Repeat for staging if wanted. Alert on HTTP failure; the health payload does
not prove Scryfall/bucket availability. Inspect Settings job failures and
backup timestamps as well. No Sentry SDK or `SENTRY_DSN` integration is added;
optional error aggregation is deferred instead of installing an unused SDK.

## Rollback and restore safety

For a code-only regression, Railway → production → deck-builder → Deployments
→ the previous known-good deployment → menu → Redeploy. Or use
`railway redeploy --environment production --service deck-builder --yes`
for the latest uploaded artifact (not `--from-source` if the source is bad).
Recheck its exact deployment ID and health. `railway down --environment
production --service deck-builder --yes` removes the running deployment but
retains the volume; use it to halt a broken instance, not as a data rollback.
Do not delete the service, project, volume or bucket.

Before running old code against a database, confirm it supports the current
schema. There are no down migrations. A data rollback means maintenance
mode, restore a pre-change online backup, and deploy compatible code. It
loses all changes since the snapshot, so reconcile user data and repeat any
account deletions/revocations. Do not force-reset `release` casually; select
a fix-forward commit or deliberately coordinate a history rollback with the
owner. Platform deployment rollback alone never rewinds SQLite.

## Secret rotation and owner claim lifecycle

Secrets are sealed Railway variables; don't print `railway variable list`
or bucket-credential output. Feed sensitive values on stdin:

```sh
# Read the value inside the same command that sets it. No argv expansion.
railway variable set DECKBUILDER_CLAIM_CODE --stdin --environment production --service deck-builder < ~/.config/deck-builder/claim-code
```

The initial bootstrap normalized the file's trailing newline before setting
it. For subsequent stdin use, store the file without a trailing newline, or
strip it in a small private process that pipes directly to Railway. Never
paste a production claim code into the repository, shell argv, logs or chat.
The owner's production file is `~/.config/deck-builder/claim-code`, mode 600;
staging has a distinct random file at `~/.config/deck-builder/claim-code-staging`,
mode 600. Auth secrets are distinct random values per environment. Discord
credentials are shared by explicit owner choice; add both exact redirect URLs.
Legal identity values are required public environment configuration, not
source-controlled personal information.

Owner bootstrap: open `https://deck-builder-production-00d7.up.railway.app/signin`,
paste the production file contents into **Invite or owner claim code**, and
sign in with Discord. Confirm user 1 has a non-null auth_id, remains admin,
and owns all 18 decks and 36 memories. After the `owner_claimed` event:

```sh
railway variable delete DECKBUILDER_CLAIM_CODE --environment production --service deck-builder
railway redeploy --environment production --service deck-builder --yes
```

The code is already unusable once claimed. Removing it and redeploying closes
the operational lifecycle. Do not reset `auth_id`, re-enable claims or reuse
production's code in staging. A staging restore of a claimed production owner
can be signed in with the same Discord identity without a claim code; its
sessions use staging's own origin/secret. Restoring auth/provider rows does
not make their encrypted provider tokens portable across secrets; re-sign-in
on staging and never copy its credentials back to production.

Rotate a personal token in hosted Settings → Agent connection: create a new
labelled token, update the client, verify `start_session`, then revoke the old
one. Nothing reads the laptop's legacy token into hosted mode. For Discord,
regenerate the provider secret in the Developer Portal, set the sealed variable
in **both** environments via a private process, and redeploy both. Keep
`BETTER_AUTH_SECRET` stable; rotation invalidates sessions and encrypted
provider tokens, requires fresh sign-ins and OAuth reconnects, and must be
planned with a fresh backup. Generate the replacement in memory and pipe it
to `railway variable set BETTER_AUTH_SECRET --stdin`; don't save it in a file.
For bucket rotation, reset credentials only for that environment's bucket,
capture the response privately, set both `BACKUP_S3_ACCESS_KEY` and
`BACKUP_S3_SECRET_KEY` sealed, redeploy, run a backup and verify its object.

Mint an invite as the signed-in admin: Settings → Admin → Create invite.
Give the single-use code privately to one tester. They open `/signin`, enter
it, sign in with Discord, and should see zero decks; requests for owner decks
must return 404. Existing accounts sign in with an empty code field. Account
delete/export stay self-service; never impersonate a tester or manually link
a new identity to user 1.

## Spending alert and native volume snapshots

The workspace **soft email alert is $25**, verified with CLI 5.63.1; no hard
cap or other project's settings were changed:

```sh
railway usage limit status --workspace 51a41279-bc88-41a9-b0d5-4ef13a0f1a9d --target workspace --json
railway usage limit set --workspace 51a41279-bc88-41a9-b0d5-4ef13a0f1a9d --target workspace --soft 25 --json
railway usage projects --workspace 51a41279-bc88-41a9-b0d5-4ef13a0f1a9d --project deck-builder --json
```

The alert counts the entire workspace, including the two pre-existing
projects. It is not a $25 per-project cap. Dashboard equivalent: workspace
Settings → Usage → Usage Limits → set the email/soft threshold to $25 and
save; leave any existing hard cap alone.

**Native volume backups are blocked on Hobby.** For both production and staging,
`volumeInstanceBackupScheduleUpdate(kinds:[DAILY,WEEKLY])` returned:
`Manual backups and backup schedules are only available for Pro workspaces`.
No upgrade was made. Nightly online SQLite snapshots to the Railway bucket
remain configured and independently tested. To satisfy native snapshot
redundancy, the owner chooses a Pro upgrade in workspace Settings → Plans →
Pro, then project → each environment → deck-builder → Backups → select Daily
and Weekly → Save. Verify both schedules appear. Native retention is six
days daily / 27 days weekly; see [Railway backups](https://docs.railway.com/volumes/backups).
Native snapshots do not replace the consistent online backups or restore drill.

## Move an instance and rehearse a bucket restore

Never copy a bare database from a running instance and never change the
laptop's live `data/`. Use SQLite's online backup API with a read-only source,
then validate/migrate only the destination. For the initial move, the server
was stopped, but the same online API was still used:

```sh
# Run from the repository root; destination is ignored/private, never data/.
mkdir -p packages/deck-builder/.dev/phase3
chmod 700 packages/deck-builder/.dev/phase3
python3 - <<'PY'
import sqlite3
src = sqlite3.connect('file:/home/josh/Code/mtg-ecosystem/packages/deck-builder/data/deck-builder.db?mode=ro', uri=True)
dst = sqlite3.connect('packages/deck-builder/.dev/phase3/owner-online.db')
src.backup(dst)
dst.close()
src.close()
PY
chmod 600 packages/deck-builder/.dev/phase3/owner-online.db
```

Create the destination with its own volume, public origin and secrets; do not
copy token files or `.env`. Before replacing an existing destination, back it
up, warn users about maintenance, and stop its web process and jobs. On Railway,
use a maintenance deployment that holds the volume but does not open SQLite:

```sh
# Set the destination environment explicitly. This example is STAGING.
railway environment edit --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --json <<'JSON'
{"services":{"94e7ffb1-12ce-4fd0-a8cc-afad081d9881":{"deploy":{"startCommand":"node -e 'require(\"node:http\").createServer((q,s)=>{s.writeHead(q.url===\"/api/health\"?200:503);s.end(\"maintenance\")}).listen(8787,\"0.0.0.0\")'"}}}}
JSON
```

Deploy the correct branch and wait for that exact deployment's `SUCCESS`.
Confirm `/api/health` now says `maintenance` (the maintenance probe is not
normal application health). For initial bootstrap, before the Dockerfile
exists on the remote branches, upload the selected checkout with `railway
up --environment staging --service deck-builder --detach --json`; for normal
operation deploy `main` to staging or `release` to production. Do not use an
old image rollback for maintenance: it restores that image's original startup
configuration, not the newly edited command. If a container is still stopping,
wait for the new instance before SSH/SFTP. Use `railway api` to inspect a
specific deployment's `instances { id status }` when necessary.

For initial production import, upload the read-only source snapshot; for a
restore drill, take the source from the production **bucket**, not its live
file. Take the online production backup as UID 1000:

```sh
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder -- 'setpriv --reuid=node --regid=node --init-groups node src/cli.ts backup'
railway bucket --environment production info --bucket backups-production --json
```

Download the backup with S3 credentials captured in process memory (nothing
is printed or written to a credential file). This uses the already installed
ops dependency. Run at the repository root and select the actual UTC date:

```sh
node --input-type=module <<'JS'
import { execFileSync } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { S3Client, GetObjectCommand } from '@aws-sdk/client-s3';
const c = JSON.parse(execFileSync('railway', ['bucket', '--environment', 'production', 'credentials', '--bucket', 'backups-production', '--json'], { encoding: 'utf8' }));
const s3 = new S3Client({ endpoint:c.endpoint, region:'auto', forcePathStyle:true,
  credentials:{accessKeyId:c.accessKeyId, secretAccessKey:c.secretAccessKey} });
const object = await s3.send(new GetObjectCommand({Bucket:c.bucketName, Key:'backups/deck-builder-2026-10-25.db'}));
await pipeline(object.Body, createWriteStream('packages/deck-builder/.dev/phase3/production-bucket-backup.db', { mode:0o600, flags:'wx' }));
s3.destroy();
JS
railway volume --environment staging --service deck-builder files --volume staging-data upload packages/deck-builder/.dev/phase3/production-bucket-backup.db /restore-production.db --json
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --service deck-builder -- 'setpriv --reuid=node --regid=node --init-groups node src/cli.ts restore /data/restore-production.db'
```

For initial production import the corresponding upload was `files --volume
production-data upload .../owner-online.db /owner-online.db`, and restore
used `/data/owner-online.db` in the production maintenance instance. `restore`
checks integrity/FKs, migrates a temporary online copy, clears stale job
leases and atomically swaps the stopped destination. It refuses an active
server. If a process lock remains after a crash, verify the previous server
and workers are gone before removing the stale lock; PID 1 can be reused
across containers. Never bypass a live lock.

Open the restored file read-only and compare deck/memory counts, owner ids,
schema and `PRAGMA foreign_key_check` before serving. Return the environment
to the normal start command, deploy, then wait for `SUCCESS` and health:

```sh
railway environment edit --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --json <<'JSON'
{"services":{"94e7ffb1-12ce-4fd0-a8cc-afad081d9881":{"deploy":{"startCommand":"sh /usr/local/bin/deck-builder-entrypoint node src/server.ts"}}}}
JSON
curl --fail https://deck-builder-staging.up.railway.app/api/health
```

Verify UID 1000 in `/proc/1/status`. If restoring with root-owned operator
files, only the upload needs to be readable; run restore as node so the new
database and sidecars stay writable by the service. If an operator previously
ran Node as root, repair the volume's file ownership before starting again.
Remove uploaded restore inputs after verification; keep dated backups in the
bucket. Repeat restores/deletions/revocations deliberately; a backup restores
authentication state as well as deck data. A different public origin needs
provider redirects and OAuth client reconnects.

### Phase 3 live evidence, 2026-10-25

- Source: **schema 10, 18 decks, 36 memories**. Online backup: **0.056 s**;
  SHA-256 comparisons of every original local data file found no changes.
- Production migration: **schema 12, 18 decks, 36 memories**, zero FK errors,
  user 1 unclaimed before owner sign-in, **zero personal tokens**. No local
  legacy credential was uploaded; hosted Settings creates new credentials.
- Production forced refresh: **15.14 s**, child peak **264.10 MiB**, server
  peak **179.55 MiB**; **148/148** concurrent health probes succeeded, slowest
  **36.72 ms**, server process/start time unchanged, UID 1000. Persistent
  `job:refresh` recorded success; daily scheduling remains the web child's job.
- Online production backup uploaded to `backups-production`; object
  `backups/deck-builder-2026-10-25.db`, **73,711,616 bytes**. Downloaded from
  that bucket in **3.205 s**, uploaded to staging, then restored/opened in
  **4.961 s**. Restored staging: **18 decks, 36 memories, schema 12**, zero
  FK errors. Deployment `279a5657-7a39-4032-87e5-d94a30197f7f` reached
  `SUCCESS` and passed Railway's native healthcheck.
- Production deployment `bd39f6e9-57c3-4304-838c-48b14544ef77` reached
  `SUCCESS`; native `/api/health` check passed. Internal request with
  `Host: healthcheck.railway.app` plus a wrong Origin returned HTTP 200,
  exactly `{ok:true,schema_version:12}`, no Set-Cookie. UID/GID both 1000.
- Temporary staging ingress probe (removed immediately afterwards) observed
  `X-Real-IP` equal to the caller's independently measured public IPv4.
  `X-Forwarded-For` contained that address followed by a Railway proxy;
  supplying fake X-Real-IP `203.0.113.9` and XFF
  `198.51.100.7, 203.0.113.8` did not change the forwarded client. Host was
  the staging domain, protocol HTTPS. This confirms **X-Real-IP first**;
  rightmost XFF would identify a proxy on this path. No raw client IP is
  retained in logs/runbook. The Railway domain had no resolvable IPv6 address from the caller, so a
  real IPv6 ingress request remains unverified.
- Live Discord `/api/auth/sign-in/social` returned 200 and
  `__Secure-better-auth.state` with **Secure, HttpOnly, SameSite=Lax**.
  Cookie values and OAuth responses were suppressed. PUBLIC_ORIGIN is HTTPS
  in both environments, and HSTS is enabled.
- The production sign-in UI rendered the invite/claim field, Discord button,
  legal links and Fan Content footer. T3 DOM evaluation worked; its snapshot
  endpoint timed out twice. The alternate browser lacked an installed Chrome,
  so no screenshot-based visual acceptance is claimed.
- `npm run check`: **961/961**, including the new concurrent request/tool
  logging and secret-redaction test; both source and web typechecks passed.
  IaC staging plan: **No changes**, no destructive operations applied.
- Initial push-build attempts against `main`/`release` correctly failed while
  those branches still lacked this PR's Dockerfile. The initial uploaded
  container assets supply the live beta; merge then promote this phase before
  relying on GitHub push builds. One upload hit a Railway snapshot-fetch
  failure; it was cancelled and a fresh upload reached `SUCCESS`.

## Connect the owner's clients to hosted production

The agent does **not** edit `~/.claude.json` or `~/.codex/config.toml`.
After the owner claim, open hosted Settings → Agent connection → create a
new labelled token for each CLI. Save each once-shown value privately. Do
not reuse the local legacy token. The examples use a distinct server name
so the owner can test hosted access before removing the old local entry.
Disable/remove the local `deck-builder` connection when using hosted as the
primary copy, to avoid sending writes to the development database.

Claude Code, from the owner's zsh terminal (syntax checked against the
installed `claude mcp add --help`):

```sh
read -rs 'DECKBUILDER_HOSTED_TOKEN?Paste the Claude Code token from hosted Settings: '
printf '\n'
claude mcp add --scope user --transport http deck-builder-hosted \
  https://deck-builder-production-00d7.up.railway.app/mcp/claude-code \
  --header "Authorization: Bearer ${DECKBUILDER_HOSTED_TOKEN}"
unset DECKBUILDER_HOSTED_TOKEN
```

Restart Claude Code, use `/mcp` to confirm Connected, then ask it to call
`start_session` and check it reports the claimed identity and 18 decks.
The CLI saves the bearer header in its own private configuration. Keep
normal tool approvals unless the owner deliberately changes them.

Codex CLI, with a **separate** hosted Settings token (syntax checked against
installed `codex mcp add --help`):

```sh
read -rs 'DECKBUILDER_HOSTED_TOKEN?Paste the Codex token from hosted Settings: '
printf '\n'
export DECKBUILDER_HOSTED_TOKEN
codex mcp add deck-builder-hosted \
  --url https://deck-builder-production-00d7.up.railway.app/mcp/codex \
  --bearer-token-env-var DECKBUILDER_HOSTED_TOKEN
```

Launch Codex from that environment, call `start_session` and check identity
and roster. The config stores only the environment variable's name; the
variable must be present whenever the CLI, desktop or extension starts.
For an owner-managed static header instead, manually add this to their
private `~/.codex/config.toml` and restart that client:

```toml
[mcp_servers.deck-builder-hosted]
url = "https://deck-builder-production-00d7.up.railway.app/mcp/codex"
http_headers = { Authorization = "Bearer <new Codex token from hosted Settings>" }
default_tools_approval_mode = "writes"
```

Claude.ai: Settings → Connectors → Add custom connector → name
`Deck builder` → URL `https://deck-builder-production-00d7.up.railway.app/mcp`
→ Add/Connect → sign in with the claimed Discord account → verify the
connector identity/redirect on consent → approve deck-builder access. Team
or Enterprise users may need an organization owner to add/enable it first.
Select the connector in a conversation and call `start_session`. See
[Claude's remote connector instructions](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp).

ChatGPT web: Settings → Apps → Advanced settings → Developer mode (or,
for workspace admins, Workspace settings → Apps → Create). Choose Create
app, set MCP URL to
`https://deck-builder-production-00d7.up.railway.app/mcp`, choose **OAuth**
and CIMD/automatic discovery, then Scan Tools/Connect. Sign in with the
claimed account and review consent; finish Create, select the app in a chat,
and call `start_session`. Availability/actions depend on the plan/workspace;
OpenAI currently documents full write MCP access for Business/Enterprise/Edu,
with more restricted Pro read/fetch access. See
[OpenAI's current developer-mode instructions](https://help.openai.com/en/articles/12584461-developer-mode-and-full-mcp-connectors-in-chatgpt).
Do not enter the Discord client secret or a personal bearer token in either
web connector: each uses deck-builder OAuth with the connecting client's
published CIMD document. Reconnect after changing the public domain.
