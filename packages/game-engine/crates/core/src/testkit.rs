//! Integration-test harness surface, enabled only by the non-default `testkit`
//! feature. Test-only crates (`crates/core/tests`, `crates/witness`) enable it;
//! production crates never do (`scripts/engine-structure-check.py`). It exposes
//! the engine namespace that in-source tests previously saw through
//! `use super::*`, so scenario and witness binaries build outside `--lib`.
pub use crate::engine::*;
// Engine modules shadow same-named contracts modules, as inside the engine.
pub use crate::engine::{
    impulse_play, mana_tap_observer, optional_block, proliferate, resolution_mana, target_color,
};
use crate::engine::{
    keywords::KeywordSource, object_facts::DerivedObject, payments::AnnouncementPayment,
    phasing::PhaseSelection, stored_history::StoredPermittedView,
};
pub use crate::{build_id, formats, query};
pub use mtg_contracts::kir::{Amount, Context, EventPlayerRole, PlayerExpr, Ref, SetExpr, Verb};
pub use mtg_contracts::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Crate-private generic `History` methods that white-box test binaries call.
pub trait HistoryTestkit<T> {
    fn canonical_entry_bytes(&self, index: usize) -> Result<&[u8], String>
    where
        T: Serialize;
    fn canonical_entry_bytes_with(
        &self,
        index: usize,
        initialize: impl FnOnce(&T) -> Result<Box<[u8]>, String>,
    ) -> Result<&[u8], String>;
    fn eq_reflexive(&self, other: &History<T>) -> bool
    where
        T: Eq;
}
impl<T> HistoryTestkit<T> for History<T> {
    fn canonical_entry_bytes(&self, index: usize) -> Result<&[u8], String>
    where
        T: Serialize,
    {
        History::canonical_entry_bytes(self, index)
    }
    fn canonical_entry_bytes_with(
        &self,
        index: usize,
        initialize: impl FnOnce(&T) -> Result<Box<[u8]>, String>,
    ) -> Result<&[u8], String> {
        History::canonical_entry_bytes_with(self, index, initialize)
    }
    fn eq_reflexive(&self, other: &History<T>) -> bool
    where
        T: Eq,
    {
        History::eq_reflexive(self, other)
    }
}

// ---- generated forwarding traits ----
/// Crate-private `Budget` methods that white-box test binaries call.
pub trait BudgetTestkit {
    fn consume(&mut self) -> bool;
}
impl BudgetTestkit for Budget {
    fn consume(&mut self) -> bool {
        Budget::consume(self)
    }
}

/// Crate-private `CommanderLedger` methods that white-box test binaries call.
pub trait CommanderLedgerTestkit {
    fn record_cast(&mut self, card: CardId, caster: PlayerId, from: Zone) -> Result<(), String>;
    fn record_damage(
        &mut self,
        card: CardId,
        recipient: PlayerId,
        combat: bool,
        actual: &Quantity,
    ) -> Result<(), String>;
}
impl CommanderLedgerTestkit for CommanderLedger {
    fn record_cast(&mut self, card: CardId, caster: PlayerId, from: Zone) -> Result<(), String> {
        CommanderLedger::record_cast(self, card, caster, from)
    }
    fn record_damage(
        &mut self,
        card: CardId,
        recipient: PlayerId,
        combat: bool,
        actual: &Quantity,
    ) -> Result<(), String> {
        CommanderLedger::record_damage(self, card, recipient, combat, actual)
    }
}

/// Crate-private `DelayedChapterRegistration` methods that white-box test binaries call.
pub trait DelayedChapterRegistrationTestkit {
    fn chapter(&self) -> Result<u32, String>;
}
impl DelayedChapterRegistrationTestkit for DelayedChapterRegistration {
    fn chapter(&self) -> Result<u32, String> {
        DelayedChapterRegistration::chapter(self)
    }
}

/// Crate-private `Game` methods that white-box test binaries call.
pub trait GameTestkit {
    fn action_hand(&self, player: PlayerId) -> Vec<ObjectId>;
    fn actions_request(&self) -> Result<Option<Request>, String>;
    fn advance_internal<const REPORT: bool, const COALESCE: bool>(
        &mut self,
        budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance;
    fn allocated_capacity_count(&self) -> usize;
    fn announce_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String>;
    fn announce_priority_spell(&mut self, player: PlayerId, card: CardId) -> Result<(), String>;
    fn announced_graveyard_group(
        &self,
        caster: PlayerId,
        chooser: PlayerId,
        maximum: u8,
        entities: &[TargetEntity],
    ) -> Result<(PlayerId, Vec<ObjectId>), String>;
    fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool>;
    fn apply_departure(&mut self, player: PlayerId) -> Result<(), String>;
    fn apply_noncombat_damage(
        &mut self,
        damage: &NoncombatDamageOccurrence,
    ) -> Result<EventOutcome, String>;
    fn apply_occurrence(&mut self, proposed: &ProposedEvent) -> Result<EventOutcome, String>;
    fn apply_occurrence_with_prior(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String>;
    fn attachment_sbas(&self, occurrences: &mut Vec<ProposedEvent>) -> Result<(), String>;
    fn attachment_target_type_valid(
        &self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<bool, String>;
    fn back_face_sequence(&self, object: ObjectId) -> Option<u64>;
    fn begin_trigger_placement(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<bool, String>;
    fn blank_view(&self, viewer: PlayerId) -> Result<PermittedView, String>;
    fn can_block(&self, blocker: ObjectId, attacker: ObjectId) -> bool;
    fn can_pay(&self, player: PlayerId, cost: &ManaCost) -> Result<bool, String>;
    fn capture_trigger_source_handles(
        &mut self,
        object: ObjectId,
    ) -> Result<Vec<publish::StoredObjectHandle>, String>;
    fn cascade_charge(&self) -> usize;
    fn cast_is_prohibited(&self, player: PlayerId) -> Result<bool, String>;
    fn casting_cost(&self, card: CardId, caster: PlayerId, from: Zone) -> Result<ManaCost, String>;
    fn check_history_capacity(&mut self) -> Result<(), String>;
    fn check_resolution_condition(
        &mut self,
        stack: &StackInstance,
    ) -> Result<Option<InterveningCheck>, String>;
    fn check_stack_capacity(&mut self, additional: usize) -> Result<(), String>;
    fn collect_cast_triggers(&mut self, entry: &StackInstance) -> Result<(), String>;
    fn collect_triggers(
        &mut self,
        cause: &BatchCause,
        before: &Game,
        occurrences: &[OccurrenceResult],
    ) -> Result<(), String>;
    fn color_layer(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String>;
    fn combat_step(&mut self) -> Result<(), String>;
    fn commit(
        &mut self,
        proposed: ProposedEvent,
        cause: BatchCause,
    ) -> Result<EventOutcome, String>;
    fn commit_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String>;
    fn complete_test_mana(&mut self);
    fn control_before(&self) -> Result<Option<Box<ControlBefore>>, String>;
    fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId>;
    fn convert_response(
        &self,
        viewer: PlayerId,
        response: &PlayerResponse,
    ) -> Result<Response, String>;
    fn copiable_definition(
        &self,
        object: impl std::borrow::Borrow<ObjectId>,
    ) -> Result<&Definition, String>;
    fn cost_lock_charge(&self) -> usize;
    fn counter_placement_candidates(&self) -> Vec<CounterPlacementEffectKey>;
    fn counter_placement_request(&self) -> Result<Option<Request>, String>;
    fn creature_or_planeswalker(&self, id: ObjectId) -> bool;
    fn creatures(&self, player: PlayerId, attacking: bool) -> Vec<ObjectId>;
    fn current(&self, id: ObjectId) -> Result<&Object, String>;
    fn current_keywords(&self, object: ObjectId) -> Vec<KeywordAbility>;
    fn damage_allocation_request(&self) -> Result<Option<Request>, String>;
    fn damage_effect_group(
        &self,
    ) -> Result<Option<(PlayerId, TargetEntity, Vec<DamageEffectCandidate>)>, String>;
    fn damage_effect_request(&self) -> Result<Option<Request>, String>;
    fn damage_effect_step(&mut self) -> Result<(), String>;
    fn damage_keywords(
        &mut self,
        actual: &[ActualDamage],
        arg2: Option<&BTreeMap<DamageId, ObjectBefore>>,
    ) -> Result<Vec<LifelinkGain>, String>;
    fn damage_limit(&self) -> usize;
    fn damage_transaction(&self) -> Result<&DamageTransaction, String>;
    fn definition(&self, card: CardId) -> Result<&Definition, String>;
    fn derived_characteristics(&self, object: ObjectId) -> Result<Characteristics, String>;
    fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String>;
    fn effect_decision_context(
        &self,
        viewer: PlayerId,
        effect: &EffectKey,
    ) -> Option<DecisionContext>;
    fn effect_handle(&self, viewer: PlayerId, key: &EffectKey) -> Result<EffectHandle, String>;
    fn effect_limit(&self) -> usize;
    fn eligible_players(&self, domain: TargetDomain, controller: PlayerId) -> Vec<PlayerId>;
    fn eligible_stack_targets_for(
        &self,
        spec: &TargetSpec,
        controller: PlayerId,
        exclude: Option<StackId>,
    ) -> Vec<StackId>;
    fn enter_turn_step(&mut self) -> Result<(), String>;
    fn entity_legal(&self, target: TargetEntity) -> bool;
    fn entry_cause_for_test(&self) -> BatchCause;
    fn entry_copy_origin(&self, object: ObjectId) -> Option<(ObjectId, CardId)>;
    fn entry_group_for_test(&self) -> EventGroup;
    fn entry_search_block(&self, frame: &Resolving) -> Result<u32, String>;
    fn event_group_limit(&self) -> usize;
    fn execute_damage_life_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String>;
    fn execute_kernel_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String>;
    fn execute_effect_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String>;
    fn execute_wipes_instruction(&mut self, frame: Resolving, i: Instruction)
    -> Result<(), String>;
    fn finish_committed_player_losses(&mut self, batch: &CommittedBatch) -> Result<bool, String>;
    fn grant_payload_charge(&self) -> usize;
    fn granted_land_mana_colors() -> [Color; 5];
    fn graveyard_target_legal(
        &self,
        object: &Object,
        player: PlayerId,
        domain: TargetDomain,
    ) -> bool;
    fn stored_handle_for(
        &self,
        viewer: PlayerId,
        id: ObjectId,
    ) -> Result<publish::StoredObjectHandle, String>;
    fn handle_for(&self, viewer: PlayerId, id: ObjectId) -> Result<ObjectHandle, String>;
    fn has_color(source: &Characteristics) -> bool;
    fn has_granted_land_mana(&self, land: ObjectId) -> Result<bool, String>;
    fn has_keyword<'a>(
        &self,
        object: impl Into<KeywordSource<'a>>,
        keyword: KeywordAbility,
    ) -> bool;
    fn has_land_play_allowance(&self, player: PlayerId) -> bool;
    fn has_restriction(&self, object: ObjectId, restriction: CreatureRestriction) -> bool;
    fn has_target_domain(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
    ) -> bool;
    fn history_charge(&self) -> usize;
    fn initial_query_task(&self, query: &Query) -> Result<Task, QueryRejection>;
    fn is_battlefield_creature(&self, id: ObjectId) -> bool;
    fn is_bestowed_spell(&self, object: ObjectId) -> bool;
    fn is_blocking_creature(&self, object: ObjectId) -> bool;
    fn is_live(&self, player: PlayerId) -> bool;
    fn is_spell_copy(&self, entry: &StackInstance) -> bool;
    fn known_library_positions(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<KnownLibraryPosition>, String>;
    fn land_entry_charge(&self) -> usize;
    fn land_entry_step(&mut self) -> Result<(), String>;
    fn lethal_requirement(&self, source: ObjectId, blocker: ObjectId) -> Result<Quantity, String>;
    fn library_prefix(&self, player: PlayerId, amount: &Quantity) -> Result<Vec<ObjectId>, String>;
    fn linked_hideaway_cards(&self, source: ObjectId, source_block: u32) -> Vec<ObjectId>;
    fn live_effect_keys(&self) -> Vec<EffectKey>;
    fn main_phase_action(&self, player: PlayerId) -> bool;
    fn mana_ability_used(&self, source: ObjectId) -> bool;
    fn mana_continuation_charge(&self) -> usize;
    fn matches_permanent_filter(
        &self,
        object: ObjectId,
        controller: PlayerId,
        f: &PermanentFilter,
    ) -> bool;
    fn may_play_exile(&self, player: PlayerId, object: ObjectId) -> bool;
    fn mixed_mana_choices(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<ManaAbilityIdentity>, String>;
    fn modes_request(&self) -> Result<Option<Request>, String>;
    fn move_object(&mut self, object: ObjectId, destination: Zone) -> Result<MoveOutcome, String>;
    fn move_object_for(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String>;
    fn next_turn_state(&self) -> Result<(TurnState, PlayerId), String>;
    fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String>;
    fn object_facts<'a>(&'a self, object: &'a Object) -> DerivedObject<'a>;
    fn object_target_legal(&self, entry: &StackInstance, object: ObjectId) -> bool;
    fn objects_in_set(&self, set: &ObjectSet, controller: PlayerId) -> Vec<ObjectId>;
    fn observation_work_units(&self, viewer: PlayerId) -> Result<u64, String>;
    fn offer_commander_sba(&mut self, resume: Frame) -> Result<bool, String>;
    fn opponent_land_mana_colors(&self, player: PlayerId) -> Result<Vec<Color>, String>;
    fn pay_announcement_costs(&mut self, cast: &Casting) -> Result<AnnouncementPayment, String>;
    fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, payments::PaymentError>;
    fn permanent_colors(
        &self,
        scope: PermanentColorScope,
        controller: PlayerId,
    ) -> Result<BTreeSet<Color>, String>;
    fn permanent_target_legal(
        &self,
        object: ObjectId,
        controller: PlayerId,
        spec: &TargetSpec,
    ) -> bool;
    fn permitted_decision_context(
        &self,
        viewer: PlayerId,
        request: Option<&PlayerRequest>,
    ) -> Option<DecisionContext>;
    fn phase_in_for_next_untap(
        &mut self,
        previous_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String>;
    fn phase_out_permanents(&mut self, selection: PhaseSelection) -> Result<EventOutcome, String>;
    fn physical_transform_mana_value(&self, object: ObjectId) -> Option<Quantity>;
    fn player_entity(
        &self,
        viewer: PlayerId,
        target: TargetEntity,
    ) -> Result<PlayerTargetEntity, String>;
    fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool;
    fn player_protected(&self, player: PlayerId) -> bool;
    fn preflight_trigger_emission(&mut self, additional: usize) -> Result<(), String>;
    fn prepare_combat_damage(&mut self, damage: Vec<DamageOccurrence>) -> Result<(), String>;
    fn prepare_noncombat_damage(
        &mut self,
        frame: Resolving,
        damage: NoncombatDamageOccurrence,
        block: u32,
    ) -> Result<(), String>;
    fn program(&self) -> Result<&SpellProgram, String>;
    fn program_for<'a>(&'a self, stack: &'a StackInstance) -> Result<&'a SpellProgram, String>;
    fn protected_from(&self, target: ObjectId, source: &Characteristics) -> bool;
    fn query_work_units(&self, query: &Query) -> Result<u64, String>;
    fn read_snapshot(prepared: Prepared, json: &str) -> Result<(Game, String), String>;
    fn read_snapshot_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<(Game, String), String>;
    fn reconcile_controllers(&mut self) -> Result<(), String>;
    fn record_entry_copy(&mut self, entering: ObjectId, donor: ObjectId) -> Result<(), String>;
    fn refresh_effect_handles(&mut self) -> Result<Vec<EffectKey>, String>;
    fn refresh_visibility(&mut self) -> Result<(), String>;
    fn remaining_graveyard_targets(&self, owner: PlayerId, cards: &[ObjectId]) -> Vec<ObjectId>;
    fn replacement_reveals(&self, object: ObjectId) -> bool;
    fn replay_to(
        prepared: Prepared,
        journal: &ReplayJournal,
        boundary: Option<u64>,
        mode: TraceMode,
    ) -> Result<Game, String>;
    fn request_header(&self, viewer: PlayerId) -> Result<Option<PlayerRequest>, String>;
    fn resolve_damage_effect_pair(
        &self,
        viewer: PlayerId,
        effect: &EffectHandle,
        damage: &DamageHandle,
    ) -> Result<(EffectKey, DamageId), String>;
    fn resolve_damage_handle(
        &self,
        viewer: PlayerId,
        handle: &DamageHandle,
    ) -> Result<DamageId, String>;
    fn resolve_entity(
        &self,
        viewer: PlayerId,
        target: &PlayerTargetEntity,
    ) -> Result<TargetEntity, String>;
    fn resolve_handle(&self, viewer: PlayerId, handle: &ObjectHandle) -> Result<ObjectId, String>;
    fn resolve_search_handle(
        &self,
        player: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String>;
    fn resolve_trigger_handle(
        &self,
        viewer: PlayerId,
        handle: &TriggerHandle,
    ) -> Result<TriggerId, String>;
    fn restore_hideaway_link(&mut self, receipt: HideawayLink) -> Result<(), String>;
    fn resulting_permission_matches(
        &self,
        permission: &CastPermission,
        stack: &StackInstance,
    ) -> Result<bool, String>;
    fn retain_source_facts(&mut self, fact: &ObjectBefore);
    fn retain_source_lki(&mut self, object: ObjectId) -> Result<(), String>;
    fn sacrifice_candidates(&self, player: PlayerId, filter: &SacrificeFilter) -> Vec<ObjectId>;
    fn search_lands(&self, player: PlayerId) -> Vec<ObjectId>;
    fn searched_land_force_tapped(&self, player: PlayerId) -> Result<bool, String>;
    fn selected_payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
    ) -> Result<BTreeMap<Color, Quantity>, String>;
    fn selected_program<'a>(
        &self,
        root: &'a SpellProgram,
        mode: Option<u32>,
    ) -> Result<&'a SpellProgram, String>;
    fn shield_limit(&self, effect: &ResolvedEffect) -> Result<Quantity, String>;
    fn source_facts(&self, entry: &StackInstance) -> Result<ObjectBefore, String>;
    fn stabilization_wave_with_legends(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
        kept: Option<Vec<ObjectId>>,
    ) -> Result<(), String>;
    fn stabilize(&mut self) -> Result<(), String>;
    fn stack_handle_for(&self, viewer: PlayerId, id: StackId) -> Result<StackHandle, String>;
    fn stack_limit(&self) -> usize;
    fn stack_mana_value(&self, entry: &StackInstance) -> Result<Quantity, String>;
    fn start_work(&self, query: Query) -> Result<Work, QueryRejection>;
    fn static_damage_effects(&self) -> Vec<EffectKey>;
    fn static_modifications(&self, target: ObjectId) -> Vec<&StaticEffect>;
    fn step(&mut self) -> Result<(), String>;
    fn target_is_creature(&self, object: ObjectId) -> bool;
    fn targetable_by(&self, object: ObjectId, controller: PlayerId) -> bool;
    fn targetable_player(&self, player: PlayerId) -> bool;
    fn targeted_loss_authority(&self) -> Result<(BatchCause, PlayerId), String>;
    fn targeting_characteristics<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<std::borrow::Cow<'a, Characteristics>, String>;
    fn token_capacity(&self) -> u32;
    fn top(&self) -> Result<&StackInstance, String>;
    fn transition_turn_step(&mut self) -> Result<(), String>;
    fn trigger_order_ids(&self) -> Vec<TriggerId>;
    fn trigger_source_label(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Option<&str>;
    fn turn_up_linked_hideaway(
        &mut self,
        source: ObjectId,
        source_block: u32,
        card: ObjectId,
    ) -> Result<HideawayLink, String>;
    fn update_library_position_memory(&mut self, event: &ProposedEvent, actual: &EventOutcome);
    fn valid_set_effect_targets(
        &self,
        e: &ResolvedEffect,
        targets: &[ObjectId],
        set: Option<&ObjectSet>,
    ) -> bool;
    fn validate_actions_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>>;
    fn validate_actions_state(&self) -> Result<(), String>;
    fn validate_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String>;
    fn validate_attachment_state(&self) -> Result<(), String>;
    fn validate_cast_permission(&self, player: PlayerId) -> Result<(), String>;
    fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        zone: Zone,
        c: &Characteristics,
    ) -> Result<(), String>;
    fn validate_combat_response(&self, player: PlayerId, response: &Response)
    -> Result<(), String>;
    fn validate_condition_cache(&self, frame: &Resolving) -> Result<(), String>;
    fn validate_control_state(&self) -> Result<(), String>;
    fn validate_cost_lock(&self, entry: &StackInstance) -> Result<(), String>;
    fn validate_counter_placement_response(
        &self,
        player: PlayerId,
        key: &CounterPlacementEffectKey,
    ) -> Result<(), String>;
    fn validate_damage_transaction(&self, tx: &DamageTransaction) -> Result<(), String>;
    fn validate_effect_state(&self) -> Result<(), String>;
    fn validate_entry_copy_identities(&self) -> Result<(), String>;
    fn validate_land_play(&self, player: PlayerId, card: CardId) -> Result<(), String>;
    fn validate_mana_activation(&self, player: PlayerId, object: ObjectId) -> Result<(), String>;
    fn validate_nested_mana_frame(&self, frame: &Frame) -> Result<(), String>;
    fn validate_permanent_mana_activation(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<(), String>;
    fn validate_player_token(&self, viewer: PlayerId, token: &DecisionToken) -> Result<(), String>;
    fn validate_resolution_payment(
        &self,
        frame: &Resolving,
        payment: &ResolutionPayment,
    ) -> Result<(), String>;
    fn validate_resolving(&self, frame: &Resolving) -> Result<(), String>;
    fn validate_response(&self, player: PlayerId, response: &Response) -> Result<(), String>;
    fn validate_state(&self) -> Result<(), String>;
    fn validate_tap_life_sacrifice_source(
        &self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
    ) -> Result<(), String>;
    fn validate_tokens_state(&self) -> Result<(), String>;
    fn validate_trigger_entry(&self, entry: &StackInstance) -> Result<Option<TriggerId>, String>;
    fn validate_trigger_state(&self) -> Result<(), String>;
    fn visible_allocation_candidate(
        &self,
        viewer: PlayerId,
        c: &DamageAllocationCandidate,
    ) -> Result<VisibleDamageAllocationCandidate, String>;
    fn visible_damage_effect_candidate(
        &self,
        viewer: PlayerId,
        pair: &DamageEffectCandidate,
    ) -> Result<VisibleDamageEffectCandidate, String>;
    fn visible_effect(&self, viewer: PlayerId, key: &EffectKey) -> Result<VisibleEffect, String>;
    fn visible_effects(&self, viewer: PlayerId) -> Result<Vec<VisibleEffect>, String>;
    fn visible_object(
        &self,
        viewer: PlayerId,
        object: &Object,
    ) -> Result<Option<VisibleObject>, String>;
    fn visible_pending_trigger(
        &self,
        viewer: PlayerId,
        id: TriggerId,
    ) -> Result<VisiblePendingTrigger, String>;
    fn visible_player(&self, player: PlayerId) -> Result<VisiblePlayer, String>;
    fn visible_stack(&self, viewer: PlayerId, index: usize) -> Result<VisibleStackEntry, String>;
    fn instruction_cause(&self, frame: &Resolving, source_block: u32)
    -> Result<BatchCause, String>;
    fn entry(&self, id: StackId) -> Result<&StackInstance, String>;
    fn unstack(&mut self, index: usize) -> Result<StackId, String>;
    fn release_detached(&mut self);
}
impl GameTestkit for Game {
    fn action_hand(&self, player: PlayerId) -> Vec<ObjectId> {
        RulesTxn::action_hand(self, player)
    }
    fn actions_request(&self) -> Result<Option<Request>, String> {
        RulesTxn::actions_request(self)
    }
    fn advance_internal<const REPORT: bool, const COALESCE: bool>(
        &mut self,
        budget: Budget,
        processed: &mut Vec<InputProcessingFact>,
    ) -> Advance {
        Game::advance_internal::<REPORT, COALESCE>(self, budget, processed)
    }
    fn allocated_capacity_count(&self) -> usize {
        RulesTxn::allocated_capacity_count(self)
    }
    fn announce_ability(
        &mut self,
        player: PlayerId,
        decision_id: u64,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        RulesTxn::announce_ability(self, player, decision_id, object, ability)
    }
    fn announce_priority_spell(&mut self, player: PlayerId, card: CardId) -> Result<(), String> {
        RulesTxn::announce_priority_spell(self, player, card)
    }
    fn announced_graveyard_group(
        &self,
        caster: PlayerId,
        chooser: PlayerId,
        maximum: u8,
        entities: &[TargetEntity],
    ) -> Result<(PlayerId, Vec<ObjectId>), String> {
        RulesTxn::announced_graveyard_group(self, caster, chooser, maximum, entities)
    }
    fn announced_target_legality(&self, entry: &StackInstance) -> Vec<bool> {
        RulesTxn::announced_target_legality(self, entry)
    }
    fn apply_departure(&mut self, player: PlayerId) -> Result<(), String> {
        RulesTxn::apply_departure(self, player)
    }
    fn apply_noncombat_damage(
        &mut self,
        damage: &NoncombatDamageOccurrence,
    ) -> Result<EventOutcome, String> {
        RulesTxn::apply_noncombat_damage(self, damage)
    }
    fn apply_occurrence(&mut self, proposed: &ProposedEvent) -> Result<EventOutcome, String> {
        RulesTxn::apply_occurrence(self, proposed)
    }
    fn apply_occurrence_with_prior(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        RulesTxn::apply_occurrence_with_prior(self, proposed, before)
    }
    fn attachment_sbas(&self, occurrences: &mut Vec<ProposedEvent>) -> Result<(), String> {
        RulesTxn::attachment_sbas(self, occurrences)
    }
    fn attachment_target_type_valid(
        &self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<bool, String> {
        RulesTxn::attachment_target_type_valid(self, source, target)
    }
    fn back_face_sequence(&self, object: ObjectId) -> Option<u64> {
        RulesTxn::back_face_sequence(self, object)
    }
    fn begin_trigger_placement(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<bool, String> {
        RulesTxn::begin_trigger_placement(self, resume, cleanup, changed)
    }
    fn blank_view(&self, viewer: PlayerId) -> Result<PermittedView, String> {
        RulesTxn::blank_view(self, viewer)
    }
    fn can_block(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        RulesTxn::can_block(self, blocker, attacker)
    }
    fn can_pay(&self, player: PlayerId, cost: &ManaCost) -> Result<bool, String> {
        RulesTxn::can_pay(self, player, cost)
    }
    fn capture_trigger_source_handles(
        &mut self,
        object: ObjectId,
    ) -> Result<Vec<publish::StoredObjectHandle>, String> {
        RulesTxn::capture_trigger_source_handles(self, object)
    }
    fn cascade_charge(&self) -> usize {
        RulesTxn::cascade_charge(self)
    }
    fn cast_is_prohibited(&self, player: PlayerId) -> Result<bool, String> {
        RulesTxn::cast_is_prohibited(self, player)
    }
    fn casting_cost(&self, card: CardId, caster: PlayerId, from: Zone) -> Result<ManaCost, String> {
        RulesTxn::casting_cost(self, card, caster, from)
    }
    fn check_history_capacity(&mut self) -> Result<(), String> {
        Ok(RulesTxn::check_history_capacity(self)?)
    }
    fn check_resolution_condition(
        &mut self,
        stack: &StackInstance,
    ) -> Result<Option<InterveningCheck>, String> {
        RulesTxn::check_resolution_condition(self, stack)
    }
    fn check_stack_capacity(&mut self, additional: usize) -> Result<(), String> {
        RulesTxn::check_stack_capacity(self, additional)
    }
    fn collect_cast_triggers(&mut self, entry: &StackInstance) -> Result<(), String> {
        RulesTxn::collect_cast_triggers(self, entry)
    }
    fn collect_triggers(
        &mut self,
        cause: &BatchCause,
        before: &Game,
        occurrences: &[OccurrenceResult],
    ) -> Result<(), String> {
        RulesTxn::collect_triggers(self, cause, before, occurrences)
    }
    fn color_layer(
        &self,
        object: ObjectId,
    ) -> Result<std::borrow::Cow<'_, Characteristics>, String> {
        RulesTxn::color_layer(self, object)
    }
    fn combat_step(&mut self) -> Result<(), String> {
        let frame = RulesTxn::working_frame(self);
        RulesTxn::combat_step(self, frame)
    }
    fn commit(
        &mut self,
        proposed: ProposedEvent,
        cause: BatchCause,
    ) -> Result<EventOutcome, String> {
        RulesTxn::commit(self, proposed, cause)
    }
    fn commit_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String> {
        RulesTxn::commit_batch(self, cause, group)
    }
    fn complete_test_mana(&mut self) {
        Game::complete_test_mana(self)
    }
    fn control_before(&self) -> Result<Option<Box<ControlBefore>>, String> {
        RulesTxn::control_before(self)
    }
    fn controlled_creatures(&self, player: PlayerId) -> Vec<ObjectId> {
        RulesTxn::controlled_creatures(self, player)
    }
    fn convert_response(
        &self,
        viewer: PlayerId,
        response: &PlayerResponse,
    ) -> Result<Response, String> {
        RulesTxn::convert_response(self, viewer, response)
    }
    fn copiable_definition(
        &self,
        object: impl std::borrow::Borrow<ObjectId>,
    ) -> Result<&Definition, String> {
        RulesTxn::copiable_definition(self, object)
    }
    fn cost_lock_charge(&self) -> usize {
        RulesTxn::cost_lock_charge(self)
    }
    fn counter_placement_candidates(&self) -> Vec<CounterPlacementEffectKey> {
        RulesTxn::counter_placement_candidates(self)
    }
    fn counter_placement_request(&self) -> Result<Option<Request>, String> {
        RulesTxn::counter_placement_request(self)
    }
    fn creature_or_planeswalker(&self, id: ObjectId) -> bool {
        RulesTxn::creature_or_planeswalker(self, id)
    }
    fn creatures(&self, player: PlayerId, attacking: bool) -> Vec<ObjectId> {
        RulesTxn::creatures(self, player, attacking)
    }
    fn current(&self, id: ObjectId) -> Result<&Object, String> {
        Ok(RulesTxn::current(self, id)?)
    }
    fn current_keywords(&self, object: ObjectId) -> Vec<KeywordAbility> {
        RulesTxn::current_keywords(self, object)
    }
    fn damage_allocation_request(&self) -> Result<Option<Request>, String> {
        RulesTxn::damage_allocation_request(self)
    }
    fn damage_effect_group(
        &self,
    ) -> Result<Option<(PlayerId, TargetEntity, Vec<DamageEffectCandidate>)>, String> {
        RulesTxn::damage_effect_group(self)
    }
    fn damage_effect_request(&self) -> Result<Option<Request>, String> {
        RulesTxn::damage_effect_request(self)
    }
    fn damage_effect_step(&mut self) -> Result<(), String> {
        RulesTxn::damage_effect_step(self)
    }
    fn damage_keywords(
        &mut self,
        actual: &[ActualDamage],
        arg2: Option<&BTreeMap<DamageId, ObjectBefore>>,
    ) -> Result<Vec<LifelinkGain>, String> {
        RulesTxn::damage_keywords(self, actual, arg2)
    }
    fn damage_limit(&self) -> usize {
        RulesTxn::damage_limit(self)
    }
    fn damage_transaction(&self) -> Result<&DamageTransaction, String> {
        RulesTxn::damage_transaction(self)
    }
    fn definition(&self, card: CardId) -> Result<&Definition, String> {
        Ok(RulesTxn::definition(self, card)?)
    }
    fn derived_characteristics(&self, object: ObjectId) -> Result<Characteristics, String> {
        RulesTxn::derived_characteristics(self, object)
    }
    fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        RulesTxn::derived_controllers(self)
    }
    fn effect_decision_context(
        &self,
        viewer: PlayerId,
        effect: &EffectKey,
    ) -> Option<DecisionContext> {
        RulesTxn::effect_decision_context(self, viewer, effect)
    }
    fn effect_handle(&self, viewer: PlayerId, key: &EffectKey) -> Result<EffectHandle, String> {
        RulesTxn::effect_handle(self, viewer, key)
    }
    fn effect_limit(&self) -> usize {
        RulesTxn::effect_limit(self)
    }
    fn eligible_players(&self, domain: TargetDomain, controller: PlayerId) -> Vec<PlayerId> {
        RulesTxn::eligible_players(self, domain, controller)
    }
    fn eligible_stack_targets_for(
        &self,
        spec: &TargetSpec,
        controller: PlayerId,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        RulesTxn::eligible_stack_targets_for(self, spec, controller, exclude)
    }
    fn enter_turn_step(&mut self) -> Result<(), String> {
        RulesTxn::enter_turn_step(self)
    }
    fn entity_legal(&self, target: TargetEntity) -> bool {
        RulesTxn::entity_legal(self, target)
    }
    fn entry_cause_for_test(&self) -> BatchCause {
        RulesTxn::entry_cause_for_test(self)
    }
    fn entry_copy_origin(&self, object: ObjectId) -> Option<(ObjectId, CardId)> {
        RulesTxn::entry_copy_origin(self, object)
    }
    fn entry_group_for_test(&self) -> EventGroup {
        RulesTxn::entry_group_for_test(self)
    }
    fn entry_search_block(&self, frame: &Resolving) -> Result<u32, String> {
        RulesTxn::entry_search_block(self, frame)
    }
    fn event_group_limit(&self) -> usize {
        RulesTxn::event_group_limit(self)
    }
    fn execute_damage_life_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        RulesTxn::execute_damage_life_instruction(self, frame, instruction)
    }
    fn execute_kernel_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let Instruction::Kernel { node, source_block } = instruction else {
            return Err("expected a kernel instruction".into());
        };
        RulesTxn::execute_kernel(self, frame, node, source_block)
    }
    fn execute_effect_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        RulesTxn::execute_effect_instruction(self, frame, instruction)
    }
    fn execute_wipes_instruction(
        &mut self,
        frame: Resolving,
        i: Instruction,
    ) -> Result<(), String> {
        RulesTxn::execute_wipes_instruction(self, frame, i)
    }
    fn finish_committed_player_losses(&mut self, batch: &CommittedBatch) -> Result<bool, String> {
        RulesTxn::finish_committed_player_losses(self, batch)
    }
    fn grant_payload_charge(&self) -> usize {
        RulesTxn::grant_payload_charge(self)
    }
    fn granted_land_mana_colors() -> [Color; 5] {
        RulesTxn::granted_land_mana_colors()
    }
    fn graveyard_target_legal(
        &self,
        object: &Object,
        player: PlayerId,
        domain: TargetDomain,
    ) -> bool {
        RulesTxn::graveyard_target_legal(self, object, player, domain)
    }
    fn stored_handle_for(
        &self,
        viewer: PlayerId,
        id: ObjectId,
    ) -> Result<publish::StoredObjectHandle, String> {
        self.object_handle_id(viewer, id).cloned()
    }
    fn handle_for(&self, viewer: PlayerId, id: ObjectId) -> Result<ObjectHandle, String> {
        RulesTxn::handle_for(self, viewer, id)
    }
    fn has_color(source: &Characteristics) -> bool {
        RulesTxn::has_color(source)
    }
    fn has_granted_land_mana(&self, land: ObjectId) -> Result<bool, String> {
        RulesTxn::has_granted_land_mana(self, land)
    }
    fn has_keyword<'a>(
        &self,
        object: impl Into<KeywordSource<'a>>,
        keyword: KeywordAbility,
    ) -> bool {
        RulesTxn::has_keyword(self, object, keyword)
    }
    fn has_land_play_allowance(&self, player: PlayerId) -> bool {
        RulesTxn::has_land_play_allowance(self, player)
    }
    fn has_restriction(&self, object: ObjectId, restriction: CreatureRestriction) -> bool {
        RulesTxn::has_restriction(self, object, restriction)
    }
    fn has_target_domain(
        &self,
        program: &SpellProgram,
        player: PlayerId,
        source: &Characteristics,
    ) -> bool {
        RulesTxn::has_target_domain(self, program, player, source)
    }
    fn history_charge(&self) -> usize {
        RulesTxn::history_charge(self).expect("valid fixture continuation")
    }
    fn initial_query_task(&self, query: &Query) -> Result<Task, QueryRejection> {
        RulesTxn::initial_query_task(self, query)
    }
    fn is_battlefield_creature(&self, id: ObjectId) -> bool {
        RulesTxn::is_battlefield_creature(self, id)
    }
    fn is_bestowed_spell(&self, object: ObjectId) -> bool {
        RulesTxn::is_bestowed_spell(self, object)
    }
    fn is_blocking_creature(&self, object: ObjectId) -> bool {
        RulesTxn::is_blocking_creature(self, object)
    }
    fn is_live(&self, player: PlayerId) -> bool {
        RulesTxn::is_live(self, player)
    }
    fn is_spell_copy(&self, entry: &StackInstance) -> bool {
        RulesTxn::is_spell_copy(self, entry)
    }
    fn known_library_positions(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<KnownLibraryPosition>, String> {
        RulesTxn::known_library_positions(self, viewer)
    }
    fn land_entry_charge(&self) -> usize {
        RulesTxn::land_entry_charge(self)
    }
    fn land_entry_step(&mut self) -> Result<(), String> {
        RulesTxn::land_entry_step(self)
    }
    fn lethal_requirement(&self, source: ObjectId, blocker: ObjectId) -> Result<Quantity, String> {
        RulesTxn::lethal_requirement(self, source, blocker)
    }
    fn library_prefix(&self, player: PlayerId, amount: &Quantity) -> Result<Vec<ObjectId>, String> {
        RulesTxn::library_prefix(self, player, amount)
    }
    fn linked_hideaway_cards(&self, source: ObjectId, source_block: u32) -> Vec<ObjectId> {
        RulesTxn::linked_hideaway_cards(self, source, source_block)
    }
    fn live_effect_keys(&self) -> Vec<EffectKey> {
        RulesTxn::live_effect_keys(self)
    }
    fn main_phase_action(&self, player: PlayerId) -> bool {
        RulesTxn::main_phase_action(self, player)
    }
    fn mana_ability_used(&self, source: ObjectId) -> bool {
        RulesTxn::mana_ability_used(self, source)
    }
    fn mana_continuation_charge(&self) -> usize {
        RulesTxn::mana_continuation_charge(self).expect("valid fixture continuation")
    }
    fn matches_permanent_filter(
        &self,
        object: ObjectId,
        controller: PlayerId,
        f: &PermanentFilter,
    ) -> bool {
        RulesTxn::matches_permanent_filter(self, object, controller, f)
    }
    fn may_play_exile(&self, player: PlayerId, object: ObjectId) -> bool {
        RulesTxn::may_play_exile(self, player, object)
    }
    fn mixed_mana_choices(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<ManaAbilityIdentity>, String> {
        RulesTxn::mixed_mana_choices(self, player, source)
    }
    fn modes_request(&self) -> Result<Option<Request>, String> {
        RulesTxn::modes_request(self)
    }
    fn move_object(&mut self, object: ObjectId, destination: Zone) -> Result<MoveOutcome, String> {
        RulesTxn::move_object(self, object, destination)
    }
    fn move_object_for(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String> {
        RulesTxn::move_object_for(self, object, destination, cause)
    }
    fn next_turn_state(&self) -> Result<(TurnState, PlayerId), String> {
        RulesTxn::next_turn_state(self)
    }
    fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String> {
        RulesTxn::object_before(self, object)
    }
    fn object_facts<'a>(&'a self, object: &'a Object) -> DerivedObject<'a> {
        RulesTxn::object_facts(self, object)
    }
    fn object_target_legal(&self, entry: &StackInstance, object: ObjectId) -> bool {
        RulesTxn::object_target_legal(self, entry, object)
    }
    fn objects_in_set(&self, set: &ObjectSet, controller: PlayerId) -> Vec<ObjectId> {
        RulesTxn::objects_in_set(self, set, controller)
    }
    fn observation_work_units(&self, viewer: PlayerId) -> Result<u64, String> {
        RulesTxn::observation_work_units(self, viewer)
    }
    fn offer_commander_sba(&mut self, resume: Frame) -> Result<bool, String> {
        RulesTxn::offer_commander_sba(self, resume)
    }
    fn opponent_land_mana_colors(&self, player: PlayerId) -> Result<Vec<Color>, String> {
        RulesTxn::opponent_land_mana_colors(self, player)
    }
    fn pay_announcement_costs(&mut self, cast: &Casting) -> Result<AnnouncementPayment, String> {
        RulesTxn::pay_announcement_costs(self, cast)
    }
    fn payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
    ) -> Result<BTreeMap<Color, Quantity>, payments::PaymentError> {
        RulesTxn::payment_pool(self, player, cost)
    }
    fn permanent_colors(
        &self,
        scope: PermanentColorScope,
        controller: PlayerId,
    ) -> Result<BTreeSet<Color>, String> {
        RulesTxn::permanent_colors(self, scope, controller)
    }
    fn permanent_target_legal(
        &self,
        object: ObjectId,
        controller: PlayerId,
        spec: &TargetSpec,
    ) -> bool {
        RulesTxn::permanent_target_legal(self, object, controller, spec)
    }
    fn permitted_decision_context(
        &self,
        viewer: PlayerId,
        request: Option<&PlayerRequest>,
    ) -> Option<DecisionContext> {
        RulesTxn::permitted_decision_context(self, viewer, request)
    }
    fn phase_in_for_next_untap(
        &mut self,
        previous_owner: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        RulesTxn::phase_in_for_next_untap(self, previous_owner)
    }
    fn phase_out_permanents(&mut self, selection: PhaseSelection) -> Result<EventOutcome, String> {
        RulesTxn::phase_out_permanents(self, selection)
    }
    fn physical_transform_mana_value(&self, object: ObjectId) -> Option<Quantity> {
        RulesTxn::physical_transform_mana_value(self, object)
    }
    fn player_entity(
        &self,
        viewer: PlayerId,
        target: TargetEntity,
    ) -> Result<PlayerTargetEntity, String> {
        RulesTxn::player_entity(self, viewer, target)
    }
    fn player_graveyard_card(&self, object: &Object, player: PlayerId) -> bool {
        RulesTxn::player_graveyard_card(self, object, player)
    }
    fn player_protected(&self, player: PlayerId) -> bool {
        RulesTxn::player_protected(self, player)
    }
    fn preflight_trigger_emission(&mut self, additional: usize) -> Result<(), String> {
        RulesTxn::preflight_trigger_emission(self, additional)
    }
    fn prepare_combat_damage(&mut self, damage: Vec<DamageOccurrence>) -> Result<(), String> {
        RulesTxn::prepare_combat_damage(self, damage)
    }
    fn prepare_noncombat_damage(
        &mut self,
        frame: Resolving,
        damage: NoncombatDamageOccurrence,
        block: u32,
    ) -> Result<(), String> {
        RulesTxn::prepare_noncombat_damage(self, frame, damage, block)
    }
    fn program(&self) -> Result<&SpellProgram, String> {
        Ok(RulesTxn::program(self)?)
    }
    fn program_for<'a>(&'a self, stack: &'a StackInstance) -> Result<&'a SpellProgram, String> {
        Ok(RulesTxn::program_for(self, stack)?)
    }
    fn protected_from(&self, target: ObjectId, source: &Characteristics) -> bool {
        RulesTxn::protected_from(self, target, source)
    }
    fn query_work_units(&self, query: &Query) -> Result<u64, String> {
        RulesTxn::query_work_units(self, query)
    }
    fn read_snapshot(prepared: Prepared, json: &str) -> Result<(Game, String), String> {
        Game::read_snapshot(prepared, json)
    }
    fn read_snapshot_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<(Game, String), String> {
        Game::read_snapshot_from_reader(prepared, source)
    }
    fn reconcile_controllers(&mut self) -> Result<(), String> {
        RulesTxn::reconcile_controllers(self)
    }
    fn record_entry_copy(&mut self, entering: ObjectId, donor: ObjectId) -> Result<(), String> {
        RulesTxn::record_entry_copy(self, entering, donor)
    }
    fn refresh_effect_handles(&mut self) -> Result<Vec<EffectKey>, String> {
        RulesTxn::refresh_effect_handles(self)
    }
    fn refresh_visibility(&mut self) -> Result<(), String> {
        RulesTxn::refresh_visibility(self)
    }
    fn remaining_graveyard_targets(&self, owner: PlayerId, cards: &[ObjectId]) -> Vec<ObjectId> {
        RulesTxn::remaining_graveyard_targets(self, owner, cards)
    }
    fn replacement_reveals(&self, object: ObjectId) -> bool {
        RulesTxn::replacement_reveals(self, object)
    }
    fn replay_to(
        prepared: Prepared,
        journal: &ReplayJournal,
        boundary: Option<u64>,
        mode: TraceMode,
    ) -> Result<Game, String> {
        Game::replay_to(prepared, journal, boundary, mode)
    }
    fn request_header(&self, viewer: PlayerId) -> Result<Option<PlayerRequest>, String> {
        RulesTxn::request_header(self, viewer)
    }
    fn resolve_damage_effect_pair(
        &self,
        viewer: PlayerId,
        effect: &EffectHandle,
        damage: &DamageHandle,
    ) -> Result<(EffectKey, DamageId), String> {
        RulesTxn::resolve_damage_effect_pair(self, viewer, effect, damage)
    }
    fn resolve_damage_handle(
        &self,
        viewer: PlayerId,
        handle: &DamageHandle,
    ) -> Result<DamageId, String> {
        RulesTxn::resolve_damage_handle(self, viewer, handle)
    }
    fn resolve_entity(
        &self,
        viewer: PlayerId,
        target: &PlayerTargetEntity,
    ) -> Result<TargetEntity, String> {
        RulesTxn::resolve_entity(self, viewer, target)
    }
    fn resolve_handle(&self, viewer: PlayerId, handle: &ObjectHandle) -> Result<ObjectId, String> {
        RulesTxn::resolve_handle(self, viewer, handle)
    }
    fn resolve_search_handle(
        &self,
        player: PlayerId,
        handle: &ObjectHandle,
    ) -> Result<ObjectId, String> {
        RulesTxn::resolve_search_handle(self, player, handle)
    }
    fn resolve_trigger_handle(
        &self,
        viewer: PlayerId,
        handle: &TriggerHandle,
    ) -> Result<TriggerId, String> {
        RulesTxn::resolve_trigger_handle(self, viewer, handle)
    }
    fn restore_hideaway_link(&mut self, receipt: HideawayLink) -> Result<(), String> {
        RulesTxn::restore_hideaway_link(self, receipt)
    }
    fn resulting_permission_matches(
        &self,
        permission: &CastPermission,
        stack: &StackInstance,
    ) -> Result<bool, String> {
        RulesTxn::resulting_permission_matches(self, permission, stack)
    }
    fn retain_source_facts(&mut self, fact: &ObjectBefore) {
        RulesTxn::retain_source_facts(self, fact)
    }
    fn retain_source_lki(&mut self, object: ObjectId) -> Result<(), String> {
        RulesTxn::retain_source_lki(self, object)
    }
    fn sacrifice_candidates(&self, player: PlayerId, filter: &SacrificeFilter) -> Vec<ObjectId> {
        RulesTxn::sacrifice_candidates(self, player, filter)
    }
    fn search_lands(&self, player: PlayerId) -> Vec<ObjectId> {
        RulesTxn::search_lands(self, player)
    }
    fn searched_land_force_tapped(&self, player: PlayerId) -> Result<bool, String> {
        RulesTxn::searched_land_force_tapped(self, player)
    }
    fn selected_payment_pool(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        spending: &BTreeMap<Color, Quantity>,
    ) -> Result<BTreeMap<Color, Quantity>, String> {
        RulesTxn::selected_payment_pool(self, player, cost, spending)
    }
    fn selected_program<'a>(
        &self,
        root: &'a SpellProgram,
        mode: Option<u32>,
    ) -> Result<&'a SpellProgram, String> {
        RulesTxn::selected_program(self, root, mode)
    }
    fn shield_limit(&self, effect: &ResolvedEffect) -> Result<Quantity, String> {
        RulesTxn::shield_limit(self, effect)
    }
    fn source_facts(&self, entry: &StackInstance) -> Result<ObjectBefore, String> {
        RulesTxn::source_facts(self, entry)
    }
    fn stabilization_wave_with_legends(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
        kept: Option<Vec<ObjectId>>,
    ) -> Result<(), String> {
        RulesTxn::stabilization_wave_with_legends(self, resume, cleanup, changed, kept)
    }
    fn stabilize(&mut self) -> Result<(), String> {
        Ok(RulesTxn::stabilize(self)?)
    }
    fn stack_handle_for(&self, viewer: PlayerId, id: StackId) -> Result<StackHandle, String> {
        RulesTxn::stack_handle_for(self, viewer, id)
    }
    fn stack_limit(&self) -> usize {
        RulesTxn::stack_limit(self)
    }
    fn stack_mana_value(&self, entry: &StackInstance) -> Result<Quantity, String> {
        RulesTxn::stack_mana_value(self, entry)
    }
    fn start_work(&self, query: Query) -> Result<Work, QueryRejection> {
        RulesTxn::start_work(self, query)
    }
    fn static_damage_effects(&self) -> Vec<EffectKey> {
        RulesTxn::static_damage_effects(self)
    }
    fn static_modifications(&self, target: ObjectId) -> Vec<&StaticEffect> {
        RulesTxn::static_modifications(self, target)
    }
    fn step(&mut self) -> Result<(), String> {
        Ok(RulesTxn::step(self)?)
    }
    fn target_is_creature(&self, object: ObjectId) -> bool {
        RulesTxn::target_is_creature(self, object)
    }
    fn targetable_by(&self, object: ObjectId, controller: PlayerId) -> bool {
        RulesTxn::targetable_by(self, object, controller)
    }
    fn targetable_player(&self, player: PlayerId) -> bool {
        RulesTxn::targetable_player(self, player)
    }
    fn targeted_loss_authority(&self) -> Result<(BatchCause, PlayerId), String> {
        RulesTxn::targeted_loss_authority(self)
    }
    fn targeting_characteristics<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Result<std::borrow::Cow<'a, Characteristics>, String> {
        RulesTxn::targeting_characteristics(self, entry)
    }
    fn token_capacity(&self) -> u32 {
        RulesTxn::token_capacity(self)
    }
    fn top(&self) -> Result<&StackInstance, String> {
        Ok(RulesTxn::top(self)?)
    }
    fn transition_turn_step(&mut self) -> Result<(), String> {
        RulesTxn::transition_turn_step(self)
    }
    fn trigger_order_ids(&self) -> Vec<TriggerId> {
        RulesTxn::trigger_order_ids(self)
    }
    fn trigger_source_label(
        &self,
        source: ObjectId,
        ability: EffectiveTriggerAbility,
    ) -> Option<&str> {
        RulesTxn::trigger_source_label(self, source, ability)
    }
    fn turn_up_linked_hideaway(
        &mut self,
        source: ObjectId,
        source_block: u32,
        card: ObjectId,
    ) -> Result<HideawayLink, String> {
        RulesTxn::turn_up_linked_hideaway(self, source, source_block, card)
    }
    fn update_library_position_memory(&mut self, event: &ProposedEvent, actual: &EventOutcome) {
        RulesTxn::update_library_position_memory(self, event, actual)
    }
    fn valid_set_effect_targets(
        &self,
        e: &ResolvedEffect,
        targets: &[ObjectId],
        set: Option<&ObjectSet>,
    ) -> bool {
        RulesTxn::valid_set_effect_targets(self, e, targets, set)
    }
    fn validate_actions_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Option<Result<(), String>> {
        RulesTxn::validate_actions_response(self, player, response)
    }
    fn validate_actions_state(&self) -> Result<(), String> {
        RulesTxn::validate_actions_state(self)
    }
    fn validate_activation(
        &self,
        player: PlayerId,
        object: ObjectId,
        ability: u32,
    ) -> Result<(), String> {
        RulesTxn::validate_activation(self, player, object, ability)
    }
    fn validate_attachment_state(&self) -> Result<(), String> {
        RulesTxn::validate_attachment_state(self)
    }
    fn validate_cast_permission(&self, player: PlayerId) -> Result<(), String> {
        RulesTxn::validate_cast_permission(self, player)
    }
    fn validate_characteristic_identity(
        &self,
        source: ObjectId,
        zone: Zone,
        c: &Characteristics,
    ) -> Result<(), String> {
        RulesTxn::validate_characteristic_identity(self, source, zone, c)
    }
    fn validate_combat_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        RulesTxn::validate_combat_response(self, player, response)
    }
    fn validate_condition_cache(&self, frame: &Resolving) -> Result<(), String> {
        RulesTxn::validate_condition_cache(self, frame)
    }
    fn validate_control_state(&self) -> Result<(), String> {
        RulesTxn::validate_control_state(self)
    }
    fn validate_cost_lock(&self, entry: &StackInstance) -> Result<(), String> {
        RulesTxn::validate_cost_lock(self, entry)
    }
    fn validate_counter_placement_response(
        &self,
        player: PlayerId,
        key: &CounterPlacementEffectKey,
    ) -> Result<(), String> {
        RulesTxn::validate_counter_placement_response(self, player, key)
    }
    fn validate_damage_transaction(&self, tx: &DamageTransaction) -> Result<(), String> {
        RulesTxn::validate_damage_transaction(self, tx)
    }
    fn validate_effect_state(&self) -> Result<(), String> {
        RulesTxn::validate_effect_state(self)
    }
    fn validate_entry_copy_identities(&self) -> Result<(), String> {
        RulesTxn::validate_entry_copy_identities(self)
    }
    fn validate_land_play(&self, player: PlayerId, card: CardId) -> Result<(), String> {
        RulesTxn::validate_land_play(self, player, card)
    }
    fn validate_mana_activation(&self, player: PlayerId, object: ObjectId) -> Result<(), String> {
        RulesTxn::validate_mana_activation(self, player, object)
    }
    fn validate_nested_mana_frame(&self, frame: &Frame) -> Result<(), String> {
        RulesTxn::validate_nested_mana_frame(self, frame)
    }
    fn validate_permanent_mana_activation(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<(), String> {
        RulesTxn::validate_permanent_mana_activation(self, player, source)
    }
    fn validate_player_token(&self, viewer: PlayerId, token: &DecisionToken) -> Result<(), String> {
        RulesTxn::validate_player_token(self, viewer, token)
    }
    fn validate_resolution_payment(
        &self,
        frame: &Resolving,
        payment: &ResolutionPayment,
    ) -> Result<(), String> {
        RulesTxn::validate_resolution_payment(self, frame, payment)
    }
    fn validate_resolving(&self, frame: &Resolving) -> Result<(), String> {
        RulesTxn::validate_resolving(self, frame)
    }
    fn validate_response(&self, player: PlayerId, response: &Response) -> Result<(), String> {
        RulesTxn::validate_response(self, player, response)
    }
    fn validate_state(&self) -> Result<(), String> {
        Ok(RulesTxn::validate_state(self)?)
    }
    fn validate_tap_life_sacrifice_source(
        &self,
        player: PlayerId,
        source: ObjectId,
        life: &Quantity,
    ) -> Result<(), String> {
        RulesTxn::validate_tap_life_sacrifice_source(self, player, source, life)
    }
    fn validate_tokens_state(&self) -> Result<(), String> {
        RulesTxn::validate_tokens_state(self)
    }
    fn validate_trigger_entry(&self, entry: &StackInstance) -> Result<Option<TriggerId>, String> {
        RulesTxn::validate_trigger_entry(self, entry)
    }
    fn validate_trigger_state(&self) -> Result<(), String> {
        RulesTxn::validate_trigger_state(self)
    }
    fn visible_allocation_candidate(
        &self,
        viewer: PlayerId,
        c: &DamageAllocationCandidate,
    ) -> Result<VisibleDamageAllocationCandidate, String> {
        RulesTxn::visible_allocation_candidate(self, viewer, c)
    }
    fn visible_damage_effect_candidate(
        &self,
        viewer: PlayerId,
        pair: &DamageEffectCandidate,
    ) -> Result<VisibleDamageEffectCandidate, String> {
        RulesTxn::visible_damage_effect_candidate(self, viewer, pair)
    }
    fn visible_effect(&self, viewer: PlayerId, key: &EffectKey) -> Result<VisibleEffect, String> {
        RulesTxn::visible_effect(self, viewer, key)
    }
    fn visible_effects(&self, viewer: PlayerId) -> Result<Vec<VisibleEffect>, String> {
        RulesTxn::visible_effects(self, viewer)
    }
    fn visible_object(
        &self,
        viewer: PlayerId,
        object: &Object,
    ) -> Result<Option<VisibleObject>, String> {
        RulesTxn::visible_object(self, viewer, object)
    }
    fn visible_pending_trigger(
        &self,
        viewer: PlayerId,
        id: TriggerId,
    ) -> Result<VisiblePendingTrigger, String> {
        RulesTxn::visible_pending_trigger(self, viewer, id)
    }
    fn visible_player(&self, player: PlayerId) -> Result<VisiblePlayer, String> {
        RulesTxn::visible_player(self, player)
    }
    fn visible_stack(&self, viewer: PlayerId, index: usize) -> Result<VisibleStackEntry, String> {
        RulesTxn::visible_stack(self, viewer, index)
    }
    fn instruction_cause(
        &self,
        frame: &Resolving,
        source_block: u32,
    ) -> Result<BatchCause, String> {
        Ok(RulesTxn::instruction_cause(self, frame, source_block)?)
    }
    fn entry(&self, id: StackId) -> Result<&StackInstance, String> {
        Ok(RulesTxn::entry(self, id)?)
    }
    fn unstack(&mut self, index: usize) -> Result<StackId, String> {
        Ok(RulesTxn::unstack(self, index)?)
    }
    fn release_detached(&mut self) {
        RulesTxn::release_detached(self)
    }
}

/// Crate-private `MatchRandom` methods that white-box test binaries call.
pub trait MatchRandomTestkit {
    fn word(&mut self) -> Result<u32, String>;
}
impl MatchRandomTestkit for MatchRandom {
    fn word(&mut self) -> Result<u32, String> {
        MatchRandom::word(self)
    }
}

/// Crate-private `PermittedHistory` methods that white-box test binaries call.
pub trait PermittedHistoryTestkit {
    fn push_public(&mut self, view: PermittedView);
    fn push_stored(&mut self, view: StoredPermittedView);
    fn stored(&self, index: usize) -> Option<&StoredPermittedView>;
}
impl PermittedHistoryTestkit for PermittedHistory {
    fn push_public(&mut self, view: PermittedView) {
        PermittedHistory::push_public(self, view)
    }
    fn push_stored(&mut self, view: StoredPermittedView) {
        PermittedHistory::push_stored(self, view)
    }
    fn stored(&self, index: usize) -> Option<&StoredPermittedView> {
        PermittedHistory::stored(self, index)
    }
}

/// Crate-private `ScenarioConfig` methods that white-box test binaries call.
pub trait ScenarioConfigTestkit {
    fn initial_libraries(&self) -> BTreeMap<PlayerId, Vec<CardId>>;
}
impl ScenarioConfigTestkit for ScenarioConfig {
    fn initial_libraries(&self) -> BTreeMap<PlayerId, Vec<CardId>> {
        ScenarioConfig::initial_libraries(self)
    }
}

/// Crate-private `SnapshotVerifier` methods that white-box test binaries call.
pub trait SnapshotVerifierTestkit {
    fn verify_game(&mut self, game: Game, digest: String) -> Result<Game, String>;
}
impl SnapshotVerifierTestkit for SnapshotVerifier {
    fn verify_game(&mut self, game: Game, digest: String) -> Result<Game, String> {
        SnapshotVerifier::verify_game(self, game, digest)
    }
}

/// Crate-private `Work` methods that white-box test binaries call.
pub trait WorkTestkit {
    fn belongs_to_viewer(&self, viewer: PlayerId) -> bool;
}
impl WorkTestkit for Work {
    fn belongs_to_viewer(&self, viewer: PlayerId) -> bool {
        Work::belongs_to_viewer(self, viewer)
    }
}
