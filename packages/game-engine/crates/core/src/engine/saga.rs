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

impl RulesTxn {
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
            self.rules_mut()
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
// The registration is the frozen record of what its issuing chapter ability
// fixed; it names that invocation and holds no copy of its stack entry.
testkit_fields! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct DelayedChapterRegistration {
        /// The chapter ability whose resolution issued this registration.
        pub(super) issuer: StackId,
        /// That ability's lore trigger and the batch that added the counter.
        pub(super) trigger_id: TriggerId,
        pub(super) origin: OccurrenceId,
        /// Index of the printed chapter ability on the source.
        pub(super) chapter: u32,
        pub(super) source: ObjectId,
        pub(super) controller: PlayerId,
        /// The source's facts when the chapter ability issued the registration.
        pub(super) source_facts: ObjectBefore,
        pub(super) source_handles: Vec<publish::StoredObjectHandle>,
        pub(super) turn: u64,
    }
}

impl DelayedChapterRegistration {
    pub(crate) fn chapter(&self) -> Result<u32, String> {
        Ok(self.chapter)
    }
    pub(super) fn source_handles(&self) -> Result<&[publish::StoredObjectHandle], String> {
        Ok(&self.source_handles)
    }
}

impl RulesTxn {
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
            _ => Err("missing printed delayed chapter instruction".into()),
        }
    }
    /// Freezes what the resolving chapter ability `issuer` fixes (CR 603.7e).
    fn delayed_registration(
        &self,
        issuer: &StackInstance,
    ) -> Result<DelayedChapterRegistration, String> {
        let StackKind::Triggered {
            ability: EffectiveTriggerAbility::Intrinsic { index },
            witness: TriggerEventWitness::LoreAdded { .. },
            source_handles,
            trigger_id,
            event: TriggerOccurrenceId::Batch(origin),
            ..
        } = &issuer.kind
        else {
            return Err("delayed entry requires its original lore trigger".into());
        };
        self.validate_trigger_entry(issuer)?;
        if issuer.paid.as_ref() != Some(&issuer.total_cost) {
            return Err("invalid retained delayed chapter issuer".into());
        }
        Ok(DelayedChapterRegistration {
            issuer: issuer.id,
            trigger_id: *trigger_id,
            origin: origin.clone(),
            chapter: *index,
            source: issuer.source,
            controller: issuer.controller,
            source_facts: issuer
                .source_facts
                .clone()
                .ok_or("missing delayed source facts")?,
            source_handles: source_handles.clone(),
            turn: self.state.rules.turn.number,
        })
    }
    pub(super) fn validate_delayed_registration(
        &self,
        registration: &DelayedChapterRegistration,
    ) -> Result<(), String> {
        self.delayed_chapter_program(registration.source, registration.chapter)?;
        let facts = &registration.source_facts;
        if registration.issuer.0 >= self.state.rules.next_stack_id
            || registration.trigger_id.0 >= self.state.rules.next_trigger_id
            || registration.origin.batch >= self.state.rules.next_batch_id
            || registration.controller.0 >= 4
            || registration.source_handles.len() != 4
            || facts.object != registration.source
            || registration.turn == 0
            || registration.turn > self.state.rules.turn.number
        {
            return Err("invalid retained delayed chapter issuer".into());
        }
        // While the issuing ability still has an owner, the record must be
        // exactly what that invocation fixes.
        if let Ok(issuer) = self.entry(registration.issuer) {
            let mut fixed = self.delayed_registration(issuer)?;
            fixed.turn = registration.turn;
            // The invocation's same-incarnation facts can refresh on a
            // departure; the registration keeps its original (CR 113.7a).
            fixed.source_facts.clone_from(&registration.source_facts);
            if fixed != *registration {
                return Err("invalid retained delayed chapter issuer".into());
            }
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
            if row.turn != self.state.rules.turn.number
                || !self.is_live(row.controller)
                || !stack_ids.insert(row.issuer)
                || !trigger_ids.insert(row.trigger_id)
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
        let row = self.delayed_registration(self.entry(issuer)?)?;
        self.validate_delayed_registration(&row)?;
        if self.is_live(row.controller) {
            self.reserve_effect_slot()?;
            self.rules_mut().delayed_chapters.push(row);
        }
        self.finish_instruction(
            frame,
            InstructionOutcome::DelayedChapterRegistered { issuer },
        )
    }
}
