# Multiverse desktop client

An isolated React / Vite / React Router / CSS Modules application. This checkpoint
is an inspection foundation, not connected human play or M1 acceptance. The Join
route explains the boundary; it does not collect credentials or make requests.

From the repository root, using its pinned Node 26.11.1:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run dev --workspaces=false
```

The local install and lockfile deliberately avoid changing the root workspace or
shared dependencies. No command builds Rust or generates SDK artifacts. `build`
creates ignored `dist/`; `check` runs types, source boundaries, pure Node tests and
that production build. Native and SDK preparation are separate explicit operations.

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

Source-qualified fixtures and producer presentation exports precede the data
adapter. Committed SDK browser exports and generated types precede live wiring.
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
