//! Finite attachments; pinned CR 301.5, 303.4, 701.3, 702.6/11/18,
//! 611.3a, 613.1f/4c, 704.5m/n and 603.10a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
// One equip can begin an otherwise empty-stack episode. Four Rancors can
// each enter the graveyard once per episode; recasting requires an empty stack.

impl RulesTxn {
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
    pub(crate) fn attachment_target_type_valid(
        &self,
        source: ObjectId,
        target: ObjectId,
    ) -> Result<bool, String> {
        let definition = self.copiable_definition(source)?;
        let target_characteristics = self.structural_characteristics(target)?;
        if definition.characteristics.subtypes == ["Equipment"] {
            return Ok(!self
                .structural_characteristics(source)?
                .has_type(CardKind::Creature)
                && target_characteristics.has_type(CardKind::Creature));
        }
        if definition.characteristics.subtypes == ["Aura"] {
            return Ok(definition
                .spell
                .as_ref()
                .and_then(|p| p.targets.first())
                .is_some_and(|spec| match spec.domain {
                    TargetDomain::BattlefieldCreature => {
                        target_characteristics.has_type(CardKind::Creature)
                    }
                    TargetDomain::BattlefieldPermanent => {
                        spec.filter.as_ref().is_some_and(|filter| {
                            filter.types.is_empty()
                                || filter
                                    .types
                                    .iter()
                                    .any(|kind| target_characteristics.has_type(*kind))
                        })
                    }
                    _ => false,
                }));
        }
        Ok(false)
    }

    pub(crate) fn validate_attachment_state(&self) -> Result<(), String> {
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
                    .structural_characteristics(o.id)?
                    .subtypes
                    .iter()
                    .any(|s| s == "Equipment" || s == "Aura")
                || !known_target
            {
                return Err("snapshot attachment invariant".into());
            }
        }
        // Illegal attachments may exist during resolution. A priority request
        // proves stabilization finished, so every applicable SBA was applied.
        if matches!(self.state.rules.frame, Frame::Priority) {
            let mut pending = Vec::new();
            self.attachment_sbas(&mut pending)?;
            if !pending.is_empty() {
                return Err("unstabilized attachment at priority".into());
            }
        }
        Ok(())
    }

    pub(crate) fn target_is_creature(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.present_on_battlefield())
            && self
                .structural_characteristics(object)
                .is_ok_and(|d| d.has_type(CardKind::Creature))
    }
    // An indexed list is valid only for the current, unchanged rules state.
    // Both paths preserve object-table order and exact phased-in membership.
    pub(super) fn battlefield_static_sources<'a>(
        &'a self,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = &'a Object> + 'a {
        let objects = &self.state.rules.objects;
        sources
            .into_iter()
            .flatten()
            .filter_map(move |card| objects.get(card))
            .chain(
                sources
                    .is_none()
                    .then(|| objects.values())
                    .into_iter()
                    .flatten(),
            )
            .filter(|object| object.present_on_battlefield())
    }

    pub(crate) fn current_keywords(&self, object: ObjectId) -> Vec<KeywordAbility> {
        self.current_keywords_with_sources(object, None)
    }

    pub(super) fn current_keywords_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Vec<KeywordAbility> {
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
        for (timestamp, change) in
            self.static_changes(object, mtg_contracts::kir::Layer::Ability, sources)
        {
            let mtg_contracts::kir::Change::AddKeywords { keywords: grants } = change else {
                continue;
            };
            for keyword in grants {
                if *keyword == KeywordAbility::Indestructible
                    && latest_removal.is_some_and(|(_, removed_at)| timestamp <= removed_at)
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
                } if *target == object && self.is_battlefield_creature(object) => Some(keywords),
                ResolvedEffectKind::GrantKeywordsSet {
                    targets, keywords, ..
                } if targets.contains(&object)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.present_on_battlefield()) =>
                {
                    Some(keywords)
                }
                ResolvedEffectKind::SetCharacteristics {
                    targets, values, ..
                } if targets.contains(&object)
                    && self
                        .current(object)
                        .is_ok_and(|o| o.present_on_battlefield()) =>
                {
                    Some(&values.keywords)
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
    pub(crate) fn has_restriction(
        &self,
        object: ObjectId,
        restriction: CreatureRestriction,
    ) -> bool {
        self.creature_restrictions(object).any(|r| r == restriction)
    }

    pub(super) fn creature_restrictions(
        &self,
        object: ObjectId,
    ) -> impl Iterator<Item = CreatureRestriction> + '_ {
        self.creature_restrictions_with_sources(object, None)
    }

    pub(super) fn creature_restrictions_with_sources<'a>(
        &'a self,
        object: ObjectId,
        sources: Option<&'a [CardId]>,
    ) -> impl Iterator<Item = CreatureRestriction> + 'a {
        use mtg_contracts::kir::CombatRequirement as R;
        self.combat_requirements(object, sources)
            .filter_map(|rule| match rule {
                R::CantAttack => Some(CreatureRestriction::CantAttack),
                R::CantBlock => Some(CreatureRestriction::CantBlock),
                R::CantAttackOrBlock => Some(CreatureRestriction::CantAttackOrBlock),
                R::CantBeBlocked => Some(CreatureRestriction::CantBeBlocked),
                R::CanBlockOnlyFlying => Some(CreatureRestriction::CanBlockOnlyFlying),
                R::AttackEachCombat => None,
            })
    }

    pub(crate) fn targetable_by(&self, object: ObjectId, controller: PlayerId) -> bool {
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
    pub(crate) fn attachment_sbas(
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
            let subtypes = &self.copiable_definition(o.id)?.characteristics.subtypes;
            let legal = match o.attached_to {
                Some(target) => {
                    target != o.id
                        && self
                            .current(target)
                            .is_ok_and(|t| t.present_on_battlefield() && self.is_live(t.controller))
                        && self.attachment_target_type_valid(o.id, target)?
                        && !self.protected_from(target, self.color_layer(o.id)?.as_ref())
                }
                None => false,
            };
            if legal {
                continue;
            }
            if subtypes == &["Aura"] {
                occurrences.push(ProposedEvent::Move {
                    object: o.id,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&o.id.card].owner,
                    },
                });
            } else if subtypes == &["Equipment"] && o.attached_to.is_some() {
                occurrences.push(ProposedEvent::Attach {
                    object: o.id,
                    target: None,
                });
            }
        }
        Ok(())
    }
}
