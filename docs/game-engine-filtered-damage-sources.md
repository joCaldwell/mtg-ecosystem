# Controlled creature damage sources

IR30 adds `FilteredDamageTriggerV1` and
`DamageTriggerSource::CreatureFilter { controller, types }`. The finite control
relation is `Controller`: the actual damager's battlefield appearance at the
event must have the observer's controller. Other relations reject. The source domain
is battlefield creatures, further restricted by the complete bounded
`TypePredicate`. Type arrays are conjunctive; artifact **and** creature are both
required by the source witness. Kind and recipient remain independent dimensions
with their existing capabilities and version gates.

The complete observer witness is **Research Thief**, Oracle ID
`ba5055ce-6809-47a4-bee6-fb97a5b60f8a`, retained in `cards-data-d2ce/` beneath
the engine fixtures. The compiler preserves its artifact-creature type, flash,
flying and “Whenever an artifact creature you control deals combat damage to a
player, draw a card.” The trigger program remains one controller draw. The
printed creature domain, explicit “you”, unit cardinality and every supported
filter field must lower completely; artifact-only sources, missing/opponent
control, ownership/status/other qualifiers, grouping and event quantities reject.

**Alpha Myr**, Oracle ID `4caa8e68-e599-416f-b839-3d009569ff29`, is a distinct
complete vanilla artifact-creature witness. Ordinary extraction verified archive
SHA-256 `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`
before extracting line 11539. The two-row `cards-data-filtered-damage/` group
retains its source and refers to the original Research Thief file; original
records are unchanged. No new permanent catalog or profile is introduced.

Matching consumes actual positive player damage members, keeping assignment-time
source facts separate from event-time battlefield appearance. See the subsequent
[matching correction](game-engine-damage-match-appearance.md). The trigger observer and damager can be different incarnations with
different owners. Observer source/LKI validation remains separate from damage
witness validation, which checks the actual damager's card identity and owner.
One matching occurrence per observer/damager member retains ordinary multiplicity;
aggregate life totals and current combat membership cannot fabricate matches.
Self-source predicates still require exact incarnation equality. Accepted-input
replay proves coherently forged observer, damager and recipient histories false.

## Current compiler source binding

At schema 1, an effect's damage source must remain distinct from the trigger's
observer. The current damage instructions execute using the invocation source;
they do not implement an arbitrary triggering object's damage action. An effect
using the pronoun `it` therefore lowers only when the complete trigger proves
that object is the invocation source: self entry, death, attack or damage.
The reference must also be the first top-level effect step, before another
instruction can introduce a different antecedent; later or nested references
require a richer binder before support.
Filtered/grouped damage, another creature's entry/death, spell-cast and step
events cannot supply that proof and reject an `it` damage source explicitly,
including within modal branches. Explicit `self` or `this` source references
retain their own meaning. This does not add event-object damage execution,
capabilities, catalog admission or a new schema version.

The distinction follows the pinned rules below: CR 120.7 identifies the object
that deals damage; CR 113.7a and 608.2h distinguish that object's action from the
ability and retain source/LKI behavior. Synthetic, coherently rebound source
regressions test the rejection boundary; they are not new archived card facts.

The trigger-source correction passed all 131 library tests, including freshness of
every committed catalog, formatting and production compiler Clippy. The actual
TypeScript parser/binder reproduction matches the retained synthetic record and
reference graph byte for byte. Paired probes cover eight nonself event contexts,
four self contexts, deferred/modal references and grouped damage. Five complete
supported self-source records still lower; Falkenrath Perforator's distinct,
unsupported defending-player recipient remains rejected. No previous source,
expected IR or catalog digest changed. The original incorrect lowering and an
intermediate test-classification failure remain retained alongside the passing
`filtered-event-source-green-02` receipts under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261113/`.
Runtime/replay, server/SDK, full source-tooling and release/profile matrices were
not repeated for this compiler-only correction.
The subsequent [damage-source binding contract](game-engine-damage-source-binding.md)
also covers activated costs and recursively checked spell modes.

The previously verified 32-definition headless profile, its IR29 catalog/digest,
hosting/search sets and snapshot/public formats are unchanged. New capabilities
do not grant feature admission. This increment does not renew combined acceptance.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 109.2/109.5 define the permanent domain and controller; 205.2b preserves
combined type membership; 120.7 identifies the damage source; 603.2c/603.3a/603.10
govern occurrence, control and observers; 615.6 excludes prevented damage.
Official source: <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused validation at source checkpoint `7c75107` passed formatting, workspace
all-target compilation, 111 compiler tests, 19 contracts tests and 48 selected
core tests (32 damage-trigger, six recipient-discard, six recipient-sacrifice
and four profile tests). The 33 executable schema/catalog tests passed, including
current-parser agreement for both exact source records and the unchanged IR29
match catalog. Selected tests executed with no skips. The damage tests include
transactional combat, replay, separate observer/damager ownership, private choices
and coherent forged-witness rejection.

Raw logs and source pins are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261110/`
as `filtered-damage-focused-01.log`, `filtered-damage-schema-01.log` and
`filtered-damage-extract-01.log`. Full runtime suites, a new complete-match
capture, hosting/search, release/profile matrices and retention rebuilds were
not run. The prior match evidence retains its original source identity.
