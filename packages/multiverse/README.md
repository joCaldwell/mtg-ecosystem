# Multiverse application

The account-backed product combines Home, the deck library/editor and Play setup.
Read [AGENTS](AGENTS.md), [architecture](web/ARCHITECTURE.md) and
[design](web/DESIGN.md). Backend identity/data stay in deck-builder behind its
public host/contracts exports. [Launch gates](web/ARCHITECTURE.md#game-launch) cover real play.

## Local setup

From repository root, use pinned Node and each package's isolated lock:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-ui ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/multiverse run dev --workspaces=false
```

Backend launch commands delegate to the product manager, preserving backend `.dev`
lease/log and storage defaults. Use explicit disposable DB/DATA paths for tests.
Product builds verify preview provenance without generated native/SDK artifacts.

## Native admission configuration

Admission is opt-in and does not by itself enable games. Supply all five settings
from independently qualified native artifacts, never guessed values:
`MULTIVERSE_NATIVE_EXECUTABLE` (absolute path), `MULTIVERSE_NATIVE_CATALOG_SHA256`,
`MULTIVERSE_NATIVE_PROFILE`, `MULTIVERSE_NATIVE_ENGINE_BUILD_ID` and
`MULTIVERSE_NATIVE_SERVER_BUILD_ID`. Partial configuration refuses startup; absent
configuration reports unavailable. The adapter calls `mtg-server admit-client`
([contract](../../docs/game-engine-server.md#read-only-product-admission)) with no
shell, application credentials or arbitrary native commands; native rejection
stays distinct from process failure and stale deck revision.

The lifecycle bridge additionally needs `MULTIVERSE_NATIVE_LIFECYCLE_DIRECTORY`
(absolute private 0700 directory) and `MULTIVERSE_NATIVE_HUMAN_ORIGIN` (exact numeric
loopback HTTP product origin); `mtg-server lifecycle-profile commander-foundations-v1`
reports pins. Retain the executable and directory. The bridge runs at most two
concurrent games, each with one offline registry/server and three seat consumers
([lifecycle](../../docs/game-engine-server.md#product-native-lifecycle)). Start game
requires the approved foundations release and native deck admission. Shared
registries, remote HTTPS bootstrap, automated attachments and whole-profile play
are outside this qualification. No product command builds Rust or generates SDK artifacts.

## Verification

Run product `check` (server/browser types and tests), `build:web`, then
`node scripts/check-public-bundle.mjs` from this package (initial public JS under
110,000 gzip bytes). Capability/identity changes also use the
[backend gate](../deck-builder/AGENTS.md#focused-test-gate); shared display and
session changes use their own package checks. [Browser regressions](../deck-builder/browser-checks/README.md)
cover disposable cross-app behavior; [client evidence](../../docs/game-client.md#reproducibility-and-current-acceptance)
binds source-qualified acceptance.
