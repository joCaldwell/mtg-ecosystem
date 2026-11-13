# Live opening-hand actions

This slice connects the production `/join` → `/table` App to
`browserCommandSession`, the existing durable journal and real DecisionPanel
adapters. Only `mulligan` and `bottom_opening` are enabled. Other requests explain
that their live control/context is unavailable, including the seven other local
fixture adapters. This is automated finite opening-prefix evidence, not a full
game, full M2, human usability acceptance or new hosted-profile admission.

Base/native/SDK source: `2aaf466b6a331846baea95116c6c0ebf26d953cb`.
All authored paths are within `packages/game-client/`. No compiler, core, worker,
SDK wire/serialization, card data, dependency/lockfile or styling changes. No
subdelegation, root install, deployment or merge. Deck-builder visual cohesion
remains deferred. Parent owns independent review and ordered integration.
Later compiler/main changes do not inherit this producer's acceptance.

## Behavior and boundaries

App captures the **full original CommandContext** when a draft starts, including
origin/match/seat, engine/server/catalog/schema, revision/token and incarnation.
Completion passes that captured context through the existing session's checks;
it never stamps an old selection with a fresh context. Permitted inputs are
immutable. Handles distinguish card occurrences; bottom count/order come only
from the request. Native keyboard controls confirm explicitly; remote updates
do not focus controls. Only session effects perform network/storage operations.

The journal persists exact SDK-prepared bytes before send. Duplicate clicks are
disabled. Saving, pending, uncertainty, rejection, unavailable storage and explicit
saved-choice retry have player-facing status. Accepted acknowledgments, newer
observations, missing execution rows, stop and refresh do not resolve uncertainty.
Retries preserve body/ID/revision/token; actual terminal disposition precedes a
coherent resume and new explicit confirmation. No auto-action, concession,
forced takeover, cancellation UI or universal Undo was added.

One demonstrated session change gives each long poll its own cancellation signal.
Terminal resolution wakes an idle poll and requests coherent resume immediately,
without treating read abort as command cancellation or consumption. Session-stop
and generation fences remain authoritative; failed resume still uses the existing
bounded reconnect policy. The new unit regression covers this exact idle-poll
case, failed refresh and old-incarnation rejection remain covered.

Build preparation verifies current SDK generation, schema, source and artifact
hashes, then emits the runtime schema constant from **SDK dist/build.json**.
No user/server draft selects that identity. Qualification and the local-control
inventory were deliberately regenerated; historical fixture/prefix provenance
bytes remain unchanged. The inventory's engine-map evidence row now matches the
base's updated document; its guard was retained.

Vite still checks exact local Host/Origin/Fetch Metadata before upstream rewrite.
The only added proxy paths are exact command POST and 32-hex-ID execution GET.
Queries, queued cancellation, aliases, suffixes and arbitrary/private paths stay
closed. Native `serve-client` authenticates seats, validates/bounds command JSON,
uses bounded body/request reads and applies its origin/privacy policies. Native
cancel remains an authenticated server capability, unused by this UI/proxy.
Fixture/gallery routes make no API/storage effects before explicit join; the
native SPA gallery allowlist was not expanded.

## Exact producer and artifacts

Evidence root in the active checkout: `generated/live-opening/`. Native binaries,
exports, source manifest and copied test executables are in `native/`. Before/after
fingerprints match, and all **676** rows independently match `git show BASE:PATH`.
A fresh provisioner creates random-seed single-seat files; no outcome seed selection.
Presentation is produced from the exact local `expected-match-ir.json` source
provenance, with no network/card inference or fixture edits.

| Identity / artifact | SHA-256 |
| --- | --- |
| Native source manifest | `818ddd3c681c352e70bb3bd66e4109d2e05e8644f4db9a12181842d9b2e7f7df` |
| Engine build | `5a39a51f75897cee17cced3085b449742dd5d07fc4b710816f9e302ec02f6735` |
| Server build | `23c756da0c8a60adc434187b63da955f105656de94c58b86752f0df3c7e5e9ec` |
| Copied mtg-server | `8ade791ecbb5b4204a66b9edb845931b5850ed21e881e7b3184133b218492d7f` |
| Copied provision_client | `6b73fb5323b1d0615e4425ba351095717b207b641886cd711f6c86bd716874f3` |
| Protocol schema | `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3` |
| Bootstrap schema | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| Catalog | `d00ee2cf3760ddaf9a807af0c79764f4bea2f21177c350a30d2d4f0a087c9ef0` |
| Fresh presentation artifacts.json | `4d997a84b6e27b54cceb35f301258c8326e6580c0d15bcf9e6065fe64e1f02a0` |
| SDK dist/build.json | `793333249d3f65e391505bb346415ef5ec010141e5c0e7574a9d4e9ee27f8123` |
| Final client-assets.json | `434feec631bef373031a70f121513b3ba6dcf27ed11d47d0314a2a1bcbbb78bc` |

`qualification.json` additionally pins application sources, schemas, binaries,
assets, harnesses, commands, logs and final receipts. Raw captures/private seat
files remain in ignored, private run directories; authenticated traces/HAR/video
are off. Original source-module harness/report copies in `reference/` retain
original historical paths and hashes, without relabeling them as App evidence.

The native build ran under the shared Cargo flock with cache admission,
`ENGINE_CACHE_MAX_GIB=32`, `CARGO_INCREMENTAL=0`, process-local test umask 0022 and
the then-authorized `perf-engine-verification-loop/target`. Binaries/exports were
copied before lock release. Josh subsequently authorized retiring that historical
cache/worktree; cleanup coordinator was directly notified of release and no
remaining dependency. **Do not rebuild against that retiring path.** Any future
native build must use a surviving target agreed with the coordinator. This lane
needs only its retained native inputs for reproduction; no additional Cargo ran.

## Executed focused checks

All commands use root `sh scripts/engine-env.sh`; npm installs were local locked
`ci --workspaces=false --ignore-scripts`, SDK then client. No root bootstrap.

| Executed check | Result / receipt under evidence root |
| --- | --- |
| Native delivery tests, HTTP delivery test, protocol adversarial tests and debug server/provisioner build | 4 + 1 + 10 tests passed; `native/` logs and copied executables. No full engine matrix. |
| SDK explicit-export verify | Strict types, generation/browser packaging and 66 tests passed; `sdk-verify.log`. |
| SDK browser CSP proof | 13 checks each Chromium/Firefox, 26 total; `sdk-browser.log`. |
| Client check | Types, boundaries (31 files), all 69 tests and production build passed; `client-check-final.log`. |
| Production fixture/gallery browser suite, explicit port 4194 | All 22 passed; `fixtures-browser-final.log` and package generated browser report. Empty storage/API isolation retained. |
| Actual IDB journal browser suite | All 16 passed; `journal-browser.log`. Synthetic HTTP faults are separate from native evidence. |
| Current native live read/recovery suite, port 19010 | All 18 passed; `read-suite01.log`. Existing fault injection remains labeled. Uses preceding bundle03 with identical session runtime; final opening/proxy receipts use final bundle. |
| Final production App/native opening acceptance | 10 case groups, five each browser; `accept-final/report.json`. Four isolated contexts per opening prefix, real keep/three mulligans, supplied bottom counts 1/2 with exact handles/order, keyboard, duplicate click/tab, post-opening priority blocked. |
| Final native command recovery | Same final receipt: actual IDB before POST, response loss after genuine native HTTP 200/commit, accepted retained, stop/refresh/reimport, actual consumed status, native missing status/exact retry, original delayed intent rejected after explicit SDK collision, refreshed/new-confirmation gate, remote focus, denied/corrupt IDB sends zero. |
| Final restricted production preview proxy | 17 path/origin/privacy checks, four authenticated body/schema bounds, five native execution/cancel restriction checks and real keep/execution reads on both browsers; `proxy-final04/report.json`, ports 18985/18990. |
| Source parity, inventory guard, git whitespace | Passed; `native/git-source-parity.json`, `inventory-final-check.log`, `git diff --check`. |

Browsers: Chromium **158.0.8127.18**, Firefox **159.0**, Playwright 1.65.0.
The prefix harness is explicitly automated. Other SDK seats are used only for the
named stale-intent collision; no hidden play policy drives the browser seat.
Missing/aborted status is never replaced by synthetic execution or success.
All owned browsers/hosts/previews are closed. The user's existing 4179 listener
was never stopped or reused.

Reproduce from root using the retained inputs (choose NEW output directories):

```sh
sh scripts/engine-env.sh env \
  MTG_SERVER_SCHEMA="$PWD/generated/live-opening/native/protocol.schema.json" \
  MTG_SERVER_BOOTSTRAP_SCHEMA="$PWD/generated/live-opening/native/bootstrap.schema.json" \
  npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run test:browser --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh env MTG_CLIENT_FIXTURE_PORT=4194 npm --prefix packages/game-client run test:browser --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run test:journal:browser --workspaces=false
sh scripts/engine-env.sh node packages/game-client/scripts/accept-opening.mjs \
  "$PWD/generated/live-opening/native/mtg-server" \
  "$PWD/generated/live-opening/native/provision_client" \
  "$PWD/generated/live-opening/native/native-info.json" \
  "$PWD/generated/live-opening/native/source-after.json" \
  "$PWD/generated/live-opening/bundle-final" \
  "$PWD/generated/live-opening/presentation" \
  "$PWD/generated/live-opening/NEW_ACCEPT" 18960
sh scripts/engine-env.sh node packages/game-client/scripts/accept-opening-proxy.mjs \
  "$PWD/generated/live-opening/native/mtg-server" \
  "$PWD/generated/live-opening/native/provision_client" \
  "$PWD/generated/live-opening/bundle-final" \
  "$PWD/generated/live-opening/presentation" \
  "$PWD/generated/live-opening/NEW_PROXY" 18985 18990
```

## Preserved failures and remaining gates

All numbered failed attempts remain in `generated/live-opening/`, including their
private registries and logs. The first native `http::tests` filter ran zero tests;
correct `http::delivery_tests` then ran one from the retained binary. Root doctor
reported missing root workspace dependencies; no forbidden root install followed.
The first client boundary check rejected a feature's direct SDK type import; it
now uses the decisions layer's generated-type alias without relaxing the guard.
Initial fixture checks exposed hardcoded 4179 and old About copy assertions;
tests now use the configured origin. The failed browser results were copied before
rerun. Early prefix waits assumed a null decision was ready, omitted the native
intermediate bottom requests, or required an explicitly null optional setup field;
the corrected harness follows supplied requests and normal optional absence.
A stale-rejection attempt exposed the idle-poll refresh delay fixed above.
The first version of that fix also bypassed failed-resume backoff; the existing
regression failed, and interruption is now limited to the in-flight polling case.
Retry fault attempts initially waited for an idle poll or transient pending copy;
final proof uses explicit human retry and authoritative native results/IDB state.
The added cancellation-body probe initially expected 413; native `http::cancel` deliberately maps all nonempty-body failures to 400. The source-backed expectation now checks 400 and retains both failed attempts. No schema, native outcome, privacy guard or command-uncertainty rule was weakened.

Remaining gates: independent parent review; qualification of any later combined
native source; action-query/menu integration, richer source context and the other
decision families; actual human play/usability. Full native runtime/release/search/
performance/retention/natural-game matrices, Safari, all-profile reachability and
deployment were not run or claimed. Existing bundle-size and React Router module
directive warnings remain, without a performance claim.

Parent subsequently published compiler-only `dc12222cd6251c3c20c3f15b4a0ae2e6ea19333a`.
This branch deliberately retains the tested 2aaf producer pin. Parent will preserve
that compiler fix and decide minimal final combined-source qualification; no duplicate
native build or automatic matrix was started here.
