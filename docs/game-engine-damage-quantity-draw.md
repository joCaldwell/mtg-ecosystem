# Draw the frozen damage amount

IR32 adds `DamageQuantityDrawV1` and `DrawDamageAmount { source_block }`.
The instruction draws for the frozen invocation controller using the positive
actual amount from that invocation's `DamageDealt` witness. It never uses printed
power, assigned damage, life-total changes or a sum over other batch members.
Only damage-trigger programs admit the operation; spells, activations and other
trigger families reject it. Version guards inspect nested opcodes independently
of capability labels. Ordinary fixed `DrawCards` and optional-draw bytes and
semantics retain their historical versions.

The complete source witness is **Fear of Failed Tests**, Oracle ID
`34fd461b-a913-4a60-9477-ce2bd5c6fc5a`, retained unchanged in `cards-data-d2ce/`.
Its full type is Enchantment Creature — Nightmare, its power/toughness are 2/7,
and its only ability reads “Whenever this creature deals combat damage to a
player, draw that many cards.” Archive line 7911 has source SHA-256
`d839a89067547311d9d390e90fd7e7fc1c976adfdafb2f0098d32760f2f72c4d`.
The Rust semantic binder consumes the complete typed ability, rather than
executing the diagnostic reference graph. Source lowering admits the standalone
single-step mandatory controller draw in a damage trigger, so earlier quantity
producers cannot silently rebind “that many.” Optional, qualified, unbound,
non-damage and other dynamic amount forms reject.

Resolution performs at most one draw attempt per rules work unit. Optional
internal progress on `Resolving` retains exact `Quantity` completed, remaining
and failed counters, bound to the current cursor and source block. Their sum is
checked against the immutable damage witness; no machine-sized conversion or
artificial draw-count limit truncates the effect. A final `DamageDrawn` outcome
reports attempts and failures. A Commander replacement consumes one attempt,
then resumes the remaining sequence; it does not draw an extra card. Empty
libraries still complete the instructed attempts before state-based actions.
A departed controller's remaining personal actions stop without redirection;
source-owner departure does not change a surviving controller.

The progress field is omitted for historical unit instructions. Snapshot and
public format numbers remain unchanged; exact engine build identity rejects
snapshots from other source builds. Structural progress guards and accepted-input
replay both remain required for restore. No new player request, historical
catalog/profile rewrite, general draw replacement family or hosting/search
admission is introduced. Prior match evidence keeps its original identity.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 109.5/603.3a bind the trigger controller; 120.8/614.6/615.6 distinguish actual,
zero and prevented damage; 121.2 requires individual card draws; 121.4 and 704.4
place failed-draw loss after resolution; 903.9b governs the Commander hand
replacement. Instruction-wide count replacements in CR 121.2a remain outside
this finite family. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused validation passed formatting and dependency/structure guards, workspace
all-target compilation, 115 compiler tests, 21 contracts tests and 25 selected
core tests. The runtime selection executes all six new quantity-draw cases,
six optional-draw cases, five controller-discard cases, four profile cases and
four existing fixed/event-player/Commander draw cases. Actual new damage amounts
1, 2 and 4 are exercised; no source-backed huge-amount gameplay fixture was run.
Counter arithmetic uses exact `Quantity` operations without a machine-sized cast.

All 35 executable source/schema/catalog cases passed with no skips. The tiny
ordinary smoke and explicit saved-artifact replay passed at engine build
`d0da7181b93ac942cbdfe48fed5e49689d8d10979b978d76db83460eb2545304`,
reproducing final state
`b34e18dae91dbc123485bae4075754c0464e732ffb1296878d1d6a8a738751ac`.
This smoke exercises common engine recovery, not the new damage instruction;
its source-backed behavior is established by the selected runtime cases.

Logs and the smoke artifact are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/` as
`damage-quantity-focused-01.log`, `damage-quantity-schema-01.log`,
`damage-quantity-smoke-01.json`, `damage-quantity-smoke-01.log` and
`damage-quantity-smoke-replay-01.log`. Smoke timing receipts remain beneath
`generated/engine-check-timing/`. Full core/sim/server suites, a fresh complete
match, SDK, hosting/search, release/profile gameplay matrices and retention
rebuilds were not run. Focused validation does not renew combined acceptance.
