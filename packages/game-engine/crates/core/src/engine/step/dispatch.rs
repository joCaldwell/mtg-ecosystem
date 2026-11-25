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
            Frame::BattlefieldEntry { entry } => self.battlefield_entry_step(*entry)?,
            Frame::Kernel { task } => self.step_kernel_task(*task)?,
            Frame::GraveyardGroup { tx } => self.graveyard_group_step(tx)?,

            Frame::ResolutionPayment { frame, payment } => {
                self.resolution_payment_step(frame, payment)?
            }
            frame @ Frame::LegendRule { .. } => self.legend_step(frame)?,
            frame @ (Frame::SacrificeCost { .. } | Frame::ForcedSacrifice { .. }) => {
                self.sacrifice_step(frame)?
            }
            Frame::Setup => self.setup_step()?,

            Frame::AdditionalCosts { cast } => self.bind_additional_costs(cast)?,
            Frame::HybridCost { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing hybrid cost choice")?;

                let colors = input.hybrid_colors()?;
                self.rules_mut()
                    .stack
                    .last_mut()
                    .ok_or("missing hybrid spell")?
                    .hybrid_colors = colors;
                self.rules_mut().frame = self.spell_announcement_frame(cast)?;
            }
            Frame::AnnounceX { cast } => self.bind_x(cast)?,
            Frame::CounterPlacement { .. } => self.counter_placement_step()?,
            Frame::DamageEffects { .. } => self.damage_effect_step()?,
            Frame::Priority => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted priority input")?;
                match input.action()? {
                    Action::ActivateAbility { object, ability } => {
                        self.activate_ability(input.player, input.decision_id, object, ability)?
                    }
                    Action::PlayLand { object } => {
                        self.play_land(input.player, input.decision_id, object.card)?;
                    }
                    Action::ActivateMana { object } => {
                        self.begin_payment_mana(&input, object, Frame::Priority)?;
                    }
                    Action::Cast { card } => {
                        self.announce_priority_spell(input.player, card.card)?;
                    }
                    Action::Pass => {
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
                }
            }
            Frame::Targets { cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted target input")?;
                let objects = input.objects()?;
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
                if cast.origin.is_spell() {
                    self.rules_mut().frame = self.announcement_mana_frame(cast)?;
                } else {
                    // The activation cost determines its next payment stage.
                    self.continue_activation_after_targets(cast, input.decision_id)?;
                }
            }
            Frame::Mana { mut cast } => {
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing accepted mana input")?;
                match &input.answer {
                    Answer::Pay {
                        pay: PayAnswer::Activate { source },
                    } => {
                        let object = input.mana_source(*source)?;
                        self.begin_payment_mana(&input, object, Frame::Mana { cast })?;
                    }
                    Answer::Pay {
                        pay: PayAnswer::Finish { undo_on_failure },
                    } => {
                        cast.undo_mana = *undo_on_failure;
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
            Frame::Resolve { frame } => {
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
                    self.finish_spell(stack.id, false)?;
                } else if let Some(instruction) = program.instructions.get(frame.cursor as usize) {
                    let Instruction::Kernel { node, .. } = instruction;
                    // CR 115.1d, 608.2b: choosing zero doesn't stop the spell
                    // or ability. Only its independently targeted leaves do nothing.
                    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                    if program.optional_target
                        && frame.legal_targets.is_empty()
                        && !mtg_contracts::kir::target_slots(node).is_empty()
                    {
                        self.finish_instruction(
                            frame,
                            InstructionOutcome::Effect { actual: None },
                        )?;
                        return Ok(());
                    }
                    return Ok(self.execute_kernel(
                        frame,
                        node.clone(),
                        instruction.source_block(),
                    )?);
                } else {
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
