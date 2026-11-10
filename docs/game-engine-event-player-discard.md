# Discard by a frozen damage recipient

IR28 adds `EventPlayerDiscardV1` and an optional recipient field on
`DiscardCards`. An omitted recipient retains the historical controller behavior
and serialization. `Controller` is explicit controller behavior; `EventPlayer`
means the player from a damage trigger's frozen actual damage witness. Only
one-card discards are supported. A new capability/version gate covers the event
binding, independently of labels and recursively through nested programs.

The complete source witness is **Dimir Cutpurse**, Oracle ID
`56c1a548-c80c-432b-bf92-d4ef6117b298`, retained in
`packages/game-engine/tests/fixtures/cards-data-d2ce/`. Its exact text is
“Whenever this creature deals combat damage to a player, that player discards
a card and you draw a card.” Archive line 13027 has source SHA-256
`a61836e5ea6cfde0bbfd7820718e67ca6e04eb0b31e84cd948a9304071a87bd3`.
The whole source lowers to the existing combat/player damage predicate, a
one-card event-player discard, then an ordinary controller draw. Its historical
unsupported disposition is superseded without changing frozen source records.

The damaged player chooses from their own hand through the existing private
`ResolveDiscard` request. The invocation controller does not choose for them
or receive their candidates/handles. Choice execution and snapshot validation
resolve the same frozen actor; neither current source control nor current combat
membership can rebind the damaged player. Empty hands and departed recipients
skip the discard and continue the controller's draw. The invocation controller
remains “you” after source/owner departure; their departure does not redirect a
personal draw. Commander discard uses ordinary graveyard movement and waits for
the entire program before the owner receives the graveyard return choice.

Event-player discard is admitted only in a damage trigger. Spells, activations,
step/cast triggers and unbound/qualified recipients reject that binding. Existing
event-player draw/life operations retain their step/cast-only restriction.
General controller-discard trigger lowering is not added by this increment.
No new public request or host/search admission is introduced. Older catalogs,
expected IR, format versions and complete-source hashes remain unchanged.

The retained 2026-09-25 Comprehensive Rules have SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
CR 603.2c/603.3a govern occurrences and trigger control; 608.2c preserves written
order; 701.9a defines discarding; 609.3 performs the possible parts of effects;
117.5/704.4 defer state-based actions during resolution; 903.9a governs the
subsequent Commander graveyard choice. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Source checkpoint `6b80321` passed formatting, workspace all-target compilation,
106 compiler and 17 contracts library tests, six new event-player discard tests
and six existing action tests. All 30 executable source/schema/catalog checks
passed. Accepted-input runtime checks cover public checked-response submission,
mid-choice recovery, written sequencing, player departures, Commander discard
ordering and coherent recipient forgery rejection. Historical discard bytes and
all prior source dispositions remain checked, with Cutpurse explicitly added.

Logs are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261110/`,
including an initial formatting-only module-order failure and its correction.
Full core/sim/server runtime suites, Clippy, SDK, release/profile gameplay
matrices, coverage measurements and retention rebuilds were not run. The schema
checks built the release CLI; no combined acceptance or performance claim follows.
