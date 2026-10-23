# Compiler and engine development

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
`combat-keywords-match-batch OUTPUT` and `combat-keywords-match-replay FILE`.

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
- `cargo run --locked -p mtg-sim -- demo`: verified source-to-runtime demonstration.
- `cargo run --locked -p mtg-sim -- schema`: export the executable JSON Schema.
- `npm run retain:engine`: retain source/dependency closure and verify an offline
  rebuild/replay with a fresh Cargo home and target directory.

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
Legal match setup is available only through the three named foundations, keywords and combat-keywords
profiles. Ordinary combat is enabled in the reviewed combat, Commander and action profiles.

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
toughness and all-player loss/draw remain private domain fixtures: neither is
reachable from this positive-life, positive-toughness admitted pool.

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
