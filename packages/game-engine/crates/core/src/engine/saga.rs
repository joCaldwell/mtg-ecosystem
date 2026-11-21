//! CR 714.2b-e, 714.3a/c, 714.4 and 704.5s.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoreChange {
    pub object: ObjectId,
    pub controller: PlayerId,
    pub before: Quantity,
    pub after: Quantity,
}

impl LoreChange {
    pub(super) fn crosses(&self, number: std::num::NonZeroU32) -> bool {
        let threshold = Quantity::from(i64::from(number.get()));
        !self.before.is_negative() && self.before < threshold && self.after >= threshold
    }
}

impl Game {
    pub(super) fn saga_final_chapter(
        &self,
        object: ObjectId,
    ) -> Result<Option<std::num::NonZeroU32>, String> {
        Ok(self
            .copiable_definition(object)?
            .triggered
            .iter()
            .filter_map(|t| match t.condition {
                TriggerCondition::SagaChapter { number } => Some(number),
                _ => None,
            })
            .max())
    }

    pub(super) fn advance_saga_lore(
        &mut self,
        active: PlayerId,
    ) -> Result<Vec<LoreChange>, String> {
        let mut changes = vec![];
        for object in self.state.rules.objects.values() {
            if object.present_on_battlefield()
                && object.controller == active
                && self.saga_final_chapter(object.id)?.is_some()
            {
                let before = object.lore_counters.clone();
                let mut after = before.clone();
                after.add(&Quantity::from(1));
                changes.push(LoreChange {
                    object: object.id,
                    controller: active,
                    before,
                    after,
                });
            }
        }
        for change in &changes {
            self.state
                .rules
                .objects
                .get_mut(&change.object.card)
                .ok_or("missing Saga object")?
                .lore_counters = change.after.clone();
        }
        Ok(changes)
    }

    fn chapter_identity(&self, source: ObjectId, ability: EffectiveTriggerAbility) -> bool {
        let EffectiveTriggerAbility::Intrinsic { index } = ability else {
            return false;
        };
        self.copiable_definition(source).is_ok_and(|d| {
            d.triggered
                .get(index as usize)
                .is_some_and(|t| matches!(t.condition, TriggerCondition::SagaChapter { .. }))
        })
    }

    pub(super) fn completed_sagas(&self) -> Result<Vec<ProposedEvent>, String> {
        let mut events = vec![];
        for object in self
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.present_on_battlefield())
        {
            let Some(final_chapter) = self.saga_final_chapter(object.id)? else {
                continue;
            };
            if object.lore_counters < Quantity::from(i64::from(final_chapter.get())) {
                continue;
            }
            let pending = self
                .state
                .rules
                .pending_triggers
                .iter()
                .any(|t| t.source == object.id && self.chapter_identity(t.source, t.ability));
            let stacked=self.state.rules.stack.iter().any(|s|s.source==object.id &&
                matches!(s.kind,StackKind::Triggered {ability,..} if self.chapter_identity(s.source,ability)));
            if !pending && !stacked {
                events.push(ProposedEvent::Sacrifice {
                    object: object.id,
                    player: object.controller,
                });
            }
        }
        Ok(events)
    }
}

// CR 603.7e: the resolving chapter fixes the controller and the source of the
// delayed ability. Subsequent entries use their actual battlefield appearance.
testkit_fields! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct DelayedChapterRegistration {
        pub(super) issuer: Box<StackInstance>,
        pub(super) turn: u64,
    }
}

impl DelayedChapterRegistration {
    pub(crate) fn chapter(&self) -> Result<u32, String> {
        match &self.issuer.kind {
            StackKind::Triggered {
                ability: EffectiveTriggerAbility::Intrinsic { index },
                witness: TriggerEventWitness::LoreAdded { .. },
                ..
            } => Ok(*index),
            _ => Err("delayed entry requires its original lore trigger".into()),
        }
    }
    pub(super) fn source_handles(&self) -> Result<&[ObjectHandle], String> {
        match &self.issuer.kind {
            StackKind::Triggered { source_handles, .. } => Ok(source_handles),
            _ => Err("missing delayed chapter source handles".into()),
        }
    }
}

impl Game {
    pub(super) fn delayed_chapter_program(
        &self,
        source: ObjectId,
        chapter: u32,
    ) -> Result<&SpellProgram, String> {
        self.delayed_chapter(source, chapter)
            .map(|(_, program)| program)
    }

    pub(super) fn delayed_chapter(
        &self,
        source: ObjectId,
        chapter: u32,
    ) -> Result<(TriggerCondition, &SpellProgram), String> {
        let ability = self
            .copiable_definition(source)?
            .triggered
            .get(chapter as usize)
            .ok_or("missing delayed chapter")?;
        if !matches!(ability.condition, TriggerCondition::SagaChapter { .. }) {
            return Err("delayed issuer is not a chapter".into());
        }
        match ability.program.instructions.as_slice() {
            [Instruction::RegisterControllerCreatureEntryThisTurn { program, .. }] => {
                Ok((TriggerCondition::DelayedControllerCreatureEntry, program))
            }
            [Instruction::RegisterControllerSpellCastThisTurn { program, .. }] => {
                Ok((TriggerCondition::DelayedControllerSpellCast, program))
            }
            _ => Err("missing printed delayed chapter instruction".into()),
        }
    }
    pub(super) fn validate_delayed_registration(
        &self,
        registration: &DelayedChapterRegistration,
    ) -> Result<(), String> {
        let issuer = &registration.issuer;
        let chapter = registration.chapter()?;
        self.delayed_chapter_program(issuer.source, chapter)?;
        self.validate_trigger_entry(issuer)?;
        if issuer.id.0 >= self.state.rules.next_stack_id
            || issuer.paid.as_ref() != Some(&issuer.total_cost)
            || registration.turn == 0
            || registration.turn > self.state.rules.turn.number
        {
            return Err("invalid retained delayed chapter issuer".into());
        }
        Ok(())
    }
    pub(super) fn validate_delayed_chapters(&self) -> Result<(), String> {
        let rows = &self.state.rules.delayed_chapters;
        if rows.len().saturating_add(self.state.rules.effects.len()) > self.effect_limit() {
            return Err("delayed chapter capacity".into());
        }
        let mut stack_ids = BTreeSet::new();
        let mut trigger_ids = BTreeSet::new();
        for row in rows {
            self.validate_delayed_registration(row)?;
            let StackKind::Triggered { trigger_id, .. } = row.issuer.kind else {
                return Err("delayed chapter has no original trigger".into());
            };
            if row.turn != self.state.rules.turn.number
                || !self.is_live(row.issuer.controller)
                || !stack_ids.insert(row.issuer.id)
                || !trigger_ids.insert(trigger_id)
            {
                return Err("duplicate or expired delayed chapter registration".into());
            }
        }
        Ok(())
    }
    pub(super) fn reserve_effect_slot(&mut self) -> Result<(), String> {
        let requested =
            self.state.rules.effects.len() + self.state.rules.delayed_chapters.len() + 1;
        let capacity = self.effect_limit();
        if requested > capacity {
            return Err(self.capacity_error(RuleFault::EffectCapacityExceeded {
                capacity,
                requested,
            }));
        }
        Ok(())
    }
    pub(super) fn register_delayed_chapter(&mut self, frame: Resolving) -> Result<(), String> {
        self.validate_resolving(&frame)?;
        if frame.cursor != 0 {
            return Err("wrong delayed chapter cursor".into());
        }
        let issuer = frame.stack_id;
        let row = DelayedChapterRegistration {
            issuer: Box::new(frame.spell.clone()),
            turn: self.state.rules.turn.number,
        };
        self.validate_delayed_registration(&row)?;
        if self.is_live(row.issuer.controller) {
            self.reserve_effect_slot()?;
            self.state.rules.delayed_chapters.push(row);
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::DelayedChapterRegistered { issuer },
        )
    }
}
