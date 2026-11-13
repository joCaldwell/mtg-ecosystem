# Multiverse desktop client

An isolated React/Vite client with live local joining and a read-only table.
Import the operator's generated single-seat bootstrap at `/join`, then open
`/table`. The session validates identities, owns a Web Lock, reads a coherent
resume and polls permitted updates. It never submits choices or automatic moves.
Capabilities stay in memory; refresh requires reimport. The original inspection
fixtures remain at `/`, `/fixtures/opening` and `/fixtures/missing-metadata`.

## Explicit setup

Use pinned Node 26.11.1 through `sh scripts/engine-env.sh`. Never run a root npm
install, share node_modules or invoke Cargo from client/SDK commands. Install both
packages locally with their own lockfiles:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
```

Obtain the serving owner's exact native exports for source
`6bf0609eed12273f6dae44d7cc97766299ebbcc4` (protocol/bootstrap bytes are
identical to the SDK prerequisite `bb41898b5eeca0c3357c63f03ecdf7180c51e76c`):

- Protocol SHA-256: `59deeafe34996f38e7a8905df9c75e9c4c424d54e9ae33bed2a4b23634f99bc6`.
- Bootstrap SHA-256: `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.

Generate and verify the SDK explicitly, then check/build the client using that
same SDK. Do not replace it with historical fixture declarations between checks.
`sdk-qualification.json` pins current validation separately from unchanged capture
provenance in `fixtures/foundations-opening/provenance.json`.

```sh
MTG_SERVER_SCHEMA=/absolute/protocol.schema.json \
MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/bootstrap.schema.json \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
```

The native operator exports an explicit allowlist from `dist/`, provisions a
private match and four individual seat files, and serves the production bundle,
API and catalog-bound presentation artifact at one numeric loopback origin.
Follow [the serving contract](../../docs/game-client-local-serving.md) for the
exact export/launch commands and native source identities. Never place operator
credentials or seat files under client/dist, public bundle or presentation roots.

For a Vite development session, explicitly allowlist the upstream:

```sh
MTG_CLIENT_UPSTREAM=http://127.0.0.1:18741 \
sh scripts/engine-env.sh npm --prefix packages/game-client run dev --workspaces=false
```

Open `http://127.0.0.1:5173/join`, not a localhost alias. Vite serves its public
`/client-config.json`, checks local Host/Origin before proxying, and forwards only
read routes to the configured upstream. An imported file cannot change that
mapping. Production native mode supplies the same config schema. No wildcard
CORS or cross-origin credential fetch is used. A plain unconfigured Vite instance
still serves fixtures, but live config fails explicitly.

## Focused checks and evidence

`check` verifies fixture hashes, current SDK source/build/schema receipts, strict
TypeScript, import/effect boundaries, pure/controller tests and the production
runtime-module allowlist. No native schema/SDK generation happens implicitly.
`test:browser` builds and starts an isolated production preview on port 4179,
checks the fixture, keyboard/focus, narrow/reduced-motion and disconnected join
routes, then stops it. Use the locked Playwright browser installs:

```sh
sh scripts/engine-env.sh node packages/game-client/node_modules/@playwright/test/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game-client run test:browser --workspaces=false
```

`test:browser:live` requires the independently running, source-qualified native
host (or the explicitly configured production preview proxy), with four separate
human-owned seats and no bots. Set paths only; never print capability contents:

```sh
MTG_CLIENT_LIVE_ORIGIN=http://127.0.0.1:18741 \
MTG_CLIENT_SEAT_FILES=/private/seat-0.json,/private/seat-1.json,/private/seat-2.json,/private/seat-3.json \
sh scripts/engine-env.sh npm --prefix packages/game-client run test:browser:live --workspaces=false
```

Real-host tests verify four authenticated permitted views, separate browser
contexts, same-profile duplicate-tab exclusion/release, reimport after refresh,
CSP, empty browser persistence and GET-only traffic. Separately labeled injected
browser faults test metadata loss/corruption, late responses and HTTP errors.
Traces, HAR, videos and automatic screenshots are disabled for authenticated
tests so Authorization headers cannot be captured. Reports retain public
identities/counts only. Preserve failed evidence before repeating a run.

See [LIVE-SESSION-REPORT.md](LIVE-SESSION-REPORT.md) for exact tested source,
schema/artifact/browser identities, failures, omissions and acceptance boundaries.
The foundation report and historical fixture provenance remain historical evidence.

## Ownership and remaining work

`app` owns composition/lifetime; `session` owns identity, one read loop, ownership
and bounded recovery; `session/effects` owns SDK/file/browser I/O. Components
consume immutable permitted snapshots and format human-readable views. Core
semantics and visibility remain authoritative; the SDK validates wire data.
Missing metadata keeps observed identity and characteristics; no card facts,
active faces, legal actions or source-ability labels are inferred.

This is read-only M1 and the read-side session foundation for M2. Persistent
exact-envelope journaling, submission/reconciliation, playable decision controls,
action/context integration, natural games and M2–M4 acceptance remain separate.
Existing journal interfaces specify unresolved-record preservation but implement
no storage or sending. Safari and human usability studies are not claimed.
