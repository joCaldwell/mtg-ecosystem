# MTG Commander Deck Builder — Build Spec (v3)

> Rewritten 2026-10-10. This replaces the 2026-07-26 spec, which is in git
> history. It was designed as if no code existed. `packages/deck-builder` was
> then rebuilt against it the same day; places where building it corrected or
> sharpened this document are marked *(as built)*.
>
> **Decisions settled with Josh on 2026-10-10:**
> - The app makes no AI model calls. Agents are external and reach it through a documented interface (§10).
> - Local mode remains the daily product. Hosted accounts and remote access are v3 goals (§17).
> - Typed rejections, hard filters and the 3–5 item proposal cap are gone. A rejection carries an optional free-text reason (§8).
> - Memories replace card notes, hard filters and playtest notes. They are written by agents without approval and are always visible to the owner (§7).
> - The player profile and deck briefs are the owner's documents; agent edits to them are proposed (§6).
> - Agents edit Theorycrafted cards freely; Paper cards and all brief edits require approval (§5.2). Legacy is read-only.
> - Physical assembly is tracked with ordinary deck tags (§5.5); the dedicated toggle was removed on 2026-10-11.
> - Slots describe purpose: one optional primary plus any number of secondary slots. Targets count primary assignments only; tags track sourcing and status (2026-10-14, §5.4).
> - Cross-deck membership is off by default in card search and is turned on per call (§4.2).

---

## 1. What this is

An application for building Magic: The Gathering Commander decks, with a
local implicit user and a hosted multi-user mode (§17), designed to be driven by a person and by external AI agents through the
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
4. **The owner rules on membership of Paper decks.** An agent changes which cards are in a Paper deck only through an accepted proposal.
5. **Everything an agent writes is attributed and visible.** No write is hidden, whether or not it needed approval.

---

## 2. Non-goals

- No model calls inside the app. This includes embeddings and any "smart" ranking that needs a model.
- No chat UI, no transcript storage, no context management. Those belong to the agent's own harness.
- Hosting, deployment, accounts and multi-user support are v3 goals (§17).
  Phase 1 establishes identity and tenancy; login, sessions, Better Auth and
  OAuth arrive in Phase 2, hosting in Phase 3, and visibility/public pages
  in Phase 4a. Phase 1 exposes no cross-owner decks.
- No collaborators, co-owners, shared memories, chat, paid tiers or formats
  beyond Commander. Forks and later suggestions are the collaboration unit.
- No lifecycle or rulings terminology changes in the identity phase.
- No external knowledge sources (EDHREC, Commander Spellbook, vendor sites). An agent may consult them itself; whatever it finds must resolve through the app's card search before it can be proposed. Scryfall's bulk data is the one source the app reads (§4.1, §4.4).
- No collection or inventory management; ordinary deck tags can record organization such as cards to obtain (§5.5).
- No live sync with any external service.
- No formats other than Commander.

Carried over from the previous spec without being re-examined; say so if any
should be reopened: no mobile layout.

Reopened on 2026-10-11: **cards carry a price** (§4.4). It is an estimate
from the same Scryfall refresh that loads the cards. There is still no price
history, no choice of vendor and no record of which printing the owner holds.

Reopened on 2026-10-10: **card images are shown on demand.** The deck stays a
text list; a card's picture appears only in its hover card and its detail
panel, using image URLs stored from Scryfall bulk data (§4.1, §13).
No image lookup by name is sent to the Scryfall API. `get_cards` includes
image facts in its structured result; its compact text stays unchanged.

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

- SQLite, with an FTS5 index over name and oracle text. The server schedules a refresh child process daily and on boot when stale, retrying failed refreshes one hour after their last run; the CLI runs the same operation. Settings enqueues it and returns immediately. A shared timestamped lease prevents overlapping refresh/backup jobs; leases expire after two hours. Job result, last run and next run appear in Settings.
- Oracle data is streamed and staged on disk, then upserted in savepoint batches of roughly 500. Malformed input and a file with fewer than 90% of the currently active cards are refused before publishing. Missing IDs become `retired=1`, preserving exact resolution, references, history and images. General search excludes retired IDs; a named deck may still find retired cards it holds, and its checks flag “retired printing/oracle id”. Returning IDs are reactivated. Retired rows are excluded from the next truncation threshold.
- The per-instance oracle cache lives under `DATA_DIR/.scryfall-cache`, reused for 24 hours. The Settings path never forces a download; CLI `--force` remains available. All connections wait up to five seconds for SQLite locks. Refresh and nightly backup run outside the serving event loop; see `packages/deck-builder/OPERATIONS.md`.
- Keyed on `oracle_id` internally.
- Retain: `oracle_id`, `name`, `mana_cost`, `cmc`, `type_line`, `oracle_text`, `power`, `toughness`, `loyalty`, `color_identity`, `legalities.commander`, `layout`, `card_faces`, nullable `game_changer` (§5.7), `image_small`, `image_normal`, `image_art_crop` (front-face imagery for double-sided layouts), and `retired`. Discard everything else.
- Use Scryfall's `color_identity` directly. Do not compute it.
- `legalities.commander` supplies the banned list. Commander eligibility is a separate check.

### 4.2 Search

A subset of Scryfall query syntax: `t:`, `o:`, `cmc`/`mv` with comparators,
`id:`/`id<=`, `c:`, `pow`/`tou`, `usd` with comparators (§4.4), `is:`, extended
with `slot:` and `tag:`.
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

### 4.4 Prices

Every card has at most one price: what its **cheapest playable paper
printing** costs, in USD. A deck records cards, not printings (§4.1), so this
is the only price the app can state honestly: what it would cost to get a
copy, not what the owner's copy is worth.

**Source:** Scryfall bulk data, the `default-cards` file (every printing).
The `oracle-cards` file also carries prices, but for one representative
printing per card, chosen for recognisability; measured on 2026-10-11 that
printing cost over 10% more than the cheapest one for about a quarter of
priced cards, and had no price at all for 1,600 cards that another printing
priced.

- A printing counts if it is a paper card, not oversized, and not from a `memorabilia` set (gold-bordered, Collectors' Edition, 30th Anniversary Edition). Those are not legal to play and would otherwise be the "cheapest printing" of most Reserved List cards.
- The lowest of the printing's `usd`, `usd_foil` and `usd_etched` is used; a foil is sometimes the cheapest copy.
- Stored as whole cents in `card_prices`, with the printing it came from. Loaded into a staging table in bounded transactions, then atomically swapped with the live prices and as-of timestamp: a card that lost its price is unpriced, not stale. A failed staging load preserves the previous prices.
- The file is read as it downloads and not kept. A refresh skips the download when Scryfall has published nothing newer than the prices in use.
- A card with no price on any counted printing has none. Totals leave it out and say how many cards they left out.

Scryfall calls bulk prices "dangerously stale after 24 hours" and fit only
for "a general estimate of card value". The app treats them that way: every
total is labelled an estimate and carries the date of the prices.

**Where a price appears.** On the card wherever the card is shown, to the
owner and to agents alike; as a deck total in computed state (the companion
counts, the maybe list does not).
`usd<=2` in a search compares this same price. Price is on by default for agents: it is a fact about
the card, and a budget is a constraint the brief can state.

---

## 5. Decks

### 5.1 Model

- A deck targets exactly **100 cards** including commanders.
- **Commanders are a list.** Zero to two, plus Background, plus Companion (outside the 100).
- Each deck row stores its denormalized colour identity. Changing the commander may invalidate cards; that surfaces as a check finding (§9), never as silent corruption.
- Each deck-card row: card, quantity, role (`card` / `commander` / `companion`), optional primary slot, secondary slots (zero or more), tags (zero or more), `maybe` (on the maybe list, outside the 100). Zone and purpose are independent: commanders and companions may also have slot assignments.
- Every change to membership bumps the deck's **revision** number.
- Each deck has a category: `paper`, `theorycrafted`, or `legacy` (§5.6).

### 5.2 Lifecycle and editing permissions

A deck’s lifecycle is **Paper**, **Theorycrafted**, or **Legacy** (§5.6).
There is no separate Draft/Active flag. Only the owner changes lifecycle.

| | Theorycrafted | Paper | Legacy |
| --- | --- | --- | --- |
| Agent changes cards | Directly (`edit_cards`) | By accepted proposal | Read-only |
| Agent changes brief or power target | By accepted proposal | By accepted proposal | Read-only |
| Owner edits | Directly | Directly | Read-only |
| Organization, memories, assessments | Direct writes | Direct writes | Read-only |

New agent-created decks and all forks are Theorycrafted. The owner may create
Paper or Theorycrafted decks and archive them as Legacy. Legacy decks can be
viewed, exported, compared and forked, but neither the owner nor agents can
rename, delete, change cards, change organization, revise deck memories, or
rule on proposals while archived. The owner can restore them to Paper or
Theorycrafted. Pending proposals stay stored, leave the actionable inbox
while archived, and return on restoration. All writes are attributed and
membership changes retain their revision and history behavior.

---

### 5.3 Forks

`fork_deck` copies the card list, organization, brief and deck memories into a
new Theorycrafted deck, and records the parent. The copies then diverge independently; the
parent link exists so the two can be compared (`compare_decks` defaults to
the parent) and so an agent working on the fork can read where it came from.

The owner can also record a parent on an existing deck, or clear it: a deck
copied by hand has no link until one is set, and with one it compares and
inherits history like any fork.

Forks replace named checkpoints. There is no snapshot system and no DAG.

### 5.4 Slots, tags and the maybe list

- **Slots:** purposes such as ramp, interaction, card advantage, and a deck's particular engines. A card may have one optional **primary** slot and any number of **secondary** slots. A slot cannot be both primary and secondary on the same card. New decks define no slots. Primary assignments group the ordinary deck list; the command zone remains separate. Secondary purposes are available in expandable contributor lists for each slot. Card rows do not repeat the card's secondary slots beneath its name in either grouping mode, including in the command zone, proposed additions and maybe list. The card panel and bulk actions edit primary and secondary assignments separately.
- **Targets:** a number or min–max range, soft and visibly checked. **Only primary assignments count toward targets.** The server reports primary count, secondary count, and total coverage separately, weighted by quantity. Coverage counts a card once per slot; secondary coverage never changes target status or pending target deltas. Parked cards and companions contribute to neither primary nor secondary counts; commanders in the 100 contribute according to their assignments. Slot totals must not be used to derive the deck's card count.
- **Tags:** sourcing, ownership, assembly, and overall status (for example Have, Ordered, or From another deck), from a per-deck vocabulary. Additional card purposes belong in secondary slots. The card panel uses a searchable checkbox dropdown and shows only assigned tags beneath it; an untagged card shows no tag chips. The deck list can be filtered by one or more tags, matching **any** (union) or **all** (intersection), together with its name search. **Invert filter** negates the selected any/all tag condition, so selecting Have and inverting matches cards without that tag, including untagged cards. Name search still applies normally. The closed filter shows the inversion; clearing all selected tags resets it. Filters apply to commanders, companions and the maybe list as well as ordinary cards. Matching rows are highlighted and nonmatches muted; every card and group stays visible in its original position and remains interactive. Counts, targets, costs and review decisions still describe the full deck. Selected cards support adding and removing tags in bulk.
- **Maybe list:** cards under consideration, outside the 100. A parked card keeps its primary slot, secondary slots, and tags.
- **Naming rule:** slot and tag names may not equal a card type in singular or plural (`land`, `lands`), nor any search prefix. Slot and card type are independent axes; whole-name matches only, so `land destruction` is allowed.

**All of this is organization, not membership, and agents do it directly** on
any deck: define and retarget slots, file cards, tag, shortlist candidates
onto the maybe list. None of it changes the 100 or bumps the revision. To keep
the tag vocabulary from drifting, creating a tag that closely matches an
existing one fails and names the existing tag. Renaming follows the same rule.
Tag assignments are unique per deck-card-tag and constrained to the same deck
at the database level. A filing update replaces a card's tag set atomically.
Secondary assignments are likewise unique and deck-scoped. Deleting a slot
removes only that purpose, preserving the card's other assignments. Changing
a primary slot to an existing secondary promotes it and removes the duplicate;
explicitly assigning the same slot in both roles fails atomically. Clearing
the primary slot leaves secondary purposes intact. Forks remap both kinds of
assignments; membership undo restores surviving purposes. Schema 8 preserves
all existing primary assignments and starts with empty secondary assignments;
it does not guess roles from existing tags. `slot:` search matches either
kind of assignment; `slot:none` means no primary slot on an ordinary in-deck card.
For an explicit cleanup, `file_cards` accepts `remove_unused_tags`: named
vocabulary entries are removed after filing only if no card still uses them.
Other unused tags are preserved; an assigned tag aborts the entire filing.
Choosing tags in the list filter is temporary UI state, independent of assigning
tags to a card; a deleted tag is removed from that filter.

### 5.5 Physical assembly through tags

Physical assembly and cards to obtain use ordinary per-deck tags (§5.4).
The owner chooses their vocabulary; agents can assign and remove those tags
with `file_cards` on Paper or Theorycrafted decks, with attribution in Activity.
The app gives no tag special inventory or ownership meaning, and makes no
inference about which physical copies are available elsewhere.

The dedicated per-card assembly field, toggle, counts, markers, Paper panel,
pull/buy lists, agent tool and missing-only export option were removed on
2026-10-11 at the owner's request. Schema 6 discards the previous marks rather
than converting them to tags, preserves other deck-card fields and tag links,
and removes the obsolete mark from historical undo snapshots.

---

### 5.6 Deck lifecycle

The owner marks each deck **Paper** (a physical deck), **Theorycrafted** (an
idea being explored), or **Legacy** (a read-only archive). Lifecycle controls
editing permissions (§5.2); per-card tags remain independent (§5.4). Changing
lifecycle never changes the card list or its revision.

The UI shows one Lifecycle control in the deck header and one column in All
decks. New decks and forks default to Theorycrafted. Schema 9 preserves
existing categories and removes the Draft/Active column; it does not infer a
lifecycle from the former flag. The internal field remains `category`.

The sidebar labels Theorycrafted decks in blue and Legacy decks in amber;
Paper decks have no extra sidebar label. All decks lifecycle pills use the
same colors, with green for Paper.

Only Paper decks contribute to All decks statistics and `player_stats`.
Changing lifecycle immediately updates statistics. Fork-family ancestry is
preserved through Theorycrafted and Legacy ancestors, whose cards and totals
are excluded.

All decks has a menu for rename, change lifecycle and delete. Agents can
rename or delete Paper and Theorycrafted decks through `rename_deck` and
`delete_deck`; lifecycle changes are owner-only and have no MCP tool. Legacy
rename and delete actions are disabled. Deletion permanently removes the
associated records, including deck-scoped memories; global memories and
fork decks remain. Agent actions are attributed in Activity.

---

### 5.7 Deck power: target and current assessment

Power is a structured description of gameplay, with an explained suggestion
for the community's Commander bracket (1–5). It is not an averaged score,
a certification or an automatically evaluated card list.

**Two independent profiles.** The optional target describes the experience
being built toward. The optional current assessment describes one exact deck
revision. Both describe intent (theme, social, strong synergy, optimized,
cEDH metagame), usual win-turn range, an occasional early win, consistency,
resilience, interaction, win conditions, infinite combos, extra turns and
mass land denial. Named choices have gameplay anchors. Unknown is allowed;
new decks receive no guessed power profile. The usual range describes normal
win attempts under representative conditions, not a perfect opening draw.
The target's range is acceptable; a current range contained within it fits.
An occasional early win is an outlier, not a frequently accessible line.

The target also sets an optional maximum Game Changer count and freeform
preferences. Actual Game Changers are computed from Scryfall's nullable
`game_changer` flag, retained on ingestion. Commanders and companions count;
maybe cards do not. Missing metadata stays unknown and prompts a card-data
refresh. No hand-maintained card list or agent-supplied count is accepted.

**Mapping.** The app translates intent, speed and declared restricted play
patterns into a suggested bracket, reports which inputs led there, and calls
out unknowns and conflicts. It follows Wizards' October 2025 expectations and
February 2026 update, with source links and a guidance version in the view.
Speed refers to usual win attempts: a usual turn-7 or turn-8 win exceeds Core's
eight-turn development expectation; turn 6 or earlier exceeds Upgraded's
six-turn expectation. Outliers alone do not raise a bracket. Game Changers,
early two-card infinites, chained extra turns and mass land denial may impose
a higher ordinary bracket. Thematic exceptions in Exhibition need pod
agreement. Fast casual decks do not become cEDH without competitive metagame
intent. Descriptive dimensions are not averaged, and cheap interaction alone
never determines power. The result is a conversation aid; ambiguous gameplay
needs judgment and testing. This is separate from Commander legality (§9).

**Assessment evidence.** Every assessment carries author, creation time,
revision, evidence (`list`, `goldfish`, `games`), confidence (`tentative`,
`supported`) and required reasoning. Notes are linted for card references.
The app makes no model calls: an owner or connected external agent supplies
judgments. Agents must not fabricate playtest observations. Assessments are
retained as history (the most recent 20 are returned), with captured Game
Changer facts for the assessed list. Current-list bracket suggestions use
current Scryfall metadata; historical lists use captured facts. A card change
flags the assessment for review; submission against an outdated revision fails
with the current revision. Slots, tags and target edits do not
invalidate it. An agent cannot replace the owner's assessment of the same
revision; after the list changes it may assess the new revision.

**Permissions.** The owner sets or clears the target directly. Agents always propose target changes on Paper and Theorycrafted decks through the existing
document-edit workflow (§6), with a reason. The owner sees a readable diff,
including a warning if the target changed since proposal. Assessments may be
recorded by agents on Paper and Theorycrafted decks without approval, are visibly
attributed, and land in Activity. Target and assessment writes never change
membership or bump the deck revision.

**UI and tools.** A Power tab compares target and current values, shows
reasoning, evidence, gaps and bracket explanations, and edits each separately.
The header opens it. Differences from target and stale assessments surface in
Checks. `get_deck` includes a selectable `power` section with field anchors;
`propose_power_target` and `assess_deck_power` provide the write capabilities.
Forks inherit the target and start unassessed. Deletion removes target and
assessment history along with the deck.

Sources: [Scryfall card objects](https://scryfall.com/docs/api/cards),
[Wizards bracket expectations](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025),
[Wizards February update](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026).

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
and accepts or rejects it. Agents always propose brief changes on both Paper and Theorycrafted decks; Legacy is read-only.

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
- **Revise before adding.** `remember` returns existing memories that resemble the new one, so the agent can update or supersede instead of duplicating. Similarity search filters to reachable scopes before taking its top results; unrelated decks cannot crowd out duplicates.
- **Nothing is deleted by an agent.** Superseding keeps the old row, marked and linked. Retiring hides a memory from loading and search but keeps it on disk.
- **The owner outranks the agent.** An agent cannot edit, supersede, retire, restore **or confirm** a memory the owner wrote or edited. Confirm changes its epistemic basis, so it is subject to the same owner-control check as an edit. It may write a new memory that disagrees and say so.
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

On a Paper deck, an agent changes membership by calling `propose_changes`.

- A proposal has a one-line summary and **any number of items**. The agent sizes it to what was asked for.
- An item is an add or a cut of one copy of one card, with a reason.
- A cut may be marked `to_maybe`: if accepted, the card moves to the maybe list (§5.4) instead of leaving the deck, keeping its slot and tags. It is off unless the agent sets it, and the owner sees it on the item before ruling. `edit_cards` takes the same flag on a Theorycrafted deck. A card with several copies cannot be parked this way, because the maybe list holds a card once.
- Items may be **grouped**. A group is ruled on as a unit, so a swap justified by one argument cannot be half-accepted.
- The app tracks the pending delta against 100 and against each slot target, and returns it with the proposal.
- The response also returns, for each card, its linked memories and prior rulings in this deck (§8.3), so the agent sees what it should have considered.

### 8.2 Rulings

The owner rules in the app: per item, per group, or the whole proposal at
once. A ruling is accept or reject with an **optional free-text reason**.
There are no reason types. “Accept all” sends one atomic batch, with the
revision read at the click and stale acknowledgements only for proposals that
were already stale then. The capability checks all bases before applying any
proposal; conflicts roll back the whole batch. Changes within that batch do
not require another stale acknowledgement. Older proposals use the app's
ConfirmDialog, never a browser-native confirmation. Pre-existing proposals
and deck document edits with a missing baseline are anchored once to the
deck's revision during upgrade; subsequent edits make them stale normally.

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
owner makes to a Paper deck. What an agent did itself, and the owner
assembling a Theorycrafted deck card by card, are not queued.)* `start_session` reports how
many rulings are unreviewed, `list_rulings` returns them, and
`mark_rulings_reviewed` clears them once the agent has decided what, if
anything, is worth a memory. A reason given in the app is never lost for lack
of a listener. `through` clears only the deck (or profile scope) of the
anchor ruling; a cross-deck result gives explicit IDs to mark instead.

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

One process serves the UI, HTTP API and Streamable HTTP MCP endpoint. Both
front doors pass an authenticated `Actor` to the same capability layer (§17).
Local mode binds loopback and builds the implicit user-1 UI actor without a
login. Hosted mode binds all interfaces; until Phase 2 supplies sessions its
UI API returns 401. Health remains unauthenticated.

MCP always requires a bearer token. Personal agent tokens are per user,
`dbk_` plus 32 random bytes in base64url, hashed at rest, and shown exactly
once when created. Settings lists labels, prefixes and last-used timestamps,
creates tokens and revokes them. It never returns stored secrets. Verification
looks up the prefix and compares hashes with `timingSafeEqual`; usage updates
at most once per minute. `DECKBUILDER_TOKEN` is removed. On first local boot,
the old `data/agent-token` file is imported, hashed for user 1 and deleted only
after durable insertion; existing HTTP client configurations remain valid.
The migrated credential is the deliberate exception to the new `dbk_` format.

The stdio shim proxies to the running endpoint. Because hashes cannot recover
a token, its command accepts `--token-file <private-file>` (or `--token <token>`
for clients that supply arguments); users save the once-shown token privately.
No shim read creates or rotates credentials. Missing credentials fail clearly.
Configuration snippets use a placeholder until creation, then the once-shown
value. Existing HTTP Claude Code and Codex configurations need no changes.

Host and Origin checks precede the SDK. Local requests require loopback
Host/Origin; hosted requests require the configured public origin. Unauthenticated
MCP requests return 401 and `WWW-Authenticate: Bearer resource_metadata="<origin>/.well-known/oauth-protected-resource/mcp"`;
Phase 2 implements metadata and the OAuth JWT path beside personal-token
verification. A URL suffix (`/mcp/claude-code`) is an attribution hint only:
it never changes the principal or token ownership and cannot impersonate UI.

Use the official MCP TypeScript SDK v2: the same stateless handler supports
2026-07-28 and 2025-revision clients. No connection carries tenant state;
every request resolves its bearer and every tool names its deck.

### 10.2 Design principles

- **Stateless.** Every call names its deck. There is no "current deck" on the server.
- **Few tools, shaped around tasks,** not one per database operation.
- **Results are compact text meant to be read,** grouped the way the deck is organized. Comparison and proposal-management workflows also return MCP `structuredContent` so agents consume stable IDs, quantities, evidence and prices without parsing prose. `get_deck` takes a `sections` argument so re-reads are cheap, and the full view must stay well under a typical client's tool-result cap.
- **Writes return the new state:** card count, slot deltas and check results come back with the write, so the agent never re-fetches or counts.
- **Errors say how to fix the call.** A failed name resolution lists suggestions; an `edit_cards` on a Paper deck says to use `propose_changes`.
- **Annotations are honest:** read-only tools are marked read-only; `delete_deck` is marked destructive.
- **Tool order is deterministic,** for client-side prompt caching.

### 10.3 Tools

| Group | Tool | Purpose |
| --- | --- | --- |
| Orient | `start_session` | Playbook, profile, global memories, deck roster (one line each), the owner's focus, unreviewed-ruling count, memory budget state |
| | `player_stats` | Computed across Paper decks only: colours, commander types, curves, slot habits, most-reused cards |
| Read | `get_deck` | Brief, list grouped by slot with oracle text, computed state, checks, deck memories, pending proposals, recent rulings; `sections` selects parts |
| | `compare_decks` | Cards only in one, only in the other, and shared; defaults to a fork's parent |
| | `compare_deck_sources` | Target versus multiple sources: shared cards, persistent ownership evidence, availability, timestamped price estimates and computed totals; optionally adds source tags with revision guards |
| | `list_proposals` | Current card/document proposals, revision bases, stale flags, supersession and preserved history |
| | `search_cards` | §4.2 |
| | `get_cards` | Exact-name lookup of several cards: full text, linked memories, and rulings if a deck is named. Also the way to check that names are real |
| | `list_rulings` | By deck, by date, or unreviewed only |
| | `search_memories` | Full-text, filtered by scope, deck or card |
| | `export_deck` | Archidekt-importable text (§12) |
| Write freely | `create_deck` | New Theorycrafted deck; supplied thesis becomes a proposal |
| | `rename_deck` | Rename a Paper or Theorycrafted deck |
| | `delete_deck` | Permanently delete a deck and its associated records when requested |
| | `fork_deck` | §5.3 |
| | `edit_cards` | Add and remove cards, set commanders; Theorycrafted only |
| | `define_slots` | Create, rename, retarget and remove slots |
| | `file_cards` | Assign a primary slot (`slot`), replace secondary purposes (`secondary_slots`), and manage tags on cards already in the deck |
| | `record_ownership` | Explicit ownership/availability evidence independent of deck membership; unknown quantities stay unknown |
| | `shortlist_cards` | Add to or remove from the maybe list, with a note |
| | `remember` | §7.3 |
| | `revise_memory` | Update, supersede, retire or confirm |
| | `mark_rulings_reviewed` | §8.4 |
| Propose | `propose_changes` | §8.1 |
| | `propose_brief_edit` | §6 |
| | `propose_profile_edit` | §6 |
| | `propose_power_target` | Propose a target change on editable decks (§5.7) |
| | `propose_document_changes` | Related brief/power-target changes in one atomic owner approval group; explicitly supersede prior pending drafts |
| | `revise_proposal` | Amend or withdraw the calling agent's own pending card/document proposals, retaining history and links |
| Write freely | `assess_deck_power` | Attributed, revision-bound gameplay assessment (§5.7) |

**Focus.** The app records what the owner has open in the UI (which deck,
which cards are selected). `start_session` returns it and `get_deck` accepts
`"focus"` as the deck, so "this deck" in conversation means the one on
screen.

Ownership evidence persists after cutting, moving, untagging, forking or deleting
a deck. An explicit `Have` tag records ownership with unknown physical quantity
and availability. Source-deck appearances and `Keep` preferences do not prove
ownership; source quantities must never be added into a physical inventory.
Unknown ownership is separate from an explicit statement of being unowned.

Proposal amendments create linked replacements. Superseded/withdrawn drafts
cannot be ruled on; their payloads and earlier owner rulings remain in history.
Naming one document-group member for approval, rejection, withdrawal or
supersession covers the whole group. Pending proposals record their membership
revision and document bases (including engines). Changed bases/revisions require
owner review and explicit stale acknowledgment before acceptance; expected
revisions reject conflicting writes. Checks are recomputed in the returned state.
The inbox shows each approval group once, with read-only history available.

See [MCP workflow examples and migration details](deck-builder-mcp-workflows.md)
for tool arguments, evidence semantics, response fields and limitations.

**Owner-only actions** have no tool: ruling on proposals and document edits,
changing a deck’s lifecycle, importing over an existing deck,
undo.

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

- **Decks:** list, create, fork, delete; Paper, Theorycrafted or Legacy lifecycle.
- **Deck view:** the list grouped by slot or by card type, slot targets and deltas, the maybe list, the brief, a live checks count, tags, direct editing, card search.
- **Proposals inbox:** everything waiting on the owner across decks. A proposal is ruled on in its deck's list, where each change is drawn in place among the cards around it — per item, per group or all at once, with an optional reason; the inbox links there. Proposed brief and profile edits are ruled in the inbox as diffs. (Settled 2026-10-10: the owner preferred reviewing in the list to ruling from the inbox.)
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

Construction and desktop solo goldfishing happen here. The manual playtester
was added on 2026-10-13; see [its requirements](playtester-design-brief.md)
and [implementation guide](playtester.md). Archidekt interop remains available.

- **Export:** an Archidekt-importable list. With active name or tag filters, **Export matches** opens an export restricted to matching cards, including inverted tag filters. The export dialog can switch between current matches and the whole deck, independently include or exclude the maybe list (excluded by default), and toggle categories. Filtered exports default to plain quantity-and-name lines for a buying list. Included maybe-list cards use `Maybeboard` when categories are enabled. Quantities are preserved; commanders and companions are eligible, and pending proposed additions are excluded. An empty selection exports nothing. Export counts describe the exported copies and lines, and exporting changes no deck records.
- **Import:** the owner pastes a list; the app diffs it against the deck, shows the diff with destructive cuts called out first, and applies it in full on confirmation. It is one rulings-log entry, so one undo reverts it. Lines in a `Maybeboard` or `Sideboard` category, or under a section header of that name, are shortlisted on the maybe list, not added, and the preview says so. A `Commander` category or section header puts the card in the command zone, including a card already in the deck.
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
| Images use ingested facts | Scryfall bulk `image_uris` (card or front face), returned by table lookup/search/`get_cards`; all browser images use these stored CDN URLs and load lazily. Missing imagery has a placeholder; no `cards/named` URL is constructed. |

**Not guaranteed:** what an agent says in its own chat window. The app cannot
lint text it never sees. The playbook's citation rule and the `get_cards`
lookup reduce this; they do not remove it. The same goes for an agent
reasoning wrongly from correct card text about timing, layers or replacement
effects.

---

## 14. Build order

Each phase is verified before the next begins, and each is testable without
the one above it.

1. **Capability core.** Card database and search; decks with lifecycle permissions, forks, slots, tags, maybe list; checks; the rulings log with undo. Tested with no UI and no agent.
2. **Agent interface, read side.** MCP endpoint with token and Origin check, stdio shim, `start_session`, the read tools, the playbook. Verify by connecting a real agent and reading decks.
3. **The gate.** `edit_cards`, organization tools, `propose_changes`, prior rulings following the card, linting on write; the proposals inbox and live UI updates.
4. **Memories and documents.** The memory store, budgets, the review queue, the profile, proposed document edits, the Memories page and activity feed.
5. **Cross-deck.** `player_stats`, `compare_decks`, focus.
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

Checked on 2026-10-11, when prices were added (§4.4):

- **Scryfall's price data.** Read from the live `default-cards` file: 118,624 printings reduce to a price for 32,974 of 34,890 cards in about seven seconds. The unpriced remainder is digital-only cards and cards no counted printing prices.
- **Bulk downloads decode correctly.** The oracle-cards download decoded each chunk separately, which corrupted a character that straddled two chunks: two cards in the owner's database had a mangled type line. Both downloads now decode as a stream; a refresh repairs the rows.

Checked on 2026-10-11, when deck power profiles were added (§5.7):

- Capability and protocol tests cover bracket boundaries, unknown metadata,
  target approval and diffs, owner corrections, stale revisions, attribution,
  forks, deletion, and schema migration preserving document-edit IDs.
- A scripted session through the official MCP client read a demo deck,
  recorded an illustrative assessment and proposed an active-deck target.
  Its transcript was read. No agent model's assessment quality was measured.
- Browser verification on an isolated database copy covered the target editor,
  owner assessment editor, live agent assessment updates and target approval.
  The comparison and dialog were inspected at desktop sizes.
- The localhost app was rebuilt and restarted. Existing deck contents and
  documents were verified against a pre-upgrade SQLite backup; targets and
  assessments remain unset until deliberately written. Game Changer metadata
  was populated from the installed Scryfall cache, without downloading it.

Checked on 2026-10-25, v3 Phase 0 hardening:

- A forced live Scryfall refresh on an isolated copy completed in 12.95 s:
  35,134 active cards and 33,327 priced. The child peaked at 246.21 MiB RSS;
  all 124 concurrent HTTP probes succeeded, maximum 6.55 ms. The serving
  process stayed running through download, ingest and price swap.
- An online backup with the serving process running restored to a separate
  database, served 19 decks and returned healthy status on a second port.
  Automated tests also verify committed WAL content, retention and refusal
  while the server lease is held. Optional S3 delivery needs credentials and
  has not been verified against a real bucket.
- Chrome verified the seeded demo, a loaded CDN image, stale accept-all's
  ConfirmDialog and successful ruling, and styled proposal history. An
  automated hover rendered the stored CDN URL with lazy loading; browser
  resource entries showed zero `api.scryfall.com` requests. T3 snapshots
  timed out; Chrome supplied visual inspection and T3 evaluation supplied
  the hover/network evidence.
- The official MCP client transcript was read through `start_session`,
  `get_cards` (including structured image URLs), deck-scoped `list_rulings`
  and `mark_rulings_reviewed`: other decks' unread rulings remained unread.
- `npm run check` passes 449 tests on Node 26.12.1, and the same suite
  passes on the declared Node 24.12.0 floor.
- Phase 0 keeps schema version 10's existing additive-column migration style;
  image/retirement columns and missing proposal baselines upgrade on open.
  Phase 1 owns the ordered migration framework and schema 11 tenant rewrite.

## 16. Not verified

Ownership-aware sourcing and proposal cleanup were added on 2026-10-15.
`npm run check` passes 422 tests, including migration, evidence persistence,
multi-source tagging, grouped approval, supersession, stale revisions and both
MCP protocol eras. A scripted official-client transcript against an isolated
database copy was read through sourcing, amendment and owner approval. At that time visual
browser QA could not complete because preview automation failed. Phase 0
subsequently verified the current approval/history controls on an isolated
copy (see §15); the live owner database was not upgraded during that drill.

Everything in §1–§14 is built. What has not been checked:

- Any client other than Claude Code and Codex.
- How agents use prices. The price beside each card, the deck's estimated cost and the playbook's rule about them are pinned by tests, but no real transcript has been read since they were added.
- ChatGPT on the web or phone. Its servers cannot reach a loopback address. The route is OpenAI's Secure MCP Tunnel running the stdio shim; the Settings page gives the commands from OpenAI's guide, but they have not been run, because they need a tunnel created in the owner's OpenAI Platform organization.
- The evaluation was run once per task, on one model, with tasks written by the same hand as the tools. It shows the interface works; it does not measure how often.


## 17. Accounts, modes and roles

### 17.1 Modes and users

`DECKBUILDER_MODE=local` is the default. It binds `127.0.0.1`, has no login
page, and uses implicit user 1 for the UI while MCP requires a token. This is
the owner's daily product. `DECKBUILDER_MODE=hosted` binds all interfaces and
requires `DECKBUILDER_PUBLIC_ORIGIN`; authentication secrets and sessions are
Phase 2 work. Phase 1 rejects hosted UI API access with 401. Invalid modes
fail startup instead of falling back to local.

One SQLite file holds shared card facts and tenant data. `users` has
`id INTEGER PRIMARY KEY AUTOINCREMENT`, unique case-insensitive `handle`
(`[a-z0-9_-]{3,24}`), `display_name`, unique nullable `auth_id`, `is_admin`
(default 0), `created_at` and nullable `deleted_at`. Migration creates user 1
(`owner`, `Owner`, admin, no auth id). Phase 2's first hosted sign-in claims
that user when it is the instance's sole unclaimed account, assigning the
provider identity, handle and display name without export/import.

Schema 11 uses ordered `{ version, up(db) }` migration steps, all within one
transaction after database identity validation. It preserves old migration
steps, Phase 0 image/retirement facts and proposal baselines. All existing
rows backfill to user 1. `decks` names are unique within a user, case
insensitively; decks, memories, document edits, log, activity and ownership
evidence carry `user_id`. Deck-child tables inherit tenancy through their
validated deck. Profile has one row per user; playtest sessions use
`(deck_id, user_id)`. `user_settings(user_id,key,value)` replaces `meta`
`setting:*` keys (focus and memory budgets); card freshness and jobs remain
global. Deleted users cannot authenticate.

### 17.2 Actor and permissions

```ts
export type Actor = {
  user_id: number;            // principal: whose data is being touched
  via: "ui" | "agent";
  label: string;              // "ui" or agent attribution label
  token_id?: number;          // authenticated personal token, when present
};
```

Ownership is a relation: `actor.user_id === decks.user_id`.
`requireDeckOwner(db,actor,deckId,what)`, `requireSelf(actor,userId)` and
`isDeckOwner` enforce it below both front doors. `requireDeck` returns the
deck and caller role; an unreadable deck is 404. Writes on another user's
deck throw `PermissionError` (403). Every exported capability requires an
actor. Shared card-only utilities remain global, but any deck or memory
context is authorized. No implicit actor is supplied by a service.

V3 roles are owner, viewer (visibility admits them), and suggester (signed
in on a deck accepting suggestions). No collaborators, co-owners or members
table. Phase 1 admits only owners; visibility and suggestions arrive later.
Lifecycle, rulings, undo, import, deletion and visibility belong to the deck
owner; existing human-only actions additionally require `via === "ui"`.
Agents retain attributed rename/delete on their principal's editable decks.
No suggester may rule on their own proposal, including agent-via proposals;
the existing owner-agent proposal reviewed by that owner's UI remains the
local workflow. Paper membership always needs human approval. All current
`owner_touched`, memory basis and review semantics distinguish `via`, never
ownership or a self-asserted label.

Profile edits, global memories, budgets, focus, stats, inbox, review queue,
activity, ownership evidence and token management are per principal.
`include_my_decks` means that principal's decks. Parent and fork sources must
belong to that principal in Phase 1; card-history lineage stops at unreadable
boundaries. Missing IDs never permit global/profile scope to bypass tenancy.

### 17.3 Attribution, tokens and live updates

Every existing `author` column gains `author_user_id` (FK users),
`author_via` (`ui` or `agent`) and `author_label`, all NOT NULL. Old `owner`
authors backfill to user 1/ui/ui; other labels to user 1/agent/original label.
The existing author string remains for wire compatibility; principal/via/label
are authoritative for permission checks. Attribution renders the human's
handle for UI and `handle via label` for agents; a future external author
renders `@handle`. The local `owner` handle preserves existing UI attribution.
Activity records the owning `user_id` plus `actor_user_id`, `actor_via` and
`actor_label` for every agent write.

`agent_tokens(id,user_id,label,prefix,token_hash,created_at,last_used_at,revoked_at)`
stores hashes only. Token create/list/revoke require the principal's UI;
revocation takes effect on the next request, including an already-created MCP
handler. URL labels cannot grant another token's amendment rights: attribution
checks include principal and agent label, with the token's configured label
as fallback. Token credentials resolve to an Actor before tools execute.

SSE listeners use `Map<user_id,Set<response>>`; changes emit
`notify(user_id,{deck_id?,kind})`. UI and MCP writes notify only that user.
The browser initially refetches all queries for its own stream; the payload
names what changed for later selective refetch. Global job changes may notify
all connected users with `kind: "jobs"` and reveal no tenant data.

### 17.4 Phase 1 choices and handoff

Profile rows are created lazily for new users (empty profile reads remain
valid); user 1 is seeded on migration. This avoids an identity-registration
API before sessions exist. Shared card ingestion/backup functions are operator
utilities rather than tenant capabilities; HTTP refresh additionally requires
`users.is_admin=1`. Token management is UI-only to avoid granting an agent a
way to mint durable credentials. Legacy token import is idempotent by hash,
then file deletion follows the committed insert, so a crash cannot lose access.
No fresh token is minted at boot: the human explicitly creates and saves it
once in Settings.

Phase 2 plugs its session resolver into the UI Actor-building function in
`app.ts` and its OAuth access-token verifier alongside `verifyToken` in
`tokens.ts`/`mcp/http.ts`. It inherits per-user data and streams, with no
login, sessions, Better Auth, visibility, public pages or renames in Phase 1.
Hosted signup will be invite-gated (D14); the invite table and UI arrive with
Phase 2 account creation, not in this identity-only phase.
