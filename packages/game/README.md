# Multiverse game application

Setup, public exports and commands for this package. Ownership rules are in
[the package manual](AGENTS.md); behavior and remaining gates are in
[client contracts](../../docs/game-client.md). Multiverse composes `/server` with
the existing deck/account host in one process; `@multiverse/game-client` preserves
the former browser exports.

## Public integration boundary

- `/preview` exports router-free `OpeningTablePreview()` over the qualified captured
  opening. The host supplies account/navigation, constrained height and deduplicated
  React; scoped CSS Modules use `--game-*`, with no iframe, app shell,
  auth/session/SDK runtime or host-document styles. Confirmation changes local
  feedback only. Public declarations need no SDK-generated inputs.
- Node-only `/verify-preview` exports `verifyPreviewFixtures()` for consumer builds,
  checking recorded fixture hashes and capture/catalog/viewer/source binding without
  native/SDK generation. Historical fixture provenance is no fresh native acceptance.
- `/session` exports `GameSession({gameId, bootstrapFile, headingLevel, interactive})`
  over a memory-only imported File and the same-origin `/api/games/:id/native`
  endpoint with account cookies plus human seat authorization. Reload requires a
  fresh grant. `/session-qualification` supplies the pure profile/catalog/build fence.
- Harness fixture routes are `/`, `/fixtures/opening`, `/fixtures/missing-metadata`
  and `/fixtures/decisions`; `#table` opens the local interactive preview.

## Explicit setup

From repository root, use pinned Node and package-local locked installs:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-ui ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game ci --workspaces=false --ignore-scripts
MTG_SERVER_SCHEMA=/absolute/protocol.schema.json \
MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/bootstrap.schema.json \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game run check --workspaces=false
```

Obtain native exports matching [`sdk-qualification.json`](sdk-qualification.json)
and keep one SDK through verification and client checks; never substitute
historical fixture declarations. [Native delivery](../../docs/game-client.md#native-delivery-and-operator-procedure)
owns export/provision/serve commands and private file boundaries.

For Vite development, explicitly allowlist the upstream:

```sh
MTG_CLIENT_UPSTREAM=http://127.0.0.1:18741 \
sh scripts/engine-env.sh npm --prefix packages/game run dev --workspaces=false
```

Open numeric `http://127.0.0.1:5173/join`, import exactly one seat, then `/table`.
Unconfigured Vite serves fixtures; live config then fails explicitly.

## Focused checks

`check` verifies qualified fixture/SDK receipts, strict types, boundaries, pure
tests and the production build. `test:browser` builds and serves production
fixtures on 4179. `test:tooling` covers `browser-checks/` safety/auth tooling.
Browser installation is explicit:

```sh
sh scripts/engine-env.sh node packages/game/node_modules/@playwright/test/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game run test:browser --workspaces=false
MTG_CLIENT_LIVE_ORIGIN=http://127.0.0.1:18741 \
MTG_CLIENT_SEAT_FILES=/private/seat-0.json,/private/seat-1.json,/private/seat-2.json,/private/seat-3.json \
sh scripts/engine-env.sh npm --prefix packages/game run test:browser:live --workspaces=false
```

Live smoke needs an independently running qualified host (or configured proxy) and
four human-owned seats with no bots; it starts neither host nor Cargo. It checks
separate permitted views, duplicate-tab lock ownership, reimport, CSP, empty
persistence and GET-only traffic; injected faults are labelled. Fixture and opening
proofs do not qualify full games, Safari or human usability; see
[acceptance](../../docs/game-client.md#reproducibility-and-current-acceptance).
