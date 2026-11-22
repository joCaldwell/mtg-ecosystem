//! P6.9 finite sacrifice programs. Pinned CR 118.8, 601.2c/h, 602.2b,
//! 603.2c/10a, 608.2h, 700.4, 701.21a, 101.4, 800.4a and 509.1b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(crate) fn sacrifice_candidates(
        &self,
        player: PlayerId,
        filter: &SacrificeFilter,
    ) -> Vec<ObjectId> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| {
                crate::query::matches_sacrifice_filter(&self.object_facts(o), player, filter)
                    .is_yes()
            })
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn invocation_sacrifice<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Option<(&'a SacrificeFilter, u32)> {
        match entry.kind {
            StackKind::Spell => self
                .program_for(entry)
                .ok()?
                .additional_sacrifice
                .as_ref()
                .map(|f| (f, 1)),
            StackKind::Activated { ability, .. } => match &self
                .copiable_definition(entry.source)
                .ok()?
                .activated
                .get(ability as usize)?
                .cost
            {
                ActivationCost::SacrificePermanent { filter, count, .. } => Some((filter, *count)),
                _ => None,
            },
            _ => None,
        }
    }

    /// Present each unordered payment exactly once. An offered object must
    /// leave enough higher identities to finish the selected prefix.
    fn next_sacrifices(
        &self,
        stack: &StackInstance,
        prefix: &[ObjectId],
    ) -> Result<Vec<ObjectId>, String> {
        let (filter, count) = self
            .invocation_sacrifice(stack)
            .ok_or("missing sacrifice cost")?;
        let mut eligible = self.sacrifice_candidates(stack.controller, filter);
        // Never allocate from a declared count. Only the actual permanents
        // bound selection work, including on narrower hosts.
        let Ok(count) = usize::try_from(count) else {
            return Ok(vec![]);
        };
        if count > eligible.len()
            || prefix.len() >= count
            || !prefix.windows(2).all(|w| w[0] < w[1])
            || !prefix.iter().all(|id| eligible.binary_search(id).is_ok())
        {
            return Ok(vec![]);
        }
        if let Some(last) = prefix.last() {
            eligible.retain(|id| id > last);
        }
        let needed_after = count - prefix.len() - 1;
        eligible.truncate(eligible.len().saturating_sub(needed_after));
        Ok(eligible)
    }
    pub(super) fn check_sacrifice_payment(
        &self,
        stack: &StackInstance,
        cast: &Casting,
    ) -> Result<(), String> {
        if let Some((filter, count)) = self.invocation_sacrifice(stack) {
            let eligible = self.sacrifice_candidates(stack.controller, filter);
            if u32::try_from(cast.sacrifices.len()) != Ok(count)
                || !cast.sacrifices.windows(2).all(|w| w[0] < w[1])
                || !cast
                    .sacrifices
                    .iter()
                    .all(|id| eligible.binary_search(id).is_ok())
            {
                return Err("sacrifice cost is not payable".into());
            }
        } else if !cast.sacrifices.is_empty() {
            return Err("unexpected sacrifice cost selection".into());
        }
        Ok(())
    }
    pub(super) fn sacrifice_payment_group(
        &self,
        stack: &StackInstance,
        selected: &[ObjectId],
    ) -> Result<Option<EventGroup>, String> {
        let leaf = |object| ProposedEvent::Sacrifice {
            object,
            player: stack.controller,
        };
        if selected.len() > 1 {
            return Ok(Some(EventGroup::Simultaneous {
                occurrences: selected.iter().copied().map(leaf).collect(),
            }));
        }
        let self_cost = match stack.kind {
            StackKind::Activated { ability, .. } => {
                self.copiable_definition(stack.source)?.activated[ability as usize].cost
                    == ActivationCost::SacrificeSource
            }
            _ => false,
        };
        Ok(selected
            .first()
            .copied()
            .or(self_cost.then_some(stack.source))
            .map(|object| EventGroup::Occurrence {
                proposed: leaf(object),
            }))
    }

    pub(super) fn pay_sacrifice_cost(
        &mut self,
        stack: &StackInstance,
        selected: &[ObjectId],
    ) -> Result<(), String> {
        let Some(group) = self.sacrifice_payment_group(stack, selected)? else {
            return Ok(());
        };
        let fact = match &group {
            EventGroup::Occurrence {
                proposed: ProposedEvent::Sacrifice { object, .. },
            } => Some(self.object_before(*object)?),
            _ => None,
        };
        self.commit_batch(
            BatchCause::Payment {
                stack_id: stack.id,
                player: stack.controller,
            },
            group,
        )?;
        let entry = self
            .rules_mut()
            .stack
            .last_mut()
            .ok_or("missing sacrifice invocation")?;
        if let Some(fact) = fact {
            entry.sacrificed = Some(fact);
        } else {
            entry.sacrificed_set = selected.to_vec();
        }
        Ok(())
    }
    pub(super) fn sacrifice_request(&self) -> Result<Option<Request>, String> {
        let decision_id = self.state.rules.revision;
        Ok(match &self.state.rules.frame {
            Frame::SacrificeCost { cast } => {
                let stack = self.top()?;
                let candidates = self.next_sacrifices(stack, &cast.sacrifices)?;
                (!candidates.is_empty()).then_some(Request::SacrificeCost {
                    decision_id,
                    player: stack.controller,
                    candidates,
                })
            }
            Frame::ForcedSacrifice {
                remaining, choices, ..
            } => remaining.first().and_then(|player| {
                let candidates = self.controlled_creatures(*player);
                (self.is_live(*player) && !candidates.is_empty()).then_some(
                    Request::ForcedSacrifice {
                        decision_id,
                        player: *player,
                        candidates,
                        previous_choices: choices.clone(),
                    },
                )
            }),
            Frame::OptionalDamage { frame } => self
                .is_live(self.entry(frame.stack_id)?.controller)
                .then_some(Request::OptionalEffect {
                    decision_id,
                    player: self.entry(frame.stack_id)?.controller,
                }),
            _ => None,
        })
    }
    pub(super) fn validate_sacrifice_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        match (self.sacrifice_request()?, response) {
            (
                Some(Request::SacrificeCost {
                    player: p,
                    candidates,
                    ..
                }),
                Response::SacrificeCost { object },
            )
            | (
                Some(Request::ForcedSacrifice {
                    player: p,
                    candidates,
                    ..
                }),
                Response::ForcedSacrifice { object },
            ) if p == player && candidates.contains(object) => Ok(()),
            (Some(Request::OptionalEffect { player: p, .. }), Response::OptionalEffect { .. })
                if p == player =>
            {
                Ok(())
            }
            _ => Err("invalid sacrifice continuation response".into()),
        }
    }
    pub(super) fn sacrifice_step(&mut self, frame: Frame) -> Result<(), String> {
        match frame {
            Frame::SacrificeCost { mut cast } => {
                let stack = self.top()?.clone();
                if self.next_sacrifices(&stack, &cast.sacrifices)?.is_empty() {
                    let undo = cast.undo_mana;
                    return self.reverse_cast(cast, "sacrifice no longer payable", undo);
                }
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing sacrifice cost input")?;
                self.validate_sacrifice_response(input.player, &input.response)?;
                let Response::SacrificeCost { object } = input.response else {
                    return Err("cost response invariant".into());
                };
                cast.sacrifices.push(object);
                let (_, count) = self.invocation_sacrifice(&stack).ok_or("missing cost")?;
                if u32::try_from(cast.sacrifices.len()) != Ok(count) {
                    self.rules_mut().frame = Frame::SacrificeCost { cast };
                    return Ok(());
                }
                self.payment_step(cast)
            }
            Frame::OptionalDamage { frame } => {
                let perform = if self.is_live(self.entry(frame.stack_id)?.controller) {
                    let input = self
                        .rules_mut()
                        .pending
                        .take()
                        .ok_or("missing optional damage input")?;
                    self.validate_sacrifice_response(input.player, &input.response)?;
                    let Response::OptionalEffect { perform } = input.response else {
                        return Err("optional damage response".into());
                    };
                    perform
                } else {
                    false
                };
                if perform {
                    self.start_sacrifice_damage(frame)
                } else {
                    self.finish_instruction(frame, InstructionOutcome::Damage { actual: None })
                }
            }
            Frame::ForcedSacrifice {
                frame,
                mut remaining,
                mut choices,
                source_block,
            } => {
                if let Some(player) = remaining.first().copied() {
                    if self.is_live(player) && !self.controlled_creatures(player).is_empty() {
                        let input = self
                            .rules_mut()
                            .pending
                            .take()
                            .ok_or("missing forced sacrifice input")?;
                        self.validate_sacrifice_response(input.player, &input.response)?;
                        let Response::ForcedSacrifice { object } = input.response else {
                            return Err("forced sacrifice response".into());
                        };
                        choices.push((player, object));
                    }
                    remaining.remove(0);
                    self.rules_mut().frame = Frame::ForcedSacrifice {
                        frame,
                        remaining,
                        choices,
                        source_block,
                    };
                    Ok(())
                } else {
                    let children = choices
                        .into_iter()
                        .filter(|(p, o)| {
                            self.is_live(*p)
                                && self
                                    .current(*o)
                                    .is_ok_and(|v| v.present_on_battlefield() && v.controller == *p)
                        })
                        .map(|(player, object)| ProposedEvent::Sacrifice { player, object })
                        .collect::<Vec<_>>();
                    if children.is_empty() {
                        return self.finish_instruction(
                            frame,
                            InstructionOutcome::Effect { actual: None },
                        );
                    }
                    self.complete_graveyard_group(
                        self.instruction_cause(&frame, source_block)?,
                        EventGroup::Simultaneous {
                            occurrences: children,
                        },
                        GroupFinish::Instruction {
                            frame,
                            result: GroupResult::Effect,
                        },
                    )
                }
            }
            _ => Err("sacrifice frame invariant".into()),
        }
    }
    // A departure can invalidate a privately selected prefix before payment.
    // Cancel the unpaid activation when the complete cost is impossible,
    // otherwise reoffer the whole selection if the old prefix cannot finish.
    pub(super) fn revalidate_sacrifice_departure(&mut self) -> Result<(), String> {
        let Frame::SacrificeCost { cast } = &self.state.rules.frame else {
            return Ok(());
        };
        let stack = self.top()?;
        let Some((_, count)) = self.invocation_sacrifice(stack) else {
            return Err("missing sacrifice cost".into());
        };
        if count <= 1 || !self.next_sacrifices(stack, &cast.sacrifices)?.is_empty() {
            return Ok(());
        }
        let mut cast = cast.clone();
        if self.next_sacrifices(stack, &[])?.is_empty() {
            let undo = cast.undo_mana;
            return self.reverse_cast(cast, "departure made artifact sacrifice unpayable", undo);
        }
        cast.sacrifices.clear();
        self.rules_mut().pending = None;
        self.rules_mut().frame = Frame::SacrificeCost { cast };
        Ok(())
    }
    pub(super) fn execute_sacrifice_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        match instruction {
            Instruction::EachOpponentSacrifice { source_block } => {
                let controller = self.entry(frame.stack_id)?.controller;
                let remaining = (0..4)
                    .map(|n| PlayerId((self.state.rules.turn_owner.0 + n) % 4))
                    .filter(|p| self.is_live(*p) && *p != controller)
                    .collect();
                self.rules_mut().frame = Frame::ForcedSacrifice {
                    frame,
                    remaining,
                    choices: vec![],
                    source_block,
                };
                Ok(())
            }
            Instruction::SacrificeDamage { optional: true, .. } => {
                self.rules_mut().frame = Frame::OptionalDamage { frame };
                Ok(())
            }
            Instruction::SacrificeDamage { .. } => self.start_sacrifice_damage(frame),
            i @ Instruction::ModifySourcePowerToughness { .. } => {
                self.execute_effect_instruction(frame, i)
            }
            _ => Err("not sacrifice instruction".into()),
        }
    }
    pub(super) fn sacrifice_damage_spec(
        &self,
        frame: &Resolving,
    ) -> Result<(Quantity, u32, Vec<TargetEntity>), String> {
        let Instruction::SacrificeDamage {
            target_slot,
            recipient,
            amount,
            source_block,
            ..
        } = &self.program_for(self.entry(frame.stack_id)?)?.instructions[frame.cursor as usize]
        else {
            return Err("not sacrifice damage".into());
        };
        let amount = match amount {
            SacrificeDamageAmount::Fixed { value } => value.clone(),
            SacrificeDamageAmount::SacrificedPower => std::cmp::max(
                self.entry(frame.stack_id)?
                    .sacrificed
                    .as_ref()
                    .and_then(|f| f.characteristics.power.clone())
                    .ok_or("missing sacrificed power")?,
                Quantity::zero(),
            ),
        };
        let targets = if let Some(slot) = target_slot {
            if frame.legal_targets.get(*slot as usize) == Some(&true) {
                vec![
                    *self
                        .entry(frame.stack_id)?
                        .entity_targets
                        .get(*slot as usize)
                        .ok_or("missing sacrifice damage target")?,
                ]
            } else {
                vec![]
            }
        } else {
            match recipient {
                Some(LifeRecipient::Controller) => vec![TargetEntity::Player {
                    player: self.entry(frame.stack_id)?.controller,
                }],
                Some(LifeRecipient::EachOpponent) => self
                    .opponents(self.entry(frame.stack_id)?.controller)
                    .into_iter()
                    .map(|player| TargetEntity::Player { player })
                    .collect(),
                _ => return Err("invalid sacrifice damage recipient".into()),
            }
        };
        Ok((amount, *source_block, targets))
    }
    fn start_sacrifice_damage(&mut self, frame: Resolving) -> Result<(), String> {
        let (amount, block, targets) = self.sacrifice_damage_spec(&frame)?;
        let targets = targets
            .into_iter()
            .filter(|t| self.entity_legal(*t))
            .collect::<Vec<_>>();
        if amount == Quantity::zero() || targets.is_empty() {
            return self.finish_instruction(frame, InstructionOutcome::Damage { actual: None });
        }
        self.prepare_noncombat_set_damage(frame, amount, block, targets)
    }
}
impl Game {
    pub(super) fn validate_sacrifice_state(&self) -> Result<(), String> {
        for e in &self.state.rules.stack {
            if let Some(f) = &e.sacrificed {
                if f.zone != Zone::Battlefield
                    || f.controller != e.controller
                    || self
                        .state
                        .rules
                        .cards
                        .get(&f.object.card)
                        .is_none_or(|c| c.owner != f.owner)
                    || (!matches!(e.kind, StackKind::Activated { .. })
                        && self.invocation_sacrifice(e).is_none())
                {
                    return Err("snapshot sacrificed cost facts invariant".into());
                }
                self.validate_characteristic_identity(f.object, f.zone, &f.characteristics)?;
            }
            let count = self.invocation_sacrifice(e).map_or(0, |(_, count)| count);
            if self.stack_entry_complete(e) && count == 1 && e.sacrificed.is_none() {
                return Err("snapshot missing sacrifice cost facts".into());
            }
            let expected = if count > 1 && self.stack_entry_complete(e) {
                count
            } else {
                0
            };
            if u32::try_from(e.sacrificed_set.len()) != Ok(expected)
                || count > 1 && e.sacrificed.is_some()
                || !e.sacrificed_set.windows(2).all(|w| w[0] < w[1])
            {
                return Err("snapshot fixed sacrifice cost facts".into());
            }
        }
        match &self.state.rules.frame {
            Frame::SacrificeCost { cast } => {
                if self.invocation_sacrifice(self.top()?).is_none()
                    || cast.spending.is_none()
                    || (!cast.sacrifices.is_empty()
                        && self
                            .next_sacrifices(self.top()?, &cast.sacrifices)?
                            .is_empty())
                {
                    return Err("snapshot sacrifice cost frame invariant".into());
                }
            }
            Frame::ForcedSacrifice {
                frame,
                remaining,
                choices,
                source_block,
            } => {
                let instruction = self
                    .program_for(self.entry(frame.stack_id)?)?
                    .instructions
                    .get(frame.cursor as usize);
                let includes_caster =
                    matches!(instruction, Some(Instruction::EachPlayerSacrifice { .. }));
                let valid = matches!(instruction,Some(Instruction::EachOpponentSacrifice {source_block:b} | Instruction::EachPlayerSacrifice {source_block:b}) if b==source_block);
                let controller = self.entry(frame.stack_id)?.controller;
                let all = (0..4)
                    .map(|n| PlayerId((self.state.rules.turn_owner.0 + n) % 4))
                    .filter(|p| includes_caster || *p != controller)
                    .collect::<Vec<_>>();
                if !valid
                    || remaining.len() > all.len()
                    || choices.len() > all.len()
                    || remaining.windows(2).any(|w| {
                        all.iter().position(|p| p == &w[0]) >= all.iter().position(|p| p == &w[1])
                    })
                    || choices.iter().any(|(p, o)| {
                        !all.contains(p)
                            || remaining.contains(p)
                            || !self.state.rules.cards.contains_key(&o.card)
                    })
                    || choices
                        .iter()
                        .map(|(p, _)| p)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != choices.len()
                {
                    return Err("snapshot APNAP sacrifice invariant".into());
                }
            }
            Frame::OptionalDamage { frame }
                if !matches!(
                    self.program_for(self.entry(frame.stack_id)?)?
                        .instructions
                        .get(frame.cursor as usize),
                    Some(Instruction::SacrificeDamage { optional: true, .. })
                ) =>
            {
                return Err("snapshot optional damage invariant".into());
            }
            _ => {}
        }
        Ok(())
    }
}
