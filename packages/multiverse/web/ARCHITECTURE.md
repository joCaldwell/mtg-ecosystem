# Frontend architecture

The React application lives in `packages/multiverse`. Browser paths below are
relative to `web/src`; commands run from this package. [DESIGN.md](DESIGN.md)
owns visual and interaction rules; [the README](../README.md) owns setup.

## Application boundaries

| Component | Owns |
| --- | --- |
| Multiverse | Account/session UX, navigation, browser features, Home, deck editor, Play setup, frontend build and process composition |
| deck-builder | Existing authentication, Actor authorization, deck services, SQLite, API, CLI and MCP |
| game-ui | Router-free, effect-free shared display over permitted local view models, scoped CSS and host React |
| game | Browser/session adapters, qualified preview, shared coordination DTOs, durable launch/recovery, native supervision and human-seat proxy |
| Rust host/core | Authoritative supported-catalog admission, rules, visibility, durable execution and outcomes |

The product uses the backend's public `./host` factory and type-only `./contracts`.
No cross-package private imports, second identity system or storage migration.
Preserve DECKBUILDER settings, data defaults, credentials, MCP and shared URL formats.
The deck library retains its independent source database and release behavior.

Both table consumers use public `@multiverse/game-ui`; the product loads the
qualified `@multiverse/game/preview` adapter. The host supplies React.
No iframe, copied table, compiler/IR, trusted snapshot or bot credential enters
that integration. Display models are local views, not gameplay wire authority.
The game application consumes public deck-builder `./application` ports for the
existing Actor, reentrant SQL reservation, human/account authorization and owned
deck capture. Game code never imports private deck/auth modules. The legacy solo
playtester is not a game-client design reference.

## Layers

| Layer | Owns | May import |
| --- | --- | --- |
| `design` | Tokens and document styles | — |
| `ui` | Generic controls | Design CSS variables |
| `mtg` | Card presentation | `ui`, `domain` |
| `domain` | Pure transformations, no React/DOM | Type-only wire types from `api/client.ts` |
| `api` | Typed HTTP, queries and change stream | API modules; type-only public backend contracts |
| `features/x` | A screen or shared workflow | `ui`, `mtg`, `domain`, `api`, app hooks; other features only through `index.ts` |
| `app` | Routes, shells, providers and lifetime | All layers |

`test/web-architecture.test.ts` enforces the boundaries. Feature barrels contain
re-exports only for tree shaking. Components never fetch or type in card facts.
Use one component and adjacent CSS Module per file; raw design values belong in
tokens. Inline styles are for runtime values. Context is for distant shared
consumers; otherwise use props. Screen shortcuts use `useHotkeys`.

To add a page, add its feature, typed route, `SignedIn` composition and server
`src/page-paths.ts` allowlist entry, then decide sidebar and phone access. Add
pure transformations in `domain`. New primitives need a public UI export and
`/design` example. New backend fields flow through public contracts and the API
client; the product build identifies affected consumers.

## Data flow and recovery

- `useQuery`/`useLive` read server truth and subscribe by resource family/deck.
  SSE reconnect or an unknown legacy change invalidates all queries; normal
  changes invalidate only matching subscriptions. Streams are principal-scoped.
- `DeckStore` owns the authoritative full composite. Mutations replace it; the
  browser never merges receipts or calculates membership, checks, deltas or prices.
  Sessions are keyed by deck ID, reject older membership/edit versions, fence
  pre-command reads and old mounted generations, and refresh equal-version metadata.
- `DeckUi` owns drafts, selection and review position. Keep it through inspection,
  filtering, remote refresh and responsive layout changes. Remote updates announce
  without taking focus.
- `ApiError` keeps status and server text. Hosted 401 returns to sign-in; 403
  shows the readable-action error, 429 includes retry time. Missing/forbidden decks
  clear access; network/5xx refresh failures retain an already loaded editor and
  offer Retry. Render failures offer Reload. Bootstrap cancels obsolete reads.
- Domain commands use stable retry keys. Refresh replayed composites before
  applying them. Late reads, commands, searches and public-page responses cannot
  replace another deck/query/generation. Card-cache failures evict only the promise
  they own; live invalidation and price-source changes clear cached facts.
- Activity uses 100-row `before_id` keyset pages; Memories uses `before_id` plus
  `before_revised_at`, filtering before pagination. Memory responses supply scope
  choices and budgets. Reset cursor on filter changes and follow `has_more` for
  all memories needed by an open deck.

## Routing and identity

`app/routes.ts` owns `parseRoute`, `href`, `navigate` and `useRoute`. Use normal
links and let the router intercept eligible same-origin navigation. Downloads,
server documents, external/modified links and same-page fragments retain native
handling. Hash-era bookmarks rewrite once; malformed paths become missing pages.
The product server serves only its explicit SPA allowlist. Shared pages receive
backend-generated link-preview metadata and a 404 when unreadable.

Home `/` shows actual account-scoped decks and links to `/decks` and `/play`.
Setup's optional owned deck carries real revision/commander data; unreadable or
stale selections remain explicit, never silently switch decks. `/play/preview`
shows the captured opening sample, with optional return context. It does not use
the selected deck, create a game record or send gameplay commands. Do not infer
recent views, active games or results from IDs, revisions or creation dates.

`SessionProvider` sits above private live/inbox/focus providers. Local mode uses
implicit user 1; hosted mode resolves existing cookie sessions. No private query
or SSE starts before authentication. Legal/shared pages remain independently
accessible. Sign-out unmounts private providers. Runtime session data supplies
legal identity and configured providers; local mode has no legal footer/routes.
The native consent document retains signed-query server authority.

Shared pages receive `CardProviders`, not private providers. `ReadOnlyDeck` takes
`fromPublic` or owner-phone models and uses `DeckListProvider readOnly`, never a
full private composite. Attribution uses structured `author_via` and display
fields; public readers receive generic agent attribution. Public pricing uses
the viewer's preference, or anonymous USD/lowest, never owner preferences.

Settings keeps once-shown secrets only in mounted component state, uses placeholder
snippets afterwards, and confirms revoke/disconnect/deletion through backend
capabilities. Export excludes secrets; account deletion needs typed confirmation.
Admin visibility never replaces server authorization. Currency/store save together;
changing currency resets the draft store to Lowest. Display server-computed cost
and pending deltas with source/date, including unpriced counts.

## Responsive composition and navigation

`PHONE_MAX_WIDTH=719`, `DRAWER_BREAKPOINT=960` and `PANEL_BREAKPOINT=1100` are the
approved CSS boundaries. The current navigation drawer is selected by
`PANEL_QUERY`: **all 720–1099px widths use tablet navigation**, including 960–1099.
The 960 constant still bounds other responsive layouts; it does not select the
current navigation shell. Phone uses `PhoneApp`; desktop panes begin at 1100px.

Phone supports Home, Play setup, the library, read-only owned decks and Inbox,
including proposal decisions. Game preview and desktop-only features show a
larger-screen explanation; missing addresses remain missing. Public pages retain
their separate responsive composition. The phone shell stays inside private
providers so proposals still arrive live.

Tablet, phone, shared, sign-in and standalone long-document shells call
`useWindowScroll`; desktop pins the document and scrolls panes. Verify actual
wheel/touch input, not just scripted `scrollTo`. AppShell keeps `<main>` mounted
at a stable position; lazy pages suspend inside it. DeckBody renders one panel
through a portal into the side pane or requested tablet sheet. Drafts and review
position survive resizing. Card/header/tab/Deck details actions open that sheet;
list review alone does not. The tablet review reason shortcut closes its temporary
sheet after submitting or cancelling.

Section navigation is Home, My decks, Play, Inbox, Memories, Activity, Profile,
Settings. Activity is its own page; Profile has no tabs. The server-persisted
`sidebar_collapsed` preference applies only at 1100px+, defaults expanded and
retains its saved value on failure. Tablet drawers stay expanded; widening
restores the desktop preference. The rail retains names, lifecycle, badge and
thumbnail accessible labels. More than eight decks enable name/commander search
only in expanded desktop navigation; rail/drawer keep all decks. Active-deck
reveal scrolls only the navigation container.

The agent footer uses `/api/agents`: latest authenticated request among current
credentials, including reads; recent contact is not a connection count. First-run
`has_agent_activity` remains write-based and ignores sample writes/token creation.
Focus context uses server-confirmed tab tokens, visible heartbeat and
compare-and-clear cleanup; failed reports retain confirmed context.

## Loading, media and focus

Every signed-in screen, phone composition, shared page and game surface loads
lazily. Authentication, shared actions, inspection and public comparison/export
load when requested. The product preview imports no SDK/session runtime.
After a build, run `node scripts/check-public-bundle.mjs`: it sums unique gzip JS
in the initial public visit, including React, and requires less than 110,000 bytes.
The build command itself and this explicit budget check are separate.

Navigation updates titles and focuses the rendered h1 once, waiting for lazy/data
content. Drafts, query changes and remote refresh do not refocus it. Public errors
replace old titles; successful public titles retain owner/deck information. Long
account/legal documents own a main landmark and release window scrolling.

Images use stored Scryfall URLs through shared lookup, with explicit placeholders.
Gallery/rail art waits until near its actual scroll container; inspection images
load/decode when opened. Only mana symbols construct static CDN URLs from the
cited Card Symbols contract; keep their spoken-label and failed-image fallback.
Art eligibility, layout, credit and contrast are specified in [DESIGN](DESIGN.md#art).

Card hover previews support pointer travel, focus and Escape. Keyboard intent
wins over synthetic mouse entry after top-layer removal. Activation cancels delayed
previews; detached invokers, navigation or a later request invalidate async
inspection. IME Enter/Escape keeps composition semantics, including Safari keyCode
229. Native dialog/popover focus and fallback rules are in
[DESIGN](DESIGN.md#overlays-and-controls).

## Deck workflows

- Grouping, primary/secondary purposes, tags and maybes stay independent. Name/tag
  filters hide nonmatches while retaining selection, details and server totals;
  clearing restores scroll. Read-only decks have no find filter. `copy_limit`
  comes from the server; UI never parses Oracle text for eligibility.
- Review keeps its cursor/drafts and displays actual changes/reasons. Fold unchanged
  document lines. Accept-all sends one atomic batch at the displayed revision,
  acknowledging only already-stale proposals; backend validation owns all bases.
  Inbox describes pending state; Activity describes historical writes.
- `features/import` owns new/existing import. New preview diffs against an empty
  deck without writing; apply creates the deck then imports. A failed apply keeps
  its ID for retry, and closing opens that deck. Undo responses replace the store.
  Presentation of warnings/counts/change sections belongs in DESIGN.
- `src/art.ts` admits exact stored credited art facts; `domain/art.ts` performs the
  client admission check. Deck summaries and session bootstrap supply art without
  tenant leakage or browser source lookups. `DeckArt`/`DeckThumbnail` only display.
- `useDeckHeaderScroll` follows native list offset, caches dimensions on resize and
  stops style writes after the fold. Equal translation/spacer compensation preserves
  scroll range and row geometry. Never intercept wheel events; panel scrolling
  must not fold the header. Find/review reveals and filter restoration preserve
  both outer and list positions.

## Game launch

The first game mode is one human against three automated opponents. Cookie sign-in
grants no native seat authority. Captured preview stays separate from admission,
selected library decks and live records.

`@multiverse/game/server` supplies the authenticated `/api/games` extension to the
existing public backend host. The host retains session resolution, Host/Origin
checks, rate limits and response error handling. It dispatches the extension
outside the handler-wide SQL reservation; the coordinator reserves SQL only for
queries/transactions and rechecks authority after native I/O.

Deck-builder's public application port checks the existing human Actor/account and
deck owner and captures exact Oracle IDs/roles/quantities at the requested revision,
excluding maybes. The game coordinator hashes and durably freezes that capture,
build/catalog identity and retry key before native provisioning. Recovery uses the
immutable capture. Historical schema migrations, immutable-capture constraints and
account deletion triggers remain in the existing database. Deletion atomically
retains only minimal native stop identities until acknowledgment; no second
identity system or destructive migration is involved.

The game application owns native admission, worker/bot lifetime, private journals
and human-only bootstrap/proxy at `/api/games/:id/native/{client-config.json,v1/...}`.
It preserves exact UTF-8 command bytes, same-origin seat 0 authority, pre/post-I/O
owner/identity checks and generation fences. Secrets and Node/native code stay in
`server/`; the browser consumes only public session/preview exports and type-only
`/contracts`. [Native lifecycle](../../../docs/game-engine-server.md#product-native-lifecycle)
owns durable native semantics and qualification requirements.

Game-owned hosted components receive stable product HTTP/navigation/UI ports;
Multiverse keeps thin route wrappers. Separate launch/saved/session entries retain
lazy SDK loading. The lazy `/play/games/:id` obtains an Actor-authorized human
seat File held only in component memory. Game/attempt keys, abort and cleanup
fence late grants; unmount/reload discards that File. Preview remains separately
lazy and router/session/SDK-runtime free. Phone retains its larger-screen fallback.
Browser qualification remains false and approved identities empty until actual
product proof; this ownership extraction qualifies no full game and fixes no
supervision fault. [Client contracts](../../../docs/game-client.md) own journaling,
Web Locks and delivery scope; [setup](../README.md) owns native configuration.

## Checking a change

Run product `check` and `build:web`, plus the explicit public-bundle check above.
Backend capability/host changes use its separate gate; shared display/session
changes use their owning packages. Browser regressions use
[disposable fixtures](../../deck-builder/browser-checks/README.md): direct loads,
normal/empty/foreign accounts, delayed deck switches, both table consumers,
keyboard/draft/focus continuity, desktop/tablet/phone navigation and unchanged
stored decks. Real host/full-game acceptance remains separately source-qualified
in [client evidence](../../../docs/game-client-evidence.md).
