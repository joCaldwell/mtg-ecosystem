# Noncombat damage and direct life changes

Contract frozen October 15, 2026 after independent archived-source and rules
review. This bounded continuation adds Lightning Bolt, Prodigal Sorcerer,
Soul Feast, Healing Hands, Sacred Nectar and intrinsic red mana. It retains
all historical catalog/profile families independently. It does not admit general
triggers, damage replacement/prevention, infect, wither, lifelink, protection,
planeswalkers, battles, or arbitrary Commander matches.

## Frozen source and vocabulary

Read complete schema-3 records from the historical archive; do not refresh cards,
rerun the parser, dispatch by card name, or execute raw text at runtime.

| Card | Source index | Oracle identity | Source SHA-256 |
| --- | --- | --- | --- |
| Lightning Bolt | 10249 | `4457ed35-7c10-48c8-9776-456485fdf070` | `20ccb4e02f26dd5fd99c4a9d3616b7f9ef4b96442484808cc324a52d0735f18f` |
| Prodigal Sorcerer | 14170 | `5e961d15-5972-4e4b-9385-1cd7cd7c6bbe` | `18b86676187cd9d65e57f740d33eb4a2269f597798b3d01d3c1c0327ed49295b` |
| Soul Feast | 19440 | `8186fd80-015f-470c-9e1c-cbf45764a057` | `d2a826aa1a024ab009b2dddbab58f06563025d1ec6bdd5f91449cf0584bc9286` |
| Healing Hands | 9128 | `3cc48835-3ac0-4774-b380-f9b21d2dc974` | `5da894433f37708bc58ae017caa6c499a8c4b8299d90a675081962f4eb9642b7` |
| Sacred Nectar | 7234 | `30870ee5-6ad7-48a9-983e-d3b018f2344f` | `c93432aa9fe8536aadb22fb18a0af83365545b13428c3c1d99eb367b34956c07` |
| Mountain | 24682 | `a3fb7228-e76b-4e96-a40e-20b5fed75685` | `a36be15fb1c2c6f31dc840854997ae1867516db5dd49d311de6669cf4d2de3df` |

These six records are fully bound. Bolt is a {R} instant dealing 3 damage to one
any-target selection. Sorcerer is a {2}{U} 1/1 with a tap-cost ability dealing 1
damage to one any-target selection. Soul Feast is a {3}{B}{B} sorcery whose one
player target loses 4 life, followed by its controller gaining 4. Healing Hands
is a {2}{W} sorcery whose target player gains 4, followed by its controller drawing
one card. Sacred Nectar is a targetless {1}{W} sorcery gaining its controller 4.
Mountain supplies intrinsic red mana from its basic-land subtype.

Flame Rift (index 6036, oracle `28479aa8-d1cd-421a-8bbb-0594bb4dd410`, source
SHA-256 `c3f4f56bd5ad286b6d0ce69c41ca7dc14742db1e9ba5d08fa783f699db35a864`)
is not fully bound: its aggregate each-player scope stays unsupported. Retain it
and Blood Artist as paired negative records; neither enters the executable pool.

The new catalog contains exactly 21 definitions: the prior fifteen stack/cost
cards plus these six. Its 23 source records include the two negative records.
IR v7 adds `AnyDamageTarget`, `AnyPlayer`, `DealDamage`, `GainLife`, `LoseLife`
and `LifeRecipient::{Controller, Target}`. The complete any-target domain retains
creatures, players, planeswalkers and battles under CR 115.4; exact admission
supplies only creature permanents and players. Do not narrow the source meaning
or manufacture candidates of unsupported types. Existing Edict opponent targets
retain their separate domain; any-player targeting includes the caster.

Rules are the [official September 25, 2026 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf),
independently checked against the local pinned TXT SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules: 113.7a/8, 115.4/5, 119.2/3/6, 120.2–8, 302.6, 400.7,
601.2, 602.2/5a, 608.2b/c/h/m, 704.3–5, 800.4 and 903.9/10a.

## Exact admission and player API

Five exact profiles share the new catalog and explicit mana spending. Their
frozen configuration table follows below. Initial life is 40 except the named
low-life profile, which starts every seat at 4; life is never seeded as previous
damage or Commander history. Histories, stack and pools start empty, with P0
active in turn-1 precombat main, original four-seat order, no control grants,
incarnation 0, and continuous-control eligibility for initial creatures.

### `damage_life()`

| IDs | Owner/controller | Definition | Initial zone |
| --- | --- | --- | --- |
| 0 | P0 | Prodigal Sorcerer | battlefield |
| 1,2 | P0 | Swamp | battlefield |
| 3,7 | P0 | Mountain | battlefield |
| 4,8 | P0 | Plains | battlefield |
| 5,6 | P0 | Island | battlefield |
| 9 | P0 | Soul Feast | hand |
| 10 | P0 | Healing Hands | hand |
| 11 | P0 | Lightning Bolt | hand |
| 12,13 | P0 | Island | ordered library **12,13** |
| 14 | P1 | Isamaru | battlefield; designated Commander |
| 15 | P1 | Grizzly Bears | battlefield |
| 16 | P1 | Hapless Researcher | battlefield |
| 17 | P1 | Mountain | battlefield |
| 18,19 | P1 | Island | battlefield |
| 20 | P1 | Lightning Bolt | hand |
| 21 | P1 | Unsummon | hand |
| 22 | P1 | Counterspell | hand |
| 23 | P1 | Stifle | hand |
| 24,25 | P1 | Island | ordered library **24,25** |
| 26 | P2 | Prodigal Sorcerer | battlefield |
| 27 | P2 | Mountain | battlefield |
| 28 | P2 | Lightning Bolt | hand |
| 29,30 | P2 | Island | ordered library **29,30** |
| 31 | P3 | Island | battlefield |
| 32,33 | P3 | Island | ordered library **32,33** |
| 34 | P0 | Sacred Nectar | hand |

`damage_life_low()` changes only every initial life total to 4.
`damage_life_borrowed()` instead changes Sorcerer 0 to owner P2 while its default
controller remains P0. `damage_life_empty_library()` instead removes cards 12/13.
`damage_life_library()` instead changes Commander 14 to owner/controller P0,
removes Sacred Nectar 34, and adds Time Ebb 35 in P0's hand. This fifth exact
configuration allows P0 to tuck its own Commander, then cast Healing Hands and
offer the second mandatory owner replacement during its draw. All other facts
remain exactly the base setup. No seat exceeds sixteen owned cards.

`EntityTargets` combines creature object and player candidates in one selection.
Player responses use `PlayerTargetEntity::{Player { player }, Object { object }}`;
object entries require current viewer-scoped handles, never raw physical IDs,
stack handles, historical announced handles or hidden-library references.
Target-player life spells reuse `PlayerTargets` with the complete any-player
domain. Recovery versions are snapshot 13, bundle 14, projection/context 11 and
query sidecar 10. Retain selected entity kind and identity through announcement,
resolution, receipts, snapshots and replay; never reinterpret a player as a card.

## Damage and direct life semantics

Represent noncombat damage as its own typed event and actual outcome, retaining
source, recipient, amount and consumed pre-event facts. It is damage from the
frozen spell or ability source under CR 120.7, even if that source has left its
expected zone. An activated ability retains its historical source incarnation,
oracle facts, controller and characteristics without requiring a current
battlefield source at resolution. A bounced/recast source cannot replace that
historical identity. Current source references may be absent while the announced
historical reference remains. The source card never moves onto the ability stack.

For the admitted sources, damage to a player lowers life; damage to a creature
adds marked damage without changing toughness. Repeated damage accumulates until
cleanup. Lethal marked damage causes ordinary destruction in the next SBA wave,
with a fresh Commander graveyard-return opportunity if applicable. Damage does
not directly destroy a creature or move a Commander to command. Damage from
these spells/abilities is noncombat and never advances physical Commander combat
damage history, even if a Commander supplied the source in a private domain test.
Direct life gain/loss also never creates damage or Commander-history events.

Life events adjust the actual recipient total by the fixed amount, preserving
signed totals rather than clamping at zero. Apply each instruction in source
order. No SBA runs between Soul Feast's loss and gain: a self-target at life 4
reaches 0 and then returns to 4 before any loss check. When targeting another
player at 4, its controller gains 4 and the complete spell finishes before the
target loses and departure cleanup begins. Life gain is independent of how much
life the target had available. Sacred Nectar has no targets and must not fail
an empty-target legality test.

At resolution start, recheck all bound targets using CR 608.2b. If the sole
player target is gone, the complete Soul Feast or Healing Hands fails: do not
perform its later controller gain or draw. A stale creature incarnation cannot
rebind after bounce/recast. Once a legal invocation begins, retain its frozen
program if its source subsequently leaves. Skip personal life/draw actions for
a departed affected player; do not transfer them to another seat. A surviving
controller still performs later instructions when the original target leaves
during an already-started resolution.

## Targeted tap activation and draw closure

Sorcerer chooses targets before paying its tap cost. Check current control,
untapped state and continuous-control eligibility when paying the cost. A newly
cast or bounced/recast Sorcerer cannot activate immediately; an initial eligible
Sorcerer can. Reject invalid/stale/wrong-domain selections without tapping the
source, consuming priority or modifying pools. Tap-only activation introduces no
mana window. Complete cost payment and the noncard invocation before priority;
once complete, source departure, countering or target disappearance never
refunds the tap. Sorcerer may target itself or its own player. Its self-targeted
1 damage kills its 1/1 source only in the SBA following complete resolution.

The independent noncard invocation can be countered by Stifle, but not by
Counterspell; its target list survives source movement. Whole-program target
failure performs no damage. Lower completed stack entries and historical targets
retain their identities during this new announcement and its recovery paths.

Healing Hands draws through the existing ordered-library continuation. Its gain
occurs before drawing, while a failed draw becomes an SBA loss only after the
whole invocation finishes. Drawing a tucked Commander must offer the mandatory
CR 903.9b pre-move owner replacement with no actionable hidden object handle.
Accept replaces exactly that draw attempt without consuming the next library
card; decline draws to hand once. Any nested owner choice remains resolution,
never priority or an early SBA window. The empty-library profile proves actual
failed-draw loss after the life gain, rather than substituting a truncated run.

## Exit evidence

Require exact complete-source lowering and paired unsupported/malformed cases,
including aggregate Flame Rift and trigger-bearing Blood Artist. Preserve every
historical exact catalog, admission profile and gameplay reference independently.

Positive witnesses cover Bolt damage to player and creature; accumulated marks,
cleanup and lethal Commander return; Sorcerer opposing/self/own-player targets,
paid tap and counter/source departure; Soul Feast self-survival and opponent
loss after ordered gain; Sacred Nectar targetless gain; Healing Hands gain/draw,
sole-target failure, actual empty-library loss, and nested Commander draw return.
Use actual resulting facts/events, not merely occurrence counts.

Independent adversarial evidence must reject wrong entity kinds, noncreatures,
hidden/stale handles, stale validation receipts and rehashed source/target/event
forgeries. Check accepted target input followed by source/target/owner departure,
source bounce/recast historical identity, freshly cast tap readiness, source
removal after resolution starts, and departed-controller personal-action skips.
Verify life/damage deltas and source provenance separately from combat history.

Compare uninterrupted, one-unit-budgeted, cloned, restored and replayed execution
at every new request and decisive rule boundary, including nested library draws.
Queries must not mint handles or mutate semantic state. Off/compact/detailed
traces agree semantically. Complete formatting, Clippy, debug/release workspace
tests, cross-language fixtures, source-to-runtime witnesses and fresh retained
offline rebuild/replay before accepting this slice.
