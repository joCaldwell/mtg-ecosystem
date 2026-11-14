# Indestructible

The schema-1 `indestructible_v1` capability covers intrinsic `indestructible`
on supported permanent kinds: creature, artifact, enchantment and land. Complete
frozen Darksteel Myr and Darksteel Relic records exercise creature/artifact
lowering, including combined artifact-creature types. Printed repetitions are
retained, like other intrinsic keywords; they are redundant under CR 702.12c.
A separately bounded static grant is also supported through `ModifyCreatures`:
an Artifact with exactly the Equipment subtype, `Equipped` scope, zero power and
toughness, exactly `[Indestructible]`, and no restrictions. Complete frozen
[Darksteel Plate source](../packages/game-engine/tests/fixtures/b5b4cf54-ed5e-42d0-9d98-5fec76b0b0b8.json)
lowers its intrinsic keyword, this grant and ordinary main-phase equip for {2};
casting costs {3}. The grant itself requires `indestructible_v1`, even without an
intrinsic keyword. Rust and exported schema reject missing/spare capability,
other grant scopes or source kinds, mixed stats/keywords/restrictions, tokens and
other temporary keyword-grant contexts. Other admitted attachment grants are unchanged.
This foundation adds no pool, hosting, search or consumer admission.

Destruction checks the exact incarnation before invoking movement or
movement replacement. A forbidden attempt records `DestructionPrevented`, with
no fabricated movement. Lethal-damage and deathtouch destruction SBAs ignore
indestructible creatures, so stabilization terminates. Damage remains marked,
with ordinary lifelink and damage-trigger processing. Damage-source historical
keyword validation includes grants from validated ordinary spell programs as
well as intrinsic and static abilities. This vocabulary check does not prove a
grant occurred: departure-invariant facts and restore replay authenticate the
captured appearance, allowing legitimate attachment departure during damage. Each completed SBA check
consumes the checked deathtouch window, including protected survivors, after
committing that check's simultaneous actions and before any repeat check, trigger
placement or priority. This bookkeeping is an atomic recorded event, not an SBA
action or a cleanup priority exception. Later grant loss cannot make old nonlethal
deathtouch damage actionable (CR 702.2b/704.5h). Ordinary damage marks remain until
cleanup: previously marked lethal damage still destroys at the next SBA check
after equip changes or source removal end protection. Fresh deathtouch damage
opens a new window. Ordinary CleanupEffects atomically clears both marks and the
deathtouch flag alongside expiring effects (CR 514.2).
Simultaneous destruction consumes each leaf's captured common prior keyword facts,
so a destructible granting Equipment leaving earlier in deterministic event order
cannot remove protection during that event. Ordered children capture fresh facts;
subsequent SBA waves use current grants. Missing/mismatched prior object facts
reject; direct internal occurrences retain authoritative current checks. Independent
read/write guards and atomic rollback remain intact. Existing evidence carries the
facts; no stored fields or format versions change.
Set destruction counts actual destroyed creatures, including death movements
replaced with exile, and excludes forbidden destruction from Fumigate's gain.
Zero toughness, sacrifice, exile, bounce, legend-rule moves and departures
continue through their existing movement paths. In particular an Anger-style
replacement cannot turn forbidden destruction into exile, but can replace an
actual zero-toughness or sacrifice death.

Rules authority: frozen September 25, 2026 Comprehensive Rules,
CR 611.3b, 613.1f, 702.6a, 702.2b, 704.3, 702.12a–c, 701.8a–b, 704.5f–h, 701.21a, 120.3, 514.2 and 614.17c
([official source](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)).
Focused candidate-source evidence does not establish consumer acceptance.

The equipped-grant source-qualified producer is
`ea450fcc4ae3487a6cedb5cfcc686307d636d928fe677e3ba01e2e87347df4b5`;
its exact base/source inputs, receipts and omissions are retained under
`generated/equipped-indestructible/`, separately from consumer acceptance. Focused runtime witnesses cover paid Plate cast,
equip/re-equip, grant loss and marked lethal damage, source exile, intrinsic
survival, public keywords, response rejection, and snapshot/replay across quanta
and trace modes. The paid Planar Cleansing regression uses a clearly labelled typed
nonintrinsic Equipment control derived from Plate IR, not an asserted real card:
source-first and target-first destruction both preserve its previously protected
creature. Ordered destruction and frozen-fact authority probes are direct internal
invariant tests. No pool, policy, host/search capability set or SDK expands.

The deathtouch-window correction has source-qualified producer
`33648eacc7ced9472037b16b6c394666dab6ca4b8484b397d7e8322eb43c1b43`.
Its RED/GREEN gameplay, 33 focused tests, source closure and replay evidence are
retained under `generated/deathtouch-window/`; compiler/contracts and exported
schema bytes are unchanged from the equipped-grant foundation.

The separate `temporary_indestructible_spell_v1` capability admits one complete,
ordinary nonmodal instant/sorcery granting exactly `[Indestructible]` to exactly
one unqualified battlefield creature target until end of turn. Its `GrantKeywords`
instruction also requires `indestructible_v1`. Rust and exported schema fence that capability to Indestructible and reject
duplicate/mixed grants, filters, additional costs, modes,
activated/triggered programs and mixed instructions. The existing intrinsic and
equipped grants retain their contracts. No hosting/search capability set, pool,
policy, SDK, server contract or schema version expands.

Complete frozen [Withstand Death source](../packages/game-engine/tests/fixtures/c462adf3-4ade-4b90-bdec-7247b080471f.json)
comes from the retained d705 archive, physical line 29602/source index 29600.
The record SHA-256 is
`f68167a0729366ded35cdd527d118c5f499b85226c6b816238674ea065fa2f75`;
its archive, source, AST and binding identities remain in fixture provenance
and the accepted-archive registry. The retained previous producer rejects this
exact source with `effect:gain-abilities`; that RED witness and subsequent
attempts remain under `generated/temporary-indestructible/`.

A resolved grant uses the existing atomic `CreateEffect` event, source/cursor
provenance, captured target incarnation, historical public handles and cleanup
expiry. Source movement to the graveyard does not end it (CR 611.2a/c). It unions
with intrinsic/static layer-6 grants (CR 613.1f/702.12c), cannot follow a creature
through bounce/reentry (CR 400.7), and rechecks targets at resolution
(CR 608.2b). Cleanup removes the grant and damage marks in the same recorded
`CleanupEffects` event (CR 514.2): a protected creature with lethal marked damage
survives that cleanup. A fresh destruction attempt after expiry can destroy it.
The completed deathtouch-window correction remains authoritative.

Six focused source-backed gameplay tests exercise paid {G} response to Doom Blade,
source departure, lethal marked damage through real cleanup then fresh destruction,
bounce/reentry, ordinary opposing-creature targeting with atomic invalid/stale
response rejection, illegal target at resolution, and union with equipped
protection. They use private source-backed configurations and accepted actions,
with snapshot/replay checks at q=1/detailed and q=4096/off. Contract tests use
explicitly labelled typed IR controls; compiler tests also retain unknown-opcode
coverage using `future-effect` and test malformed `gain-abilities` separately.

The temporary-grant source-qualified producer is
`56c44471f6d28415cf4229b6b751442a4a0a84bca79d5c32eeeface2f308ba09`
(748 native source rows), based on `0d2ed30bc71f97b6574b59afa6f578098afcfe6b`.
Its `green07` receipts, copied executable/schema, 21 exported-schema probes,
source snapshots, CLI smoke/replay and preserved failures are retained under
`generated/temporary-indestructible/`. Focused verification ran 192 compiler,
68 contracts and 17 core tests, formatting, workspace all-target checking and
compiler/core no-deps Clippy with warnings denied. Broader runtime/consumer,
release/performance and full-matrix checks were omitted; local JavaScript
installation was absent and no dependencies were installed. This candidate
checkpoint remains separately source-qualified from combined integration and
consumer acceptance.


## Temporary Flying source foundation

The separate `temporary_flying_spell_v1` capability admits exactly one ordinary
nonmodal instant/sorcery instruction granting `[Flying]` to exactly one
unfiltered battlefield creature until end of turn. It also requires `flying_v1`.
`temporary_indestructible_spell_v1` and its `indestructible_v1` requirement are
unchanged. Schema 1 remains the only IR schema. Grants of other keywords,
compound/duplicate grants, mixed instructions, additional costs, filters, modes,
activations and triggers reject. No pool, default deck, host, search, policy or
consumer admission follows from source lowering.

Complete frozen [Jump source](../packages/game-engine/tests/fixtures/f7518456-45ed-41d4-bd3c-5aacea28eb35.json)
is the exact physical line 37410/source index 37408 of archive
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
The complete source is a normal-layout `{U}` instant, blue identity, Commander
legal, with sole Oracle instruction “Target creature gains flying until end of
turn.” Its AST has that sole end-of-turn gain step and a single unfiltered
creature target; its reference binding is fully bound with no diagnostics.
Record SHA-256 (excluding the line terminator):
`a164fca57e67134c6f5e1a44b482f4f2a945aac42e41ead3e9586e845de30e8f`;
source `315f9af7b00e1d3c7c122a0be565cac5f7abfd89de5a346a4c3595faaba5b81e`;
AST `fd86fe1803247a0573bdc4c45c9dbaa18ea76fd03b2582744c8fea681eca764c`;
binding `8c5dce2342d524fe04512520f3ef40628e37fc2bacb0aeb35b0c9e28d9ae0269`.
Fixture provenance and the accepted-archive list add this record without changing
existing archive or catalog membership. The original positive test rejects before
feature enablement; its RED log remains under `generated/temporary-flying/`.

The existing current-keyword union supplies Flying to the existing blocking rule
(CR 702.9b/702.17b), which excludes grounded blockers and permits Flying or Reach.
It retains the same exact-incarnation resolved effect, source independence,
historical public target handles and atomic cleanup expiry as temporary
Indestructible (CR 611.2a/c, 613.1f, 400.7, 514.2). Intrinsic/static grants survive
expiry. Tests use paid exact-source casts and normal combat declarations, plus
bounce/reentry, opponent targeting, invalid-response atomicity, forged snapshot
rejection and snapshot/replay checks with varying execution budgets.
Rules are the pinned 2026-09-25 file, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.

Damage-source frozen-keyword validation must recognize event-time resolved grants
without replacing exact source authority with a blanket catalog vocabulary.
Exact consumed source facts, duplicate rejection and accepted-input replay remain
restore requirements. The suspended-boundary test exercises damage/snapshot
execution and rejects forged missing, unrelated and duplicate source keywords.
The shared source-LKI correction is a separate dependency of this foundation.

Its source-qualified producer is
`04bf3603573fd58966fde374bdd436ca815b646b0226994f3bc174f2e9fa3a09`
at `f7ee94483e7186d656975902704383f18e4773d7`, with all 754 compiled source
rows matched to that commit. Ninety focused Rust tests, 24 exported-schema
probes and CLI smoke/replay passed. Production-library Clippy used the explicit
inherited `collapsible_if` exception; the strict failed attempt remains retained.
The schema SHA-256 is
`6be3ff88b68b3ea9cd77eac65c80790fdf5144e3ee7420ddcbe4be6667a7eaa8`.
Its immutable artifacts and receipts are under `generated/temporary-flying/` in
the implementation checkout; broader runtime/release/consumer checks are excluded.

## Compound pump and Indestructible source foundation

The schema-1 `compound_indestructible_spell_v1` capability separately admits
exactly one ordinary nonmodal instant/sorcery program: +1/+1 followed by exactly
`[Indestructible]`, both until end of turn, on the same sole unfiltered creature
target slot. Both instructions belong to source block 0. It also requires
`temporary_power_toughness_v1`, `indestructible_v1` and `creature_target_v1`.
The existing `temporary_indestructible_spell_v1` and `temporary_flying_spell_v1`
capabilities continue to describe only their pure one-instruction grants.
Rust and exported schema reject reordered, extra, duplicate or mixed grants,
other scalar amounts, separate targets, filters, additional sacrifice costs,
modes, activations and triggers. This source foundation adds no pool, profile,
policy, hosting, search, server or SDK admission.

Complete frozen [Mortal's Resolve source](../packages/game-engine/tests/fixtures/ef29f979-1a6d-47bb-83f5-4679389471da.json)
is taken directly from physical line 36184/source index 36182 of retained archive
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
Its complete normal-layout source is a green, Commander-legal instant costing
`{1}{G}`. Its sole parsed sentence has the pump and grant steps sharing
`line:0:0`; the binding has two bound references and no diagnostics.
The record SHA-256 is
`dd42d4da1b3412af58b145b7a6657dfb5cb98a047efca164ecaa1f5fa34560ad`;
source `71b794378a2dc7529072ebcdc59ed52b203e0e164510d758c3f1de160cbd0d59`;
AST `3f1ca48780e3f8a0661d987a50c1a69b54e3d3af315f2589afe09b8c2f225811`;
binding `5f2ba2465e70f10af914637a882a0b8c33466130b22c159b98ecba6f909acef2`.
The fixture provenance and accepted-archive record list extend additively.

Existing compiler emission and core execution supply both instructions without
a production core change. The two resolved effects preserve their source/cursor
provenance and capture the same exact incarnation. Source departure does not end
either effect; bounce/reentry does not rebind either. Resolution rechecks target
legality before the program, and SBAs follow the complete invocation (CR 608.2b/c,
704.3). The keyword union stays unique alongside intrinsic/equipped protection.
`CleanupEffects` removes both effects and damage marks atomically, so a creature
with lethal marked damage survives true cleanup; fresh destruction after expiry
can kill it (CR 514.2). The existing source-LKI correction remains a dependency.
Rules authority is the pinned 2026-09-25 rules file cited above.

Dedicated compiler/contracts controls fence the source and schema shape.
Private source-backed gameplay tests use accepted casts and payment, q=1/detailed
and q=4096/off, with rejected query/response atomicity, incoming destruction,
+1/+1 and keyword union, marked lethal cleanup, bounce/reentry, resolution
revalidation, snapshot/replay and suspended invocation-prefix restore. Exact
copied producers, exported schemas, source manifests, command receipts and
preserved failures are retained under `generated/compound-indestructible/` in
the implementation checkout; these identify the tested source independently
from later integration and consumer acceptance.

## Temporary Deathtouch source foundation

Schema 1's separate `temporary_deathtouch_spell_v1` capability admits exactly
one ordinary nonmodal instant/sorcery instruction granting `[Deathtouch]` to one
unfiltered battlefield creature until end of turn. It also requires
`deathtouch_v1` and the ordinary creature-target capability. Pure Flying,
pure Indestructible and the exact compound Indestructible program retain their
separate capability meanings. Mixed/duplicate keywords, arbitrary keywords,
extra instructions, qualifiers, modes, activations, triggers and additional
costs remain rejected. No pool, profile, policy, hosting/search, server or SDK
admission follows from this foundation.

Complete frozen [Serpent's Gift source](../packages/game-engine/tests/fixtures/b75fb789-6a78-4d23-b872-7627b478fb6f.json)
is physical line 27602/source index 27600 of the retained d705 archive. It is a
normal-layout, green Commander-legal instant costing `{2}{G}`, with one bound
`line:0:0` creature selection and a sole end-of-turn Deathtouch grant. The full
archive SHA-256 is
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
record `609022e2d6a4569c836a8490ab9b200501f7abf203eaebaaa9bbf6e149f6e836`;
source `5e6f68fe1681e09e9f3faa1979b217619b0c58a6d43f2f88159d770c64579e6f`;
AST `17b8cc784edcd8b8501740cd8f60cbd7a6e04613e18166c75c91db1d7abdbb1d`;
binding `920737a787011947206ff954053c8cb90425605fc14268f4d16c4ac678879ac1`.
The record is added to fixture provenance and the accepted archive registry,
without adding catalog membership. The retained baseline CLI
`007dfac0f81217d9b409de83eaf577c0933468bda9ace3f639a305cc16cc8bba`
rejects this exact source; its full-archive RED output remains under
`generated/temporary-deathtouch/baseline-red/`.

The existing keyword union and damage-time appearance supply deathtouch without
a combat rewrite. The grant captures an exact incarnation, survives its spell's
departure, does not follow bounce/reentry, and expires with damage marks in atomic
cleanup (CR 400.7, 611.2a/c, 613.1f, 514.2). A granted source's positive damage
can destroy a creature with greater toughness; granting deathtouch after ordinary
damage cannot retroactively change that damage (CR 702.2a-e, 704.5h). Current
sources use current appearance at damage, and sacrificed sources retain their
exact last-known keywords (CR 113.7a, 608.2h, 702.2d/e). Indestructible victims
retain marked damage but consume the completed deathtouch observation window;
removing protection later cannot revive that nonlethal window. These use the
pinned 2026-09-25 rules identified above.

Private source-backed scenarios use paid casts and real damage activations and
combat: one damage from Prodigal Sorcerer or departed Mogg Fanatic kills Yoked
Ox, while prior ordinary damage stays ordinary. Other checks cover protection
loss, source independence, target resolution revalidation, bounce/reentry,
intrinsic/static keyword union, query and invalid-response atomicity, cleanup,
and snapshot/replay at varied execution budgets and trace modes. Suspended
damage snapshots retain unmodified positive controls and reject forged missing,
unrelated and duplicate keywords after normalizing the state checksum, so the
negative reaches semantic restore/replay validation.

Exact tested producers, source manifests, command receipts, schema probes and
preserved failures remain under `generated/temporary-deathtouch/` in the
implementation checkout. This is focused source-foundation qualification;
combined integration and exported-client acceptance remain separately owned.

Its focused producer is `962c859db1340f78f1884b2f87babb1e60f4fe4cf8679d8784ebf85387cf915f`
(767 native source rows); exported schema SHA-256 is
`544f1d14d2a1f7f6ce5d0c3b016e3929678f76b3443445afb0d445b8df9441c7`. The retained `native02`/`native03`
receipts record 203 compiler, 79 contracts and 31 distinct core tests, 33
exported-schema probes, all-target checking and CLI smoke/replay. Production
compiler/core/simulator Clippy is strict with `--no-deps`; contracts uses the
inherited `collapsible_if` exception. The source runner passed tooling and
structure checks but its archive-parser import lacked local Ajv; no dependencies
were installed. Full runtime, release/profile/consumer and completion matrices
were omitted. Preserved earlier attempts remain failures, not acceptance.
