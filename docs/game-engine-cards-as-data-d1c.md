# D1c: generic simulator policies

D1c replaces the five catalog-specific completion-policy modules with
`match_games/generic_policy.rs`. The policy consumes only permitted observations
and the public catalog. Core rules, admission, capacity guards, decks and profile
completion layouts are unchanged.

The chooser uses IR target domains and filters for removal and counters,
characteristics for target value, instruction amounts for burn, and set membership
and damage/toughness amounts for wipe loss estimates. Wipes require an estimated
opposing loss value greater than our own. Damage estimates include public static
doublers; self-lethal player wipes are declined. X uses public mana production and
floating mana, selects the smallest affordable X with the best estimated result,
and reads generic cost reduction from static IR. It does not reproduce replacement
ordering, shields, future triggers or full rules evaluation. Core validates every
response.

Activated abilities use their actual index, tap/mana/sacrifice costs and programs.
Loot/draw and other usable abilities run after development in either main phase,
so tap abilities and equipment can be used before combat; expensive
source sacrifices are deferred until turn 20, while filtered sacrifice outlets
prefer tokens. Equipment moves only to a more valuable suitable creature.
Optional effects and modes inspect the current invocation's program. Combat keeps
the generic positive-power attack and public blocking heuristics. Mana planning
remains generic. Develop and the draw/decking witness's DrawOnly are the only
policy variants; neither selects a catalog.

`match-batch PROFILE OUTPUT` and `match-replay PROFILE FILE` are the sole batch
and completion-replay commands. They were already generic at the D1b-3 base.
The profile JSON's rotation and two homogeneous-deck cases remain useful data.
The throughput and ordinary-diagnostic adapters use the same current Strategy;
their old performance results retain their historical policy qualifications.

## Interaction accounting

One generic summary replaces every per-profile interaction report. Per-game
counts are aggregated in `report.json` under `interactions`:

- Instruction kinds and exact catalog sites have separate `chosen` and
  `committed` counts. A site includes Oracle identity, spell/activation/trigger,
  mode and cursor. A nonempty instruction batch counts once per stack/cursor,
  even when it contains many targets or resumes through several choices.
- `chosen` means an instruction belongs to the selected program of a cast spell,
  activated ability or placed trigger. It does not assert execution. `committed`
  means a nonempty instruction batch; a declined effect, no-op or fizzle may
  produce none. The report retains those sites as unreached.
- Trigger kinds count collection, placement and successful resolution separately.
- Keyword counts measure actual attack/block participants and damage-source
  occurrences, using frozen characteristics including tokens and grants. This
  measures involvement, not whether a keyword changed a result.
- `cards_used` counts casts, nonmana activations, land plays and mana activations.
  `unused_catalog_cards` lists every remaining catalog identity, including cards
  outside the default decks. `unreached_instruction_sites` includes the complete
  instruction, card display name and chosen count for every site without a
  committed batch. Names are report labels only.

The new policy regressions preserve existing counter/X/payment/optional-effect
checks and add renamed/resized-catalog invariance, changed IR damage amounts,
missing-creature readiness, friendly prevention targeting and precombat tap loot. The summary test
proves that choosing one mode neither counts the other mode nor fabricates a
committed instruction.

## Card-name search

The exact quoted-name scan uses names extracted from committed expected IR and
`grep -nFf`, with unit-test modules and test-only diagnostics excluded.
`generated/final-engine-verification/d1c/quoted-card-name-grep.txt` retains its
output. Five matches remain, all in `search_candidates.rs:113`–117: `Plains`,
`Island`, `Swamp`, `Mountain`, `Forest`. That match reads
`characteristics.subtypes`, implementing the basic-land subtype vocabulary
(CR 305.6); it is not a card-name branch. The new policy contains no card names,
Oracle-ID branches or definition-count fingerprints. `rg 'definitions.len\('
packages/game-engine/crates/sim/src` produces no output.

Historical scenario modules also retain card names inside human-readable witness
error messages; those are not decision branches or exact quoted-name literals.
