# Desktop Commander client

Read the root manual and `docs/game-client-{plan,contracts,boundaries,delivery}.md`.
This package owns interaction, display and session recovery, never gameplay rules.
Current source supports live read-only joining plus retained inspection fixtures.
Source-qualified acceptance and remaining gates belong in LIVE-SESSION-REPORT.md;
read-side recovery does not establish playable decisions or full M2 completion.

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
`test:browser` runs focused Playwright checks against the production fixture build.
`test:browser:live` requires an explicitly supplied real host origin and four private
single-seat file paths; it does not start Cargo, a host or bots. Keep authenticated
traces/HAR/video off, and never record Authorization headers. Separate injected
HTTP faults from unmodified host evidence. Keep Chromium/Firefox versions and
exact source/artifact digests in the checkpoint report. No default full engine matrix.

## Live integration gates

Use only committed browser SDK exports after source/schema parity and CSP proof.
Requalify sdk-qualification.json from explicit producer exports when contracts
change; retain historical fixture/provenance bytes and use one current SDK build
for SDK and client checks. Never silently install historical schemas to make
fixture checks pass. Vite must match the agreed native Host/Origin, read-route and
explicit upstream configuration contract before forwarding credentials.
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
