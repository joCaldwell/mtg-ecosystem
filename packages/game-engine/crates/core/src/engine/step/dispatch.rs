//! One rule step: the suspended frame selects the routine that advances it.
use crate::engine::*;

impl RulesTxn {
    pub(crate) fn step(&mut self) -> Result<(), RuleError> {
        self.check_history_capacity()?;
        self.refresh_removal_target_facts()?;
        if let Some(player) = self.state.rules.pending_concessions.first().copied() {
            self.rules_mut().pending_concessions.remove(0);
            self.apply_departure(player)?;
        } else {
            let frame = self.working_frame();
            self.step_frame(frame)?;
        }
        // An invocation that left the stack stays owned only while the
        // installed frame still resumes it (CR 608.2m).
        if self.kernel_leaf.is_some() {
            self.resume_finished_kernel_leaf()?;
        }
        self.retire_returned_frames()?;
        self.release_detached();
        Ok(())
    }
    /// The working copy of the suspended frame that one rule step advances.
    /// A frame holds progress only, so the copy has nothing to reconcile. The
    /// suspended frame stays installed, for authority checks, until the step
    /// installs its successor.
    pub(crate) fn working_frame(&self) -> Frame {
        Frame::clone(&self.state.rules.frame)
    }
    pub(in crate::engine) fn step_frame(&mut self, frame: Frame) -> Result<(), RuleError> {
        match frame {
            Frame::Kernel { task } => self.step_kernel_task(*task)?,
            Frame::GraveyardGroup { tx } => self.graveyard_group_step(tx)?,
            Frame::GraveyardReplacement { transaction } => {
                self.graveyard_replacement_step(transaction)?
            }
            Frame::LandEntry { .. } => self.land_entry_step()?,
            Frame::PlayHideawayLinked { progress } => self.linked_play_step(progress)?,
            Frame::RevealAndCast { progress } => self.reveal_and_cast_step(progress)?,
            Frame::HandCast { frame, child } => self.hand_cast_step(frame, child)?,
            Frame::Cascade { progress } => self.cascade_step(*progress)?,
            Frame::ResolutionPayment { frame, payment } => {
                self.resolution_payment_step(frame, payment)?
            }
            frame @ Frame::LegendRule { .. } => self.legend_step(frame)?,
            frame @ (Frame::SacrificeCost { .. }
            | Frame::ForcedSacrifice { .. }
            | Frame::OptionalDamage { .. }) => self.sacrifice_step(frame)?,
            Frame::Setup => self.setup_step()?,
            frame @ Frame::DamageAllocation { .. } => self.damage_allocation_step(frame)?,
            Frame::AdditionalCosts { cast } => self.bind_additional_costs(cast)?,
            Frame::HybridCost { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing hybrid cost choice")?;
                self.validate_response(input.player, &input.response)?;
                let Response::HybridCost { colors } = input.response else {
                    return Err("missing hybrid cost response".into());
                };
                self.rules_mut()
                    .stack
                    .last_mut()
                    .ok_or("missing hybrid spell")?
                    .hybrid_colors = colors;
                self.rules_mut().frame = self.spell_announcement_frame(cast)?;
            }
            Frame::AnnounceX { cast } => self.bind_x(cast)?,
            Frame::Modes { cast } => self.bind_mode(cast)?,
            Frame::CounterPlacement { .. } => self.counter_placement_step()?,
            Frame::DamageEffects { .. } => self.damage_effect_step()?,
            Frame::Priority => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted priority input")?;
                match input.response {
                    Response::ActivateAbility { object, ability } => {
                        self.activate_ability(input.player, input.decision_id, object, ability)?
                    }
                    Response::PlayLand { card } => {
                        self.play_land(input.player, input.decision_id, card, false)?;
                    }
                    Response::PlayBackLand { card } => {
                        self.play_land(input.player, input.decision_id, card, true)?;
                    }
                    Response::ActivateMana { object } => {
                        self.begin_payment_mana(&input, object, Frame::Priority)?;
                    }
                    Response::Bestow { card } => {
                        self.announce_priority_bestow(input.player, card)?
                    }
                    Response::Cast { card } => {
                        self.announce_priority_spell(input.player, card)?;
                    }
                    Response::Pass => {
                        self.record_event(Event::PriorityPassed {
                            player: input.player,
                        })?;
                        self.rules_mut().passes.push(input.player);
                        // CR 117.4 applies to the current live player set.
                        if self.all_live_passed() {
                            self.begin_resolution()?;
                        } else {
                            self.rules_mut().priority = self
                                .next_live(input.player)
                                .ok_or("no live priority recipient")?;
                            self.stabilize()?;
                        }
                    }
                    _ => return Err("response/frame invariant".into()),
                }
            }
            Frame::Targets { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted target input")?;
                let Response::Targets { objects } = input.response else {
                    return Err("target response/frame invariant".into());
                };
                let announced = (0..4)
                    .map(|v| {
                        objects
                            .iter()
                            .map(|object| self.object_handle_id(PlayerId(v), *object).cloned())
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let top = self.rules_mut().stack.last_mut().ok_or("missing stack")?;
                top.targets.extend(objects);
                if top.announced_targets.is_empty() {
                    top.announced_targets = announced;
                } else {
                    for (old, new) in top.announced_targets.iter_mut().zip(announced) {
                        old.extend(new);
                    }
                }
                let entry = self.top()?;
                let immediate_cost = match &entry.kind {
                    StackKind::Activated { ability, .. } => {
                        matches!(
                            self.copiable_definition(entry.source)?.activated[*ability as usize]
                                .cost,
                            ActivationCost::TapSource | ActivationCost::SacrificePermanent { .. }
                        )
                    }
                    _ => false,
                };
                if immediate_cost {
                    // The activation cost determines its next payment stage.
                    self.continue_activation_after_targets(cast, input.decision_id)?;
                } else {
                    self.rules_mut().frame = self.continue_spell_targeting(cast)?;
                }
            }
            Frame::Mana { mut cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted mana input")?;
                match input.response {
                    Response::ActivateMana { object } => {
                        self.begin_payment_mana(&input, object, Frame::Mana { cast })?;
                    }
                    Response::FinishMana { undo_mana } => {
                        cast.undo_mana = undo_mana;
                        self.rules_mut().frame = self.finish_mana_frame(cast)?;
                    }
                    _ => return Err("mana response/frame invariant".into()),
                }
            }
            Frame::ManaSourceChoice {
                player,
                source,
                scope,
                resume,
            } => self.mana_source_choice_step(
                player,
                source,
                scope,
                self.saved_frame(resume)?.clone(),
            )?,
            Frame::ManaAbility { activation } => self.permanent_mana_step(*activation)?,
            Frame::Payment { cast } => self.payment_step(cast)?,
            Frame::SpendMana { cast } => self.spend_mana_step(cast)?,
            frame @ (Frame::TriggerOrder { .. } | Frame::TriggerTargets { .. }) => {
                self.trigger_step(frame)?
            }
            frame @ (Frame::EntityTargets { .. }
            | Frame::PlayerTargets { .. }
            | Frame::StackTargets { .. }
            | Frame::ResolutionChoice { .. }) => self.actions_step(frame)?,
            Frame::Resolve { mut frame } => {
                let stack = self.entry(frame.stack_id)?.clone();
                let program = self.program_for(&stack)?.clone();
                // One spell-wide legality check before any block resolves.
                // CR 608.2b: if all targets are illegal, the entire spell does
                // not resolve, including its otherwise untargeted blocks.
                if frame
                    .condition_check
                    .as_ref()
                    .is_some_and(|check| !check.passed())
                    || (!frame.legal_targets.is_empty() && !frame.legal_targets.iter().any(|l| *l))
                {
                    if stack.alternative_cost == Some(cascade::AlternativeCost::Bestow)
                        && frame.condition_check.is_none()
                        && frame.legal_targets == [false]
                    {
                        self.complete_spell(stack.id, true, true)?;
                    } else {
                        self.finish_spell(stack.id, false)?;
                    }
                } else if let Some(instruction) = program.instructions.get(frame.cursor as usize) {
                    match instruction {
                        Instruction::OfferHandCastWithoutManaCost { .. } => {
                            self.rules_mut().frame = Frame::HandCast {
                                frame: Box::new(frame),
                                child: None,
                            };
                            return Ok(());
                        }
                        Instruction::ExileResolvingSpell { .. } => {
                            if !stack.kind.is_spell()
                                || stack.paid.is_none()
                                || frame.cursor as usize + 1 != program.instructions.len()
                            {
                                return Err(
                                    "self exile requires the final instruction of a paid spell"
                                        .into(),
                                );
                            }
                            return self.finish_spell(stack.id, true);
                        }
                        Instruction::Cascade { source_block, .. } => {
                            return Ok(self.begin_cascade(frame, *source_block)?);
                        }
                        Instruction::MoveTriggeredSuccessor { .. } => {
                            return Ok(self.resolve_successor_tuck(frame)?);
                        }
                        kernel!(Move { what: target_object!(target_slot), to: mtg_contracts::kir::Destination::OwnerHand } @ source_block)
                        | kernel!(Move { what: target_object!(target_slot), to: mtg_contracts::kir::Destination::OwnerLibraryTop } @ source_block) =>
                        {
                            let target = *self
                                .bound_object_target(&stack, *target_slot as usize)
                                .ok_or("missing return target")?;
                            let outcome = if !frame.legal_targets[*target_slot as usize] {
                                InstructionOutcome::IllegalTarget { target }
                            } else if self.current(target).is_err() {
                                InstructionOutcome::ObjectNoLongerPresent { target }
                            } else {
                                let owner = self.state.rules.cards[&target.card].owner;
                                let destination = if matches!(
                                    instruction,
                                    kernel!(Move {
                                        what: target_object!(_),
                                        to: mtg_contracts::kir::Destination::OwnerLibraryTop
                                    })
                                ) {
                                    Zone::Library { owner }
                                } else {
                                    Zone::Hand { owner }
                                };
                                if self.state.rules.commanders.history(target.card).is_some() {
                                    self.rules_mut().frame = Frame::CommanderReturn {
                                        object: target,
                                        destination,
                                        continuation: CommanderContinuation::SpellMove {
                                            frame,
                                            source_block: *source_block,
                                        },
                                    };
                                    return Ok(());
                                }
                                InstructionOutcome::Moved {
                                    movement: self.move_instruction_object(
                                        target,
                                        destination,
                                        BatchCause::Instruction {
                                            stack_id: stack.id,
                                            player: stack.controller,
                                            source: stack.source,
                                            cursor: frame.cursor,
                                            source_block: *source_block,
                                        },
                                    )?,
                                }
                            };
                            frame.outcomes.push(outcome);
                        }
                        other => return Ok(self.execute_action_instruction(frame, other.clone())?),
                    }
                    frame.cursor = frame
                        .cursor
                        .checked_add(1)
                        .ok_or("instruction cursor resource limit")?;
                    self.rules_mut().frame = Frame::Resolve { frame };
                } else {
                    if stack.kind.is_spell()
                        && program.resolution == SpellResolution::Permanent
                        && self.current(stack.source).is_ok()
                        && self.begin_copy_entry(&frame, stack.source)?
                    {
                        return Ok(());
                    }
                    self.finish_spell(stack.id, true)?;
                }
            }
            Frame::CommanderReturn {
                object,
                destination,
                continuation,
            } => self.answer_commander_return(object, destination, continuation)?,
            frame @ (Frame::Attackers
            | Frame::Blockers { .. }
            | Frame::Damage
            | Frame::DamageChoice { .. }
            | Frame::Discard { .. }
            | Frame::CleanupClear
            | Frame::Stabilize { .. }) => self.combat_step(frame)?,
            Frame::Draw => {}
            Frame::Transition => self.transition_turn_step()?,
            Frame::EnterStep => self.enter_turn_step()?,
            Frame::Fault { reason } => return Err(RuleError::invariant(reason)),
            Frame::Finished { .. } => {}
        }
        Ok(())
    }
}
