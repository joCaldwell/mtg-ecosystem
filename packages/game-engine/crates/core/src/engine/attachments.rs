//! Finite attachments; pinned CR 301.5, 303.4, 701.3, 702.6/11/18,
//! 611.3a, 613.1f/4c, 704.5m/n and 603.10a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
// One equip can begin an otherwise empty-stack episode. Four Rancors can
// each enter the graveyard once per episode; recasting requires an empty stack.
pub(super) const STACK_BOUND: usize = four_colors::STACK_BOUND + 8 + 1 + 4;
impl Game {
    pub(super) fn attachments_enabled(&self) -> bool {
        self.removal_enabled()
            || (self.prepared.config.profile == ATTACHMENTS_PROFILE
                && self.prepared.catalog.schema_version == 17)
    }
    pub(super) fn target_is_creature(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.zone == Zone::Battlefield)
            && self
                .definition(object.card)
                .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
    }
    pub(super) fn creature_modifications(&self, target: ObjectId) -> Vec<&StaticEffect> {
        if !self.attachments_enabled() || !self.target_is_creature(target) {
            return vec![];
        }
        let Ok(creature) = self.current(target) else {
            return vec![];
        };
        self.state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .flat_map(|o| {
                self.definition(o.id.card).into_iter().flat_map(move |d| {
                    d.static_abilities
                        .iter()
                        .filter_map(move |a| match &a.effect {
                            StaticEffect::ModifyCreatures { scope, .. }
                                if match scope {
                                    CreatureScope::Source => o.id == target,
                                    CreatureScope::Controller => {
                                        o.controller == creature.controller
                                    }
                                    CreatureScope::Equipped | CreatureScope::Enchanted => {
                                        o.attached_to == Some(target)
                                    }
                                } =>
                            {
                                Some(&a.effect)
                            }
                            _ => None,
                        })
                })
            })
            .collect()
    }
    pub(super) fn current_keywords(&self, object: ObjectId) -> Vec<KeywordAbility> {
        let mut keywords = self
            .definition(object.card)
            .map(|d| d.keywords.clone())
            .unwrap_or_default();
        for effect in self.creature_modifications(object) {
            if let StaticEffect::ModifyCreatures {
                keywords: grants, ..
            } = effect
            {
                for keyword in grants {
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
        self.creature_modifications(object).iter().any(|a| matches!(a, StaticEffect::ModifyCreatures { restrictions, .. } if restrictions.contains(&restriction)))
    }
    pub(super) fn targetable_by(&self, object: ObjectId, controller: PlayerId) -> bool {
        // CR 702.11b/18a apply only while the ability functions on battlefield.
        !self.current(object).is_ok_and(|o| {
            o.zone == Zone::Battlefield
                && (self.has_keyword(object, KeywordAbility::Shroud)
                    || (o.controller != controller
                        && self.has_keyword(object, KeywordAbility::Hexproof)))
        })
    }
    pub(super) fn entity_target_legal(&self, target: TargetEntity, controller: PlayerId) -> bool {
        self.entity_legal(target)
            && match target {
                TargetEntity::Player { .. } => true,
                TargetEntity::Object { object } => self.targetable_by(object, controller),
            }
    }
    pub(super) fn attachment_sbas(
        &self,
        occurrences: &mut Vec<ProposedEvent>,
    ) -> Result<(), String> {
        if !self.attachments_enabled() {
            return Ok(());
        }
        for o in self
            .state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
        {
            let subtypes = &self.definition(o.id.card)?.characteristics.subtypes;
            let legal = o
                .attached_to
                .is_some_and(|t| t != o.id && self.target_legal(t));
            if subtypes == &["Aura"] && !legal {
                occurrences.push(ProposedEvent::Move {
                    object: o.id,
                    destination: Zone::Graveyard {
                        owner: self.state.cards[&o.id.card].owner,
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
        if !self.attachments_enabled() {
            return Err("unsupported attachment instruction".into());
        }
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
                        .is_ok_and(|o| o.zone == Zone::Battlefield && o.attached_to != Some(target))
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
                let owner = self.state.cards[&source.card].owner;
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
