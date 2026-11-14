# Ramos M2: land and ramp family contract

Preparation against `46ef08faedb8c5fa9c1c1fa9a8afefd1f85e1f6c`, whose parent is
`origin/main` `bbdba9f5add0bfd0c75c0ec18eb2986581c3e534`. This document specifies
future work; it adds no executable source support, catalog/profile admission,
host capability or sampled-search qualification. M0 owns the production compiler
audit and native verification this wave. Its single support matrix remains the
authority for per-card diagnostics and evidence; this is not a second matrix.
The [Ramos plan](game-engine-ramos-plan.md) owns the exact revision-17 list.

## Frozen authorities and reproducible lookup

Read-only local archive: `/home/josh/Code/mtg-ecosystem/generated/oracle-ast.jsonl`.
Full-file SHA-256, recomputed during preparation:
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
The first line, without LF, hashes to
`994bc48e8d8791696330f63338bc32ee0bd17c8c7e9901956ebd35c171f9315a`;
it agrees with `packages/game-engine/tests/fixtures/archive-manifest-current.json`.
The header pins corpus
`f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738`, parser
revision `327b3360ea8e5cffb75b136223e75a348e903cbe`, and parser source
`316c9deaba977533329b903e9d9ba48b6d02057651702c05efa460fb651fbd3a`.
Use [catalog extraction/provenance](game-engine-catalogs.md), preserving raw lines;
do not reconstruct fixtures from the prose below or substitute historical records.

These one-based archive line locators, under that full-file hash, identify the
reviewed complete source records and their embedded source/AST/binding hashes:

- Existing primitives: Command Tower 1317, Arcane Signet 1796, Golgari Signet
  4431; Swamp 12988, Mountain 24738, Island 26926, Forest 27016, Plains 28345.
- Entry family: Stomping Ground 3364, Breeding Pool 4870, Overgrown Tomb 22805,
  Cinder Glade 33734, Hallowed Fountain 36510, Temple Garden 36913.
- Cycling lands: Spara's Headquarters 7337, Ketria Triome 16210,
  Ziatora's Proving Ground 37492.
- Search family: Kodama's Reach 3296, Windswept Heath 6206, Skyshroud Claim
  8276, Farseek 11025, Nature's Lore 18157, Cultivate 21034, Polluted Delta 36241.
- Other production: Talisman of Creativity 3185, Exotic Orchard 5958,
  Yavimaya Coast 9724, Talisman of Curiosity 21139, Fellwar Stone 22522,
  Talisman of Unity 34693, City of Brass 36660, Sulfurous Springs 37160.

All 32 records were located once and their full Oracle text reviewed. The six
representative AST/binding records inspected in detail (Breeding Pool, Cinder
Glade, Ketria Triome, Polluted Delta, Cultivate, City of Brass) are parsed and
fully bound in this archive. That is not a compiler pass. For first-checkpoint
extraction, raw-line SHA-256 including LF is
`705033949a16e2e665f524fef1e72663b6bf1e4adbd7c6db31a1cc5f303bd57d`
for Cultivate and
`d5d2612e7add559bffd594aa52be174831cda4e50c732ceff8deef7b16416c9c`
for Kodama's Reach. Their common AST hash is
`d9a54c65f069a5b54c205956f62b5c6a200e9abd34f0d9e6d0d5f4c8336bf821`.
M0 must reconcile app text, source pins, legality and actual compiler diagnostics.

Rules authority: local
`/home/josh/Code/mtg-ecosystem/.scryfall-cache/rules-20260925.txt`, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`,
the pinned [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt).
Rule numbers below use this edition (Search is 701.23; Shuffle is 701.24).
No live deck edits or newer card/rules substitution are authorized by this work.

## Complete-source boundaries

**Types and entry.** Basic is a supertype, independent of land subtypes
(205.4b–c). Each basic land subtype supplies its intrinsic tap-for-one ability
(305.6); multiple types offer separate abilities, never simultaneous multicolor
production. Nonbasic Forest cards qualify for a Forest search. Preserve printed
types in library as well as battlefield characteristics. M5's future type additions
must feed the same characteristic/production queries (305.7), without making a
land basic or extending battlefield-only effects into the library.

Each of the five target shocks has its printed pair of land types and an optional
pay-2-life entry replacement; declining or inability to pay means tapped entry.
This choice occurs before entry, with no intervening priority (614.1c, 614.12a,
118.3b, 119.4). Paying exactly the remaining two life is legal. Farseek's explicit
tapped destination still applies if life is paid: payment does not untap a land.
Do not suppress a legal life choice merely because another effect makes entry
tapped. Cinder Glade checks control of at least two **basic lands**, not two basic
land types, before entry. Other simultaneously entering lands do not count as
already controlled. Multiple simultaneous shock choices must respect total
payability (614.12b); commit their resulting entries together after choices.
Play, search and other admitted battlefield moves share this entry authority.

**Triomes.** Ketria, Spara and Ziatora each have three printed land subtypes,
unconditional tapped entry, and cycling `{3}`. All three branches are obligations
for complete source support. Cycling functions from hand: pay `{3}` and discard
that exact card as a cost, then resolve a stack ability drawing one card
(702.29a–d, 602.2, 701.8, 121.1). It is neither casting nor a battlefield sacrifice.
Source movement does not remove the activated invocation. Opponents can respond
after costs; the drawn card is private. Do not admit only the land-play half,
confuse ordinary cycling with typecycling (702.29e–f), or add Mosswort Bridge's
hideaway half here; its complete source belongs to M9.

**Search and ramp.** These searches choose during resolution, not announcement,
and have no targets. A quality search in a hidden zone permits failure to find,
even when a match exists (701.23a–b). Do not require one card merely because the
printed count is singular. Always perform the specified final shuffle, including
zero found cards or a zero/one-card remaining library (701.24a/e).

- Cultivate and Kodama's Reach: up to two basic land cards; reveal the selection;
  choose one to enter tapped and send the other to hand, then shuffle. With one
  found card, it goes to the battlefield; there is no hand-only option. With two,
  partition by identity, not sorted response position. Preserve the Arcane subtype
  on Reach. Hand movement is not a draw (121.5).
- Nature's Lore: one Forest card to battlefield with no imposed tapped status.
  Skyshroud Claim: up to two Forest cards, entering together, with no imposed
  tapped status. Intrinsic/conditional entry replacements still apply.
- Farseek: Plains **or** Island **or** Swamp **or** Mountain card to battlefield
  tapped. A Forest-only card is excluded; a Forest/Island card qualifies once.
- Polluted Delta: tap, pay 1 life, sacrifice this land; then a normal stack
  ability searches for Island **or** Swamp, puts it onto battlefield, then
  shuffles. Windswept Heath substitutes Forest **or** Plains. Pay all costs
  before opponents receive priority; failed announcement pays nothing
  (118.3, 602.2, 733.1). At one life, payment is legal, with ordinary subsequent
  SBA/departure handling. Countering the ability does not refund costs.

Only Cultivate/Reach explicitly reveal. Do not invent a reveal event for the
other searches (701.23e); battlefield entry itself makes found cards public.
Reveal is temporary visibility without movement (701.20a–b); the selected hand
card's identity remains known as permitted, without exposing unrelated hand or
library cards. Hidden candidates must not reveal library order (401.2).

**Production and damage.** Sulfurous Springs and Yavimaya Coast, and each target
Talisman, have two activated mana abilities: a colorless option and a choice of
one of two printed colors with one damage to their controller. The latter damage
is part of the resolving mana ability, not a life payment, not a separate trigger,
and not a target (605.1a, 605.3b, 120.2b/3a). Use the damage/replacement authority;
do not implement it as unconditional life subtraction. A player at one life can
activate it; no life affordability check applies to its damage effect.

City of Brass instead has any-color mana and a separate **becomes tapped** damage
trigger. Collect on actual untapped-to-tapped occurrences, whether caused by a
mana cost or another effect; already tapped and enters-tapped are controls
(603.2e). Freeze controller/source facts at the event. This trigger does not add
mana and therefore is not a triggered mana ability (605.1b); place it on the stack
at the next priority boundary, after the enclosing announcement/resolution and
SBAs, under ordinary APNAP rules (603.3). No immediate damage while paying for a
spell, and no trigger from a reversed illegal action (733.1).

**Opponent production.** Exotic Orchard and Fellwar Stone query colors a land an
opponent controls **could produce**, not colors currently in opponents' pools,
their color identities, or their currently activatable land abilities. CR 106.7
ignores affordability, taps and other unpaid activation costs, and includes
applicable replacement effects in any possible order. Union colors across all
opponents' current lands, exclude `{C}`, and use the same result for offered color
choices and resolution validation. Tapped pain/shock lands still contribute;
fetches have no mana ability and contribute nothing. An opponent's Command Tower
uses that opponent's commander's identity. With no definable colors, activation
may still tap the source and produce no mana: do not fabricate a color or demand
an impossible color response. Recursive Orchard queries need a terminating
dependency evaluation: mutually dependent Orchards with no independent color
source define no colors, rather than inventing all colors. Land/control changes
invalidate query work. Future M5 grants, types and mana replacements must enter
this authority before their sources join the same admitted pool.

## Reusable code and concrete gaps at the base

Paths in this section are relative to `packages/game-engine/crates/`.

| Existing authority | Reuse and required extension |
| --- | --- |
| `contracts/src/lib.rs`: `Characteristics`, `basic_land_color`, land validation | Carries `basic` and subtype metadata; rejects nonbasic land subtypes and nonbasic lands without explicit mana. Allow reviewed typed nonbasics and intrinsic production without weakening complete-source checks. |
| `contracts/src/lib.rs`: `ManaAbility`, `ManaProduction`, `PermanentActivationCosts`; `compiler/src/lib.rs`: `lower_mana_ability`, `lower_permanent_costs` | Fixed/any/commander production and tap/mana/sacrifice cost representation exist; validation admits one tap mana ability, rejects sacrifice mana costs. Compiler costs currently only tap/mana/sacrifice; production excludes choice/could-produce and rejects extra damage effects. Ramos's nontap counter cost belongs to M1. Fetch is a nonmana activation. |
| `core/src/engine/mana_permanents.rs`, `core/src/engine.rs::mana_abilities` | Nested payment, source reservation, color choice and undo receipts exist. Index zero/first-ability assumptions occur in execution, query and restore validation. Add stable ability identity before multi-ability sources; intrinsic plus explicit abilities must compose. Current `mana_abilities` reports production, not affordability, but is not yet a general 106.7 evaluator. |
| `contracts/src/lib.rs`: `SearchBasicLands`; `compiler/src/lib.rs` complete search lowering | Only own-library basic-land search, maximum one/two, all to battlefield tapped then shuffle. Cultivate's reveal/simultaneous split AST and typed-filter ASTs must not be squeezed into this old instruction. Consume the complete actor/filter/count/move/reveal/shuffle sequence or reject. |
| `core/src/engine/staples.rs`: `search_lands`, handles, `ResolutionChoice::{Search,Shuffle}` | Private scoped handles, duplicate/cardinality rejection and resumable deterministic shuffle exist. Add explicit filter, reveal and destination/partition state. Candidate sorting is independent of library order; preserve that property. |
| `core/src/engine/events.rs`: `SearchLandEntry`, `Move`, `Tap`; `compiler/src/lib.rs` static entry lowering | `Move` incorporates unconditional `EntersTapped`; `SearchLandEntry` forces tapped status after moving. Unify entry intent before committing, add conditional/optional choices and preserve simultaneous groups. Compiler explicitly rejects conditional enters-tapped. |
| `contracts/src/lib.rs`: `FunctioningZone`, `ActivationCost`, `TriggerCondition` | Zones are battlefield/stack only; no hand activation/discard-source cost or becomes-tapped trigger. Reuse ordinary draw, discard movement, invocation, damage and trigger pipelines, with new validated source/zone/event domains. |

Current tests to extend, not acceptance evidence for this proposed work:
`compiler/src/lib.rs::{mana_complete_sources_and_negatives,basic_land_search_rejects_qualified_search_and_shuffle_players,white_mana_fixture_does_not_admit_additional_land_semantics}`;
`core/src/engine/staples_tests.rs::search_zero_one_two_private_handles_shuffle_and_pending_restore`;
`core/src/engine/mana_tests.rs` nested spending, forged receipts and recovered queries.
The historical [mana contract](game-engine-mana-permanents.md) explicitly excludes
could-produce and damage mana; its old version/capacity claims are not current gates.

## Checkpoint and integration ownership

**First useful checkpoint after M0: complete Cultivate and Kodama's Reach.**
Reuse the existing basic-land private search and shuffle foundation; add explicit
reveal and identity-bound battlefield/hand partition, and validate complete source
lowering/capability closure at schema 1. This advances exact target ramp without
needing shock choices, typed nonbasic admission, cycling, or M1's counter-mana
payment changes. Do not claim either source executable until its complete
sequence runs. M0 supplies frozen fixtures/provenance and observed baseline
diagnostics first. A reduced headless profile/witness is separate admission work,
and never qualifies the exact Ramos deck.

Subsequent batches: typed search plus shock/Cinder entry; fetch costs/activation;
hand cycling plus complete triome admission; multiple mana abilities plus pain
lands/Talismans; City tap trigger; opponent production. Split further when needed
for a small checked checkpoint. Do not add incomplete typed triomes to make the
typed-search demonstration look broader. Source-backed shocks, once complete,
provide nonbasic search witnesses before cycling lands become admissible.

Before implementation, the coordinator assigns these concrete boundaries:

- M1 owns counter cost/once-per-turn semantics and the shared payment/receipt
  changes in `core/src/engine/mana_permanents.rs` and its announcement callers.
  Agree on `(exact source incarnation, ability identity)` through request,
  response, nested reservation, receipt and restore. No source-only/index-zero
  selection shortcut. M2 later supplies life/discard cost requirements, mana
  choice/side-effect requirements and typed-query inputs to that owner.
- The single contracts/compiler integration owner applies agreed schema-1 types,
  capability closure and compiler dispatch changes for both families. M2's first
  slice needs a bounded basic-search plan with selected identities, an explicit
  battlefield selection, revealed identities and a resumable shuffle cursor;
  final wire names are implementation decisions, not a new schema version.
- M2 owns the future land-search/entry/cycling implementation and dedicated tests
  only after path assignment. Entry continuations must return to the exact
  enclosing play/resolving search group, with entry-tapped intent retained.
  Multi-entry choices cannot serialize a simultaneous entry into independent
  battlefield moves. Review event-group conflict footprints (shared life costs
  and library writes) before claiming the current simultaneous primitive fits.
- Coordinate rollback centrally: current mana undo records pool/source/counters
  and cost triggers, not general damage/life/replacement effects. M1's new counter
  receipts alone will not qualify pain-mana undo or City trigger rollback.
  Preserve unrelated floated mana, earlier taps, ability usage and departures;
  reverse only effects belonging to the reversed action under CR 733.1.
- M2 never opens a second native build this wave. Later runtime owners use the
  shared lock and focused selectors. Host/SDK and sampled-search owners review
  their explicit capability sets separately; private **library search** support
  is not simulator **sampled-search** qualification. Recheck common resource
  bounds before admission: City adds triggered land sources, multiple mana
  abilities invalidate the old single-ability assumption, and simultaneous entry
  choices retain more continuation state. Policies use public IR and permitted
  requests, without card-name dispatch or authoritative hidden-state access.

## Focused witnesses and rejection boundaries

Use exact compiled fixtures and accepted inputs from legal paid setup/pods.
Synthetic states supplement, rather than replace, source-backed execution.
For each new pending decision, restore and accepted-input replay must agree at
the suspended choice and after completion in off/compact/detailed traces; include
small-budget suspension and recovered public/private queries where applicable.

1. First slice: paid Cultivate and Reach, selecting zero/one/two cards; two-card
   selections in opposite input order with an explicit battlefield choice;
   reveal before simultaneous split, hand identity knowledge without a draw;
   shuffle even on zero selection and empty/singleton remainder. Restore before
   search, after reveal/partition and during shuffle. Reject duplicate, over-count,
   nonbasic, wrong-zone, foreign-seat and stale handles atomically. Opponent
   queries expose neither candidate identities nor library order; query calls
   do not change state/history/handles. Preserve known found-card history through
   shuffle without retaining handles into the randomized library.
2. Typed entry: Nature's Lore finds a nonbasic shock with Forest; Farseek rejects
   Forest-only but accepts Forest/Island once; Claim finds two distinct Forest
   cards entering simultaneously. Pay/decline/cannot-pay shock choices, pay at
   exactly two life, Farseek remaining tapped despite payment; Cinder with zero,
   one and two basic lands, and with basic-type nonbasics that do not count.
   Check simultaneous choice affordability and pre-entry characteristic timing.
3. Fetch: all tap/life/sacrifice costs and one invocation precede priority; source
   graveyard incarnation differs from frozen ability source. Fail-to-find still
   shuffles. Countering the invocation keeps costs paid. Reject tapped source,
   insufficient life, duplicate source reuse and illegal timing with no partial
   payment. Snapshot before costs and in the later private search; include
   payer/source-owner departures without resurrecting sources or redirected draws.
4. Cycling: hand `{3}`/discard before priority, opponents see discarded identity
   and stack invocation, private draw only on resolution; countered ability keeps
   cost paid. Reject battlefield/graveyard cycling, another player's hand and
   insufficient payment; no cast/land-play history. Restore in payment and after
   discard; check empty-library draw/SBA and source movement while pending.
5. Mana: distinguish colorless and each colored pain option; immediate damage
   versus separate City stack trigger; tap City via a nonmana effect; already
   tapped/enters-tapped do not trigger. Exercise nested Signet/Ramos payments,
   a failed enclosing cast with keep/undo paths, and source departure. Verify
   exact counters/usage/pool/life/trigger restoration against unrelated resources.
6. Production: tapped opponents' typed/pain lands, different opponent commanders,
   only-colorless land, no mana-producing lands, control changes and seedless
   Orchard cycles. Recovered query and resolution offer the same legal colors;
   reject stale/foreign ability and color handles. Add M5 type/grant/replacement
   interactions when M5 joins the admitted pool, rather than assuming them now.

Compiler/contracts near-misses must isolate the new guard: changed search actor
or library owner, added filter qualifier, AND instead of OR land types, extra
reveal/draw/destination, conditional shuffle, unsupported entry payment, typecycling
instead of cycling, additional tap trigger condition, and could-produce querying
artifacts or including colorless. Unknown extra clauses remain whole-source
rejections. Distinguish unsupported semantic forms from malformed IR; do not
relax another guard merely to reach the intended diagnostic.

Preparation verification: Git ancestry/clean scope, archive byte hash and source
lookup, representative AST/binding review, pinned rules lookup, local doc links
and `git diff --check`. No compiler execution, native build/test, gameplay witness,
host/SDK or sampled-search test was run here. Entry doctor found pinned tools but
missing worktree-local npm dependencies; docs-only preparation does not need an
install. A future executable owner should use `scripts/engine-bootstrap.sh` before
checks if still missing, without concurrent installation in that worktree.
