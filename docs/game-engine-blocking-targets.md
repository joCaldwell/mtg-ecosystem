# Blocking-creature spell targets

The current schema-1 IR supports `BlockingBattlefieldCreature` with the separate
`BlockingCreatureTargetV1` capability. This is a finite root spell-pump domain:
ordinary instant/sorcery instructions modify fixed P/T until end of turn. Other
target predicates, opcodes, additional sacrifice costs, modal programs, activated
abilities and triggers cannot silently inherit this domain.

## Complete source

The frozen [Righteousness record](../packages/game-engine/tests/fixtures/righteousness.json)
is Oracle identity `3f6b2f76-0364-415e-a6a2-e9e5bf31b745`, archive index 9534:
an instant costing `{W}` with “Target blocking creature gets +7/+7 until end of
turn.” Its complete source, AST, binding and provenance are consumed; no card
definition is edited to manufacture this behavior.

| Pin | SHA-256 |
| --- | --- |
| Source | `f9af121b8a7525a90421c871f441e59b9449d1509a571e54b59656aca48f8177` |
| AST | `39040712bbdeb4100fbbe67e4010a216db8a3cb431bd754c078285ab3167932f` |
| Binding | `eba91cf74d2af16085aab09b7a771235dede6bc6cebed8f34c3c6e05696e0351` |

The dedicated pump-target binder consumes the exact blocking-creature selection.
Other creature binders retain their original strict filter. Mixed domains or a
reused selection identity cannot discard the restriction. Capability closure and
current schema constraints reject unsupported contexts; there is no version gate
or downgrade matrix. General slot binding remains a Rust validation obligation.

Righteousness remains outside the existing reviewed catalog pools. Source
coverage classifies it as lowered but unadmitted. Hosting/search capability sets
and profile decks do not expand with compiler support.

## Status, casting and resolution

The rules reference is the pinned
[2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt),
SHA-256 `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.

- CR 509.1g: committed blocker declarations populate an exact-incarnation
  `blocking_creatures` set. Removing the attacker preserves its blockers' status.
  Attacker/blocker links continue to determine damage recipients; orphan blockers
  assign no damage under CR 510.1d.
- CR 506.4: that blocker leaving combat removes its own membership, including on
  zone/control changes or departure. Tapping or later restrictions alone do not
  remove it. First-strike eligibility scans current attackers and all blockers.
- CR 511.3: membership lasts through EndCombat priority and clears atomically as
  that step ends, before the separately budgeted postcombat entry unit.
- CR 601.2c/e and 601.6: choose a legal target during proposal. Once proposal is
  complete, losing target legality during mana/payment does not reverse casting.
  Exact bindings, source-departure checks and cost guards remain enforced.
- CR 608.2b and 514.2: recheck current blocking/targetability at resolution. An
  illegal sole target fizzles after payment. A resolved boost stays on that exact
  incarnation after blocking ends and expires with ordinary atomic cleanup.

Blocking membership is trusted rules state, with structural invariants and
accepted-input replay proving its origin. It is not a new public object field.
Existing permitted target candidates and action discovery use the same legality
predicate without mutating state or minting handles.

## Focused evidence and limits

Runtime source checkpoint `66aef199` includes the payment correction. Its copied
debug executables identify core build
`8dabf0e48d8a224d610b4728d246baf51021966a2200a270fa24380e578c2344`.
Raw receipts are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/blocking-target-*`.

Compiler/contracts checks passed 128/25 tests; current schema/catalog checks
passed 41 Node cases. Eight new runtime witnesses cover ordinary casting and
cleanup, orphan blockers, privacy/atomic rejection, action discovery, target loss,
snapshot forgery and payment-time departure. Ordinary witnesses prove
restore/replay; one explicitly injected control-grant kernel probe makes no
ordinary replay claim. Final nine-case validation includes the revised legacy
departure test: proposal boundaries reverse, payment boundaries pay and fizzle.
Adjacent combat, turn, departure, query and capability checks were exercised in
the retained focused runs; this is not a full runtime acceptance claim.

The three affected legacy families were run against both the retained pre-change
executable and the new executable: 94 scenarios. Every report field except seven
final State hashes is identical, including events, choices, accepted journals and
final player observations. Those seven cases retain surviving blockers at their
stop boundary. Their current hash fixture was updated only after source/rules
review and exact comparison; old values and full reports remain retained in
`blocking-target-scenario-equivalence.json` and adjacent evidence.

Production focused Clippy/builds, generated SDK strict TypeScript and 54 tests,
and the small smoke plus explicit replay passed. Full runtime suites, full demo,
legal-pod/profile or release-test matrices, live hosting/browser acceptance and
offline retention rebuilds were omitted. New pool admission and hosting/search
review remain separate work.
