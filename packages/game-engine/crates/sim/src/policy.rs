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
pub(crate) use mana_policy::available_tap;
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

// Reserve every possible convoker while planning real mana. A land creature
// may otherwise be planned both as a mana source and as a convoker.
fn tap_payment_plan<'a>(
    view: &'a mtg_core::PermittedView,
    cost: &ManaCost,
    kind: CardKind,
) -> (ManaCost, Vec<&'a ObjectHandle>) {
    let permanents: Vec<_> = view
        .objects
        .iter()
        .filter(|object| {
            object.zone == Zone::Battlefield
                && object.controller == view.viewer
                && !object.tapped
                && !object.phased_out
                && object
                    .characteristics
                    .as_ref()
                    .is_some_and(|c| c.has_type(kind))
        })
        .collect();
    let mut due = cost.clone();
    if permanents.is_empty() {
        return (due, vec![]);
    }
    let colors: std::collections::BTreeSet<_> = cost
        .colored
        .iter()
        .copied()
        .filter(|&c| kind == CardKind::Creature && c != Color::C)
        .collect();
    let mut slots: Vec<_> = colors
        .iter()
        .map(|color| {
            let allowed: Vec<_> = permanents
                .iter()
                .enumerate()
                .filter(|(_, o)| {
                    o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.colors.contains(color))
                })
                .map(|(i, _)| i as u32)
                .collect();
            mtg_core::Slot {
                role: mtg_core::SlotRole::Choice,
                subject: None,
                min: 0,
                max: (cost.colored.iter().filter(|c| *c == color).count() as u32)
                    .min(allowed.len() as u32),
                allowed,
                repeat: false,
            }
        })
        .collect();
    slots.push(mtg_core::Slot {
        role: mtg_core::SlotRole::Choice,
        subject: None,
        min: 0,
        max: (permanents.len() as u32).min(cost.generic.to_u32().unwrap_or(u32::MAX)),
        allowed: (0..permanents.len() as u32).collect(),
        repeat: false,
    });
    let mut picked = mtg_core::distinct_payment_choices(&slots, 0, permanents.len() as u32)
        .expect("convoke planning permits no picks");
    let generic = picked.pop().unwrap_or_default().len();
    due.generic.subtract(&Quantity::from(generic as i64));
    for (color, picks) in colors.into_iter().zip(picked) {
        for _ in picks {
            let index = due
                .colored
                .iter()
                .position(|symbol| *symbol == color)
                .expect("convoke slot is capped by printed symbols");
            due.colored.remove(index);
        }
    }
    (
        due,
        permanents
            .into_iter()
            .map(|object| &object.handle)
            .collect(),
    )
}

fn delve_mana_due(
    mut cost: ManaCost,
    view: &mtg_core::PermittedView,
    casting_card: Option<&ObjectHandle>,
) -> ManaCost {
    let graveyard = Quantity::from(
        view.objects
            .iter()
            .filter(|object| {
                object.zone == Zone::Graveyard { owner: view.viewer }
                    && casting_card != Some(&object.handle)
            })
            .count() as i64,
    );
    cost.generic.subtract(&cost.generic.clone().min(graveyard));
    cost
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
    mana_bundle: Option<Vec<Color>>,
    mana_ability: Option<mtg_core::ManaAbilityIdentity>,
    payment: PaymentAttempt,
    planned_x: Option<Quantity>,
    hybrid_choices: Vec<mtg_contracts::HybridPayment>,
    /// CR 118.9: the alternative cost the planned cast announces.
    alternative_cost: Option<String>,
    /// The activations the pending priority decision offers, when one is
    /// published; Develop never selects an activation outside them.
    offered_activations: Option<Vec<Action<Viewer>>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum PaymentAction {
    Cast(String, Zone),
    /// CR 702.37a: a face-down cast of this card.
    CastFaceDown(ObjectHandle),
    /// CR 702.37e: turning this permanent face up.
    TurnFaceUp(ObjectHandle),
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
        self.mana_bundle = None;
        self.mana_ability = None;
        self.hybrid_choices.clear();
        self.alternative_cost = None;
        self.planned_x = None;
    }

    pub fn choose(
        &mut self,
        catalog: &Catalog,
        policy: Policy,
        observation: &Observation,
    ) -> Result<Answer, String> {
        self.choose_inner(catalog, policy, observation, None)
    }

    pub fn choose_with_game(
        &mut self,
        catalog: &Catalog,
        policy: Policy,
        observation: &Observation,
        game: &mtg_core::Game,
    ) -> Result<Answer, String> {
        self.choose_inner(catalog, policy, observation, Some(game))
    }

    fn choose_inner(
        &mut self,
        catalog: &Catalog,
        policy: Policy,
        observation: &Observation,
        game: Option<&mtg_core::Game>,
    ) -> Result<Answer, String> {
        let mut strategy = Strategy {
            catalog,
            policy,
            memory: std::mem::take(self),
        };
        let answer = strategy.choose_inner(observation, game);
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
        self.choose_inner(o, None)
    }

    pub fn choose_with_game(
        &mut self,
        o: &Observation,
        game: &mtg_core::Game,
    ) -> Result<Answer, String> {
        self.choose_inner(o, Some(game))
    }

    fn choose_inner(
        &mut self,
        o: &Observation,
        game: Option<&mtg_core::Game>,
    ) -> Result<Answer, String> {
        let v = &o.view;
        let d = o.decision.as_ref().ok_or("missing match decision")?;
        let object = |i: u32| match d.candidate(i)? {
            Candidate::Object { object } => Ok(object.clone()),
            _ => Err("expected object".to_owned()),
        };
        let answer = match (&d.reason, &d.shape) {
            (Reason::Priority, _) => d.pick_one(&Candidate::Action {
                action: self.priority_with_game(o, game)?,
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
            // Reveal at entry or choose an offered resolution payment object.
            // A filtered discard can avoid the larger default discard.
            (Reason::OptionalEffect, Shape::Pick { slots, .. })
                if slots.first().is_some_and(|slot| {
                    slot.allowed
                        .iter()
                        .any(|i| matches!(d.candidate(*i), Ok(Candidate::Object { .. })))
                }) =>
            {
                let index = slots[0]
                    .allowed
                    .iter()
                    .copied()
                    .find(|i| matches!(d.candidate(*i), Ok(Candidate::Object { .. })))
                    .ok_or("missing optional object")?;
                d.pick_one(d.candidate(index)?)?
            }
            // A reveal with no matching hand card offers Decline alone.
            (Reason::OptionalEffect, _)
                if d.index(&Candidate::Flag { flag: Flag::Accept }).is_none() =>
            {
                flag(d, Flag::Decline)?
            }
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
            // CR 601.2b-c: the announced X stays within the rules bounds
            // that the spell's targets impose.
            (Reason::AnnounceX, Shape::Number { min, max }) => {
                let planned = self
                    .memory
                    .planned_x
                    .take()
                    .or_else(|| {
                        self.top_program(v)
                            .and_then(|(d, p)| self.choose_x(v, d, p, None))
                    })
                    .unwrap_or_default();
                let capped = max
                    .as_ref()
                    .map_or(planned.clone(), |max| planned.min(max.clone()));
                Answer::Number {
                    value: capped.max(min.clone()),
                }
            }
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
                    pool,
                },
            ) => {
                let outer = mana_ability_source.is_none();
                let available = pool.available();
                let spell = outer
                    .then(|| self.top_program(v))
                    .flatten()
                    .map(|(definition, _)| definition);
                let (due, reserved) = if spell.is_some_and(|d| d.convoke) {
                    tap_payment_plan(v, cost, CardKind::Creature)
                } else if spell.is_some_and(|d| d.improvise) {
                    tap_payment_plan(v, cost, CardKind::Artifact)
                } else if spell.is_some_and(|d| d.delve) {
                    (delve_mana_due(cost.clone(), v, None), vec![])
                } else {
                    (cost.clone(), vec![])
                };
                let payable = if spell.is_some_and(|d| d.convoke || d.delve || d.improvise)
                    && let Some(game) = game
                {
                    game.quote_finish_spell_mana(v.viewer)?
                } else {
                    payment(cost, &available).is_some() || payment(&due, &available).is_some()
                };
                if outer && self.memory.payment.announced_cost.as_ref() != Some(cost) {
                    // Modes, X, tax and additional costs can change the final
                    // price. Nested activations consume the outer plan in order.
                    if !payable {
                        self.memory.mana_plan = mana_policy::plan_with_reservations(
                            self.catalog,
                            v,
                            &due,
                            self.memory.payment.exclude.as_ref(),
                            &pool.restricted,
                            &reserved,
                        )
                        .or_else(|| {
                            mana_policy::plan_for_payment(
                                self.catalog,
                                v,
                                cost,
                                self.memory.payment.exclude.as_ref(),
                                &pool.restricted,
                            )
                        })
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
                    self.memory.mana_bundle = next.bundle;
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
            ) => crate::mana_spend::pool_spend_answer(cost, pool)
                .ok_or("cannot spend announced cost")?,
            (Reason::Targets { specs }, Shape::Pick { slots, rules }) => {
                let (definition, program) = self.top_program(v).ok_or("missing target program")?;
                let mut picks = Vec::<Vec<u32>>::new();
                for (ordinal, (slot, spec)) in slots.iter().zip(specs).enumerate() {
                    let allowed:Vec<_>=slot.allowed.iter().copied().filter(|i|!rules.iter().any(|rule|
                        matches!(rule, JointRule::Distinct{slots} if slots.contains(&(ordinal as u32)) && slots.iter().any(|s|picks.get(*s as usize).is_some_and(|p|p.contains(i)))))).collect();
                    if allowed.is_empty() && slot.min == 0 {
                        picks.push(vec![]);
                        continue;
                    }
                    if matches!(
                        spec.domain,
                        mtg_contracts::TargetDomain::StackSpell
                            | mtg_contracts::TargetDomain::ControlledCreatureSpell
                            | mtg_contracts::TargetDomain::StackActivatedOrTriggeredAbility
                    ) {
                        let handles = allowed
                            .iter()
                            .map(|&i| match d.candidate(i)? {
                                Candidate::Stack { stack } => Ok(stack.clone()),
                                _ => Err("invalid stack target candidate".to_owned()),
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let target = self.stack_target(v, spec, Some(&handles));
                        let index = target
                            .and_then(|h| handles.iter().position(|row| *row == h))
                            .and_then(|n| allowed.get(n))
                            .or_else(|| allowed.first());
                        picks.push(index.copied().into_iter().collect());
                        continue;
                    }
                    // CR 115.3: a counted group slot names `min` distinct
                    // objects; choose them one at a time from the rest. An
                    // up-to-X group continues while a desirable object
                    // remains; an X of zero announces none (CR 601.2c).
                    let mut allowed = allowed;
                    let mut chosen_slot = vec![];
                    let up_to_x = program.variable_target_count()
                        == Some(mtg_contracts::kir::VariableTargetCount::UpToX);
                    let count = if up_to_x {
                        slot.max
                    } else {
                        slot.min.max(1).min(slot.max)
                    };
                    for picked in 0..count {
                        if allowed.is_empty() && picked >= slot.min {
                            break;
                        }
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
                            | mtg_contracts::TargetDomain::OpponentPlayer
                            | mtg_contracts::TargetDomain::PlayerOrPlaneswalker => {
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
                        if up_to_x && chosen.is_none() && picked >= slot.min {
                            break;
                        }
                        let n = chosen
                            .and_then(|e| entities.iter().position(|c| *c == e))
                            .unwrap_or(0);
                        chosen_slot.push(*allowed.get(n).ok_or("empty target slot")?);
                        allowed.remove(n);
                    }
                    picks.push(chosen_slot);
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
                    // CR 508.1d: a required attacker with a legal defender attacks.
                    picks.push(
                        defender
                            .filter(|_| attack || slot.min > 0)
                            .into_iter()
                            .collect(),
                    );
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
                // CR 509.1b, 702.111b: (least, greatest) blockers of each
                // attacker named by a count rule.
                let mut counted: Vec<(_, usize, Option<usize>)> = vec![];
                for r in rules {
                    let (candidates, least, most) = match r {
                        JointRule::PickedAtLeast { candidates, count } => {
                            (candidates, *count as usize, None)
                        }
                        JointRule::PickedAtMost { candidates, count } => {
                            (candidates, 1, Some(*count as usize))
                        }
                        _ => continue,
                    };
                    for c in candidates {
                        let attacker = object(*c)?;
                        match counted.iter_mut().find(|(a, _, _)| *a == attacker) {
                            Some((_, l, m)) => {
                                *l = (*l).max(least);
                                *m = match (*m, most) {
                                    (Some(a), Some(b)) => Some(a.min(b)),
                                    (a, b) => a.or(b),
                                };
                            }
                            None => counted.push((attacker, least, most)),
                        }
                    }
                }
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
                // CR 509.1b, 702.111b: complete a counted block with remaining
                // legal blockers, or withdraw it. Observation/catalog only.
                for (attacker, least, most) in &counted {
                    let n = blocks.iter().filter(|(_, a)| a == attacker).count();
                    if n == 0 {
                        continue;
                    }
                    let mut added = vec![];
                    while n + added.len() < *least {
                        let Some((blocker_handle, _)) =
                            legal_attackers.iter().find(|(blocker, allowed)| {
                                allowed.contains(attacker)
                                    && !blocks.iter().any(|(b, _)| b == blocker)
                                    && !added.contains(blocker)
                            })
                        else {
                            break;
                        };
                        added.push(blocker_handle.clone());
                    }
                    // CR 509.1b: least above most leaves no legal block.
                    let total = n + added.len();
                    if total < *least || most.is_some_and(|m| total > m) {
                        blocks.retain(|(_, a)| a != attacker);
                    } else {
                        blocks.extend(added.into_iter().map(|b| (b, attacker.clone())));
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
                let bundle = self.memory.mana_bundle.take();
                let allowed = &slots[0].allowed;
                let n = allowed
                    .iter()
                    .find(|i| match &d.candidates[**i as usize] {
                        Candidate::Color { color } => Some(*color) == preferred,
                        Candidate::ManaBundle { symbols } => Some(symbols) == bundle.as_ref(),
                        _ => false,
                    })
                    .or(allowed.first())
                    .ok_or("empty mana production slot")?;
                Answer::Pick {
                    picks: vec![vec![*n]],
                }
            }
            (Reason::LegendRule { .. }, Shape::Pick { slots, .. }) => Answer::Pick {
                picks: vec![vec![*slots[0].allowed.last().ok_or("empty legend slot")?]],
            },
            // CR 303.4f: enchant the first offered host.
            (Reason::EntryAttachment | Reason::ResolveObjectChoice, Shape::Pick { slots, .. }) => {
                Answer::Pick {
                    picks: slots
                        .iter()
                        .map(|s| s.allowed.iter().take(1).copied().collect())
                        .collect(),
                }
            }
            // CR 701.23a: find as many of the offered library cards as
            // allowed, in the offered order.
            (
                Reason::SearchLibrary | Reason::SearchSplitBattlefield | Reason::LookChooseHand,
                Shape::Pick { slots, .. },
            ) => Answer::Pick {
                picks: slots
                    .iter()
                    .map(|s| s.allowed.iter().take(s.max as usize).copied().collect())
                    .collect(),
            },
            // CR 601.2b: the baseline policy conserves mana by declining
            // optional kicker and buyback costs before target selection, and
            // announces an alternative cost only when its cast planned one.
            (Reason::AdditionalCosts, Shape::Pick { slots, .. }) => {
                let planned = self.memory.alternative_cost.take();
                Answer::Pick {
                    picks: slots
                        .iter()
                        .map(|s| {
                            s.allowed
                                .iter()
                                .copied()
                                .filter(|i| {
                                    // The permitted id is "<stack handle>/cost/<identity>".
                                    matches!(&d.candidates[*i as usize],
                                        Candidate::AdditionalCost { id, .. }
                                            if planned.as_ref().is_some_and(|p| {
                                                id.rsplit_once("/cost/").map(|(_, cost)| cost) == Some(p.as_str())
                                            }))
                                })
                                .take(1)
                                .collect()
                        })
                        .collect(),
                }
            }
            (
                Reason::ChosenObjectCost {
                    action: mtg_contracts::ChosenCostAction::Tap,
                },
                Shape::Pick { slots, rules },
            ) if self
                .top_program(v)
                .is_some_and(|(d, _)| d.convoke || d.improvise) =>
            {
                Answer::Pick {
                    picks: mtg_core::distinct_payment_choices(
                        slots,
                        mtg_core::payment_choice_minimum(rules),
                        mtg_core::payment_choice_minimum(rules),
                    )
                    .ok_or("missing legal convoke allocation")?,
                }
            }
            // CR 701.22a, 701.25a: keep every looked-at card on top.
            (
                Reason::Discard
                | Reason::ChosenObjectCost { .. }
                | Reason::ResolveDiscard
                | Reason::DamageEffect { .. }
                | Reason::ChooseModes
                | Reason::ScryBottom
                | Reason::Populate
                | Reason::Proliferate
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
