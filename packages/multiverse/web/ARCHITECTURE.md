# Frontend architecture

The React application lives in `packages/multiverse`; browser paths below are
relative to `web/src`. [DESIGN.md](DESIGN.md) owns appearance and interaction and
[the README](../README.md) owns setup and checks.

## Application boundaries

The [root package table](../../../AGENTS.md) assigns ownership; the Rust host/core
is authoritative for supported-catalog admission, rules, visibility, durable
execution and outcomes. The product uses the backend's public `./host` factory and
type-only `./contracts`; game code uses public deck-builder `./application` ports
for the existing Actor, reentrant SQL reservation, account authorization and owned
deck capture. No cross-package private imports, second identity system or storage
migration. Preserve DECKBUILDER settings, data defaults, credentials, MCP and shared
URL formats; the deck library keeps its independent database and release behavior.

Live, preview and solo tables share public `@multiverse/game-ui` display, which is
router-free and effect-free over permitted view models ([manual](../../game-ui/AGENTS.md));
solo gestures and edits stay in the product. The legacy solo playtester is not a
game-client design reference.

## Layers

| Layer | Owns | May import |
| --- | --- | --- |
| `design` | Tokens and document styles | — |
| `ui` | Generic controls | Design CSS variables |
| `mtg` | Card presentation | `ui`, `domain`, public game-ui |
| `domain` | Pure transformations, no React/DOM | Type-only wire types from `api/client.ts` |
| `api` | Typed HTTP, queries and change stream | API modules; type-only public backend contracts |
| `features/x` | A screen or shared workflow | `ui`, `mtg`, `domain`, `api`, app hooks; other features only through `index.ts` |
| `app` | Routes, shells, providers and lifetime | All layers |

`test/web-architecture.test.ts` enforces the boundaries. Feature barrels only
re-export; components never fetch or type in card facts. One component and CSS
Module per file; context only for distant consumers. A new page needs its feature,
typed route, `SignedIn` composition, server `src/page-paths.ts` allowlist entry and
a sidebar/phone decision. New backend fields flow through public contracts and the
API client.

## Data flow and recovery

- `useQuery`/`useLive` read server truth over principal-scoped streams; SSE
  reconnect or an unknown change invalidates all queries, others only matching ones.
- `DeckStore` owns the authoritative full composite and mutations replace it; the
  browser never merges receipts or calculates membership, checks, deltas or prices.
  Sessions are keyed by deck ID, reject older membership/edit versions, fence
  pre-command reads and old mounted generations, and refresh equal-version metadata.
- `DeckUi` keeps drafts, selection and review position through inspection,
  filtering, remote refresh and resizing; remote updates announce without taking focus.
- `ApiError` keeps status and server text: hosted 401 returns to sign-in, 429 shows
  retry time, missing/forbidden decks clear access, network/5xx refresh failures
  keep a loaded editor with Retry, and render failures offer Reload.
- Domain commands use stable retry keys; refresh replayed composites before
  applying. Late reads, commands, searches and public-page responses cannot replace
  another deck/query/generation. Card-cache failures evict only their own promise;
  live invalidation and price-source changes clear cached facts.
- Activity pages by 100-row `before_id`; Memories by `before_id` plus
  `before_revised_at`, filtering before pagination, with server-supplied scope
  choices and budgets. Reset cursors on filter change and follow `has_more` for
  all memories an open deck needs.

## Routing and identity

`app/routes.ts` owns routing. Use normal links; the router intercepts only eligible
same-origin navigation. Hash-era bookmarks rewrite once; malformed paths become
missing pages. The server serves only its SPA allowlist; unreadable shared pages 404.

Home `/` shows account-scoped decks. Play setup's optional owned deck carries real
revision/commander data; stale selections stay explicit, never silently switching.
`/play/preview` shows the captured opening sample without the selected deck, a game
record or commands. Never infer recent views, active games or results from IDs,
revisions or dates.

`SessionProvider` sits above private live/inbox/focus providers. Local mode uses
implicit user 1 with no legal footer/routes; hosted mode resolves cookie sessions.
No private query or SSE starts before authentication; legal/shared pages stay
independently accessible, sign-out unmounts private providers, and the native
consent document keeps signed-query server authority.

Shared pages receive `CardProviders`, not private providers. `ReadOnlyDeck` takes
`fromPublic` or owner-phone models with `DeckListProvider readOnly`, never a private
composite. Attribution uses structured `author_via`; pricing follows spec §4.4.
Settings keeps once-shown secrets only in mounted state, excludes them from export
and requires typed confirmation for account deletion. Admin visibility never
replaces server authorization. Changing currency resets the draft store to Lowest.

## Responsive composition and navigation

`PHONE_MAX_WIDTH=719`, `DRAWER_BREAKPOINT=960` and `PANEL_BREAKPOINT=1100` are the
approved CSS boundaries. `PANEL_QUERY` selects navigation: **all 720–1099px widths
use tablet navigation**; 960 bounds other layouts only. Phone uses `PhoneApp`;
desktop panes begin at 1100px.

Phone supports Home, Play setup, the library, read-only owned decks and Inbox
(including proposal decisions) inside the private providers, so proposals arrive
live; other screens explain the larger-screen requirement. Public pages keep their
own responsive composition.

Below 1100px shells call `useWindowScroll`; desktop pins the document and scrolls
panes. Verify real wheel/touch input, not scripted `scrollTo`. AppShell keeps
`<main>` mounted; lazy pages suspend inside it. DeckBody portals one panel into the
side pane or a tablet sheet opened only by card/header/tab/Deck details actions.

The server-persisted `sidebar_collapsed` preference applies only at 1100px+,
defaults expanded and keeps its saved value on failure; tablet drawers stay
expanded. Over eight decks enables name/commander search only in expanded desktop
navigation; rail/drawer list all decks. Active-deck reveal scrolls only navigation.

The agent footer reads `/api/agents` (latest authenticated request, reads included);
first-run `has_agent_activity` stays write-based (spec §11.3). Focus context uses
server-confirmed tab tokens, visible heartbeat and compare-and-clear cleanup.

## Loading, media and focus

Every signed-in screen, phone composition, shared page and game surface loads
lazily; authentication, shared actions, inspection and public comparison/export
load on request. The preview imports no SDK/session runtime. The README's
public-bundle check enforces the initial-visit JS budget.

Navigation updates titles and focuses the rendered h1 once, after lazy/data
content; drafts, query changes and remote refresh never refocus it. Images use
stored Scryfall URLs through shared lookup with explicit placeholders; inspection
images load when opened. Only mana symbols construct static CDN URLs, from the
cited Card Symbols contract.

Hover previews support pointer travel, focus and Escape; activation cancels delayed
previews, and detached invokers, navigation or a later request invalidate async
inspection. DESIGN owns [art](DESIGN.md#art) and [overlay focus/IME](DESIGN.md#overlays-and-controls) rules.

## Deck workflows

- Grouping, primary/secondary purposes, tags and maybes stay independent. Filters
  hide nonmatches but keep selection, details and server totals; read-only decks
  have no find filter. `copy_limit` comes from the server; the UI never parses
  Oracle text for eligibility.
- Review keeps cursor/drafts and shows actual changes/reasons; accept-all follows
  spec §8.2 and backend validation owns all bases. Inbox is pending state;
  Activity is historical writes.
- `features/import` owns new/existing import. New preview diffs against an empty
  deck without writing; apply creates then imports. A failed apply keeps its ID
  for retry and closing opens that deck. Undo responses replace the store.
- `useDeckHeaderScroll` follows native list offset, caches dimensions on resize and
  stops style writes after the fold, compensating translation with an equal spacer.
  Never intercept wheel events; panel scrolling never folds the header. Find/review
  reveals and filter restoration preserve outer and list positions.

## Game launch

The first mode is one human against three automated opponents. Cookie sign-in
grants no native seat authority, and the captured preview stays separate from
admission, selected decks and live records.

`@multiverse/game/server` mounts `/api/games`; the backend owns session resolution,
Host/Origin checks, rate limits and errors. Hosted play paces product and native
fetches from the shared Actor limit. Native I/O runs outside SQL reservations and
authority is rechecked afterward.

Deck-builder's application port checks the human Actor/account and deck owner and
captures exact Oracle IDs/roles/quantities at the requested revision, excluding
maybes. The coordinator hashes and durably freezes that capture, build/catalog
identity and retry key before native provisioning; recovery uses the immutable
capture. Migrations, capture constraints and account-deletion triggers stay in the
existing database; deletion atomically keeps only minimal native stop identities
until acknowledgment.

The game application owns native admission, worker/bot lifetime, private journals
and the human-only proxy at `/api/games/:id/native/{client-config.json,v1/...}`
(exact command bytes, same-origin seat 0, pre/post-I/O owner checks, generation
fences); see its [manual](../../game/AGENTS.md) and
[native lifecycle](../../../docs/game-engine-server.md#product-native-lifecycle).
Multiverse keeps thin route wrappers over game-owned hosted components, which load
the SDK lazily. `/play/games/:id` holds an Actor-authorized human seat File only in
component memory; late grants are fenced and unmount/reload discards it. Launch
requires an exact approved native identity and deck admission.
[Client contracts](../../../docs/game-client.md) own journaling, Web Locks and
delivery scope; [setup](../README.md#native-admission-configuration) owns native
configuration.

## Checking a change

The [README](../README.md#verification) owns commands. [Browser regressions](../../deck-builder/browser-checks/README.md)
cover normal/empty/foreign accounts, delayed deck switches, both table consumers,
focus/draft continuity and all three navigation sizes.
