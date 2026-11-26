//! Prospective permanent and player counter placements. CR 101.4, 122.6, 601.2d, 608.2b, 704.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Freeze instruction allocations, then place all surviving amounts in one
//! batch. Costs and their reversal are separate.
use super::*;
use mtg_contracts::kir::{PlayerCounterKind, PlayerExpr};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementMember {
    pub object: ObjectId,
    pub amount: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerCounterPlacement {
    player: PlayerId,
    poison: Quantity,
    energy: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementTransaction {
    pub members: Vec<CounterPlacementMember>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    players: Vec<PlayerCounterPlacement>,
    pub cause: BatchCause,
    pub frame: Box<Resolving>,
}
impl RulesTxn {
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
        let members = allocations
            .into_iter()
            .map(|(object, amount)| CounterPlacementMember { object, amount })
            .collect();
        self.begin_counter_transaction(frame, members, vec![], cause)
    }

    fn begin_counter_transaction(
        &mut self,
        frame: Resolving,
        members: Vec<CounterPlacementMember>,
        players: Vec<PlayerCounterPlacement>,
        cause: BatchCause,
    ) -> Result<(), String> {
        let transaction = CounterPlacementTransaction {
            members,
            players,
            cause,
            frame: Box::new(frame),
        };
        self.validate_counter_allocations(&transaction)?;
        if transaction.members.is_empty() && transaction.players.is_empty() {
            return self.finish_instruction(
                *transaction.frame,
                InstructionOutcome::CounterPlacements { actual: vec![] },
            );
        }
        self.rules_mut().frame = Frame::CounterPlacement { transaction };
        self.counter_placement_step()
    }

    pub(super) fn can_proliferate_permanent(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.present_on_battlefield() && o.plus_one_counters > Quantity::zero())
    }

    pub(super) fn can_proliferate_player(&self, player: PlayerId) -> bool {
        self.proliferate_player(player).is_some()
    }

    fn proliferate_player(&self, player: PlayerId) -> Option<PlayerCounterPlacement> {
        if !self.is_live(player) {
            return None;
        }
        let row = &self.state.rules.players[player.0 as usize];
        let poison = row.poison_counters > Quantity::zero();
        let energy = row.energy_counters > Quantity::zero();
        (poison || energy).then(|| PlayerCounterPlacement {
            player,
            poison: Quantity::from(i64::from(poison)),
            energy: Quantity::from(i64::from(energy)),
        })
    }

    pub(super) fn proliferate(
        &mut self,
        frame: Resolving,
        mut entities: Vec<TargetEntity>,
        cause: BatchCause,
    ) -> Result<(), String> {
        // The rules place all counter kinds simultaneously (CR 701.34a).
        entities.sort_unstable();
        let mut members = vec![];
        let mut players = vec![];
        for recipient in entities {
            match recipient {
                TargetEntity::Object { object } => members.push(CounterPlacementMember {
                    object,
                    amount: Quantity::from(1),
                }),
                TargetEntity::Player { player } => players.push(
                    self.proliferate_player(player)
                        .ok_or("proliferate selected a player without counters")?,
                ),
            }
        }
        self.begin_counter_transaction(frame, members, players, cause)
    }

    pub(super) fn give_counters(
        &mut self,
        frame: Resolving,
        player: &PlayerExpr,
        counter: PlayerCounterKind,
        amount: &mtg_contracts::kir::Amount,
        source_block: u32,
    ) -> Result<(), String> {
        let players = self.player_counter_recipients(
            &frame,
            player,
            counter,
            amount.as_fixed().ok_or("missing fixed player counters")?,
        )?;
        let cause = self.instruction_cause(&frame, source_block)?;
        self.begin_counter_transaction(frame, vec![], players, cause)
    }

    fn player_counter_recipients(
        &self,
        frame: &Resolving,
        player: &PlayerExpr,
        counter: PlayerCounterKind,
        amount: &Quantity,
    ) -> Result<Vec<PlayerCounterPlacement>, String> {
        let controller = self.entry(frame.stack_id)?.controller;
        let players = match self.kernel_players(frame, player)? {
            kir::Players::One(player) => {
                self.is_live(player).then_some(player).into_iter().collect()
            }
            kir::Players::EachOpponent => self
                .live_players()
                .into_iter()
                .filter(|p| *p != controller)
                .collect(),
            kir::Players::Nobody => vec![],
        };
        Ok(players
            .into_iter()
            .map(|player| {
                let (poison, energy) = match counter {
                    PlayerCounterKind::Poison => (amount.clone(), Quantity::zero()),
                    PlayerCounterKind::Energy => (Quantity::zero(), amount.clone()),
                };
                PlayerCounterPlacement {
                    player,
                    poison,
                    energy,
                }
            })
            .collect())
    }

    fn counter_instruction<'a>(&'a self, frame: &'a Resolving) -> Result<&'a Instruction, String> {
        self.program_for(self.entry(frame.stack_id)?)?
            .instructions
            .get(frame.cursor as usize)
            .ok_or_else(|| "missing counter placement instruction".into())
    }

    fn validate_counter_allocations(&self, tx: &CounterPlacementTransaction) -> Result<(), String> {
        let frame = &tx.frame;
        let CounterPlacementTransaction {
            members,
            players,
            cause,
            ..
        } = tx;
        if members.iter().any(|m| m.amount <= Quantity::zero()) {
            return Err("invalid counter placement members".into());
        }
        let block = match self.counter_instruction(frame)? {
            kernel!(PutCounters {
                on,
                counter: mtg_contracts::kir::CounterKind::PlusOnePlusOne,
                amount: fixed!(amount),
            } @ source_block) => {
                let matches = match on {
                    mtg_contracts::kir::SetExpr::Matching { zone, filter } => {
                        let expected = self.kernel_set(frame, zone, filter)?.0;
                        expected.len() == members.len()
                            && expected
                                .iter()
                                .zip(members)
                                .all(|(id, m)| *id == m.object && &m.amount == amount)
                    }
                    mtg_contracts::kir::SetExpr::One {
                        object: mtg_contracts::kir::Ref::EventObject { .. },
                    } => {
                        let object = self
                            .entry(frame.stack_id)?
                            .kind
                            .observed_object()?
                            .last_known
                            .object;
                        matches!(members.as_slice(), [m] if m.object == object && &m.amount == amount)
                    }
                    source_object!() => {
                        matches!(members.as_slice(), [m] if m.object == self.entry(frame.stack_id)?.source && &m.amount == amount)
                    }
                    target_object!(slot) => {
                        frame.legal_targets.get(*slot as usize) == Some(&true)
                            && matches!(members.as_slice(), [m] if Some(&m.object) == self.bound_object_target(self.entry(frame.stack_id)?, *slot as usize) && &m.amount == amount)
                    }
                    _ => false,
                };
                if !matches || !players.is_empty() {
                    return Err("fixed counter allocation mismatch".into());
                }
                source_block
            }
            kernel!(GiveCounters { player, counter, amount: fixed!(amount) } @ source_block) => {
                if !members.is_empty()
                    || *players
                        != self.player_counter_recipients(frame, player, *counter, amount)?
                {
                    return Err("fixed player counter allocation mismatch".into());
                }
                source_block
            }
            kernel!(Proliferate { .. } @ source_block) => {
                if !self.is_live(self.entry(frame.stack_id)?.controller)
                    || members
                        .windows(2)
                        .any(|pair| pair[0].object >= pair[1].object)
                    || players
                        .windows(2)
                        .any(|pair| pair[0].player >= pair[1].player)
                    || members.iter().any(|m| {
                        m.amount != Quantity::from(1) || !self.can_proliferate_permanent(m.object)
                    })
                    || players
                        .iter()
                        .any(|m| self.proliferate_player(m.player).as_ref() != Some(m))
                {
                    return Err("invalid proliferate counter selection".into());
                }
                source_block
            }
            _ => return Err("counter placement instruction origin".into()),
        };
        if *cause != self.instruction_cause(frame, *block)? {
            return Err("counter placement cause".into());
        }
        Ok(())
    }

    fn counter_transaction(&self) -> Result<&CounterPlacementTransaction, String> {
        let Frame::CounterPlacement { transaction } = &self.state.rules.frame else {
            return Err("no counter placement transaction".into());
        };
        Ok(transaction)
    }
    fn counter_placement_group(
        &self,
        tx: &CounterPlacementTransaction,
    ) -> Result<Option<EventGroup>, String> {
        let instruction = self.counter_instruction(&tx.frame)?;
        let proliferating = matches!(instruction, kernel!(Proliferate { .. }));
        let event_object = matches!(
            instruction,
            kernel!(PutCounters {
                on: mtg_contracts::kir::SetExpr::One {
                    object: mtg_contracts::kir::Ref::EventObject { .. }
                },
                ..
            })
        );
        let mut leaves: Vec<_> = tx
            .members
            .iter()
            .filter(|m| {
                if proliferating {
                    self.can_proliferate_permanent(m.object)
                } else if event_object {
                    self.current(m.object)
                        .is_ok_and(Object::present_on_battlefield)
                } else {
                    self.is_battlefield_creature(m.object)
                }
            })
            .map(|m| ProposedEvent::PutPlusOneCounters {
                object: m.object,
                amount: m.amount.clone(),
            })
            .collect();
        leaves.extend(
            tx.players
                .iter()
                .map(|m| ProposedEvent::GivePlayerCounters {
                    player: m.player,
                    poison: m.poison.clone(),
                    energy: m.energy.clone(),
                }),
        );
        Ok(match leaves.len() {
            0 => None,
            1 => Some(EventGroup::Occurrence {
                proposed: leaves.remove(0),
            }),
            _ => Some(EventGroup::Simultaneous {
                occurrences: leaves,
            }),
        })
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
        if self.counter_placement_group(tx)?.as_ref() != Some(group) {
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
        // A recipient may have departed; never recreate a departed incarnation.
        let tx = self.counter_transaction()?;
        if let Some(group) = self.counter_placement_group(tx)? {
            self.commit_batch(cause, group)?;
            Ok(())
        } else {
            let outcome = { InstructionOutcome::Effect { actual: None } };
            let frame = tx.frame.as_ref().clone();
            self.finish_instruction(frame, outcome)
        }
    }
    pub(super) fn validate_counter_placement_state(&self) -> Result<(), String> {
        let Frame::CounterPlacement { transaction: tx } = &self.state.rules.frame else {
            return Ok(());
        };
        self.validate_resolving(&tx.frame)?;
        self.validate_counter_transaction(tx)
    }

    fn validate_counter_transaction(&self, tx: &CounterPlacementTransaction) -> Result<(), String> {
        if tx.members.is_empty() && tx.players.is_empty() {
            return Err("counter placement transaction size".into());
        }
        self.validate_counter_allocations(tx)
    }

    pub(super) fn counter_placement_charge(&self) -> usize {
        let frame = frame::retained_effect_frame(&self.state.rules.frames, &self.state.rules.frame);
        if !matches!(frame, Frame::CounterPlacement { .. }) {
            return 0;
        }
        // The frame names its invocation; the invocation's owner is charged
        // for its cost lock and grant payload.
        stored_history::payload_charge(frame)
    }
}
