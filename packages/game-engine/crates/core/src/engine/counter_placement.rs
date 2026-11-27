//! Prospective permanent and player counter placements. CR 101.4, 122.6, 601.2d, 608.2b, 704.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Freeze instruction allocations, then place all surviving amounts in one
//! batch. Costs and their reversal are separate.
use super::*;
use mtg_contracts::kir::{CounterKind, PlayerCounterKind, PlayerExpr};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterPlacementMember {
    pub object: ObjectId,
    pub counter: CounterKind,
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
    /// The saved kernel task an evaluated placement resumes, whose resolving
    /// invocation `frame` copies for its cause.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<FrameLink>,
}
impl RulesTxn {
    /// CR 122.6: an evaluated kernel step's placement, which resumes `kernel`.
    pub(super) fn begin_kernel_counter_placements(
        &mut self,
        frame: Resolving,
        counter: CounterKind,
        allocations: Vec<(ObjectId, Quantity)>,
        cause: BatchCause,
        kernel: FrameLink,
    ) -> Result<(), String> {
        let members = allocations
            .into_iter()
            .map(|(object, amount)| CounterPlacementMember {
                object,
                counter: counter.clone(),
                amount,
            })
            .collect();
        self.begin_counter_transaction(frame, members, vec![], cause, Some(kernel))
    }

    fn begin_counter_transaction(
        &mut self,
        frame: Resolving,
        members: Vec<CounterPlacementMember>,
        players: Vec<PlayerCounterPlacement>,
        cause: BatchCause,
        kernel: Option<FrameLink>,
    ) -> Result<(), String> {
        let transaction = CounterPlacementTransaction {
            members,
            players,
            cause,
            frame: Box::new(frame),
            kernel,
        };
        self.validate_counter_allocations(&transaction)?;
        if transaction.members.is_empty() && transaction.players.is_empty() {
            return self.finish_counter_placement(*transaction.frame, kernel, std::iter::empty());
        }
        self.rules_mut().frame = Frame::CounterPlacement { transaction };
        self.counter_placement_step()
    }

    pub(super) fn can_proliferate_permanent(&self, object: ObjectId) -> bool {
        self.current(object)
            .is_ok_and(|o| o.present_on_battlefield() && o.counter_kinds().next().is_some())
    }

    /// CR 122.1: P/T and keyword counters go on battlefield creatures here;
    /// other kinds on any present permanent.
    pub(super) fn counter_recipient(&self, object: ObjectId, counter: &CounterKind) -> bool {
        if counter.needs_creature() {
            self.is_battlefield_creature(object)
        } else {
            self.current(object)
                .is_ok_and(Object::present_on_battlefield)
        }
    }

    /// CR 701.34a: one more counter of each kind the permanent has.
    fn proliferate_permanent(
        &self,
        object: ObjectId,
    ) -> Result<Vec<CounterPlacementMember>, String> {
        if !self.can_proliferate_permanent(object) {
            return Err("proliferate selected a permanent without counters".into());
        }
        Ok(self
            .current(object)?
            .counter_kinds()
            .map(|counter| CounterPlacementMember {
                object,
                counter,
                amount: Quantity::from(1),
            })
            .collect())
    }

    /// CR 122.3, 704.5q: the annihilation a permanent with both +1/+1 and
    /// -1/-1 counters undergoes at the next state-based check.
    pub(super) fn annihilation(o: &Object) -> Option<ProposedEvent> {
        let amount = std::cmp::min(
            o.plus_one_counters.clone(),
            o.counter_count(&CounterKind::MinusOneMinusOne),
        );
        (o.present_on_battlefield() && amount > Quantity::zero()).then_some(
            ProposedEvent::AnnihilateCounters {
                object: o.id,
                amount,
            },
        )
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
                TargetEntity::Object { object } => {
                    members.extend(self.proliferate_permanent(object)?);
                }
                TargetEntity::Player { player } => players.push(
                    self.proliferate_player(player)
                        .ok_or("proliferate selected a player without counters")?,
                ),
            }
        }
        self.begin_counter_transaction(frame, members, players, cause, None)
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
        self.begin_counter_transaction(frame, vec![], players, cause, None)
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
        if let Some(kernel) = tx.kernel {
            if !players.is_empty() || **frame != *self.kernel_root()? {
                return Err("kernel counter placement outside its invocation".into());
            }
            return self.validate_kernel_counters(members, cause, kernel);
        }
        let block = match self.counter_instruction(frame)? {
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
                let mut objects: Vec<_> = members.iter().map(|m| m.object).collect();
                objects.dedup();
                let expected = objects
                    .iter()
                    .map(|object| self.proliferate_permanent(*object))
                    .collect::<Result<Vec<_>, _>>();
                if !self.is_live(self.entry(frame.stack_id)?.controller)
                    || objects.windows(2).any(|pair| pair[0] >= pair[1])
                    || players
                        .windows(2)
                        .any(|pair| pair[0].player >= pair[1].player)
                    || expected.map_or(true, |e| e.concat() != *members)
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
        let proliferating = tx.kernel.is_none()
            && matches!(
                self.counter_instruction(&tx.frame)?,
                kernel!(Proliferate { .. })
            );
        // CR 701.34a: one atomic placement per permanent carries every kind
        // it receives; members are grouped by object in allocation order.
        let mut leaves: Vec<ProposedEvent> = vec![];
        for m in tx.members.iter().filter(|m| {
            if proliferating {
                self.can_proliferate_permanent(m.object)
            } else {
                self.counter_recipient(m.object, &m.counter)
            }
        }) {
            let amount = CounterAmount {
                counter: m.counter.clone(),
                amount: m.amount.clone(),
            };
            match leaves.last_mut() {
                Some(ProposedEvent::PutCounters { object, counters }) if *object == m.object => {
                    counters.push(amount);
                }
                _ => leaves.push(ProposedEvent::PutCounters {
                    object: m.object,
                    counters: vec![amount],
                }),
            }
        }
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

    pub(super) fn finish_counter_batch<'a>(
        &mut self,
        actuals: impl IntoIterator<Item = &'a Actual>,
    ) -> Result<(), String> {
        let tx = self.counter_transaction()?;
        self.finish_counter_placement(tx.frame.as_ref().clone(), tx.kernel, actuals)
    }

    fn finish_counter_placement<'a>(
        &mut self,
        frame: Resolving,
        kernel: Option<FrameLink>,
        actuals: impl IntoIterator<Item = &'a Actual>,
    ) -> Result<(), String> {
        match kernel {
            Some(resume) => self.finish_kernel_counters(resume, actuals),
            None => self.finish_instruction(
                frame,
                InstructionOutcome::CounterPlacements {
                    events: actuals
                        .into_iter()
                        .map(|actual| actual.id.clone())
                        .collect(),
                },
            ),
        }
    }

    pub(super) fn counter_placement_step(&mut self) -> Result<(), String> {
        // A recipient may have departed; never recreate a departed incarnation.
        let tx = self.counter_transaction()?;
        if let Some(group) = self.counter_placement_group(tx)? {
            self.commit_batch(tx.cause.clone(), group)?;
            Ok(())
        } else {
            self.finish_counter_batch(std::iter::empty())
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
