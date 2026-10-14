use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Live,
    Left,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlayerLastKnown {
    pub(super) life: Quantity,
    pub(super) mana: BTreeMap<Color, Quantity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalInput {
    pub after_decisions: u64,
    pub boundary: u64,
    pub player: PlayerId,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayJournal {
    pub decisions: Vec<AcceptedInput>,
    pub concessions: Vec<ExternalInput>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ControlSource {
    Persistent,
    Static { object: ObjectId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlEffect {
    pub(super) id: u32,
    pub(super) object: ObjectId,
    pub(super) controller: PlayerId,
    pub(super) source: ControlSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderHistory {
    pub owner: PlayerId,
    pub color_identity: Vec<Color>,
    pub command_casts: BTreeMap<PlayerId, Quantity>,
    pub combat_damage: BTreeMap<PlayerId, Quantity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderLedger {
    pub(super) records: BTreeMap<CardId, CommanderHistory>,
}
impl CommanderLedger {
    // CR 903.3, 903.8, 903.10a: physical designation, per-caster command
    // casts, and per-recipient damage from the same physical commander.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub fn history(&self, card: CardId) -> Option<&CommanderHistory> {
        self.records.get(&card)
    }
    pub fn tax(&self, card: CardId, caster: PlayerId) -> Result<Quantity, String> {
        let h = self
            .records
            .get(&card)
            .ok_or("physical card is not designated")?;
        let mut amount = h.command_casts.get(&caster).cloned().unwrap_or_default();
        let prior = amount.clone();
        amount.add(&prior);
        Ok(amount)
    }
    // Only completed casting calls this hook. The first admitted spell is not
    // a commander; command-zone casting remains outside the reviewed profile.
    pub(super) fn record_cast(
        &mut self,
        card: CardId,
        caster: PlayerId,
        from: Zone,
    ) -> Result<(), String> {
        let Some(h) = self.records.get_mut(&card) else {
            return Ok(());
        };
        if let Zone::Command { owner } = from {
            if caster != h.owner || owner != h.owner {
                return Err("unsupported command-zone casting permission".into());
            }
            h.command_casts
                .entry(caster)
                .or_default()
                .add(&Quantity::from(1));
        }
        Ok(())
    }
    #[cfg(test)]
    // Domain contract only until a production combat/damage slice exists.
    pub(super) fn record_damage(
        &mut self,
        card: CardId,
        recipient: PlayerId,
        combat: bool,
        actual: &Quantity,
    ) -> Result<(), String> {
        if actual.is_negative() || recipient.0 >= 4 {
            return Err("invalid actual damage fact".into());
        }
        if combat && let Some(h) = self.records.get_mut(&card) {
            h.combat_damage.entry(recipient).or_default().add(actual);
        }
        Ok(())
    }
    pub fn combat_loss_candidates(&self) -> Vec<PlayerId> {
        self.records
            .values()
            .flat_map(|h| {
                h.combat_damage
                    .iter()
                    .filter(|(_, n)| **n >= Quantity::from(21))
                    .map(|(p, _)| *p)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

impl Game {
    pub(super) fn initialize_commanders(&mut self) -> Result<(), String> {
        for card in self.prepared.config.commanders.clone() {
            let d = self.definition(card)?;
            if !d.characteristics.legendary || d.characteristics.kind != CardKind::Creature {
                return Err("unsupported commander eligibility".into());
            }
            let history = CommanderHistory {
                owner: self.state.cards[&card].owner,
                color_identity: d.characteristics.color_identity.clone(),
                command_casts: BTreeMap::new(),
                combat_damage: BTreeMap::new(),
            };
            self.state.commanders.records.insert(card, history);
        }
        Ok(())
    }
    pub fn commander_ledger(&self) -> &CommanderLedger {
        &self.state.commanders
    }
    pub fn live_players(&self) -> Vec<PlayerId> {
        self.state
            .original_seating
            .iter()
            .copied()
            .filter(|p| self.is_live(*p))
            .collect()
    }
    pub fn active_player(&self) -> Option<PlayerId> {
        self.state.active
    }
    pub fn turn_owner(&self) -> PlayerId {
        self.state.turn_owner
    }
    pub fn replay_journal(&self) -> ReplayJournal {
        ReplayJournal {
            decisions: self.state.journal.clone(),
            concessions: self.state.external_inputs.clone(),
        }
    }
    pub(super) fn is_live(&self, player: PlayerId) -> bool {
        self.state
            .players
            .get(player.0 as usize)
            .is_some_and(|p| p.status == PlayerStatus::Live)
    }
    pub(super) fn next_live(&self, after: PlayerId) -> Option<PlayerId> {
        (1..=4)
            .map(|n| PlayerId((after.0 + n) % 4))
            .find(|p| self.is_live(*p))
    }
    pub(super) fn all_live_passed(&self) -> bool {
        self.live_players()
            .iter()
            .all(|p| self.state.passes.contains(p))
    }
    pub(super) fn begin_resolution(&mut self) -> Result<(), String> {
        self.state.passes.clear();
        let stack = self.top()?.clone();
        let legal_targets = stack
            .targets
            .iter()
            .map(|t| self.target_legal(*t))
            .collect();
        self.state.frame = Frame::Resolve {
            frame: Resolving {
                stack_id: stack.id,
                spell: stack,
                cursor: 0,
                legal_targets,
                outcomes: vec![],
            },
        };
        Ok(())
    }
    /// Queued at an explicit boundary; advance applies it before ordinary work.
    /// CR 104.3a, 800.4a/j. Pinned official rules:
    /// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub fn concede(&mut self, player: PlayerId) -> Result<(), String> {
        if !self.is_live(player)
            || self.state.pending_concessions.contains(&player)
            || matches!(
                self.state.frame,
                Frame::Finished { .. } | Frame::Fault { .. }
            )
            || self
                .live_players()
                .len()
                .saturating_sub(self.state.pending_concessions.len())
                <= 1
        {
            return Err("concession is stale or game has already ended".into());
        }
        let revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or("revision resource limit")?;
        let mut next = self.fork();
        next.state.external_inputs.push(ExternalInput {
            after_decisions: u64::try_from(next.state.journal.len())
                .map_err(|_| "input count resource limit")?,
            boundary: next.state.rule_steps,
            player,
        });
        next.state.pending_concessions.push(player);
        next.state.revision = revision;
        next.refresh_visibility()?;
        self.state = next.state;
        self.trace = next.trace;
        self.state_token = Arc::new(());
        Ok(())
    }
    fn reverse_departure_cast(&mut self, cast: Casting) -> Result<(), String> {
        self.reverse_cast(cast, "departure made casting targets illegal", true)
    }
    fn refresh_controllers(&mut self, departing: PlayerId) -> Result<(), String> {
        // CR 110.2, 800.4a-c: ending a grant restores the latest remaining
        // grant or the default controller, never ownership as a substitute.
        let objects = self
            .state
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .cloned()
            .collect::<Vec<_>>();
        for object in objects {
            let controller = self
                .state
                .control_effects
                .iter()
                .filter(|e| e.object == object.id && self.is_live(e.controller))
                .max_by_key(|e| e.id)
                .map(|e| e.controller)
                .unwrap_or(object.default_controller);
            if !self.is_live(controller) {
                self.move_object_for(
                    object.id,
                    Zone::Exile,
                    BatchCause::Departure { player: departing },
                )?;
            } else if controller != object.controller {
                self.state
                    .objects
                    .get_mut(&object.id.card)
                    .ok_or("missing control object")?
                    .controller = controller;
                self.record_event(Event::ControlChanged {
                    object: object.id,
                    previous: object.controller,
                    controller,
                })?;
            }
        }
        Ok(())
    }
    pub(super) fn apply_departure(&mut self, player: PlayerId) -> Result<(), String> {
        if !self.is_live(player) {
            return Err("duplicate departure".into());
        }
        let last = PlayerLastKnown {
            life: self.state.players[player.0 as usize].life.clone(),
            mana: self.state.players[player.0 as usize].mana.clone(),
        };
        self.state.players[player.0 as usize].last_known = Some(last);
        self.state.players[player.0 as usize].status = PlayerStatus::Left;
        self.record_event(Event::PlayerDeparted { player })?;
        // Owned objects leave, not a graveyard move and not an SBA (800.4a).
        let removed = self
            .state
            .objects
            .values()
            .filter(|o| self.state.cards[&o.id.card].owner == player)
            .map(|o| o.id)
            .collect::<Vec<_>>();
        if !removed.is_empty() {
            self.commit_batch(
                BatchCause::Departure { player },
                EventGroup::Simultaneous {
                    occurrences: removed
                        .iter()
                        .map(|object| ProposedEvent::LeaveGame {
                            object: *object,
                            departing_owner: player,
                        })
                        .collect(),
                },
            )?;
        }
        let ended = self
            .state
            .control_effects
            .iter()
            .filter(|e| {
                e.controller == player
                    || removed.contains(&e.object)
                    || matches!(e.source,ControlSource::Static{object} if removed.contains(&object))
            })
            .map(|e| e.id)
            .collect::<Vec<_>>();
        self.state
            .control_effects
            .retain(|e| !ended.contains(&e.id));
        for id in ended {
            self.record_event(Event::ControlEffectEnded { effect_id: id })?;
        }
        self.refresh_controllers(player)?;
        let lost_stack = self
            .state
            .stack
            .iter()
            .filter(|s| self.current(s.source).is_err())
            .map(|s| s.id)
            .collect::<Vec<_>>();
        self.state.stack.retain(|s| !lost_stack.contains(&s.id));
        for stack_id in &lost_stack {
            self.record_event(Event::StackRemovedForDeparture {
                stack_id: *stack_id,
                player,
            })?;
        }
        if self.state.active == Some(player) {
            self.state.active = None;
        }
        if self.state.priority == player {
            self.state.priority = self.next_live(player).ok_or("no remaining player")?;
        }
        if self
            .state
            .pending
            .as_ref()
            .is_some_and(|p| p.player == player)
        {
            self.state.pending = None;
        }
        let live = self.live_players();
        self.state.passes.retain(|p| live.contains(p));
        if live.len() == 1 {
            let winner = live[0];
            self.state.pending = None;
            self.state.frame = Frame::Finished { winner };
            self.state.priority = winner;
            self.record_event(Event::GameWon { winner })?;
            return Ok(());
        }
        // CR 608.2m: a started invocation owns its frozen instance and keeps
        // resolving after its source leaves. Only an unfinished casting frame
        // whose own source left is discarded; lower departures cannot replace it.
        if let Frame::Targets { cast } | Frame::Mana { cast } | Frame::Payment { cast } =
            self.state.frame.clone()
            && lost_stack.contains(&cast.stack_id)
        {
            self.state.pending = None;
            self.state.frame = Frame::Priority;
            // The departure removes the unfinished spell, but does not undo
            // legal casting-time mana activations. Those interrupt CR 117.4's
            // consecutive pass round even if their controller has now left.
            self.state.passes = if cast.mana_activations.is_empty() {
                cast.prior_passes
                    .into_iter()
                    .filter(|p| live.contains(p))
                    .collect()
            } else {
                vec![]
            };
        }
        if let Frame::Targets { cast } | Frame::Mana { cast } | Frame::Payment { cast } =
            self.state.frame.clone()
        {
            let targets_bad = self.state.stack.last().is_some_and(|s| {
                !s.targets.is_empty() && s.targets.iter().any(|t| !self.target_legal(*t))
            });
            let pending_bad=self.state.pending.as_ref().is_some_and(|p|matches!(&p.response,Response::Targets{objects} if objects.iter().any(|t|!self.target_legal(*t))));
            let no_domain = !self.state.objects.values().any(|o| self.target_legal(o.id));
            if targets_bad || pending_bad || no_domain {
                self.reverse_departure_cast(cast)?;
            }
        }
        // A departure can invalidate an accepted mana activation before its
        // rule step. Keep the input journal, discard only the stale proposal.
        if matches!(self.state.frame, Frame::Mana { .. })
            && self
                .state
                .pending
                .as_ref()
                .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
        {
            self.state.pending = None;
        }
        if matches!(self.state.frame, Frame::Priority)
            && self.state.pending.is_none()
            && self.all_live_passed()
        {
            self.begin_resolution()?;
        }
        self.stabilize()
    }
    pub(super) fn replay_to(
        prepared: Prepared,
        journal: &ReplayJournal,
        boundary: Option<u64>,
        mode: TraceMode,
    ) -> Result<Self, String> {
        let mut game = Self::with_trace(prepared, mode);
        let mut external = 0;
        for index in 0..=journal.decisions.len() {
            while external < journal.concessions.len()
                && journal.concessions[external].after_decisions == index as u64
            {
                let input = &journal.concessions[external];
                if boundary.is_some_and(|limit| input.boundary > limit) {
                    return Err("external input exceeds snapshot boundary".into());
                }
                let remaining = input
                    .boundary
                    .checked_sub(game.state.rule_steps)
                    .ok_or("external input boundaries are not ordered")?;
                if remaining > 0 {
                    let _ = game.advance(Budget::new(remaining));
                }
                if game.state.rule_steps != input.boundary {
                    return Err("external boundary is unreachable".into());
                }
                game.concede(input.player)?;
                if game.state.external_inputs.last() != Some(input) {
                    return Err("external ordering mismatch".into());
                }
                external += 1;
            }
            if let Some(input) = journal.decisions.get(index) {
                let budget = match boundary {
                    Some(limit) => Budget::new(
                        limit
                            .checked_sub(game.state.rule_steps)
                            .ok_or("snapshot boundary mismatch")?,
                    ),
                    None => Budget::unlimited(),
                };
                let Advance::AwaitingDecision(request) = game.advance(budget) else {
                    return Err("replay decision is unreachable".into());
                };
                if request.player() != input.player || request.decision_id() != input.decision_id {
                    return Err("replay decision mismatch".into());
                }
                game.respond(input.player, input.decision_id, input.response.clone())?;
            }
        }
        if external != journal.concessions.len() {
            return Err("external input count/order mismatch".into());
        }
        let budget = match boundary {
            Some(limit) => Budget::new(
                limit
                    .checked_sub(game.state.rule_steps)
                    .ok_or("snapshot boundary mismatch")?,
            ),
            None => Budget::unlimited(),
        };
        let outcome = game.advance(budget);
        if boundary.is_none()
            && let Advance::Fault(reason) = outcome
        {
            return Err(reason);
        }
        Ok(game)
    }
}
