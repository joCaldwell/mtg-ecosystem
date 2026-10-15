# Command-zone casting and Commander return choices

Contract frozen October 15, 2026 after independent rules, source and runtime
review. This is a bounded continuation of the combat and complete-scenario
kernel. It does not admit arbitrary configurations, legal Commander matches,
general replacement effects, or Commander combat-damage losses.

## Source and exact admission

Reuse the existing five reviewed definitions and add only Plains from the exact
historical schema-3 archive. The extracted archive manifest hashes to
`b20406acec7b9af4610324af8848c76a2966315f1bb4043de2b03d53aa1a6f1b`,
matching the retained manifest. Plains has Oracle identity
`bc71ebf6-2056-41f7-be35-b2e5c34afa99`, source index 28278, source SHA-256
`2d754c3d2e6a5f780dac19f38fbe26e8631cfecdbe8a4372f095dbdfa308a1d5`,
and exact raw record SHA-256
`6b23cdb80b5897603e4bf4afc0d07c59e7b49c25672fc6c7fd6ba02855e5e6c0`.
The complete frozen source is a normal basic Plains with only its intrinsic
white mana ability. Its empty AST hash is
`cf1cbb66a638b4860a516671fb74850e6ccf787fe6c4c8d29e9c04efe880bd05`.
No parser rerun, current card refresh, raw-text execution fallback, or card-name
runtime dispatch is introduced. IR remains v3.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
whose pinned local bytes independently hash to
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules are 117.3–5, 400.7, 601.2a/f–i, 608.2, 614, 704.3–4,
733.1–2, 800.4 and 903.3/8/9/10.

Three new exact configurations use four seats, 40 life each, turn 1, P0 active
with precombat-main priority, empty stack and mana, no history or control grants,
and all initial incarnations 0. Objects are untapped and default-controlled by
their owners except the explicit borrowed commander below. Initial battlefield
creatures meet continuous-control eligibility. All three profiles use explicit
ordered libraries, ordinary combat and cleanup.
Each player's reachable mana remains monochromatic: P0 white, P1/P2/P3 blue.
Mixed-color configurations and arbitrary cards reject. Existing profiles retain
their original exact five-definition catalog and previously reviewed gameplay
behavior. The new profiles require the separate exact six-definition catalog.

### `command_casting()` / `four-seat-commander-casting-v1`

| Physical IDs | Owner | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Isamaru | command; designated commander |
| 1–5 | P0 | Plains | battlefield |
| 6 | P1 | Island | battlefield |
| 7 | P1 | Unsummon | hand |
| 8 | P2 | Island | battlefield |
| 9 | P3 | Island | battlefield |
| 10 | P1 | Island | library |
| 11 | P2 | Island | library |
| 12 | P3 | Island | library |
| 13 | P0 | Plains | library |

Each library has exactly the indicated one-card explicit order. This profile
proves command-zone casting, a successful first {W} payment, Unsummon owner
replacement choices, a second {2}{W} command-zone cast or a {W} hand cast, and
failed {2}{W} payment/reversal when the player supplies too little mana.
Witnesses may fork at a choice or carry the deterministic journal forward;
they must not seed cast history or inject mana outside the accepted action path.

### `commander_lifecycle()` / `four-seat-commander-lifecycle-v1`

Use the same objects and setup, except commander 0 starts on the battlefield,
and card 14 is P1's Grizzly Bears on the battlefield. Designation does not imply
that Isamaru has previously been cast from the command zone: initial tax is zero.
An ordinary attack by Isamaru blocked by Bears deals simultaneous lethal damage
to both creatures. Both move to their owners' graveyards in the first SBA wave;
the next SBA check offers Isamaru's owner its command-zone return. Returning and
subsequent casting uses the original physical identity and a new incarnation.

### `commander_exile()` / `four-seat-commander-exile-v1`

Use the lifecycle configuration, except Isamaru 0 has P1 as its default
controller while P0 remains its owner. There is no control-changing effect to
end. When P1 concedes, P1-owned cards leave and surviving P0's commander is
exiled under CR 800.4a. The next SBA check offers living owner P0 a fresh
command-zone return. Acceptance and decline are separate source-to-runtime
witnesses; a later P0 departure removes the card rather than resurrecting it.

Commander-to-player combat damage is a known unsupported boundary in these
profiles: it must fault explicitly before committing such damage. Ordinary
noncommander combat remains supported. Selecting an unblocked commander attack
must never silently omit CR 903.10 history or claim a full Commander result.
Library replacement is deferred: no admitted effect moves a commander to a
library. Any newly reachable commander-library movement must reject or fault
until an explicit replacement-choice contract exists.

## Casting and payment

CR 903.8 grants a player permission to cast their own designated commander from
the command zone. It does not override ordinary creature-spell timing. Require
active-player main-phase priority and an empty stack. Hand casting continues to
use the existing permission; designation alone grants no exile or graveyard
casting permission, and no nonowner may cast the commander from command.

At proposal preserve the actual original zone and incarnation. Freeze total
cost from the base cost plus twice the physical commander's prior completed
command-zone casts by this caster. Use exact quantities and the existing mana
window/payment event path. No reducer or alternate-cost vocabulary is added.
Always expose the casting mana window, even if already funded. A payable cast
cannot be canceled through `FinishMana`.

Only completing casting increments command history. A proposal, failed payment,
reversal, resolution, return to command, and hand cast do not increment it.
History persists across zone changes, new incarnations and departures, remaining
keyed by physical commander and caster. Casts cost {W}, {2}{W}, {4}{W}, and so
on. No unrelated commander or caster's history contributes.

CR 733.1 reversal restores the original zone and original incarnation, including
command-origin attempts. It does not create a new commander return choice.
Preserve the existing keep-all/undo-all casting-mana distinction, pre-cast mana,
surviving passes and departures. Reversal cannot resurrect a removed caster or
its removed sources. Saved announcement handles remain historical references.

## Return choices and stabilization

Expose the owner decision through the existing player-facing request/response
and budgeted query machinery. The response is
`CommanderReturn { to_command: bool }`; the request identifies the owner,
current object and proposed destination through ordinary player-facing handles.
Only the living owner may answer. Raw physical IDs and trusted snapshot state
remain host interfaces. Invalid, stale or unauthorized responses reject without
mutating semantic state.

CR 903.9b is a replacement choice before an Unsummon instruction moves the
commander to its owner's hand. Acceptance moves it directly to command; decline
moves it to hand. Never first commit the hand move and then return it to command.
Preserve the detached resolving frame, instruction cursor, spell-wide target
legality, consumed source facts and actual destination outcome while paused.
Resume the rest of the spell exactly once. The caster does not answer for an
opponent commander owner. No player receives priority during this choice.

CR 903.9a is different: a commander actually enters graveyard or exile first,
then its living owner may return it at the next SBA check. Track eligibility
for that specific new incarnation since the previous check. Declining consumes
the opportunity for that incarnation; later checks do not offer it repeatedly.
A later fresh entry creates a new opportunity. Initial historical placement is
not a fresh entry. Lethal destruction and command return occupy distinct SBA
waves; no return choice occurs during combat damage or between resolving
instructions.

Preserve simultaneous SBA semantics and budgeted serializable continuation.
Freeze loss/death candidates before committing each wave; never select a winner
while simultaneous losers remain. Pending concessions drain before ordinary
work or exposing another query. Revalidate a suspended choice after departure:
owned objects leave with their owner, never resurrect, and removed targets cannot
be moved again. Exiling a surviving owner's commander because its default
controller left creates a fresh exile return opportunity. Preserve physical
history while removing obsolete incarnation-local marks and combat membership.

## Required evidence and explicit limits

Verify successful command casts and exact tax, ordinary timing and ownership
rejection, hand casts without tax/history increment, command-origin insufficient
payment with both keep/undo behavior, and repeated failed attempts without tax
growth. Verify public command handles and stale incarnations, owner versus caster
authorization, replacement accept/decline, resolution continuation exactly once,
grave/exile accept/decline freshness, departure during each choice, and commander
combat damage failing explicitly before mutation.

For each suspended choice and automatic/payment boundary, compare uninterrupted,
budget-partitioned, clone/forked, restored and journal-replayed state, observations,
queries, actual events and semantic hash. Prove independently restored snapshots
by replay; reject rehashed forged costs, origins, histories, eligible incarnations,
choices and continuation cursors. Trace off/compact/detailed modes must agree
semantically. Retain exact-source/IR fixtures, schema/readback checks, paired
unsupported compiler inputs, debug and release Rust checks, Clippy/formatting,
cross-language fixture contracts, runnable demo and fresh offline rebuild/replay.

Recovery formats advance to snapshot v9, bundle v10, player projection/context
v7 and query sidecar v6. IR remains v3. Historical fixture semantics and exact
five-definition catalogs remain unchanged; regenerated recovery identities are
versioned separately from those gameplay references.

This delivery does not add partner/multiple-commander permissions, general
replacement ordering, arbitrary exile/graveyard casts, legend-rule choices,
Commander damage losses, deck construction, shuffled libraries, general triggers,
or legal Commander match admission. These require separate frozen contracts.
