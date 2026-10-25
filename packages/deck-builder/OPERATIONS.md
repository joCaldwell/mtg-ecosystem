# Local operations

Use Node 26 (`nvm use`, `.nvmrc`). The package declares Node >=24.12 for
stable native TypeScript stripping; `src/sqlite.ts` uses the renamed Node 26
`Database` class and the equivalent constructor on Node 24. All connections
use WAL, foreign keys and a 5-second SQLite busy timeout.

Run commands from `packages/deck-builder`. No build is needed for server
TypeScript. `npm start` builds the browser assets and starts the local server;
`npm stop`, `npm run status`, and `npm run logs` manage the laptop launcher.
The server binds only to `127.0.0.1`. Authentication and hosting are later v3
phases.

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
  thereafter. Backup runs at 02:00 in the server's local timezone; an overdue
  backup runs on the next scheduler tick (30 seconds). Only one job runs at
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
job statuses are marked failed. The token lives outside SQLite and is kept.

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
| `DECKBUILDER_DATA` | Package `data/`; cache, token and backups live here |
| `DECKBUILDER_DB` | `DATA_DIR/deck-builder.db`; also determines server lock path |
| `DECKBUILDER_PORT` | `8787` |
| `DECKBUILDER_WEB_PORT` | `5173` in the dev script |
| `DECKBUILDER_TOKEN` | Optional fixed agent token; disables in-app rotation |
| `BACKUP_S3_*` | Optional off-box backup credentials above |

Server startup explicitly creates a missing token with owner-only file
permissions. Reading it never creates or changes anything. The stdio shim
fails with a clear message if none is configured. Without an environment
override, Settings can rotate the token and closes the previous MCP handler.
Settings never exposes an absolute server filesystem path: run its relative
stdio command from the package directory.

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
