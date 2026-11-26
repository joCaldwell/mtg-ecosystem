//! CR 115.1, 601.2c/i, 602.2b, 603.3d, 733.1: choosing targets captures
//! observers before the costs are paid. Successful announcements publish the
//! triggers; reversing an announcement drops its captured matches.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::engine) struct TargetTriggerDraft {
    source: Box<ObjectBefore>,
    controller: PlayerId,
    ability: EffectiveTriggerAbility,
    witness: TriggerEventWitness,
}

impl TargetTriggerDraft {
    pub(in crate::engine) fn source(&self) -> ObjectId {
        self.source.object
    }

    pub(in crate::engine) fn refresh_source(&mut self, facts: &ObjectBefore) {
        if self.source.object == facts.object {
            *self.source = facts.clone();
        }
    }
}

impl RulesTxn {
    pub(super) fn targeted_permanents(entry: &StackInstance) -> BTreeSet<ObjectId> {
        entry
            .targets
            .iter()
            .copied()
            .chain(
                entry
                    .entity_targets
                    .iter()
                    .filter_map(|target| match target {
                        TargetEntity::Object { object } => Some(*object),
                        TargetEntity::Player { .. } => None,
                    }),
            )
            .collect()
    }

    pub(in crate::engine) fn stage_target_triggers(&mut self) -> Result<(), String> {
        let entry = self.top()?;
        let actor = entry.controller;
        let is_spell = entry.kind.is_spell();
        let mut drafts = Vec::new();
        let limit = self.stack_limit();
        for subject in Self::targeted_permanents(entry) {
            if !self
                .current(subject)
                .is_ok_and(Object::present_on_battlefield)
            {
                continue;
            }
            let target = self.object_before(subject)?;
            for object in self
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.present_on_battlefield())
            {
                let definition = self.copiable_definition(object.id)?;
                for (index, ability) in definition.triggered.iter().enumerate() {
                    let EventPattern::BecomesTarget {
                        object: pred,
                        by,
                        kind,
                    } = ability.condition.pattern()
                    else {
                        continue;
                    };
                    let identity = |r: &Ref| match r {
                        Ref::Source => Some(object.id),
                        Ref::AttachedTo => object.attached_to,
                        _ => None,
                    };
                    let facts = Frozen {
                        facts: &target,
                        token: self.state.rules.tokens.contains_key(&subject.card),
                        live: self.is_live(target.controller),
                    };
                    if kind.matches(is_spell)
                        && player_in(by, Some(actor), object.controller)
                        && holds(pred, Some(&facts), object.controller, subject, &identity)
                    {
                        if drafts.len() >= limit {
                            return self.preflight_trigger_emission(drafts.len() + 1);
                        }
                        drafts.push(TargetTriggerDraft {
                            source: Box::new(self.object_before(object.id)?),
                            controller: object.controller,
                            ability: EffectiveTriggerAbility::Intrinsic {
                                index: u32::try_from(index)
                                    .map_err(|_| "trigger ability index limit")?,
                            },
                            witness: TriggerEventWitness::BecameTarget {
                                target: Box::new(target.clone()),
                                actor,
                                is_spell,
                                controller_at_match: object.controller,
                            },
                        });
                    }
                }
            }
        }
        if !drafts.is_empty() {
            self.rules_mut()
                .stack
                .last_mut()
                .ok_or("missing targeting entry")?
                .target_triggers = drafts;
        }
        Ok(())
    }

    pub(in crate::engine) fn finish_target_triggers(&mut self, id: StackId) -> Result<(), String> {
        let entry = self.top()?;
        if entry.id != id || !self.stack_entry_complete(entry) {
            return Err("target occurrence requires a completed announcement".into());
        }
        if entry.target_triggers.is_empty() {
            return Ok(());
        }
        let drafts = std::mem::take(
            &mut self
                .rules_mut()
                .stack
                .last_mut()
                .ok_or("missing targeting entry")?
                .target_triggers,
        );
        self.preflight_trigger_emission(
            drafts.iter().filter(|d| self.is_live(d.controller)).count(),
        )?;
        for draft in drafts {
            if self.is_live(draft.controller) {
                self.enqueue_trigger(
                    draft.ability,
                    draft.controller,
                    *draft.source,
                    None,
                    None,
                    (
                        TriggerOccurrenceId::Target(TargetOccurrenceId { targeting: id }),
                        draft.witness,
                    ),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn valid_target_match(
        &self,
        source: ObjectId,
        controller: PlayerId,
        pattern: &EventPattern,
        target: &ObjectBefore,
        actor: PlayerId,
        is_spell: bool,
    ) -> Result<bool, String> {
        let EventPattern::BecomesTarget { object, by, kind } = pattern else {
            return Ok(false);
        };
        self.validate_characteristic_identity(target.object, &target.characteristics)?;
        let facts = Frozen {
            facts: target,
            token: self.state.rules.tokens.contains_key(&target.object.card),
            live: true,
        };
        let identity = |reference: &Ref| match reference {
            Ref::Source => Some(source),
            Ref::AttachedTo => (target.object != source).then_some(target.object),
            _ => None,
        };
        let seated = |id: PlayerId| self.state.rules.original_seating.contains(&id);
        Ok(kind.matches(is_spell)
            && seated(actor)
            && seated(target.controller)
            && seated(target.default_controller)
            && target.zone == Zone::Battlefield
            && self
                .state
                .rules
                .cards
                .get(&target.object.card)
                .is_some_and(|c| c.owner == target.owner)
            && (target.object != source || controller == target.controller)
            && player_in(by, Some(actor), controller)
            && holds(object, Some(&facts), controller, target.object, &identity))
    }

    pub(in crate::engine) fn validate_target_triggers(
        &self,
        entry: &StackInstance,
    ) -> Result<(), String> {
        if entry.target_triggers.is_empty() {
            return Ok(());
        }
        if entry.target_triggers.len() > self.stack_limit()
            || self.stack_entry_complete(entry)
            || matches!(entry.kind, StackKind::Triggered { .. })
        {
            return Err("snapshot target announcement evidence invariant".into());
        }
        let targets = Self::targeted_permanents(entry);
        let mut seen = BTreeSet::new();
        for draft in &entry.target_triggers {
            let TriggerEventWitness::BecameTarget {
                target,
                actor,
                is_spell,
                ..
            } = &draft.witness
            else {
                return Err("snapshot missing target announcement witness".into());
            };
            if *actor != entry.controller
                || *is_spell != entry.kind.is_spell()
                || !targets.contains(&target.object)
                || !seen.insert((draft.source.object, draft.ability, target.object))
                || !self
                    .state
                    .rules
                    .original_seating
                    .contains(&draft.controller)
                || !self
                    .state
                    .rules
                    .original_seating
                    .contains(&draft.source.controller)
                || draft.source.zone != Zone::Battlefield
                || self
                    .state
                    .rules
                    .cards
                    .get(&draft.source.object.card)
                    .is_none_or(|c| c.owner != draft.source.owner)
            {
                return Err("snapshot target announcement witness invariant".into());
            }
            self.validate_characteristic_identity(
                draft.source.object,
                &draft.source.characteristics,
            )?;
            self.validate_trigger_witness(
                draft.source.object,
                draft.ability,
                draft.controller,
                None,
                &draft.witness,
            )?;
        }
        Ok(())
    }
}
