# Grouped creature damage and opponent casting prohibition

The unreleased schema remains **33**. `GroupedCreatureDamageTriggerV1` and
`OpponentCastProhibitionV1` complete the retained **Kutzil, Malamet Exemplar**
source, Oracle ID `3233af43-7826-4895-b67e-03c6102c2cd5`: a legendary 3/3 Cat
Warrior costing `{1}{G}{W}`. Its source SHA-256 is
`f9215334c5e84315012376dc5f75ef0e56f6b16579452f15b9b7408ad5d038f1`,
AST SHA-256 `2660fb5ec4d5a3b24462a2b08a5c00303b99560dacca0fd20b091d4aa27db95f`.
The exact source/AST/diagnostic binding remains unchanged in `cards-data-d2ce/`.
The archived graph is nonexecutive: its unresolved action scopes and incorrect
source-level binding of “it” remain evidence. Strict Rust lowering independently
consumes the complete casting prohibition and binds base power to each candidate
damager. Qualified or partial action/property/grouping shapes reject.

The grouped condition admits controlled creatures whose current power exceeds
their own base power, combat damage to a player, and one mandatory controller
draw. It groups qualifying positive actual damage by observer, ability, genuine
damage occurrence and damaged player. Two qualifying creatures hitting one player
draw once; hitting two players draws twice. First/double-strike damage steps are
separate occurrences. Prevention, assignment amounts and life-total changes do
not substitute for actual damage.

Ordinary trigger-time battlefield appearances determine controller, creature
membership and power. Prepared damage-source facts retain their separate
assignment-time meaning. The admitted P/T families are additive modifiers and
counters, so immutable definition power is base power; setting, CDA, copying and
swapping families require extending that evaluator before admission. An absent
damager cannot qualify using old assignment facts. Each collected group freezes
its complete canonical member set and appearances. Later departure or loss of
the boost does not cancel it. Existing bounded token identities retain every
member's definition after token cessation. Witness payloads use existing
retention charging; no capacity ceiling increases.

The casting prohibition follows the source's current battlefield controller and
the retained turn owner, including a turn continuing without an active player.
It applies to opposing spell casts from all admitted zones regardless of timing
permission. Lands, mana/activated abilities, triggers and already-cast spells
remain governed by their ordinary rules. Checked public responses and execution
reject illegal proposals before mutation; departure revalidation before the
cost stage uses existing announcement rollback. CR 601.6 preserves a spell that
becomes prohibited during determination/payment of costs. The simulator policy observes the same lock
through public controllers, turn owner and catalog effects.

Rust contracts and generated schemas validate the new bounded shapes and required
capabilities in place. Old per-member damage witnesses retain their bytes.
Exact build identity fences recovery. Historical catalogs and profile digests
remain unchanged; these capabilities are outside reviewed hosting/search sets.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 101.2, 109.2/109.5, 208.4b/613.4, 510.2, 601.2e/601.3/601.6, 603.2c/g/603.10,
700.1 and 800.4j. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.
