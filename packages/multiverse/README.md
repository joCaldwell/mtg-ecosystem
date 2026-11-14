# Multiverse application

The account-backed product combines Home, the deck library/editor and Play setup.
Read [AGENTS](AGENTS.md), [architecture](web/ARCHITECTURE.md) and
[design](web/DESIGN.md). Backend identity/data remain in deck-builder behind its
public host/contracts exports; the captured table preview does not start a game
or use a selected deck. [Launch gates](web/ARCHITECTURE.md#game-launch) cover real play.

## Local setup

From repository root, use pinned Node and each package's isolated lock:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-ui ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse run dev --workspaces=false
```

Existing backend launch commands delegate to the product manager, preserving
backend `.dev` lease/log and storage defaults. Use explicit disposable DB/DATA
paths for tests. Product builds verify exact preview provenance without generated
native/SDK artifacts; standalone game-client setup remains independent.

## Native admission configuration

Admission is opt-in and does not enable real games. Supply all five settings:
`MULTIVERSE_NATIVE_EXECUTABLE` (absolute executable path),
`MULTIVERSE_NATIVE_CATALOG_SHA256`, `MULTIVERSE_NATIVE_PROFILE`,
`MULTIVERSE_NATIVE_ENGINE_BUILD_ID` and `MULTIVERSE_NATIVE_SERVER_BUILD_ID`.
Use independently qualified native artifacts and their actual pins, never guessed
values. Partial configuration refuses startup; absent configuration reports
unavailable. No product command builds Rust or generates SDK artifacts.

The adapter invokes `mtg-server admit-client` with bounded version-1 JSON stdin,
checks the exact normalized returned deck and native identity, bounds stdout,
discards stderr and passes only the runtime library path rather than application
credentials to the child. It has no shell or arbitrary native command surface.
Native rejection stays distinct from process failure and stale deck revision.
The existing offline registry CLI cannot supply a recoverable account launch or
three supervised consumers; its lifecycle adapter remains unqualified, as does
the browser live session. Start game remains disabled even after admission succeeds.

## Verification

Run product `check` (server/browser types and tests), `build:web`, then from this
package `node scripts/check-public-bundle.mjs`. Capability/identity changes also
use the [backend gate](../deck-builder/AGENTS.md#focused-test-gate); shared display
and session changes use their own package checks. [Browser regressions](../deck-builder/browser-checks/README.md)
cover disposable cross-app behavior. [Client evidence](../../docs/game-client-evidence.md)
binds historical source/acceptance and omissions.
