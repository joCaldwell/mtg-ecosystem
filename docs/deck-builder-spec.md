# MTG Commander Deck Builder — Build Spec

> Copied verbatim from the authoring session on 2026-07-26 so it survives outside chat.
> **Settled decisions (from the "Open knobs" section, chosen 2026-07-26):**
> - Tech stack: localhost web app; TypeScript; `node:sqlite` (built-in FTS5, zero native deps); lives in `packages/deck-builder`.
> - Slot targets: min–max ranges (a single number is shorthand for min = max).
> - Brief: hybrid — engines as structured records (required pieces keyed by `oracle_id`), thesis and constraints as freeform markdown.
> - Audit findings queue for manual promotion into proposals; never auto-generate.
> - Audit runs are backgrounded and recorded; the newest 5 per deck are kept (§8).
> - Default slot set for a new deck: **empty** — every deck defines its own slots.
> - Decision-log retention `N` = 30 (config value).
> - Agreed deviation: an Archidekt import (§9) applies in full and immediately rather than becoming a proposal — the gate is aimed at the agent, and the preview diff is the owner's approval. The 3–5 item cap in §7.1 therefore has no exemptions.

---

## 1. What this is

A local, single-user, desktop-only Magic: The Gathering Commander deck builder with a deeply integrated AI agent. The agent knows what each deck is trying to do, proposes changes with reasoning, remembers why I rejected things, and can never invent a card.

This is not a Moxfield or Archidekt replacement. Those tools treat a deck as a list with tags. This one models *what the deck is trying to do* — its thesis, its named engines, its win path — and evaluates the list against that.

### Non-goals

- No card images. Text only.
- No hosting, no deployment, no auth, no multi-user. Runs locally, always.
- No mobile or responsive layout. Desktop only.
- No collection or inventory management.
- No price tracking.
- No live sync with any external service.
- No formats other than Commander.

---

## 2. Card data

**Source:** Scryfall bulk data, the **`oracle-cards`** file specifically — one entry per unique card rather than per printing (~35k rows instead of ~500k). Not `default-cards`.

- Store in SQLite. Provide a refresh command that re-pulls the bulk file and upserts.
- **Key everything on `oracle_id`.** Never on printing ID, never on name.
- FTS5 index over card name and oracle text.
- Fields to retain: `oracle_id`, `name`, `mana_cost`, `cmc`, `type_line`, `oracle_text`, `power`, `toughness`, `loyalty`, `color_identity`, `legalities.commander`, `layout`, `card_faces` (for MDFCs/split cards).
- Discard everything else — no prices, set codes, rulings, art, artist, flavor text.

**Color identity:** use Scryfall's `color_identity` field directly. Do not compute it. It already handles mana symbols in rules text, hybrid, reminder text, and the weird cases.

**Legality:** `legalities.commander` gives the banned list for free. Commander *eligibility* (legendary creature, or oracle text containing "can be your commander") is a separate check.

**Search syntax:** support a subset of Scryfall query syntax — `t:`, `o:`, `cmc`/`mv` with comparators, `id:`/`id<=`, `c:`, `pow`/`tou`, `is:`. It's the format's lingua franca and I already know it. Extend it with `slot:` and `tag:` (see §4).

---

## 3. Deck model

- A deck targets exactly **100 cards** including commanders.
- **Commanders are a list, not a single field.** Support zero to two, plus Background, plus Companion. Do not model this as a single foreign key — retrofitting partners later is painful.
- Deck rows store denormalized `color_identity` for fast legality checks.
- Changing a deck's commander may invalidate cards by color identity. This must surface as an audit finding, not silently corrupt the deck.

### Deck-card rows

Each card in a deck has:
- `oracle_id`
- `slot_id` — exactly one, may be null
- tags — zero or more, from a controlled vocabulary
- `owned` — boolean

### The `owned` flag

Set manually by me when a card physically arrives. Used only to filter the export into a buy list (`where owned = false`).

**This field must never appear in the agent's context.** If the agent can see what I own, it will quietly bias its suggestions toward my binder. Budget-awareness is a mode I turn on deliberately, not an ambient bias.

---

## 4. Slots and tags

**Slots:** a card belongs to at most one slot. Slots are optional — a deck may define none, and a card may sit unslotted indefinitely; nothing (proposals included) requires one. Slots have **target counts** — a number or a range. Targets are soft; a deck can deliberately violate one, but the violation should be visible.

Slot targets are what turn "you're 2 cards short" into "you're 2 short and interaction is 3 under target while ramp is 2 over." They also supply half the audit rubric for free.

**Slots are the agent's to manage, directly.** It creates, renames, retargets and deletes slots, and files cards into them in bulk, with no approval gate — `create_slot`, `update_slot`, `delete_slot`, `move_cards`. This is a deliberate exception to §7, and the line it draws is *membership vs. organization*: none of these change which cards are in the 100, none bump the deck revision, and any of them is reversed with a dropdown. Gating filing behind the same ceremony as a cut would dilute the ceremony rather than protect anything. Filing a card is addressed by **name**, resolved exactly against the deck's own contents — the anti-hallucination guarantee (§6) is intact because a name that isn't already in the deck resolves to nothing and fails the whole call.

Tags are the counter-example and stay gated: a new tag is a permanent addition to a controlled vocabulary that the search grammar and every future filter depend on, which is a different kind of act from filing a card.

**Tags:** a card can have many. Tags are a **controlled vocabulary** — the agent picks from existing tags; creating a new tag is itself a proposal I approve. Without this you get `ramp`, `mana-ramp`, and `acceleration` as three tags in a month and the filters quietly stop working.

**Naming rule:** slot and tag names may not collide with card types (`land`, `artifact`, `creature`, `instant`, etc.), **in either singular or plural**, nor with any supported search prefix.

Two different reasons sit behind that one rule. Prefixes are reserved to protect the search grammar, since slots and tags are queryable via `slot:` and `tag:`. Card types are reserved because **slot and card type are meant to be independent axes**: a slot says what a card *does* (`ramp`, `interaction`), the type says what it *is*, and the decklist groups by either one on demand. A slot named `lands` collapses the two, and makes "34 lands" ambiguous about which of them it counts.

The rule matches whole names only — `creature removal` and `land destruction` are roles that happen to mention a type, and are allowed.

---

## 5. The brief

A per-deck document holding:
- The thesis — what this deck is trying to do
- Named engines, each with its required pieces
- Constraints (budget, no infinite combos, pod meta, whatever)
- Anything else durably true about the deck's intent

The brief is **seldom changed**. It is editable by me directly, and by the agent via tools — but agent edits go through the approval gate like everything else.

---

## 6. Anti-hallucination architecture (highest priority)

This is the core of the project. Chat-based deckbuilding fails because nothing in the loop distinguishes "I read this card" from "I'm confident about this card." The fix is to make recall structurally unable to reach the output. Four layers:

### 6.1 The full decklist is permanently resident in context

100 cards with full oracle text is roughly 8–9k tokens. Cheap. If the whole list is in every request, the agent never *recalls* what's in the deck or what those cards do — it reads them. This alone kills most errors.

### 6.2 Mutations take `oracle_id`, never names

`add_card(oracle_id, reason)`, `remove_card(oracle_id, reason)`, `swap(...)`. The agent cannot produce a valid `oracle_id` from memory, so the only way to add a card is to have found it in a search result first. **A hallucinated card physically cannot enter the deck.**

### 6.3 Candidates come only from search results

The search tool returns name, mana cost, type line, color identity, and full oracle text. System prompt rule, stated explicitly: *you may reference a card only if it is in the current deck or was returned by a search this turn; if you have a hunch a card exists, search for it before saying anything about it.*

This inverts the model's role — its training becomes a **query generator** ("there's probably a green card that untaps permanents during each opponent's untap step") rather than a fact source. The hunch is usually right about the shape of the thing; the specifics are what rot.

**All searches are pre-filtered by the commander's color identity**, so illegal cards never come back at all.

### 6.4 Output linting

- Every card mention in agent output must use `[[Card Name]]` syntax.
- Before anything renders, resolve all `[[...]]` against the database.
- **Exact match only. Never fuzzy-match.** Fuzzy matching turns a visible hallucination into a silent wrong answer — the model invents "Seedborn Sage," you resolve it to Seedborn Muse, and a fabrication now looks like a citation.
- Near-miss → bounce back to the agent as a "did you mean" it must explicitly re-resolve.
- No match → hard error, hand back to the agent, retry. Unresolved names never reach my screen.

### 6.5 Residual risk

The agent will sometimes have correct oracle text and still reason wrongly about interactions — layers, timing, replacement effects, "can't" vs "doesn't." Mitigation: **require it to quote the specific oracle clause it's relying on.** A wrong inference usually can't produce a supporting quote. This does not go to zero and shouldn't be expected to.

---

## 7. Proposal and approval system

**The agent never edits the deck's contents directly.** It proposes a changeset; I rule on it item by item. The gate is on membership — what is in the 100. **Organization is not membership**: the agent manages slots and how cards are filed into them directly, without a ruling (see §4).

The reasons are the point. In a direct-edit model my objection is said once and evaporates. Here the objection is the durable output — the changes are almost incidental, but the reasons are what the agent can't reconstruct and what makes each session smarter than the last.

### 7.1 Changeset structure

- A proposal contains **3–5 items maximum.** Hand me fifteen changes and I rubber-stamp them. The cap forces the agent to rank, and its ranking is itself informative.
- Item: `action` (add/cut), `oracle_id`, `rationale`, `group_id`, `status`.
- **`group_id` marks atomic bundles.** A swap justified by one line of reasoning must be accepted or rejected as a unit — otherwise I take the cut, reject the add, and sit at 99 cards holding half an argument. Everything else is independent.
- Track **pending delta** against 100 and against each slot target, so the app always knows I'm two cards short and the agent can be told to fill exactly that.

### 7.2 Typed rejections

Rejecting an item **requires a reason**, and the reason is **typed**. Four types, routing differently:

| Type | Meaning | Destination |
|---|---|---|
| Hard filter | Don't own it, out of budget, hard no | Never suggest again; also filters at search level |
| Thesis change | "That's not what this deck is" | Triggers a proposed edit to the brief |
| Playtest finding | "I tried it, it underperformed" | Durable card-specific note. Strongest reason type. |
| Soft / not now | Deck isn't ready, want the mana base settled first | Decays or resurfaces; does not become permanent |

If these all land in one undifferentiated "rejected" bucket, the agent over-applies them — it treats "not yet" as "never," quietly stops exploring a line, and I never notice because an absent suggestion is invisible.

### 7.3 Re-proposal rules

- The agent **may** re-propose a previously rejected card — decks change and yesterday's bad fit is today's answer.
- But a re-proposal **must cite the prior rejection and state what changed.**
- The agent gets **one push-back**, with the specific oracle clause it's relying on, then defers. This is the line between a collaborator and a nag.
- This is a system prompt rule plus injecting the rejection log. No code required.

### 7.4 Versioning comes free

Every mutation is an approved proposal item with a timestamp and a reason. **That log is the version history.** Undo is reversing one item.

- Do **not** build a separate undo stack or snapshot system.
- Do **not** build event sourcing. Materialize the deck table, append to the log, accept that they could theoretically drift.
- Named checkpoints ("pre-Omen-Machine rebuild") are just a copy of the card list. Not a git-like DAG.

---

## 8. Audit

**A permanent section below the decklist**, not a dialog. On-demand only — a button, plus an optional free-text field for instructions ("focus on the mana base," "I keep losing to graveyard decks") — but the results are part of the deck's record, and I should be able to scroll to them the same way I scroll to the mana base.

Two rules follow from that, and they are the whole reason this section is not a modal:

- **A run is a server-side job.** Starting one returns immediately; the reasoning pass finishes on the server and writes its result whether or not anything is watching. Closing the panel, navigating away, or closing the browser cannot cancel an audit — it costs a model call, and losing it to a stray Escape key taught me not to run one.
- **Runs persist.** The newest **5** runs per deck are kept, with their instructions, the revision they were taken at, and both halves of their findings. Deeper history has no reader: an old deterministic finding is stale the moment the deck changes, and old reasoning is superseded advice.

What the section shows between runs is deliberately mixed: **the deterministic checks are recomputed live** on every deck change, because they are SQL and a wrong count on screen poisons trust in everything near it; **the reasoning pass is the stored one**, labelled with its run and flagged when the deck has moved past the revision it was taken at.

### 8.1 Deterministic checks — SQL only, never the model

- Card count against 100
- Singleton violations (accounting for basic lands and cards that say otherwise)
- Color identity legality of every card
- Banned list (`legalities.commander`)
- Commander eligibility
- Slot counts against targets
- Mana curve distribution, land count, mana symbol distribution

**Never ask the model whether the deck is 100 cards.** It's a `count(*)`, the model will get it wrong often enough to poison trust in every other finding, and it will then reason confidently from the wrong number.

### 8.2 Reasoning checks — the model, with deterministic results supplied as context

- Does the deck have a path to its stated win condition?
- Which cards break symmetry, and which quietly hand parity back?
- Anti-synergies — cards that undo a named engine
- Are all pieces of each named engine present, and is there redundancy?
- Can the deck survive long enough to assemble its engine?

The model reasons much better about these when it already knows the deck is two cards over and light on interaction.

### 8.3 Findings route into the proposal system

An audit finding becomes a proposal. Dismissing a finding requires a typed reason and gets logged exactly like a rejection. Otherwise the audit reports the same four things forever and I learn to ignore it.

*(Settled: findings queue for manual promotion; they do not auto-generate proposals.)*

### 8.4 Findings route into the chat

The other thing I want to do with a finding is argue with it. When the reasoning pass says the deck has no way to win, the next move is to ask the agent about *that*, in the chat, without retyping it.

So every finding has an **"ask agent"** button, and it appends a reference — `audit#12/reasoning:no-win-path` — to the chat draft. The reference is a **pointer, not a paste**: the agent resolves it with a `get_audit` tool that reads the recorded run. Pasting the finding's text instead would freeze a copy of it into the transcript forever, and the run is already on disk.

The agent is told what a run is when it reads one: a snapshot at a revision, whose deterministic half is authoritative for *that* revision, and whose reasoning half is another model's judgement — to be engaged with from the oracle text, agreed with or not.

---

## 9. Archidekt interop

Deck construction happens here. Playtesting and goldfishing happen in Archidekt.

- **Export:** produce an Archidekt-importable list. Optionally filter to `owned = false` to produce a buy list for card seller sites.
- **Import (paste-back):** I paste a list back in; the app **diffs it against the current deck, shows me the diff, and on my say-so applies the whole thing at once.** The approval gate exists to stop the *agent* from changing the deck unilaterally — a pasted list is my own list, and clicking through 99 per-card accepts was friction with no information in it. The preview diff is the approval step. Destructive cuts (cards in the deck but absent from the paste) are called out before applying. The import lands as **one** decision-log entry carrying a snapshot of the prior list, so undoing that row reverts the entire import — and one line rather than 99 keeps a big import from flushing real rulings out of the agent's retention window (§12).
- **Do not build a sync layer.** One-way export, one-way import, both through the same door. Archidekt never silently becomes the source of truth.

> ⚠️ **Verify Archidekt's actual text import/export format and its tag/category syntax against their current documentation before implementing.** Do not rely on model memory for this — it's exactly the kind of detail that gets confabulated.

### Playtest notes

A freeform note field, **stamped to the deck version that was exported.** Goldfishing generates the best data in the whole system ("never found Omen Machine before turn 9," "Static Orb was dead in three of five opens," "had the lock and no way to close"). If it lives in my head, the agent never sees it and I re-explain it every session. Since the approval log already gives version history for free, a note attaches to a specific 100-card list rather than floating.

---

## 10. Context assembly

The single most important function in the codebase. Everything above converges here. Assembled in this order:

| # | Segment | Approx. tokens | Notes |
|---|---|---|---|
| 1 | Agent rules and output contract | ~600 | Static across all decks |
| 2 | Deck brief | ~500 | Seldom changes |
| 3 | Slots, targets, tag vocabulary | ~300 | Seldom changes |
| 4 | Decklist — 100 cards with oracle text | ~9k | Changes per accepted proposal |
| 5 | Computed state — counts and deltas | ~150 | From SQL |
| 6 | Pending proposals | ~200 | Un-ruled items |
| 7 | Decision log — resident portion | ~2k | Compaction zone |
| 8 | Session transcript | grows | Compaction zone |
| 9 | Search results — this turn | ~1k | Ephemeral |
| — | Tail restate block | ~75 | See below |

**Order is driven by stability, for prompt caching.** Segments 1–3 form a stable cached prefix. Segment 4 breaks cache on accepted proposals but sits above the transcript, which changes every turn. On a local app hitting the API this is real money and real latency.

**Decklist format:** grouped by slot (not alphabetically), tags inline on the card line. Send only name, mana cost, type line, oracle text, P/T. No JSON blobs, no set codes, no prices, no rulings, no legality objects. Grouping by slot means the model reads the deck the way the deck is organized and can *see* that interaction is thin rather than inferring it.

**Computed state is given, never derived.** The model must never be in a position where it needs to count something.

**Tail restate block** — 50–100 tokens after the transcript, immediately before my message: cards to 100, which slots are under/over, and hard-filter card names (names only; the reasons live up in the log). This is deliberately redundant with material already in the payload. The constraints that get violated are the ones buried in the middle, and a compact restatement at the boundary is the cheapest fix for that.

---

## 11. Compaction

**One chat per deck, forever.** The chat is an append-only session log with periodic manual compaction.

The chat is **not the source of truth** — the deck, brief, and decision log are all in the database. Compaction discards conversational texture, not data. That's what makes it safe to do aggressively.

### Behavior

- A **segmented context meter**, not a single number. "Transcript is 40k" means compact. "Decision log is 30k" means the retention policy is wrong and compacting won't help.
- A manual **consolidate** command. On invoke it: proposes brief updates for high-impact new information, discards what's outdated, and summarizes the remainder as the starting context for the continued chat.
- **Consolidation runs through the approval gate.** Show me proposed brief edits and proposed discards; I rule on them.
- **Keep the raw transcript on disk** after it leaves context. Disk is free; this makes compaction non-destructive.
- **Report what it rescued.** If consolidation regularly pulls important facts out of the transcript into the brief, that's a signal a tool is missing — those facts should have been written to a structured record when they happened. The report is the diagnostic.

### Hard boundary — enforce in code, not in the prompt

Compaction may touch **only** the session transcript and the aged, soft portion of the decision log.

It may never touch: the decklist, computed state, pending proposals, hard filters, playtest findings, or the brief (except via approved proposals).

With that enforced in code, a bad consolidation run can't do lasting damage — worst case I lose some conversational context and re-explain something once.

---

## 12. Decision log retention

Which portion of the log is resident in context. Make this a **config value I can edit**, not a constant baked into the assembly function — it needs tuning against real logs.

Starting default:
- All hard filters, forever (they're short — a card name and a type)
- All playtest findings, forever
- The last N decisions regardless of type *(settled: N = 30)*
- Soft notes until superseded or a few deck revisions old

**Log consolidation:** periodically, the agent should notice when several rejections say the same thing and propose a brief edit — "you've cut every counterspell I've suggested; should the brief say this deck doesn't run counterspells?" This is what keeps the brief seldom-changed but not stale, and it's the mechanism that makes the memory feel like it's consolidating rather than just accumulating.

---

## 13. Phased build order

Build and verify each phase before starting the next. Each layer should be testable without the one above it.

**Phase 1 — Card database.** Ingest the Scryfall `oracle-cards` bulk file into SQLite. FTS5 index. Search implementation with the Scryfall syntax subset. Refresh command. Verify: I can search and get correct results, keyed on `oracle_id`. ✅ *Completed 2026-07-26.*

**Phase 2 — Deck model, no agent.** Deck CRUD, commander list, slots with targets, tag vocabulary, `owned` flag, computed state. Manual card add/remove through the UI. Verify: I can build a 100-card deck by hand and see accurate slot deltas.

**Phase 3 — Proposal system, still no agent.** Proposals, items, atomic groups, per-item accept/reject, typed rejections and their routing, the decision log, undo via log reversal. Test by creating proposals manually. Verify: the approval gate and the log work end to end.

**Phase 4 — The agent.** Context assembly per §10, tool definitions (search, propose, brief edit), `[[Card Name]]` linting with exact-match resolution and retry. Verify: the agent cannot get a hallucinated card into the deck or onto my screen.

**Phase 5 — Audit.** Deterministic checks first, then the reasoning pass, then routing findings into proposals.

**Phase 6 — Interop and compaction.** Archidekt export/import-as-proposal (verify the format first), playtest notes, context meter, consolidate command. ✅ *Completed 2026-07-26. Archidekt's format was verified against their own FAQ and forum before implementing — the sources are cited in `src/deck/interop.ts`.*
