use super::*;
use crate::engine::*;

impl RulesTxn {
    pub(in crate::engine) fn viewer_decision(
        &self,
        viewer: PlayerId,
    ) -> Result<Option<Decision<Viewer>>, String> {
        let knowledge = self
            .state
            .knowledge
            .visibility
            .get(viewer.0 as usize)
            .ok_or("invalid viewer")?;
        let Some(token) = knowledge.decision_token.clone() else {
            return Ok(None);
        };
        let Some(decision) = self.decision_for(viewer)? else {
            return Ok(None);
        };
        let reason = self.viewer_reason(viewer, decision.reason)?;
        let candidates = decision
            .candidates
            .into_iter()
            .map(|candidate| self.viewer_candidate(viewer, candidate))
            .collect::<Result<_, _>>()?;
        Ok(Some(Decision {
            id: token,
            player: viewer,
            reason,
            candidates,
            shape: decision.shape,
        }))
    }

    fn viewer_reason(
        &self,
        viewer: PlayerId,
        reason: Reason<Rules>,
    ) -> Result<Reason<Viewer>, String> {
        let object = |o| self.handle_for(viewer, o);
        let current = |o: Option<ObjectId>| {
            o.filter(|o| self.current(*o).is_ok())
                .map(object)
                .transpose()
        };
        Ok(match reason {
            Reason::Priority => Reason::Priority,
            Reason::Targets { specs } => Reason::Targets { specs },
            Reason::Mana { source } => Reason::Mana {
                source: current(source)?,
            },
            Reason::Attackers => Reason::Attackers,
            Reason::Blockers => Reason::Blockers,
            Reason::CombatDamage { source, power } => Reason::CombatDamage {
                source: object(source)?,
                power,
            },
            Reason::Discard => Reason::Discard,
            Reason::CommanderReturn {
                object: id,
                designation,
                destination,
                ..
            } => Reason::CommanderReturn {
                object: id.and_then(|o| object(o).ok()),
                designation: self.commander_designation(viewer, designation)?,
                destination,
                previous: self.visible_commander_choices(viewer)?,
            },
            Reason::GraveyardReplacementOrder {
                object: id,
                offered,
            } => Reason::GraveyardReplacementOrder {
                object: object(id)?,
                offered,
            },
            Reason::OptionalEffect => Reason::OptionalEffect,
            Reason::DamageEffect { recipient } => Reason::DamageEffect {
                recipient: self.player_entity(viewer, recipient)?,
            },
            Reason::OrderTriggers => Reason::OrderTriggers,
            Reason::Mulligan {
                mulligans_taken,
                bottom_if_mulligan,
            } => Reason::Mulligan {
                mulligans_taken,
                bottom_if_mulligan,
            },
            Reason::BottomOpening => Reason::BottomOpening,
            Reason::Sacrifice => Reason::Sacrifice,
            Reason::SacrificeCost => Reason::SacrificeCost,
            Reason::ForcedSacrifice { previous } => Reason::ForcedSacrifice {
                previous: previous
                    .into_iter()
                    .filter(|(p, o)| self.is_live(*p) && self.current(*o).is_ok())
                    .map(|(p, o)| Ok((p, object(o)?)))
                    .collect::<Result<_, String>>()?,
            },
            Reason::AnnounceX => Reason::AnnounceX,
            Reason::LegendRule { previous } => Reason::LegendRule {
                previous: previous.into_iter().map(object).collect::<Result<_, _>>()?,
            },
            Reason::AdditionalCosts => Reason::AdditionalCosts,
            Reason::ChooseModes => Reason::ChooseModes,
            Reason::HybridCost => Reason::HybridCost,
            Reason::ManaAbility { source } => Reason::ManaAbility {
                source: object(source)?,
            },
            Reason::ManaColor { source } => Reason::ManaColor {
                source: current(source)?,
            },
            Reason::SpendMana { source } => Reason::SpendMana {
                source: current(source)?,
            },
            Reason::ResolveDiscard => Reason::ResolveDiscard,
            Reason::SearchLibrary => Reason::SearchLibrary,
            Reason::ScryBottom => Reason::ScryBottom,
            Reason::SurveilGraveyard => Reason::SurveilGraveyard,
            Reason::OrderLibraryTop => Reason::OrderLibraryTop,
            Reason::OrderLibraryBottom => Reason::OrderLibraryBottom,
            Reason::EntryAttachment => Reason::EntryAttachment,
        })
    }

    fn viewer_candidate(
        &self,
        viewer: PlayerId,
        candidate: Candidate<Rules>,
    ) -> Result<Candidate<Viewer>, String> {
        let handle = |o| self.handle_for(viewer, o);
        Ok(match candidate {
            Candidate::Object { object } => Candidate::Object {
                object: handle(object)?,
            },
            Candidate::Player { player } => Candidate::Player { player },
            Candidate::Trigger { trigger } => Candidate::Trigger {
                trigger: self.visible_pending_trigger(viewer, trigger)?.handle,
            },
            Candidate::DamageEffect { effect, damage } => {
                let pair = self.visible_damage_effect_candidate(
                    viewer,
                    &DamageEffectCandidate { effect, damage },
                )?;
                Candidate::DamageEffect {
                    effect: pair.effect,
                    damage: pair.damage,
                }
            }
            Candidate::Color { color } => Candidate::Color { color },
            Candidate::ManaAbility { ability } => Candidate::ManaAbility { ability },
            Candidate::AdditionalCost { id, mana } => {
                let entry = self.stack_handle_for(viewer, self.top()?.id)?;
                Candidate::AdditionalCost {
                    id: format!("{}/cost/{id}", entry.0),
                    mana,
                }
            }
            Candidate::HybridPayment { payment } => Candidate::HybridPayment { payment },
            Candidate::Modes { mask, labels } => Candidate::Modes { mask, labels },
            Candidate::Flag { flag } => Candidate::Flag { flag },
            Candidate::Action { action } => Candidate::Action {
                action: match action {
                    Action::Pass => Action::Pass,
                    Action::Cast { card } => Action::Cast {
                        card: handle(card)?,
                    },
                    Action::PlayLand { object } => Action::PlayLand {
                        object: handle(object)?,
                    },
                    Action::CastBackFace { card } => Action::CastBackFace {
                        card: handle(card)?,
                    },
                    Action::PlayLandBackFace { object } => Action::PlayLandBackFace {
                        object: handle(object)?,
                    },
                    Action::ActivateMana { object } => Action::ActivateMana {
                        object: handle(object)?,
                    },
                    Action::ActivateAbility { object, ability } => Action::ActivateAbility {
                        object: handle(object)?,
                        ability,
                    },
                },
            },
        })
    }
}
