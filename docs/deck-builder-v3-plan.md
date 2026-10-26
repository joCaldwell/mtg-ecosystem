# Deck-builder v3: the public, multi-user release — execution plan

Written 2026-10-25 from the readiness review in
[deck-builder-public-readiness.md](deck-builder-public-readiness.md). That
document holds the evidence (file:line findings); this one holds the decisions
and the work, cut into phases that one agent can carry end to end without
asking the owner anything.

**Read before any phase:** this file, the readiness review, the spec
([deck-builder-spec.md](deck-builder-spec.md)), `packages/deck-builder/AGENTS.md`,
and `packages/deck-builder/web/ARCHITECTURE.md`. The spec remains the source of
truth; every phase that changes behaviour updates the spec in the same PR.

**Status (2026-10-25):** phases 0–2 merged (PRs #3–5). Phase 3 is deployed
with the owner's 18 decks and 36 memories; PR #7 awaits owner review/merge.
Owner claim, claim-code removal, personal-token MCP access, SSE reconnect,
refresh and bucket restore are verified. Native volume snapshots remain
blocked by Railway's Pro-only restriction on this Hobby workspace; external
monitor setup and second-account/client acceptance remain owner steps.
Parallel feature phases continue in their own worktrees and reach staging first.

---

## 0. How the phases are run

- **One branch per phase**, named `v3/<n>-<slug>` (e.g. `v3/1-identity`), cut
  from `main`. One PR per phase; the owner merges. The PR description is the
  phase report: what was built, what was deliberately left out, what the next
  phase inherits.
- **Gate before every commit:** `npm run check` in `packages/deck-builder`
  (typecheck src + web, then `node --test`). Tool text tests pin wording that
  agents see; change wording deliberately and update the test in the same
  commit, never loosen the assertion.
- **The spec, AGENTS.md and ARCHITECTURE.md are deliverables.** A phase is not
  done until they describe the new behaviour and no longer describe the old.
- **Decisions in this document are made.** If a phase meets a question this
  document does not answer, pick the option that keeps the five rules at the
  top of AGENTS.md intact, write the choice and the reason into the spec, and
  continue. Do not stop to ask.
- **Phases are sequential** (0 → 1 → 2 → 3 → 4a → 4b → 5 → 6) with two
  exceptions, each run in its own worktree once 1 has merged: 1b (price
  sources) needs only phase 1's per-user settings and may run in parallel
  with 2 and 3; 4b (UX overhaul) touches only the web client and may run in
  parallel with 2 and 3.
- **Nothing the owner must do mid-phase.** What the owner must provide
  *before* a phase starts is listed in §2; everything else the agent obtains
  itself (Railway via the CLI, Scryfall via bulk data, docs via the web).
- **Prerequisite for phase 0:** the current uncommitted working tree
  (tag-filter inversion, filtered export, stale-proposal acknowledgement,
  proposal history; ~46 files, tests green) is committed on `main` first.
  The phase-0 agent may do this as its first commit if it is still pending,
  after fixing the two issues noted in §3 phase 0 item 11.

---

## 1. Decisions that bind every phase

These settle the questions the readiness review left open. They are stated
once here; phases reference them by number.

**D1. Two modes, one codebase.** `DECKBUILDER_MODE=local` (default when
unset) keeps today's experience: loopback bind, no login page, one implicit
user, agent token required for MCP. `DECKBUILDER_MODE=hosted` binds all
interfaces, requires `DECKBUILDER_PUBLIC_ORIGIN` and auth secrets, and gates
every request on a session or token. The test suite runs in local mode unless
a test opts into hosted. Local mode is not a shim: it is the product the owner
uses every day, and the spec keeps describing it.

**D2. One SQLite file, tenant key on every per-user table.** Not a file per
user: card data is shared and `deck_cards` holds a foreign key into `cards`.

**D3. Identity.**

```ts
// src/actor.ts
export type Actor = {
  user_id: number;            // the principal: whose data is being touched
  via: "ui" | "agent";        // a human in the browser, or an agent acting for them
  label: string;              // "ui" or the agent's attribution label
  token_id?: number;          // set when via === "agent"
};
```

"Owner" is no longer a kind of actor; it is a relation: `actor.user_id ===
decks.user_id`. `requireOwner(actor, what)` becomes
`requireDeckOwner(db, actor, deckId, what)` (and `requireSelf(actor, userId)`
for profile/settings). Agent-versus-human distinctions that exist today
(`awaitsReview`, `owner_touched`, Paper proposal rule, memory `basis`) key on
`actor.via`. Attribution renders "Josh via claude-code" and stores
`author_user_id`, `author_via`, `author_label` on every row that has an
`author` column today.

**D4. Roles in v3.** Owner (decks.user_id); viewer (anyone the visibility
rule admits); suggester (any signed-in user on a deck with
`accept_suggestions = 1`). No collaborator, no co-owner, no `deck_members`
table in v3; forks plus suggestions are the collaboration unit. Lifecycle,
rulings, undo, import, delete and visibility stay with the owner. Nobody rules
on their own proposal, agent-via included.

**D5. Users.**

```sql
CREATE TABLE users (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  handle       TEXT NOT NULL UNIQUE COLLATE NOCASE,   -- [a-z0-9_-]{3,24}
  display_name TEXT NOT NULL,
  auth_id      TEXT UNIQUE,            -- Better Auth user id; NULL in local mode
  is_admin     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  deleted_at   TEXT
);
```

The tenant migration creates user 1 (`handle='owner'`, `display_name='Owner'`,
`is_admin=1`) and assigns every existing row to it. In local mode user 1 is
the implicit user. Hosted claiming requires `DECKBUILDER_CLAIM_CODE`, entered
in the invite/claim field, compared timing-safely and consumed only while
user 1 has `auth_id IS NULL`. The code then becomes dead even if still set;
log one line telling the owner to remove it. Unset disables claims; new
identities create new users, invite permitting. Wrong/already-used codes
confer no access. This owner amendment supersedes the unsafe first-sign-in
claim rule.

**D6. Auth library: Better Auth.** Discord and Google providers, cookie
sessions, existing `agent_tokens` for personal credentials (skip api-key),
`@better-auth/mcp` (+ `@better-auth/cimd`) so the app is an OAuth 2.1
authorization server for Claude.ai and ChatGPT connectors. Phase 2 pins these packages and their OAuth provider peer to 1.7.13, adds
`jose@6.2.3` for JWKS verification, and retains personal tokens without api-key.
It runs on the existing `node:sqlite` connection via its native adapter and on the plain
`http` server via `toNodeHandler`. Better Auth owns its own tables (`user`,
`session`, `account`, `verification`, JWT and OAuth tables); our `users`
table links through `auth_id`. This adds dependencies beyond the AGENTS.md
trio; AGENTS.md is updated to say so and why. Pin the version in
`package.json` and read that version's docs, not the latest.

**D7. Agent credentials.** Two kinds, both resolving to an `Actor`:
personal tokens (`dbk_` prefix, 32 random bytes base64url, stored hashed,
shown once, label + last-used + revoke in Settings) for CLIs; OAuth access
tokens issued by Better Auth for Claude.ai/ChatGPT. The `/mcp/<label>` path
suffix remains an attribution hint only. Unauthenticated `/mcp` requests get
`401` with `WWW-Authenticate: Bearer resource_metadata="<origin>/.well-known/oauth-protected-resource/mcp"`.

**D8. Visibility and sharing.**

```sql
ALTER TABLE decks ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private'
  CHECK (visibility IN ('private','unlisted','public'));
ALTER TABLE decks ADD COLUMN slug TEXT UNIQUE;          -- 10 chars base62, set when visibility first leaves private, never changes
ALTER TABLE decks ADD COLUMN publish_decisions INTEGER NOT NULL DEFAULT 0;
ALTER TABLE decks ADD COLUMN accept_suggestions INTEGER NOT NULL DEFAULT 0;
ALTER TABLE log   ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;   -- owner hides single entries from the public page
```

Public URL: `/d/<slug>`. A public page shows the list by slot, brief, checks,
bracket target and the owner's latest assessment, price estimate with source
and date, and the decisions log only if `publish_decisions = 1`, minus hidden
entries. Never shown: memories, proposals, pending edits, ownership evidence,
activity, the owner's agent names. Cross-owner forks copy list, slots, tags,
brief and engines; never memories, ownership evidence, assessments or
unpublished log entries.

**D9. Naming.** `legacy` → `archived` (data value, tool text, UI). "Rulings"
→ "decisions" in all user-facing and agent-facing text; the `log` table and
`rule_items` code names may stay. "Theorycrafted" stays. Done in phase 4b, in
one PR, with the spec updated.

**D10. Card images.** Ingest stores `image_small`, `image_normal`,
`image_art_crop` URLs (from `image_uris`, or the front face's for multi-face
layouts) on `cards`; the client links `cards.scryfall.io` directly. Nothing
ever calls `api.scryfall.com/cards/named` from a browser again. Artist and
copyright stay visible on full card images; art crops show the artist line.

**D11. Refresh and backups run as child processes of the web service.**
Railway attaches a volume to exactly one service, so a separate cron service
cannot see the database. The web process schedules `node src/cli.ts refresh`
(daily, plus on boot if `last_ingest` is older than 24 h) and
`node src/cli.ts backup` (nightly) as child processes with their own DB
connection and `timeout` (busy timeout) set. The web process sets
`timeout: 5000` on its own connection. Ingest streams the oracle file instead
of parsing it whole; prices build into a staging table and swap. A `meta`
lock row prevents overlap. The HTTP trigger becomes admin-only
(`is_admin = 1`) and only enqueues the child.

**D12. Hosting.** Railway, new project `deck-builder`, one service built from
a Dockerfile (`node:26-slim`) with `packages/deck-builder` as the app, a
volume at `/data` (`DECKBUILDER_DATA=/data`), a Railway bucket for off-volume
backups, Railway's own volume backups enabled, the Railway-provided domain for
the beta and a custom domain when the owner supplies one. Staging environment
mirrors production with its own volume. Hobby plan is sufficient (see review).

Phase 3 deployment decisions (2026-10-25): production follows **`release`**;
staging follows **`main`**. The owner promotes with `git push origin
main:release` (or an explicitly selected, staging-verified commit). `release`
was initialized at **`3e1798d`**, the merge of Phase 2 PR #5 and this phase's
predecessor. Parallel 1b/4a migrations must reach staging before production.
Exactly **one replica in each environment**: one SQLite writer and in-memory
rate limits. From go-live the hosted database is the owner's primary copy;
the untouched local database becomes development data and is never synced
back over production. The initial deployment packages the Phase 3 container
and observability assets with that predecessor's product code; after this PR
merges the owner promotes its merge commit to put those assets on `release`.

Railway's live API rejected daily/weekly native volume backup schedules on
the owner's Hobby workspace: **"Manual backups and backup schedules are only
available for Pro workspaces"**. Both environments still have nightly online
SQLite backups uploaded to their separate Railway buckets. Native snapshots
remain an explicit owner action requiring a plan upgrade; Phase 3 does not
silently change the workspace subscription. See OPERATIONS.md for the exact
enablement procedure and live evidence.

**D13. Legal posture.** Free, donations at most, no paid tiers (WotC Fan
Content Policy). Public deck pages and card search work with no account.
Accounts are required only to save decks. Footer carries the Fan Content
notice; prices are labelled as Scryfall daily estimates with the store and
the as-of date (D18);
privacy policy and terms pages exist before anyone outside the owner signs in;
self-service account deletion and data export exist before public sign-up.

**D14. Beta gating.** Hosted sign-up requires an invite code until phase 6
removes the gate. `invites(code, created_by, used_by, used_at)`; the admin
creates codes in Settings.

**D15. Third-party text is untrusted.** Any text a non-owner wrote
(suggestion reasons, later comments) is rendered to agents inside an explicit
quoted block with the author's handle, and the playbook tells agents it is not
the owner's voice. Nothing a suggester writes reaches memories or the brief
without a ruling.

**D16. Routing.** The web client moves from hash routes to path routes in
phase 4a so public pages have real URLs with server-injected Open Graph tags.
The static fallback serves `index.html` for app routes only; `/api/*`,
`/mcp*`, `/.well-known/*`, `/assets/*` and unknown file extensions return 404.

*D17–D20 were added on 2026-10-25, after phase 1 merged, from the owner's
answers to a documentation review.*

**D17. Phones read and rule; agents edit.** Below 720 px the signed-in app
is a read-and-rule view, not a shrunken editor. It shows the deck list, each
deck through the same read-only composition as the public page (4a), the
inbox, and the deck's pending proposals with accept/reject per item, per
group or all at once, plus the optional reason. Accept and reject are the
only writes on a phone. Changes to a deck from a phone are made by prompting
an agent (Claude or ChatGPT on the phone, through the phase 2 OAuth
connectors) and ruling on what it proposes. Not on phones: direct editing,
hotkeys, the playtester and Settings (each says "open on a larger
screen"). 720 px and wider keep the full app; 4b
makes it hold up down to that width. Ruling uses the existing capabilities
and needs no new backend. This workflow depends on hosting: phone agents
cannot reach a loopback app.

**D18. Price sources from Scryfall data only.** Scryfall's `default-cards`
file, already read daily, carries `usd`/`usd_foil`/`usd_etched` (TCGplayer)
and `eur`/`eur_foil` (Cardmarket) per printing. Verify the field-to-store
mapping against Scryfall's documentation and cite it. The app stores each
card's cheapest playable printing per store, using the playable-printing
rules already in spec §4.4. Each user chooses a currency (`USD` default,
or `EUR`) and a store: one of that currency's stores, or `lowest` (the
cheapest store in that currency; there is no exchange-rate source, so
`lowest` never compares USD with EUR). With one store per currency today,
the choice is effectively TCGplayer or Cardmarket; the shape admits more
stores if Scryfall adds them. The choice drives the price on every card,
deck totals and unpriced counts, any price-based check, which comparator
search treats as "my price", and what agents are told. Every price shown or
returned names its store and as-of date once per view or response. Public
pages use the viewer's choice when signed in, otherwise `USD`/`lowest`.
No other price source (MTGJSON, vendor APIs, scraping) and no exchange rates.

**D19. The public surface is stable once others depend on it.** The root
AGENTS.md rule "rewrite rather than shim" keeps applying to internals. From
the first non-owner account (phase 3), user data is never dropped: every
migration preserves and backfills user-authored content. From public sign-up
(phase 6), three surfaces change only additively or through a deprecation
that keeps the old form working for at least one release and says so in tool
text: MCP tool names, argument names and result shapes; shared URLs
(`/d/:slug`, `/u/:handle`); the token format. D9's renames land in 4b,
during the invite-only beta, with migrations and no deprecation period.

**D20. Art where people decide whether to look.** The editor stays text-first
(DESIGN.md principle 3). Public deck pages carry the commander's art crop in
the header, and link previews use it as the Open Graph image, with the
artist line per D10. Cards in the list stay text rows; their pictures are on
demand, as in the editor.

---

## 2. What the owner provides, and when

| Before phase | Item | Why |
| --- | --- | --- |
| 0 | Commit the current working tree to `main` (or allow the agent to) | Phase 0 builds on it |
| 2 | A Discord application (client id + secret) with redirect URIs `http://localhost:8787/api/auth/callback/discord` and `https://<railway-domain>/api/auth/callback/discord` | Discord login |
| 2 | Optionally a Google OAuth client with the same two redirect paths | Google login; phase 2 ships Discord-only if absent |
| 2 | A contact email and the operator's name for the privacy policy and terms | Required text |
| 3 | Nothing: the Railway CLI is logged in on this machine and the agent creates the project. Optionally a custom domain name | Beta can run on `*.up.railway.app` |
| 6 | The decision to open sign-up (remove invite gating) | Public launch |

Secrets are never pasted into chat or committed. The agent asks for them via
Railway variables (`railway variable set`) and local `.env` (gitignored).

---

## 3. The phases

Each phase lists: goal, scope (numbered, with the files it starts from),
explicitly out of scope, acceptance criteria, and documentation deliverables.
Effort labels are relative sizes for sequencing, not calendar promises.

---

### Phase 0 — Hardening that holds regardless of launch (size: L)

**Goal.** Fix everything the review found that is wrong today, single-user
included, and make the process safe to run unattended: images from bulk data,
refresh out of the event loop, consistent backups, honest error handling.
After this phase the local app is strictly better and nothing about it is
multi-user yet.

**Scope.**

1. **Images at ingest (D10).** Add `image_small`, `image_normal`,
   `image_art_crop` to `cards` (`src/db.ts`), populate in `src/ingest.ts`
   from `image_uris` (front face for `card_faces` layouts; verify the Scryfall
   bulk `oracle_cards` shape against https://scryfall.com/docs/api/cards and
   cite it in a comment). Return them from `/api/cards/:id/table`, search
   results and `get_cards`. Replace `web/src/mtg/cardImage.ts` so every
   `<img>` uses the stored URL; `loading="lazy"` on hover images too. Delete
   the `cards/named` construction entirely.
2. **Refresh as a child process (D11).** New `src/jobs.ts`: a scheduler in
   the web process that spawns `node src/cli.ts refresh` daily and on boot
   when stale, and `node src/cli.ts backup` nightly; one job at a time;
   stdout/stderr captured to the log; status (`last run, result, next run`)
   readable at `GET /api/settings`. `POST /api/cards/refresh` only enqueues.
   Both CLI entry points open their own `DatabaseSync` with `timeout: 5000`;
   the server connection sets `timeout: 5000` as well. `meta` row
   `job:lock` with a timestamp guards overlap; stale locks (> 2 h) are
   reclaimed.
3. **Streaming ingest.** `src/ingest.ts` parses `oracle-cards.json` as a
   stream (it is a JSON array; implement an incremental array reader over
   `fs.createReadStream`, no new dependency) and upserts in batches of ~500
   inside savepoints, so peak heap is bounded. `src/prices.ts` loads into
   `card_prices_staging` and swaps tables in one short transaction. Move
   `.scryfall-cache` under `DATA_DIR`. Remove `force: true` from the HTTP
   path; honour the 24 h freshness check; `--force` stays on the CLI.
4. **Ingest safety.** Cards that disappear from the bulk file are never
   deleted while referenced: mark them `retired = 1` (new column) instead,
   keep them resolvable by exact name, exclude from search unless the deck
   already holds them, and surface a check ("retired printing/oracle id") in
   `src/deck/checks.ts`. Refuse to ingest a bulk file whose card count is
   below 90 % of the current table (truncation guard).
5. **Backups.** `node src/cli.ts backup`: `sqlite.backup()` to
   `DATA_DIR/backups/deck-builder-YYYY-MM-DD.db`, keep 14, then upload to an
   S3-compatible bucket when `BACKUP_S3_*` variables are set (use
   `@aws-sdk/client-s3`; it is an ops dependency and AGENTS.md says so). A
   `node src/cli.ts restore <file>` that verifies the file opens, counts
   decks, and swaps it in only when the server is not running. Document the
   restore drill in a new `packages/deck-builder/OPERATIONS.md`.
6. **HTTP hygiene** (`src/app.ts`): 1 MiB body limit with 413; reject
   non-object JSON with 400; `Content-Type` check; `limit`/`quantity`/ids
   parsed through a small `num()` helper that 400s on NaN; `items/rule`
   validates `item_ids` is an array of integers and that each item belongs to
   the URL's deck; `close()` the previous MCP handler on token rotation; drop
   the absolute server path from `/api/settings`; `GET /api/health`
   (unauthenticated, returns `{ ok, schema_version, cards, last_ingest }`).
   Static fallback per D16 (hash routes still work because `/` serves the
   app); `Cache-Control: immutable` for `/assets/*`, `no-cache` for
   `index.html`; MIME entries for woff2/png/svg.
7. **Token file semantics** (`src/token.ts`, `src/mcp/stdio.ts`): reading
   never writes; the shim fails with a clear message if no token is
   configured; rotation while `DECKBUILDER_TOKEN` is set is refused with an
   explanation.
8. **SSE heartbeat.** `/api/events` sends a comment line every 25 s and sets
   `X-Accel-Buffering: no`.
9. **Capability-layer fixes** from the review: `markReviewed({through})`
   scoped to the deck being read; `findSimilar` filters by scope *before*
   taking the top N; `reviseMemory` `confirm` runs `assertMayChange`
   (document the choice in spec §7.3); `createDeck` takes an actor and
   writes activity for agent creations; UNIQUE violations detected by SQLite
   error code, not message substring; backfill `base_revision` for
   pre-existing proposals and document edits to the deck's current revision
   so they are not permanently stale.
10. **Client error handling** (`web/src/api/client.ts`, `DeckStore.tsx`):
    `ApiError` carries `status` and the server's `error` text; check
    `res.ok` and content type before parsing; `DeckStore` distinguishes 404
    ("does not exist") from 403 ("you do not have access") from network/5xx
    ("could not load, retry"); a top-level React error boundary with a
    "reload" action; `window.confirm` in `useReview.ts` replaced by the
    existing `ConfirmDialog`; fix `acceptAll` so `acknowledge_stale` is only
    sent for proposals that were stale at the moment of the click.
11. **Finish the WIP:** style `ProposalHistory.tsx` with the existing
    primitives (no raw `<details>`, no ISO timestamps, no kind identifiers);
    `MemoriesPage.tsx` effect dependency array; dedupe `familyOrder` and
    `Pill` into `domain/` and `ui/`.
12. **Runtime pins:** `"engines": { "node": ">=24.12" }`, `.nvmrc` with
    `26`, `erasableSyntaxOnly: true` in both tsconfigs, rename
    `DatabaseSync` → `Database` (DEP0210).

**Out of scope.** Anything with a `user_id`. Auth. Visibility. Renames.

**Acceptance.**
- `npm run check` green; new tests for: body limit, non-object JSON, NaN
  limits, `items/rule` cross-deck rejection, static 404s, health, image URLs
  present after ingest of a fixture, truncation guard, retired-card
  behaviour, backup/restore round trip on a temp DB, job lock reclaim,
  `ApiError.status`.
- A full `npm run refresh` against live Scryfall completes with the server
  running and serving requests throughout (manual check, recorded in the PR
  with peak RSS of the child).
- No request to `api.scryfall.com` appears in the browser network panel when
  hovering cards.

**Docs.** Spec §4.1 (refresh is scheduled; retired cards), §4.4 (price swap),
§13 (images); AGENTS.md (commands, dependencies, OPERATIONS.md pointer);
OPERATIONS.md (new: jobs, backup, restore drill, env vars).

---

### Phase 1 — Identity foundation: users, tenancy, actor, tokens (size: XL)

**Goal.** Every row that belongs to someone knows whom; every capability takes
an `Actor` with a `user_id`; every read and write is scoped to it; agents
authenticate with per-user tokens. The UI keeps working in local mode as the
implicit user 1. No login page yet. This is the phase on which everything else
stands, so it ships with a permission matrix test that later phases extend.

**Scope.**

1. **Spec first.** Rewrite spec §2 (non-goals: hosting and accounts are now
   goals; list what remains out), §10.1 (identity, tokens, modes), add §17
   "Accounts, modes and roles" containing D1, D3, D4, D5, D7 verbatim or
   better. Commit the spec change before the code.
2. **Migration framework.** Replace the version ladder in `src/db.ts`
   `openDb` with an ordered list of `{ version, up(db) }` steps; keep the
   single-transaction guarantee and `assertOurs`. Port the existing steps.
3. **Schema (SCHEMA_VERSION → 11):** `users` (D5); `user_id` on `decks`,
   `memories`, `document_edits`, `log`, `activity`, `ownership_evidence`;
   `profile` rebuilt as one row per user; `playtest_sessions` PK
   `(deck_id, user_id)`; `user_settings(user_id, key, value)` replacing
   `meta` `setting:*` keys; `decks` `UNIQUE(user_id, name)`;
   `agent_tokens(id, user_id, label, prefix, token_hash, created_at,
   last_used_at, revoked_at)`; `author_user_id`, `author_via`,
   `author_label` beside every existing `author` column, backfilled
   (`"owner"` and `"owner (from v1)"` → user 1 / `ui` / `ui`;
   system `"migration"` → user 1 / `ui` / `migration`, displayed as
   “System migration”; other labels → user 1 / `agent` / label; spec §17.3).
   Backfill everything to user 1. Migration tests construct a v10 file and
   assert the result, following `test/db.test.ts`.
4. **Actor (D3).** Rewrite `src/actor.ts`; `requireDeckOwner`,
   `requireSelf`, `isDeckOwner`. Every exported capability function takes
   `actor` (the 59 that don't today). `requireDeck(db, actor, id)` returns
   the deck plus the caller's role; `requireEditableDeck` requires owner (v3
   has no other editor). All cross-tenant queries from review §1.6 gain the
   scope. `include_my_decks` means the caller's decks. `setDeckParent` and
   fork accept only decks the caller owns (cross-owner fork arrives in 4a).
   `cardHistory` lineage stops at deck boundaries the caller cannot read.
5. **Per-user singletons.** Profile, settings/budgets, focus, stats, inbox,
   review queue, activity: all keyed by `actor.user_id`. Activity rows record
   both `user_id` (whose data) and `actor_user_id`/`actor_via`/`actor_label`.
6. **Live stream.** `/api/events` becomes per-user: the server keeps
   `Map<user_id, Set<res>>`, `notify(user_id, { deck_id?, kind })`; MCP
   writes notify the owning user. The client payload now names what changed
   so `useQuery` can refetch selectively (keep "refetch all for that user" as
   the first implementation; the payload shape is the contract).
7. **Tokens (D7).** `src/token.ts` becomes `src/tokens.ts`: create, list,
   revoke, verify (hash lookup by prefix, `timingSafeEqual` on the hash),
   `last_used_at` updated at most once a minute. `src/mcp/http.ts` resolves
   the bearer to `{ user_id, token_id, label }` and builds the Actor; the URL
   label is attribution only. In local mode the legacy `data/agent-token`
   file is migrated into an `agent_tokens` row for user 1 on first boot so
   existing client configs keep working; after commit the file is renamed to
   `stdio-token` (0600), the local shim’s default. Explicit private files use
   `--token-file` or `DECKBUILDER_TOKEN_FILE`; secrets in argv are rejected. Settings
   API: list/create/revoke tokens; the token value is returned exactly once,
   from create.
8. **Local mode (D1).** `src/mode.ts` reads `DECKBUILDER_MODE`; in local
   mode `app.ts` builds `Actor { user_id: 1, via: "ui", label: "ui" }` for
   every UI request. Hosted mode routes return 401 for the UI API until phase
   2 wires sessions (this is acceptable: hosted mode is not deployable until
   phase 3).
9. **MCP tools.** Every tool goes through the Actor. `start_session` returns
   the user's handle and display name. `deckId()` name resolution searches
   only the caller's decks. New tool `whoami` is not needed; fold into
   `start_session`. Tool text that says "the owner" is reviewed: it still
   means the human whose data this is, which is now `actor.user_id`'s human.
10. **Permission matrix test.** `test/permissions.test.ts`: two users, each
    with a Paper and a Theorycrafted deck; for every capability function and
    every MCP tool, assert: own deck behaves as before; other user's deck is
    `404` for every unreadable target, on reads and writes alike (never
    `403`, to avoid existence leaks); `PermissionError` is reserved for an
    action forbidden on a readable target; tokens of user A never touch user B's rows; memory search,
    inbox, activity, stats and `include_my_decks` never return the other
    user's rows. Generate the capability list from the exports so new
    functions fail the test until classified.
11. **Settings UI:** tokens card (create with label, show once, copy, list
    with last used, revoke) replaces the single-token display; client
    snippets use a placeholder until a token is created; "Reachable from"
    copy driven by mode.

**Out of scope.** Login, sessions, Better Auth, visibility, public pages,
renames, collaborator roles.

**Acceptance.**
- `npm run check` green including the permission matrix.
- A v10 database (copy the owner's real `data/deck-builder.db` to a temp
  path) migrates, boots in local mode, shows the same decks, and existing
  Claude Code / Codex configs keep working through the migrated token.
- Playbook transcript check: run one `start_session` + `get_deck` through
  the stdio shim and confirm wording is unchanged except the new identity
  line.

**Docs.** Spec §2, §10.1, §17, §7.3, §8.4 (review queue per user), §15/§16
updated; AGENTS.md rules 1 and 5 rewritten for Actor-with-principal; the
constraints list (owner-only → deck-owner-only; tokens).

---

### Phase 1b — Price sources (size: M)

**Goal.** Each user chooses where their prices come from (D18). Every price
the app shows or hands an agent follows that choice and names its store.
Prices stay daily Scryfall estimates, with no new external source. May run in
a worktree in parallel with phases 2 and 3.

**Scope.**

1. **Spec first.** Rewrite spec §4.4 for per-store prices and the user's
   choice, and update the §2 "reopened" entry from planned to built.
2. **Prices per store.** `card_prices` (today one row per `oracle_id` with
   `usd_cents`) becomes one row per `(oracle_id, store)`. `store` is
   `tcgplayer` or `cardmarket`. Each row stores `currency`, `cents` and the
   printing and finish it came from. Per store, take the lowest of that store's
   finishes across the counted printings. The counting rules stay as they are:
   paper, not oversized, not memorabilia. Keep the staged load and atomic swap
   from phase 0 (`src/prices.ts`, `src/refresh.ts`) and the bounded peak heap.
   Migrate through the ordered migration list; existing USD rows become
   `tcgplayer` rows.
3. **The user's choice.** `user_settings` gains `price_currency` (`USD` |
   `EUR`, default `USD`) and `price_store` (`lowest` or a store in that
   currency, default `lowest`). Add `getPriceSource(db, actor)` and
   `setPriceSource(db, actor, …)`. The setter rejects a store that does not
   sell in the chosen currency. Agents may read the choice and not change it
   (`requireUi`), like other settings. Classify both in the permission matrix.
4. **Every consumer follows it.** The price on cards and rows, deck totals
   and unpriced counts (`src/deck/service.ts`, `src/deck/sourcing.ts`,
   proposals' price deltas), `src/cards.ts` card facts and `get_cards`
   structured results all resolve through one function taking the actor.
   Search keeps Scryfall's own operators (`usd`, `eur`), each comparing that
   currency under the user's store choice for that currency, else `lowest`.
   Verify the syntax against Scryfall's syntax guide and cite it.
5. **Agent text.** Per-card lines keep the compact price with its currency
   symbol (`$1.71`, `€1.20`). `start_session` and each deck render state the
   store and as-of date once. The playbook's price paragraph says the price
   is the user's chosen store. Change `tools.test.ts` deliberately.
6. **UI.** Settings gains a price-source control: currency, then store or
   "Lowest". The deck header total and card panel name the store and date.
   `web/src/domain/price.ts` formats by currency.
7. **Public pages (when 4a lands).** `PublicDeckState.price` follows the
   viewer's choice if signed in, otherwise `USD`/`lowest` (§4 shape). If 1b
   merges first, 4a builds on it; if 4a merges first, 1b updates the public
   page.

**Out of scope.** MTGJSON or any non-Scryfall price data, exchange rates,
price history, foil-only preferences, printings the owner holds, buy links.

**Acceptance.**
- `npm run check` green, including the permission matrix with the new
  settings capabilities.
- On a copy of the owner's database after a real refresh, user 1's deck
  totals and per-card prices under the default (`USD`/`lowest`) equal the
  pre-phase values exactly, since today's price is the TCGplayer one. Record
  priced-card counts per store.
- A second user set to `EUR` sees Cardmarket prices in the UI, in agent text
  and in `eur<=` search, while user 1 sees USD.
- The refresh's peak heap stays within phase 0's bound.

**Docs.** Spec §2, §4.2 (search operators), §4.4, §10.3 (tool results),
§11 (Settings); playbook; AGENTS.md if a constraint changes; OPERATIONS.md if
refresh behaviour changes.

---

### Phase 2 — Accounts, sessions and OAuth for hosted mode (size: XL)

**Goal.** A stranger can sign in with Discord (or Google), claim or create an
account, connect Claude Code with a personal token and Claude.ai/ChatGPT via
OAuth, and leave with their data deleted. Hosted mode becomes complete; local
mode is untouched.

**Scope.**

1. **Better Auth (D6).** Pin a version; install `better-auth`,
   `@better-auth/api-key` only if its key format can satisfy D7 (otherwise
   keep our `agent_tokens` and skip the plugin; record the decision),
   `@better-auth/mcp`, `@better-auth/cimd`. Mount at `/api/auth/*` via
   `toNodeHandler` in `src/app.ts` before the route table. Session cookie
   `SameSite=Lax`, `Secure` in hosted mode; keep the Origin check as CSRF
   defence for all mutating routes. `src/auth.ts` resolves a request to an
   `Actor` (session → `users.auth_id` → `user_id`), creating the `users` row
   on first sign-in (handle from provider username, sanitised and
   de-duplicated; display name from provider), or claiming user 1 per D5.
2. **Invite gate (D14).** `invites` table; hosted sign-in for a new
   `auth_id` requires a valid unused code supplied on the sign-in page;
   admins mint codes in Settings. Claiming user 1 requires the owner claim code (D5), instead of an invite.
3. **OAuth 2.1 authorization server for MCP (D7).** `@better-auth/mcp`
   serves `/.well-known/oauth-protected-resource/mcp` and
   `/.well-known/oauth-authorization-server`; implement the consent page
   (plain HTML served by the app, listing the client name and the single
   scope `deck-builder`); `src/mcp/http.ts` accepts personal tokens *or*
   JWTs verified via the JWKS endpoint with `expectedResource` audience check;
   unauthenticated requests return the `WWW-Authenticate` challenge from D7.
   Verify against the MCP authorization spec 2026-07-28 and cite it.
   Register the deck-builder's Client ID Metadata Document for CIMD clients.
4. **Hosted request pipeline** (`src/app.ts`, `src/mcp/http.ts`):
   configurable allowed origin (`DECKBUILDER_PUBLIC_ORIGIN`) replacing the
   loopback lists in both doors. Hosted GET `/api/health` alone bypasses Host/Origin
   for Railway probes, returning only `{ok,schema_version}` without cookies/auth;
   local health keeps loopback checks. Trust forwarded host/protocol and Railway
   `X-Real-IP` (fallback rightmost XFF) only when
   `DECKBUILDER_TRUST_PROXY=1`; bind `0.0.0.0` in hosted mode;
   `settings.agent.url` derived from the public origin; security headers
   (`Content-Security-Policy` allowing `cards.scryfall.io` images,
   `X-Content-Type-Options`, `Referrer-Policy`, `Strict-Transport-Security`
   in hosted mode).
5. **Rate limiting.** `src/ratelimit.ts`: sliding window per IP for
   unauthenticated routes (auth endpoints tight), per user for the UI API,
   per token for MCP; 429 with `Retry-After`; limits in one table at the top
   of the file with comments. Search and compare get stricter buckets.
6. **Account lifecycle.** `GET /api/account/export` (JSON of everything the
   user owns, plus Archidekt text per deck); `DELETE /api/account` (cascades
   decks, memories, proposals, log, activity, tokens, OAuth grants, Better
   Auth rows; writes nothing to the activity feed afterwards). Settings UI
   for both with a typed-handle confirmation.
7. **Web client session layer.** `SessionProvider` above `LiveProvider`;
   `useSession()`; sign-in page (`/signin`, provider buttons, invite code
   field in hosted mode); first-run handle picker when the provider handle
   collided; `request()` maps 401 → sign-in redirect, 403 → read-only toast,
   429 → "slow down" toast with the retry time; `EventSource` carries the
   cookie automatically. Local mode never shows the sign-in page.
8. **Connect wizard** (Settings): per client (Claude Code, Codex, Claude.ai,
   ChatGPT) the exact steps with the hosted URL, a "create token" button
   inline for CLI clients, and a live "last seen: <client label>, <ago>"
   line from `agent_tokens.last_used_at` / OAuth grant usage. Remove the
   Secure MCP Tunnel instructions (they were never run and hosting makes
   them moot).
9. **Legal pages (D13).** `/privacy` and `/terms` as static markdown
   rendered through the existing `RichText`, with the operator name and
   contact from required non-secret `DECKBUILDER_OPERATOR_NAME` and
   `DECKBUILDER_CONTACT_EMAIL`; no legal pages/footer in local mode. Footer with the Fan Content notice and
   "prices are daily estimates from Scryfall bulk data, as of <date>".
   Content covers what is stored, why, retention, third parties (Railway,
   Scryfall images fetched by the browser), cookies, deletion and export,
   and the no-warranty clause.
10. **Admin surface.** `is_admin` users see an Admin card in Settings: job
    status and manual enqueue (from phase 0), invite codes, user count,
    database size. No user impersonation.

**Out of scope.** Public deck pages, visibility, path routing, renames,
suggestions.

**Acceptance.**
- Claim tests cover success, wrong code, already claimed and unset.
- `npm run check` green; hosted-mode HTTP tests: unauthenticated UI API →
  401; session cookie → 200 scoped to that user; invite required; user-1
  claim path; personal token and a locally minted JWT both reach a tool as
  the right user; PRM and AS metadata endpoints return spec-shaped JSON;
  rate limit returns 429 with `Retry-After`; export contains every table the
  user touches; delete leaves zero rows for that user in every table.
- Manual, recorded in the PR: Claude Code connects with a personal token;
  `claude mcp add` OAuth flow (`/mcp` → login) completes against a local
  hosted-mode instance behind `DECKBUILDER_PUBLIC_ORIGIN=http://localhost:8787`.

**Docs.** Spec §10.1 (both credential kinds, challenge), §17 (sign-in,
claim, invites, deletion/export, admin); AGENTS.md (dependencies, modes,
env vars); OPERATIONS.md (secrets list, how to mint an invite).

---

### Phase 3 — Deploy the private beta on Railway (size: L)

**Goal.** The hosted app runs on Railway with the owner's real data, daily
refresh, nightly backups, a rehearsed restore, logs and alerts. Only the owner
and invited testers can sign in.

**Scope.**

1. **Container.** `packages/deck-builder/Dockerfile` (`node:26-slim`,
   `npm ci` for the workspace from the repo-root lockfile, `vite build`,
   `node src/server.ts`, non-root user, `HEALTHCHECK` on `/api/health`).
   `.dockerignore`. Build must work from the repo root as context because of
   the workspace lockfile; document the exact command.
2. **Railway project** via the CLI (`railway init --name deck-builder`, add
   service from the GitHub repo `joCaldwell/mtg-ecosystem` with the Dockerfile
   path and watch paths limited to `packages/deck-builder/**`), environments
   `production` and `staging`, a 5 GB volume mounted at `/data` in each,
   variables (`DECKBUILDER_MODE=hosted`, `DECKBUILDER_DATA=/data`,
   `DECKBUILDER_PUBLIC_ORIGIN`, `DECKBUILDER_TRUST_PROXY=1`, auth secrets,
   `BACKUP_S3_*` from a Railway bucket created with `railway bucket`),
   Railway volume backups enabled (daily + weekly), a workspace soft
   spending alert at $25. Prefer Railway config-as-code (`railway config`
   / `.railway/railway.ts`) so the setup is reviewable in the repo; verify
   the IaC path exists on the installed CLI and fall back to documented CLI
   commands if not.
3. **Structured logs.** One JSON line per request and per tool call
   (`ts, level, req_id, user_id, via, label, method, path|tool, status,
   ms`), errors with stack; no secrets, no full bodies. `railway logs --json`
   must be greppable by `user_id`.
4. **Data migration.** Upload the owner's current database to the production
   volume (`railway ssh` or a one-off `restore` through the backup bucket),
   boot, confirm user-1 claim on first Discord sign-in, confirm decks,
   memories and tokens survived. Record the steps in OPERATIONS.md as the
   "move an instance" runbook.
5. **Restore drill.** Take a backup from production, restore it into staging,
   open it, count decks, serve it. Record timing and the exact commands.
6. **Uptime and errors.** `/api/health` polled by an external monitor (the
   agent sets up UptimeRobot only if credentials are provided; otherwise
   documents the step). Optional Sentry: behind `SENTRY_DSN`, no dependency
   unless set (use the HTTP envelope endpoint directly or skip; do not add
   the SDK by default).
7. **Deploy verification.** Follow the Railway skill's rule: observe
   `SUCCESS` for the specific deployment; smoke test sign-in, `start_session`
   via a personal token from the owner's machine, SSE reconnect, a refresh
   job run on the server (watch RSS), and a backup landing in the bucket.
8. **Domain.** Railway domain for the beta; if the owner supplied a custom
   domain, add it and update `DECKBUILDER_PUBLIC_ORIGIN` and the Discord
   redirect list.

**Out of scope.** Any product change. Public pages.

**Acceptance.** Production and staging deployments at `SUCCESS`; health
green; the owner signed in and sees their decks; a refresh ran on the server
without the service restarting; a backup object exists in the bucket; the
restore drill is written up; `railway usage projects --project deck-builder`
is quoted in the PR.

**Docs.** OPERATIONS.md (full runbook: deploy, roll back with
`railway down`/redeploy, rotate secrets, mint invites, restore, move
instance, spending alert); AGENTS.md pointer; spec §15 "verified at build
time" gains the hosted checks.

---

### Phase 4a — Public surface: visibility, public pages, sharing, forks (size: XL)

**Goal.** A deck can be shared by link or published; anyone can read it
without an account; a signed-in user can fork it or compare against it; the
page looks right on a phone and unfurls in Discord. On a phone, the owner reads
their own decks and rules on their agents' proposals (D17). Decks import from
Moxfield as well as Archidekt.

**Scope.**

1. **Schema and capability (D8).** Visibility, slug, `publish_decisions`,
   `accept_suggestions` (unused until phase 5), `log.hidden`. Owner-only
   `setVisibility`, `hideLogEntry`. `requireDeck` admits viewers per
   visibility; viewer reads return a trimmed `PublicDeckState` (new type in
   `src/state.ts`) that contains exactly the D8 list and nothing else; the
   permission matrix test gains viewer rows (private → 404, unlisted/public →
   trimmed state, never memories/proposals/activity). MCP tools: `get_deck`
   on a slug the caller can view returns the public rendering with a header
   line "Public deck owned by <handle>; read-only"; `compare_decks` accepts a
   slug as the other side; `search` with `include_my_decks` is unaffected.
2. **Cross-owner fork.** `forkDeck` accepts a readable deck of another user;
   copies per D8; records `parent_deck_id` plus `parent_owner_handle` and
   `parent_slug` snapshot (so lineage survives the parent going private or
   being deleted); the public page shows "forked from <handle>/<name>";
   `compareDecks` against a parent the caller can still read.
3. **Path routing (D16).** Replace the hash router with path routes
   (`/`, `/decks`, `/deck/:id`, `/deck/:id/review`, `/deck/:id/playtest`,
   `/inbox`, `/memories`, `/profile`, `/activity`, `/settings`, `/design`,
   `/signin`, `/privacy`, `/terms`, `/d/:slug`, `/u/:handle`). `href()`
   remains the single place URLs are built. Server fallback per D16. For
   `/d/:slug` and `/u/:handle` the server injects `<title>`, `<meta
   name="description">` and Open Graph tags (deck name, commander, colour
   identity, bracket, owner handle; image = commander art crop from D10)
   into `index.html` before serving.
4. **Public deck page** (`web/src/features/public/`): a composition of the
   existing `DeckList`, `GroupBlock`, `CardRow` (wrapped in read-only
   `DeckProvider`/`DeckUiProvider`), `ChecksPanel`, read-only `BriefField`s,
   the power panel's read-only branch, price estimate with store and date
   (D18), the decisions log when published (hidden entries omitted), owner
   byline linking to `/u/:handle`, and a header carrying the commander's art
   crop with its artist line (D20). Actions: Fork (signed-in), Compare with
   one of my decks (signed-in), Export (Archidekt text, no account needed),
   Copy link. No sidebar, no inbox, no focus reporting, no ruling hotkeys.
   Fully responsive: single column under 720 px, list rows wrap, hover card
   becomes a tap-to-open sheet, tap targets at least 44 px. Build the read-only
   composition as one component (`ReadOnlyDeck`) so the phone view (item 9)
   reuses it rather than copying it.
5. **Share dialog** in the deck header: visibility segmented control with
   plain-language consequences, publish-decisions toggle with the per-entry
   hide affordance in the History panel, copy link, "what viewers see"
   preview link. Visibility state in `DeckState`; sidebar and decks list show
   a small public/unlisted glyph.
6. **Public profile** `/u/:handle`: display name, public decks table (reuse
   `DecksPage` table), nothing else. Owner controls nothing here yet beyond
   which decks are public.
7. **Card search without an account.** `/api/search`, `/api/resolve`,
   `/api/cards/:id/table` and the hover lookup work unauthenticated in hosted
   mode under the unauthenticated rate bucket (D13).
8. **Agent-facing.** `start_session` lists the user's public decks count;
   playbook gains a paragraph on public decks and forks; `tools.test.ts`
   updated deliberately.
9. **Phone view for the owner (D17).** Below 720 px the signed-in app
   renders a phone shell (`web/src/features/phone/`, or a branch of the app
   shell): a top bar, the deck list, and the inbox. A deck opens in
   `ReadOnlyDeck` (item 4) plus its pending proposals, drawn in place as on
   desktop, with accept/reject per item, per group and all at once, each
   taking the optional reason. Document (brief/profile) proposals rule from the
   inbox as diffs. Ruling calls the existing capabilities through the
   existing API, with no new routes. Direct editing, hotkeys, the playtester and
   Settings render "Open on a larger screen". The breakpoint is one constant
   shared with the public page. Live updates (SSE) work as on desktop, so a
   proposal an agent makes appears without a reload.
10. **Moxfield import.** The import dialog (spec §12) also accepts Moxfield's
   text export: the same diff preview, one log entry and one undo. Verify
   Moxfield's export format against Moxfield's own documentation, or against
   a real export when there is none. Cite it in the parser and add fixtures
   for each section header it uses (commander, sideboard/maybeboard). Paste
   only: no URL fetching, no Moxfield API, no sync. Export stays Archidekt.

**Out of scope.** Suggestions from other users (phase 5), comments, renames
and onboarding copy (4b), playgroups, editing on phones.

**Acceptance.** Permission matrix green with viewer rows; an unauthenticated
`curl` of `/d/<slug>` returns HTML with correct OG tags; a private slug 404s;
a Discord/Slack unfurl preview (manual) shows the commander art; Lighthouse
mobile layout has no horizontal scroll at 375 px on the public page or the
phone view; at 375 px the owner can open a deck, accept one proposed item
with a reason, reject another, and see an agent's new proposal arrive live
(screenshots in the PR); fork of a public deck by a second user contains no
memories; export works signed out; a real Moxfield export imports with the
same result as the equivalent Archidekt text.

**Docs.** Spec §5.3 (cross-owner forks), §11 (public pages, routing, phone
view), §12 (Moxfield import), new §18 "Sharing" (D8 verbatim), §13 (what is
guaranteed on public pages); ARCHITECTURE.md (routing, `features/public`,
`features/phone`, read-only providers); DESIGN.md (narrow screens).

---

### Phase 4b — UX overhaul for strangers (size: L; web client only)

**Goal.** Someone who has never heard of agents, proposals or briefs opens an
empty account and understands what to do; the app reads as a deck builder
first and an agent workbench second; it holds up on a laptop at 1280 px and
degrades gracefully down to 720 px, below which phones get 4a's phone view
(D17).

**Scope.**

1. **Renames (D9)** everywhere: data value `legacy` → `archived` with a
   migration (SCHEMA_VERSION bump), tool descriptions, rendered text,
   playbook, SKILL.md, UI copy, spec. "Rulings" → "decisions" in UI and
   agent text; `#/deck/:id/review` stays "Review".
2. **Agent-optional voice.** Every empty state, subtitle and help line has
   two forms: with no agent ever connected (talk about decks, briefs,
   decisions; one gentle pointer to Settings → Connect an agent) and after
   the first agent write (today's copy). Driven by a `has_agent_activity`
   flag in `/api/settings`. Sidebar presence block appears only in the second
   state.
3. **First run.** New-account landing: three cards (Create a deck, Import
   from Archidekt, Try the sample deck); "Try the sample deck" creates the
   seed demo deck for *this* user through the capability layer (move
   `scripts/seed-demo.mjs` logic into `src/demo.ts`, callable by route).
   Inline explainers (one sentence plus "learn more" popover) at the point of
   use for: lifecycle control, slots vs tags, brief, bracket, proposals
   banner, memories page header.
4. **Hide the plumbing:** secondary slots, memory budgets, review-queue
   flags, ownership evidence and revision chips move behind an "Advanced"
   disclosure in their panels; defaults unchanged for existing users
   (persist the disclosure in `user_settings`).
5. **Tablet shell (720 px and up; D17):** viewport meta; sidebar collapses
   to a top bar with a drawer under 960 px; deck page stacks list over panel
   under 1100 px with the panel as a bottom sheet; tables scroll
   horizontally inside their container instead of the page. The full editor
   is not made to work below 720 px; that width belongs to the phone view.
   Playtest keeps its own rules and stays desktop-only.
6. **Accessibility:** raise `--faint` to ≥ 4.5:1 at 12 px or stop using it
   below `--text-sm`; global `prefers-reduced-motion` rule zeroing `--fast`
   and disabling transitions; `MultiSelect` as `listbox` +
   `aria-multiselectable`; hover card reachable by keyboard (`?` on a focused
   row) and touch (long-press/tap on the mana cost); read-only title no
   longer a button; skip link; `main` landmark label.
7. **Performance:** `React.lazy` for playtest, styleguide, public pages and
   settings; `manualChunks` for react; self-host Inter (woff2, `font-display:
   swap`) or remove it from the stack; `listActivity` in the sidebar replaced
   by a `GET /api/activity/latest` returning one row; `MemoriesPage` queries
   consolidated; activity and memories pages paginate at 100.
8. **Design system:** the two states of every changed primitive appear in
   `#/design` → `/design`; `web-architecture.test.ts` gains a rule that
   components do not call `fetch` (closes the `MtgExamples.tsx` gap).

**Out of scope.** Backend behaviour beyond the `archived` migration, the
`has_agent_activity` flag, demo route and `activity/latest`.

**Acceptance.** `npm run check` green; `tools.test.ts` and transcript check
re-run after renames; bundle gzip for the public page route under 90 KB JS;
axe (or equivalent) reports no contrast or role violations on decks, deck,
inbox, public page; manual narrow-width pass recorded with screenshots in
the PR, at 768, 1024 and 1280 for the full app and at 375 for the phone view.

**Docs.** Spec §5.6 (archived), §8 (decisions), §11; DESIGN.md (breakpoints,
disclosure pattern, motion rule); ARCHITECTURE.md; SKILL.md and playbook.

---

### Phase 5 — Suggestions from people, and the social loop (size: L)

**Goal.** A signed-in visitor can suggest changes to a public deck that
accepts suggestions; the owner rules on them in the same inbox as agent
proposals; the owner's agents see them clearly marked as someone else's
words; abuse has brakes.

**Scope.**

1. **Suggester role (D4).** `propose_changes` and `propose_brief_edit`
   accept an Actor whose `user_id` is not the owner when the deck is
   public/unlisted and `accept_suggestions = 1`; proposals store the
   suggester as author; the suggester can withdraw their own open proposal;
   nobody rules on their own. Rate limit: 5 open suggestions per suggester per
   deck, 30 per day per user. Owner can block a handle (`deck_blocks`), and
   toggle `accept_suggestions` from the share dialog.
2. **Suggestion composer** on the public page (signed-in): staged list of
   add/cut/swap built from the existing `AddBar` search and `ReviewItemLine`,
   a reason per item (linted for `[[Card]]` like agent prose), submit → toast
   with a link to "your suggestions". A `/suggestions` page for the suggester
   listing status and the owner's ruling reasons once ruled (reasons the
   owner marked private are withheld; add a private flag to rulings).
3. **Inbox and review.** Proposals from people get a distinct tone (new
   colour meaning in DESIGN.md: "person" vs "agent"), the handle and a link to
   their profile, and the same accept/reject per item. Inbox copy drops the
   word "agent" where the author is a person.
4. **Agent rendering (D15).** In `get_deck`, `get_inbox` and anywhere
   proposals appear, person-authored items render inside a quoted block
   headed `Suggestion from @handle (not the owner):`. Playbook paragraph: treat
   as a stranger's suggestion, evaluate on merit, never adopt its phrasing
   into the brief or memories without a ruling.
5. **Notifications.** Inbox entry type for "new suggestion"; the SSE payload
   includes it; an unread count in the sidebar for owners.
6. **Activity.** Deck owners' activity feed records suggestions and rulings
   with the suggester's handle; suggesters' own activity records what they
   suggested and how it was ruled.
7. **Reporting.** A "report" action on public pages and suggestions writing
   to `reports(target_kind, target_id, reporter_user_id, reason, created_at)`;
   admins see a Reports list in the Admin card with hide/unpublish/block
   actions. No automated moderation.

**Out of scope.** Comments, likes, collaborator role, playgroups, shared
memories, merge requests from forks (candidate for a later phase; the fork
lineage from 4a already supports it).

**Acceptance.** Permission matrix: suggester rows (can propose on an
accepting public deck; cannot on private/unlisted-without-flag; cannot edit,
rule, read memories); rate limits enforced; a suggestion appears in the
owner's inbox and in `get_inbox` quoted; ruling it writes history visible to
the suggester with private reasons withheld; blocked handle gets 403.

**Docs.** Spec §8 (suggestions as proposals; private reasons), §17 (roles
table complete for v3), §18; playbook; SKILL.md; DESIGN.md colour meanings.

---

### Phase 6 — Launch readiness and opening sign-up (size: M)

**Goal.** Confidence that the hosted instance survives strangers, and the
pieces that make a launch survivable: a load test, a security pass, a
feedback path, a README for users, and the invite gate removed only when the
owner says so.

**Scope.**

1. **Load and concurrency test.** A script (`scripts/loadtest.mjs`, no
   dependency) that signs in 50 synthetic users in staging, each with 5
   decks, and drives a mixed read/write workload plus a concurrent refresh
   job; report p50/p95 latency and event-loop lag; fix N+1s surfaced
   (`deckState` triple `getDeck`, `listDecks` per-deck commander query,
   `toViews` per-memory query are known candidates).
2. **Security pass.** Re-run the review checklist from the readiness
   document §2 against staging: headers, CSRF with cookies, IDOR across the
   matrix via HTTP (not just service calls), rate limits, OAuth flows with a
   wrong `resource`, SSE as another user, static path probing, body limits.
   Fix what fails; record the pass.
3. **Operations.** Restore drill repeated from a bucket object; alert on
   failed jobs (admin card shows red; optional email via Railway webhook if
   configured); spending limit set; a weekly `npm audit` reminder in
   OPERATIONS.md.
4. **User-facing docs.** `packages/deck-builder/README.md` for strangers
   (what it is, how to connect each agent, what is public, how to delete
   your account), linked from the sign-in page and footer. A short
   changelog page.
5. **Feedback path.** A "Feedback" link to a GitHub Discussions category or
   a form URL the owner provides; otherwise mailto the contact address.
6. **Privacy-respecting counts.** Admin card shows total users, decks,
   public decks, suggestions, tool calls per day from the structured logs or
   a daily aggregate table; no third-party analytics.
7. **Open sign-up.** Behind `DECKBUILDER_INVITES_REQUIRED=0`; the owner
   flips it. Public landing page copy and a sample public deck of the
   owner's choosing featured on `/`.

**Acceptance.** Load test report in the PR with numbers before/after fixes;
security pass checklist all green; README reviewed against the actual
hosted flow by the agent performing each client's connect steps once more;
the flag exists and defaults to invites required.

**Docs.** OPERATIONS.md, README.md, spec §15/§16 brought current (what is
verified, what is not), AGENTS.md status line (v3 hosted).

---

## 4. Canonical shapes (so phases agree)

**Env vars (final set):** `DECKBUILDER_MODE`, `DECKBUILDER_PORT`,
`DECKBUILDER_DATA`, `DECKBUILDER_DB`, `DECKBUILDER_TOKEN_FILE` (stdio only),
`DECKBUILDER_PUBLIC_ORIGIN`,
`DECKBUILDER_TRUST_PROXY`, `DECKBUILDER_INVITES_REQUIRED`,
`DECKBUILDER_CLAIM_CODE` (secret, at least 32 characters; remove after claiming),
`DECKBUILDER_OPERATOR_NAME`, `DECKBUILDER_CONTACT_EMAIL` (required non-secret hosted legal identity),
`BETTER_AUTH_SECRET`, `DISCORD_CLIENT_ID`, `DISCORD_CLIENT_SECRET`,
`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `BACKUP_S3_ENDPOINT`,
`BACKUP_S3_BUCKET`, `BACKUP_S3_ACCESS_KEY`, `BACKUP_S3_SECRET_KEY`,
`SENTRY_DSN` (optional). `DECKBUILDER_TOKEN` is removed in phase 1 (tokens
live in the database).

**Error statuses:** 400 malformed; 401 no session/token (hosted); 403
authenticated but forbidden on a readable target (blocked, rate-limited
suggesters); 404 deck not readable by this actor (never 403, to avoid
existence leaks); 409 revision conflicts; 413 body too large; 429 rate
limited with `Retry-After`.

**Author columns:** `author_user_id INTEGER NOT NULL REFERENCES users(id)`,
`author_via TEXT NOT NULL CHECK (author_via IN ('ui','agent'))`,
`author_label TEXT NOT NULL`. Rendering: owner via ui → the owner's handle;
owner via agent → `handle via label`; other user → `@handle`.

**Public deck state (`PublicDeckState`):** `deck { name, slug, owner
{ handle, display_name }, commander, color_identity, category, visibility,
parent? { owner_handle, slug, name } }`, `cards`, `slots`, `tags`, `brief`,
`engines`, `checks`, `power { target, latest_owner_assessment? }`,
`price { total_cents, currency, store, unpriced, as_of, source: "Scryfall" }`
(D18),
`decisions?: LogEntry[]` (only when published; hidden omitted), `computed`.

**Permission matrix dimensions:** actor ∈ {owner-ui, owner-agent,
other-ui, other-agent, anonymous} × deck ∈ {private, unlisted, public,
public+suggestions, archived} × capability ∈ {every exported service
function, every MCP tool, every HTTP route}. The test file enumerates the
first two; capabilities are discovered from exports and must be classified
(`read`, `write`, `owner-only`, `suggest`, `public-read`) in a map the test
owns, so an unclassified new export fails.

---

## 5. Open items deliberately deferred

Collaborator and co-owner roles; merge requests from forks back to the
parent; shared playtest replays; playgroups and shared memories; comments;
email sign-in; paid anything; a native mobile app, and editing on phones
beyond ruling (D17); price sources beyond Scryfall's data, and exchange
rates (D18); Moxfield export and URL import; the ChatGPT tunnel path; other
formats; printings and collection. Each is a candidate for a later phase and
none blocks launch.
