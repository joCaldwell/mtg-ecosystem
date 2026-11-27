//! P6.9 finite sacrifice programs. Pinned CR 118.8, 601.2c/h, 602.2b,
//! 603.2c/10a, 608.2h, 700.4, 701.21a, 101.4, 800.4a and 509.1b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl RulesTxn {
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
    /// CR 109.1, 701.21a: the permanents `player` could sacrifice for a
    /// cost, excluding `source` when the cost says "another".
    pub(crate) fn sacrifice_cost_candidates(
        &self,
        player: PlayerId,
        filter: &SacrificeFilter,
        another_than: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        let mut candidates = self.sacrifice_candidates(player, filter);
        if let Some(source) = another_than {
            candidates.retain(|id| id.card != source.card);
        }
        candidates
    }
    pub(super) fn invocation_sacrifice<'a>(
        &'a self,
        entry: &'a StackInstance,
    ) -> Option<(&'a SacrificeFilter, u32, Option<ObjectId>)> {
        match entry.kind {
            StackKind::Spell => self
                .program_for(entry)
                .ok()?
                .additional_sacrifice
                .as_ref()
                .map(|f| (f, 1, None)),
            StackKind::Activated { ability, .. } => match &self
                .stack_definition(entry)
                .ok()?
                .activated
                .get(ability as usize)?
                .cost
            {
                ActivationCost::SacrificePermanent {
                    filter,
                    count,
                    another,
                    ..
                } => Some((filter, *count, another.then_some(entry.source))),
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
        let (filter, count, another_than) = self
            .invocation_sacrifice(stack)
            .ok_or("missing sacrifice cost")?;
        let mut eligible = self.sacrifice_cost_candidates(stack.controller, filter, another_than);
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
        if let Some((filter, count, another_than)) = self.invocation_sacrifice(stack) {
            let eligible = self.sacrifice_cost_candidates(stack.controller, filter, another_than);
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
                self.stack_definition(stack)?.activated[ability as usize].cost
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

    // CR 701.21a and 702.26b: only a present permanent controlled by
    // the chooser qualifies. Read derived characteristics during resolution.
    pub(super) fn sacrifice_options(
        &self,
        frame: &Resolving,
        player: PlayerId,
    ) -> Result<Vec<ObjectId>, String> {
        let entry = self.entry(frame.stack_id)?;
        let instruction = self
            .program_for(entry)?
            .instructions
            .get(frame.cursor as usize);
        match instruction {
            Some(
                kernel!(Sacrifice {
                    filter,
                    greatest_power,
                    ..
                }),
            ) => {
                use mtg_contracts::kir::{Atom, Ref};
                let mut filter = filter.clone();
                let exclude_source = filter.0.iter().any(|atom| {
                    matches!(
                        atom,
                        Atom::IsNot {
                            object: Ref::Source
                        }
                    )
                });
                filter.0.retain(|atom| {
                    !matches!(
                        atom,
                        Atom::IsNot {
                            object: Ref::Source
                        }
                    )
                });
                let options = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|object| {
                        object.controller == player
                            && (!exclude_source || object.id != entry.source)
                            && crate::query::matches_battlefield_pred(
                                &self.object_facts(object),
                                entry.controller,
                                &filter,
                            )
                            .is_yes()
                    })
                    .map(|object| object.id)
                    .collect::<Vec<_>>();
                if !greatest_power {
                    return Ok(options);
                }
                let powers = options
                    .into_iter()
                    .map(|object| {
                        let power = self
                            .characteristics(object)?
                            .power
                            .ok_or("missing creature power")?;
                        Ok((object, power))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                // The comparison can rank negative power (CR 107.1b).
                let greatest = powers.iter().map(|(_, power)| power).max();
                Ok(powers
                    .iter()
                    .filter(|(_, power)| Some(power) == greatest)
                    .map(|(object, _)| *object)
                    .collect())
            }
            Some(Instruction::Kernel {
                node: mtg_contracts::kir::Node::SacrificeForTappedReturn { .. },
                ..
            }) => Ok(self.controlled_creatures(player)),
            _ => Err("missing sacrifice instruction".into()),
        }
    }

    pub(super) fn resolving_sacrifice_player(
        &self,
        frame: &Resolving,
        player: &mtg_contracts::kir::PlayerExpr,
    ) -> Result<PlayerId, String> {
        use mtg_contracts::kir::{EventPlayerRole, PlayerExpr, Ref};
        match player {
            you!() => Ok(self.entry(frame.stack_id)?.controller),
            PlayerExpr::Is {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::DamagedPlayer,
                    },
            } => self.resolution_player(frame, Some(&ActionRecipient::EventPlayer)),
            PlayerExpr::Is {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::ActivePlayer,
                    },
            } => self
                .entry(frame.stack_id)?
                .kind
                .event_player()?
                .ok_or_else(|| "missing original active player".into()),
            _ => Err("unbound sacrifice player".into()),
        }
    }

    pub(super) fn resolving_sacrifice_count(&self, frame: &Resolving) -> Result<usize, String> {
        let entry = self.entry(frame.stack_id)?;
        match self
            .program_for(entry)?
            .instructions
            .get(frame.cursor as usize)
        {
            Some(
                kernel!(Sacrifice {
                    count: fixed!(count),
                    ..
                }),
            ) => count
                .to_u32()
                .map(|count| count as usize)
                .ok_or_else(|| "invalid fixed sacrifice count".into()),
            Some(Instruction::Kernel {
                node: mtg_contracts::kir::Node::SacrificeForTappedReturn { .. },
                ..
            }) => Ok(1),
            _ => Err("missing sacrifice instruction".into()),
        }
    }

    pub(super) fn sacrifice_request(&self) -> Result<Option<Decision<Rules>>, String> {
        Ok(match &self.state.rules.frame {
            Frame::SacrificeCost { cast } => {
                let stack = self.top()?;
                let choices = self.next_sacrifices(stack, &cast.sacrifices)?;
                (!choices.is_empty())
                    .then(|| self.pick_objects(stack.controller, Reason::SacrificeCost, choices, 1))
            }
            Frame::ForcedSacrifice {
                frame,
                remaining,
                choices,
                ..
            } => match remaining.first().filter(|&&player| self.is_live(player)) {
                Some(&player) => {
                    let options = self.sacrifice_options(frame, player)?;
                    let count = self.resolving_sacrifice_count(frame)?.min(options.len());
                    (count != 0).then(|| {
                        self.pick_objects(
                            player,
                            Reason::ForcedSacrifice {
                                previous: choices.clone(),
                            },
                            options,
                            count,
                        )
                    })
                }
                None => None,
            },
            _ => None,
        })
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

                let object = input.object()?;
                cast.sacrifices.push(object);
                let (_, count, _) = self.invocation_sacrifice(&stack).ok_or("missing cost")?;
                if u32::try_from(cast.sacrifices.len()) != Ok(count) {
                    self.rules_mut().frame = Frame::SacrificeCost { cast };
                    return Ok(());
                }
                self.payment_step(cast)
            }

            Frame::ForcedSacrifice {
                frame,
                mut remaining,
                mut choices,
                source_block,
            } => {
                if let Some(player) = remaining.first().copied() {
                    if self.is_live(player) && !self.sacrifice_options(&frame, player)?.is_empty() {
                        let input = self
                            .rules_mut()
                            .pending
                            .take()
                            .ok_or("missing forced sacrifice input")?;

                        choices.extend(input.objects()?.into_iter().map(|object| (player, object)));
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
        let Some((_, count, _)) = self.invocation_sacrifice(stack) else {
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
    /// CR 701.21a, 702.74a, 702.30a: the source's current controller ("its
    /// controller sacrifices it") or the ability's controller ("sacrifice
    /// it") sacrifices the exact source permanent, if that player still
    /// controls it on the battlefield. A new object is not "it" (CR 400.7).
    pub(super) fn kernel_sacrifice_source(
        &mut self,
        frame: Resolving,
        player: &mtg_contracts::kir::PlayerExpr,
        source_block: u32,
    ) -> Result<(), String> {
        use mtg_contracts::kir::{PlayerExpr, Ref};
        let entry = self.entry(frame.stack_id)?;
        let (source, controller) = (entry.source, entry.controller);
        let present = self
            .state
            .rules
            .objects
            .get(&source.card)
            .filter(|o| o.id == source && o.present_on_battlefield())
            .map(|o| o.controller);
        let actor = match player {
            PlayerExpr::ControllerOf {
                object: Ref::Source,
            } => present,
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Some(controller),
            _ => return Err("sacrifice-source player has no audited executor".into()),
        };
        match actor {
            Some(actor) if present == Some(actor) && self.is_live(actor) => {
                let cause = BatchCause::Instruction {
                    stack_id: frame.stack_id,
                    player: controller,
                    source,
                    cursor: frame.cursor,
                    source_block,
                };
                self.complete_graveyard_group(
                    cause,
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::Sacrifice {
                            object: source,
                            player: actor,
                        },
                    },
                    GroupFinish::Instruction {
                        frame,
                        result: GroupResult::Sacrifice { player: actor },
                    },
                )
            }
            _ => self.finish_instruction(
                frame,
                InstructionOutcome::Sacrificed {
                    player: actor.unwrap_or(controller),
                    movement: None,
                },
            ),
        }
    }
    pub(super) fn execute_sacrifice_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        match instruction {
            kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachOpponent, .. } @ source_block) =>
            {
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

            i @ kernel!(ModifyUntil {
                what: source_object!(),
                change: mtg_contracts::kir::Change::PowerToughness {
                    power: _,
                    toughness: _
                },
                until: mtg_contracts::kir::Until::EndOfTurn,
                ..
            }) => self.execute_effect_instruction(frame, i),
            _ => Err("not sacrifice instruction".into()),
        }
    }
}
impl RulesTxn {
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
                self.validate_fact_identity(f)?;
            }
            let count = self
                .invocation_sacrifice(e)
                .map_or(0, |(_, count, _)| count);
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
                let includes_caster = matches!(
                    instruction,
                    Some(kernel!(Sacrifice {
                        player: mtg_contracts::kir::PlayerExpr::EachPlayer,
                        ..
                    }))
                );
                let valid = matches!(instruction,Some(kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachOpponent, .. } @ b) | kernel!(Sacrifice { player: mtg_contracts::kir::PlayerExpr::EachPlayer, .. } @ b)) if b==source_block);
                let controller = self.entry(frame.stack_id)?.controller;
                let all = (0..4)
                    .map(|n| PlayerId((self.state.rules.turn_owner.0 + n) % 4))
                    .filter(|p| includes_caster || *p != controller)
                    .collect::<Vec<_>>();
                let count = self.resolving_sacrifice_count(frame)?;
                let order = |player| all.iter().position(|p| p == player);
                if !valid
                    || remaining.iter().any(|player| order(player).is_none())
                    || remaining.windows(2).any(|w| order(&w[0]) >= order(&w[1]))
                    || choices.windows(2).any(|w| order(&w[0].0) > order(&w[1].0))
                    || choices
                        .chunk_by(|a, b| a.0 == b.0)
                        .any(|group| group.len() > count)
                    || choices.iter().any(|(p, o)| {
                        !all.contains(p)
                            || remaining.contains(p)
                            || !self.state.rules.cards.contains_key(&o.card)
                    })
                    || choices
                        .iter()
                        .map(|(_, object)| object)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != choices.len()
                {
                    return Err("snapshot APNAP sacrifice invariant".into());
                }
            }

            _ => {}
        }
        Ok(())
    }
}
