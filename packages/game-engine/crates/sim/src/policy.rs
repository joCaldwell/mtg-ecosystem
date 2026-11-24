//! Policies choose a response from the acting seat's permitted observation.
use mtg_contracts::{CardKind, Catalog, Color, ManaCost, Quantity};
use mtg_core::{
    ObjectHandle, Observation, PlayerId, PlayerRequest, PlayerResponse, PlayerTargetEntity,
    TurnStep, Zone,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod generic_policy;
mod mana_policy;
use crate::mana_spend::{payment, spend};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Develop,
    DrawOnly,
}
pub fn can_fund(
    catalog: &Catalog,
    view: &mtg_core::PermittedView,
    cost: ManaCost,
    symbols: &[mtg_contracts::ColorHybridSymbol],
) -> bool {
    mana_policy::hybrid_plan(catalog, view, cost, symbols).is_some()
}

fn definition<'a>(catalog: &'a Catalog, id: &str) -> Result<&'a mtg_contracts::Definition, String> {
    let found = catalog.definition_by_oracle_id(id);
    // Policy tests deliberately exercise catalogs with permuted definitions.
    #[cfg(test)]
    let found = found.or_else(|| catalog.definitions.iter().find(|d| d.oracle_id == id));
    found.ok_or_else(|| format!("public definition missing {id}"))
}
// One distinct card per color. Reassign flexible cards when a later slot
// has no other candidate. The search depth is bounded by the five colors.

pub struct Strategy<'a> {
    catalog: &'a Catalog,
    policy: Policy,
    mulligans: [u8; 4],
    mana_plan: std::collections::VecDeque<mana_policy::PlannedMana>,
    mana_color: Option<Color>,
    mana_ability: Option<mtg_core::ManaAbilityIdentity>,
    payment: PaymentAttempt,
    hybrid_choices: Vec<mtg_contracts::HybridPayment>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum PaymentAction {
    Cast(String, Zone),
    Activate(ObjectHandle, usize),
}

#[derive(Default)]
struct PaymentAttempt {
    action: Option<PaymentAction>,
    exclude: Option<ObjectHandle>,
    announced_cost: Option<ManaCost>,
    turn: u64,
    failed: [Vec<PaymentAction>; 4],
}
impl PaymentAttempt {
    fn next_priority(&mut self, view: &mtg_core::PermittedView) {
        self.action = None;
        self.exclude = None;
        self.announced_cost = None;
        if self.turn != view.turn.number {
            self.turn = view.turn.number;
            self.failed.iter_mut().for_each(Vec::clear);
        }
    }

    fn abandoned(&mut self, player: PlayerId) {
        if let Some(action) = self.action.take() {
            self.failed[usize::from(player.0)].push(action);
        }
    }
}

impl<'a> Strategy<'a> {
    pub fn new(catalog: &'a Catalog, policy: Policy, mulligans: [u8; 4]) -> Self {
        Self {
            catalog,
            policy,
            mulligans,
            mana_plan: Default::default(),
            mana_color: None,
            mana_ability: None,
            payment: PaymentAttempt::default(),
            hybrid_choices: vec![],
        }
    }
}
impl Strategy<'_> {
    pub fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let request = o.decision.as_ref().ok_or("missing match request")?;
        Ok(match request {
            PlayerRequest::CounterPlacementEffect { candidates, .. } => {
                PlayerResponse::CounterPlacementEffect {
                    effect: candidates
                        .first()
                        .ok_or("empty counter replacement choices")?
                        .effect
                        .clone(),
                }
            }

            // The same owner-only look payload is used by the optional
            // reveal-for-mana continuation. Resolve the choice using its
            // public IR and the ordinary decision token.
            PlayerRequest::LegendRule { candidates, .. } => self.legend_choice(candidates)?,
            PlayerRequest::AdditionalCosts { .. } => {
                PlayerResponse::AdditionalCosts { selected: vec![] }
            }
            PlayerRequest::HybridCost { symbols, .. } => {
                let cached = std::mem::take(&mut self.hybrid_choices);
                let colors = if cached.len() == symbols.len()
                    && cached
                        .iter()
                        .zip(symbols)
                        .all(|(choice, symbol)| symbol.colors.contains(choice))
                {
                    cached
                } else {
                    let (definition, _) = self
                        .top_program(v)
                        .ok_or("hybrid announcement missing public spell")?;
                    let (choices, plan) = mana_policy::hybrid_plan(
                        self.catalog,
                        v,
                        definition.characteristics.mana_cost.clone(),
                        symbols,
                    )
                    .ok_or("no affordable hybrid announcement")?;
                    self.mana_plan = plan;
                    choices
                };
                PlayerResponse::HybridCost { colors }
            }
            PlayerRequest::AnnounceX { .. } => PlayerResponse::AnnounceX {
                value: self
                    .top_program(v)
                    .and_then(|(d, p)| self.choose_x(v, d, p))
                    .unwrap_or_default(),
            },
            PlayerRequest::ChooseManaAbility { choices, .. } => {
                let desired = self.mana_color;
                use mtg_core::ManaAbilityIdentity::{GrantedLandTap, Intrinsic};
                let ability = self
                    .mana_ability
                    .take()
                    .filter(|choice| choices.contains(choice))
                    .or_else(|| choices.iter().copied().find(|choice|
                        matches!(choice, Intrinsic { color } | GrantedLandTap { color } if Some(*color)==desired)))
                    .or_else(|| choices.first().copied())
                    .ok_or("empty mana ability choices")?;
                if matches!(ability, Intrinsic { .. } | GrantedLandTap { .. }) {
                    self.mana_color = None;
                }
                PlayerResponse::ChooseManaAbility { ability }
            }
            PlayerRequest::ManaCombination { .. } => {
                return Err("no admitted mana ability requests a mana combination".into());
            }
            PlayerRequest::ManaColor { colors, .. } => PlayerResponse::ManaColor {
                color: self
                    .mana_color
                    .take()
                    .filter(|c| colors.contains(c))
                    .or_else(|| colors.first().copied())
                    .ok_or("empty mana colors")?,
            },
            PlayerRequest::EntryReveal { candidates, .. } => PlayerResponse::EntryReveal {
                card: candidates.first().cloned(),
            },

            PlayerRequest::EntryCopy { candidates, .. } => PlayerResponse::EntryCopy {
                object: self.best_permanent(v, candidates),
            },
            PlayerRequest::EntryAttachment { candidates, .. } => PlayerResponse::EntryAttachment {
                object: self
                    .best_permanent(v, candidates)
                    .ok_or("empty entry attachments")?,
            },
            PlayerRequest::GraveyardReplacementOrder { .. } => {
                PlayerResponse::OptionalEffect { perform: true }
            }

            PlayerRequest::SacrificeCost { candidates, .. } => PlayerResponse::SacrificeCost {
                object: self.sacrifice_choice(v, candidates)?,
            },
            PlayerRequest::ForcedSacrifice { candidates, .. } => PlayerResponse::ForcedSacrifice {
                object: self.sacrifice_choice(v, candidates)?,
            },

            PlayerRequest::OptionalEffect { .. } => PlayerResponse::OptionalEffect {
                perform: self.optional_effect(
                    v,
                    o.decision_context
                        .as_ref()
                        .and_then(|context| context.instruction.as_ref())
                        .map(|instruction| instruction.source_block),
                ),
            },

            PlayerRequest::OpeningPlacement { candidates, .. } => {
                PlayerResponse::OpeningPlacement {
                    object: candidates.first().cloned(),
                }
            }
            PlayerRequest::Mulligan {
                mulligans_taken,
                can_mulligan,
                ..
            } => PlayerResponse::Mulligan {
                take: *can_mulligan && *mulligans_taken < self.mulligans[usize::from(v.viewer.0)],
            },
            PlayerRequest::BottomOpening {
                count, candidates, ..
            } => PlayerResponse::BottomOpening {
                objects: candidates.iter().rev().take(*count).cloned().collect(),
            },
            PlayerRequest::Priority { .. } => return self.priority(o),
            PlayerRequest::Modes { .. } => {
                return Err("no admitted spell offers modes".into());
            }
            PlayerRequest::Targets {
                occurrences,
                candidates,
                ..
            } => {
                let (d, p) = self.top_program(v).ok_or("missing target program")?;
                let objects = occurrences
                    .iter()
                    .map(|t| {
                        self.object_target(v, d, p, t, Some(candidates))
                            .or_else(|| candidates.first().cloned())
                            .ok_or("empty object targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::Targets { objects }
            }
            PlayerRequest::PlayerTargets {
                occurrences,
                candidates,
                ..
            } => {
                let (d, p) = self.top_program(v).ok_or("missing player program")?;
                let players = occurrences
                    .iter()
                    .map(|t| {
                        self.player_target(v, candidates, self.target_beneficial(d, p, t.slot))
                            .ok_or("empty player targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::PlayerTargets { players }
            }
            PlayerRequest::EntityTargets {
                occurrences,
                candidates,
                ..
            } => {
                if let [owner, cards] = occurrences.as_slice()
                    && let mtg_contracts::TargetDomain::PlayerGraveyardCards {
                        owner_slot,
                        maximum,
                    } = cards.domain
                    && owner.slot == owner_slot
                    && owner.domain == mtg_contracts::TargetDomain::AnyPlayer
                {
                    let offered: Vec<_> = candidates
                        .iter()
                        .filter_map(|c| match c {
                            PlayerTargetEntity::Player { player } => Some(*player),
                            _ => None,
                        })
                        .collect();
                    let player = self
                        .player_target(v, &offered, true)
                        .ok_or("missing offered graveyard owner")?;
                    let mut entities = vec![PlayerTargetEntity::Player { player }];
                    // A single library move is sufficient for this mode.
                    // Avoid a simultaneous group of writes to the same library
                    // in the finite executor.
                    if maximum > 0
                        && let Some(card) = candidates.iter().find(|candidate| {
                            matches!(candidate, PlayerTargetEntity::Object { object } if
                                v.objects.iter().any(|o| &o.handle == object
                                    && o.zone == (Zone::Graveyard { owner: player })
                                    && o.token.is_none()))
                        })
                    {
                        entities.push(card.clone());
                    }
                    return Ok(PlayerResponse::EntityTargets { entities });
                }
                let (_, p) = self.top_program(v).ok_or("missing damage program")?;
                let mut entities: Vec<PlayerTargetEntity> = vec![];
                for _ in occurrences {
                    let distinct: Vec<_> = candidates
                        .iter()
                        .filter(|c| !entities.contains(*c))
                        .cloned()
                        .collect();
                    entities.push(
                        self.damage_target(v, p, &distinct)
                            .or_else(|| self.damage_target(v, p, candidates))
                            .ok_or("empty entity targets")?,
                    );
                }
                PlayerResponse::EntityTargets { entities }
            }
            PlayerRequest::StackTargets {
                occurrences,
                candidates,
                ..
            } => {
                let entries = occurrences
                    .iter()
                    .map(|t| {
                        self.stack_target(v, t, Some(candidates))
                            .or_else(|| candidates.first().cloned())
                            .ok_or("empty stack targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::StackTargets { entries }
            }

            PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
                order: candidates.iter().map(|c| c.handle.clone()).collect(),
            },
            PlayerRequest::Mana {
                cost,
                mana_ability_source,
                ..
            } => {
                let outer = mana_ability_source.is_none();
                let purpose = if let Some(source) = mana_ability_source {
                    Some(mana_policy::ManaPurpose::activation(v, source))
                } else {
                    match &self.payment.action {
                        Some(PaymentAction::Activate(source, _)) => {
                            Some(mana_policy::ManaPurpose::activation(v, source))
                        }
                        Some(PaymentAction::Cast(oracle_id, _)) => {
                            definition(self.catalog, oracle_id).ok().map(|definition| {
                                mana_policy::ManaPurpose::Cast(
                                    mtg_contracts::ManaSpellTraits::from_characteristics(
                                        &definition.characteristics,
                                    ),
                                )
                            })
                        }
                        _ => None,
                    }
                };
                let available =
                    mana_policy::spendable_pool(v, purpose).ok_or("missing mana payer")?;
                let payable = payment(cost, &available).is_some();
                if outer && self.payment.announced_cost.as_ref() != Some(cost) {
                    // Modes, X, tax and additional costs can change the final
                    // price. Nested activations consume the outer plan in order.
                    if !payable {
                        self.mana_plan = mana_policy::plan_for(
                            self.catalog,
                            v,
                            cost,
                            self.payment.exclude.as_ref(),
                            purpose,
                        )
                        .unwrap_or_default();
                    }
                    self.payment.announced_cost = Some(cost.clone());
                }
                if payable {
                    PlayerResponse::FinishMana { undo_mana: true }
                } else if let Some(next) = self.mana_plan.pop_front() {
                    self.mana_color = next.color;
                    self.mana_ability = next.ability;
                    PlayerResponse::ActivateMana {
                        object: next.source,
                    }
                } else {
                    if outer {
                        self.payment.abandoned(v.viewer);
                    }
                    PlayerResponse::FinishMana { undo_mana: true }
                }
            }

            PlayerRequest::SpendMana {
                cost,
                available,
                restricted_available,
                ..
            } => crate::mana_spend::spend_response(cost, available, restricted_available)
                .ok_or("cannot spend announced cost")?,
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
                    .min_by_key(|p| (p.0 + 4 - v.viewer.0) % 4);
                let attacks = if self.policy == Policy::DrawOnly {
                    vec![]
                } else {
                    candidates
                        .iter()
                        .filter(|h| {
                            v.objects.iter().any(|x| {
                                &x.handle == *h
                                    && !self.reserve_reactive_tap(x)
                                    && x.characteristics
                                        .as_ref()
                                        .and_then(|c| c.power.as_ref())
                                        .is_some_and(|p| p > &Quantity::zero())
                            })
                        })
                        .filter_map(|h| defender.map(|p| (h.clone(), p)))
                        .collect()
                };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers {
                legal_attackers,
                menace_attackers,
                ..
            } => {
                let mut blocks = vec![];
                let mut blocked = Vec::new();
                if self.policy == Policy::Develop {
                    for hint in legal_attackers {
                        let blocker = v
                            .objects
                            .iter()
                            .find(|o| o.handle == hint.blocker)
                            .ok_or("missing public blocker")?;
                        let bd = definition(self.catalog, &blocker.oracle_id).ok();
                        let bd_keywords = { Some(blocker.keywords.as_slice()) }
                            .ok_or("missing public blocker keywords")?;
                        let toughness = blocker
                            .characteristics
                            .as_ref()
                            .or_else(|| bd.map(|d| &d.characteristics))
                            .ok_or("missing public blocker characteristics")?
                            .toughness
                            .as_ref()
                            .ok_or("missing blocker toughness")?;
                        for attacker in &hint.attackers {
                            if blocked.contains(attacker) {
                                continue;
                            }
                            let a = v
                                .objects
                                .iter()
                                .find(|o| &o.handle == attacker)
                                .ok_or("missing public attacker")?;
                            let ad = definition(self.catalog, &a.oracle_id).ok();
                            let ad_keywords = { Some(a.keywords.as_slice()) }
                                .ok_or("missing public attacker keywords")?;
                            let power = a
                                .characteristics
                                .as_ref()
                                .or_else(|| ad.map(|d| &d.characteristics))
                                .ok_or("missing public attacker characteristics")?
                                .power
                                .clone()
                                .unwrap_or_default()
                                .max(Quantity::zero());
                            let mut marked = blocker.marked_damage.clone();
                            marked.add(&power);
                            let survives = marked < *toughness
                                && (power == Quantity::zero()
                                    || !ad_keywords
                                        .contains(&mtg_contracts::KeywordAbility::Deathtouch));
                            if survives
                                || bd_keywords.contains(&mtg_contracts::KeywordAbility::Deathtouch)
                            {
                                blocks.push((hint.blocker.clone(), attacker.clone()));
                                blocked.push(attacker.clone());
                                break;
                            }
                        }
                    }
                }
                // CR702.111b: complete a menace pair with a remaining legal
                // blocker, or withdraw the singleton. Observation/catalog only.
                for attacker in menace_attackers {
                    if blocks.iter().filter(|(_, a)| a == attacker).count() == 1 {
                        if let Some(hint) = legal_attackers.iter().find(|h| {
                            h.attackers.contains(attacker)
                                && !blocks.iter().any(|(b, _)| b == &h.blocker)
                        }) {
                            blocks.push((hint.blocker.clone(), attacker.clone()));
                        } else {
                            blocks.retain(|(_, a)| a != attacker);
                        }
                    }
                }
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage {
                power,
                recipients,
                player_recipient: Some(player),
                lethal_requirements,
                ..
            } => {
                let mut remaining = power.clone();
                let mut assignments = vec![];
                for h in recipients {
                    let lethal = lethal_requirements
                        .iter()
                        .find(|(o, _)| o == h)
                        .ok_or("missing public lethal requirement")?
                        .1
                        .clone();
                    let amount = lethal.min(remaining.clone());
                    remaining.subtract(&amount);
                    assignments.push((h.clone(), amount));
                }
                PlayerResponse::TrampleDamage {
                    assignments,
                    player: *player,
                    amount: remaining,
                }
            }
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
            }
            | PlayerRequest::ResolveDiscard {
                count, candidates, ..
            } => {
                let objects: Vec<ObjectHandle> = candidates.iter().take(*count).cloned().collect();
                if matches!(request, PlayerRequest::Discard { .. }) {
                    PlayerResponse::Discard { objects }
                } else {
                    PlayerResponse::ResolveDiscard { objects }
                }
            }
            PlayerRequest::Sacrifice { candidates, .. } => PlayerResponse::Sacrifice {
                object: { self.sacrifice_choice(v, candidates)? },
            },
            PlayerRequest::DamageEffect { candidates, .. } => {
                let c = candidates.first().ok_or("empty damage effects")?;
                PlayerResponse::DamageEffect {
                    effect: c.effect.clone(),
                    damage: c.damage.clone(),
                }
            }
        })
    }
}
