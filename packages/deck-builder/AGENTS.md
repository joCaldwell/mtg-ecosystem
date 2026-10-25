# 🃏 deck-builder — a Commander deck builder that agents drive from outside

> **Status: ACTIVE.** Built against
> [docs/deck-builder-spec.md](../../docs/deck-builder-spec.md) (2026-10-10).
> Section numbers in code comments (§5.2 lifecycle, §7 memories, §8
> rulings, §10 agent interface) refer to that document.

A localhost app for building Commander decks, designed to be used by a person
through its UI and by external AI agents through an MCP endpoint. **The app
makes no AI model calls.** It stores structured data, validates, computes, and
keeps an auditable record; interpretation and conversation happen in whatever
agent the owner connects.

**Read the spec before changing behaviour.** It is the source of truth for
design intent.

---

## 🎯 The rules that explain most of the design

1.  **One capability layer, two front doors.** The UI's HTTP API and the MCP
    tools call the same service functions, each passing an `Actor` with a
    `user_id` principal, `via` (`ui` or `agent`) and attribution `label`.
    Ownership is a relation to the deck, never a kind of actor. Every
    permission and tenant filter is enforced *below* both doors.
    Nothing in `src/mcp/` or `src/app.ts` decides what an agent may do.
2.  **Lifecycle controls editing.** Agents edit Theorycrafted cards directly;
    Paper card changes and all brief/power-target edits require proposals.
    Legacy is read-only for both actors; the owner alone changes lifecycle or
    restores an archive. Membership is enforced by `applyChanges`; all deck
    mutators use `requireEditableDeck` in `src/deck/service.ts`.
3.  **No invented card anywhere in the app.** Card references resolve exactly
    (`src/cards.ts`) or the whole call fails with suggestions; prose an agent
    stores is linted for `[[Card Name]]` on write. What an agent says in its
    own chat window is outside the app and is not covered.
4.  **If the app can compute it, nobody else derives it.** Counts, deltas and
    checks are returned with every read and every write.
5.  **Everything an agent writes is attributed and visible to its principal.**
    Store the author user, via and label beside the compact author string;
    UI attribution reads `handle via label`. Unapproved writes (memories,
    filing, Theorycrafted card edits) land in that user’s activity feed.
    A URL label grants no identity or UI privileges. Pending card/document
    proposals record `author_token_id`; amendment rights follow the verified
    credential, never its URL hint. Legacy import binds existing agent drafts
    to the migrated credential so outstanding local work stays editable.

---

## 🏗️ Stack and layout

Use Node 26 (`nvm use`; the package floor is >=24.12). Both tsconfigs enforce
`erasableSyntaxOnly`. Node runs the `.ts` files directly via type stripping — **there is no build
step for `src/`**. `node:sqlite` (built in, has FTS5) means no native deps. Use `Database`
from `src/sqlite.ts` (Node 26 rename; the same constructor on Node 24).
Connections have a 5-second busy timeout.
Runtime dependencies: React, `@modelcontextprotocol/server` (the official MCP
TypeScript SDK, v2 line) and `zod`. `@aws-sdk/client-s3` is the ops-only
exception for optional S3-compatible backup uploads; it is loaded only when
configured. Keep product dependencies to the original set.

```
src/
  errors.ts          AppError base — every user-facing failure carries its HTTP status
  actor.ts           Actor with principal; requireDeckOwner, requireSelf, requireUi, PermissionError
  db.ts              ordered migrations through v11, nested savepoints, per-user settings
  cards.ts           ⭐ exact card resolution, suggestions, prose linting
  ingest.ts          Scryfall bulk → SQLite            search/  Scryfall-syntax subset
  prices.ts          the cheapest playable printing of each card (spec §4.4)
  refresh.ts         refreshCardData(): streamed cards, then staged price swap — CLI child only
  jobs.ts            daily/stale-boot refresh and nightly backup scheduler; shared SQLite lease
  backups.ts         online SQLite backup, optional S3 upload, validated offline restore
  json-array.ts      incremental oracle JSON-array reader
  deck/
    service.ts       decks (lifecycle, parent), slots, tags, maybe list, computed state
    membership.ts    ⭐ applyChanges — the one place a deck's contents change; undo
    log.ts           the rulings log, per-card history, the review queue
    proposals.ts     proposals; accept/reject per item, group or whole proposal
    brief.ts         thesis / constraints / engines (direct writes)
    power.ts         target, revision-bound assessments, explained bracket suggestions
    checks.ts        deterministic checks
    fork.ts          forkDeck, compareDecks
    interop.ts       Archidekt export/import
  documents.ts       profile + proposed edits to profile and brief, and their rulings
  memory.ts          ⭐ memories: scopes, card links, budgets, supersede/retire
  playtest/          owner-only manual session service and action model; snapshot, undo, resume
  stats.ts           player statistics across Paper decks only
  activity.ts        the activity feed          focus.ts   what the owner has open
  tokens.ts          per-user hashed personal credentials; create/list/revoke/verify, legacy import
  mode.ts            local/hosted mode, local actor, Host/Origin boundary
  state.ts           deckState() — the composite the UI gets from every mutation
  app.ts             request-to-Actor seam, UI API, per-user event streams, static files, /mcp
  server.ts          entry point: loopback in local mode, all interfaces in hosted mode
  mcp/
    playbook.ts      INSTRUCTIONS (short) and PLAYBOOK (returned by start_session)
    render.ts        text for agents — compact, grouped, counts stated
    tools.ts         the 30 tools: thin adapters over the capability layer
    http.ts          Streamable HTTP endpoint: Host/Origin checks, token, attribution
    stdio.ts         stdio shim: a pipe to the running app
skill/commander-deck-building/SKILL.md   method for agents (optional Agent Skill)
web/                 React UI. web/ARCHITECTURE.md is its manual; web/DESIGN.md the design system
  src/design/        tokens.css, base.css — the only place raw colours and sizes exist
  src/ui/            generic primitives (Button, Tabs, Dialog, Toast, useHotkeys…)
  src/mtg/           how a card is shown (ManaCost, CardRef + hover card, RichText)
  src/domain/        pure functions over wire types (grouping, review, marks) — tested in test/web/
  src/api/           client.ts (typed client — wire types imported from src/, erased at build),
                     live.tsx (event stream), useQuery.ts
  src/app/           routes, sidebar shell, app-wide providers
  src/features/      one folder per screen: deck/, inbox/, rulings/, memories/, activity/,
                     decks/, profile/, settings/, styleguide/
```

---

## 🚀 Commands

Run from this directory (`packages/deck-builder/`):

| Command | What it does |
| --- | --- |
| `npm run launch` | Start the app if needed and open it in the default browser |
| `npm start` | Build the UI and start the server in the background at :8787 |
| `npm stop` / `npm run restart` | Stop it / rebuild and restart it |
| `npm run status` / `npm run logs` | Is it up / last log lines (`-f` follows) |
| `npm run dev` | **Frontend loop**: API on :8787 + Vite HMR on :5173. Use :5173 |
| `npm run check` | Typecheck (src + web) then test — the gate before committing |
| `npm test` | Tests only — `node --test`, **not** Vitest |
| `npm run seed:demo` | Create/replace the demo deck and its fork, which exercise every panel |
| `npm run refresh` | CLI job: stream cards, then prices. `-- --force` bypasses freshness; Settings only enqueues a child job |
| `npm run backup` | Consistent online SQLite snapshot, keep 14 days, optional S3 upload |
| `npm run restore -- <file>` | Verify and restore a snapshot while the server is stopped |
| `npm run search '<query>'` | Scryfall-syntax search from the CLI |
| `npm run mcp` | The stdio shim, for clients that only launch a command |

`DECKBUILDER_MODE` defaults to `local`: loopback, implicit UI user 1, no login.
`hosted` binds all interfaces and requires `DECKBUILDER_PUBLIC_ORIGIN`; UI
APIs return 401 until Phase 2 supplies sessions. `DECKBUILDER_PORT`,
`DECKBUILDER_DB` and `DECKBUILDER_DATA` override port, database and data folder.
`DECKBUILDER_TOKEN` is removed; credentials live hashed in `agent_tokens`.

See [OPERATIONS.md](OPERATIONS.md) for scheduled jobs, backup/restore drills,
job/process locks and environment variables. Never copy a live `.db` alone.

### Connecting an agent

The Settings page shows the endpoint and token list, creates labelled tokens
(shown once), and revokes them. Save a new token before dismissing it;
configuration snippets use a placeholder until creation. For Claude Code:

```
claude mcp add --transport http deck-builder http://127.0.0.1:8787/mcp/claude-code \
  --header "Authorization: Bearer <token from Settings>"
```

For Codex (its CLI, desktop app and IDE extension share `~/.codex/config.toml`):

```toml
[mcp_servers.deck-builder]
url = "http://127.0.0.1:8787/mcp/codex"
http_headers = { Authorization = "Bearer <token from Settings>" }
default_tools_approval_mode = "approve"
```

The name after `/mcp/` is a label for attribution in the activity feed.
ChatGPT on the web cannot reach a loopback address; the Settings page describes
the route through OpenAI's Secure MCP Tunnel and the stdio shim.

### Doing frontend work

Run `npm run dev`, then `npm run seed:demo` once, then open
`http://localhost:5173/#/deck/<id>`. **Seed the demo before judging any
layout** — against an empty deck most panels render nothing.

**Read [web/ARCHITECTURE.md](web/ARCHITECTURE.md) first.** The UI is layered
(`design` → `ui` → `mtg` → `domain` → `api` → `app` → `features`) with
one-way imports, and `test/web-architecture.test.ts` fails the build on an
import that goes the wrong way or a hex colour in a component's CSS. Every
primitive is shown in every state at `#/design`; a new screen is assembled
from those, so it matches without new CSS for the basics. The rules behind
the look are in [web/DESIGN.md](web/DESIGN.md).

`DECKBUILDER_PORT` and `DECKBUILDER_WEB_PORT` move the two dev servers; the
Vite proxy follows the API port.

---

## ⚠️ Constraints — these are load-bearing, not style

*   **Permissions live in the capability layer.** If a new capability must be
    deck-owner-only, call `requireDeckOwner(db, actor, deckId)` inside it,
    or `requireSelf` for a user-scoped record. `requireEditableDeck` includes
    the ownership check. Reads use `requireDeck(db, actor, deckId)` and return
    the caller role; unreadable decks are 404, foreign writes PermissionError
    (403). Never guard only in a tool or route. Human-only actions additionally
    use `requireUi`: ruling, undo, lifecycle, import, direct brief/target/profile
    edits. An agent acting for the deck owner still needs Paper proposals.
    Owners and agents may rename and delete editable decks. Only the owner
    classifies decks (`paper` / `theorycrafted` / `legacy`, spec §5.6), since
    lifecycle governs permissions. Agent actions are attributed in Activity.
*   **A deck's contents change only through `applyChanges`.** It applies a
    batch atomically, bumps the revision once, and writes one log entry. Do
    not add a second write path to `deck_cards` for membership.
*   **Every tenant capability takes an explicit Actor.** Global card facts
    are shared; profile, global memories, settings/budgets, focus, stats, inbox,
    review queue, activity and ownership evidence are per principal. Never
    provide a default actor in a service. A parent/fork source must belong to
    the caller; lineage stops at unreadable boundaries.
*   **Cross-deck membership in search is off** unless the call sets
    `include_my_decks`; that always means the principal’s own decks.
*   **Card facts come from the database, never from model memory.** This
    applies to you: do not type a card name into code, a fixture, or a seed
    without checking it resolves.
*   **A price is the cheapest playable printing's, in whole cents, and is an
    estimate.** Decks hold cards, not printings, so nothing may present a
    price as what the owner's copy is worth. Totals come from `estimateCost`
    and always report how many cards had no price.
*   **Retired cards stay resolvable.** Refresh never deletes a missing oracle ID.
    Hide it from general search, retain it in its deck context and flag its check.
    Images come from stored bulk URLs; never construct a Scryfall API image lookup.
*   **Exact name resolution only — never fuzzy.** A full-name match wins over
    a face-name match; what is still ambiguous needs an `oracle_id`.
*   **Agent prose is linted on write.** Reasons, memory text, brief and
    profile edits, shortlist notes: every `[[Card Name]]` must be a real card.
*   **Agents never delete individual memories.** Memories are superseded or
    retired and stay on disk. An agent cannot change a memory the owner wrote
    or edited (`owner_touched`). Deleting a deck, when requested, also deletes
    its deck-scoped memories. The `delete_deck` tool is marked destructive.
*   **The endpoint is stateless.** Every tool names its deck; nothing is kept
    between requests. Do not add per-connection state — the stdio shim and the
    2025-protocol fallback both rely on its absence.
*   **Host, Origin and token are checked in front of the SDK.** The SDK's
    handler validates none of them. `app.ts` applies the Host/Origin check to
    the UI API as well. Local UI requests build implicit user 1; hosted UI
    requests await the session resolver. MCP resolves a per-user bearer on
    every request, with revoked/deleted-user credentials rejected immediately.
*   **Personal tokens are secrets shown once.** `dbk_` + 32 random bytes,
    SHA-256 at rest, prefix lookup and timing-safe hash comparison. Label,
    last-used (updated at most once a minute) and revoke live in Settings;
    token CRUD is UI-only. Local boot commits the legacy file’s hash for user
    1 before renaming the file to `stdio-token` (0600). The local stdio shim
    defaults to that file, or takes `--token-file` / `DECKBUILDER_TOKEN_FILE`;
    reading it never creates credentials.
*   **Streams are per user.** `notify(user_id, { kind, deck_id? })` reaches
    only that principal. Global job invalidations may notify all users and
    contain no tenant payload; HTTP refresh additionally requires admin.
*   **No model calls, including embeddings.** Memory search is FTS5 plus
    scope and card links.
*   **External formats get verified against their own documentation first.**
    The Archidekt research is cited in `src/deck/interop.ts`; the MCP
    behaviour in `src/mcp/http.ts`.
*   **Slots are optional.** Nothing — proposals included — may require one.
*   **Slots describe purpose; tags track sourcing and status.** `slot_id` is
    the optional primary purpose; `secondary_slot_ids` contains additional
    purposes. Targets count only primary copies. Secondary counts and total
    coverage are separate computed fields. A card cannot carry the same slot
    in both roles. Preserve both on parking, promotion, forks and undo.
*   **Slot and card type are independent axes.** `validateVocabName` rejects
    card types as slot or tag names in singular and plural.
*   **Tool text is an interface.** `tools.test.ts` pins the wording agents see.
    Change a description or a rendered line deliberately, then read a real
    transcript (spec §14, phase 6) before trusting it.
