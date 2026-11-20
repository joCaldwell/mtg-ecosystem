//! Finite attachments; pinned CR 301.5, 303.4, 701.3, 702.6/11/18,
//! 611.3a, 613.1f/4c, 704.5m/n and 603.10a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
// One equip can begin an otherwise empty-stack episode. Four Rancors can
// each enter the graveyard once per episode; recasting requires an empty stack.

impl Game {
    /// Only the inner permanent-completion movement changes; the caller still
    /// owns stack removal, completion history, priority and stabilization.
    pub(super) fn finish_spell_aura_entry(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        target: ObjectId,
        stack_id: StackId,
    ) -> Result<(), String> {
        if self.current(source)?.controller != controller {
            return Err("Aura completion controller mismatch".into());
        }
        self.commit(
            ProposedEvent::EnterAttached {
                object: source,
                target,
            },
            BatchCause::SpellCompletion {
                stack_id,
                player: controller,
                source,
            },
        )?;
        Ok(())
    }

    pub(super) fn next_attachment_allocation(&self) -> Result<(u64, u64), String> {
        let timestamp = self.state.rules.next_attachment_timestamp;
        if timestamp == 0 {
            return Err("invalid attachment timestamp counter".into());
        }
        let next = timestamp
            .checked_add(1)
            .ok_or("attachment timestamp resource limit")?;
        Ok((timestamp, next))
    }

    /// CR 301.5/303.4/701.3: type/domain validity only. Exact incarnation,
    /// battlefield presence and spell targetability belong to the caller. In
    /// particular, gaining Hexproof/Shroud never ends an existing attachment.
    pub(super) fn attachment_target_type_valid(
        &self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<bool, String> {
        let definition = self.copiable_definition(source)?;
        let target_characteristics = &self.copiable_definition(target)?.characteristics;
        if definition
            .static_abilities
            .iter()
            .any(|ability| ability.effect == StaticEffect::ControlEnchantedPermanent)
        {
            // Validate the whole narrow source: a malformed control Aura must
            // not fall back to the legacy creature-only attachment family.
            Ok(mtg_contracts::aura_control_domain(definition)
                .is_some_and(|domain| domain.accepts(target_characteristics)))
        } else if definition
            .characteristics
            .subtypes
            .iter()
            .any(|s| s == "Equipment" || s == "Aura")
        {
            Ok(target_characteristics.has_type(CardKind::Creature))
        } else {
            Ok(false)
        }
    }

    pub(super) fn validate_attachment_state(&self) -> Result<(), String> {
        let next = self.state.rules.next_attachment_timestamp;
        if next == 0 {
            return Err("snapshot attachment counter invariant".into());
        }
        let mut timestamps = BTreeSet::new();
        for o in self.state.rules.objects.values() {
            if o.attached_to.is_some() != o.attachment_timestamp.is_some()
                || o.attachment_timestamp
                    .is_some_and(|t| t == 0 || t >= next || !timestamps.insert(t))
            {
                return Err("snapshot attachment timestamp invariant".into());
            }
            let Some(target) = o.attached_to else {
                continue;
            };
            // A departed/moved target can remain recorded until CR 704.5m/n.
            // Never follow its successor or accept an invented future target.
            let known_target = self.state.rules.objects.get(&target.card).map_or_else(
                || {
                    self.state
                        .rules
                        .cards
                        .get(&target.card)
                        .is_some_and(|c| !self.is_live(c.owner))
                },
                |current| {
                    current.id.incarnation > target.incarnation
                        || (current.id == target && current.zone == Zone::Battlefield)
                },
            );
            if o.zone != Zone::Battlefield
                || !self.is_live(o.controller)
                || o.id == target
                || !self
                    .copiable_definition(o.id)?
                    .characteristics
                    .subtypes
                    .iter()
                    .any(|s| s == "Equipment" || s == "Aura")
                || !known_target
                || !self.attachment_target_type_valid(o.id, target)?
            {
                return Err("snapshot attachment invariant".into());
            }
        }
        Ok(())
    }

    pub(super) fn target_is_creature(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.present_on_battlefield())
            && self
                .copiable_definition(object)
                .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
    }
    pub(super) fn static_modifications(&self, target: ObjectId) -> Vec<&StaticEffect> {
        self.timestamped_static_modifications(target)
            .into_iter()
            .map(|(effect, _)| effect)
            .collect()
    }
    fn timestamped_static_modifications(
        &self,
        target: ObjectId,
    ) -> Vec<(&StaticEffect, Option<u64>)> {
        let Ok(permanent) = self.current(target) else {
            return vec![];
        };
        if !permanent.present_on_battlefield() {
            return vec![];
        }
        let creature = self.target_is_creature(target);
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
            .flat_map(|o| {
                self.copiable_definition(o.id)
                    .into_iter()
                    .flat_map(move |d| {
                        d.static_abilities
                            .iter()
                            .filter_map(move |a| match &a.effect {
                                StaticEffect::GrantOtherControlledPermanentsIndestructible
                                    if a.functioning_zone == FunctioningZone::Battlefield
                                        && o.id != target
                                        && o.controller == permanent.controller =>
                                {
                                    Some((&a.effect, o.keyword_timestamp))
                                }
                                StaticEffect::ModifyCreatures { scope, .. }
                                    if creature
                                        && match scope {
                                            CreatureScope::Source => o.id == target,
                                            CreatureScope::Controller => {
                                                o.controller == permanent.controller
                                            }
                                            CreatureScope::AttackingController => {
                                                // CR 508.1k/506.4/611.3a: attacking is
                                                // incarnation-bound combat membership,
                                                // independent of tapping or readiness.
                                                o.controller == permanent.controller
                                                    && self
                                                        .state
                                                        .rules
                                                        .combat
                                                        .iter()
                                                        .any(|attacker| attacker.attacker == target)
                                            }
                                            CreatureScope::Equipped | CreatureScope::Enchanted => {
                                                o.attached_to == Some(target)
                                            }
                                        } =>
                                {
                                    Some((&a.effect, o.keyword_timestamp))
                                }
                                _ => None,
                            })
                    })
            })
            .collect()
    }
    pub(super) fn current_keywords(&self, object: ObjectId) -> Vec<KeywordAbility> {
        if self.current(object).is_ok_and(|o| o.hideaway.is_some()) {
            return vec![];
        }
        let mut keywords = self
            .copiable_definition(object)
            .map(|d| d.keywords.clone())
            .unwrap_or_default();
        let present = self
            .current(object)
            .is_ok_and(|o| o.zone == Zone::Battlefield);
        let latest_removal = present
            .then(|| {
                self.state
                    .rules
                    .effects
                    .iter()
                    .filter_map(|effect| match &effect.kind {
                        ResolvedEffectKind::RemoveIndestructibleSet { targets, timestamp }
                            if targets.contains(&object) =>
                        {
                            Some((effect.id, *timestamp))
                        }
                        _ => None,
                    })
                    .max()
            })
            .flatten();
        if latest_removal.is_some() {
            keywords.retain(|keyword| *keyword != KeywordAbility::Indestructible);
        }
        for (effect, timestamp) in self.timestamped_static_modifications(object) {
            let grants: &[KeywordAbility] = match effect {
                StaticEffect::ModifyCreatures {
                    keywords: grants, ..
                } => grants,
                StaticEffect::GrantOtherControlledPermanentsIndestructible => {
                    &[KeywordAbility::Indestructible]
                }
                _ => continue,
            };
            for keyword in grants {
                if *keyword == KeywordAbility::Indestructible
                    && latest_removal
                        .is_some_and(|(_, removed_at)| timestamp.unwrap_or(0) <= removed_at)
                {
                    continue;
                }
                if !keywords.contains(keyword) {
                    keywords.push(*keyword);
                }
            }
        }
        // CR 611.2c/613.1f/400.7: union only for the captured incarnation;
        // source departure does not end an already resolved EOT grant.
        for effect in &self.state.rules.effects {
            let grants = match &effect.kind {
                ResolvedEffectKind::GrantKeywords {
                    target, keywords, ..
                } if *target == object && self.scalar_keyword_effect_applies(effect, object) => {
                    Some(keywords)
                }
                ResolvedEffectKind::GrantKeywordsSet {
                    targets, keywords, ..
                } if targets.contains(&object)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.present_on_battlefield()) =>
                {
                    Some(keywords)
                }
                _ => None,
            };
            if let Some(grants) = grants {
                for keyword in grants {
                    if *keyword == KeywordAbility::Indestructible
                        && latest_removal.is_some_and(|(removed_id, _)| effect.id <= removed_id)
                    {
                        continue;
                    }
                    if !keywords.contains(keyword) {
                        keywords.push(*keyword);
                    }
                }
            }
        }
        keywords
    }
    pub(super) fn has_restriction(
        &self,
        object: ObjectId,
        restriction: CreatureRestriction,
    ) -> bool {
        self.static_modifications(object).iter().any(|a| matches!(a, StaticEffect::ModifyCreatures { restrictions, .. } if restrictions.contains(&restriction)))
    }
    pub(super) fn targetable_by(&self, object: ObjectId, controller: PlayerId) -> bool {
        // CR 702.11b/18a apply only while the ability functions on battlefield.
        if self.current(object).is_ok_and(|o| o.phased_out.is_some()) {
            return false;
        }
        !self.current(object).is_ok_and(|o| {
            o.present_on_battlefield()
                && (self.has_keyword(object, KeywordAbility::Shroud)
                    || (o.controller != controller
                        && self.has_keyword(object, KeywordAbility::Hexproof)))
        })
    }
    pub(super) fn entity_target_legal(&self, target: TargetEntity, entry: &StackInstance) -> bool {
        self.entity_legal(target)
            && match target {
                TargetEntity::Player { .. } => true,
                TargetEntity::Object { object } => {
                    self.targetable_by(object, entry.controller)
                        && self
                            .targeting_characteristics(entry)
                            .is_ok_and(|source| !self.protected_from(object, &source))
                }
            }
    }
    pub(super) fn attachment_sbas(
        &self,
        occurrences: &mut Vec<ProposedEvent>,
    ) -> Result<(), String> {
        for o in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
        {
            let source = self.color_layer(o.id)?;
            let subtypes = &source.subtypes;
            let legal = match o.attached_to {
                Some(target) => {
                    target != o.id
                        && self
                            .current(target)
                            .is_ok_and(|t| t.present_on_battlefield() && self.is_live(t.controller))
                        && self.attachment_target_type_valid(o.id, target)?
                        && !self.protected_from(target, &source)
                }
                None => false,
            };
            if subtypes == &["Aura"] && !legal {
                occurrences.push(ProposedEvent::Move {
                    object: o.id,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&o.id.card].owner,
                    },
                });
            } else if subtypes == &["Equipment"] && o.attached_to.is_some() && !legal {
                occurrences.push(ProposedEvent::Attach {
                    object: o.id,
                    target: None,
                });
            }
        }
        Ok(())
    }
    pub(super) fn execute_attachment_instruction(
        &mut self,
        frame: Resolving,
        instruction: Instruction,
    ) -> Result<(), String> {
        let block = match instruction {
            Instruction::AttachEquipment { source_block, .. }
            | Instruction::ReturnAuraFromGraveyard { source_block } => source_block,
            _ => return Err("not an attachment instruction".into()),
        };
        let cause = BatchCause::Instruction {
            stack_id: frame.spell.id,
            player: frame.spell.controller,
            source: frame.spell.source,
            cursor: frame.cursor,
            source_block: block,
        };
        match instruction {
            Instruction::AttachEquipment { target_slot, .. } => {
                let object = frame.spell.source;
                let target = frame.spell.targets[target_slot as usize];
                if frame.legal_targets.get(target_slot as usize) == Some(&true)
                    && self.object_target_legal(&frame.spell, target)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.present_on_battlefield() && o.attached_to != Some(target))
                {
                    self.commit(
                        ProposedEvent::Attach {
                            object,
                            target: Some(target),
                        },
                        cause,
                    )?;
                }
            }
            Instruction::ReturnAuraFromGraveyard { .. } => {
                // CR 400.7d: the trigger finds the new graveyard incarnation,
                // but cannot find a later incarnation after another zone move.
                let source = frame.spell.source;
                let incarnation = source
                    .incarnation
                    .checked_add(1)
                    .ok_or("Aura incarnation overflow")?;
                let object = ObjectId {
                    card: source.card,
                    incarnation,
                };
                let owner = self.state.rules.cards[&source.card].owner;
                if self
                    .current(object)
                    .is_ok_and(|o| o.zone == (Zone::Graveyard { owner }))
                {
                    self.move_object_for(object, Zone::Hand { owner }, cause)?;
                }
            }
            _ => return Err("not an attachment instruction".into()),
        }
        self.finish_instruction(frame, InstructionOutcome::Effect { actual: None })
    }
}
