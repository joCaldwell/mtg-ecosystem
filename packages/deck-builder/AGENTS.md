# 🃏 deck-builder — a Commander deck builder that agents drive from outside

> **Status: ACTIVE.** Built against
> [docs/deck-builder-spec.md](../../docs/deck-builder-spec.md) (2026-10-10).
> Section numbers in code comments (§5.2 draft/active, §7 memories, §8
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
    tools call the same service functions, each passing an `Actor` (`OWNER`,
    or the calling agent). Every permission is enforced *below* both doors.
    Nothing in `src/mcp/` or `src/app.ts` decides what an agent may do.
2.  **The owner rules on membership of active decks.** A deck is `draft` or
    `active`. An agent edits a draft's cards directly and changes an active
    deck's only through a proposal the owner accepts. The single enforcement
    point is `applyChanges` in `src/deck/membership.ts`.
3.  **No invented card anywhere in the app.** Card references resolve exactly
    (`src/cards.ts`) or the whole call fails with suggestions; prose an agent
    stores is linted for `[[Card Name]]` on write. What an agent says in its
    own chat window is outside the app and is not covered.
4.  **If the app can compute it, nobody else derives it.** Counts, deltas and
    checks are returned with every read and every write.
5.  **Everything an agent writes is attributed and visible.** Unapproved
    writes (memories, filing, draft edits) land in the activity feed.

---

## 🏗️ Stack and layout

Node runs the `.ts` files directly via type stripping — **there is no build
step for `src/`**. `node:sqlite` (built in, has FTS5) means no native deps.
Runtime dependencies: React, `@modelcontextprotocol/server` (the official MCP
TypeScript SDK, v2 line) and `zod`. Keep it to those.

```
src/
  errors.ts          AppError base — every user-facing failure carries its HTTP status
  actor.ts           Actor (owner | agent), requireOwner, PermissionError
  db.ts              schema, withTransaction (savepoints — they nest), settings
  cards.ts           ⭐ exact card resolution, suggestions, prose linting
  ingest.ts          Scryfall bulk → SQLite            search/  Scryfall-syntax subset
  prices.ts          the cheapest playable printing of each card (spec §4.4)
  refresh.ts         refreshCardData(): cards, then prices — the CLI and Settings both call it
  deck/
    service.ts       decks (draft/active, parent), slots, tags, maybe list, computed state
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
  stats.ts           player statistics across Paper decks only
  activity.ts        the activity feed          focus.ts   what the owner has open
  token.ts           the agent bearer token
  state.ts           deckState() — the composite the UI gets from every mutation
  app.ts             createApp(): UI API (as owner), event stream, static files, /mcp
  server.ts          entry point: binds 127.0.0.1
  mcp/
    playbook.ts      INSTRUCTIONS (short) and PLAYBOOK (returned by start_session)
    render.ts        text for agents — compact, grouped, counts stated
    tools.ts         the 26 tools: thin adapters over the capability layer
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
| `npm run refresh` | Bring card data up to date from Scryfall: every card, then every card's price. `--force` re-downloads even if nothing is newer |
| `npm run search '<query>'` | Scryfall-syntax search from the CLI |
| `npm run mcp` | The stdio shim, for clients that only launch a command |

`DECKBUILDER_PORT`, `DECKBUILDER_DB`, `DECKBUILDER_DATA` and `DECKBUILDER_TOKEN`
override the port, database file, data folder and agent token.

### Connecting an agent

The Settings page shows the endpoint, the token, and ready-to-paste
configuration. For Claude Code:

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
    owner-only, call `requireOwner` inside it. Never guard it only in a tool
    or a route. Owner-only today: ruling, undo, draft/active,
    importing over a deck, editing the profile directly.
    Owners and agents may rename, delete and classify decks (`paper` /
    `theorycrafted` / `legacy`, spec §5.6). Agent actions are attributed in
    Activity. Category is independent of draft/active and card membership.
*   **A deck's contents change only through `applyChanges`.** It applies a
    batch atomically, bumps the revision once, and writes one log entry. Do
    not add a second write path to `deck_cards` for membership.
*   **Cross-deck membership in search is off** unless the call sets
    `include_my_decks`.
*   **Card facts come from the database, never from model memory.** This
    applies to you: do not type a card name into code, a fixture, or a seed
    without checking it resolves.
*   **A price is the cheapest playable printing's, in whole cents, and is an
    estimate.** Decks hold cards, not printings, so nothing may present a
    price as what the owner's copy is worth. Totals come from `estimateCost`
    and always report how many cards had no price.
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
    the UI API as well, which has no token.
*   **No model calls, including embeddings.** Memory search is FTS5 plus
    scope and card links.
*   **External formats get verified against their own documentation first.**
    The Archidekt research is cited in `src/deck/interop.ts`; the MCP
    behaviour in `src/mcp/http.ts`.
*   **Slots are optional.** Nothing — proposals included — may require one.
*   **Slot and card type are independent axes.** `validateVocabName` rejects
    card types as slot or tag names in singular and plural.
*   **Tool text is an interface.** `tools.test.ts` pins the wording agents see.
    Change a description or a rendered line deliberately, then read a real
    transcript (spec §14, phase 6) before trusting it.
