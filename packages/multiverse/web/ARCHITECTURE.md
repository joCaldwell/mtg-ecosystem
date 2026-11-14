# Frontend architecture

This is the top-level Multiverse browser application. Unless stated otherwise,
browser paths below are relative to `packages/multiverse/web/src`; commands run
from `packages/multiverse`. Backend code belongs to `packages/deck-builder/src`
and is reached only through public package contracts, not relative source imports.

The UI is a React single-page app with no state library, no router library
and no CSS framework. What keeps it changeable is a small number of layers
with one-way dependencies, and one rule about where each kind of code goes.

## Layers

`Page` supplies a shared centered frame through the inherited `--page-width`:
`md` is reading width, `lg` grows to `--page-wide` (1440 px), and `full` is
uncapped. `PageHeader` uses the same black canvas, centered frame and gutter as its
page content, with a real h1, aligned actions and optional compact state metadata.
It has no separate background band or explanatory subtitle. A feature must not add a second,
left-aligned width cap inside that frame. Prose can retain its reading measure.

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
| `api` | API modules and public `@mtg-ecosystem/deck-builder/contracts`, type-only |
| `features/x` | `ui`, `mtg`, `domain`, `api`, `app` hooks, and another feature **only through its `index.ts`** |
| `app` | everything |

`test/web-architecture.test.ts` enforces the table. If a change needs an
import that the table forbids, the code is in the wrong layer.

## Where does it go?

- **It is a number or a colour.** `design/tokens.css`. Never a literal in a component.
- **It could be used by an app that has nothing to do with Magic.** `ui/`.
- **It draws part of a Magic card.** `mtg/`.
- **It turns server data into something to display, and has no JSX.** `domain/`. Test only consequential domain invariants.
- **It talks to the server.** `api/client.ts`. Components never call `fetch`.
- **It is one screen, or a workflow shared by two screens.** `features/<name>/`.

## Data flow

The server is the source of truth and the client never merges.

1. **Reads.** `useQuery(() => api.something(), [deps], scope)` fetches, and refetches
   when that user’s event stream names a matching resource/deck (`useLive`). That
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

**A new product page:**

1. Add its component in `features/<name>/`, built from existing public primitives.
2. Add the route to `app/routes.ts` (a path, built only by `href()`), the page
   to the switch in `app/SignedIn.tsx`, and the path to the product's
   `packages/multiverse/src/page-paths.ts` allowlist (D16: anything else is 404).
3. Add a sidebar entry in `app/Sidebar.tsx` if it needs one, and decide what
   the phone shell shows for it (`features/phone/PhoneApp.tsx`; by default,
   "Open on a larger screen").
4. Any data shaping goes in `domain/` with a test.

**A new primitive:** add it to `ui/`, export it from `ui/index.ts`, and add
an example to `features/styleguide/StyleguidePage.tsx` (`/design`). If it
needs a new colour or size, that is a token first.

**A new backend field:** change its backend type and expose the necessary type
through `packages/deck-builder/src/contracts.ts`. The product API client imports
that public contract type-only; `npm run build` identifies affected consumers.

## Checking a change

`npm run check` runs product/server-browser types and product tests;
`npm run build:web` creates the production bundle. Run the backend's separate
`check` for capability/host changes. `npm run dev` serves the UI with hot reload;
seed a disposable populated deck through the backend's `seed:demo` capability,
not against owner data. The [product README](../README.md) records setup.
`/design` shows every primitive in every state.

## Routing (v3 phase 4a, spec §11.1)

Path routes: `/` (Home), `/decks` (library), `/play`, `/play/preview`, `/deck/:id`, `/deck/:id/review`,
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
Graph tags (`packages/deck-builder/src/preview.ts`) and answers 404 when the requester cannot read
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
Scryfall API image URL in the client. Mana symbols are the one constructed
image URL: `mtg/manaSymbols.symbolSvgUrl` names the static SVG on
`svgs.scryfall.io` that Scryfall's Card Symbols API documents for each symbol
(pattern cited there, checked against real `/symbology` entries in
`test/fixtures/scryfall-symbology.json`). The hosted CSP allows that host
in `img-src`. `ManaSymbol` keeps its plain fallback disc until the image
loads, and keeps it if the image fails.

Settings enqueues refresh jobs and displays last/next run and result for
refresh and backup. The server's SSE has a 25-second comment heartbeat;
job completion emits a normal invalidation. The shared `domain/familyOrder`
keeps all fork descendants in the roster; `ui/Pill` owns selectable filter
pills. Proposal history uses Section/Box/Stack/Chip and human-readable dates
and edit titles, without approval controls. Memories resets a deleted scope from its response’s owned-deck metadata, and
resets pagination when a filter changes.


## Account identity

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

`api/live.tsx` consumes `{ kind, resources?, deck_ids?, global_resources? }`
events on a per-user SSE stream. Reconnect invalidates every query; ordinary
events refresh matching resource/deck subscriptions. Another principal's mutations never
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


## Price sources

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
principal, and agents cannot set them. PublicDeckState prices use viewer choice when authenticated and USD/lowest for
anonymous readers. `priceResolver(db, viewer | null)` is that entry point;
it never reads owner preferences for anonymous viewers. Both proposal cost
compositions show store/date beside their estimates. Settings saves source and
budgets in one transaction.

## First run, tablets and loading

`PreferencesProvider` reads /api/preferences for the private shell. It
supplies `has_agent_activity`, both copy voices through `useVoice`, and the
account’s desktop sidebar collapse state. Saving that preference is UI-only
below HTTP. Sidebar status uses /api/agents and includes authenticated reads. The
first-run voice remains write-based; token creation and read-only requests
do not enable it. /api/activity/latest remains a write-history endpoint.

Owner decision (2026-10-27, §11.3): Advanced and its account preference are
removed. History owns current and entry revisions; card decisions show their
revision, while memory and power assessment metadata uses native titles.
Neither desktop nor phone deck headers show revisions. CardControls keeps
secondary slots directly under the primary field, ungated.
LogRow shows review status, and the inbox's proposal history shows the count
of decisions no agent has read yet. CardHistory's owner-scoped response adds ownership
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
rotation. Only a card, a header stat, a tab request or the tablet bar’s "Deck details"
opens the sheet; a review runs in the list with its keys, and R opens the
sheet only for the reason field, closing it again after the reason is sent or
cancelled. The list scrolls the window. At 1100+ the existing panes scroll
independently. Dialog's body scroll lock is counted, so nested dialogs that
close together release the page; Dialog's focus trap listens natively,
because the portalled panel's React events do not bubble through the sheet.
Escape that starts in a field reaches the field first: an inline editor in a
sheet cancels, and the next Escape closes the sheet. A specifically labelled Explainer keeps the
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


## Credited art

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
and rail art is not downloaded. The owner's editor uses `editor`: native-size
right-aligned crops and a dim blurred wash behind the title. `useDeckHeaderScroll`
uses the list's native scroll offset for the entire header-to-list gesture.
The first hero-to-compact pixels scroll the outer page and expand the workspace;
a matching translation inside the list compensates for those pixels. Its range
stays constant, including on short decks. No wheel event is cancelled or smoothed
in JavaScript, so native wheel animation and precise trackpad momentum both
survive the hand-off in either direction. Dimensions are cached on resize, and
past the fold the hook makes no style writes. The headers crossfade with scroll
position and only the active copy is accessible. Panel-only scrolling does not
fold it. Explicit find/review/card reveals expose the full viewport before
measuring their target; native keyboard/scrollbar scrolling uses the continuous
offset. Filter restore remembers both list and outer page positions.
Tablet/phone/public shells retain
`useWindowScroll` and let their headers leave naturally with the document. Sign-in uses that same hook and a labelled main. The public/phone
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
Nonzero change counts link to their sections inside the changes pane and
focus the section's native summary. Each section starts open, can collapse
after review, and reopens when its count is chosen; warnings and Apply stay
in place while navigating the change list. Against an empty deck (no cuts,
unchanged, quantity or role changes) the summary omits zero counts and the
add and maybe sections start closed behind their counts; while every section
is closed the dialog sizes to its content. Skipped lines and
the parser's after-import checks share one warning callout, named by its
headings; the section title is a real heading under the dialog title.

## Sidebar navigation

Expanded desktop navigation gives deck names their full row width, with
shared-state icons and the waiting badge beside the title. Theorycrafted and
Archived retain a readable second line after repeated blind review (2026-10-28);
collapsed links keep the lifecycle in their accessible names and tooltips.
Collections with more than eight decks expose a native name/commander
filter over the existing actor-scoped summaries. Search never remounts the deck
editor or writes data. Escape clears it; empty results provide a clear action.
Collapsed rail and tablet drawer always retain the full deck list, even while an
expanded-desktop filter is set. Route/rail changes reveal the active deck only
inside the navigation scroller, without shifting the deck list or page.

All decks leads the section navigation, followed by Inbox, Memories, Activity,
Profile and Settings. Following review, the owner decided on 2026-10-27 to
restore the standalone ActivityPage and its own page title. ProfilePage has
no tabs; ActivityPage owns its Page and scroll container around ActivityPanel.
The inbox badge uses a solid violet fill with black ink in expanded and
collapsed layouts. Expanded deck rows end in the same badge with that deck's
waiting count (`domain/inbox` `waitingByDeck`, from the shared inbox); the rail
keeps a violet dot on the thumbnail instead.
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

## Current deck editor and secondary-page contracts

- Feature barrels and typed routes are the cross-feature API. Deck core state,
  proposals and server-computed counts come from composite reads; local UI state
  stores drafts/selection only. No component fetches or independently computes
  backend truth. Use resource-family/deck subscriptions through `useLive`/`useQuery`.
- Main grouping, secondary purposes, tags and maybes are separate axes. Filtering
  hides nonmatches without changing totals/projections/layout semantics. Preserve
  selection/open card and restore unfiltered scroll when clearing; readonly decks
  have no find filter. Server `copy_limit`, never Oracle parsing, drives quantity UI.
- Review uses the current proposal/decision cursor. Show actual deltas/reasons,
  fold unchanged document lines, preserve drafts across cursor movement and explain
  stale touches using actual changes since, not revision-number prose. Public
  readers get public allowlisted state and generic agent attribution.
- Inbox shows proposal state, while historical Activity describes write type.
  Settings retains token creation/revocation/setup consequences and per-user price
  source/budget units. Do not expose internal design references as product controls.
- Focus reporting uses server-confirmed context with compare-and-clear tab tokens,
  visible heartbeats and pagehide cleanup. Failed reports retain confirmed context.
  Domain writes use stable retry keys; replayed composites refresh before applying.
- Responsive shell widths, text/hand scrolling, keyboard focus, reduced motion,
  visible empty/loading/error states and source-coverage disclosures follow
  [DESIGN.md](DESIGN.md). Keep local drawer/inspector navigation deliberate;
  remote updates announce without stealing focus. Game styles never leak to editor.

## Unified application and game surface (2026-11-14)

The existing account/session and Actor-scoped API form the product boundary.
Home is `/`; `/decks` remains the full deck library. Game setup is `/play`, with
an optional `deck` query identifying an owned deck. `/play/preview` is a compact
game frame around the qualified opening sample; it carries optional deck return
context, never a fabricated game ID. Shared/public routes retain their existing
anonymous behavior. Phone/tablet navigation recognizes the added destinations.

`features/home` and `features/play` consume existing typed deck reads. Only
`app/routes.ts` constructs their URLs. Setup selection must not retain stale
results for another requested deck or fall back from an unreadable deck. The
current summary does not contain last-viewed time or active games, so neither is
invented by the UI.

The game route lazy-loads the public `@multiverse/game-client/preview` entrypoint.
Its source-qualified fixture adapter and local drafts belong to that client;
`@multiverse/game-ui` owns the shared router-free display over permitted models.
Its styles and tokens are scoped to the game surface and React comes from the
host. No iframe, second SessionProvider, SDK effects, private app imports or
copied table implementation belong in this adapter. Normal editor/public routes
must retain their lazy loading and public bundle budget.

This scaffold has no native create-match service. Real Start game stays
unavailable with a concrete explanation; Preview table sends no gameplay request
or durable record. See [the coordinator and launch gates](../../../docs/multiverse-app.md)
before introducing account-to-seat authorization, immutable game decks, native
admission, automated consumers, active-game records or results.
