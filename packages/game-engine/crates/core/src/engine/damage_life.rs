use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetEntity {
    Player { player: PlayerId },
    Object { object: ObjectId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoncombatDamageOccurrence {
    pub source: ObjectBefore,
    /// Controller of the resolving invocation; source.controller remains its
    /// current or last-known controller (CR 608.2h, 113.7a).
    pub controller: PlayerId,
    pub recipient: DamageRecipient,
    pub amount: Quantity,
}
fn update_entry(entry: &mut StackInstance, fact: &ObjectBefore) {
    if entry.source == fact.object && entry.source_facts.is_some() {
        entry.source_facts = Some(fact.clone());
    }
}
fn resolving_entry(frame: &Frame) -> Option<&StackInstance> {
    match frame {
        Frame::Resolve { frame }
        | Frame::ResolutionChoice { frame, .. }
        | Frame::ForcedSacrifice { frame, .. }
        | Frame::OptionalDamage { frame } => Some(&frame.spell),
        Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } => {
            match transaction.continuation.as_ref() {
                DamageContinuation::Instruction { frame } => Some(&frame.spell),
                DamageContinuation::Combat => None,
            }
        }
        Frame::Stabilize { resume, .. } => resolving_entry(resume),
        Frame::CommanderReturn { continuation, .. } => match continuation.as_ref() {
            CommanderContinuation::BoardReturn { frame, .. }
            | CommanderContinuation::SpellMove { frame, .. }
            | CommanderContinuation::ResolutionDraw { frame, .. } => Some(&frame.spell),
            CommanderContinuation::StateBased { resume }
            | CommanderContinuation::StateBasedBatch { resume, .. } => resolving_entry(resume),
            CommanderContinuation::Draw { .. } => None,
        },
        _ => None,
    }
}
fn update_frame(frame: &mut Frame, fact: &ObjectBefore) {
    match frame {
        Frame::Resolve { frame }
        | Frame::ResolutionChoice { frame, .. }
        | Frame::ForcedSacrifice { frame, .. }
        | Frame::OptionalDamage { frame } => update_entry(&mut frame.spell, fact),
        Frame::DamageEffects { transaction } | Frame::DamageAllocation { transaction, .. } => {
            if let DamageContinuation::Instruction { frame } = transaction.continuation.as_mut() {
                update_entry(&mut frame.spell, fact);
            }
            // Assigned damage retains its frozen assignment source facts.
        }
        Frame::Stabilize { resume, .. } => update_frame(resume, fact),
        Frame::CommanderReturn { continuation, .. } => match continuation.as_mut() {
            CommanderContinuation::BoardReturn { frame, .. }
            | CommanderContinuation::SpellMove { frame, .. }
            | CommanderContinuation::ResolutionDraw { frame, .. } => {
                update_entry(&mut frame.spell, fact)
            }
            CommanderContinuation::StateBased { resume }
            | CommanderContinuation::StateBasedBatch { resume, .. } => update_frame(resume, fact),
            CommanderContinuation::Draw { .. } => {}
        },
        _ => {}
    }
}
impl Game {
    pub(super) fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String> {
        let o = self.current(object)?;
        Ok(ObjectBefore {
            keywords: { self.current_keywords(object) },
            deathtouch_damage: o.deathtouch_damage,
            object,
            owner: self
                .state
                .cards
                .get(&object.card)
                .ok_or("missing source card")?
                .owner,
            controller: o.controller,
            default_controller: o.default_controller,
            zone: o.zone,
            tapped: o.tapped,
            marked_damage: o.marked_damage.clone(),
            attack_ready: o.attack_ready,
            characteristics: self.characteristics(object)?,
        })
    }
    // CR 608.2h/113.7a: retain the exact last-known incarnation before it
    // leaves, including paid tap state; a successor cannot become this source.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn retain_source_lki(&mut self, object: ObjectId) -> Result<(), String> {
        let fact = self.object_before(object)?;
        self.retain_source_facts(&fact);
        Ok(())
    }
    pub(super) fn retain_source_facts(&mut self, fact: &ObjectBefore) {
        let object = fact.object;
        for trigger in &mut self.state.pending_triggers {
            if trigger.source == object {
                trigger.source_facts = fact.clone();
            }
        }
        for entry in &mut self.state.stack {
            update_entry(entry, fact);
        }
        update_frame(&mut self.state.frame, fact);
    }
    pub(super) fn source_facts(&self, entry: &StackInstance) -> Result<ObjectBefore, String> {
        self.object_before(entry.source).or_else(|_| {
            entry
                .source_facts
                .clone()
                .ok_or("missing last-known damage source".into())
        })
    }
    pub(super) fn eligible_players(
        &self,
        domain: TargetDomain,
        controller: PlayerId,
    ) -> Vec<PlayerId> {
        self.live_players()
            .into_iter()
            .filter(|p| domain == TargetDomain::AnyPlayer || *p != controller)
            .collect()
    }
    pub(super) fn player_target_legal(
        &self,
        entry: &StackInstance,
        slot: usize,
        player: PlayerId,
    ) -> bool {
        self.program_for(entry).is_ok_and(|p| {
            p.targets.get(slot).is_some_and(|spec| {
                matches!(
                    spec.domain,
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer
                ) && self
                    .eligible_players(spec.domain, entry.controller)
                    .contains(&player)
            })
        })
    }
    pub(super) fn entity_legal(&self, target: TargetEntity) -> bool {
        match target {
            TargetEntity::Player { player } => self.is_live(player),
            TargetEntity::Object { object } => self.target_legal(object),
        }
    }
    pub(super) fn eligible_entities(&self) -> Vec<TargetEntity> {
        self.live_players()
            .into_iter()
            .map(|player| TargetEntity::Player { player })
            .chain(
                self.state
                    .objects
                    .values()
                    .filter(|o| {
                        self.target_legal(o.id)
                            && self
                                .top()
                                .is_ok_and(|e| self.targetable_by(o.id, e.controller))
                    })
                    .map(|o| TargetEntity::Object { object: o.id }),
            )
            .collect()
    }
    pub(super) fn validate_entity_targets(
        &self,
        player: PlayerId,
        entities: &[TargetEntity],
    ) -> Result<(), String> {
        let top = self.top()?;
        let program = self.program_for(top)?;
        if player != top.controller
            || entities.len() != program.targets.len()
            || entities.iter().enumerate().any(|(slot, target)| {
                program.targets[slot].domain != TargetDomain::AnyDamageTarget
                    || !self.entity_target_legal(*target, top.controller)
            })
        {
            return Err("invalid damage targets".into());
        }
        Ok(())
    }
    pub(super) fn bind_entity_targets(&mut self, cast: Casting) -> Result<(), String> {
        let input = self.state.pending.take().ok_or("missing entity targets")?;
        let Response::EntityTargets { entities } = input.response else {
            return Err("entity target response invariant".into());
        };
        self.validate_entity_targets(input.player, &entities)?;
        let announced = (0..4)
            .map(|viewer| {
                entities
                    .iter()
                    .map(|target| match target {
                        TargetEntity::Player { .. } => Ok(None),
                        TargetEntity::Object { object } => {
                            self.handle_for(PlayerId(viewer), *object).map(Some)
                        }
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .state
            .stack
            .last_mut()
            .ok_or("missing targeted invocation")?;
        top.entity_targets = entities;
        top.announced_entity_targets = announced;
        if cast.origin.is_spell() {
            self.state.frame = Frame::Mana { cast };
        } else {
            self.complete_targeted_activation(cast, input.decision_id)?;
        }
        Ok(())
    }
    pub(super) fn complete_targeted_activation(
        &mut self,
        cast: Casting,
        decision_id: u64,
    ) -> Result<(), String> {
        let entry = self.top()?.clone();
        let StackKind::Activated { ability, .. } = entry.kind else {
            return Err("targeted activation kind".into());
        };
        {
            match &self.definition(entry.source.card)?.activated[ability as usize].cost {
                ActivationCost::SacrificePermanent { mana: None, .. } => {
                    let mut cast = cast;
                    cast.spending = Some(BTreeMap::new());
                    self.state.frame = Frame::SacrificeCost { cast };
                    return Ok(());
                }
                ActivationCost::SacrificePermanent { mana: Some(_), .. } => {
                    self.state.frame = Frame::Mana { cast };
                    return Ok(());
                }
                ActivationCost::SacrificeSource => {
                    let mut cast = cast;
                    cast.spending = Some(BTreeMap::new());
                    return self.payment_step(cast);
                }
                _ => {}
            }
        }
        // CR 602.2b/601.2c,h: announce targets before paying the tap cost;
        // there is no mana window when this cost contains no mana.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let source_payable = self.current(entry.source).is_ok_and(|source| {
            source.zone == Zone::Battlefield
                && source.controller == entry.controller
                && !source.tapped
                && (source.attack_ready || self.has_keyword(source.id, KeywordAbility::Haste))
        });
        if !source_payable
            || self.definition(entry.source.card)?.activated[ability as usize].cost
                != ActivationCost::TapSource
        {
            return self.reverse_cast(cast, "activation source no longer payable", true);
        }
        self.commit(
            ProposedEvent::Tap {
                object: entry.source,
            },
            BatchCause::AbilityActivation {
                stack_id: entry.id,
                player: entry.controller,
                source: entry.source,
                decision_id,
                ability,
            },
        )?;
        self.state
            .stack
            .last_mut()
            .ok_or("missing activated invocation")?
            .paid = Some(entry.total_cost);
        self.record_event(Event::AbilityActivated {
            stack_id: entry.id,
            source: entry.source,
            player: entry.controller,
            ability,
        })?;
        self.state.priority = entry.controller;
        self.state.frame = Frame::Priority;
        self.stabilize()
    }
    pub(super) fn validate_damage_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        if let Some(fact) = &entry.source_facts {
            self.validate_characteristic_identity(entry.source, &fact.characteristics)?;
        }
        if entry.kind.is_spell() {
            if entry.effect_source_handles.len() != 4 {
                return Err("missing public spell source references".into());
            }
        } else if !entry.effect_source_handles.is_empty() {
            return Err("unexpected spell effect source references".into());
        }
        let program = self.program_for(entry)?;
        let damage = program
            .targets
            .first()
            .is_some_and(|t| t.domain == TargetDomain::AnyDamageTarget);
        {
            let fact = entry
                .source_facts
                .as_ref()
                .ok_or("snapshot missing source facts")?;
            let card = self
                .state
                .cards
                .get(&entry.source.card)
                .ok_or("snapshot unknown source")?;
            if fact.object != entry.source
                || fact.owner != card.owner
                || fact.controller.0 >= 4
                || fact.default_controller.0 >= 4
            {
                return Err("snapshot damage source facts invariant".into());
            }
        }
        if !entry.entity_targets.is_empty() {
            if !damage
                || !entry.targets.is_empty()
                || !entry.player_targets.is_empty()
                || !entry.stack_targets.is_empty()
                || entry.entity_targets.len() > program.targets.len()
                || entry.announced_entity_targets.len() != 4
                || entry.announced_entity_targets.iter().any(|row| {
                    row.len() != entry.entity_targets.len()
                        || row
                            .iter()
                            .zip(&entry.entity_targets)
                            .any(|(h, t)| h.is_some() != matches!(t, TargetEntity::Object { .. }))
                })
                || entry.entity_targets.iter().any(|t| match t {
                    TargetEntity::Player { player } => player.0 >= 4,
                    TargetEntity::Object { object } => !self.state.cards.contains_key(&object.card),
                })
            {
                return Err("snapshot entity binding invariant".into());
            }
        } else if !entry.announced_entity_targets.is_empty() {
            return Err("snapshot entity announcement invariant".into());
        }
        if damage
            && (!entry.targets.is_empty()
                || !entry.player_targets.is_empty()
                || !entry.stack_targets.is_empty())
        {
            return Err("snapshot wrong damage binding domain".into());
        }
        Ok(())
    }
    // CR 608.2b-c/704.4: execute the ordered program without an intervening
    // SBA check. A self-targeted Soul Feast may reach zero then recover before
    // the next priority checkpoint; an all-target-illegal spell never starts.
    pub(super) fn execute_damage_life_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let gain = matches!(instruction, Instruction::GainLife { .. });
        let (block, proposed, damage) = match instruction {
            Instruction::DealDamage {
                target_slot,
                amount,
                source_block,
            } => {
                let target = *frame
                    .spell
                    .entity_targets
                    .get(target_slot as usize)
                    .ok_or("missing damage binding")?;
                if !frame
                    .legal_targets
                    .get(target_slot as usize)
                    .copied()
                    .unwrap_or(false)
                    || !self.entity_legal(target)
                {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Damage { actual: None });
                }
                let recipient = match target {
                    TargetEntity::Player { player } => DamageRecipient::Player { player },
                    TargetEntity::Object { object } => DamageRecipient::Creature { object },
                };
                (
                    source_block,
                    ProposedEvent::NoncombatDamage {
                        damage: Box::new(NoncombatDamageOccurrence {
                            source: self.source_facts(&frame.spell)?,
                            controller: frame.spell.controller,
                            recipient,
                            amount,
                        }),
                    },
                    true,
                )
            }
            Instruction::GainLife {
                recipient,
                amount,
                source_block,
            }
            | Instruction::LoseLife {
                recipient,
                amount,
                source_block,
            } => {
                let player = match recipient {
                    LifeRecipient::EachOpponent => {
                        return self.sacrifice_life(frame, amount, source_block, gain);
                    }
                    LifeRecipient::Controller => frame.spell.controller,
                    LifeRecipient::Target { target_slot } => {
                        if !frame
                            .legal_targets
                            .get(target_slot as usize)
                            .copied()
                            .unwrap_or(false)
                        {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::Life { actual: None },
                            );
                        }
                        *frame
                            .spell
                            .player_targets
                            .get(target_slot as usize)
                            .ok_or("missing life player target")?
                    }
                };
                if !self.is_live(player) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Life { actual: None });
                }
                let proposed = if gain {
                    ProposedEvent::GainLife { player, amount }
                } else {
                    ProposedEvent::LoseLife { player, amount }
                };
                (source_block, proposed, false)
            }
            _ => return Err("unexpected damage/life instruction".into()),
        };
        if let ProposedEvent::NoncombatDamage { damage } = proposed {
            return self.prepare_noncombat_damage(frame, *damage, block);
        }
        let actual = self.commit(
            proposed,
            BatchCause::Instruction {
                stack_id: frame.spell.id,
                player: frame.spell.controller,
                source: frame.spell.source,
                cursor: frame.cursor,
                source_block: block,
            },
        )?;
        self.finish_instruction(
            frame,
            if damage {
                InstructionOutcome::Damage {
                    actual: Some(actual),
                }
            } else {
                InstructionOutcome::Life {
                    actual: Some(actual),
                }
            },
        )
    }
    // CR 120.2b/120.3a,e/903.10a: spell/ability damage changes life or marks
    // creature damage but never records Commander combat history. CR 119.3
    // life changes use their separate events; damage does not destroy (120.5).
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn apply_noncombat_damage(
        &mut self,
        damage: &NoncombatDamageOccurrence,
    ) -> Result<EventOutcome, String> {
        if damage.amount <= Quantity::zero()
            || damage.controller.0 >= 4
            || damage.source.controller.0 >= 4
        {
            return Err("invalid noncombat damage fact".into());
        }
        let source_card = self
            .state
            .cards
            .get(&damage.source.object.card)
            .ok_or("unknown damage source card")?;
        if source_card.owner != damage.source.owner {
            return Err("invalid noncombat source identity".into());
        }
        let bound = self
            .state
            .stack
            .iter()
            .chain(resolving_entry(&self.state.frame))
            .any(|entry| {
                entry.source == damage.source.object
                    && entry.controller == damage.controller
                    && entry.paid.is_some()
                    && self
                        .source_facts(entry)
                        .is_ok_and(|source| source == damage.source)
            });
        match self.object_before(damage.source.object) {
            Ok(current)
                if current == damage.source
                    && (damage.controller == damage.source.controller || bound) => {}
            Ok(_) => return Err("noncombat source facts not current".into()),
            Err(_) if bound => {}
            Err(_) => return Err("unbound last-known noncombat source".into()),
        }
        if !matches!(damage.source.zone, Zone::Battlefield | Zone::Stack) {
            return Err("invalid damage source functioning zone".into());
        }
        let (previous, current) = match damage.recipient {
            DamageRecipient::Player { player } => {
                if !self.is_live(player) {
                    return Err("departed damage recipient".into());
                }
                let life = &mut self.state.players[player.0 as usize].life;
                let previous = life.clone();
                life.subtract(&damage.amount);
                (previous, life.clone())
            }
            DamageRecipient::Creature { object } => {
                if !self.target_legal(object) {
                    return Err("invalid creature damage recipient".into());
                }
                let o = self
                    .state
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing damage object")?;
                let previous = o.marked_damage.clone();
                o.marked_damage.add(&damage.amount);
                (previous, o.marked_damage.clone())
            }
        };
        let life_gains = self.damage_keywords(&[ActualDamage {
            id: DamageId(0),
            source: damage.source.clone(),
            controller: damage.controller,
            recipient: damage.recipient.clone(),
            amount: damage.amount.clone(),
        }])?;
        Ok(EventOutcome::NoncombatDamageDealt {
            life_gains,
            damage: Box::new(damage.clone()),
            result: DamageResult {
                recipient: damage.recipient.clone(),
                previous,
                current,
            },
        })
    }
}
