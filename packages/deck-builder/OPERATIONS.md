# Deck-builder operations

Run from `packages/deck-builder` unless marked repository root, using pinned Node
through `scripts/engine-env.sh`. Connections use WAL, foreign keys and a five-second
busy timeout. Launch commands delegate to the Multiverse process manager (data,
dotenv and `.dev` PID/log paths preserved); the direct entrypoint is
`node ../multiverse/src/server.ts`. The [spec](../../docs/deck-builder-spec.md) owns
behavior, authorization, OAuth, rate limits and health/logging; this guide owns
operator commands. Historical rollout receipts are at Git `33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb`.

## Card refresh and scheduled jobs

The serving process schedules work; it never ingests bulk data itself.

- `npm run refresh [-- --force]` refreshes oracle cards, then prices, on a separate
  connection; `--force` bypasses cache freshness and the unchanged-price check.
  The Settings button enqueues the same child without `--force`.
- Oracle downloads are cached in `DATA_DIR/.scryfall-cache/oracle-cards.json` for
  24 hours (independent of the parser's cache); price downloads are streamed, not kept.
- Refresh runs on boot when card/price data or a price-store marker is stale or
  missing, then daily. Backup runs at 02:00 UTC; an overdue backup runs on the next
  30-second tick. Failed jobs retry an hour after their last run. One job runs at a
  time; queued requests coalesce.
- `GET /api/settings`, Settings, Admin Settings and `GET /api/admin` show each job's
  last run/result, next run, active job and queue; backup adds `last_success_at` and
  `stale` (over 36 hours or never). CLI runs write the same status. Hosted failures
  emit one redacted `job_error`.
- `meta['job:lock']` is an atomic SQLite lease shared by refresh and backup,
  including manual commands. Leases older than two hours are reclaimed; release
  removes only the holder's lease. After a killed process wait for expiry; never
  delete a lock while a job is alive.
- Truncation (below 90% of active cards), retirement and atomic price/token
  publication follow spec §§4.1, 4.4 and 4.5; failed staging leaves previous facts
  usable. Audit stored quotes with
  `SELECT store, currency, count(*) FROM card_prices JOIN cards USING(oracle_id) GROUP BY store, currency;`

Local `GET /api/health` returns `{ ok, schema_version, cards, last_ingest }` under
loopback checks (hosted: spec §10.1). It proves liveness and data date, not Scryfall
or bucket reachability. Test refresh/migrations on disposable copies, never live data.

## Consistent backups

```sh
npm run backup
```

Node's online SQLite backup (including committed WAL) writes a temporary file, then
renames it to `DATA_DIR/backups/deck-builder-YYYY-MM-DDTHH-mm-ss-SSSZ-<uuid>.db`;
the newest 14 local snapshots are kept. Never copy only `deck-builder.db` while a
server runs: committed data may be in its WAL. Budget disk for 14 snapshots, one in
flight, the live DB/WAL and the download cache.

Off-volume upload needs all of `BACKUP_S3_ENDPOINT`, `BACKUP_S3_BUCKET`,
`BACKUP_S3_ACCESS_KEY` and `BACKUP_S3_SECRET_KEY` (last two secret). The ops-only
`@aws-sdk/client-s3` dependency uploads to `backups/<same name>` with region `auto`
and path-style addressing. Partial configuration or upload failure fails the job
but keeps the local snapshot. After a successful upload and complete listing, remote
retention keeps the newest snapshot of each of the last 14 UTC days and 6 UTC
months, plus the newest upload. `archive/` keys and unrecognised objects are never
touched; failed uploads/listings never prune. Documentation never authorizes bucket
cleanup; inspect deployed code before relying on a retention change.

## Restore drill

Restore replaces the destination database: take a backup, then stop the server.
Restore validates integrity, foreign keys and supported schema on a temporary
online copy, reports the deck count, checkpoints the stopped destination and
atomically renames the copy into place. A running server or active job blocks it
(the database `.server-lock` is shared with startup; a dead PID is reclaimable).
Restored job leases clear and interrupted jobs are marked failed. Snapshots carry
hashed credentials and revocations: revoke again after an older restore; once-shown
secrets cannot be recovered.

```sh
npm run backup
npm stop
node src/cli.ts restore /private/path/post-claim-backup.db
npm start
curl --fail http://127.0.0.1:8787/api/health
```

Nondestructive rehearsal against a separate destination, then compare deck counts
and contents at `http://localhost:8799` and stop the drill (one server per database):

```sh
DECKBUILDER_DB=/tmp/deck-builder-restore.db node src/cli.ts restore /private/path/post-claim-backup.db
DECKBUILDER_DB=/tmp/deck-builder-restore.db DECKBUILDER_DATA=/tmp/deck-builder-drill DECKBUILDER_PORT=8799 node ../multiverse/src/server.ts
curl --fail http://127.0.0.1:8799/api/health
```

To copy a database without touching its source (read-only online backup,
destination 0600, never overwrites), from the repository root:

```sh
node packages/deck-builder/scripts/online-copy.mjs SOURCE.db packages/deck-builder/.dev/copy/owner-online.db
```

## Environment and tokens

Local defaults: package `data/`, DB `DATA_DIR/deck-builder.db`, port 8787 and dev
web port 5173, overridden by `DECKBUILDER_DATA`, `DECKBUILDER_DB`, `DECKBUILDER_PORT`
and `DECKBUILDER_WEB_PORT`. Local mode is implicit UI user 1; MCP always needs a
bearer.

Settings creates labelled once-shown tokens; store them in a private mode 0600
file. The stdio shim neither recovers nor mints secrets:

```sh
node /absolute/path/to/packages/deck-builder/src/mcp/stdio.ts --name my-agent --token-file /private/path/to/token
```

Priority is `--token-file`, `DECKBUILDER_TOKEN_FILE`, then local
`DATA_DIR/stdio-token`; missing credentials fail and `--token` argv is rejected.
The first local migration hashes the legacy `agent-token`, binds its drafts and
renames it to `stdio-token` (0600); retries are idempotent. Hosted mode never
imports a laptop token. Rehearse migrations only on an online copy plus a private
copied token with explicit DB/DATA overrides.

## Hosted variables, secrets and provider redirects

The service reads the process environment; `.env` loads only via
`node --env-file=.env ../multiverse/src/server.ts`. Never print `.env`, tokens,
OAuth responses or provider credentials. Local `.env` is ignored and mode 0600.

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

`DECKBUILDER_TOKEN_FILE` is a local stdio client path; `DECKBUILDER_WEB_PORT` is
dev-only. No Google button appears without both Google credentials. There is no
Sentry integration. Distinct environments use distinct secrets; never commit
operator identity.

Before using a Railway domain, add the exact Discord redirect
`https://<railway-domain>/api/auth/callback/discord` (Developer Portal → OAuth2 →
Redirects; keep `http://localhost:8787/api/auth/callback/discord` for isolated
tests) and, for Google, `https://<railway-domain>/api/auth/callback/google`. A
domain change changes issuer/resource: reconnect OAuth clients and sign in again.
Connector client metadata: [Claude](https://claude.ai/oauth/mcp-oauth-client-metadata)
(`https://claude.ai/api/mcp/auth_callback`), [ChatGPT](https://chatgpt.com/oauth/client.json)
(`https://chatgpt.com/connector_platform_oauth_redirect`, [flow](https://developers.openai.com/plugins/build/auth#oauth-flow)).
Use their current metadata; CIMD/DCR rules are spec §17.7.

Keep `BETTER_AUTH_SECRET` stable: it protects sessions and encrypted provider
tokens. TLS terminates at Railway. Trust proxy headers only behind one ingress
that overwrites them; on a new ingress verify X-Real-IP/rightmost XFF including
IPv6 ([network specs](https://docs.railway.com/networking/public-networking/specs-and-limits))
and never add a second untrusted route. Use `/api/health` as the health-check path
([healthcheck hostname](https://docs.railway.com/deployments/healthchecks#healthcheck-hostname));
the deployment port must match `PORT`. One replica is mandatory (in-memory limits,
one SQLite writer).

## Account lifecycle and auth schema

Settings exports/deletes accounts (spec §17.5). Restoring an older snapshot
restores credentials and deleted content: repeat erasures/revocations. No
impersonation or manual `auth_id` rewrite; delete any identity created before the
owner claim through its own UI before claiming user 1. Generate auth SQL against an
**in-memory** database and compare with `src/migrations/auth-v12.sql`; never
generate against production or run `auth migrate`:

```sh
npx auth@1.7.13 generate --config scripts/auth-schema.ts --output <temporary.sql> --yes
```

## Railway private beta: topology and releases

Project **deck-builder** (`2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8`), service
**deck-builder** (`94e7ffb1-12ce-4fd0-a8cc-afad081d9881`), workspace
`51a41279-bc88-41a9-b0d5-4ef13a0f1a9d`. Never mutate another linked project.

| Environment | Source branch | Public origin | Volume / bucket |
| --- | --- | --- | --- |
| production | `release` | `https://deck-builder-production-00d7.up.railway.app` | `production-data`, 5000 MB at `/data`; `backups-production` |
| staging | `main` | `https://deck-builder-staging.up.railway.app` | `staging-data`, 5000 MB at `/data`; `backups-staging` |

Exactly **one replica per environment**, no sleep/serverless; the web process
schedules refresh/backup children on the same volume. **Production is the primary
database**: never deploy a local database over hosted user data, and migrations
preserve/backfill all user content (D19). Production releases use an explicit
owner-verified SHA on `release`; staging follows `main`.

From the repository root: `docker build -f packages/deck-builder/Dockerfile -t deck-builder .`
The `node:26-slim` image builds the product with Vite and runs
`node ../multiverse/src/server.ts` with production dependencies only. The allowlist
`.dockerignore` admits product/backend/shared game inputs plus qualified SDK and
generated manifests prepared through their owning procedures (the image never
generates them); never admit `.env`, databases, tokens, caches, tests or `.git`.
Final-image checks verify SDK hashes and public imports; `HEALTHCHECK` uses
`/api/health` on 8787. Verify the actual image before promotion.

The image defaults to `USER node`. Railway needs `RAILWAY_RUN_UID=0` for its
root-owned volume; the entrypoint prepares only the data directory, then
`exec setpriv --reuid=node --regid=node --init-groups ...`, so Node and scheduler
children run as UID/GID 1000. SSH commands run as root: prefix database commands
with the same `setpriv`. Never leave a root-running Node service
([volume permissions](https://docs.railway.com/volumes/reference)).

Infrastructure lives in `.railway/railway.ts` (CLI 5.63.1; `railway@3.16.0` in
`.railway/package-lock.json`, install with `npm ci --prefix .railway`). It picks
`release`/`main` from the linked environment, keeps secrets with `preserve()`,
stores no identities, credentials or domains, and sets `deploy.drainingSeconds: 30`
so a refresh child can release SQLite before SIGKILL ([teardown](https://docs.railway.com/deployments/deployment-teardown)).
Plan before changing either graph and review every delete:

```sh
railway link --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --service deck-builder
railway config plan
railway link --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder
railway config plan
```

Never use `config pull --include-variables`: it writes unsealed secrets to source.
Both graphs deliberately omit claim codes. Billing alerts and native backup
schedules are not config-as-code.

## Logs and monitoring

Request IDs correlate logs and never authorize. Railway owns
[log retention](https://docs.railway.com/observability/logs) and parses application
JSON into fields; filter on them or inspect `.message`:

```sh
railway logs --environment production --service deck-builder --lines 200 --json | jq 'select(.user_id == 1 or (.message // "" | contains("user_id")))'
```

External monitor (owner action in UptimeRobot): HTTP(s) monitor `deck-builder
production` on `https://deck-builder-production-00d7.up.railway.app/api/health`,
5-minute interval, owner's alert contact; optionally repeat for staging. Also
watch Settings job failures and backup age.

The workspace **soft email alert is $25** for the whole workspace, not a project
cap; leave any hard cap alone:

```sh
railway usage limit status --workspace 51a41279-bc88-41a9-b0d5-4ef13a0f1a9d --target workspace --json
railway usage limit set --workspace 51a41279-bc88-41a9-b0d5-4ef13a0f1a9d --target workspace --soft 25 --json
```

Native volume [backups](https://docs.railway.com/volumes/backups) need a Pro
workspace and are not enabled; if the owner upgrades, enable Daily and Weekly per
environment. They never replace online backups or the restore drill.

## Rollback and restore safety

**Stop before an old-image rollback.** Railway restores that deployment's
variables as well as its image. Any production deployment before
`b7c4ec34-dc77-47e5-acc4-ab8bbc34d235` brings back `DECKBUILDER_CLAIM_CODE` and can
revert current secrets/draining settings. A claim code is dead while user 1 is
claimed but dangerous with an unclaimed restore. Prefer an owner-promoted
fix-forward SHA ([rollback behavior](https://docs.railway.com/guides/roll-back-bad-deploy)).
Compare schemas before stopping anything:

```sh
ROLLBACK_SHA='paste-the-verified-target-SHA'
curl --fail -sS https://deck-builder-production-00d7.up.railway.app/api/health
git show "${ROLLBACK_SHA}:packages/deck-builder/src/db.ts" | sed -nE 's/^export const SCHEMA_VERSION = ([0-9]+);/target schema: \1/p'
```

Versions must match for a code-only rollback; older code refuses a newer schema
(`assertOurs`), still causing downtime. There are no down migrations. For a
code-only regression redeploy the previous good deployment (dashboard Redeploy, or
`railway redeploy --environment production --service deck-builder --yes`, not
`--from-source` for bad source) and recheck its ID and health. `railway down` halts
an instance but keeps the volume. Never delete the service, project, volume or
bucket. A data rollback (maintenance mode, pre-change backup, compatible code) loses
later writes: reconcile and repeat deletions/revocations. Platform rollback never
rewinds SQLite; do not force-reset `release` casually.

**Never restore the pre-claim production snapshot.** `backups/deck-builder-2026-10-25.db`
resets user 1 to `auth_id NULL`, locking the owner out or permitting a new claim.
Its copy `archive/pre-claim-deck-builder-2026-10-25.db` (73,711,616 bytes) is
marked `restore_policy=do-not-restore`; both are quarantined owner-only evidence and
retention never touches `archive/`. For recovery choose a fresh post-claim
snapshot, open it read-only and confirm user 1's `auth_id` is non-null, the schema
is supported and deck/memory counts match. Check identity, not filename dates.

## Secret rotation and owner claim lifecycle

Secrets are sealed Railway variables; never print `railway variable list` or
bucket-credential output. Feed values on stdin from private files without a
trailing newline:

```sh
railway variable set DECKBUILDER_CLAIM_CODE --stdin --environment production --service deck-builder < ~/.config/deck-builder/claim-code
```

Owner bootstrap files are `~/.config/deck-builder/claim-code` and the distinct
`claim-code-staging`, mode 0600. Auth secrets differ per environment; Discord
credentials are shared only by explicit owner choice with both redirects.

Owner claim: open `/signin` on the environment's origin, paste the code into
**Invite or owner claim code** and sign in with Discord. Confirm user 1 has a
non-null `auth_id`, remains admin and matches the expected roster. After the
`owner_claimed` event:

```sh
railway variable delete DECKBUILDER_CLAIM_CODE --environment production --service deck-builder
railway redeploy --environment production --service deck-builder --yes
```

Never reset `auth_id`, re-enable claims or reuse production's code in staging. A
staging restore of a claimed production owner signs in with the same Discord
identity and staging's own origin/secret; provider tokens are not portable across
secrets, so never copy staging credentials back. If an approved **staging-only**
restore leaves user 1 unclaimed, generate a fresh 32+ character code into
`claim-code-staging`, set it sealed, redeploy, claim, then delete the variable and
redeploy again. Never use this as a routine production step.

Rotation: create a new personal token, update the client, verify `start_session`,
then revoke the old one. For Discord, set the regenerated sealed secret in **both**
environments and redeploy both. Avoid rotating `BETTER_AUTH_SECRET` (it invalidates
sessions and provider tokens); if required, take a fresh backup and pipe an
in-memory value to `railway variable set BETTER_AUTH_SECRET --stdin`. For a bucket,
reset only that environment's credentials, set both keys sealed, redeploy and verify
a backup object.

## Move an instance or rehearse a bucket restore

Never copy a bare database from a running instance or change the laptop's live
`data/`. Copy with `scripts/online-copy.mjs` and validate/migrate only the
destination. Give the destination its own volume, origin and secrets; do not copy
token files or `.env`. Before replacing a destination, back it up, warn users and
switch to a maintenance deployment that holds the volume without opening SQLite:

```sh
# Set the destination environment explicitly. This example is STAGING.
railway environment edit --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --json <<'JSON'
{"services":{"94e7ffb1-12ce-4fd0-a8cc-afad081d9881":{"deploy":{"startCommand":"node -e 'require(\"node:http\").createServer((q,s)=>{s.writeHead(q.url===\"/api/health\"?200:503);s.end(\"maintenance\")}).listen(8787,\"0.0.0.0\")'"}}}}
JSON
```

Deploy the environment's branch, wait for that exact deployment's `SUCCESS` and
confirm `/api/health` says `maintenance`. Never use an old-image rollback for
maintenance (it restores that image's start command). Wait for the new instance
before SSH; `railway api` can inspect a deployment's `instances { id status }`.

Take the source from the production **bucket**, not its live file. Back up as UID
1000, then download (credentials stay in memory; the file is verified as post-claim
and never overwritten), upload and restore:

```sh
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder -- 'setpriv --reuid=node --regid=node --init-groups node src/cli.ts backup'
node packages/deck-builder/scripts/fetch-bucket-backup.mjs --out packages/deck-builder/.dev/restore/production-bucket-backup.db
railway volume --environment staging --service deck-builder files --volume staging-data upload packages/deck-builder/.dev/restore/production-bucket-backup.db /restore-production.db --json
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --service deck-builder -- 'setpriv --reuid=node --regid=node --init-groups node src/cli.ts restore /data/restore-production.db'
```

Pass `--key backups/...` to choose a specific snapshot. Open the restored file
read-only and compare deck/memory counts, owner IDs, schema and
`PRAGMA foreign_key_check`. Then restore the normal start command, deploy, and
wait for `SUCCESS` and health:

```sh
railway environment edit --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment staging --json <<'JSON'
{"services":{"94e7ffb1-12ce-4fd0-a8cc-afad081d9881":{"deploy":{"startCommand":"sh /usr/local/bin/deck-builder-entrypoint node ../multiverse/src/server.ts"}}}}
JSON
curl --fail https://deck-builder-staging.up.railway.app/api/health
```

Verify UID 1000 in `/proc/1/status`. Restore as node so the database and sidecars
stay service-writable; if Node ever ran as root, repair ownership first. Remove
uploaded restore inputs after verification. A different public origin needs
provider redirects and OAuth reconnects.

## Server won't start: stale lock recovery

Locks contain `{pid, hostname, token}`. Startup, restore and CLI commands reclaim a
lock from a foreign hostname, a dead PID or their own reused PID (including
container PID 1); a live same-host holder or a second in-process acquisition still
refuses.

If an older deployment is crash-looping, do not redeploy repeatedly or delete
locks blindly. Pause retries, enter a maintenance container on the same volume and
inspect hostname/PID and lock metadata (never auth/token files). Confirm no web,
refresh, backup or restore process is alive; take an online backup if possible.
Only then remove `/data/deck-builder.db.server-lock` and deploy compatible code.
The separate `meta['job:lock']` lease follows its own two-hour rule; never clear
it while its child is alive.

## Authenticated smoke tests

Use the owner's browser for sign-in/session/SSE checks. For CLI checks the owner
creates a labelled token in hosted Settings → Agent connection and saves it to a
private mode 0600 file (for example `~/.config/deck-builder/hosted-smoke-token`).
Read the file inside the MCP test process, never via argv/chat/logs/PRs; call
`start_session`, verify the principal and roster, then revoke the token and delete
the file. Never create or borrow session rows, call `internalAdapter.createSession`,
sign cookies with `BETTER_AUTH_SECRET` or add an impersonation path. There is no
emergency admin credential CLI.

## Independent monthly backup copy (owner action)

Buckets and volumes share a provider. Monthly, after a successful production
backup, download the newest post-claim snapshot to the owner's machine or an
encrypted removable drive (repository root, logged-in Railway CLI):

```sh
node packages/deck-builder/scripts/fetch-bucket-backup.mjs
```

It writes `~/.local/share/deck-builder-offline/<YYYY-MM>/` (0700/0600), never
deletes bucket objects or overwrites files, and verifies integrity plus the
claimed owner. Store the copy independently, keep six monthly copies and repeat
erasures/revocations if recovering one. The pre-claim quarantine stays excluded
from restore selection.

## Owner release promotion

Promotion is an explicit owner action, never an agent task or a side effect of
development. Select one reviewed SHA after staging checks,
schema/migration/content review, refresh/store checks and an owner session/SSE
smoke. Immediately before promotion take a pre-promotion snapshot (below), then
push that SHA to `release`; never push whatever `main` names. Follow the exact
deployment to `SUCCESS` (a build upload is not success), then check health/schema,
owner roster, jobs and backups. Protect `release` against unreviewed updates,
deletion and force pushes with an explicit owner bypass, or record the plan
limitation that prevents it.

### Pre-promotion online snapshot

From the repository root, run `scripts/pre-promotion-snapshot.mjs` inside the
production container as UID 1000. It opens SQLite read-only, requires a claimed
user 1, uploads to a unique `archive/pre-promotion/` key (outside pruned
`backups/`) with overwrite protection and verifies the stored size. Keep the
printed key; do not promote if it fails.

```sh
railway ssh --project 2c9c40a2-4b69-4d5e-a2c4-188a6f5894b8 --environment production --service deck-builder -- "setpriv --reuid=node --regid=node --init-groups node --input-type=module <<'JS'
$(cat packages/deck-builder/scripts/pre-promotion-snapshot.mjs)
JS"
```

### Archive retirement policy

Automatic jobs never list or delete `archive/`. Retain each pre-change snapshot
for **at least 30 days after the change**, and longer while rollback to that schema
remains an option. Before retiring one object, verify the new schema/data, a
successful restore of a newer backup and an independent off-Railway copy, and
record the exact key and the owner's decision. Deletion is a manual owner action
on one verified key, never a wildcard job. The pre-claim quarantine keeps its
**do-not-restore** restriction. Documentation maintenance grants no cleanup
permission for archives, quarantined snapshots, live databases, credential files
or backup objects.
