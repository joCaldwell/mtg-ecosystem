# Desktop Commander client

Read the root manual and `docs/game-client-{plan,contracts,boundaries,delivery}.md`.
This package owns interaction, display and session recovery, never gameplay rules.
The first checkpoint is a read-only fixture surface, not M1 acceptance or live play.

## Boundaries

- `app` composes routes and session lifetime. Components never fetch or access storage.
- `session` owns eventual SDK effects, authorization, ownership and recovery.
  Pure session modules have no browser, React or SDK runtime dependencies. Inject
  effects. Generated SDK types are the only gameplay wire definitions.
- `decisions` contains pure local drafts: syntax, supplied candidates/constraints,
  order and binding. Never infer eligibility, costs or visibility from printed text.
- `presentation` contains pure display transformations. Only authorized projections
  and producer-owned public presentation metadata may enter it; never executable IR.
- `features` composes cards, decisions, presentation and generic `ui` primitives.
  `ui` knows no protocol; `cards` formats source-backed text without fetching it.
- Never import deck-builder private features, SDK example policies, trusted snapshots,
  compiler internals or private source fixtures into the runtime bundle.
- Fixture provenance belongs in `fixtures/`; raw generated evidence in ignored
  `generated/`. Obtain permitted fixtures and presentation artifacts from their
  producer lane. Never invent card names, printed values, images or Oracle text.

## Toolchain and checks

Use repository `.node-version` via `sh scripts/engine-env.sh` from the root.
This first slice uses a local isolated npm install/lockfile, always
`npm --prefix packages/game-client … --workspaces=false`. Do not run root installs,
replace shared node_modules or modify root dependencies without coordination.
No package command may implicitly invoke Cargo, generate native schemas or build
SDK artifacts. Native builds belong to the native lane under the shared build lock.

`check` runs strict types, import boundaries, pure tests and production build.
`test:browser` runs focused Playwright checks against the production fixture build;
it does not establish a live server integration. Keep Chromium/Firefox versions and
exact source/artifact digests in the checkpoint report. No default full engine matrix.

## Live integration gates

Use only committed browser SDK exports after source/schema parity and CSP proof.
Import a validated one-seat bootstrap through a file picker; keep capability only
in memory. Reimport after refresh. Verify actual viewer and all identity fields.
Web Locks must precede polling/sending; no forced takeover, broadcast secrets or
fallback ownership. Fence async completions by generation and authorization binding.
Journal exact validated envelopes in IndexedDB before sending; failure blocks send.
Accepted/pending and unknown outcomes retain the exact body/ID and original revision.
Scan unresolved origin/match/seat records before changing build/catalog. Never clear
ambiguity on local stop, missing status row or timeout. Gates and required fault
cases remain in the delivery plan until actual integration tests establish them.

Keep keyboard controls, visible focus, reduced motion, text fallbacks and deliberate
focus restoration. Remote updates announce without stealing focus. Commit/push
coherent focused checkpoints; draft PRs only, no incidental merge or deployment.
