use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommanderHistory {
    pub owner: PlayerId,
    pub color_identity: Vec<Color>,
    #[serde(deserialize_with = "player_quantity_map")]
    pub command_casts: BTreeMap<PlayerId, Quantity>,
    #[serde(deserialize_with = "player_quantity_map")]
    pub combat_damage: BTreeMap<PlayerId, Quantity>,
}
// JSON object keys are strings. Internally tagged publication/IPC enums
// deserialize buffered Content, which cannot decode a string as the numeric
// scalar inside PlayerId. Adapt these map KEYS only; PlayerId values remain
// numeric. Serialization and JsonSchema still use the original field types.
fn player_quantity_map<'de, D>(deserializer: D) -> Result<BTreeMap<PlayerId, Quantity>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct PlayerQuantities;
    impl<'de> serde::de::Visitor<'de> for PlayerQuantities {
        type Value = BTreeMap<PlayerId, Quantity>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with canonical decimal u8 player keys")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some((key, quantity)) = map.next_entry::<String, Quantity>()? {
                let player = key
                    .parse::<u8>()
                    .map_err(|_| serde::de::Error::custom("invalid player history map key"))?;
                if key != player.to_string() {
                    return Err(serde::de::Error::custom(
                        "noncanonical player history map key",
                    ));
                }
                if result.insert(PlayerId(player), quantity).is_some() {
                    return Err(serde::de::Error::custom("duplicate player history map key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(PlayerQuantities)
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
    // Only completed casting calls this hook; reversal never advances tax.
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
    // Only the typed actual combat-damage path records production history.
    // The boolean also lets private domain tests prove noncombat exclusion.
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
        if *actual == Quantity::zero() {
            return Ok(());
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
                owner: self.state.rules.cards[&card].owner,
                color_identity: d.characteristics.color_identity.clone(),
                command_casts: BTreeMap::new(),
                combat_damage: BTreeMap::new(),
            };
            self.state.rules.commanders.records.insert(card, history);
        }
        Ok(())
    }
    pub fn commander_ledger(&self) -> &CommanderLedger {
        &self.state.rules.commanders
    }
    pub fn live_players(&self) -> Vec<PlayerId> {
        self.state
            .rules
            .original_seating
            .iter()
            .copied()
            .filter(|p| self.is_live(*p))
            .collect()
    }
    pub fn active_player(&self) -> Option<PlayerId> {
        self.state.rules.active
    }
    pub fn turn_owner(&self) -> PlayerId {
        self.state.rules.turn_owner
    }
    pub fn replay_journal(&self) -> ReplayJournal {
        ReplayJournal {
            decisions: self.state.audit.journal.iter().cloned().collect(),
            concessions: self.state.audit.external_inputs.iter().cloned().collect(),
        }
    }
    pub(super) fn is_live(&self, player: PlayerId) -> bool {
        self.state
            .rules
            .players
            .get(player.0 as usize)
            .is_some_and(|p| p.status == PlayerStatus::Live)
            && !self.state.rules.losing_players.contains(&player)
    }
    pub(super) fn next_live(&self, after: PlayerId) -> Option<PlayerId> {
        (1..=4)
            .map(|n| PlayerId((after.0 + n) % 4))
            .find(|p| self.is_live(*p))
    }
    pub(super) fn all_live_passed(&self) -> bool {
        self.live_players()
            .iter()
            .all(|p| self.state.rules.passes.contains(p))
    }
    pub(super) fn begin_resolution(&mut self) -> Result<(), String> {
        self.state.rules.passes.clear();
        if self.state.rules.stack.is_empty() {
            self.state.rules.frame = Frame::Transition;
            return Ok(());
        }
        let stack = self.top()?.clone();
        let condition_check = self.check_resolution_condition(&stack)?;
        let legal_targets = self.announced_target_legality(&stack);
        self.state.rules.frame = Frame::Resolve {
            frame: Resolving {
                damage_draw: None,
                set_damage: None,
                target_facts: self.capture_target_facts(&stack, &legal_targets)?,
                condition_check,
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
            || self.state.rules.pending_concessions.contains(&player)
            || matches!(
                self.state.rules.frame,
                Frame::Finished { .. } | Frame::Draw | Frame::Fault { .. }
            )
            || self
                .live_players()
                .len()
                .saturating_sub(self.state.rules.pending_concessions.len())
                <= 1
        {
            return Err("concession is stale or game has already ended".into());
        }
        let revision = self
            .state
            .rules
            .revision
            .checked_add(1)
            .ok_or("revision resource limit")?;
        let mut next = self.fork();
        next.state.audit.external_inputs.push(ExternalInput {
            after_decisions: u64::try_from(next.state.audit.journal.len())
                .map_err(|_| "input count resource limit")?,
            boundary: next.state.rules.rule_steps,
            player,
        });
        next.state.rules.retained_auxiliary_charge =
            next.state.rules.retained_auxiliary_charge.saturating_add(
                stored_history::payload_charge(next.state.audit.external_inputs.last().unwrap()),
            );
        next.state.rules.pending_concessions.push(player);
        next.state.rules.revision = revision;
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
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Battlefield)
            .cloned()
            .collect::<Vec<_>>();
        for object in objects {
            let controller = self
                .state
                .rules
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
                    .rules
                    .objects
                    .get_mut(&object.id.card)
                    .ok_or("missing control object")?
                    .controller = controller;
                self.remove_from_combat(object.id);
                self.state
                    .rules
                    .objects
                    .get_mut(&object.id.card)
                    .ok_or("missing control object")?
                    .attack_ready = false;
                self.record_event(Event::ControlChanged {
                    object: object.id,
                    previous: object.controller,
                    controller,
                })?;
            }
        }
        Ok(())
    }
    fn begin_departure(&mut self, player: PlayerId) -> Result<Vec<ObjectId>, String> {
        if self
            .state
            .rules
            .players
            .get(player.0 as usize)
            .is_none_or(|p| p.status != PlayerStatus::Live)
        {
            return Err("duplicate departure".into());
        }
        let last = PlayerLastKnown {
            life: self.state.rules.players[player.0 as usize].life.clone(),
            mana: self.state.rules.players[player.0 as usize].mana.clone(),
        };
        self.state.rules.players[player.0 as usize].last_known = Some(last);
        self.state.rules.players[player.0 as usize].status = PlayerStatus::Left;
        self.record_event(Event::PlayerDeparted { player })?;
        // Owned objects leave, not a graveyard move and not an SBA (800.4a).
        let removed = self
            .state
            .rules
            .objects
            .values()
            .filter(|o| self.state.rules.cards[&o.id.card].owner == player)
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
        Ok(removed)
    }
    pub(super) fn apply_departure(&mut self, player: PlayerId) -> Result<(), String> {
        let removed = self.begin_departure(player)?;
        self.finish_departure(player, &removed)
    }
    fn finish_departure(&mut self, player: PlayerId, removed: &[ObjectId]) -> Result<(), String> {
        let ended = self
            .state
            .rules
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
            .rules
            .control_effects
            .retain(|e| !ended.contains(&e.id));
        for id in ended {
            self.record_event(Event::ControlEffectEnded { effect_id: id })?;
        }
        self.refresh_controllers(player)?;
        // A departure can remove a legend or change a controller while choices
        // are pending. Recompute the entire SBA wave; no stale keep survives.
        if let Frame::LegendRule { resume, .. } = self.state.rules.frame.clone() {
            self.state.rules.pending = None;
            self.state.rules.frame = *resume;
        }
        let lost_stack = self
            .state
            .rules
            .stack
            .iter()
            .filter(|s| {
                if s.kind.is_spell() {
                    self.current(s.source).is_err()
                } else {
                    !self.is_live(s.controller)
                }
            })
            .map(|s| s.id)
            .collect::<Vec<_>>();
        self.state
            .rules
            .stack
            .retain(|s| !lost_stack.contains(&s.id));
        self.revalidate_trigger_departure()?;
        self.revalidate_damage_departure()?;
        for stack_id in &lost_stack {
            self.record_event(Event::StackRemovedForDeparture {
                stack_id: *stack_id,
                player,
            })?;
        }
        if self.state.rules.active == Some(player) {
            self.state.rules.active = None;
        }
        if self.state.rules.priority == player {
            if let Some(next) = self.next_live(player) {
                self.state.rules.priority = next;
            } else if !self.state.rules.simultaneous_losses {
                return Err("no remaining player".into());
            }
        }
        if self
            .state
            .rules
            .pending
            .as_ref()
            .is_some_and(|p| p.player == player)
        {
            self.state.rules.pending = None;
        }
        if matches!(self.state.rules.frame, Frame::Setup) {
            self.revalidate_setup_departure()?;
        }
        let live = self.live_players();
        self.state.rules.passes.retain(|p| live.contains(p));
        if !self.state.rules.simultaneous_losses && self.finish_if_ended()? {
            return Ok(());
        }
        self.revalidate_mana_departure()?;
        // CR 608.2m: a started invocation owns its frozen instance and keeps
        // resolving after its source leaves. Only an unfinished casting frame
        // whose own source left is discarded; lower departures cannot replace it.
        if let Frame::AnnounceX { cast }
        | Frame::Modes { cast }
        | Frame::EntityTargets { cast }
        | Frame::Targets { cast }
        | Frame::StackTargets { cast }
        | Frame::PlayerTargets { cast }
        | Frame::Mana { cast }
        | Frame::SacrificeCost { cast }
        | Frame::SpendMana { cast }
        | Frame::Payment { cast } = self.state.rules.frame.clone()
            && lost_stack.contains(&cast.stack_id)
        {
            self.state.rules.pending = None;
            self.state.rules.frame = Frame::Priority;
            // The departure removes the unfinished spell, but does not undo
            // legal casting-time mana activations. Those interrupt CR 117.4's
            // consecutive pass round even if their controller has now left.
            self.state.rules.passes = if cast.mana_activations.is_empty() {
                cast.prior_passes
                    .into_iter()
                    .filter(|p| live.contains(p))
                    .collect()
            } else {
                vec![]
            };
        }
        if let Frame::AnnounceX { cast }
        | Frame::Modes { cast }
        | Frame::EntityTargets { cast }
        | Frame::Targets { cast }
        | Frame::StackTargets { cast }
        | Frame::PlayerTargets { cast }
        | Frame::Mana { cast }
        | Frame::SacrificeCost { cast }
        | Frame::SpendMana { cast }
        | Frame::Payment { cast } = self.state.rules.frame.clone()
        {
            let targets_bad = self.state.rules.stack.last().is_some_and(|s| {
                self.bound_target_count(s) != 0
                    && self
                        .announced_target_legality(s)
                        .iter()
                        .any(|legal| !*legal)
            });
            let pending_bad =
                self.state
                    .rules
                    .pending
                    .as_ref()
                    .is_some_and(|p| match &p.response {
                        Response::EntityTargets { entities } => {
                            self.validate_entity_targets(p.player, entities).is_err()
                        }
                        // Graveyard-card targets (Raise Dead) use their own domain.
                        Response::Targets { objects } => self.top().map_or(true, |top| {
                            objects.iter().any(|t| !self.object_target_legal(top, *t))
                        }),
                        Response::StackTargets { entries } => {
                            self.validate_stack_targets(p.player, entries).is_err()
                        }
                        Response::PlayerTargets { players } => {
                            players.iter().enumerate().any(|(slot, target)| {
                                !self.player_target_legal(self.top().unwrap(), slot, *target)
                            })
                        }
                        _ => false,
                    });
            let no_domain = !self.has_target_domain(self.program()?, self.top()?.controller);
            let source_bad = cast.origin == AnnouncementOrigin::Activated
                && (matches!(self.state.rules.frame, Frame::EntityTargets { .. })
                    || matches!(self.top()?.kind, StackKind::Activated {ability,..} if self.top().is_ok_and(|top| self.definition(top.source.card).is_ok_and(|d| d.activated.get(ability as usize).is_some_and(|a| matches!(a.cost, ActivationCost::Composite {..}))))))
                && self.current(self.top()?.source).is_err();
            if targets_bad || pending_bad || no_domain || source_bad {
                self.reverse_departure_cast(cast)?;
            }
        }
        // A departure can invalidate an accepted priority cast or mana
        // activation before its rule step. Keep the input journal, discard
        // only the stale proposal, and leave valid pending actions intact.
        if matches!(
            self.state.rules.frame,
            Frame::Priority
                | Frame::Mana { .. }
                | Frame::SacrificeCost { .. }
                | Frame::SpendMana { .. }
                | Frame::ManaAbility { .. }
        ) && self
            .state
            .rules
            .pending
            .as_ref()
            .is_some_and(|p| self.validate_response(p.player, &p.response).is_err())
        {
            self.state.rules.pending = None;
        }
        // Drain boundary-ordered concessions before ordinary rules work:
        // later departures may finish the game or change target legality.
        if matches!(self.state.rules.frame, Frame::Priority)
            && self.state.rules.pending.is_none()
            && self.state.rules.pending_concessions.is_empty()
            && self.all_live_passed()
        {
            self.begin_resolution()?;
        }
        self.revalidate_commander_departure()?;
        self.revalidate_actions_departure()?;
        if !self.state.rules.simultaneous_losses {
            self.revalidate_combat_departure()?;
        }
        self.stabilize()
    }
    pub(super) fn apply_simultaneous_losses(&mut self, players: &[PlayerId]) -> Result<(), String> {
        self.state.rules.simultaneous_losses = true;
        self.state.rules.losing_players = players.to_vec();
        let mut removed = vec![];
        // 800.4a: every losing owner's objects leave before control grants end.
        // Frozen doomed membership prevents temporary control by another loser.
        for player in players {
            removed.push((*player, self.begin_departure(*player)?));
        }
        let ended=self.state.rules.control_effects.iter().filter(|e|!self.is_live(e.controller) || self.current(e.object).is_err() || matches!(e.source,ControlSource::Static {object} if self.current(object).is_err())).map(|e|e.id).collect::<Vec<_>>();
        self.state
            .rules
            .control_effects
            .retain(|e| !ended.contains(&e.id));
        for effect_id in ended {
            self.record_event(Event::ControlEffectEnded { effect_id })?;
        }
        for (player, objects) in removed {
            self.finish_departure(player, &objects)?;
        }
        self.state.rules.simultaneous_losses = false;
        self.state.rules.losing_players.clear();
        Ok(())
    }
    pub(super) fn replay_to(
        prepared: Prepared,
        journal: &ReplayJournal,
        boundary: Option<u64>,
        mode: TraceMode,
    ) -> Result<Self, String> {
        let mut game = Self::with_trace(prepared, mode);
        game.extend_replay(journal, boundary)?;
        Ok(game)
    }
    // Extend only a proved prefix. Accepted-but-pending inputs and queued
    // concessions belong to that prefix even when the rule boundary is unchanged.
    pub(super) fn extend_replay(
        &mut self,
        journal: &ReplayJournal,
        boundary: Option<u64>,
    ) -> Result<(), String> {
        if !self.state.audit.journal.is_prefix_of(&journal.decisions)
            || !self
                .state
                .audit
                .external_inputs
                .is_prefix_of(&journal.concessions)
            || boundary.is_some_and(|limit| limit < self.state.rules.rule_steps)
        {
            return Err("snapshot does not extend the verified replay prefix".into());
        }
        let game = self;
        let mut external = game.state.audit.external_inputs.len();
        for index in game.state.audit.journal.len()..=journal.decisions.len() {
            while external < journal.concessions.len()
                && journal.concessions[external].after_decisions == index as u64
            {
                let input = &journal.concessions[external];
                if boundary.is_some_and(|limit| input.boundary > limit) {
                    return Err("external input exceeds snapshot boundary".into());
                }
                let remaining = input
                    .boundary
                    .checked_sub(game.state.rules.rule_steps)
                    .ok_or("external input boundaries are not ordered")?;
                if remaining > 0 {
                    let _ = game.advance(Budget::new(remaining));
                }
                if game.state.rules.rule_steps != input.boundary {
                    return Err("external boundary is unreachable".into());
                }
                game.concede(input.player)?;
                if game.state.audit.external_inputs.last() != Some(input) {
                    return Err("external ordering mismatch".into());
                }
                external += 1;
            }
            if let Some(input) = journal.decisions.get(index) {
                let budget = match boundary {
                    Some(limit) => Budget::new(
                        limit
                            .checked_sub(game.state.rules.rule_steps)
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
                    .checked_sub(game.state.rules.rule_steps)
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
        Ok(())
    }
}

#[cfg(test)]
mod history_key_tests {
    use super::*;
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum Tagged {
        History { history: CommanderHistory },
    }
    fn history() -> CommanderHistory {
        CommanderHistory {
            owner: PlayerId(2),
            color_identity: vec![Color::W],
            command_casts: [(PlayerId(2), 1.into()), (PlayerId(255), 3.into())].into(),
            combat_damage: [(PlayerId(0), 22.into()), (PlayerId(3), 2.into())].into(),
        }
    }
    #[test]
    fn tagged_and_value_history_keys_roundtrip_with_identical_canonical_bytes() {
        let expected = Tagged::History { history: history() };
        let text = mtg_contracts::canonical(&expected).unwrap();
        assert_eq!(
            text,
            r#"{"history":{"color_identity":["W"],"combat_damage":{"0":"22","3":"2"},"command_casts":{"2":"1","255":"3"},"owner":2},"kind":"history"}"#
        );
        let direct: Tagged = serde_json::from_str(&text).unwrap();
        let buffered: Tagged =
            serde_json::from_value(serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(direct, expected);
        assert_eq!(buffered, expected);
        assert_eq!(mtg_contracts::canonical(&buffered).unwrap(), text);
        assert!(serde_json::from_value::<PlayerId>(serde_json::json!("2")).is_err());
    }
    #[test]
    fn history_map_keys_reject_aliases_ranges_duplicates_and_wrong_shapes() {
        let original = mtg_contracts::canonical(&history()).unwrap();
        for key in [
            "", "+2", "02", "-0", "-1", "256", " 2", "2 ", "2.0", "2e0", "２",
        ] {
            let bad = original.replace(
                "\"2\":\"1\"",
                &format!("{}:\"1\"", serde_json::to_string(key).unwrap()),
            );
            assert!(
                serde_json::from_str::<CommanderHistory>(&bad).is_err(),
                "{key}"
            );
        }
        let duplicate = original.replace("\"2\":\"1\"", "\"2\":\"1\",\"2\":\"2\"");
        assert!(serde_json::from_str::<CommanderHistory>(&duplicate).is_err());
        for value in [
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!(2),
        ] {
            let mut bad = serde_json::to_value(history()).unwrap();
            bad["combat_damage"] = value;
            assert!(serde_json::from_value::<CommanderHistory>(bad).is_err());
        }
    }
}
