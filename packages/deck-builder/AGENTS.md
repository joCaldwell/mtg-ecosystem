# 🃏 deck-builder — Commander deck builder with a resident AI agent

> **Status: ACTIVE.** This is the project under development. All six phases of
> the spec are built; see the bottom of
> [docs/deck-builder-spec.md](../../docs/deck-builder-spec.md) for the phase
> list and what each one delivered.

A localhost web app for building **one Commander deck well**, with an agent
that participates in the deck's history instead of answering cold questions.

**Read [docs/deck-builder-spec.md](../../docs/deck-builder-spec.md) before
changing behaviour.** It is the source of truth for design intent, and section
numbers (§5 brief, §7 proposals, §9 interop, §10 context, §11 compaction,
§12 retention) are referenced throughout the code comments.

---

## 🎯 What makes this different from Moxfield or Archidekt

Those tools treat a deck as a list with tags. This one models **what the deck
is trying to do** — its thesis, named engines, win path — and evaluates the
list against that. Two rules follow from it and explain most of the design:

1.  **The agent cannot hallucinate a card.** It may only reference cards in
    the current decklist or in a search result from the live conversation.
    Every `[[Card Name]]` in its output is resolved exactly against the
    database before display; unresolvable names are bounced back to the model
    and never reach the screen.
2.  **The agent proposes; the owner rules.** Nothing the agent wants changes
    the deck except an accepted proposal. Rejections are typed, and the type
    routes them — a `hard_filter` rejection removes the card from future
    searches forever, a `playtest_finding` becomes a durable note. The reasons
    are the product. (The owner's own edits — manual card changes, a pasted
    Archidekt import — apply directly; the gate is aimed at the agent.)

---

## 🏗️ Stack and layout

Node runs the `.ts` files directly via type stripping — **there is no build
step for `src/`**. `node:sqlite` (built in, has FTS5) means no native deps.
Runtime dependencies are React and Vite only; keep it that way.

```
src/
  db.ts            schema, migrations, settings (retention N)
  server.ts        node:http routes; every mutation returns one composite deckState
  ingest.ts        Scryfall bulk → SQLite            search/  Scryfall-syntax subset
  deck/
    service.ts     deck CRUD, slots, tags, computed state
    proposals.ts   the approval gate, typed rejections, decision log, undo
    brief.ts       thesis/constraints/engines + gated agent edits
    audit.ts       deterministic checks (§8.1), recorded runs, dismissals
    interop.ts     Archidekt export/import, playtest notes (§9)
  agent/
    context.ts     ⭐ context assembly (§10) — the most important function here
    agent.ts       turn loop      tools.ts   agent tool defs      lint.ts  [[ref]] linting
    reasoning.ts   audit reasoning pass (§8.2)
    consolidate.ts manual compaction (§11)      meter.ts  segmented context meter
web/src/          React UI, one component per panel
```

---

## 🚀 Commands

Run from this directory (`packages/deck-builder/`):

| Command | What it does |
| --- | --- |
| `npm start` | Build the UI and start the server in the background at :8787 |
| `npm stop` / `npm run restart` | Stop it / rebuild and restart it |
| `npm run status` | Is it up, on what port, and which decks exist |
| `npm run logs` | Last 50 log lines (`-f` follows, `-n N` for more) |
| `npm run dev` | **Frontend loop**: API on :8787 + Vite HMR on :5173. Use :5173 |
| `npm run check` | Typecheck (src + web) then test — the gate before committing |
| `npm test` | Tests only — `node --test`, **not** Vitest |
| `npm run seed:demo` | Create/replace "Demo — Atraxa Superfriends", which exercises every panel |
| `npm run refresh` | Re-download the Scryfall oracle-cards bulk file into SQLite |
| `npm run search '<query>'` | Scryfall-syntax search from the CLI |

`npm start` uses a pidfile in `.dev/`, so stopping never means `pkill -f node`
and taking something unrelated down with it.

### Doing frontend work

Run `npm run dev`, then `npm run seed:demo` once, then point the browser
preview at `http://localhost:5173/#/deck/<id>`. Edits to `web/src/*.tsx` hot
reload. **Seed the demo deck before judging any layout** — against an empty
deck most panels render nothing, and the interesting states (a slot under
target *and* another over it, an atomic swap awaiting a ruling, a hard filter
from a rejected card) never appear at all.

### Configuration

`.env` in this directory, gitignored, `.env.example` provided.
`OPENROUTER_API_KEY` plus `OPENROUTER_MODEL` (currently `openai/gpt-5.6-sol`;
swap the slug to change models). Everything agent-facing goes through
OpenRouter's chat-completions API over plain `fetch` — no SDK. The transport
is injectable, so agent tests never touch the network.

---

## ⚠️ Constraints — these are load-bearing, not style

*   **The `owned` flag must never reach agent context.** It is a shopping
    concern and would bias recommendations toward what you already have.
    `src/agent/context.ts` is the chokepoint and there is a test asserting it.
*   **Card facts come from the database, never from model memory.** This
    applies to you as well: do not type a card name into code, a fixture, or a
    seed script without checking it resolves. (A seed script written from
    memory in this repo invented "Signet of the Adeptus".)
*   **Exact name resolution only — never fuzzy.** A near miss becomes a
    "did you mean" bounced back to the agent. Note that `resolveExactName`
    matches face names too, so common cards can look ambiguous; deck-list
    parsing uses `resolveListName`, which prefers a unique full-name match.
*   **External formats get verified against their own documentation first.**
    The Archidekt research is cited with sources in `src/deck/interop.ts`.
    Extend that comment rather than guessing at a new field — that file exists
    because the spec explicitly forbids relying on model memory for it.
*   **Compaction may only touch the chat transcript.** The boundary in
    `src/agent/consolidate.ts` is enforced in code, not in a prompt: accepting
    a consolidation performs exactly two writes and never a DELETE. It is
    covered by a test that snapshots the whole deck before and after. Do not
    widen it.
*   **Proposals are capped at 3–5 items** so the agent has to rank. Nothing is
    exempt: imports no longer create proposals at all (see below).
*   **An Archidekt import applies in full, immediately** (`applyImport`). The
    preview diff is the approval step, destructive cuts are called out first,
    and the whole import is *one* decision-log row carrying a snapshot of the
    prior list — so undo on that row reverts the entire import. Keep it one
    row: a 99-card import logged per card would flush every real ruling out of
    the agent's retention window.
*   **Slots are optional.** A deck may define none and cards may stay
    unslotted forever. Nothing — proposals included — may require one.
*   **An audit run outlives whoever started it.** `POST /audit` opens a row in
    `audit_runs` and returns; the reasoning pass finishes on the server and
    `finishAuditRun` writes the result regardless of who is watching. The UI
    polls. Never make a run's survival depend on a mounted component — that is
    the bug the section replaced. Runs in flight at shutdown are reclaimed at
    boot (`reclaimStaleRuns`), and only the newest `AUDIT_RUN_RETENTION` per
    deck are kept.
*   **The audit section shows live checks and stored reasoning.** §8.1
    findings are recomputed per request so a wrong count never sits on screen;
    §8.2 findings come from the recorded run and carry its revision. Do not
    "simplify" this by rendering the whole stored snapshot.
*   **Computed state is given to the model, never derived by it.** If the
    agent would need to count something, that count belongs in
    `computeState()` first.
*   **Slot and card type are independent axes.** The decklist groups by either
    one (the toggle in `DeckView.tsx`), so `validateVocabName` rejects card
    types as slot/tag names in both numbers — "lands" as well as "land". Type
    grouping puts a card under the first match in `TYPE_GROUPS`, except that
    Land wins outright so the group total agrees with `computed.land_count`.
