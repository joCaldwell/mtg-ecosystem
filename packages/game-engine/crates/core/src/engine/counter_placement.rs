//! Prospective +1/+1 placements. CR 122.6, 614.1a/4/5/12, 616.1e-f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Every admitted producer is one post-entry instruction on one incarnation.
//! No entry-with-counters or simultaneous-placement producer is admitted. Costs
//! and their reversal restore counters directly, outside this authority.
use super::*;
use mtg_contracts::CounterPlacementModification;

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
pub(super) struct CounterPlacementTransaction {
    pub object: ObjectId,
    pub original_amount: Quantity,
    pub amount: Quantity,
    pub applications: Vec<CounterPlacementApplication>,
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
        self.state.rules.frame = Frame::CounterPlacement {
            transaction: CounterPlacementTransaction {
                object,
                original_amount: amount.clone(),
                amount,
                applications: vec![],
                cause,
                frame: Box::new(frame),
            },
        };
        if self.counter_placement_candidates().is_empty() {
            return self.counter_placement_step();
        }
        Ok(())
    }
    pub(super) fn revalidate_counter_placement_departure(&mut self) -> Result<(), String> {
        if matches!(self.state.rules.frame, Frame::CounterPlacement { .. })
            && self
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
    fn counter_application(
        &self,
        key: &CounterPlacementEffectKey,
    ) -> Result<CounterPlacementApplication, String> {
        let tx = self.counter_transaction()?;
        if tx.amount <= Quantity::zero()
            || tx.applications.iter().any(|a| a.effect == *key)
            || !self.is_battlefield_creature(tx.object)
        {
            return Err("counter replacement outside live positive placement".into());
        }
        let provider = self.current(key.source)?;
        let recipient = self.current(tx.object)?;
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
            recipient: self.object_before(tx.object)?,
            modification: modification.clone(),
            before: tx.amount.clone(),
            after: modification.apply(&tx.amount),
        })
    }
    pub(super) fn counter_placement_candidates(&self) -> Vec<CounterPlacementEffectKey> {
        if !matches!(self.state.rules.frame, Frame::CounterPlacement { .. }) {
            return vec![];
        }
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
            .filter(|key| self.counter_application(key).is_ok())
            .collect()
    }
    pub(super) fn counter_placement_request(&self) -> Result<Option<Request>, String> {
        let Frame::CounterPlacement { transaction: tx } = &self.state.rules.frame else {
            return Ok(None);
        };
        let candidates = self.counter_placement_candidates();
        if candidates.len() <= 1 {
            return Ok(None);
        }
        Ok(Some(Request::CounterPlacementEffect {
            decision_id: self.state.rules.revision,
            player: self.current(tx.object)?.controller,
            recipient: tx.object,
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
    pub(super) fn apply_counter_placement_effect(
        &mut self,
        key: &CounterPlacementEffectKey,
    ) -> Result<EventOutcome, String> {
        let application = self.counter_application(key)?;
        let Frame::CounterPlacement { transaction } = &mut self.state.rules.frame else {
            unreachable!()
        };
        transaction.amount = application.after.clone();
        transaction.applications.push(application.clone());
        Ok(EventOutcome::CounterPlacementModified { application })
    }
    pub(super) fn validate_counter_placement_commit(
        &self,
        object: ObjectId,
        amount: &Quantity,
    ) -> Result<(), String> {
        let tx = self.counter_transaction()?;
        if tx.object != object
            || tx.amount != *amount
            || !self.counter_placement_candidates().is_empty()
        {
            return Err("counter placement bypassed replacement processing".into());
        }
        Ok(())
    }
    pub(super) fn counter_placement_step(&mut self) -> Result<(), String> {
        let tx = self.counter_transaction()?.clone();
        let candidates = self.counter_placement_candidates();
        if !candidates.is_empty() {
            let effect = if candidates.len() == 1 {
                self.state.rules.pending = None;
                candidates[0].clone()
            } else {
                let input = self
                    .state
                    .rules
                    .pending
                    .take()
                    .ok_or("missing counter replacement choice")?;
                let Response::CounterPlacementEffect { effect } = input.response else {
                    return Err("counter placement response invariant".into());
                };
                self.validate_counter_placement_response(input.player, &effect)?;
                effect
            };
            self.commit(
                ProposedEvent::ApplyCounterPlacementEffect { effect },
                tx.cause,
            )?;
            return Ok(());
        }
        // Concessions may remove recipient/provider between recorded prefixes.
        // Preserve every applied prefix; never recreate a departed incarnation.
        let actual = if self.is_battlefield_creature(tx.object) {
            Some(self.commit(
                ProposedEvent::PutPlusOneCounters {
                    object: tx.object,
                    amount: tx.amount,
                },
                tx.cause,
            )?)
        } else {
            None
        };
        self.finish_instruction(*tx.frame, InstructionOutcome::Effect { actual })
    }
    pub(super) fn visible_counter_candidate(
        &self,
        viewer: PlayerId,
        key: &CounterPlacementEffectKey,
    ) -> Result<VisibleCounterPlacementCandidate, String> {
        let a = self.counter_application(key)?;
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
        if tx.original_amount <= Quantity::zero()
            || tx.frame.cursor as usize >= self.program_for(&tx.frame.spell)?.instructions.len()
            || tx.frame.stack_id != tx.frame.spell.id
        {
            return Err("counter placement transaction origin".into());
        }
        let (Instruction::PutPlusOneCounters { source_block, .. }
        | Instruction::PutSourcePlusOneCountersForCastColors { source_block }) =
            &self.program_for(&tx.frame.spell)?.instructions[tx.frame.cursor as usize]
        else {
            return Err("counter placement instruction origin".into());
        };
        if tx.cause
            != (BatchCause::Instruction {
                stack_id: tx.frame.spell.id,
                player: tx.frame.spell.controller,
                source: tx.frame.spell.source,
                cursor: tx.frame.cursor,
                source_block: *source_block,
            })
        {
            return Err("counter placement cause".into());
        }
        let mut value = tx.original_amount.clone();
        let mut keys = BTreeSet::new();
        for a in &tx.applications {
            let ability = self
                .definition(a.effect.source.card)?
                .static_abilities
                .get(a.effect.ability as usize)
                .ok_or("counter prefix ability")?;
            if !keys.insert(&a.effect)
                || a.before != value
                || a.after != a.modification.apply(&value)
                || a.provider.object != a.effect.source
                || a.recipient.object != tx.object
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
        if value != tx.amount {
            return Err("counter placement quantity/history".into());
        }
        // Snapshot recovery additionally replays every accepted input, proving
        // original quantity, recipient, provider appearances and prefix reachability.
        Ok(())
    }
}
