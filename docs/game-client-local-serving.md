# Local production delivery and presentation metadata

Current integration qualification is recorded in the final section below. Earlier
sections retain their original pre-reset source and evidence boundaries.

This lane implements the server/SDK portion of live read-only M1 from
`0b674d8872087bc96f6078d0daeb1fb2d6c17758`. Core/compiler and client source are not
edited here. The concurrent engine schema-1 reset and action-query API are separate
source boundaries; this binary/catalog remains pinned to the baseline. Combining
those changes requires fresh native exports and client qualification. No M2 query
transport, decision submission UI, human game, deployment or merge is included.

## Contract agreed with the session lane

`GET /client-config.json` returns Rust-owned `ClientConfig`:
`client_config_version: 1`, `server_origin`. It is public, bounded, and no-store.
Native mode returns the configured canonical numeric loopback origin. Vite's
client-owned adapter returns its explicit `MTG_CLIENT_UPSTREAM`; imported bootstrap
files cannot select a proxy target. The SDK parses its generated Rust schema and
validates the numeric origin. The session compares that mapping with the bootstrap
before authenticated reads, and then checks protocol/match/viewer/build/catalog.

Authenticated `GET /v1/matches/{match_id}/presentation` returns Rust-generated
`PresentationDescriptor`: version 1, catalog/presentation/schema/producer SHA-256
identities and `/presentation/<presentation_sha256>.json`. Missing metadata is
404. The descriptor is no-store. Its public digest URL serves only the exact
immutable producer artifact with `application/json` and
`public, max-age=31536000, immutable`. No authorization is sent on that public
fetch. Existing manifest, resume, stream, submission and execution contracts retain
their semantics; no HTTP handler computes rules or visibility.

`GameClient.presentation({signal?})` returns `{descriptor, artifact}`. It validates
the Rust descriptor, exact producer/schema qualification, bounded raw JSON and
SHA-256 of the received bytes before accepting the producer-schema-generated
`PresentationArtifact`. It also rejects duplicate card identities, nonsequential
face indices, mismatched catalogs and unsafe artwork URLs. A 404 remains an
`HttpError` so the session can choose a text/current-identity fallback. Invalid
hash/schema/provenance is a `ProtocolError`, never fallback success. The SDK does
not retry, own a journal, choose a viewer or interpret Oracle text/IR.

The existing offline producer now reads its authoritative source schema from
`packages/game-server/tools/presentation.schema.json`. Its emitted schema bytes
retain their previous identity. Rust embeds that schema; SDK generation imports
it and emits types plus standalone validators. No handwritten artifact DTO is
introduced. The native host build identity includes both producer and schema.
The operator supplies an explicit SHA-256 pin for the producer's `artifacts.json`;
startup verifies that pin, producer/schema, artifact bytes, canonical catalog and
every Oracle/printing/source-digest association. The artifact is a trusted offline
build output, not arbitrary remotely supplied JSON. Request handlers only serve
startup-validated bytes; they do not reproduce source extraction.

## HTTP and asset boundary

Both `serve` and `serve-client` require exactly `127.0.0.1` or `[::1]`, nonzero
port, exact Host, and absent or matching Origin. Duplicate Host/Origin and
cross-site Fetch Metadata reject. Browser normalization of explicit port 80 is
handled while bootstrap/config retain `:80`. Forwarded headers never establish
identity. No CORS headers are emitted. The Vite adapter must first enforce its own
local Host/Origin, then rewrite to the explicitly allowlisted upstream; only the
reviewed read routes are forwarded.

All external responses carry nosniff, no-referrer, same-origin resource policy,
frame denial, and CSP: default none; self scripts/styles/fonts/connections;
self plus `https://cards.scryfall.io` images; no base, objects, frames, forms or
workers. There is no unsafe-eval/unsafe-inline or service worker. Other stored
artwork URLs remain source metadata but are blocked by CSP and use UI fallback.
See [CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy)
and [cache directives](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Cache-Control).

Only the explicit export manifest's `index.html` and single-level `assets/` JS,
CSS, WOFF2, PNG, WebP and JPEG files are loaded. No recursive directory server,
source maps, arbitrary JSON, SVG, credentials, operator files, native outputs or
working files are admitted. Bundle and registry roots cannot overlap. Symlinks in
file/ancestor paths and nonregular files reject; bytes are snapshotted at startup.
Each file is at most 16 MiB, all public files at most 64 MiB, at most 128 files.
Only `/`, `/join`, `/table`, `/about`, `/fixtures/opening` and
`/fixtures/missing-metadata` alias the index; other missing paths return 404.
Asset/HTML responses are no-store because their names need not be content hashes.
HEAD is supported for public assets; gameplay APIs require their declared methods.
Percent-encoded paths, dot segments, backslashes and absolute-form targets reject.
Queries on static artifacts reject. Generic error JSON is bounded and no-store,
including router/extractor errors. Metadata is capped at 4 MiB, descriptor/config
at 4096 bytes in the SDK. Existing host request/worker/stream bounds remain intact.

## Reproducible explicit build and launch

Use the pinned Node/Rust toolchain through `scripts/engine-env.sh`. No client or
SDK command invokes Cargo. Never replace root dependencies or share node_modules.
A native owner first acquires the documented `cargo.lock`, runs cache admission,
then `cargo test --locked -p mtg-server --lib`, builds the server and
`provision_client` example, and checks server Clippy. Copy executables to a new
ignored lane evidence directory before releasing the shared native slot. On a
fresh machine use the documented repository/toolchain cache; reuse of a historical
cache requires coordination and its explicit budget. See
[verification](game-engine-verification-workflow.md).

With an explicit copied `MTG_SERVER_BIN` (absolute path), export `schema`,
`bootstrap-schema`, `client-config-schema` and `native-info` to new artifact files.
Then, from repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_BIN=/absolute/copied/mtg-server sh scripts/engine-env.sh npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh node packages/game-server/tools/client-artifacts.ts \
  packages/game-engine/tests/fixtures/expected-match-ir.json \
  /absolute/protocol.schema.json /absolute/new-presentation
```

Alternatively provide both `MTG_SERVER_SCHEMA` and
`MTG_SERVER_BOOTSTRAP_SCHEMA` exports with their exact source qualification; the
SDK never fabricates a missing export or falls back to historical schemas. Its
browser build receipts pin producer inputs as well as native schemas. The client
lane must explicitly requalify its historical fixture capture against this final
SDK without changing capture bytes or switching SDK builds between checks.

Build the production client following its lane report with that exact SDK. Export
only the reviewed build filenames (the exporter takes an explicit list):

```sh
sh scripts/engine-env.sh node packages/game-server/tools/export-client.ts \
  /absolute/client/dist /absolute/new-public-bundle \
  index.html assets/EXACT.js assets/EXACT.css
```

Keep the resulting bundle SHA-256 and producer `artifacts.json` SHA-256 as launch
inputs. Create a **new private 0700 parent** for the registry and seat export;
never put either below the public bundle or presentation root. The explicitly
built `provision_client NEW_REGISTRY NEW_PRIVATE_EXPORT_DIR ORIGIN` helper creates
one foundations match `a1` with approved decks, random native seed, and four
0600 single-seat files. It starts no bots, prints no capabilities, and refuses
existing paths. The original issued file stays operator-private. For other
already provisioned matches use the unchanged `export-seat` command before serve.

```sh
/absolute/mtg-server serve-client /absolute/private/registry 127.0.0.1:18741 \
  /absolute/new-public-bundle BUNDLE_SHA256 \
  /absolute/new-presentation ARTIFACT_MANIFEST_SHA256
```

Open `/join` and import exactly one seat file through the file picker. The
browser origin and config must match the explicit upstream mapping. Capabilities
remain memory/header-only. Human seats have no bot consumer. SIGINT gracefully
stops only the task-owned host. Existing different-build registries remain
incompatible; retain their binary/data rather than migrating them implicitly.

## Verification and exact qualification

Evidence is retained under this worktree's ignored `generated/local-serving/`.
The initial native attempt failed before server tests because the shared Cargo
cache reused schema-1 contracts against baseline source. The full failure is
`native-first.log`. The cache owner authorized touching timestamps only on
existing local contracts/build-identity sources; `cache-invalidation.json`
records identical before/after byte hashes. No core bytes/catalogs/guards changed,
no cache or historical evidence was deleted. The retry and final results are
recorded below.

Focused producer/export tests: three passed, including exact source extraction,
face preservation, path restrictions, symlinks and explicit copying. Native: 60 server library tests passed, including four new delivery/security
cases; server plus provisioning/pause helper builds passed. Full dependency
Clippy stopped at the baseline contracts `collapsible_if` warning; preserved in
`native-second.log`, no unrelated source fix. Server all-targets Clippy with
`--no-deps -- -D warnings` passed in `native-third.log`. SDK explicit generation,
production browser build, strict TypeScript and 59 Node tests passed, including
19,763 standalone/runtime validator comparisons and five new metadata/config tests.
Production real-host browser acceptance follows in the integration checkpoint.
No full engine matrix, release/performance/retention gate, natural game, four-player
command recovery, semantic query or human usability acceptance is implied.


Initial exact artifact qualification (debug, Rust 1.99.0):

| Artifact | SHA-256 |
| --- | --- |
| mtg-server | `3f9fbfce4fe1eec45fc54e5e915540c5c8a9859dc14e5d58554d5e6cd0daa7c7` |
| provision-client | `4bd8fcf45588e1e7764a6cbf5323206284b3b7864069ccd6cd2eeec304eb11df` |
| pause-client | `c6919552359338682c21be93bce7d536885e8f495f34fc4b4df36c1bb2a52e9a` |
| protocol.schema.json | `59deeafe34996f38e7a8905df9c75e9c4c424d54e9ae33bed2a4b23634f99bc6` |
| bootstrap.schema.json | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| client-config.schema.json | `82c71b74373615553d598e126735ddb154d208ed7fdb807959bf377ad938287b` |
| native-info.json | `13491c03822925e2f5b79473651be4809bae976a041566f35df6cecbbdd9b170` |
| presentation/presentation.json | `f3d1c0c33a76565ab107c0201320072c705845cadc4508c7364f7c0b3d3b16b7` |
| presentation/presentation.schema.json | `a7045da3459cd5b6a126ca804ea22716062c63ce3127017bb5f3637c840231a6` |
| presentation/artifacts.json | `84d1b578929ddad8baab9754072c8914b21702cf5a04f32028dbe99c13192891` |

Engine build `8804fe0e03df54a16a04abf87ea7b1883da559dd880909cbf000ee4c9385d568`; server build `f0f48e58088f88a4092badc0c1160b8391eb3646c05cbd99b3fe875722034eae`.


## Production integration checkpoint

Native source checkpoint is `bb41898b5eeca0c3357c63f03ecdf7180c51e76c`
([draft PR 16](https://github.com/joCaldwell/mtg-ecosystem/pull/16)). The session
lane carries that unchanged as prerequisite; production UI source is
`20bb013c1ef7f6c3c2f7fbee0a9c47f499086d28`. The exported three-file public bundle
manifest SHA-256 is
`a9e9c6f16377dd9133f2c5dc41bd5b3b4f67573fc2f266c15be24c4dde0fbc81`.
The real host serves this bundle and the exact native executable above together
at `http://127.0.0.1:18741` with four human-owned seats and no bot processes.

The source-controlled `tools/check-local-serving.ts` probe passed 25 HTTP
route/security cases and all four seats' authenticated manifest/resume/presentation/
stream reads. It verified own-hand projection, source/build/catalog binding,
no-store APIs, immutable metadata, unauthenticated rejection, disallowed origins,
traversal, excluded operator/native paths, explicit methods, bounded errors and
empty continuation from the committed cursor. Its report contains only public
identity/status/cursor data, never capabilities or observations.
Receipt: `generated/local-serving/http-read-proof.json`; runtime source digest is
recorded with the final evidence index. This is HTTP proof, not browser proof.
A separate T3 browser opened the actual production `/join` successfully without
console errors, imported no seat, and was closed after inspection.

A clean source export of client commit `20bb013c` was extracted into the lane-owned
`generated/local-serving/clean-build/`. Fresh real local SDK/client `node_modules`
directories were installed from their lockfiles. Explicit native protocol/bootstrap
exports built the SDK; the normal client production build then succeeded without
Cargo, root dependencies or shared node_modules. All three dist hashes exactly
match the served bundle. `clean-build-proof.json` and `clean-build.log` preserve
this reproducibility evidence. Vite's large-chunk and React Router directive
warnings are retained; no performance claim follows from the successful build.

Both pinned browsers additionally passed all 12 SDK production-fixture checks
(Chromium 158.0.8127.18, Firefox 159.0). Receipt:
`packages/game-server/sdk/generated/browser-proof-1794543001840.json`.
The client lane owns the separate real-host four-seat and lifecycle browser proof;
its final qualified evidence is recorded below.

For the lifecycle test only, `pause_client REGISTRY MATCH_ID` is an explicitly
built offline helper. The supervisor must first stop; the exclusive registry lock
rejects concurrent ownership. It invokes existing `Worker::clock(Pause)`, never
edits a ledger or synthesizes a publication. Restart the identical native binary
with the same pinned public artifacts. This is a durable resource-paused lifecycle
boundary, not a Magic result, gameplay action or operator API exposed to browsers.

### Distinct core-query follow-up

The core lane published draft PR 17 at
`93515a3f1236a53cee9e6b5141361be76572fe74`, native source
`d1d039539d63aa17cd411c654e22086ccf3c9322` on schema-1 base `2c18fd3`.
Its explicit action-query outcome schema SHA-256 is
`5eb6c30f6939eadb3192eb86b3f7e2f63f6ca769aea7abefe7ddd49a5e2125e5`.
That schema is **not** an input to this M1 SDK/server protocol. Future transport
must own seat/token correlation, opaque continuation IDs, quotas/expiry and
scheduling, while core owns eligibility, budgets and private continuation work.
Incremental pages never repeat entries and a full page may need a final empty
completion; zero-budget context null does not establish token validity. Labels
remain unavailable without compiler source mapping. No M1 route or schema was
expanded for that handoff.

### Final loader correction

A source audit found that checking file type only after `File::open` could block
startup on a FIFO. The loader now rejects nonregular and oversized files before
open and retains its descriptor check afterward. The new real-FIFO regression
requires bounded rejection within two seconds. Five focused delivery/security
tests passed (56 unrelated server tests omitted on this final rerun); all three
executables rebuilt and server all-targets `clippy --no-deps -D warnings` passed.
The first final-pass process was externally interrupted with exit 143 without a
Rust diagnostic; `final/native.log` is retained. No owner reported cancelling it.
The unchanged-source retry passed in `final/native-retry.log`; no source/guard
change was made in response to the interruption.

Final binary SHA-256:
`11a68dab211e45ab237e383580007da2cd5895cc904942c6e1e68df309edd966`.
Final server build:
`4248a94314b70f9dc823e227afdb5702c9056d16982c5b108b36413dcd887fcf`.
Engine build remains `8804fe0e03df54a16a04abf87ea7b1883da559dd880909cbf000ee4c9385d568`.
Both freshly exported protocol/bootstrap schemas are byte-identical to the first
checkpoint's `59deeafe…` / `0655664d…` exports. Producer, artifact, SDK runtime and
client production bytes are unchanged. Final executables/exports and new private
registry/seats live separately in `generated/local-serving/final/`; the earlier
binary, registry, failed checks and browser evidence remain untouched. The final
host receives fresh identities rather than attempting a different-build recovery.


Final native source is `6bf0609eed12273f6dae44d7cc97766299ebbcc4`; SDK runtime
remains byte-equal to `bb41898`, and the production client bundle remains
byte-equal to `20bb013c`. The final real HTTP probe again passed all 25 security
cases and all four seats' read paths, retained in `final/http-read-proof.json`.
`final/artifact-identities.json` records exact final helper, binary, schema and
receipt hashes. Fresh schema equality means no SDK runtime regeneration or
unrelated repeat of the 59 SDK tests was needed after the loader-only correction.

The initial browser lifecycle exercise retained the seven-card hand and keyboard
focus during reconnection, then reached its existing bounded disconnected state
before the manually coordinated restart completed. Explicit reimport recovered
cursor 2, resource_paused and the identical rendered hand with no rules outcome.
This establishes bounded-stop/reimport behavior, **not** automatic reconnect for
that outage. The original lifecycle receipt remains separate from the timed
final-binary exercise below; the retry budget was not raised to make a test pass.


## Final M1 serving acceptance

The session lane's final Playwright run against native source `6bf0609` passed
**18/18** production native-browser scenarios in Chromium 158.0.8127.18 and
Firefox 159.0, with zero skips, failures or retries. Four restricted-proxy browser
scenarios and eleven proxy security probes also passed. These include actual
four-seat read/privacy/ownership paths, not a synthetic host substituted for Rust.
The native host served the exact production files reproduced by the clean build.

A separate held T3 browser (Chrome 152.0.7977.169 / Electron 44.10.1) then proved
automatic reconnect through the final same-binary durable pause/restart:

- Shutdown: 70.45 ms; existing offline `Worker::clock(Pause)`: 1390.65 ms;
  restart to HTTP readiness: 2299.98 ms; total: **3761.08 ms**.
- Cursor advanced 1 → 2 with seven own-hand cards and the same rendered-hand
  SHA-256; focus remained on `decision-title`.
- Display stayed connected after recovery and reported resource paused, with no
  rules outcome, credential reimport or gameplay command. Retry policy unchanged.
- Four subsequent long polls returned HTTP 200 approximately 20.1 seconds apart.

The redacted peer receipts were inspected, SHA-256 verified and copied to this
lane's `generated/local-serving/final/client-proof/`:

| Receipt | SHA-256 |
| --- | --- |
| final-native-browser-report.json | `5b165bca5890a64c45b6139cfdbb6934865102d33754f23f7a7a8a4dc10dd45b` |
| final-proxy-browser-report.json | `b5fe44ebdd7d6315eb3df92bc1a5a51879a8a1059044dba2d6798c1d2bd3c437` |
| proxy-security.json | `1615175ff62a1f383bb408b4530f8e0a82316d40645f97647945c069fdc0b861` |
| final-lifecycle-browser.json | `f158bf90c6a230907c0c69ec12a89992b5f50db32357ea79864e8ba1772ce901` |

Local receipts `final/transition-timing.json` and `final/lifecycle-resume.json`
independently qualify the host timing and committed cursor/status. The final
native runtime source is `6bf0609`, SDK runtime `bb41898`, and client production
runtime `20bb013c`; report-only commits do not change those identities. No private
capability/observation, HAR, browser trace or video was included in these receipts.

After browser ownership was explicitly released, the task-owned host was stopped
with SIGINT, validating its exact executable/PID first. `final/shutdown.json`
records completion. All native slots and task browser contexts are released; no
task-owned live server or bot remains. Private seat files, registries, exact
executables, clean-build outputs and meaningful failed results are retained.

This completes the assigned M1 **serving/SDK** deliverable. It does not accept
M2 gameplay controls/journals, semantic query transport, natural matches, four-seat
command recovery, human usability, deployment, or a full engine/release/performance/
retention matrix. Integration with current main's separately merged schema-1 reset
still requires explicit native/schema/catalog and client requalification before
merge; no compatibility shim or silent artifact switch was introduced here.

The browser harness and its source-qualified report are committed in the client
lane at `2b4526b5c3d1a292490d3b45137fa268887f2500`,
[draft PR 18](https://github.com/joCaldwell/mtg-ecosystem/pull/18), stacked on this
serving PR. See its
[immutable report](https://github.com/joCaldwell/mtg-ecosystem/blob/2b4526b5c3d1a292490d3b45137fa268887f2500/packages/game-client/LIVE-SESSION-REPORT.md)
and committed `tests/live/read-only.spec.ts` / `playwright.live.config.ts`.
That commit qualifies the final `6bf0609` native binary while retaining the exact
`20bb013c` production runtime bytes; it is not a new server/native measurement.

## Independent-review fixes and schema-1 integration

Native source `147dec571e58a531b1229f61a9f78733c021eadf` integrates current main
`bcb9cb45b2ca6bbf0a01dc25f325ac5811b269a2` (including reviewed core queries),
and corrects both retained-source layouts. The simulator explicitly retains the
presentation producer and schema required by the host build. The SDK reconstruction
copies those two pinned native files plus both foundations/attachment fixtures to
their repository-relative paths, and checks the exact input closure again after
verification. This also repairs the previously documented attachment-fixture gap.
No semantic engine change or query transport is introduced by the closure fix.

Passed on pinned tools: one native source-list regression, all 61 server library
tests, server/provision/pause builds and all-target server Clippy with `--no-deps`;
18 retention-tool tests; three producer/export tests; strict TypeScript and 59 SDK
tests in both the full checkout and a fresh minimal retained-input reconstruction.
That reconstruction contains only SDK sources, the two producer files and two
fixtures, with a fresh locked install. It establishes those declared inputs without
claiming a full offline Cargo/npm retention run. Both browser CSP suites passed.
All native checks used the shared Cargo lock and existing cache; byte-preserving
timestamp refresh is recorded. No historical cache, registry or receipt was removed.

Fresh protocol SHA-256:
`2796d38cc8dc4ef67d836fbe890ba31170869f4562c91b8edf7c6f1b91396e64`.
Bootstrap and client-config schema hashes are unchanged. Engine build:
`81b5711e7ec5dde8dd55cdb8abc73d9f151464ed5d0373f7dd4039a9ef069606`.
Server build: `2e0c848d22e2ef3232ebf0576cd882faca91bc9594e1cfed2f6551fb771176e2`.
Fresh schema-1 presentation artifacts and a new public bundle from client source
`865df11bd13373b38192878764fcfc3303ae8f0b` were exported explicitly. Private registry
and four seat files were newly provisioned for these exact build/catalog identities.

Combined acceptance: 25 real HTTP security/route checks and all four seat reads;
59 SDK tests followed by 34 client tests/type/boundary/build checks without schema
switching; 18 native-browser, four proxy-browser, 11 proxy-security and 10 fixture
browser cases passed. An initial proxy test launch used the wrong cwd and failed;
that run is retained, the package-cwd launch was corrected, and all four cases
passed afresh. No product guard was weakened.

A 4.122-second same-binary stop/pause/restart automatically advanced the held
Chromium browser from cursor 1 to 2, preserved the exact seven-card hand and focus,
reported resource_paused with no outcome, and completed four more normal long
polls without reimport or commands. All browser contexts and the owned host are
closed. Detailed hashes, source pins and copied client receipts are in this
worktree's `generated/review-m1/qualification.json`; native/test/build/Clippy logs,
exports, minimal reconstruction and private files are retained in that directory.
The client lane retains original reports and lifecycle harness in its own
`generated/review-m1/` and `packages/game-client/generated/`.

This supersedes the schema-1 premerge qualification gap for the integrated source
above. Core query transport, command journals, natural games, full retention,
performance and human usability remain separate milestones.
