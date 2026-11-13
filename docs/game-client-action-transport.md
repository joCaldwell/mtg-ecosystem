# Authenticated action queries and exact command bytes

This is the action-transport lane of the desktop client, based on clean main
`144fe059ab2707877109d7e9dbe8751f204681dd`. It adds bounded discovery transport,
not full M2, source labels, all decision contexts, a playable UI, deployment or a
new hosted profile. Core eligibility and deterministic semantics are unchanged.
The existing [core contract](game-client-core-queries.md) defines the supported
priority/mana initiation domain; the live code and this source-qualified report
supersede planning status prose for this slice.

## Wire contract and ownership

Rust `protocol::{ActionQueryRequest,ActionQueryResponse}` and `ROUTES` are the
schema authority. `POST /v1/matches/{match_id}/actions/query` authenticates the
seat capability from its Authorization header. It accepts no URL query string,
viewer field or other unknown fields. Nested duplicate JSON keys reject before
DTO decoding. HTTP parses/bounds input and forwards a private IPC command to the
existing match broker. Only that match's exclusive worker touches `Game`, using
`PlayerSession::{action_beginnings,resume_actions}`. No HTTP eligibility logic,
core changes, Oracle interpretation, new label mapping or policy was added.

Requests are `start {token,scope,page_size,work}`, `continue {cursor,work}` or
`cancel {cursor}`, tagged by `operation`. The result envelope carries version 1,
match ID, authenticated viewer and `outcome`. Outcomes:

| Outcome | Contract |
| --- | --- |
| complete {page} | Core exhausted its supported finite initiation domain; page may be empty. |
| incomplete {page,required_minimum:1,cursor} | Incremental core page; continue explicitly, with the new opaque cursor. Never interpret as an empty legal menu. |
| stale | Unknown, foreign, replayed, expired, invalidated or recovered cursor, or stale/foreign decision token; no private reason. |
| unavailable {reason} | Core reason (faithfully nested), quota, in_flight, total_work, response_limit or executor. Query status only, never a game loss/draw or host pause. |
| cancelled | This authenticated seat's live cursor was dropped. Repeating cancellation yields stale. No accepted command or rule action is cancelled. |

Context and initiation fields embed core `ActionPage` directly. All current
`ActionLabel` values remain `unavailable`. Ordinary spell/ability payment source
context remains unavailable where core supplies it; nested permanent-mana source
context passes through unchanged. Consumers must block ambiguous source-dependent
controls. Compiler source labels and richer semantic/source context are separate
consumer gates, not transport work.

## Frozen bounds and lifecycle

| Resource | Bound and rationale |
| --- | --- |
| Request body | 2,048 bytes; ASCII graphic token/object-handle strings 1–128 bytes; rejects oversized/unknown/duplicate inputs. |
| Page | 1–32 entries; narrower than core's 64-entry maximum to bound public response and work per IPC call. |
| Work per call | 1–1,000 deterministic logical units; no trusted unlimited budget crosses HTTP. This is not a CPU-instruction/time guarantee. |
| Result body | 65,536 bytes, including public identity envelope; overflow drops scratch and reports response_limit, never truncates. |
| HTTP concurrency | One action-query request per authenticated seat; existing 64 global request and 16 match broker queue bounds remain. 429 denotes transport admission only. |
| Worker quota | Fixed one-second windows per seat: at most 16 charged start/continue calls and 8,000 requested work units. Reserve the full allowance before work; no progress/timing refund leaks validator cost. |
| In-flight scratch | One continuation per seat, four per match. Starting while one is live reports in_flight; it never replaces another query. |
| Whole query | At most 32,000 reserved units and 128 calls; exhaustion destroys scratch and reports total_work. All pages already received are still incomplete overall. |
| TTL | 30 seconds absolute from start, never refreshed. All expired slots reaped on every query; at most four slots remain allocated even without traffic. |
| Cursor | 256 OS-random bits, lowercase hex, rotated on every successful continuation; old value consumed exactly once. No URL/log/storage exposure. |
| Cancellation | Own current cursor only, exempt from worker work quota so exhaustion cannot prevent release. Unknown/foreign/expired/replayed values all stale. |
| Abort | SDK AbortSignal controls one fetch/body read. Broker skips abandoned unsent queries. A query already in IPC completes within its logical allowance; an inaccessible continuation expires/invalidates. Aborting is not evidence of server cancellation. |
| HTTP deadlines | Existing five-second body read and 25-second request timeout; existing worker IPC timeout/isolation apply. No unbounded background query tasks. |

The private table is per worker/match and indexed by authenticated seat. Core's
opaque continuation retains its own viewer, decision token, executor and private
state binding. Neither continuation nor Work/receipt/epoch is serializable here.
A lookup never consumes another viewer's cursor. Quotas survive ordinary state
commits. Every committed executor replacement clears all continuations (including
host-only clock/rejection records in the existing worker architecture); recovery
starts with an empty table. Thus callers must tolerate stale even when the public
decision token is unchanged. No durable query recovery is promised.

Discovery writes no Game state, head, publications, ready flags or remaining
seat clock budget. Executor query processing time is excluded from the next clock
sample, using the existing worker busy-time convention. The broker still runs
ordinary independent clock ticks/progress. Polling/discovery is not a keepalive
that resets the action clock. No timing-side-channel or universal completion claim
follows from logical work bounds.

## Generated SDK and journal boundary

`GameClient.queryActions(request,{signal})` performs exactly one validated POST,
with generated request/result types and standalone validators. There is no SDK
pagination, retry, clock, journal or recovery policy. It correlates match and
start-page token/scope/page size; the session must also bind authenticated viewer,
current decision/generation and continuation history before using results.
The generator's closed envelope/route checks, validator inventory, browser type
exports and verification counter now explicitly include this operation. `uint16`
is validated exactly alongside existing native numeric formats. Raw JSON strict
scanning, decimal quantities, CSP without unsafe-eval and sanitized response-body
`TransportError` handling are retained.

The journal lane requested `prepareCommand(envelope)`,
`parsePreparedCommand(exactBody)` and `GameClient.submitExact(exactBody,{signal})`.
Preparation snapshots into a detached deeply frozen validated envelope/body pair.
Recovery validates strict JSON/schema/body size and preserves the original string,
including whitespace/member order. Exact submission sends that string once and
verifies the returned command ID. The session alone owns persistence, retries,
unknown outcomes and reconciliation. Initial pushed checkpoint is `d2fee399`;
64 baseline-qualified SDK checks passed. The journal lane cherry-picked it as
`305b90bd`; integration must avoid applying that patch twice.

## Reproducible setup and evidence

Run package-local locked installs through `sh scripts/engine-env.sh`; never root
npm installs or implicit Cargo. Root doctor found pinned native/Node tools but
missing root TypeScript/Ajv/workspace dependencies; no root bootstrap was run.
The SDK uses its independent lockfile. Native work uses the shared flock at
`/home/josh/.local/share/mtg-ecosystem/engine-verification/cargo.lock`, explicit
`CARGO_TARGET_DIR=/home/josh/.t3/worktrees/mtg-ecosystem/perf-engine-verification-loop/target`,
`ENGINE_CACHE_MAX_GIB=32`, `CARGO_INCREMENTAL=0`, and cache admission before compiling.
No cache/evidence/worktree cleanup. Test children use process-local umask 0022.

Baseline protocol/bootstrap copies are in this worktree's ignored
`generated/action-transport/baseline/`, SHA-256
`7fe738bb5a4aecaedb23a9a6405691c6497c6b117b38464aacde6601f152204c` /
`0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
Their original source `936566b7` is preserved, never relabeled as a new producer.
New schemas/builds and client compatibility are separately qualified below.

The new `action_fixture` example captures only committed permitted observations
and core query outcomes from frozen local foundations/attachments catalogs.
It uses a bounded opening keep/pass/land/cast prefix with every initiation obtained
from core, not card-name policy. Tokens/handles belong to each isolated capture
namespace and are not reusable live references or credentials. Captures establish
only observed families, not all profile reachability. Native retention's recursive
source closure now includes server examples; SDK retention already recursively
captures new SDK source/test/generator files, and server source recursion includes
the private table. No retained rebuild was performed merely to test these additions.

`tools/check-action-transport.mjs` consumes explicit retained host/provisioner
executables and emits a scrubbed real-host report/public queries. It never invokes
Cargo or fetches card data. The first command loss is explicitly injected after a
real successful HTTP response, then reconciled using real durable execution status
and exact duplicate bytes. It is not represented as an unmodified network failure.

## Consumer handoff and omissions

Journal/session owns query lifetime/generation/viewer binding and retry/recovery
policy; controls owns local drafts/rendering. They must integrate explicitly with
this new SDK/schema checkpoint and its qualified pin; their baseline results do
not prove compatibility with this changed schema. Missing source-label/context
mapping stays blocked. No other lane's session/decision/app paths are edited here.

Full runtime/release/performance/retention/natural-game matrices, full M2, complete
request reachability, human usability and deployment are omitted. No merge is
part of this task. Focused results and exact artifact identities follow after the
stable producer run.

## Executed qualification: 2026-11-13

Native checkpoint: `1e6fbefffb908cdf7dd741c4baa97f6467c4c85d`.
SDK implementation checkpoint: `76b5f3a1e4cfe8e32189fd197b3f1edf05670f80`.
The final documentation/probe/qualification commit does not change native or SDK
runtime build inputs. Every row of the native before/after fingerprint was also
checked against `git show 1e6fbeff:PATH`; all match. This does not include the later
independent compiler-only filtered-source fix or constitute a rebuilt merged tip.

Ignored evidence root, absolute:
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/`.
Native artifacts are under `attempt02/`; package SDK verification/build/browser
receipts remain under `packages/game-server/sdk/{generated,dist}/`. No host/probe
process remains; the native slot was released and the engine lane notified.

| Executed focused check | Result |
| --- | --- |
| `cargo test --locked -p mtg-server --test action_queries` | 4 passed: purity/publication/clock/head equality, cursor rotation/replay/viewer, cancellation/expiry, quotas/recovery, complete/core context/stale/invalidation and strict input/schema bounds. |
| `cargo test --locked -p mtg-server --lib action_queries` | 2 passed: total-work/call exhaustion destroys scratch without rules mutation; work-rate quota preserves own cursor; absolute TTL never slides. |
| `cargo test --locked -p mtg-server --lib ipc::tests` | 3 passed. |
| `cargo test --locked -p mtg-server --lib protocol::tests` | 5 passed. |
| `cargo test --locked -p mtg-server --test protocol_adversarial` | 10 passed. |
| `cargo test --locked -p mtg-core --lib action_query::tests` | 11 passed, including hidden-state paired transcripts, restore/privacy, validator parity, scope/budget and both admitted prefix witnesses. |
| `cargo test --locked -p mtg-core --lib player_session::tests` | 4 passed. |
| Copied `mtg_sim-08ef4bc0a9d316d1 native_source_collection --nocapture` | 1 passed, including new example source closure. |
| `cargo clippy --locked -p mtg-server --lib --bin mtg-server --examples --no-deps -- -D warnings` | Passed. |
| `cargo build --locked -p mtg-server --bin mtg-server --example action_fixture --example provision_client` | Passed; exact executables copied before lock release; copied fixture producer executed. |
| SDK `npm run verify --workspaces=false` | Passed generation, browser package, strict types and 66 Node tests; 20,785 standalone/reference validator comparisons, 28 request/33 response wire variants plus all 3 query operations/5 outcome variants. |
| SDK `npm run test:browser --workspaces=false` | 13 checks each on Chromium 158.0.8127.18 and Firefox 159.0 under restrictive CSP; actual eval blocking, exact command bytes, query validation, abort/body bounds and existing TransportError behavior. Synthetic HTTP, not browser gameplay. |
| Client `npm run check --workspaces=false` | Passed fixture preparation, strict types, boundaries, 38 tests and production build using the **same** final SDK build. Existing chunk-size/React Router directive warnings retained. Only client change is the explicit sdk-qualification pin. |
| `node --test scripts/retain-server-evidence.test.ts` | 18 source/layout guards passed; no offline rebuild claim. |
| `python3 scripts/engine-structure-check.py`, targeted `rustfmt --check`, `git diff --check` | Passed. |
| Current generated validators against public prefixes/presentation artifacts | 8 permitted observations, 6 query outcomes and both offline presentation artifacts passed. Synthetic stream envelopes used only for observation-schema validation. |
| Real host `tools/check-action-transport.mjs` | `host01`: 9 case groups + 4 real commands; `host02`: 11 groups + 4 real commands, adding exhausted continuation-chain equivalence and concurrent per-seat HTTP 429 admission. Both passed. |

All Cargo commands above ran through `sh scripts/engine-env.sh` inside the shared
flock with the explicit cache environment and preflight. Logs and the exact
command sequence are retained in `run-native.sh`/`attempt02/*.log`; test executables
are retained alongside them. Cache admission recorded 31,950,114,816 allocated
bytes against 34,359,738,368 allowed, with no errors. Production host/examples are
debug opt-level 0; focused test artifacts use the repository test profile,
opt-level 2. Their distinct executable hashes are retained; test binaries are not
substituted for the hosting producer.

SDK/client setup and current explicit schema verification, from repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts --no-audit --no-fund
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts --no-audit --no-fund
MTG_SERVER_SCHEMA="$PWD/generated/action-transport/attempt02/protocol.schema.json" \
MTG_SERVER_BOOTSTRAP_SCHEMA="$PWD/generated/action-transport/attempt02/bootstrap.schema.json" \
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run test:browser --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
```

Real-host probe setup (choose a new output directory; it provisions its own new
private registry, exports four separate seat files, launches/stops the supplied
host, and preserves all evidence; never use another lane's registry):

```sh
sh scripts/engine-env.sh node packages/game-server/tools/check-action-transport.mjs \
  "$PWD/generated/action-transport/attempt02/mtg-server" \
  "$PWD/generated/action-transport/attempt02/provision_client" \
  "$PWD/generated/action-transport/attempt02/native-info.json" \
  "$PWD/generated/action-transport/attempt02/source-before.json" \
  "$PWD/generated/action-transport/NEW_PROBE_DIRECTORY"
```

The final probe's source SHA-256 is
`547730f98b9feaf37f309f1aef773972fef6715cc30b295986e3317952eac61b`.
`host01` remains its earlier 9-case receipt and is not rewritten to claim the
later two checks. `host02` pins its own exact probe and SDK build hashes. Host
restart recovery, decision/cursor/handle foreignness, body/schema/auth bounds,
quota status, cancellation and continuation completion were real authenticated
HTTP. SDK retries remain absent; bounded polling in the explicit test harness is
labeled harness behavior. In-flight abort after IPC dispatch/queue-drop timing
was not independently fault-injected; bounded scratch lifetime/ownership follows
from the table/broker paths plus tested expiry/recovery and SDK abort boundaries.

### Exact retained identities

| Artifact / identity | SHA-256 or value |
| --- | --- |
| Engine build, hosting producer | `0e8abd9759e99c6be84492e859396a90fcdc4254b88d125e294114f2ed50730d` |
| Server build, hosting producer | `99d06fefdcff1569acd690b5c4f1f7d78ecc34452399f3530b39cbc45acf3777` |
| Native source fingerprint, before = after | `ba6379af2980ffc855635c9662c3989af1a11b93ff8e635fda1d778362fd7692` |
| `attempt02/mtg-server` | `fb53ef1b029dc6f3c7abeb21791347cff07ef3f1958b484dcb138e2018c6d521` |
| `attempt02/provision_client` | `b86c480eb8129c55e8700f34311eb5d2de6cc1840450ce39c664e40356c73b50` |
| `attempt02/action_fixture` | `b0dd647cc6396df261b9b24dbf31ea00a26e1a828b05c2510da448810fd434b2` |
| Current protocol schema | `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3` |
| Bootstrap schema (unchanged) | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| SDK verification JSON | `22067a2b5e6db673c41df5420d1094bf3a7324adb85699efa786cf084cc319f1` |
| SDK browser build manifest | `793333249d3f65e391505bb346415ef5ec010141e5c0e7574a9d4e9ee27f8123` |
| SDK browser CSP receipt | `8289274a5ca4009d9d8b6374e671630cd0812652b4451c373ffb6bcc456009d1` |
| Public prefix captures | `5dc2b69175a9b4a5f42fda822cd52f57a6acf4a7311289a9a90b47c554aca9f4` |
| Frozen `producer-manifest-final.json` | `8a802cb0ea26f53cef88415fb50b3c81a390d8206e0f107c77ecfcab46510aa9` |
| `host01/report.json` | `184d1153d6830b898100d35bb04a3227062f757153a7ae33bf2853051c7dcd9f` |
| `host02/report.json` | `e309f1a6e3d72cc3a965ae444c722a1f57686e79ea01657685dad89652e958bd` |

The frozen final producer manifest supplies every binary/export/capture path,
source fingerprint, catalog identities, capture namespaces/stages and offline
presentation inputs. It includes `foundations-presentation/` and
`attachments-presentation/` artifacts produced by the existing frozen-source
`tools/client-artifacts.ts` using the exact corresponding local fixture plus the
current protocol export. Their `artifacts.json` hashes are
`4d997a84b6e27b54cceb35f301258c8326e6580c0d15bcf9e6065fe64e1f02a0` and
`23cd19bb7708f8910ce17fdb75b83c95fe9676cef64fa6ebc9fa1bbea7d96b74`.
No live Scryfall/image request ran. Presentation ability mapping stays unavailable.

### Preserved failures, manifest correction and handoff

The initial SDK check correctly failed its closed runtime-export inventory after
adding prepared-command exports; the inventory was updated and 64 baseline tests
then passed. A second attempt omitted explicit schema environment variables and
failed to find a default producer; no schema fallback or Cargo invocation was
added. The first query generation failed on the previously unhandled native
`uint16` format; exact validation and independent parity support were added,
without relaxing strict formats. The first source-closure Cargo filter matched
zero tests; the copied test binary subsequently ran the exact relevant test (one
passed). All original logs remain under `generated/action-transport/`.

After publishing `producer-manifest.json`, this lane mistakenly appended commit,
SDK-parity and presentation annotations to that file. Artifact/source/schema/
capture bytes never changed, but downstream digest checks correctly detected the
manifest change. The issue was disclosed and separate frozen versions preserved:
`producer-manifest-initial.json` (09891385…), `producer-manifest-annotated.json`
(7e32bcb5…), `producer-manifest-checkpoint.json` (0092c3fe…) and
`producer-manifest-final.json` (8a802cb0…). Use the frozen final filename for new
handoffs; historical downstream receipts retain their original manifest hash and
source qualification. Do not mutate these published files. The later SDK README
addition is documentation only; the retained SDK verification source inventory
and runtime build remain qualified to the measured implementation checkpoint.

Controls consumed the 8 observations/6 query captures as separately qualified
prefix evidence, retaining their baseline SDK/runtime pin and unavailable labels.
Journal consumed the immutable real-host producer evidence separately from its
baseline-SDK IndexedDB/browser session tests. Command/execution/submission/resume/
stream/manifest schema components are unchanged; the new query exports are
additive, with a deliberately new complete schema digest. This branch explicitly
qualifies the existing client against that new SDK, but does not compose either
lane's new implementation. The final integration owner must combine branches,
apply the exact-command patch once, adopt the qualified SDK/pin, fence query
lifetimes in session, and run focused combined browser/session/controls checks.
Source-label mapping, unobserved family reachability and human play remain gates.
