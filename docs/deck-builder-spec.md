# MTG Commander Deck Builder — Build Spec (v3)

Current deck/account behavior and public compatibility. Section numbers remain
stable for source comments. Browser implementation and visual rules belong to
[Multiverse architecture](../packages/multiverse/web/ARCHITECTURE.md) and
[design](../packages/multiverse/web/DESIGN.md); operating commands belong to
[operations](../packages/deck-builder/OPERATIONS.md). Historical decisions and
build receipts are recoverable at Git revision `33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb`.

## 1. What this is

A Commander deck library driven by humans and external MCP agents through one
Actor-scoped capability layer. The app stores validated records, computes state
and audits writes; interpretation and conversation happen outside it.

### Design rules

Exact installed card facts, computed counts/checks, human approval of Paper
membership, attributed visible agent writes and authorization below both front
doors are mandatory. Full capability constraints live in the package manual.

## 2. Non-goals

No model calls/embeddings, chat/transcripts, external recommendation data, sync,
printing inventory/reservations, collaborators/co-owners or non-Commander formats.
Scryfall supplies card facts/prices; stored CDN images load on demand. Local and
hosted accounts/sharing are implemented; suggestions and native full-game launch
retain separate gates. The library does not depend on Card IR or engine support.

## 3. The knowledge model

| Kind | Authority and use |
| --- | --- |
| Records | App-owned cards, decks, decisions/proposals, counts and Activity; read on demand |
| Documents | Owner's global profile/per-deck brief; agents propose edits |
| Memories | Attributed preference/reason/outcome statements; loaded by scope and searched |

Never copy a computable record or card text into memory. Documents are confirmed
owner voice; memories capture learning, with inferred statements marked explicitly.

## 4. Card data and search

### 4.1 Card data

The independent SQLite/FTS5 store ingests Scryfall `oracle_cards`, one representative
per Oracle identity. `oracle_id` keys facts; retain exact name/cost/value/type/text,
characteristics, colour identity, Commander legality, layout/faces, nullable Game
Changer flag, stored image/art URLs/artist and retirement. Never derive identity
from text. Commander eligibility is a separate check. `src/cards.ts` owns the
consumed field contract; [operations](../packages/deck-builder/OPERATIONS.md#card-refresh-and-scheduled-jobs)
owns schedules, staging, thresholds, locks and failure recovery.

Absent IDs retire rather than delete: references/history/images remain resolvable,
search hides them except in a named containing deck, checks flag them, and returning
IDs reactivate. Images are ingested facts, never runtime name lookups. Malformed or
truncated refreshes cannot delete deck contents. Token/price refresh has its own
atomic publication (§§4.4–4.5).

### 4.2 Search

A subset of Scryfall query syntax: `t:`, `o:`, `cmc`/`mv` with comparators,
`id:`/`id<=`, `c:`, `pow`/`tou`, `usd`/`eur` with comparators (§4.4), `is:`, extended
with `slot:` and `tag:`.
Verify each operator's semantics against Scryfall's syntax documentation and
cite it in a comment.

When a search names a deck:

- Results are pre-filtered to the deck's colour identity, so illegal cards never come back.
- Each result is marked if it is already in that deck or on its maybe list.
- Each result carries any memories linked to that card (§7.2) and any prior decisions on it in that deck (§8.3).

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

Things the app mints (decks, proposals, memories, decisions) are referred to by
short integer ids, always shown beside their names.

### 4.4 Prices

Prices estimate the cheapest playable paper printing per store, in whole cents,
not the owner's actual copy. Only Scryfall `default_cards` supplies prices:
TCGplayer USD and Cardmarket EUR, across nonfoil/foil/etched values when present.
Exclude oversized/memorabilia; ties use set, collector number, then finish order.
Sources: [Card objects](https://scryfall.com/docs/api/cards),
[price sources](https://scryfall.com/docs/faqs/where-do-scryfall-prices-come-from-7),
[syntax](https://scryfall.com/docs/syntax#prices).

Per-user currency/store selects USD or EUR and its compatible store/Lowest.
Lowest compares within that currency; no exchange rate or cross-currency fallback.
Missing/corrupt preferences resolve USD/lowest. `setPriceSource` is UI-only and
validates/saves the pair atomically; explicit null rejects writes. Every personal
consumer uses the same actor-taking resolver; public reads use viewer or explicit
null (USD/lowest), never owner preferences. Agents read prices but cannot set source.

Wire fields are `cents`, `currency`, `store`, `as_of`, `source: "Scryfall"`.
Label estimates with store/date; missing price is unknown, not free. Totals count
copies/companions, exclude maybes, and report distinct unpriced cards. Pending
projections simulate one-copy changes/whole-card parking in order; unpriced delta
is signed. Price-bearing tool responses label source once; exports/history/memory
writes stay price-free. `get_cards` includes structured price metadata.

`usd`/`eur` comparisons use that currency's cheapest playable printing under the
chosen compatible store, otherwise Lowest. Unpriced cards match no comparison,
including `!=`; negating a comparison includes them. Bulk data older than 24 hours
is stale, never a live offer. No price history, buy links or printing collection.

Refresh streams bounded staging batches, then atomically replaces both stores/date;
failure preserves old quotes and missing new quotes become unpriced. Marker/version
changes force boot refresh even with a recent timestamp; failure retries hourly.
Operations owns schema/refresh commands and the `(oracle_id,store)` storage detail.

### 4.5 Tokens and emblems

Deck reads return distinct sorted Scryfall-related tokens/emblems with stored
characteristics/images and visible producers. Main membership, commanders and
companions contribute; maybes/pending proposals do not. `producer_count` counts
producing cards, not token quantities. Tokens never enter membership/counts/checks/
prices/exports. Same names with different Oracle IDs remain distinct. These are
reference relationships, not rules evaluation, quantities or inferred text.

Global token/link tables publish atomically with prices after bounded staging and
coverage/link validation. Failure retains prior facts/date; retirement retains
known links. Missing catalog marker forces refresh. `tokens_status` distinguishes
uninitialized from ready-empty and carries source date. Serving uses local joins;
`get_deck sections:["tokens"]` and allowlisted public state expose the same facts.

Related printing IDs resolve to token Oracle IDs; `combo_piece` identifies emblems,
with Flip Role/reversible faces supported. Rare non-English related printings may
use a bounded Scryfall collection lookup, the only refresh-time bulk supplement.
Sources: [relationships](https://scryfall.com/docs/api/cards#related-card-objects),
[collection](https://scryfall.com/docs/api/cards/collection). No card-name lists or
browser Scryfall API calls.

## 5. Decks

### 5.1 Model

- A deck targets exactly **100 cards** including commanders.
- **Commanders are a list.** Zero to two, plus Background, plus Companion (outside the 100).
- Each deck row stores its denormalized colour identity. Changing the commander may invalidate cards; that surfaces as a check finding (§9), never as silent corruption.
- Each deck-card row: card, quantity, role (`card` / `commander` / `companion`), optional primary slot, secondary slots (zero or more), tags (zero or more), `maybe` (on the maybe list, outside the 100). Zone and purpose are independent: commanders and companions may also have slot assignments.
- Every change to membership bumps the deck's **revision** number.
- Each deck has a category: `paper`, `theorycrafted`, or `archived` (§5.6).

### 5.2 Lifecycle and editing permissions

A deck’s lifecycle is **Paper**, **Theorycrafted**, or **Archived** (§5.6).
There is no separate Draft/Active flag. Only the owner changes lifecycle.

| | Theorycrafted | Paper | Archived |
| --- | --- | --- | --- |
| Agent changes cards | Directly (`edit_cards`) | By accepted proposal | Read-only |
| Agent changes brief or power target | By accepted proposal | By accepted proposal | Read-only |
| Owner edits | Directly | Directly | Read-only |
| Organization, memories, assessments | Direct writes | Direct writes | Read-only |

New agent-created decks and all forks are Theorycrafted. The owner may create
Paper or Theorycrafted decks and archive them as Archived. Archived decks can be
viewed, exported, compared and forked, but neither the owner nor agents can
rename, delete, change cards, change organization, revise deck memories, or
rule on proposals while archived. The owner can restore them to Paper or
Theorycrafted. Pending proposals stay stored, leave the actionable inbox
while archived, and return on restoration. All writes are attributed and
membership changes retain their revision and history behavior.


### 5.3 Forks

`fork_deck` copies the card list, organization, brief and deck memories into a
new Theorycrafted deck, and records the parent. The copies then diverge independently; the
parent link exists so the two can be compared (`compare_decks` defaults to
the parent) and so an agent working on the fork can read where it came from.

The owner can also record a parent on an existing deck, or clear it: a deck
copied by hand has no link until one is set, and with one it compares and
inherits history like any fork.

Forks replace named checkpoints. There is no snapshot system and no DAG.

**Forks across owners** (phase 4a, §18.4). A signed-in user, or their agent,
may fork another user's unlisted or public deck by its link. That fork copies
less: the list (maybe list included, without its private notes), slots with
their targets and both primary and secondary assignments, the brief's thesis
and constraints, and engines with their pieces. It never copies tags (they
record the other owner's sourcing and status, and "Have" is ownership
evidence), memories, ownership evidence, the power target or assessments,
proposals, playtest state, or any log entry. The new deck is the forker's:
Theorycrafted, private, with one log entry naming its source. It records a
snapshot of the parent's owner handle, slug and name, so "forked from
handle/name" survives the parent going private, being renamed or being
deleted. It does not record the parent's numeric id, which stays its owner's. Comparison against that parent works while the forker can
still read it; card history never crosses the owner boundary.

### 5.4 Slots, tags and the maybe list

Slots describe purpose; tags describe sourcing/status. Each card has an optional
primary slot and any number of distinct secondary slots, never the same in both
roles. New decks have no slots. Only primary copies count toward soft targets;
secondary counts and total coverage are separate server values. Commanders in the
100 count; parked cards and companions do not. Never derive card count from slots.
Primary assignment groups ordinary membership; the command zone stays separate.
Secondary contributors are expandable, not repeated on every card row.

Tags form a per-deck vocabulary with stable IDs, case-insensitive uniqueness,
reserved-name and near-duplicate validation on creation/rename. Assignments are
unique per deck/card/tag and constrained to same-deck membership by composite
foreign keys. Every copy in an Oracle-card row shares its tags. Global, hierarchical,
named saved-filter or per-copy tags are not modeled. Archidekt categories map to
slots/roles; native tag sets do not round-trip through that format (§12).

The card panel shows assigned chips and a searchable native-checkbox dropdown;
async writes disable choices and preserve focus. Deck filters combine name search
with any/all selected tags, optionally inverted. They cover commanders, companions,
ordinary cards and maybes; proposed additions use existing assignments and new
untagged proposals do not match tag filters. Nonmatches and empty groups hide;
headers show matched/full counts and omit group targets/costs while filtered.
Global totals and review scope remain the whole deck. Clearing restores scroll;
selection and inspection persist. Deleted tags leave filters and assignments.

Organization writes are direct on Paper/Theorycrafted decks, attributed, and do
not bump membership revision. Filing replaces tags and purposes atomically.
Primary-to-secondary promotion removes the duplicate; explicitly assigning both
roles fails. Removing a slot preserves other purposes; parking preserves all
filing; forks remap IDs and undo restores surviving assignments. `slot:` searches
either purpose; `slot:none` means no primary on an ordinary in-deck card. Names
cannot equal a card type (singular/plural) or search prefix; `land destruction`
is valid. `file_cards.remove_unused_tags` removes only explicitly named unused
vocabulary; an assigned tag aborts the batch.

### 5.5 Physical assembly through tags

Physical assembly uses ordinary per-deck tags. **Have is an explicit ownership
statement:** assigning it or renaming an assigned tag to Have records owned status
with unknown physical quantity/availability, preserving an already more precise
owned statement. Removing Have does not retract evidence. Keep, Ordered and source
membership do not prove ownership. Evidence survives membership, tags, forks and
originating-deck deletion; corrections require a new statement with its basis.
Source quantities are never added into an inventory. See [ownership workflows](deck-builder-mcp-workflows.md#record-ownership-without-inventing-quantities).

The removed dedicated assembly field/tool/export is not restored by these tags.
Schema 6 discarded its marks without inventing tags and preserved other fields,
filing and historical undo. Whole-deck lifecycle remains independent.

### 5.6 Deck lifecycle

Only the owner changes Paper/Theorycrafted/Archived lifecycle; it never changes
membership revision. New decks/forks default Theorycrafted. Archived decks remain
read-only until restored (§5.2), except owner-controlled sharing (§18.2). Rename
and delete are available to owners and their agents only on editable decks.
Deletion removes associated records, including deck memories, while preserving
global memories and other forks; agent actions are attributed.

Only Paper contributes to collection/player statistics. Ancestry traverses other
lifecycles without counting their cards. Display lifecycle in neutral text; phone,
tablet and desktop archive notices/navigation follow the product design manual.

The internal field is `category`. Schema 9 removed the former Draft/Active flag;
schema 15 renamed persisted `legacy` to `archived` without changing identities,
relations, sequences or user prose. Historical storage keeps `ruling_reason`,
rendered as `decision_reason`. D19 allowed these beta renames; current compatibility
is additive or explicitly deprecated, with no old-tool aliases (§19).

### 5.7 Deck power: target and current assessment

Optional target and revision-bound current assessment describe intent, usual and
occasional win turns, consistency, resilience, interaction, win conditions, infinite
combos, extra turns and mass land denial. Unknown stays unknown. The target's
acceptable usual range contains a fitting assessment; occasional outliers are not
usual turns. The target may cap Game Changers and carry preferences. Actual counts
come from ingested nullable `game_changer`, including commanders/companions but
not maybes; missing metadata stays unknown and prompts refresh.

Suggested brackets 1–5 explain inputs/conflicts; no dimension is averaged and no
list is certified. Usual turn 7/8 exceeds Core's eight-turn development expectation;
turn 6 or earlier exceeds Upgraded's six-turn expectation. Outliers alone do not
raise a bracket. Game Changers, early two-card infinites, chained extra turns and
mass land denial can impose higher brackets. Exhibition exceptions need pod
agreement; fast casual does not imply competitive metagame intent. The versioned
mapping follows [October expectations](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025)
and [February update](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026),
independently of Commander legality.

Assessments retain author/date/revision, `list|goldfish|games` evidence,
`tentative|supported` confidence, linted reasoning and captured Game Changer facts;
never invent games. Return the latest 20. Current suggestions use current facts,
historical assessments captured facts. Membership changes flag review and stale
writes reject; organization/target edits do not invalidate. An agent cannot replace
an owner's same-revision assessment. Owners edit targets directly; agents always
propose target changes. Assessments write directly on editable decks with Activity;
neither changes membership revision. Forks copy targets, start unassessed; deck
deletion removes power history. The Power view compares values/reasons/gaps and
Checks flags stale/target mismatches. `get_deck` exposes power; tools are §10.3.

## 6. Documents

The owner directly edits one global profile (tastes/playgroup/power/budget, freeform
Markdown, soft cap about 600 words) and a brief per deck (thesis, constraints and
named engines with descriptions/exact required card pieces). Piece presence is
computed, not an agent judgment. Both always load at their scope. Agents propose
replacement profile/brief text with a reason through the document workflow on
Paper/Theorycrafted; Archived is read-only. The owner reviews a diff. Confirmed
learning may move from memories into documents, retiring memories after approval.

## 7. Memories

### 7.1 Shape

A row holds one short statement/reason, linted `text`, global or deck scope,
optional card links, stated/inferred basis, principal/via/label attribution,
optional source decision/revision, timestamps and active/superseded/retired status.
Supersession links the retained old row to its replacement.

### 7.2 Scope and surfacing

Principal-global memories load at session start, deck memories with the deck,
and linked-card memories follow that principal's cards into lookups, searches and
proposal responses. Everything else uses scoped FTS `search_memories`.

### 7.3 Writing

`remember` writes without approval and returns similar reachable memories so an
agent revises before adding; irrelevant scopes cannot crowd out duplicates.
Every scope, source decision/replacement and mutation stays principal-owned.
Agents never delete memories, or edit/supersede/retire/restore/confirm an
owner-touched memory; `actor.via` controls human authority. They may write a
separate disagreement. Confirm changes inferred to stated and is owner-controlled.
Retired rows disappear from normal loading/search but remain stored. Deck-game
observations retain the revision actually observed; never invent evidence or store
computable records/card text/web claims as owner preferences.

### 7.4 Budgets and upkeep

Configurable defaults are about 1,500 estimated global tokens and 1,000 per deck.
Load newest-revised memories that fit, reporting omissions/over-budget scope.
Agents consolidate duplicates, supersede changes, promote cross-deck preferences
and propose durable document edits. The app never uses a model to consolidate.

### 7.5 Visibility

The owner can browse/filter/edit/retire/restore by scope/card/deck with basis,
author/source visible. Agent writes appear in Activity; no hidden learning store.

## 8. Proposals and decisions

Phase 4b names the tools `list_decisions` and `mark_decisions_reviewed`.
The previous names are removed during the invite-only beta (D9/D19). Existing
stored reasons, memories, summaries and briefs keep their original text.

### 8.1 Proposals

On a Paper deck, an agent changes membership by calling `propose_changes`.

- A proposal has a one-line summary and **any number of items**. The agent sizes it to what was asked for.
- An item is an add or a cut of one copy of one card, with a reason.
- A cut may be marked `to_maybe`: if accepted, the card moves to the maybe list (§5.4) instead of leaving the deck, keeping its slot and tags. It is off unless the agent sets it, and the owner sees it on the item before decision. `edit_cards` takes the same flag on a Theorycrafted deck. A card with several copies cannot be parked this way, because the maybe list holds a card once.
- Items may be **grouped**. A group is ruled on as a unit, so a swap justified by one argument cannot be half-accepted.
- The app tracks the pending delta against 100 and against each slot target, and returns it with the proposal.
- The response also returns, for each card, its linked memories and prior decisions in this deck (§8.3), so the agent sees what it should have considered.

### 8.2 Decisions

The owner rules in the app: per item, per group, or the whole proposal at
once. A decision is accept or reject with an **optional free-text reason**.
There are no reason types. “Accept all” sends one atomic batch, with the
revision read at the click and stale acknowledgements only for proposals that
were already stale then. The capability checks all bases before applying any
proposal; conflicts roll back the whole batch. Changes within that batch do
not require another stale acknowledgement. Older proposals use the app's
ConfirmDialog, never a browser-native confirmation. Pre-existing proposals
and deck document edits with a missing baseline are anchored once to the
deck's revision during upgrade; subsequent edits make them stale normally.

Every decision is a row in the **decisions log**: deck, card, action, outcome,
reason, proposal, author, timestamp, and the deck revision it produced. The
owner's direct edits and imports are logged in the same place. This log is the
version history, and **undo is the reversal of a log entry**. An import is one
entry carrying a snapshot of the prior list.

### 8.3 Prior decisions follow the card

When a card is proposed, searched with a deck named, or looked up, the app
attaches earlier decisions on that card in that deck: "rejected at revision 14:
too slow for this list". This is a lookup the app does every time. It replaces
the old hard filter without asking the owner to classify anything and without
depending on an agent remembering to check.

A fork inherits this history from the decks it descends from, each entry
labelled with the deck it was made in *(as built: a fork starts with no decisions
of its own, and the first agent to rework one said it had nothing to read)*.
History does not flow the other way.

An agent may re-propose a rejected card. The playbook requires it to cite the
prior decision and say what changed.

### 8.4 The review queue

Each principal has their own review queue. The owner often rules while no
agent is running. Each decision therefore has a
`reviewed` mark, unset until an agent has read it. *(As built: the queue holds
every decision on a proposal or a proposed edit, and every direct change the
owner makes to a Paper deck. What an agent did itself, and the owner
assembling a Theorycrafted deck card by card, are not queued.)* `start_session` reports how
many decisions are unreviewed, `list_decisions` returns them, and
`mark_decisions_reviewed` clears them once the agent has decided what, if
anything, is worth a memory. A reason given in the app is never lost for lack
of a listener. `through` clears only the deck (or profile scope) of the
anchor decision, within the authenticated principal. Explicit ID batches
validate every tenant before changing any row; a foreign anchor or ID is
404. A cross-deck result gives explicit IDs to mark instead.


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


## 10. The agent interface

### 10.1 Transport and security

One process serves UI/API and stateless Streamable HTTP MCP. Both use verified
Actors and the same capability layer (§17); every tool names its deck. The official
SDK v2 supports 2026-07-28 and stateless 2025 fallback. Host/Origin checks precede
the SDK; configured proxy headers are trusted only behind one explicit trusted
ingress. Local requests remain loopback. Hosted GET `/api/health` alone bypasses
Host/Origin and returns `{ok,schema_version}` without cookies/auth/tenant facts.

MCP always requires a personal bearer or verified resource-bound OAuth token.
Settings creates/revokes personal tokens, reveals secrets once and stores hashes;
URL labels grant no identity/UI/amendment rights (§17.3). Missing/invalid credentials
return 401 with resource metadata (invalid bearer adds `invalid_token`). Hosted
OAuth supports authorization code + S256 PKCE, consent, refresh rotation, CIMD and
native/loopback-only DCR; local mode exposes no OAuth endpoints. JWKS checks pin
issuer, EdDSA signature, expiry, `at+jwt` and exact resource audience. DPoP-bound
tokens reject. Deleted identities/disconnected grants fail even before JWT expiry.

Cookies are HttpOnly/SameSite=Lax and Secure on HTTPS; explicit loopback development
permits non-Secure cookies. Mutations require configured same-origin CSRF checks.
CSP/nosniff/referrer/HSTS and bounded rate limits protect hosted requests. 429 has
Retry-After; unreadable targets are 404, forbidden readable actions 403. Concrete
bootstrap, token-file, redirects and secret commands belong to operations.

Sources: [MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization),
[pinned auth documentation](https://github.com/better-auth/better-auth/tree/v1.7.13/docs/content/docs/plugins).

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
| Orient | `start_session` | User handle/display name, playbook, profile, global memories, deck roster (one line each), the owner's focus, unreviewed-decision count, memory budget state |
| | `player_stats` | Computed across Paper decks only: colours, commander types, curves, slot habits, most-reused cards |
| Read | `get_deck` | Brief, list grouped by slot with oracle text, computed state, checks, deck memories, pending proposals, recent decisions; `sections` selects parts |
| | `compare_decks` | Cards only in one, only in the other, and shared; defaults to a fork's parent |
| | `compare_deck_sources` | Target versus multiple sources: shared cards, persistent ownership evidence, availability, timestamped price estimates and computed totals; optionally adds source tags with revision guards |
| | `list_proposals` | Current card/document proposals, revision bases, stale flags, supersession and preserved history |
| | `search_cards` | §4.2 |
| | `get_cards` | Exact-name lookup of several cards: full text, linked memories, and decisions if a deck is named. Also the way to check that names are real |
| | `list_decisions` | By deck, by date, or unreviewed only |
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
| | `mark_decisions_reviewed` | §8.4 |
| Propose | `propose_changes` | §8.1 |
| | `propose_brief_edit` | §6 |
| | `propose_profile_edit` | §6 |
| | `propose_power_target` | Propose a target change on editable decks (§5.7) |
| | `propose_document_changes` | Related brief/power-target changes in one atomic owner approval group; explicitly supersede prior pending drafts |
| | `revise_proposal` | Amend or withdraw the calling agent's own pending card/document proposals, retaining history and links |
| Write freely | `assess_deck_power` | Attributed, revision-bound gameplay assessment (§5.7) |

Price and ownership semantics are §§4.4 and 5.5; the calling principal
controls resolution. Workflow examples below use actual returned IDs/revisions.

**Focus.** The most recently interacted visible deck/playtest tab owns the
per-user context. Pointer-down, key-down and window focus claim it; hover,
scrolling and visibility alone never steal it. Visible owners heartbeat each
minute even while reading. Hidden tabs stop reporting, but keep last-viewed
read context: stale after three minutes, absent after thirty. Stale implicit
writes are refused; the agent must reread and name the deck explicitly.
Reports have a tab token and increasing sequence; leaving a deck or pagehide
clears only that token's context. A restored bfcache page gets a new token.
My decks/Inbox tabs never clear another tab. The sidebar reads the confirmed
server context and retains its previous line on a failed request.

**Command contract (schema 17).** UI domain writes and MCP mutations use a
shared synchronous transaction containing capability writes, attribution and
result construction. One agent command has one Activity entry. Optional
`expected_revision` checks membership and `expected_edit_version` additionally
checks organization, brief, target and deck metadata. The latter is an opaque
monotonic version, not a count of commands; membership revision keeps its
existing meaning. The guards are additive and optional for existing clients.

MCP writes accept `idempotency_key`; synchronous domain HTTP writes accept
`Idempotency-Key`. User plus verified credential scopes a key, not the URL
label. Identical requests replay their stored outcome for 24 hours without
another write/event; different arguments or expired keys fail. Receipts
survive restarts. Expired response payloads are pruned on the user's next keyed
command; compact tombstones remain until account deletion. Account export and
deletion include receipts. Credentials are still verified on every request.
The browser retries an interrupted domain request once with the same key and
refreshes a replayed deck composite before showing it. Authentication and
once-shown token creation are excluded.

Structured command results carry resource/deck IDs, versions, count changes
and deterministic findings; tool-specific IDs accompany the existing text.
SSE names resource families and deck IDs; cross-deck effects explicitly name
global resources. Reconnect and unknown legacy events invalidate all reads.
Request-local read reuse clears after SQLite writes and never persists between
requests; brief-only reads avoid building the full deck composite.

Proposal amendments create linked replacements; superseded/withdrawn work cannot
be ruled on. Naming a document-group member covers the entire group for ruling,
withdrawal/supersession. Changes to membership/document/engine bases require explicit
owner stale acknowledgment and expected-revision checks; prior payloads/decisions
remain immutable. The inbox presents one group and read-only history.

See [MCP workflow examples and migration details](deck-builder-mcp-workflows.md)
for tool arguments, evidence semantics, response fields and limitations.

**Owner-only actions** have no tool: decision on proposals and document edits,
changing a deck’s lifecycle, importing over an existing deck,
undo.

### 10.4 The playbook

`src/mcp/playbook.ts` owns runtime agent rules; optional SKILL.md adds method.
Server instructions stay self-contained within 512 characters and tell clients
to call `start_session`, which returns the full playbook. Review unread decisions
before proposing; use only cards read/looked up in this conversation, quote the
Oracle clause when reasoning depends on it, use computed values, cite prior
rejections and what changed, and consolidate memories when signalled. Validation
lives below tools, never solely in prompt instructions.

### 10.5 Linting what agents write

Every piece of prose an agent stores (proposal reasons, memory text, brief and
profile edits, shortlist notes) writes cards as `[[Card Name]]`. On write, the
app resolves every reference exactly; one unresolvable name fails the call
with suggestions. Whatever the app displays therefore contains no invented
card.


## 11. The app for people

The product hosts deck editing/review, Inbox, Memories, Activity, Profile and
Settings under the existing account. SSE exposes agent writes without reloading.
Membership, organization, documents and permission behavior remain defined above;
[frontend architecture](../packages/multiverse/web/ARCHITECTURE.md) owns composition,
routes, loading/recovery and API effects. [Design](../packages/multiverse/web/DESIGN.md)
owns controls, typography, layout, art, review/inspection and focus contracts.

### 11.1 Routes *(phase 4a)*

The product's typed `href()` and server page allowlist own current routes and
legacy hash-bookmark conversion. Home, library and Play coexist with existing
deck/shared URLs. Sign-in return paths are same-origin only; unknown pages and
unreadable public targets are 404. Public link previews use permitted data (§18).

### 11.2 Phones *(phase 4a, D17)*

Below 720px, signed-in users can access Home, library, Play setup, read-only decks
and Inbox; preview/game displays retain explicit larger-screen requirements.
Phone writes are proposal accept/reject (individual/group/all) with an optional
reason up to 500 characters, plus cross-owner public-deck forks. The owner's own
public page hides self-fork on phones. Direct edits, playtest, Memories, Profile,
Activity, Settings and design tools require a larger screen. No new capability
is granted by responsive presentation. The owner sees actual agent attribution;
public readers see generic attribution. Decisions preserve draft reasons until
committed and use the same capability layer as desktop.

### 11.3 First run and tablet editor *(phase 4b)*

At 720+px, empty accounts offer New deck, Import and Try the sample deck. Import
always previews before writes; its one-shot query is consumed on opening. Human-only
`createDemo` is transactional and idempotent per user while its private sample
exists (rename does not change that; deletion allows recreation). It resolves
installed facts, returns missing optional cards, fails on a missing commander,
and never replaces user content or writes profile/global memories/ownership.
Illustrative writes use the same principal with reserved label `sample-agent`,
never Activity; a real client choosing it becomes `sample-agent (connected)`.
The deliberately unfinished list demonstrates checks, not completed gameplay.

The first-run voice becomes connected only after real agent writes, excluding
token creation, reads and sample writes, even after sample deletion. Sidebar
presence independently reports last authenticated request, including reads from
non-revoked credentials; it is recent contact, not a persistent connection count.
Visible tabs own context through token/sequence guards and heartbeats (§10.3).

Phone maximum is 719px. The full tablet editor uses a navigation drawer and window
scrolling throughout 720–1099px; desktop panes/optional collapsed sidebar start at
1100px. The requested details sheet preserves drafts, focus and list position.
`DRAWER_BREAKPOINT=960` is not the shell's actual navigation boundary. MultiSelect
uses labelled native checkboxes. Current control and breakpoint implementation
belongs to the frontend manuals, not a duplicated build-phase recipe here.

### 11.4 Visual identity (Phase 4c; D21)

Only exact stored Scryfall art with known artist is admitted. Missing/failed art
has a reserved fallback, no invented facts. Partner crops retain individual
credits; public pages use only public commander facts. Placement, contrast,
loading and motion belong to the design manual. Account backdrop selection uses
shared server-resolved facts, not tenant data or a browser Scryfall API request.

### Desktop statistics and content headers (owner direction, 2026-10-27)

Stats uses server-computed data. Fixed WUBRG cost symbols and recognized mana-source
copies have separate denominators; multi-colour rows overlap, other printed faces
are alternatives, and conditional/unknown output remains explicit. These estimates
are not available mana, casting odds or engine legality. Design owns chart/layout
presentation; the capability layer supplies the same analysis to UI and agents.

## 12. Archidekt and Moxfield interop

Import/export is paste-based Archidekt/Moxfield interoperability, independent of
the legacy manual playtester and native game launch.

- **Export:** an Archidekt-importable list. With active name or tag filters, **Export matches** opens an export restricted to matching cards, including inverted tag filters. The export dialog can switch between current matches and the whole deck, independently include or exclude the maybe list (excluded by default), and toggle categories. Filtered exports default to plain quantity-and-name lines for a buying list. Included maybe-list cards use `Maybeboard` when categories are enabled. Quantities are preserved; commanders and companions are eligible, and pending proposed additions are excluded. An empty selection exports nothing. Export counts describe the exported copies and lines, and exporting changes no deck records.
- **Import:** the owner pastes a list; the app diffs it against the deck, shows the diff with destructive cuts called out first, and applies it in full on confirmation. It is one decisions-log entry, so one undo reverts it. Lines in a `Maybeboard` or `Sideboard` category, or under a section header of that name, are shortlisted on the maybe list, not added, and the preview says so. A `Commander` category or section header puts the card in the command zone, including a card already in the deck.
- One-way out, one-way in. No sync layer.
- **Moxfield import** *(phase 4a)*: the same import dialog accepts the text
  Moxfield's Export gives, with the same preview, one log entry and one
  undo. The format is read off Moxfield's help pages and 72 real exports and
  cited, with fixtures, in `src/deck/interop.ts`: quantity, name, optional
  `(SET) number`, `*F*`/`*E*` and `#tags`; boards after a blank line under
  `SIDEBOARD:`, `ATTRACTIONS:` or `STICKERS:`; multi-face names with one
  slash, tried as `A // B` only when the name as written does not resolve.
  "Copy Plain Text" (and "Copy for MTGO") end with the commander in a block
  of its own, which the import puts in the command zone only when the text
  has the export's shape (no command zone named anywhere, by header or
  `[Commander]`; no word headers; 90–100 cards counting the block) and only
  for cards that can be commanders (or Backgrounds), so an ordinary list
  ending in a blank line and a land or two is left alone; the sideboard
  (where Moxfield keeps the companion) goes to the maybe list like
  Archidekt's, with a note that a companion must then be set by hand;
  attraction and sticker boards are left out and reported. A paste is
  bounded at 5,000 lines of at most 300 characters (blank lines and
  comments of any length still count as such); whitespace runs collapse to
  one space and each trailing field is found from the end, so every line is
  linear work. Each distinct name resolves once per paste, and only the
  first 25 distinct misses get suggestions (the costly part of a miss);
  later ones are still listed. Imports, preview and apply, are limited to
  20 a minute per account in hosted mode.
  "Copy for Moxfield" marks no commander at all, so the preview says so
  rather than guessing (reading its order would be wrong for 2 of the 12
  sampled decks). The dialog recommends "Copy Plain Text": it uses the names
  Scryfall uses, where "Copy for MTGO" may use MTGO's own. A Moxfield list
  and the equivalent Archidekt text import to the same deck. Paste only: no
  URL fetching, no Moxfield API, no sync. Export stays Archidekt.
  *Known gap:* every Moxfield variant writes a flavor-named printing by its
  flavor name ("The Ball" for Fellwar Stone). Exact resolution knows oracle
  and face names only, so such a line is reported as unresolved for the owner
  to fix, never guessed; 3 of the 6 sampled decks had one. Resolving
  Scryfall's per-printing `flavor_name` exactly would need it ingested from
  the `default-cards` file and is left for later.

Verify Archidekt's and Moxfield's text formats against their own
documentation (or, where none exists, a real export) and cite the sources in
the code. Do not rely on model memory for them.


## 13. What is guaranteed about card facts

Exact database resolution (§4.3), write-time card-reference linting (§10.5) and
server-computed checks (§9) prevent invented stored cards and independent agent
arithmetic. Images use ingested URLs. Public reads use an allowlist (§18.3), never
the owner's full composite. This does not certify an external agent's chat,
strategy or reasoning about correct Oracle text.

## 14. Build order

Historical build order is complete. Current implementation/verification instructions
are in the [backend manual](../packages/deck-builder/AGENTS.md#focused-test-gate).
Section numbering is retained for source references; no new phase sequence is implied.

## 15. Verified at build time

Historical evaluations, migrations, deployment IDs and counts are preserved in
[the source-qualified spec](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/deck-builder-spec.md#15-verified-at-build-time).
They are not current acceptance. Current gate ownership belongs to the package
manual; exact product/client evidence has its own source qualification.

## 16. Not verified

Hosted login, OAuth, sharing and public pages are implemented. Open integration
limits include broader connector interoperability, long-running agent/memory
workflows and human usability. Old single-model evaluation is not a success-rate
measurement. Native arbitrary-deck admission, account-to-seat provisioning,
automated launch and full browser games retain the separate gates in §19.

## 17. Accounts, modes and roles

### 17.1 Modes and users

Local mode defaults to loopback/implicit UI user 1 without login; hosted mode
requires public origin, secret and a complete provider credential pair. Invalid
mode fails startup. Shared card/job data and tenant data coexist in one SQLite
file. Every personal root carries principal ownership; validated children inherit
it. Deck names are case-insensitively unique per user; profiles/settings/focus and
playtest sessions are per principal. Deleted users cannot authenticate.

User 1 is seeded owner/admin, initially unclaimed. Only an explicit timing-safe
claim code may link it while `auth_id IS NULL`, transactionally. Unset disables
claims; wrong/spent codes confer no access; success logs secret-free removal
instructions and makes the code dead. New identities otherwise need an unused
invite unless invites are explicitly disabled; existing accounts need none.
Browser-bound registration intent revalidates/consumes codes at linkage. Handles
are unique case-insensitive `[a-z0-9_-]{3,24}`, separate from display names, with
sanitization/padding and first-run collision/reserved-name picker. No password
signup, implicit provider linking or first-arrival owner claim.

Ordered migrations validate identity first, re-read schema under `BEGIN IMMEDIATE`,
check foreign keys before commit and roll back atomically. Concurrent openers
serialize. Existing user content/IDs/sequences, attributions and proposal bases
are preserved; diagnostic FK failure reports only bounded table/row/parent facts.
Auth-generated SQL participates in this same migration transaction (§17.6).

### 17.2 Actor and permissions

`src/actor.ts` owns Actor: principal `user_id`, `via: ui|agent`, attribution label
and optional verified token/OAuth-client identity. Ownership is the relation to
the deck, not an actor kind. All tenant capabilities require it explicitly.
`requireDeck`/`requireDeckOwner` and `requireSelf` enforce access below HTTP/MCP;
missing and unreadable reads/writes are 404, forbidden actions on readable targets
403. Card-only shared facts remain global; any personal context is authorized.

Owners control lifecycle, decisions, undo, import, deletion and sharing; human-only
actions additionally require UI. Agents may rename/delete their principal's editable
decks, but Paper membership needs approval (§5.2). Viewer reads are slug-only §18.3;
suggestions remain unimplemented, with no collaborator/co-owner model. Human versus
agent semantics use `via`, never labels or deck ownership alone.

Profile, memories, settings/budgets, focus, statistics, inbox/review, evidence and
tokens remain principal-scoped; `include_my_decks` is opt-in for that principal.
Lineage stops at unreadable/cross-owner boundaries; missing IDs never grant global
scope. Public cross-owner forks use the explicit exception in §18.4.

### 17.3 Attribution, tokens and live updates

Store principal/via/label beside compact legacy author strings; UI shows handle
or `handle via label`. System migration is distinguished by via, so an agent named
migration cannot impersonate it. Agent writes have owning and acting principals.
Token/client identity binds pending-work amendment rights independently of URL hints;
trusted credential-less fixtures retain label identity. Migrated local drafts bind
to the imported credential in the same transaction.

Personal tokens are `dbk_` + 32 random bytes, SHA-256 hashed, prefix-looked-up and
constant-time verified. UI-only creation reveals once; list never reveals secrets;
revocation applies on the next request. Last-use writes throttle to once/minute.
Local old-token import commits before private-file rename and retries safely by
hash; pre-v11 restores can reimport the saved file without reviving revoked rows.
The old format is a deliberate migration exception. Shim reads never mint or
rotate credentials and reject secret argv; operations owns exact file commands.

SSE only reaches the principal. Payloads name resource families/deck IDs; ordinary
changes invalidate matching reads and reconnect invalidates all. Global job changes
may notify all but contain no tenant data. No per-connection tenant state/cache.

### 17.4 Phase 1 choices and handoff

Credential ownership, unreadable-target 404s and structured attribution are current
contracts in §§17.2–17.3. Hosted session resolution and OAuth verification are
implemented in `src/app.ts`/`src/auth.ts`; this is no longer a future handoff.
Migration/runtime insertion tests must preserve tenant, credential and attribution
fields, including grouped composite foreign keys and every personal table.

### 17.5 Account lifecycle and hosted UI

Session bootstrap precedes private providers/SSE. Hosted sign-in uses configured
providers and one invite/claim field; local mode never signs in. UI maps 401 to
sign-in, 403 to readable-action errors and 429 to Retry-After feedback. Only hosted
mode serves legal pages/footer with required public operator/contact values;
never commit personal contact details. Product architecture owns screen composition.

UI-only export includes owned rows/children and Archidekt lists but no auth secrets.
Typed-handle-confirmed deletion atomically removes owned data, FTS, credentials,
sessions/providers, OAuth clients/grants/state; no Activity is written afterwards.
Preserve foreign content/spent invites with non-login user-0 tombstone attribution,
clearing identifying labels/token/client IDs. Never infer ownership from authorship
or invite use. Shared card/price/jobs remain; the last admin cannot delete itself.
Lifecycle inventory must classify every table/composite FK and fail closed for
unknowns. Admin job/invite/user-count/storage actions recheck UI/admin permission;
no impersonation. Operations owns retention, restoration and repeating erasures.

### 17.6 Phase 2 implementation choices

Pinned dependencies and generated auth SQL are recorded in the package manifest,
lock and `src/migrations/auth-v12.sql`. The reproducible in-memory schema-generation
command belongs to operations; never run auth auto-migration or generate against
production. Registration provisions identities atomically from browser-bound,
revalidated invite/claim tickets; failed registration leaves no principal.
Provider tokens are encrypted; implicit account linking is disabled.

The official native SQLite dialect reserves per query/transaction; capability
batches use the same reentrant gate. HTTP bodies buffer before database work.
Never hold the reservation across auth HTTP handlers, open MCP streams, DNS or
provider exchanges. Lifecycle inventories treat composite foreign keys as one
relation, explicitly classify global tables and reject unknown tables.

CIMD metadata identifies the connecting client, not this resource server. Public
fetches validate every DNS answer and pin one IPv4-preferred address with original
Host/SNI/TLS and no redirects; unpinned fetch is not a fallback. §17.7 owns limits.

### 17.7 Owner review security corrections

Verify signed `oauth_query` before consent rendering; reject expired/tampered input.
Show full redirect URI/host and CIMD identity host, loopback impersonation warning,
and unverified-name notice for DCR. Anonymous DCR admits only native/loopback
callbacks; web connectors use CIMD. Public metadata fetches have four slots,
five-second abortable DNS-inclusive deadline, 64KiB bodies and 30-second/128-entry
cache; late DNS never opens a connection. No auth HTTP/outbound I/O holds SQLite.

Hosted startup rejects configured claim codes under 32 characters or blank public
operator/contact. Open signup never implicitly claims user 1. Handles reserve route
words and change only in the first-run picker. Trusted-proxy rate limits use
X-Real-IP/rightmost XFF; otherwise socket only. IPv6 groups /64; separate bounded
LRUs isolate verified traffic. Code guesses use 5/minute/IP; known OAuth exchange
240/minute/client; sign-in 20/minute/IP. `src/ratelimit.ts` is numeric authority.

UI Disconnect removes consent, refresh/opaque tokens, codes and usage and records
JWT issuance cutoff; re-consent cannot revive old JWTs. Same-second new grants may
need next-second retry. Unused unconsented DCR expires after a day (hourly cleanup);
refresh rotation has five-second grace. Unused auth/session/linking/list/revoke
endpoints stay disabled, with no raw session-token browser API. Callback recovery
restarts sign-in; registration errors mention the code. Hashed assets cache for a
year; HTML/auth/API/consent are no-store. Local mode exposes no legal pages/footer.

## 18. Sharing

Current visibility, public projection and cross-owner fork contracts follow.

<a id="181-the-decision-v3-plan-d8-verbatim"></a>

### 18.1 Historical decision

The historical D8 proposal is retained in Git; current behavior below supersedes
its proposed tag copying and viewer authorization. Schema 14 added visibility,
slug, published-decision/hidden flags and fork-lineage snapshots. The unused
`accept_suggestions` column has no setter; suggestions remain unimplemented.

### 18.2 Visibility

| | Who can read `/d/<slug>` | On `/u/<handle>` | Search engines |
| --- | --- | --- | --- |
| **Private** (default) | The owner only; 404 for everyone else | No | `noindex` |
| **Unlisted** | Anyone with the link, signed in or not | No | `noindex` |
| **Public** | Anyone | Yes | Indexable |

- The slug is 10 base62 characters from `crypto.randomInt`, assigned the
  first time a deck leaves private, unique across all decks, and never
  changed. Going private again keeps it, so re-sharing restores the old
  link; nobody but the owner can read it in between.
- An unlisted slug is a capability URL, so it never reaches the logs.
  Request logs write `/d/:value`, `/u/:value` and
  `/api/public/decks/:value`. A request without a session logs principal
  `anonymous`, and no address is recorded.
- `setVisibility` and `setPublishDecisions` are owner-only and human-only
  (`requireDeckOwner` + `requireUi`): publishing is outward-facing, like the
  lifecycle choice, so no agent can do it. `hideLogEntry` (hide or show one
  decisions-log entry on the public page) has the same gate. All three work
  on Archived decks: sharing describes who may read a deck, not what is in it,
  and an archived deck is often the one worth showing. *(Choice: the plan
  says only "owner-only".)*
- The deck roster, sidebar and deck header carry a small glyph for unlisted
  and public decks.

### 18.3 Who reads what

Numeric IDs are owner-only in every capability/tool/route. Viewers (signed-in
nonowners, their agents or anonymous null) read by slug through `requireViewableDeck`;
private targets are 404. `publicDeckState` explicitly builds `PublicDeckState`, never
trims a full owner composite. Even owner slug previews show exactly that projection.

| Public field | Allowed content |
| --- | --- |
| deck | Name/slug, owner handle/display, commanders, identity/category/visibility; no ID/revision |
| parent | Live public parent's handle/name/link; unlisted parent handle/name only; absent if private/gone |
| cards | Visible list including maybes, stored facts/quantity/role/slots; empty `maybe_note` and `tag_ids` |
| slots/tags | Slots/targets; tags always empty |
| brief/engines | Thesis/constraints; engine names/descriptions/pieces/presence |
| checks | Ordinary checks, excluding `power_*` findings that could disclose agent assessments |
| power | Target and newest owner-UI assessment only, profile/evidence/confidence/notes/mapping/staleness; no attribution |
| price/computed.cost | Viewer-resolved estimate/currency/store/date/Scryfall and distinct unpriced count |
| decisions | Only when published: newest 100 nonhidden entries, generated card-change summary, reason/time/cards, generic owner/agent attribution |
| computed/tokens | Server counts/curve/pips/slot deltas as if nothing pending; token producers only from visible membership |

Never expose memories, proposals/pending edits, private notes/tags/ownership evidence,
Activity, owner pricing preferences, agent identities, proposal/edit IDs or private
numeric IDs/revisions. Decision summaries must be generated, not stored free text
that may quote agents/proposals; hidden-entry undo stays hidden. Parent lineage
snapshots do not override live privacy. Assert exact projection keys across actors
and visibility, with viewer-resolved pricing and no pending-state trace.

### 18.4 Forks across owners

`forkDeck(db, actor, source, name)` takes the owner's deck id as before, or
`{ slug }` for a deck the actor can view. A fork of one's own deck is
unchanged (§5.3). A cross-owner fork copies what §5.3 lists and records the
snapshot `parent_owner_handle`, `parent_slug` and `parent_name` at fork
time, and copies no tags. *(Choice: the plan says it records the parent's id too. That id is
the other user's, and it would reach the forker through their account
export; the slug addresses the parent instead. When the parent's owner
deletes their account, forks keep the lineage as "deleted_account/a deleted
deck".)* The fork's own page always says "forked from handle/name" from the
snapshot; its public page shows the parent only as §18.3 allows.

`compareDecks` accepts `{ slug }` as the other side. With no second deck, a
cross-owner fork compares against its parent while the caller can still view
it, and says so when it cannot. The other side of a comparison by slug
reports its name and slug, not an id or revision.

Card history (§8.3) follows parents only within the principal's own decks,
so a cross-owner parent's decisions never reach the fork.

### 18.5 Pages

`/d/<slug>` uses the shared read-only deck composition (also owner phones): list,
brief/engines, checks, target/owner assessment, estimate and optionally published
history. Anyone can copy/export; signed-in users may fork/compare through public
capabilities. Public shells have no private providers, focus reporting or decision
hotkeys. Phones retain 44px targets/no page overflow; DESIGN owns art/layout.

`/u/<handle>` exposes display name and public decks only; unknown/deleted users or
users without any public decks return 404, preventing guessed-handle name disclosure.
Server-rendered title/description/OpenGraph/Twitter preview uses only permitted
commander, identity, bracket/owner and stored credited art; unreadable slugs return
404 with plain preview. No browser Scryfall API lookup. The public initial JS budget
is 110,000 gzip bytes; images remain external, not bundled.

### 18.6 Without an account

In hosted mode, without a session: `GET /api/public/decks/:slug`, its
`/export`, `GET /api/public/users/:handle`, `/api/search` (without a deck),
`/api/resolve` and `/api/cards/:id/table` answer under the per-IP
`unauthenticated` rate bucket (D13); anonymous search also has its own
`anonymous_search` bucket, 30 a minute per IP (/64 for IPv6), half a
signed-in user's. Every search runs synchronously on the one SQLite
connection, so its size is bounded: without an account 256 characters and
30 terms, with one 600 characters and 50 terms (the worst allowed measured about 50 ms on the full card database), and at most 20 levels of
parentheses or negation for anyone. A search over a bound, or one SQLite
calls too complex, is a 400 that says so. A search naming a deck still
needs that deck's owner. Every other API stays 401. Forking and comparing need a
session.

### 18.7 Agents

- `get_deck` accepts a slug or a `/d/<slug>` link to a deck the principal
  can view and returns the public rendering, headed "Public deck owned by
  <handle>; read-only". It contains only `PublicDeckState`.
- `compare_decks` accepts a slug as `other`; `fork_deck` accepts one as
  `deck`. Every other tool treats a slug as an unknown deck and says which
  tools read shared decks.
- `start_session` reports how many of the principal's decks are public and
  unlisted. The playbook explains that shared decks are read-only, that a
  fork of one starts without memories or decisions, and that agents cannot
  change visibility.

## 19. Unified product shell and game setup

One account hosts deck building and game setup. Home, library and Play retain
existing routes and data. The captured table preview is explicitly a sample,
never the selected builder deck or a created match. Real one-human/three-automated
launch requires immutable deck snapshots, authoritative native admission,
account-to-seat authorization and supervised consumers. Ordinary Commander
legality and product sign-in establish none of those. See [application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch).

## Compatibility and product ownership

Public compatibility policy (v3 D19): never drop other users' data. From public
sign-up, MCP tools, shared URLs and token formats evolve additively or through
explicit deprecation; preserve/backfill content through migrations. Internal
implementations may be rewritten. Auth/account backend, Actor/SQLite services,
API, CLI and MCP remain in deck-builder behind public `./host` and type-only
`./contracts`; account/session UX, browser features, build/process composition
belong to Multiverse. One existing account/data system remains, with no auth/DB
migration from this move. See [application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch).

The original solo manual playtest service is legacy functionality, not a rules
engine, browser-play acceptance or approved game-client design reference. New
games follow native admission and separate account-to-seat authorization. The
additive Actor-scoped coordinator captures immutable revision-bound game decks,
retains owner-scoped launch/outcome/recovery records and requires explicit native
admission. Its concrete CLI adapter provides admission only; real one-human/three-
automated provisioning and full browser decisions remain gated by the
[product launch contract](../packages/multiverse/web/ARCHITECTURE.md#game-launch).
