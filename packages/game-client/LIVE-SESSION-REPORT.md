# Live read-only client and session foundation

Worktree: `feat/game-client-live-session`, T3-bound under
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-live-s-4bbe9a746c2777b6`.
Initial HEAD and freshly fetched origin/main were both
`0b674d8872087bc96f6078d0daeb1fb2d6c17758`.
Authored ownership: `packages/game-client/` only. No Cargo, root install,
shared node_modules, game submission, new thread/agent, deployment or PR merge.

## Agreed integration contract

Serving peer owns native host and SDK additions. Session peer owns Vite and client.
`/client-config.json` provides Rust-generated, SDK-validated `ClientConfig` with
an explicit upstream SocketAddr origin. Native mode maps to its own listener;
Vite maps only to the operator-configured `MTG_CLIENT_UPSTREAM`. Bootstrap origin
must exactly match that configuration before constructing an authenticated SDK.
The browser always sends to its current origin. Vite checks exact numeric local
Host, optional same Origin and Fetch Metadata before rewriting upstream headers.
It forwards only generated read routes and digest presentation assets; commands
and arbitrary filesystem/upstream paths are excluded. CORS is disabled.

`presentation()` returns a Rust descriptor and producer-schema-generated artifact.
SDK owns bounded reads, wire/schema validation and content digest checking. The
session compares catalog with the bootstrap/manifest/resume. Missing (404)
metadata produces explicit observed-identity/current-characteristic fallbacks;
invalid metadata or incompatible identity stops the session. Artwork stays unloaded.

SPA paths agreed: `/`, `/join`, `/table`, `/about`, `/fixtures/opening`, and
`/fixtures/missing-metadata`. `/` retains the historical fixture. The separate
`/table` route displays live permitted snapshots only.

## Implementation boundaries

The pure read controller injects authorization, transport, cancellation, lock and
timer effects. It owns exactly one loop and latest immutable read snapshot, with
no command API or persistent storage. It acquires a Web Lock before authenticated
reads. A denied lock has no hidden waiter/takeover; explicit reimport is required.
Missing Web Locks rejects clearly. Stop/reimport immediately clear private views,
abort work, release ownership and fence late authorization/lock/resume/poll results.
React subscribes with `useSyncExternalStore`; app lifetime owns cleanup.

Resume validates protocol, match, authenticated viewer, engine/server builds,
catalog, read profile and observation version before atomic installation. Pending
resumes retain an earlier permitted observation in the same binding. Stream pages
validate all replacements before any publication, and lifecycle/outcome entries
retain the last observation. Host fault/pause/finished statuses do not invent a
rules outcome. Only the supplied outcome ends polling.

Poll limits: 32 entries, 20-second server wait, 30-second client deadline including
body decoding, at least 100 ms between successful reads. Transient retries use
500/1000/2000/4000 ms, then stop and release ownership. Recovery reads coherent
resume; a successful resume alone cannot reset a persistently failing poll budget.
429 has a separate cumulative per-authorization limit; empty successes cannot
hide repeated contention. HTTP 401/403, invalid protocol/identity, HTTP 429, HTTP
503 and disconnect have distinct fixed, credential-free messages. Reimport is
manual. No cross-device ownership or rightful-controller inference is claimed.

Components receive frozen permitted snapshots or derived readonly display models.
They do not fetch, read files, access storage or import SDK runtime. Join passes the
explicitly selected File as user intent to the session adapter and clears the file
input. Only the adapter reads/validates it. Capabilities live only in the SDK's
memory/Authorization headers, never state snapshots, URL/history, storage or logs.
No service worker, analytics, journal or broadcast channel was added.

## Evidence in progress

Initial explicit schema input source:
`3372b40aba4f6eca29ea946d136745f83996b4c3`; protocol
`9457a3fb0d62606530a86166cd2c921b1a20646e9268d4f2e2242c9283d14134`, bootstrap
`0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
These were read-only copied from the native owner's retained export into this
client's `generated/initial-inputs/`. Initial SDK generation/browser build was
explicit and ran no Cargo. SDK strict types and 54 tests passed. Client pure tests
currently pass 32 cases; initial controller checkpoint also passed strict types,
boundary checks and production fixture build before the new SDK adapter was wired.
The final serving source/schema qualification and real-host evidence will replace
this progress section before handoff.

Meaningful intermediate failures retained: `generated/meaningful-target-type-failure.log`
caught an incorrect assumption that generated VisibleTarget had a single handle;
fixed to display supplied announced/current references. `generated/pending-sdk-contract-typecheck.log`
records the expected missing readClientConfig/presentation exports while awaiting
the serving peer's committed SDK prerequisite. Neither used a declaration shim.

## Separate M2 handoff

`CommandJournal`, exact-envelope `JournalRecord`, seat/build binding, definitive
execution disposition and `ExternalStore` interfaces remain in `session/contracts.ts`.
There is deliberately no journal implementation, command construction, send,
reconciliation, automatic move or synthetic legal-action surface. Next M2 owner
must add persistent-before-send exact validated envelopes, unresolved seat scan
across builds, durable pending/unknown reconciliation, and explicit human intent
against generated core action/context contracts. Accepted/pending, missing status,
local stop, refresh, changed observation and timeout cannot resolve a journal.
The read-side session is not full M0/M2–M4 acceptance or playable Commander.

## Source-qualified pre-host checkpoint

Serving prerequisite carried unchanged as the exact parent commit
`bb41898b5eeca0c3357c63f03ecdf7180c51e76c` (not a client-authored server/SDK diff).
Final serving protocol SHA-256:
`59deeafe34996f38e7a8905df9c75e9c4c424d54e9ae33bed2a4b23634f99bc6`.
Bootstrap schema is unchanged. `sdk-qualification.json` pins this source/schema
separately from immutable historical fixture provenance. Initial SDK outputs and
receipts were copied to `generated/initial-sdk-evidence/` before regeneration.

With one explicitly generated final SDK build: SDK verify passed strict types and
59 Node tests; client check passed strict types, source/runtime boundaries, 34 pure
and controller tests, fixture schema validation and production build. Ten fixture
browser scenarios passed (Chromium/Firefox, 5 each). Evidence:
`generated/serving-sdk-verification.log`, `serving-client-check-fixed.log`,
`fixture-browser-check.log`, `browser-report.json`, `fixture-verification.json`,
`bundle-inputs.json`. The first final-schema client typecheck caught a test closure
narrowing issue; `serving-client-check.log` retains it, fixed without changing DTOs.

Pre-host production artifacts:

| File | SHA-256 |
| --- | --- |
| SDK browser.js | `597cd1c25d4291234e26c2079216906f41f1e0fc8da17f9fb7ae48813cbcdb65` |
| dist/index.html | `a33b2fb7945d46d066eb6530f85290ff2bee4f3d9bca1dfad9b782b68047db78` |
| dist/assets/index-Ba7Op3pa.css | `8032b902c38ef615450a0ca0dfbf4046698b89f8bf04cb25c9f9a404704abbab` |
| dist/assets/index-Ds1idBNn.js | `fa9753165b5500959f3346924c3c6a9b879dca921d76a6e3a48f304d0a32c0b9` |

Real-host acceptance has not yet run at this checkpoint. The next report update
will distinguish native outcomes from injected HTTP/browser fault scenarios.
