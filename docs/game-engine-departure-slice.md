# Commander state and multiplayer departure

The follow-up [event contract](game-engine-event-slice.md) advances snapshots
and bundles to version 4, preserving the departure behavior and flat trace.

Scope frozen October 13 2026. Extend the existing scenario kernel with physical
commander identity/history, original seating and live-player state, explicit
default/control-effect provenance, and out-of-band concessions. Legal Commander
matches, command-zone casting/return rules, combat, full phases, and general
continuous effects remain later rules slices.

## Source and admission

Add the exact schema-3 Isamaru record to the reviewed pool: Oracle identity
`cfb756dd-6fe1-430a-8f04-8b10cac62a86`, source SHA-256
`b71e7507f77abada88bb3512955ed9ca8fd5a3ff395ec8f1ff915940166b9602`.
Its complete frozen source is a normal legendary creature with empty Oracle text,
constant characteristics, and pregame color identity. Rust IR version 2 preserves
the legendary characteristic. This fixture supports a second exact four-seat
scenario with one designated physical card in P0's command zone. Eligibility is
limited to this reviewed legendary-creature case; other commander permissions,
multiple commanders, construction, and banned-list validation remain unsupported.

Keep designation separate from object characteristics. Cast history is keyed by
physical commander and casting player; combat-damage history is keyed by physical
commander and recipient, using exact quantities. Tax derives from completed
command-zone casts. Noncombat damage does not add to combat history. Zone/control
changes and player departure do not erase the ledger. The admitted scenarios do
not yet cast commanders or deal combat damage: domain tests exercise these ledger
contracts without exposing an authoritative mutation bypass.

## Departure semantics

Read the pinned September 25 rules: CR 104.2a, 104.3a, 110.2, 117.3–4,
800.4a–k, and 903.3/8/10 at
https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt.

The borrowed Bears enter the fixture with P2 as their **default controller**.
There is no initial control-changing effect. If P1 leaves, the owned creature
leaves the game. If P2 leaves, it remains P1's card but is exiled. Paired private
kernel fixtures instead supply explicit control grants, proving that ending a
grant can restore the default or another remaining grant. Ownership alone never
determines this controller restoration. Static-source and persistent grants have
distinct provenance; these tests do not admit general control-changing cards.

Concessions queue at the current deterministic rule-step boundary, record their
ordering relative to accepted decision inputs, and invalidate work/receipts.
Before ordinary automatic work continues, apply each departure through one
atomic rule step: owned objects leave, relevant grants end, remaining controlled
objects are exiled, and affected stack/casting frames are removed or reversed.
No new player query/decision is exposed while departure work is pending.
`ReplayJournal` retains both decision records and concessions; `Game::concede`
is the trusted host ingress, and `Advance::Finished` reports the surviving winner.
Player observations/context version 2 expose status, original turn owner, live
seat order, and a public commander designation handle separate from physical IDs.

Keep original seating, physical cards, last-known player facts, and history.
Priority follows surviving seats. Preserve surviving players' prior passes: a
concession by the last unpassed player can allow resolution without a redundant
new pass. If the active player leaves, the existing turn continues with no active
player; record its original turn owner instead of starting a new turn. The final
remaining opponent wins immediately under CR 104.2a. Ordered concessions cannot
be turned into a simultaneous draw or allow another concession after that win.

## Evidence and limits

Replay combines accepted decision-stage inputs and boundary-ordered external
records. Snapshot version 3 preserves queued departures and exact execution
position. Restore/replay must agree across budgets and concessions during target
announcement, payment, priority passes, and resolution. Verify departed-player
references, source versus target departure, control restoration versus exile,
active-player absence, shrinking from four players to two without reinitializing,
winner timing, stale work, physical commander persistence, and tampered snapshots.
Retain the existing P1 seven-input/thirteen-event trace as an unchanged gameplay
reference, with a separate version-3 state hash.

Token/noncard stack objects, departed-player resolution-time choice reassignment,
control durations, commander movement choices, combat loss SBAs, and complete
phase progression require their own source/runtime fixtures before admission.
Those paths must reject or fault rather than guess. No product decision blocks
this bounded slice.

Status: implemented and verified for these two closed profiles. Production state
retains the exact ledger; combat-damage accumulation is a private domain fixture
until a real damage interpreter exists. The native demo carries thirteen exact
departure checkpoints and a mixed-input replay journal, alongside the unchanged
P1 gameplay trace. See [implementation evidence](game-engine-progress.md).
