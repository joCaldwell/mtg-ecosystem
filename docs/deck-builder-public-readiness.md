# Deck-builder: going public — readiness review (2026-10-25)

Five parallel audits of `packages/deck-builder` (working tree as of 2026-10-25,
including ~46 uncommitted files for tag-filter inversion, filtered export, stale
proposal acknowledgement and proposal history). `npm run check` passes: typecheck
clean, 429 tests green. All paths below are relative to `packages/deck-builder/`
unless stated.

Headline: **"public" is a multi-tenant rewrite before it is a deploy.** The spec
(§2, §10.1) and the code assume one owner at every level. The sections below are
the raw findings; the chat summary that accompanied this file is the synthesis.

---

## 1. Data model and capability layer

### Tables and tenancy (`src/db.ts:13-387`, 30 tables/virtual tables/triggers)

Global card data, shared by all users: `cards`, `card_names`, `cards_fts`,
`card_prices`, and the `meta` keys `schema_version`, `last_ingest`, `prices_as_of`.

Per-user but reachable through a deck (`deck_id -> decks.user_id` would suffice):
`slots`, `tags`, `deck_cards`, `deck_card_secondary_slots`, `deck_card_tags`,
`briefs`, `engines`, `engine_pieces`, `power_targets`, `power_assessments`,
`proposals`, `proposal_items`, `log_cards`, `playtest_sessions`,
`playtest_observations`.

Per-user with **no natural owner column** — need `user_id` directly:

| Table | Line | Why |
| --- | --- | --- |
| `decks` | 91 | The root. `name UNIQUE COLLATE NOCASE` is global; must become `UNIQUE(user_id, name)`. |
| `profile` | 209 | `CHECK (id = 1)` singleton; `documents.ts:36,50` use `WHERE id = 1`. |
| `memories` | 329 | `scope='global'` rows have `deck_id NULL`. `memories_fts` must JOIN back for tenant filtering. |
| `document_edits` | 217 | `deck_id NULL` means a profile edit. |
| `log` | 281 | `deck_id NULL` for profile rulings; `reviewed_at` review queue is global (`log.ts:263-287`). |
| `activity` | 377 | No FK, no owner. |
| `ownership_evidence` | 16 | Keyed only on `oracle_id`; `sourcing.ts:562` reads with no deck/user. |
| `meta` `setting:*` | 550-561 | `focus`, `memory_budget_*` are user preferences beside global keys. |

`playtest_sessions` PK is `deck_id` alone (363): with sharing it needs
`(deck_id, user_id)` or two people fight via `checkVersion` 409s.

### Actor model (`src/actor.ts`)

`Actor = { kind: "owner" } | { kind: "agent"; name }`. `OWNER` is a constant
with no identity. The UI hard-codes `OWNER` in every route (`src/app.ts:101-102`).
MCP builds `agent(ctx.authInfo?.clientId)` (`src/mcp/http.ts:87`) where
`clientId` is the self-asserted URL label. The single bearer token contributes
nothing to identity. `authorOf` collapses identity to the string `"owner"` or a
label, which is what every `author` column stores.

`requireOwner` has 19 call sites (`documents.ts:45,382`;
`playtest/service.ts:121,137,258,288,336`; `deck/membership.ts:312`;
`deck/power.ts:185,195`; `deck/service.ts:144,164`; `deck/interop.ts:509`;
`deck/brief.ts:69,94,132,139`; `deck/proposals.ts:307,355`). `actor.kind` is
also read directly to change semantics: `membership.ts:250-252`,
`service.ts:247-251`, `log.ts:38-42` (`awaitsReview`), `memory.ts:217-220,243,
256-262,288`, `power.ts:812,817`, `proposals.ts:404`, `documents.ts:197,328`,
`activity.ts:27`.

Proposed shape:

```ts
type Actor = { user_id: number; via: "ui" | "agent"; label: string; token_id?: number };
type DeckRole = "owner" | "collaborator" | "viewer" | null;
```

`requireOwner(actor, what)` becomes `requireDeckOwner(db, actor, deckId, what)`
plus `requireSelf(actor)` for profile edits; agent-vs-human distinctions key on
`actor.via`. `requireDeck` (`service.ts:221`, 29 sites) becomes
`requireReadableDeck(db, actor, id)`; `requireEditableDeck` (`service.ts:239`,
36 sites) adds a role check. 59 exported `(db, ...)` functions take no actor
today vs 41 that do. Every `author TEXT` column needs an `author_user_id`.

### Singletons

Profile document; settings in `meta`; memory budgets; player stats
(`stats.ts:53-56` iterates all decks); focus (`focus.ts`, really per browser
session); activity feed; review queue; the agent token (`token.ts`, one file);
inbox (`app.ts:235-239`); the live stream (`app.ts:107-112`, one listener set,
one version counter, every write pings everyone); bulk refresh
(`POST /api/cards/refresh`, any UI caller).

### Migrations

No framework. `openDb` (`db.ts:392-493`) runs `CREATE IF NOT EXISTS` then a
hand-written ladder keyed on the old version string and `PRAGMA table_info`
probes, all in one transaction (good: a failed migration rolls back). SQLite
`ALTER TABLE ADD COLUMN` cannot add NOT NULL without default or change a
UNIQUE/PK, so `decks`, `profile`, `playtest_sessions` need the
create-copy-drop-rename pattern already used at 410-420 and 428-438. Backfill is
trivial (everything is user 1). Good moment to replace the ladder with an
ordered `(version, fn)` list. 1-2 days with tests.

### Concurrency and scale

`node:sqlite` `DatabaseSync` is synchronous: every statement blocks the event
loop; every user serialises behind every other user's DB work. No
`PRAGMA busy_timeout` (default 0): the documented `npm run refresh` while the
server runs can throw `SQLITE_BUSY` on either side. Long transactions:
`ingestCards` (`ingest.ts:757-796`, whole oracle file `JSON.parse`d, ~1 GB heap
peak, tens of seconds frozen), `ingestPrices` (`prices.ts:719-737`, DELETE +
~34k inserts). `POST /api/cards/refresh` has no mutex and races on
`CACHE_FILE + ".tmp"` (`scryfall.ts:118-131`). N+1s that matter at scale:
`deckState` calls `getDeck` three times; `listDecks` correlated subqueries +
one commander query per deck; `toViews` one query per memory; `loadScope`
loads up to 1000 memories; `playerStats` loads every Paper deck.

Verdict: SQLite+WAL is viable for a modest hosted product if refresh moves out
of the request path (separate process, `busy_timeout` set) and reads stay cheap.
Beyond modest, the sync driver is the ceiling.

### Cross-tenant queries (would leak if `user_id` were added naively)

Roster `service.ts:197-210` (used by `deckId()` resolution `tools.ts:113-115`,
`start_session`, `playerStats`); `myDecksFor` `tools.ts:140-151`;
`listMemories` without `deckId` `memory.ts:152-193`; `memoriesForCards`
`memory.ts:199-216`; `findSimilar` FTS over all memories `memory.ts:248-267`;
inbox/proposal history `app.ts:235-243`, `proposals.ts:206-226`,
`documents.ts:276-297`; `listLog` `log.ts:150-170`; `markReviewed({through})`
`log.ts:270-276` (a cross-deck **write**); `cardHistory` lineage via
`parent_id` `log.ts:184-231` and `setDeckParent` `service.ts:157-179` (any
deck as parent); activity `activity.ts:36-43`; ownership evidence
`sourcing.ts:562`; focus; playtest by deck id; search `search/index.ts:44`;
export/import/compare `interop.ts:83,301`, `fork.ts:485`. Every MCP tool
resolves a deck through `requireDeck` with no ownership check: with per-user
tokens this is the central authorization gap.

### Effort (engineer-days, someone who knows the code; session layer excluded)

| Work | Days | Notes |
| --- | --- | --- |
| (a) users table, tenant key, Actor redesign, threading through ~80 service fns, 65 require sites, ~15 unfiltered queries, 30 tools, ~60 routes; tests | 7-12 | Mostly mechanical once signatures change. Design: global memory scope, `owner_touched` semantics, focus. |
| (b) per-user agent tokens (hashed table, lookup → user + label, Settings CRUD) | 1-2 | Mechanical. |
| (c) viewer role (shared read-only) | 3-5 | Half design: what a viewer sees in `deckState`, `cardHistory`, memories, playtest, SSE, `include_my_decks`; forks must not copy owner memories (`fork.ts:151` → `memory.ts:364`). |
| (d) collaborator role | 4-8 | Mostly product decisions: Paper edits direct or proposed, who rules, review queue, undo/lifecycle, `expected_revision` on direct edits. |

### Bugs and fragile spots

1. **Refresh can brick itself on an oracle_id merge.** `ingest.ts:780` deletes
   absent cards; `deck_cards`, `engine_pieces`, `proposal_items`, `memory_cards`
   reference `cards(oracle_id)` with no `ON DELETE` (`db.ts:135,203,265,352`)
   and `foreign_keys = ON`. First retired id held by a deck fails every refresh.
2. No `busy_timeout`.
3. Agent identity is self-asserted; `clean()` (`mcp/http.ts:63`) permits the
   label `owner`; same-label agents can amend each other's proposals
   (`proposals.ts:404`, `documents.ts:328`).
4. `markReviewed({through})` marks every deck's entries.
5. `findSimilar` takes top 12 FTS hits before scope filter: duplicate check is
   starvable (`memory.ts:225-232,248-267`).
6. `reviseMemory` `confirm` (`memory.ts:347-354`) has no `assertMayChange`;
   agent can promote an owner-touched memory's basis. Decide deliberately.
7. WIP: proposals/document edits predating v10 have NULL `base_revision`
   (`db.ts:466-472` no backfill) so each needs `acknowledge_stale` to accept.
8. `createDeck` takes no actor (`service.ts:91`): agent deck creation leaves no
   activity record; UNIQUE violations detected by error-message substring
   (`service.ts:106,122,401,427,525`).
9. Refresh in the request path: no mutex, sync, any UI caller, `.tmp` race.
10. `setDeckParent` accepts any deck; lineage pulls another deck's rulings.
11. Dangling references by design (no FK): `activity.deck_id`,
    `ownership_evidence.source_deck_id`, `memories.source_log_id`,
    `log.proposal_id`, `proposal_items.slot_id`.
12. `mcp/http.ts:84` `seen` set unbounded.

---

## 2. Server, MCP and security

### Route inventory (`src/app.ts`)

Every `route()` runs as `OWNER` (`:101-102`); the only gate on `/api/*`,
`/api/events` and static files is `sameMachine()` (`:86-96`, applied `:474`).
`/mcp*` is matched before that gate (`:471-472`). No route reads a session.

Notable routes: `GET /api/settings` (`:365`) **returns the agent token in
plaintext** (`:359`) and an absolute server path (`:361`);
`POST /api/settings/rotate-token` (`:370`); `POST /api/cards/refresh` (`:376`)
triggers the full Scryfall download with `force: true` from the Settings page
(`web/src/features/settings/CardDataCard.tsx:12-17`);
`POST /api/decks/:id/items/rule` (`:252`) does not reconcile the URL deck with
the items' deck (IDOR once multi-user) and does not check `item_ids` is an
array; `POST /api/decks` does `String(body.name)` and crashes on non-object
body; `Number(body.quantity)` unvalidated at `:187`; `limit=abc` → `NaN` into
`LIMIT ?` (`:278,345,408`).

Static serving: no traversal found (`new URL` resolves `..`, `normalize` +
`startsWith(WEB_DIST)` at `:439-440`). Every unknown GET returns `index.html`
with 200 (`:441`), which will break `/.well-known/oauth-protected-resource`
discovery. No `Cache-Control`/ETag; MIME map lacks fonts/png; `readFileSync`
per request. No `child_process` in `src/`. Fixed-path writes only:
`token.ts:28-30` and `scryfall.ts:119-134` (cache at **repo root**, outside
`DATA_DIR`).

### Auth today

UI: `sameMachine()` requires Host in `{127.0.0.1, localhost, [::1], ::1}` and,
if present, Origin hostname in the same set. A DNS-rebinding/CSRF check, not
authentication. MCP: SDK `hostHeaderValidationResponse(localhostAllowedHostnames())`
(`mcp/http.ts:162-165`; allowlist exactly `localhost, 127.0.0.1, [::1]`), then
bearer check with `timingSafeEqual` (`token.ts:34-38`, correct). 401 carries
`WWW-Authenticate: Bearer` with no `resource_metadata` (`:174`).

On a public hostname: every UI request 403s at `app.ts:88`, every MCP request
at `mcp/http.ts:163`; proxy Host rewrite still fails on Origin (`app.ts:92`);
`settings.agent.url` hard-codes `http://127.0.0.1:${port}/mcp` (`app.ts:358`);
`server.ts:16` binds 127.0.0.1; nothing reads `X-Forwarded-*`; once reachable,
the whole UI API is open to anyone.

### MCP multi-user

Spec 2026-07-28: a protected MCP server is an OAuth 2.1 resource server; MUST
implement RFC 9728 PRM, validate audience (RFC 8707), 401 on bad tokens; AS
MUST be OAuth 2.1, SHOULD support CIMD; DCR deprecated-but-allowed.
Authorization overall is OPTIONAL (static bearer works where clients let you
set headers).

`@modelcontextprotocol/server` 2.3.1 ships resource-server helpers:
`requireBearerAuth`, `verifyBearerToken`, `bearerAuthChallengeResponse`,
`OAuthTokenVerifier`, `expectedResource`, `buildOAuthProtectedResourceMetadata`,
`oauthMetadataResponse`. Caveat: bearer helpers reject tokens with no
`expiresAt`. No authorization-server implementation in the v2 server exports.

Statelessness is compatible with per-user access: a verifier returns `AuthInfo`
with the user id and the tool context carries it; no sessions required.

Client requirements (vendor docs, 2026-10-25):

- **Claude Code**: static `--header` or OAuth via `claude mcp login` (DCR, CIMD,
  pre-registered client). Requires Pro/Max/Team/Enterprise/Console.
- **Codex**: `bearer_token_env_var`, `http_headers`, or `codex mcp login`.
- **Claude.ai custom connectors**: public HTTPS; OAuth (CIMD, DCR, own client)
  or fixed request headers — the latter "in beta, limited set of organizations".
  Free tier: one custom connector. Auth settings cannot be changed after adding.
- **ChatGPT**: OAuth only; PRM required; PKCE S256; CIMD preferred; no API keys.
  Plan gating unverified (snippets: Pro read-only, Business/Enterprise full).

Implication: per-user static bearer tokens cover CLIs; **Claude.ai and ChatGPT
effectively require being an OAuth 2.1 authorization server.**

### Input handling

UI body parsing (`app.ts:452-461`): no size limit, no Content-Type check,
valid non-object JSON reaches handlers → 500. MCP body capped at 4 MiB
(`mcp/http.ts:34,70-79`). MCP tools fully zod-validated; UI routes have zod only
for power and playtest. No max lengths on `reason`, `thesis`, `list`, profile
`text`, `summary`; arrays unbounded except `get_cards` (25).

Search compiler (`src/search/*.ts`): every user value bound via `?`; only
interpolated literals are integer masks and whitelisted columns; LIKE wildcards
escaped. **No SQL injection.** FTS5 MATCH terms split on non-alphanumerics and
quoted: safe. Archidekt import: regex parser, unbounded size and quantity.
Deep queries (~1000 nested terms) hit SQLite expression depth → 500.

### Abuse and limits

No rate limiting, no UI auth, no request timeouts, no listener caps. Sync
sqlite stalls all users on slow `compare`/`stats`/`export`. Refresh:
oracle-cards 203 MB on disk, `readFileSync` + `JSON.parse` (~1 GB heap), then
default-cards 79 MB gz / 634 MB streamed; minutes of wall time; one-click DoS.
SSE: global set, every mutation pings everyone, no heartbeat (proxies drop idle
SSE ~60 s), no per-user channel.

### Runtime

No `engines` field; type stripping unflagged needs Node ≥ 22.18/23.6,
`node:sqlite` ≥ 22.5; local is 26.12.1. `tsconfig.json` lacks
`erasableSyntaxOnly`. Container recipe: `npm ci` → `npx vite build` →
`node src/server.ts`, `data/` on a volume, cache dir relocated. No security
headers set (do at proxy). 500s return generic bodies; stacks to logs only.

### Prioritised changes

Blockers: user accounts + cookie sessions (2-4 d); tenant isolation (4-7 d, or
one SQLite file per user 1-2 d but cards must then be a shared DB); per-user
agent tokens (1 d); configurable Host/Origin + proxy awareness + correct
`agent.url` (0.5 d); refresh out of request path with lock and streaming ingest
(1 d); body limit + non-object rejection + rate limiting (0.5 d); stop returning
token/path from settings (0.5 d); SSE per-user channels + heartbeat (0.5 d).

Hardening: OAuth 2.1 resource server + external AS, fix SPA fallback for
`/.well-known/*` (2-4 d + AS); input caps; scope `items/rule` to URL deck;
`close()` old MCP handler on rotation; `engines`, `erasableSyntaxOnly`,
Dockerfile, `/api/health`; nightly `VACUUM INTO`; request timeouts.

### Bugs

1. Non-object JSON body → TypeError → 500.
2. With `DECKBUILDER_TOKEN` set, rotation rewrites the file but restart reverts
   to env; old `agents` handler never closed (`app.ts:370-375`).
3. `items/rule` not validated/scoped.
4. Concurrent refresh race; `force: true` discards freshness check.
5. NaN `limit`.
6. `mcp/http.ts:142` `new URL(req.url, http://${host})` before Host validation.
7. `stdio.ts:49` → `token.ts:22`: reading the token mints a new file as a side
   effect if missing.
8. Short-but-valid bulk file deletes cards (see §1 bug 1 for cascade).
9. SPA fallback 200s on unknown paths.
10. `seen` unbounded.

Not covered by tests: body-size limits, non-object JSON, traversal, refresh,
rotate-token, NaN limit, concurrency, DELETE deck via UI, non-loopback hosts.

---

## 3. Web client

### Shell and routing

Hash router (`web/src/app/routes.ts:15-22`), one `switch` in `App.tsx:17-38`,
providers Live → Toast → CardLookup → CardHover → Inbox → Focus
(`Providers.tsx:15-28`). `familyOrder` duplicated in `Sidebar.tsx:25-32` and
`DecksPage.tsx:26-33`.

Read-only is conflated with lifecycle: every gate is `category === "legacy"`
(`DeckHeader.tsx:23,52-54,62`; `DeckPage.tsx:118,123`; `DeckToolbar.tsx:32`;
`CardPanel.tsx:106`; `CardControls.tsx:119`; `BriefPanel.tsx:24`;
`PowerPanel.tsx:20`; `MemoriesPanel.tsx:50`; `EngineCard.tsx:23-26`;
`InteropDialog.tsx:19,33`; `DecksPage.tsx:66`; `DeckActions.tsx:15-17`;
`ProfilePage.tsx:85`). The server `read_only` flag pattern used by
`MemoryCard.tsx:107`, `DocumentEditCard.tsx:122`, `LogRow.tsx:124` is the one
that survives a non-owner viewer. Settings shows and copies the bearer token
(`AgentConnectionCard.tsx:70-75`, `clients.tsx:33,67,128`).

Needed: a `SessionProvider` and login route; a server-provided `can`/`viewer`
object on `DeckState` replacing ~20 legacy checks; status-aware `DeckStore`
(`DeckStore.tsx:67-76` treats every failure as "deck does not exist"); a
`PublicDeckPage` composition (DeckPage calls `useReportFocus`, `useReview`,
ruling hotkeys; `CardRow.tsx:30`, `GroupBlock.tsx:47-48` reach `useDeckUi`);
opaque ids/slugs; path routing for OG metadata (`index.html` has no viewport,
description or favicon).

### API client and live stream

`client.ts:86-96`: one `request()`, relative paths, no credentials option,
no timeout. **`res.json()` before `res.ok`** (`:93`): proxy 502/401 HTML
surfaces as `SyntaxError`. `ApiError` carries no status; no 401 handling
anywhere. SSE (`live.tsx:13-17`): one global `EventSource`, every message
refetches everything in `useQuery` and `DeckStore`; no dedup (sidebar,
memories page, `useDeckView.ts:43` each fetch independently). No connectivity
indicator; `SelectionBar.tsx:30-45` runs N sequential PATCHes.

### State

No optimistic updates; every mutation returns the full `DeckState`. Revision is
threaded for rulings, assessments and playtest but **not** for card membership,
slots/tags, brief, engines, rename: two humans would last-write-wins. WIP
`useReview.ts:48,81` uses `window.confirm`; `:85` auto-acknowledges staleness
for every proposal after the first in `acceptAll` (defeats the new check).
`localStorage` keys not namespaced by user.

### Screens

decks, deck (dense, complete), inbox (complete; untracked `ProposalHistory.tsx`
is unstyled debugging output), memories, activity, profile: all have
loading/error/empty states. Settings and `#/design` (1.2k lines, in the prod
bundle; `MtgExamples.tsx:157` calls `fetch` directly) are owner/dev tooling.
Playtest has the app's only responsive CSS. `HistoryPanel.tsx:40` caps at 40
entries with no way to see more.

### Responsiveness D, accessibility B-

No `<meta name="viewport">`; `body { overflow: hidden }`; fixed 216px + 1fr
grid; deck page minimum ~920px; only `@media` rules are in playtest. Hover card
mouse-only; Ctrl/Shift-click selection has no touch path.

Good: Dialog focus trap/restore, Tabs roving tabindex, Menu roles, AddBar
combobox pattern, hotkeys suppressed in inputs/modals, focus-visible ring,
toast live region, 111 aria-labels. Problems: one `prefers-reduced-motion`
rule in the app; `--faint: #5c636d` on `#0d0e10` ≈ 3.1:1 used for 11-13px text;
muted rows at 0.45 opacity; `MultiSelect.tsx:116` `role="dialog"` on a popover;
hover `role="tooltip"` never referenced; read-only title button focusable with
no action (`DeckHeader.tsx:62`); no skip link; no error boundary.

### Performance

Bundle: 483 KB raw / 153 KB gzip JS, 85 KB / 17 KB CSS, **one chunk**, no
`React.lazy`. Splitting playtest and styleguide likely cuts 30-40%. No
virtualization (fine at 100 cards; activity/memories unbounded). **Card images
hotlink `api.scryfall.com/cards/named?format=image`** (`mtg/cardImage.ts:6-8`),
a rate-limited API endpoint (2 req/s class), from every browser. `tokens.css:74`
lists Inter but nothing loads it.

### Onboarding

Empty account sees agent-centric copy everywhere (`Sidebar.tsx:102,117`,
`DecksPage.tsx:64,78-80`, `InboxPage.tsx:53,88-89`, `MemoriesPage.tsx:70`).
No first-run flow; `seed-demo.mjs` is a dev script. Concepts a newcomer will
not understand: agents, Paper/Theorycrafted/Legacy, proposals/rulings/"unread
by agents", memories stated/inferred/budgets in tokens, slots vs tags vs
secondary, brief/engines, brackets, revisions, keyboard-only review.

### Social UI effort (client only)

| Feature | Reuse | New | Days |
| --- | --- | --- | --- |
| Public read-only deck page | DeckList/GroupBlock/CardRow, ChecksPanel, BriefField readOnly, PowerPanel, hover | PublicDeckPage, public route, unauthenticated shell, viewer gating, byline | 4-6 |
| Share link | CopyButton, Dialog, Segmented | ShareDialog, visibility state, slug | 1-2 |
| Fork from someone else | ForkDialog verbatim, lineage, CompareDialog | button on public page, cross-owner parent display | 1-2 |
| Human suggestions | **proposal pipeline is exactly this**: banner, ReviewPanel, inbox, LogRow | a suggestion composer (nothing creates proposals in the UI today), a "human" tone (violet means agent in DESIGN.md), copy rewrite | 5-8 |
| Comments | RichText, TextArea, MemoryCard layout | whole feature + moderation + inbox entry type | 5-7 |
| Public profile | ProfilePage render path, DecksPage table | public route, handle, privacy toggle | 2-3 |

Plus 5-8 days of session/auth/viewer/error/SSE plumbing on the critical path.

### Code quality

128 web tests, all in `domain/`. Untested: `inbox/entries.ts`,
`rulings/logText.ts`, `usePlaytest.ts` (280 lines, riskiest). No interaction
tests. Duplicates: `familyOrder`, `Pill`. `MemoriesPage.tsx:55-57` effect with
no deps. `AgentConnectionCard.tsx:9` compatibility type for "older servers".

---

## 4. Product and social features

### Distinctive value

Moxfield stores *what* your deck is; this stores *why*: the brief, every
decision with its reason, what your agent learned about you, and lets any AI you
already pay for work inside those constraints without inventing a card. The
demo nobody else can give: an agent proposing a swap with a quoted oracle
clause, the owner rejecting it with a reason, and that reason following the
card forever.

Rename for newcomers: **Legacy → Archived** (Legacy is a format name),
**rulings → decisions** (rulings means card rulings). Hide: secondary slots,
review queue flag, memory budgets, ownership evidence, focus,
`compare_deck_sources`. Keep visible: brief, inbox, decisions log with undo,
Paper vs Theorycrafted, checks, forks, activity feed.

### Bring-your-own-agent onboarding

Claude.ai: Settings > Connectors > add URL > OAuth sign-in; three minutes, no
terminal, but needs OAuth (CIMD + DCR). ChatGPT: OAuth only; connector UI has
moved around; Secure MCP Tunnel is a developer path, drop it once hosted.
Claude Code / Codex: work today with bearer tokens; config in dotfiles.
Ship per-user OAuth for all plus long-lived personal tokens for CLIs, with a
"last seen from this client" indicator in Activity.

Addressable public is paying AI subscribers who play Commander. A deck read is
4-7k tokens; evaluation sessions tens of thousands. The app must be worth using
with no agent attached (spec §1 already promises this).

### Prioritised social features

| # | Feature | Builds on | New data | Size |
| --- | --- | --- | --- | --- |
| 1 | Public read-only deck page (list, brief, checks, bracket, decisions log on explicit opt-in, per-entry hide) | §11, §8.2, §5.7 | `visibility`, slug, `hidden` on log rows | M |
| 2 | Fork someone's public deck (copy list/slots/brief; **not** memories or ownership evidence; only published decisions) | §5.3 | cross-owner `parent_deck_id` | S |
| 3 | Human suggestions as proposals, ruled in the same inbox; third-party text quoted in agent renders | §8.1, Actor | principal on proposals | M |
| 4 | Compare to a friend's deck | `compare_decks` | visibility check | S |
| 5 | Publish bracket assessment ("conversation aid, not certification") | §5.7 | none | S |
| 6 | Shared playtest replay | playtest snapshots | share token | M |
| 7 | Merge request from fork to parent as one grouped proposal | compare + propose | `source_deck_id` | M |
| 8 | Playgroup (invite-only; members' public decks + shared table note) | profile, memories | group tables | L |
| 9 | Shared memories (owner-authored, deck-scoped, opt-in) | §7 | `visibility` on memories | M |

Do not build: likes/upvotes/trending/views; free-form comment threads (use #3);
separate primers (the brief is the primer); collection/inventory/price history
(§2 non-goals); real-time co-editing (collides with `applyChanges`); bracket
certification badges; image galleries.

Prompt-injection note: a stranger's text becomes untrusted input to the owner's
agent the moment `get_deck` renders it. Quote it as third-party and say so in
the playbook.

### Permission roles

| Role | Can |
| --- | --- |
| Viewer | read public/unlisted, compare, fork into own account |
| Suggester | + `propose_changes`, `propose_brief_edit` |
| Collaborator | + agent-level rights (edit Theorycrafted, organise, shortlist, assess); proposes on Paper |
| Co-owner | + rule, undo, lifecycle, import; not delete/transfer/visibility — **drop for v3**, use forks + merge requests |
| Owner | everything |

Rules: no principal rules on their own proposal (agent-via included); Paper
means *my* physical deck, so collaborators on Paper always propose; lifecycle
stays with the single owner regardless of roles; memories need a principal and
per-author visibility; profile, stats, focus, review queue become per-principal.

### Privacy

Private always: global memories, profile, ownership evidence/Have tags,
activity, proposal/ruling reasons on private decks. Opt-in per item/deck: brief,
decisions log, deck-scoped stated memories, owner assessments, deck price total.
Never: `inferred` memories, anything naming third parties, budget numbers,
superseded history, review queue. Cross-owner forks strip memories.

### Top 5 for v3 public

1. Hosted, multi-tenant, OAuth 2.1 on `/mcp` + personal tokens; Actor gains a
   principal; rewrite spec §2 and §10.1; real auth on the UI API.
2. Public read-only deck pages + cross-owner fork.
3. Agent-free first run plus a per-client connect wizard with live "last seen".
4. Human suggestions as proposals.
5. Scheduled card-data and price refresh with the data date shown everywhere.

Do not do yet: co-owners, real-time, playgroups, shared memories, ChatGPT
tunnel path, likes/comments/trending, collection, other formats, mobile app
(but public pages must be responsive).

---

## 5. Hosting, operations, legal, cost

### Local facts

Node v26.12.1 (Active LTS from 2026-10-28). DB 57 MB + **46 MB uncheckpointed
WAL**; 34,966 cards, 33,953 priced, 18 decks. `data/backups/` 109 MB manual
copies (made with WAL sidecars present, so only trustworthy if WAL was empty);
`.dev/` ≈320 MB; `.scryfall-cache/` 1.6 GB (mostly parser). Live Scryfall
manifest: oracle_cards 25 MB gz, default_cards 80 MB gz. Repo license MIT.

### Scryfall terms (scryfall.com/docs returns 403 to fetchers; wording via
proxy/snippets — verify in a browser)

- Data free under WotC Fan Content Policy; may not imply endorsement; **may not
  paywall** (payments, surveys, subscriptions, chat-server joins); "if you have
  an account system, end-users should be able to access card data anonymously
  or with free accounts"; may not simply repackage/proxy; mishandling may get
  API access blocked.
- Headers: `User-Agent` and `Accept` required (repo sends them).
- Rate limits: `/cards/named`, `/cards/search`, `/cards/random`,
  `/cards/collection` **2 req/s**; others 10/s; `*.scryfall.io` file hosts
  **unlimited**; "for resolving many card images, you must use the bulk data
  files". 429 → 30 s restriction, then bans.
- Bulk: daily; "prices should be considered dangerously stale after 24 hours";
  gameplay data weekly is fine.
- Images: **no hotlinking prohibition found**; don't crop copyright/artist,
  don't watermark. Fix: store `image_uris` at ingest and link `cards.scryfall.io`.
- Prices: no explicit attribution clause found; label as estimates from
  Scryfall daily bulk data with an as-of timestamp.

### WotC Fan Content Policy (company.wizards.com/en/legal/fancontentpolicy)

Must be free to view; can't require payments, subscriptions, or **email
registration** to access; can subsidise with sponsorships, ads, donations; can't
sell without permission; login allowed for private group play. Required notice:

> "[Title] is unofficial Fan Content permitted under the Fan Content Policy.
> Not approved/endorsed by Wizards. Portions of the materials used are property
> of Wizards of the Coast. ©Wizards of the Coast LLC."

Design consequence: public deck pages and card search viewable without an
account; account only to save your own decks. Free + donations only; no paid
tiers.

### Hosting

| | Volume | Deploy with SQLite | Backups | Est. cost |
| --- | --- | --- | --- | --- |
| Fly.io | $0.15/GB-mo, one Machine per volume | seconds of downtime per deploy | daily snapshots 5 d default; not primary | shared-cpu-1x 1 GB $6.70 ≈ **$7-8/mo**; 2 GB $13.39 |
| Railway | $0.15/GB-mo; Hobby 5 GB cap | small downtime; no replicas with volumes | daily/weekly/monthly built in | Hobby $5 incl. $5 usage; 1 GB always-on ≈ **$10-20/mo** |
| Render | $0.25/GB-mo | no zero-downtime with disk | 24 h snapshots, 7 d | 512 MB $7 (too small for refresh) / 2 GB **$25/mo** |
| VPS | local disk | you control (~1 s restart) | DIY Litestream + snapshots | Hetzner CX23 €5.99 (all plans "unavailable" on 2026-10-25); DO $6 (1 GB) / **$12 (2 GB)** |

Node prerequisites: `node:sqlite` RC since 25.7, unflagged since 22.13/23.4;
type stripping stable 24.12/25.2; DEP0210: `DatabaseSync` renamed `Database`
in 26.11 (alias kept, no warning). `sqlite.backup()` online backup available;
`DatabaseSync` `timeout` option (busy timeout) defaults to 0. Pin ≥ 24.12 or 26
via Dockerfile.

Refresh must not run in the request process in production: cron a second
process with `timeout` set, or build-and-swap tables; keep daily cadence.

Backups: Litestream 0.5.x (separate process, S3/B2/Spaces/Tigris; 0.5 cannot
restore 0.3 backups) to Backblaze B2 (first 10 GB free). Never copy the `.db`
alone: use `sqlite.backup()` or `VACUUM INTO`.

### Production checklist

Domain + TLS (Caddy on VPS; platform TLS on PaaS); configurable Host/Origin;
systemd or platform restart (`scripts/serve.mjs` is a laptop launcher);
JSON-line logs with request id/user/tool/status/ms; Sentry Developer free (5k
errors); UptimeRobot free (50 monitors, 5 min); Litestream + nightly backup +
**rehearsed restore**; secrets via env (`.env` gitignored; rotate agent token
on first deploy); `npm audit` in CI, monthly bumps, Node major pinned; privacy
policy + ToS (what is stored, why, retention, third parties incl. Scryfall
image requests from the browser, cookies, delete/export, governing law, WotC
notice, prices-are-estimates, as-is); GDPR: household exemption unlikely to
apply; answer access/erasure/portability within one month, free; minimise
(no raw IPs long-term, Discord email only if needed); self-service account
delete that cascades everything + "download my data" JSON.

### Auth build vs buy

MCP clients: static per-user bearer suffices for Claude Code/Codex; Claude.ai
(static headers beta-gated) and ChatGPT (OAuth only) need you to be an OAuth
2.1 AS with PRM, AS metadata, CIMD/DCR, PKCE, consent screen, refresh rotation.

| Option | Effort | Per-user MCP tokens | OAuth 2.1 AS |
| --- | --- | --- | --- |
| Roll your own (Discord + cookie sessions in SQLite + API keys) | 1-2 wk login; **+2-3 wk and real risk** for a compliant AS | trivial | only if you write it |
| Lucia-style guides | reference only (library deprecated March 2025) | you write it | no |
| **Better Auth** (`toNodeHandler`, native `node:sqlite` adapter, Discord provider, `@better-auth/api-key`, `@better-auth/mcp` + `@better-auth/cimd`, `@better-auth/oauth-provider`) | **days**; you supply the consent page | yes, with per-key rate limit/expiry | **yes** (RFC 9728 + 8414, JWKS, CIMD; DCR opt-in) |
| Auth.js | days | you write it | no (now "part of Better Auth") |
| Clerk | days (Express adapter; plain http needs shim) | OAuth tokens | yes, hosted (CIMD/DCR toggles); free to 50k MRU, Pro $25/mo; lock-in |
| Discord/Google only | as roll-your-own | static bearer | no |

Recommendation: Better Auth on the existing `node:sqlite` connection and plain
`http` server; resolve bearer keys to a user in `mcp/http.ts`; return
`401 + WWW-Authenticate: Bearer resource_metadata=…`. Clerk if you'd rather not
own a consent screen. Note Better Auth docs are mid-transition (v1.8 beta MCP
plugin): pin a version. This adds a dependency beyond the AGENTS.md trio
(React, MCP SDK, zod); a deliberate decision.

### Cost

A few hundred users: **$8-25/month** (compute + volume; B2/Sentry/UptimeRobot/
Better Auth/Discord free; domain ~$10-15/yr). A few thousand: **$15-45/month**
(2-4 GB RAM, refresh out of process). Images stay hotlinked to Scryfall's CDN,
so egress is negligible.

Biggest operational risk: one SQLite file is the whole product with no
automated, consistent, off-box backup, while refresh runs inside the serving
process with a heap spike and a long write transaction at busy-timeout 0.

Could not be verified: Scryfall price-attribution clause; Scryfall hotlinking
prohibition (none found); exact ChatGPT plan gating; Hetzner availability;
platform default Node versions; domain cost; jurisdiction-specific legal
requirements.
