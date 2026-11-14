# Frontend modernization — 2026-11-14

Implemented on `feat/frontend-modern-web`, based on current `main` at
`a42b76cd`, with subsequent main changes integrated through `ab9aa3c4`.
Three implementation agents covered product primitives/design,
product/deck-building features, and shared game presentation/client. The parent
owned routing, account bootstrap, card previews/cache, standalone consent and
integration. A separate review of the parent changes found three defects;
all were fixed and covered by the final browser checks.

## Scope and verified changes

The current product frontend lives in `packages/multiverse`, not the former
deck-builder web directory. The audit covered every product feature directory,
its generic UI/design and card presentation, `packages/game-ui`, the standalone
`packages/game-client`, server-rendered OAuth consent, and the SDK's browser
proof documents. It preserved package boundaries and exact source-backed card
facts. No dependencies, public URL/token formats, account migrations, gameplay
semantics or engine/schema sources changed.

### Product primitives and design

- Replaced the custom modal overlay, document Tab trap, counted body scroll lock
  and manual focus restoration with native `dialog.showModal()`. Browser
  inertness, top-layer rendering and invoker focus return own those behaviors.
  `cancel` still honors workflow busy guards and inspection-back callbacks.
  Safe confirmation autofocus, close controls, backdrop versus padding/selection
  gestures, sheets and drawers retain their intended behavior.
- Popovers use the native top layer, source association and per-instance CSS
  anchors where supported. Existing coordinate positioning and pointer/Escape
  dismissal remain feature-detected fallbacks. ResizeObserver handles changed
  content. `beforetoggle` keeps React state synchronized during quick
  Escape/reopen sequences. Nested popovers remain inside modal DOM boundaries.
  Labelled button anchors retain manual dismissal in the native top layer:
  native light-dismiss otherwise closes before the label forwards its click,
  reopening the controlled Select instead of closing it.
  Screen hotkeys defer while a native popover is open. Trigger and portal
  boundaries stop Escape propagation without preventing the browser's default
  dismissal, so a parent inspector/editor cannot close on the same keystroke.
- Menus use native focusgroup only where available, retaining a single keyboard
  navigation system and legacy ARIA otherwise. Pointer and keyboard entry,
  ArrowUp/Down, disabled items, typeahead and Tab have explicit contracts.
  Chromium's actual accessibility tree verifies inferred menu/item semantics;
  old Playwright selectors were updated without weakening production markup.
- MultiSelect now contains actual labelled checkboxes in a named group rather
  than mixing tabbable option buttons and search inside a listbox. Field honors
  explicit child IDs and preserves caller descriptions when adding hint IDs.
- Font tokens use rem equivalents with root font-size 100%; controls grow with
  content, meet minimum target sizes and use larger coarse-pointer entry text.
  Meaningful control borders have their own 3:1 contrast token. Forced Colors
  retains outlines and selected/current states. Standard scrollbar properties
  replace unconditional WebKit styling. Tabs scroll within their own row.
  Eyebrow/Section support real headings, and focused/hovered toast actions do
  not expire underneath the user.

### Deck-building and product features

- Fixed real IME Enter findings in slot/tag/copy editors, and paired existing
  composition guards with Safari's keyCode 229 safeguard across text handlers.
- Moved AddBar actions outside listbox options so Add, Maybe and Commander are
  actually reachable by Tab. Existing row/Enter/Shift+Enter actions remain.
- Converted account/handle, deck/fork/rename/engine, memory and budget flows to
  native forms/constraints where appropriate. Invalid submissions remain
  reachable, focus errors and avoid per-keystroke live announcements. Controls
  freeze during writes; pending deck/fork dialogs cannot dismiss accidentally.
- Fenced token/engine search and public-page responses by query/target identity.
  Old results disappear immediately during debounce. Shared comparison/export
  errors and unavailable public profiles have deliberate recovery.
- Deck sessions are internally keyed by deck ID. Full server composites reject
  backward membership/edit versions and mismatched decks; write epochs fence
  older reads and mounted generations fence old command completions. Each apply
  refreshes authoritative equal-version metadata without merging receipts.
  Retryable refresh failures keep the loaded editor; 403/404 still clear access.
- Added semantic section headings and unique power-form associations. Standalone
  legal/handle screens own main landmarks and window scrolling. Missing/error
  public routes now replace old tab titles while successful public titles retain
  owner information.

### Navigation, previews and browser infrastructure

- All signed-in screens, including the editor and phone composition, are lazy
  route imports. Game loading remains behind public preview/package boundaries.
- SPA page changes update titles and focus the rendered h1 once, including lazy
  and data-backed screens. Query selection, draft and remote updates do not
  steal focus. Native handling remains for server documents, downloads,
  external/modified links, and same-page fragments.
- Session bootstrap cancels obsolete reads and offers an accessible Retry after
  failure. Card-cache failures can evict only their own promise, so delayed old
  failures cannot discard newer requests or primed facts.
- Card previews support keyboard focus, pointer travel into their content and
  Escape. Native popovers work within modal dialogs; a positioned fallback
  remains without Popover. Genuine pointer movement is distinguished from
  synthetic entry after removing a top-layer preview, so a stationary pointer
  cannot replace the keyboard-focused card. IME Escape remains native.
  Pointer/keyboard activation cancels delayed previews so Escape in an actual
  card inspector preserves the existing inspection-back/rejection-draft order.
  Async inspection is fenced against later requests, detached invokers and
  navigation. On-demand pictures decode asynchronously when opened; ordinary
  lazy rail thumbnails no longer carry an additional low-priority penalty.
- Standalone OAuth consent releases the app's pinned-body styling, wraps long
  redirect URIs, exposes status/errors and restores the failed action's focus.
  Existing escaping, exact OAuth-query forwarding and server consent authority
  remain. SDK proof documents now have main landmarks.

### Shared game UI and standalone client

- Shared tables respond to their actual containing width using named container
  queries, preserve DOM reading order, scale with text preferences, and use
  per-instance ID/label relationships. Embedded previews can request h2 instead
  of creating a second product h1.
- Card collections are semantic lists. Generic buttons explicitly avoid form
  submission. Removed objects clear local inspection selection; explicit Close
  returns to the original trigger, with a decision fallback if detached.
- Decision confirmation remains reachable when invalid and focuses persistent
  error summaries. Validation is quiet during typing, exact quantities remain
  strings, and disabled ordering actions cannot invoke callbacks.
- Standalone routes now have meaningful titles and navigation focus. Control
  contrast, forced-color selection outlines, coarse-pointer targets and 200%
  text scaling have browser regressions. Reduced motion retains useful
  non-spatial color feedback while scrolling remains non-smooth.

## Guidance and compatibility

Used `modern-web-guidance@latest` (resolved 1.0.0), with searches specifying
skill version `2026_11_09-5bcc021b`. Applied guides: `accessibility`, `forms`,
`ime-safe-enter-submit`, `dialog`, `menu`, `viewport-aware-dropdowns`, `tabs`,
`color`, `customize-scrollbar-color-and-thickness`, `form-factor-adaptation`,
`motion`, `performance`, and `optimize-image-priority`.

The initial current-main legacy scan confirmed five real findings: two WebKit
scrollbar locations and three unguarded IME handlers. Final package-wide scan
finds zero. This scanner covers only 33 guides/35 deterministic rules, about
20% of the catalog; it is not a full accessibility or modernization certificate.
The manual audit found most of the consequential changes above.

Followed the skill's default browser policy: broadly available features may be
used directly; newer capabilities are detected and retain working fallbacks.
Missing Popover/focusgroup/anchor/closedby paths are exercised explicitly.
Native modal scroll containment degrades progressively without reinstating a
body overflow lock. The existing dark product design remains; no light theme,
experimental animation or new polyfill dependency was imposed. Retained exact
preview fixtures, native form/checkbox/radio semantics, nonmodal game inspection
and intentional Safari IME fallback. No matching async-request guide was found;
race fixes were justified by observed correctness failures, not unrelated guides.
The interactive-link `hovercard` guide was reviewed but its navigation/invoker
pattern does not fit generic non-link card rows; card previews instead preserve
the existing trigger contract with native Popover and a manual fallback.
Security review retained React-node rich text, escaping, existing CSP,
same-origin requests and transient credentials; it introduced no new HTML sink.

## Final validation

Pinned Node 26.11.1 via `sh scripts/engine-env.sh`; isolated package installs,
never a root install. Product integration uses immutable builds and disposable
actor-scoped fixture data; the final router/preview build is
`/tmp/mtg-modern-product-reviewed-dist`. Build outputs were never overwritten
while the final tests used them.

| Gate | Result |
| --- | --- |
| Multiverse `check` | Types, architecture boundaries and 37 tests passed |
| Multiverse `build:web` | Production build passed; existing public JS budget passed |
| Deck backend `check` | Types and 103 tests passed |
| game-ui `check` | Types, boundaries and 3 tests passed |
| game-client `check` | Qualified fixtures, types, boundaries, 116 tests and production build passed |
| game-client production browser suite | 42 passed in Chromium 158.0.8127.18 / Firefox 159.0; final label/focus follow-up 6 passed |
| SDK synthetic production browser proof | 13 checks each in Chromium 158.0.8127.18 / Firefox 159.0 |
| Product primitive regression | 14 contracts each in Chromium 151.0.7922.34 / Firefox 153.0, native and missing-feature paths |
| Product navigation/hover/consent regression | 6 contracts each in Chromium 151 / Firefox 153, including stationary-pointer/IME/public-error regressions |
| Product feature regression | 6 contracts passed in Chromium 151, including reversed command completions and keyboard AddBar actions |
| Existing product-shell regression | 7 contracts passed, including desktop/tablet/phone, private/public boundaries, delayed reads and unchanged deck data |
| Existing panel-inspection regression | Passed at 1280/1440/1920/2560px, with draft/DOM/scroll/focus preservation, resizing, distinct sheet Escape/Close behavior, unchanged deck data and axe checks |
| Existing Select workflow regression | Passed keyboard contract and all migrated controls at 1440/768px, including label toggling, writes, native popovers within an inspector, dialogs and playtester |
| Final legacy scan / whitespace check | Zero deterministic findings; `git diff --check` passed |

New reproducible product browser scripts live in
`packages/deck-builder/browser-checks/check-{modern-primitives,modern-navigation,feature-modernization}.mjs`.
They accept only disposable loopback origins. See the existing browser-checks
README for actor-scoped seed/server setup. The game suite includes
`tests/browser/modern-web.spec.ts`. Architecture/design manuals describe the
new native overlay, semantic-selection and typography contracts.

The previous checkout's ignored SDK generation did not match main's qualified
schema. Preserved it under `generated/frontend-modern-web/previous-sdk`, then
explicitly regenerated the current SDK from main-qualified producer exports:
protocol SHA256 `a7416856c2879f2d0a69531d1ccf12b4ea5f2cbd5da6811a0cc8f175650870b3`,
bootstrap SHA256 `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a`.
No historical fixture schema substitution or Cargo build occurred.

Detailed lane notes, independent review, successful logs and meaningful failed
attempts are retained in ignored `generated/frontend-modern-web/`. Early test
authoring failures and one overlapping-dist rewrite failure were resolved;
existing workflow regressions also exposed and verified fixes for label
activation, native Escape propagation, delayed previews after activation and
an inert duplicate deck heading being chosen for route focus. Final suites
use immutable/sequential builds. WebKit could not launch because
the host lacks required libraries; no packages were installed to change the
machine. No Safari/VoiceOver, real screen-reader/human usability, live OAuth
provider, live private game-host, engine matrix, retention, deployment or merge
acceptance is claimed. The standalone diagnostic harness still has its existing
large eager SDK bundle warning; the product preview remains free of SDK/session
runtime. No Core Web Vitals or performance acceptance follows from bundle gates.

## Independent PR review and focused regressions

Three independent reviewers examined exact implementation head `f68062e9`,
covering generic platform/primitives, deck features/consent, and shared game
presentation/client. Feature/consent review was clean. Two confirmed findings
were corrected: semantic list wrappers now preserve each hand card's intrinsic
width instead of shrinking beneath its button, and toast pause state reconciles
against current DOM focus/hover when a toast is replaced or dismissed. Removing
a focused action no longer leaves subsequent notifications permanently paused.

The browser regressions check card/wrapper containment, nonoverlapping hit
targets, local hand scrolling and focus at desktop, narrow embedded and wide
sizes; toast checks keep a keyboard action focused beyond expiry, remove it,
then verify a subsequent ordinary notification expires. The toast regression
passes in both native and missing-feature paths in Chromium 151 / Firefox 153
(16 checks each). Multiverse, game-ui and game-client package gates pass after
integrating main; the public bundle remains within budget at 109060 gzip bytes.
The full game-client browser suite passes all 44 cases in Chromium 158 / Firefox
159 after the fixes. A Firefox automation crash in locator element adoption was
isolated with a minimal static reproduction; the new geometry regression keeps
measurements and DOM focus inside page evaluation, retaining all assertions.
Review reports, meaningful failed browser attempts and focused validation logs
remain under ignored `generated/frontend-modern-web/`.
