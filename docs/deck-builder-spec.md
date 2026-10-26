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
> - Agents edit Theorycrafted cards freely; Paper cards and all brief edits require approval (§5.2). Archived is read-only.
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
to do** (its brief), **what the owner has decided and why** (decisions), and
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
  Phase 1 established identity and tenancy; login, sessions, Better Auth and
  OAuth arrived in Phase 2, hosting in Phase 3, and visibility, public
  pages and cross-owner forks in Phase 4a (§18).
- No collaborators, co-owners, shared memories, chat, paid tiers or formats
  beyond Commander. Forks and later suggestions are the collaboration unit.
- No lifecycle or decisions terminology changes in the identity phase.
- No external knowledge sources (EDHREC, Commander Spellbook, vendor sites). An agent may consult them itself; whatever it finds must resolve through the app's card search before it can be proposed. Scryfall's bulk data is the one source the app reads (§4.1, §4.4).
- No collection or inventory management; ordinary deck tags can record organization such as cards to obtain (§5.5).
- No live sync with any external service.
- No formats other than Commander.

Reopened on 2026-10-25 (planned in the
[v3 plan](deck-builder-v3-plan.md); each entry becomes a description of
built behaviour when its phase lands):

- **Phones read and rule; agents edit** (D17, built in phase 4a, §11). Below
  720 px the app is a read-and-rule view: the deck list, each deck
  read-only, the inbox, and accept/reject on pending proposals with the
  optional reason. A phone user changes a deck by prompting an agent and
  decision on its proposals. No direct editing, hotkeys, playtester or
  Settings on phones. 720 px and wider keep the full app. The playtester
  stays desktop-only.
- **Price source per user — built** (D18, phase 1b). Each user picks a currency (USD
  or EUR) and a store in it (TCGplayer, Cardmarket) or the lowest in that
  currency. The chosen price is shown, totalled, searched and given to
  agents, always labelled with its store and date. Scryfall's bulk data
  remains the only source: no other price data, no exchange rates.
- **Commander art on public pages** (D20, built in phase 4a, §18): the
  public deck page's header and its link preview, with the artist line.
  The editor stays a text list.
- **Moxfield import** (built in phase 4a, §12): paste-in text, alongside
  Archidekt. No URL fetching and no sync.
- **Sharing** (D8, built in phase 4a, §18): decks can be unlisted or public,
  read without an account, and forked by another user.

Reopened on 2026-10-11: **cards carry a price** (§4.4). It is an estimate
from the same Scryfall refresh that loads the cards. There is still no price
history and no record of which printing the owner holds.

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
| **Records** | Cards, decks, proposals, decisions, computed state, activity log | The app, from actions | Through tools, on demand |
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

- SQLite, with an FTS5 index over name and oracle text. The server schedules a refresh child process daily and on boot when stale, retrying failed refreshes one hour after their last run; the CLI runs the same operation. Settings enqueues it and returns immediately. A shared timestamped lease prevents overlapping refresh/backup jobs; leases expire after two hours. Job result, last run and next run appear in Settings. Backups retry after
one hour on failure and expose last_success_at plus a 36-hour stale flag in
admin job status, without expanding public health.
- Oracle data is streamed and staged on disk, then upserted in savepoint batches of roughly 500. Malformed input and a file with fewer than 90% of the currently active cards are refused before publishing. Missing IDs become `retired=1`, preserving exact resolution, references, history and images. General search excludes retired IDs; a named deck may still find retired cards it holds, and its checks flag “retired printing/oracle id”. Returning IDs are reactivated. Retired rows are excluded from the next truncation threshold.
- The per-instance oracle cache lives under `DATA_DIR/.scryfall-cache`, reused for 24 hours. The Settings path never forces a download; CLI `--force` remains available. All connections wait up to five seconds for SQLite locks. Refresh and nightly backup run outside the serving event loop; see `packages/deck-builder/OPERATIONS.md`.
- Keyed on `oracle_id` internally.
- Retain: `oracle_id`, `name`, `mana_cost`, `cmc`, `type_line`, `oracle_text`, `power`, `toughness`, `loyalty`, `color_identity`, `legalities.commander`, `layout`, `card_faces`, nullable `game_changer` (§5.7), `image_small`, `image_normal`, `image_art_crop` (front-face imagery for double-sided layouts), `artist` (the illustrator of the same imagery: Scryfall's print-field `artist`, or the front face's when the image comes from the face; added in phase 4a for the art-crop credit, §18.5), and `retired`. Discard everything else.
- Use Scryfall's `color_identity` directly. Do not compute it.
- `legalities.commander` supplies the banned list. Commander eligibility is a separate check.

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

Every card has at most one estimate **per store**: its cheapest playable
paper printing, in whole cents. Decks record cards, not printings (§4.1):
this estimates acquiring a copy, never the value of the owner's copy.

**Source:** only Scryfall's daily `default-cards` bulk file. `usd`,
`usd_foil`, `usd_etched` are TCGplayer USD prices; `eur`, `eur_foil`, and
`eur_etched` when present are Cardmarket EUR prices. Per store, take the
lowest across those finishes and all counted printings. A printing counts
only when paper, not oversized, and not from a `memorabilia` set. Ties use
set code, then collector number; finish order is nonfoil, foil, etched.
Sources: [Card objects](https://scryfall.com/docs/api/cards),
[Scryfall price sources](https://scryfall.com/docs/faqs/where-do-scryfall-prices-come-from-7),
and [search syntax](https://scryfall.com/docs/syntax#prices).

`card_prices` has a primary key `(oracle_id, store)`, currency, cents,
printing and finish. Migration preserves existing USD rows as TCGplayer.
Refresh streams the file into a bounded reduction, stages rows in batches
of 500, then swaps prices and timestamp atomically. Failure retains the old
prices; lost prices become unpriced. A missing or old `price_stores_version`
marker makes refresh due at scheduler boot and in next-run status, even when
the last refresh was recent. Failed jobs retain the hourly retry schedule.
That refresh loads both stores even if the bulk timestamp is unchanged.

**Personal choice:** `price_currency` (`USD` default or `EUR`) and
`price_store` (`lowest` default, `tcgplayer` for USD, `cardmarket` for EUR)
are keys in per-user `user_settings`. `getPriceSource(db, actor)` reads;
`setPriceSource(db, actor, source)` requires UI and rejects mismatched stores.
Both `getPriceSource(db, null)` and `priceResolver(db, null)` accept an explicit
anonymous viewer and resolve USD/lowest without reading any user's settings.
`setPriceSource(db, null, source)` is refused with 401 before any write.
Both are capability-layer permissions, not transport policy. Settings saves
the pair atomically (along with other settings in the same request). Any
invalid stored currency/store pair reads as USD/lowest, so old/corrupt
preferences cannot produce an undefined store. `lowest` compares only stores
in the same currency;
there are no exchange rates or cross-currency fallbacks. With one store per
currency today it resolves to TCGplayer or Cardmarket. Missing prices in the
selected store remain unpriced, even if another currency has a price.

**Every consumer** uses the same actor-taking resolver: card facts, search
results, deck rows/totals, sourcing and proposal deltas. Wire amounts use
`cents` with `currency`, `store`, `as_of` metadata, never a USD-labelled EUR
amount. Deck totals count copies and companions, exclude the maybe list,
and report the number of distinct unpriced cards. Agents read prices but
cannot change preferences. Compact card lines retain $/€ amounts; each
price-bearing response/view names the resolved store and Scryfall as-of date
once. `start_session` also names the source; exports, decision lists and
memory writes retain their price-free text.
`get_cards` includes resolved price metadata in structured results.

`usd<=2` and `eur<=2` compare the respective currency's cheapest playable
printing under the user's chosen store when it belongs to that currency,
otherwise `lowest`. Unpriced cards match no direct comparison, including `!=`.
Negating a comparison (`-usd>5` or `-eur>5`) includes unpriced cards:
it complements the predicate, as for power/toughness searches.
This preserves Scryfall's operators while applying the app's card-level
printing rules. Price is on by default for agents; budgets belong in briefs.

Public pages call `priceResolver(db, viewer | null)`: the signed-in
viewer's choice, or explicit USD/lowest for anonymous readers without reading
any principal's settings (v3 plan §4). They never use the deck owner's actor.
Phase 4a wires this into `PublicDeckState` (§18.3): `getDeck` prices for the viewer there and counts nothing pending.
Every shown total is labelled an estimate with store/date. Scryfall warns
bulk prices are dangerously stale after 24 hours: these are general
estimates, never live offers. No history, collection, buy links, foil-only
preferences, vendor API or other price source is introduced.

---

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
idea being explored), or **Archived** (a read-only archive). Lifecycle controls
editing permissions (§5.2); per-card tags remain independent (§5.4). Changing
lifecycle never changes the card list or its revision.

The UI shows one Lifecycle control in the deck header and one column in All
decks. New decks and forks default to Theorycrafted. Schema 9 preserves
existing categories and removes the Draft/Active column; it does not infer a
lifecycle from the former flag. The internal field remains `category`.
Schema 15 (v3 4b, after sharing's 14) changes only the persisted lifecycle
value `legacy` to `archived`, preserving every deck id, relation, high-water
sequence, and user-authored string. Its rebuilt CHECK accepts only Paper,
Theorycrafted and Archived values. Internal log storage keeps `ruling_reason`;
HTTP/MCP render it as `decision_reason`. D19 permits this beta rename before
public sign-up; there is no compatibility alias for the old tools.

The sidebar labels Theorycrafted decks in blue and Archived decks in amber;
Paper decks have no extra sidebar label. All decks lifecycle pills use the
same colors, with green for Paper.

Only Paper decks contribute to All decks statistics and `player_stats`.
Changing lifecycle immediately updates statistics. Fork-family ancestry is
preserved through Theorycrafted and Archived ancestors, whose cards and totals
are excluded.

All decks has a menu for rename, change lifecycle and delete. Agents can
rename or delete Paper and Theorycrafted decks through `rename_deck` and
`delete_deck`; lifecycle changes are owner-only and have no MCP tool. Archived
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

**Player profile** (one per user, global within their account): tastes, playgroup and power level, budget
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
reason that cites what prompted it (decisions, memories). The owner sees a diff
and accepts or rejects it. Agents always propose brief changes on both Paper and Theorycrafted decks; Archived is read-only.

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
| `basis` | `stated` (the owner said it, including in a decision reason) or `inferred` (the agent concluded it) |
| `author` | Compact author text, with `author_user_id`, `author_via`, `author_label` and UI `author_display` (§17.3) |
| `source` | Optional: the decision that prompted it, and the deck revision if deck-scoped |
| `status` | `active`, `superseded` (with a link to the replacement), or `retired` |
| timestamps | Created, last revised |

Rows, not files, so that the app can attach a memory to a card or deck and
surface it without relying on the agent to go looking.

### 7.2 Scope and surfacing

- **Global** memories belong to one principal and load at their session start.
- **Deck** memories load with the deck.
- **Card-linked** memories, of either scope, are attached to that card wherever it appears: search results, card lookups, and the response to a proposal that includes it. A global memory linked to a card follows the card into every deck that principal owns.
- Everything else is reached with `search_memories` (full-text, filterable by scope, deck and card).

### 7.3 Writing

- **No approval.** `remember` writes immediately for the authenticated
  principal. Global, deck, FTS, similarity and card-linked reads are tenant
  scoped; a source decision and any replacement deck must belong to that same
  principal. Another principal cannot revise, supersede, retire, restore or
  confirm the memory, regardless of label. Human/agent semantics use
  `actor.via`; `owner_touched` remains the human-control flag.
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
login. Hosted mode binds all interfaces and resolves cookie sessions through Better
Auth to `users.auth_id`. Unauthenticated UI APIs return 401. Health remains
unauthenticated, behind the same Host/Origin boundary.

MCP always requires a bearer token. Personal agent tokens are per user,
`dbk_` plus 32 random bytes in base64url, hashed at rest, and shown exactly
once when created. Settings lists labels, prefixes and last-used timestamps,
creates tokens and revokes them. It never returns stored secrets. Verification
looks up the prefix and compares hashes with `timingSafeEqual`; usage updates
at most once per minute. `DECKBUILDER_TOKEN` is removed. On first local boot,
the old `data/agent-token` file is imported, hashed for user 1 and renamed to `data/stdio-token` (0600) only
after durable insertion; existing HTTP client configurations remain valid.
The migrated credential is the deliberate exception to the new `dbk_` format.

The stdio shim proxies to the running endpoint. Because hashes cannot recover
a token, its command accepts `--token-file <private-file>` or
`DECKBUILDER_TOKEN_FILE`; local mode defaults to `data/stdio-token`. The old
no-argument stdio configuration therefore survives migration. New tokens are
saved privately by the user. Secrets in `--token` argv are rejected.
No shim read creates or rotates credentials. Missing credentials fail clearly.
Configuration snippets use a placeholder until creation, then the once-shown
value. Existing HTTP Claude Code and Codex configurations need no changes.

Host and Origin checks precede the SDK. Local requests require loopback
Host/Origin; hosted requests require the configured public origin. Unauthenticated
MCP requests return 401 (a presented invalid bearer adds `error="invalid_token"`) and `WWW-Authenticate: Bearer resource_metadata="<origin>/.well-known/oauth-protected-resource/mcp"`;
Hosted mode serves RFC 9728 resource metadata and RFC 8414 authorization
server metadata. Better Auth 1.7.13 with JWT, MCP and CIMD plugins implements
authorization code + S256 PKCE, resource-bound JWTs, refresh rotation, consent,
CIMD (MCP 2026-07-28 profile) and DCR fallback limited to native/loopback clients.
The api-key plugin is omitted: existing `agent_tokens` satisfies D7. Local
mode exposes no OAuth endpoints.

OAuth tokens are verified against this issuer’s JWKS, signature, expiry and
EdDSA algorithm (30-second JWKS cache), the `at+jwt` type and exact `<public-origin>/mcp` audience; another resource’s token is 401.
This Bearer endpoint rejects DPoP-bound tokens rather than discarding their
proof requirement. Subjects map through `users.auth_id`; the OAuth client name is the agent
label. Deleted identities and disconnected grants cannot use MCP. Both
credential kinds yield the same agent Actor and capability checks. Consent
identifies the client and the single application scope `deck-builder`;
`offline_access` permits renewal. Protocol reference:
https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization.
Pinned library reference:
https://github.com/better-auth/better-auth/tree/v1.7.13/docs/content/docs/plugins.

Hosted mode allows only `DECKBUILDER_PUBLIC_ORIGIN`. Forwarded host, protocol
and client IP are trusted only with `DECKBUILDER_TRUST_PROXY=1`, intended for
one trusted ingress and one service instance. Headers include CSP (own
scripts/styles, Scryfall images), nosniff, Referrer-Policy and HSTS on HTTPS.
Mutating browser routes retain same-origin CSRF checks. Sessions use
HttpOnly, SameSite=Lax cookies, Secure on HTTPS; explicit loopback HTTP
development uses non-Secure cookies for the documented Discord callback.
In-memory sliding-window limits apply by unauthenticated IP, UI principal
and MCP credential, with stricter auth/search/compare limits. Rejection is
429 with Retry-After. Unreadable targets stay 404; 403 is reserved for
forbidden actions on readable targets. A local URL suffix (`/mcp/claude-code`) is an attribution hint only; hosted
labels come from the stored token or registered OAuth client name:
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

Price-bearing structured results use `cents`, `currency`, `store`, `as_of`
and `source: "Scryfall"`; `get_cards` attaches these to each card. Amounts
are resolved for the calling principal, not the deck owner. Price-bearing
tools and start_session have one store/date line; card lines keep compact $/€
amounts. HTTP and local calls use the same execution/formatting helper.
Export text, decision lists and memory writes have no price-source line.
`computed.pending.projected_cost` and `cost_delta` use the same price source
and simulate pending one-copy adds/cuts and whole-card parking in item order.
The delta's `unpriced` is signed (change in distinct unpriced cards).

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
cannot be ruled on; their payloads and earlier owner decisions remain in history.
Naming one document-group member for approval, rejection, withdrawal or
supersession covers the whole group. Pending proposals record their membership
revision and document bases (including engines). Changed bases/revisions require
owner review and explicit stale acknowledgment before acceptance; expected
revisions reject conflicting writes. Checks are recomputed in the returned state.
The inbox shows each approval group once, with read-only history available.

See [MCP workflow examples and migration details](deck-builder-mcp-workflows.md)
for tool arguments, evidence semantics, response fields and limitations.

**Owner-only actions** have no tool: decision on proposals and document edits,
changing a deck’s lifecycle, importing over an existing deck,
undo.

### 10.4 The playbook

The rules an agent needs to work well with this app. It is delivered three
ways because clients differ in what they read:

1. The MCP server `instructions`: within 512 characters and self-contained, covering only cross-tool rules. *(As built: 512 is the figure Codex's documentation gives for what it can rely on; a test holds the text to it.)*
2. The `start_session` result, which carries the full playbook. The instructions tell the agent to call it first.
3. An optional **Agent Skill** in the repository, holding method rather than rules: how to evaluate a deck against its brief, how to build from a commander, how to run a memory tidy-up.

The playbook's rules:

- Call `start_session` first. Review unreviewed decisions before proposing anything.
- Mention a card only if it is in a deck you have read or came back from a search or lookup in this conversation. A hunch is a search query, not a fact.
- When an argument depends on what a card does, quote the oracle clause.
- Never count or compute what the app reports.
- Re-proposing a rejected card requires citing the decision and what changed. One push-back, then defer.
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

- **Decks:** list, create, fork, delete; Paper, Theorycrafted or Archived lifecycle.
- **Deck view:** the list grouped by slot or by card type, slot targets and deltas, the maybe list, the brief, a live checks count, tags, direct editing, card search.
- **Proposals inbox:** everything waiting on the owner across decks. A proposal is ruled on in its deck's list, where each change is drawn in place among the cards around it — per item, per group or all at once, with an optional reason; the inbox links there. Proposed brief and profile edits are ruled in the inbox as diffs. (Settled 2026-10-10: the owner preferred reviewing in the list to decision from the inbox.)
- **Decisions log:** per deck, with undo.
- **Memories:** §7.5.
- **Profile:** edit directly.
- **Activity feed:** every agent write, with the agent's name, time and target. The audit trail for unapproved writes.
- **Settings:** connection details for agents, memory budgets, card data refresh,
  and a currency then store/Lowest price-source control (§4.4). Saving is
  UI-only and invalidates that principal's views, including hover lookups.
  The deck estimate and card panel name the resolved store and Scryfall date.
- **Sharing:** the deck header's Share dialog sets visibility, publishes the decisions log, copies the link and opens what viewers see. Public deck pages (`/d/<slug>`) and profiles (`/u/<handle>`) are described in §18.

The UI updates live when an agent writes (server-sent events from the app to
the browser). A proposal made in a terminal appears in the inbox without a
reload.

### 11.1 Routes *(phase 4a)*

The client uses path routes, so a deck has a real URL and a public page can
carry server-rendered link-preview tags: `/` and `/decks`, `/deck/:id`,
`/deck/:id/review`, `/deck/:id/playtest`, `/inbox`, `/memories`, `/profile`,
`/activity`, `/settings`, `/design`, `/signin`, `/privacy`, `/terms`,
`/d/:slug` and `/u/:handle`. `href()` in `web/src/app/routes.ts` is the only
place a URL is built; same-origin links navigate without a reload. A
bookmark from the hash-route era (`/#/deck/5`) is rewritten once to its
path on load, and a malformed escape in a path is a missing page. Going
back restores the scroll position of the phone and shared pages once their
data has rendered. Signing in from a shared page returns to it
(`/signin?next=/d/<slug>`); `next` must be a same-origin path, so a
crafted link cannot redirect elsewhere. The server serves `index.html` for those app routes only;
`/api/*`, `/mcp*`, `/.well-known/*`, `/assets/*` and any path with an
unknown file extension return 404 (D16).

### 11.2 Phones *(phase 4a, D17)*

Below 720 px (`PHONE_MAX_WIDTH` in `web/src/app/breakpoints.ts`, the one
constant shared with the public page) the signed-in app is a phone shell
with a top bar instead of the sidebar, and only these screens:

- **Decks:** the deck list, one tappable row per deck with its pending count.
- **A deck:** the same read-only composition as the public page
  (`ReadOnlyDeck`), fed by the owner's own deck state, with its pending
  proposals drawn in place in the list as on desktop. Each decision (an add,
  a cut, a swap or a group) has Accept and Reject; above them Accept all and
  Reject all cover every open proposal. Each takes the optional reason (at
  most 500 characters; blank is none), which stays in its field until the
  decision has landed, and every decision goes through the existing capabilities
  and routes. The owner's own decisions name the agent that acted
  (DESIGN.md principle 5); a viewer's say only "an agent". The reason is recorded on the log entry whichever way the owner
  ruled; on desktop an accept still takes none, so nothing changes there.
- **Inbox:** the cross-deck list; card proposals open their deck in review,
  and brief and profile edits rule in place as diffs.

Accept and reject are the only writes on a phone, with one clarification of
D17: forking another user's shared deck from its public page. A fork edits
nothing; it makes a new private deck in the forker's account, which fits
D17's workflow of forking and then prompting an agent. The owner's own
public page hides "Fork your deck" on a phone. *(Recorded 2026-10-25 in the
phase 4a review; the owner can veto it.)* An address that is no page says
so on a phone as on desktop; only real desktop screens say "Open on a
larger screen". Direct editing, hotkeys,
the playtester, memories, profile, activity, Settings and the design page
each render "Open on a larger screen" with what the screen is for. Live
updates work as on desktop: an agent's new proposal appears without a
reload. Changing a deck from a phone means prompting an agent (Claude or
ChatGPT through the hosted connectors) and decision on what it proposes; a
loopback app cannot be reached from a phone.

At 720–1099 px the full editor uses the tablet shell (§11.3). At 1100 px
and wider it keeps the desktop panes. Phones retain this read-and-rule scope.

### 11.3 First run and tablet editor *(phase 4b)*

- An empty account at 720+ px has three actions: Create a deck; Import an
  Archidekt or Moxfield text export; Try the sample deck. Import first names
  its destination and then opens the existing import preview (`?import=1`);
  no membership changes happen without that preview's approval. The query is
  consumed when the Import tab opens, so reloading does not reopen it.
- `POST /api/demo` calls human-only `createDemo(db, actor)` in the capability
  layer, within a transaction. It creates one private sample for that user,
  including illustrative proposals, decisions and deck-scoped memories. The
  synthetic actor is explicitly labelled `sample-agent` with the same
  principal; every illustrative agent write (proposals, deck memories, the
  brief edit) carries that label, and none is written to activity. It is
  demonstration data, not a claim that an external model connected. The
  label is reserved: a connected client that claims it is attributed as
  `sample-agent (connected)`. Nothing touches the profile, global memories or physical ownership evidence.
  Sample tags are sourcing/status examples (Testing, Wishlist, Keep), with
  purposes recorded as slots. A repeat call
  returns the recorded sample id while that user's deck exists, including
  after a rename; deleting it allows another. The CLI calls this capability
  too and never replaces user content. Missing optional cards are skipped
  and returned (`missing` beside `state`); a missing commander fails the whole transaction with a
  card-data recovery message. Hosted, `/api/demo` has its own per-user
  rate-limit bucket. An intentionally unfinished list demonstrates
  checks and slot targets rather than pretending it is ready to play.
- `/api/settings.has_agent_activity` means an agent has written activity,
  a memory, proposal, document edit or log entry for this principal. Token
  creation, read-only agent calls and the sample's `sample-agent` writes do
  not enable it, before or after the sample deck is deleted. Each table is
  probed through an index. The sidebar presence
  block is absent until then. Agent-sensitive empty states, subtitles and
  help have a deck-first and an agent-connected voice. Neutral hints and
  errors are identical in both. A first-run pointer leads to Settings →
  Connect an agent, without making connection a prerequisite.
- Lifecycle, slots vs tags, brief, bracket, proposals and memories have an
  inline one-sentence explanation and a Learn more popover. Help does not
  mutate data or infer card facts.
- **Advanced** is one shared boolean per account in `user_settings`, not
  localStorage. Migration 15 sets it open for existing users, retaining their
  visible controls; accounts registered later start closed. New local files
  also start closed. It gates secondary purposes, memory budgets, review
  queue flags, revision chips and ownership evidence when rendered. It never
  hides authorship, decision reasons, checks or stale-approval warnings.
- `PHONE_MAX_WIDTH=719` retains the phone boundary. Named constants
  `DRAWER_BREAKPOINT=960` and `PANEL_BREAKPOINT=1100` select the tablet
  drawer and deck-details sheet. Under 960 the normal sidebar lives in a
  modal drawer. Under 1100 the list scrolls the window and the owner opens
  the panel as a bottom sheet. That explicit request is the reason a sheet
  may cover the list without changing DESIGN.md principle 1. The tablet
  shell calls `useWindowScroll()` like the phone and shared shells. At
  1100+ the body stays pinned and panes scroll. Playtest needs 1100+ px.
- Tables scroll sideways inside their containers. MultiSelect is a
  searchable multi-select listbox; card rows support `?` to inspect and a
  tapped mana cost opens CardSheet. Archived titles are plain headings. A
  skip link targets a labelled main landmark. Faint text meets AA contrast
  on the app's neutral surfaces, and reduced-motion disables animation.
- Playtest, settings, /design and public pages load lazily; public fork,
  compare, export and card-inspection code loads on demand. System fonts
  replace the unused Inter entry. `GET /api/activity/latest` returns at most
  one row for this principal, never a `sample-agent` row. Activity and
  Memories paginate at 100 by keyset, so writes between loads never shift
  rows: `/api/activity?before_id=` (newest first, with `actor=<label>`
  filtering before the page) returns `entries`, `has_more` and every agent
  label of the principal as `actors`; `/api/memories` continues with the
  previous page's last `before_id` and `before_revised_at`, a search
  filtering the same newest-revision order.
  Memories gets its filtered page, scope choices and budgets together.
  Scope choices include empty owned decks, so the owner can add that deck's
  first memory without another screen. UI components never call fetch.

These choices preserve the five AGENTS.md rules and introduce no suggestions
from other users. Public pricing, projection, attribution and phone editing
limits are unchanged from §18 and §11.2.

## 12. Archidekt and Moxfield interop

Construction and desktop solo goldfishing happen here. The manual playtester
was added on 2026-10-13; see [its requirements](playtester-design-brief.md)
and [implementation guide](playtester.md). Archidekt interop remains available.

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

---

## 13. What is guaranteed about card facts

| Guarantee | How |
| --- | --- |
| No invented card can enter a deck | Every card argument resolves exactly or the call fails (§4.3) |
| No invented card appears anywhere in the app | Agent prose is linted on write (§10.5) |
| An agent reads card text; it does not recall it | Deck views, searches and lookups all return oracle text |
| Counts and legality are never an agent's arithmetic | Computed by the app and returned with every read and write (§9) |
| Images use ingested facts | Scryfall bulk `image_uris` (card or front face), returned by table lookup/search/`get_cards`; all browser images use these stored CDN URLs and load lazily. Missing imagery has a placeholder; no `cards/named` URL is constructed. |

**On public pages** *(phase 4a, §18)* the same guarantees hold, plus:

| Guarantee | How |
| --- | --- |
| A viewer sees exactly the published shape | `publicDeckState` builds `PublicDeckState` through an allow-list; the permission matrix asserts its keys for every visibility and actor |
| Nothing private reaches a viewer | Memories, proposals, pending edits, ownership evidence, activity, maybe-list notes and agent names are never in it; decision entries say "owner" or "an agent", never which |
| Private decks are not discoverable | A private slug is 404 for everyone but the owner, as HTML and as API; numeric deck ids stay the owner's namespace |
| Art is credited | The header's commander art crop is the stored Scryfall URL (D10) with the card's stored artist; the art is not shown without its artist line |
| Prices are labelled | The total names Scryfall, the store, the currency and the as-of date |

**Not guaranteed:** what an agent says in its own chat window. The app cannot
lint text it never sees. The playbook's citation rule and the `get_cards`
lookup reduce this; they do not remove it. The same goes for an agent
reasoning wrongly from correct card text about timing, layers or replacement
effects.

---

## 14. Build order

Each phase is verified before the next begins, and each is testable without
the one above it.

1. **Capability core.** Card database and search; decks with lifecycle permissions, forks, slots, tags, maybe list; checks; the decisions log with undo. Tested with no UI and no agent.
2. **Agent interface, read side.** MCP endpoint with token and Origin check, stdio shim, `start_session`, the read tools, the playbook. Verify by connecting a real agent and reading decks.
3. **The gate.** `edit_cards`, organization tools, `propose_changes`, prior decisions following the card, linting on write; the proposals inbox and live UI updates.
4. **Memories and documents.** The memory store, budgets, the review queue, the profile, proposed document edits, the Memories page and activity feed.
5. **Cross-deck.** `player_stats`, `compare_decks`, focus.
6. **Method and evaluation.** The Agent Skill, and a set of realistic multi-step tasks ("rework this fork toward a new thesis", "what should I build next") run against a real agent, with transcripts read and the tools revised from what they show.
7. **Interop.** Archidekt export and import.

## 15. Verified at build time

Checked on 2026-10-10 while building; recorded here so the next change starts
from what is known.

- **Protocol revisions.** The official SDK's v2 packages (2.3.1) implement 2026-07-28 and fall back statelessly for 2025-revision clients; both paths are covered by tests that use the official client. Claude Code 2.1.296 connected speaking 2026-07-28; Codex CLI 0.162.1 connected speaking the 2025 revision. Both work.
- **Codex.** A real Codex run completed "find what most needs fixing in this deck and act on it" with no failed tool call, following the same loop as Claude Code (session, deck, unread decisions, memory, proposal). It connects with a static `Authorization` header in `~/.codex/config.toml`, which its CLI, desktop app and IDE extension share. Its `default_tools_approval_mode` honours the tools' read-only annotations: under `writes` a read ran and `remember` was held for approval; under `approve` nothing is asked. Without a label in the URL its writes are attributed to `codex-mcp-client`.
- **Server `instructions`.** Delivered in both eras (asserted in tests). Claude Code called `start_session` first in every real run, unprompted. No other client was checked.
- **Permission prompts.** Claude Code refuses each tool call until the server is allowed: a run without an allowlist was denied `start_session`. Allowing `mcp__deck-builder` (the `--allowedTools` flag, or `permissions.allow` in its settings) makes every tool, including the unapproved writes, run without asking. The Settings page shows the snippet.
- **Real-agent evaluation.** Seven tasks against a copy of real decks, Claude Code with a Sonnet model, given nothing but the tools (one run also had the Agent Skill installed): find and fix what most needs fixing in a deck; say what the owner builds and draft the next deck; rework a deck as a fork; list what is missing in paper; evaluate a deck against its brief; catch up on decisions and suggest cards; tidy cluttered memories. All seven completed. Three tool calls failed across them, each recovered in one step: an unsupported search key, and the two below.
- **What the transcripts changed.** (1) Forks now inherit their parents' decisions (§8.3). (2) `fork_deck` says to rewrite the thesis first; the agent that was not told left the parent's brief on a deck with a new aim. (3) The paper list carries type lines; an agent sorting bare names by kind got two wrong. (4) Every tool calls a body of prose `text`; an agent wrote `content` to `remember` after seeing it on another tool. (5) `list_decisions` ends with the exact `mark_decisions_reviewed` call; an agent first tried it with no arguments.
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
  ConfirmDialog and successful decision, and styled proposal history. An
  automated hover rendered the stored CDN URL with lazy loading; browser
  resource entries showed zero `api.scryfall.com` requests. T3 snapshots
  timed out; Chrome supplied visual inspection and T3 evaluation supplied
  the hover/network evidence.
- The official MCP client transcript was read through `start_session`,
  `get_cards` (including structured image URLs), deck-scoped `list_decisions`
  and `mark_decisions_reviewed`: other decks' unread decisions remained unread.
- `npm run check` passes 449 tests on Node 26.12.1, and the same suite
  passes on the declared Node 24.12.0 floor.
- Phase 0 keeps schema version 10's existing additive-column migration style;
  image/retirement columns and missing proposal baselines upgrade on open.
  Phase 1 owns the ordered migration framework and schema 11 tenant rewrite.

Checked on 2026-10-25, v3 Phase 1 identity foundation:

- `npm run check` passes 910 tests on Node 26.12.1 after PR review. The permission matrix
  globs `src/**/*.ts` with explicit exclusions, classifies every discovered
  runtime function and all 30 MCP tools, requires real user-2 global probes,
  and covers all 58 registered HTTP routes, tenant feeds and FTS, creator
  credentials, archived reads/writes, hosted API 401s and per-user SSE.
  Mixed-ID probes snapshot every personal table, FTS shadow and sequence.
  Weakening `requireSlot` to ignore `deck_id` failed eight probes; adding an
  unclassified export in a new module failed discovery. Both temporary
  mutations were removed. The frozen actual main-v10 schema, preservation,
  concurrent openers and complete rollback are tested explicitly.
- An online read-only snapshot of the owner’s v10 database migrated to v11:
  **18 decks → 18; 36 memories → 36**, with roster IDs/names/revisions,
  memory text/scopes and profile identical. The copied legacy token file
  imported and was renamed to `stdio-token` (0600) only in the copy. Its secret authenticated MCP;
  both existing Claude Code and Codex HTTP configurations were checked to
  use that same credential, without displaying their secrets.
- The official MCP client, launching the stdio shim from `/tmp`, called
  `start_session` and `get_deck` against both the Phase 0 baseline and the
  migrated copy. The new shim used its default credential path, with no
  token arguments. `get_deck` was byte-identical; `start_session` was
  byte-identical after removing the new `# Identity` block. This is a
  scripted transport/text check, not a new model-quality evaluation.
- Seeded browser QA on the isolated copy exercised token creation, the
  once-shown value, dismissal, placeholder snippets, reload and confirmed
  revocation. Browser storage contained no credentials; desktop and narrow
  Settings layouts were inspected. The owner’s live database, token file
  and client configurations were not changed. Vite production build passes.

Checked on 2026-10-25, v3 Phase 3 private beta deployment:

- The repo-root Docker build uses `node:26-slim`, a selected npm workspace,
  Vite assets and production-only runtime dependencies. Railway's root-owned
  volume is prepared by a startup helper; the serving Node PID and its jobs
  run as UID/GID 1000. Production follows `release` initialized at Phase 2's
  merge `3e1798d`; staging follows `main`. One replica per environment.
- A read-only online backup of the owner's schema-10 database migrated to
  schema 12 on production: **18 decks and 36 memories preserved**, zero FK
  errors, no legacy token file transferred. Production becomes primary at
  go-live; local data is an unchanged development copy.
- A forced production refresh completed in **15.14 s**, child peak
  **264.10 MiB**, with **148/148** concurrent health probes passing and no
  server restart. A **73,711,616-byte** online backup reached the Railway
  bucket and was downloaded and restored into staging in **4.961 s** for
  the restore/open step: the same 18 decks, 36 memories and schema 12.
  Both exact deployments reached `SUCCESS` and passed Railway healthchecks.
- A real internal probe with Railway's healthcheck Host and a wrong Origin
  returned only `{ok:true,schema_version:12}`, with no cookies. A temporary
  live staging ingress probe confirmed X-Real-IP matched the caller's public
  IPv4, and spoofed client forwarding headers were overwritten. XFF's last
  hop was a proxy. Real IPv6 ingress remains unverified.
- Live Discord sign-in issued a Secure, HttpOnly, SameSite=Lax state cookie
  behind Railway TLS. Request/tool JSON logs correlate generated request IDs
  with resolved Actors and omit credentials, bodies, queries and tool data;
  error logs contain stack frames without potentially sensitive messages.
  All **961 tests** and both typechecks pass. OPERATIONS.md records commands,
  deployment IDs and the claim lifecycle. The real owner then signed in and
  confirmed all decks; user 1 is claimed as `jcaldwell`, with all 18 decks and
  36 memories. The claim variable was removed and deployment
  `b7c4ec34-dc77-47e5-acc4-ab8bbc34d235` reached SUCCESS without it. A temporary
  personal token called `start_session` through the official MCP client from
  the owner machine; authenticated SSE disconnected/reconnected across the
  deployment and retained user 1. Temporary test credentials were revoked/removed.
- The $25 workspace soft spending alert is configured. Railway rejected
  native daily/weekly volume backups as Pro-only on this Hobby workspace;
  the runbook explicitly records that unsatisfied redundancy requirement
  and the owner upgrade/enablement steps. Nightly SQLite bucket backups are
  configured and real bucket recovery was tested. External uptime monitor
  setup remains an owner step because no monitor credentials were supplied.

Round-1 Phase 3 review (2026-10-26 UTC), rebased on phase 1b **fc24ca8**:

- Process locks record hostname, reclaim a previous container's reused PID 1
  and still exclude live same-host operations. The real PID-namespace kill/
  restart regression passes. Railway has a 30-second drain and a root tsconfig
  watch path, verified in staging before the identical config-only production
  patch (`skipDeploys:true`); production code is unchanged until owner promotion.
- Build context is verified using Moby's actual matcher; explicit COPYs exclude
  data/credentials. The final image copies nested workspace dependencies and
  resolves every runtime import/exact pin, including jose 6.2.3; Node is pinned
  by digest. Hosted job errors preserve redacted name/code/frames, backup failures
  retry hourly, and admin status exposes backup age without public-health data.
- Snapshots have unique UTC timestamp/UUID keys. Successful uploads retain 14
  daily and six monthly recovery points plus the latest upload; upload/list
  failures never prune. Archive keys are excluded, and no live bucket object
  was deleted in this review. Privacy describes that policy and operational logs.
- Staging successfully migrated v12→v13 and automatically loaded both stores:
  33,327 TCGplayer/USD and 33,092 Cardmarket/EUR quotes; user 1's 18 decks,
  36 memories, row fingerprints and per-deck USD totals stayed unchanged.
  Both claim variables are removed. The read-only credential audit and exact
  historical internal-adapter/HMAC smoke method are documented; future smoke
  tests require owner-minted Settings tokens, never operator-made sessions.
- The rebased gate passes **1003 tests** and both typechecks. Promotion must
  use the verified #7 merge SHA and a fresh immutable post-claim v12 snapshot
  immediately beforehand, because production still runs v12 and this promotion
  includes v13. OPERATIONS documents old-variable rollback and schema refusal.

## 16. Not verified

Ownership-aware sourcing and proposal cleanup were added on 2026-10-15.
`npm run check` passes 422 tests, including migration, evidence persistence,
multi-source tagging, grouped approval, supersession, stale revisions and both
MCP protocol eras. A scripted official-client transcript against an isolated
database copy was read through sourcing, amendment and owner approval. At that time visual
browser QA could not complete because preview automation failed. Phase 0
subsequently verified the current approval/history controls on an isolated
copy (see §15); the live owner database was not upgraded during that drill.

The original local capabilities in §1–§14 and the Phase 1 identity foundation
are built. Hosted login, sessions, OAuth, visibility and public pages are
future v3 phases, not verified features. What has not been checked:

- Any client other than Claude Code and Codex.
- Longer real agent budgeting workflows remain unverified. Phase 1b read a
  live-data tool transcript for a second EUR user (start_session, get_deck,
  eur<=1 search) and verified USD isolation for user 1; it does not establish
  how agents use those prices over a full deck-building conversation.
- Claude.ai/ChatGPT web connector interoperability with the live hosted service. Hosted HTTPS OAuth/CIMD and consent are configured; completing these client connections remains owner testing. Remote clients cannot reach the local loopback instance.
- The evaluation was run once per task, on one model, with tasks written by the same hand as the tools. It shows the interface works; it does not measure how often.


## 17. Accounts, modes and roles

### 17.1 Modes and users

`DECKBUILDER_MODE=local` is the default. It binds `127.0.0.1`, has no login
page, and uses implicit user 1 for the UI while MCP requires a token. This is
the owner's daily product. `DECKBUILDER_MODE=hosted` binds all interfaces and
requires `DECKBUILDER_PUBLIC_ORIGIN`, `BETTER_AUTH_SECRET`, and at least one
configured provider. Discord activates when both credentials are set; Google
activates only when both of its credentials are set. Invalid modes
fail startup instead of falling back to local.

One SQLite file holds shared card facts and tenant data. `users` has
`id INTEGER PRIMARY KEY AUTOINCREMENT`, unique case-insensitive `handle`
(`[a-z0-9_-]{3,24}`), `display_name`, unique nullable `auth_id`, `is_admin`
(default 0), `created_at` and nullable `deleted_at`. Migration creates user 1
(`owner`, `Owner`, admin, no auth id). Claiming user 1 requires `DECKBUILDER_CLAIM_CODE`, entered on sign-in in the
same field as an invite. Compare code hashes timing-safely and link only
while user 1 has `auth_id IS NULL`, in one transaction. A successful claim
makes the code dead even if still configured; log one secret-free line
telling the owner to remove the variable. Unset disables claims. Wrong or
already-used claim codes confer no access; new identities must independently
present a valid unused invite. This owner amendment replaces D5’s unsafe
first-sign-in rule.

New identities need an unused invite unless claiming (or the operator sets
`DECKBUILDER_INVITES_REQUIRED=0`). Existing identities need no code. A short
lived, browser-bound registration intent carries the validated code through
the redirect; revalidate and consume it atomically at identity linkage.
Handles derive from provider identity, lowercase/sanitised to
`[a-z0-9_-]{3,24}`, padded/truncated as needed. Collisions get a unique
suffix and require a first-run handle picker. Display names stay separate.
No email/password sign-up or implicit provider-account linking is exposed.

Better Auth 1.7.13 uses the existing built-in node:sqlite connection, with
no native dependency. Its CLI-generated SQL is committed as the ordered
v12 migration, in our single migration transaction; never auto-migrate at
runtime.

Schema 11 uses ordered `{ version, up(db) }` migration steps, all within one
`BEGIN IMMEDIATE` transaction after database identity validation. The schema
version is re-read under that write reservation, so concurrent openers
serialize migration rather than racing with an old version. Failed FK checks
report the first five `{ table, rowid, parent }` rows before rolling back.
It preserves old migration steps, Phase 0 image/retirement facts and proposal baselines. All existing
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
  oauth_client_id?: string;   // authenticated OAuth client, when present
};
```

Ownership is a relation: `actor.user_id === decks.user_id`.
`requireDeckOwner(db,actor,deckId,what)`, `requireSelf(actor,userId)` and
`isDeckOwner` enforce it below both front doors. `requireDeck` returns the
deck and caller role; an unreadable deck is 404. Writes on an unreadable deck
also return 404, with the same missing-target behavior as reads.
`PermissionError` (403) is reserved for readable targets whose action is
forbidden (human-only, admin-only, or Archived writes). Every exported tenant
capability requires an actor. Shared card-only utilities remain global, but any deck or memory
context is authorized. No implicit actor is supplied by a service.

V3 roles are owner, viewer (visibility admits them), and suggester (signed
in on a deck accepting suggestions). No collaborators, co-owners or members
table. Viewers arrived in phase 4a (§18.3) and read only by slug;
suggestions arrive in phase 5.
Lifecycle, decisions, undo, import, deletion and visibility belong to the deck
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
and `owner (from v1)` authors backfill to user 1/ui/ui,
preserving their compact author string. System-authored `migration` rows use
user 1/ui/`migration` and display **System migration**, avoiding an invented
agent or a claim that the human performed the import. All remaining labels
backfill to user 1/agent/original label. An actual agent choosing the label
`migration` still displays `handle via migration`; via disambiguates them.
The existing author string remains for wire compatibility; structured
principal/via fields and credential ownership govern permissions. Attribution renders the human's
handle for UI and `handle via label` for agents; a future external author
renders `@handle`. The local `owner` handle preserves existing UI attribution.
Activity records the owning `user_id` plus `actor_user_id`, `actor_via` and
`actor_label` for every agent write.

`agent_tokens(id,user_id,label,prefix,token_hash,created_at,last_used_at,revoked_at)`
stores hashes only. Token create/list/revoke require the principal's UI;
revocation takes effect on the next request, including an already-created MCP
handler. URL labels cannot grant another token's amendment rights: attribution
checks use principal, via and `author_token_id` on proposals/document edits.
The token’s configured label is the attribution fallback; URL labels never
grant draft-amendment rights. Token credentials resolve to an Actor before
tools execute. Trusted in-process actors without a credential retain label
identity (fixtures and the operator’s demo seed).

SSE listeners use `Map<user_id,Set<response>>`; changes emit
`notify(user_id,{deck_id?,kind})`. UI and MCP writes notify only that user.
The browser initially refetches all queries for its own stream; the payload
names what changed for later selective refetch. Global job changes may notify
all connected users with `kind: "jobs"` and reveal no tenant data.

### 17.4 Phase 1 choices and handoff

**Credential ownership of pending work.** Proposals and document edits also
store nullable `author_token_id` (FK to `agent_tokens`). The canonical three
author fields alone cannot distinguish credentials when a client changes its
URL hint; letting a hint authorize amendments would violate D7. New MCP
proposals bind to the verified credential. Archived-file import binds existing
user-1 agent proposals/edits to that migrated credential in the same durable
transaction, preserving outstanding work for existing CLI configurations.
UI-authored proposals remain UI-only for amendments by agents. Changing a
label cannot grant or remove a verified token’s rights; cross-principal
checks always precede these creator checks.

**Read/error contract.** Individual proposal/edit/log lookups, like deck and
memory lookups, return 404 when unreadable or absent. Collection reads are
filtered by principal; a memory list for a deleted deck remains empty (the
existing cleanup behavior). Numeric write references reach owner guards and
return 404 on foreign/unreadable data, including memory, log, proposal, edit
and credential IDs. Human/admin action gates remain 403. Names resolve only in the
principal’s roster, so an unknown name is 404 without searching other users.

**Local text and attribution.** The existing compact `author` and `actor`
strings stay stable in MCP prose; UI wire views add `author_display` or
`actor_display` derived from the canonical fields and current handle. This
preserves the agent playbook and deck transcript while giving UI attribution
a principal. Focus and resumable playtest-table state remain usable on Archived
lists; observations that write deck memories still require an editable deck.
Audit recording can follow lifecycle changes; it is not a second deck-edit
path. Pure calculations/card-fact utilities and operator migration/backup/
ingest utilities are classified separately from tenant capabilities.

Profile rows are created on first write for new users (empty profile reads remain
valid); user 1 is seeded on migration. This avoids an identity-registration
API before sessions exist. Shared card ingestion/backup functions are operator
utilities rather than tenant capabilities; HTTP refresh additionally requires
`users.is_admin=1`. Token management is UI-only to avoid granting an agent a
way to mint durable credentials. Archived token import is idempotent by hash,
then file rename follows the committed insert, so a crash cannot lose access.
A missing credential after a pre-v11 restore is re-imported from the saved
`stdio-token`; existing revoked rows stay revoked. No fresh token is minted at boot: the human explicitly creates and saves it
once in Settings.

Phase 2 plugs its session resolver into
`resolveUiActor(req, mode): Actor | null` in `app.ts` (the async-capable
`AppOptions.resolveUiActor` hook is called before API routes and SSE). Local
mode keeps `localActor()`; hosted mode replaces the current `null` with the
session’s principal. OAuth access-token verification goes beside
`verifyToken(db, secret, now?): Actor | null` in `tokens.ts`; `mcp/http.ts`
passes the verified Actor through the SDK’s `authInfo.extra.actor`. Phase 2
implements these seams; visibility and public pages followed in phase 4a (§18); renames remain later work.


Phase 1 review follow-up: the permission suite discovers `src/**/*.ts` exports
with a documented exclusion list for executable front doors and operator-only
modules. Every global classification has a real, invocation-counted user-2
probe; outputs and user-1 snapshots are checked. Mixed-ID rows pair an owned
deck with foreign slots/tags/engines and linked records, snapshotting all
personal tables, FTS shadows and sequences before/after rejection. Child SQL
writes bind both child and deck IDs. All private foreign targets return 404,
including writes; this resolves the plan §3/§4 contradiction in favor of §4.

The review migration fixture is frozen from the actual main v10 `openDb` at
`8b6c012`, independently of v11. Tests compare its migrated `sqlite_master`
with a fresh v11, verify historical human/system attribution and sequences,
exercise bounded FK diagnostics/rollback, and run concurrent process openers
behind a held SQLite writer lock. An INSERT audit requires every runtime
personal-table insertion to name its tenant and attribution columns, so the
migration-only defaults cannot silently authorize a future runtime write.

### 17.5 Account lifecycle and hosted UI

`SessionProvider` wraps `LiveProvider`. Hosted sign-in offers configured
providers and one invite/claim field; local mode never shows sign-in. Cookies
automatically accompany requests and EventSource. `request()` maps 401 to
sign-in, 403 to a readable-action toast, 429 to a slowdown toast with retry
time. Hosted `/privacy` and `/terms` are public and identify the operator and
contact from required non-secret `DECKBUILDER_OPERATOR_NAME` and
`DECKBUILDER_CONTACT_EMAIL` environment values, served by `/api/session`.
They describe stored data, cookies, retention, Railway, Scryfall browser
requests, export/deletion and no warranty. The footer carries the WotC Fan
Content notice and current daily Scryfall estimate/as-of wording until 1b.

Settings has hosted connection steps for Claude Code, Codex, Claude.ai and
ChatGPT, inline CLI token creation and last-seen personal/OAuth usage. Remove
Secure MCP Tunnel instructions. Local HTTP and stdio configs remain valid.

`GET /api/account/export` is UI-only: all owned rows, deck
children and Archidekt text per deck. Auth exports exclude all secrets
(session/provider tokens, credentials, signing keys). `DELETE /api/account`
requires UI and typed-handle confirmation, atomically removing all personal
rows, FTS entries, tokens, sessions, linked providers, OAuth grants and owned
clients. Foreign ownership is never inferred from authorship or invite use.
Retain others’ content and spent invitations, anonymizing references/labels
to non-login tombstone user 0 (`deleted_account`, deleted_at set). No author
token/client identifiers survive on those records. Write no activity afterwards.
Shared card/price/job data remain.
A sqlite_master inventory test verifies every tenant table participates, so
a new table cannot silently escape export/deletion. Backups expire under
14 local snapshots; bucket retention keeps the newest snapshot
for each of the last 14 UTC days and six UTC months, plus the latest upload.
Retention runs after successful upload/listing and excludes `archive/`, including
deliberate pre-change recovery snapshots and the owner-only pre-launch quarantine.
The operator manually reviews pre-change archives after at least 30 days,
verified recovery and an independent backup; never while old-schema rollback
is still needed. Operational request/tool logs omit sensitive
payloads; Railway Hobby exposes a seven-day log window. The last
admin cannot delete their account because nobody would remain to manage
invites/jobs; Settings explains why.

Admins have a Settings card for jobs/enqueue, invite creation/listing, user
count and database size. Invites are random and single-use. UI/admin guards
live in the capability layer. No impersonation. Public deck visibility,
profiles and path routing arrived in phase 4a (§18, §11.1); suggestions and renames remain later phases.

### 17.6 Phase 2 implementation choices

- Pinned `better-auth`, `@better-auth/core`, `@better-auth/mcp`,
  `@better-auth/cimd` and `@better-auth/oauth-provider` at **1.7.13**;
  `jose` at **6.2.3** verifies access tokens. No api-key plugin or native
  SQLite dependency. The official built-in driver supports our connection.
  `scripts/auth-schema.ts` is the reproducible in-memory CLI input;
  `src/migrations/auth-v12.sql` is its unmodified generated SQL.
- A random browser registration ticket stores only a hash of the invite or
  claim code. Provisioning runs in the social user-create **before** hook,
  inside Better Auth's transaction, with an explicitly generated auth ID.
  Provider access/refresh/ID tokens are encrypted with BETTER_AUTH_SECRET;
  implicit provider-account linking is disabled. The after-commit hook logs
  successful claims once. Invalid/reused tickets
  cannot leave a principal behind. Existing accounts need no ticket.
- CIMD registers **the connecting client's** metadata document (e.g. Claude
  Code's document at claude.ai), with DCR as fallback. Deck builder is the
  authorization/resource server, not an outbound OAuth client: it publishes
  resource/AS metadata, rather than inventing a client document with a
  nonfunctional callback. The real Claude Code flow verifies this choice.
- The documented CIMD Node transport pins the first DNS result, which can
  be unreachable IPv6. Our MIT-attributed adaptation uses the same public
  address policy, checks every DNS answer, resolves once and pins one
  IPv4-preferred answer, preserving Host/SNI/TLS and refusing redirects.
  There is no fallback to unpinned fetch.
- Hosted handlers buffer bounded bodies before database work. The native
  auth dialect reserves the connection per query/transaction, and service
  batches use the same gate. Neither outbound auth I/O nor an open MCP
  stream holds a reservation. Scheduler failure persistence uses the same database reservation.
  Limits remain single-instance in-memory windows. The local
  handler stays unchanged. The launcher probes public health rather than
  the now-authenticated roster to determine readiness.
- OAuth identity and label come from subject/client metadata, and draft
  amendments bind to `author_oauth_client_id`. Hosted personal labels use
  the stored token label; local URL attribution hints preserve existing
  configs. Consent and a live enabled client are rechecked for JWT usage,
  so deleting a grant disconnects an otherwise unexpired signed token.
- Lifecycle inventories group composite foreign keys as a single relation.
  Matching columns separately would leak/delete rows for another user who
  uses the same card. Global tables are explicitly listed; unknown tables
  fail closed. Auth verification payloads are searched for the identity
  when deleting codes/state, and omitted from export.
- Railway's native healthcheck Host differs from the public domain. The
  only exception to the hosted Host/Origin boundary is `GET /api/health`,
  which returns exactly `{ok,schema_version}` without cookies, auth or tenant
  data. It runs before session/body/database reservations so Railway probes
  work directly. All other hosted requests keep the single-origin rule;
  local health keeps its loopback Host/Origin check.

Existing runtime validation dependencies also receive compatible security
patches: Zod's minimum is 4.6.7, and the lockfile selects fast-uri 3.1.9.
These address the published constrained-collection resource-exhaustion and
URI-normalization advisories without changing the tool contract.

### 17.7 Owner review security corrections

Consent verifies the provider's signed oauth_query server-side before
rendering. It prominently identifies the redirect host and complete URI,
warns about local-app impersonation for loopback callbacks, and identifies
CIMD clients by their document host. DCR names are self-asserted and labelled
“Unverified app — its name was chosen by the app itself”. Anonymous DCR is
limited to loopback HTTP/native-app callbacks; web connectors use CIMD.
Unsigned/tampered/expired consent queries are refused, not rendered. See
the MCP 2026-07-28 localhost redirect security considerations.

The shared SQLite reservation now covers each auth query or transaction,
using the official pinned node:sqlite dialect wrapped at its connection
acquire/release boundary. Application service batches and job failure writes
use the same reentrant gate. Auth handlers, CIMD DNS/fetch and provider token
exchanges run outside it. A stalled anonymous auth request must not delay
a concurrent authenticated roster read or health request. The public-document
transport races DNS against the abort signal, limits concurrent fetches to
four, bounds response bytes and caches successful public documents for
30 seconds (128 entries). Late DNS answers never open a connection.


Hosted startup rejects a configured claim code shorter than 32 characters,
and missing/blank operator name or contact email. Blank-code sign-in creates
an account when invites are disabled; it never implicitly claims user 1.
Route words are reserved as handles; short names pad with neutral underscores.
Only a collision/reserved-name first-run picker may change a handle. Once
picked it is stable for future shared URLs.

With trusted ingress, rate-limit IP comes from Railway’s documented
X-Real-IP, falling back to the rightmost X-Forwarded-For hop; otherwise only
the socket counts. IPv6 addresses share a /64 bucket. Independent bounded
LRU stores per limit kind prevent anonymous capacity from denying verified
users/credentials. Unknown-code attempts have their own 5/minute/IP bucket;
known OAuth clients exchange tokens in a 240/minute/client bucket rather
than the 20/minute/IP sign-in bucket. Phase 3 verifies ingress IP headers.

Settings lists every connected OAuth consent, including unused grants, with
Disconnect. UI-only `DELETE /api/account/oauth` removes that user/client’s
consent, refresh/opaque tokens, pending codes and usage. A stored issuance
cutoff also rejects previously issued JWTs after re-consent. JWT timestamps
have one-second precision: a new grant issued in the disconnect second must
be retried in the following second. Other clients/users retain access.
Unused unconsented DCR registrations expire after one day (hourly cleanup
on auth requests). Refresh retries have five seconds of rotation grace.
Unused social linking/account listing/session listing/generic HTTP get-session/revoke endpoints are
disabled; no browser API exposes raw session tokens. Callback state/provider
errors ask the user to restart sign-in; registration errors mention the code.

Hosted legal routes and legal footer are absent in local mode. Hashed SPA
assets use `public, max-age=31536000, immutable`; APIs, auth, consent, HTML
and the index route remain `no-store`.

---

## 18. Sharing

*Built in v3 phase 4a (2026-10-25). The plan's decision is reproduced
verbatim; the rest of this section records how it was built and the choices
the plan left open.*

### 18.1 The decision (v3 plan D8, verbatim)

> **D8. Visibility and sharing.**
>
> ```sql
> ALTER TABLE decks ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private'
>   CHECK (visibility IN ('private','unlisted','public'));
> ALTER TABLE decks ADD COLUMN slug TEXT UNIQUE;          -- 10 chars base62, set when visibility first leaves private, never changes
> ALTER TABLE decks ADD COLUMN publish_decisions INTEGER NOT NULL DEFAULT 0;
> ALTER TABLE decks ADD COLUMN accept_suggestions INTEGER NOT NULL DEFAULT 0;
> ALTER TABLE log   ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;   -- owner hides single entries from the public page
> ```
>
> Public URL: `/d/<slug>`. A public page shows the list by slot, brief, checks,
> bracket target and the owner's latest assessment, price estimate with source
> and date, and the decisions log only if `publish_decisions = 1`, minus hidden
> entries. Never shown: memories, proposals, pending edits, ownership evidence,
> activity, the owner's agent names. Cross-owner forks copy list, slots, tags,
> brief and engines; never memories, ownership evidence, assessments or
> unpublished log entries.

Schema 14 (following phase 1b's 13) adds exactly those columns, plus the fork lineage snapshot
(`decks.parent_owner_handle`, `parent_slug`, `parent_name`, §18.4) and
`cards.artist` (§18.5). `accept_suggestions` has no setter until phase 5.

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

- **Viewers address a deck only by its slug.** Numeric deck ids stay the
  owner's namespace: every id-addressed capability, tool and route is 404
  for anyone but the owner, whatever the visibility, exactly as in phase 1.
  `requireDeck` therefore remains the owner's read gate, and
  `requireViewableDeck(db, viewer, slug)` admits viewers by visibility and
  returns the role. Only the public capabilities call it:
  `publicDeckState`, `exportPublicDeck`, forking or comparing by slug, and
  `publicProfile`. *(Choice: the plan has `requireDeck` admit viewers. Doing
  that would have opened every existing id-addressed read — proposals,
  memories, log — to viewers, and made rule 1 depend on every caller
  remembering to trim. A separate gate keeps viewers to one projection.)*
- A viewer is a signed-in non-owner, any agent acting for one, or nobody at
  all (`viewer = null`, hosted mode without a session). Private decks are
  404 to all of them; unlisted and public decks return `PublicDeckState`.
- The owner opening their own slug gets the same page with a banner saying
  it is how viewers see it, including while the deck is private. That is
  the Share dialog's "what viewers see" preview.

**`PublicDeckState`** (plan §4) is built by `publicDeckState()` in
`src/state.ts` from an explicit allow-list, never by trimming the owner's
composite:

- `deck { name, slug, owner { handle, display_name }, commander,
  color_identity, category, visibility, parent? { owner_handle, slug, name } }`.
  `commander` lists the commander names. No deck id or revision. `parent`
  is looked up live, not read from the fork's snapshot: with its link while
  the parent is public, with only handle and name while it is unlisted (its
  link is its owner's to give out), and absent once it is private or gone.
- `cards`: the list including the maybe list, each with its card facts,
  quantity, role and slots; `maybe_note` is always empty, because shortlist
  notes are working notes, and `tag_ids` is always empty.
- `slots`, `tags` (always empty), `brief { thesis, constraints_md }`,
  `engines` (name, description, pieces and whether each is in the deck),
  `checks` without the `power_*` findings, which read the latest assessment
  and so may describe an agent's. *(Choice, overriding D8's "tags" on both
  the page and forks: tags record sourcing and status (§5.4), and "Have"
  creates ownership evidence (§5.5), which D8 never shows. The key stays so
  the §4 shape holds.)*
- `power { target, latest_owner_assessment }`: the target's profile, Game
  Changer limit, notes and bracket mapping; and the newest assessment the
  owner recorded themselves in the app (`author_via = 'ui'`), with its
  profile, evidence, confidence, notes, mapping and whether the list has
  changed since. No attribution on either, and agent assessments never
  appear.
- `price { total_cents, currency, store, unpriced, as_of, source: "Scryfall" }`.
  Priced for the viewer (D18): a signed-in viewer's own currency and store,
  else `USD`/`lowest`; never the owner's choice. Each card's price follows the
  same choice, and `computed.cost` is the same total.
- `decisions` only when `publish_decisions = 1`: the newest 100 log entries
  that are not hidden (an undo of a hidden entry is hidden with it), each
  with its kind, a summary the app writes from the cards it moved, the
  owner's reason, time and cards. The stored summary is not used: it is
  free text that can quote a proposal ("proposed because: …") or an agent.
  Who acted is `by: "owner" | "agent"`, never an agent's name or handle.
  Proposal and edit ids are dropped.
- `computed`: the app's counts, curve, pips and slot deltas, computed as if
  nothing were pending, so pending proposals leave no trace.

The capability reads the owner's deck through the existing read functions
with a read-only owner context and then projects; the projection is the
only thing returned. The permission matrix asserts the exact key set for
every viewer and visibility.

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

- **`/d/<slug>`** is one component, `ReadOnlyDeck`, also used by the phone
  view (§11.2): the list by slot or type, checks, read-only brief and
  engines, the power target and owner assessment, the price with its
  source, store and date, and the decisions log when published. The header
  carries the first commander's stored art crop with "Art by …" from
  `cards.artist`; with no stored artist (a database not refreshed since
  schema 14) the art is left out rather than shown uncredited. Cards stay
  text rows; tapping one opens its card sheet, as hover does on desktop.
- **Actions:** Copy link and Export (Archidekt text, no account) for anyone;
  Fork and Compare with one of my decks when signed in. In local mode the
  local user is always signed in. No sidebar, inbox, focus reporting or
  decision hotkeys.
- **`/u/<handle>`**: the display name and the user's public decks (not
  unlisted ones), nothing else. 404 for unknown or deleted handles, and for
  anyone without at least one public deck: the display name comes from the
  sign-in provider (often a real name) and is not shown to someone who
  merely guesses a handle. A display name the user sets themselves is
  deferred to phase 4b or 6.
- **Link previews.** For `/d/<slug>` and `/u/<handle>` the server injects
  `<title>`, `<meta name="description">`, Open Graph (`og:title`,
  `og:description`, `og:type`, `og:url`, `og:image`, `og:site_name`) and
  `twitter:card` tags into `index.html` before serving it. The deck
  description names the commander, colour identity, bracket target and
  owner. `og:image` is the commander's stored art crop on
  `cards.scryfall.io`, only when the card's artist is stored too, as on the
  page. A slug the requester cannot read gets the plain page
  with status 404. Nothing calls `api.scryfall.com` from a browser.
- The page is laid out single-column below 720 px with wrapping rows and
  44 px tap targets, and never scrolls horizontally at 375 px.

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

