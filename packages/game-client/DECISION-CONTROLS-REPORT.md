# Local decision controls and coverage evidence

This slice adds reusable local drafts and an isolated `/fixtures/decisions`
gallery. It sends no gameplay commands. The real table and existing
`browserReadSession` remain read-only. Nine request shapes have local response
adapters; this is not full M2, playability, semantic legality or whole-profile
coverage.

Base: clean T3-bound `144fe059ab2707877109d7e9dbe8751f204681dd` on
`feat/game-client-decision-controls`. Runtime implementation:
`f33fbf1c77d7d62325148ff0b1d5e19c8a7d5de4`. Fresh-evidence checkpoint:
`067114d0d1260f8beeafaf7c46ccbbad3233231a`; immutable-manifest binding:
`92478316be22e1fd5d0be8baf8d6e64d3baac57a`. This report follows those code
checkpoints. Authored paths are client-only; app composition and the Vite fixture
route allowlist are the only changes outside decisions, gallery, scoped tests,
inventory tooling and fixtures. No session, SDK, native, dependency, lockfile or
sdk-qualification edits; no Cargo, cleanup, merge or deployment. The original
shared checkout and all other worktrees/evidence remain untouched.

## Delivered contract and coverage

`src/decisions/controls.ts` switches exhaustively over generated `PlayerRequest`
and constructs only generated `PlayerResponse` branches. No standalone gameplay
wire types, rules defaults, text/IR interpretation or automatic pass/payment.
`fixtures/decision-controls/coverage.json` is the durable consumer inventory:
28 wire requests and 33 wire responses, separately recording fields, required
fields, known admitted evidence, implemented control, missing core/source mapping,
permitted fixture and test/gallery locator. Schema presence never implies admission.
Its generator checks exact SDK generation/build/source/artifact hashes and pinned
historical fixture provenance. Normal client tests reject stale inventory.

| Local adapter | Supplied bookkeeping | Evidence limit |
| --- | --- | --- |
| mulligan | Explicit keep/take; can_mulligan and displayed counts | Historical permitted HTTP opening in gallery; fresh offline openings from both admitted profiles separately validated |
| bottom_opening | Unique supplied handles, exact count, explicit order | Synthetic structural unit/gallery cases; no captured admitted request |
| discard, resolve_discard | Unique supplied handles and exact count | Synthetic units; discard gallery only; no captured admitted request |
| legend_rule | One supplied keep handle | Synthetic unit; no captured admitted request |
| sacrifice, sacrifice_cost, forced_sacrifice | One supplied object handle | Synthetic units; no captured admitted request |
| search_lands | Supplied search-result handles, zero through supplied maximum | Synthetic unit; no captured admitted request |

The remaining 19 requests explicitly return 11 `context-missing` and eight
`unsupported` states. Numeric mode/ability indices, optional-effect booleans and
unlabeled trigger/effect handles are not human explanations. X, color, optional
effects/recursion, modes, trigger ordering, replacement/allocation and commander
return require reviewed source/context mappings. Priority/mana require bound
action-query integration and source labels. Joint targeting, payment and combat
controls remain unimplemented. Generic quantity entry is exact canonical
nonnegative text/BigInt syntax with an explicit input-size limit, including values
beyond Number precision. It constructs no gameplay response. There is no damage
allocation or payment-legality adapter; missing costs, eligibility and lethal
requirements are never computed locally.

Components accept immutable inputs and typed callbacks. Native radios, checkboxes,
text inputs and buttons provide keyboard completion. Every candidate retains its
own visible handle even when names match or metadata is absent. Selection/count,
order and canonical syntax checks are local shape checks; core remains the final
joint validator. `DecisionPanel` binds origin, match, seat, incarnation, revision
and token. Binding changes discard intent before a stale render can commit;
candidate changes remove only absent handles. No selection transfers by name.
Stable DOM identity and polite announcements preserve focus on remote-shaped
updates; ordering is user-initiated and retains keyboard focus.

The gallery uses the unchanged qualified historical foundations capture and its
existing producer presentation mapping. Captured hand inspection is available;
no new card facts are invented. Structural discard/bottom/quantity/modes examples
are visibly synthetic and contain no card facts. Components contain no poller,
storage, network or SDK runtime. Confirming emits only an in-memory local callback.
Refresh discards the local draft. The fixture origin is explicitly
`fixture:foundations-opening`, never a usable server authorization binding.

## Exact qualification

One baseline SDK build was used for every SDK/client check. Unchanged
`sdk-qualification.json` pins native/SDK source
`936566b76e9819c4503c9065590be2a66037d26a`. Historical fixture provenance and
its original source/schema hashes are unchanged; current validation is separate.

| Artifact | SHA-256 |
| --- | --- |
| Tested SDK protocol | `7fe738bb5a4aecaedb23a9a6405691c6497c6b117b38464aacde6601f152204c` |
| Tested SDK bootstrap | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| SDK dist/build.json | `72c04c45977288dcb74055d7f5238f3a91f586949bcd17250a56145e065d8717` |
| SDK generated/generation.json | `6d4ccd06a8cb00bd3133b1d7363e009525159dcec6047cf1ec79eb79e411e58d` |
| Client package lock | `1f837860ed1ab73700fbf18193eb1deaa1e2fadee31f520a1d808ae9fa38a45e` |
| SDK package lock | `e5034526be40dc01509df43489d7a4e7dd6bf259e7258a1ad76868cd925480d9` |
| Coverage inventory | `c89cbad07e7a9ddacfe8c91e6b30b7cd68de3859fc1bfd4028e28da055449350` |
| Prefix provenance | `bd4b542c92689c5f91108bc7ef1a63af508ff36743702f9b01bd432ba0a94511` |
| Client dist/index.html | `a04cda73b6c3c8fe2d5a01832faa8f68d1810a01372083662a9fd68b1d7d520a` |
| Client dist/assets/index-YRmsUjMs.js | `b9151a8e54cd93c1ac6466a8bba37de6f874c5814b5e0a22d83f11a15adf4d10` |
| Client dist/assets/index-DabgF6tn.css | `96cb1999ecb6e3dbb96f20cd07775b52604f2f21ca92c4f5fc03d83a9528b26e` |

Local `generated/decision-controls/qualification.json` hashes code sources, logs,
builds, inventory, fixture verification and browser receipts. Its SHA-256 is
`7f4cd55d4fa9cba482a8f95b0fa86ef2f629cf49aa40916bddc569bba15e9dbb`.
Raw generated/dist outputs remain ignored. They are retained, not cleaned.

## Fresh producer evidence, separate from runtime qualification

The action/native lane supplied an explicit producer directory:
`/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/attempt02/`.
`scripts/decision-prefixes.mjs` hashes all seven named producer artifacts plus
the source manifest without executing native code. It validates eight permitted
observations against both the tested baseline schema and the separately supplied
new producer schema, and six queries against that new schema. Match/viewer/token,
namespace/catalog and engine/server identities are checked against the manifest.
The resulting small tracked `prefix-provenance.json` records stage hashes and
observed query action families. Raw captures stay outside the runtime bundle.

Exact producer identities:

- Native checkpoint `1e6fbefffb908cdf7dd741c4baa97f6467c4c85d`; separately
  reported SDK checkpoint `76b5f3a1e4cfe8e32189fd197b3f1edf05670f80` is not installed here.
- Native source manifest `ba6379af2980ffc855635c9662c3989af1a11b93ff8e635fda1d778362fd7692`.
- Engine build `0e8abd9759e99c6be84492e859396a90fcdc4254b88d125e294114f2ed50730d`;
  server build `99d06fefdcff1569acd690b5c4f1f7d78ecc34452399f3530b39cbc45acf3777`.
- Evidence-only protocol `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`;
  bootstrap unchanged. This schema was never substituted into SDK generation.
- `public-prefixes.json` `5dc2b69175a9b4a5f42fda822cd52f57a6acf4a7311289a9a90b47c554aca9f4`.
- Immutable `producer-manifest-checkpoint.json`
  `0092c3fe4e4e70aa7e11682cb024fd1994d04b7e689e9b17977b0759e271bcbf`
  matches the original measured receipt and is the verifier's fixed input.
- The later separately frozen `producer-manifest-final.json`
  `8a802cb0ea26f53cef88415fb50b3c81a390d8206e0f107c77ecfcab46510aa9`
  adds presentation annotations. Its presentation inputs are not runtime gallery
  inputs in this slice; no source-ability labels become available.

Both isolated profile/seed3/offline namespaces contain opening mulligan,
main-priority-before-land, main-priority-before-cast and cast-mana stages.
Observed query initiation families are pass, cast, play_land, activate_mana and
finish_mana. These are permitted beginnings, not accepted-command evidence.
All captured labels remain unavailable and ordinary cast-mana source context is
unavailable. No unobserved family, whole-profile reachability, completed match,
live query integration or new source mapping follows from these prefixes.

## Reproducible setup and focused checks

From repository root, use the pinned Node 26.11.1 environment. These are isolated
locked installs; no root install and no implicit native generation:

```sh
sh scripts/engine-env.sh npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh npm --prefix packages/game-client ci --workspaces=false --ignore-scripts
sh scripts/engine-env.sh env \
  MTG_SERVER_SCHEMA=/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/baseline/protocol.schema.json \
  MTG_SERVER_BOOTSTRAP_SCHEMA=/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/baseline/bootstrap.schema.json \
  npm --prefix packages/game-server/sdk run verify --workspaces=false
sh scripts/engine-env.sh env \
  MTG_DECISION_PRODUCER_DIR=/home/josh/.t3/worktrees/mtg-ecosystem/feat-game-client-action-ee918eaddd68f820/generated/action-transport/attempt02 \
  node packages/game-client/scripts/decision-prefixes.mjs
sh scripts/engine-env.sh node packages/game-client/scripts/decision-inventory.mjs
sh scripts/engine-env.sh npm --prefix packages/game-client run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/game-client run test:browser --workspaces=false
```

The baseline exports are also preserved under this worktree's
`packages/game-client/generated/decision-controls/inputs/`. Reproduction elsewhere
requires the explicit retained producer exports/binaries; no command rebuilds
native artifacts. `--write` on either inventory script is an explicit reviewed
regeneration step, not a test fallback. Browser checks use the package's already
installed Playwright 1.65.0 Chromium/Firefox binaries; they do not download tools.
Use the client dev/preview surface for `/fixtures/decisions`; the native serving
allowlist has not been extended for this isolated route.

Passed checks, with logs under `generated/decision-controls/`:

- `sdk-verify.log`: strict SDK types, browser build and 62 Node tests, 33 response
  variants. This exact SDK build remains unchanged through all client checks.
- `check-final.log`, `check-prefix-final.log`: fixture/schema/hash preparation,
  strict client types, import/effect guards over 26 source files, 45 Node tests
  (seven new decision/inventory cases), runtime module allowlist and production build.
- `prefixes-frozen-check.log`: all producer hashes, eight observations and six
  query schemas/bindings passed. `inventory-frozen-test.log` checks final inventory.
- `browser-02.log`: 22 production cases, including all ten existing read-only
  table cases and 12 new gallery cases. No skips/retries. `browser-04.log`:
  final 12 gallery cases passed after improving the captured-hand keyboard case.
  Runtime source is unchanged between these runs.
- Chromium 158.0.8127.18 and Firefox 159.0: explicit radio/checkbox/Enter
  completion, visible keyboard focus, stable same-label identity, order/focus,
  candidate removal, incarnation/token invalidation, polite announcements,
  exact huge values, invalid syntax, blocked context, reduced motion, 640px
  layout, route refresh, no API requests and empty local/session/IndexedDB/cache/
  service-worker/cookie state. Desktop/narrow screenshots were visually inspected.
- `git diff --check` passed. No native or live-host checks were run by this lane.

## Preserved failures

`check-02.log` retains a test-only TypeScript failure: Playwright locator elements
can be SVGElement; the synthetic update helper now narrows to HTMLButtonElement
before invoking click. No type suppression or boundary relaxation.

`browser-03.log`, `browser-03-report.json` and copied `browser-03-results/`
retain two failed visible-focus assertions. The new test clicked a details
summary then programmatically focused a radio, retaining pointer modality.
The corrected test uses Tab through the source-inspection link into the radio
and keeps the visible-outline assertion. Both browsers then passed all six cases.
The component's existing focus-within cue was already visible; no product CSS
or browser expectation was weakened for this correction.

`prefixes-final.log` retains rejection of the generic producer manifest after
the producer appended metadata. The producer confirmed unchanged artifact/source/
schema/capture bytes and supplied frozen versions. Original measured checkpoint
manifest/provenance bytes are retained locally and in commit `067114d0`; the
verifier now explicitly names the immutable matching checkpoint. The later final
manifest has its own separately recorded hash above. No schema pins, historical
fixture facts or source identities were rewritten to bypass this failure.

Existing React Router module-directive and production chunk-size warnings remain;
bundle size is not a performance measurement. No full engine, performance,
retention, natural-game or cross-device matrix, accessibility audit/screen-reader
study, Safari or real human usability acceptance is claimed.

## Integration handoff

The journal owner confirmed `DecisionIntent { kind:'decision', source:'human',
context:CommandContext, response:PlayerResponse }`, where context retains full
JournalBinding, incarnation, revision and token. A future app owner must capture
that ORIGINAL full context when creating a draft, compare this completion's local
binding against it, and submit through `submitIntent` only after session recovery,
ownership, compatibility and durable-before-send gates. Never attach a fresh
context to an old response. Incarnation advances even after definitive rejection
with an unchanged token/revision; such a draft must be discarded. This slice
imports no session command interface and does not compose browserCommandSession.

The native/SDK owner owns deliberate new-schema SDK qualification and action-query
transport. After integrating those changes, regenerate this inventory explicitly
from one consistent SDK build; its source/artifact guard intentionally rejects
stale qualification. Query completeness/continuation and stale/unavailable handling
belong to session integration, not controls. Compiler/core must supply permitted
source/action/mode/trigger/effect identities and source-label mappings before
ambiguous controls are enabled. Rich initiating context for the simple selection
families and additional source-qualified captures remain required for reviewed
live presentation. Extend the native SPA allowlist only if the isolated gallery
is deliberately included in a later hosted build. No live control wiring, merge
or deployment is authorized by this checkpoint's passing fixture checks.

## Parent combined-source qualification

Parent combined current main `0b4a8757`, reviewed action transport and the journal
branch at `7673f19c0885c98b4af7ceb1c02635ad243dfd52`. Native/SDK source is
`e86df7e73f5ac896a4b2488779efb46ef28240d4`, explicitly rebuilt after the main
compiler correction. Protocol is now deliberately qualified to
`a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`.
The prepared-command prerequisite is preserved once. Historical fixture and
prefix capture/provenance facts above are unchanged. The controls inventory was
explicitly regenerated with `decision-inventory.mjs --write` from that same
current SDK build; it still reports 28 requests/33 responses, nine local adapters
and 19 blocked shapes. No missing context was inferred to bypass those gates.

Combined generation/strict types and all 66 SDK tests passed. Then client strict
types, boundaries, all 67 tests and production build, all 22 production fixture
browser cases and all 16 actual IndexedDB journal browser cases passed, without
switching SDK artifacts. Root `generated/m2-integration/` retains these new logs,
inventory output, exported production assets and qualification hashes. Older
standalone evidence remains separately qualified above.

An additional automated integration harness composed the actual command-session
factory, pure control adapter and built SDK against fresh native `serve-client`,
through a test-only same-origin proxy. Chromium and Firefox each imported an
actual seat file, captured the original full CommandContext, and explicitly kept
the source-backed permitted opening hand through a test button. The journal
persisted before send; its exact body hash matched the genuine native POST. The
proxy dropped the response body only after native HTTP 200. Local stop preserved
the unresolved slot. Navigation/reimport recovered through native execution status
and a fresh coherent resume; `consumed_by_rules` cleared the slot. Each browser
sent exactly one command, advanced its own cursor, and ignored the old intent.
No synthetic execution status or game response was supplied to make that pass.

The receipt is `generated/m2-integration/native-journal-02/report.json` and the
reproducible source is `native-journal.mjs`, both in this worktree. The initial
run checked for the opening immediately after HTTP readiness while native progress
was still pending. Its failure/log/private registry are retained as attempt 01;
the corrected harness waits within a fixed bound for the committed request.
No product code changed for that harness correction. All owned browsers and
native host processes are stopped. `docs/game-client-action-transport.md` records
the fresh native build identities, source fingerprints and 40 focused tests.

This proves automated source-module/SDK/native journal integration with a local
control completion. The deployed App remains read-only, the product development
proxy remains read-only, and the native gallery route remains excluded. Enabling
reviewed live controls, session-owned query lifetime, source labels/context,
unobserved families and human play remain later implementation gates.
