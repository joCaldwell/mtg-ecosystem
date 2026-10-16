use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "snake_case", deny_unknown_fields)]
pub enum BatchCause {
    Setup,
    AbilityActivation {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
        decision_id: u64,
        ability: u32,
    },
    CommanderReturn,
    Combat,
    Cleanup,
    StateBased,
    LandPlay {
        player: PlayerId,
        decision_id: u64,
        source: ObjectId,
    },
    TurnProgression,
    UntapStep,
    DrawStep {
        player: PlayerId,
    },
    ManaActivation {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
    },
    Casting {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
    },
    Payment {
        stack_id: StackId,
        player: PlayerId,
    },
    CastReversal {
        stack_id: StackId,
        player: PlayerId,
    },
    Instruction {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
        cursor: u32,
        source_block: u32,
    },
    SpellCompletion {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
    },
    Departure {
        player: PlayerId,
    },
    Fixture,
}
impl BatchCause {
    fn rules(&self) -> Vec<String> {
        let rules: &[&str] = match self {
            Self::Setup => &["103.5", "103.5c", "103.8c", "903.6", "903.7"],
            Self::AbilityActivation { .. } => &["602.2", "701.17", "113.7a"],
            Self::CommanderReturn => &["903.9a", "903.9b"],
            Self::Combat => &["508", "509", "510"],
            Self::Cleanup => &["514.1", "514.2"],
            Self::StateBased => &["704.3", "704.5a", "704.5b", "704.5f", "704.5g"],
            Self::LandPlay { .. } => &["305.1", "305.2", "305.3", "400.7"],
            Self::TurnProgression => &["106.4", "500.5", "800.4j"],
            Self::UntapStep => &["502.3", "502.4"],
            Self::DrawStep { .. } => &["504.1", "400.7"],
            Self::ManaActivation { .. } => &["106.3", "305.6", "605.3b"],
            Self::Casting { .. } | Self::Payment { .. } => &["601.2"],
            Self::CastReversal { .. } => &["733.1", "733.2"],
            Self::Instruction { .. } => &["400.7", "608.2"],
            Self::SpellCompletion { .. } => &["400.7", "608.2", "608.3a"],
            Self::Departure { .. } => &["800.4a"],
            Self::Fixture => &["700.1"],
        };
        rules.iter().map(|r| (*r).into()).collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "group", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventGroup {
    Occurrence { proposed: ProposedEvent },
    Ordered { children: Vec<EventGroup> },
    Simultaneous { occurrences: Vec<ProposedEvent> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBefore {
    pub object: ObjectId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub default_controller: PlayerId,
    pub zone: Zone,
    pub tapped: bool,
    pub marked_damage: Quantity,
    pub attack_ready: bool,
    pub characteristics: Characteristics,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerBefore {
    pub failed_draw: bool,
    pub player: PlayerId,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    pub status: PlayerStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreEventFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_state: Option<MatchState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<ResolvedEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_transaction: Option<DamageTransactionBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub damage_sources: Vec<ObjectBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack: Vec<StackBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commander_damage: Vec<CommanderDamageBefore>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<LibraryBefore>,
    pub turn: Option<TurnBefore>,
    pub objects: Vec<ObjectBefore>,
    pub players: Vec<PlayerBefore>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackBefore {
    pub id: StackId,
    pub source: ObjectId,
    pub controller: PlayerId,
    pub spell: bool,
    pub paid: Option<ManaCost>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommanderDamageBefore {
    pub commander: CardId,
    pub recipient: PlayerId,
    pub previous: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryBefore {
    pub owner: PlayerId,
    pub cards: Vec<CardId>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnBefore {
    pub turn: TurnState,
    pub owner: PlayerId,
    pub active: Option<PlayerId>,
    pub priority: Option<PlayerId>,
    pub live_turn_order: Vec<PlayerId>,
    pub stack_empty: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccurrenceId {
    pub batch: u64,
    pub path: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccurrenceResult {
    pub id: OccurrenceId,
    pub before: PreEventFacts,
    pub proposed: ProposedEvent,
    pub actual: EventOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedBatch {
    pub batch_id: u64,
    pub cause: BatchCause,
    pub rules: Vec<String>,
    pub group: EventGroup,
    pub before: PreEventFacts,
    pub occurrences: Vec<OccurrenceResult>,
}
impl CommittedBatch {
    pub fn result(&self, path: &[u32]) -> Option<&EventOutcome> {
        self.occurrences
            .iter()
            .find(|r| r.id.path == path)
            .map(|r| &r.actual)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FactKey {
    Stack(StackId),
    Turn,
    Object(CardId),
    Mana(PlayerId),
    Life(PlayerId),
    CommanderDamage(PlayerId),
    Libraries,
    AllDamage,
    Effects,
}
fn access(event: &ProposedEvent) -> (BTreeSet<FactKey>, BTreeSet<FactKey>) {
    match event {
        ProposedEvent::Setup { .. } => {
            let keys = BTreeSet::from([FactKey::Libraries, FactKey::Turn]);
            (keys.clone(), keys)
        }
        ProposedEvent::AllocateDamageEffect { .. }
        | ProposedEvent::CreateEffect { .. }
        | ProposedEvent::ApplyDamageEffect { .. }
        | ProposedEvent::CommitDamage { .. }
        | ProposedEvent::CleanupEffects => {
            let keys = BTreeSet::from([
                FactKey::Effects,
                FactKey::AllDamage,
                FactKey::Turn,
                FactKey::Libraries,
            ]);
            (keys.clone(), keys)
        }
        ProposedEvent::GainLife { player, .. } | ProposedEvent::LoseLife { player, .. } => {
            let keys = BTreeSet::from([FactKey::Life(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::NoncombatDamage { damage } => {
            let read = BTreeSet::from([FactKey::Object(damage.source.object.card)]);
            let write = BTreeSet::from([match damage.recipient {
                DamageRecipient::Player { player } => FactKey::Life(player),
                DamageRecipient::Creature { object } => FactKey::Object(object.card),
            }]);
            (read.union(&write).copied().collect(), write)
        }
        ProposedEvent::CounterStackObject { target, .. } => {
            let keys = BTreeSet::from([FactKey::Stack(*target)]);
            (keys.clone(), keys)
        }
        ProposedEvent::Lose { player, reason } => {
            let mut keys = BTreeSet::from([FactKey::Life(*player)]);
            if *reason == PlayerLossReason::CommanderDamage {
                keys.insert(FactKey::CommanderDamage(*player));
            }
            (keys.clone(), keys)
        }
        ProposedEvent::DeclareAttackers { attacks } => {
            let mut keys = BTreeSet::from([FactKey::Turn]);
            keys.extend(attacks.iter().map(|(o, _)| FactKey::Object(o.card)));
            (keys.clone(), keys)
        }
        ProposedEvent::DeclareBlockers { blocks, .. } => {
            let mut keys = BTreeSet::from([FactKey::Turn]);
            for (b, a) in blocks {
                keys.insert(FactKey::Object(b.card));
                keys.insert(FactKey::Object(a.card));
            }
            (keys.clone(), keys)
        }
        ProposedEvent::CombatDamage { damage } => {
            let mut keys = BTreeSet::new();
            for d in damage {
                keys.insert(FactKey::Object(d.source.card));
                match d.recipient {
                    DamageRecipient::Player { player } => {
                        keys.insert(FactKey::Life(player));
                        keys.insert(FactKey::CommanderDamage(player));
                    }
                    DamageRecipient::Creature { object } => {
                        keys.insert(FactKey::Object(object.card));
                    }
                }
            }
            (keys.clone(), keys)
        }
        ProposedEvent::FailedDraw { player } => {
            let keys = BTreeSet::from([FactKey::Life(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::ClearDamage => {
            let keys = BTreeSet::from([FactKey::AllDamage]);
            (keys.clone(), keys)
        }
        ProposedEvent::StateBasedGraveyard { object, .. } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::Sacrifice { object, player }
        | ProposedEvent::DiscardCard { object, player } => {
            let keys = BTreeSet::from([FactKey::Object(object.card), FactKey::Life(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::PutOnTopOwnerLibrary { object } => {
            let keys = BTreeSet::from([FactKey::Object(object.card), FactKey::Libraries]);
            (keys.clone(), keys)
        }
        ProposedEvent::Move {
            object,
            destination: Zone::Library { .. },
        } => {
            let keys = BTreeSet::from([FactKey::Object(object.card), FactKey::Libraries]);
            (keys.clone(), keys)
        }
        ProposedEvent::Move { object, .. } | ProposedEvent::LeaveGame { object, .. } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (
                BTreeSet::from([FactKey::Object(object.card), FactKey::Libraries]),
                keys,
            )
        }
        ProposedEvent::Untap { object } | ProposedEvent::Tap { object } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::AddMana { player, source, .. } => (
            BTreeSet::from([FactKey::Object(source.card), FactKey::Mana(*player)]),
            BTreeSet::from([FactKey::Mana(*player)]),
        ),
        ProposedEvent::ClearMana { player }
        | ProposedEvent::PayMana { player, .. }
        | ProposedEvent::PayManaSelected { player, .. } => {
            let keys = BTreeSet::from([FactKey::Mana(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::LandPlayed { .. } | ProposedEvent::TurnTransition { .. } => {
            let keys = BTreeSet::from([FactKey::Turn]);
            (keys.clone(), keys)
        }
        ProposedEvent::ReverseMana { player, source, .. } => {
            let mut keys = BTreeSet::from([FactKey::Mana(*player)]);
            if let Some(object) = source {
                keys.insert(FactKey::Object(object.card));
            }
            (keys.clone(), keys)
        }
    }
}
fn collect<'a>(
    group: &'a EventGroup,
    depth: u32,
    limit: usize,
    out: &mut Vec<&'a ProposedEvent>,
) -> Result<(), String> {
    if depth > 4 {
        return Err("event group depth resource limit".into());
    }
    match group {
        EventGroup::Occurrence { proposed } => out.push(proposed),
        EventGroup::Ordered { children } => {
            if children.is_empty() || children.len() > limit {
                return Err("empty or oversized ordered group".into());
            }
            for child in children {
                collect(child, depth + 1, limit, out)?;
            }
        }
        EventGroup::Simultaneous { occurrences } => {
            if occurrences.is_empty() || occurrences.len() > limit {
                return Err("empty or oversized simultaneous group".into());
            }
            if occurrences.len() > 1
                && occurrences.iter().any(|e| {
                    matches!(
                        e,
                        ProposedEvent::Setup { .. }
                            | ProposedEvent::AllocateDamageEffect { .. }
                            | ProposedEvent::CreateEffect { .. }
                            | ProposedEvent::ApplyDamageEffect { .. }
                            | ProposedEvent::CommitDamage { .. }
                            | ProposedEvent::CleanupEffects
                            | ProposedEvent::CounterStackObject { .. }
                            | ProposedEvent::ClearDamage
                            | ProposedEvent::TurnTransition { .. }
                            | ProposedEvent::DeclareAttackers { .. }
                            | ProposedEvent::DeclareBlockers { .. }
                    )
                })
            {
                return Err("global aggregate cannot compose simultaneously".into());
            }
            out.extend(occurrences);
            let keys = occurrences.iter().map(access).collect::<Vec<_>>();
            for i in 0..keys.len() {
                for j in i + 1..keys.len() {
                    if !keys[i].1.is_disjoint(&keys[j].0) || !keys[j].1.is_disjoint(&keys[i].0) {
                        return Err("unsupported simultaneous read/write dependency".into());
                    }
                }
            }
        }
    }
    if out.len() > limit {
        return Err("event occurrence resource limit".into());
    }
    Ok(())
}
impl Game {
    fn facts(&self, events: &[&ProposedEvent]) -> Result<PreEventFacts, String> {
        let mut damage_sources = vec![];
        let effects_event = events.iter().any(|e| {
            matches!(
                e,
                ProposedEvent::AllocateDamageEffect { .. }
                    | ProposedEvent::CreateEffect { .. }
                    | ProposedEvent::ApplyDamageEffect { .. }
                    | ProposedEvent::CommitDamage { .. }
                    | ProposedEvent::CleanupEffects
            )
        });
        let effects = if effects_event {
            self.state.effects.clone()
        } else {
            vec![]
        };
        let damage_transaction = if effects_event {
            self.damage_transaction()
                .ok()
                .map(DamageTransactionBefore::from)
        } else {
            None
        };
        let mut objects = BTreeSet::new();
        let mut players = BTreeSet::new();
        let mut turn = (effects_event
            || events
                .iter()
                .any(|e| matches!(e, ProposedEvent::Setup { .. })))
        .then(|| TurnBefore {
            turn: self.state.turn.clone(),
            owner: self.state.turn_owner,
            active: self.state.active,
            priority: matches!(self.state.frame, Frame::Priority).then_some(self.state.priority),
            live_turn_order: self.live_players(),
            stack_empty: self.state.stack.is_empty(),
        });
        let mut commander_damage = BTreeSet::new();
        let mut libraries = BTreeSet::new();
        let mut stack = BTreeSet::new();
        for event in events {
            match event {
                ProposedEvent::Setup { action } => {
                    players.extend(self.live_players());
                    match action {
                        SetupAction::ShuffleWord { player, .. } => {
                            libraries.insert(*player);
                        }
                        SetupAction::Draw { player } => {
                            libraries.insert(*player);
                            if let Some(card) =
                                self.state.libraries.get(player).and_then(|l| l.first())
                            {
                                objects.insert(
                                    self.object_id(*card).ok_or("missing opening draw object")?,
                                );
                            }
                        }
                        SetupAction::Return { player } => {
                            libraries.insert(*player);
                            objects.extend(
                                self.state
                                    .objects
                                    .values()
                                    .filter(|o| o.zone == (Zone::Hand { owner: *player }))
                                    .map(|o| o.id),
                            );
                        }
                        SetupAction::Bottom {
                            player,
                            objects: bottom,
                        } => {
                            libraries.insert(*player);
                            objects.extend(bottom);
                        }
                        SetupAction::Declare { .. } | SetupAction::Finish => {}
                    }
                }
                ProposedEvent::CreateEffect { effect } => {
                    if self.current(effect.origin.source).is_ok() {
                        objects.insert(effect.origin.source);
                    }
                    players.insert(effect.origin.controller);
                    if self
                        .state
                        .stack
                        .iter()
                        .any(|s| s.id == effect.origin.stack_id)
                    {
                        stack.insert(effect.origin.stack_id);
                    }
                    match &effect.kind {
                        ResolvedEffectKind::ModifyPowerToughness { target, .. } => {
                            objects.insert(*target);
                        }
                        ResolvedEffectKind::PreventNextDamage {
                            target: TargetEntity::Object { object },
                            ..
                        } => {
                            objects.insert(*object);
                        }
                        ResolvedEffectKind::PreventNextDamage {
                            target: TargetEntity::Player { player },
                            ..
                        } => {
                            players.insert(*player);
                        }
                        _ => {}
                    }
                }
                ProposedEvent::AllocateDamageEffect { .. }
                | ProposedEvent::ApplyDamageEffect { .. }
                | ProposedEvent::CommitDamage { .. }
                | ProposedEvent::CleanupEffects => {
                    if let Ok(tx) = self.damage_transaction() {
                        for d in &tx.entries {
                            damage_sources.push(d.source.clone());
                            match d.recipient {
                                DamageRecipient::Player { player } => {
                                    players.insert(player);
                                    if self
                                        .state
                                        .commanders
                                        .history(d.source.object.card)
                                        .is_some()
                                    {
                                        commander_damage.insert((d.source.object.card, player));
                                    }
                                }
                                DamageRecipient::Creature { object } => {
                                    if self.current(object).is_ok() {
                                        objects.insert(object);
                                    }
                                }
                            }
                        }
                    }
                    objects.extend(
                        self.state
                            .objects
                            .values()
                            .filter(|o| o.zone == Zone::Battlefield)
                            .map(|o| o.id),
                    );
                }
                ProposedEvent::GainLife { player, .. } | ProposedEvent::LoseLife { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::NoncombatDamage { damage } => {
                    damage_sources.push(damage.source.clone());
                    match damage.recipient {
                        DamageRecipient::Player { player } => {
                            players.insert(player);
                        }
                        DamageRecipient::Creature { object } => {
                            objects.insert(object);
                        }
                    }
                }
                ProposedEvent::CounterStackObject { target, .. } => {
                    let entry = self
                        .state
                        .stack
                        .iter()
                        .find(|entry| entry.id == *target)
                        .ok_or("missing counter stack facts")?;
                    stack.insert(*target);
                    players.insert(entry.controller);
                    if entry.kind.is_spell() {
                        objects.insert(entry.source);
                    }
                }
                ProposedEvent::Lose { player, reason } => {
                    players.insert(*player);
                    if *reason == PlayerLossReason::CommanderDamage {
                        commander_damage.extend(
                            self.state
                                .commanders
                                .records
                                .keys()
                                .map(|card| (*card, *player)),
                        );
                    }
                }
                ProposedEvent::Sacrifice { object, player }
                | ProposedEvent::DiscardCard { object, player } => {
                    objects.insert(*object);
                    players.insert(*player);
                }
                ProposedEvent::PutOnTopOwnerLibrary { object } => {
                    objects.insert(*object);
                    libraries.insert(
                        self.state
                            .cards
                            .get(&object.card)
                            .ok_or("missing library movement card")?
                            .owner,
                    );
                }
                ProposedEvent::DeclareAttackers { attacks } => {
                    for (o, p) in attacks {
                        objects.insert(*o);
                        players.insert(*p);
                    }
                }
                ProposedEvent::DeclareBlockers { player, blocks } => {
                    players.insert(*player);
                    for (b, a) in blocks {
                        objects.insert(*b);
                        objects.insert(*a);
                    }
                }
                ProposedEvent::CombatDamage { damage } => {
                    for d in damage {
                        objects.insert(d.source);
                        players.insert(d.controller);
                        match d.recipient {
                            DamageRecipient::Player { player } => {
                                players.insert(player);
                                if self.commander_damage_enabled()
                                    && self.state.commanders.history(d.source.card).is_some()
                                {
                                    commander_damage.insert((d.source.card, player));
                                }
                            }
                            DamageRecipient::Creature { object } => {
                                objects.insert(object);
                            }
                        }
                    }
                }
                ProposedEvent::FailedDraw { player } => {
                    players.insert(*player);
                }
                ProposedEvent::ClearDamage => {
                    objects.extend(
                        self.state
                            .objects
                            .values()
                            .filter(|o| o.marked_damage > Quantity::zero())
                            .map(|o| o.id),
                    );
                }
                ProposedEvent::StateBasedGraveyard { object, .. } => {
                    objects.insert(*object);
                }
                ProposedEvent::Untap { object }
                | ProposedEvent::Tap { object }
                | ProposedEvent::Move { object, .. } => {
                    objects.insert(*object);
                }
                ProposedEvent::LeaveGame {
                    object,
                    departing_owner,
                } => {
                    objects.insert(*object);
                    players.insert(*departing_owner);
                }
                ProposedEvent::AddMana { player, source, .. } => {
                    objects.insert(*source);
                    players.insert(*player);
                }
                ProposedEvent::ClearMana { player }
                | ProposedEvent::PayMana { player, .. }
                | ProposedEvent::PayManaSelected { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::LandPlayed { .. } | ProposedEvent::TurnTransition { .. } => {
                    if let ProposedEvent::TurnTransition { next, owner } = event
                        && next.number != self.state.turn.number
                    {
                        objects.extend(
                            self.state
                                .objects
                                .values()
                                .filter(|o| o.zone == Zone::Battlefield && o.controller == *owner)
                                .map(|o| o.id),
                        );
                    }
                    turn = Some(TurnBefore {
                        turn: self.state.turn.clone(),
                        owner: self.state.turn_owner,
                        active: self.state.active,
                        priority: matches!(self.state.frame, Frame::Priority)
                            .then_some(self.state.priority),
                        live_turn_order: self.live_players(),
                        stack_empty: self.state.stack.is_empty(),
                    });
                }
                ProposedEvent::ReverseMana { player, source, .. } => {
                    players.insert(*player);
                    if let Some(object) = source {
                        objects.insert(*object);
                    }
                }
            }
        }
        let objects = objects
            .into_iter()
            .map(|object| {
                let o = self.current(object)?;
                Ok(ObjectBefore {
                    object,
                    owner: self.state.cards[&object.card].owner,
                    controller: o.controller,
                    default_controller: o.default_controller,
                    zone: o.zone,
                    tapped: o.tapped,
                    marked_damage: o.marked_damage.clone(),
                    attack_ready: o.attack_ready,
                    characteristics: self.characteristics(object)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let players = players
            .into_iter()
            .map(|player| {
                let p = self
                    .state
                    .players
                    .get(player.0 as usize)
                    .ok_or("invalid event player")?;
                Ok(PlayerBefore {
                    failed_draw: p.failed_draw,
                    player,
                    life: p.life.clone(),
                    mana: p.mana.clone(),
                    status: p.status,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let commander_damage = commander_damage
            .into_iter()
            .map(|(commander, recipient)| CommanderDamageBefore {
                commander,
                recipient,
                previous: self
                    .state
                    .commanders
                    .history(commander)
                    .and_then(|history| history.combat_damage.get(&recipient))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        let libraries = libraries
            .into_iter()
            .map(|owner| LibraryBefore {
                owner,
                cards: self
                    .state
                    .libraries
                    .get(&owner)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect();
        let stack = stack
            .into_iter()
            .map(|id| {
                let entry = self
                    .state
                    .stack
                    .iter()
                    .find(|entry| entry.id == id)
                    .ok_or("missing consumed stack entry")?;
                Ok(StackBefore {
                    id,
                    source: entry.source,
                    controller: entry.controller,
                    spell: entry.kind.is_spell(),
                    paid: entry.paid.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(PreEventFacts {
            match_state: if events
                .iter()
                .any(|e| matches!(e, ProposedEvent::Setup { .. }))
            {
                self.state.match_state.clone()
            } else {
                None
            },
            effects,
            damage_transaction,
            damage_sources,
            stack,
            commander_damage,
            libraries,
            turn,
            objects,
            players,
        })
    }
    // CR 700.1: grouping is explicit. Ordinary trigger matching consumes the
    // actual outcomes and group observer views. General replacement/prevention
    // consumers remain outside these closed profiles.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn commit_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String> {
        let mut events = vec![];
        collect(
            &group,
            0,
            if self.prepared.config.match_profile() {
                400
            } else {
                16
            },
            &mut events,
        )?;
        let before = self.facts(&events)?;
        let batch_id = self.state.next_batch_id;
        let next_batch = batch_id.checked_add(1).ok_or("batch ID resource limit")?;
        let mut staged = self.fork();
        staged.state.next_batch_id = next_batch;
        let mut occurrences = vec![];
        staged.apply_group(&group, batch_id, &mut vec![], &mut occurrences)?;
        let result = CommittedBatch {
            batch_id,
            rules: cause.rules(),
            cause,
            group,
            before,
            occurrences,
        };
        staged.record_evidence(EvidenceRecord::Batch {
            batch: Box::new(result.clone()),
        })?;
        self.state = staged.state;
        self.trace = staged.trace;
        self.state_token = Arc::new(());
        Ok(result)
    }
    fn apply_group(
        &mut self,
        group: &EventGroup,
        batch: u64,
        path: &mut Vec<u32>,
        out: &mut Vec<OccurrenceResult>,
    ) -> Result<(), String> {
        match group {
            EventGroup::Occurrence { proposed } => {
                let trigger_before = self.prepared.config.triggers_profile().then(|| self.fork());
                let offset = out.len();
                let before = self.facts(&[proposed])?;
                self.apply_leaf(proposed, batch, path, before, out)?;
                if let Some(before) = trigger_before {
                    self.collect_triggers(&before, &out[offset..])?;
                }
            }
            EventGroup::Ordered { children } => {
                for (i, child) in children.iter().enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_group(child, batch, path, out)?;
                    path.pop();
                }
            }
            EventGroup::Simultaneous { occurrences } => {
                // Independent writes only: validate every leaf against the same
                // pre-state before committing any leaf; no replacement ordering
                // or aggregation of overlapping writes is inferred.
                let common = self.fork();
                let offset = out.len();
                let mut facts = vec![];
                for event in occurrences {
                    facts.push(common.facts(&[event])?);
                    common.fork().apply_occurrence(event)?;
                }
                for (i, (event, before)) in occurrences.iter().zip(facts).enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_leaf(event, batch, path, before, out)?;
                    path.pop();
                }
                self.collect_triggers(&common, &out[offset..])?;
            }
        }
        Ok(())
    }
    fn apply_leaf(
        &mut self,
        proposed: &ProposedEvent,
        batch: u64,
        path: &[u32],
        before: PreEventFacts,
        out: &mut Vec<OccurrenceResult>,
    ) -> Result<(), String> {
        let actual = self.apply_occurrence(proposed)?;
        // Keep the pre-existing native flat trace as a compatibility view.
        let event = match &actual {
            EventOutcome::LeftGame { object, owner } => Event::ObjectLeftGame {
                object: *object,
                owner: *owner,
            },
            _ => Event::Committed {
                proposed: proposed.clone(),
                actual: actual.clone(),
            },
        };
        self.record_event(event)?;
        out.push(OccurrenceResult {
            id: OccurrenceId {
                batch,
                path: path.to_vec(),
            },
            before,
            proposed: proposed.clone(),
            actual,
        });
        Ok(())
    }
    pub(super) fn commit(
        &mut self,
        proposed: ProposedEvent,
        cause: BatchCause,
    ) -> Result<EventOutcome, String> {
        let result = self.commit_batch(cause, EventGroup::Occurrence { proposed })?;
        result
            .result(&[])
            .cloned()
            .ok_or_else(|| "missing singleton actual outcome".into())
    }
    pub(super) fn move_object_for(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String> {
        match self.commit(
            ProposedEvent::Move {
                object,
                destination,
            },
            cause,
        )? {
            EventOutcome::Moved { movement } => Ok(movement),
            _ => Err("movement outcome invariant".into()),
        }
    }
    pub(super) fn move_instruction_object(
        &mut self,
        object: ObjectId,
        destination: Zone,
        cause: BatchCause,
    ) -> Result<MoveOutcome, String> {
        if matches!(destination, Zone::Library { .. }) {
            match self.commit(ProposedEvent::PutOnTopOwnerLibrary { object }, cause)? {
                EventOutcome::LibraryTopMoved { movement, .. } => Ok(movement),
                _ => Err("missing top-library movement outcome".into()),
            }
        } else {
            self.move_object_for(object, destination, cause)
        }
    }
    #[cfg(test)]
    pub(super) fn move_object(
        &mut self,
        object: ObjectId,
        destination: Zone,
    ) -> Result<MoveOutcome, String> {
        self.move_object_for(object, destination, BatchCause::Fixture)
    }
}

impl Game {
    pub(super) fn apply_occurrence(
        &mut self,
        proposed: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        let actual = match proposed {
            ProposedEvent::Setup { action } => EventOutcome::Setup {
                actual: self.apply_setup_action(action)?,
            },
            event @ (ProposedEvent::AllocateDamageEffect { .. }
            | ProposedEvent::CreateEffect { .. }
            | ProposedEvent::ApplyDamageEffect { .. }
            | ProposedEvent::CommitDamage { .. }
            | ProposedEvent::CleanupEffects) => self.apply_effect_event(event)?,
            ProposedEvent::NoncombatDamage { damage } => self.apply_noncombat_damage(damage)?,
            ProposedEvent::GainLife { player, amount }
            | ProposedEvent::LoseLife { player, amount } => {
                if !self.prepared.config.damage_life_profile()
                    || !self.is_live(*player)
                    || *amount <= Quantity::zero()
                {
                    return Err("invalid life change".into());
                }
                let life = &mut self.state.players[player.0 as usize].life;
                let previous = life.clone();
                if matches!(proposed, ProposedEvent::GainLife { .. }) {
                    life.add(amount);
                    EventOutcome::LifeGained {
                        player: *player,
                        amount: amount.clone(),
                        previous,
                        current: life.clone(),
                    }
                } else {
                    life.subtract(amount);
                    EventOutcome::LifeLost {
                        player: *player,
                        amount: amount.clone(),
                        previous,
                        current: life.clone(),
                    }
                }
            }
            ProposedEvent::CounterStackObject {
                target,
                countering,
                domain,
            } => {
                if !self.prepared.config.stack_costs_profile()
                    || countering.0 >= self.state.next_stack_id
                    || !self.stack_target_legal(*target, *countering, *domain)
                {
                    return Err("invalid countered stack object".into());
                }
                let index = self
                    .state
                    .stack
                    .iter()
                    .position(|entry| entry.id == *target)
                    .ok_or("missing counter target")?;
                let entry = self.state.stack[index].clone();
                let spell = entry.kind.is_spell();
                let movement = if spell {
                    let owner = self
                        .state
                        .cards
                        .get(&entry.source.card)
                        .ok_or("missing countered spell physical card")?
                        .owner;
                    let EventOutcome::Moved { movement } =
                        self.apply_occurrence(&ProposedEvent::Move {
                            object: entry.source,
                            destination: Zone::Graveyard { owner },
                        })?
                    else {
                        return Err("missing countered spell movement".into());
                    };
                    Some(movement)
                } else {
                    None
                };
                self.state.stack.remove(index);
                EventOutcome::StackObjectCountered {
                    target: *target,
                    source: entry.source,
                    spell,
                    movement,
                }
            }
            ProposedEvent::PayManaSelected {
                player,
                cost,
                spending,
            } => {
                let pool = self.selected_payment_pool(*player, cost, spending)?;
                self.state.players[player.0 as usize].mana = pool;
                EventOutcome::ManaPaidSelected {
                    player: *player,
                    cost: cost.clone(),
                    spending: spending.clone(),
                }
            }
            ProposedEvent::Sacrifice { object, player } => {
                if !self.is_live(*player)
                    || !self.target_legal(*object)
                    || self.current(*object)?.controller != *player
                {
                    return Err("invalid sacrifice event".into());
                }
                let owner = self
                    .state
                    .cards
                    .get(&object.card)
                    .ok_or("missing sacrificed card")?
                    .owner;
                let EventOutcome::Moved { movement } =
                    self.apply_occurrence(&ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard { owner },
                    })?
                else {
                    return Err("missing sacrifice movement".into());
                };
                EventOutcome::Sacrificed {
                    player: *player,
                    movement,
                }
            }
            ProposedEvent::DiscardCard { object, player } => {
                if !self.is_live(*player)
                    || self.current(*object)?.zone != (Zone::Hand { owner: *player })
                {
                    return Err("invalid discard event".into());
                }
                let EventOutcome::Moved { movement } =
                    self.apply_occurrence(&ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard { owner: *player },
                    })?
                else {
                    return Err("missing discard movement".into());
                };
                EventOutcome::CardDiscarded {
                    player: *player,
                    movement,
                }
            }
            ProposedEvent::PutOnTopOwnerLibrary { object } => {
                if !self.prepared.config.library_profile()
                    && !self.prepared.config.actions_profile()
                {
                    return Err("unsupported top-library movement profile".into());
                }
                let owner = self
                    .state
                    .cards
                    .get(&object.card)
                    .ok_or("missing top-library card")?
                    .owner;
                let previous_top = self
                    .state
                    .libraries
                    .get(&owner)
                    .and_then(|cards| cards.first())
                    .copied();
                let EventOutcome::Moved { movement } =
                    self.apply_occurrence(&ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Library { owner },
                    })?
                else {
                    return Err("top-library movement invariant".into());
                };
                let cards = self
                    .state
                    .libraries
                    .get_mut(&owner)
                    .ok_or("missing owner library")?;
                cards.retain(|card| *card != object.card);
                cards.insert(0, object.card);
                EventOutcome::LibraryTopMoved {
                    movement,
                    previous_top,
                }
            }
            ProposedEvent::Lose { player, reason } => {
                if !self.is_live(*player)
                    || match reason {
                        PlayerLossReason::Life => {
                            self.state.players[player.0 as usize].life > Quantity::zero()
                        }
                        PlayerLossReason::CommanderDamage => {
                            !self.commander_damage_enabled()
                                || !self
                                    .state
                                    .commanders
                                    .combat_loss_candidates()
                                    .contains(player)
                        }
                        PlayerLossReason::FailedDraw => {
                            !self.state.players[player.0 as usize].failed_draw
                        }
                    }
                {
                    return Err("invalid state-based player loss".into());
                }
                self.state.losing_players.push(*player);
                EventOutcome::Lost {
                    player: *player,
                    reason: *reason,
                }
            }
            ProposedEvent::DeclareAttackers { attacks } => {
                let player = self.state.active.ok_or("missing attacking player")?;
                self.validate_combat_response(
                    player,
                    &Response::Attackers {
                        attacks: attacks.clone(),
                    },
                )?;
                for (object, _) in attacks {
                    self.apply_occurrence(&ProposedEvent::Tap { object: *object })?;
                }
                self.state.combat_declared = !attacks.is_empty();
                self.state.combat = attacks
                    .iter()
                    .map(|(attacker, defender)| Attacker {
                        attacker: *attacker,
                        defender: *defender,
                        blocked: false,
                        blockers: vec![],
                    })
                    .collect();
                EventOutcome::AttackersDeclared {
                    attacks: attacks.clone(),
                }
            }
            ProposedEvent::DeclareBlockers { player, blocks } => {
                self.validate_combat_response(
                    *player,
                    &Response::Blockers {
                        blocks: blocks.clone(),
                    },
                )?;
                for (b, a) in blocks {
                    let attacker = self
                        .state
                        .combat
                        .iter_mut()
                        .find(|x| x.attacker == *a)
                        .ok_or("missing blocker attacker")?;
                    attacker.blocked = true;
                    attacker.blockers.push(*b);
                }
                EventOutcome::BlockersDeclared {
                    player: *player,
                    blocks: blocks.clone(),
                }
            }
            ProposedEvent::CombatDamage { damage } => self.apply_damage(damage)?,
            ProposedEvent::FailedDraw { player } => {
                if !self.is_live(*player)
                    || self
                        .state
                        .libraries
                        .get(player)
                        .is_none_or(|l| !l.is_empty())
                {
                    return Err("invalid failed draw".into());
                }
                self.state.players[player.0 as usize].failed_draw = true;
                EventOutcome::DrawFailed { player: *player }
            }
            ProposedEvent::ClearDamage => {
                let mut previous = vec![];
                for o in self.state.objects.values_mut() {
                    if o.marked_damage > Quantity::zero() {
                        previous.push((o.id, std::mem::take(&mut o.marked_damage)));
                    }
                }
                EventOutcome::DamageCleared { previous }
            }
            ProposedEvent::StateBasedGraveyard { object, reason } => {
                let o = self.current(*object)?;
                let toughness = self
                    .characteristics(*object)?
                    .toughness
                    .ok_or("missing SBA toughness")?;
                if !self.target_legal(*object)
                    || match reason {
                        CreatureLossReason::ZeroToughness => toughness > Quantity::zero(),
                        CreatureLossReason::LethalDamage => {
                            toughness <= Quantity::zero() || o.marked_damage < toughness
                        }
                    }
                {
                    return Err("invalid creature SBA".into());
                }
                let destination = Zone::Graveyard {
                    owner: self.state.cards[&object.card].owner,
                };
                let EventOutcome::Moved { movement } =
                    self.apply_occurrence(&ProposedEvent::Move {
                        object: *object,
                        destination,
                    })?
                else {
                    return Err("SBA movement".into());
                };
                EventOutcome::StateBasedMoved {
                    movement,
                    reason: *reason,
                }
            }
            ProposedEvent::Untap { object } => {
                let o = self.current(*object)?;
                if o.zone != Zone::Battlefield || !o.tapped {
                    return Err("invalid untap event".into());
                }
                self.state
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing untap object")?
                    .tapped = false;
                EventOutcome::Untapped { object: *object }
            }
            ProposedEvent::ClearMana { player } => {
                if !self.is_live(*player) {
                    return Err("invalid mana clearing player".into());
                }
                let previous = std::mem::take(&mut self.state.players[player.0 as usize].mana);
                EventOutcome::ManaCleared {
                    player: *player,
                    previous,
                }
            }
            ProposedEvent::LandPlayed { player } => {
                if !self.main_phase_action(*player) || self.state.turn.lands_played != 0 {
                    return Err("invalid land-play counting event".into());
                }
                let previous = self.state.turn.lands_played;
                let current = previous.checked_add(1).ok_or("land play resource limit")?;
                self.state.turn.lands_played = current;
                EventOutcome::LandPlayCounted {
                    player: *player,
                    previous,
                    current,
                }
            }
            ProposedEvent::TurnTransition { next, owner } => {
                if !self.turns_enabled()
                    || !matches!(self.state.frame, Frame::Transition)
                    || self.next_turn_state()? != (next.clone(), *owner)
                {
                    return Err("invalid turn-transition event".into());
                }
                let previous = std::mem::replace(&mut self.state.turn, next.clone());
                let previous_owner = std::mem::replace(&mut self.state.turn_owner, *owner);
                self.state.active = self.is_live(*owner).then_some(*owner);
                if next.number != previous.number {
                    for o in self.state.objects.values_mut() {
                        if o.zone == Zone::Battlefield && Some(o.controller) == self.state.active {
                            o.attack_ready = true;
                        }
                    }
                }
                EventOutcome::TurnTransitioned {
                    previous,
                    current: next.clone(),
                    previous_owner,
                    owner: *owner,
                }
            }
            ProposedEvent::Tap { object } => {
                let o = self.current(*object)?;
                if o.zone != Zone::Battlefield || o.tapped {
                    return Err("invalid tap event".into());
                }
                self.state
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing tap object")?
                    .tapped = true;
                EventOutcome::Tapped { object: *object }
            }
            ProposedEvent::AddMana {
                player,
                color,
                amount,
                source,
            } => {
                self.current(*source)?;
                if amount.is_negative() || !self.is_live(*player) {
                    return Err("negative mana or departed recipient".into());
                }
                self.state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing mana player")?
                    .mana
                    .entry(*color)
                    .or_default()
                    .add(amount);
                EventOutcome::ManaAdded {
                    player: *player,
                    color: *color,
                    amount: amount.clone(),
                }
            }
            ProposedEvent::PayMana { player, cost } => {
                if cost.generic.is_negative() || !self.is_live(*player) {
                    return Err("invalid payment cost or player".into());
                }
                let pool = self.payment_pool(*player, cost)?;
                self.state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing payment player")?
                    .mana = pool;
                EventOutcome::ManaPaid {
                    player: *player,
                    cost: cost.clone(),
                }
            }
            ProposedEvent::ReverseMana {
                player,
                color,
                amount,
                source,
                remove_zero,
            } => {
                // CR 733.1/733.2: undo only recorded casting-stage mana. A
                // departed source stays absent; a surviving source may have a
                // new controller after the intervening departure boundary.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                if *amount <= Quantity::zero() || !self.is_live(*player) {
                    return Err("invalid mana reversal quantity or player".into());
                }
                if let Some(object) = source {
                    let o = self.current(*object)?;
                    if o.zone != Zone::Battlefield
                        || !o.tapped
                        || self.characteristics(*object)?.kind != CardKind::Land
                    {
                        return Err("invalid mana reversal source".into());
                    }
                }
                let pool = &mut self
                    .state
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing mana reversal player")?
                    .mana;
                let remaining = pool.get_mut(color).ok_or("missing reversal mana")?;
                if *remaining < *amount {
                    return Err("insufficient reversal mana".into());
                }
                remaining.subtract(amount);
                let removed_entry = *remove_zero && *remaining == Quantity::zero();
                if removed_entry {
                    pool.remove(color);
                }
                if let Some(object) = source {
                    self.state
                        .objects
                        .get_mut(&object.card)
                        .ok_or("missing mana reversal source")?
                        .tapped = false;
                }
                EventOutcome::ManaReversed {
                    player: *player,
                    color: *color,
                    amount: amount.clone(),
                    source: *source,
                    removed_entry,
                }
            }
            ProposedEvent::LeaveGame {
                object,
                departing_owner,
            } => {
                self.current(*object)?;
                self.retain_source_lki(*object)?;
                if self.state.cards[&object.card].owner != *departing_owner
                    || self.is_live(*departing_owner)
                {
                    return Err("invalid leave-game ownership/status".into());
                }
                self.state
                    .commander_returns
                    .retain(|id| id.card != object.card);
                self.remove_from_combat(*object);
                for library in self.state.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                self.state.objects.remove(&object.card);
                EventOutcome::LeftGame {
                    object: *object,
                    owner: *departing_owner,
                }
            }
            ProposedEvent::Move {
                object,
                destination,
            } => {
                let o = self.current(*object)?.clone();
                self.retain_source_lki(*object)?;
                let owner = self
                    .state
                    .cards
                    .get(&object.card)
                    .ok_or("missing movement card")?
                    .owner;
                if let Zone::Hand { owner: p }
                | Zone::Graveyard { owner: p }
                | Zone::Library { owner: p }
                | Zone::Command { owner: p } = destination
                    && *p != owner
                {
                    return Err("zone owner invariant".into());
                }
                // CR 400.7: a zone move creates a new incarnation, and an actual
                // outcome explicitly provides its successor. No stale lookup.
                let successor = ObjectId {
                    card: object.card,
                    incarnation: object
                        .incarnation
                        .checked_add(1)
                        .ok_or("incarnation resource limit")?,
                };
                self.state
                    .commander_returns
                    .retain(|id| id.card != object.card);
                if self.commander_enabled()
                    && self.state.commanders.history(object.card).is_some()
                    && matches!(destination, Zone::Graveyard { .. } | Zone::Exile)
                {
                    self.state.commander_returns.insert(successor);
                }
                let controller = if *destination == Zone::Battlefield {
                    o.controller
                } else {
                    owner
                };
                self.remove_from_combat(*object);
                for library in self.state.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                if let Zone::Library { owner } = destination {
                    self.state
                        .libraries
                        .entry(*owner)
                        .or_default()
                        .push(object.card);
                }
                self.state.objects.insert(
                    object.card,
                    Object {
                        marked_damage: Quantity::zero(),
                        attack_ready: false,
                        id: successor,
                        controller,
                        default_controller: controller,
                        zone: *destination,
                        tapped: false,
                    },
                );
                EventOutcome::Moved {
                    movement: MoveOutcome {
                        previous: *object,
                        successor,
                        from: o.zone,
                        destination: *destination,
                    },
                }
            }
        };
        Ok(actual)
    }
}

impl Game {
    // CR 120.3 / 510.2: all sources and recipients are validated against the
    // common pre-state. One typed aggregate owns overlapping recipient writes.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    fn apply_damage(&mut self, damage: &[DamageOccurrence]) -> Result<EventOutcome, String> {
        let mut totals: BTreeMap<(u8, u32), Quantity> = BTreeMap::new();
        let mut commander_totals: BTreeMap<(CardId, PlayerId), Quantity> = BTreeMap::new();
        for d in damage {
            if !self.target_legal(d.source)
                || self.current(d.source)?.controller != d.controller
                || !self.is_live(d.controller)
                || d.amount <= Quantity::zero()
            {
                return Err("invalid damage source/amount".into());
            }
            let key = match d.recipient {
                DamageRecipient::Player { player } => {
                    if self.state.commanders.history(d.source.card).is_some()
                        && !self.commander_damage_enabled()
                    {
                        return Err("unsupported commander combat damage to player".into());
                    }
                    if !self.is_live(player) {
                        return Err("departed damage recipient".into());
                    }
                    if self.state.commanders.history(d.source.card).is_some() {
                        commander_totals
                            .entry((d.source.card, player))
                            .or_default()
                            .add(&d.amount);
                    }
                    (0, u32::from(player.0))
                }
                DamageRecipient::Creature { object } => {
                    if !self.target_legal(object) {
                        return Err("invalid creature damage recipient".into());
                    }
                    (1, object.card.0)
                }
            };
            totals.entry(key).or_default().add(&d.amount);
        }
        let mut results = vec![];
        for ((kind, id), amount) in totals {
            let (recipient, previous, current) = if kind == 0 {
                let player = PlayerId(id as u8);
                let life = &mut self.state.players[player.0 as usize].life;
                let previous = life.clone();
                life.subtract(&amount);
                (DamageRecipient::Player { player }, previous, life.clone())
            } else {
                let object = self
                    .state
                    .objects
                    .get_mut(&CardId(id))
                    .ok_or("missing damage object")?;
                let previous = object.marked_damage.clone();
                object.marked_damage.add(&amount);
                (
                    DamageRecipient::Creature { object: object.id },
                    previous,
                    object.marked_damage.clone(),
                )
            };
            results.push(DamageResult {
                recipient,
                previous,
                current,
            });
        }
        let mut commander_damage = vec![];
        for ((commander, recipient), amount) in commander_totals {
            let previous = self
                .state
                .commanders
                .history(commander)
                .and_then(|history| history.combat_damage.get(&recipient))
                .cloned()
                .unwrap_or_default();
            self.state
                .commanders
                .record_damage(commander, recipient, true, &amount)?;
            let current = self
                .state
                .commanders
                .history(commander)
                .and_then(|history| history.combat_damage.get(&recipient))
                .cloned()
                .ok_or("missing actual Commander damage")?;
            commander_damage.push(CommanderDamageResult {
                commander,
                recipient,
                amount,
                previous,
                current,
            });
        }
        Ok(EventOutcome::DamageDealt {
            damage: damage.to_vec(),
            results,
            commander_damage,
        })
    }
}
