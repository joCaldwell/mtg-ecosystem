# Deck-builder operations

Run from `packages/deck-builder` unless marked repository root, using pinned Node
through `scripts/engine-env.sh`. Backend TypeScript executes directly; connections
use WAL, foreign keys and five-second busy timeout. Product build/process manager
lives in Multiverse; backend launch commands delegate there, preserving data,
environment/dotenv and `.dev` PID/log paths. Direct entrypoint is
`node ../multiverse/src/server.ts`. Package scripts own CLI/MCP/refresh/backup.
The [spec](../../docs/deck-builder-spec.md) owns behavior/authorization; this guide
owns operator commands. Historical receipts remain at Git `33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb`.

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
  thereafter. A failed refresh or backup retries one hour after its last run, still
  respecting the active-job and shared-lease checks. Backup runs at 02:00 UTC; an overdue backup runs on the next scheduler
  tick (30 seconds). Only one job runs at
  a time; repeated queued requests coalesce.
- `GET /api/settings` and Settings show each job's last run, result, next run,
  active job and queue. Local child output goes to the terminal. Hosted
  failures emit one redacted `job_error` (name, code, stack frames) rather than
  raw stdout/stderr; arbitrary messages and bodies are discarded. Backup status
  also exposes `last_success_at` and `stale` (over 36 hours or never successful),
  shown in Admin Settings and its authenticated `GET /api/admin` response. CLI jobs
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
`DATA_DIR/backups/deck-builder-YYYY-MM-DDTHH-mm-ss-SSSZ-<uuid>.db`.
UTC timestamp plus random suffix prevents same-day replacement. Keep the
newest 14 local snapshots, then optionally upload the new snapshot. Never copy only
`deck-builder.db` while the server is running: committed data may be in its
WAL sidecar.

Budget disk for 14 snapshots, one in-flight snapshot, live DB/WAL and download
cache; measure current sizes rather than relying on historical database counts.

To upload each snapshot to an S3-compatible bucket, configure all four:

| Variable | Purpose |
| --- | --- |
| `BACKUP_S3_ENDPOINT` | Bucket provider's S3 endpoint |
| `BACKUP_S3_BUCKET` | Existing bucket name |
| `BACKUP_S3_ACCESS_KEY` | Access key (secret) |
| `BACKUP_S3_SECRET_KEY` | Secret key (secret) |

The ops-only `@aws-sdk/client-s3` dependency uploads to
`backups/deck-builder-YYYY-MM-DDTHH-mm-ss-SSSZ-<uuid>.db`, with region `auto` and path-style
addressing. Partial configuration or an upload failure fails the job while
keeping the local snapshot. Store credentials in the process environment,
never in source, commands copied into issues, or screenshots. Remote-object
retention runs only after a successful upload and complete listing: keep the
newest snapshot of each of the last 14 UTC days and each of the last
6 UTC calendar months, plus the newest uploaded snapshot. Archive keys and
unrecognised objects are untouched. Failed uploads/listing never prune; retry
is one hour later. Local retention is 14 snapshots. Running source/documentation checks never
authorizes bucket cleanup or promotion; inspect deployed code before relying on
a retention change.

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
node src/cli.ts restore /private/path/post-claim-backup.db
npm start
curl --fail http://127.0.0.1:8787/api/health
```

For a nondestructive rehearsal, point the restore at a separate destination:

```sh
DECKBUILDER_DB=/tmp/deck-builder-restore.db node src/cli.ts restore /private/path/post-claim-backup.db
DECKBUILDER_DB=/tmp/deck-builder-restore.db DECKBUILDER_DATA=/tmp/deck-builder-drill DECKBUILDER_PORT=8799 node ../multiverse/src/server.ts
curl --fail http://127.0.0.1:8799/api/health
```

Open `http://localhost:8799`, compare the deck count and a deck's contents,
then stop the drill process. Run only one server for a database, and stop
older versions that predate the process-lock mechanism before restoring.

## Environment and tokens

Local defaults: package `data/`, DB `DATA_DIR/deck-builder.db`, port 8787 and dev
web port 5173. `DECKBUILDER_DATA`, `DECKBUILDER_DB`, `DECKBUILDER_PORT` and
`DECKBUILDER_WEB_PORT` override them. Local mode is implicit UI user 1 with no
login; MCP always needs a bearer. Hosted variables follow below.

Settings creates labelled once-shown tokens; store secrets in a private mode0600
file. SQLite holds hashes only. Revocation affects the next request. The shim
cannot recover secrets and does not mint/rotate them:

```sh
node /absolute/path/to/packages/deck-builder/src/mcp/stdio.ts --name my-agent --token-file /private/path/to/token
```

Priority is `--token-file`, `DECKBUILDER_TOKEN_FILE`, then local
`DATA_DIR/stdio-token`; missing credentials fail. Secret `--token` argv is rejected.
`DECKBUILDER_TOKEN` is removed. Hosted mode never imports the laptop token.

First local migration hashes/commits `agent-token` and binds its pending drafts
before renaming to `stdio-token` (0600); retries are idempotent. Existing client
secrets stay valid. Pre-v11 restores may reimport the saved file without reviving
revoked rows. Rehearse only on an online snapshot plus private copied token with
explicit DB/DATA overrides. Never migrate/test against the owner's live file.
Spec §§10.1 and17 own credential, attribution, tenant and migration contracts.

## Hosted bootstrap, secrets and provider redirects

The hosted runtime variables are listed here. The Node service reads the
process environment; `.env` is loaded only when explicitly requested with
`node --env-file=.env ../multiverse/src/server.ts`. Never print `.env`, tokens, OAuth
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
a service credential. `DECKBUILDER_WEB_PORT` is dev-only. There is no Sentry integration. No Google provider button appears until both Google
credentials exist.

Initial owner bootstrap requires an explicit code before exposure; no first-arrival
claim. Use the claim lifecycle below, then remove the variable and redeploy. Never
reset auth_id. New invited identities start empty and cannot read owner decks;
existing identities need no code. Admin Settings creates private single-use invites.
The last admin cannot delete itself. Distinct environments use distinct secrets.

Before using a new Railway domain, add this **exact redirect** in Discord's
Developer Portal → application → OAuth2 → Redirects:
`https://<railway-domain>/api/auth/callback/discord`. Keep the localhost
redirect `http://localhost:8787/api/auth/callback/discord` for isolated tests.
For Google, add `https://<railway-domain>/api/auth/callback/google` to its
OAuth client's authorized redirect URIs. Set the service public origin to
the same domain. A domain change changes the issuer/resource: reconnect OAuth
clients and sign in again rather than accepting tokens for the old audience.

Keep BETTER_AUTH_SECRET stable: it protects sessions/encrypted provider tokens.
TLS terminates at Railway; cookies stay Secure on public HTTPS. Only explicit
loopback development omits Secure. Trust proxy headers only behind one ingress
that overwrites them; verify X-Real-IP/rightmost XFF including IPv6 on a new ingress.
Never add a second untrusted container route. Auth issuer/public Origin stay fixed.
See [network specs](https://docs.railway.com/networking/public-networking/specs-and-limits).

Hosted `GET /api/health` bypasses Host/Origin and authentication, returns only
`{ok,schema_version}`, sets no cookies and reveals no card/tenant/session facts.
This explicit exception supports Railway's `Host: healthcheck.railway.app`;
use `/api/health` directly as the Railway health-check path, with no relay.
Every other path keeps the public-origin rule. Local health retains its
loopback checks and existing payload. Deployment port must match `PORT`.
See [Railway health-check hostname documentation](https://docs.railway.com/deployments/healthchecks#healthcheck-hostname).

One replica is mandatory while limits are in-memory and SQLite has one writer.
`src/ratelimit.ts` owns limits. Do not replace DNS-pinned CIMD transport with ordinary
fetch; spec §17 owns its validation and concurrency contracts.

## Account lifecycle and auth schema

Settings exports/deletes the signed-in account under the spec's UI/tenant guards.
Exports omit secrets; deletion preserves other users' data with tombstone
attribution. Restoring old snapshots also restores credentials and deleted content:
repeat erasures/revocations deliberately. No impersonation or manual auth_id rewrite.

Generate auth SQL with the pinned config and **in-memory** database:
`npx auth@1.7.13 generate --config scripts/auth-schema.ts --output <temporary.sql> --yes`.
Compare `src/migrations/auth-v12.sql`; never generate against production or run
`auth migrate`. Ordered migrations apply it in the existing transaction. Body
limits are 1MiB/10 seconds; database reservations are per query/transaction, never
across outbound I/O or MCP streams. Spec §17 owns implementation constraints.

## OAuth review hardening

Web connectors use CIMD; anonymous DCR allows native/loopback callbacks only.
Client documents: [Claude](https://claude.ai/oauth/mcp-oauth-client-metadata),
[ChatGPT](https://chatgpt.com/oauth/client.json). Corresponding redirects are
`https://claude.ai/api/mcp/auth_callback` and
`https://chatgpt.com/connector_platform_oauth_redirect`; use the connector's actual
current metadata and [documented flow](https://developers.openai.com/plugins/build/auth#oauth-flow).
Consent/host validation, public fetch bounds and grant/JWT disconnect semantics
are canonical in spec §17.7. Local mode exposes no legal pages/footer. Hosted public
operator/contact configuration belongs in environment, never committed identity.

If an invited/open identity was created before claiming, delete that new account
through its own UI before claiming user 1. Never manually rewrite auth_id. UI
Disconnect revokes a user/client grant; a new same-second grant may need next-second
retry because JWT timestamps have one-second precision.

## Per-store prices

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

Use disposable source-backed database copies for refresh/migration/browser checks,
not a live owner database. Historical refresh counts, timing/heap measurements and
old phone-layout caveats are in Git; they are not current operating requirements.

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

Production releases use an explicit owner-verified SHA on `release`; staging follows
`main`. Do not infer promotion authorization from a development or docs task.

From the repository root, build exactly:

```sh
docker build -f packages/deck-builder/Dockerfile -t deck-builder .
```

The current Dockerfile uses `node:26-slim`, the root npm lockfile selecting
`packages/multiverse`, a product Vite build stage, and production dependencies
only in the runtime. From the retained backend working directory its PID 1 is
`node ../multiverse/src/server.ts`; no server TypeScript compilation is needed.
The allowlist `.dockerignore` includes product/backend/shared game inputs,
explicitly qualified SDK `dist`, public source/standalone generated runtime and
verification manifests, plus prepared client SDK identity. Prepare SDK/client
outputs through their owning procedures before Docker; the image never generates
SDK/native artifacts. Final-image checks verify SDK hashes and import all public
entries, including source `./policies`. Context excludes `.env`, databases/tokens,
caches, tests, `.git`, native binaries and unrelated generated artifacts. Docker's
`HEALTHCHECK` uses `/api/health`; Railway checks that path on port 8787.
Verify the actual image before promotion; build/runtime/context checks alone do
not establish a working container. Watch paths must include product, backend,
shared preview/display inputs and root lock/config. Deployment is explicit, never
an effect of source/documentation changes.

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
the desired graph after claiming in both environments; neither can recreate it.
Config-as-code does not express billing alerts or native backup schedules;
those use the CLI/API below.

## Logs, health and monitoring

Logs contain generated request ID, time/level, resolved principal/via/label,
method/redacted path or tool, status and duration. Finish/close logs once (aborted
SSE is499). Hosted errors retain name/code/frames, never messages, payloads, queries,
headers/cookies/tokens/raw IP; dynamic segments redact and MCP path is `/mcp`.
Child output is similarly redacted, Better Auth logger disabled. Local errors remain
readable; successful health probes are skipped. Settings/admin retains job status
and backup age. No external log exporter; [Railway retention](https://docs.railway.com/observability/logs)
is provider-owned. IDs correlate logs, never authorize requests.

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

**Stop before an old-image rollback.** Railway restores that deployment's
variables as well as its image. Any production deployment before
`b7c4ec34-dc77-47e5-acc4-ab8bbc34d235` (including `bd39f6e9`) brings back
`DECKBUILDER_CLAIM_CODE`, and can revert current secrets/draining settings.
Do not expose such a rollback until its configuration is corrected. A claim
code is dead while user 1 is claimed, but becomes dangerous when combined
with an unclaimed restore. Prefer a compatible owner-promoted fix-forward
SHA using current configuration. [Railway rollback behavior](https://docs.railway.com/guides/roll-back-bad-deploy).

Compare live schema with the exact target commit before stopping anything:

```sh
ROLLBACK_SHA='paste-the-verified-target-SHA'
curl --fail -sS https://deck-builder-production-00d7.up.railway.app/api/health
git show "${ROLLBACK_SHA}:packages/deck-builder/src/db.ts" | sed -nE 's/^export const SCHEMA_VERSION = ([0-9]+);/target schema: \1/p'
```

The versions must match for a code-only rollback. Older code on a newer schema
is refused by `assertOurs`; volume deployments stop the predecessor before startup,
so safe refusal still causes downtime. If versions differ, plan a compatible
fix-forward or an explicitly authorized post-claim restore reconciling later writes.
Never infer today's live schema from an old release report.

**Never restore the pre-claim production snapshot.** The old
`backups/deck-builder-2026-10-25.db` resets user 1 to `auth_id NULL`, locking
the owner out or permitting a new claim if an old code is reintroduced. It
was copied, not moved, to
`archive/pre-claim-deck-builder-2026-10-25.db` (73,711,616 bytes), marked
`restore_policy=do-not-restore`; the original remains. Both are quarantined
historical owner-only evidence. Automatic retention never touches archive/.
The historical known post-claim example was
`backups/deck-builder-2026-10-26.db` (74,711,040 bytes). Choose a fresh verified
post-claim snapshot for real recovery. After downloading,
open it read-only and confirm user 1's auth_id is non-null, its schema is
supported, and the deck/memory counts match before any restore. Future
timestamped backups need the same identity check, not just a filename date.

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

Files must contain no trailing newline (or strip it in a private stdin pipeline).
Never expose secrets via argv/logs/chat. Owner bootstrap files are
`~/.config/deck-builder/claim-code` and distinct `claim-code-staging`, mode0600.
Auth secrets differ per environment; Discord credentials are shared only by explicit
owner choice with both redirects. Operator legal identity is environment data.

Owner bootstrap: open `https://deck-builder-production-00d7.up.railway.app/signin`,
paste the production file contents into **Invite or owner claim code**, and
sign in with Discord. Confirm user 1 has a non-null auth_id, remains admin,
and matches the verified source roster/counts. After the `owner_claimed` event:

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
const object = await s3.send(new GetObjectCommand({Bucket:c.bucketName, Key:'backups/deck-builder-2026-10-26.db'}));
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
{"services":{"94e7ffb1-12ce-4fd0-a8cc-afad081d9881":{"deploy":{"startCommand":"sh /usr/local/bin/deck-builder-entrypoint node ../multiverse/src/server.ts"}}}}
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

## Server won't start: stale lock recovery

New locks contain `{pid, hostname, token}`. Startup reclaims a file from a
foreign hostname, a dead PID, or its own reused PID (including container PID
1). A second acquisition held inside the current process still refuses, as
does a live holder on the same hostname. These checks protect restore and
local CLI operations as well as server startup.

If an older deployment is already crash-looping, do not repeatedly redeploy
it or delete locks blindly. Pause automatic retries, enter a maintenance
container holding the same volume, and inspect its hostname/PID plus the
lock metadata (never auth/token files). Confirm no web process, refresh,
backup or restore is alive on that volume. Take an online backup if possible.
Only then remove `/data/deck-builder.db.server-lock` and deploy compatible
code. Never remove it underneath a live holder. A legacy lock without a
hostname and reused PID 1 may need this manual recovery while old code runs.
Do not clear the separate `meta['job:lock']` lease while its child is alive;
its two-hour stale-lease rule is independent of server process locks.

`.railway/railway.ts` now sets `deploy.drainingSeconds: 30`. Railway sends
SIGTERM and waits that long before SIGKILL; this leaves time for the roughly
15-second refresh child to finish and release SQLite. Drain is not a backup
and does not make OOM/host loss clean. Both restart paths must work. See
[Railway deployment teardown](https://docs.railway.com/deployments/deployment-teardown).

## Authenticated smoke tests

Use the real owner's browser for sign-in/session/SSE checks. For CLI checks,
the owner creates a labelled token in **hosted Settings → Agent connection**
and saves it once to a private local file, mode 600 (for example
`~/.config/deck-builder/hosted-smoke-token`). Read that file directly inside
the MCP test process; never put its value in argv, chat, logs or a PR. Call
`start_session`, verify the claimed principal/roster, then have the owner
revoke the temporary token in Settings and remove the private test file.
Do not create session rows, call `internalAdapter.createSession`, sign cookies
with BETTER_AUTH_SECRET, borrow session rows, or create an impersonation path.
There is no supported emergency admin credential CLI in this release.

## Staging claim after a future restore

Both IaC graphs omit claim codes. Inspect current owner-claim state and variables
before a restore; old deployment configuration can reintroduce bootstrap secrets.

Use a verified post-claim backup for routine restores. If an explicitly
approved future **staging-only** restore leaves user 1 unclaimed, keep the
claim variable absent until the owner is ready. Generate a fresh random
code of at least 32 characters into `~/.config/deck-builder/claim-code-staging`
(mode 600), replacing the obsolete code, and set it as a **sealed** variable
in Railway → staging → deck-builder → Variables. Redeploy staging, then the
owner opens `https://deck-builder-staging.up.railway.app/signin`, pastes the
new file contents and signs in through Discord. Verify user 1 is claimed,
remove the staging variable and redeploy again. Never rewrite auth_id by hand
and never re-enable production's bootstrap path as a routine restore step.

## Independent monthly backup copy (owner action)

Railway buckets and volumes share a provider; bucket recovery alone is not
provider-independent protection. Once per month, after a successful production
backup, download the latest post-claim snapshot to the owner's machine or an
encrypted removable drive. Run from the repository root with the logged-in
Railway CLI and installed ops dependency. This command reads credentials only
in memory, never deletes a bucket object, refuses to overwrite an existing
local file and verifies integrity plus the claimed owner before reporting:

```sh
node --input-type=module <<'JS'
import {execFileSync} from 'node:child_process';
import {mkdirSync,createWriteStream,chmodSync} from 'node:fs';
import {homedir} from 'node:os';
import {join,basename} from 'node:path';
import {pipeline} from 'node:stream/promises';
import {Database} from 'node:sqlite';
import {S3Client,ListObjectsV2Command,GetObjectCommand} from '@aws-sdk/client-s3';
const c=JSON.parse(execFileSync('railway',['bucket','--environment','production','credentials','--bucket','backups-production','--json'],{encoding:'utf8'}));
const s3=new S3Client({endpoint:c.endpoint,region:'auto',forcePathStyle:true,credentials:{accessKeyId:c.accessKeyId,secretAccessKey:c.secretAccessKey}});
try {
  const objects=[];let ContinuationToken;
  do {const p=await s3.send(new ListObjectsV2Command({Bucket:c.bucketName,Prefix:'backups/',ContinuationToken}));objects.push(...p.Contents??[]);ContinuationToken=p.IsTruncated?p.NextContinuationToken:undefined;} while(ContinuationToken);
  const latest=objects.filter(x=>/^backups\/deck-builder-\d{4}-\d{2}-\d{2}.*\.db$/.test(x.Key)).sort((a,b)=>new Date(b.LastModified)-new Date(a.LastModified))[0];
  if(!latest)throw Error('No snapshot found');
  const dir=join(homedir(),'.local/share/deck-builder-offline',new Date().toISOString().slice(0,7));mkdirSync(dir,{recursive:true,mode:0o700});chmodSync(dir,0o700);
  const file=join(dir,basename(latest.Key));
  const object=await s3.send(new GetObjectCommand({Bucket:c.bucketName,Key:latest.Key}));
  await pipeline(object.Body,createWriteStream(file,{flags:'wx',mode:0o600}));
  const db=new Database(file,{readOnly:true});
  try {if(db.prepare('pragma integrity_check').get().integrity_check!=='ok'||!db.prepare('select auth_id from users where id=1').get()?.auth_id)throw Error('Not a verified post-claim backup');console.log({file,decks:db.prepare('select count(*) n from decks').get().n});} finally {db.close();}
} finally {s3.destroy();}
JS
```

Disconnect/store the external copy independently, keep six monthly copies,
and repeat erasures/revocations if recovering one. The quarantined pre-claim
archive is excluded from normal restore selection and remains owner-only;
review its disposition after an independent recovery copy exists. Documentation maintenance grants no bucket cleanup authorization.


## Owner release promotion

Production code promotion is an explicit owner action, never a standing agent task.
Select one exact reviewed SHA after staging checks, schema/migration/content review,
refresh/store checks and owner session/SSE smoke. Take a fresh **post-claim** online
production snapshot immediately before promotion, verify the uploaded key/size,
then push that SHA to `release`. Never push whichever `main` happens to name or
use old PR7/v12→v13 instructions. Follow the exact resulting deployment to SUCCESS,
then check health/schema, owner roster, jobs and backups. Build upload alone is
not deployment success. Documentation maintenance never grants production promotion authority.

Protect `release` against unreviewed updates/deletion/force pushes, retaining an
explicit owner promotion bypass. If repository plan limits prevent that protection,
record the actual limitation. There are no down migrations. An incompatible data
restore loses/reconciles later writes and must repeat erasures/revocations.

### Pre-promotion online snapshot

Use a unique UTC/UUID key under `archive/pre-promotion/`, outside automatically
pruned `backups/`. The running runtime opens SQLite read-only and uses online backup;
check claimed user 1 and object size, with overwrite protection. Keep the verified
key for recovery and do not promote if this fails. Run from the repository root:

```sh
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder -- 'setpriv --reuid=node --regid=node --init-groups node --input-type=module <<"JS"
import {Database,backup} from "node:sqlite";
import {mkdirSync,createReadStream,statSync,chmodSync} from "node:fs";
import {randomUUID} from "node:crypto";
import {S3Client,PutObjectCommand,HeadObjectCommand} from "@aws-sdk/client-s3";
async function snapshot(){
const db=new Database(process.env.DECKBUILDER_DB??"/data/deck-builder.db",{readOnly:true,timeout:5000});
const name=`deck-builder-${new Date().toISOString().replaceAll(":","-").replace(".","-")}-${randomUUID()}.db`;
const file=`/data/backups/${name}`,Key=`archive/pre-promotion/${name}`;
try {if(!db.prepare("select auth_id from users where id=1").get()?.auth_id)throw Error("Owner must be claimed");mkdirSync("/data/backups",{recursive:true});await backup(db,file);chmodSync(file,0o600);}finally{db.close();}
const s3=new S3Client({endpoint:process.env.BACKUP_S3_ENDPOINT,region:"auto",forcePathStyle:true,credentials:{accessKeyId:process.env.BACKUP_S3_ACCESS_KEY,secretAccessKey:process.env.BACKUP_S3_SECRET_KEY}});
try {const Bucket=process.env.BACKUP_S3_BUCKET;await s3.send(new PutObjectCommand({Bucket,Key,Body:createReadStream(file),ContentLength:statSync(file).size,IfNoneMatch:"*"}));const head=await s3.send(new HeadObjectCommand({Bucket,Key}));if(head.ContentLength!==statSync(file).size)throw Error("Backup size mismatch");console.log(JSON.stringify({key:Key,verified_bytes:head.ContentLength}));}finally{s3.destroy();}
}
await snapshot().catch(error=>{const safe=v=>typeof v==="string"&&/^[A-Z][A-Za-z0-9_.$-]{0,63}$/.test(v)?v:undefined;console.error(JSON.stringify({event:"pre_promotion_backup_error",name:safe(error.name)??"Error",code:safe(error.code)}));process.exitCode=1;});
JS'
```


Archive keys are recovery evidence, not automatic cleanup candidates. Do not remove
quarantined pre-claim snapshots, live databases, credential files or backup objects
as part of source/documentation maintenance. Monthly independent copies and the
restore drill above remain required operational procedures.

### Archive retirement policy

Automatic backup jobs never list or delete `archive/` objects. Retain every
pre-change snapshot for **at least 30 days after the change**, and longer while
rollback to that schema remains a recovery option. Before retiring any one object,
verify the new schema/data, a successful restore of a newer post-change backup,
and an independent off-Railway copy. Record the exact archived key and the owner's
recovery decision. Deletion is a manual owner action against one verified key,
never a scheduled wildcard job. Pre-claim quarantine retains its separate
**do-not-restore** restriction. Documentation/source maintenance grants no cleanup
permission for these objects.
