//! CR 115.1, 601.2c/i, 602.2b, 603.3d, 733.1: choosing targets captures
//! observers before the costs are paid. Successful announcements publish the
//! triggers; reversing an announcement drops its captured matches.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::engine) struct TargetTriggerDraft {
    source: Box<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_known: Option<Box<ObjectBefore>>,
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
            self.last_known = Some(Box::new(facts.clone()));
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
            let target_controller_live = self.is_live(target.controller);
            if !target_controller_live {
                continue;
            }
            let facts = Frozen {
                facts: &target,
                token: self.state.rules.tokens.contains_key(&subject.card),
                live: target_controller_live,
            };
            for object in self.state.rules.objects.values().filter(|o| {
                o.present_on_battlefield()
                    && self.is_live(o.controller)
                    && !self.printed_abilities_lost(o.id)
            }) {
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
                    if kind.matches(is_spell)
                        && player_in(by, Some(actor), object.controller)
                        && holds(pred, Some(&facts), object.controller, subject, &identity)
                    {
                        if drafts.len() >= limit {
                            return self.preflight_trigger_emission(drafts.len() + 1);
                        }
                        drafts.push(TargetTriggerDraft {
                            source: Box::new(self.object_before(object.id)?),
                            last_known: None,
                            controller: object.controller,
                            ability: EffectiveTriggerAbility::Intrinsic {
                                index: u32::try_from(index)
                                    .map_err(|_| "trigger ability index limit")?,
                            },
                            witness: TriggerEventWitness::BecameTarget {
                                target: Box::new(target.clone()),
                                target_controller_live,
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
                let binding = self.followup_event_binding(&draft.witness)?;
                self.enqueue_trigger(
                    draft.ability,
                    draft.controller,
                    *draft.source,
                    None,
                    None,
                    (
                        TriggerOccurrenceId::Target(TargetOccurrenceId { targeting: id }),
                        draft.witness,
                        binding,
                    ),
                )?;
                // Enqueue with the announcement face, then supply any later
                // LKI to the pending invocation (CR 113.7a, 608.2h).
                if let Some(last_known) = draft.last_known {
                    self.rules_mut()
                        .pending_triggers
                        .last_mut()
                        .ok_or("missing collected target trigger")?
                        .source_facts = *last_known;
                }
            }
        }
        Ok(())
    }

    pub(super) fn valid_target_match(
        &self,
        source: ObjectId,
        controller: PlayerId,
        pattern: &EventPattern,
        facts: Frozen<'_>,
        actor: PlayerId,
        is_spell: bool,
    ) -> Result<bool, String> {
        let EventPattern::BecomesTarget { object, by, kind } = pattern else {
            return Ok(false);
        };
        // A departure after targeting cannot change the controller's
        // liveness at the event. Replay authenticates the captured liveness.
        let target = facts.facts;
        self.validate_fact_identity(target)?;
        let identity = |reference: &Ref| match reference {
            Ref::Source => Some(source),
            Ref::AttachedTo => (target.object != source).then_some(target.object),
            _ => None,
        };
        let seated = |id: PlayerId| self.state.rules.original_seating.contains(&id);
        Ok(kind.matches(is_spell)
            && facts.live
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
                || draft.controller != draft.source.controller
                || !targets.contains(&target.object)
                || !seen.insert((draft.source.object, draft.ability, target.object))
                || !self
                    .state
                    .rules
                    .original_seating
                    .contains(&draft.controller)
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
            self.validate_fact_identity(&draft.source)?;
            if let Some(last_known) = &draft.last_known {
                if last_known.object != draft.source.object
                    || last_known.owner != draft.source.owner
                    || last_known.zone != Zone::Battlefield
                    || last_known.transforms < draft.source.transforms
                    || !self
                        .state
                        .rules
                        .original_seating
                        .contains(&last_known.controller)
                {
                    return Err("snapshot target announcement last-known source invariant".into());
                }
                self.validate_fact_identity(last_known)?;
            }
            let source_face = face_after(draft.source.object.face, draft.source.transforms);
            self.validate_trigger_witness(
                draft.source.object,
                source_face,
                draft.ability,
                draft.controller,
                None,
                &draft.witness,
            )?;
        }
        Ok(())
    }
}
