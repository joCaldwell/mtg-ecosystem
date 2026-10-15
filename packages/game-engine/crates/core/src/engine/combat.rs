use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Attacker {
    pub attacker: ObjectId,
    pub defender: PlayerId,
    pub blocked: bool,
    pub blockers: Vec<ObjectId>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "recipient", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageRecipient {
    Player { player: PlayerId },
    Creature { object: ObjectId },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageOccurrence {
    pub source: ObjectId,
    pub controller: PlayerId,
    pub recipient: DamageRecipient,
    pub amount: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageResult {
    pub recipient: DamageRecipient,
    pub previous: Quantity,
    pub current: Quantity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatureLossReason {
    LethalDamage,
    ZeroToughness,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerLossReason {
    Life,
    FailedDraw,
}

impl Game {
    pub(super) fn combat_enabled(&self) -> bool {
        self.prepared.config.combat_profile()
    }
    pub(super) fn combat_decision_player(&self) -> Option<PlayerId> {
        match &self.state.frame {
            Frame::Attackers => self.state.active,
            Frame::Blockers { remaining } => remaining.first().copied(),
            Frame::DamageChoice { source } => self.current(*source).ok().map(|o| o.controller),
            Frame::Discard { player, .. } => Some(*player),
            _ => None,
        }
    }
    fn creatures(&self, player: PlayerId, attacking: bool) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| {
                o.controller == player
                    && !o.tapped
                    && self.target_legal(o.id)
                    && (!attacking || o.attack_ready)
            })
            .map(|o| o.id)
            .collect()
    }
    fn hand(&self, player: PlayerId) -> Vec<ObjectId> {
        self.state
            .objects
            .values()
            .filter(|o| o.zone == (Zone::Hand { owner: player }))
            .map(|o| o.id)
            .collect()
    }
    fn power(&self, source: ObjectId) -> Result<Quantity, String> {
        Ok(std::cmp::max(
            self.characteristics(source)?
                .power
                .ok_or("missing creature power")?,
            Quantity::zero(),
        ))
    }
    fn recipients(&self, source: ObjectId) -> Vec<ObjectId> {
        self.state
            .combat
            .iter()
            .find(|a| a.attacker == source)
            .map(|a| a.blockers.clone())
            .unwrap_or_default()
    }
    pub(super) fn combat_request(&self) -> Result<Option<Request>, String> {
        let Some(player) = self.combat_decision_player() else {
            return Ok(None);
        };
        let decision_id = self.state.revision;
        Ok(Some(match &self.state.frame {
            Frame::Attackers => Request::Attackers {
                decision_id,
                player,
                candidates: self.creatures(player, true),
                defenders: self
                    .live_players()
                    .into_iter()
                    .filter(|p| *p != player)
                    .collect(),
            },
            Frame::Blockers { .. } => Request::Blockers {
                decision_id,
                player,
                candidates: self.creatures(player, false),
                attackers: self
                    .state
                    .combat
                    .iter()
                    .filter(|a| a.defender == player)
                    .map(|a| a.attacker)
                    .collect(),
            },
            Frame::DamageChoice { source } => Request::Damage {
                decision_id,
                player,
                source: *source,
                power: self.power(*source)?,
                recipients: self.recipients(*source),
            },
            Frame::Discard { count, .. } => Request::Discard {
                decision_id,
                player,
                count: *count,
                candidates: self.hand(player),
            },
            _ => return Ok(None),
        }))
    }
    pub(super) fn validate_combat_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        if self.combat_decision_player() != Some(player) || !self.is_live(player) {
            return Err("wrong combat chooser".into());
        }
        let size = match response {
            Response::Attackers { attacks } => attacks.len(),
            Response::Blockers { blocks } => blocks.len(),
            Response::Damage { assignments } => assignments.len(),
            Response::Discard { objects } => objects.len(),
            _ => 0,
        };
        if size > self.state.objects.len() {
            return Err("oversized complete declaration".into());
        }
        match (&self.state.frame, response) {
            (Frame::Attackers, Response::Attackers { attacks }) => {
                let eligible = self.creatures(player, true);
                let mut seen = BTreeSet::new();
                if attacks.iter().any(|(o, p)| {
                    !seen.insert(*o) || !eligible.contains(o) || *p == player || !self.is_live(*p)
                }) {
                    return Err("invalid complete attack declaration".into());
                }
            }
            (Frame::Blockers { .. }, Response::Blockers { blocks }) => {
                let eligible = self.creatures(player, false);
                let mut seen = BTreeSet::new();
                if blocks.iter().any(|(b, a)| {
                    !seen.insert(*b)
                        || !eligible.contains(b)
                        || !self
                            .state
                            .combat
                            .iter()
                            .any(|x| x.attacker == *a && x.defender == player)
                }) {
                    return Err("invalid complete block declaration".into());
                }
            }
            (Frame::DamageChoice { source }, Response::Damage { assignments }) => {
                self.validate_split(*source, assignments)?
            }
            (Frame::Discard { count, .. }, Response::Discard { objects }) => {
                let hand = self.hand(player);
                if objects.len() != *count
                    || objects.iter().collect::<BTreeSet<_>>().len() != *count
                    || objects.iter().any(|o| !hand.contains(o))
                {
                    return Err("invalid complete cleanup discard".into());
                }
            }
            _ => return Err("wrong combat response".into()),
        }
        Ok(())
    }
    fn validate_split(
        &self,
        source: ObjectId,
        assignments: &[(ObjectId, Quantity)],
    ) -> Result<(), String> {
        let recipients = self.recipients(source);
        let mut seen = BTreeSet::new();
        let mut sum = Quantity::zero();
        for (o, n) in assignments {
            if !seen.insert(*o) || !recipients.contains(o) || n.is_negative() {
                return Err("invalid damage recipient or quantity".into());
            }
            sum.add(n);
        }
        // CR 510.1c (2026-09-25): no assignment order, and no lethal-first rule.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if seen.len() != recipients.len() || sum != self.power(source)? {
            return Err("incomplete damage distribution".into());
        }
        Ok(())
    }
    pub(super) fn remove_from_combat(&mut self, object: ObjectId) {
        self.state.combat.retain(|a| a.attacker != object);
        for a in &mut self.state.combat {
            a.blockers.retain(|b| *b != object);
        }
        self.state.damage_choices.remove(&object);
    }
    pub(super) fn grant_turn_priority(&mut self) -> Result<(), String> {
        if matches!(self.state.frame, Frame::Finished { .. } | Frame::Draw) {
            return Ok(());
        }
        self.state.priority = self
            .state
            .active
            .or_else(|| self.next_live(self.state.turn_owner))
            .ok_or("no priority recipient")?;
        self.state.frame = Frame::Priority;
        self.stabilize()
    }
    pub(super) fn schedule_stabilization(&mut self, resume: Frame, cleanup: bool) {
        self.state.frame = Frame::Stabilize {
            resume: Box::new(resume),
            cleanup,
            changed: false,
        };
    }
    pub(super) fn combat_step(&mut self) -> Result<(), String> {
        match self.state.frame.clone() {
            Frame::Attackers => {
                let input = self.state.pending.take().ok_or("missing attackers")?;
                self.validate_combat_response(input.player, &input.response)?;
                let Response::Attackers { attacks } = input.response else {
                    return Err("attack response".into());
                };
                self.commit(
                    ProposedEvent::DeclareAttackers { attacks },
                    BatchCause::Combat,
                )?;
                self.grant_turn_priority()?;
            }
            Frame::Blockers { mut remaining } => {
                let input = self.state.pending.take().ok_or("missing blockers")?;
                self.validate_combat_response(input.player, &input.response)?;
                let Response::Blockers { blocks } = input.response else {
                    return Err("block response".into());
                };
                self.commit(
                    ProposedEvent::DeclareBlockers {
                        player: input.player,
                        blocks,
                    },
                    BatchCause::Combat,
                )?;
                remaining.remove(0);
                if remaining.is_empty() {
                    self.grant_turn_priority()?;
                } else {
                    self.state.frame = Frame::Blockers { remaining };
                }
            }
            Frame::DamageChoice { source } => {
                let input = self
                    .state
                    .pending
                    .take()
                    .ok_or("missing damage assignment")?;
                self.validate_combat_response(input.player, &input.response)?;
                let Response::Damage { assignments } = input.response else {
                    return Err("damage response".into());
                };
                self.state.damage_choices.insert(source, assignments);
                self.state.frame = Frame::Damage;
                self.assign_damage()?;
            }
            Frame::Damage => self.assign_damage()?,
            Frame::Discard { player, .. } => {
                let input = self.state.pending.take().ok_or("missing discard")?;
                self.validate_combat_response(input.player, &input.response)?;
                let Response::Discard { objects } = input.response else {
                    return Err("discard response".into());
                };
                self.commit_batch(
                    BatchCause::Cleanup,
                    EventGroup::Simultaneous {
                        occurrences: objects
                            .into_iter()
                            .map(|object| ProposedEvent::Move {
                                object,
                                destination: Zone::Graveyard { owner: player },
                            })
                            .collect(),
                    },
                )?;
                self.state.frame = Frame::CleanupClear;
            }
            Frame::CleanupClear => {
                self.commit(ProposedEvent::ClearDamage, BatchCause::Cleanup)?;
                self.schedule_stabilization(Frame::Transition, true);
            }
            Frame::Stabilize {
                resume,
                cleanup,
                changed,
            } => self.stabilization_wave(*resume, cleanup, changed)?,
            _ => return Err("invalid combat continuation".into()),
        }
        Ok(())
    }
    fn assign_damage(&mut self) -> Result<(), String> {
        let mut damage = vec![];
        for a in self.state.combat.clone() {
            let power = self.power(a.attacker)?;
            let controller = self.current(a.attacker)?.controller;
            if power > Quantity::zero() {
                if a.blockers.len() > 1 {
                    let choice = self
                        .state
                        .damage_choices
                        .get(&a.attacker)
                        .filter(|v| self.validate_split(a.attacker, v).is_ok());
                    let Some(choice) = choice else {
                        self.state.damage_choices.remove(&a.attacker);
                        self.state.frame = Frame::DamageChoice { source: a.attacker };
                        return Ok(());
                    };
                    for (object, amount) in choice {
                        if *amount > Quantity::zero() {
                            damage.push(DamageOccurrence {
                                source: a.attacker,
                                controller,
                                recipient: DamageRecipient::Creature { object: *object },
                                amount: amount.clone(),
                            });
                        }
                    }
                } else if let Some(object) = a.blockers.first() {
                    damage.push(DamageOccurrence {
                        source: a.attacker,
                        controller,
                        recipient: DamageRecipient::Creature { object: *object },
                        amount: power,
                    });
                } else if !a.blocked && self.is_live(a.defender) {
                    damage.push(DamageOccurrence {
                        source: a.attacker,
                        controller,
                        recipient: DamageRecipient::Player { player: a.defender },
                        amount: power,
                    });
                }
            }
            for b in a.blockers {
                let power = self.power(b)?;
                if power > Quantity::zero() {
                    damage.push(DamageOccurrence {
                        source: b,
                        controller: self.current(b)?.controller,
                        recipient: DamageRecipient::Creature { object: a.attacker },
                        amount: power,
                    });
                }
            }
        }
        if !damage.is_empty() {
            self.commit(ProposedEvent::CombatDamage { damage }, BatchCause::Combat)?;
        }
        self.state.damage_choices.clear();
        self.grant_turn_priority()
    }
    fn stabilization_wave(
        &mut self,
        resume: Frame,
        cleanup: bool,
        changed: bool,
    ) -> Result<(), String> {
        let mut occurrences = vec![];
        for o in self
            .state
            .objects
            .values()
            .filter(|o| self.target_legal(o.id))
        {
            let toughness = self
                .characteristics(o.id)?
                .toughness
                .ok_or("missing toughness")?;
            let reason = if toughness <= Quantity::zero() {
                Some(CreatureLossReason::ZeroToughness)
            } else if o.marked_damage >= toughness {
                Some(CreatureLossReason::LethalDamage)
            } else {
                None
            };
            if let Some(reason) = reason {
                occurrences.push(ProposedEvent::StateBasedGraveyard {
                    object: o.id,
                    reason,
                });
            }
        }
        let losses = self
            .live_players()
            .into_iter()
            .filter_map(|p| {
                let x = &self.state.players[p.0 as usize];
                if x.life <= Quantity::zero() {
                    Some((p, PlayerLossReason::Life))
                } else if x.failed_draw {
                    Some((p, PlayerLossReason::FailedDraw))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if occurrences.is_empty()
            && losses.is_empty()
            && self.offer_commander_sba(Frame::Stabilize {
                resume: Box::new(resume.clone()),
                cleanup,
                changed,
            })?
        {
            return Ok(());
        }
        if occurrences.is_empty() && losses.is_empty() {
            self.state.frame = if cleanup && changed {
                self.state.cleanup_repeat = true;
                Frame::Priority
            } else {
                resume
            };
            if cleanup && changed && matches!(self.state.frame, Frame::Priority) {
                self.state.priority = self
                    .state
                    .active
                    .or_else(|| self.next_live(self.state.turn_owner))
                    .ok_or("no stabilization priority")?;
            }
            if matches!(self.state.frame, Frame::Priority)
                && self.state.pending.is_none()
                && self.state.pending_concessions.is_empty()
                && self.all_live_passed()
            {
                self.begin_resolution()?;
            }
            return Ok(());
        }
        // CR 704.3/704.5: capture the whole wave before removal or winner tests.
        // All constituent changes commit in this single atomic rules unit.
        occurrences.extend(losses.iter().map(|(player, reason)| ProposedEvent::Lose {
            player: *player,
            reason: *reason,
        }));
        self.commit_batch(
            BatchCause::StateBased,
            EventGroup::Simultaneous { occurrences },
        )?;
        if !losses.is_empty() {
            self.record_event(Event::PlayersLost {
                losses: losses.clone(),
            })?;
            self.apply_simultaneous_losses(&losses.iter().map(|(p, _)| *p).collect::<Vec<_>>())?;
            if self.finish_if_ended()? {
                return Ok(());
            }
        }
        self.state.frame = Frame::Stabilize {
            resume: Box::new(resume),
            cleanup,
            changed: true,
        };
        Ok(())
    }
    pub(super) fn finish_if_ended(&mut self) -> Result<bool, String> {
        match self.live_players().as_slice() {
            [] => {
                self.state.pending = None;
                self.state.frame = Frame::Draw;
                self.record_event(Event::GameDrawn)?;
                Ok(true)
            }
            [winner] => {
                let winner = *winner;
                self.state.pending = None;
                self.state.frame = Frame::Finished { winner };
                self.state.priority = winner;
                self.record_event(Event::GameWon { winner })?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub(super) fn revalidate_combat_departure(&mut self) -> Result<(), String> {
        match self.state.frame.clone() {
            Frame::Attackers if self.state.active.is_none() => self.grant_turn_priority()?,
            Frame::Blockers { remaining } => {
                let remaining = remaining
                    .into_iter()
                    .filter(|p| self.is_live(*p))
                    .collect::<Vec<_>>();
                if remaining.is_empty() {
                    self.grant_turn_priority()?;
                } else {
                    self.state.frame = Frame::Blockers { remaining };
                }
            }
            Frame::DamageChoice { source }
                if self.current(source).is_err() || self.recipients(source).len() < 2 =>
            {
                self.state.pending = None;
                self.state.frame = Frame::Damage;
            }
            Frame::Discard { player, .. } if !self.is_live(player) => {
                self.state.pending = None;
                self.state.frame = Frame::CleanupClear;
            }
            _ => {}
        }
        if self
            .state
            .pending
            .as_ref()
            .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
        {
            self.state.pending = None;
        }
        Ok(())
    }
    pub(super) fn validate_combat_state(&self) -> Result<(), String> {
        for (p, library) in &self.state.libraries {
            let actual = self
                .state
                .objects
                .values()
                .filter(|o| o.zone == (Zone::Library { owner: *p }))
                .map(|o| o.id.card)
                .collect::<BTreeSet<_>>();
            if library.iter().copied().collect::<BTreeSet<_>>() != actual
                || actual.len() != library.len()
            {
                return Err("library order membership invariant".into());
            }
        }
        if self
            .state
            .objects
            .values()
            .any(|o| o.marked_damage.is_negative())
        {
            return Err("negative damage".into());
        }
        Ok(())
    }
}
