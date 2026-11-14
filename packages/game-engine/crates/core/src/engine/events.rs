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
    DeathtouchCheck,
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
            Self::DeathtouchCheck => &["702.2b", "704.3", "704.5h"],
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<KeywordAbility>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deathtouch_damage: bool,
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
    TokenAllocation,
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
fn independent_footprints(
    footprints: impl IntoIterator<Item = (BTreeSet<FactKey>, BTreeSet<FactKey>)>,
) -> bool {
    let mut prior_reads = BTreeSet::new();
    let mut prior_writes = BTreeSet::new();
    for (reads, writes) in footprints {
        // This is exactly the pairwise dependency rule: a new write must not
        // intersect any earlier read, nor a new read any earlier write. Shared
        // reads remain allowed; an occurrence's own accesses are not compared.
        if !writes.is_disjoint(&prior_reads) || !reads.is_disjoint(&prior_writes) {
            return false;
        }
        prior_reads.extend(reads);
        prior_writes.extend(writes);
    }
    true
}

#[cfg(test)]
mod footprint_tests {
    use super::*;

    #[test]
    fn streaming_conflicts_match_pairwise_reference_exhaustively() {
        let keys = [FactKey::Object(CardId(0)), FactKey::Mana(PlayerId(1))];
        let sets = (0..4)
            .map(|mask| {
                keys.iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, key)| *key)
                    .collect::<BTreeSet<_>>()
            })
            .collect::<Vec<_>>();
        let footprints = sets
            .iter()
            .flat_map(|reads| {
                sets.iter()
                    .map(move |writes| (reads.clone(), writes.clone()))
            })
            .collect::<Vec<_>>();
        assert!(independent_footprints([]));
        for a in &footprints {
            assert!(independent_footprints([a.clone()]));
            for b in &footprints {
                for c in &footprints {
                    let sequence = [a.clone(), b.clone(), c.clone()];
                    let reference = (0..sequence.len()).all(|i| {
                        (i + 1..sequence.len()).all(|j| {
                            sequence[i].1.is_disjoint(&sequence[j].0)
                                && sequence[j].1.is_disjoint(&sequence[i].0)
                        })
                    });
                    assert_eq!(independent_footprints(sequence), reference);
                }
            }
        }
    }

    #[test]
    fn streaming_conflicts_cover_full_capacity_and_distant_dependencies() {
        let footprints = (0..capacities::EVENT_GROUP_LIMIT)
            .map(|index| {
                let keys = BTreeSet::from([FactKey::Object(CardId(index as u32))]);
                (keys.clone(), keys)
            })
            .collect::<Vec<_>>();
        assert!(independent_footprints(footprints.clone()));
        let mut conflicting = footprints;
        conflicting
            .last_mut()
            .unwrap()
            .0
            .insert(FactKey::Object(CardId(0)));
        assert!(!independent_footprints(conflicting));
    }
}

fn access(event: &ProposedEvent, game: &Game) -> (BTreeSet<FactKey>, BTreeSet<FactKey>) {
    match event {
        ProposedEvent::CreateCreatureTokens { .. } => {
            let keys = BTreeSet::from([FactKey::TokenAllocation]);
            (keys.clone(), keys)
        }
        ProposedEvent::PutPlusOneCounters { object, .. } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::CeaseToken { object } => {
            // A token put on a library (Time Ebb) ceases from that library.
            let mut keys = BTreeSet::from([FactKey::Object(object.card)]);
            if matches!(
                game.current(*object).map(|o| o.zone),
                Ok(Zone::Library { .. })
            ) {
                keys.insert(FactKey::Libraries);
            }
            (keys.clone(), keys)
        }
        ProposedEvent::FinishLibraryShuffle { .. }
        | ProposedEvent::ShuffleLibraryWord { .. }
        | ProposedEvent::Setup { .. } => {
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
            let mut write = BTreeSet::from([match damage.recipient {
                DamageRecipient::Player { player } => FactKey::Life(player),
                DamageRecipient::Creature { object } => FactKey::Object(object.card),
            }]);
            if game.has_keyword(&damage.source, KeywordAbility::Lifelink) {
                write.insert(FactKey::Life(damage.source.controller));
            }
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
                if game.has_keyword(d.source, KeywordAbility::Lifelink) {
                    keys.insert(FactKey::Life(d.controller));
                }
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
        ProposedEvent::ConsumeDeathtouchWindow { objects } => {
            let mut keys = BTreeSet::from([FactKey::AllDamage]);
            keys.extend(objects.iter().map(|o| FactKey::Object(o.card)));
            (keys.clone(), keys)
        }
        ProposedEvent::ClearDamage => {
            let keys = BTreeSet::from([FactKey::AllDamage]);
            (keys.clone(), keys)
        }
        ProposedEvent::Attach { object, target } => {
            let writes = BTreeSet::from([FactKey::Object(object.card)]);
            let mut reads = writes.clone();
            if let Some(target) = target {
                reads.insert(FactKey::Object(target.card));
            }
            (reads, writes)
        }
        ProposedEvent::DestroyPermanent { object }
        | ProposedEvent::DestroyCreature { object }
        | ProposedEvent::StateBasedGraveyard { object, .. } => {
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
        ProposedEvent::SearchLandEntry { object, .. }
        | ProposedEvent::Move { object, .. }
        | ProposedEvent::LeaveGame { object, .. } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            let mut reads = keys.clone();
            // Zone membership is authoritative (CR 400.7). A battlefield Aura
            // moving to graveyard neither consumes nor changes library order.
            // Its SBA may coincide with a tucked token ceasing (704.5d/m).
            // Distinct identity removals commute (including departure cleanup).
            // They still read library order, rejecting composition with tuck.
            if game
                .current(*object)
                .is_ok_and(|o| matches!(o.zone, Zone::Library { .. }))
            {
                reads.insert(FactKey::Libraries);
            }
            (reads, keys)
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
        ProposedEvent::ReversePermanentMana { receipt } => {
            let keys = BTreeSet::from([
                FactKey::Mana(receipt.player),
                FactKey::Object(receipt.source.object.card),
            ]);
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
    game: &Game,
    depth: u32,
    limit: usize,
    out: &mut Vec<&'a ProposedEvent>,
) -> Result<(), String> {
    if depth > capacities::EVENT_GROUP_DEPTH {
        return Err("event group depth resource limit".into());
    }
    match group {
        EventGroup::Occurrence { proposed } => out.push(proposed),
        EventGroup::Ordered { children } => {
            if children.is_empty() || children.len() > limit {
                return Err("empty or oversized ordered group".into());
            }
            for child in children {
                collect(child, game, depth + 1, limit, out)?;
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
                            | ProposedEvent::ReversePermanentMana { .. }
                            | ProposedEvent::CounterStackObject { .. }
                            | ProposedEvent::CreateCreatureTokens { .. }
                            | ProposedEvent::ClearDamage
                            | ProposedEvent::ConsumeDeathtouchWindow { .. }
                            | ProposedEvent::TurnTransition { .. }
                            | ProposedEvent::DeclareAttackers { .. }
                            | ProposedEvent::DeclareBlockers { .. }
                    )
                })
            {
                return Err("global aggregate cannot compose simultaneously".into());
            }
            out.extend(occurrences);
            if !independent_footprints(occurrences.iter().map(|event| access(event, game))) {
                return Err("unsupported simultaneous read/write dependency".into());
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
            self.state.rules.effects.clone()
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
            || events.iter().any(|e| {
                matches!(
                    e,
                    ProposedEvent::FinishLibraryShuffle { .. }
                        | ProposedEvent::Setup { .. }
                        | ProposedEvent::ShuffleLibraryWord { .. }
                )
            }))
        .then(|| TurnBefore {
            turn: self.state.rules.turn.clone(),
            owner: self.state.rules.turn_owner,
            active: self.state.rules.active,
            priority: matches!(self.state.rules.frame, Frame::Priority)
                .then_some(self.state.rules.priority),
            live_turn_order: self.live_players(),
            stack_empty: self.state.rules.stack.is_empty(),
        });
        let mut commander_damage = BTreeSet::new();
        let mut libraries = BTreeSet::new();
        let mut stack = BTreeSet::new();
        for event in events {
            match event {
                ProposedEvent::CreateCreatureTokens { controller, .. } => {
                    // CR608.2m: the invocation owns its fixed token definition.
                    // It reads no current source incarnation after departure.
                    players.insert(*controller);
                }
                ProposedEvent::PutPlusOneCounters { object, .. } => {
                    objects.insert(*object);
                }
                ProposedEvent::CeaseToken { object } => {
                    objects.insert(*object);
                    if let Zone::Library { owner } = self.current(*object)?.zone {
                        libraries.insert(owner);
                    }
                }
                ProposedEvent::FinishLibraryShuffle { player }
                | ProposedEvent::ShuffleLibraryWord { player, .. } => {
                    libraries.insert(*player);
                }
                ProposedEvent::DestroyPermanent { object }
                | ProposedEvent::DestroyCreature { object }
                | ProposedEvent::SearchLandEntry { object, .. } => {
                    objects.insert(*object);
                    if let Zone::Library { owner } = self.current(*object)?.zone {
                        libraries.insert(owner);
                    }
                }
                ProposedEvent::Setup { action } => {
                    players.extend(self.live_players());
                    match action {
                        SetupAction::ShuffleWord { player, .. } => {
                            libraries.insert(*player);
                        }
                        SetupAction::Draw { player } => {
                            libraries.insert(*player);
                            if let Some(card) = self
                                .state
                                .rules
                                .libraries
                                .get(player)
                                .and_then(|l| l.first())
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
                                    .rules
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
                        .rules
                        .stack
                        .iter()
                        .any(|s| s.id == effect.origin.stack_id)
                    {
                        stack.insert(effect.origin.stack_id);
                    }
                    match &effect.kind {
                        ResolvedEffectKind::PumpSet { targets, .. }
                        | ResolvedEffectKind::ExileDamaged { targets } => objects.extend(targets),
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
                            let appearance = self.damage_source_appearance(d)?;
                            if self.has_keyword(&appearance, KeywordAbility::Lifelink)
                                && self.is_live(appearance.controller)
                            {
                                players.insert(appearance.controller);
                            }
                            // Consumed damage-time properties remain distinct
                            // from damage_transaction.entries assignment facts.
                            damage_sources.push(appearance);
                            match d.recipient {
                                DamageRecipient::Player { player } => {
                                    players.insert(player);
                                    if self
                                        .state
                                        .rules
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
                            .rules
                            .objects
                            .values()
                            .filter(|o| {
                                o.zone == Zone::Battlefield
                                    || (matches!(event, ProposedEvent::CleanupEffects)
                                        && (o.marked_damage > Quantity::zero()
                                            || o.deathtouch_damage))
                            })
                            .map(|o| o.id),
                    );
                }
                ProposedEvent::GainLife { player, .. } | ProposedEvent::LoseLife { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::NoncombatDamage { damage } => {
                    damage_sources.push(damage.source.clone());
                    if self.has_keyword(&damage.source, KeywordAbility::Lifelink)
                        && self.is_live(damage.source.controller)
                    {
                        players.insert(damage.source.controller);
                    }
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
                        .rules
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
                                .rules
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
                            .rules
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
                                if self.state.rules.commanders.history(d.source.card).is_some() {
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
                ProposedEvent::ConsumeDeathtouchWindow { objects: checked } => {
                    objects.extend(checked);
                }
                ProposedEvent::ClearDamage => {
                    objects.extend(
                        self.state
                            .rules
                            .objects
                            .values()
                            .filter(|o| o.marked_damage > Quantity::zero() || o.deathtouch_damage)
                            .map(|o| o.id),
                    );
                }
                ProposedEvent::Attach { object, target } => {
                    objects.insert(*object);
                    if let Some(target) = target {
                        objects.insert(*target);
                    }
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
                    if self.current(*source).is_ok() {
                        objects.insert(*source);
                    }
                    players.insert(*player);
                }
                ProposedEvent::ClearMana { player }
                | ProposedEvent::PayMana { player, .. }
                | ProposedEvent::PayManaSelected { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::LandPlayed { .. } | ProposedEvent::TurnTransition { .. } => {
                    if let ProposedEvent::TurnTransition { next, owner } = event
                        && next.number != self.state.rules.turn.number
                    {
                        objects.extend(
                            self.state
                                .rules
                                .objects
                                .values()
                                .filter(|o| o.zone == Zone::Battlefield && o.controller == *owner)
                                .map(|o| o.id),
                        );
                    }
                    turn = Some(TurnBefore {
                        turn: self.state.rules.turn.clone(),
                        owner: self.state.rules.turn_owner,
                        active: self.state.rules.active,
                        priority: matches!(self.state.rules.frame, Frame::Priority)
                            .then_some(self.state.rules.priority),
                        live_turn_order: self.live_players(),
                        stack_empty: self.state.rules.stack.is_empty(),
                    });
                }
                ProposedEvent::ReversePermanentMana { receipt } => {
                    players.insert(receipt.player);
                    if let Some(o) = self.state.rules.objects.get(&receipt.source.object.card) {
                        objects.insert(o.id);
                    }
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
                    keywords: { self.current_keywords(object) },
                    deathtouch_damage: o.deathtouch_damage,
                    object,
                    owner: self.state.rules.cards[&object.card].owner,
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
                    .rules
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
                    .rules
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
                    .rules
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
                    .rules
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
            match_state: if events.iter().any(|e| {
                matches!(
                    e,
                    ProposedEvent::FinishLibraryShuffle { .. }
                        | ProposedEvent::Setup { .. }
                        | ProposedEvent::ShuffleLibraryWord { .. }
                )
            }) {
                self.state.rules.match_state.clone()
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
        {
            fn members(g: &EventGroup, depth: u32) -> Result<usize, u32> {
                if depth > capacities::EVENT_GROUP_DEPTH {
                    return Err(depth);
                }
                Ok(match g {
                    EventGroup::Occurrence { .. } => 1,
                    EventGroup::Simultaneous { occurrences } => occurrences.len(),
                    EventGroup::Ordered { children } => {
                        let mut total = 0usize;
                        for child in children {
                            total = total.saturating_add(members(child, depth + 1)?);
                        }
                        total
                    }
                })
            }
            let requested = match members(&group, 0) {
                Ok(n) => n,
                Err(requested) => {
                    return Err(
                        self.capacity_error(RuleFault::EventGroupDepthCapacityExceeded {
                            capacity: capacities::EVENT_GROUP_DEPTH,
                            requested,
                        }),
                    );
                }
            };
            if requested > self.event_group_limit() {
                return Err(self.capacity_error(RuleFault::EventGroupCapacityExceeded {
                    capacity: self.event_group_limit(),
                    requested,
                }));
            }
        }
        collect(&group, self, 0, self.event_group_limit(), &mut events)?;
        let before = self.facts(&events)?;
        let batch_id = self.state.rules.next_batch_id;
        let next_batch = batch_id.checked_add(1).ok_or("batch ID resource limit")?;
        if self.in_rule_step {
            // The enclosing rule-step staging owns rollback. No intermediate
            // state is observable, and every failed batch aborts that step.
            return self.apply_batch(cause, group, before, batch_id, next_batch);
        }
        let mut staged = self.fork();
        match staged.apply_batch(cause, group, before, batch_id, next_batch) {
            Ok(result) => {
                self.state = staged.state;
                self.trace = staged.trace;
                self.state_token = Arc::new(());
                Ok(result)
            }
            Err(error) => {
                self.state.rules.typed_fault = staged.state.rules.typed_fault;
                Err(error)
            }
        }
    }
    fn apply_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
        before: PreEventFacts,
        batch_id: u64,
        next_batch: u64,
    ) -> Result<CommittedBatch, String> {
        self.state.rules.next_batch_id = next_batch;
        let mut occurrences = vec![];
        self.apply_group(&group, batch_id, &mut vec![], &mut occurrences)?;
        let result = CommittedBatch {
            batch_id,
            rules: cause.rules(),
            cause,
            group,
            before,
            occurrences,
        };
        self.record_evidence(EvidenceRecord::Batch {
            batch: Box::new(result.clone()),
        })?;
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
                let trigger_before = Some(self.fork());
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
                    let mut validation = common.fork();
                    if let Err(error) = validation.apply_occurrence(event) {
                        // The validation branch is discarded, but its typed
                        // resource failure must reach the enclosing atomic
                        // transaction just as an application failure does.
                        self.state.rules.typed_fault = validation.state.rules.typed_fault;
                        return Err(error);
                    }
                }
                for (i, (event, before)) in occurrences.iter().zip(facts).enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_leaf(event, batch, path, before, out)?;
                    path.pop();
                }
                // CR 603.10a, 702.15c/702.2e: simultaneous departures use
                // the common prior layer-6 view, even if a granting source
                // happened to be applied first by deterministic event order.
                {
                    for o in common
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| o.zone == Zone::Battlefield)
                    {
                        if self.current(o.id).is_err() {
                            self.retain_source_facts(&common.object_before(o.id)?);
                        }
                    }
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
        let actual = self.apply_occurrence_with_prior(proposed, Some(&before))?;
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
    fn destruction_is_prevented(
        &self,
        object: ObjectId,
        before: Option<&PreEventFacts>,
    ) -> Result<bool, String> {
        let Some(before) = before else {
            return Ok(self.has_keyword(object, KeywordAbility::Indestructible));
        };
        // The leaf's facts are produced against its pre-event fork, not supplied
        // by a caller. Exact incarnation/zone/type must still agree at commit;
        // missing facts never mean an unprotected target. Restore verifies the
        // complete prior payload (including keywords) through boundary replay.
        let [fact] = before.objects.as_slice() else {
            return Err("missing or ambiguous destruction prior object".into());
        };
        let current = self.current(object)?;
        let types = &self.definition(object.card)?.characteristics;
        if fact.object != object
            || fact.zone != Zone::Battlefield
            || current.zone != fact.zone
            || fact.characteristics.kind != types.kind
            || fact.characteristics.additional_types != types.additional_types
            || fact.owner != self.state.rules.cards[&object.card].owner
            || fact.controller != current.controller
        {
            return Err("mismatched destruction prior object".into());
        }
        Ok(fact.keywords.contains(&KeywordAbility::Indestructible))
    }
    pub(super) fn apply_occurrence(
        &mut self,
        proposed: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        self.apply_occurrence_with_prior(proposed, None)
    }
    // CR 611.3b, 613.1f, 702.12b, 704.3: simultaneous destruction
    // consumes the common prior layer-6 view. Ordered leaves capture fresh facts.
    // No new snapshot state: this authority is already in occurrence evidence.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(super) fn apply_occurrence_with_prior(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        let actual = match proposed {
            ProposedEvent::FinishLibraryShuffle { player } => {
                if !matches!(self.state.rules.frame,Frame::ResolutionChoice {choice:ResolutionChoice::Shuffle {player:p,index:0,..},..} if p==*player)
                {
                    return Err("invalid shuffle completion".into());
                }
                let count = self
                    .state
                    .rules
                    .match_state
                    .as_mut()
                    .ok_or("missing match randomness")?
                    .library_shuffles
                    .get_mut(player.0 as usize)
                    .ok_or("invalid shuffle player")?;
                *count = count
                    .checked_add(1)
                    .ok_or("public shuffle count overflow")?;
                EventOutcome::LibraryShuffled {
                    player: *player,
                    count: *count,
                }
            }
            ProposedEvent::Attach { object, target } => {
                if self.current(*object)?.zone != Zone::Battlefield
                    || !self
                        .definition(object.card)?
                        .characteristics
                        .subtypes
                        .iter()
                        .any(|s| s == "Equipment" || s == "Aura")
                    || target.is_some_and(|t| t == *object || !self.is_battlefield_creature(t))
                {
                    return Err("invalid attachment event".into());
                }
                let o = self
                    .state
                    .rules
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing attachment")?;
                let previous = o.attached_to;
                o.attached_to = *target;
                EventOutcome::AttachmentChanged {
                    object: *object,
                    previous,
                    target: *target,
                }
            }
            ProposedEvent::DestroyPermanent { object } => {
                if !self
                    .current(*object)
                    .is_ok_and(|o| o.zone == Zone::Battlefield)
                {
                    return Err("invalid destroy permanent event".into());
                }
                // CR 702.12b, 701.8b, 614.17c: a forbidden destruction
                // never reaches Move or its death-to-exile replacement.
                if self.destruction_is_prevented(*object, before)? {
                    return Ok(EventOutcome::DestructionPrevented { object: *object });
                }
                self.apply_occurrence(&ProposedEvent::Move {
                    object: *object,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&object.card].owner,
                    },
                })?
            }
            ProposedEvent::DestroyCreature { object } => {
                if !self.is_battlefield_creature(*object) {
                    return Err("invalid destroy creature event".into());
                }
                // CR 702.12b, 701.8b, 614.17c: a forbidden destruction
                // never reaches Move or its death-to-exile replacement.
                if self.destruction_is_prevented(*object, before)? {
                    return Ok(EventOutcome::DestructionPrevented { object: *object });
                }
                self.apply_occurrence(&ProposedEvent::Move {
                    object: *object,
                    destination: Zone::Graveyard {
                        owner: self.state.rules.cards[&object.card].owner,
                    },
                })?
            }
            ProposedEvent::SearchLandEntry { object, player } => {
                if !self.search_lands(*player).contains(object) {
                    return Err("invalid searched land entry".into());
                }
                let moved = self.apply_occurrence(&ProposedEvent::Move {
                    object: *object,
                    destination: Zone::Battlefield,
                })?;
                let o = self
                    .state
                    .rules
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing searched land")?;
                o.controller = *player;
                o.tapped = true;
                moved
            }
            ProposedEvent::ShuffleLibraryWord { player, index } => {
                self.apply_search_shuffle_word(*player, *index)?
            }
            event @ (ProposedEvent::CreateCreatureTokens { .. }
            | ProposedEvent::PutPlusOneCounters { .. }
            | ProposedEvent::CeaseToken { .. }) => self.apply_token_event(event)?,
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
                if !self.is_live(*player) || *amount <= Quantity::zero() {
                    return Err("invalid life change".into());
                }
                let life = &mut self.state.rules.players[player.0 as usize].life;
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
                if countering.0 >= self.state.rules.next_stack_id
                    || !self.stack_target_legal(*target, *countering, *domain)
                {
                    return Err("invalid countered stack object".into());
                }
                if self.cant_counter(*target) {
                    return Ok(EventOutcome::CounterPrevented { target: *target });
                }
                let index = self
                    .state
                    .rules
                    .stack
                    .iter()
                    .position(|entry| entry.id == *target)
                    .ok_or("missing counter target")?;
                let entry = self.state.rules.stack[index].clone();
                let spell = entry.kind.is_spell();
                let movement = if spell {
                    let owner = self
                        .state
                        .rules
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
                self.state.rules.stack.remove(index);
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
                self.state.rules.players[player.0 as usize].mana = pool;
                EventOutcome::ManaPaidSelected {
                    player: *player,
                    cost: cost.clone(),
                    spending: spending.clone(),
                }
            }
            ProposedEvent::Sacrifice { object, player } => {
                if !self.is_live(*player)
                    || self.current(*object)?.zone != Zone::Battlefield
                    || self.current(*object)?.controller != *player
                {
                    return Err("invalid sacrifice event".into());
                }
                let owner = self
                    .state
                    .rules
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
                let owner = self
                    .state
                    .rules
                    .cards
                    .get(&object.card)
                    .ok_or("missing top-library card")?
                    .owner;
                let previous_top = self
                    .state
                    .rules
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
                    .rules
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
                            self.state.rules.players[player.0 as usize].life > Quantity::zero()
                        }
                        PlayerLossReason::CommanderDamage => !self
                            .state
                            .rules
                            .commanders
                            .combat_loss_candidates()
                            .contains(player),
                        PlayerLossReason::FailedDraw => {
                            !self.state.rules.players[player.0 as usize].failed_draw
                        }
                    }
                {
                    return Err("invalid state-based player loss".into());
                }
                self.state.rules.losing_players.push(*player);
                EventOutcome::Lost {
                    player: *player,
                    reason: *reason,
                }
            }
            ProposedEvent::DeclareAttackers { attacks } => {
                let player = self.state.rules.active.ok_or("missing attacking player")?;
                self.validate_combat_response(
                    player,
                    &Response::Attackers {
                        attacks: attacks.clone(),
                    },
                )?;
                for (object, _) in attacks {
                    if !self.has_keyword(*object, KeywordAbility::Vigilance) {
                        self.apply_occurrence(&ProposedEvent::Tap { object: *object })?;
                    }
                }
                self.state.rules.combat_declared = !attacks.is_empty();
                self.state.rules.combat = attacks
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
                        .rules
                        .combat
                        .iter_mut()
                        .find(|x| x.attacker == *a)
                        .ok_or("missing blocker attacker")?;
                    attacker.blocked = true;
                    attacker.blockers.push(*b);
                    self.state.rules.blocking_creatures.insert(*b);
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
                        .rules
                        .libraries
                        .get(player)
                        .is_none_or(|l| !l.is_empty())
                {
                    return Err("invalid failed draw".into());
                }
                self.state.rules.players[player.0 as usize].failed_draw = true;
                EventOutcome::DrawFailed { player: *player }
            }
            ProposedEvent::ConsumeDeathtouchWindow { objects } => {
                if objects.is_empty() || objects.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err("empty or noncanonical deathtouch window".into());
                }
                // Exact checked incarnations only. Validate the whole set before writing.
                for object in objects {
                    let o = self.current(*object)?;
                    if o.zone != Zone::Battlefield || !o.deathtouch_damage {
                        return Err("invalid checked deathtouch survivor".into());
                    }
                }
                for object in objects {
                    self.state
                        .rules
                        .objects
                        .get_mut(&object.card)
                        .ok_or("missing checked deathtouch object")?
                        .deathtouch_damage = false;
                }
                EventOutcome::DeathtouchWindowConsumed {
                    objects: objects.clone(),
                }
            }
            ProposedEvent::ClearDamage => {
                let mut previous = vec![];
                for o in self.state.rules.objects.values_mut() {
                    o.deathtouch_damage = false;
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
                if !self.is_battlefield_creature(*object)
                    || match reason {
                        CreatureLossReason::ZeroToughness => toughness > Quantity::zero(),
                        CreatureLossReason::Deathtouch => {
                            toughness <= Quantity::zero() || !o.deathtouch_damage
                        }
                        CreatureLossReason::LethalDamage => {
                            toughness <= Quantity::zero() || o.marked_damage < toughness
                        }
                    }
                {
                    return Err("invalid creature SBA".into());
                }
                if *reason != CreatureLossReason::ZeroToughness
                    && self.has_keyword(*object, KeywordAbility::Indestructible)
                {
                    return Err("indestructible creature ignores destruction SBA".into());
                }
                let destination = Zone::Graveyard {
                    owner: self.state.rules.cards[&object.card].owner,
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
                    .rules
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
                let previous =
                    std::mem::take(&mut self.state.rules.players[player.0 as usize].mana);
                EventOutcome::ManaCleared {
                    player: *player,
                    previous,
                }
            }
            ProposedEvent::LandPlayed { player } => {
                if !self.main_phase_action(*player) || self.state.rules.turn.lands_played != 0 {
                    return Err("invalid land-play counting event".into());
                }
                let previous = self.state.rules.turn.lands_played;
                let current = previous.checked_add(1).ok_or("land play resource limit")?;
                self.state.rules.turn.lands_played = current;
                EventOutcome::LandPlayCounted {
                    player: *player,
                    previous,
                    current,
                }
            }
            ProposedEvent::TurnTransition { next, owner } => {
                if !matches!(self.state.rules.frame, Frame::Transition)
                    || self.next_turn_state()? != (next.clone(), *owner)
                {
                    return Err("invalid turn-transition event".into());
                }
                // CR510.4/702.7b: capture eligibility atomically as the first
                // damage step begins, before any later departure boundary.
                if next.step == TurnStep::FirstStrikeDamage {
                    self.state.rules.first_strike_participants = self.first_strikers();
                }
                // CR511.3: remove every combatant as the end of combat step
                // ends, before the separately budgeted postcombat entry unit.
                if self.state.rules.turn.step == TurnStep::EndCombat {
                    self.clear_combat();
                }
                let previous = std::mem::replace(&mut self.state.rules.turn, next.clone());
                let previous_owner = std::mem::replace(&mut self.state.rules.turn_owner, *owner);
                self.state.rules.active = self.is_live(*owner).then_some(*owner);
                if next.number != previous.number {
                    for o in self.state.rules.objects.values_mut() {
                        if o.zone == Zone::Battlefield
                            && Some(o.controller) == self.state.rules.active
                        {
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
                    .rules
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
                if self.current(*source).is_err()
                    && !(self
                        .state
                        .rules
                        .cards
                        .get(&source.card)
                        .is_some_and(|c| !self.is_live(c.owner)))
                {
                    return Err("missing mana source".into());
                }
                if amount.is_negative() || !self.is_live(*player) {
                    return Err("negative mana or departed recipient".into());
                }
                self.state
                    .rules
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
                    .rules
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing payment player")?
                    .mana = pool;
                EventOutcome::ManaPaid {
                    player: *player,
                    cost: cost.clone(),
                }
            }
            ProposedEvent::ReversePermanentMana { receipt } => {
                self.reverse_permanent_mana(receipt)?
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
                    .rules
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
                        .rules
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
                if self.state.rules.cards[&object.card].owner != *departing_owner
                    || self.is_live(*departing_owner)
                {
                    return Err("invalid leave-game ownership/status".into());
                }
                self.state
                    .rules
                    .commander_returns
                    .retain(|id| id.card != object.card);
                self.remove_from_combat(*object);
                for library in self.state.rules.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                self.state.rules.objects.remove(&object.card);
                EventOutcome::LeftGame {
                    object: *object,
                    owner: *departing_owner,
                }
            }
            ProposedEvent::Move {
                object,
                destination,
            } => {
                // Pinned CR 614.1a/6: replace every would-die cause, including
                // costs and SBAs, before any graveyard move or dies observer.
                let replaced = if self.anger_replaces_death(*object, *destination) {
                    Zone::Exile
                } else {
                    *destination
                };
                let destination = &replaced;
                let o = self.current(*object)?.clone();
                // CR 111.8: once a token has left the battlefield it cannot move again.
                if self.state.rules.tokens.contains_key(&object.card) && o.zone != Zone::Battlefield
                {
                    return Err("token cannot move again after leaving battlefield".into());
                }
                self.retain_source_lki(*object)?;
                let owner = self
                    .state
                    .rules
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
                if self
                    .state
                    .rules
                    .match_state
                    .as_ref()
                    .is_some_and(|m| m.setup.is_none())
                    && o.zone == (Zone::Library { owner })
                    && *destination == (Zone::Hand { owner })
                {
                    let oracle_id = self.state.rules.cards[&object.card].oracle_id.clone();
                    self.remember_draw(owner, oracle_id);
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
                    .rules
                    .commander_returns
                    .retain(|id| id.card != object.card);
                if self.state.rules.commanders.history(object.card).is_some()
                    && matches!(destination, Zone::Graveyard { .. } | Zone::Exile)
                {
                    self.state.rules.commander_returns.insert(successor);
                }
                let controller = if *destination == Zone::Battlefield {
                    o.controller
                } else {
                    owner
                };
                self.remove_from_combat(*object);
                for library in self.state.rules.libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                if let Zone::Library { owner } = destination {
                    self.state
                        .rules
                        .libraries
                        .entry(*owner)
                        .or_default()
                        .push(object.card);
                }
                // CR 614.1c/614.12: this source replacement modifies entry,
                // rather than tapping the permanent in a subsequent event.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                let enters_tapped = *destination == Zone::Battlefield
                    && self
                        .definition(object.card)?
                        .static_abilities
                        .iter()
                        .any(|a| a.effect == StaticEffect::EntersTapped);
                self.state.rules.objects.insert(
                    object.card,
                    Object {
                        attached_to: None,
                        plus_one_counters: Quantity::zero(),
                        marked_damage: Quantity::zero(),
                        attack_ready: false,
                        deathtouch_damage: false,
                        id: successor,
                        controller,
                        default_controller: controller,
                        zone: *destination,
                        tapped: enters_tapped,
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
        // This legacy typed leaf means combat damage. Preserve the same
        // per-source/player occurrence invariant as transactional combat.
        if damage.len() > self.damage_limit() {
            return Err(self.capacity_error(RuleFault::DamageCapacityExceeded {
                capacity: self.damage_limit(),
                requested: damage.len(),
            }));
        }
        Self::validate_combat_player_members(damage.iter().map(|d| (d.source, &d.recipient)))?;
        let mut totals: BTreeMap<(u8, u32), Quantity> = BTreeMap::new();
        let mut commander_totals: BTreeMap<(CardId, PlayerId), Quantity> = BTreeMap::new();
        for d in damage {
            if !self.is_battlefield_creature(d.source)
                || self.current(d.source)?.controller != d.controller
                || !self.is_live(d.controller)
                || d.amount <= Quantity::zero()
            {
                return Err("invalid damage source/amount".into());
            }
            let key = match d.recipient {
                DamageRecipient::Player { player } => {
                    if !self.is_live(player) {
                        return Err("departed damage recipient".into());
                    }
                    if self.state.rules.commanders.history(d.source.card).is_some() {
                        commander_totals
                            .entry((d.source.card, player))
                            .or_default()
                            .add(&d.amount);
                    }
                    (0, u32::from(player.0))
                }
                DamageRecipient::Creature { object } => {
                    if !self.is_battlefield_creature(object) {
                        return Err("invalid creature damage recipient".into());
                    }
                    (1, object.card.0)
                }
            };
            totals.entry(key).or_default().add(&d.amount);
        }
        let actual = damage
            .iter()
            .enumerate()
            .map(|(i, d)| {
                Ok(ActualDamage {
                    id: DamageId(i as u32),
                    source: self.object_before(d.source)?,
                    controller: d.controller,
                    recipient: d.recipient.clone(),
                    amount: d.amount.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut results = vec![];
        for ((kind, id), amount) in totals {
            let (recipient, previous, current) = if kind == 0 {
                let player = PlayerId(id as u8);
                let life = &mut self.state.rules.players[player.0 as usize].life;
                let previous = life.clone();
                life.subtract(&amount);
                (DamageRecipient::Player { player }, previous, life.clone())
            } else {
                let object = self
                    .state
                    .rules
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
                .rules
                .commanders
                .history(commander)
                .and_then(|history| history.combat_damage.get(&recipient))
                .cloned()
                .unwrap_or_default();
            self.state
                .rules
                .commanders
                .record_damage(commander, recipient, true, &amount)?;
            let current = self
                .state
                .rules
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
        let life_gains = self.damage_keywords(&actual, None)?;
        Ok(EventOutcome::DamageDealt {
            life_gains,
            damage: damage.to_vec(),
            results,
            commander_damage,
        })
    }
}
