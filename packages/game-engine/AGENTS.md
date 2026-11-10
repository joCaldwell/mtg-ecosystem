# Compiler and engine development

## Review and verification ownership

Josh's 2026-11-03 workflow requires focused implementer checks, independent source
review, then **one full acceptance run on the final reviewed freeze**, owned by
the reviewer or a designated verifier. Implementers must not each run the full
engine/demo/all-profile suite. Do not rerun a passed full gate after read-only
review or documentation-only edits; explain which source change invalidates a
phase before repeating it. Preserve every required guard, workload and replay.
See `docs/game-engine-verification-workflow.md` for ownership, parallel work and
the measured optimization loop. This supersedes earlier task briefs assigning
the complete gate to every implementation agent.

D1b-2 removes runtime lineage gates. The historical profile descriptions below
record earlier release boundaries; they no longer enable rules or select limits.
All admitted catalogs execute the common rules implementation. See
`docs/game-engine-capacity-policy.md` for current limits and typed atomic faults.
D1b-3 now loads legal match admission from `tests/fixtures/profiles/`: catalog
manifest references, decks, commander policy, hosting/search flags and completion
layout. See `docs/game-engine-profiles.md`. Foundations and attachments are hosted;
only foundations is search-enabled. `match-batch PROFILE OUTPUT` and
`match-replay PROFILE FILE` replace per-profile CLI selectors. The capacity policy
now uses measured shared ceilings and bounds retained token identities.
Feature requests must fit the explicit reviewed capability sets; catalog integrity
and the selected dated banned-list digest remain bound independently.
`catalog build MANIFEST [--write]` replaces all per-catalog compile commands.
D1c uses one IR-driven Develop policy and one generic interaction summary;
see `docs/game-engine-cards-as-data-d1c.md` for the re-baseline and witnesses.

D2b adds common APNAP legend-rule choices, exact archive-bound source references,
intrinsic flash and unconditional source entry tapped. The two new capabilities
require IR v20 and remain outside reviewed hosting/search sets. Ability-word
labels never replace underlying semantics. Rule-bearing subtypes retain explicit
rejections. See `docs/game-engine-cards-as-data-d2b.md` for witnesses, reviews,
verification and the 1,904/32,116 compiler coverage measurement.


D2c-a adds `StepBeginningTriggerV1` at IR v21: independent upkeep/draw/end and
controller/any-player/opponent predicates, actual transition occurrence evidence,
entry actions before APNAP placement, and typed movement/token/step witnesses.
Trusted snapshot/bundle versions are now 19/20; public projection/context/sidecar
remain 16/16/15. D2c-a initially rejected step conditions and event-player recipients; D2c-b below
extends that finite subset. Optional effects and unsupported functioning zones
still reject whole cards. Hosting/search review
sets exclude the capability. See `docs/game-engine-cards-as-data-d2c-a.md`.
The settled check passed 959 Rust tests (18 existing diagnostics ignored), 57 Node
checks, 114 focused release checks, and 47 SDK tests with strict TypeScript.
Coverage is 1,923/32,116 (5.98767%), nineteen additions and zero losses from D2b.
All twelve unchanged profile batches and explicit replays passed 216/216 with
winners, zero faults/truncations. The report records the 173-snapshot proof for
37 representation-driven baseline changes, retained identities, failed attempts
and evidence paths. These are functional checks, not a new quiet P7 claim.

D2c-b adds frozen event-player recipients for mandatory draw/life programs and
source-status intervening conditions at IR v22. Status reads the exact source's
current battlefield information or departure-refreshed LKI, independent of traces
and bounded by existing live continuations. Snapshot/bundle remain 19/20 and public
versions 16/16/15; all old catalog bytes and 173 historical state hashes are unchanged.
Verification passed **976 Rust tests (18 existing diagnostics ignored), 58 Node,
131 focused release checks, 47 SDK tests/strict TypeScript**, and all **216 games
and 216 explicit replays** across twelve unchanged profiles. Coverage is
**1,929/32,116 (6.00635%)**, six additions and zero losses from D2c-a. New
capabilities stay outside reviewed hosting/search sets; Master of the Feast's
multiplayer-draw rejection and runtime guard remain. Parent independent review is
pending. See `docs/game-engine-cards-as-data-d2c-b.md` for source witnesses, identities and retained evidence.

Rust owns contracts, offline compilation, deterministic core, and the scenario
CLI. Read `docs/game-engine-first-scenario.md` for frozen inputs and restrictions.
The current follow-up contracts are `docs/game-engine-p2-slice.md` and
`docs/game-engine-departure-slice.md`; the latter adds the exact Isamaru scenario,
IR v2 and the exact Isamaru profile. The next contract is
`docs/game-engine-event-slice.md`: atomic event groups, nonsemantic trace modes,
and snapshot/bundle v4. `docs/game-engine-stack-slice.md` adds the exact
nine-card instant-response profile, detached resolution, snapshot/bundle v5,
projection v3, and query-sidecar v2. `docs/game-engine-payment-slice.md` adds
casting-time intrinsic mana, keep-all/undo-all failed-payment choices, the exact
four-card Forest reversal profile, snapshot/bundle v6, projection v4, and
query-sidecar v3. `docs/game-engine-turns-slice.md` adds the exact eight-card
turn/land/creature profile, IR v3, snapshot/bundle v7, projection v5 and
query-sidecar v4. `docs/game-engine-combat-games.md` adds two exact combat
configurations, ordered libraries, cleanup, damage/SBAs, and complete scenario
games; snapshot v8, bundle v9, projection v6 and query-sidecar v5. Bundle v9
records effective optimization/debug settings; ordered snapshot verification
shares a proved independent replay prefix without weakening standalone restore.
These historical profiles are scenarios, not legal Commander matches.

`docs/game-engine-commander-casting.md` adds three exact Commander profiles,
a separate six-definition catalog with source-backed Plains, command casting/tax,
owner hand-replacement and fresh graveyard/exile SBA return choices, and recasting.
Snapshot v9, bundle v10, projection v7 and query-sidecar v6 retain these choices.
Historical profiles retain their separate five-definition catalog.

`docs/game-engine-commander-damage-library.md` adds three exact profiles with
physical Commander combat-damage accumulation/loss and Time Ebb library-top
movement. Damage uses a separate seven-definition IR v3 catalog; library uses
another seven-definition IR v4 catalog with sorcery timing and explicit top
placement. Snapshot v10, bundle v11, projection/context v8 and query-sidecar v7
retain damage facts and mandatory library-to-hand draw replacement choices.

`docs/game-engine-actions-payments.md` adds four exact action profiles and a
separate eleven-definition IR v5 catalog. These profiles require explicit mana
spending, admit Cruel Edict opponent targets and resolution sacrifice choices,
and Hapless Researcher source-sacrifice activation with detached draw/discard
resolution. Snapshot v11, bundle v12, projection/context v9 and query-sidecar v8
retain all nested choices and noncard stack identities. Historical catalogs,
profiles and gameplay references remain separate.

`docs/game-engine-stack-costs.md` adds four exact stack/cost profiles and a
fifteen-definition IR v6 catalog. Counterspell/Stifle bind distinct stack
instances; Merfolk Looter pays a tap cost and Azure Mage pays an explicit mana
cost. Snapshot v12, bundle v13, projection/context v10 and query-sidecar v9 retain
announcements, current/historical stack handles and counter outcomes. The five
historical catalogs and all historical gameplay hashes remain separate references.

`docs/game-engine-damage-life.md` adds five exact damage/life profiles and a
twenty-one-definition IR v7 catalog. Lightning Bolt and Prodigal Sorcerer use
the complete any-target domain with only creature/player inhabitants admitted.
Soul Feast, Healing Hands and Sacred Nectar retain ordered life/draw programs.
Snapshot v13, bundle v14, projection/context v11 and query-sidecar v10 retain
mixed target identities, targeted tap announcements and historical damage sources.
The six earlier catalogs and their gameplay references remain separate.

`docs/game-engine-triggers.md` adds five exact ordinary-trigger profiles and a
separate twenty-four-definition IR v8 catalog. Blood Artist, Elvish Visionary
and Essence Warden use actual typed events, simultaneous death lookback, pending
identities, APNAP/controller ordering and mandatory targets. Stifle includes
triggered invocations. Snapshot v14, bundle v15, projection/context v12 and
query-sidecar v11 retain these choices. Historical catalogs remain separate;
complete compiler lowering is distinct from their admission exclusions.

`docs/game-engine-effects.md` adds five exact temporary-characteristic and damage
interaction profiles with a separate thirty-definition IR v9 catalog. Giant
Growth/Disfigure use incarnation-bound additive P/T; Fog and Samite Healer create
turn-scoped prevention; Furnace of Rath supplies a battlefield static damage
doubler. Affected players/controllers choose replacement order and finite shield
allocation. Applied transformations persist across later provider departures;
final damage remains simultaneous. Cleanup removes marks and expires effects
atomically. Snapshot v15, bundle v16, projection/context v13 and query-sidecar v12
retain these continuations. Historical catalogs and gameplay references remain
separate.

`docs/game-engine-choices.md` adds six exact profiles with a separate
thirty-three-definition IR v10 catalog. Healing Salve selects a complete mode;
Seeds of Strength retains three target occurrences; Slaughterhouse Bouncer
checks its intervening condition at matching and resolution. Three-point
prevention uses complete simultaneous allocation vectors. Snapshot v16, bundle
v17, projection/context v14 and query-sidecar v13 retain these continuations.
Historical catalogs remain separate from the legal-match catalog below.

`docs/game-engine-matches.md` releases the finite `commander-foundations-v1`
profile: a separate 34-definition IR v11 catalog, a source-backed 21-card
construction pool, full four-seat deck/pod admission, explicit seeded shuffle,
multiplayer mulligans and natural complete games. Isamaru and Yargle and Multani
are the admitted commanders. Snapshot v17, bundle v18, projection/context v15
and query-sidecar v14 retain setup and simultaneous owner-return continuations.
Do not generalize this named profile to arbitrary Commander decks or strategies.

`docs/game-engine-keywords.md` adds the separate `commander-keywords-v1`
profile: 50 definitions in IR v12, intrinsic flying, reach, vigilance, haste,
lifelink and deathtouch, and public per-blocker legality hints. Damage gains use
final dealt quantities atomically; deathtouch is consumed by the next SBA wave.
The profile table binds its own policy, catalog and default decks. Foundations
IR v11, policy, projection bytes, search and server scope remain unchanged;
snapshot/bundle/projection/sidecar versions stay 18/19/16/15. No trample, first/double strike,
menace, keyword grants, indestructible or regeneration is admitted.

`docs/game-engine-combat-keywords.md` adds the separate
`commander-combat-keywords-v1` profile: 59 definitions in IR v13, intrinsic
trample, first strike, double strike and menace. An incarnation-bound first-step
set selects damage participants; priority, SBAs, triggers and instants separate
the two steps. Public requests expose menace attackers and trample's player
recipient/lethal requirements; `TrampleDamage` responses retain object-handle
assignments plus explicit player/amount. Both steps use final damage prevention,
lifelink, deathtouch and physical Commander history. All earlier catalogs,
policies, expected fixtures and projection fields remain unchanged; optional
fields omit empty values. Versions remain 18/19/16/15. Search and server hosting
remain foundations-only. No keyword grants/removal, protection, planeswalkers,
banding or damage-assignment order is admitted. The simulator exposes
`match-batch commander-combat-keywords-v1 OUTPUT` and `match-replay commander-combat-keywords-v1 FILE`.

`docs/game-engine-tokens-counters.md` adds the separate `commander-tokens-v1`
profile: 72 definitions in IR v14, fixed defined creature tokens and fixed
+1/+1 counters in spells and self-enters triggers. Tokens use a reserved physical
identity range and enter simultaneously; dies triggers see their destination
before the next SBA removes their objects. Counters join additive layer 7c and
reset on zone change. Public token rows expose defined characteristics, names,
keywords and viewer handles; they have no Oracle identity. Optional token/counter
fields omit absent values. Token-only capacities are 64 lifetime tokens, 344
damage occurrences, 1,892 stack/pending entries and 468 event-group members;
`Game::typed_fault` identifies overflow and the contract derives each bound.
Earlier profile limits and bytes, versions 18/19/16/15, search and foundations-only
server hosting remain unchanged. The simulator adds `match-batch commander-tokens-v1 OUTPUT`
and `match-replay commander-tokens-v1 FILE`. Copy/noncreature/variable tokens, other counter
kinds, proliferate and keyword grants remain outside this finite release.

`docs/game-engine-staples.md` defines `commander-staples-v1`: a separate
81-definition IR v15 catalog adds creature destruction, graveyard-to-hand
recursion, optional Gravedigger resolution, private basic-land search with
seeded shuffle, targeted draw/life loss, and a continuous controller-scoped
layer-7c anthem. Enchantment permanent spells use ordinary match casting.
Staples alone uses 256 live tokens, 736 damage occurrences, 3,752 stack/pending
entries and 660 event members; the contract derives these bounds with recursion.
The exact pool's separate 68-token lifetime proof is guarded by a catalog test.
Search handles last only for that resolution choice. Earlier limits, fixtures,
versions 18/19/16/15, and foundations-only search/server scope remain unchanged.
The simulator adds `match-batch commander-staples-v1 OUTPUT` and `match-replay commander-staples-v1 FILE`.
Regeneration, restricted destruction, other searches/recursion and general
continuous effects remain excluded.

`docs/game-engine-mana-permanents.md` defines `commander-mana-v1`: a separate
90-definition IR v16 catalog adds artifacts, colorless mana, fixed/any-color/
commander-identity mana, rocks, dorks and nonbasic Command Tower. Mana abilities
resolve without stack entries, with controller-only color choices and nested
payments for Golgari Signet. Receipts undo exact production and nested spending;
creature tap costs obey continuous control/haste. Sphere and Mind Stone's
sacrifice-draw abilities use the existing nonmana stack path. Only mana-v1 uses
760 damage occurrences and 3,868 stack/pending entries; 256 live tokens, 660
event members and 32 effects retain the staples derivation. Versions remain
18/19/16/15; optional fields omit absent values. The simulator adds
`match-batch commander-mana-v1 OUTPUT` and `match-replay commander-mana-v1 FILE`, with generic instruction, trigger, keyword and card-use counts. Server hosting and
search remain foundations-only. Colorless-specific costs, "could produce",
triggered mana abilities, damage-dealing mana abilities and multiple commanders
remain excluded.

`docs/game-engine-blue-red.md` defines `commander-four-colors-v1`: the seventh
legal profile, with 106 definitions at unchanged IR v16. It adds Caelorna and
Rorix commanders, exact mono-blue/red decks (80 Island/84 Mountain), and a
white/BG/blue/red default pod. Thirteen match-mode witness groups cover the
reused blue/red spells, abilities, tokens, counters, anthems, commander returns
and final Furnace damage. Bounds are 256 live tokens, 760 damage occurrences,
4,080 stack/pending entries, 660 event members and 32 effects; the contract
re-derives them including repeatable Azure Mage activation and bounce/tuck.
The simulator adds `match-batch commander-four-colors-v1 OUTPUT` and
`match-replay commander-four-colors-v1 FILE`, with generic instruction, trigger, keyword and card-use counts. Earlier fixtures/limits, versions 18/19/16/15 and
foundations-only search/server hosting remain separate. Target-opponent token
creation (Hunted Dragon) remains a whole-card rejection.

`docs/game-engine-attachments.md` defines `commander-attachments-v1`: a separate
118-definition IR v17 catalog adds Equipment/equip, creature Auras, continuous
layer-6 keyword grants (including hexproof/shroud), attachment P/T and combat
restrictions, attachment SBAs and Rancor return triggers. Targeting restrictions
use invocation controllers at announcement and resolution; damage LKI retains
granted keywords. Its stack/pending bound is 4,093; damage/event/effect/live-token
bounds remain 760/660/32/256. The simulator adds `match-batch commander-attachments-v1 OUTPUT`
and `match-replay commander-attachments-v1 FILE`. Versions remain 18/19/16/15; omitted empty
fields preserve earlier projections. Search and server remain foundations-only.
See the progress ledger for acceptance status and private witness boundaries.

`docs/game-engine-removal.md` defines `commander-removal-v1`: a separate
135-definition IR v18 catalog adds filtered permanent destruction/exile,
resolution-start target-controller/power/mana-value LKI, optional targeted
self-enters triggers, targeted sacrifice/mana activations, and typed spell
counters with compensation tokens. Public player rows expose owned exile cards
by viewer handles; optional effects have their own response. Bounds are 512
live tokens, 1,296 damage occurrences, 6,381 stack/pending entries, 916 event
members and 32 temporary effects, with typed capacity faults and a conservative
116-token lifetime proof. The simulator adds `match-batch commander-removal-v1 OUTPUT` and
`match-replay commander-removal-v1 FILE`. Earlier limits/fixtures and versions 18/19/16/15
remain unchanged; server hosting and search remain foundations-only.

`docs/game-engine-sacrifice.md` defines `commander-sacrifice-v1`: a separate
153-definition IR v19 catalog adds filtered sacrifice costs, additional spell
costs, filtered dies payoffs, sacrificed-power LKI, each-opponent effects, APNAP
simultaneous forced sacrifices and source can't-block. This profile alone uses
8,192 live tokens, 17,184 damage occurrences, 294,540 stack/pending entries,
8,596 event members and 8,192 effects, with typed faults and the contract's
finite lifetime/episode proofs. The simulator adds `match-batch commander-sacrifice-v1 OUTPUT`
and `match-replay commander-sacrifice-v1 FILE`. Earlier fixtures/capacities and versions
18/19/16/15 remain unchanged; hosting remains foundations/attachments only and
search foundations-only. See the progress ledger for acceptance evidence and
the reachable Requiem Angel witness substitution.

`docs/game-engine-board-wipes.md` defines `commander-wipes-v1`: a separate
168-definition IR v20 catalog adds filtered simultaneous destroy/exile/bounce,
fixed-membership -N/-N sets, simultaneous creature/player damage, counted destroy
results, announced exact X, self cost reduction, uncounterable spells, APNAP
including-caster sacrifice, and turn-scoped Anger exile replacement. Wurm dies
triggers retain the dying controller. Profile-only capacities are 65,536 live
tokens, 131,872 damage occurrences, 2,442,108 stack/pending entries, 65,940 event
members and 65,536 effects; the contract includes mass-return/Gravedigger reuse
in both lifetime and episode proofs. The simulator adds `match-batch commander-wipes-v1` and
`match-replay commander-wipes-v1`. Earlier fixtures/capacities and versions 18/19/16/15 remain
unchanged; hosting remains foundations/attachments and search foundations only.
See the progress ledger for acceptance evidence and witness reachability.

`docs/game-engine-search.md` releases finite P7 search for that same
pool. Sealed captures contain only permitted facts/history; complete hypothetical
hands/libraries and independent future randomness construct sampled games.
Supported roots are fresh active PrecombatMain priority with four, three or two
living seats and no unresolved continuation. Own mulligan/bottom knowledge and
remembered announced cards constrain hypotheses. Sample restore proves its origin
and future replay; real restore never accepts a sample origin. Snapshot v18,
bundle v19, projection/context v16 and query-sidecar v15 remain separate from P5.
The frozen build passed all sixty primary search jobs, independent semantic
replay, workspace checks, fresh offline rebuild/replay and the retained-bundle audit. Performance
claims must identify their exact source/executable, not merely the profile name.

`docs/game-engine-server.md` defines the finite local Rust host and generated
TypeScript SDK. Hosting owns OS processes, authentication, HTTP, SQLite, clocks
and resource groups; these dependencies never enter the deterministic core.
The core's complete immutable history-byte memos preserve canonical State bytes,
with full generic serialization as the independent oracle and COW invalidation
before mutation. Field-specific Commander-history map decoding preserves the
existing wire format through nested tagged publications. Snapshot/bundle and
public projection versions remain those of P7. The current P8 build
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`
passed the sixty-job quiet primary measurement and all sixty independent semantic
replays: p95 2,662,119µs, maximum 2,690,433µs, kernel whole-process peak
394,698,752 bytes, 4,188 useful trials and 3,012 unstarted trials excluded.
The independent review is
`server-proof-archive/files/current-search-primary/run-1-independent-primary-review.json`
in the external store named by `docs/game-engine-server.md`
(SHA256 `4650f87e12bf94f2ec35c2427c7d5aa4db463b91320253a049c05371af1527e4`).
These figures concern the finite search workload, not full-pod throughput or scaling.

The final check-engine phase passed 526 Rust debug tests and 40 Node checks;
release passed 526 Rust tests; SDK verification passed 40 tests and strict
TypeScript. Fresh engine retention subsequently passed; server/SDK retention
passed after two host-only corrections for npm 12 and repository-relative SDK
fixtures. The final four-phase reuse preserves each original source/freeze/log
qualification; all 192 native and 23 SDK files stayed unchanged. Fourteen focused
host guards and independent native/server artifact audits passed. Failed and
cancelled attempts remain immutable. The final external archive and release
identities are in `docs/game-engine-server.md`. Dense/scaling/omniscient diagnostic
measurements remain required broader-plan work, separate from finite acceptance.

Run from repository root, using pinned Rust 1.99.0:

- `npm run check:engine`: toolchain check, formatting, Clippy, Rust tests, and
  the Node cross-language fixture contract check. The test profile uses optimization
  level 2 with debug assertions and overflow checks enabled.
- `cargo test --workspace --locked` and `cargo test --workspace --locked --release`.
- `target/release/mtg-sim match-batch PROFILE OUTPUT`: 18 games and independent replay,
  with generic batch coverage in `report.json`; OUTPUT must be new.
- `target/release/mtg-sim match-replay PROFILE FILE`: verify a saved game.
- `cargo run --locked -p mtg-sim -- demo`: verified source-to-runtime demonstration.
- `cargo run --locked -p mtg-sim -- schema`: export the executable JSON Schema.
- `npm run retain:engine`: retain source/dependency closure and verify an offline
  rebuild/replay with a fresh Cargo home and target directory.
  Retention runs through a bounded scratch supervisor; use the npm entry point.
  See `docs/game-engine-storage.md` for disk budgets, cancellation cleanup and
  dry-run inventory/cleanup commands. Historical generated evidence is not a cache.

Search also exposes `search-roots`, `search-diagnostic INDEX`,
`search-benchmark OUTPUT --quiescent` and `search-replay REPORT` through the
release `mtg-sim` CLI. The primary benchmark requires a quiet reference machine;
semantic replay verifies recorded work without remeasuring its timing. Keep
fixture/evidence adapters separate from the permitted-data sampler and policies.

Contracts' Rust types are the IR authority; schema output is generated and
ignored. Source archive/AST, IR, and internal snapshots have separate versions.
Core imports contracts only, never compiler/parser. Network, filesystem, clock,
OS randomness, async tasks, and agent inference belong outside core. Shared
immutable catalogs and owned match tables are the baseline. No unsafe code.

Card facts come from the frozen exact source fixtures. Cite pinned official
rules in implementation comments. Compile the complete source card or reject
it; unknown semantics never disappear. Admission checks build/profile and
whole-scenario configuration separately from lowering. Test both successful
source-to-runtime behavior and paired unsupported/invalid cases.

Keep physical IDs, zone incarnations, stack IDs, owners, and controllers distinct.
Use exact game quantities and checked structural arithmetic. Frames, inputs,
events, and outcomes serialize as data. Queries cannot mutate semantic state.
Restore/replay must preserve behavior and clone branches must be independent.
Use explicit `Budget` with `advance`. Player-facing code consumes `Observation`,
`PlayerRequest`, and `PlayerResponse` and budgeted query outcomes; raw IDs,
`decision_for`, snapshots, and full-state getters are trusted host APIs. Never
send query recovery sidecars or validation receipts to clients. Queries cannot
mint handles or observation history. See `docs/game-engine-p2-slice.md` for
handle lifetime and work invalidation/recovery contracts.
Do not update a failing expected result without reviewing its source and rules.

Commit small reviewed fixtures and gate summaries. Keep downloaded corpora,
bulk IR, schema exports, replay bundles, timing reports, and Cargo target output
ignored. Add mechanics only after their compiler/runtime/interaction evidence;
do not broaden the first pool as an incidental fix.

Concessions are trusted host external inputs queued at recorded rule boundaries;
process them before ordinary work and expose no query while they are pending.
Keep original seating, departed-player facts, and physical Commander history.
Default control is separate from ownership and explicit control grants.
Command-zone casting/tax and hand/graveyard/exile return choices are supported
in the exact Commander, damage/library and action profiles. A failed cast restores its original
zone/incarnation; completing a command-origin cast alone advances physical,
per-caster tax history. The damage/library and action profiles accumulate
actual Commander combat damage and apply the 21-damage SBA loss rule. Earlier
Commander profiles retain their explicit damage fault. Library replacement is
enabled only for the reviewed Time Ebb and subsequent ordinary/ability draw paths.
Replacement ordering is limited to the reviewed effects and subsequent profiles.
Legal match setup is admitted through the profile data registry; its current
files also include the D1b-3 data-only demonstration profile. Ordinary combat is enabled in the reviewed combat, Commander and action profiles.

Batch mutation is private and atomic. Capture consumed pre-event facts and use
actual typed outcome slots. Simultaneous groups admit independent read/write
footprints only; general event/replacement interactions remain unsupported.
`Game::with_trace` selects off, compact, or detailed retention. Diagnostics never
enter semantic hashes or player projections. Native `events()` is populated only
in detailed mode; `trace()` is trusted evidence, not a client stream.

Stack projections run bottom to top. Saved announcement handles are historical
references; they cannot rebind to a later incarnation or authorize responses.
Lower entries remain completed while the top is announced. Reversal restores
prior passes filtered by living players. Resolving frames own frozen instances
and continue supported choice-free effects after their source leaves (CR 608.2m).
Only the caster answers target and mana-window requests; no player receives
priority during announcement or resolution. Always offer casting-time mana,
even when already funded. FinishMana attempts complete payment and cannot
cancel a payable spell. Undo subtracts only receipt-recorded casting-time mana
and untaps surviving sources; preserve pre-cast mana and departures. Retained
legal mana activations clear prior passes. A removed caster never restores its
last-known mana or resurrects sources. Empty-stack progression is enabled only
for the exact turns and combat profiles. Earlier profiles retain the original
fault. The old turns profile retains singleton draws and forced-empty combat;
new combat profiles use explicit ordered libraries, ordinary combat, failed-draw/
life losses, and cleanup discard. Unsupported boundaries fault explicitly. Restore proves fault
reachability by replay. Land plays and creature casts require active-player
main-phase priority and an empty stack; land usage resets only on a new turn.

Combat declarations, simultaneous damage and SBA waves use the typed event path.
Multiple blockers have no damage-assignment order under the pinned current rules.
Preserve blocked status after blockers leave, and skip blockers/damage only if
no attackers were declared. Continuous-control eligibility is stamped at turn
beginning and resets on new incarnations or controller changes. All simultaneous
losers are absent before departure cleanup and outcome adjudication. Zero
toughness and all-player loss/draw remain private domain fixtures in the earlier
positive-life, positive-toughness pools. Wipes-v1 has accepted-pod witnesses for
zero toughness and simultaneous last-player loss/draw.

Historical profiles enforce monochromatic reachable mana. The four action
profiles allow mixed colors only through an explicit exact spending decision;
never choose generic payment colors implicitly in those profiles. The simulator's policies consume only player-facing
observations/requests; host checkpoint/replay verification is separate. Budget
truncation and faults never substitute for rules-defined outcomes.

Owner return decisions are resumable continuations, never priority windows.
Hand return replaces the movement before it happens; graveyard/exile return
requires actual entry and the next SBA check. Decline consumes that incarnation's
opportunity. Drain concessions and revalidate pending choices before exposing
queries. Source departure preserves a started resolution; owner departure never
resurrects an object. Queries and recovered validation work retain executor/state
scoping at these choices.

Drawing a tucked commander requires a second owner replacement under CR 903.9b.
The request uses an existing viewer-scoped public designation and an optional
object handle: hidden-library draws carry `None`, visible-zone choices carry
`Some(handle)`. Never reveal library rows or mint hidden object handles. Complete
one draw attempt once, preserving the next library card when the move is replaced.
Explicit library-top events retain prior ordering and cannot compose with a
conflicting order write. Commander damage belongs to the physical source and
recipient, survives zone/controller changes, and never derives from life changes.
Validate the entire simultaneous damage group before writing life/history; freeze
all simultaneous loss candidates before departures and outcome adjudication.

Activated abilities have their own stack identity and historical source handle;
their source card never moves to the stack. Sacrifice costs happen before
priority and use the ordinary post-activation SBA check. Sacrifice/discard during
resolution defer SBAs until the complete invocation finishes. The invocation's
controller remains the meaning of "you" after source or player departure; a
departed controller's personal draw/discard is skipped, never redirected. A
borrowed source's owner leaving does not remove a surviving activator's ability.

Stack target inputs use current viewer-scoped `StackHandle`s bound to a distinct
`StackId`; physical source handles cannot authorize stack targets. Exclude the
announcing invocation itself and all incomplete entries. Counterspell targets
spells; Stifle's complete activated-or-triggered domain contains only nonmana
activated entries in the older stack/cost pool; the trigger and effects pools
also admit ordinary triggered invocations.
Countering a spell moves its card to its owner's graveyard; countering an ability
removes only the noncard entry. Never refund paid costs or resolve its program.
Commander grave return waits until the complete counter invocation finishes.
The counter event owns aggregate stack membership/control-flow writes and cannot
compose with another simultaneous occurrence.

Tap activation costs require an untapped source and continuous-control eligibility;
the keywords profile additionally admits the intrinsic haste exception (CR702.10c).
Mana-cost activations have their own announcement,
mana window and exact spending choice. Source movement or source-owner departure
after announcement preserves a surviving activator's frozen invocation with a mana cost.
Failed payment removes that proposal and optionally undoes only its recorded mana
activations, preserving source state, prior combat taps and preexisting mana.

Damage/life profiles use `EntityTargets` for player/creature damage targets and
`PlayerTargets` for any-player life targets, including self. Historical Edict
targets remain opponent-only. Targeted tap abilities choose targets before paying
their cost and expose no mana window. Noncombat damage retains its historical
source incarnation and frozen controller after source movement or owner departure;
it never updates Commander combat history. Direct life changes are separate
typed events. Run SBAs after the complete program, allowing Soul Feast's self
loss/gain to recover from zero and Healing Hands' gain/draw to finish before loss.
If every target is illegal at resolution start, skip the complete program.
Creature damage accumulates until cleanup; lethal damage uses the ordinary SBA
destruction and Commander-return paths. Trigger scheduling and damage prevention remain unsupported in those historical
damage/life profiles; the later trigger/effects profiles enable their reviewed
finite families.


D2c-c ordinary cast triggers passed **combined acceptance at frozen `a7f5571`**
and independent parent evidence audit. Later D2c-d/lifetime changes are excluded.
IR23 separates caster
relations from compositional spell-type predicates, collects only after successful
complete casting/payment, and retains frozen typed cast witnesses. Snapshot/bundle
stay 19/20; old catalog/history references are unchanged. Hosting/search exclude
the new capability. Implementer checks are focused only; the parent owns final
acceptance. See `docs/game-engine-cards-as-data-d2c-c.md` for exact scope, source
witnesses, coverage and retained development evidence.

D2c-d ordinary per-creature declared-attack triggers passed combined acceptance
and independent parent audit at `394301d` with reviewed demo/Bundle lifetimes.
The gate passed 1,040 Rust/21 ignored,67 Node+26 cheap,195 release,47 SDK/strictTS,
540 trace executions and216 games/216 explicit replays; coverage 1,985/32,116,+17/-0.
IR25 and subsequent digest reuse are excluded. IR24 adds
`AttackTriggerV1`, exact-source/compositional attacker predicates, and frozen typed
attacker/defender witnesses under the actual declaration batch occurrence.
Snapshot/bundle stay 19/20 and public versions stay 16/16/15. Hosting/search exclude
the new capability. No parser or old golden changes are part of this increment.
See `docs/game-engine-cards-as-data-d2c-d.md` for exact accepted evidence and scope.
