//! Policies choose a response from the acting seat's permitted observation.
use mtg_contracts::{CardKind, Catalog, Color, ManaCost, Quantity};
use mtg_core::{
    Action, Answer, Candidate, Decision, Flag, JointRule, ObjectHandle, Observation, PayAnswer,
    PayStep, PlayerId, PlayerTargetEntity, Reason, Shape, TurnStep, Viewer, Zone,
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
    memory: PolicyMemory,
}
/// Payment choices spanning multiple published decisions in one game.
#[derive(Default)]
pub struct PolicyMemory {
    mulligans: [u8; 4],
    mana_plan: std::collections::VecDeque<mana_policy::PlannedMana>,
    mana_color: Option<Color>,
    mana_ability: Option<mtg_core::ManaAbilityIdentity>,
    payment: PaymentAttempt,
    hybrid_choices: Vec<mtg_contracts::HybridPayment>,
    /// The activations the pending priority decision offers, when one is
    /// published; Develop never selects an activation outside them.
    offered_activations: Option<Vec<Action<Viewer>>>,
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
            memory: PolicyMemory {
                mulligans,
                ..Default::default()
            },
        }
    }
}
impl PolicyMemory {
    /// An external agent selected the priority action. Discard plans and
    /// exclusions associated with the previous announcement.
    pub fn external_priority(&mut self, view: &mtg_core::PermittedView) {
        self.payment.next_priority(view);
        self.mana_plan.clear();
        self.mana_color = None;
        self.mana_ability = None;
        self.hybrid_choices.clear();
    }

    pub fn choose(
        &mut self,
        catalog: &Catalog,
        policy: Policy,
        observation: &Observation,
    ) -> Result<Answer, String> {
        let mut strategy = Strategy {
            catalog,
            policy,
            memory: std::mem::take(self),
        };
        let answer = strategy.choose(observation);
        *self = strategy.memory;
        answer
    }
}
pub(crate) fn payment_source(d: &Decision<Viewer>, object: ObjectHandle) -> Result<Answer, String> {
    let source = d
        .index(&Candidate::Action {
            action: Action::ActivateMana { object },
        })
        .ok_or("payment source not offered")?;
    Ok(Answer::Pay {
        pay: PayAnswer::Activate { source },
    })
}
pub(crate) fn flag(d: &Decision<Viewer>, flag: Flag) -> Result<Answer, String> {
    d.pick_one(&Candidate::Flag { flag })
}

pub(crate) fn damage_assignment(
    total: &Quantity,
    buckets: &[mtg_core::Bucket],
    gates: &[mtg_core::Gate],
) -> Answer {
    let mut amounts = vec![Quantity::zero(); buckets.len()];
    let mut remaining = total.clone();
    if let Some(gate) = gates.first() {
        for (index, required) in &gate.requires {
            let amount = required.clone().min(remaining.clone());
            remaining.subtract(&amount);
            amounts[*index as usize] = amount;
        }
        amounts[gate.bucket as usize] = remaining;
    } else if let Some(first) = amounts.first_mut() {
        *first = remaining;
    }
    Answer::Distribute { amounts }
}
impl Strategy<'_> {
    pub fn choose(&mut self, o: &Observation) -> Result<Answer, String> {
        let v = &o.view;
        let d = o.decision.as_ref().ok_or("missing match decision")?;
        let object = |i: u32| match d.candidate(i)? {
            Candidate::Object { object } => Ok(object.clone()),
            _ => Err("expected object".to_owned()),
        };
        let answer = match (&d.reason, &d.shape) {
            (Reason::Priority, _) => d.pick_one(&Candidate::Action {
                action: self.priority(o)?,
            })?,
            (
                Reason::Mulligan {
                    mulligans_taken, ..
                },
                _,
            ) => {
                let take = *mulligans_taken < self.memory.mulligans[usize::from(v.viewer.0)]
                    && d.index(&Candidate::Flag {
                        flag: Flag::Mulligan,
                    })
                    .is_some();
                flag(d, if take { Flag::Mulligan } else { Flag::Keep })?
            }
            (Reason::CommanderReturn { .. }, _) => flag(d, Flag::CommandZone)?,
            (Reason::OptionalEffect, _) => flag(
                d,
                if self.optional_effect(
                    v,
                    o.decision_context
                        .as_ref()
                        .and_then(|c| c.instruction.as_ref())
                        .map(|i| i.source_block),
                ) {
                    Flag::Accept
                } else {
                    Flag::Decline
                },
            )?,
            (Reason::GraveyardReplacementOrder { .. }, _) => flag(d, Flag::Accept)?,
            (Reason::AnnounceX, Shape::Number { .. }) => Answer::Number {
                value: self
                    .top_program(v)
                    .and_then(|(d, p)| self.choose_x(v, d, p))
                    .unwrap_or_default(),
            },
            (Reason::BottomOpening, Shape::Order { items, min, .. }) => Answer::Order {
                order: items.iter().rev().take(*min as usize).copied().collect(),
            },
            (_, Shape::Order { items, min, .. }) => Answer::Order {
                order: items.iter().take(*min as usize).copied().collect(),
            },
            (
                _,
                Shape::Distribute {
                    total,
                    buckets,
                    gates,
                },
            ) => damage_assignment(total, buckets, gates),
            (
                Reason::Mana {
                    source: mana_ability_source,
                },
                Shape::Pay {
                    cost,
                    step: PayStep::Produce { .. },
                    ..
                },
            ) => {
                let outer = mana_ability_source.is_none();
                let available = mana_policy::spendable_pool(v).ok_or("missing mana payer")?;
                let payable = payment(cost, &available).is_some();
                if outer && self.memory.payment.announced_cost.as_ref() != Some(cost) {
                    // Modes, X, tax and additional costs can change the final
                    // price. Nested activations consume the outer plan in order.
                    if !payable {
                        self.memory.mana_plan = mana_policy::plan_for(
                            self.catalog,
                            v,
                            cost,
                            self.memory.payment.exclude.as_ref(),
                        )
                        .unwrap_or_default();
                    }
                    self.memory.payment.announced_cost = Some(cost.clone());
                }
                if payable {
                    Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: true,
                        },
                    }
                } else if let Some(next) = self.memory.mana_plan.pop_front() {
                    self.memory.mana_color = next.color;
                    self.memory.mana_ability = next.ability;
                    payment_source(d, next.source)?
                } else {
                    if outer {
                        self.memory.payment.abandoned(v.viewer);
                    }
                    Answer::Pay {
                        pay: PayAnswer::Finish {
                            undo_on_failure: true,
                        },
                    }
                }
            }
            (
                _,
                Shape::Pay {
                    cost,
                    pool,
                    step: PayStep::Spend,
                },
            ) => crate::mana_spend::spend_answer(cost, &pool.unrestricted)
                .ok_or("cannot spend announced cost")?,
            (Reason::Targets { specs }, Shape::Pick { slots, rules }) => {
                let (definition, program) = self.top_program(v).ok_or("missing target program")?;
                let mut picks = Vec::<Vec<u32>>::new();
                for (ordinal, (slot, spec)) in slots.iter().zip(specs).enumerate() {
                    let allowed:Vec<_>=slot.allowed.iter().copied().filter(|i|!rules.iter().any(|rule|
                        matches!(rule, JointRule::Distinct{slots} if slots.contains(&(ordinal as u32)) && slots.iter().any(|s|picks.get(*s as usize).is_some_and(|p|p.contains(i)))))).collect();
                    let entities: Vec<_> = allowed
                        .iter()
                        .map(|i| match d.candidate(*i)? {
                            Candidate::Player { player } => {
                                Ok(PlayerTargetEntity::Player { player: *player })
                            }
                            Candidate::Object { object } => Ok(PlayerTargetEntity::Object {
                                object: object.clone(),
                            }),
                            _ => Err("invalid target candidate".to_owned()),
                        })
                        .collect::<Result<_, _>>()?;
                    let chosen = match spec.domain {
                        mtg_contracts::TargetDomain::AnyDamageTarget => {
                            self.damage_target(v, program, &entities)
                        }
                        mtg_contracts::TargetDomain::AnyPlayer
                        | mtg_contracts::TargetDomain::OpponentPlayer => {
                            let players = entities
                                .iter()
                                .filter_map(|e| {
                                    if let PlayerTargetEntity::Player { player } = e {
                                        Some(*player)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();
                            self.player_target(
                                v,
                                &players,
                                self.target_beneficial(definition, program, spec.slot),
                            )
                            .map(|player| PlayerTargetEntity::Player { player })
                        }
                        _ => {
                            let objects = entities
                                .iter()
                                .filter_map(|e| {
                                    if let PlayerTargetEntity::Object { object } = e {
                                        Some(object.clone())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();
                            self.object_target(v, definition, program, spec, Some(&objects))
                                .map(|object| PlayerTargetEntity::Object { object })
                        }
                    };
                    let n = chosen
                        .and_then(|e| entities.iter().position(|c| *c == e))
                        .unwrap_or(0);
                    picks.push(vec![*allowed.get(n).ok_or("empty target slot")?]);
                }
                Answer::Pick { picks }
            }
            (Reason::Attackers, Shape::Pick { slots, .. }) => {
                let mut picks = vec![];
                for slot in slots {
                    let subject = object(slot.subject.ok_or("missing attacker")?)?;
                    let attack = self.policy == Policy::Develop
                        && v.objects.iter().any(|o| {
                            o.handle == subject
                                && !self.reserve_reactive_tap(o)
                                && o.characteristics
                                    .as_ref()
                                    .and_then(|c| c.power.as_ref())
                                    .is_some_and(|p| p > &Quantity::zero())
                        });
                    let defender = slot.allowed.iter().copied().min_by_key(|i| {
                        match &d.candidates[*i as usize] {
                            Candidate::Player { player } => (player.0 + 4 - v.viewer.0) % 4,
                            _ => u8::MAX,
                        }
                    });
                    picks.push(defender.filter(|_| attack).into_iter().collect());
                }
                Answer::Pick { picks }
            }
            (Reason::Blockers, Shape::Pick { slots, rules }) => {
                let legal_attackers = slots
                    .iter()
                    .map(|slot| {
                        Ok((
                            object(slot.subject.ok_or("missing blocker")?)?,
                            slot.allowed
                                .iter()
                                .copied()
                                .map(object)
                                .collect::<Result<Vec<_>, _>>()?,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let menace_attackers = rules
                    .iter()
                    .filter_map(|r| {
                        if let JointRule::PickedAtLeast { candidates, .. } = r {
                            Some(candidates)
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .copied()
                    .map(object)
                    .collect::<Result<Vec<_>, _>>()?;
                let mut blocks = vec![];
                let mut blocked = Vec::new();
                if self.policy == Policy::Develop {
                    for (blocker_handle, attackers) in &legal_attackers {
                        let blocker = v
                            .objects
                            .iter()
                            .find(|o| &o.handle == blocker_handle)
                            .ok_or("missing public blocker")?;
                        let bd = definition(self.catalog, &blocker.oracle_id).ok();
                        let bd_keywords = blocker.keywords.as_slice();
                        let toughness = blocker
                            .characteristics
                            .as_ref()
                            .or_else(|| bd.map(|d| &d.characteristics))
                            .ok_or("missing public blocker characteristics")?
                            .toughness
                            .as_ref()
                            .ok_or("missing blocker toughness")?;
                        for attacker in attackers {
                            if blocked.contains(attacker) {
                                continue;
                            }
                            let a = v
                                .objects
                                .iter()
                                .find(|o| &o.handle == attacker)
                                .ok_or("missing public attacker")?;
                            let ad = definition(self.catalog, &a.oracle_id).ok();
                            let ad_keywords = a.keywords.as_slice();
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
                                blocks.push((blocker_handle.clone(), attacker.clone()));
                                blocked.push(attacker.clone());
                                break;
                            }
                        }
                    }
                }
                // CR702.111b: complete a menace pair with a remaining legal
                // blocker, or withdraw the singleton. Observation/catalog only.
                for attacker in &menace_attackers {
                    if blocks.iter().filter(|(_, a)| a == attacker).count() == 1 {
                        if let Some((blocker_handle, _)) =
                            legal_attackers.iter().find(|(blocker, allowed)| {
                                allowed.contains(attacker)
                                    && !blocks.iter().any(|(b, _)| b == blocker)
                            })
                        {
                            blocks.push((blocker_handle.clone(), attacker.clone()));
                        } else {
                            blocks.retain(|(_, a)| a != attacker);
                        }
                    }
                }

                Answer::Pick {
                    picks: legal_attackers
                        .iter()
                        .map(|(blocker_handle, _)| {
                            blocks
                                .iter()
                                .find(|(b, _)| b == blocker_handle)
                                .and_then(|(_, a)| {
                                    d.index(&Candidate::Object { object: a.clone() })
                                })
                                .into_iter()
                                .collect()
                        })
                        .collect(),
                }
            }
            (
                Reason::Sacrifice | Reason::SacrificeCost | Reason::ForcedSacrifice { .. },
                Shape::Pick { slots, .. },
            ) => {
                let candidates = slots[0]
                    .allowed
                    .iter()
                    .copied()
                    .map(object)
                    .collect::<Result<Vec<_>, _>>()?;
                d.pick_one(&Candidate::Object {
                    object: self.sacrifice_choice(v, &candidates)?,
                })?
            }
            (Reason::HybridCost, Shape::Pick { slots, .. }) => {
                let mut choices = std::mem::take(&mut self.memory.hybrid_choices);
                if choices.len() != slots.len() {
                    let (definition, _) = self.top_program(v).ok_or("missing public spell")?;

                    let (planned, plan) = mana_policy::hybrid_plan(
                        self.catalog,
                        v,
                        definition.characteristics.mana_cost.clone(),
                        &definition.characteristics.hybrid_mana,
                    )
                    .ok_or("no affordable hybrid announcement")?;
                    choices = planned;
                    self.memory.mana_plan = plan;
                }
                Answer::Pick {
                    picks: choices
                        .into_iter()
                        .map(|payment| {
                            d.index(&Candidate::HybridPayment { payment })
                                .map(|i| vec![i])
                                .ok_or("missing hybrid choice")
                        })
                        .collect::<Result<_, _>>()?,
                }
            }
            (Reason::ManaAbility { .. }, Shape::Pick { slots, .. }) => {
                let preferred = self.memory.mana_ability.take();
                let allowed = &slots[0].allowed;
                let n=allowed.iter().find(|i|matches!(d.candidates[**i as usize],Candidate::ManaAbility{ability} if Some(ability)==preferred))
                    .or_else(||allowed.iter().find(|i|matches!(d.candidates[**i as usize],Candidate::ManaAbility{ability:mtg_core::ManaAbilityIdentity::Intrinsic{color}} if Some(color)==self.memory.mana_color))).or(allowed.first()).ok_or("empty mana ability slot")?;
                if matches!(
                    d.candidates[*n as usize],
                    Candidate::ManaAbility {
                        ability: mtg_core::ManaAbilityIdentity::Intrinsic { .. }
                    }
                ) {
                    self.memory.mana_color = None;
                }
                Answer::Pick {
                    picks: vec![vec![*n]],
                }
            }
            (Reason::ManaColor { .. }, Shape::Pick { slots, .. }) => {
                let preferred = self.memory.mana_color.take();
                let allowed = &slots[0].allowed;
                let n=allowed.iter().find(|i|matches!(d.candidates[**i as usize],Candidate::Color{color} if Some(color)==preferred)).or(allowed.first()).ok_or("empty mana color slot")?;
                Answer::Pick {
                    picks: vec![vec![*n]],
                }
            }
            (Reason::LegendRule { .. }, Shape::Pick { slots, .. }) => Answer::Pick {
                picks: vec![vec![*slots[0].allowed.last().ok_or("empty legend slot")?]],
            },
            // CR 303.4f: enchant the first offered host.
            (Reason::EntryAttachment, Shape::Pick { slots, .. }) => Answer::Pick {
                picks: slots
                    .iter()
                    .map(|s| s.allowed.iter().take(1).copied().collect())
                    .collect(),
            },
            // CR 701.23a: find as many of the offered library cards as
            // allowed, in the offered order.
            (Reason::SearchLibrary, Shape::Pick { slots, .. }) => Answer::Pick {
                picks: slots
                    .iter()
                    .map(|s| s.allowed.iter().take(s.max as usize).copied().collect())
                    .collect(),
            },
            // CR 701.22a, 701.25a: keep every looked-at card on top.
            (
                Reason::Discard
                | Reason::ResolveDiscard
                | Reason::DamageEffect { .. }
                | Reason::AdditionalCosts
                | Reason::ChooseModes
                | Reason::ScryBottom
                | Reason::SurveilGraveyard,
                Shape::Pick { slots, .. },
            ) => Answer::Pick {
                picks: slots
                    .iter()
                    .map(|s| s.allowed.iter().take(s.min as usize).copied().collect())
                    .collect(),
            },
            _ => return Err("unrecognized policy decision".into()),
        };
        Ok(answer)
    }
}
