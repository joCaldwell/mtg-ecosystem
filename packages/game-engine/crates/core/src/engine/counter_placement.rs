//! Prospective +1/+1 placements. CR 101.4, 122.6, 601.2d, 608.2b, 614.1a/4/5, 616.1, 704.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Freeze instruction allocations, stage replacements separately for each incarnation,
//! then place all surviving amounts in one batch. Costs and their reversal are separate.
use super::*;
use mtg_contracts::{CounterPlacementModification, CounterRecipient};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementEffectKey {
    pub source: ObjectId,
    pub ability: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementApplication {
    pub effect: CounterPlacementEffectKey,
    pub provider: ObjectBefore,
    pub recipient: ObjectBefore,
    pub modification: CounterPlacementModification,
    pub before: Quantity,
    pub after: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CounterPlacementMember {
    pub object: ObjectId,
    pub original_amount: Quantity,
    pub amount: Quantity,
    pub applications: Vec<CounterPlacementApplication>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CounterPlacementTransaction {
    pub members: Vec<CounterPlacementMember>,
    current: usize,
    pub cause: BatchCause,
    pub frame: Box<Resolving>,
}
/// A viewer's public provider reference plus the definition's ability index.
/// Decision tokens bind it to one exact instruction and applied prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementEffectHandle {
    pub source: ObjectHandle,
    pub ability: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleCounterPlacementCandidate {
    pub effect: CounterPlacementEffectHandle,
    pub modification: CounterPlacementModification,
    pub amount_before: Quantity,
    pub amount_after: Quantity,
}
impl Game {
    pub(super) fn begin_counter_placement(
        &mut self,
        frame: Resolving,
        object: ObjectId,
        amount: Quantity,
        cause: BatchCause,
    ) -> Result<(), String> {
        if amount.is_negative() {
            return Err("negative counter placement".into());
        }
        if amount == Quantity::zero() || !self.is_battlefield_creature(object) {
            return self.finish_instruction(frame, InstructionOutcome::Effect { actual: None });
        }
        self.begin_counter_placements(frame, vec![(object, amount)], cause)
    }
    pub(super) fn begin_counter_placements(
        &mut self,
        frame: Resolving,
        allocations: Vec<(ObjectId, Quantity)>,
        cause: BatchCause,
    ) -> Result<(), String> {
        self.validate_counter_allocations(&frame, &allocations, &cause)?;
        if allocations.is_empty() {
            return self.finish_instruction(
                frame,
                InstructionOutcome::CounterPlacements { actual: vec![] },
            );
        }
        let members = allocations
            .into_iter()
            .map(|(object, amount)| CounterPlacementMember {
                object,
                original_amount: amount.clone(),
                amount,
                applications: vec![],
            })
            .collect();
        let mut transaction = CounterPlacementTransaction {
            members,
            current: 0,
            cause,
            frame: Box::new(frame),
        };
        transaction.current = self.counter_member_index(&transaction);
        self.state.rules.frame = Frame::CounterPlacement { transaction };
        if self.counter_placement_candidates().is_empty() {
            return self.counter_placement_step();
        }
        Ok(())
    }

    fn counter_instruction(&self, frame: &Resolving) -> Result<&Instruction, String> {
        self.program_for(&frame.spell)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or_else(|| "missing counter placement instruction".into())
    }

    fn validate_counter_allocations(
        &self,
        frame: &Resolving,
        allocations: &[(ObjectId, Quantity)],
        cause: &BatchCause,
    ) -> Result<(), String> {
        if allocations.len() > 2
            || allocations.iter().any(|(_, n)| *n <= Quantity::zero())
            || matches!(allocations, [(a, _), (b, _)] if a.card == b.card)
        {
            return Err("invalid counter placement members".into());
        }
        let block = match self.counter_instruction(frame)? {
            Instruction::DistributePlusOneCounters {
                count,
                minimum_targets: 1,
                maximum_targets: 2,
                source_block,
            } if *count == Quantity::from(2) => {
                if allocations != self.allocated_counter_targets(frame)?.as_slice() {
                    return Err("divided counter allocation mismatch".into());
                }
                source_block
            }
            Instruction::PutPlusOneCounters {
                on,
                amount,
                source_block,
            } => {
                let object = match on {
                    CounterRecipient::Source => Some(&frame.spell.source),
                    CounterRecipient::Target { target_slot }
                        if frame.legal_targets.get(*target_slot as usize) == Some(&true) =>
                    {
                        frame.spell.targets.get(*target_slot as usize)
                    }
                    _ => None,
                };
                if !matches!(allocations, [(id, original)] if Some(id) == object && original == amount)
                {
                    return Err("fixed counter allocation mismatch".into());
                }
                source_block
            }
            Instruction::PutSourcePlusOneCountersForCastColors { source_block } => {
                if !matches!(allocations, [(object, _)] if *object == frame.spell.source) {
                    return Err("cast color counter allocation mismatch".into());
                }
                source_block
            }
            _ => return Err("counter placement instruction origin".into()),
        };
        if frame.stack_id != frame.spell.id || *cause != Self::staple_cause(frame, *block) {
            return Err("counter placement cause".into());
        }
        Ok(())
    }

    // CR 616.1 / 101.4. Each affected controller orders the applications to
    // each of their permanents. Recipients under one controller use announced
    // order; no replacement in this family consumes a shared budget.
    fn counter_member_index(&self, tx: &CounterPlacementTransaction) -> usize {
        if tx.members.len() > 1 {
            for player in self.apnap_players() {
                for (i, member) in tx.members.iter().enumerate() {
                    if self
                        .current(member.object)
                        .is_ok_and(|o| o.controller == player)
                        && !self.member_candidates(member).is_empty()
                    {
                        return i;
                    }
                }
            }
        }
        0
    }
    pub(super) fn revalidate_counter_placement_departure(&mut self) -> Result<(), String> {
        let Frame::CounterPlacement { transaction } = &self.state.rules.frame else {
            return Ok(());
        };
        let previous = transaction.current;
        let current = self.counter_member_index(transaction);
        let Frame::CounterPlacement { transaction } = &mut self.state.rules.frame else {
            unreachable!()
        };
        transaction.current = current;
        // A raw response names a provider. Never carry an accepted response
        // across recipients, even if both currently have the same provider.
        if previous != current
            || self
                .state
                .rules
                .pending
                .as_ref()
                .is_some_and(|input| match &input.response {
                    Response::CounterPlacementEffect { effect } => self
                        .validate_counter_placement_response(input.player, effect)
                        .is_err(),
                    _ => true,
                })
        {
            self.state.rules.pending = None;
        }
        Ok(())
    }
    fn counter_transaction(&self) -> Result<&CounterPlacementTransaction, String> {
        let Frame::CounterPlacement { transaction } = &self.state.rules.frame else {
            return Err("no counter placement transaction".into());
        };
        Ok(transaction)
    }
    fn current_counter_member(&self) -> Result<&CounterPlacementMember, String> {
        let tx = self.counter_transaction()?;
        tx.members
            .get(tx.current)
            .ok_or_else(|| "missing counter placement member".into())
    }
    fn counter_application(
        &self,
        member: &CounterPlacementMember,
        key: &CounterPlacementEffectKey,
    ) -> Result<CounterPlacementApplication, String> {
        if member.amount <= Quantity::zero()
            || member.applications.iter().any(|a| a.effect == *key)
            || !self.is_battlefield_creature(member.object)
        {
            return Err("counter replacement outside live positive placement".into());
        }
        let provider = self.current(key.source)?;
        let recipient = self.current(member.object)?;
        let ability = self
            .definition(key.source.card)?
            .static_abilities
            .get(key.ability as usize)
            .ok_or("counter replacement ability")?;
        let StaticEffect::ControllerCreaturePlusOneCounterReplacement { modification } =
            &ability.effect
        else {
            return Err("not a counter placement replacement".into());
        };
        if provider.zone != Zone::Battlefield
            || ability.functioning_zone != FunctioningZone::Battlefield
            || provider.controller != recipient.controller
            || !self.is_live(recipient.controller)
        {
            return Err("counter replacement controller/zone".into());
        }
        Ok(CounterPlacementApplication {
            effect: key.clone(),
            provider: self.object_before(key.source)?,
            recipient: self.object_before(member.object)?,
            modification: modification.clone(),
            before: member.amount.clone(),
            after: modification.apply(&member.amount),
        })
    }
    pub(super) fn counter_placement_candidates(&self) -> Vec<CounterPlacementEffectKey> {
        self.current_counter_member()
            .map(|member| self.member_candidates(member))
            .unwrap_or_default()
    }
    fn member_candidates(&self, member: &CounterPlacementMember) -> Vec<CounterPlacementEffectKey> {
        self.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .flat_map(|o| {
                self.definition(o.id.card).into_iter().flat_map(move |d| {
                    d.static_abilities
                        .iter()
                        .enumerate()
                        .filter_map(move |(i, a)| {
                            matches!(
                                a.effect,
                                StaticEffect::ControllerCreaturePlusOneCounterReplacement { .. }
                            )
                            .then_some(CounterPlacementEffectKey {
                                source: o.id,
                                ability: i as u32,
                            })
                        })
                })
            })
            .filter(|key| self.counter_application(member, key).is_ok())
            .collect()
    }
    pub(super) fn counter_placement_request(&self) -> Result<Option<Request>, String> {
        let Frame::CounterPlacement { .. } = &self.state.rules.frame else {
            return Ok(None);
        };
        let candidates = self.counter_placement_candidates();
        if candidates.len() <= 1 {
            return Ok(None);
        }
        let object = self.current_counter_member()?.object;
        Ok(Some(Request::CounterPlacementEffect {
            decision_id: self.state.rules.revision,
            player: self.current(object)?.controller,
            recipient: object,
            candidates,
        }))
    }
    pub(super) fn validate_counter_placement_response(
        &self,
        player: PlayerId,
        key: &CounterPlacementEffectKey,
    ) -> Result<(), String> {
        let Some(Request::CounterPlacementEffect {
            player: chooser,
            candidates,
            ..
        }) = self.counter_placement_request()?
        else {
            return Err("no counter replacement choice".into());
        };
        if player != chooser || !candidates.contains(key) {
            return Err("invalid counter replacement choice".into());
        }
        Ok(())
    }
    fn selected_counter_effect(&self) -> Result<Option<CounterPlacementEffectKey>, String> {
        let candidates = self.counter_placement_candidates();
        match candidates.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(only.clone())),
            _ => {
                let input = self
                    .state
                    .rules
                    .pending
                    .as_ref()
                    .ok_or("missing counter replacement choice")?;
                let Response::CounterPlacementEffect { effect } = &input.response else {
                    return Err("counter placement response invariant".into());
                };
                let object = self.current_counter_member()?.object;
                if input.player != self.current(object)?.controller || !candidates.contains(effect)
                {
                    return Err("invalid counter replacement choice".into());
                }
                Ok(Some(effect.clone()))
            }
        }
    }
    pub(super) fn apply_counter_placement_effect(
        &mut self,
        key: &CounterPlacementEffectKey,
    ) -> Result<EventOutcome, String> {
        let application = self.counter_application(self.current_counter_member()?, key)?;
        self.state.rules.pending = None;
        let Frame::CounterPlacement { transaction } = &mut self.state.rules.frame else {
            unreachable!()
        };
        let member = &mut transaction.members[transaction.current];
        member.amount = application.after.clone();
        member.applications.push(application.clone());
        let current = self.counter_member_index(self.counter_transaction()?);
        let Frame::CounterPlacement { transaction } = &mut self.state.rules.frame else {
            unreachable!()
        };
        transaction.current = current;
        Ok(EventOutcome::CounterPlacementModified { application })
    }
    fn counter_placement_group(
        &self,
        tx: &CounterPlacementTransaction,
    ) -> Result<Option<EventGroup>, String> {
        let mut leaves: Vec<_> = tx
            .members
            .iter()
            .filter(|m| self.is_battlefield_creature(m.object))
            .map(|m| ProposedEvent::PutPlusOneCounters {
                object: m.object,
                amount: m.amount.clone(),
            })
            .collect();
        if leaves.is_empty() {
            Ok(None)
        } else if matches!(
            self.counter_instruction(&tx.frame)?,
            Instruction::DistributePlusOneCounters { .. }
        ) {
            Ok(Some(EventGroup::Simultaneous {
                occurrences: leaves,
            }))
        } else {
            Ok(Some(EventGroup::Occurrence {
                proposed: leaves.remove(0),
            }))
        }
    }

    pub(super) fn authorize_counter_batch(
        &self,
        cause: &BatchCause,
        group: &EventGroup,
    ) -> Result<(), String> {
        let tx = self.counter_transaction()?;
        self.validate_counter_transaction(tx)?;
        if &tx.cause != cause {
            return Err("counter placement batch cause".into());
        }
        if let EventGroup::Occurrence {
            proposed: ProposedEvent::ApplyCounterPlacementEffect { effect },
        } = group
        {
            if self.selected_counter_effect()?.as_ref() != Some(effect) {
                return Err("counter replacement choice mismatch".into());
            }
            return Ok(());
        }
        if tx
            .members
            .iter()
            .any(|m| !self.member_candidates(m).is_empty())
            || self.counter_placement_group(tx)?.as_ref() != Some(group)
        {
            return Err("counter placement batch does not match fully staged recipients".into());
        }
        Ok(())
    }

    pub(super) fn finish_counter_batch(&mut self, batch: &CommittedBatch) -> Result<(), String> {
        let frame = self.counter_transaction()?.frame.as_ref().clone();
        let outcome = match &batch.group {
            EventGroup::Occurrence { .. } => InstructionOutcome::Effect {
                actual: Some(
                    batch
                        .result(&[])
                        .ok_or("missing counter placement outcome")?
                        .clone(),
                ),
            },
            EventGroup::Simultaneous { .. } => InstructionOutcome::CounterPlacements {
                actual: batch
                    .occurrences
                    .iter()
                    .map(|leaf| leaf.actual.clone())
                    .collect(),
            },
            _ => return Err("unexpected counter placement group".into()),
        };
        self.finish_instruction(frame, outcome)
    }
    pub(super) fn counter_placement_step(&mut self) -> Result<(), String> {
        let cause = self.counter_transaction()?.cause.clone();
        if let Some(effect) = self.selected_counter_effect()? {
            self.commit(ProposedEvent::ApplyCounterPlacementEffect { effect }, cause)?;
            return Ok(());
        }
        // Concessions may remove recipient/provider between recorded prefixes.
        // Preserve every applied prefix; never recreate a departed incarnation.
        let tx = self.counter_transaction()?;
        if let Some(group) = self.counter_placement_group(tx)? {
            self.commit_batch(cause, group)?;
            Ok(())
        } else {
            let outcome = if matches!(
                self.counter_instruction(&tx.frame)?,
                Instruction::DistributePlusOneCounters { .. }
            ) {
                InstructionOutcome::CounterPlacements { actual: vec![] }
            } else {
                InstructionOutcome::Effect { actual: None }
            };
            let frame = tx.frame.as_ref().clone();
            self.finish_instruction(frame, outcome)
        }
    }
    pub(super) fn visible_counter_candidate(
        &self,
        viewer: PlayerId,
        key: &CounterPlacementEffectKey,
    ) -> Result<VisibleCounterPlacementCandidate, String> {
        let a = self.counter_application(self.current_counter_member()?, key)?;
        Ok(VisibleCounterPlacementCandidate {
            effect: CounterPlacementEffectHandle {
                source: self.handle_for(viewer, key.source)?,
                ability: key.ability,
            },
            modification: a.modification,
            amount_before: a.before,
            amount_after: a.after,
        })
    }
    pub(super) fn resolve_counter_effect(
        &self,
        viewer: PlayerId,
        effect: &CounterPlacementEffectHandle,
    ) -> Result<CounterPlacementEffectKey, String> {
        let key = CounterPlacementEffectKey {
            source: self.resolve_handle(viewer, &effect.source)?,
            ability: effect.ability,
        };
        self.validate_counter_placement_response(viewer, &key)?;
        Ok(key)
    }
    pub(super) fn validate_counter_placement_state(&self) -> Result<(), String> {
        let Frame::CounterPlacement { transaction: tx } = &self.state.rules.frame else {
            return Ok(());
        };
        self.validate_resolving(&tx.frame)?;
        self.validate_counter_transaction(tx)
    }

    fn validate_counter_transaction(&self, tx: &CounterPlacementTransaction) -> Result<(), String> {
        if tx.members.is_empty() || tx.members.len() > 2 || tx.current >= tx.members.len() {
            return Err("counter placement transaction size/cursor".into());
        }
        let allocations = tx
            .members
            .iter()
            .map(|m| (m.object, m.original_amount.clone()))
            .collect::<Vec<_>>();
        self.validate_counter_allocations(&tx.frame, &allocations, &tx.cause)?;
        for member in &tx.members {
            self.validate_counter_member(member)?;
        }
        if tx.current != self.counter_member_index(tx) {
            return Err("counter placement transaction chooser".into());
        }
        Ok(())
    }

    fn validate_counter_member(&self, member: &CounterPlacementMember) -> Result<(), String> {
        let mut value = member.original_amount.clone();
        let mut keys = BTreeSet::new();
        for a in &member.applications {
            let ability = self
                .definition(a.effect.source.card)?
                .static_abilities
                .get(a.effect.ability as usize)
                .ok_or("counter prefix ability")?;
            if !keys.insert(&a.effect)
                || a.before != value
                || a.after != a.modification.apply(&value)
                || a.provider.object != a.effect.source
                || a.recipient.object != member.object
                || a.provider.zone != Zone::Battlefield
                || a.recipient.zone != Zone::Battlefield
                || !a.recipient.characteristics.has_type(CardKind::Creature)
                || a.provider.controller != a.recipient.controller
                || ability.functioning_zone != FunctioningZone::Battlefield
                || ability.effect
                    != (StaticEffect::ControllerCreaturePlusOneCounterReplacement {
                        modification: a.modification.clone(),
                    })
            {
                return Err("counter placement applied prefix".into());
            }
            value = a.after.clone();
        }
        if value != member.amount {
            return Err("counter placement quantity/history".into());
        }
        // Snapshot recovery additionally replays every accepted input, proving
        // original quantity, recipient, provider appearances and prefix reachability.
        Ok(())
    }
    pub(super) fn counter_placement_charge(&self) -> usize {
        let frame = spell_cascade_grants::retained_effect_frame(&self.state.rules.frame);
        let Frame::CounterPlacement { transaction } = frame else {
            return 0;
        };
        let spell = &transaction.frame.spell;
        stored_history::payload_charge(frame)
            .saturating_sub(costs::entry_cost_lock_charge(spell))
            .saturating_sub(spell_cascade_grants::entry_grant_charge(spell))
    }
}
