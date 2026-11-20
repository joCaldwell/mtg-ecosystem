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
        card: CardId,
    ) -> Result<Option<std::num::NonZeroU32>, String> {
        if self.state.rules.tokens.contains_key(&card) {
            return Ok(None);
        }
        Ok(self
            .definition(card)?
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
            if object.zone == Zone::Battlefield
                && object.controller == active
                && self.saga_final_chapter(object.id.card)?.is_some()
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
        self.definition(source.card).is_ok_and(|d| {
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
            .filter(|o| o.zone == Zone::Battlefield)
        {
            let Some(final_chapter) = self.saga_final_chapter(object.id.card)? else {
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
