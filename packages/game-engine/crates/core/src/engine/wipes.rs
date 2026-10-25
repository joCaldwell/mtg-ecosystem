//! P6.10. Pinned rules-20260925.txt: CR 101.2, 104.4a, 107.3a/c,
//! 113.6g, 118.7a, 120.4, 202.3e, 601.2b/f,
//! 603.2c/10a, 608.2c/h, 611.2c, 614.1a/6, 701.8a, 903.9a/b, 514.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{ObjectSet, SetMove, WipeAmount};
// See docs/game-engine-board-wipes.md, Reviewed resource bounds.

impl Game {
    pub(super) fn objects_in_set(&self, set: &ObjectSet, controller: PlayerId) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .filter(|o| {
                let Ok(d) = self.definition(o.id.card) else {
                    return false;
                };
                let creature = d.characteristics.kind == CardKind::Creature;
                match set {
                    ObjectSet::Creatures => creature,
                    ObjectSet::NonlandPermanents => d.characteristics.kind != CardKind::Land,
                    ObjectSet::Enchantments => d.characteristics.kind == CardKind::Enchantment,
                    ObjectSet::AttackingCreatures => {
                        creature && self.state.combat.iter().any(|a| a.attacker == o.id)
                    }
                    ObjectSet::OpponentCreatures => creature && o.controller != controller,
                    ObjectSet::FlyingCreatures => {
                        creature && self.has_keyword(o.id, KeywordAbility::Flying)
                    }
                    ObjectSet::NonflyingCreatures => {
                        creature && !self.has_keyword(o.id, KeywordAbility::Flying)
                    }
                }
            })
            .map(|o| o.id)
            .collect()
    }
    pub(super) fn wipe_cause(frame: &Resolving, source_block: u32) -> BatchCause {
        BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block,
        }
    }
    pub(super) fn stack_mana_value(&self, entry: &StackInstance) -> Result<Quantity, String> {
        let c = &self
            .definition(entry.source.card)?
            .characteristics
            .mana_cost;
        let mut q = c.generic.clone();
        q.add(&Quantity::from(c.colored.len() as i64));
        if let Some(x) = &entry.x {
            q.add(x);
        }
        Ok(q)
    }
    pub(super) fn bind_x(&mut self, cast: Casting) -> Result<(), String> {
        let input = self.state.pending.take().ok_or("missing X input")?;
        self.validate_response(input.player, &input.response)?;
        let Response::AnnounceX { value } = input.response else {
            return Err("X response invariant".into());
        };
        let top = self.state.stack.last_mut().ok_or("missing X spell")?;
        if top.x.is_some() {
            return Err("X already announced".into());
        }
        top.x = Some(value.clone());
        top.total_cost.generic.add(&value);
        let stack_id = top.id;
        self.record_event(Event::XAnnounced {
            stack_id,
            player: input.player,
            value,
        })?;
        self.state.frame = Frame::Mana { cast };
        Ok(())
    }
    pub(super) fn cant_counter(&self, id: StackId) -> bool {
        self.state
            .stack
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|e| {
                e.kind.is_spell()
                    && self.definition(e.source.card).is_ok_and(|d| {
                        d.static_abilities
                            .iter()
                            .any(|a| a.effect == StaticEffect::CantBeCountered)
                    })
            })
    }
    pub(super) fn anger_replaces_death(&self, object: ObjectId, destination: Zone) -> bool {
        matches!(destination,Zone::Graveyard { .. }) && self.current(object).is_ok_and(|o|o.zone==Zone::Battlefield) && self.state.effects.iter().any(|e|matches!(&e.kind,ResolvedEffectKind::ExileDamaged {targets} if targets.contains(&object)))
    }
    pub(super) fn move_board_set(
        &mut self,
        frame: Resolving,
        set: ObjectSet,
        action: SetMove,
        block: u32,
    ) -> Result<(), String> {
        let objects = self.objects_in_set(&set, frame.spell.controller);
        if action == SetMove::Hand {
            let mut remaining = vec![];
            for n in 0..4 {
                let p = PlayerId((self.state.turn_owner.0 + n) % 4);
                remaining.extend(objects.iter().copied().filter(|o| {
                    self.state.cards[&o.card].owner == p
                        && self.state.commanders.history(o.card).is_some()
                }));
            }
            return self.continue_board_return(frame, objects, remaining, vec![], block);
        }
        self.commit_board_moves(frame, objects, action, vec![], block)
    }
    fn commit_board_moves(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        action: SetMove,
        choices: Vec<(ObjectId, bool)>,
        block: u32,
    ) -> Result<(), String> {
        let mut destroyed_creatures = 0;
        let occurrences = objects
            .into_iter()
            .filter(|o| self.current(*o).is_ok_and(|o| o.zone == Zone::Battlefield))
            .map(|object| {
                let owner = self.state.cards[&object.card].owner;
                match action {
                    SetMove::Destroy => {
                        if self.target_is_creature(object) {
                            destroyed_creatures += 1;
                        }
                        ProposedEvent::DestroyPermanent { object }
                    }
                    SetMove::Exile => ProposedEvent::Move {
                        object,
                        destination: Zone::Exile,
                    },
                    SetMove::Hand => ProposedEvent::Move {
                        object,
                        destination: if choices.contains(&(object, true)) {
                            Zone::Command { owner }
                        } else {
                            Zone::Hand { owner }
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        let mut movements = vec![];
        if !occurrences.is_empty() {
            let batch = self.commit_batch(
                Self::wipe_cause(&frame, block),
                EventGroup::Simultaneous { occurrences },
            )?;
            for r in batch.occurrences {
                if let EventOutcome::Moved { movement } = r.actual {
                    movements.push(movement);
                }
            }
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::SetMoved {
                movements,
                destroyed_creatures,
            },
        )
    }
    pub(super) fn continue_board_return(
        &mut self,
        frame: Resolving,
        objects: Vec<ObjectId>,
        mut remaining: Vec<ObjectId>,
        choices: Vec<(ObjectId, bool)>,
        source_block: u32,
    ) -> Result<(), String> {
        remaining.retain(|o| {
            self.current(*o).is_ok_and(|v| v.zone == Zone::Battlefield)
                && self.is_live(self.state.cards[&o.card].owner)
        });
        if let Some(object) = remaining.first().copied() {
            self.state.frame = Frame::CommanderReturn {
                object,
                destination: Zone::Hand {
                    owner: self.state.cards[&object.card].owner,
                },
                continuation: Box::new(CommanderContinuation::BoardReturn {
                    frame,
                    objects,
                    remaining,
                    choices,
                    source_block,
                }),
            };
            Ok(())
        } else {
            self.commit_board_moves(frame, objects, SetMove::Hand, choices, source_block)
        }
    }
    pub(super) fn validate_board_return(
        &self,
        frame: &Resolving,
        objects: &[ObjectId],
        remaining: &[ObjectId],
        choices: &[(ObjectId, bool)],
        block: u32,
    ) -> Result<(), String> {
        self.validate_resolving(frame)?;
        if !matches!(self.program_for(&frame.spell)?.instructions.get(frame.cursor as usize),Some(Instruction::MoveSet {action:SetMove::Hand,source_block,..}) if *source_block==block)
            || objects.len() > self.event_group_limit()
            || objects.iter().collect::<BTreeSet<_>>().len() != objects.len()
            || remaining.iter().any(|o| !objects.contains(o))
            || choices
                .iter()
                .any(|(o, _)| !objects.contains(o) || remaining.contains(o))
        {
            return Err("invalid mass return continuation".into());
        }
        Ok(())
    }
    pub(super) fn wipe_damage_spec(
        &self,
        frame: &Resolving,
    ) -> Result<(Quantity, u32, Vec<TargetEntity>), String> {
        let Instruction::DamageSet {
            sets,
            each_player,
            amount,
            source_block,
        } = &self.program_for(&frame.spell)?.instructions[frame.cursor as usize]
        else {
            return Err("not set damage".into());
        };
        let amount = match amount {
            WipeAmount::Fixed { value } => value.clone(),
            WipeAmount::AnnouncedX => frame.spell.x.clone().ok_or("missing announced X")?,
            WipeAmount::CreatureCount => Quantity::from(
                self.objects_in_set(&ObjectSet::Creatures, frame.spell.controller)
                    .len() as i64,
            ),
        };
        let mut objects = BTreeSet::new();
        for set in sets {
            objects.extend(self.objects_in_set(set, frame.spell.controller));
        }
        let mut targets = objects
            .into_iter()
            .map(|object| TargetEntity::Object { object })
            .collect::<Vec<_>>();
        if *each_player {
            targets.extend(
                self.live_players()
                    .into_iter()
                    .map(|player| TargetEntity::Player { player }),
            );
        }
        Ok((amount, *source_block, targets))
    }
    pub(super) fn anger_targets(&self, frame: &Resolving) -> Vec<ObjectId> {
        frame
            .outcomes
            .iter()
            .flat_map(|o| match o {
                InstructionOutcome::Damage {
                    actual: Some(EventOutcome::DamageCommitted { actual, .. }),
                } => actual
                    .iter()
                    .filter_map(|d| match d.recipient {
                        DamageRecipient::Creature { object } if d.amount > Quantity::zero() => {
                            Some(object)
                        }
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            })
            .collect()
    }
    pub(super) fn create_set_effect(
        &mut self,
        frame: Resolving,
        kind: ResolvedEffectKind,
        block: u32,
    ) -> Result<(), String> {
        let handles = match &frame.spell.kind {
            StackKind::Spell => frame.spell.effect_source_handles.clone(),
            StackKind::Activated { source_handles, .. }
            | StackKind::Triggered { source_handles, .. } => source_handles.clone(),
        };
        let targets = match &kind {
            ResolvedEffectKind::PumpSet { targets, .. }
            | ResolvedEffectKind::ExileDamaged { targets } => targets,
            _ => return Err("not set effect".into()),
        };
        let mut target_handles = vec![];
        for object in targets {
            target_handles.extend(self.capture_trigger_source_handles(*object)?);
        }
        let effect = ResolvedEffect {
            target_handles,
            id: EffectId(self.state.next_effect_id),
            origin: EffectOrigin {
                mode: frame.spell.mode,
                stack_id: frame.spell.id,
                source: frame.spell.source,
                controller: frame.spell.controller,
                source_handles: handles,
                cursor: frame.cursor,
                source_block: block,
            },
            expires_in_cleanup_of_turn: self.state.turn.number,
            kind,
        };
        let actual = self.commit(
            ProposedEvent::CreateEffect {
                effect: Box::new(effect),
            },
            Self::wipe_cause(&frame, block),
        )?;
        self.finish_instruction(
            frame,
            InstructionOutcome::Effect {
                actual: Some(actual),
            },
        )
    }
    pub(super) fn execute_wipes_instruction(
        &mut self,
        mut frame: Resolving,
        i: Instruction,
    ) -> Result<(), String> {
        match i {
            Instruction::MoveSet {
                set,
                action,
                source_block,
            } => self.move_board_set(frame, set, action, source_block),
            Instruction::PumpSet {
                set,
                power,
                toughness,
                source_block,
            } => {
                let targets = self.objects_in_set(&set, frame.spell.controller);
                self.create_set_effect(
                    frame,
                    ResolvedEffectKind::PumpSet {
                        targets,
                        power,
                        toughness,
                    },
                    source_block,
                )
            }
            Instruction::ExileDamagedThisTurn { source_block } => {
                let targets = self.anger_targets(&frame);
                self.create_set_effect(
                    frame,
                    ResolvedEffectKind::ExileDamaged { targets },
                    source_block,
                )
            }
            Instruction::DamageSet { .. } => {
                let (amount, block, targets) = self.wipe_damage_spec(&frame)?;
                frame.set_damage = Some(SetDamageFacts {
                    cursor: frame.cursor,
                    amount: amount.clone(),
                    targets: targets.clone(),
                });
                if amount == Quantity::zero() || targets.is_empty() {
                    self.finish_instruction(frame, InstructionOutcome::Damage { actual: None })
                } else {
                    self.prepare_noncombat_set_damage(frame, amount, block, targets)
                }
            }
            Instruction::GainLifeDestroyed { source_block } => {
                let count = frame
                    .outcomes
                    .iter()
                    .filter_map(|o| {
                        if let InstructionOutcome::SetMoved {
                            destroyed_creatures,
                            ..
                        } = o
                        {
                            Some(*destroyed_creatures)
                        } else {
                            None
                        }
                    })
                    .sum::<usize>();
                let actual = if count > 0 && self.is_live(frame.spell.controller) {
                    Some(self.commit(
                        ProposedEvent::GainLife {
                            player: frame.spell.controller,
                            amount: Quantity::from(count as i64),
                        },
                        Self::wipe_cause(&frame, source_block),
                    )?)
                } else {
                    None
                };
                self.finish_instruction(frame, InstructionOutcome::Life { actual })
            }
            Instruction::LoseLifeDeathController {
                amount,
                source_block,
            } => {
                let player = frame
                    .spell
                    .death_controller
                    .ok_or("missing death controller LKI")?;
                let actual = if self.is_live(player) {
                    Some(self.commit(
                        ProposedEvent::LoseLife { player, amount },
                        Self::wipe_cause(&frame, source_block),
                    )?)
                } else {
                    None
                };
                self.finish_instruction(frame, InstructionOutcome::Life { actual })
            }
            Instruction::EachPlayerSacrifice { source_block } => {
                let remaining = (0..4)
                    .map(|n| PlayerId((self.state.turn_owner.0 + n) % 4))
                    .filter(|p| self.is_live(*p))
                    .collect();
                self.state.frame = Frame::ForcedSacrifice {
                    frame,
                    remaining,
                    choices: vec![],
                    source_block,
                };
                Ok(())
            }
            _ => Err("not wipe instruction".into()),
        }
    }
}

impl Game {
    pub(super) fn valid_set_effect_targets(
        &self,
        e: &ResolvedEffect,
        targets: &[ObjectId],
    ) -> bool {
        e.target_handles.len() == targets.len() * 4
            && targets.len() <= self.event_group_limit()
            && targets.iter().collect::<BTreeSet<_>>().len() == targets.len()
            && targets.iter().all(|o| {
                self.state.cards.contains_key(&o.card)
                    && self
                        .definition(o.card)
                        .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
            })
    }
    pub(super) fn validate_wipe_damage_transaction(
        &self,
        tx: &DamageTransaction,
        frame: &Resolving,
    ) -> Result<(), String> {
        self.validate_set_damage_facts(frame)?;
        let facts = frame
            .set_damage
            .as_ref()
            .ok_or("missing frozen set damage")?;
        let Instruction::DamageSet { source_block, .. } =
            &self.program_for(&frame.spell)?.instructions[frame.cursor as usize]
        else {
            return Err("not set damage".into());
        };
        let source = self.source_facts(&frame.spell)?;
        if facts.cursor != frame.cursor
            || tx.cause != Self::wipe_cause(frame, *source_block)
            || tx.entries.len() != facts.targets.len()
            || tx.entries.iter().zip(&facts.targets).any(|(d, t)| {
                d.recipient.entity() != *t
                    || d.original_amount != facts.amount
                    || d.controller != frame.spell.controller
                    || !self.frozen_damage_source_matches(&d.source, source.clone())
            })
        {
            return Err("invalid frozen set damage transaction".into());
        }
        Ok(())
    }
    pub(super) fn validate_set_damage_facts(&self, frame: &Resolving) -> Result<(), String> {
        let Some(f) = &frame.set_damage else {
            return Ok(());
        };
        let Some(Instruction::DamageSet { amount, .. }) = self
            .program_for(&frame.spell)?
            .instructions
            .get(f.cursor as usize)
        else {
            return Err("unbound set damage facts".into());
        };
        if f.cursor > frame.cursor
            || f.amount.is_negative()
            || f.targets.len() > self.damage_limit()
            || f.targets.iter().collect::<BTreeSet<_>>().len() != f.targets.len()
            || f.targets.iter().any(|t| match t {
                TargetEntity::Player { player } => player.0 >= 4,
                TargetEntity::Object { object } => {
                    !self.state.cards.contains_key(&object.card)
                        || !self
                            .definition(object.card)
                            .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
                }
            })
            || match amount {
                WipeAmount::Fixed { value } => *value != f.amount,
                WipeAmount::AnnouncedX => frame.spell.x.as_ref() != Some(&f.amount),
                WipeAmount::CreatureCount => {
                    f.amount
                        > Quantity::from(
                            (4 * capacities::OWNED_LIMIT + capacities::TOKEN_CAPACITY as usize)
                                as i64,
                        )
                }
            }
        {
            return Err("invalid set damage facts".into());
        }
        Ok(())
    }
}
