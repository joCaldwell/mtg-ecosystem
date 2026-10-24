# Cards-as-data audit (2026-10-24, HEAD 36733fe)

**Question (Josh):** are card rules hard-coded in the engine, so that newly
printed cards need engine work?

**Answer:** the core runtime never branches on card identity. Even so, a newly
printed card that uses only existing mechanics cannot be played without code
changes and hand work. The cost comes from three places, not from
`if card == X` checks:

- template-shaped IR and validation;
- exact-match lowering in the compiler;
- digest-pinned, linear profile governance.

Raw outputs are in `generated/cards-as-data-audit/` (ignored).

## Findings

1. **No identity-based rule branching in core.** Every oracle-ID constant in
   non-test core code is one of:
   - scenario setup;
   - an admission allowlist entry or definition digest;
   - a deck or commander table;
   - a banned-list entry.

   Functions named after cards (`anger_targets`) are driven by IR
   instructions, not by identity.

2. **The IR is partly generic and partly per-template.** Generic operations
   include `DealDamage`, `DestroyPermanent` with a `PermanentFilter`,
   `CreateCreatureTokens`, `ModifyPowerToughness`, `MoveSet` and `DamageSet`.

   Each of these variants encodes one card's template:

   | Variant | Card |
   | --- | --- |
   | `GainLifeDestroyed` | Fumigate |
   | `ExileDamagedThisTurn` | Anger of the Gods |
   | `LoseLifeDeathController` | Massacre Wurm |
   | `ReturnAuraFromGraveyard` | Rancor |
   | `ReduceGenericByCreatureCount` | Blasphemous Act |
   | `DoubleDamageToPermanentOrPlayer` | Furnace of Rath |
   | `PreventCombatDamage` | Fog |
   | `DamageTargetController` | Smash to Smithereens |
   | `TargetPlayerSacrificeCreature` | Cruel Edict |
   | `WipeAmount::CreatureCount` | Chain Reaction |

   Other problems:
   - `ObjectSet` is a closed list of seven sets, not a filter. `Duration` has
     only `UntilEndOfTurn`.
   - Validation contains literal card text: the sacrifice subtype must be
     "Goblin", the death-filter subtype must be "Spirit", and composite
     activations must match the Mind Stone or Viridian Zealot shape.
   - Duplicate encodings are kept for historical bytes, for example
     `DestroyTargetCreature` alongside `DestroyPermanent`.
   - Capabilities are named after slices (`BoardWipesV1`, `SacrificeV1`), not
     after mechanics.

   Mitigation: templates do recur. Brilliant Halo reuses the Rancor
   instruction, Vanquish the Horde reuses Blasphemous Act's, and Assault
   Intercessor reuses Massacre Wurm's.

3. **The compiler matches exact templates.** It does 53 exact `json!` equality
   checks against AST subtrees. Examples:
   - set lookup;
   - sacrifice filters;
   - Anger's whole replacement subtree;
   - Fumigate's for-each;
   - Blasphemous Act's cost reduction;
   - land search.

   Other problems:
   - Type lines are a closed list of eight spellings, so "Artifact Creature"
     and "Legendary Enchantment" are rejected.
   - The compiler reads the AST into a strict, closed mirror; any unknown node
     rejects the whole card.
   - The compiler accepts only one compiled-in archive manifest. It therefore
     rejects every record of the current archive, which was regenerated on
     2026-10-24.

4. **Behaviour is gated by profile lineage, not capability.** About 250
   call sites use `*_enabled()` or `profile_at_least`.
   - Admission requires a byte-exact catalog digest and a per-profile
     commander allowlist.
   - Capacities are per-profile if/else chains. The same token maker faults at
     64 tokens in tokens-v1 but at 65,536 in wipes-v1.
   - Some instructions silently do nothing below a lineage point. This is
     unreachable today only because catalog version checks happen to agree
     with the lineage.
   - The lineage is strictly linear, and hosting and search are allowlists.
   - `BUILD_ID` hashes every source file and fixture, so adding one card
     invalidates every saved snapshot.

5. **Simulator policies contain card names.** There are 110 card-name uses
   across 65 cards. They are strategy only: the engine validates every
   response. They are brittle, though:
   - some detect their profile by definition count, so adding one card turns
     them off;
   - some re-implement card rules, such as Blasphemous Act's 13 damage.

## Corpus measurement (IR v20, 38,721 archive records)

- Commander-legal records: 32,116.
- With the current archive manifest, 0 lower, because of finding 3's manifest
  pin.
- With the pinned header substituted, 1,719 lower and validate (5.35%):
  - 168 are profile cards and 1,551 are not in any profile;
  - 776 of those 1,551 carry instructions.

Top rejection reasons:

| Records | Reason |
| --- | --- |
| 2,481 | Unparsed card |
| 2,263 | Keyword AST shape not in the compiler mirror |
| 2,229 | Source-line normalisation mismatch |
| 1,071 | `phase` trigger |
| 890 | `gain-abilities` |
| 766 | Multi-face card mislabelled Invalid |
| 756 | `cast` trigger |
| 746 | `attacks` trigger |
| 595 | `enters-tapped` |

19,905 of the 30,397 rejections are in the closed `abilities` mirror.

## Cost of adding three already-lowerable cards

The test cards were Canyon Minotaur, Open Fire and Flesh to Dust. Making them
playable took 14 steps:

- **Code:**
  - accept a new archive manifest;
  - add `include_str!` arrays and catalog builders;
  - edit the per-group "must need vN" invariant;
  - change a digest constant and count words in error text;
  - add a `MatchProfile` entry, decks, capacity chains and sim subcommands;
  - change hard-coded counts in tests.
- **Data:**
  - fixtures and provenance;
  - regenerated expected IR (no generator exists);
  - banned-list review rows.
- **Hand work:**
  - re-checking the resource bounds;
  - witnesses and an 18-game batch;
  - docs;
  - accepting snapshot invalidation.

With those steps done, the 18-game batch had 0 faults.

## Recommendations (priority order)

1. **Admit by capability.** A catalog is legal if it validates at IR vN. A
   profile becomes a capability set plus a commander policy plus a bounds
   policy, and the catalog digest lives in the match identity, not in code.
2. **Archives as data.** Accept a list of archive manifests, each record
   carrying its own provenance.
3. **Catalogs from a data manifest.** Add a generator CLI for expected IR, and
   delete the hard-coded counts and per-group invariants.
4. **Delete lineage gates.** Let catalog validation be the single source of
   truth. Turn silent no-ops into errors, and name capabilities by mechanic.
5. **Mechanical bounds.** Compute capacities from catalog and deck features,
   or use one generous cap with a typed fault as a defined outcome. Keep hand
   proofs for benchmarks.
6. **Generalise the IR.** Replace `ObjectSet` with filters plus relations,
   drop the literal subtype restrictions, and merge the duplicate variants.
   Add generic for-each amounts, replacement effects, cost modification and
   filtered trigger events.
7. **Compositional compiler.** Lower filters, amounts and players by
   composition instead of `json!` equality. Widen type lines, and report
   multi-face cards as Unsupported, not Invalid.
8. **One banned-list data file.** Derive pool reviews from it.
9. **Generic sim policies.** Use IR-derived heuristics, drop the
   definition-count fingerprints, and add one `match-batch PROFILE` command.
10. **Scope `BUILD_ID`.** Separate code identity from card data.

## Classification

- **Hard-coded card rules (to fix):**
  - template IR variants;
  - literal subtype and shape checks in validation;
  - exact-match lowering.
- **Deliberate curation (a choice that could be relaxed):**
  - digest-pinned profiles and linear lineage;
  - per-profile bounds, decks and banned lists;
  - hosting and search allowlists;
  - `BUILD_ID` scope;
  - the archive pin.
- **Fine as is:** identity constants in test and scenario fixtures.
