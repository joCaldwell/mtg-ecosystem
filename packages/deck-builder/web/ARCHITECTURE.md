# Frontend architecture

The UI is a React single-page app with no state library, no router library
and no CSS framework. What keeps it changeable is a small number of layers
with one-way dependencies, and one rule about where each kind of code goes.

## Layers

```
web/src/
  design/    tokens.css, base.css          the only place raw colours and sizes exist
  ui/        Button, Chip, Tabs, Dialog…   generic primitives; know nothing about Magic or this app
  mtg/       ManaCost, CardRef, RichText…  how a Magic card is shown; no API, no app state
  domain/    grouping, review, marks…      pure functions over wire types; no React, no DOM
  api/       client.ts, live.tsx, useQuery the typed HTTP client and the change stream
  app/       App, routes, shell, providers wiring: routing, sidebar, app-wide providers, the breakpoint
  features/  deck/, inbox/, public/, phone/… one folder per screen or shared workflow
```

Imports go **down this list only**:

| Layer | May import from |
| --- | --- |
| `ui` | `design` (CSS variables only) |
| `mtg` | `ui`, `domain` |
| `domain` | wire types from `api/client.ts`, **type-only** |
| `api` | server types, type-only |
| `features/x` | `ui`, `mtg`, `domain`, `api`, `app` hooks, and another feature **only through its `index.ts`** |
| `app` | everything |

`test/web-architecture.test.ts` enforces the table. If a change needs an
import that the table forbids, the code is in the wrong layer.

## Where does it go?

- **It is a number or a colour.** `design/tokens.css`. Never a literal in a component.
- **It could be used by an app that has nothing to do with Magic.** `ui/`.
- **It draws part of a Magic card.** `mtg/`.
- **It turns server data into something to display, and has no JSX.** `domain/`, with a test in `test/web/`.
- **It talks to the server.** `api/client.ts`. Components never call `fetch`.
- **It is one screen, or a workflow shared by two screens.** `features/<name>/`.

## Data flow

The server is the source of truth and the client never merges.

1. **Reads.** `useQuery(() => api.something(), [deps])` fetches, and refetches
   whenever that user’s event stream names a change (`useLive`). That
   is what makes an agent's proposal appear without a reload.
2. **The open deck.** `features/deck/DeckStore.tsx` holds the `DeckState`
   composite. Every mutation endpoint returns a fresh composite under
   `state`; `run(() => api.x(...))` applies it, so no component ever patches
   state by hand.
3. **Screen state** that the server does not know about (which tab is open,
   which card is selected, whether a review is in progress) lives in a
   reducer next to the screen: `features/deck/DeckUi.tsx`.
4. **Computed numbers come from the server.** Counts, slot deltas, the curve,
   pips and the projected count after pending proposals are all in
   `state.computed`. The client arranges them; it does not recount them.

## Conventions

- **One component per file**, named for the component. Its styles sit beside
  it as `Name.module.css`. No global class names outside `design/`.
- **CSS modules use tokens only.** No hex colours, no pixel font sizes. If a
  value is missing, add a token.
- **`style={{…}}` is for values computed at runtime** (a meter's width, a
  popover's position). Everything else is a class.
- **Props over context.** Context is for things many distant components need:
  the deck store, the screen state, toasts, the card lookup.
- **A feature exports through `index.ts`.** Other features import from there
  and nowhere else inside it.
- **Keyboard shortcuts** are declared with `useHotkeys` at the screen that
  owns them, never on `document` directly.
- **Card facts are never typed in.** Names, costs and text come from the API.
- Comments say why. The spec section (`§8.2`) is the usual why.

## Adding things

**A new page** (say, a playtester):

1. `features/playtest/PlaytestPage.tsx`, built from `ui/` and `mtg/` pieces.
2. Add the route to `app/routes.ts` (a path, built only by `href()`), the page
   to the switch in `app/App.tsx`, and the path to the server's app-route
   list in `src/app.ts` `serveStatic` (D16: anything else is 404).
3. Add a sidebar entry in `app/Sidebar.tsx` if it needs one, and decide what
   the phone shell shows for it (`features/phone/PhoneApp.tsx`; by default,
   "Open on a larger screen").
4. Any data shaping goes in `domain/` with a test.

**A new primitive:** add it to `ui/`, export it from `ui/index.ts`, and add
an example to `features/styleguide/StyleguidePage.tsx` (`/design`). If it
needs a new colour or size, that is a token first.

**A new server field:** change the server type; `api/client.ts` imports it
type-only, so the typecheck (`npm run build`) points at every place the UI
has to follow.

## Checking a change

`npm run build` typechecks the server and the UI. `npm test` runs the domain
tests and the layering test. `npm run dev` serves the UI with hot reload;
seed the demo deck first (`npm run seed:demo`) so every panel has content.
`/design` shows every primitive in every state.

## Routing (v3 phase 4a, spec §11.1)

Path routes: `/`, `/decks`, `/deck/:id`, `/deck/:id/review`,
`/deck/:id/playtest`, `/inbox`, `/memories`, `/profile`, `/activity`,
`/settings`, `/design`, `/signin`, `/privacy`, `/terms`, `/d/:slug`,
`/u/:handle`. Following review, the owner decided on 2026-10-27 to restore
`/activity` as a standalone ActivityPage with its own title and sidebar entry
after Memories. `/profile` renders a single ProfilePage without tabs. Their
named routes and `href()` paths stay independent; the server already
allow-lists both paths (D16). `app/routes.ts` owns them: `parseRoute`, `href()` (the only
place a URL is built), `navigate()`, and `useRoute()`, which also turns a
plain same-origin `<a href>` click into in-place navigation, so components
keep writing ordinary links. A hash-era bookmark (`/#/deck/5`) is rewritten
once on load. The server answers only those paths with `index.html`; for
`/d/:slug` and `/u/:handle` it first injects the title, description and Open
Graph tags (`src/preview.ts`) and answers 404 when the requester cannot read
what the path names. Vite's dev server falls back to `index.html` for every
path, so link previews exist only when the API server serves the build.

## Shared pages and read-only decks (`features/public`, spec §18)

`/d/:slug` and `/u/:handle` render outside the signed-in app:
`SessionProvider open` lets them through signed out, and they get only
`CardProviders` (lookup and hover), never `LiveProvider`, `InboxProvider` or
`FocusProvider`, which need a session. They read `api.publicDeck`,
`api.publicExport` and `api.publicProfile`, which the server answers without
an account.

`features/deck/ReadOnlyDeck.tsx` is the one read-only composition, exported
from the deck feature. It takes a `ReadOnlyDeckModel` from
`domain/readOnlyDeck.ts` (`fromPublic` for a shared deck, `fromOwner` for the
owner's own deck on a phone) and draws the existing `DeckList`, `GroupBlock`,
`CardRow`, `ChecksPanel`, read-only `BriefField`s and `EngineCard`s inside
read-only providers: `DeckListProvider readOnly` and its own
`DeckUiProvider`. List components read `useDeckList()` (a `DeckListState`:
cards, slots, tags, computed, proposals, log), not the whole `DeckState`, so
they work from either source; the live `DeckProvider` provides the same
context. Read-only rows cannot be selected, and a tap opens `CardSheet`
instead of the side panel.

## The phone view (`features/phone`, spec §11.2, D17)

`app/breakpoints.ts` holds the one breakpoint, `PHONE_MAX_WIDTH = 719`, and
`useIsPhone()`. CSS repeats it as `@media (max-width: 719px)`, and
`web-architecture.test.ts` fails on any other width query outside the
playtester. Below it, `App` renders `PhoneApp` instead of `AppShell`: a top
bar, `PhoneDecks`, `PhoneDeck` (the live `DeckProvider` around
`ReadOnlyDeck`, with `PhoneDecisions` drawing every decision and Accept all,
each with its optional reason, through the same `useReview` as desktop) and
`PhoneInbox` (proposals open their deck; document edits rule in place with
`DocumentEditCard`). Every other route renders `LargerScreen`. The phone
shell sits inside `AppProviders`, so the event stream refetches as on
desktop and an agent's new proposal appears without a reload. 4b inherits
the breakpoint, `ReadOnlyDeck` and the router.

The desktop app scrolls inside its panes, so `design/base.css` pins the body
with `overflow: hidden`. The phone shell and `PublicShell` scroll the window
instead, and call `useWindowScroll()` (`app/windowScroll.ts`), which marks
`<html data-scroll="window">` while they are mounted so base.css releases the
body. Without it, touch and wheel scrolling do nothing (a script's
`scrollTo` still works, so automated checks must scroll by wheel or touch).

## Recovery and background work

`api/client.ts` checks HTTP status and content type before decoding JSON.
`ApiError` retains both the status and server error text. `DeckStore` treats
404 as missing, 403 as forbidden, and network/5xx failures as retryable load
errors; `DeckGate` renders each state and offers Retry. A top-level
`app/ErrorBoundary` catches render failures and offers Reload.

`useReview.tsx` renders the existing `ConfirmDialog` for older proposals.
Accept-all sends one atomic batch at the revision visible when clicked,
acknowledging only proposals already stale then. All original baselines are
validated before the server applies any changes.

Images are ingested card facts (`image_small`, `image_normal`,
`image_art_crop`); `mtg/cardImage.ts` selects a stored URL. `CardImage`
uses a supplied card's stored image URL directly, or resolves through
`CardLookup` (shared with hover cards) when no URL is known. Every image uses
lazy loading. Missing image data retains the placeholder. Never construct a
Scryfall API image URL in the client.

Settings enqueues refresh jobs and displays last/next run and result for
refresh and backup. The server's SSE has a 25-second comment heartbeat;
job completion emits a normal invalidation. The shared `domain/familyOrder`
keeps all fork descendants in the roster; `ui/Pill` owns selectable filter
pills. Proposal history uses Section/Box/Stack/Chip and human-readable dates
and edit titles, without approval controls. Memories resets a deleted scope from its response’s owned-deck metadata, and
resets pagination when a filter changes.


## Identity foundation (v3 Phase 1)

The browser calls the same actor-scoped capabilities as MCP. Local mode builds
implicit user 1 without a login. Hosted APIs use cookie sessions; the
unauthenticated session bootstrap is `/api/session`. `SessionProvider` sits
above `LiveProvider` and gates private providers/screens until signed in, so
SSE and private data queries never start on the sign-in page. `useSession()`
exposes mode, configured providers, the current account and a reload hook.
The first-run handle picker appears only on a provider-handle collision.
Legal pages and shared pages bypass sign-in; `/signin` is an auth boundary. The server serves `/consent` as plain
HTML with an external script; it submits Better Auth's signed oauth_query.
Local mode never renders sign-in, regardless of the path.

`api/live.tsx` consumes `{ kind, deck_id? }` events on a per-user SSE stream.
It initially invalidates all queries for that stream; the typed payload is
available for later selective refetch. Another principal's mutations never
reach the stream. Attribution uses structured user/via/label fields and
`author_display` (`handle via label`), while compact MCP author text stays
stable. UI/agent distinctions must use `author_via`, not an agent's name.

The Settings connection card manages labelled per-user tokens: create, show
once, copy, list with last use, and revoke through ConfirmDialog. Secrets
stay only in component state until dismissed or unmounted, never browser
storage. Client snippets use a placeholder outside that one-time display.
Reachability copy follows server mode. Revoke actions align at the row edge;
labels and metadata stay grouped and long labels wrap. Existing generic
primitives and design tokens supply the form and dialogs.

## Hosted request recovery and account settings

`request()` explicitly uses same-origin cookies and emits an `api-error`
event on HTTP failure. `SessionProvider` maps 401 to `/signin` only in hosted
mode, 403 to the server's readable-action error toast, and 429 to a slowdown
toast including Retry-After seconds. It retains ApiError status and text;
404/revision/5xx behavior is unchanged. EventSource automatically sends the
same-origin cookie. Sign-out navigates away and unmounts all private providers.

Settings retains once-shown personal tokens in component state, adds hosted
OAuth connector steps and last-seen grant usage (Settings refreshes every
30 seconds while open), and has Account export/delete
with typed-handle confirmation. The Admin card appears only for is_admin;
the server rechecks admin/UI permissions. The final admin's disabled deletion
button explains why. The legal footer names WotC's Fan Content policy and
Scryfall daily estimates with as-of date; no new price source is introduced.


Hosted `/api/session` also bootstraps `legal` (public operator name/contact)
and `invites_required`. LegalPage renders the runtime identity as React text;
no source placeholders or personal operator data are committed. Local mode
renders no legal routes/footer. Open signup requests a blank registration
ticket. Provider/state failures and invite registration failures have distinct
recovery messages. HandlePicker is allowed only on first-run collisions.

Settings lists consent-backed OAuth apps separately from personal tokens,
including unused grants, and confirms Disconnect through the UI-only
account capability. The server deletes consent/refresh tokens and remembers
an issuance cutoff, so re-consent cannot revive old credentials. Public
hashed assets cache immutably; hosted HTML, API and consent remain no-store.


## Price sources (v3 Phase 1b)

Settings' PriceSourceCard saves currency and store together through the typed
client; changing currency resets the draft store to Lowest. The server applies
the principal's choice everywhere. Wire prices use `cents`, `currency`, `store`,
`as_of`, `source`; domain/price formats $ or € and propagates currency into group
totals. Deck headers and card panels label the store/date. Proposal projections
consume server-computed pending.projected_cost and cost_delta, including
unpriced counts; components never calculate proposal price deltas.

The shared card lookup cache clears on live invalidation; a card panel resolved
outside the deck also refetches. Source changes in another tab therefore cannot
leave a cached USD quote attached to an EUR view. Source preferences are per
principal, and agents cannot set them. PublicDeckState.price integration belongs
to Phase 4a if it lands later: viewer choice when authenticated, USD/lowest for
anonymous readers. `priceResolver(db, viewer | null)` is that entry point;
it never reads owner preferences for anonymous viewers. Both proposal cost
compositions show store/date beside their estimates. Settings saves source and
budgets in one transaction.

## Phase 4b: first run, tablets and loading

`PreferencesProvider` reads /api/settings once for the private shell. It
supplies `has_agent_activity`, both copy voices through `useVoice`, and the
account’s desktop sidebar collapse state. Saving that preference is UI-only
below HTTP. Sidebar
presence is absent until an agent write; /api/activity/latest reads one
activity row. Creating a token or reading with it does not count.

Owner decision (2026-10-27, §11.3): Advanced and its account preference are
removed. History owns current and entry revisions; card decisions show their
revision, while memory and power assessment metadata uses native titles.
Neither desktop nor phone deck headers show revisions. CardControls keeps
secondary slots directly under the primary field, compact and ungated.
LogRow shows review status, and InboxPage keeps the unreviewed count visible
alongside the queue. CardHistory's owner-scoped response adds ownership
evidence, rendered only when nonempty with basis, quantities and attribution.
Settings alone shows memory limits, usage and omitted counts for the global
scope and owned decks, computed through loadScope. No new migration. The
disclosure primitive is removed.

The tablet frame runs from 720 through 1099 px; `TabletScroll`
(`app/TabletShell.tsx`) **calls useWindowScroll()** while it applies. The
sidebar lives in a drawer throughout 720–1099 px. The top bar names the
current page and links directly to Inbox with its waiting count. Inbox selection
uses `/inbox?entry=proposal-<id>` or `edit-<id>`, with Back to inbox above
the title; desktop retains the split view. `AppShell` keeps every child at
a fixed position (navigation,
`<main>`, then `TabletScroll`), so crossing a breakpoint, as an iPad rotating
does, re-styles the frame without remounting the page. Lazy pages suspend
inside `<main>`, so a first visit keeps the sidebar. `DeckBody` likewise keeps
the list in place and renders the panel once, through a portal into one host
element that it attaches to the side pane or to a requested Dialog sheet below
`PANEL_BREAKPOINT` (1100). A Brief draft or a half-typed reason survives a
rotation. Only a card, a header stat, a tab request or "Open deck details"
opens the sheet; a review runs in the list with its keys, and R opens the
sheet only for the reason field, closing it again after the reason is sent or
cancelled. The list scrolls the window. At 1100+ the existing panes scroll
independently. Dialog's body scroll lock is counted, so nested dialogs that
close together release the page; Dialog's focus trap listens natively,
because the portalled panel's React events do not bubble through the sheet.
Escape that starts in a field reaches the field first: an inline editor in a
sheet cancels, and the next Escape closes the sheet. An Explainer keeps the
trigger's place in the Tab order and closes when focus leaves it. Up to
1099 px, editor header, toolbar, banner and panel controls are `--tap` tall
and card rows grow to 44 px, regardless of pointer type or import history.
Desktop rows remain 24 px. `PHONE_MAX_WIDTH` and
`useIsPhone()` still select 4a's read-only phone view. Tables contain their own
horizontal overflow. Every shell provides the labelled `main-content` skip
link target. QA must use actual wheel/touch input, never script scrolling.

`App` lazily loads the signed-in routes and public pages. `SignedIn` lazily
loads playtest, settings and the style guide. Authentication screens, shared
action dialogs and card inspection load when requested. `CardProviders` is a
small public provider independent of private inbox/focus/preferences. The
reader API and HTTP transport are separate from editor API methods; wire types
still flow through client.ts. DeckListStore is the shared read-only context;
DeckStore owns editing and re-exports the same hooks. useDeckView receives
memories as data instead of fetching in the read-only composition. The deck
page asks `api.deckMemories` for only the memories that can mark its rows: the
deck's own and the global ones, following `has_more` past the 100-row cap.

Barrel index.ts files are re-exports only; an architecture test enforces this
so Vite may tree-shake their unused exports without executing the editor.
React has its own manual chunk. `node scripts/check-public-bundle.mjs` after
`vite build` sums unique gzip JS chunks in the initial public-deck visit,
including React and the entry, and fails at 110,000 bytes (raised from 90,000 by the owner on 2026-10-27). Deferred interaction
chunks are excluded, and CSS is reported by Vite separately.

Components never call fetch. **All HTTP calls live in api/**, including
style-guide examples. web-architecture.test.ts enforces that boundary alongside
layer imports, token colors, the exact three width queries and the window-scroll
contract. Activity pages request 100 rows at an offset; Memories requests one
100-row response containing scope choices, budget metadata and has_more, with
scope/status/basis/search applied before pagination. It no longer fetches a
second unfiltered memory list or a deck roster just to construct its filters.


## Credited art (v3 Phase 4c)

`src/art.ts` admits only stored Scryfall art-crop URLs paired with an artist.
Actor-scoped deck summaries add `commander_art`; `/api/session` adds one
shared-card `backdrop` resolved from the curated exact-name set. No new table,
tenant lookup, external HTTP call or client Scryfall lookup is involved.
`domain/art.ts` repeats the admission rule for the client (`creditedArt`,
parsed URL) and words the credit; `mtg/DeckArt` draws strips, banners,
showcases and backdrops, and `mtg/DeckThumbnail` the rail's decorative art.
Both consume facts, never API state. DESIGN.md and /design own their tokens,
variants and fallbacks. Features choose placement and loading: roster cards
pass `load` from `ui/useNearView`, which observes the nearest container that
actually scrolls (or the window) with a 200 px margin, so off-screen roster
and rail art is not downloaded. `app/useArtCollapse` only observes the
desktop list's `data-deck-list` scroll events, reads its tokens once, and
drops a fold below 1100 px; its rule is `domain/artFold.ts`, kept apart so
the public chunk does not carry it. Tablet/phone/public shells retain
`useWindowScroll` and let their short banners leave naturally with the
document. Sign-in uses that same hook and a labelled main. The public/phone
reader uses `BriefText`, without the editable
brief wrapper's state or loader in its initial dependency graph.

## Direct deck import

`features/import` owns the shared new/existing-deck dialog. All decks and
first run open it directly; the deck menu has separate Import and Export
actions. New-deck preview calls `/api/decks/import/preview`, which uses the
same capability-layer diff against an empty snapshot without writing a deck.
Apply calls `createDeck` and then the existing import endpoint. A failed
apply retains the created id for retry; closing opens that empty deck.
Existing-deck results and undo responses replace the deck store as usual.

The preview unmounts the source form behind Edit list. Totals and warnings
stay above the independently scrolling changes, with cuts first; Apply and
the optional History note remain in Dialog's footer. Formats is a native
disclosure in the source step. Dialog's optional `bodyClassName` lets this
workflow size its scrolling region without changing other dialogs.

## Sidebar navigation

All decks leads the section navigation, followed by Inbox, Memories, Activity,
Profile and Settings. Following review, the owner decided on 2026-10-27 to
restore the standalone ActivityPage and its own page title. ProfilePage has
no tabs; ActivityPage owns its Page and scroll container around ActivityPanel.
The inbox badge uses a solid violet fill with black ink in expanded and
collapsed layouts. Unreviewed decisions appear in InboxPage’s padded header
subtitle.
The route allow-list and public-path decoding share compact parsing to keep
the initial public bundle below its existing budget.

At 1100+ px, AppShell applies the account's `sidebar_collapsed` preference
from /api/settings to its grid and Sidebar. The icon rail uses
`--sidebar-rail-w`; links retain names, titles, the inbox count and deck
thumbnails/dots. The collapse button names its action and exposes
`aria-expanded`. Saving is UI-only in `src/preferences.ts`, uses an existing
user_settings key (default expanded), and sends no data invalidation.
PreferencesProvider reports a failed save in a toast and retains the saved
state. It does not use browser storage.

The 720–1099 px drawer stays expanded inside, with no collapse control.
Its top bar shows the current page and Inbox count. Returning to 1100+ restores the
account's saved choice. AppShell keeps `<main>` at the same child position;
collapse and breakpoint changes never replace it. TabletScroll still calls
useWindowScroll; the phone shell below 720 is unchanged.

## Deck editor scanning (2026-10-27)

Tablet headers combine lifecycle help, counts and actions; Open deck details
lives among those actions and sends the same explicit details request. The
panel host and list remain mounted across 1100 px. Lifecycle, proposal and
brief explanations live inside the existing Learn more popovers. Toolbar
controls share a raised surface; tablet add/search fields and tag/group/sort
controls wrap into two rows before the proposal banner.

Deck filtering preserves all rows, groups and totals. Nonmatches use readable
muted text; matches get a blue edge/outline and stronger names. The count sits
beside search. Query/tag changes scroll the first rendered match into view
without moving focus, expanding disclosures or changing the scroll owner.
Phone decks use native fragment links between the title, card list and pending
proposals. The read-only composition accepts optional list navigation; editing
and phone decision permissions are unchanged.

Deck-editor UX regressions can be checked against the disposable `ux-eval/serve.ts`
server with `scripts/check-deck-editor.mjs`; set `PLAYWRIGHT_MODULE` to an installed
Playwright entry point and `AXE_MODULE` to axe-core's `axe.min.js`. It checks all
five viewport widths, real wheel scrolling, proposal handoffs, add/undo, column
reflow, post-import tablet targets, and panel identity across 1100 px. It requires
the seeded evaluation database and never runs against live data.

The card list uses native balanced columns with a 250 px target minimum and
unfragmented row/header tracks. The browser recalculates breaks as content sizes
change. Review arrival reveals the chosen decision's first card, then focuses
its optional reason in the panel (the requested sheet on tablets). Panel-only
scrolling and resize observation keep asynchronously loaded context from moving
the controls away or undoing the list reveal.

Secondary-page UX (2026-10-27): inbox ProposalDecision calls the existing ruleProposal capability with the revision read at the click and captures stale confirmation before submitting. It applies the returned inbox without client merging. PhoneInbox preserves the proposal parameter; PhoneDecisions keeps selected units first and discloses bulk decisions. The shared mtg/DeckCard is the roster presentation used by All decks and public profiles; publicProfile supplies only public commanders’ credited art facts alongside their names.

Review refinement (2026-10-27): CardPanel reserves a readable field width beside
shrinkable art. AddBar retains search results after maybe-list adds, while its
Undo uses removeCard; ordinary membership Undo still uses its log entry.
PhoneDecisions falls back to the first pending proposal after its selected one
is resolved. DeckArt folds its decorative crop with transform and opacity in
a fixed reserved box, without resizing the header or either desktop pane.

Active tablet filters keep only their query/tag summary, match count and Clear
action below navigation while the toolbar scrolls away. Matching-row reveal
reserves that row's height. Pending impact headings in Inbox and Review name
the full proposal count; individual decisions retain their existing scope.
Settings' existing selector separates Claude Code token and OAuth setup, with
optional tool permissions in a native disclosure. Token and grant management
remain visible regardless of the selected method. Phone larger-screen notices
use page titles and a separate back action; sign-in consent uses inline links
with natural sentence wrapping.
