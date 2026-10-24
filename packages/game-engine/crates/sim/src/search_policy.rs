//! Stateless rollout policy. Each call receives only the simulated chooser's
//! current permitted observation and the immutable public catalog.
use super::search_candidates::{self, ActionPlan, TargetPlan};
use mtg_contracts::{Catalog, Quantity, SpellProgram};
use mtg_core::{Observation, PlayerRequest, PlayerResponse, PlayerTargetEntity, TurnStep};

pub(super) const ROLLOUT_POLICY: &str = "foundations-current-view-development-v1";
fn selected_program<'a>(
    catalog: &'a Catalog,
    observation: &Observation,
) -> Option<&'a SpellProgram> {
    let entry = observation.view.stack.last()?;
    let program = if let Some(trigger) = &entry.triggered {
        &search_candidates::definition(catalog, &trigger.oracle_id)
            .ok()?
            .triggered
            .get(usize::try_from(trigger.ability).ok()?)?
            .program
    } else if let Some(ability) = &entry.ability {
        &search_candidates::definition(catalog, &ability.oracle_id)
            .ok()?
            .activated
            .get(usize::try_from(ability.ability).ok()?)?
            .program
    } else {
        let object = observation
            .view
            .objects
            .iter()
            .find(|o| Some(&o.handle) == entry.spell.as_ref())?;
        search_candidates::definition(catalog, &object.oracle_id)
            .ok()?
            .spell
            .as_ref()?
    };
    match entry.mode {
        Some(mode) => program
            .modes
            .get(usize::try_from(mode).ok()?)
            .map(|m| &m.program),
        None => Some(program),
    }
}
pub(super) fn initial_response(plan: &ActionPlan) -> PlayerResponse {
    match plan {
        ActionPlan::Pass => PlayerResponse::Pass,
        ActionPlan::PlayLand { object } => PlayerResponse::PlayLand {
            object: object.clone(),
        },
        ActionPlan::Cast { card, .. } => PlayerResponse::Cast { card: card.clone() },
        ActionPlan::Activate {
            object, ability, ..
        } => PlayerResponse::ActivateAbility {
            object: object.clone(),
            ability: *ability,
        },
    }
}
pub(super) fn choose(
    catalog: &Catalog,
    observation: &Observation,
    check: &mut dyn FnMut() -> Result<(), String>,
) -> Result<PlayerResponse, String> {
    check()?;
    let view = &observation.view;
    let request = observation
        .decision
        .as_ref()
        .ok_or("rollout observation has no decision")?;
    let beneficial =
        selected_program(catalog, observation).is_some_and(search_candidates::beneficial);
    Ok(match request {
        PlayerRequest::ManaColor { .. }
        | PlayerRequest::SearchLands { .. }
        | PlayerRequest::OptionalEffect { .. }
        | PlayerRequest::OptionalRecursion { .. } => {
            return Err("staples request outside historical policy".into());
        }
        PlayerRequest::Priority { .. } => {
            if view.active != Some(view.viewer)
                || !view.stack.is_empty()
                || !matches!(
                    view.turn.step,
                    TurnStep::PrecombatMain | TurnStep::PostcombatMain
                )
            {
                PlayerResponse::Pass
            } else {
                let candidates = search_candidates::generate(catalog, observation, check)?;
                candidates
                    .candidates
                    .iter()
                    .find(|c| !matches!(c.plan, ActionPlan::Pass))
                    .map(|c| initial_response(&c.plan))
                    .unwrap_or(PlayerResponse::Pass)
            }
        }
        PlayerRequest::Modes { candidates, .. } => PlayerResponse::Modes {
            mode: *candidates.first().ok_or("empty rollout modes")?,
        },
        PlayerRequest::Targets {
            occurrences,
            candidates,
            ..
        } => {
            let target = candidates
                .iter()
                .find(|handle| {
                    view.objects.iter().any(|o| {
                        o.handle == **handle && (o.controller == view.viewer) == beneficial
                    })
                })
                .or(candidates.first())
                .ok_or("empty rollout creature domain")?;
            PlayerResponse::Targets {
                objects: vec![target.clone(); occurrences.len()],
            }
        }
        PlayerRequest::PlayerTargets {
            occurrences,
            candidates,
            ..
        } => {
            let target = candidates
                .iter()
                .find(|p| (**p == view.viewer) == beneficial)
                .or(candidates.first())
                .ok_or("empty rollout player domain")?;
            PlayerResponse::PlayerTargets {
                players: vec![*target; occurrences.len()],
            }
        }
        PlayerRequest::EntityTargets {
            occurrences,
            candidates,
            ..
        } => {
            let target=candidates.iter().find(|entity|matches!(entity,PlayerTargetEntity::Player{player}if(*player==view.viewer)==beneficial)).or(candidates.first()).ok_or("empty rollout entity domain")?;
            PlayerResponse::EntityTargets {
                entities: vec![target.clone(); occurrences.len()],
            }
        }
        PlayerRequest::StackTargets {
            occurrences,
            candidates,
            ..
        } => PlayerResponse::StackTargets {
            entries: vec![
                candidates
                    .first()
                    .ok_or("empty rollout stack domain")?
                    .clone();
                occurrences.len()
            ],
        },
        PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
            order: candidates.iter().map(|c| c.handle.clone()).collect(),
        },
        PlayerRequest::Mana { cost, .. } => {
            let funds = &view
                .players
                .iter()
                .find(|p| p.player == view.viewer)
                .ok_or("missing rollout chooser")?
                .mana;
            if search_candidates::spend(cost, funds).is_some() {
                PlayerResponse::FinishMana { undo_mana: true }
            } else if let Some((sources, _)) =
                search_candidates::funding(catalog, observation, cost)
            {
                PlayerResponse::ActivateMana {
                    object: sources
                        .first()
                        .ok_or("unfunded rollout with no mana sources")?
                        .clone(),
                }
            } else {
                PlayerResponse::FinishMana { undo_mana: true }
            }
        }
        PlayerRequest::SpendMana {
            cost, available, ..
        } => PlayerResponse::SpendMana {
            spending: search_candidates::spend(cost, available)
                .ok_or("rollout explicit spending is infeasible")?,
        },
        PlayerRequest::CommanderReturn { .. } => {
            PlayerResponse::CommanderReturn { to_command: true }
        }
        PlayerRequest::Attackers {
            candidates,
            defenders,
            ..
        } => {
            let defender = defenders
                .iter()
                .copied()
                .min_by_key(|p| (p.0 + 4 - view.viewer.0) % 4);
            let attacks = candidates
                .iter()
                .filter(|handle| {
                    view.objects.iter().any(|o| {
                        o.handle == **handle
                            && o.characteristics
                                .as_ref()
                                .and_then(|c| c.power.as_ref())
                                .is_some_and(|power| power > &Quantity::zero())
                    })
                })
                .filter_map(|handle| defender.map(|player| (handle.clone(), player)))
                .collect();
            PlayerResponse::Attackers { attacks }
        }
        PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
        PlayerRequest::Damage {
            power, recipients, ..
        } => PlayerResponse::Damage {
            assignments: recipients
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    (
                        h.clone(),
                        if i == 0 {
                            power.clone()
                        } else {
                            Quantity::zero()
                        },
                    )
                })
                .collect(),
        },
        PlayerRequest::Discard {
            count, candidates, ..
        } => PlayerResponse::Discard {
            objects: candidates.iter().rev().take(*count).cloned().collect(),
        },
        PlayerRequest::ResolveDiscard {
            count, candidates, ..
        } => PlayerResponse::ResolveDiscard {
            objects: candidates.iter().rev().take(*count).cloned().collect(),
        },
        PlayerRequest::Sacrifice { candidates, .. } => PlayerResponse::Sacrifice {
            object: candidates
                .first()
                .ok_or("empty rollout sacrifice domain")?
                .clone(),
        },
        PlayerRequest::DamageEffect { candidates, .. } => {
            let candidate = candidates
                .first()
                .ok_or("empty rollout damage effect domain")?;
            PlayerResponse::DamageEffect {
                effect: candidate.effect.clone(),
                damage: candidate.damage.clone(),
            }
        }
        PlayerRequest::DamageAllocation {
            required,
            candidates,
            ..
        } => {
            let mut remaining = required.clone();
            let allocations = candidates
                .iter()
                .map(|candidate| {
                    let amount = std::cmp::min(candidate.amount.clone(), remaining.clone());
                    remaining.subtract(&amount);
                    (candidate.damage.clone(), amount)
                })
                .collect();
            if remaining != Quantity::zero() {
                return Err("rollout allocation cannot satisfy required prevention".into());
            }
            PlayerResponse::DamageAllocation { allocations }
        }
        PlayerRequest::Mulligan { .. } | PlayerRequest::BottomOpening { .. } => {
            return Err("sample rollout unexpectedly entered opening setup".into());
        }
    })
}
pub(super) fn target_response(
    targets: &TargetPlan,
    request: &PlayerRequest,
) -> Result<PlayerResponse, String> {
    match (targets, request) {
        (TargetPlan::Objects { objects }, PlayerRequest::Targets { occurrences, .. })
            if objects.len() == occurrences.len() =>
        {
            Ok(PlayerResponse::Targets {
                objects: objects.clone(),
            })
        }
        (TargetPlan::Players { players }, PlayerRequest::PlayerTargets { occurrences, .. })
            if players.len() == occurrences.len() =>
        {
            Ok(PlayerResponse::PlayerTargets {
                players: players.clone(),
            })
        }
        (TargetPlan::Entities { entities }, PlayerRequest::EntityTargets { occurrences, .. })
            if entities.len() == occurrences.len() =>
        {
            Ok(PlayerResponse::EntityTargets {
                entities: entities.clone(),
            })
        }
        _ => Err("root plan target stage disagrees with selected program".into()),
    }
}
