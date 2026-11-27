//! Discard choices and mill use the common evaluator's players and amount.
use super::*;

impl StepResult {
    pub(super) fn record_move(
        &mut self,
        player: PlayerId,
        movement: &MoveOutcome,
        public_card: Option<Box<ObjectBefore>>,
    ) {
        self.amount
            .as_mut()
            .expect("movement projection")
            .add(&1.into());
        self.objects.push(ResultObject {
            prior: movement.previous,
            successor: Some(movement.successor),
            controller_before: player,
            public_card,
        });
        if !self.players.contains(&player) {
            self.players.push(player);
        }
    }
}

impl RulesTxn {
    /// CR 701.9c, 701.17c: no characteristic is read for an unrevealed move
    /// into a hidden zone. A public destination supplies the incarnation a
    /// later "this way" reader can find, and its known characteristics.
    pub(super) fn public_move_characteristics(
        &self,
        movement: &MoveOutcome,
    ) -> Result<Option<Box<ObjectBefore>>, String> {
        if matches!(
            movement.destination,
            Zone::Hand { .. } | Zone::Library { .. }
        ) {
            return Ok(None);
        }
        Ok(Some(Box::new(self.object_before(movement.successor)?)))
    }

    pub(in crate::engine::kir::control) fn result_cards(
        &self,
        task: &Task,
        step: u32,
        filter: &Pred,
    ) -> Result<Vec<ObjectId>, String> {
        let Some(result) = self.read_step_result(task, step)? else {
            return Ok(vec![]);
        };
        let mut matching = vec![];
        for member in &result.objects {
            if filter.0.is_empty() {
                matching.push(member.prior);
            } else if let Some(facts) = &member.public_card {
                let object = member.successor.ok_or("missing result card successor")?;
                let current = self
                    .current(object)
                    .ok()
                    .map(|_| self.object_before(object))
                    .transpose()?;
                if self.damage_fact_matches(
                    filter,
                    facts.controller,
                    None,
                    current.as_ref().unwrap_or(facts),
                ) {
                    matching.push(object);
                }
            }
        }
        Ok(matching)
    }

    pub(super) fn begin_discard(
        &mut self,
        mut task: Task,
        players: Vec<PlayerId>,
        amount: Option<Quantity>,
        random: bool,
        block: u32,
    ) -> Result<(), String> {
        let Some(count) = amount else {
            let chosen = players
                .into_iter()
                .flat_map(|p| self.action_hand(p).into_iter().map(move |o| (p, o)))
                .collect();
            return self.commit_kernel_moves(task, chosen, block);
        };
        if players.is_empty() || count == Quantity::zero() {
            return self.commit_kernel_moves(task, vec![], block);
        }
        task.stage = Stage::Step(if random {
            if self.state.rules.match_state.is_none() {
                return Err("random discard requires a seeded match".into());
            }
            StepProgress::Sample {
                remaining: players,
                chosen: vec![],
                count,
                sampling: None,
            }
        } else {
            StepProgress::Collect {
                remaining: players,
                chosen: vec![],
                count,
                private: true,
            }
        });
        self.install_kernel(task);
        Ok(())
    }

    pub(in crate::engine::kir::control) fn step_sample(
        &mut self,
        mut task: Task,
    ) -> Result<(), String> {
        let (_, block) = self.node_at(&task)?;
        let Stage::Step(StepProgress::Sample {
            remaining,
            chosen,
            count,
            sampling: progress,
        }) = &mut task.stage
        else {
            return Err("missing sample progress".into());
        };
        let Some(player) = remaining.first().copied() else {
            let chosen = std::mem::take(chosen)
                .into_iter()
                .filter(|(p, _)| self.is_live(*p))
                .collect();
            return self.commit_kernel_moves(task, chosen, block);
        };
        if self.is_live(player) {
            let hand = self.action_hand(player);
            if let Some(cards) = self.advance_seat_sample(player, hand, count, progress)? {
                chosen.extend(cards.into_iter().map(|object| (player, object)));
                remaining.remove(0);
            }
        } else {
            *progress = None;
            remaining.remove(0);
        }
        self.install_kernel(task);
        Ok(())
    }

    pub(in crate::engine::kir::control) fn validate_sample(
        &self,
        task: &Task,
        state: &StepProgress,
    ) -> Result<(), String> {
        let StepProgress::Sample {
            remaining,
            chosen,
            count,
            sampling: progress,
        } = state
        else {
            return Err("missing sample progress".into());
        };
        let Some((
            _,
            Evaluated::Discard {
                player,
                amount: Some(amount),
                random: true,
            },
        )) = self.node_at(task)?.0.verb().and_then(Verb::evaluated)
        else {
            return Err("missing random discard signature".into());
        };
        self.validate_private_choices(task, player, remaining, chosen, count, amount)?;
        if let Some(progress) = progress {
            let p = *remaining.first().ok_or("random selection without a seat")?;
            if self.is_live(p) {
                self.validate_seat_sample(p, &self.action_hand(p), count, progress)?;
            }
        }
        Ok(())
    }

    pub(super) fn validate_private_choices(
        &self,
        task: &Task,
        player: &PlayerExpr,
        remaining: &[PlayerId],
        chosen: &[(PlayerId, ObjectId)],
        count: &Quantity,
        amount: &Amount,
    ) -> Result<(), String> {
        let live_remaining: Vec<_> = remaining
            .iter()
            .copied()
            .filter(|p| self.is_live(*p))
            .collect();
        let Resolved::Present(players) = self.evaluated_players(task, player)? else {
            if live_remaining.is_empty() && chosen.iter().all(|(p, _)| !self.is_live(*p)) {
                return Ok(());
            }
            return Err("hidden choices without a live recipient".into());
        };
        if !players.ends_with(&live_remaining)
            || *count <= Quantity::zero()
            || (matches!(
                amount,
                Amount::Fixed { .. }
                    | Amount::Var {
                        of: Ref::AnnouncedX
                    }
            ) && self.evaluated_amount(task, amount)? != Resolved::Present(count.clone()))
        {
            return Err("invalid hidden choice progress".into());
        }
        let processed = &players[..players.len() - live_remaining.len()];
        let expected = processed.iter().copied().flat_map(|p| {
            let wanted = self
                .action_hand(p)
                .len()
                .min(count.to_u32().unwrap_or(u32::MAX) as usize);
            std::iter::repeat_n(p, wanted)
        });
        if !chosen
            .iter()
            .filter(|(p, _)| self.is_live(*p))
            .map(|(p, _)| *p)
            .eq(expected)
            || chosen
                .iter()
                .any(|(p, o)| self.is_live(*p) && self.zone(*o) != Some(Zone::Hand { owner: *p }))
            || chosen.iter().map(|(_, o)| o).collect::<BTreeSet<_>>().len() != chosen.len()
        {
            return Err("invalid hidden selection".into());
        }
        Ok(())
    }

    /// CR 701.17a-b and 608.2c: compute each affected library's actual
    /// prefix before committing any member of the simultaneous event.
    pub(super) fn perform_mill(
        &mut self,
        task: &Task,
        players: Vec<PlayerId>,
        amount: &Quantity,
        block: u32,
    ) -> Result<(), String> {
        let mut occurrences = players
            .into_iter()
            .map(|player| {
                Ok(ProposedEvent::Mill {
                    player,
                    count: amount.clone(),
                    cards: self.library_prefix(player, amount)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let group = if occurrences.len() == 1 {
            EventGroup::Occurrence {
                proposed: occurrences.remove(0),
            }
        } else {
            EventGroup::Simultaneous { occurrences }
        };
        let cause = self.instruction_cause(self.kernel_root()?, block)?;
        let batch = self.commit_batch(cause, group)?;
        let mut result = StepResult {
            amount: Some(Quantity::zero()),
            ..StepResult::default()
        };
        let mut outcomes = Vec::new();
        for receipt in batch.occurrences {
            let EventOutcome::Milled { player, movements } = &receipt.actual else {
                return Err("unexpected mill receipt".into());
            };
            result.events.push(receipt.id);
            for movement in movements {
                result.record_move(
                    *player,
                    movement,
                    self.public_move_characteristics(movement)?,
                );
            }
            outcomes.push(InstructionOutcome::Effect {
                actual: Some(receipt.actual),
            });
        }
        self.complete_evaluated(task, Some(result), outcomes)
    }
}
