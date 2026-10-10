# MTG Commander Deck Builder — Build Spec (v2)

> Rewritten 2026-10-10. This replaces the 2026-07-26 spec, which is in git
> history. It was designed as if no code existed. `packages/deck-builder` was
> then rebuilt against it the same day; places where building it corrected or
> sharpened this document are marked *(as built)*.
>
> **Decisions settled with Josh on 2026-10-10:**
> - The app makes no AI model calls. Agents are external and reach it through a documented interface (§10).
> - Local only. No hosting, no remote access.
> - Typed rejections, hard filters and the 3–5 item proposal cap are gone. A rejection carries an optional free-text reason (§8).
> - Memories replace card notes, hard filters and playtest notes. They are written by agents without approval and are always visible to the owner (§7).
> - The player profile and deck briefs are the owner's documents; agent edits to them are proposed (§6).
> - Agents edit draft decks freely; the approval gate applies to active decks (§5.2).
> - The `owned` flag becomes a per-deck "in paper" flag (§5.5).
> - Cross-deck membership is off by default in card search and is turned on per call (§4.2).

---

## 1. What this is

A local, single-user application for building Magic: The Gathering Commander
decks, designed to be driven by a person and by external AI agents through the
same capabilities.

The app is the source of truth. It stores structured data, enforces
validation and permissions, does every calculation, and keeps an auditable
record of every change. People use it directly through its UI. Agents use it
through an MCP interface. Interpretation, planning and conversation happen in
whatever agent the owner points at it (Claude Code, a desktop assistant,
anything that speaks MCP), never inside the app. The app is fully usable with
no agent attached.

What it models that a list-with-tags tool does not: **what each deck is trying
to do** (its brief), **what the owner has decided and why** (rulings), and
**what agents have learned about the owner** (memories and the profile).

### Design rules

1. **Card facts come from the database, never from model memory.** Names, oracle text and legality are looked up, on every path.
2. **If the app can compute it or look it up, nobody else derives it.** Counts, deltas, legality and cross-deck statistics are computed by the app and handed over. This applies to agents and to memories alike (§3).
3. **One capability layer, two front doors.** The UI and the agent interface call the same service functions. Validation lives below both.
4. **The owner rules on membership of active decks.** An agent changes which cards are in an active deck only through an accepted proposal.
5. **Everything an agent writes is attributed and visible.** No write is hidden, whether or not it needed approval.

---

## 2. Non-goals

- No model calls inside the app. This includes embeddings and any "smart" ranking that needs a model.
- No chat UI, no transcript storage, no context management. Those belong to the agent's own harness.
- No hosting, deployment, accounts or multi-user support.
- No external knowledge sources (EDHREC, Commander Spellbook, price sites). An agent may consult them itself; whatever it finds must resolve through the app's card search before it can be proposed.
- No collection or inventory management beyond the per-deck paper flag (§5.5).
- No live sync with any external service.
- No formats other than Commander.

Carried over from the previous spec without being re-examined; say so if any
should be reopened: no card images, no price tracking, no mobile layout.

---

## 3. The knowledge model

Three kinds of knowledge, separated by who is authoritative for them.

| Kind | Contents | Written by | Reaches an agent |
| --- | --- | --- | --- |
| **Records** | Cards, decks, proposals, rulings, computed state, activity log | The app, from actions | Through tools, on demand |
| **Documents** | Player profile (global), deck brief (per deck) | The owner; agents propose edits | Always loaded at their scope |
| **Memories** | Short notes about preferences, reasons and outcomes | Agents, without approval; the owner too | Loaded by scope; the rest searched |

**A record is never copied into a memory.** "The owner has three Golgari
decks" is a query. Memory that duplicates the source of truth is the main way
memory goes stale, so the app offers computed answers (§10.3, `player_stats`)
and the playbook forbids storing them.

---

## 4. Card data and search

### 4.1 Card data

**Source:** Scryfall bulk data, the `oracle-cards` file: one entry per unique
card, not per printing.

- SQLite, with an FTS5 index over name and oracle text. A refresh command re-pulls the bulk file and upserts.
- Keyed on `oracle_id` internally.
- Retain: `oracle_id`, `name`, `mana_cost`, `cmc`, `type_line`, `oracle_text`, `power`, `toughness`, `loyalty`, `color_identity`, `legalities.commander`, `layout`, `card_faces`. Discard everything else.
- Use Scryfall's `color_identity` directly. Do not compute it.
- `legalities.commander` supplies the banned list. Commander eligibility is a separate check.

### 4.2 Search

A subset of Scryfall query syntax: `t:`, `o:`, `cmc`/`mv` with comparators,
`id:`/`id<=`, `c:`, `pow`/`tou`, `is:`, extended with `slot:` and `tag:`.
Verify each operator's semantics against Scryfall's syntax documentation and
cite it in a comment.

When a search names a deck:

- Results are pre-filtered to the deck's colour identity, so illegal cards never come back.
- Each result is marked if it is already in that deck or on its maybe list.
- Each result carries any memories linked to that card (§7.2) and any prior rulings on it in that deck (§8.3).

**Cross-deck membership is off by default.** A result does not say which of
the owner's other decks contain the card unless the call sets
`include_my_decks: true`. Left on permanently, that information biases
recommendations toward cards the owner already has; it is a mode, turned on
deliberately.

### 4.3 Referring to cards

Agents refer to cards by **exact name**. Names cost a fraction of the tokens
of a UUID and are far less likely to be mis-transcribed.

- Resolution is exact, never fuzzy. A fuzzy match turns a visible mistake into a silent wrong answer.
- A full-name match wins over a face-name match.
- A name that resolves to nothing fails the **whole call**, with "did you mean" suggestions. No partial application.
- A name that is genuinely ambiguous fails with the candidates and their `oracle_id`s. Every card argument also accepts an `oracle_id` for that case.

Things the app mints (decks, proposals, memories, rulings) are referred to by
short integer ids, always shown beside their names.

---

## 5. Decks

### 5.1 Model

- A deck targets exactly **100 cards** including commanders.
- **Commanders are a list.** Zero to two, plus Background, plus Companion (outside the 100).
- Each deck row stores its denormalized colour identity. Changing the commander may invalidate cards; that surfaces as a check finding (§9), never as silent corruption.
- Each deck-card row: card, quantity, role (`card` / `commander` / `companion`), slot (at most one, may be null), tags (zero or more), `maybe` (on the maybe list, outside the 100), `in_paper` (§5.5).
- Every change to membership bumps the deck's **revision** number.

### 5.2 Draft and active

A deck is either **draft** or **active**. Only the owner changes this.

| | Draft | Active |
| --- | --- | --- |
| Agent changes membership | Directly (`edit_draft`) | Only by proposal (§8) |
| Agent edits the brief | Directly | Only by proposed edit (§6) |
| Owner changes anything | Directly | Directly |
| Changes are logged with author and revision | Yes | Yes |

Every new deck starts as a draft, whoever creates it *(as built: the owner's
new decks too, since an empty deck is a draft by any definition)*. A fork
starts as a draft. The owner makes a deck active when it is ready, and may
return an active deck to draft for a rebuild. This is what makes
"build me a first 100" workable: a hundred cards do not go through a
per-item gate.

### 5.3 Forks

`fork_deck` copies the card list, organization, brief and deck memories into a
new draft, and records the parent. The copies then diverge independently; the
parent link exists so the two can be compared (`compare_decks` defaults to
the parent) and so an agent working on the fork can read where it came from.

The owner can also record a parent on an existing deck, or clear it: a deck
copied by hand has no link until one is set, and with one it compares and
inherits history like any fork.

Forks replace named checkpoints. There is no snapshot system and no DAG.

### 5.4 Slots, tags and the maybe list

- **Slots:** optional; a card belongs to at most one. Slots have target counts (a number or a min–max range). Targets are soft, and violations are visible. New decks define no slots.
- **Tags:** many per card, from a per-deck vocabulary.
- **Maybe list:** cards under consideration, outside the 100. A parked card keeps its slot and tags.
- **Naming rule:** slot and tag names may not equal a card type in singular or plural (`land`, `lands`), nor any search prefix. Slot and card type are independent axes; whole-name matches only, so `land destruction` is allowed.

**All of this is organization, not membership, and agents do it directly** on
any deck: define and retarget slots, file cards, tag, shortlist candidates
onto the maybe list. None of it changes the 100 or bumps the revision. To keep
the tag vocabulary from drifting, creating a tag that closely matches an
existing one fails and names the existing tag.

### 5.5 The paper flag

`in_paper` on a deck-card row means the physical card is in the physical deck.
It does not mean "owned": a card can be owned and still missing from this
deck. The UI works exception-first: mark a deck "fully assembled", then mark
the few cards that are missing.

- It is written only by the owner.
- It appears in no default tool output.
- `paper_status(deck)` returns the deck's missing cards and, for each, any other deck where that card is marked in paper. That separates a **pull list** from a **buy list**. Each pull-list line states that moving the copy leaves the other deck missing it.
- There are no inventory counts. One flag per deck-card is the whole feature.

---

## 6. Documents

Two documents, both short, both the owner's voice, both always in context at
their scope.

**Player profile** (one, global): tastes, playgroup and power level, budget
stance, standing dislikes, anything durably true about the owner as a
builder. Freeform markdown with a soft cap of about 600 words; over the cap,
the app flags it.

**Deck brief** (one per deck): the thesis, constraints, and **engines**.
Thesis and constraints are freeform markdown. An engine is a structured
record: a name, a description, and its required pieces as card references, so
that "is every piece of this engine in the deck" is a check (§9), not a
judgement.

**Editing:** the owner edits either directly. An agent calls
`propose_profile_edit` or `propose_brief_edit` with the replacement text and a
reason that cites what prompted it (rulings, memories). The owner sees a diff
and accepts or rejects it. On a draft deck, brief edits apply directly.

Memories are where an agent learns; documents are what the owner has
confirmed. When a group of memories has settled into something durable, the
playbook tells the agent to propose folding it into the relevant document and
to retire the memories once accepted.

---

## 7. Memories

### 7.1 Shape

One memory is one statement with its reason, a few sentences at most, stored
as a row:

| Field | Meaning |
| --- | --- |
| `text` | The statement, with `[[Card Name]]` for any card mentioned (§10.5) |
| `scope` | `global` or `deck` (with the deck) |
| `cards` | Optional links to cards |
| `basis` | `stated` (the owner said it, including in a ruling reason) or `inferred` (the agent concluded it) |
| `author` | The owner, or the agent's self-reported client name |
| `source` | Optional: the ruling that prompted it, and the deck revision if deck-scoped |
| `status` | `active`, `superseded` (with a link to the replacement), or `retired` |
| timestamps | Created, last revised |

Rows, not files, so that the app can attach a memory to a card or deck and
surface it without relying on the agent to go looking.

### 7.2 Scope and surfacing

- **Global** memories load at session start.
- **Deck** memories load with the deck.
- **Card-linked** memories, of either scope, are attached to that card wherever it appears: search results, card lookups, and the response to a proposal that includes it. A global memory linked to a card follows the card into every deck.
- Everything else is reached with `search_memories` (full-text, filterable by scope, deck and card).

### 7.3 Writing

- **No approval.** `remember` writes immediately.
- **Revise before adding.** `remember` returns existing memories that resemble the new one, so the agent can update or supersede instead of duplicating.
- **Nothing is deleted by an agent.** Superseding keeps the old row, marked and linked. Retiring hides a memory from loading and search but keeps it on disk.
- **The owner outranks the agent.** An agent cannot edit, supersede or retire a memory the owner wrote or edited. It may write a new memory that disagrees and say so.
- **Inferred memories are hypotheses.** They load marked as inferred. When the owner confirms one, it becomes stated.
- Deck-scoped memories written after a playtest carry the deck revision, so a note about how the deck played attaches to the list that was played.

What belongs in a memory is set by the playbook (§10.4): preferences,
corrections, reasons, playgroup facts and outcomes. Never anything a tool can
answer, never card text, never something read from a web page presented as
the owner's preference.

### 7.4 Budgets and upkeep

Loaded memory has a budget: about 1,500 estimated tokens of global memories
and about 1,000 per deck. Both are settings. When a scope is over budget, the
app loads the most recently revised memories that fit, reports how many were
left out, and says the scope needs consolidating.

Consolidation is the agent's work, prompted by the app's signals: merge
duplicates, supersede what has changed, promote a deck memory to global when
the same thing has shown up in several decks, and propose a document edit when
a cluster has settled (§6). The app never consolidates by itself; that would
need a model.

### 7.5 Visibility

A Memories page in the app: browse by scope, deck and card; see basis, author
and source; edit, retire or restore. The activity feed (§11) shows what each
agent session wrote. Memories are written without asking; they are never
hidden.

---

## 8. Proposals and rulings

### 8.1 Proposals

On an active deck, an agent changes membership by calling `propose_changes`.

- A proposal has a one-line summary and **any number of items**. The agent sizes it to what was asked for.
- An item is an add or a cut of one copy of one card, with a reason.
- Items may be **grouped**. A group is ruled on as a unit, so a swap justified by one argument cannot be half-accepted.
- The app tracks the pending delta against 100 and against each slot target, and returns it with the proposal.
- The response also returns, for each card, its linked memories and prior rulings in this deck (§8.3), so the agent sees what it should have considered.

### 8.2 Rulings

The owner rules in the app: per item, per group, or the whole proposal at
once. A ruling is accept or reject with an **optional free-text reason**.
There are no reason types.

Every ruling is a row in the **rulings log**: deck, card, action, outcome,
reason, proposal, author, timestamp, and the deck revision it produced. The
owner's direct edits and imports are logged in the same place. This log is the
version history, and **undo is the reversal of a log entry**. An import is one
entry carrying a snapshot of the prior list.

### 8.3 Prior rulings follow the card

When a card is proposed, searched with a deck named, or looked up, the app
attaches earlier rulings on that card in that deck: "rejected at revision 14:
too slow for this list". This is a lookup the app does every time. It replaces
the old hard filter without asking the owner to classify anything and without
depending on an agent remembering to check.

A fork inherits this history from the decks it descends from, each entry
labelled with the deck it was made in *(as built: a fork starts with no rulings
of its own, and the first agent to rework one said it had nothing to read)*.
History does not flow the other way.

An agent may re-propose a rejected card. The playbook requires it to cite the
prior ruling and say what changed.

### 8.4 The review queue

The owner often rules while no agent is running. Each ruling therefore has a
`reviewed` mark, unset until an agent has read it. *(As built: the queue holds
every ruling on a proposal or a proposed edit, and every direct change the
owner makes to an active deck. What an agent did itself, and the owner
assembling a draft card by card, are not queued.)* `start_session` reports how
many rulings are unreviewed, `list_rulings` returns them, and
`mark_rulings_reviewed` clears them once the agent has decided what, if
anything, is worth a memory. A reason given in the app is never lost for lack
of a listener.

---

## 9. Checks

Deterministic, computed in SQL on request, never stored as a snapshot:

- Card count against 100
- Singleton violations, accounting for basic lands and cards that say otherwise
- Colour identity of every card
- Banned and not-legal cards
- Commander eligibility and partner pairing
- Slot counts against targets
- Engine pieces present or missing (§6)
- Mana curve, land count and mana symbol distribution
- Companion present: flagged as **unverified**, since the condition needs rules evaluation the app does not have

The UI shows a live problem count on the deck. Agents receive check results
as a section of `get_deck` and in the response to every write that could
change them.

Judgement about a deck (does it have a path to its win condition, what are
its anti-synergies) is an agent's work. It reaches the app as proposals,
memories and proposed brief edits, not as a stored report.

---

## 10. The agent interface

### 10.1 Transport and security

- One process owns the database and serves the UI, its HTTP API, and an **MCP endpoint over Streamable HTTP** on the same local port.
- Bound to `127.0.0.1`. That is not an authentication boundary: the endpoint also requires a bearer token (generated on first run) and validates the `Origin` header, so a web page in the owner's browser cannot call it.
- A small **stdio shim** proxies to the endpoint for clients that only launch stdio servers.
- The settings page shows the token and ready-to-paste configuration for common clients.
- Use the official MCP TypeScript SDK. The protocol had a breaking revision in July 2026 (stateless requests, no `initialize` handshake) and clients will speak a mix of revisions for some time; that is not worth hand-rolling. *(As built: the SDK's v2 line, `@modelcontextprotocol/server`, which implements the 2026-07-28 revision and serves 2025-revision clients statelessly from the same handler. The older `@modelcontextprotocol/sdk` package stops at 2025-11-25.)*
- The UI's own API has no token, so it is closed to other sites the same way: a request whose Host is not loopback, or whose Origin is present and not loopback, is refused.
- **Attribution.** A 2026-07-28 client names itself on every request. A 2025-revision client named itself only in its handshake, which a stateless server does not keep, so the endpoint also accepts a label in the URL (`/mcp/claude-code`); the Settings page generates configuration with one.

### 10.2 Design principles

- **Stateless.** Every call names its deck. There is no "current deck" on the server.
- **Few tools, shaped around tasks,** not one per database operation.
- **Results are compact text meant to be read,** grouped the way the deck is organized. No JSON blobs, no ids the agent will not need. `get_deck` takes a `sections` argument so re-reads are cheap, and the full view must stay well under a typical client's tool-result cap.
- **Writes return the new state:** card count, slot deltas and check results come back with the write, so the agent never re-fetches or counts.
- **Errors say how to fix the call.** A failed name resolution lists suggestions; an `edit_draft` on an active deck says to use `propose_changes`.
- **Annotations are honest:** read-only tools are marked read-only; nothing an agent can call is destructive.
- **Tool order is deterministic,** for client-side prompt caching.

### 10.3 Tools

| Group | Tool | Purpose |
| --- | --- | --- |
| Orient | `start_session` | Playbook, profile, global memories, deck roster (one line each), the owner's focus, unreviewed-ruling count, memory budget state |
| | `player_stats` | Computed across decks: colours, commander types, curves, slot habits, most-reused cards |
| Read | `get_deck` | Brief, list grouped by slot with oracle text, computed state, checks, deck memories, pending proposals, recent rulings; `sections` selects parts |
| | `compare_decks` | Cards only in one, only in the other, and shared; defaults to a fork's parent |
| | `search_cards` | §4.2 |
| | `get_cards` | Exact-name lookup of several cards: full text, linked memories, and rulings if a deck is named. Also the way to check that names are real |
| | `list_rulings` | By deck, by date, or unreviewed only |
| | `search_memories` | Full-text, filtered by scope, deck or card |
| | `paper_status` | §5.5 |
| | `export_deck` | Archidekt-importable text (§12) |
| Write freely | `create_deck` | New draft from a name, commanders, and optionally a pasted list |
| | `fork_deck` | §5.3 |
| | `edit_draft` | Add and remove cards, set commanders, write the brief; drafts only |
| | `define_slots` | Create, rename, retarget and remove slots |
| | `file_cards` | Assign slot and tags to cards already in the deck |
| | `shortlist_cards` | Add to or remove from the maybe list, with a note |
| | `remember` | §7.3 |
| | `revise_memory` | Update, supersede, retire or confirm |
| | `mark_rulings_reviewed` | §8.4 |
| Propose | `propose_changes` | §8.1 |
| | `propose_brief_edit` | §6 |
| | `propose_profile_edit` | §6 |

**Focus.** The app records what the owner has open in the UI (which deck,
which cards are selected). `start_session` returns it and `get_deck` accepts
`"focus"` as the deck, so "this deck" in conversation means the one on
screen.

**Owner-only actions** have no tool: ruling on proposals and document edits,
switching a deck between draft and active, deleting a deck, writing the paper
flag, importing over an existing deck, undo.

### 10.4 The playbook

The rules an agent needs to work well with this app. It is delivered three
ways because clients differ in what they read:

1. The MCP server `instructions`: within 512 characters and self-contained, covering only cross-tool rules. *(As built: 512 is the figure Codex's documentation gives for what it can rely on; a test holds the text to it.)*
2. The `start_session` result, which carries the full playbook. The instructions tell the agent to call it first.
3. An optional **Agent Skill** in the repository, holding method rather than rules: how to evaluate a deck against its brief, how to build from a commander, how to run a memory tidy-up.

The playbook's rules:

- Call `start_session` first. Review unreviewed rulings before proposing anything.
- Mention a card only if it is in a deck you have read or came back from a search or lookup in this conversation. A hunch is a search query, not a fact.
- When an argument depends on what a card does, quote the oracle clause.
- Never count or compute what the app reports.
- Re-proposing a rejected card requires citing the ruling and what changed. One push-back, then defer.
- What to remember and what not to (§7.3). Revise before adding.
- When a memory scope is over budget, consolidate it.

Rules live in the app's playbook; validation lives in the app's code. Nothing
that matters is enforced only by a sentence in a prompt.

### 10.5 Linting what agents write

Every piece of prose an agent stores (proposal reasons, memory text, brief and
profile edits, shortlist notes) writes cards as `[[Card Name]]`. On write, the
app resolves every reference exactly; one unresolvable name fails the call
with suggestions. Whatever the app displays therefore contains no invented
card.

---

## 11. The app for people

- **Decks:** list, create, fork, delete; draft or active state.
- **Deck view:** the list grouped by slot or by card type, slot targets and deltas, the maybe list, the brief, a live checks count, the paper flag, direct editing, card search.
- **Proposals inbox:** pending proposals across decks; rule per item, per group or all at once, with an optional reason; diffs for proposed brief and profile edits.
- **Rulings log:** per deck, with undo.
- **Memories:** §7.5.
- **Profile:** edit directly.
- **Activity feed:** every agent write, with the agent's name, time and target. The audit trail for unapproved writes.
- **Settings:** connection details for agents, memory budgets, card data refresh.

The UI updates live when an agent writes (server-sent events from the app to
the browser). A proposal made in a terminal appears in the inbox without a
reload.

---

## 12. Archidekt interop

Construction happens here. Playtesting and goldfishing happen in Archidekt.

- **Export:** an Archidekt-importable list, optionally filtered to cards missing in paper.
- **Import:** the owner pastes a list; the app diffs it against the deck, shows the diff with destructive cuts called out first, and applies it in full on confirmation. It is one rulings-log entry, so one undo reverts it. Lines in a `Maybeboard` or `Sideboard` category are shortlisted on the maybe list, not added, and the preview says so.
- One-way out, one-way in. No sync layer.

Verify Archidekt's text format against its own documentation and cite the
sources in the code. Do not rely on model memory for it.

---

## 13. What is guaranteed about card facts

| Guarantee | How |
| --- | --- |
| No invented card can enter a deck | Every card argument resolves exactly or the call fails (§4.3) |
| No invented card appears anywhere in the app | Agent prose is linted on write (§10.5) |
| An agent reads card text; it does not recall it | Deck views, searches and lookups all return oracle text |
| Counts and legality are never an agent's arithmetic | Computed by the app and returned with every read and write (§9) |

**Not guaranteed:** what an agent says in its own chat window. The app cannot
lint text it never sees. The playbook's citation rule and the `get_cards`
lookup reduce this; they do not remove it. The same goes for an agent
reasoning wrongly from correct card text about timing, layers or replacement
effects.

---

## 14. Build order

Each phase is verified before the next begins, and each is testable without
the one above it.

1. **Capability core.** Card database and search; decks with draft and active states, forks, slots, tags, maybe list; checks; the rulings log with undo. Tested with no UI and no agent.
2. **Agent interface, read side.** MCP endpoint with token and Origin check, stdio shim, `start_session`, the read tools, the playbook. Verify by connecting a real agent and reading decks.
3. **The gate.** `edit_draft`, organization tools, `propose_changes`, prior rulings following the card, linting on write; the proposals inbox and live UI updates.
4. **Memories and documents.** The memory store, budgets, the review queue, the profile, proposed document edits, the Memories page and activity feed.
5. **Cross-deck.** `player_stats`, `compare_decks`, `paper_status`, focus.
6. **Method and evaluation.** The Agent Skill, and a set of realistic multi-step tasks ("rework this fork toward a new thesis", "what should I build next") run against a real agent, with transcripts read and the tools revised from what they show.
7. **Interop.** Archidekt export and import.

## 15. Verified at build time

Checked on 2026-10-10 while building; recorded here so the next change starts
from what is known.

- **Protocol revisions.** The official SDK's v2 packages (2.3.1) implement 2026-07-28 and fall back statelessly for 2025-revision clients; both paths are covered by tests that use the official client. Claude Code 2.1.296 connected speaking 2026-07-28; Codex CLI 0.162.1 connected speaking the 2025 revision. Both work.
- **Codex.** A real Codex run completed "find what most needs fixing in this deck and act on it" with no failed tool call, following the same loop as Claude Code (session, deck, unread rulings, memory, proposal). It connects with a static `Authorization` header in `~/.codex/config.toml`, which its CLI, desktop app and IDE extension share. Its `default_tools_approval_mode` honours the tools' read-only annotations: under `writes` a read ran and `remember` was held for approval; under `approve` nothing is asked. Without a label in the URL its writes are attributed to `codex-mcp-client`.
- **Server `instructions`.** Delivered in both eras (asserted in tests). Claude Code called `start_session` first in every real run, unprompted. No other client was checked.
- **Permission prompts.** Claude Code refuses each tool call until the server is allowed: a run without an allowlist was denied `start_session`. Allowing `mcp__deck-builder` (the `--allowedTools` flag, or `permissions.allow` in its settings) makes every tool, including the unapproved writes, run without asking. The Settings page shows the snippet.
- **Real-agent evaluation.** Seven tasks against a copy of real decks, Claude Code with a Sonnet model, given nothing but the tools (one run also had the Agent Skill installed): find and fix what most needs fixing in a deck; say what the owner builds and draft the next deck; rework a deck as a fork; list what is missing in paper; evaluate a deck against its brief; catch up on rulings and suggest cards; tidy cluttered memories. All seven completed. Three tool calls failed across them, each recovered in one step: an unsupported search key, and the two below.
- **What the transcripts changed.** (1) Forks now inherit their parents' rulings (§8.3). (2) `fork_deck` says to rewrite the thesis first; the agent that was not told left the parent's brief on a deck with a new aim. (3) The paper list carries type lines; an agent sorting bare names by kind got two wrong. (4) Every tool calls a body of prose `text`; an agent wrote `content` to `remember` after seeing it on another tool. (5) `list_rulings` ends with the exact `mark_rulings_reviewed` call; an agent first tried it with no arguments.
- **Full-text search for memories.** Adequate at the two dozen that exist. Not tested at the hundreds §7.4 anticipates.
- **Memory budgets.** The over-budget signal worked in a real run: a scope pushed past its budget was consolidated when the owner asked for a tidy-up. The default sizes themselves are untested against long use; they are settings.
- **Output sizes.** A full 100-card deck reads at 4–7k estimated tokens; `start_session` at about 1.5k.

## 16. Not verified

Everything in §1–§14 is built. What has not been checked:

- Any client other than Claude Code and Codex.
- ChatGPT on the web or phone. Its servers cannot reach a loopback address. The route is OpenAI's Secure MCP Tunnel running the stdio shim; the Settings page gives the commands from OpenAI's guide, but they have not been run, because they need a tunnel created in the owner's OpenAI Platform organization.
- The "download the latest card data" button, which was not pressed: it rewrites the bulk file the oracle parser shares. The code under it is the same `refresh` command the CLI runs.
- The evaluation was run once per task, on one model, with tasks written by the same hand as the tools. It shows the interface works; it does not measure how often.
