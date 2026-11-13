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

## Final qualification and acceptance

The bounded M1 joining/read-only deliverable is implemented and exercised against
an actual native host with four distinct authenticated seats. This is also the
read-side session foundation for M2. It is **not** complete M0, command journaling,
playable decisions, M2–M4 acceptance or a natural game. The admitted foundations
profile was exercised; attachments runtime acceptance remains a later gate.

Client runtime checkpoint: `20bb013c1ef7f6c3c2f7fbee0a9c47f499086d28`.
Subsequent client-owned changes tighten Vite's static route guard, correct test
assumptions, and update qualification/documentation; all three production asset
hashes below remain identical. Serving/SDK prerequisite `bb41898b5eeca0c3357c63f03ecdf7180c51e76c`
is the exact parent, unchanged. The final native-only loader hardening source
`6bf0609eed12273f6dae44d7cc97766299ebbcc4` is independently qualified, not authored
or rebuilt by this lane. Its freshly exported protocol/bootstrap files compare
byte-for-byte equal with the prerequisite exports. The SDK source/runtime and
producer remain unchanged. No historical SDK schema was substituted at any point.

Final native binary: debug, Rust 1.99.0, x86_64-unknown-linux-gnu, opt-level 0.
Engine build: `8804fe0e03df54a16a04abf87ea7b1883da559dd880909cbf000ee4c9385d568`.
Server build: `4248a94314b70f9dc823e227afdb5702c9056d16982c5b108b36413dcd887fcf`.

| Identity/artifact | SHA-256 |
| --- | --- |
| Final native executable | `11a68dab211e45ab237e383580007da2cd5895cc904942c6e1e68df309edd966` |
| Final native-info.json | `b864efbef6c1d82b5323bc01a30cf880b6a573da2a126eb69952dda792f6d229` |
| Protocol schema | `59deeafe34996f38e7a8905df9c75e9c4c424d54e9ae33bed2a4b23634f99bc6` |
| Bootstrap schema | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| Client config schema | `82c71b74373615553d598e126735ddb154d208ed7fdb807959bf377ad938287b` |
| Canonical catalog | `fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f` |
| Producer source | `9a1304423d9d6133d7ba4fab51fc86d1c81bb9c877d4c57745908ef617675b35` |
| Presentation schema | `a7045da3459cd5b6a126ca804ea22716062c63ce3127017bb5f3637c840231a6` |
| Public presentation bytes | `f3d1c0c33a76565ab107c0201320072c705845cadc4508c7364f7c0b3d3b16b7` |
| Presentation artifact manifest | `84d1b578929ddad8baab9754072c8914b21702cf5a04f32028dbe99c13192891` |
| SDK browser.js | `597cd1c25d4291234e26c2079216906f41f1e0fc8da17f9fb7ae48813cbcdb65` |
| dist/index.html | `a33b2fb7945d46d066eb6530f85290ff2bee4f3d9bca1dfad9b782b68047db78` |
| dist/assets/index-Ba7Op3pa.css | `8032b902c38ef615450a0ca0dfbf4046698b89f8bf04cb25c9f9a404704abbab` |
| dist/assets/index-Ds1idBNn.js | `fa9753165b5500959f3346924c3c6a9b879dca921d76a6e3a48f304d0a32c0b9` |
| Exported public client manifest | `a9e9c6f16377dd9133f2c5dc41bd5b3b4f67573fc2f266c15be24c4dde0fbc81` |
| Client package lock | `1f837860ed1ab73700fbf18193eb1deaa1e2fadee31f520a1d808ae9fa38a45e` |
| SDK package lock | `e5034526be40dc01509df43489d7a4e7dd6bf259e7258a1ad76868cd925480d9` |

`sdk-qualification.json` pins current validation independently of unchanged
historical `fixtures/foundations-opening/provenance.json` and fixture bytes.
Initial SDK outputs were preserved in `generated/initial-sdk-evidence/` before
explicit generation from the serving export. The final native-only hardening
required no new SDK generation because both exports and SDK runtime sources are
identical; `generated/final-native-inputs/` preserves the fresh comparison inputs.

All client/SDK checks used pinned Node 26.11.1 and local package installs. Passed:

- SDK explicit verify: strict TypeScript, browser build and all **59 Node tests**,
  one current SDK artifact for all subsequent client checks. Evidence:
  `generated/serving-sdk-verification.log` plus SDK `generated/verification.*`.
- Client `check`: strict types, import/effect boundaries, **34 pure/controller
  tests**, exact fixture/schema/build qualification, runtime module allowlist and
  production build. Evidence: `generated/final-client-check.log`,
  `final-native-qualification.log`, `fixture-verification.json`, `bundle-inputs.json`.
- **10 production fixture browser cases**, five each Chromium **158.0.8127.18**
  and Firefox **159.0**, Playwright **1.65.0**. Keyboard inspection/restoration,
  decision jump, reduced motion, 640px layout, missing metadata, route refresh,
  no API requests on fixture paths and empty browser persistence passed. Evidence:
  `generated/final-fixture-browser.log`, `browser-report.json`, `browser-results/`.
- **18 final native production-browser cases**, nine each Chromium/Firefox,
  against final binary/source above on `http://127.0.0.1:18741`. Each browser read
  all four separate authenticated seats in isolated contexts: correct bound
  match/viewer/protocol/build/catalog, own hand only, seven own hand objects,
  public board, no-store API responses, no CSP violations, GET-only traffic,
  empty local/session/IndexedDB/cache/service-worker state and no cookies.
  Same-profile duplicate tabs were blocked before authenticated reads; explicit
  release, owner close and refresh released ownership; refresh required reimport.
  Unavailable locks failed clearly. Evidence: `generated/final-native-browser.log`,
  `final-native-browser-report.json`. Zero skips, retries or failures in final run.
- The same suite separately labels **injected** faults: missing metadata fallback,
  corrupt digest failure, wrong configured origin/malformed file, delayed old-seat
  completion, 401, 429 and 503 with bounded retries. These use real native inputs
  with explicit browser interception; they are not native fault-injection claims.
- **4 final restricted-proxy browser cases** (four seats and duplicate-tab/reimport
  in both browsers), plus **11 actual HTTP route/security checks**. Bad Host,
  foreign Origin/cross-site metadata, commands, forged resume query and unknown
  private-looking paths reject; CORS is absent. Evidence:
  `generated/final-proxy-browser.log`, `final-proxy-browser-report.json`,
  `final-proxy-security.log`, `proxy-security.json`.
- Serving peer independently exported source `20bb013c` into a fresh owned
  scratch checkout, installed both isolated locked packages, explicitly built
  SDK/client, and reproduced all three dist hashes exactly. Peer evidence:
  `generated/local-serving/clean-build.log` in the serving worktree. No Cargo,
  root installation or shared dependency replacement. This is peer-produced proof.

Final browser receipt hashes:

| Redacted receipt under generated/ | SHA-256 |
| --- | --- |
| final-native-browser-report.json | `5b165bca5890a64c45b6139cfdbb6934865102d33754f23f7a7a8a4dc10dd45b` |
| final-proxy-browser-report.json | `b5fe44ebdd7d6315eb3df92bc1a5a51879a8a1059044dba2d6798c1d2bd3c437` |
| proxy-security.json | `1615175ff62a1f383bb408b4530f8e0a82316d40645f97647945c069fdc0b861` |

All test browser contexts/tabs and the lane-owned Vite preview were closed after
acceptance. The native owner confirmed graceful shutdown of the exact test host;
private registries/files and all historical/failed evidence are retained.

The native owner supplied public bundles, exact binaries and private single-seat
paths. This lane never read the operator issued-credential file or started a native
process. Native/schema/security tests and build-cache failures remain in that
owner's source-qualified report, not attributed to client checks here.

## Real lifecycle-only update and bounded recovery

The host owner stopped only the task-owned host, invoked existing offline
`Worker::clock(Pause)`, and restarted the same executable/bundle. No gameplay
command or synthetic ledger write was used. The first manually coordinated outage
exceeded the 7.5-second four-retry window. The browser retained cursor 1, the exact
rendered seven-card hand and keyboard focus, then stopped/released ownership.
Explicit reimport read cursor 2, `resource_paused`, the identical hand and no rules
outcome. This is retained as `generated/lifecycle-browser-reimport.json`; it is not
claimed as automatic reconnection.

The final source repeated the transition in one prepared invocation: **3,761.08 ms**
(shutdown 70.45 ms, native offline pause 1,390.65 ms, restart-to-readiness 2,299.98 ms).
The held production browser automatically recovered within the unchanged policy,
advanced cursor 1 → 2, displayed `resource_paused`, and retained the exact hand
hash, seven cards and decision-region focus. No reimport, invented outcome or
command occurred. Four subsequent native long polls completed HTTP200 approximately
20.1 seconds apart, confirming the live bounded loop continued after recovery. The additional T3 browser is Chrome **152.0.7977.169** /
Electron **44.10.1**, separately qualified from the pinned Playwright browsers.
Final redacted receipt: `generated/final-lifecycle-browser.json`
(SHA-256 `f158bf90c6a230907c0c69ec12a89992b5f50db32357ea79864e8ba1772ce901`); native timing
receipt is `generated/local-serving/final/transition-timing.json` in the serving
worktree. Bounded pending/outcome, atomic stream-page rejection, foreign-view
rejection and late success/failure fences additionally have injected pure tests.

## Preserved failures and practical limits

- `generated/meaningful-target-type-failure.log`: generated VisibleTarget exposes
  announced/current references, not an assumed single handle. Corrected display
  adapter; no handwritten DTO shim.
- `generated/pending-sdk-contract-typecheck.log`: expected missing new SDK exports
  before the peer's committed prerequisite. Resolved by exact-source integration.
- `generated/serving-client-check.log`: test closure lost union narrowing; fixed
  test binding, retained strict types.
- `generated/native-live-first/` and `native-live-first.log`: first four-seat
  keyboard assertion assumed every seat had a mulligan request. Native supplied
  no decision for nonactive seats and UI correctly showed that. Assertion now
  targets the actual decision region; final all-seat suite passes.
- `generated/proxy-preview.log`: npm exec launched Vite with the root cwd, so the
  initial manual preview returned404. Corrected to package cwd; no root edits.
- `generated/proxy-fetch-probe-failure.txt`: Node fetch did not send the attempted
  adversarial Host; replaced the probe with node:http/curl, which verify403.
- `generated/proxy-static-fallback-failure.log`: Vite returned its app shell for an
  unknown private-looking path. No private file was served, but preview now uses
  explicit route/asset allowlists and development excludes generated/private paths.
- Existing React Router module-directive and production bundle-size warnings are
  retained. Build size is not a latency/performance measurement.

No actual human usability or screen-reader study, natural match, dense-board or
performance benchmark, attachments live acceptance, Safari, cross-device takeover,
command durability/journal, semantic action queries, full engine matrix or
M2–M4 completion is claimed. This lane started no Cargo. The existing engine
schema-1 reset/action-query branch is not silently incorporated; integrating it
requires new source-qualified exports and deliberate client compatibility review.

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

## Parent integration review: schema-1 requalification

Runtime source `865df11bd13373b38192878764fcfc3303ae8f0b` integrates serving
`147dec571e58a531b1229f61a9f78733c021eadf` and current schema-1/query main.
The separate SDK qualification now pins native protocol
`2796d38cc8dc4ef67d836fbe890ba31170869f4562c91b8edf7c6f1b91396e64`; bootstrap
is unchanged. Historical fixture capture and provenance bytes were preserved.
This integration includes the final loader fix and retained-input corrections,
not only the earlier copied prerequisite. README conflict resolution keeps the
new live setup instructions with current exports.

Against the newly built native binary and fresh catalog/presentation/registry/seat
files, one current SDK build passed 59 tests and strict types, then client fixture
validation, types, boundaries, 34 tests and build. All 18 native browser cases,
four restricted-proxy cases, 11 proxy security checks and 10 fixture browser cases
passed. Initial proxy invocation ran Vite from repository cwd and failed its four
browser cases; failure log/report is retained, corrected package-cwd launch passed
all four afresh. No guards, retry budget or product behavior were changed to pass.

The new production client automatically recovered after a 4.122-second native
stop/durable-pause/restart. Cursor 1 became 2; the same seven-card hand and decision
focus remained; resource_paused was distinct from a rules outcome; four subsequent
long polls completed. No reimport, game commands, traces or capability logging.
The standalone harness closed its browser; the native owner stopped the exact
task host. Root `generated/review-m1/lifecycle.{mjs,json,log}` preserves this proof.
Other new logs are in root `generated/review-m1/`; browser reports are client
`generated/review-native-browser-report.json`, `review-proxy-fixed-browser-report.json`
and `browser-report.json`. The native lane's `generated/review-m1/qualification.json`
records binary/build/schema/asset hashes and copied client receipts.

Earlier source-qualified receipts above remain history. This closes the schema-1
integration gap, not command journal/send/reconciliation or gameplay acceptance.

### Independent-review response-body recovery correction

SDK source `9046c4f10795cb83f4160953f16f71d73f26adb7` distinguishes a
rejected response-body read from a completed invalid response. It reports the
existing sanitized `TransportError`, which this client's existing adapter maps
to disconnected recovery. No retry policy moved into the SDK and no rules work
moved into the client. Native source/build/schema pins above are unchanged;
`sdk-qualification.json` now records the corrected SDK source separately.

Four new regression cases run the actual browser session adapter and production
SDK against a local Node HTTP server. After a valid initial view, it sends headers
and partial JSON then drops the socket. The client retains the exact permitted
view, resumes successfully, or stops after four retries if every poll drops.
Recovery resumes do not replenish that budget. Completed malformed JSON and
schema-invalid responses instead clear the view and never retry. Lock release
and GET-only requests are checked. Browser window/Web Lock availability is
injected; these cases are not native-host or cross-tab browser evidence.

With one unchanged current schema setup: all 62 SDK tests and strict types,
38 client tests/types/boundaries, production build, and 10 production fixture
browser cases pass. The SDK source also passed 12 CSP checks in each browser in
the serving worktree. Root `generated/review-m1/interrupted-body-qualification.json`
hashes the updated logs and artifact manifests. The first regression run exposed
test-harness timing of the asynchronous lock callback; it is retained as
`interrupted-body-client.log`. The corrected test awaits callback completion;
`interrupted-body-client-fixed.log` records all 38 passing. Product locking was
unchanged.

Prior native live/proxy/lifecycle acceptance keeps its original SDK/client source
qualification. This correction required no Rust change or new native build;
the new checks qualify the transport failure through the actual client adapter.
