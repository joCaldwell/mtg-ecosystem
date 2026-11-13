//! Finite permanent removal. Pinned CR 115, 202.3a, 406.1–3, 601.2c/h,
//! 602.2b, 603.3d/5, 608.2b/c/h, 701.6a/8a-b/13a/19c, 704.5d/m/n,
//! 903.9a and 903.10a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{EffectRecipient, PermanentFilter, TargetAmount, TargetAttribute};

// Reviewed derivation: docs/game-engine-removal.md. Earlier limits are intact.

/// Current facts refresh while the exact target remains in its expected public
/// zone, then survive as its last appearance. A successor never supplies them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TargetFacts {
    controller: PlayerId,
    power: Option<Quantity>,
    mana_value: Quantity,
}
impl Game {
    fn target_information_frame(frame: &mut Frame) -> Option<&mut Resolving> {
        match frame {
            Frame::Resolve { frame }
            | Frame::ResolutionChoice { frame, .. }
            | Frame::ForcedSacrifice { frame, .. }
            | Frame::OptionalDamage { frame } => Some(frame),
            Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } => {
                match transaction.continuation.as_mut() {
                    DamageContinuation::Instruction { frame } => Some(frame),
                    DamageContinuation::Combat => None,
                }
            }
            Frame::Stabilize { resume, .. } | Frame::LegendRule { resume, .. } => {
                Self::target_information_frame(resume)
            }
            Frame::CommanderReturn { continuation, .. } => match continuation.as_mut() {
                CommanderContinuation::BoardReturn { frame, .. }
                | CommanderContinuation::SpellMove { frame, .. }
                | CommanderContinuation::ResolutionDraw { frame, .. } => Some(frame),
                CommanderContinuation::StateBased { resume }
                | CommanderContinuation::StateBasedBatch { resume, .. } => {
                    Self::target_information_frame(resume)
                }
                CommanderContinuation::Draw { .. } => None,
            },
            _ => None,
        }
    }
    /// CR608.2h: read current information when an effect applies, retaining the
    /// last expected-zone appearance before a move/departure. Refresh on every
    /// atomic step, including concessions between instructions. This never
    /// repeats the resolution-start target legality check (CR608.2b).
    pub(super) fn refresh_removal_target_facts(&mut self) -> Result<(), String> {
        let (entry, legal) = match Self::target_information_frame(&mut self.state.rules.frame) {
            Some(frame) if !frame.target_facts.is_empty() => {
                (frame.spell.clone(), frame.legal_targets.clone())
            }
            _ => return Ok(()),
        };
        let current = self
            .program_for(&entry)?
            .targets
            .iter()
            .enumerate()
            .map(|(slot, spec)| {
                legal.get(slot) == Some(&true)
                    && if spec.domain == TargetDomain::StackSpell {
                        entry.stack_targets.get(slot).is_some_and(|id| {
                            self.state.rules.stack.iter().any(|entry| entry.id == *id)
                        })
                    } else {
                        entry.targets.get(slot).is_some_and(|id| {
                            self.current(*id)
                                .is_ok_and(|object| object.zone == Zone::Battlefield)
                        })
                    }
            })
            .collect::<Vec<_>>();
        let fresh = self.capture_target_facts(&entry, &current)?;
        let frame = Self::target_information_frame(&mut self.state.rules.frame)
            .ok_or("missing target information frame")?;
        for (slot, fact) in fresh.into_iter().enumerate() {
            if fact.is_some() {
                *frame
                    .target_facts
                    .get_mut(slot)
                    .ok_or("missing target information slot")? = fact;
            }
        }
        Ok(())
    }
    fn matches_permanent_filter(
        &self,
        object: ObjectId,
        controller: PlayerId,
        f: &PermanentFilter,
    ) -> bool {
        self.current(object).is_ok_and(|o| {
            o.zone == Zone::Battlefield
                && (!f.opponent || o.controller != controller)
                && (!f.tapped || o.tapped)
        }) && self.definition(object.card).is_ok_and(|d| {
            let c = &d.characteristics;
            !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
                && (f.types.is_empty() || f.types.iter().any(|k| c.has_type(*k)))
                && !f.non_types.iter().any(|k| c.has_type(*k))
                && !c.colors.iter().any(|c| f.non_colors.contains(c))
        })
    }
    pub(super) fn permanent_target_legal(
        &self,
        object: ObjectId,
        controller: PlayerId,
        spec: &TargetSpec,
    ) -> bool {
        self.targetable_by(object, controller)
            && spec
                .filter
                .as_ref()
                .is_some_and(|f| self.matches_permanent_filter(object, controller, f))
    }
    pub(super) fn eligible_stack_targets_for(
        &self,
        spec: &TargetSpec,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        self.eligible_stack_targets(spec.domain, exclude)
            .into_iter()
            .filter(|id| {
                spec.filter.as_ref().is_none_or(|f| {
                    self.state
                        .rules
                        .stack
                        .iter()
                        .find(|e| e.id == *id)
                        .is_some_and(|e| {
                            self.definition(e.source.card).is_ok_and(|d| {
                                f.types.iter().any(|k| d.characteristics.has_type(*k))
                            })
                        })
                })
            })
            .collect()
    }
    pub(super) fn capture_target_facts(
        &self,
        entry: &StackInstance,
        legal: &[bool],
    ) -> Result<Vec<Option<TargetFacts>>, String> {
        let p = self.program_for(entry)?;
        if !p.targets.iter().any(|t| t.filter.is_some()) {
            return Ok(vec![]);
        }
        p.targets
            .iter()
            .enumerate()
            .map(|(slot, t)| {
                if !legal[slot] {
                    return Ok(None);
                }
                let (controller, c) = if t.domain == TargetDomain::StackSpell {
                    let e = self
                        .state
                        .rules
                        .stack
                        .iter()
                        .find(|e| Some(&e.id) == entry.stack_targets.get(slot))
                        .ok_or("missing LKI stack target")?;
                    (
                        e.controller,
                        self.definition(e.source.card)?.characteristics.clone(),
                    )
                } else {
                    let object = *entry
                        .targets
                        .get(slot)
                        .ok_or("missing LKI permanent target")?;
                    (
                        self.current(object)?.controller,
                        self.characteristics(object)?,
                    )
                };
                let mut mana_value = c.mana_cost.generic;
                mana_value.add(&Quantity::from(
                    i64::try_from(c.mana_cost.colored.len()).map_err(|_| "mana value overflow")?,
                ));
                if t.domain == TargetDomain::StackSpell {
                    let entry = self
                        .state
                        .rules
                        .stack
                        .iter()
                        .find(|e| Some(&e.id) == entry.stack_targets.get(slot))
                        .ok_or("missing stack mana value")?;
                    mana_value = self.stack_mana_value(entry)?;
                }
                Ok(Some(TargetFacts {
                    controller,
                    power: c.power,
                    mana_value,
                }))
            })
            .collect()
    }
    pub(super) fn validate_target_facts(&self, frame: &Resolving) -> Result<(), String> {
        let expected = self
            .program_for(&frame.spell)?
            .targets
            .iter()
            .any(|t| t.filter.is_some());
        if (!expected && !frame.target_facts.is_empty())
            || (expected
                && (frame.target_facts.len() != frame.legal_targets.len()
                    || frame
                        .target_facts
                        .iter()
                        .zip(&frame.legal_targets)
                        .any(|(f, l)| {
                            f.is_some() != *l
                                || f.as_ref().is_some_and(|f| {
                                    f.controller.0 >= 4 || f.mana_value.is_negative()
                                })
                        })))
        {
            return Err("snapshot removal target facts invariant".into());
        }
        Ok(())
    }
    pub(super) fn removal_target_controller(
        &self,
        frame: &Resolving,
        slot: u32,
    ) -> Result<PlayerId, String> {
        Ok(self.target_facts(frame, slot)?.controller)
    }
    fn effect_recipient(&self, frame: &Resolving, r: &EffectRecipient) -> Result<PlayerId, String> {
        match r {
            EffectRecipient::Controller => Ok(frame.spell.controller),
            EffectRecipient::TargetController { target_slot } => {
                Ok(self.target_facts(frame, *target_slot)?.controller)
            }
        }
    }
    fn target_facts<'a>(&self, frame: &'a Resolving, slot: u32) -> Result<&'a TargetFacts, String> {
        frame
            .target_facts
            .get(slot as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing legal target LKI".into())
    }
    fn removal_cause(frame: &Resolving, block: u32) -> BatchCause {
        BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: block,
        }
    }
    pub(super) fn execute_removal_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let (event, block) = match instruction {
            Instruction::UntapPermanent {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                // CR608.2b checks targeting once as resolution starts. The
                // instruction still needs that same battlefield incarnation;
                // no later incarnation or new targetability check replaces it.
                if frame.legal_targets.get(target_slot as usize) != Some(&true) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                let Ok(object) = self.current(target) else {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::ObjectNoLongerPresent { target },
                    );
                };
                if object.zone != Zone::Battlefield {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::ObjectNoLongerPresent { target },
                    );
                }
                // CR701.26b/609.3: an untapped permanent is a legal target,
                // but cannot be untapped again. Preserve the strict event
                // guard and record no change rather than a false untap event.
                if !object.tapped {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (ProposedEvent::Untap { object: target }, source_block)
            }
            Instruction::DestroyPermanent {
                target_slot,
                source_block,
                ..
            }
            | Instruction::ExilePermanent {
                target_slot,
                source_block,
            } => {
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || !self.object_target_legal(&frame.spell, target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                if matches!(
                    instruction,
                    Instruction::DestroyPermanent { optional: true, .. }
                ) {
                    // CR 603.5: "may" is the controller's resolution choice; a
                    // departed controller (800.4a) makes none, as in
                    // removal_choice_step.
                    if !self.is_live(frame.spell.controller) {
                        let player = frame.spell.controller;
                        return self.finish_instruction(
                            frame,
                            InstructionOutcome::PlayerActionSkipped { player },
                        );
                    }
                    self.state.rules.frame = Frame::ResolutionChoice {
                        choice: ResolutionChoice::OptionalEffect {
                            player: frame.spell.controller,
                            target,
                            source_block,
                        },
                        frame,
                    };
                    return Ok(());
                }
                (
                    if matches!(instruction, Instruction::ExilePermanent { .. }) {
                        ProposedEvent::Move {
                            object: target,
                            destination: Zone::Exile,
                        }
                    } else {
                        ProposedEvent::DestroyPermanent { object: target }
                    },
                    source_block,
                )
            }
            Instruction::ChangeLifeFromTarget {
                recipient,
                amount,
                gain,
                source_block,
            } => {
                let player = self.effect_recipient(&frame, &recipient)?;
                let amount = match amount {
                    TargetAmount::Fixed { amount } => amount,
                    TargetAmount::Attribute {
                        target_slot,
                        attribute,
                    } => {
                        let f = self.target_facts(&frame, target_slot)?;
                        match attribute {
                            TargetAttribute::Power => f.power.clone().ok_or("missing LKI power")?,
                            TargetAttribute::ManaValue => f.mana_value.clone(),
                        }
                    }
                };
                // CR107.1b: a negative quantity is zero for an effect's result.
                if !self.is_live(player) || amount <= Quantity::zero() {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Life { actual: None });
                }
                (
                    if gain {
                        ProposedEvent::GainLife { player, amount }
                    } else {
                        ProposedEvent::LoseLife { player, amount }
                    },
                    source_block,
                )
            }
            Instruction::CreateTokensFor {
                recipient,
                count,
                token,
                source_block,
            } => {
                let controller = self.effect_recipient(&frame, &recipient)?;
                if !self.is_live(controller) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    ProposedEvent::CreateCreatureTokens {
                        controller,
                        source: frame.spell.source,
                        count,
                        token: Box::new(token),
                    },
                    source_block,
                )
            }
            Instruction::DamageTargetController {
                target_slot,
                amount,
                source_block,
            } => {
                let player = self.target_facts(&frame, target_slot)?.controller;
                if !self.is_live(player) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Damage { actual: None });
                }
                let damage = NoncombatDamageOccurrence {
                    source: self.source_facts(&frame.spell)?,
                    controller: frame.spell.controller,
                    recipient: DamageRecipient::Player { player },
                    amount,
                };
                return self.prepare_noncombat_damage(frame, damage, source_block);
            }
            _ => return Err("not removal instruction".into()),
        };
        let actual = self.commit(event, Self::removal_cause(&frame, block))?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn removal_choice_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            self.state.rules.pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing optional effect answer")?;
        let Response::OptionalEffect { perform } = input.response else {
            return Err("invalid optional effect answer".into());
        };
        if input.player != player {
            return Err("wrong optional effect player".into());
        }
        let actual = if perform && self.object_target_legal(&frame.spell, target) {
            Some(self.commit(
                ProposedEvent::DestroyPermanent { object: target },
                Self::removal_cause(&frame, block),
            )?)
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}
