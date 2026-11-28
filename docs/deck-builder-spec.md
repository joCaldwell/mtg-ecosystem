# MTG Commander Deck Builder — Build Spec (v3)

Current deck/account behavior and public compatibility; section numbers stay
stable for source comments. [Architecture](../packages/multiverse/web/ARCHITECTURE.md)
and [design](../packages/multiverse/web/DESIGN.md) own browser implementation and
visuals; [operations](../packages/deck-builder/OPERATIONS.md) owns commands.
History and build receipts are at Git `33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb`.

## 1. What this is

A Commander deck library driven by humans and external MCP agents through one
Actor-scoped capability layer. The app stores validated records, computes state
and audits writes; interpretation and conversation happen outside it. Exact card
facts, computed counts/checks, human approval of Paper membership, visible
attributed agent writes and authorization below both front doors are mandatory
([capability constraints](../packages/deck-builder/AGENTS.md#capability-constraints)).

## 2. Non-goals

No model calls/embeddings, chat/transcripts, external recommendation data, sync,
printing inventory/reservations, collaborators/co-owners or non-Commander formats.
Scryfall supplies card facts and prices; stored CDN images load on demand.
Suggestions and native full-game launch keep separate gates. The library does not
depend on Card IR or the engine.

## 3. The knowledge model

| Kind | Authority and use |
| --- | --- |
| Records | App-owned cards, decks, decisions/proposals, counts and Activity; read on demand |
| Documents | Owner's global profile/per-deck brief; agents propose edits |
| Memories | Attributed preference/reason/outcome statements; loaded by scope and searched |

Never copy a computable record or card text into memory. Documents are confirmed
owner voice; memories capture learning and mark inferred statements.

## 4. Card data and search

### 4.1 Card data

The independent SQLite/FTS5 store ingests Scryfall `oracle_cards`, one row per
`oracle_id` with exact facts, colour identity, Commander legality, a nullable Game
Changer flag, stored image/art URLs/artist and retirement (fields in
`src/cards.ts`). Never derive identity from text; Commander eligibility is a
separate check.
[Operations](../packages/deck-builder/OPERATIONS.md#card-refresh-and-scheduled-jobs)
owns schedules, locks and recovery.

Malformed, duplicate or truncated input (under 90% of active cards) leaves live
facts untouched; a failed publish is retryable and never deletes deck contents.
Absent IDs retire: references, history and images stay resolvable, search hides
them except in a named containing deck, checks flag them, and returning IDs
reactivate. Images are ingested facts, never runtime name lookups.

### 4.2 Search

A subset of Scryfall syntax (`t:`, `o:`, `cmc`/`mv`, `id:`/`id<=`, `c:`,
`pow`/`tou`, `usd`/`eur` (§4.4), `is:`) plus `slot:` and `tag:`; code cites
Scryfall's syntax documentation per operator. A deck-named search pre-filters to
its colour identity, marks deck/maybe membership and attaches linked memories
(§7.2) and prior decisions (§8.3). **Cross-deck membership is off by default**:
only `include_my_decks: true` names the owner's other containing decks, because it
biases recommendations toward owned cards.

### 4.3 Referring to cards

Agents name cards by **exact name** or `oracle_id`; resolution is never fuzzy, and
full names beat face names. An unresolved name fails the **whole call** with
suggestions; an ambiguous one fails with candidate `oracle_id`s. App-minted records
(decks, proposals, memories, decisions) use short integer IDs shown beside names.

### 4.4 Prices

Prices estimate, in whole cents, the cheapest playable paper printing per store,
not the owner's copy, from Scryfall `default_cards` only: TCGplayer USD and
Cardmarket EUR across nonfoil/foil/etched, excluding oversized/memorabilia; ties use
set, collector number, then finish ([card objects](https://scryfall.com/docs/api/cards),
[price sources](https://scryfall.com/docs/faqs/where-do-scryfall-prices-come-from-7),
[syntax](https://scryfall.com/docs/syntax#prices)). Each user picks USD or EUR and a
compatible store or Lowest (no exchange rates); missing/corrupt preferences resolve
to USD/Lowest. UI-only `setPriceSource` saves the validated pair atomically and
rejects explicit null; agents cannot set it. One actor-taking resolver serves
personal reads; public reads use the viewer's preference or USD/Lowest, never the
owner's.

Wire fields are `cents`, `currency`, `store`, `as_of`, `source: "Scryfall"`; missing
is unknown, not free. Totals count copies/companions, exclude maybes and report
distinct unpriced cards; pending projections report a signed unpriced delta.
Exports, history and memories are price-free. `usd`/`eur` search uses the chosen
store, else Lowest; unpriced cards match no comparison (including `!=`) but match
its negation. Data over 24 hours old is stale. No price history, buy links or
printing collection. Refresh atomically replaces both stores and the date per
`(oracle_id, store)`; failure keeps old quotes.

### 4.5 Tokens and emblems

Deck reads return distinct sorted Scryfall-related tokens/emblems (stored
characteristics/images) with visible producers among main membership, commanders
and companions, never maybes or pending proposals. `producer_count` counts
producing cards, not token quantities. Tokens never enter membership, counts,
checks, prices or exports; same names with different Oracle IDs stay distinct.
These are reference relationships, not rules evaluation.

Token/link tables publish atomically with prices; failure keeps prior facts and
retirement keeps known links. `tokens_status` distinguishes uninitialized from
ready-empty. Related printing IDs resolve to token Oracle IDs; `combo_piece`
identifies emblems. The only supplement is a bounded Scryfall
[collection](https://scryfall.com/docs/api/cards/collection) lookup for rare
non-English [related printings](https://scryfall.com/docs/api/cards#related-card-objects);
no card-name lists or browser Scryfall calls.

## 5. Decks

### 5.1 Model

- A deck targets exactly **100 cards** including commanders: zero to two
  commanders plus Background, and a Companion outside the 100.
- The deck stores its colour identity; a commander change that invalidates cards
  surfaces as a check (§9), never silent corruption.
- Each deck-card row has card, quantity, role (`card`/`commander`/`companion`),
  optional primary slot, secondary slots, tags and `maybe` (outside the 100).
- Every membership change bumps the deck **revision**. Lifecycle field `category`
  is `paper`, `theorycrafted` or `archived`.

### 5.2 Lifecycle and editing permissions

There is no Draft/Active flag; only the owner changes lifecycle.

| | Theorycrafted | Paper | Archived |
| --- | --- | --- | --- |
| Agent changes cards | Directly (`edit_cards`) | By accepted proposal | Read-only |
| Agent changes brief or power target | By accepted proposal | By accepted proposal | Read-only |
| Owner edits | Directly | Directly | Read-only |
| Organization, memories, assessments | Direct writes | Direct writes | Read-only |

Agent-created decks and all forks start Theorycrafted. Archived decks can be
viewed, exported, compared, forked and shared (§18.2); until the owner restores
one, nobody renames, deletes, edits or rules on its proposals, and its pending
proposals leave the inbox.

### 5.3 Forks

`fork_deck` copies list, organization, brief and deck memories into a new
Theorycrafted deck recording the parent (the `compare_decks` default); the copies
diverge, and the owner may set or clear a parent. Forks replace named checkpoints:
no snapshots or DAG.

**Forks across owners** (§18.4) of unlisted or public decks copy only the list
(maybes without notes), slots with targets/assignments, thesis, constraints and
engines; never tags ("Have" is ownership evidence), memories, ownership evidence,
power target/assessments, proposals, playtest state or log entries. The result is
the forker's private Theorycrafted deck with one log entry naming its source; card
history never crosses the owner boundary.

### 5.4 Slots, tags and the maybe list

Slots describe purpose; tags describe sourcing/status. A card has an optional
primary slot and distinct secondary slots, never one slot in both roles. Only
primary copies count toward soft targets; secondary and total coverage are separate
server values. Commanders in the 100 count; parked cards and companions do not.
Never derive card count from slots.

Tags are a per-deck vocabulary with stable IDs, case-insensitive uniqueness and
reserved-name/near-duplicate validation. Assignments are unique per deck/card/tag,
held to same-deck membership by composite foreign keys and shared by all copies in
a row; no global, hierarchical, saved-filter or per-copy tags. Slot/tag names
cannot equal a card type (singular/plural) or search prefix (`land destruction` is
valid). Archidekt categories map to slots/roles; tags do not round-trip (§12).
Filters combine name search with any/all selected tags, optionally inverted, across
all zones including maybes; totals and review scope stay whole-deck.

Organization writes are direct on Paper/Theorycrafted decks, attributed and
revision-neutral. Filing replaces tags and purposes atomically; promotion to
secondary removes the primary duplicate, and assigning both fails. Removing a slot,
parking, forks (remapped IDs) and undo preserve other assignments. `slot:` searches
either purpose; `slot:none` means no primary. `file_cards.remove_unused_tags`
aborts if a named tag is assigned.

### 5.5 Physical assembly through tags

Assembly uses ordinary per-deck tags; no dedicated field, tool or export. **Have
is an explicit ownership statement:** assigning it, or renaming an assigned tag to
Have, records owned status with unknown quantity/availability unless a more
precise statement exists. Removing Have retracts nothing. Keep, Ordered, engines,
memories and source membership prove no ownership, and source quantities are never
summed into an inventory. Evidence survives membership, tags, forks and
originating-deck deletion; corrections are new statements with a basis (§10.6).

### 5.6 Deck lifecycle

Lifecycle changes never bump revision. Owners and their agents rename and delete
only editable decks; deletion removes deck records and memories but keeps global
memories and other forks. Only Paper counts toward collection/player statistics;
ancestry traverses other lifecycles without counting them. Stored `ruling_reason`
renders as `decision_reason`.

### 5.7 Deck power: target and current assessment

An optional target and a revision-bound current assessment describe intent, usual
and occasional win turns, consistency, resilience, interaction, win conditions,
infinite combos, extra turns and mass land denial; unknown stays unknown. A fitting
assessment lies in the target's usual range; outliers are not usual turns. A target
may cap Game Changers, counted from nullable `game_changer` including
commanders/companions but not maybes.

Suggested brackets 1–5 explain inputs and conflicts; nothing is averaged or
certified. Usual turn 7/8 exceeds Core's eight-turn expectation and turn 6 or
earlier exceeds Upgraded's six; outliers alone never raise a bracket. Game
Changers, early two-card infinites, chained extra turns and mass land denial can
impose higher brackets; exhibition exceptions need pod agreement. The versioned
mapping follows [October](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025)
and [February](https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026)
guidance, independent of legality.

Assessments keep author/date/revision, `list|goldfish|games` evidence,
`tentative|supported` confidence, linted reasoning and captured Game Changer facts;
never invent games. The latest 20 return. Membership changes flag review; stale
writes reject; an agent cannot replace an owner's same-revision assessment.
Assessments write directly with Activity, never changing revision. Forks copy
targets and start unassessed.

## 6. Documents

The owner directly edits one global profile (tastes/playgroup/power/budget,
Markdown, soft cap about 600 words) and a per-deck brief (thesis, constraints and
named engines with descriptions and exact required card pieces; piece presence is
computed). Both always load at their scope. On Paper/Theorycrafted decks agents
propose replacement text with a reason and the owner reviews a diff. Confirmed
learning may move from memories into documents, retiring memories after approval.

## 7. Memories

### 7.1 Shape

A row holds one short linted statement/reason, global or deck scope, optional card
links, stated/inferred basis, principal/via/label attribution, optional source
decision/revision, timestamps and active/superseded/retired status. Supersession
links the retained old row to its replacement.

### 7.2 Scope and surfacing

Principal-global memories load at session start, deck memories with the deck, and
linked-card memories follow that principal's cards into lookups, searches and
proposal responses. Everything else uses scoped FTS `search_memories`.

### 7.3 Writing

`remember` writes without approval and returns similar reachable memories so an
agent revises before adding; irrelevant scopes cannot crowd out duplicates. Every
scope, source and mutation stays principal-owned. Agents never delete memories or
edit/supersede/retire/restore/confirm an owner-touched one (`actor.via` decides
human authority); they may write a separate disagreement. Only the owner confirms,
turning inferred into stated. Retired rows leave loading/search but stay stored.
Game observations keep the revision observed; never invent evidence or store
computable records, card text or web claims as preferences.

### 7.4 Budgets and upkeep

Configurable defaults are about 1,500 estimated global tokens and 1,000 per deck;
the newest-revised memories that fit load, reporting omissions. Agents consolidate
duplicates, supersede changes, promote cross-deck preferences and propose document
edits. The app never uses a model to consolidate.

### 7.5 Visibility

The owner can browse/filter/edit/retire/restore by scope/card/deck with basis,
author and source visible. Agent writes appear in Activity; no hidden learning store.

## 8. Proposals and decisions

Stored reasons, memories, summaries and briefs keep their original text.

### 8.1 Proposals

On Paper, an agent changes membership with `propose_changes`: a one-line summary
and items that each add or cut one copy with a reason. A cut may set `to_maybe` to
park the card with its slot and tags, shown before ruling (`edit_cards` takes the
same flag on Theorycrafted; multi-copy cards cannot park). **Grouped** items are
ruled as a unit, so a swap cannot be half-accepted. The response returns the
pending delta against 100 and each slot target, plus linked memories and prior
decisions (§8.3).

### 8.2 Decisions

The owner rules in the app per item, group or proposal, with an **optional
free-text reason**. “Accept all” is one atomic batch at the revision read at the
click, acknowledging only proposals already stale then; any conflict rolls back
the batch. Proposals use the app's ConfirmDialog, never a native confirmation.

Every decision, owner direct edit and import is a row in the **decisions log**
(deck, card, action, outcome, reason, proposal, author, timestamp, resulting
revision). The log is the version history; **undo reverses a log entry**. An import
is one entry carrying a snapshot of the prior list.

### 8.3 Prior decisions follow the card

When a card is proposed, looked up or searched with a deck named, the app attaches
its earlier decisions in that deck ("rejected at revision 14: too slow"). A fork
inherits ancestors' history labelled by deck; history never flows back. An agent
may re-propose a rejected card only by citing the decision and what changed.

### 8.4 The review queue

Each principal's queue holds rulings on proposals/edits and direct owner changes to
Paper decks (not an agent's own writes or Theorycrafted owner edits).
`start_session` reports the count, `list_decisions` returns them and
`mark_decisions_reviewed` clears them once the agent decides what deserves a memory.
`through` clears only the anchor decision's deck (or profile scope) for the
authenticated principal; explicit ID batches validate every tenant first, and a
foreign anchor or ID is 404.

## 9. Checks

Deterministic, computed in SQL on request, never stored: card count against 100;
singleton violations (respecting basics and cards that say otherwise); colour
identity; banned/not-legal cards; commander eligibility and partner pairing; slot
targets; engine pieces (§6); curve, land count and mana symbols; and Companion,
always **unverified** because it needs rules evaluation. Checks return in
`get_deck` and every write response that could change them. Judgement (win paths,
anti-synergies) arrives as proposals, memories or brief edits, not stored reports.

## 10. The agent interface

### 10.1 Transport and security

One process serves UI/API and stateless Streamable HTTP MCP through verified Actors
and the same capability layer (§17); the official SDK v2 supports 2026-07-28 with
stateless 2025 fallback. Host/Origin checks precede the SDK; proxy headers are
trusted only behind one explicit trusted ingress; local requests stay loopback.
Only hosted GET `/api/health` bypasses Host/Origin, returning `{ok,schema_version}`
without cookies, auth or tenant facts.

MCP always requires a personal bearer or verified resource-bound OAuth token; URL
labels grant no identity, UI or amendment rights (§17.3). Missing/invalid
credentials return 401 with resource metadata (`invalid_token` for a bad bearer).
Hosted OAuth supports authorization code + S256 PKCE, consent, refresh rotation,
CIMD and native/loopback-only DCR; local mode has no OAuth endpoints. JWKS checks
pin issuer, EdDSA signature, expiry, `at+jwt` and exact resource audience;
DPoP-bound tokens reject; deleted identities and disconnected grants fail before
JWT expiry.

Cookies are HttpOnly, SameSite=Lax and Secure on HTTPS (explicit loopback
development excepted); mutations require same-origin CSRF checks. Hosted requests
get CSP, nosniff, referrer, HSTS and bounded rate limits; 429 carries Retry-After.
Unreadable targets are 404, forbidden readable actions 403.

Each request logs once (aborted SSE is 499): request ID, principal/via/label,
method, redacted path or tool, status and duration. Hosted errors keep only
name/code/stack frames, never messages, payloads, queries, headers, cookies, tokens
or raw IPs; child output is redacted and successful health probes are unlogged
([MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization),
[pinned auth documentation](https://github.com/better-auth/better-auth/tree/v1.7.13/docs/content/docs/plugins)).

### 10.2 Design principles

Every call names its deck (no server-side current deck). Few task-shaped tools,
not one per database operation, keep a **deterministic order** for prompt caching
and honest annotations (read-only; `delete_deck` destructive). Output is compact
text grouped like the deck; comparison and proposal tools add `structuredContent` with stable IDs,
quantities, evidence and prices, and `get_deck` `sections` keeps re-reads under
typical tool-result caps. **Writes return the new state** (counts, slot deltas,
checks) so agents never recount, and **errors say how to fix the call**
(suggestions, or `propose_changes` for a Paper `edit_cards`).

### 10.3 Tools

`src/mcp/tools.ts` owns names, schemas and descriptions; names are a public
interface (see Compatibility).

| Group | Tools |
| --- | --- |
| Orient | `start_session` (playbook, profile, global memories, roster, focus, unreviewed count, budget), `player_stats` (Paper only) |
| Read | `get_deck` (`sections`), `compare_decks` (defaults to a fork's parent), `compare_deck_sources` (§10.6), `list_proposals`, `search_cards` (§4.2), `get_cards`, `list_decisions`, `search_memories`, `export_deck` (§12) |
| Write freely | `create_deck` (Theorycrafted; a supplied thesis becomes a proposal), `rename_deck`, `delete_deck`, `fork_deck` (§5.3), `edit_cards` (Theorycrafted only), `define_slots`, `file_cards`, `record_ownership` (§10.6), `shortlist_cards`, `remember` (§7.3), `revise_memory`, `mark_decisions_reviewed` (§8.4), `assess_deck_power` (§5.7) |
| Propose | `propose_changes` (§8.1), `propose_brief_edit`/`propose_profile_edit` (§6), `propose_power_target` (§5.7), `propose_document_changes` (§10.6), `revise_proposal` (amend/withdraw own pending work) |

**Owner-only actions** have no tool: ruling on proposals and document edits,
changing lifecycle, importing over an existing deck, undo.

**Focus.** The visible deck/playtest tab most recently interacted with
(pointer-down, key-down, window focus; never hover/scroll/visibility) owns per-user
context. Visible owners heartbeat each minute; context is stale after three minutes
and absent after thirty. Stale implicit writes are refused, so the agent rereads
and names the deck. Increasing tab-token sequences let a tab clear only its own
context ([architecture](../packages/multiverse/web/ARCHITECTURE.md#responsive-composition-and-navigation)).

**Command contract.** UI domain writes and MCP mutations share one synchronous
transaction of capability writes, attribution and result construction; one agent
command is one Activity entry. Optional `expected_revision` guards membership;
optional opaque monotonic `expected_edit_version` also guards organization, brief,
target and deck metadata.

MCP `idempotency_key` and HTTP `Idempotency-Key` are scoped to user plus verified
credential, not URL label. Identical requests replay the stored outcome for 24
hours without another write or event; different arguments or expired keys fail.
Receipts survive restarts, prune lazily to tombstones and join export/deletion. The
browser retries an interrupted domain request once with the same key, except auth
and once-shown token creation. SSE names resource families and deck IDs; reconnects
and unknown events invalidate all reads. Request-local read reuse never outlives a
write or request.

Proposal amendments create linked replacements; superseded/withdrawn work cannot be
ruled on, and naming one document-group member covers the group. Changed
membership/document/engine bases need explicit owner stale acknowledgment and
expected-revision checks; prior payloads/decisions stay immutable.

### 10.4 The playbook

`src/mcp/playbook.ts` owns runtime agent rules; the optional
[method skill](../packages/deck-builder/skill/commander-deck-building/SKILL.md) adds
method. Server instructions stay self-contained within 512 characters and tell
clients to call `start_session`, which returns the full playbook. Validation lives
below tools, never solely in prompt instructions.

### 10.5 Linting what agents write

All stored agent prose (proposal reasons, memories, brief/profile edits, shortlist
notes) writes cards as `[[Card Name]]`. Every reference resolves exactly on write;
one unresolvable name fails the call with suggestions, so displayed prose contains
no invented card.

### 10.6 Sourcing, ownership and proposal cleanup

The app computes overlaps, quantities, estimates and checks; agents supply deck
references, explicit evidence and the IDs/revisions the app returned.
`check-agent-workflows.mjs` in the
[browser fixture](../packages/deck-builder/browser-checks/README.md) exercises these flows.

- **Sources.** One `compare_deck_sources` call takes the target, every source,
  filters and sort; it excludes maybes, needs no revision guards and may read
  Archived decks. `shared_cards` and `remaining_cards` ignore filters;
  `remaining_cards` is not a shopping list, and unpriced cards are excluded from
  estimates. `apply_source_tags: true` atomically adds `From <exact deck name>` to
  **all** shared target cards and needs `expected_revision` plus
  `expected_source_revisions` for every source. Source tags are additive history:
  reruns never remove old labels, unrelated tags or slots, and renames leave them.
- **Ownership.** With better owner-supplied evidence than §5.5 tags,
  `record_ownership` on an editable deck takes a `basis` and the newest returned
  evidence ID (or null) as `expected_evidence_id`. A batch is all-or-nothing, one
  entry per card. Each statement replaces the whole claim, so omitted quantities
  become unknown; history is kept. `unowned` requires `quantity: 0`; available
  quantity cannot exceed a known owned quantity.
- **Plans.** `propose_document_changes` groups thesis, engine (exact `get_deck`
  names) and power-target changes, never card changes, ruled as one group. Change
  it by sending the **complete** replacement via `revise_proposal`
  (`action: "amend"`, any member ID) or `supersedes`; omitted unaccepted changes are
  abandoned. `revise_proposal` also withdraws. Agents revise only their own pending
  work; retired edits show `withdrawn`/`superseded`, not an owner rejection.

This is an evidence ledger, not a printing-level collection or reservation system:
moving a card never adjusts availability. Staleness counts every membership
revision and compares stored engine structure and text, not meaning. Archived
decks reject cleanup, tagging and evidence writes.

## 11. The app for people

The product hosts deck editing/review, Inbox, Memories, Activity, Profile and
Settings; SSE shows agent writes live. Responsive layout never grants a capability.

### 11.1 Routes

The typed `href()` and server page allowlist own routes and legacy hash-bookmark
conversion. Sign-in return paths are same-origin only; unknown pages and unreadable
public targets are 404. Link previews use permitted data (§18).

### 11.2 Phones

Below 720px, phone writes are limited to proposal accept/reject (individual, group
or all) with an optional reason up to 500 characters, and cross-owner public-deck
forks (the owner's own public page hides self-fork); everything else needs a larger
screen. The owner sees actual agent attribution; public readers see generic
attribution. Draft reasons persist until committed and use the desktop capability layer.

### 11.3 First run and tablet editor

Empty accounts at 720px or wider offer New deck, Import (always previewed) and Try
the sample deck. Human-only `createDemo` is transactional and idempotent per user
while its private sample exists; it uses installed facts, reports missing optional
cards, fails on a missing commander and never replaces content or writes profile,
global memories or ownership. Its writes use reserved label `sample-agent` without
Activity; a real client choosing that label becomes `sample-agent (connected)`.
First-run "connected" needs real agent writes, not tokens, reads or sample writes.

### 11.4 Visual identity

Only exact stored Scryfall art with a known artist is admitted; missing art uses a
reserved fallback. Partner crops keep individual credits, public pages use only
public commander facts, and account backdrops use shared server-resolved facts,
never tenant data or browser Scryfall requests. Statistics use the server analysis
agents receive; fixed cost symbols and mana-source copies have separate
denominators and are estimates, not available mana or casting odds.

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
[fixtures](../packages/deck-builder/test/fixtures/moxfield/README.md) pin real exports.

## 13–16. Guarantees, build order and verification

Exact resolution (§4.3), write-time linting (§10.5), server checks (§9), ingested
image URLs and the public allowlist (§18.3) prevent invented stored cards and
independent agent arithmetic. Nothing certifies an external agent's chat, strategy
or Oracle reading, broader connector interoperability, long-running agent/memory
workflows or human usability; old single-model evaluation is not a success-rate
measurement. The build is complete; current gates are in the
[backend manual](../packages/deck-builder/AGENTS.md#focused-test-gate), and
[build-time evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/deck-builder-spec.md#15-verified-at-build-time)
is not current acceptance.

## 17. Accounts, modes and roles

### 17.1 Modes and users

Local mode is loopback with implicit UI user 1 and no login; hosted mode requires
public origin, secret and a complete provider credential pair; an invalid mode
fails startup. Shared card/job data and tenant data share one SQLite file; every
personal root carries principal ownership that children inherit. Deck names are
case-insensitively unique per user; profiles, settings, focus and playtest sessions
are per principal. Deleted users cannot authenticate.

User 1 is the seeded, initially unclaimed owner/admin, linked only by an explicit
timing-safe claim code, transactionally, while `auth_id IS NULL`. Unset disables
claims, wrong/spent codes confer nothing, and success kills the code and logs
secret-free removal instructions. Other new identities need an unused invite unless
invites are disabled; registration revalidates and consumes codes at linkage.
Handles are unique case-insensitive `[a-z0-9_-]{3,24}`, separate from display
names and chosen in a first-run picker. No password signup, implicit provider
linking or first-arrival claim.

### 17.2 Actor and permissions

`src/actor.ts` owns Actor: principal `user_id`, `via: ui|agent`, attribution label
and optional verified token/OAuth-client identity. Ownership is a relation to the
deck, not an actor kind; every tenant capability takes an explicit Actor.
`requireDeck`/`requireDeckOwner`/`requireSelf` enforce access below HTTP/MCP
(404 missing/unreadable, 403 forbidden on readable).

Owners control lifecycle, decisions, undo, import, deletion and sharing; human-only
actions also require UI, and human authority uses `via`, never labels. Agent rights
follow §5.2; viewers read by slug only (§18.3). Profile, memories, settings, focus,
statistics, inbox, evidence and tokens are principal-scoped; lineage stops at
unreadable or cross-owner boundaries except public forks (§18.4).

### 17.3 Attribution, tokens and live updates

Principal/via/label are stored beside compact legacy author strings; UI shows
handle or `handle via label`. System migration is distinguished by via, so an agent
named migration cannot impersonate it. Token/client identity, not URL hints, binds
pending-work amendment rights; trusted credential-less fixtures keep label identity.

Personal tokens are `dbk_` + 32 random bytes, SHA-256 hashed, prefix-looked-up and
constant-time verified. UI-only creation reveals once; lists never reveal secrets;
revocation applies on the next request; last-use writes throttle to once a minute.
The legacy local token file is a migration exception: import commits before the
private rename, retries by hash and never revives revoked rows. The stdio shim
never mints or rotates credentials and rejects secret argv.

SSE reaches only the principal, naming resource families/deck IDs; global job
changes carry no tenant data. No per-connection tenant state or cache.

### 17.4 Migrations

Ordered migrations validate identity, re-read schema under `BEGIN IMMEDIATE`, check
foreign keys before commit and roll back atomically; concurrent openers serialize.
They preserve content, IDs, sequences, attributions and proposal bases; FK failures
report only bounded table/row/parent facts. Generated auth SQL shares the
transaction. Migration tests preserve tenant, credential and attribution fields
across every personal table and composite FK.

### 17.5 Account lifecycle and hosted UI

Session bootstrap precedes private providers/SSE. Only hosted mode serves legal
pages, with configured operator/contact values; never commit personal contact details.

UI-only export covers owned rows and Archidekt lists but no auth secrets.
Typed-handle-confirmed deletion atomically removes owned data, FTS, credentials,
sessions, providers and OAuth state, writing no later Activity. Foreign content and
spent invites stay with non-login user-0 tombstone attribution and cleared labels;
never infer ownership from authorship or invite use. The last admin cannot delete
itself. Lifecycle inventory classifies every table/composite FK and fails closed on
unknowns. Admin actions recheck UI/admin permission; no impersonation.

### 17.6 Auth implementation

`src/app.ts`/`src/auth.ts` own sessions and OAuth verification with pinned
dependencies. Auth SQL (`src/migrations/auth-v12.sql`) is regenerated offline in
memory, never against production or by auto-migration. Registration provisions
identities atomically from browser-bound, revalidated invite/claim tickets; provider
tokens are encrypted and implicit linking is disabled. The shared reentrant SQLite
gate reserves per query/transaction; HTTP bodies (1 MiB, 10 seconds) buffer first,
and no reservation spans auth handlers, MCP streams, DNS or provider exchanges.
CIMD metadata identifies the client; fetches validate every DNS answer and pin one
IPv4-preferred address with original Host/SNI/TLS and no redirects, never falling
back to unpinned fetch.

### 17.7 OAuth consent, rate limits and disconnect

Consent verifies signed `oauth_query` (rejecting expired/tampered input) and shows
the full redirect URI/host, CIMD identity host, a loopback impersonation warning and
an unverified-name notice for DCR. Anonymous DCR admits only native/loopback
callbacks; web connectors use CIMD. Metadata fetches are bounded in concurrency,
time, size and cache (`src/cimd-transport.ts`).

Hosted startup rejects claim codes under 32 characters or blank operator/contact.
Rate limits use X-Real-IP/rightmost XFF only behind the trusted proxy, otherwise
the socket; IPv6 groups by /64 and bounded LRUs isolate verified traffic. Code
guesses allow 5/minute/IP, OAuth exchange 240/minute/client and sign-in
20/minute/IP; `src/ratelimit.ts` is authoritative and limits are in memory (one
replica).

Disconnect removes consent, tokens, codes and usage and records a JWT issuance
cutoff that re-consent cannot undo (a same-second grant may need a retry).
Unconsented DCR expires after a day; refresh rotation has five-second grace. Unused
auth/session endpoints stay disabled, with no raw session-token browser API. Hashed
assets cache for a year; HTML/auth/API/consent are no-store.

## 18. Sharing

<a id="181-the-decision-v3-plan-d8-verbatim"></a>

### 18.1 Historical decision

v3 decision D8 is in Git; this section supersedes its tag copying and viewer
authorization. The `accept_suggestions` column has no setter.

### 18.2 Visibility

| | Who can read `/d/<slug>` | On `/u/<handle>` | Search engines |
| --- | --- | --- | --- |
| **Private** (default) | The owner only; 404 for everyone else | No | `noindex` |
| **Unlisted** | Anyone with the link, signed in or not | No | `noindex` |
| **Public** | Anyone | Yes | Indexable |

- The slug is 10 base62 characters from `crypto.randomInt`, assigned when a deck
  first leaves private, globally unique and never changed, so re-sharing restores
  the link.
- Unlisted slugs are capability URLs and never reach logs: request logs write
  `/d/:value`, `/u/:value` and `/api/public/decks/:value`; sessionless requests log
  principal `anonymous` with no address.
- `setVisibility`, `setPublishDecisions` and `hideLogEntry` are owner-only and
  human-only (`requireDeckOwner` + `requireUi`), so no agent can publish; they work
  on Archived decks.

### 18.3 Who reads what

Numeric IDs are owner-only in every capability/tool/route. Viewers (signed-in
nonowners, their agents or anonymous null) read by slug through
`requireViewableDeck`; private targets are 404. `publicDeckState` explicitly builds
`PublicDeckState`, never trimming a full owner composite; owner slug previews show
exactly that projection.

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

Never expose memories, proposals/pending edits, private notes/tags/ownership
evidence, Activity, owner pricing preferences, agent identities, proposal/edit IDs
or private numeric IDs/revisions. Decision summaries are generated, never stored
free text; hidden-entry undo stays hidden; parent snapshots never override live
privacy. Tests assert exact projection keys across actors and visibility.

### 18.4 Forks across owners

`forkDeck` takes an owned deck ID or `{ slug }` for a viewable deck. A cross-owner
fork copies what §5.3 lists and snapshots `parent_owner_handle`, `parent_slug` and
`parent_name`, never the other user's ID (export would leak it), so "forked from"
survives privacy changes, renames and deletion ("deleted_account/a deleted deck").
`compareDecks` accepts `{ slug }`, reporting name and slug without ID or revision;
a cross-owner fork compares against its parent while viewable and says so otherwise.

### 18.5 Pages

`/d/<slug>` uses the shared read-only composition (also owner phones): list,
brief/engines, checks, target/owner assessment, estimate and optional published
history. Anyone can copy/export; signed-in users may fork/compare. Public shells
have no private providers, focus reporting or decision hotkeys.

`/u/<handle>` shows display name and public decks; unknown/deleted users or users
without public decks are 404, so handles cannot probe names. Title/OpenGraph
previews use only permitted commander, identity, bracket/owner and credited art.
The public initial JS budget is 110,000 gzip bytes.

### 18.6 Without an account

In hosted mode without a session, `GET /api/public/decks/:slug` and `/export`,
`GET /api/public/users/:handle`, deckless `/api/search`, `/api/resolve` and
`/api/cards/:id/table` answer under the per-IP `unauthenticated` bucket (D13);
anonymous search adds `anonymous_search`, 30 a minute per IP (/64 for IPv6).
Search is bounded: 256 characters and 30 terms anonymously, 600 and 50 signed in,
at most 20 nesting levels for anyone; exceeding a bound or SQLite complexity is a
400. A deck-named search still needs that deck's owner. Every other API is 401;
forking and comparing need a session.

### 18.7 Agents

`get_deck` accepts a slug or `/d/<slug>` link to a viewable deck and returns only
`PublicDeckState`, headed "Public deck owned by <handle>; read-only".
`compare_decks` accepts a slug as `other` and `fork_deck` as `deck`; every other
tool treats a slug as an unknown deck and names the tools that read shared decks.
`start_session` reports how many of the principal's decks are public and unlisted.
The playbook says shared decks are read-only, their forks start without memories
or decisions, and agents cannot change visibility.

## 19. Unified product shell and game setup

One account hosts deck building and game setup; Commander legality and product
sign-in establish no launch authority. Multiverse owns the preview and
[launch contract](../packages/multiverse/web/ARCHITECTURE.md#game-launch).

## Compatibility and product ownership

Public compatibility policy (v3 D19): never drop other users' data. MCP tools,
shared URLs and token formats evolve additively or through explicit deprecation;
migrations preserve/backfill content; internals may be rewritten. Package ownership
follows the [application boundaries](../packages/multiverse/web/ARCHITECTURE.md#application-boundaries).
