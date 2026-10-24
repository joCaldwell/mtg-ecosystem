//! P6.9 finite sacrifice programs. Pinned CR 118.8, 601.2c/h, 602.2b,
//! 603.2c/10a, 608.2h, 700.4, 701.21a, 101.4, 800.4a and 509.1b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
// Proof and pool guards: docs/game-engine-sacrifice.md, Reviewed resource bounds.
pub(super) const LIVE_TOKEN_CAPACITY: usize = 8192;
pub(super) const DAMAGE_BOUND: usize = 2 * (400 + LIVE_TOKEN_CAPACITY);
pub(super) const EVENT_GROUP_BOUND: usize = 400 + LIVE_TOKEN_CAPACITY + 4;
pub(super) const DEATH_BOUND: usize = 2 * (400 + LIVE_TOKEN_CAPACITY + 1600 + 4);
pub(super) const STACK_BOUND: usize =
    400 + 208 + 24 + 12 * DEATH_BOUND + 400 + 4 + 4 * (400 + 1600 + 4 + DEATH_BOUND / 2);
pub(super) const EFFECT_BOUND: usize = 8192;
impl Game {
    pub(super) fn sacrifice_enabled(&self) -> bool {
        self.profile_at_least(SACRIFICE_PROFILE)
    }
    pub(super) fn sacrifice_candidates(
        &self,
        player: PlayerId,
        filter: &SacrificeFilter,
    ) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield && o.controller == player)
            .filter(|o| {
                self.definition(o.id.card).is_ok_and(|d| {
                    (filter.types.is_empty() || filter.types.contains(&d.characteristics.kind))
                        && filter
                            .subtypes
                            .iter()
                            .all(|s| d.characteristics.subtypes.contains(s))
                })
            })
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn invocation_sacrifice_filter(
        &self,
        entry: &StackInstance,
    ) -> Option<SacrificeFilter> {
        if !self.sacrifice_enabled() {
            return None;
        }
        match entry.kind {
            StackKind::Spell => self.program_for(entry).ok()?.additional_sacrifice.clone(),
            StackKind::Activated { ability, .. } => match &self
                .definition(entry.source.card)
                .ok()?
                .activated
                .get(ability as usize)?
                .cost
            {
                ActivationCost::SacrificePermanent { filter, .. } => Some(filter.clone()),
                _ => None,
            },
            _ => None,
        }
    }
    pub(super) fn check_sacrifice_payment(
        &self,
        stack: &StackInstance,
        cast: &Casting,
    ) -> Result<(), String> {
        if let Some(f) = self.invocation_sacrifice_filter(stack) {
            if cast
                .sacrifice
                .is_none_or(|o| !self.sacrifice_candidates(stack.controller, &f).contains(&o))
            {
                return Err("sacrifice cost is not payable".into());
            }
        } else if cast.sacrifice.is_some() {
            return Err("unexpected sacrifice cost selection".into());
        }
        Ok(())
    }
    pub(super) fn pay_sacrifice_cost(
        &mut self,
        stack: &StackInstance,
        selected: Option<ObjectId>,
    ) -> Result<(), String> {
        let self_cost = match stack.kind {
            StackKind::Activated { ability, .. } => {
                self.definition(stack.source.card)?.activated[ability as usize].cost
                    == ActivationCost::SacrificeSource
            }
            _ => false,
        };
        let selected = selected.or(self_cost.then_some(stack.source));
        if let Some(object) = selected {
            let fact = self.object_before(object)?;
            self.commit(
                ProposedEvent::Sacrifice {
                    object,
                    player: stack.controller,
                },
                BatchCause::Payment {
                    stack_id: stack.id,
                    player: stack.controller,
                },
            )?;
            self.state
                .stack
                .last_mut()
                .ok_or("missing sacrifice invocation")?
                .sacrificed = Some(fact);
        }
        Ok(())
    }
    pub(super) fn sacrifice_request(&self) -> Result<Option<Request>, String> {
        let decision_id = self.state.revision;
        Ok(match &self.state.frame {
            Frame::SacrificeCost { .. } => {
                let stack = self.top()?;
                let f = self
                    .invocation_sacrifice_filter(stack)
                    .ok_or("missing sacrifice filter")?;
                let candidates = self.sacrifice_candidates(stack.controller, &f);
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
            Frame::OptionalDamage { frame } => {
                self.is_live(frame.spell.controller)
                    .then_some(Request::OptionalEffect {
                        decision_id,
                        player: frame.spell.controller,
                    })
            }
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
    pub(super) fn sacrifice_step(&mut self) -> Result<(), String> {
        match self.state.frame.clone() {
            Frame::SacrificeCost { mut cast } => {
                let stack = self.top()?.clone();
                let filter = self
                    .invocation_sacrifice_filter(&stack)
                    .ok_or("missing cost")?;
                if self
                    .sacrifice_candidates(stack.controller, &filter)
                    .is_empty()
                {
                    let undo = cast.undo_mana;
                    return self.reverse_cast(cast, "sacrifice no longer payable", undo);
                }
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing sacrifice cost input")?;
                self.validate_sacrifice_response(input.player, &input.response)?;
                let Response::SacrificeCost { object } = input.response else {
                    return Err("cost response invariant".into());
                };
                cast.sacrifice = Some(object);
                self.payment_step(cast)
            }
            Frame::OptionalDamage { frame } => {
                let perform = if self.is_live(frame.spell.controller) {
                    let input = self
                        .state
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
                            .state
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
                    self.state.frame = Frame::ForcedSacrifice {
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
                                && self.current(*o).is_ok_and(|v| {
                                    v.zone == Zone::Battlefield && v.controller == *p
                                })
                        })
                        .map(|(player, object)| ProposedEvent::Sacrifice { player, object })
                        .collect::<Vec<_>>();
                    if !children.is_empty() {
                        self.commit_batch(
                            Self::sacrifice_cause(&frame, source_block),
                            EventGroup::Simultaneous {
                                occurrences: children,
                            },
                        )?;
                    }
                    self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
                }
            }
            _ => Err("sacrifice frame invariant".into()),
        }
    }
    fn sacrifice_cause(frame: &Resolving, source_block: u32) -> BatchCause {
        BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block,
        }
    }
    pub(super) fn execute_sacrifice_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        if !self.sacrifice_enabled() {
            return Err("unsupported sacrifice profile".into());
        }
        match instruction {
            Instruction::EachOpponentSacrifice { source_block } => {
                let remaining = (0..4)
                    .map(|n| PlayerId((self.state.turn_owner.0 + n) % 4))
                    .filter(|p| self.is_live(*p) && *p != frame.spell.controller)
                    .collect();
                self.state.frame = Frame::ForcedSacrifice {
                    frame,
                    remaining,
                    choices: vec![],
                    source_block,
                };
                Ok(())
            }
            Instruction::SacrificeDamage { optional: true, .. } => {
                self.state.frame = Frame::OptionalDamage { frame };
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
        } = &self.program_for(&frame.spell)?.instructions[frame.cursor as usize]
        else {
            return Err("not sacrifice damage".into());
        };
        let amount = match amount {
            SacrificeDamageAmount::Fixed { value } => value.clone(),
            SacrificeDamageAmount::SacrificedPower => std::cmp::max(
                frame
                    .spell
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
                    *frame
                        .spell
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
                    player: frame.spell.controller,
                }],
                Some(LifeRecipient::EachOpponent) => self
                    .opponents(frame.spell.controller)
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
    pub(super) fn sacrifice_life(
        &mut self,
        frame: Resolving,
        amount: Quantity,
        block: u32,
        gain: bool,
    ) -> Result<(), String> {
        if !self.sacrifice_enabled() {
            return Err("unsupported each-opponent life".into());
        }
        let children = self
            .opponents(frame.spell.controller)
            .into_iter()
            .map(|player| {
                if gain {
                    ProposedEvent::GainLife {
                        player,
                        amount: amount.clone(),
                    }
                } else {
                    ProposedEvent::LoseLife {
                        player,
                        amount: amount.clone(),
                    }
                }
            })
            .collect::<Vec<_>>();
        if !children.is_empty() {
            self.commit_batch(
                Self::sacrifice_cause(&frame, block),
                EventGroup::Simultaneous {
                    occurrences: children,
                },
            )?;
        }
        self.finish_instruction(frame, InstructionOutcome::Life { actual: None })
    }
}
impl Game {
    pub(super) fn validate_sacrifice_state(&self) -> Result<(), String> {
        for e in &self.state.stack {
            if let Some(f) = &e.sacrificed {
                if !self.sacrifice_enabled()
                    || f.zone != Zone::Battlefield
                    || f.controller != e.controller
                    || self
                        .state
                        .cards
                        .get(&f.object.card)
                        .is_none_or(|c| c.owner != f.owner)
                    || (!matches!(e.kind, StackKind::Activated { .. })
                        && self.invocation_sacrifice_filter(e).is_none())
                {
                    return Err("snapshot sacrificed cost facts invariant".into());
                }
                self.validate_characteristic_identity(f.object, &f.characteristics)?;
            }
            if e.paid.is_some()
                && self.invocation_sacrifice_filter(e).is_some()
                && e.sacrificed.is_none()
            {
                return Err("snapshot missing sacrifice cost facts".into());
            }
        }
        match &self.state.frame {
            Frame::SacrificeCost { cast } => {
                if !self.sacrifice_enabled()
                    || self.invocation_sacrifice_filter(self.top()?).is_none()
                    || cast.spending.is_none()
                    || cast.sacrifice.is_some()
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
                    .program_for(&frame.spell)?
                    .instructions
                    .get(frame.cursor as usize);
                let includes_caster =
                    matches!(instruction, Some(Instruction::EachPlayerSacrifice { .. }));
                let valid = matches!(instruction,Some(Instruction::EachOpponentSacrifice {source_block:b} | Instruction::EachPlayerSacrifice {source_block:b}) if b==source_block)
                    && (!includes_caster || self.wipes_enabled());
                let all = (0..4)
                    .map(|n| PlayerId((self.state.turn_owner.0 + n) % 4))
                    .filter(|p| includes_caster || *p != frame.spell.controller)
                    .collect::<Vec<_>>();
                if !self.sacrifice_enabled()
                    || !valid
                    || remaining.len() > all.len()
                    || choices.len() > all.len()
                    || remaining.windows(2).any(|w| {
                        all.iter().position(|p| p == &w[0]) >= all.iter().position(|p| p == &w[1])
                    })
                    || choices.iter().any(|(p, o)| {
                        !all.contains(p)
                            || remaining.contains(p)
                            || !self.state.cards.contains_key(&o.card)
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
                if !self.sacrifice_enabled()
                    || !matches!(
                        self.program_for(&frame.spell)?
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
