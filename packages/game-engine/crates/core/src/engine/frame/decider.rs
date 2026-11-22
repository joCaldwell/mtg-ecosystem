//! Who decides at the suspended frame.
//!
//! Exactly one player can owe a decision, and the frame alone selects the
//! one request generator that can produce it. Each viewer's decision token is
//! keyed by the revision and the numeric kind returned here; the kinds are
//! part of each viewer's stored knowledge and never change meaning.
use super::*;

/// The player who owes the pending decision and the kind of its token key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decider {
    pub player: PlayerId,
    pub kind: u8,
}

impl Decider {
    fn of(player: PlayerId, kind: u8) -> Option<Self> {
        Some(Self { player, kind })
    }
    fn requested(request: Option<Request>, kind: u8) -> Option<Self> {
        request.and_then(|request| Self::of(request.player(), kind))
    }
}

impl Frame {
    /// The decider at this frame for `game`, whose suspended frame it is.
    /// Every variant is named: a new frame must say who decides at it.
    pub fn decider(&self, game: &RulesTxn) -> Result<Option<Decider>, RuleError> {
        let rules = &game.state.rules;
        if rules.pending.is_some() || !rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        // The controller of the object being announced or placed.
        let top = || -> Result<PlayerId, RuleError> { Ok(game.top()?.controller) };
        Ok(match self {
            Frame::Kernel { task } => Decider::requested(game.kernel_request(task)?, 47),
            Frame::GraveyardReplacement { transaction: _ } | Frame::GraveyardGroup { tx: _ } => {
                Decider::requested(game.replacement_request(), 4)
            }
            Frame::DamageAllocation {
                transaction: _,
                effect: _,
            } => Decider::requested(game.damage_allocation_request()?, 14),
            Frame::SacrificeCost { cast: _ }
            | Frame::ForcedSacrifice {
                frame: _,
                remaining: _,
                choices: _,
                source_block: _,
            }
            | Frame::OptionalDamage { frame: _ } => {
                Decider::requested(game.sacrifice_request()?, 17)
            }
            Frame::HandCast { frame: _, child: _ } => {
                Decider::requested(game.hand_cast_request()?, 45)
            }
            Frame::PlayHideawayLinked { progress: _ }
            | Frame::RevealAndCast { progress: _ }
            | Frame::Cascade { progress: _ } => match game.resolution_cast_request()? {
                Some(request) => {
                    let kind = match request {
                        Request::CascadeLandEntry { .. } => 23,
                        Request::PlayHideawayLinked { .. } => 46,
                        _ => 22,
                    };
                    Decider::of(request.player(), kind)
                }
                None => None,
            },
            Frame::Modes { cast: _ } => Decider::requested(game.modes_request()?, 13),
            Frame::CounterPlacement { transaction: _ } => {
                Decider::requested(game.counter_placement_request()?, 22)
            }
            Frame::DamageEffects { transaction: _ } => {
                Decider::requested(game.damage_effect_request()?, 12)
            }
            Frame::TriggerOrder { placement: _ } => Decider::requested(game.trigger_request()?, 11),
            Frame::ResolutionChoice {
                frame: _,
                choice: _,
            } => Decider::requested(game.actions_request()?, 6),
            Frame::Attackers
            | Frame::Blockers { remaining: _ }
            | Frame::DamageChoice { source: _ }
            | Frame::Discard {
                player: _,
                count: _,
            } => game
                .combat_decision_player()
                .and_then(|player| Decider::of(player, 3)),
            Frame::LegendRule {
                resume: _,
                groups: _,
                kept: _,
            } => Decider::requested(game.legend_request()?, 19),
            Frame::Setup => Decider::requested(game.setup_request()?, 15),
            Frame::EntityTargets { cast: _ } => {
                let any_damage_or_shuffle = game.program()?.targets.first().is_some_and(|t| {
                    t.domain == TargetDomain::AnyDamageTarget
                        || game.program().is_ok_and(|p| {
                            mtg_contracts::graveyard_shuffle::offered_by(p).is_some()
                        })
                });
                if any_damage_or_shuffle {
                    Decider::of(top()?, 10)
                } else {
                    None
                }
            }
            Frame::TriggerTargets {
                placement: _,
                stack_id: _,
            } => {
                let any_damage_or_shuffle = game.program()?.targets.first().is_some_and(|t| {
                    t.domain == TargetDomain::AnyDamageTarget
                        || game.program().is_ok_and(|p| {
                            mtg_contracts::graveyard_shuffle::offered_by(p).is_some()
                        })
                });
                if any_damage_or_shuffle {
                    Decider::of(top()?, 10)
                } else if !game.top_has_target_domain()? {
                    // CR 603.3d: a trigger with no legal target is removed by
                    // the next rule step; nobody chooses.
                    None
                } else {
                    Decider::of(top()?, 7)
                }
            }
            Frame::StackTargets { cast: _ } => Decider::of(top()?, 9),
            Frame::PlayerTargets { cast: _ } => Decider::of(top()?, 7),
            Frame::LandEntry {
                origin: _,
                objects: _,
                choices: _,
            } => game
                .land_entry_offer()?
                .and_then(|(player, _, _)| Decider::of(player, 20)),
            Frame::ResolutionPayment { frame: _, payment } => Decider::of(payment.payer, 20),
            Frame::ManaSourceChoice {
                player,
                source: _,
                scope: _,
                resume: _,
            } => Decider::of(*player, 20),
            Frame::ManaAbility { activation } => Decider::of(activation.player, 20),
            Frame::SpendMana { cast: _ } => Decider::of(top()?, 8),
            Frame::CommanderReturn {
                object,
                destination: _,
                continuation: _,
            } => Decider::of(rules.cards[&object.card].owner, 4),
            Frame::Priority => Decider::of(rules.priority, 0),
            Frame::AdditionalCosts { cast: _ } => Decider::of(top()?, 20),
            Frame::HybridCost { cast: _ } => Decider::of(top()?, 51),
            Frame::AnnounceX { cast: _ } => Decider::of(top()?, 18),
            Frame::Targets { cast: _ } => Decider::of(top()?, 1),
            Frame::Mana { cast: _ } => Decider::of(top()?, 2),
            // Automatic rule work: nobody decides.
            Frame::Damage
            | Frame::CleanupClear
            | Frame::Stabilize {
                resume: _,
                cleanup: _,
                changed: _,
            }
            | Frame::Draw
            | Frame::Transition
            | Frame::EnterStep
            | Frame::Payment { cast: _ }
            | Frame::Resolve { frame: _ }
            | Frame::Fault { reason: _ }
            | Frame::Finished { winner: _ } => None,
        })
    }
}
