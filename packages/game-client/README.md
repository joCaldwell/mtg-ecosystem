# Multiverse desktop client

An isolated React / Vite / React Router / CSS Modules application. This checkpoint
renders one source-qualified opening hand and its permitted public commanders. It
is not connected human play or M1 acceptance. The Join
route explains the boundary; it does not collect credentials or make requests.

From the repository root, using its pinned Node 26.11.1, first prepare the SDK
from the native lane's **explicit qualified integration exports** (never from an
arbitrary local server). The current integration protocol SHA-256 is
`18d1b8007acca14e0909fabdeaa6e3aa191598c1dddc4515b6301a3eefa1037c`;
bootstrap SHA-256 is
`0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
These exports come from native source `5d9c6fbd24bc4745cb66e18a0c32dfe1f9b910c4`
(schema-1 blocking-target support and native integration). Obtain the retained exports from the native
build owner, or explicitly build this integrated source under the shared native
verification lock and export `schema` and `bootstrap-schema`. No client command
performs that build. The native handoff report records the retained locations.

The fixture's original capture source and schema hash remain unchanged in
`fixtures/foundations-opening/provenance.json`; `sdk_qualification` separately
pins the current schema used to validate those historical bytes. The SDK and client
use one current SDK build, without replacing it with historical declarations.

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_SCHEMA=/absolute/path/to/protocol.schema.json \
MTG_SERVER_BOOTSTRAP_SCHEMA=/absolute/path/to/bootstrap.schema.json \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run build:browser --workspaces=false
```

Then prepare/check the client (no native binary is needed):

```sh
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run dev --workspaces=false
```

The local install and lockfile deliberately avoid changing the root workspace or
shared dependencies. Client commands never build Rust or generate SDK artifacts.
`prepare:fixtures` verifies exact fixture/schema hashes, validates the captured
resume and public presentation, and generates ignored presentation declarations
using the SDK's locked `json-schema-to-typescript@16.0.0`. `build` repeats that
fixture gate before creating ignored `dist/`; `check` also runs strict types,
source boundaries and pure Node tests. Native/SDK preparation stays explicit.

Focused browser checks run the production bundle under a restricted CSP, with no
API connectivity. Install the exact Playwright browsers once if absent:

```sh
sh scripts/engine-env.sh node packages/game-client/node_modules/@playwright/test/cli.js install chromium firefox
sh scripts/engine-env.sh npm --prefix packages/game-client run test:browser --workspaces=false
```

This starts and stops a loopback Vite preview at port 4179. JSON results, retained
failure traces and screenshots go to ignored `generated/`. The preview CSP is a
fixture test policy; it does not establish the future native host's serving policy.

## Package ownership

`app` composes declarative routes; `features` owns the table and inspectors;
`cards` renders text faces; `ui` supplies protocol-free primitives; `presentation`
holds local display models. `session` and `decisions` are framework-independent.
See [AGENTS.md](AGENTS.md) for enforceable dependency and integration constraints.

Pure primitives currently cover authorization-generation fencing, preserving the
last observation across lifecycle/outcome publications, token-bound ordered
selection and exact quantity syntax. `CommandJournal` and `ExternalStore` are
interfaces, not implemented persistence or a connected controller. No action
availability, rules, automatic choices, transport retries or recovery is simulated.

## Integration gates

The adapter consumes the exact native-lane HTTP observation and public producer
allowlist; only observed identities enter display models. `/fixtures/missing-metadata`
uses the same observation with metadata deliberately omitted. Artwork is never
loaded. Multi-face mapping is not inferred; no active face is guessed. Only the
mulligan request has a reviewed read-only inspector; the exhaustive generated
request switch stops visibly for every other request. This is not decision-control
coverage or proof that the fixture follows live client authentication.

Live wiring still requires the committed native/SDK contracts to be exercised in
the client controller.
Web Locks, credential-file validation/reimport, coherent resume, IndexedDB exact
pre-send journaling, identity/cursor validation, bounded retry and all associated
fault tests remain required. The normative acceptance matrix is in
`docs/game-client-delivery.md`; none of M1–M4 is accepted by this package alone.

Design: a restrained tabletop with stable seat columns, a central board, text card
faces and a separate inspector. Keyboard selection, focus restoration, an explicit
jump to the decision and reduced-motion styling retain access without dragging.

Framework references checked during implementation:
[React Router declarative routing](https://reactrouter.com/start/declarative/routing),
[Vite JSX and CSS](https://vite.dev/guide/features).
