//! Finite permanent removal. Pinned CR 115, 202.3a, 406.1–3, 601.2c/h,
//! 602.2b, 603.3d/5, 608.2b/c/h, 701.6a/8a-b/13a/19c, 704.5d/m/n,
//! 903.9a and 903.10a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Ref, SetExpr};

// See docs/game-engine-board-mechanics.md#movement-sacrifice-and-board-wide-effects.
// Historical derivations remain linked from docs/game-engine-evidence.md.

/// Current facts refresh while the exact target remains in its expected public
/// zone, then survive as its last appearance. A successor never supplies them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetFacts {
    controller: PlayerId,
    power: Option<Quantity>,
    mana_value: Quantity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    toughness: Option<Quantity>,
}
impl TargetFacts {
    pub(in crate::engine) fn controller(&self) -> PlayerId {
        self.controller
    }

    pub(in crate::engine) fn mana_value(&self) -> &Quantity {
        &self.mana_value
    }

    pub(in crate::engine) fn toughness(&self) -> Option<&Quantity> {
        self.toughness.as_ref()
    }

    pub(in crate::engine) fn power(&self) -> Option<&Quantity> {
        self.power.as_ref()
    }
}
impl RulesTxn {
    fn tracks_stack_target_facts(domain: TargetDomain) -> bool {
        matches!(
            domain,
            TargetDomain::StackSpell | TargetDomain::ControlledCreatureSpell
        )
    }

    fn tracks_target_slot(domain: TargetDomain) -> bool {
        Self::tracks_stack_target_facts(domain)
            || matches!(
                domain,
                TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::CreatureOrPlaneswalker
                    | TargetDomain::GraveyardCard { .. }
                    | TargetDomain::OwnGraveyardCard
                    | TargetDomain::OwnGraveyardCreatureCard
            )
    }

    fn tracks_target_facts(program: &SpellProgram) -> bool {
        program
            .targets
            .iter()
            .any(|spec| Self::tracks_target_slot(spec.domain))
    }

    fn target_information_frame(&mut self) -> Option<&mut Resolving> {
        let rules = self.rules_mut();
        let mut frame = &mut rules.frame;
        let mut frames = rules.frames.as_mut_slice();
        while let Frame::Stabilize { resume, .. }
        | Frame::LegendRule { resume, .. }
        | Frame::CommanderReturn {
            continuation:
                CommanderContinuation::StateBased { resume }
                | CommanderContinuation::StateBasedBatch { resume, .. },
            ..
        } = frame
        {
            let (rest, top) = frames.split_at_mut(resume.0);
            frame = &mut top[0];
            frames = rest;
        }

        if matches!(
            frame,
            Frame::Kernel { .. }
                | Frame::CommanderReturn {
                    continuation: CommanderContinuation::ResolutionDraw { .. },
                    ..
                }
        ) {
            return match frames.first_mut() {
                Some(Frame::Resolve { frame }) => Some(frame),
                _ => None,
            };
        }

        match frame {
            Frame::Resolve { frame }
            | Frame::ResolutionChoice { frame, .. }
            | Frame::ForcedSacrifice { frame, .. } => Some(frame),
            Frame::GraveyardGroup { tx } => tx.finish.frame_mut(),
            Frame::CounterPlacement { transaction } => Some(&mut transaction.frame),
            Frame::DamageEffects { transaction } => match &mut transaction.continuation {
                DamageContinuation::Instruction { frame } => Some(frame),
                DamageContinuation::Combat { .. } | DamageContinuation::Mana { .. } => None,
            },
            Frame::CommanderReturn { continuation, .. } => match continuation {
                CommanderContinuation::BoardReturn { frame, .. }
                | CommanderContinuation::SpellMove { frame, .. }
                | CommanderContinuation::Search { frame, .. }
                | CommanderContinuation::LookHand { frame, .. } => Some(frame),
                CommanderContinuation::StateBased { .. }
                | CommanderContinuation::StateBasedBatch { .. } => None,

                CommanderContinuation::Draw { .. }
                | CommanderContinuation::ResolutionDraw { .. } => None,
            },
            _ => None,
        }
    }
    /// CR608.2h: read current information when an effect applies, retaining the
    /// last expected-zone appearance before a move/departure. Refresh on every
    /// atomic step, including concessions between instructions. This never
    /// repeats the resolution-start target legality check (CR608.2b).
    pub(super) fn refresh_removal_target_facts(&mut self) -> Result<(), String> {
        let (id, legal) = match self.target_information_frame() {
            Some(frame) if !frame.target_facts.is_empty() => {
                (frame.stack_id, frame.legal_targets.clone())
            }
            _ => return Ok(()),
        };
        let entry = self.entry(id)?;
        let current = self
            .program_for(entry)?
            .targets
            .iter()
            .enumerate()
            .map(|(slot, spec)| {
                legal.get(slot) == Some(&true)
                    && if Self::tracks_stack_target_facts(spec.domain) {
                        self.bound_stack_target(entry, slot).is_some_and(|id| {
                            self.state.rules.stack.iter().any(|entry| entry.id == *id)
                        })
                    } else {
                        self.bound_object_target(entry, slot).is_some_and(|id| {
                            self.current(*id)
                                .is_ok_and(|object| object.present_on_battlefield())
                        })
                    }
            })
            .collect::<Vec<_>>();
        let fresh = self.capture_target_facts(entry, &current)?;
        let frame = self
            .target_information_frame()
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
    /// CR 109.2, 201.5, 601.2c, 608.2b: a battlefield target's criteria for
    /// `controller`. "Another" (`IsNot` the source) excludes `source`, the
    /// targeting object's own identity; `source_power` is its power, current
    /// or last known (CR 702.134a).
    pub(crate) fn permanent_target_legal(
        &self,
        object: ObjectId,
        controller: PlayerId,
        source: Option<ObjectId>,
        source_power: Option<&Quantity>,
        spec: &TargetSpec,
    ) -> bool {
        self.targetable_by(object, controller)
            && (spec.domain != TargetDomain::ControlledBattlefieldPermanent
                || self
                    .current(object)
                    .is_ok_and(|o| o.controller == controller))
            && spec.filter.as_ref().is_some_and(|filter| {
                self.current(object).is_ok_and(|current| {
                    crate::query::matches_permanent_filter_with_source(
                        &self.object_facts(current),
                        controller,
                        filter,
                        Some(crate::query::TargetSource {
                            is_source: source == Some(object),
                            power: source_power,
                        }),
                    )
                    .is_yes()
                })
            })
    }

    pub(crate) fn eligible_stack_targets_for(
        &self,
        spec: &TargetSpec,
        controller: PlayerId,
        exclude: Option<StackId>,
    ) -> Vec<StackId> {
        let controlled_creature = spec.domain == TargetDomain::ControlledCreatureSpell;
        let domain = if controlled_creature {
            TargetDomain::StackSpell
        } else {
            spec.domain
        };
        self.eligible_stack_targets(domain, exclude)
            .into_iter()
            .filter(|id| {
                if !controlled_creature && spec.filter.is_none() {
                    return true;
                }
                self.state
                    .rules
                    .stack
                    .iter()
                    .find(|e| e.id == *id)
                    .is_some_and(|entry| {
                        self.targeting_characteristics(entry).is_ok_and(|c| {
                            (!controlled_creature
                                || (entry.controller == controller
                                    && c.has_type(CardKind::Creature)))
                                && spec.filter.as_ref().is_none_or(|f| {
                                    self.stack_mana_value(entry).is_ok_and(|mana_value| {
                                        crate::query::spell_matches_filter(&c, &mana_value, f)
                                    })
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
        if !Self::tracks_target_facts(p) {
            return Ok(vec![]);
        }
        p.targets
            .iter()
            .enumerate()
            .map(|(slot, t)| {
                if legal.get(slot) != Some(&true) || !Self::tracks_target_slot(t.domain) {
                    return Ok(None);
                }
                if Self::tracks_stack_target_facts(t.domain) {
                    let e = self
                        .state
                        .rules
                        .stack
                        .iter()
                        .find(|e| Some(&e.id) == self.bound_stack_target(entry, slot))
                        .ok_or("missing LKI stack target")?;
                    Ok(Some(TargetFacts {
                        controller: e.controller,
                        // CR 604.3, 613.4a: a creature spell's CDA defines
                        // its power on the stack too.
                        power: {
                            let mut c = self.targeting_characteristics(e)?.into_owned();
                            self.characteristic_power_toughness(e.source, &mut c)?;
                            c.power
                        },
                        mana_value: self.stack_mana_value(e)?,
                        toughness: None,
                    }))
                } else {
                    let object = *self
                        .bound_object_target(entry, slot)
                        .ok_or("missing LKI permanent target")?;
                    let c = self.characteristics(object)?;
                    Ok(Some(TargetFacts {
                        controller: self.current(object)?.controller,
                        mana_value: c.printed_mana_value(),
                        power: c.power,
                        toughness: c.toughness,
                    }))
                }
            })
            .collect()
    }
    pub(super) fn validate_target_facts(&self, frame: &Resolving) -> Result<(), String> {
        let entry = self.entry(frame.stack_id)?;
        let program = self.program_for(entry)?;
        let expected = Self::tracks_target_facts(program);
        if (!expected && !frame.target_facts.is_empty())
            || (expected
                && (frame.target_facts.len() != program.targets.len()
                    || frame.target_facts.iter().enumerate().any(|(slot, fact)| {
                        let legal = frame.legal_targets.get(slot) == Some(&true)
                            && Self::tracks_target_slot(program.targets[slot].domain);
                        fact.is_some() != legal
                            || fact
                                .as_ref()
                                .is_some_and(|f| f.controller.0 >= 4 || f.mana_value.is_negative())
                    })))
        {
            return Err("snapshot removal target facts invariant".into());
        }
        Ok(())
    }

    pub(super) fn execute_removal_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let (event, block) = match instruction {
            kernel!(Untap { what: SetExpr::One { object } } @ source_block)
            | kernel!(Tap { what: SetExpr::One { object } } @ source_block) => {
                let tap = matches!(instruction, kernel!(Tap { .. }));
                let entry = self.entry(frame.stack_id)?;
                // CR608.2b checks targeting once as resolution starts. The
                // instruction still needs that same battlefield incarnation;
                // no later incarnation or new targetability check replaces it.
                let target = match object {
                    Ref::Source => entry.source,
                    Ref::Target { slot } => {
                        let target = *self
                            .bound_object_target(entry, slot as usize)
                            .ok_or("missing tap-status target")?;
                        if frame.legal_targets.get(slot as usize) != Some(&true) {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::IllegalTarget { target },
                            );
                        }
                        target
                    }
                    _ => return Err("tap status requires a source or target".into()),
                };
                let Ok(object) = self.current(target) else {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::ObjectNoLongerPresent { target },
                    );
                };
                if !object.present_on_battlefield() {
                    return self.finish_instruction(
                        frame,
                        InstructionOutcome::ObjectNoLongerPresent { target },
                    );
                }
                // CR701.26a/b,609.3: a permanent already in the desired
                // state is a legal target, but cannot be tapped/untapped
                // again. Preserve the strict event guards and record no change.
                if object.tapped == tap {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Effect { actual: None });
                }
                (
                    if tap {
                        ProposedEvent::Tap { object: target }
                    } else {
                        ProposedEvent::Untap { object: target }
                    },
                    source_block,
                )
            }

            _ if instruction.target_destroy().is_some()
                || matches!(
                    instruction,
                    kernel!(Move {
                        what: target_object!(_),
                        to: mtg_contracts::kir::Destination::Exile
                    })
                ) =>
            {
                let (target_slot, optional, source_block) =
                    match (&instruction, instruction.target_destroy()) {
                        (_, Some((slot, optional, _, block))) => (slot, optional, block),
                        (kernel!(Move { what: target_object!(slot), .. } @ block), None) => {
                            (*slot, false, *block)
                        }
                        _ => return Err("expected target removal".into()),
                    };
                let target = *self
                    .bound_object_target(self.entry(frame.stack_id)?, target_slot as usize)
                    .ok_or("missing removal target")?;
                if frame.legal_targets.get(target_slot as usize) != Some(&true)
                    || !self.object_target_legal(
                        self.entry(frame.stack_id)?,
                        target_slot as usize,
                        target,
                    )
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::IllegalTarget { target });
                }
                if optional {
                    return self.offer_optional_target_effect(frame, target, source_block);
                }
                (
                    if matches!(
                        instruction,
                        kernel!(Move {
                            what: target_object!(_),
                            to: mtg_contracts::kir::Destination::Exile
                        })
                    ) {
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

            _ => return Err("not removal instruction".into()),
        };
        if matches!(&event, ProposedEvent::DestroyPermanent { .. }) {
            return self.complete_graveyard_group(
                self.instruction_cause(&frame, block)?,
                EventGroup::Occurrence { proposed: event },
                GroupFinish::Instruction {
                    frame,
                    result: GroupResult::ActualEffect,
                },
            );
        }
        let actual = self.commit(event, self.instruction_cause(&frame, block)?)?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    fn offer_optional_target_effect(
        &mut self,
        frame: Resolving,
        target: ObjectId,
        source_block: u32,
    ) -> Result<(), String> {
        // CR603.5/800.4a: the invocation's controller decides during resolution.
        let player = self.entry(frame.stack_id)?.controller;
        if !self.is_live(player) {
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        self.rules_mut().frame = Frame::ResolutionChoice {
            choice: ResolutionChoice::OptionalEffect {
                player,
                target,
                source_block,
            },
            frame,
        };
        Ok(())
    }
    pub(super) fn removal_choice_step(
        &mut self,
        frame: Resolving,
        player: PlayerId,
        target: ObjectId,
        block: u32,
    ) -> Result<(), String> {
        if !self.is_live(player) {
            self.rules_mut().pending = None;
            return self
                .finish_instruction(frame, InstructionOutcome::PlayerActionSkipped { player });
        }
        let input = self
            .rules_mut()
            .pending
            .take()
            .ok_or("missing optional effect answer")?;
        let perform = input.flag(Flag::Accept)?;
        if input.player != player {
            return Err("wrong optional effect player".into());
        }
        if !perform {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        let entry = self.entry(frame.stack_id)?;
        let Some((slot, true, ..)) = self
            .program_for(entry)?
            .instructions
            .get(frame.cursor as usize)
            .and_then(Instruction::target_destroy)
        else {
            return Err("optional target instruction invariant".into());
        };
        if self.object_target_legal(entry, slot as usize, target) {
            return self.complete_graveyard_group(
                self.instruction_cause(&frame, block)?,
                EventGroup::Occurrence {
                    proposed: ProposedEvent::DestroyPermanent { object: target },
                },
                GroupFinish::Instruction {
                    frame,
                    result: GroupResult::ActualEffect,
                },
            );
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
    }
}
