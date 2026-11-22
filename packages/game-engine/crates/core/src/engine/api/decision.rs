//! The pending decision and the validation of a proposed answer.
use crate::engine::*;

impl RulesTxn {
    /// The decision `viewer` owes now, if any.
    pub fn decision_for(&self, viewer: PlayerId) -> Result<Option<Request>, String> {
        Ok(self
            .pending_decision()?
            .filter(|request| request.player() == viewer))
    }
    /// The one decision the game is waiting for, whoever owes it. None while
    /// an accepted input or a queued concession is still to be applied.
    pub(crate) fn pending_decision(&self) -> Result<Option<Request>, String> {
        if self.state.rules.pending.is_some() || !self.state.rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        if let Frame::Kernel { task } = &self.state.rules.frame {
            return self.kernel_request(task);
        }
        if matches!(
            self.state.rules.frame,
            Frame::GraveyardReplacement { .. } | Frame::GraveyardGroup { .. }
        ) {
            return Ok(self.replacement_request());
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            return Ok(self.setup_request()?);
        }
        if let Some(req) = self.damage_allocation_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.modes_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.counter_placement_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.damage_effect_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.trigger_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.actions_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.combat_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.hand_cast_request()? {
            return Ok(Some(req));
        }
        if let Some(req) = self.resolution_cast_request()? {
            return Ok(Some(req));
        }
        let req = match &self.state.rules.frame {
            Frame::LegendRule { .. } => {
                return Ok(self.legend_request()?);
            }
            Frame::EntityTargets { .. } => Request::EntityTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self.eligible_entities(),
            },
            Frame::StackTargets { .. } => Request::StackTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self.eligible_stack_targets_for(
                    &self.program()?.targets[0],
                    self.top()?.controller,
                    Some(self.top()?.id),
                ),
            },
            Frame::TriggerTargets { .. }
                if self.program()?.targets.first().is_some_and(|t| {
                    matches!(
                        t.domain,
                        TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                            | TargetDomain::OwnGraveyardCard
                    )
                }) =>
            {
                if !self.top_has_target_domain()? {
                    return Ok(None);
                }
                Request::Targets {
                    decision_id: self.state.rules.revision,
                    player: self.top()?.controller,
                    occurrences: self.program()?.targets.clone(),
                    counter_division: None,
                    candidates: self
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| self.object_target_legal(self.top().unwrap(), o.id))
                        .map(|o| o.id)
                        .collect(),
                }
            }
            Frame::TriggerTargets { .. }
                if self
                    .program()?
                    .targets
                    .first()
                    .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget) =>
            {
                Request::EntityTargets {
                    decision_id: self.state.rules.revision,
                    player: self.top()?.controller,
                    occurrences: self.program()?.targets.clone(),
                    candidates: self.eligible_entities(),
                }
            }
            Frame::PlayerTargets { .. } | Frame::TriggerTargets { .. } => Request::PlayerTargets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                candidates: self
                    .eligible_players(self.program()?.targets[0].domain, self.top()?.controller),
            },
            Frame::SacrificeCost { .. }
            | Frame::ForcedSacrifice { .. }
            | Frame::OptionalDamage { .. } => {
                return Ok(self.sacrifice_request()?);
            }
            Frame::LandEntry { .. } => {
                return self.land_entry_request();
            }
            Frame::ManaSourceChoice {
                player,
                source,
                scope: ManaSourceScope::Mixed,
                ..
            } => Request::ChooseManaAbility {
                decision_id: self.state.rules.revision,
                player: *player,
                choices: self.mixed_mana_choices(*player, *source)?,
            },
            Frame::ManaSourceChoice {
                player,
                source,
                scope,
                ..
            } => Request::ManaColor {
                decision_id: self.state.rules.revision,
                player: *player,
                colors: self.mana_source_colors(*player, *source, *scope)?,
            },
            Frame::ManaAbility { activation } => self.permanent_mana_request(activation)?,
            Frame::ResolutionPayment { payment, .. } => self.resolution_payment_request(payment),
            Frame::SpendMana { .. } => self.spend_mana_request()?,
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } => Request::CommanderReturn {
                previous_choices: self.previous_commander_choices(),
                decision_id: self.state.rules.revision,
                player: self.state.rules.cards[&object.card].owner,
                object: *object,
                destination: *destination,
            },
            Frame::Priority => Request::Priority {
                decision_id: self.state.rules.revision,
                player: self.state.rules.priority,
            },
            Frame::AdditionalCosts { .. } => Request::AdditionalCosts {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                candidates: self.program()?.optional_additional_costs.clone(),
            },
            Frame::HybridCost { .. } => Request::HybridCost {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                symbols: self
                    .copiable_definition(self.top()?.source)?
                    .characteristics
                    .hybrid_mana
                    .clone(),
            },
            Frame::AnnounceX { .. } => Request::AnnounceX {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
            },
            Frame::Mana { .. } => Request::Mana {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                cost: self.top()?.total_cost.clone(),
            },
            Frame::Targets { .. } => Request::Targets {
                decision_id: self.state.rules.revision,
                player: self.top()?.controller,
                occurrences: self.program()?.targets.clone(),
                counter_division: CounterDivision::offered_by(self.program()?),
                candidates: self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|o| self.object_target_legal(self.top().unwrap(), o.id))
                    .map(|o| o.id)
                    .collect(),
            },
            _ => return Ok(None),
        };
        Ok(Some(req))
    }
    pub(crate) fn validate_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if matches!(
            self.state.rules.frame,
            Frame::GraveyardReplacement { .. } | Frame::GraveyardGroup { .. }
        ) {
            return if self.replacement_request().is_some_and(|r| {
                r.player() == player
                    && matches!(
                        (r, response),
                        (
                            Request::CommanderReturn { .. },
                            Response::CommanderReturn { .. }
                        ) | (
                            Request::GraveyardReplacementOrder { .. },
                            Response::OptionalEffect { .. }
                        )
                    )
            }) {
                Ok(())
            } else {
                Err("invalid graveyard replacement decision".into())
            };
        }
        if let Frame::Kernel { task } = &self.state.rules.frame {
            return self.validate_kernel_response(task, player, response);
        }
        if matches!(self.state.rules.frame, Frame::LandEntry { .. }) {
            return self.validate_land_entry_response(player, response);
        }
        if let Frame::ResolutionPayment { payment, .. } = &self.state.rules.frame {
            return self.validate_resolution_payment_response(payment, player, response);
        }
        if matches!(self.state.rules.frame, Frame::PlayHideawayLinked { .. }) {
            return self.validate_linked_play_response(player, response);
        }
        if let Frame::RevealAndCast { progress } = &self.state.rules.frame {
            return self.validate_reveal_offer(progress, player, response);
        }
        if matches!(&self.state.rules.frame, Frame::HandCast { .. }) {
            return self.validate_hand_cast_response(player, response);
        }
        if let Frame::Cascade { progress } = &self.state.rules.frame {
            return self.validate_cascade_response(progress, player, response);
        }
        if matches!(self.state.rules.frame, Frame::LegendRule { .. }) {
            return self.validate_legend_response(player, response);
        }
        if let Frame::AdditionalCosts { .. } = self.state.rules.frame {
            return if let Response::AdditionalCosts { selected } = response {
                self.validate_additional_cost_selection(player, selected)
            } else {
                Err("invalid optional cost announcement".into())
            };
        }
        if let Frame::HybridCost { cast } = &self.state.rules.frame {
            let entry = self.top()?;
            if let Response::HybridCost { colors } = response
                && entry.id == cast.stack_id
                && entry.controller == player
                && entry.kind.is_spell()
                && entry.alternative_cost.is_none()
                && entry.hybrid_colors.is_empty()
            {
                self.validate_hybrid_colors(entry.source.card, colors)?;
                // CR 601.2b/g/h: life affordability is evaluated at payment,
                // after mana abilities have had a chance to change the total.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                return Ok(());
            }
            return Err("invalid hybrid mana announcement".into());
        }
        if let Frame::AnnounceX { .. } = self.state.rules.frame {
            return if matches!(response, Response::AnnounceX { value } if !value.is_negative())
                && self.top()?.controller == player
            {
                Ok(())
            } else {
                Err("invalid X announcement".into())
            };
        }
        if matches!(
            self.state.rules.frame,
            Frame::SacrificeCost { .. }
                | Frame::ForcedSacrifice { .. }
                | Frame::OptionalDamage { .. }
        ) {
            return self.validate_sacrifice_response(player, response);
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            return self.validate_setup_response(player, response);
        }
        if let Frame::Stabilize { resume, .. } = &self.state.rules.frame {
            let mut validation = self.fork();
            validation.rules_mut().frame = self.saved_frame(*resume).clone();
            return validation.validate_response(player, response);
        }
        if let (Frame::CounterPlacement { .. }, Response::CounterPlacementEffect { effect }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_counter_placement_response(player, effect);
        }
        if let (Frame::DamageEffects { .. }, Response::DamageEffect { effect, damage }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_damage_effect(player, effect, *damage);
        }
        if let (Frame::DamageAllocation { .. }, Response::DamageAllocation { allocations }) =
            (&self.state.rules.frame, response)
        {
            return self.validate_damage_allocation(player, allocations);
        }
        if let (Frame::Modes { .. }, Response::ModeSubset { modes }) =
            (&self.state.rules.frame, response)
        {
            return self.authorize_mode_subset(player, modes).map(|_| ());
        }
        if let (Frame::Modes { .. }, Response::Modes { mode }) = (&self.state.rules.frame, response)
        {
            return self.validate_mode(player, *mode);
        }
        if let Some(result) = self.validate_actions_response(player, response) {
            return result;
        }
        if let Frame::ManaSourceChoice {
            player: chooser,
            source,
            scope,
            ..
        } = &self.state.rules.frame
        {
            return match response {
                Response::ChooseManaAbility { ability }
                    if *chooser == player
                        && *scope == ManaSourceScope::Mixed
                        && self.mixed_mana_choices(player, *source)?.contains(ability) =>
                {
                    Ok(())
                }
                Response::ManaColor { color }
                    if *chooser == player
                        && *scope != ManaSourceScope::Mixed
                        && self
                            .mana_source_colors(player, *source, *scope)?
                            .contains(color) =>
                {
                    Ok(())
                }
                _ => Err("invalid mana source choice".into()),
            };
        }
        if let Frame::ManaAbility { activation } = &self.state.rules.frame {
            return self.validate_permanent_mana_response(activation, player, response);
        }
        if matches!(self.state.rules.frame, Frame::SpendMana { .. }) {
            match response {
                Response::SpendMana { spending } => {
                    return self.validate_spending(player, spending, &[]);
                }
                Response::SpendManaAllocated {
                    spending,
                    restricted,
                } => return self.validate_spending(player, spending, restricted),
                _ => {}
            }
        }
        if self.combat_decision_player().is_some() {
            return self.validate_combat_response(player, response);
        }
        match (&self.state.rules.frame, response) {
            (Frame::CommanderReturn { object, .. }, Response::CommanderReturn { .. })
                if self.state.rules.cards[&object.card].owner == player && self.is_live(player) =>
            {
                Ok(())
            }
            (Frame::Priority, Response::PlayLand { card })
                if self.state.rules.priority == player =>
            {
                self.validate_land_play(player, *card)
            }
            (Frame::Priority, Response::PlayBackLand { card })
                if self.state.rules.priority == player =>
            {
                self.validate_land_face(player, *card, true)
            }
            (Frame::Priority, Response::Pass) if self.state.rules.priority == player => Ok(()),
            (Frame::Priority, Response::ActivateMana { object })
                if self.state.rules.priority == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::ActivateMana { object })
                if self.top()?.controller == player =>
            {
                self.validate_mana_activation(player, *object)
            }
            (Frame::Mana { .. }, Response::FinishMana { .. })
                if self.top()?.controller == player =>
            {
                Ok(())
            }
            (Frame::Priority, Response::Bestow { card }) if self.state.rules.priority == player => {
                self.validate_priority_bestow(player, *card)
            }
            (Frame::Priority, Response::Cast { card }) if self.state.rules.priority == player => {
                self.validate_priority_spell_cast(player, *card)
            }
            (
                Frame::Targets { .. } | Frame::TriggerTargets { .. },
                Response::Targets { objects },
            ) if self.top()?.controller == player => {
                if self.program()?.targets.iter().any(|t| {
                    !matches!(
                        t.domain,
                        TargetDomain::CreatureOrPlaneswalker
                            | TargetDomain::ControlledBattlefieldCreature
                            | TargetDomain::ControlledBattlefieldPermanent
                            | TargetDomain::BattlefieldPermanent
                            | TargetDomain::BattlefieldCreature
                            | TargetDomain::BlockingBattlefieldCreature
                            | TargetDomain::OwnGraveyardCreatureCard
                            | TargetDomain::OwnGraveyardCard
                    )
                }) || !Self::object_target_count_valid(self.program()?, objects)
                    || objects
                        .iter()
                        .any(|o| !self.object_target_legal(self.top().unwrap(), *o))
                {
                    return Err("invalid targets".into());
                }
                Ok(())
            }
            _ => Err("invalid or unauthorized decision response".into()),
        }
    }
    pub fn check_response(
        &self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<ValidatedResponse, String> {
        if self.state.rules.pending.is_some()
            || decision_id != self.state.rules.revision
            || self.decision_for(player)?.is_none()
        {
            return Err("stale or unauthorized decision".into());
        }
        self.validate_response(player, &response)?;
        Ok(ValidatedResponse {
            executor: self.executor.clone(),
            revision: self.state.rules.revision,
            state_token: self.state_token.clone(),
            input: AcceptedInput {
                decision_id,
                player,
                response,
            },
        })
    }
}
