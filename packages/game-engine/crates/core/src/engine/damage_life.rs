use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "entity", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetEntity {
    Player { player: PlayerId },
    Object { object: ObjectId },
}
fn update_entry(entry: &mut StackInstance, fact: &ObjectBefore) {
    for target in &mut entry.target_triggers {
        target.refresh_source(fact);
    }
    if let StackKind::Triggered {
        witness, binding, ..
    } = &mut entry.kind
    {
        binding.refresh(witness, fact);
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
        || matches!(&entry.kind, StackKind::Triggered { witness, binding, .. }
            if binding.reads_departure(witness, fact)
                || witness.observed_object().is_some_and(|binding| binding.last_known.object == fact.object))
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
                frame::HeldMut::Invocation(_) => {}
            });
    }
    pub(crate) fn object_before(&self, object: ObjectId) -> Result<ObjectBefore, String> {
        let o = self.current(object)?;
        Ok(ObjectBefore {
            transforms: o.transforms,
            face_down: o.face_down,
            battlefield_entry_turn: o.battlefield_entry_turn,
            battlefield_entry_timestamp: o.battlefield_entry_timestamp,
            control_upkeeps: o.control_upkeeps,
            unearthed: o.unearthed,
            dashed: o.dashed,
            keyword_timestamp: o.keyword_timestamp,
            keyword_rank: o.keyword_rank,
            keywords: self.current_keywords(object)?,
            deathtouch_damage: o.deathtouch_damage,
            plus_one_counters: o.plus_one_counters.clone(),
            counters: o.counters.clone(),
            loyalty_counters: o.loyalty_counters.clone(),
            loyalty_used_turn: o.loyalty_used_turn,
            attached_to: o.attached_to,
            attachment_timestamp: o.attachment_timestamp,
            phased_out: o.phased_out,
            hideaway: o.hideaway.clone(),
            adventure_player: o.adventure_player,
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
            trigger.binding.refresh(&trigger.witness, fact);
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
                frame::HeldMut::Invocation(_) => {}
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
    // CR 120.3c,e: planeswalkers lose loyalty through the counter facet;
    // marked damage keeps its damage receipt.
    pub(super) fn apply_permanent_damage(
        &mut self,
        object: ObjectId,
        amount: &Quantity,
    ) -> Result<Option<DamageResult>, String> {
        let planeswalker = self.is_battlefield_planeswalker(object);
        let permanent = self
            .rules_mut()
            .objects
            .get_mut(&object.card)
            .ok_or("missing damage object")?;
        if planeswalker {
            permanent.remove_counters(&mtg_contracts::kir::CounterKind::Loyalty, amount);
            return Ok(None);
        }
        let previous = permanent.marked_damage.clone();
        permanent.marked_damage.add(amount);
        Ok(Some(DamageResult {
            recipient: DamageRecipient::Permanent { object },
            previous,
            current: permanent.marked_damage.clone(),
        }))
    }

    pub(crate) fn entity_legal(&self, target: TargetEntity) -> bool {
        match target {
            TargetEntity::Player { player } => self.targetable_player(player),
            TargetEntity::Object { object } => self.is_permanent_damage_recipient(object),
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
                !matches!(
                    program.targets[slot].domain,
                    TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker
                ) || !self.entity_target_legal(*target, top)
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
            match self.stack_definition(&entry)?.activated[ability as usize]
                .cost
                .clone()
            {
                ActivationCost::Loyalty { change } => {
                    self.pay_loyalty(
                        BatchCause::AbilityActivation {
                            stack_id: entry.id,
                            player: entry.controller,
                            source: entry.source,
                            decision_id,
                            ability,
                        },
                        &change,
                    )?;
                    self.rules_mut()
                        .stack
                        .last_mut()
                        .ok_or("missing activated invocation")?
                        .paid = Some(entry.total_cost);
                    self.record_ability_activated(
                        entry.id,
                        entry.source,
                        entry.controller,
                        ability,
                    )?;
                    self.rules_mut().priority = entry.controller;
                    self.rules_mut().frame = Frame::Priority;
                    return Ok(self.stabilize()?);
                }
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
                // CR 601.2g: announcements that leave no mana to pay open
                // no mana window.
                ActivationCost::Mana { .. } | ActivationCost::Composite { .. }
                    if self.announced_without_mana(&entry)? =>
                {
                    let mut cast = cast;
                    cast.spending = Some(BTreeMap::new());
                    return self.payment_step(cast);
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
        let source_payable = if let Ok(source) = self.current(entry.source) {
            source.present_on_battlefield()
                && source.controller == entry.controller
                && !source.tapped
                && (source.attack_ready || self.has_keyword(source.id, KeywordAbility::Haste)?)
        } else {
            false
        };
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
            if fact.object != entry.source {
                return Err("stack source fact identity mismatch".into());
            }
            self.validate_fact_identity(fact)?;
        }
        if entry.kind.is_spell() {
            if entry.effect_source_handles.len() != 4 {
                return Err("missing public spell source references".into());
            }
        } else if !entry.effect_source_handles.is_empty() {
            return Err("unexpected spell effect source references".into());
        }
        let program = self.program_for(entry)?;
        let damage = program.targets.iter().any(|t| {
            matches!(
                t.domain,
                TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker
            )
        });
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
    pub(super) fn directed_damage_target(
        &self,
        spell: &StackInstance,
        slot: u32,
    ) -> Result<TargetEntity, String> {
        let index = slot as usize;
        let spec = self.program_for(spell)?.targets.get(index);
        let target = match spec.map(|spec| spec.domain) {
            Some(TargetDomain::AnyDamageTarget | TargetDomain::PlayerOrPlaneswalker) => *self
                .bound_damage_target(spell, index)
                .ok_or("missing damage binding")?,
            Some(domain @ (TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer)) => {
                let player = *spell
                    .player_targets
                    .get(index)
                    .ok_or("missing opponent damage binding")?;
                if (domain == TargetDomain::OpponentPlayer && player == spell.controller)
                    || player.0 >= 4
                {
                    return Err("invalid announced opponent damage binding".into());
                }
                TargetEntity::Player { player }
            }
            Some(
                TargetDomain::BattlefieldCreature
                | TargetDomain::ControlledBattlefieldCreature
                | TargetDomain::BlockingBattlefieldCreature
                | TargetDomain::CreatureOrPlaneswalker
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
}
