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

## Match-policy card identity scope

The no-card-identity claim applies to the match policy only. Fixed scenario
adapters such as `effects_games.rs:10–19`, `choices_games.rs`, `triggers_games.rs`
and others retain Oracle-ID constants. `main.rs:1120–1154` also retains constants
in `verify_source_catalog`. These fixture/admission checks do not feed the match
policy or select its actions.

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

A broader quoted-string scan found twelve lines: the five subtype literals above
and seven historical scenario error messages in the action, combat, damage/life
and trigger witness adapters. Its complete output is
`generated/final-engine-verification/d1c/card-name-substring-grep.txt`. None of
those messages selects an action or implements a card rule.


## Release profile re-baselines

The implementation source is `ad5b9e9bd856d329649466cf208f86a7dbbdd42f`. All twelve primary batches passed with 18 winners apiece: **216 winners, zero draws, zero engine faults, zero cap/resource truncations and zero batch failures**. Every saved game also passed an explicit `match-replay PROFILE FILE`: **216/216**.

The old numbers come from the original `d1b3-history2/batches/*/report.json` files in the prior worktree, read without modification. `generated/final-engine-verification/d1c/previous-outcomes.json` records their paths and SHA-256 hashes. Catalog digests and profile completion layouts are unchanged; these are policy re-baselines. Remaining profiles ran in three independent processes, without concurrent Cargo builds. Elapsed times are diagnostic and make no performance claim.

| Profile | Old winners/draws/caps | New winners/draws/caps | Explicit replay |
| --- | ---: | ---: | ---: |
| `commander-attachments-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-combat-keywords-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-data-admission-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-foundations-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-four-colors-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-keywords-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-mana-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-removal-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-sacrifice-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-staples-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-tokens-v1` | 18/0/0 | 18/0/0 | 18/18 |
| `commander-wipes-v1` | 18/0/0 | 18/0/0 | 18/18 |

Release engine `BUILD_ID`: `6549006961a17e7677129d431ab97a61e8ff362304f76d7e52b3039cef0f0c28`.


## Unreached instructions and core witnesses

The batch reports contain **195 unreached instruction sites across the twelve catalogs**, spanning **26 instruction kinds**. Shared catalog sites can occur in several profiles. Complete per-site instructions, names, mode/ability paths and chosen counts remain in each profile's `report.json`; `summary.json` gathers them by kind.

Across all batches, **30 of 35 instruction kinds** produced committed instruction batches. The five with none are:

- `each_opponent_sacrifice`: `sacrifice_tests::sacrifice_butcher_apnap_public_previous_choices_simultaneous_and_finite_cascade`.
- `lose_life_death_controller`: `wipes_tests::wipes_massacre_wurm_opponents_tokens_and_later_combat_death`.
- `modify_source_power_toughness`: `sacrifice_tests::sacrifice_feeder_self_no_counter_cannot_block_and_bloodthrone_cleanup`.
- `prevent_next_damage`: `four_colors_tests::reviewed_samite_furnace_bolt_creature`.
- `pump_set`: `wipes_tests::wipes_infest_fixed_set_anthem_and_counter_survive_later_creature_unaffected`.

Every missed instruction kind has existing passing core witnesses; **none lacks a witness**, and no core rules or core tests were changed. The following index covers any kind missed by at least one profile. References are test functions in `packages/game-engine/crates/core/src/engine/`, except `damage_life_adversarial` in `crates/core/tests/`.

| Instruction kind | Missed sites across profiles | Passing core witnesses |
| --- | ---: | --- |
| `change_life_from_target` | 1 | [`removal_tests::swords_commander_equipment_lki_precedes_exile_return_and_public_exile_survives_restore`](../packages/game-engine/crates/core/src/engine/removal_tests.rs); [`removal_tests::feed_swarm_token_zero_mv_commander_printed_mv_and_opponent_filter`](../packages/game-engine/crates/core/src/engine/removal_tests.rs) |
| `counter_stack_object` | 13 | [`four_colors_tests::counterspell_commander_grave_return_and_tax_survive_countering`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs); [`stack_costs_tests::counter_event_consumes_exact_spell_membership_and_actual_physical_successor`](../packages/game-engine/crates/core/src/engine/stack_costs_tests.rs); [`trigger_tests::triggered_stack_targets_are_abilities_and_countering_moves_no_source_card`](../packages/game-engine/crates/core/src/engine/trigger_tests.rs) |
| `create_creature_tokens` | 11 | [`tokens_tests::source_backed_token_spells_and_enters_triggers_define_exact_characteristics`](../packages/game-engine/crates/core/src/engine/tokens_tests.rs); [`sacrifice_tests::sacrifice_requiem_nonspirit_card_token_and_spirit_filter`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs) |
| `create_tokens_for` | 1 | [`removal_tests::beast_within_land_rock_and_own_permanent_controller_tokens_and_mana_windows`](../packages/game-engine/crates/core/src/engine/removal_tests.rs); [`removal_tests::swan_counters_pacifism_and_day_with_controller_bird_excludes_creature_equipment`](../packages/game-engine/crates/core/src/engine/removal_tests.rs) |
| `deal_damage` | 13 | [`four_colors_tests::lightning_bolt_complete_any_target_and_furnace_doubling`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs); [`four_colors_tests::reviewed_prodigal_bounced_in_response_ping_uses_lki_with_furnace`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs) |
| `destroy_permanent` | 3 | [`removal_tests::disenchant_equipment_removes_bonus_keeps_creature`](../packages/game-engine/crates/core/src/engine/removal_tests.rs); [`removal_tests::reclamation_sage_may_declined_after_target_selection`](../packages/game-engine/crates/core/src/engine/removal_tests.rs) |
| `destroy_target_creature` | 1 | [`staples_tests::murder_destroy_fizzle_and_simultaneous_day_lookback`](../packages/game-engine/crates/core/src/engine/staples_tests.rs) |
| `discard_cards` | 13 | [`actions_tests::sacrifice_source_ability_has_own_stack_identity_and_historical_reference`](../packages/game-engine/crates/core/src/engine/actions_tests.rs) |
| `draw_cards` | 28 | [`actions_tests::sacrifice_source_ability_has_own_stack_identity_and_historical_reference`](../packages/game-engine/crates/core/src/engine/actions_tests.rs); [`staples_tests::draw_life_programs_and_continuous_anthem_layer_7c`](../packages/game-engine/crates/core/src/engine/staples_tests.rs) |
| `each_opponent_sacrifice` | 2 | [`sacrifice_tests::sacrifice_butcher_apnap_public_previous_choices_simultaneous_and_finite_cascade`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs) |
| `exile_permanent` | 1 | [`removal_tests::swords_commander_equipment_lki_precedes_exile_return_and_public_exile_survives_restore`](../packages/game-engine/crates/core/src/engine/removal_tests.rs); [`removal_tests::revoke_exiles_equipment_and_sage_chupacabra_destroy_legal_trigger_targets`](../packages/game-engine/crates/core/src/engine/removal_tests.rs) |
| `gain_life` | 5 | [`sacrifice_tests::sacrifice_day_simultaneous_deaths_artist_zulaport_bastion_butcher_ordering`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs); [`damage_life_adversarial::soul_feast_self_temporarily_zero_then_gain_precedes_any_sba_and_snapshot_replay`](../packages/game-engine/crates/core/tests/damage_life_adversarial.rs) |
| `lose_life` | 6 | [`sacrifice_tests::sacrifice_day_simultaneous_deaths_artist_zulaport_bastion_butcher_ordering`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs); [`damage_life_adversarial::soul_feast_self_temporarily_zero_then_gain_precedes_any_sba_and_snapshot_replay`](../packages/game-engine/crates/core/tests/damage_life_adversarial.rs) |
| `lose_life_death_controller` | 1 | [`wipes_tests::wipes_massacre_wurm_opponents_tokens_and_later_combat_death`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs); [`wipes_tests::wurm_simultaneous_death_looks_back`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs) |
| `modify_power_toughness` | 44 | [`four_colors_tests::reviewed_giant_growth_lost_after_bounce_and_recast`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs); [`effects_tests::signed_modifiers_are_additive_fixed_incarnation_and_cleanup_is_atomic`](../packages/game-engine/crates/core/src/engine/effects_tests.rs) |
| `modify_source_power_toughness` | 2 | [`sacrifice_tests::sacrifice_feeder_self_no_counter_cannot_block_and_bloodthrone_cleanup`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs) |
| `move_set` | 3 | [`wipes_tests::wipes_planar_cleansing_nonlands_rancor_return`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs); [`wipes_tests::wipes_final_judgment_exiles_tokens_and_commander_without_dies`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs); [`wipes_tests::wipes_evacuation_combat_commander_choices_token_aura_and_equipment`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs); [`wipes_tests::wipes_aetherize_only_attackers_and_tranquility_restores_attack`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs) |
| `prevent_next_damage` | 22 | [`four_colors_tests::reviewed_samite_furnace_bolt_creature`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs); [`effects_tests::fog_and_shield_orders_preserve_distinct_budgets_and_no_actual_zero_damage`](../packages/game-engine/crates/core/src/engine/effects_tests.rs); [`choices_tests::allocation_requires_complete_forced_vector_and_rejects_atomically`](../packages/game-engine/crates/core/src/engine/choices_tests.rs) |
| `pump_set` | 2 | [`wipes_tests::wipes_infest_fixed_set_anthem_and_counter_survive_later_creature_unaffected`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs); [`wipes_tests::wipes_massacre_wurm_opponents_tokens_and_later_combat_death`](../packages/game-engine/crates/core/src/engine/wipes_tests.rs) |
| `put_on_top_owner_library` | 6 | [`commander_damage_library_tests::ordinary_top_library_instruction_precedes_existing_order_and_draws_once`](../packages/game-engine/crates/core/src/engine/commander_damage_library_tests.rs) |
| `put_plus_one_counters` | 1 | [`tokens_tests::counter_combines_in_layer7c_and_is_lost_on_zone_change`](../packages/game-engine/crates/core/src/engine/tokens_tests.rs); [`tokens_tests::enters_counter_targets_and_source_bound_counter`](../packages/game-engine/crates/core/src/engine/tokens_tests.rs) |
| `return_aura_from_graveyard` | 2 | [`attachments_tests::rancor_returns_after_bounce_and_its_return_can_be_stifled`](../packages/game-engine/crates/core/src/engine/attachments_tests.rs) |
| `return_own_graveyard_creature_to_hand` | 1 | [`staples_tests::recursion_targets_own_cards_optional_yes_no_and_commander`](../packages/game-engine/crates/core/src/engine/staples_tests.rs) |
| `return_to_owner_hand` | 7 | [`four_colors_tests::bounce_and_tuck_commanders_offer_both_destinations_and_forget_counters`](../packages/game-engine/crates/core/src/engine/four_colors_tests.rs) |
| `sacrifice_damage` | 4 | [`sacrifice_tests::sacrifice_fling_frozen_equipment_power_furnace_and_paid_cost_snapshot`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs); [`sacrifice_tests::sacrifice_mogg_arsonist_optional_and_siege_goblin_filter`](../packages/game-engine/crates/core/src/engine/sacrifice_tests.rs) |
| `search_basic_lands` | 2 | [`staples_tests::search_zero_one_two_private_handles_shuffle_and_pending_restore`](../packages/game-engine/crates/core/src/engine/staples_tests.rs) |

Static cost reduction and unbounded-X legality also retain the existing `wipes_blasphemous_act_reduction_floor_eight_and_twelve_and_unpayable` and `wipes_x_has_no_fixed_width_ceiling_and_unpayable_cast_reverses` witnesses. The simulator estimates payments; core owns legality.


## Cards not cast or used

Every catalog card not cast, activated, played as a land or used for mana is listed in its profile report under `interactions.unused_catalog_cards`. This includes historical catalog cards outside the default decks. The following are the omitted cards actually present in the default decks:

- `commander-attachments-v1`: Queen's Commission, Spore Swarm, Midnight Haunting, Birds of Paradise.
- `commander-four-colors-v1`: Fencing Ace, Crazed Skirge, Samite Healer.
- `commander-removal-v1`: Unholy Strength, Soul Feast, Slaughterhouse Bouncer, Unsummon, Goblin Instigator, Swords to Plowshares.
- `commander-sacrifice-v1`: Fling, Healer's Hawk, Hornet Queen, Lightning Bolt, Serra Angel, Goliath Sphinx, Beast Within, Greenwood Sentinel, Soul Feast, Slaughterhouse Bouncer, Crested Herdcaller, Costly Plunder, Explosive Vegetation, Butcher of Malakir, Spidersilk Armor, Gallant Cavalry, Sign in Blood, Elvish Visionary, Counterspell, Beetleback Chief, Chimney Rabble, Alley Strangler.
- `commander-wipes-v1`: Bone Splinters, Hornet Queen, Night's Whisper, Soul Feast, Dragon Sniper, Massacre Wurm, Rancor, Explosive Vegetation, Planar Cleansing, Infest, Tranquility, Evacuation, Vampire Nighthawk.

No default-deck card was omitted in: `commander-combat-keywords-v1`, `commander-data-admission-v1`, `commander-foundations-v1`, `commander-keywords-v1`, `commander-mana-v1`, `commander-staples-v1`, `commander-tokens-v1`. All twenty catalog cards in the data-only admission profile were used, including all thirteen demonstration additions.


## Final verification

Pinned toolchain: Rust 1.99.0 and Node 26.11.1 from the requested toolchain
prefix. Implementation source: `ad5b9e9bd856d329649466cf208f86a7dbbdd42f`.
All commands below exited **0**:

- `npm run check:engine`: formatting, Clippy with warnings denied, **902 Rust
  tests passed, zero failed, 18 ignored**, then **56 Node checks
  passed, zero failed**.
- `cargo build --release --locked -p mtg-server` and
  `cargo build --release --locked -p mtg-sim`.
- `MTG_SERVER_BIN=$PWD/target/release/mtg-server npm --prefix packages/game-server/sdk run verify --workspaces=false`:
  **47 tests passed**, strict TypeScript, all **32 response variants** and five
  execution-status variants. Its reported scope remains schema/generated types
  and mock HTTP. SDK `generated/` was copied into ignored evidence and removed
  from `packages/game-server/sdk/`.
- All twelve `match-batch PROFILE OUTPUT` commands and all **216 explicit
  `match-replay PROFILE FILE` commands**. Each batch also performs its internal
  independent replay and retains its original completion and checkpoint guards.

Evidence is ignored under `generated/final-engine-verification/d1c/`:
`gates.json`, `check-engine.log`, `sdk/verification.json`, `results.json`,
`summary.json`, the two card-name scan outputs, `instruction-witnesses.json`,
`previous-outcomes.json`, and twelve profile directories with all game journals,
reports and explicit replay results. No generated artifacts are committed.

Earlier attempts remain separate: the first engine run was stopped after two
source-identity guards correctly rejected files edited during that run; those
same unchanged guards passed after rebuilding. The next full Rust phase passed,
but the Node phase found the fresh worktree lacked its declared `ajv` dependency.
After `npm ci --workspace packages/game-engine --include-workspace-root`, all
56 Node checks passed, followed by the complete end-to-end engine rerun above.
The old logs are retained and are not counted as passing final runs. The serial
batch dispatcher's partly started combat-keywords prefix was retained under
`attempts/` when scheduling the remaining profiles in three processes; only its
fresh, complete batch is counted.

There are **no unresolved D1c verification failures or missing instruction
witnesses**. Strategy remains intentionally coarse, and the unused cards and
unreached sites above are explicit. No core rules bug was fixed or core behavior
changed. Independent implementation review and merge remain pending.

## D1c review follow-up

The follow-up starts at `f02eae7` on `feat/cards-data-d1c-fixes`. Implementation
source is `3493f0f458bb1155a14811472eb08d5632ea2f16`. It changes only
simulator policy code, policy tests and this report. The original D1c results
above remain historical; the comparison below uses the unchanged reports in
`/home/josh/.t3/worktrees/mtg-ecosystem/t3-0fd71f2b/generated/final-engine-verification/d1c/`.

Prevention readiness now reads announced public damage targets and IR amounts,
and pending combat damage to our seat or creatures. Instant-speed defensive
activations can run outside our own main phase. Reactive spell discovery scans
all modes, and mode selection prefers the ready reactive mode over an otherwise
ready life-gain mode. Reactive modal cards are held until a reactive mode is
ready, and reactive tap sources are reserved from attacking. Prevention targets the threatened friendly entity. Ordinary
post-damage combat priority does not by itself prepare a new shield. These are
public-information estimates, not a replacement for core damage evaluation.

Filtered sacrifice costs accept tokens, permanents facing announced lethal
damage or removal/wipes, and sacrifices that enable lethal damage or a favorable
removal trade. Selection prefers an endangered permanent and uses the same
candidate preference for sacrificed-power estimates. Source-pump outlets are
used before combat or while the source participates in combat. Mana sources,
land search and death payoffs develop before commanders; other hand permanents
develop before taxed commander recasts. Ordinary instruction spells can use
remaining precombat mana. The planner spends dedicated mana sources before
spending a flexible source on a color another source can supply.

The audit of the sacrifice profile's 22 omitted cards found fourteen permanents
and eight instruction spells. All have supported generic/colored mana costs;
only two also require an additional sacrifice. Creature death triggers do not
form casting-time target requirements. The old sacrifice gate, commander-first
spending, postcombat-only instruction scheduling and flexible-mana allocation
were generic policy restrictions, covered by regressions here. The retained
journal survey shows one omitted card bottomed during opening setup and no
recorded discards of these 22; only game 00 retains intermediate checkpoints.
This does not establish a complete causal explanation for every noncast card,
or prove that a card was never drawn. Finite draw order, affordability, targets
and game duration still affect coverage. The machine-readable audit is
`d1c-fixes/baseline-investigation.json` under the evidence root.

Optional effects inspect the bound target's slot. Multi-target entity requests
prefer distinct legal candidates, with repetition only as a fallback. X's mana
upper estimate examines all listed production alternatives, taking at most the
largest single production per source, and no longer counts nonmana permanents
as mana. Core still admits exactly one permanent mana ability per source; its
public activation response selects a source, not an ability index. Current token
definitions explicitly clear activated and mana abilities, and their public
rows expose no ability programs. The policy no longer excludes tokens by a
blanket flag, but cannot manufacture token abilities or multi-ability mana
activation support within this simulator-only change.

Nine new policy regressions cover defensive activation, modal prevention,
sacrifice costs and profitable trades, source pumping, death-payoff development,
flexible mana, optional slots, distinct targets, precombat spell scheduling and
reservation of defensive resources.
No existing assertion, guard, completion layout, deck or capacity was weakened.

<!-- d1c-fixes-results -->

## Follow-up verification and comparison

Release `BUILD_ID`: `f3ad25c085463794c9e4603ea7a94d900bd126d3e2aaa6b6ee5003017000dd0f`. Implementation source: `3493f0f458bb1155a14811472eb08d5632ea2f16`.

The final engine gate passed: **911 Rust tests, zero failed, 18 ignored; 56 Node checks, zero failed**. Formatting and Clippy with warnings denied passed. The focused policy run passed **15/15**, including nine new regressions. The release simulator build exited 0.

All twelve batches and **216/216 explicit replays** passed, in addition to each batch's internal replay. Every profile retains **18 winners, zero draws, zero caps and zero faults**: **216 winners total**, preserving decisiveness. Every batch manifest differs from its baseline only in build identity; catalog digests, seeds, completion layouts and guards are unchanged. Three independent binary processes ran batches; Cargo builds were serial. Timings are diagnostics, not performance claims.

| Profile | Winners/draws/caps before → after | Uncast nonlands | Zero-commit kinds | Kinds with missed sites | Replay |
| --- | --- | ---: | ---: | ---: | ---: |
| `commander-attachments-v1` | 18/0/0 → 18/0/0 | 4 → 5 | 1 → 1 | 3 → 4 | 18/18 |
| `commander-combat-keywords-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-data-admission-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 0 → 0 | 0 → 0 | 18/18 |
| `commander-foundations-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-four-colors-v1` | 18/0/0 → 18/0/0 | 3 → 3 | 1 → 0 | 4 → 3 | 18/18 |
| `commander-keywords-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-mana-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-removal-v1` | 18/0/0 → 18/0/0 | 6 → 6 | 2 → 1 | 9 → 7 | 18/18 |
| `commander-sacrifice-v1` | 18/0/0 → 18/0/0 | 22 → 15 | 4 → 3 | 15 → 9 | 18/18 |
| `commander-staples-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-tokens-v1` | 18/0/0 → 18/0/0 | 0 → 0 | 6 → 5 | 8 → 8 | 18/18 |
| `commander-wipes-v1` | 18/0/0 → 18/0/0 | 13 → 10 | 6 → 5 | 17 → 13 | 18/18 |

“Zero-commit kinds” means no committed instruction batch anywhere in that profile. “Kinds with missed sites” also includes partly reached kinds with another catalog site still uncommitted. Neither metric asserts that every catalog card belongs to the default decks. Uncast lists below cover default-deck nonlands only; the existing card-use accounting implies a prior cast for nonland usages in these admitted pools.

### Instruction kinds with no committed batch

| Profile | Before | After |
| --- | --- | --- |
| `commander-attachments-v1` | `prevent_next_damage` | `return_aura_from_graveyard` |
| `commander-combat-keywords-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-data-admission-v1` | None | None |
| `commander-foundations-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-four-colors-v1` | `prevent_next_damage` | None |
| `commander-keywords-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-mana-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-removal-v1` | `prevent_next_damage`, `return_to_owner_hand` | `return_to_owner_hand` |
| `commander-sacrifice-v1` | `each_opponent_sacrifice`, `modify_source_power_toughness`, `prevent_next_damage`, `return_aura_from_graveyard` | `each_opponent_sacrifice`, `modify_source_power_toughness`, `return_aura_from_graveyard` |
| `commander-staples-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-tokens-v1` | `counter_stack_object`, `deal_damage`, `discard_cards`, `prevent_next_damage`, `put_on_top_owner_library`, `return_to_owner_hand` | `counter_stack_object`, `deal_damage`, `discard_cards`, `put_on_top_owner_library`, `return_to_owner_hand` |
| `commander-wipes-v1` | `each_opponent_sacrifice`, `lose_life_death_controller`, `modify_source_power_toughness`, `prevent_next_damage`, `pump_set`, `return_aura_from_graveyard` | `each_opponent_sacrifice`, `lose_life_death_controller`, `modify_source_power_toughness`, `pump_set`, `return_aura_from_graveyard` |

Globally, zero-commit kinds fall from **5 to 4**. `prevent_next_damage` rises from **0 to 154 committed batches**. The remaining global gaps are `each_opponent_sacrifice`, `lose_life_death_controller`, `modify_source_power_toughness`, `pump_set`. Their passing core witnesses remain listed above; these are match-policy coverage gaps, not missing core witnesses.

### Default-deck nonlands never cast

| Profile | Before | After |
| --- | --- | --- |
| `commander-attachments-v1` | Birds of Paradise, Midnight Haunting, Queen's Commission, Spore Swarm | Alley Strangler, Birds of Paradise, Midnight Haunting, Queen's Commission, Spore Swarm |
| `commander-combat-keywords-v1` | None | None |
| `commander-data-admission-v1` | None | None |
| `commander-foundations-v1` | None | None |
| `commander-four-colors-v1` | Crazed Skirge, Fencing Ace, Samite Healer | Crazed Skirge, Fencing Ace, Samite Healer |
| `commander-keywords-v1` | None | None |
| `commander-mana-v1` | None | None |
| `commander-removal-v1` | Goblin Instigator, Slaughterhouse Bouncer, Soul Feast, Swords to Plowshares, Unholy Strength, Unsummon | Deadly Recluse, Glorious Anthem, Goblin Instigator, Slaughterhouse Bouncer, Swords to Plowshares, Unsummon |
| `commander-sacrifice-v1` | Alley Strangler, Beast Within, Beetleback Chief, Butcher of Malakir, Chimney Rabble, Costly Plunder, Counterspell, Crested Herdcaller, Elvish Visionary, Explosive Vegetation, Fling, Gallant Cavalry, Goliath Sphinx, Greenwood Sentinel, Healer's Hawk, Hornet Queen, Lightning Bolt, Serra Angel, Sign in Blood, Slaughterhouse Bouncer, Soul Feast, Spidersilk Armor | Alley Strangler, Beetleback Chief, Butcher of Malakir, Chimney Rabble, Costly Plunder, Counterspell, Crested Herdcaller, Gallant Cavalry, Goliath Sphinx, Healer's Hawk, Hornet Queen, Serra Angel, Sign in Blood, Soul Feast, Spidersilk Armor |
| `commander-staples-v1` | None | None |
| `commander-tokens-v1` | None | None |
| `commander-wipes-v1` | Bone Splinters, Dragon Sniper, Evacuation, Explosive Vegetation, Hornet Queen, Infest, Massacre Wurm, Night's Whisper, Planar Cleansing, Rancor, Soul Feast, Tranquility, Vampire Nighthawk | Bone Splinters, Butcher of Malakir, Dragon Sniper, Evacuation, Ferocious Pup, Hornet Queen, Infest, Massacre Wurm, Night's Whisper, Vampire Nighthawk |

Sacrifice improves from **22 to 15** omitted default-deck cards. Fling commits **0 → 2** damage batches; Goblin Bombardment activates **0 → 4** times. Aggregate `sacrifice_damage` commitments rise **8 → 17**. Butcher of Malakir remains uncast and Bloodthrone Vampire remains unactivated in natural batches, despite their generic policy regressions. These remain explicit coverage limitations. Attachments regresses from **4 to 5** omitted cards, and its `return_aura_from_graveyard` kind is now uncommitted; its winner and replay counts are unchanged.

Full before/after lists of kinds with missed sites, newly cast/newly omitted cards, original-report paths and SHA-256 hashes are in `generated/final-engine-verification/d1c-fixes/comparison.json`. The self-contained `report.html` exposes these lists per profile. Each profile directory retains all eighteen journals, `report.json`, unchanged guards in `manifest.json`, and eighteen explicit replay result files. `source-inputs.json`, `gates.json`, `policy-tests-final.log`, `check-engine.log` and `build-sim.log` bind the implementation and exact gate results.

Two interrupted intermediate attempts remain under `attempts/pre-final/` and `attempts/pre-reservation/`; neither is counted as a final pass. The latter preserved a 910-test Rust pass, incomplete Node/replay work, and prevention-zero reports that exposed the resource-reservation gap. No assertion or guard was weakened to obtain the final results.

### Host-only audit of the remaining casting gaps

A separate native audit reconstructed all eighteen original and eighteen corrected
sacrifice games from their raw accepted journals. **36/36 terminal semantic digests
matched; zero audit failures.** This does not replace the standard batch/replay
qualification above or claim checkpoint/build replay of the older executable.
It only inspects the acting player's permitted observations, never supplies facts
to the policy, and uses the original and corrected mana planners on the same
observed mana costs. Mana funding alone does not assert valid targets or a
payable additional sacrifice.

| Original 22 omitted cards | Before | After |
| --- | ---: | ---: |
| Never seen in hand at own empty-stack main priority | 7 | 6 |
| Seen there, but no window funded by the planner | 7 | 5 |
| At least one such window with mana funding | 8 | 11 |

The old and corrected planners produced identical **per-card payable-window
totals** in these audits. The flexible-mana allocation fix is covered by its
regression, but was not identified as a cause of this cohort's omissions.
Actual selected actions show competition from other development, while reactive
and sacrifice spells still need their specific opportunities. The corrected
cohort casts seven of the original omissions; six remaining omissions never
appeared at own main priority, five had no funded main window, and four still
competed with development or needed reactive/additional-cost opportunities.

Butcher appeared at own main priority in **two games in each cohort**, with
**zero mana-plan-funded windows** in either planner. Its casting regression
proves development priority when its cost is funded. Bloodthrone had **zero
observed token-fodder priority windows** in either cohort; its token-based policy
regression is separate from this remaining natural-game coverage gap.

Actual cost sacrifices increased **36 → 52**; all sacrifices increased
**37 → 54**. Cost counts include successful `Sacrificed` outcomes under payment,
nonmana activation or mana activation causes and exclude resolution sacrifices.
These improve on `f02eae7`; Fling's two damage commitments and 52 paid sacrifices
still fall short of the review's earlier per-card figures of twelve and 57.

`audit-before.json`, `audit-after.json`, `casting-audit-summary.json` and
`audit-results.json` retain the counts, every audited card's observed windows,
selected actions, semantic checks and exact commands. `audit-priority.rs` and
`audit-build-command.json` retain the standalone audit implementation and build
inputs. No core, server, compiler or contract source was edited for this audit.
