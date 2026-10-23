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
    Deathtouch,
    ZeroToughness,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerLossReason {
    CommanderDamage,
    Life,
    FailedDraw,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderDamageResult {
    pub commander: CardId,
    pub recipient: PlayerId,
    pub amount: Quantity,
    pub previous: Quantity,
    pub current: Quantity,
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
                    && (!attacking
                        || ((o.attack_ready || self.has_keyword(o.id, KeywordAbility::Haste))
                            && !self.has_keyword(o.id, KeywordAbility::Defender)))
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
                player_recipient: self.trample_player(*source),
                lethal_requirements: if self.trample_player(*source).is_some() {
                    self.recipients(*source)
                        .into_iter()
                        .map(|o| Ok((o, self.lethal_requirement(*source, o)?)))
                        .collect::<Result<_, String>>()?
                } else {
                    vec![]
                },
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
            Response::Damage { assignments } | Response::TrampleDamage { assignments, .. } => {
                assignments.len()
            }
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
                        || !self.can_block(*b, *a)
                        || !self
                            .state
                            .combat
                            .iter()
                            .any(|x| x.attacker == *a && x.defender == player)
                }) {
                    return Err("invalid complete block declaration".into());
                }
                // CR509.1b/702.111b: restrictions apply to the whole declaration.
                for a in self.state.combat.iter().filter(|a| a.defender == player) {
                    if self.has_keyword(a.attacker, KeywordAbility::Menace)
                        && blocks.iter().filter(|(_, x)| *x == a.attacker).count() == 1
                    {
                        return Err("menace requires zero or at least two blockers".into());
                    }
                }
            }
            (Frame::DamageChoice { source }, Response::Damage { assignments }) => {
                self.validate_split(*source, assignments, None)?
            }
            (
                Frame::DamageChoice { source },
                Response::TrampleDamage {
                    assignments,
                    player,
                    amount,
                },
            ) => self.validate_split(*source, assignments, Some((*player, amount)))?,
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
        player: Option<(PlayerId, &Quantity)>,
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
        if let Some((p, amount)) = player {
            if self.trample_player(source) != Some(p) || amount.is_negative() {
                return Err("invalid trample player recipient or quantity".into());
            }
            sum.add(amount);
            // CR702.19b/702.2c: ignore prevention/replacement; account for
            // marked damage and other assignments in this same step.
            if *amount > Quantity::zero() {
                for (o, n) in assignments {
                    if *n < self.lethal_requirement(source, *o)? {
                        return Err("trample player damage before lethal to every blocker".into());
                    }
                }
            }
        }
        // CR510.1c/e: no order; validate the entire source distribution.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        if seen.len() != recipients.len() || sum != self.power(source)? {
            return Err("incomplete damage distribution".into());
        }
        Ok(())
    }
    pub(super) fn remove_from_combat(&mut self, object: ObjectId) {
        let orphaned = self
            .state
            .combat
            .iter()
            .filter(|a| a.attacker == object)
            .flat_map(|a| a.blockers.clone())
            .filter(|b| {
                self.has_keyword(*b, KeywordAbility::FirstStrike)
                    || self.has_keyword(*b, KeywordAbility::DoubleStrike)
            })
            .collect::<Vec<_>>();
        self.state.orphan_first_strike_blockers.extend(orphaned);
        self.state.orphan_first_strike_blockers.remove(&object);
        self.state.combat.retain(|a| a.attacker != object);
        for a in &mut self.state.combat {
            a.blockers.retain(|b| *b != object);
        }
        self.state.damage_choices.remove(&object);
        self.state.player_damage_choices.remove(&object);
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
                let assignments = match input.response {
                    Response::Damage { assignments } => {
                        self.state.player_damage_choices.remove(&source);
                        assignments
                    }
                    Response::TrampleDamage {
                        assignments,
                        player,
                        amount,
                    } => {
                        self.state
                            .player_damage_choices
                            .insert(source, (player, amount));
                        assignments
                    }
                    _ => return Err("damage response".into()),
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
                self.commit(
                    if self.prepared.config.effects_profile() {
                        ProposedEvent::CleanupEffects
                    } else {
                        ProposedEvent::ClearDamage
                    },
                    BatchCause::Cleanup,
                )?;
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
    /// CR510.4/702.7b/702.4b. Only this profile can introduce a second step.
    pub(super) fn first_strikers(&self) -> BTreeSet<ObjectId> {
        self.state
            .combat
            .iter()
            .flat_map(|a| std::iter::once(a.attacker).chain(a.blockers.iter().copied()))
            .chain(self.state.orphan_first_strike_blockers.iter().copied())
            .filter(|o| {
                self.has_keyword(*o, KeywordAbility::FirstStrike)
                    || self.has_keyword(*o, KeywordAbility::DoubleStrike)
            })
            .collect()
    }
    fn assigns_in_damage_step(&self, object: ObjectId) -> bool {
        if self.state.turn.step == TurnStep::FirstStrikeDamage {
            self.state.first_strike_participants.contains(&object)
        } else {
            !self.state.first_strike_participants.contains(&object)
                || self.has_keyword(object, KeywordAbility::DoubleStrike)
        }
    }
    pub(super) fn trample_player(&self, source: ObjectId) -> Option<PlayerId> {
        self.state
            .combat
            .iter()
            .find(|a| a.attacker == source)
            .filter(|a| {
                self.has_keyword(source, KeywordAbility::Trample) && self.is_live(a.defender)
            })
            .map(|a| a.defender)
    }
    pub(super) fn lethal_requirement(
        &self,
        source: ObjectId,
        blocker: ObjectId,
    ) -> Result<Quantity, String> {
        let mut remaining = self
            .characteristics(blocker)?
            .toughness
            .ok_or("missing blocker toughness")?;
        remaining.subtract(&self.current(blocker)?.marked_damage);
        // A blocker cannot block several attackers in the admitted pool. Still
        // account for other source assignments in the complete step (702.19b).
        for a in self.state.combat.iter().filter(|a| {
            a.attacker != source
                && a.blockers.contains(&blocker)
                && self.assigns_in_damage_step(a.attacker)
        }) {
            let amount = if let Some(choice) = self.state.damage_choices.get(&a.attacker) {
                choice
                    .iter()
                    .find(|(o, _)| *o == blocker)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_default()
            } else if a.blockers.len() == 1 && self.trample_player(a.attacker).is_none() {
                self.power(a.attacker)?
            } else {
                Quantity::zero()
            };
            if amount > Quantity::zero() && self.has_keyword(a.attacker, KeywordAbility::Deathtouch)
            {
                return Ok(Quantity::zero());
            }
            remaining.subtract(&amount);
        }
        remaining = remaining.max(Quantity::zero());
        if self.has_keyword(source, KeywordAbility::Deathtouch) {
            remaining = remaining.min(Quantity::from(1));
        }
        Ok(remaining)
    }
    fn assign_damage(&mut self) -> Result<(), String> {
        let mut damage = vec![];
        for a in self.state.combat.clone() {
            let power = self.power(a.attacker)?;
            let controller = self.current(a.attacker)?.controller;
            if self.assigns_in_damage_step(a.attacker) && power > Quantity::zero() {
                if a.blockers.len() > 1
                    || (!a.blockers.is_empty() && self.trample_player(a.attacker).is_some())
                {
                    let choice = self.state.damage_choices.get(&a.attacker).filter(|v| {
                        self.validate_split(
                            a.attacker,
                            v,
                            self.state
                                .player_damage_choices
                                .get(&a.attacker)
                                .map(|(p, n)| (*p, n)),
                        )
                        .is_ok()
                    });
                    let Some(choice) = choice else {
                        self.state.damage_choices.remove(&a.attacker);
                        self.state.frame = Frame::DamageChoice { source: a.attacker };
                        return Ok(());
                    };
                    if let Some((player, amount)) =
                        self.state.player_damage_choices.get(&a.attacker)
                        && *amount > Quantity::zero()
                    {
                        damage.push(DamageOccurrence {
                            source: a.attacker,
                            controller,
                            recipient: DamageRecipient::Player { player: *player },
                            amount: amount.clone(),
                        });
                    }
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
                } else if (!a.blocked || self.trample_player(a.attacker).is_some())
                    && self.is_live(a.defender)
                {
                    damage.push(DamageOccurrence {
                        source: a.attacker,
                        controller,
                        recipient: DamageRecipient::Player { player: a.defender },
                        amount: power,
                    });
                }
            }
            for b in a.blockers {
                if !self.assigns_in_damage_step(b) {
                    continue;
                }
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
        if self.prepared.config.effects_profile() && !damage.is_empty() {
            return self.prepare_combat_damage(damage);
        }
        if !damage.is_empty() {
            self.commit(ProposedEvent::CombatDamage { damage }, BatchCause::Combat)?;
        }
        self.state.damage_choices.clear();
        self.state.player_damage_choices.clear();
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
            } else if o.deathtouch_damage {
                Some(CreatureLossReason::Deathtouch)
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
                } else if self.commander_damage_enabled()
                    && self.state.commanders.combat_loss_candidates().contains(&p)
                {
                    Some((p, PlayerLossReason::CommanderDamage))
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
        if occurrences.is_empty()
            && losses.is_empty()
            && self.begin_trigger_placement(resume.clone(), cleanup, changed)?
        {
            return Ok(());
        }
        if occurrences.is_empty() && losses.is_empty() {
            debug_assert!(
                self.state
                    .objects
                    .values()
                    .all(|o| o.zone != Zone::Battlefield || !o.deathtouch_damage)
            );
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
                if self.current(source).is_err()
                    || (self.recipients(source).len() < 2
                        && (self.recipients(source).is_empty()
                            || self.trample_player(source).is_none())) =>
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
        if self.prepared.config.profile != COMBAT_KEYWORDS_PROFILE
            && (self.state.turn.step == TurnStep::FirstStrikeDamage
                || !self.state.first_strike_participants.is_empty()
                || !self.state.orphan_first_strike_blockers.is_empty()
                || !self.state.player_damage_choices.is_empty())
        {
            return Err("combat keyword continuation outside reviewed profile".into());
        }
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
