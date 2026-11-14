# Commander workspace UX checkpoint

Implementation base: `46ef08faedb8c5fa9c1c1fa9a8afefd1f85e1f6c`.
Worktree: `/home/josh/.t3/worktrees/mtg-ecosystem/t3-7ab024bd-7766f83a106e0c59`.
Branch: `t3/game-client-workspace-ux`.

This pass applies the neutral-black product identity from deck-builder's
`web/DESIGN.md` and base design tokens only. No playtester brief, table component
or playtester implementation was used as guidance. The game-client manuals and
its existing permitted views remain authoritative.

## Result

- A viewport workspace replaces the vertically stacked desktop table. Stable
  player summaries sit above a primary battlefield and horizontally scrolling own
  hand. The own command zone sits directly above the hand. Current decision and
  stack occupy an independently scrolling adjacent column; the card inspector
  remains visible beside it. Phase, turn and supplied priority are in the toolbar.
- Local black/neutral surface, typography, mono numeric, spacing and blue action
  tokens apply across table, live session status, joining, gallery and build pages.
  Source text stays readable without artwork. Empty battlefield space is compact
  at 1280×720 and remains available for actual supplied objects on larger screens.
- Unknown/pending saved-choice messages and retry controls are composed inside the
  current decision, next to opening controls. When there is no table view they
  remain available on the live-table fallback. No command/session semantics changed.
- Source identity and verbose decision details remain available through disclosures.
  Printed reference text, current observed characteristics and source identity
  remain distinct in the inspector.
- The decision gallery now inspects its exact captured hand inline, retaining the
  local draft. Its inspector and local completion receipt remain beside the
  controls on desktop. Synthetic exercises remain clearly separated and local.
- Exact inspection handles, duplicate names, keyboard decision jump, Escape/Close
  focus return, reduced motion and missing-metadata fallbacks are preserved.
  There are no inferred active-seat indicators, new action menus, made-up card
  facts or automatic gameplay controls.

## Verification

Use `sh scripts/engine-env.sh` and isolated package npm commands. No native build
was started and no qualification pins, session/controller contracts, SDK source,
root dependencies or captured card fixture bytes were changed.

- SDK explicit export-file verification passed: 66 Node tests, strict types and
  browser packaging. Current locked SDK sources were verified against retained
  exact producer exports under
  `/home/josh/.t3/worktrees/mtg-ecosystem/t3-830a8e84-6af8cf2da78714a7/generated/pr22-review/source/generated/action-session/inputs/`.
  Protocol `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`,
  bootstrap `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
  SDK generation `c8507de687345e9df5b911f3ddb12e072e8b52d7533ee901ce8268d3e26bcb5b`,
  browser build manifest `793333249d3f65e391505bb346415ef5ec010141e5c0e7574a9d4e9ee27f8123`.
  Existing qualification records integration producer source
  `ebfa6dde266139c45220959c4179ddedf6a38433`; fixture capture source remains
  `5ff4fd94b6dace1cdee0f0223305e054c42af8f3`. This pass reuses exported bytes;
  it does not claim a newly built native producer.
- Client `check`: strict TypeScript, import/effect boundaries, all 114 Node tests,
  qualified fixture preparation and production build passed.
- Client `test:browser`: 26 Chromium/Firefox production-fixture checks passed
  (Chromium 158.0.8127.18, Firefox 159.0, Playwright 1.65.0, Linux x64).
  Consequential new checks cover simultaneous hand/decision/inspection at
  1280×720 and 2560×1440, access/focus return at 960/800/640/375 pixels, exact
  same-name hand selection, source disclosure and local draft survival during
  inline card inspection. Existing CSP, no-API/no-persistence, keyboard ordering,
  remote-shaped draft invalidation and missing metadata checks remain passing.
- A real-host read-only test now also checks that an unsubmitted opening draft
  remains checked and enabled after inspection and that choice status belongs to
  the decision region. This live test was **not run**: no native host/private seat
  files were supplied. No authenticated or full-game acceptance is claimed.
- No native matrix, regenerated native schema, natural game, dense new card fixture,
  Safari acceptance or user study was run. Existing generated-artifact manifests
  and original public fixture bytes remain in place.

The source-qualified decision coverage receipt was refreshed after UI/test changes.
It also fixes two already stale source hashes (current engine map and visibility
source) without changing coverage classifications, fixture data or SDK qualification.

## Evidence and failures

Ignored evidence is retained under `generated/workspace-ux/`. Final `check.log`
and `browser.log` record focused checks. Browser report and screenshots also remain
under `generated/browser-results/` and `generated/browser-report.json`.

The initial SDK verification invocation omitted explicit schema environment inputs
and failed before generation. Its log is retained at
`../game-server/sdk/generated/verification-missing-inputs.log`; the explicitly
configured retry passed. Initial client checks found a stale coverage receipt.
Initial browser failures expected newly disclosed provenance to be immediately
visible; tests now open that disclosure. A later new viewport check exposed an
accessible region name including its visual hand count; the count is now excluded
from the heading's accessible name, preserving the established "Your hand" landmark.
Those artifacts remain under `generated/workspace-ux/initial-failures/` and
`second-failures/`. One overlapping browser invocation found port 4179 busy and
stopped before tests; `port-conflict.log` is retained. Final runs were sequential.

An ad hoc preview invocation from the repository root selected Vite's default
root instead of the client package. Numeric loopback refused the connection and
the shared T3 environment-port preview returned an empty 404. Its tab was closed.
The capture script initially hit the same refused URL. Starting preview from the
client package corrected setup; production Playwright screenshots then covered
join, disconnected live table, about, 1280/2560 inspection, narrow table and the
gallery. The client's Host/Origin policy was unchanged. Build logs retain the bundle-size and
React Router module-directive warnings; no dependency/bundling redesign was made.

Stable screenshot copies (ignored, public fixture/disconnected routes only):

- `generated/workspace-ux/table-inspecting-1280.png`
- `generated/workspace-ux/table-inspecting-2560.png`
- `generated/workspace-ux/table-narrow-640.png`
- `generated/workspace-ux/gallery-inspecting-draft.png`
- `generated/workspace-ux/join-1280.png`
- `generated/workspace-ux/table-1280.png` (disconnected live route)
- `generated/workspace-ux/about-1280.png`

## UX tradeoffs

At 1280×720 the hand scrolls horizontally so every card remains a substantial text
button and the decision/inspector remain beside it. Large decisions and long
inspector characteristics scroll within their own columns. Below 1100px the
inspector moves below the board/decision columns; below 720px the decision comes
first and all regions remain reachable by scrolling and keyboard jump. This is a
usable narrow-window fallback, not a phone-specific gameplay design. Wide empty
opening views preserve battlefield room rather than inventing objects. Later
live decision families remain outside this build's admitted behavior.

Parent-owned `docs/game-client-plan.md` and `docs/game-client-ux.md` changes remain
outside this implementation checkpoint and should be committed by the parent.

## Parent review and opening preview

Independent visual review found no blocking clipping or unreachable controls in
the supplied renders. Independent code review found a gallery focus regression:
closing inspection after collapsing the captured hand focused a hidden button.
The hand now reopens before focus returns to the exact originating card, with a
Chromium/Firefox regression check. Additional independent Chromium checks at
1100×720, 1280×480, 720×360, 640×360 and 375×720 retained inspection, decision jump
and freedom from page-wide horizontal overflow.

`/fixtures/decisions#table` now provides an interactive opening preview on the
actual table, using the unchanged qualified captured request. Confirmation updates
only a local receipt; it does not advance a game. Inspection preserves the chosen
option, reload clears it, and the fixture makes no API requests or persistent
writes. The fragment uses the existing allowed page route and requires no host
or protocol change. A focused independent source review found no blocking issues.

After these changes, strict types, import boundaries, 114 client tests, production
build and **28 Chromium/Firefox browser checks** passed. Updated browser evidence
is under `generated/browser-results/`; Node output is retained in
`generated/workspace-ux/parent-tests.log`. The existing real-host, dense-board and
full-game omissions still apply. The owner should assess the two-column context
tradeoff and hand scrolling before further layout investment.
