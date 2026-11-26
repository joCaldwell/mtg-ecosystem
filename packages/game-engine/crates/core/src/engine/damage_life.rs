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
    for target in &mut entry.target_triggers {
        target.refresh_source(fact);
    }
    if let StackKind::Triggered { witness, .. } = &mut entry.kind {
        witness.refresh_observed_object(fact);
    }
    if entry.source == fact.object && entry.source_facts.is_some() {
        entry.source_facts = Some(fact.clone());
    }
}
/// Whether `update_entry` would write `entry` for this departure.
fn reads_last_known(entry: &StackInstance, fact: &ObjectBefore) -> bool {
    entry
        .target_triggers
        .iter()
        .any(|target| target.source() == fact.object)
        || (entry.source == fact.object && entry.source_facts.is_some())
        || matches!(&entry.kind, StackKind::Triggered { witness, .. }
            if witness.observed_object().is_some_and(|binding| binding.last_known.object == fact.object))
}
fn update_source_appearances(entries: &mut [effects::PreparedDamage], fact: &ObjectBefore) {
    for entry in entries {
        if entry.source.object == fact.object {
            entry.source_appearance = Some(Box::new(fact.clone()));
        }
    }
}
impl RulesTxn {
    /// The typed control transition owns this refresh. A changed controller
    /// cannot be excused merely by recomputing or discarding assignment facts.
    pub(super) fn retain_damage_control_facts(&mut self, fact: &ObjectBefore) {
        let rules = self.rules_mut();
        rules
            .frame
            .for_each_held_mut(&mut rules.frames, &mut |held| match held {
                frame::HeldMut::Damage(entries) => update_source_appearances(entries, fact),
                frame::HeldMut::Invocation { hold: _, id: _ } => {}
            });
    }
    pub(crate) fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String> {
        let o = self.current(object)?;
        Ok(ObjectBefore {
            transforms: o.transforms,
            battlefield_entry_turn: o.battlefield_entry_turn,
            control_upkeeps: o.control_upkeeps,
            unearthed: o.unearthed,
            dashed: o.dashed,
            keyword_timestamp: o.keyword_timestamp,
            keyword_rank: o.keyword_rank,
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
        let rules = self.rules_mut();
        if !rules.resolution_triggers.is_empty() {
            for row in rules.resolution_triggers.values_mut() {
                row.refresh_source(fact);
            }
        }
        for trigger in &mut rules.pending_triggers {
            trigger.witness.refresh_observed_object(fact);
            if trigger.source == object {
                trigger.source_facts = fact.clone();
            }
        }
        // The stack is shared with the staged copy's origin: write it only
        // when an entry holds facts of this object.
        if rules
            .stack
            .iter()
            .any(|entry| reads_last_known(entry, fact))
        {
            for entry in rules.stack.iter_mut() {
                update_entry(entry, fact);
            }
        }
        for entry in rules.detached.values_mut() {
            update_entry(entry, fact);
        }
        // CR 608.2h/702.2e/702.15c: every prepared damage source the frame
        // holds retains this departure's actual last appearance. The
        // simultaneous event path then replaces appearances with its common
        // prior view. Assigned damage keeps its frozen facts.
        rules
            .frame
            .for_each_held_mut(&mut rules.frames, &mut |held| match held {
                frame::HeldMut::Invocation { hold: _, id: _ } => {}
                frame::HeldMut::Damage(entries) => update_source_appearances(entries, fact),
            });
    }
    /// CR 608.2h/702.26b: a phased-out source is unavailable even though
    /// its incarnation still has a battlefield row. Read LKI at the caller.
    pub(super) fn current_source_facts(&self, source: ObjectId) -> Result<ObjectBefore, String> {
        if self.current(source)?.phased_out.is_some() {
            return Err("phased-out source has no current facts".into());
        }
        self.object_before(source)
    }

    pub(crate) fn source_facts(&self, entry: &StackInstance) -> Result<ObjectBefore, String> {
        self.current_source_facts(entry.source).or_else(|_| {
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
            .rules_mut()
            .pending
            .take()
            .ok_or("missing entity targets")?;
        let entities = input.entities()?;
        self.validate_entity_targets(input.player, &entities)?;
        let announced = (0..4)
            .map(|viewer| {
                entities
                    .iter()
                    .map(|target| match target {
                        TargetEntity::Player { .. } => Ok(None),
                        TargetEntity::Object { object } => self
                            .object_handle_id(PlayerId(viewer), *object)
                            .cloned()
                            .map(Some),
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let top = self
            .rules_mut()
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
        self.stage_target_triggers()?;
        if cast.origin.is_spell() {
            self.rules_mut().frame = self.announcement_mana_frame(cast)?;
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
            match &self.stack_definition(&entry)?.activated[ability as usize].cost {
                ActivationCost::ChooseObjects(cost) => {
                    let mut cast = cast;
                    self.rules_mut().frame = if cost.mana.is_some() {
                        self.announcement_mana_frame(cast)?
                    } else {
                        cast.spending = Some(BTreeMap::new());
                        Self::chosen_cost_frame(cast)
                    };
                    return Ok(());
                }
                ActivationCost::SacrificePermanent { mana: None, .. } => {
                    let mut cast = cast;
                    cast.spending = Some(BTreeMap::new());
                    self.rules_mut().frame = Frame::SacrificeCost { cast };
                    return Ok(());
                }
                // CR 601.2g, 602.2b: only a cost with a mana component opens
                // a mana window after targets.
                ActivationCost::SacrificePermanent { mana: Some(_), .. }
                | ActivationCost::Mana { .. }
                | ActivationCost::DiscardSource { .. }
                | ActivationCost::ExileSource { .. } => {
                    self.rules_mut().frame = self.announcement_mana_frame(cast)?;
                    return Ok(());
                }
                ActivationCost::Composite { costs } if costs.mana.is_some() => {
                    self.rules_mut().frame = self.announcement_mana_frame(cast)?;
                    return Ok(());
                }
                // Life, counters, tap and source sacrifice with no mana are
                // paid at once, as one complete cost (CR 118.3, 602.2b).
                ActivationCost::Composite { .. } | ActivationCost::SacrificeSource => {
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
            || self.stack_definition(&entry)?.activated[ability as usize].cost
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
        self.rules_mut()
            .stack
            .last_mut()
            .ok_or("missing activated invocation")?
            .paid = Some(entry.total_cost);
        self.record_ability_activated(entry.id, entry.source, entry.controller, ability)?;
        self.rules_mut().priority = entry.controller;
        self.rules_mut().frame = Frame::Priority;
        Ok(self.stabilize()?)
    }
    pub(super) fn validate_damage_bindings(&self, entry: &StackInstance) -> Result<(), String> {
        if let Some(fact) = &entry.source_facts {
            self.validate_characteristic_identity(
                entry.source,
                fact.transforms,
                &fact.characteristics,
            )?;
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
        if damage
            && (!entry.targets.is_empty()
                || !entry.player_targets.is_empty()
                || !entry.stack_targets.is_empty())
        {
            return Err("snapshot wrong damage binding domain".into());
        }
        Ok(())
    }
    pub(super) fn damage_source(
        &self,
        entry: &StackInstance,
        source: mtg_contracts::kir::Ref,
    ) -> Result<(ObjectBefore, PlayerId), String> {
        use mtg_contracts::kir::Ref;
        match (source, &entry.kind) {
            (Ref::Source, _) => Ok((self.source_facts(entry)?, entry.controller)),
            (Ref::EventObject { .. }, StackKind::Triggered { witness, .. }) => {
                let binding = witness
                    .observed_object()
                    .ok_or("missing event object binding")?;
                let facts = self
                    .current_source_facts(binding.last_known.object)
                    .unwrap_or_else(|_| (*binding.last_known).clone());
                let controller = facts.controller;
                Ok((facts, controller))
            }
            _ => Err("unbound damage source".into()),
        }
    }

    // CR 608.2h, 107.1b, 107.2, 208.3: use the creature's power at resolution
    // or departure. A noncreature permanent's printed power is unavailable.
    pub(super) fn directed_damage_amount(
        spell: &StackInstance,
        amount: &mtg_contracts::kir::Amount,
        source: &ObjectBefore,
    ) -> Result<Quantity, String> {
        use mtg_contracts::kir::{Amount, Ref, Stat};
        match amount {
            Amount::Fixed { value } => Ok(value.clone()),
            Amount::Var {
                of: Ref::AnnouncedX,
            } => spell.x.clone().ok_or("missing announced X".into()),
            Amount::StatOf {
                object: Ref::EventObject { .. },
                stat: Stat::Power,
            } => Ok(source
                .characteristics
                .power
                .as_ref()
                .filter(|_| source.characteristics.has_type(CardKind::Creature))
                .cloned()
                .unwrap_or_default()
                .max(Quantity::zero())),
            _ => Err("unbound directed damage amount".into()),
        }
    }

    // Use the same announced target when preparing or restoring damage.
    pub(super) fn directed_damage_target(
        &self,
        spell: &StackInstance,
        slot: u32,
    ) -> Result<TargetEntity, String> {
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
            Some(
                TargetDomain::BattlefieldCreature
                | TargetDomain::BattlefieldPermanent
                | TargetDomain::ControlledBattlefieldPermanent,
            ) => TargetEntity::Object {
                object: *spell
                    .targets
                    .get(index)
                    .ok_or("missing creature damage binding")?,
            },

            _ => return Err("unsupported directed damage target domain".into()),
        };
        Ok(target)
    }

    pub(super) fn commit_opponent_life(
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
        amount: Quantity,
        source_block: u32,
    ) -> Result<(), String> {
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
        use mtg_contracts::kir::{DamageTo, Ref};
        let kernel!(Damage { source, to: DamageTo::Ref { who }, amount } @ source_block) =
            instruction
        else {
            return Err("unexpected damage instruction".into());
        };
        let entry = self.entry(frame.stack_id)?;
        let (source, controller) = self.damage_source(entry, source)?;
        let amount = Self::directed_damage_amount(entry, &amount, &source)?;
        let target = match who {
            Ref::Target { slot } if frame.legal_targets.get(slot as usize) == Some(&true) => {
                let target = self.directed_damage_target(entry, slot)?;
                self.entity_legal(target).then_some(target)
            }
            Ref::Target { .. } => None,
            Ref::Controller { .. } => Some(TargetEntity::Player {
                player: entry.controller,
            }),
            _ => return Err("unexpected directed damage recipient".into()),
        };
        let Some(target) = target.filter(|_| amount > Quantity::zero()) else {
            return self.finish_instruction(frame, InstructionOutcome::Damage { actual: None });
        };
        let recipient = match target {
            TargetEntity::Player { player } => DamageRecipient::Player { player },
            TargetEntity::Object { object } => DamageRecipient::Creature { object },
        };
        self.prepare_noncombat_damage(
            frame,
            NoncombatDamageOccurrence {
                source,
                controller,
                recipient,
                amount,
            },
            source_block,
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
                    .resolving_invocation(&self.state.rules.frames)
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
        match self.current_source_facts(damage.source.object) {
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
                self.rules_mut().players[player.0 as usize].lose_life(&damage.amount)
            }
            DamageRecipient::Creature { object } => {
                if !self.is_battlefield_creature(object) {
                    return Err("invalid creature damage recipient".into());
                }
                if self.protected_from(object, &damage.source.characteristics) {
                    return Err("protection requires the damage transaction".into());
                }
                let o = self
                    .rules_mut()
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
