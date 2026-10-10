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
  deck/
    service.ts       decks (draft/active, parent), slots, tags, maybe list, paper flag, computed state
    membership.ts    ⭐ applyChanges — the one place a deck's contents change; undo
    log.ts           the rulings log, per-card history, the review queue
    proposals.ts     proposals; accept/reject per item, group or whole proposal
    brief.ts         thesis / constraints / engines (direct writes)
    checks.ts        deterministic checks
    fork.ts          forkDeck, compareDecks
    paper.ts         pull list / buy list (the only reader of in_paper for agents)
    interop.ts       Archidekt export/import
  documents.ts       profile + proposed edits to profile and brief, and their rulings
  memory.ts          ⭐ memories: scopes, card links, budgets, supersede/retire
  stats.ts           player statistics across decks
  activity.ts        the activity feed          focus.ts   what the owner has open
  token.ts           the agent bearer token
  state.ts           deckState() — the composite the UI gets from every mutation
  app.ts             createApp(): UI API (as owner), event stream, static files, /mcp
  server.ts          entry point: binds 127.0.0.1
  mcp/
    playbook.ts      INSTRUCTIONS (short) and PLAYBOOK (returned by start_session)
    render.ts        text for agents — compact, grouped, counts stated
    tools.ts         the 22 tools: thin adapters over the capability layer
    http.ts          Streamable HTTP endpoint: Host/Origin checks, token, attribution
    stdio.ts         stdio shim: a pipe to the running app
skill/commander-deck-building/SKILL.md   method for agents (optional Agent Skill)
web/src/             React UI: api.ts (typed client — wire types imported from src/, erased
                     at build), store.tsx (deck store), live.tsx (event stream), one
                     component per panel and one per page
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
| `npm run refresh` | Re-download the Scryfall oracle-cards bulk file into SQLite |
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

The name after `/mcp/` is a label for attribution in the activity feed.

### Doing frontend work

Run `npm run dev`, then `npm run seed:demo` once, then open
`http://localhost:5173/#/deck/<id>`. **Seed the demo before judging any
layout** — against an empty deck most panels render nothing.

---

## ⚠️ Constraints — these are load-bearing, not style

*   **Permissions live in the capability layer.** If a new capability must be
    owner-only, call `requireOwner` inside it. Never guard it only in a tool
    or a route. Owner-only today: ruling, undo, deleting a deck, draft/active,
    the paper flag, importing over a deck, editing the profile directly.
*   **A deck's contents change only through `applyChanges`.** It applies a
    batch atomically, bumps the revision once, and writes one log entry. Do
    not add a second write path to `deck_cards` for membership.
*   **`in_paper` reaches agents only through `paper_status`.** `render.ts`
    reads it in `renderPaper` and nowhere else; a test asserts that setting
    the flag changes no other tool's output. Cross-deck membership in search
    is likewise off unless the call sets `include_my_decks`.
*   **Card facts come from the database, never from model memory.** This
    applies to you: do not type a card name into code, a fixture, or a seed
    without checking it resolves.
*   **Exact name resolution only — never fuzzy.** A full-name match wins over
    a face-name match; what is still ambiguous needs an `oracle_id`.
*   **Agent prose is linted on write.** Reasons, memory text, brief and
    profile edits, shortlist notes: every `[[Card Name]]` must be a real card.
*   **Agents never delete.** Memories are superseded or retired and stay on
    disk. An agent cannot change a memory the owner wrote or edited
    (`owner_touched`).
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
