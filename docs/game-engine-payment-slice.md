# Casting-time intrinsic mana

Scope frozen October 13, 2026. Extend the existing three exact admitted
configurations and five-definition catalog with a caster-only mana window
between target announcement and payment. Add one exact four-card reversal
profile from the same catalog: card 0 is
P0's Forest instead of Island, card 3 is another P0 Forest on the battlefield;
Unsummon and borrowed Bears retain cards 1/2. This witnesses an unpayable U
cost after floating and producing G without altering source definitions.
No new source cards, AST/IR vocabulary, general cost families, phase progression, or Commander match admission are added.
This is a bounded P3 slice, not complete P3 signoff.

Read CR 117.3c/4, 601.2g–i, 605.3a–c, 733.1–2, and 800.4a in the pinned
September 25 rules:
https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt.
Always offer the window, including when floated mana already suffices. The
caster can activate a controlled, untapped intrinsic basic-land mana ability;
it resolves immediately through the existing typed tap/add event batch and
never grants response priority. FinishMana ends the window, then an automatic
payment step pays the complete frozen supported mana cost or reverses the cast.
Finishing an affordable cast cannot cancel it. Pass, new casts, target changes,
opponent actions, and mana activation during payment/resolution reject.

FinishMana carries an `undo_mana` preference for an unpayable attempt: keep all
casting-time activations or undo all. Choosing individual subsets is outside
this slice. Record ordered activation receipts with source incarnation, player,
color, exact amount and whether that pool entry existed before activation.
Reversal subtracts only produced mana in reverse order and untaps surviving
same-incarnation sources through typed reversal events. Restore the original
spell incarnation, not a new ordinary zone move. Preserve pre-cast floated
mana, input history, visibility history, counters, traces, and concessions. A
source removed by a departure cannot be resurrected; its mana can still be
removed from a surviving payer. Never mutate departed-player last-known facts.
If an external departure invalidates the unfinished cast's targets, perform
restricted automatic undo-all reversal. If the caster leaves, discard its
casting frame and retain the departure effects. A kept legal mana activation
clears the old pass round (CR 117.4); an attempt with no retained activations
restores prior passes filtered to living seats. The caster retains priority
after completion or reversal (CR 601.2i / 733.2).

Snapshot/bundle v6 retain the new frame, preference, and activation receipts;
Observation/SearchContext v4 expose only caster request token/player/cost and
normal public tapped state/mana. Query sidecars use v3. IR stays v2. Existing
historical witnesses retain their flat gameplay event sequences but now include
an explicit decline of casting-time mana activation. Historical v5 evidence
remains labeled historical; new v6 expected fixtures own current state hashes.

The production witness proposes P0's Unsummon before tapping Island, targets
the borrowed Bears, taps Island in the mana window, finishes payment, and uses
four live passes to resolve. The final creature belongs in P1's hand and the
spell in P0's graveyard, Island tapped, U zero, no randomness consumed. Retain
this witness, failed-payment keep and undo witnesses, and the previous four
execution families in all three trace modes. Save/restore every accepted-input and rule
boundary, budget execution/query equivalence, receipt invalidation, hidden
opponent hands, and replay/offline source closure are required evidence.

Negative/interaction tests include insufficient mana with keep/all choices,
pre-cast mana preservation and zero-entry shape, extra legal mana activation
when already funded, wrong timing/seat/tapped/stale source, no voluntary valid
cast cancellation, previous passes, lower stack entry, caster/target/unrelated
source departure, and rehashed forged receipts/preferences. Private synthetic
additional-source/control fixtures test unsupported compositions without
broadening production admission. No product decision blocks implementation.

Control-changing abilities, mana dependencies, subset reversal choices, and
source exile from ending control effects remain unsupported compositions. A
private surviving-owner source that changes incarnation during departure faults
on attempted undo; no admitted profile can reach this case.

Status: implemented and verified with 73 Rust tests, 5 Node checks, independent
review, and an offline rebuild/replay of 21 trace-mode executions. See
[implementation evidence](game-engine-progress.md).
