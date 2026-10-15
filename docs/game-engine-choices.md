# Modes, target occurrences and intervening conditions

This slice extends the reviewed effects kernel with complete Healing Salve,
Seeds of Strength and Bouncer programs. The implementation and independent offline verification are complete;
completed evidence and retained identities are recorded in
[the progress ledger](game-engine-progress.md).

The exact IR v10 catalog contains 33 definitions and 39 source records. Its six
scenario configurations are `four-seat-choices-v1`,
`four-seat-choices-furnace-v1`, `four-seat-conditions-v1`,
`four-seat-conditions-nonempty-v1`, `four-seat-conditions-borrowed-v1` and
`four-seat-conditions-no-targets-v1`. These are injected scenarios, not legal
100-card Commander matches. Historical catalogs and semantic references remain
separate. Boros Charm and Lavaborn Muse are complete-card negative boundaries.
Healing Salve now lowers completely; its exclusion from historical catalogs is
an admission exclusion rather than an unsupported lowering claim.

## Casting and resolution

`SpellProgram.modes` contains complete source-ordered child programs. The admitted
modal family chooses exactly one nonnested mode before targets and payment.
Healing Salve chooses either a player gaining three life or prevention of the
next three damage to a creature or player this turn. A mode has its own target
domain and complete instructions. The selected zero-based source index stays
on the invocation and any long-lived effect origin. Every branch must lower;
no unsupported branch may disappear. Earlier IR versions reject the additions.

Seeds of Strength has three separate target occurrences. Each occurrence gets
its own chosen target and +1/+1 effect; the same creature may be chosen more
than once. Resolution independently checks each target occurrence. Legal
occurrences still execute when another becomes illegal; if all targets are
illegal the whole invocation fails. A later incarnation cannot inherit an old
target or temporary modification. Cleanup and source/caster departures preserve
the reviewed temporary-effect rules.

Bouncer's controller-hand-empty intervening condition is checked against the
common pre-death observer facts when matching its own death. A failed first
check creates no pending trigger identity. A successful match freezes the
controller and source facts; the condition is checked once again when resolution
starts, before target legality. A failed second check skips the program. There
is no third condition check during placement or later continuation. Mandatory
creature targets, APNAP order, Stifle, missing targets and departing owners or
controllers use the ordinary trigger path. If no legal mandatory target exists,
placement removes that pending invocation without a hanging request.

These rules follow CR 115.3, 601.2, 603.3–603.4 and 608.2 in the pinned
[September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).

## Finite prevention allocation

A three-point shield cannot use a one-point greedy allocation interface: two
simultaneous three-damage sources must admit both the 1+2 and 2+1 prevention
splits. After selecting an eligible shield, `DamageAllocation` offers every
current original occurrence for that recipient. The response contains every
offered occurrence exactly once, including explicit zero entries. Each amount
is integral and between zero and that occurrence's current damage; the total
must equal the smaller of the remaining shield and offered current damage.

The whole allocation is validated before any application or budget update.
Typed allocation and application proofs bind original occurrence identity,
source attribution, current amounts, remaining budget and final simultaneous
damage. A provider departure cannot undo an already applied prefix. Physical
Commander history records only actual combat damage. Replacement/prevention
order remains the affected player or controller's choice under CR 615–616.

## Verification contract

Snapshot v16, bundle v17, projection/context v14 and query-sidecar v13 retain
modes, individual target cursors, condition proofs and allocation vectors.
Query recovery uses task-derived work counts, including every allocation entry,
instead of a fixed heuristic bound. Queries remain pure and cannot mint handles.

Thirty player-view policies cover repeated/distinct targets, partial and total
illegality, reincarnation, counters, departure, cleanup, both Salve modes,
residual and split shields, ordering with doubling, Bouncer's two condition
checks, intervening draw/cast, Stifle and unavailable targets. Independent
adversarial tests cover rollback, rehashed provenance, source/condition binding,
all integral allocations, query prefixes, stale handles and executor isolation.
The new suite adds 2,527 recovery boundaries, bringing combined evidence to
17,627 boundaries, 42 exact configurations and 540 trace executions across ten
catalogs. Historical comparisons normalize only outer projection versions.

The reviewed compact report has SHA-256
`bed59e69e22de194d43159be809ebbacee4731a4a5d6a52260bd161e8fcd85ed`.
Full debug, release, Node integration, retained-source offline rebuild and
independent replay gates must pass on identical frozen native source bytes
before this slice is marked complete.
