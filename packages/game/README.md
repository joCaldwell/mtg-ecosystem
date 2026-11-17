# Multiverse game application

`web/` retains the React/Vite harness, operator-issued joining, session recovery
and explicit source-bound decisions plus bounded live action discovery.
Shared permitted table/inspection comes from game-ui. Natural complete games and automated launch
retain separate [client gates](../../docs/game-client.md#delivered-scope-and-remaining-gates).

`server/` owns account-scoped admission/launch/recovery, native supervision and
`/api/games`; `shared/` owns coordination DTOs. Multiverse composes `/server` with
the existing deck/account host in one process. The public deck-builder
`/application` port supplies the verified Actor, SQL reservation and authorized
deck capture. Existing credentials, database migrations and deletion triggers
remain in place. `@multiverse/game-client` preserves the former browser exports.
Game integration tooling lives in `browser-checks/`; run `npm run test:tooling`
for its safety/auth tests. Explicit native inputs remain required for live checks.

## Public integration boundary

`@multiverse/game/preview` exports router-free `OpeningTablePreview()` over
the qualified captured opening. The host supplies account/navigation, constrained
content height and deduplicated React. Scoped CSS Modules use `--game-*`; no iframe,
second app shell, auth/session/SDK runtime or host-document styles enter this path.
Confirmation changes local feedback only, never a selected deck or game command.

Public declarations need no SDK-generated inputs; implementation parity is checked
inside this package. Node-only `/verify-preview` exports `verifyPreviewFixtures()`
for consumer builds, verifying recorded fixture hashes and capture/catalog/viewer/
source binding without native/SDK generation. Historical fixture provenance stays
unchanged and establishes no fresh native acceptance.

Fixture routes remain `/`, `/fixtures/opening`, `/fixtures/missing-metadata` and
`/fixtures/decisions`; its `#table` opens the local interactive preview.

Public `/session` exports `GameSession({gameId, bootstrapFile, headingLevel, interactive})`.
It owns a memory-only imported File/session lifetime and uses the app-selected
same-origin `/api/games/:id/native` endpoint with account cookies plus human seat
authorization. Match/seat binding is checked before requests; numeric-loopback
origin and source qualification still apply. Reload requires a fresh grant.
Public `/session-qualification` supplies the host's pure exact-profile/catalog/build
qualification fence independently of React and SDK runtime.

## Explicit setup

From repository root, use pinned Node and package-local locked installs:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-ui ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game ci --workspaces=false --ignore-scripts
MTG_SERVER_SCHEMA=/absolute/protocol.schema.json \
MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/bootstrap.schema.json \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game run check --workspaces=false
```

Obtain exports matching [`sdk-qualification.json`](sdk-qualification.json), the
canonical current native/schema pin. Keep one SDK through verification and client
checks; do not substitute historical fixture declarations. Client/SDK commands
never invoke Cargo or create native schemas. Do not replace shared root dependencies.
[Native delivery](../../docs/game-client.md#native-delivery-and-operator-procedure)
owns exact export/provision/serve commands and private file boundaries.

For Vite development, explicitly allowlist the upstream:

```sh
MTG_CLIENT_UPSTREAM=http://127.0.0.1:18741 \
sh scripts/engine-env.sh npm --prefix packages/game run dev --workspaces=false
```

Open numeric `http://127.0.0.1:5173/join`, import exactly one seat, then `/table`.
Capability is memory-only; refresh requires reimport. Config/Host/Origin guards
precede the allowlisted proxy. It admits reads, submissions/execution status and exact action-query/queued-command
cancel routes. Operator and arbitrary routes remain blocked. An imported file cannot
select upstream. Unconfigured Vite serves fixtures but live config fails explicitly.

## Focused checks and evidence

`check` verifies qualified fixture/SDK receipts, strict types, boundaries, pure
controllers and production runtime-module allowlist/build. No generation is implicit.
`test:browser` builds/serves production fixtures on 4179 and closes its preview.
Browser installation is an explicit setup step:

```sh
sh scripts/engine-env.sh node packages/game/node_modules/@playwright/test/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game run test:browser --workspaces=false
MTG_CLIENT_LIVE_ORIGIN=http://127.0.0.1:18741 \
MTG_CLIENT_SEAT_FILES=/private/seat-0.json,/private/seat-1.json,/private/seat-2.json,/private/seat-3.json \
sh scripts/engine-env.sh npm --prefix packages/game run test:browser:live --workspaces=false
```

Live smoke requires an independently running qualified host (or configured preview
proxy), four human-owned seats and no bots; it starts neither host nor Cargo.
It checks separate permitted read views/contexts, duplicate-tab lock ownership,
reimport, CSP, empty persistence and GET-only traffic. Injected metadata/network
faults are separately labelled. Authenticated traces/HAR/video/automatic screenshots
stay off; preserve public identities/browser versions, failed evidence and omissions.
[Evidence](../../docs/game-client-evidence.md) owns accepted sources; fixture and
opening proofs do not qualify full-game/Safari/human usability.

See [the package manual](AGENTS.md) for ownership and
[the session contract](../../docs/game-client.md) for authorization, exact command
journaling, unavailable context and remaining integration gates.
