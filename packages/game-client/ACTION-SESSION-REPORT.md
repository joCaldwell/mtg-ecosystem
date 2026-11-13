# Session-owned action discovery

Base: clean `2aaf466b6a331846baea95116c6c0ebf26d953cb`, branch
`t3/game-client-action-query-lifecycle`. This is an opt-in pure session controller,
not App integration, a menu, command authority, another read loop or native acceptance.
Owned implementation: `src/session/action-query.ts` and two distinct test files.
Parent additionally authorized the **single** current-engine-map evidence SHA
correction in `fixtures/decision-controls/coverage.json`; no other row changed.
No existing controller, effect, App, SDK, pin, lockfile or native source changed.

## Composition contract

The sole read owner constructs an immutable `ActionQueryContext` only after its
exclusive ownership, manifest, actual resume viewer and exact identity checks:

- `authorization` and `generation`: opaque capability-free lifetime identities;
  replace on reauthorization, read-generation invalidation and decision replacement.
- `binding`: existing generated-bootstrap-derived `ReadBinding`, with actual origin,
  match, viewer, protocol version, engine/server builds and catalog.
- `protocolSchema`: the qualified SDK's exact schema digest; `token`: the current
  permitted decision token. Neither comes from a draft or inferred rules.

Inject `current()` (null until authorized/current), `authorize(context)` (returns
one closure over the exact original authorized SDK instance), monotonic `now`,
`operation(ms)` (abort signal, deadline promise, abort and disposal), abortable
`wait`, and a sanitized error classifier. The controller stores no credential.
An adapter calls `sdk.queryActions(request, {signal})` once per injected operation;
SDK owns serialization/validation only. The adapter must never resolve a different
viewer's credentials when a captured closure is later used for cleanup.

Before replacing/publishing the read owner, token, generation or seat, set the
current context accordingly and call `synchronize()` synchronously. Call and await
`stop()` before releasing ownership when possible. Stop aborts pending operations;
cleanup uses only the captured previous SDK. Every completion independently checks
current identity. Null initialization/revocation is supported. No API receives a
read cursor, journal, command callback or storage adapter.

An explicit discovery callback calls `start(ORIGINAL_QUERY_CONTEXT, scope)`.
Repeated start for the same live context/scope is idempotent, including completed
results. Changed scope retires the old intent. The promise returns **that intent's**
query generation (or null when refused), even if a newer intent superseded it.
A late completion must not read the latest snapshot generation and relabel itself.
`cancel(generation)` ignores cleanup from an older generation; `stop()` is idempotent.
A fresh explicit start after stopping/limits is a new budget, never an automatic retry.

Snapshots are immutable external-store values (`getSnapshot`, `subscribe`). They
contain local query generation, scope, generated core context and beginnings,
phase, bounded failure/reason/cancellation enums and resource counters. They contain
no token, continuation, authorization identity, credential or command envelope.
Scopes, handles and beginnings remain private authorized in-memory data; do not
serialize snapshots into URLs, logs, storage or shared telemetry.

| Snapshot phase | Consumer meaning |
| --- | --- |
| loading / incomplete | Work remains; entries are partial, possibly zero. Null context is retained only for empty incomplete initialization. |
| complete | Producer explicitly exhausted its supported initiation domain; empty is meaningful only here. This still grants no command authority. |
| stale / unavailable | Bound intent cannot be used. Worker reason remains faithful, including nested core reasons. |
| limited / uncertain | Local bounds or unknown continuation disposition; never a complete empty menu. |
| cancelled | Local intent stopped; only `cancellation: confirmed` establishes a matching server cancelled response. |

The opening/root composition must retain the **original complete CommandContext**
(binding, incarnation, revision and token) alongside the original query context
and scope before discovery. Associate that retained wrapper only with the returned
query generation; discard it on owner/decision/scope replacement. A menu callback
must carry that original wrapper and generated initiation, and verify matching
snapshot generation/phase and original command/read binding before delegating to
normal command validation/journaling. Never call `commandContext()` at callback
time to stamp an old result with newer authority. This controller cannot create
or submit a PlayerResponse. Missing labels/context remain explicit. The separate
parent-approved naming policy for Pass/PlayLand/Cast may consume generated kinds
and same permitted observed metadata; this module fabricates no labels or card facts.

## Bounds and uncertainty

Defaults: page 16; 250 requested work/call; 32 charged query attempts; 8,000 total
requested work; 256 aggregate entries; 131,072 aggregate serialized UTF-8 response
bytes; absolute 20-second chain; each request at most four seconds. Limits may only
be reduced. A 100ms inter-page wait stays below the worker's 16 calls/second; at
250 work this is also below 8,000 work/second. These conservative UI bounds are
smaller than native page32/work1000/calls128/work32000/TTL30s. Native request2KiB
and result64KiB boundaries remain SDK-owned; local token/handle and request checks
also run before query dispatch. Aggregate bounds count every received envelope.
At most one additional cancellation attempt per retired intent, no work, four-second
deadline; cancellation never replenishes the chain's charged counters.

Only HTTP429 (known admission rejection) and explicit worker quota may retry,
at most twice total, with a one-second wait. Failed attempts still reserve full
call/work budgets. Worker quota precedes continuation consumption in
`game-server/src/action_queries.rs`; no other error is assumed safe to replay.
401, 403, 503, protocol and request/body transport failures remain distinguishable
through the injected classifier. The unchanged SDK sanitizes both request/body
loss into TransportError; the representative adapter labels that coarser category
explicitly rather than pretending the SDK distinguishes the two.

Pages must match actual viewer/match/protocol plus original token/scope and owner
identity. Context may initialize from null but cannot change afterward. Duplicate
initiations (canonical object-key order), repeated cursors, changed contexts,
invalid matching page size and mixed generations cannot become complete results.
No Oracle text, IR, printed card facts or inferred eligibility is consumed.

Pending continue may have consumed its single-use cursor. Abort/lost response
therefore never cancels or retries that obsolete cursor blindly. If a late,
correlated response reveals a fresh own rotated cursor, cleanup can cancel that
cursor without publishing its entries. Otherwise disposition stays unknown and
relies on the worker's absolute 30-second expiry/invalidation. A foreign response
cursor is never cancelled. A stale cancellation response is reported as stale,
not confirmed. No local cancellation alters a game command or durable state.

## Executed verification and retained inputs

Pinned Node 26.11.1; package-local locked installs through engine-env, no Cargo.
One unchanged SDK build throughout: strict generation/types/build and **66 SDK
tests** passed. SDK sources match `e86df7e73f5ac896a4b2488779efb46ef28240d4`.
Client final check passed strict types, boundaries, **97 Node tests** (67 existing,
26 deterministic pure cases, four actual-built-SDK synthetic HTTP cases) and
production build. No skipped tests. Separate read-only compatibility replay passed
six retained native query fixtures through that built SDK/controller, with all
670 source-manifest rows checked against exact original Git blobs. This is fixture
compatibility via synthetic HTTP, not new live/native or actual App acceptance.

Evidence root, retained in this active workspace:
`generated/action-session/`. Original schema/native-info/source-manifest/public-prefix
bytes were copied to `inputs/` before authorized historical-worktree cleanup.
`inputs.sha256`, `retained-compatibility.json` and `qualification.json` retain hashes,
source/build identities, commands and receipts. Reproduction uses these local paths,
not a removed worktree. No historical receipt was rewritten or attributed to newer
source. The opening owner's newer producer exports are not this lane's producer
acceptance; unchanged schema bytes alone never renew native acceptance.

| Artifact | SHA-256 |
| --- | --- |
| Protocol / unchanged bootstrap | `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3` / `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| SDK dist/build.json | `793333249d3f65e391505bb346415ef5ec010141e5c0e7574a9d4e9ee27f8123` |
| SDK browser.js | `dc1c0139b03db602a0bb4b1873cc18b9ad74cbc8ea74fb0754948178171bbe2b` |
| SDK generation.json | `c8507de687345e9df5b911f3ddb12e072e8b52d7533ee901ce8268d3e26bcb5b` |
| Retained native source-after.json | `a6daefe2d6e2404004bbe0c7cde6fb72e9aea52d5b167308c1f863275857922d` |
| Retained public-prefixes.json | `a232fd5b2038df60143cb8f47fb052173f380daaad1b533e91e47be805f347ed` |

Reproduce from repository root with supplied local inputs:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts --no-audit --no-fund
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts --no-audit --no-fund
MTG_SERVER_SCHEMA="$PWD/generated/action-session/inputs/protocol.schema.json" \
MTG_SERVER_BOOTSTRAP_SCHEMA="$PWD/generated/action-session/inputs/bootstrap.schema.json" \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh node generated/action-session/qualify-retained.mts
```

Meaningful failures retained: `client-check-01.log` passed 91/92 cases; its sole
failure was **pre-existing source-inventory drift**, the current engine map's
stale evidence hash. Parent explicitly authorized regeneration; diagnostic and
tracked diff proved only that one SHA changed, with no qualification/provenance or
guard relaxation. The failing inventory case then passed alone and within full
checks. `query-02.log` retains a new test's initial failure: its auto-advancing fake
wait had already advanced beyond the intended cancellation checkpoint. The test
now holds that wait explicitly; the cancellation race and all final cases passed.
Existing chunk-size/React Router build warnings remain.

No browser suite was necessary for this uncomposed pure-only slice; SDK/browser
runtime and UI are unchanged. Browser CSP/native integration, actual App query
composition, native builds, broad matrices, natural games, performance, human
usability and deployment were omitted. Parent owns independent review and ordered
integration. Draft PR only; no merge by this lane.


## Independent-review corrections and current-main integration

Parent integrated current main `dc12222cd6251c3c20c3f15b4a0ae2e6ea19333a`
without conflicts, then corrected all three P2 findings from the independent
exact-head review of `edfe63a69818a8fc2a360a178418808aa0c841e2`:

- Monotonic request and chain deadlines are checked before dispatch, after adapter
  setup, after completed responses and before publication. Delayed timers cannot
  turn overdue results into complete menus. Only a fresh correlated returned
  continuation is eligible for bounded cleanup; completed overdue data is discarded.
  Cleanup also checks elapsed time rather than trusting timer scheduling.
- A retirement barrier is installed before abort, publication or cleanup callbacks.
  Reentrant stop calls and replacement intents await the same retirement, including
  pending cancellation. Cleanup installs its own promise before invoking adapters.
- Authorization callbacks are followed by another intent and current-owner check.
  Synchronous stop, replacement or authorization change cannot install the old run.

Final package `check` passed strict types, boundaries, **112 Node tests** (67 base,
41 pure query cases and four actual-built-SDK HTTP cases) and production build.
The 15 added cases retain the independent review reproductions, including three
real-timer/event-loop-stall probes, and cover adapter-setup expiry, overdue rotated
cursor cleanup, overdue cancellation and owner changes during authorization.
Six retained fixture compatibility replays passed again, with 670 exact original
Git-source fingerprint rows. The SDK build/browser hashes above remained unchanged.
No additional Cargo, native or browser acceptance was claimed for this pure module.
The compiler changes integrated from main do not renew either historical producer's
qualification. Original reports/receipts remain attributed to their actual source.

Parent evidence: `generated/query-review-fixes/{client-check.log,
retained-compatibility.log,qualification.json}`. The first parent test attempt
(`pure-first.log`) failed because a newly written TypeScript parameter-property
constructor is unsupported by Node's strip-only execution. It was replaced by an
explicit class field; subsequent pure tests and the full check passed. No guard,
limit, cancellation policy or historical provenance was relaxed to pass review.
Independent re-review and final combined integration remain parent gates.
