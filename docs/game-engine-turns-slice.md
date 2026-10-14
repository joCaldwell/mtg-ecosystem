# Turns, land plays, and targetless creatures

Scope frozen October 13, 2026, before implementation. This bounded P3 slice adds
one exact eight-card scenario, not legal Commander matches or general combat.
The parent agent owns integration; separate agents own core implementation,
compiler/contracts, and independent contract/test/review work without overlapping
edits. The existing four profiles retain their deliberately unsupported
empty-stack progression boundary; only the new profile enables the scheduler.

## Sources and admission

Reuse all five exact reviewed source definitions and the paired Cruel Edict
rejection. No parser changes or new source records. Forest is Oracle identity
`b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6`; Grizzly Bears is
`14c8f55d-d177-4c25-a931-ebeb9e6062a0`, source SHA-256
`0c19e173c1697928e3bb8c46cc6fde36912566e736c8fbfa9c6ee8fc2484743e`.
The fixture supplies empty abilities, constant 2/2 characteristics and `{1}{G}`.
The official [September 25 rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
were downloaded and verified against pinned SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Retain those bytes in the offline closure. Rules: 106.4, 117.3–5, 302.1–2,
305.1–3/6, 400.7, 500–504, 505, 506.1, 508.1/8, 511–514,
601.2, 605.3, 608.3a, 733, and 800.4a/j.

IR v3 explicitly describes spell timing (instant or main-phase), resolution
(instructions or permanent), land-play and creature-cast capabilities. Empty
creature programs are targetless permanent spells; empty instant programs still
reject. No card-name runtime dispatch. Compiler success never expands admission:
only exact reviewed definition digests and five complete configurations pass.
Creature casting/land plays are enabled only in the new profile, which contains
Forests and one Bears; this avoids ambiguous generic-mana allocation and legends.

## Initial configuration and witness

`ScenarioConfig::turns_permanents()`, profile `four-seat-turns-permanents-v1`:
four seats at 40 life, multiplayer attack-multiple-players option fixed by this
profile (CR 507.1/802, so no single defending-player choice), no commanders,
P0 active/priority, turn 1 postcombat main,
zero land plays, empty stack/pools/history. Every object starts incarnation 0,
untapped, owned and controlled by its stated player. Physical cards:

- 0: P0 Forest, battlefield.
- 1: P0 Forest, hand.
- 2: P0 Grizzly Bears, hand.
- 3: P0 Forest, hand.
- 4: P0 Forest, library (sole card).
- 5/6/7: P1/P2/P3 Forest, their respective libraries (sole card each).

P0 plays 1 as a special action; it moves hand→battlefield with a new incarnation,
increments the land count, clears passes, and returns priority to P0 without the
stack. A second land now rejects. P0 casts 2, moving it to the stack; no target
request occurs. The existing caster-only mana window activates 0 and 1, then
FinishMana pays `{1}{G}` and completes casting. Four live passes resolve it to
the battlefield under the caster's control, with a new incarnation, untapped.
No graveyard move or target-fizzle rule applies to this targetless permanent.

All players pass each supported empty-stack priority boundary. Progress through
end/cleanup; P1, P2, P3 untap/upkeep/draw/precombat main/begin combat/forced-empty
declare attackers/end combat/postcombat main/end/cleanup; then P0
untap/upkeep/draw/precombat main. No blockers or damage steps occur when no
attackers are declared (CR 508.8). Each draw moves the sole library card to its
owner's hand. P0 plays 3; playing 4 then rejects. Stop the witness at this ordinary
priority boundary, without claiming game completion.

Final state: turn 5, P0 active/priority in precombat main, one land played this
turn; 0/1/3 Forests and 2 Bears on P0 battlefield, untapped; 4 in P0 hand, 5/6/7
in their owners' hands; empty libraries/stack, empty mana pools, unchanged life,
no departed players, and no random operations. Bears incarnation is 2; played
lands 1/3 and drawn cards 4–7 have incarnation 1; card0 remains incarnation 0.

## Timing, continuation, and events

Store explicit phase/step and turn/land-count facts as serializable semantic
state, expose them in observations/history/SearchContext, and version snapshots
and replay bundles to v7, projections to v5, query sidecars to v4. Land play and
creature casting require the live active player holding priority in either main
phase with an empty stack; land play additionally requires an unused land play.
Intrinsic mana and instant timing retain existing semantics in earlier profiles.

Empty-stack all-live passes schedule a transition; a nonempty stack resolves
exactly its top. Clear pools at every ending step/phase through typed events.
Untap is automatic and simultaneous for tapped controlled permanents, no priority.
Draw is an automatic typed library→hand move before priority. Cleanup checks
its restricted domain and advances automatically, without priority. No damage,
temporary effects or triggers are reachable. Land-count resets and turn/step
changes have typed events/outcomes; structural counters use checked arithmetic.
All automatic steps are budgeted and snapshotable, including entry work before
priority. Batch commits remain atomic and queries never advance work.

Departures drain before ordinary work, preserving the merged ordering fixes.
A departed active player's current turn continues with no active player;
skip its player-specific untap/draw/cleanup work and grant priority to the next
living seat where required. Only cleanup advances to the next live turn owner.
Filter pass rounds and preserve valid accepted inputs; invalidate illegal pending
land/cast actions through the existing departure revalidation path. Departure
never resets a surviving active player's land count or restores consumed mana.

## Explicit limits, negatives, and recovery

Combat declaration is supported only when the active player controls no
creatures (or no active player remains). Otherwise fault explicitly before
claiming a declaration; creature combat, including summoning-sickness eligibility,
is outside this slice. Empty-library draw, multi-card library ordering, and
cleanup hands over seven fault explicitly. Never skip draw/discard/combat or
substitute a game result. Attack choices, damage, triggers, entry effects,
additional/alternative costs, nonbasic lands, legendary casting, other casting
zones, Commander tax/return, setup/shuffle and arbitrary configurations reject.
A scenario execution may reach an explicit unsupported boundary; admission is
bounded-scenario support, not a closed complete-game claim.

Test active/nonactive timing, both main phases, stack nonemptiness, one land per
turn and reset, stale tokens/handles/receipts, unpayable creature reversal,
payment and permanent resolution, all-live passes, departures at every scheduler
and casting boundary, and queued departure ordering. Keep the three recently
merged regressions intact. Test atomic event failures, budget zero/single/larger
partitions, incomplete/recovered queries, fork independence, trace equivalence,
forged snapshot rejection, every witness input/rule-boundary restore, and replay.
Retain the witness in off/compact/detailed modes alongside all existing families.
Run package checks, debug/release tests, and fresh offline retained-source
rebuild/replay; update maintained evidence and commit locally without merging.

Status: implemented and independently reviewed; 101 Rust tests pass in debug and
release, 6 Node checks pass, and all 24 trace-mode runs rebuild/replay offline.
See [maintained evidence](game-engine-progress.md#explicit-turns-land-plays-and-creatures--october-13-2026).
