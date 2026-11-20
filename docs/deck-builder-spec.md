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
per Oracle identity keyed by `oracle_id`: exact name/cost/value/type/text,
characteristics, colour identity, Commander legality, layout/faces, nullable Game
Changer flag, stored image/art URLs/artist and retirement. Never derive identity
from text; Commander eligibility is a separate check. `src/cards.ts` owns the field
contract; [operations](../packages/deck-builder/OPERATIONS.md#card-refresh-and-scheduled-jobs)
owns schedules, locks and recovery.

Malformed, duplicate or truncated input (fewer than 90% of active cards) leaves
live facts untouched; a failed publish is retryable and never deletes deck contents.
Absent IDs retire: references, history and images stay resolvable, search hides them
except in a named containing deck, checks flag them, and returning IDs reactivate.
Images are ingested facts, never runtime name lookups.

### 4.2 Search

A subset of Scryfall query syntax: `t:`, `o:`, `cmc`/`mv` with comparators,
`id:`/`id<=`, `c:`, `pow`/`tou`, `usd`/`eur` with comparators (§4.4), `is:`, extended
with `slot:` and `tag:`. Verify each operator against Scryfall's syntax
documentation and cite it in a comment.

When a search names a deck, results are pre-filtered to its colour identity, marked
if already in the deck or maybe list, and carry linked memories (§7.2) and prior
decisions in that deck (§8.3).

**Cross-deck membership is off by default.** A result names the owner's other decks
containing the card only when the call sets `include_my_decks: true`; left on, it
biases recommendations toward cards already owned.

### 4.3 Referring to cards

Agents refer to cards by **exact name** or `oracle_id`. Resolution is never fuzzy
(that turns a visible mistake into a silent wrong answer); full names beat face
names. An unresolved name fails the **whole call** with suggestions; an ambiguous
one fails with candidate `oracle_id`s. App-minted records (decks, proposals,
memories, decisions) use short integer IDs shown beside their names.

### 4.4 Prices

Prices estimate the cheapest playable paper printing per store, in whole cents,
not the owner's actual copy. Only Scryfall `default_cards` supplies them: TCGplayer
USD and Cardmarket EUR across nonfoil/foil/etched values. Exclude oversized/
memorabilia; ties use set, collector number, then finish order. Sources:
[Card objects](https://scryfall.com/docs/api/cards),
[price sources](https://scryfall.com/docs/faqs/where-do-scryfall-prices-come-from-7),
[syntax](https://scryfall.com/docs/syntax#prices).

Per-user preference selects USD or EUR and a compatible store or Lowest (within
that currency; no exchange rates). Missing/corrupt preferences resolve USD/lowest.
`setPriceSource` is UI-only and saves the validated pair atomically; explicit null
rejects writes. Personal consumers share one actor-taking resolver; public reads
use the viewer or explicit null (USD/lowest), never owner preferences. Agents read
prices but cannot set the source.

Wire fields are `cents`, `currency`, `store`, `as_of`, `source: "Scryfall"`. Missing
price is unknown, not free. Totals count copies/companions, exclude maybes and
report distinct unpriced cards; pending projections apply one-copy changes and
whole-card parking in order, with a signed unpriced delta. Exports, history and
memory writes stay price-free. `usd`/`eur` search uses the chosen store, otherwise
Lowest; unpriced cards match no comparison (including `!=`) but match its negation.
Data older than 24 hours is stale. No price history, buy links or printing collection.

Refresh stages bounded batches (at most 500 rows per transaction), then atomically
replaces both stores and the date per `(oracle_id, store)`; failure preserves old
quotes. A missing/old store marker forces a boot refresh and is set only by a
successful swap; failures retry hourly.

### 4.5 Tokens and emblems

Deck reads return distinct sorted Scryfall-related tokens/emblems with stored
characteristics/images and visible producers. Main membership, commanders and
companions contribute; maybes and pending proposals do not. `producer_count` counts
producing cards, not token quantities. Tokens never enter membership, counts,
checks, prices or exports; same names with different Oracle IDs stay distinct.
These are reference relationships, not rules evaluation.

Token/link tables publish atomically with prices after validation; failure keeps
prior facts and date, and retirement keeps known links. A missing catalog marker
forces refresh. `tokens_status` distinguishes uninitialized from ready-empty with a
source date. `get_deck sections:["tokens"]` and public state expose the same facts.
Related printing IDs resolve to token Oracle IDs; `combo_piece` identifies emblems,
including Flip Role/reversible faces. A bounded Scryfall
[collection](https://scryfall.com/docs/api/cards/collection) lookup for rare
non-English [related printings](https://scryfall.com/docs/api/cards#related-card-objects)
is the only refresh-time supplement. No card-name lists or browser Scryfall calls.

## 5. Decks

### 5.1 Model

- A deck targets exactly **100 cards** including commanders.
- **Commanders are a list:** zero to two, plus Background, plus Companion (outside the 100).
- Each deck stores its denormalized colour identity; a commander change that invalidates cards surfaces as a check (§9), never silent corruption.
- Each deck-card row has card, quantity, role (`card`/`commander`/`companion`), optional primary slot, secondary slots, tags and `maybe` (outside the 100). Zone and purpose are independent.
- Every membership change bumps the deck **revision**.
- Each deck has a category: `paper`, `theorycrafted` or `archived` (§5.6).

### 5.2 Lifecycle and editing permissions

A deck’s lifecycle is **Paper**, **Theorycrafted** or **Archived** (§5.6); no
Draft/Active flag. Only the owner changes lifecycle.

| | Theorycrafted | Paper | Archived |
| --- | --- | --- | --- |
| Agent changes cards | Directly (`edit_cards`) | By accepted proposal | Read-only |
| Agent changes brief or power target | By accepted proposal | By accepted proposal | Read-only |
| Owner edits | Directly | Directly | Read-only |
| Organization, memories, assessments | Direct writes | Direct writes | Read-only |

Agent-created decks and all forks start Theorycrafted. Archived decks can be viewed,
exported, compared and forked, but nobody renames, deletes, edits cards or
organization, revises deck memories or rules on proposals until the owner restores
them. Their pending proposals stay stored but leave the inbox until restoration.

### 5.3 Forks

`fork_deck` copies the card list, organization, brief and deck memories into a
new Theorycrafted deck and records the parent, so `compare_decks` defaults to it
and an agent can read where the fork came from. The copies then diverge. The owner
can also set or clear a parent on an existing deck. Forks replace named
checkpoints: no snapshot system, no DAG.

**Forks across owners** (§18.4) of unlisted or public decks copy less: the list
(maybes without private notes), slots with targets and assignments, thesis,
constraints and engines. Never tags ("Have" is ownership evidence), memories,
ownership evidence, power target/assessments, proposals, playtest state or log
entries. The fork is the forker's own private Theorycrafted deck with one log entry
naming its source; card history never crosses the owner boundary.

### 5.4 Slots, tags and the maybe list

Slots describe purpose; tags describe sourcing/status. Each card has an optional
primary slot and any number of distinct secondary slots, never the same in both
roles. Only primary copies count toward soft targets; secondary counts and total
coverage are separate server values. Commanders in the 100 count; parked cards and
companions do not. Never derive card count from slots.

Tags are a per-deck vocabulary with stable IDs, case-insensitive uniqueness and
reserved-name/near-duplicate validation. Assignments are unique per deck/card/tag
and held to same-deck membership by composite foreign keys; all copies in a row
share tags. No global, hierarchical, saved-filter or per-copy tags. Archidekt
categories map to slots/roles; tags do not round-trip (§12).

Deck filters combine name search with any/all selected tags, optionally inverted,
across all zones including maybes; proposed additions use existing assignments.
Totals and review scope remain the whole deck.

Organization writes are direct on Paper/Theorycrafted decks, attributed, and do
not bump membership revision. Filing replaces tags and purposes atomically;
promotion to secondary removes the primary duplicate, and assigning both fails.
Removing a slot preserves other purposes; parking preserves filing; forks remap IDs
and undo restores surviving assignments. `slot:` searches either purpose;
`slot:none` means no primary on an ordinary card. Names cannot equal a card type
(singular/plural) or search prefix (`land destruction` is valid).
`file_cards.remove_unused_tags` removes only named unused tags; an assigned one
aborts the batch.

### 5.5 Physical assembly through tags

Physical assembly uses ordinary per-deck tags. **Have is an explicit ownership
statement:** assigning it or renaming an assigned tag to Have records owned status
with unknown physical quantity/availability, preserving an already more precise
owned statement. Removing Have does not retract evidence. Keep, Ordered and source
membership do not prove ownership. Evidence survives membership, tags, forks and
originating-deck deletion; corrections require a new statement with its basis.
Source quantities are never added into an inventory. No dedicated assembly field,
tool or export exists. See [ownership workflows](deck-builder-mcp-workflows.md#record-ownership-without-inventing-quantities).

### 5.6 Deck lifecycle

Lifecycle changes never bump membership revision. Archived decks stay read-only
except owner-controlled sharing (§18.2). Owners and their agents rename and delete
only editable decks; deletion removes deck records and memories but keeps global
memories and other forks. Only Paper counts toward collection/player statistics;
ancestry traverses other lifecycles without counting them. The internal field is
`category`; stored `ruling_reason` renders as `decision_reason`.

### 5.7 Deck power: target and current assessment

An optional target and a revision-bound current assessment describe intent, usual
and occasional win turns, consistency, resilience, interaction, win conditions,
infinite combos, extra turns and mass land denial; unknown stays unknown. The
target's usual range contains a fitting assessment; outliers are not usual turns.
It may cap Game Changers. Counts use ingested nullable `game_changer` including
commanders/companions but not maybes; missing metadata stays unknown.

Suggested brackets 1–5 explain inputs and conflicts; nothing is averaged or
certified. Usual turn 7/8 exceeds Core's eight-turn expectation; turn 6 or earlier
exceeds Upgraded's six-turn expectation; outliers alone never raise a bracket. Game
Changers, early two-card infinites, chained extra turns and mass land denial can
impose higher brackets. Exhibition exceptions need pod agreement. The versioned
mapping follows [October](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025)
and [February](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026)
guidance, independent of legality.

Assessments keep author/date/revision, `list|goldfish|games` evidence,
`tentative|supported` confidence, linted reasoning and captured Game Changer facts;
never invent games. The latest 20 return. Membership changes flag review and stale
writes reject; organization/target edits do not. An agent cannot replace an owner's
same-revision assessment. Owners edit targets directly; agents propose them.
Assessments write directly with Activity; neither changes membership revision.
Forks copy targets and start unassessed; deletion removes power history.

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

Tools are `list_decisions` and `mark_decisions_reviewed`; stored reasons,
memories, summaries and briefs keep their original text.

### 8.1 Proposals

On a Paper deck, an agent changes membership with `propose_changes`: a one-line
summary and any number of items, each adding or cutting one copy of one card with a
reason. A cut may set `to_maybe` to park the card on the maybe list (§5.4) with its
slot and tags, shown to the owner before ruling; `edit_cards` takes the same flag on
Theorycrafted decks; multi-copy cards cannot be parked. **Grouped** items are ruled
as a unit, so one argument's swap cannot be half-accepted. The response returns the
pending delta against 100 and each slot target, plus each card's linked memories and
prior decisions in this deck (§8.3).

### 8.2 Decisions

The owner rules in the app per item, group or proposal: accept or reject with an
**optional free-text reason**. “Accept all” is one atomic batch using the revision
read at the click, with stale acknowledgements only for proposals already stale
then; all bases are checked first and any conflict rolls the batch back. Proposals
use the app's ConfirmDialog, never a native confirmation.

Every decision is a row in the **decisions log** (deck, card, action, outcome,
reason, proposal, author, timestamp, resulting revision), as are owner direct edits
and imports. The log is the version history; **undo reverses a log entry**. An
import is one entry carrying a snapshot of the prior list.

### 8.3 Prior decisions follow the card

When a card is proposed, searched with a deck named, or looked up, the app attaches
earlier decisions on it in that deck ("rejected at revision 14: too slow for this
list"), every time, without owner classification or agent recall. A fork inherits
this history from its ancestors, each entry labelled with its deck; history never
flows the other way. An agent may re-propose a rejected card only by citing the
prior decision and what changed.

### 8.4 The review queue

Each principal's queue holds decisions on proposals or proposed edits and direct
owner changes to Paper decks (not an agent's own writes or Theorycrafted owner
edits), each `reviewed` once an agent reads it. `start_session` reports the count,
`list_decisions` returns them and `mark_decisions_reviewed` clears them after the
agent decides what deserves a memory. `through` clears only the anchor decision's
deck (or profile scope) for the authenticated principal. Explicit ID batches validate
every tenant first; a foreign anchor or ID is 404.

## 9. Checks

Deterministic, computed in SQL on request, never stored: card count against 100;
singleton violations (respecting basics and cards that say otherwise); colour
identity; banned/not-legal cards; commander eligibility and partner pairing; slot
targets; engine pieces (§6); curve, land count and mana symbols; and Companion,
always **unverified** because it needs rules evaluation.

The UI shows a live problem count; agents get checks in `get_deck` and in every
write response that could change them. Judgement (win paths, anti-synergies) is
agent work that arrives as proposals, memories or brief edits, not stored reports.

## 10. The agent interface

### 10.1 Transport and security

One process serves UI/API and stateless Streamable HTTP MCP through verified Actors
and the same capability layer (§17); every tool names its deck. The official SDK v2
supports 2026-07-28 with stateless 2025 fallback. Host/Origin checks precede the SDK;
proxy headers are trusted only behind one explicit trusted ingress; local requests
stay loopback. Hosted GET `/api/health` alone bypasses Host/Origin and returns
`{ok,schema_version}` without cookies, auth or tenant facts.

MCP always requires a personal bearer or verified resource-bound OAuth token; URL
labels grant no identity, UI or amendment rights (§17.3). Missing/invalid credentials
return 401 with resource metadata (`invalid_token` for a bad bearer). Hosted OAuth
supports authorization code + S256 PKCE, consent, refresh rotation, CIMD and
native/loopback-only DCR; local mode has no OAuth endpoints. JWKS checks pin issuer,
EdDSA signature, expiry, `at+jwt` and exact resource audience; DPoP-bound tokens
reject; deleted identities and disconnected grants fail before JWT expiry.

Cookies are HttpOnly, SameSite=Lax and Secure on HTTPS (explicit loopback
development excepted). Mutations require same-origin CSRF checks. CSP, nosniff,
referrer, HSTS and bounded rate limits protect hosted requests; 429 carries
Retry-After. Unreadable targets are 404, forbidden readable actions 403.

Each request logs once on finish/close (aborted SSE is 499): generated request ID,
time/level, principal/via/label, method and redacted path or tool, status and
duration. Hosted errors keep only name/code/stack frames, never messages, payloads,
queries, headers, cookies, tokens or raw IPs; child output is redacted, the Better
Auth logger disabled and successful health probes unlogged. Sources:
[MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization),
[pinned auth documentation](https://github.com/better-auth/better-auth/tree/v1.7.13/docs/content/docs/plugins).

### 10.2 Design principles

- **Stateless.** Every call names its deck; no server-side "current deck".
- **Few task-shaped tools**, not one per database operation.
- **Compact readable text** grouped like the deck; comparison and proposal tools also return `structuredContent` with stable IDs, quantities, evidence and prices. `get_deck` `sections` keeps re-reads cheap and the full view under typical tool-result caps.
- **Writes return the new state** (counts, slot deltas, checks); agents never recount.
- **Errors say how to fix the call**, e.g. suggestions, or `propose_changes` for a Paper `edit_cards`.
- **Honest annotations** (read-only; `delete_deck` destructive) and **deterministic tool order** for prompt caching.

### 10.3 Tools

| Group | Tool | Purpose |
| --- | --- | --- |
| Orient | `start_session` | Handle/display name, playbook, profile, global memories, one-line deck roster, owner focus, unreviewed count, memory budget |
| | `player_stats` | Paper-only colours, commander types, curves, slot habits, most-reused cards |
| Read | `get_deck` | Brief, slot-grouped list with Oracle text, computed state, checks, memories, pending proposals, recent decisions; `sections` selects parts |
| | `compare_decks` | Only-left, only-right and shared cards; defaults to a fork's parent |
| | `compare_deck_sources` | Target versus sources: shared cards, ownership evidence, availability, price estimates, totals; optional guarded source tags |
| | `list_proposals` | Card/document proposals, bases, stale flags, supersession, history |
| | `search_cards` | §4.2 |
| | `get_cards` | Exact-name lookup: full text, linked memories, decisions for a named deck; validates names |
| | `list_decisions` | By deck, date or unreviewed |
| | `search_memories` | Full text by scope, deck or card |
| | `export_deck` | Archidekt-importable text (§12) |
| Write freely | `create_deck` | New Theorycrafted deck; supplied thesis becomes a proposal |
| | `rename_deck` | Rename a Paper or Theorycrafted deck |
| | `delete_deck` | Permanently delete a deck and its records when requested |
| | `fork_deck` | §5.3 |
| | `edit_cards` | Add/remove cards, set commanders; Theorycrafted only |
| | `define_slots` | Create, rename, retarget and remove slots |
| | `file_cards` | Primary `slot`, `secondary_slots` and tags for cards in the deck |
| | `record_ownership` | Explicit ownership/availability evidence; unknown quantities stay unknown |
| | `shortlist_cards` | Add to or remove from the maybe list, with a note |
| | `remember` | §7.3 |
| | `revise_memory` | Update, supersede, retire or confirm |
| | `mark_decisions_reviewed` | §8.4 |
| Propose | `propose_changes` | §8.1 |
| | `propose_brief_edit` | §6 |
| | `propose_profile_edit` | §6 |
| | `propose_power_target` | Target change on editable decks (§5.7) |
| | `propose_document_changes` | Brief/power-target changes as one atomic approval group; may supersede pending drafts |
| | `revise_proposal` | Amend or withdraw the caller's own pending proposals, keeping history |
| Write freely | `assess_deck_power` | Attributed, revision-bound assessment (§5.7) |

**Focus.** The most recently interacted visible deck/playtest tab owns the per-user
context: pointer-down, key-down and window focus claim it; hover, scrolling and
visibility never do. Visible owners heartbeat each minute; hidden tabs stop but keep
read context, stale after three minutes and absent after thirty. Stale implicit
writes are refused, so the agent must reread and name the deck. Reports carry a tab
token and increasing sequence; leaving a deck or pagehide clears only that token's
context, bfcache restores get a new token, and My decks/Inbox never clear another
tab. The sidebar shows confirmed server context, keeping its line on failure.

**Command contract (schema 17).** UI domain writes and MCP mutations share one
synchronous transaction of capability writes, attribution and result construction;
one agent command is one Activity entry. Optional `expected_revision` guards
membership; `expected_edit_version`, an opaque monotonic version, also guards
organization, brief, target and deck metadata. Both are additive and optional.

MCP `idempotency_key` and HTTP `Idempotency-Key` are scoped to user plus verified
credential, not URL label. Identical requests replay the stored outcome for 24
hours without another write or event; different arguments or expired keys fail.
Receipts survive restarts; expired payloads prune on the user's next keyed command,
leaving tombstones until account deletion, and export/deletion include them. The
browser retries an interrupted domain request once with the same key. Auth and
once-shown token creation are excluded. Structured results carry resource/deck IDs,
versions, count changes and findings beside the text. SSE names resource families
and deck IDs (global resources for cross-deck effects); reconnects and unknown
events invalidate all reads. Request-local read reuse never outlives a write or request.

Proposal amendments create linked replacements; superseded/withdrawn work cannot be
ruled on, and naming one document-group member covers the whole group. Changed
membership/document/engine bases need explicit owner stale acknowledgment and
expected-revision checks; prior payloads/decisions stay immutable. See
[MCP workflows](deck-builder-mcp-workflows.md). Price and ownership semantics are
§§4.4 and 5.5.

**Owner-only actions** have no tool: ruling on proposals and document edits,
changing lifecycle, importing over an existing deck, undo.

### 10.4 The playbook

`src/mcp/playbook.ts` owns runtime agent rules; optional SKILL.md adds method.
Server instructions stay self-contained within 512 characters and tell clients
to call `start_session`, which returns the full playbook. Review unread decisions
before proposing; use only cards read/looked up in this conversation, quote the
Oracle clause when reasoning depends on it, use computed values, cite prior
rejections and what changed, and consolidate memories when signalled. Validation
lives below tools, never solely in prompt instructions.

### 10.5 Linting what agents write

All stored agent prose (proposal reasons, memory text, brief and profile edits,
shortlist notes) writes cards as `[[Card Name]]`. On write, every reference resolves
exactly; one unresolvable name fails the call with suggestions, so displayed prose
contains no invented card.

## 11. The app for people

The product hosts deck editing/review, Inbox, Memories, Activity, Profile and
Settings under the existing account; SSE shows agent writes without reloading.
[Frontend architecture](../packages/multiverse/web/ARCHITECTURE.md) owns routes,
composition, breakpoints, loading/recovery and API effects;
[design](../packages/multiverse/web/DESIGN.md) owns controls, layout, art,
statistics presentation, review/inspection and focus. Responsive presentation
never grants a capability. The rules below are behavioral.

### 11.1 Routes

The product's typed `href()` and server page allowlist own routes and legacy
hash-bookmark conversion. Sign-in return paths are same-origin only; unknown pages
and unreadable public targets are 404. Link previews use permitted data (§18).

### 11.2 Phones

Below 720px, phone writes are limited to proposal accept/reject (individual, group
or all) with an optional reason up to 500 characters, and cross-owner public-deck
forks (the owner's own public page hides self-fork). Direct edits, playtest,
Memories, Profile, Activity, Settings and design tools require a larger screen.
The owner sees actual agent attribution; public readers see generic attribution.
Draft reasons persist until committed and use the same capability layer as desktop.

### 11.3 First run and tablet editor

Empty accounts at 720px or wider offer New deck, Import (always previewed) and Try
the sample deck. Human-only `createDemo` is transactional and idempotent per user
while its private sample exists (rename keeps it; deletion allows recreation). It
resolves installed facts, returns missing optional cards, fails on a missing
commander and never replaces content or writes profile, global memories or
ownership. Its writes use reserved label `sample-agent` without Activity; a real
client choosing that label becomes `sample-agent (connected)`. First-run "connected"
needs real agent writes, not token creation, reads or sample writes. Sidebar presence
is the last authenticated request from a live credential, not a connection count.

### 11.4 Visual identity

Only exact stored Scryfall art with a known artist is admitted; missing art uses
a reserved fallback with no invented facts. Partner crops keep individual credits,
public pages use only public commander facts, and account backdrops use shared
server-resolved facts, never tenant data or browser Scryfall requests.

### Desktop statistics and content headers

Stats use server-computed data, the same analysis agents receive. Fixed WUBRG cost
symbols and recognized mana-source copies have separate denominators;
conditional/unknown output stays explicit. These are estimates, not available
mana, casting odds or engine legality.

## 12. Archidekt and Moxfield interop

Paste-only and one-way in each direction: no sync, URL fetching or Moxfield API.

- **Export** is Archidekt text for the whole deck or current filter matches, with
  optional maybe list (`Maybeboard`) and categories; pending additions are excluded
  and exporting changes no records.
- **Import** accepts Archidekt or Moxfield text, previews the diff (destructive cuts
  first) and applies it as one decisions-log entry with one undo. `Commander`
  categories/headers set the command zone; `Maybeboard`/`Sideboard` go to the maybe
  list (a Moxfield companion must then be set by hand); attraction/sticker boards
  are reported and skipped. A trailing Moxfield commander block counts only in
  export-shaped text; otherwise the preview says none was marked. Pastes are bounded
  (5,000 lines of 300 characters); hosted imports allow 20 a minute per account.
- **Known gap:** flavor-named printings ("The Ball") are reported unresolved, never guessed.

`src/deck/interop.ts` cites the formats and
[fixtures](../packages/deck-builder/test/fixtures/moxfield/README.md) pin real
exports; verify formats against sources, never model memory.

## 13. What is guaranteed about card facts

Exact database resolution (§4.3), write-time card-reference linting (§10.5) and
server-computed checks (§9) prevent invented stored cards and independent agent
arithmetic. Images use ingested URLs. Public reads use an allowlist (§18.3), never
the owner's full composite. This does not certify an external agent's chat,
strategy or reasoning about correct Oracle text.

## 14. Build order

Complete; current gates are in the [backend manual](../packages/deck-builder/AGENTS.md#focused-test-gate).

## 15. Verified at build time

Historical evidence is [preserved in Git](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/deck-builder-spec.md#15-verified-at-build-time); it is not current acceptance.

## 16. Not verified

Open integration limits include broader connector interoperability, long-running
agent/memory workflows and human usability. Old single-model evaluation is not a
success-rate measurement. Native arbitrary-deck admission, account-to-seat
provisioning, automated launch and full browser games retain the gates in §19.

## 17. Accounts, modes and roles

### 17.1 Modes and users

Local mode is loopback with implicit UI user 1 and no login; hosted mode requires
public origin, secret and a complete provider credential pair; invalid mode fails
startup. Shared card/job data and tenant data share one SQLite file. Every personal
root carries principal ownership that children inherit. Deck names are
case-insensitively unique per user; profiles, settings, focus and playtest sessions
are per principal. Deleted users cannot authenticate.

User 1 is the seeded, initially unclaimed owner/admin. Only an explicit timing-safe
claim code links it, transactionally, while `auth_id IS NULL`; unset disables
claims, wrong/spent codes confer nothing, and success kills the code and logs
secret-free removal instructions. Other new identities need an unused invite unless
invites are disabled; registration revalidates and consumes codes at linkage.
Handles are unique case-insensitive `[a-z0-9_-]{3,24}`, separate from display names,
chosen in a first-run picker. No password signup, implicit provider linking or
first-arrival claim.

Ordered migrations validate identity, re-read schema under `BEGIN IMMEDIATE`, check
foreign keys before commit and roll back atomically; concurrent openers serialize.
They preserve content, IDs, sequences, attributions and proposal bases; FK failures
report only bounded table/row/parent facts. Generated auth SQL shares the transaction.

### 17.2 Actor and permissions

`src/actor.ts` owns Actor: principal `user_id`, `via: ui|agent`, attribution label
and optional verified token/OAuth-client identity. Ownership is a relation to the
deck, not an actor kind, and every tenant capability takes an explicit Actor.
`requireDeck`/`requireDeckOwner`/`requireSelf` enforce access below HTTP/MCP:
missing or unreadable targets are 404, forbidden actions on readable ones 403.

Owners control lifecycle, decisions, undo, import, deletion and sharing; human-only
actions also require UI. Agents may rename/delete editable decks, but Paper
membership needs approval (§5.2). Viewers read by slug only (§18.3); there are no
suggestions, collaborators or co-owners. Human authority uses `via`, never labels.
Profile, memories, settings, focus, statistics, inbox, evidence and tokens are
principal-scoped; `include_my_decks` is opt-in. Lineage stops at unreadable or
cross-owner boundaries except the public fork exception (§18.4).

### 17.3 Attribution, tokens and live updates

Store principal/via/label beside compact legacy author strings; UI shows handle or
`handle via label`. System migration is distinguished by via, so an agent named
migration cannot impersonate it. Token/client identity, not URL hints, binds
pending-work amendment rights; trusted credential-less fixtures keep label identity.

Personal tokens are `dbk_` + 32 random bytes, SHA-256 hashed, prefix-looked-up and
constant-time verified. UI-only creation reveals once; lists never reveal secrets;
revocation applies on the next request; last-use writes throttle to once a minute.
The legacy local token file is a migration exception: import commits before the
private rename, retries by hash, and never revives revoked rows. The stdio shim
never mints or rotates credentials and rejects secret argv.

SSE reaches only the principal, naming resource families/deck IDs; global job
changes carry no tenant data. No per-connection tenant state or cache.

### 17.4 Phase 1 choices and handoff

Current contracts are §§17.2–17.3; sessions and OAuth verification live in
`src/app.ts`/`src/auth.ts`. Migration tests preserve tenant, credential and
attribution fields across every personal table and composite FK.

### 17.5 Account lifecycle and hosted UI

Session bootstrap precedes private providers/SSE. UI maps 401 to sign-in, 403 to
readable-action errors and 429 to Retry-After feedback. Only hosted mode serves
legal pages with configured public operator/contact values; never commit personal
contact details.

UI-only export covers owned rows and Archidekt lists but no auth secrets.
Typed-handle-confirmed deletion atomically removes owned data, FTS, credentials,
sessions, providers and OAuth state, writing no later Activity. Foreign content and
spent invites stay with non-login user-0 tombstone attribution and cleared labels;
never infer ownership from authorship or invite use. The last admin cannot delete
itself. Lifecycle inventory classifies every table/composite FK and fails closed on
unknowns. Admin actions recheck UI/admin permission; no impersonation.

### 17.6 Phase 2 implementation choices

Auth dependencies are pinned; generated SQL is `src/migrations/auth-v12.sql`,
regenerated offline in memory, never against production or by auto-migration.
Registration provisions identities atomically from browser-bound, revalidated
invite/claim tickets. Provider tokens are encrypted; implicit linking is disabled.
The shared reentrant SQLite gate reserves per query/transaction; HTTP bodies (1 MiB,
10 seconds) buffer first, and no reservation spans auth handlers, MCP streams, DNS
or provider exchanges. CIMD metadata identifies the client; fetches validate every
DNS answer and pin one IPv4-preferred address with original Host/SNI/TLS and no
redirects, never falling back to unpinned fetch.

### 17.7 Owner review security corrections

Consent verifies signed `oauth_query` (rejecting expired/tampered input) and shows
the full redirect URI/host, CIMD identity host, a loopback impersonation warning and
an unverified-name notice for DCR. Anonymous DCR admits only native/loopback
callbacks; web connectors use CIMD. Metadata fetches have four slots, a five-second
DNS-inclusive deadline, 64 KiB bodies and a 30-second/128-entry cache.

Hosted startup rejects claim codes under 32 characters or blank operator/contact.
Rate limits use X-Real-IP/rightmost XFF only behind the trusted proxy, otherwise
the socket; IPv6 groups by /64 and bounded LRUs isolate verified traffic. Code
guesses allow 5/minute/IP, OAuth exchange 240/minute/client, sign-in 20/minute/IP;
`src/ratelimit.ts` is authoritative and limits are in memory (one replica).

Disconnect removes consent, tokens, codes and usage and records a JWT issuance
cutoff that re-consent cannot undo (a same-second grant may need a retry).
Unconsented DCR expires after a day; refresh rotation has five-second grace. Unused
auth/session endpoints stay disabled, with no raw session-token browser API. Hashed
assets cache for a year; HTML/auth/API/consent are no-store.

## 18. Sharing

<a id="181-the-decision-v3-plan-d8-verbatim"></a>

### 18.1 Historical decision

D8 is retained in Git; this section supersedes its tag copying and viewer
authorization. The `accept_suggestions` column has no setter.

### 18.2 Visibility

| | Who can read `/d/<slug>` | On `/u/<handle>` | Search engines |
| --- | --- | --- | --- |
| **Private** (default) | The owner only; 404 for everyone else | No | `noindex` |
| **Unlisted** | Anyone with the link, signed in or not | No | `noindex` |
| **Public** | Anyone | Yes | Indexable |

- The slug is 10 base62 characters from `crypto.randomInt`, assigned when a deck
  first leaves private, globally unique and never changed; going private keeps it,
  so re-sharing restores the link.
- Unlisted slugs are capability URLs and never reach logs: request logs write
  `/d/:value`, `/u/:value` and `/api/public/decks/:value`; sessionless requests log
  principal `anonymous` with no address.
- `setVisibility`, `setPublishDecisions` and `hideLogEntry` (hide/show one
  decisions-log entry publicly) are owner-only and human-only (`requireDeckOwner` +
  `requireUi`), so no agent can publish. They work on Archived decks.
- The roster, sidebar and deck header mark unlisted and public decks with a glyph.

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

`forkDeck` takes an owned deck ID or `{ slug }` for a viewable deck. A cross-owner
fork copies what §5.3 lists and snapshots `parent_owner_handle`, `parent_slug` and
`parent_name`, never the other user's ID (it would leak through export), so
"forked from handle/name" survives privacy changes, renames and deletion
("deleted_account/a deleted deck"). Its public page shows the parent only as §18.3 allows.
`compareDecks` accepts `{ slug }`, reporting name and slug without ID or revision;
a cross-owner fork compares against its parent while viewable and says so otherwise.
Card history (§8.3) follows parents only within the principal's own decks.

### 18.5 Pages

`/d/<slug>` uses the shared read-only composition (also owner phones): list,
brief/engines, checks, target/owner assessment, estimate and optional published
history. Anyone can copy/export; signed-in users may fork/compare. Public shells have
no private providers, focus reporting or decision hotkeys.

`/u/<handle>` shows display name and public decks; unknown/deleted users or users
without public decks are 404, so handles cannot probe names. Server-rendered
title/OpenGraph previews use only permitted commander, identity, bracket/owner and
stored credited art; unreadable slugs get 404 and a plain preview. The public
initial JS budget is 110,000 gzip bytes; images stay external.

### 18.6 Without an account

In hosted mode without a session, `GET /api/public/decks/:slug` and `/export`,
`GET /api/public/users/:handle`, deckless `/api/search`, `/api/resolve` and
`/api/cards/:id/table` answer under the per-IP `unauthenticated` bucket (D13);
anonymous search adds `anonymous_search`, 30 a minute per IP (/64 for IPv6). Search
is bounded: 256 characters and 30 terms anonymously, 600 and 50 signed in, at most
20 nesting levels for anyone; exceeding a bound or SQLite complexity is a 400. A
deck-named search still needs that deck's owner. Every other API is 401; forking and
comparing need a session.

### 18.7 Agents

- `get_deck` accepts a slug or `/d/<slug>` link to a viewable deck and returns only
  `PublicDeckState`, headed "Public deck owned by <handle>; read-only".
- `compare_decks` accepts a slug as `other`; `fork_deck` accepts one as `deck`.
  Every other tool treats a slug as an unknown deck and names the tools that read
  shared decks.
- `start_session` reports how many of the principal's decks are public and
  unlisted. The playbook says shared decks are read-only, their forks start without
  memories or decisions, and agents cannot change visibility.

## 19. Unified product shell and game setup

One account hosts deck building and game setup. Home, library and Play retain
existing routes and data. The captured table preview is explicitly a sample,
never the selected builder deck or a created match. Real one-human/three-automated
launch requires immutable deck snapshots, authoritative native admission,
account-to-seat authorization and supervised consumers. Ordinary Commander
legality and product sign-in establish none of those. See [application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch).

## Compatibility and product ownership

Public compatibility policy (v3 D19): never drop other users' data. MCP tools,
shared URLs and token formats evolve additively or through explicit deprecation;
migrations preserve/backfill content; internals may be rewritten. Deck-builder keeps
the auth/account backend, Actor/SQLite services, API, CLI and MCP behind public
`./host` and type-only `./contracts`; Multiverse owns account/session UX, browser
features and build/process composition, on the one existing account/data system.

The original solo playtester is legacy, not a rules engine, browser-play acceptance
or game-client reference. The additive Actor-scoped coordinator captures immutable
revision-bound game decks, keeps owner-scoped launch/outcome/recovery records and
requires explicit native admission; Multiverse owns the opt-in native bridge and its
private consumers under the
[product launch contract](../packages/multiverse/web/ARCHITECTURE.md#game-launch).
