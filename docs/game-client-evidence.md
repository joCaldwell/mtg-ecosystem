# Client and product acceptance evidence

[Client contracts](game-client.md) own current behavior and delivery gates;
[application contracts](multiverse-app.md) own product launch. These records retain
their exact producer/source/schema qualification; they are not current-engine-HEAD
acceptance or a requirement to repeat the entire matrix for documentation edits.
Original worktrees and raw ignored evidence are preserved, not cleanup targets.

## Opening and recovery evidence

The parent opening lane retained `generated/opening-integration/`, separate from
all earlier producer evidence. Historical 2aaf/e86 captures and reports above
retain their original scope. Updated `sdk-qualification.json` and regenerated
coverage change only current native/SDK source references and explanatory scope;
historical capture provenance and UI evidence hashes remain untouched.

| Fresh combined-source check | Result |
| --- | --- |
| Native delivery / HTTP delivery / protocol adversarial | 4 + 1 + 10 passed; fresh debug server and provisioner built under shared Cargo lock. |
| Native source closure | 677 exact Git-source rows; before/after identical; engine and server build IDs independently rederived from actual source/toolchain/configuration. |
| SDK from fresh exports | Generation, strict types, browser packaging and 66 tests passed; schema/bootstrap and SDK generation/build/browser bytes unchanged. |
| Client after current qualification | 114 tests, strict types, boundaries, inventory and production build passed; final copied production files match checked output. |
| Actual production App and new native | Ten opening/recovery case groups across Chromium and Firefox passed, including four isolated seats, explicit keeps/mulligans/bottom order, durable response loss, exact retry, stale rejection, and storage failures. |
| Restricted production proxy and new native | 17 route/security, four authenticated schema/body and five execution/cancellation restriction checks passed; genuine commands/execution reads passed in both browsers. |

Fresh engine build: `f9422adcd77e57bdc075646f25cc21cf923438d327794bc6944efb4166899343`. Fresh server build:
`ca2eda503a3e7a72472c149ba096cf1f6dee4864b9dd4b317aa67b2ecfb90292`. Native source manifest SHA-256:
`b7e30b5e01dfba5a0ab9d3e76996840803b35745dae878009c512ae60e5c8191`. Source-validation, build/test logs and copied
binaries are under `native/`; `verify-source.py` records the independent build
identity calculation. App receipt: `accept/report.json` (SHA-256
`3689e9c5c675a081fa013255a608a170d6d8d655d2aeb30946f0d70a40fae245`); proxy receipt: `proxy/report.json` (SHA-256
`b84bd34de35189397676bd00d8da605bb733fac1ff95e3bb54c4d01cdbf1721f`). `qualification.json` hashes sources, public
artifacts, review receipts, checks and proof scripts; private registry/seat files
are excluded from that public fingerprint list and remain under the private
0700 evidence root. No authenticated traces/HAR/video were recorded.

The shared build lock was released immediately after copying/validating native
outputs, and the engine lane was notified. This build used the surviving standard
target `/home/josh/.cache/mtg-ecosystem/cargo/d5481adfe9c96682/8bf11357d0994b4b`,
starting empty and ending at about 3.72 GB, with incremental compilation disabled.
No retired performance path was accessed. All owned test hosts/browsers closed;
the selected ports were independently checked free. No failures occurred in this
parent combined pass; earlier meaningful failed evidence remains preserved.

No new full game, human usability, full M2, query-menu composition, expanded
hosting/search, natural match, Safari, performance or full retention claim follows.
Separate fixture/gallery, IndexedDB and CSP suites were already independently
verified and were not repeated in this parent pass; actual App recovery and proxy
qualification above used fresh current-source native binaries and new seats.

## Product/shared display evidence (PR24)

Original lane: `/home/josh/.t3/worktrees/mtg-ecosystem/t3-7ab024bd-7766f83a106e0c59`.
Workspace source prerequisite `a80c2f5251c07de92245d085b63fd7c5f4937ccb`;
clean-consumer proof `generated/multiverse-clean-consumer-59576cab.json`.

| Check | Result |
| --- | --- |
| Backend `check` | Strict types and 103 tests passed |
| Product `check` | Server/browser types and 31 tests passed |
| Shared game UI `check` | Types, boundary guard and 3 tests passed |
| Client `check` | Types, boundaries, qualified fixtures, 116 tests and build passed |
| Standalone production browser checks | 28 Chromium/Firefox checks passed |
| Integrated product browser checks | Seven groups passed: account/deck setup, preview/draft/focus/return, deep links, delayed deck reads, cross-account denial, signed-out gating, desktop/tablet/phone access |
| Product production build/public budget | Build passed; public closure 104,509 gzip bytes, below existing 110,000-byte gate |
| Clean product consumer | Types and production build passed with SDK/client generated directories absent; all held bytes restored identically |
| Runtime import audit | Backend 389 imports and product 3 imports passed |
| Moby Docker context | 604 included files and 11 sensitive/generated exclusion checks passed |

The final browser fixture uses a disposable source-backed seed copy and mocked
test actors, rather than production auth or owner records. The optional review
server's default actor is a fixture-only option; production has no such setting.
Screenshots were inspected at desktop and phone sizes. Shared controls were also
inspected against the host reset: visible button borders/padding and original
card padding remain intact. The game chunk stays lazy.

### Retained proofs and omissions

Final command logs are `generated/product-{backend,app,ui,game-client}-final-check.log`
and `generated/product-game-browser-final.log`. The clean-consumer proof is
`generated/multiverse-clean-consumer-59576cab.json`; earlier clean-consumer logs
remain preserved. Integrated browser report/screenshots are under
`packages/deck-builder/.dev/product-shell/final-top-level/`. Runtime/context logs,
initial typecheck failures and the lifecycle probe remain in their generated or
package `.dev` evidence directories. Initial failures were preserved and fixed.

The actual Docker image build was not run successfully: this environment has no
Docker daemon/socket (`generated/product-docker-build.log`). No deployment,
native host game, automated match launch, full-game browser play, dense-board
acceptance or strategic-opponent quality is claimed. Those require the explicit
coordinator and admission work in `docs/multiverse-app.md`.

### Pre-merge integration

Integrated main at `ac95b1cf` before the requested fresh review. Its engine/doc
changes required refreshing only the decision inventory's current-map digest;
no application runtime or qualification pins changed. The client `check` passed
again, including all 116 tests and the production build. The log is
`generated/pr24-current-main-client-check.log`. No native matrix was repeated.

## Earlier prerequisites and unique provenance

Older foundation/read-only/decision/journal/action-lifecycle reports describe bounded
prerequisites, not today's entire App. Their exact historical source and receipt tables
are recoverable at the documentation baseline below. Tracked fixture provenance,
prefix-provenance, inventory and qualification pins remain in place. In particular,
`generated/action-transport/attempt02` and `generated/m2-integration` belong to the
original action transport/controls lanes; `generated/core-queries/attempt02` belongs
to the core query lane, whose source/schema pins are retained in its contract.
No raw ignored evidence or source fixture was deleted.

- [Original game-client-action-transport](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-client-action-transport.md) — frozen source, artifact identities, failures and omissions.
- [Original game-client-local-serving](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-client-local-serving.md) — frozen source, artifact identities, failures and omissions.
- [Original game-client-native-integration](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-client-native-integration.md) — frozen source, artifact identities, failures and omissions.
- [Original game-client-browser-sdk](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/game-client-browser-sdk.md) — frozen source, artifact identities, failures and omissions.
- [Original foundation evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/FOUNDATION-REPORT.md).
- [Original live-session evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/LIVE-SESSION-REPORT.md).
- [Original decision-controls evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/DECISION-CONTROLS-REPORT.md).
- [Original command-journal evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/COMMAND-JOURNAL-REPORT.md).
- [Original action-session evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/ACTION-SESSION-REPORT.md).
- [Original live-opening evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/LIVE-OPENING-REPORT.md).
- [Original workspace-ux evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client/WORKSPACE-UX-REPORT.md).
