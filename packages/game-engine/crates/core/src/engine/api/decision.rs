//! The pending decision and the validation of a proposed answer.
mod build;
pub(in crate::engine) use build::candidate_index;
#[cfg(any(test, feature = "testkit"))]
mod intent;
mod project;
mod selected;
#[cfg(not(feature = "testkit"))]
pub(in crate::engine) use selected::PendingInput;
#[cfg(feature = "testkit")]
pub use selected::PendingInput;
mod model;
pub use model::*;
#[cfg(test)]
mod tests;

use crate::engine::*;

impl RulesTxn {
    pub fn decision_for(&self, viewer: PlayerId) -> Result<Option<Decision<Rules>>, String> {
        Ok(self
            .pending_decision()?
            .filter(|decision| decision.player == viewer))
    }

    pub(crate) fn pending_decision(&self) -> Result<Option<Decision<Rules>>, String> {
        if self.state.rules.pending.is_some() || !self.state.rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        if matches!(self.state.rules.frame, Frame::Priority) {
            if let Some(decision) = self.priority_cache.get() {
                return Ok(Some(decision.clone()));
            }
            let decision = self.priority_decision(self.state.rules.priority)?;
            let _ = self.priority_cache.set(decision.clone());
            return Ok(Some(decision));
        }
        self.frame_decision()
    }

    pub(in crate::engine) fn frame_decision(&self) -> Result<Option<Decision<Rules>>, String> {
        let decision = match &self.state.rules.frame {
            Frame::Kernel { task } => return self.kernel_request(task),
            Frame::GraveyardGroup { .. } => return Ok(self.replacement_request()),
            Frame::Setup => return self.setup_request(),
            Frame::DamageEffects { .. } => return self.damage_effect_request(),
            Frame::TriggerOrder { .. } => return self.trigger_request(),
            Frame::ResolutionChoice { .. } => return self.actions_request(),
            Frame::Attackers
            | Frame::Blockers { .. }
            | Frame::DamageChoice { .. }
            | Frame::Discard { .. } => return self.combat_request(),
            Frame::SacrificeCost { .. } | Frame::ForcedSacrifice { .. } => {
                return self.sacrifice_request();
            }
            Frame::ChosenObjectCost { selected, .. } => return self.chosen_cost_request(selected),
            Frame::LegendRule { .. } => return self.legend_request(),
            Frame::BattlefieldEntry { entry } => return self.battlefield_entry_request(entry),
            Frame::Priority => self.priority_decision(self.state.rules.priority)?,
            Frame::TriggerTargets { .. } if !self.top_has_target_domain()? => return Ok(None),
            Frame::TriggerTargets { .. } if self.unbound_modes(self.top()?)? => {
                return self.mode_decision();
            }
            Frame::TriggerTargets { .. }
            | Frame::Targets { .. }
            | Frame::PlayerTargets { .. }
            | Frame::EntityTargets { .. }
            | Frame::StackTargets { .. } => self.target_decision()?,
            Frame::ChooseModes { .. } => return self.mode_decision(),
            Frame::AdditionalCosts { .. } => {
                let candidates: Vec<_> = self
                    .optional_cost_declarations(self.top()?)?
                    .into_iter()
                    .map(|c| Candidate::AdditionalCost {
                        id: c.identity.clone(),
                        mana: c.mana.clone(),
                    })
                    .collect();
                let max = candidates.len() as u32;
                self.pick(
                    self.top()?.controller,
                    Reason::AdditionalCosts,
                    candidates,
                    0,
                    max,
                )
            }
            Frame::HybridCost { .. } => self.hybrid_decision()?,
            Frame::AnnounceX { .. } => self.decision(
                self.top()?.controller,
                Reason::AnnounceX,
                vec![],
                Shape::Number {
                    min: 0.into(),
                    max: None,
                },
            ),
            Frame::SpendMana { cast } if let Some(decision) = self.delve_decision(cast)? => {
                decision
            }
            Frame::SpendMana { cast }
                if let Some(decision) = self.tap_payment_decision(cast)? =>
            {
                decision
            }
            Frame::Mana { cast } | Frame::SpendMana { cast } => {
                let entry = self.top()?;
                self.payment_decision(
                    entry.controller,
                    self.mana_due(entry, cast)?,
                    None,
                    matches!(self.state.rules.frame, Frame::Mana { .. }),
                    self.announced_mana_use(entry)?,
                )?
            }
            Frame::ManaSourceChoice {
                player,
                source,
                scope: ManaSourceScope::Mixed,
                ..
            } => {
                let candidates = self
                    .mixed_mana_choices(*player, *source)?
                    .into_iter()
                    .map(|ability| Candidate::ManaAbility { ability })
                    .collect();
                self.pick(
                    *player,
                    Reason::ManaAbility { source: *source },
                    candidates,
                    1,
                    1,
                )
            }
            Frame::ManaSourceChoice {
                player,
                source,
                scope,
                ..
            } => {
                let candidates = self
                    .mana_source_colors(*player, *source, *scope)?
                    .into_iter()
                    .map(|color| Candidate::Color { color })
                    .collect();
                self.pick(
                    *player,
                    Reason::ManaColor {
                        source: Some(*source),
                    },
                    candidates,
                    1,
                    1,
                )
            }
            Frame::ResolutionPayment { payment, .. } => self.resolution_payment_request(payment)?,
            Frame::ManaAbility { activation } => self.permanent_mana_request(activation)?,
            Frame::CommanderReturn {
                object,
                destination,
                ..
            } => self.commander_decision(*object, *destination, self.previous_commander_choices()),
            Frame::Damage
            | Frame::CleanupClear
            | Frame::Stabilize { .. }
            | Frame::Draw
            | Frame::Transition
            | Frame::EnterStep
            | Frame::Payment { .. }
            | Frame::CounterPlacement { .. }
            | Frame::Resolve { .. }
            | Frame::Fault { .. }
            | Frame::Finished { .. } => return Ok(None),
        };
        Ok(Some(decision))
    }

    pub(in crate::engine) fn commander_decision(
        &self,
        object: ObjectId,
        destination: Zone,
        previous: Vec<(ObjectId, bool)>,
    ) -> Decision<Rules> {
        self.pick_flags(
            self.state.rules.cards[&object.card].owner,
            Reason::CommanderReturn {
                object: Some(object),
                designation: object.card,
                destination,
                previous,
            },
            &[Flag::CommandZone, Flag::Destination],
        )
    }

    pub fn check_response(
        &self,
        player: PlayerId,
        decision_id: u64,
        answer: Answer,
    ) -> Result<ValidatedResponse, String> {
        if decision_id != self.state.rules.revision {
            return Err("stale decision".into());
        }
        let decision = self
            .decision_for(player)?
            .ok_or("stale or unauthorized decision")?;
        decision.shape.check(&answer)?;
        Ok(ValidatedResponse {
            executor: self.executor.clone(),
            revision: decision_id,
            state_token: self.state_token.clone(),
            input: PendingInput {
                input: AcceptedInput {
                    decision_id,
                    player,
                    answer,
                },
                decision,
            },
        })
    }
}
