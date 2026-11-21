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
fn update_entrant(w: &mut TriggerEventWitness, fact: &ObjectBefore) {
    if let TriggerEventWitness::CreatureEnteredWithPower {
        entrant,
        last_appearance: Some(last),
        ..
    } = w
        && entrant.object == fact.object
        && fact.zone == Zone::Battlefield
    {
        **last = fact.clone();
    }
}
fn update_entry(entry: &mut StackInstance, fact: &ObjectBefore) {
    if let StackKind::Triggered { witness, .. } = &mut entry.kind {
        update_entrant(witness, fact);
    }
    if entry.source == fact.object && entry.source_facts.is_some() {
        entry.source_facts = Some(fact.clone());
    }
}
fn update_source_appearances(entries: &mut [effects::PreparedDamage], fact: &ObjectBefore) {
    for entry in entries {
        if entry.source.object == fact.object {
            entry.source_appearance = Some(Box::new(fact.clone()));
        }
    }
}
impl Game {
    /// The typed control transition owns this refresh. A changed controller
    /// cannot be excused merely by recomputing or discarding assignment facts.
    pub(super) fn retain_damage_control_facts(&mut self, fact: &ObjectBefore) {
        self.state
            .rules
            .frame
            .for_each_held_mut(&mut |held| match held {
                frame::HeldMut::Damage(entries) => update_source_appearances(entries, fact),
                frame::HeldMut::Invocation { hold: _, id: _ } => {}
            });
    }
    pub(crate) fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String> {
        let o = self.current(object)?;
        Ok(ObjectBefore {
            battlefield_entry_turn: o.battlefield_entry_turn,
            keyword_timestamp: o.keyword_timestamp,
            keywords: { self.current_keywords(object) },
            deathtouch_damage: o.deathtouch_damage,
            object,
            owner: self
                .state
                .rules
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
    pub(crate) fn retain_source_lki(&mut self, object: ObjectId) -> Result<(), String> {
        // This exile incarnation has no printed or activated source characteristics.
        if self.current(object)?.hideaway.is_some() {
            return Ok(());
        }
        let fact = self.object_before(object)?;
        self.retain_source_facts(&fact);
        Ok(())
    }
    // CR 608.2h/113.7a: every pending trigger and every invocation's single
    // owner (its stack entry, or the detached entry a frame still resolves)
    // receives a departure's last-known facts. A frame holds no invocation.
    pub(crate) fn retain_source_facts(&mut self, fact: &ObjectBefore) {
        let object = fact.object;
        let rules = &mut self.state.rules;
        for trigger in &mut rules.pending_triggers {
            update_entrant(&mut trigger.witness, fact);
            if trigger.source == object {
                trigger.source_facts = fact.clone();
            }
        }
        for entry in rules.stack.iter_mut().chain(rules.detached.values_mut()) {
            update_entry(entry, fact);
        }
        // CR 608.2h/702.2e/702.15c: every prepared damage source the frame
        // holds retains this departure's actual last appearance. The
        // simultaneous event path then replaces appearances with its common
        // prior view. Assigned damage keeps its frozen facts.
        rules.frame.for_each_held_mut(&mut |held| match held {
            frame::HeldMut::Invocation { hold: _, id: _ } => {}
            frame::HeldMut::Damage(entries) => update_source_appearances(entries, fact),
        });
    }
    pub(crate) fn source_facts(&self, entry: &StackInstance) -> Result<ObjectBefore, String> {
        if self
            .current(entry.source)
            .is_ok_and(|o| o.phased_out.is_some())
        {
            return entry
                .source_facts
                .clone()
                .ok_or("missing phased-out source facts".into());
        }
        self.object_before(entry.source).or_else(|_| {
            entry
                .source_facts
                .clone()
                .ok_or("missing last-known damage source".into())
        })
    }
    pub(crate) fn eligible_players(
        &self,
        domain: TargetDomain,
        controller: PlayerId,
    ) -> Vec<PlayerId> {
        self.live_players()
            .into_iter()
            .filter(|p| self.targetable_player(*p))
            .filter(|p| {
                matches!(
                    domain,
                    TargetDomain::AnyPlayer | TargetDomain::PlayerOrPlaneswalker
                ) || *p != controller
            })
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
                    TargetDomain::AnyPlayer
                        | TargetDomain::OpponentPlayer
                        | TargetDomain::PlayerOrPlaneswalker
                ) && self
                    .eligible_players(spec.domain, entry.controller)
                    .contains(&player)
            })
        })
    }
    pub(crate) fn entity_legal(&self, target: TargetEntity) -> bool {
        match target {
            TargetEntity::Player { player } => self.targetable_player(player),
            TargetEntity::Object { object } => self.is_battlefield_creature(object),
        }
    }
    pub(super) fn eligible_entities(&self) -> Vec<TargetEntity> {
        self.live_players()
            .into_iter()
            .filter(|p| self.targetable_player(*p))
            .map(|player| TargetEntity::Player { player })
            .chain(
                self.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| {
                        self.top().is_ok_and(|entry| {
                            self.entity_target_legal(TargetEntity::Object { object: o.id }, entry)
                        })
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
        let program = self.program()?;
        if let Some(maximum) = mtg_contracts::graveyard_shuffle::offered_by(program) {
            return self
                .announced_graveyard_group(top.controller, player, maximum, entities)
                .map(|_| ());
        }
        if player != top.controller
            || entities.len() != program.targets.len()
            || entities.iter().enumerate().any(|(slot, target)| {
                program.targets[slot].domain != TargetDomain::AnyDamageTarget
                    || !self.entity_target_legal(*target, top)
            })
        {
            return Err("invalid damage targets".into());
        }
        Ok(())
    }
    pub(super) fn bind_entity_targets(&mut self, cast: Casting) -> Result<(), String> {
        let input = self
            .state
            .rules
            .pending
            .take()
            .ok_or("missing entity targets")?;
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
            .rules
            .stack
            .last_mut()
            .ok_or("missing targeted invocation")?;
        top.entity_targets.extend(entities);
        if top.announced_entity_targets.is_empty() {
            top.announced_entity_targets = announced;
        } else {
            for (old, new) in top.announced_entity_targets.iter_mut().zip(announced) {
                old.extend(new);
            }
        }
        if cast.origin.is_spell() {
            self.state.rules.frame = self.continue_spell_targeting(cast)?;
        } else {
            self.continue_activation_after_targets(cast, input.decision_id)?;
        }
        Ok(())
    }
    pub(super) fn continue_activation_after_targets(
        &mut self,
        cast: Casting,
        decision_id: u64,
    ) -> Result<(), String> {
        let entry = self.top()?.clone();
        let StackKind::Activated { ability, .. } = entry.kind else {
            return Err("targeted activation kind".into());
        };
        {
            match &self.copiable_definition(entry.source)?.activated[ability as usize].cost {
                ActivationCost::SacrificePermanent { mana: None, .. } => {
                    let mut cast = cast;
                    cast.spending = Some(BTreeMap::new());
                    self.state.rules.frame = Frame::SacrificeCost { cast };
                    return Ok(());
                }
                ActivationCost::SacrificePermanent { mana: Some(_), .. } => {
                    self.state.rules.frame = self.announcement_mana_frame(cast)?;
                    return Ok(());
                }
                ActivationCost::Composite { costs } if costs.mana.is_some() => {
                    self.state.rules.frame = self.announcement_mana_frame(cast)?;
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
            source.present_on_battlefield()
                && source.controller == entry.controller
                && !source.tapped
                && (source.attack_ready || self.has_keyword(source.id, KeywordAbility::Haste))
        });
        if !source_payable
            || self.copiable_definition(entry.source)?.activated[ability as usize].cost
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
            .rules
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
        self.state.rules.priority = entry.controller;
        self.state.rules.frame = Frame::Priority;
        self.stabilize()
    }
    pub(super) fn validate_damage_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        if let Some(fact) = &entry.source_facts {
            if entry.kind.is_spell()
                && mtg_contracts::bestow_alternative(self.definition(entry.source.card)?).is_some()
            {
                let bestowed = entry.alternative_cost == Some(cascade::AlternativeCost::Bestow);
                if fact.zone != Zone::Stack
                    || fact.characteristics
                        != self.proposed_cast_characteristics(entry.source.card, bestowed)?
                {
                    return Err("snapshot bestow spell characteristics invariant".into());
                }
            } else {
                self.validate_characteristic_identity(
                    entry.source,
                    fact.zone,
                    &fact.characteristics,
                )?;
            }
        }
        if entry.kind.is_spell() {
            if entry.effect_source_handles.len() != 4 {
                return Err("missing public spell source references".into());
            }
        } else if !entry.effect_source_handles.is_empty() {
            return Err("unexpected spell effect source references".into());
        }
        let program = self.program_for(entry)?;
        let mixed = self.mixed_mode_children(entry)?;
        let graveyard = mixed
            .as_ref()
            .and_then(|children| {
                children
                    .iter()
                    .find_map(|p| mtg_contracts::graveyard_shuffle::offered_by(p))
            })
            .or_else(|| mtg_contracts::graveyard_shuffle::offered_by(program));
        let damage = program
            .targets
            .iter()
            .any(|t| t.domain == TargetDomain::AnyDamageTarget);
        {
            let fact = entry
                .source_facts
                .as_ref()
                .ok_or("snapshot missing source facts")?;
            let card = self
                .state
                .rules
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
            if (!damage && graveyard.is_none())
                || (mixed.is_none() && !entry.targets.is_empty())
                || (mixed.is_none() && !entry.player_targets.is_empty())
                || (mixed.is_none() && !entry.stack_targets.is_empty())
                || if let Some(maximum) = graveyard {
                    !self.graveyard_group_structure(&entry.entity_targets, maximum)
                } else {
                    entry.entity_targets.len() > program.targets.len()
                }
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
                    TargetEntity::Object { object } => {
                        !self.state.rules.cards.contains_key(&object.card)
                    }
                })
            {
                return Err("snapshot entity binding invariant".into());
            }
        } else if !entry.announced_entity_targets.is_empty() {
            return Err("snapshot entity announcement invariant".into());
        }
        if mixed.is_none()
            && (damage || graveyard.is_some())
            && (!entry.targets.is_empty()
                || (mixed.is_none() && !entry.player_targets.is_empty())
                || (mixed.is_none() && !entry.stack_targets.is_empty()))
        {
            return Err("snapshot wrong damage binding domain".into());
        }
        Ok(())
    }
    // Use the same announced target and amount when preparing or restoring damage.
    pub(super) fn directed_damage_operands<'a>(
        &self,
        spell: &'a StackInstance,
        instruction: &'a Instruction,
    ) -> Result<(u32, &'a Quantity, u32, TargetEntity), String> {
        let (slot, amount, block) = match instruction {
            Instruction::DealDamage {
                target_slot,
                amount,
                source_block,
            } => (*target_slot, amount, *source_block),
            Instruction::DealDamageAnnouncedX {
                target_slot,
                source_block,
            } => (
                *target_slot,
                spell.x.as_ref().ok_or("missing announced X")?,
                *source_block,
            ),
            _ => return Err("expected directed damage instruction".into()),
        };
        let index = slot as usize;
        let spec = self.program_for(spell)?.targets.get(index);
        let target = match spec.map(|spec| spec.domain) {
            Some(TargetDomain::AnyDamageTarget) => *self
                .bound_damage_target(spell, index)
                .ok_or("missing damage binding")?,
            Some(TargetDomain::PlayerOrPlaneswalker) => {
                let player = *spell
                    .player_targets
                    .get(index)
                    .ok_or("missing player-or-planeswalker damage target")?;
                if player.0 >= 4 {
                    return Err("unknown target player".into());
                }
                TargetEntity::Player { player }
            }
            Some(TargetDomain::OpponentPlayer) => {
                let player = *spell
                    .player_targets
                    .get(index)
                    .ok_or("missing opponent damage binding")?;
                if player == spell.controller || player.0 >= 4 {
                    return Err("invalid announced opponent damage binding".into());
                }
                TargetEntity::Player { player }
            }
            Some(TargetDomain::BattlefieldCreature) => TargetEntity::Object {
                object: *spell
                    .targets
                    .get(index)
                    .ok_or("missing creature damage binding")?,
            },
            Some(TargetDomain::BattlefieldPermanent)
                if matches!(
                    spell.kind,
                    StackKind::Triggered {
                        witness: TriggerEventWitness::LoreAdded { .. },
                        ..
                    }
                ) && spec.is_some_and(mtg_contracts::saga_damage_target) =>
            {
                TargetEntity::Object {
                    object: *spell
                        .targets
                        .get(index)
                        .ok_or("missing chapter damage binding")?,
                }
            }
            _ => return Err("unsupported directed damage target domain".into()),
        };
        Ok((slot, amount, block, target))
    }

    fn commit_opponent_life(
        &mut self,
        frame: &Resolving,
        amount: &Quantity,
        source_block: u32,
        gain: bool,
    ) -> Result<Option<CommittedBatch>, String> {
        let occurrences = self
            .opponents(self.entry(frame.stack_id)?.controller)
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
        if occurrences.is_empty() {
            return Ok(None);
        }
        self.commit_batch(
            self.instruction_cause(frame, source_block)?,
            EventGroup::Simultaneous { occurrences },
        )
        .map(Some)
    }

    // CR 119.3/119.9, 608.2c/h, 704.4. Both commits belong to one staged
    // resolution step, so failure of the gain also rolls back the losses.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn drain_opponents(
        &mut self,
        frame: Resolving,
        amount: OpponentDrainAmount,
        source_block: u32,
    ) -> Result<(), String> {
        let amount = match amount {
            OpponentDrainAmount::Fixed { value } => value,
            OpponentDrainAmount::AnnouncedX => self
                .entry(frame.stack_id)?
                .x
                .clone()
                .ok_or("missing announced X")?,
        };
        if amount == Quantity::zero() {
            return self.finish_instruction(frame, InstructionOutcome::Life { actual: None });
        }
        let Some(losses) = self.commit_opponent_life(&frame, &amount, source_block, false)? else {
            return self.finish_instruction(frame, InstructionOutcome::Life { actual: None });
        };
        let mut total = Quantity::zero();
        for occurrence in &losses.occurrences {
            if matches!(occurrence.actual, EventOutcome::LifeChangeProhibited { .. }) {
                continue;
            }
            let EventOutcome::LifeLost { amount, .. } = &occurrence.actual else {
                return Err("missing opponent life loss outcome".into());
            };
            total.add(amount);
        }
        let player = self.entry(frame.stack_id)?.controller;
        let actual = if total > Quantity::zero() && self.is_live(player) {
            Some(self.commit(
                ProposedEvent::GainLife {
                    player,
                    amount: total,
                },
                losses.cause,
            )?)
        } else {
            None
        };
        self.finish_instruction(frame, InstructionOutcome::Life { actual })
    }

    // CR 608.2b-c/704.4: execute the ordered program without an intervening
    // SBA check. A self-targeted Soul Feast may reach zero then recover before
    // the next priority checkpoint; an all-target-illegal spell never starts.
    pub(crate) fn execute_damage_life_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        // CR 119.9: the complete permanent-grant family permits a fixed zero
        // tail, which advances without a life event or trigger. Keep ordinary
        // GainLife/LoseLife's strictly positive event boundary unchanged.
        if let Instruction::GainLife {
            recipient: LifeRecipient::Controller,
            amount,
            source_block: 0,
        } = &instruction
            && *amount == Quantity::zero()
            && frame.cursor == 1
            && self.entry(frame.stack_id)?.kind.is_spell()
            && self
                .program_for(self.entry(frame.stack_id)?)
                .is_ok_and(|program| {
                    temporary_permanent_keyword_program(program)
                        .is_some_and(|descriptor| descriptor.controller_life == Some(amount))
                })
        {
            return self.finish_instruction(frame, InstructionOutcome::Life { actual: None });
        }
        let gain = matches!(instruction, Instruction::GainLife { .. });
        let (block, proposed, damage) = match instruction {
            Instruction::DealDamage { .. } | Instruction::DealDamageAnnouncedX { .. } => {
                let (target_slot, amount, source_block, target) =
                    self.directed_damage_operands(self.entry(frame.stack_id)?, &instruction)?;
                if *amount == Quantity::zero()
                    || frame.legal_targets.get(target_slot as usize) != Some(&true)
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
                            source: self.source_facts(self.entry(frame.stack_id)?)?,
                            controller: self.entry(frame.stack_id)?.controller,
                            recipient,
                            amount: amount.clone(),
                        }),
                    },
                    true,
                )
            }
            Instruction::DealDamageToManaActivator {
                amount,
                source_block,
            } => {
                let player = Self::mana_tap_activator(self.entry(frame.stack_id)?)?;
                if !self.is_live(player) {
                    return self
                        .finish_instruction(frame, InstructionOutcome::Damage { actual: None });
                }
                let damage = NoncombatDamageOccurrence {
                    source: self.source_facts(self.entry(frame.stack_id)?)?,
                    controller: self.entry(frame.stack_id)?.controller,
                    recipient: DamageRecipient::Player { player },
                    amount,
                };
                return self.prepare_noncombat_damage(frame, damage, source_block);
            }
            Instruction::DealDamageToController {
                amount,
                source_block,
            } => {
                // CR 109.5/113.7a: use the trigger's frozen controller and
                // the land's current or last-known source facts.
                let controller = self.entry(frame.stack_id)?.controller;
                let damage = NoncombatDamageOccurrence {
                    source: self.source_facts(self.entry(frame.stack_id)?)?,
                    controller,
                    recipient: DamageRecipient::Player { player: controller },
                    amount,
                };
                return self.prepare_noncombat_damage(frame, damage, source_block);
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
                        let _ = self.commit_opponent_life(&frame, &amount, source_block, gain)?;
                        return self
                            .finish_instruction(frame, InstructionOutcome::Life { actual: None });
                    }
                    LifeRecipient::EventPlayer => {
                        let Some(actor) = self.entry(frame.stack_id)?.kind.event_player()? else {
                            return self.finish_instruction(
                                frame,
                                InstructionOutcome::Life { actual: None },
                            );
                        };
                        actor
                    }
                    LifeRecipient::Controller => self.entry(frame.stack_id)?.controller,
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
                        *self
                            .entry(frame.stack_id)?
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
                stack_id: frame.stack_id,
                player: self.entry(frame.stack_id)?.controller,
                source: self.entry(frame.stack_id)?.source,
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
    pub(crate) fn apply_noncombat_damage(
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
            .rules
            .cards
            .get(&damage.source.object.card)
            .ok_or("unknown damage source card")?;
        if source_card.owner != damage.source.owner {
            return Err("invalid noncombat source identity".into());
        }
        let bound = self
            .state
            .rules
            .stack
            .iter()
            .chain(
                self.state
                    .rules
                    .frame
                    .resolving_invocation()
                    .and_then(|id| self.state.rules.detached.get(&id)),
            )
            .any(|entry| {
                entry.source == damage.source.object
                    && entry.controller == damage.controller
                    && self.stack_entry_complete(entry)
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
                if self.player_protected(player) {
                    return Err("protection requires the damage transaction".into());
                }
                self.state.rules.players[player.0 as usize].lose_life(&damage.amount)
            }
            DamageRecipient::Creature { object } => {
                if !self.is_battlefield_creature(object) {
                    return Err("invalid creature damage recipient".into());
                }
                if self.protected_from(object, &damage.source.characteristics) {
                    return Err("protection requires the damage transaction".into());
                }
                let o = self
                    .state
                    .rules
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing damage object")?;
                let previous = o.marked_damage.clone();
                o.marked_damage.add(&damage.amount);
                (previous, o.marked_damage.clone())
            }
        };
        let life_gains = self.damage_keywords(
            &[ActualDamage {
                id: DamageId(0),
                source: damage.source.clone(),
                controller: damage.controller,
                recipient: damage.recipient.clone(),
                amount: damage.amount.clone(),
            }],
            None,
        )?;
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
