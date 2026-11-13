# Engine runtime invariants

These are current development constraints linked from the engine manual. Read
the sections affected by your edit and the linked detailed contract. Historical
profile descriptions record earlier releases; they do not choose runtime rules
or capacities. [Current scope and evidence](game-engine-current.md) and the
[capacity policy](game-engine-capacity-policy.md) supersede those old selectors.

## Source, identity and execution boundaries

- Contracts' Rust types own executable IR. Source archive/AST, IR, snapshots,
  bundles and public projections have separate version domains. Generated schema
  output is ignored. Compile the complete exact source or reject it; never drop
  unknown semantics. Admission checks build/profile and whole-pod configuration
  separately from lowering. Test positive source-to-runtime behavior and paired
  unsupported/invalid cases using exact source fixtures and pinned rules.
- Core's runtime imports contracts only; compiler-backed tests are development-only.
  Network, filesystem, clocks, OS randomness,
  asynchronous work and agent inference belong outside it. No unsafe code.
  Shared immutable catalogs and owned match tables are the baseline.
- Keep physical IDs, zone incarnations, stack IDs, owners and controllers
  distinct. Use exact game quantities and checked structural arithmetic.
  Default control differs from ownership and explicit control grants.
- Frames, inputs, events and outcomes serialize as data. Restore/replay preserves
  behavior; cloned branches are independent. Use explicit `Budget` with `advance`.
  Budget truncation and typed capacity faults never substitute for rules outcomes.
  Bounds are common resource policy, not card/catalog/profile branches.

See [catalog identities](game-engine-catalogs.md#engine-code-identity),
[profile admission](game-engine-profiles.md) and [capacity policy](game-engine-capacity-policy.md).

## Player visibility and query recovery

Player-facing code consumes `Observation`, `PlayerRequest`, `PlayerResponse` and
budgeted query outcomes. Raw IDs, `decision_for`, snapshots and full-state getters
are trusted host APIs. Never send query recovery sidecars or validation receipts
to clients. Queries cannot mutate semantic state, mint handles or add observation
history. Recovered query work remains executor/state scoped; stale work cannot
authorize choices. Public revisions change only with permitted information.
Policies consume public observations/requests; host checkpoint/replay verification
is separate. See [P2 handles and work recovery](game-engine-p2-slice.md).

Search captures contain only permitted facts/history. Sampled hands/libraries and
independent future randomness construct hypothetical games constrained by known
cards. Sample restore proves its origin; real restore never accepts a sample
origin. Fresh-root requirements and supported living-seat counts remain explicit.
New IR fields require reviewing capability requirements and hosting/search sets;
Rust enum exhaustiveness alone does not cover added struct fields. See
[search](game-engine-search.md) and [profile capabilities](game-engine-profiles.md).

## Atomic events, diagnostics and departures

Batch mutation is private and atomic. Capture consumed pre-event facts and use
actual typed outcome slots. Simultaneous groups admit only independent read/write
footprints; unsupported general interactions must fault explicitly. Validate the
whole simultaneous damage group before writing life/history. Freeze all loss
candidates before departures and adjudication. Restore proves fault reachability
by replay. See [event groups](game-engine-event-slice.md).

`Game::with_trace` selects off, compact or detailed retention. Diagnostics never
enter semantic hashes or player projections. Native `events()` is populated only
in detailed mode; `trace()` is trusted evidence, not a client stream. Trace/budget
choices must preserve rules behavior.

Concessions are trusted external inputs queued at recorded rule boundaries.
Process them before ordinary work and expose no query while they are pending.
Preserve original seating, departed-player facts and physical Commander history.
All simultaneous losers are absent before cleanup/adjudication. A removed player
never regains last-known mana or resurrects objects. Source departure preserves
started supported resolution; owner departure never resurrects an object.
See [departures](game-engine-departure-slice.md).

## Announcement, priority and payment

Stack projections run bottom to top. Saved announcement handles are historical
references: they cannot rebind to a later incarnation or authorize a response.
Lower entries stay completed while the top is announced. Reversal restores prior
passes filtered by living players. Resolving frames own frozen instances and
continue supported choice-free effects after source departure (CR 608.2m).

Only the caster answers target and casting mana requests. No player receives
priority during announcement or resolution. Always offer casting-time mana even
when already funded. FinishMana attempts complete payment and cannot cancel a
payable spell. Undo subtracts only receipt-recorded casting-time mana and untaps
surviving sources; preserve pre-cast mana, prior combat taps and departures.
Retained legal mana activations clear prior passes. Exact mixed-color spending
requires an explicit choice; do not invent generic payment colors.

Land plays and ordinary creature casts require active-player main-phase priority
and an empty stack; supported flash modifies casting timing explicitly. Land
usage resets only on a new turn. Unsupported boundaries remain explicit faults.
See [stack](game-engine-stack-slice.md), [payment](game-engine-payment-slice.md),
[actions](game-engine-actions-payments.md) and [mana permanents](game-engine-mana-permanents.md).

## Combat, damage and state-based actions

Combat declarations, simultaneous damage and SBA waves use typed events.
Multiple blockers have no damage-assignment order under the pinned rules.
Preserve blocked status after blockers leave, and blocking status after attackers
leave. Blocking lasts through EndCombat priority and ends as combat ends
(CR 509.1g/511.3); damage links alone do not define it. Skip blockers/damage only
when no attackers were declared. Continuous-control eligibility is stamped at turn entry
and resets on incarnation/controller changes. First/double-strike participant
sets, trample amounts, prevention and dealt-damage quantities retain their
reviewed distinctions. Commander combat damage never derives from life changes.

Damage accumulates until cleanup; lethal damage uses ordinary destruction and
Commander-return paths. Noncombat damage retains source incarnation and frozen
controller after departure; it never increments Commander combat history.
Direct life changes are separate typed events. Run SBAs after the complete
program: an intermediate loss/gain must not cause an early loss. If every target
is illegal at resolution start, skip the complete program. Replacement ordering
and damage prevention stay limited to reviewed supported families.
See [combat](game-engine-combat-games.md), [combat keywords](game-engine-combat-keywords.md),
[damage/life](game-engine-damage-life.md), [effects](game-engine-effects.md) and
[wipes](game-engine-board-wipes.md).

## Commander movement and hidden libraries

A failed cast restores its original zone/incarnation. Only completion of a
command-origin cast advances physical, per-caster tax history. Owner return
decisions are resumable continuations, never priority windows. Hand return
replaces movement before it occurs; graveyard/exile return requires actual entry
and the next SBA check. Decline consumes that incarnation's opportunity. Drain
concessions and revalidate choices before exposing queries.

Drawing a tucked commander requires the second owner replacement (CR 903.9b).
Use an existing viewer-scoped public designation and optional object handle:
hidden-library draws use `None`, visible-zone choices use `Some(handle)`. Never
reveal library rows or mint hidden handles. Complete one draw attempt once and
preserve the next card when movement is replaced. Library-top events preserve
prior ordering and cannot compose with a conflicting order write.
Commander damage belongs to the physical source/recipient and survives zone and
controller changes. See [casting/returns](game-engine-commander-casting.md) and
[damage/library](game-engine-commander-damage-library.md).

## Abilities, targets and triggers

Activated abilities have their own stack identity and historical source handle;
their source card never moves to the stack. Sacrifice costs occur before priority
and use ordinary post-activation SBAs. Sacrifice/discard during resolution defers
SBAs until the complete invocation. The invocation controller remains the meaning
of “you” after departure; departed controllers' personal draw/discard is skipped,
never redirected. A borrowed source's owner leaving does not remove a surviving
activator's ability.

Stack targets use current viewer-scoped `StackHandle`s bound to distinct
`StackId`s. Physical source handles do not authorize stack targets. Exclude the
announcing invocation itself and all incomplete entries. Countering a spell
moves it to its owner's graveyard; countering an ability removes only the noncard
entry. Never refund costs or resolve the countered program. Commander grave
return waits for the whole counter invocation. Counter events own aggregate
membership/control-flow writes and cannot compose simultaneously.

Tap activation costs require an untapped source and continuous-control
eligibility, subject to supported haste. Mana-cost activations have their own
announcement, mana window and exact spending choice. Source movement or owner
departure preserves a surviving activator's frozen invocation. Failed payment
removes that proposal and undoes only its recorded mana activations if requested.
Targeted tap abilities choose targets before costs and have no mana window.
Entity targets distinguish players/creatures; player targets and opponent-only
targets retain their own domains. See [stack/costs](game-engine-stack-costs.md).

Triggers collect from actual typed occurrences, then wait through SBAs and
Commander choices before APNAP placement/targets. Freeze controller, event-player
and event-specific witnesses at the defined boundary. Intervening conditions
check at matching and resolution; source status uses exact-incarnation current
facts or departure-refreshed LKI bounded by live continuations, independent of
trace retention. Do not retain an unbounded permanent LKI archive. Cast triggers
collect only after successful complete casting/payment. Declared-attack triggers
use the real batch occurrence and frozen attacker/defender witnesses.
See [ordinary triggers](game-engine-triggers.md), [choices](game-engine-choices.md),
[step triggers](game-engine-cards-as-data-d2c-a.md), [source status](game-engine-cards-as-data-d2c-b.md),
[cast triggers](game-engine-cards-as-data-d2c-c.md) and [attack triggers](game-engine-cards-as-data-d2c-d.md).
Later [IR25 damage triggers](game-engine-cards-as-data-d2c-e.md) retain their
candidate qualification; none of these additions implicitly expands host/search.

## Hosting boundary

The host owns authentication, OS processes, HTTP, SQLite, clocks and resource
groups. Its generated SDK follows Rust DTO/schema authority, not handwritten
parallel public types. Never expose snapshots, seeds, receipts, raw IDs, internal
traces, sidecars, other seats' histories or global private counters. Bearer
capabilities stay in authorization headers, not URLs/logs. Derive the viewer from
authentication. Resource/clock/fault statuses remain distinct from Magic outcomes.
See the [server manual](../packages/game-server/AGENTS.md) before host edits.
