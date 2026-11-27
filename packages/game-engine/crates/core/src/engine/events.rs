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
        #[serde(default, skip_serializing_if = "CardFace::is_front")]
        face: CardFace,
    },
    TurnProgression,
    UntapStep,
    PrecombatMainLore,
    DrawStep {
        player: PlayerId,
    },
    InstructionDraw {
        stack_id: StackId,
        player: PlayerId,
        source: ObjectId,
        cursor: u32,
        source_block: u32,
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
        #[serde(default, skip_serializing_if = "CardFace::is_front")]
        face: CardFace,
        /// CR 702.37c, 708.4: turned face down before it is put on the stack.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        face_down: bool,
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
    /// CR 116.2b, 702.37e: turning a face-down permanent face up.
    SpecialAction {
        player: PlayerId,
        object: ObjectId,
    },
    ControlReconciliation,
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
            Self::PrecombatMainLore => &["505.4", "714.3c"],
            Self::DrawStep { .. } => &["504.1", "400.7"],
            Self::InstructionDraw { .. } => &["121.1", "121.2", "400.7", "608.2"],
            Self::ManaActivation { .. } => &["106.3", "305.6", "605.3b"],
            Self::Casting { .. } | Self::Payment { .. } => &["601.2"],
            Self::CastReversal { .. } => &["733.1", "733.2"],
            Self::Instruction { .. } => &["400.7", "608.2"],
            Self::SpellCompletion { .. } => &["400.7", "608.2", "608.3a"],
            Self::Departure { .. } => &["800.4a"],
            Self::SpecialAction { .. } => &["116.2b", "702.37e", "708.8"],
            Self::ControlReconciliation => &["110.2", "613.1b", "800.4a", "800.4c"],
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
    /// CR 701.27a: the object's transform count (`Object::transforms`).
    #[serde(default, skip_serializing_if = "state::is_zero_count")]
    pub transforms: u32,
    /// CR 708.2: the object was face down (`Object::face_down`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub face_down: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword_timestamp: Option<u64>,
    /// `Object::keyword_rank` as last known.
    #[serde(default, skip_serializing_if = "state::is_zero_rank")]
    pub keyword_rank: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battlefield_entry_turn: Option<u64>,
    /// CR 702.30a: `Object::control_upkeeps` as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_upkeeps: Option<u64>,
    /// CR 702.84a: `Object::unearthed` as last known.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unearthed: bool,
    /// CR 702.109a: `Object::dashed` as last known.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dashed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<KeywordAbility>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deathtouch_damage: bool,
    /// CR 122.1, 608.2h: the counters as last known.
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub plus_one_counters: Quantity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counters: Vec<CounterStack>,
    /// CR 122.1, 606.4: `Object::loyalty_counters` as last known.
    #[serde(default, skip_serializing_if = "tokens::quantity_is_zero")]
    pub loyalty_counters: Quantity,
    /// CR 606.3: `Object::loyalty_used_turn` as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loyalty_used_turn: Option<u64>,
    /// CR 301.5, 303.4: `Object::attached_to` and its attachment timestamp
    /// (CR 613.7e) as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_to: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_timestamp: Option<u64>,
    /// CR 702.26: `Object::phased_out` as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phased_out: Option<PhaseReturn>,
    /// CR 702.75: `Object::hideaway` as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hideaway: Option<Box<HideawayLink>>,
    /// CR 715.3d: `Object::adventure_player` as last known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adventure_player: Option<PlayerId>,
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
impl ObjectBefore {
    /// CR 733.1: the exact object row these facts were captured from, for a
    /// reversal that puts it back. Both rows are destructured in full, so a
    /// new `Object` or `ObjectBefore` field fails to compile until it is
    /// captured and restored here.
    pub(in crate::engine) fn restored_object(&self) -> Object {
        let ObjectBefore {
            transforms,
            face_down,
            keyword_timestamp,
            keyword_rank,
            battlefield_entry_turn,
            control_upkeeps,
            unearthed,
            dashed,
            // Derived from the definition and layers, not stored on `Object`.
            keywords: _,
            deathtouch_damage,
            plus_one_counters,
            counters,
            loyalty_counters,
            loyalty_used_turn,
            attached_to,
            attachment_timestamp,
            phased_out,
            hideaway,
            adventure_player,
            object,
            // The physical card's owner, stored on `Card`.
            owner: _,
            controller,
            default_controller,
            zone,
            tapped,
            marked_damage,
            attack_ready,
            characteristics: _,
        } = self.clone();
        Object {
            hideaway,
            phased_out,
            attached_to,
            attachment_timestamp,
            keyword_timestamp,
            keyword_rank,
            battlefield_entry_turn,
            control_upkeeps,
            plus_one_counters,
            loyalty_counters,
            loyalty_used_turn,
            counters,
            transforms,
            adventure_player,
            face_down,
            marked_damage,
            deathtouch_damage,
            unearthed,
            dashed,
            attack_ready,
            id: object,
            controller,
            default_controller,
            zone,
            tapped,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerBefore {
    pub failed_draw: bool,
    pub player: PlayerId,
    pub life: Quantity,
    pub mana: BTreeMap<Color, Quantity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restricted_mana: Vec<RestrictedMana>,
    pub status: PlayerStatus,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentBefore {
    pub object: ObjectId,
    pub target: Option<ObjectId>,
    pub timestamp: Option<u64>,
}
testkit_fields! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct PreEventFacts {
        /// CR 406.3a: these exile objects have no characteristics to capture.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub face_down_exile: Vec<(ObjectId, PlayerId)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(super) control: Option<Box<ControlBefore>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub next_attachment_timestamp: Option<u64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub attachments: Vec<AttachmentBefore>,
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
}
/// Consumed layer-2 authority, separate from the leaf's ordinary object facts
/// (whose narrow shape is part of destruction and damage validation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlBefore {
    pub objects: Vec<ObjectBefore>,
    pub attachments: Vec<AttachmentBefore>,
    pub(super) effects: Vec<ControlEffect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) resolved_grants: Vec<ResolvedEffect>,
    pub live_players: Vec<PlayerId>,
    pub next_attachment_timestamp: u64,
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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub counter_changes: Vec<CounterChange>,
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
    ManaAbilityUse(CardId),
    ActivationUse(CardId),
    AttachmentAllocation,
    TokenAllocation,
    Stack(StackId),
    Turn,
    Object(CardId),
    Mana(PlayerId),
    Life(PlayerId),
    PlayerCounters(PlayerId),
    CommanderDamage(PlayerId),
    Libraries,
    AllDamage,
    Effects,
    PendingTriggers,
}
// Ephemeral proof from validating a complete root group against the prior
// state. Raw occurrence application receives no authority.
#[derive(Clone, Copy)]
pub(super) enum EntryOverride {
    ForceTapped,
    CreatureController {
        player: PlayerId,
        tapped: bool,
    },
    /// CR 614.12: the entry choice was made (life paid or card revealed)
    /// or declined.
    Choice {
        accepted: bool,
    },
}

#[derive(Clone, Copy)]
enum BatchAuthority<'a> {
    Ordinary,
    SpellToStack {
        player: PlayerId,
        face: CardFace,
        face_down: bool,
    },
    Graveyard(
        &'a BTreeMap<ObjectId, Zone>,
        &'a [regeneration::ShieldMember],
    ),
    CreateRegenerationShield,
    CounterStackObject,
    PlaceCounters,
    PlaceMainPhaseLore,
    PayCounters,
    RemoveLoreCounter,
    /// CR 614.1c, 614.12: the entry replacements chosen for each entrant
    /// of one entry batch, all read against the same prior state.
    BattlefieldEntry {
        entries: &'a [(ObjectId, EntryOverride)],
        land: Option<(ObjectId, PlayerId, CardFace)>,
    },
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

fn changes_control_inputs(event: &ProposedEvent) -> bool {
    matches!(
        event,
        ProposedEvent::PhaseOutControlled { .. }
            | ProposedEvent::PhaseOutTarget { .. }
            | ProposedEvent::Move { .. }
            | ProposedEvent::EnterTapped { .. }
            | ProposedEvent::Surveil { .. }
            | ProposedEvent::DestroyPermanent { .. }
            | ProposedEvent::DestroyCreature { .. }
            | ProposedEvent::StateBasedGraveyard { .. }
            | ProposedEvent::Sacrifice { .. }
            | ProposedEvent::DiscardCard { .. }
            | ProposedEvent::PutOnTopOwnerLibrary { .. }
            | ProposedEvent::EnterAttached { .. }
            | ProposedEvent::Attach { .. }
            | ProposedEvent::ReversePermanentMana { .. }
            | ProposedEvent::CleanupEffects
    ) || matches!(event, ProposedEvent::CreateEffect { effect }
            if matches!(effect.kind, ResolvedEffectKind::GainControl { .. }))
}

fn access(event: &ProposedEvent, game: &RulesTxn) -> (BTreeSet<FactKey>, BTreeSet<FactKey>) {
    let (mut reads, mut writes) = match event {
        ProposedEvent::Mill {
            player,
            cards: looked,
            ..
        } => {
            let mut read = BTreeSet::from([FactKey::Libraries, FactKey::Life(*player)]);
            read.extend(looked.iter().map(|o| FactKey::Object(o.card)));
            (read, BTreeSet::from([FactKey::Libraries]))
        }

        ProposedEvent::CreateTokenCopy { original, .. } => (
            BTreeSet::from([FactKey::TokenAllocation, FactKey::Object(original.card)]),
            BTreeSet::from([FactKey::TokenAllocation]),
        ),
        ProposedEvent::CreateEncoreTokens { source, .. } => (
            BTreeSet::from([FactKey::TokenAllocation, FactKey::Object(source.card)]),
            BTreeSet::from([FactKey::TokenAllocation]),
        ),
        ProposedEvent::CreateMyriadTokens { source, .. } => (
            BTreeSet::from([FactKey::TokenAllocation, FactKey::Object(source.card)]),
            BTreeSet::from([FactKey::TokenAllocation]),
        ),
        ProposedEvent::CreateCreatureTokens { .. }
        | ProposedEvent::CreatePredefinedTokens { .. } => {
            let keys = BTreeSet::from([FactKey::TokenAllocation]);
            (keys.clone(), keys)
        }
        ProposedEvent::GivePlayerCounters { player, .. } => {
            let keys = BTreeSet::from([FactKey::PlayerCounters(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::PutCounters { object, .. }
        | ProposedEvent::PayCounterCost { object, .. }
        | ProposedEvent::RemoveLoreCounter { object }
        | ProposedEvent::AnnihilateCounters { object, .. } => {
            // The complete, pure placement group is authorized once against
            // the common prior. Each final leaf writes only its incarnation.
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::CeaseToken { object } | ProposedEvent::CeaseSpellCopy { object } => {
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
        ProposedEvent::ReorderLibraryTop { player, looked, .. }
        | ProposedEvent::PutLibraryPrefixOnBottom { player, looked, .. }
        | ProposedEvent::Scry { player, looked, .. }
        | ProposedEvent::Surveil { player, looked, .. } => {
            let mut read = BTreeSet::from([FactKey::Libraries, FactKey::Life(*player)]);
            read.extend(looked.iter().map(|o| FactKey::Object(o.card)));
            let mut write = BTreeSet::from([FactKey::Libraries]);
            if let ProposedEvent::Surveil { graveyard, .. } = event {
                write.extend(graveyard.iter().map(|o| FactKey::Object(o.card)));
            }
            (read, write)
        }
        ProposedEvent::RevealFound { objects, .. } => {
            let keys: BTreeSet<_> = objects.iter().map(|o| FactKey::Object(o.card)).collect();
            (keys, BTreeSet::new())
        }
        ProposedEvent::FinishLibraryShuffle { .. }
        | ProposedEvent::ShuffleLibrary { .. }
        | ProposedEvent::ShuffleLibraryToTop { .. }
        | ProposedEvent::ShuffleLibraryWord { .. }
        | ProposedEvent::Setup { .. } => {
            let keys = BTreeSet::from([FactKey::Libraries, FactKey::Turn]);
            (keys.clone(), keys)
        }
        ProposedEvent::CreateEffect { .. }
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
            keys.extend(attacks.iter().map(|(o, _, _)| FactKey::Object(o.card)));
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
        ProposedEvent::FailedDraw { player } => {
            let keys = BTreeSet::from([FactKey::Life(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::ConsumeDeathtouchWindow { objects } => {
            let mut keys = BTreeSet::from([FactKey::AllDamage]);
            keys.extend(objects.iter().map(|o| FactKey::Object(o.card)));
            (keys.clone(), keys)
        }
        ProposedEvent::FreezeLifeUntilNextTurn { player } => {
            let keys = BTreeSet::from([FactKey::Life(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::PhaseOutControlled { .. } | ProposedEvent::PhaseOutTarget { .. } => {
            let keys = BTreeSet::from([FactKey::Effects, FactKey::Turn]);
            (keys.clone(), keys)
        }
        ProposedEvent::ClearDamage => {
            let keys = BTreeSet::from([FactKey::AllDamage]);
            (keys.clone(), keys)
        }
        ProposedEvent::EnterAttached { object, target } => {
            let writes =
                BTreeSet::from([FactKey::Object(object.card), FactKey::AttachmentAllocation]);
            let mut reads = writes.clone();
            reads.insert(FactKey::Object(target.card));
            (reads, writes)
        }
        ProposedEvent::Attach { object, target } => {
            let mut writes = BTreeSet::from([FactKey::Object(object.card)]);
            // Two new attachments have semantic timestamp order and cannot
            // silently acquire that order from simultaneous leaf iteration.
            if target.is_some() {
                writes.insert(FactKey::AttachmentAllocation);
            }
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
        ProposedEvent::DiscardCard { object, player } => {
            let reads = BTreeSet::from([FactKey::Object(object.card), FactKey::Life(*player)]);
            (reads, BTreeSet::from([FactKey::Object(object.card)]))
        }
        ProposedEvent::Sacrifice { object, player } => {
            // Sacrifice checks the payer is live but writes only the object.
            // Distinct permanents of the same payer may move simultaneously.
            let reads = BTreeSet::from([FactKey::Object(object.card), FactKey::Life(*player)]);
            (reads, BTreeSet::from([FactKey::Object(object.card)]))
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
        ProposedEvent::Move { object, .. }
        | ProposedEvent::EnterTapped { object }
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
        ProposedEvent::Untap { object }
        | ProposedEvent::Tap { object }
        | ProposedEvent::Transform { object }
        | ProposedEvent::TurnFaceUp { object } => {
            let keys = BTreeSet::from([FactKey::Object(object.card)]);
            (keys.clone(), keys)
        }
        ProposedEvent::AddMana { player, source, .. } => (
            BTreeSet::from([FactKey::Object(source.card), FactKey::Mana(*player)]),
            BTreeSet::from([FactKey::Mana(*player)]),
        ),
        ProposedEvent::ClearMana { player } => {
            let keys = BTreeSet::from([FactKey::Mana(*player)]);
            (keys.clone(), keys)
        }
        ProposedEvent::PayMana { player, .. } | ProposedEvent::PayManaSelected { player, .. } => {
            let writes = BTreeSet::from([FactKey::Mana(*player)]);
            let mut reads = writes.clone();
            if matches!(event, ProposedEvent::PayManaSelected { restricted, .. } if !restricted.is_empty())
                && let Ok(stack) = game.top()
            {
                reads.insert(FactKey::Stack(stack.id));
                reads.insert(FactKey::Object(stack.source.card));
            }
            (reads, writes)
        }
        ProposedEvent::LandPlayed { .. } => (
            BTreeSet::from([FactKey::Turn, FactKey::Effects]),
            BTreeSet::from([FactKey::Turn]),
        ),
        ProposedEvent::TurnTransition { .. } => {
            let keys = BTreeSet::from([FactKey::Turn]);
            (keys.clone(), keys)
        }
        ProposedEvent::ReversePermanentMana { receipt } => {
            let mut keys = BTreeSet::from([
                FactKey::Mana(receipt.player),
                FactKey::Object(receipt.source.object.card),
            ]);
            keys.insert(FactKey::PendingTriggers);
            (keys.clone(), keys)
        }
        ProposedEvent::ReverseMana { player, source, .. } => {
            let mut keys = BTreeSet::from([FactKey::Mana(*player)]);
            if let Some(object) = source {
                keys.insert(FactKey::Object(object.card));
            }
            (keys.clone(), keys)
        }
    };
    // Use history is another incarnation-bound fact. Movement/reversal writes
    // are independent for distinct sources; turn entry clears the current set.
    let keys: Vec<_> = match event {
        ProposedEvent::Move { object, .. }
        | ProposedEvent::EnterTapped { object }
        | ProposedEvent::LeaveGame { object, .. } => {
            vec![
                FactKey::ManaAbilityUse(object.card),
                FactKey::ActivationUse(object.card),
            ]
        }
        ProposedEvent::ReversePermanentMana { receipt } => {
            vec![FactKey::ManaAbilityUse(receipt.source.object.card)]
        }
        ProposedEvent::TurnTransition { .. } => game
            .state
            .rules
            .mana_ability_uses
            .iter()
            .map(|k| FactKey::ManaAbilityUse(k.source.card))
            .chain(
                game.state
                    .rules
                    .activation_uses
                    .iter()
                    .map(|k| FactKey::ActivationUse(k.source.card)),
            )
            .collect(),
        _ => vec![],
    };
    reads.extend(keys.iter().copied());
    writes.extend(keys);
    (reads, writes)
}
fn collect<'a>(
    group: &'a EventGroup,
    game: &RulesTxn,
    depth: u32,
    limit: usize,
    out: &mut Vec<&'a ProposedEvent>,
    library_append: bool,
    cause: &BatchCause,
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
                collect(child, game, depth + 1, limit, out, false, cause)?;
            }
        }
        EventGroup::Simultaneous { occurrences } => {
            if occurrences.is_empty() || occurrences.len() > limit {
                return Err("empty or oversized simultaneous group".into());
            }
            // A departure removal wave is owned by multiplayer's existing
            // refresh boundary, after every losing owner's objects are absent.
            if occurrences
                .iter()
                .any(|e| matches!(e, ProposedEvent::LeaveGame { .. }))
                && occurrences
                    .iter()
                    .any(|e| !matches!(e, ProposedEvent::LeaveGame { .. }))
            {
                return Err("unsupported simultaneous departure/control dependency".into());
            }
            if occurrences.len() > 1
                && occurrences.iter().any(|e| {
                    matches!(
                        e,
                        ProposedEvent::Setup { .. }
                            | ProposedEvent::EnterAttached { .. }
                            | ProposedEvent::CreateEffect { .. }
                            | ProposedEvent::ApplyDamageEffect { .. }
                            | ProposedEvent::CommitDamage { .. }
                            | ProposedEvent::CleanupEffects
                            | ProposedEvent::ReversePermanentMana { .. }
                            | ProposedEvent::CounterStackObject { .. }
                            | ProposedEvent::CreateTokenCopy { .. }
                            | ProposedEvent::CreateEncoreTokens { .. }
                            | ProposedEvent::CreateMyriadTokens { .. }
                            | ProposedEvent::CreateCreatureTokens { .. }
                            | ProposedEvent::CreatePredefinedTokens { .. }
                            | ProposedEvent::FreezeLifeUntilNextTurn { .. }
                            | ProposedEvent::PhaseOutControlled { .. }
                            | ProposedEvent::PhaseOutTarget { .. }
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
            if !independent_footprints(occurrences.iter().map(|event| {
                let (mut reads, mut writes) = access(event, game);
                // CR 704.3: Saga sacrifice and player loss share the SBA
                // sweep. Sacrifice reads liveness from the common prior state.
                if *cause == BatchCause::StateBased
                    && let ProposedEvent::Sacrifice { player, .. } = event
                {
                    reads.remove(&FactKey::Life(*player));
                }
                if library_append {
                    // CR 701.24a,c–d, 903.9b: the authorized paid shuffle
                    // appends distinct cards from one graveyard before randomizing
                    // the whole library. There is no placement-order operand.
                    // This only combines append access. Prior facts, movement
                    // outcomes, commander substitutions and other conflicts remain.
                    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                    reads.remove(&FactKey::Libraries);
                    writes.remove(&FactKey::Libraries);
                }
                (reads, writes)
            })) {
                return Err("unsupported simultaneous read/write dependency".into());
            }
        }
    }
    if out.len() > limit {
        return Err("event occurrence resource limit".into());
    }
    Ok(())
}
impl RulesTxn {
    pub(crate) fn control_before(&self) -> Result<Option<Box<ControlBefore>>, String> {
        let resolved_grants = self
            .state
            .rules
            .effects
            .iter()
            .filter(|effect| matches!(effect.kind, ResolvedEffectKind::GainControl { .. }))
            .cloned()
            .collect::<Vec<_>>();
        let objects = self
            .state
            .rules
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .collect::<Vec<_>>();
        if self.state.rules.control_effects.is_empty()
            && resolved_grants.is_empty()
            && objects
                .iter()
                .all(|object| self.is_live(object.default_controller))
        {
            return Ok(None);
        }
        Ok(Some(Box::new(ControlBefore {
            objects: objects
                .iter()
                .map(|object| self.object_before(object.id))
                .collect::<Result<_, _>>()?,
            attachments: objects
                .iter()
                .filter(|object| object.attached_to.is_some())
                .map(|object| AttachmentBefore {
                    object: object.id,
                    target: object.attached_to,
                    timestamp: object.attachment_timestamp,
                })
                .collect(),
            effects: self.state.rules.control_effects.clone(),
            resolved_grants,
            live_players: self.live_players(),
            next_attachment_timestamp: self.state.rules.next_attachment_timestamp,
        })))
    }

    /// CR 800.4a/c exile is an immediate rule consequence, not an SBA or a
    /// nested general commit. The enclosing rule step/batch owns rollback.
    pub(super) fn exile_without_controller(
        &mut self,
        object: ObjectId,
        authority: Option<Box<ControlBefore>>,
    ) -> Result<(), String> {
        let proposed = ProposedEvent::Move {
            object,
            destination: Zone::Exile,
        };
        let before_event = self.pre_event();
        let mut before = self.facts(&[&proposed])?;
        before.control = authority;
        let batch_id = self.state.rules.next_batch_id;
        self.rules_mut().next_batch_id =
            batch_id.checked_add(1).ok_or("batch ID resource limit")?;
        let cause = BatchCause::ControlReconciliation;
        let mut occurrences = vec![];
        self.apply_leaf(
            &proposed,
            (batch_id, &[], &cause),
            before.clone(),
            &mut occurrences,
            BatchAuthority::Ordinary,
            (&before_event, None),
        )?;
        // CR 603.6c, 800.4a/c: controller reconciliation can move a watched
        // permanent. Collect the actual exile inside the same rule transaction.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        self.publish_actuals(&before_event, &cause, None, &occurrences, false)?;
        self.record_evidence(EvidenceRecord::Batch {
            batch: Box::new(CommittedBatch {
                batch_id,
                rules: cause.rules(),
                cause,
                group: EventGroup::Occurrence { proposed },
                before,
                occurrences,
            }),
        })?;
        Ok(())
    }

    fn facts(&self, events: &[&ProposedEvent]) -> Result<PreEventFacts, String> {
        let mut damage_sources = vec![];
        let effects_event = events.iter().any(|e| {
            matches!(
                e,
                ProposedEvent::CreateEffect { .. }
                    | ProposedEvent::ApplyDamageEffect { .. }
                    | ProposedEvent::CommitDamage { .. }
                    | ProposedEvent::CleanupEffects
            )
        });
        let effects = if effects_event {
            self.state.rules.effects.to_vec()
        } else {
            self.state
                .rules
                .effects
                .iter()
                .filter(|effect| {
                    events.iter().any(|event| {
                            matches!(effect.kind, ResolvedEffectKind::RegenerationShield { target }
                                if regeneration::destruction_object(event) == Some(target)) ||
                            matches!(event, ProposedEvent::LandPlayed { player }
                                if effect.additional_land_plays(*player, self.state.rules.turn.number).is_some())
                        })
                })
                .cloned()
                .collect()
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
                ProposedEvent::Mill {
                    player,
                    cards: looked,
                    ..
                } => {
                    libraries.insert(*player);
                    players.insert(*player);
                    objects.extend(looked.iter().copied());
                }
                ProposedEvent::CreateTokenCopy {
                    controller,
                    original,
                    ..
                } => {
                    players.insert(*controller);
                    objects.insert(*original);
                }
                ProposedEvent::CreateEncoreTokens { controller, .. }
                | ProposedEvent::CreateMyriadTokens { controller, .. }
                | ProposedEvent::CreateCreatureTokens { controller, .. }
                | ProposedEvent::CreatePredefinedTokens { controller, .. } => {
                    // CR608.2m: the invocation owns its fixed token definition.
                    // It reads no current source incarnation after departure.
                    players.insert(*controller);
                }
                ProposedEvent::GivePlayerCounters { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::PutCounters { object, .. }
                | ProposedEvent::PayCounterCost { object, .. }
                | ProposedEvent::RemoveLoreCounter { object }
                | ProposedEvent::AnnihilateCounters { object, .. } => {
                    objects.insert(*object);
                }
                ProposedEvent::CeaseToken { object } | ProposedEvent::CeaseSpellCopy { object } => {
                    objects.insert(*object);
                    if let Zone::Library { owner } = self.current(*object)?.zone {
                        libraries.insert(owner);
                    }
                }
                ProposedEvent::ReorderLibraryTop { player, looked, .. }
                | ProposedEvent::PutLibraryPrefixOnBottom { player, looked, .. }
                | ProposedEvent::Scry { player, looked, .. }
                | ProposedEvent::Surveil { player, looked, .. } => {
                    libraries.insert(*player);
                    players.insert(*player);
                    objects.extend(looked.iter().copied());
                }
                ProposedEvent::RevealFound {
                    player,
                    objects: found,
                } => {
                    players.insert(*player);
                    objects.extend(found.iter().copied());
                }
                ProposedEvent::ShuffleLibraryToTop { player, found } => {
                    libraries.insert(*player);
                    objects.extend(found.iter().copied());
                }
                ProposedEvent::FinishLibraryShuffle { player }
                | ProposedEvent::ShuffleLibrary { player }
                | ProposedEvent::ShuffleLibraryWord { player, .. } => {
                    libraries.insert(*player);
                }
                ProposedEvent::DestroyPermanent { object }
                | ProposedEvent::DestroyCreature { object } => {
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
                        ResolvedEffectKind::RemoveIndestructibleSet { targets, .. }
                        | ResolvedEffectKind::PumpSet { targets, .. }
                        | ResolvedEffectKind::GrantKeywordsSet { targets, .. }
                        | ResolvedEffectKind::GainControl { targets, .. }
                        | ResolvedEffectKind::SetCharacteristics { targets, .. }
                        | ResolvedEffectKind::PumpAnnouncedX { targets, .. }
                        | ResolvedEffectKind::ExileDamaged { targets } => objects.extend(targets),
                        ResolvedEffectKind::RegenerationShield { target }
                        | ResolvedEffectKind::GrantKeywords { target, .. }
                        | ResolvedEffectKind::ModifyPowerToughness { target, .. }
                        | ResolvedEffectKind::CombatProhibition {
                            affects: mtg_contracts::kir::CombatProhibitionScope::Target { target },
                            ..
                        } => {
                            objects.insert(*target);
                        }

                        _ => {}
                    }
                }
                ProposedEvent::ApplyDamageEffect { .. }
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
                                DamageRecipient::Permanent { object } => {
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
                    for (o, p, planeswalker) in attacks {
                        objects.insert(*o);
                        objects.extend(planeswalker);
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
                ProposedEvent::FailedDraw { player } => {
                    players.insert(*player);
                }
                ProposedEvent::ConsumeDeathtouchWindow { objects: checked } => {
                    objects.extend(checked);
                }
                ProposedEvent::FreezeLifeUntilNextTurn { player } => {
                    players.insert(*player);
                }
                ProposedEvent::PhaseOutTarget { object } => {
                    objects.extend(
                        self.installed_phasing()
                            .phase_out(self, phasing::PhaseSelection::Target(*object))?,
                    );
                }
                ProposedEvent::PhaseOutControlled { controller } => {
                    players.insert(*controller);
                    objects.extend(
                        self.installed_phasing()
                            .phase_out_controlled(self, *controller)?,
                    );
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
                ProposedEvent::EnterAttached { object, target } => {
                    objects.insert(*object);
                    objects.insert(*target);
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
                | ProposedEvent::Transform { object }
                | ProposedEvent::TurnFaceUp { object }
                | ProposedEvent::Tap { object }
                | ProposedEvent::EnterTapped { object }
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
                ProposedEvent::ClearMana { player } => {
                    players.insert(*player);
                }
                ProposedEvent::PayMana { player, .. }
                | ProposedEvent::PayManaSelected { player, .. } => {
                    players.insert(*player);
                }
                ProposedEvent::LandPlayed { .. } | ProposedEvent::TurnTransition { .. } => {
                    if let ProposedEvent::TurnTransition { next, owner } = event
                        && next.number != self.state.rules.turn.number
                    {
                        objects.extend(
                            self.installed_phasing()
                                .phase_in_at_untap(self, self.state.rules.turn_owner)?,
                        );
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
        let explicit_attachments = events
            .iter()
            .filter_map(|event| match event {
                ProposedEvent::Attach { object, .. }
                | ProposedEvent::EnterAttached { object, .. } => Some(*object),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let mut attachments = vec![];
        for object in &objects {
            let o = self.current(*object)?;
            if explicit_attachments.contains(object)
                || o.attached_to.is_some()
                || o.attachment_timestamp.is_some()
            {
                attachments.push(AttachmentBefore {
                    object: *object,
                    target: o.attached_to,
                    timestamp: o.attachment_timestamp,
                });
            }
        }
        let mut face_down_exile = Vec::new();
        let mut object_facts = Vec::new();
        for id in objects {
            if self.current(id)?.hideaway.is_some() {
                face_down_exile.push((id, self.state.rules.cards[&id.card].owner));
            } else {
                object_facts.push(self.object_before(id)?);
            }
        }
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
                    restricted_mana: p.restricted_mana.clone(),
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
            face_down_exile,
            control: if events.iter().any(|event| changes_control_inputs(event)) {
                self.control_before()?
            } else {
                None
            },
            next_attachment_timestamp: events
                .iter()
                .any(|event| {
                    matches!(
                        event,
                        ProposedEvent::Attach { .. } | ProposedEvent::EnterAttached { .. }
                    )
                })
                .then_some(self.state.rules.next_attachment_timestamp),
            attachments,
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
            objects: object_facts,
            players,
        })
    }
    // CR 700.1: grouping is explicit. Ordinary trigger matching consumes the
    // actual outcomes and group observer views. General replacement/prevention
    // consumers remain outside these closed profiles.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn commit_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String> {
        self.commit_batch_inner(cause, group, false, &[], false)
    }

    pub(super) fn commit_entry_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
        entries: &[(ObjectId, EntryOverride)],
    ) -> Result<CommittedBatch, String> {
        if entries.is_empty() {
            return Err("an entry batch names its entrants".into());
        }
        self.commit_batch_inner(cause, group, false, entries, false)
    }

    /// Only the authenticated payment paths can submit counter cost leaves.
    pub(super) fn commit_counter_cost_batch(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
    ) -> Result<CommittedBatch, String> {
        self.commit_batch_inner(cause, group, false, &[], true)
    }

    fn commit_batch_inner(
        &mut self,
        cause: BatchCause,
        group: EventGroup,
        library_append: bool,
        entry_override: &[(ObjectId, EntryOverride)],
        counter_cost: bool,
    ) -> Result<CommittedBatch, String> {
        let replacement = self.prepare_graveyard_batch(&cause, &group)?;
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
        collect(
            &group,
            self,
            0,
            self.event_group_limit(),
            &mut events,
            library_append,
            &cause,
        )?;
        if cause == BatchCause::StateBased {
            for event in &events {
                if let ProposedEvent::Sacrifice { object, .. } = event
                    && self.saga_sacrifice(self.current(*object)?, None)?.as_ref() != Some(*event)
                {
                    return Err("state-based sacrifice requires a finished Saga".into());
                }
            }
        }
        if events.iter().any(|e| {
            matches!(
                e,
                ProposedEvent::PhaseOutControlled { .. }
                    | ProposedEvent::PhaseOutTarget { .. }
                    | ProposedEvent::FreezeLifeUntilNextTurn { .. }
            )
        }) {
            let Frame::Resolve { frame } = &self.state.rules.frame else {
                return Err("phase-out requires a resolving spell".into());
            };
            self.validate_resolving(frame)?;
            // The resolving program must still exist; no instruction authorizes a block.
            self.program_for(self.entry(frame.stack_id)?)?;
            let authorized_block = None;
            if !self.entry(frame.stack_id)?.kind.is_spell()
                || self.entry(frame.stack_id)?.paid.is_none()
                || self.top()? != self.entry(frame.stack_id)?
                || !authorized_block.is_some_and(|block| {
                    self.instruction_cause(frame, block)
                        .is_ok_and(|expected| cause == expected)
                })
            {
                return Err("status change requires the matching paid spell instruction".into());
            }
        }

        if events
            .iter()
            .any(|e| matches!(e, ProposedEvent::CreateTokenCopy { .. }))
        {
            let (
                Frame::ResolutionChoice {
                    frame,
                    choice: ResolutionChoice::Populate,
                },
                [
                    ProposedEvent::CreateTokenCopy {
                        controller, source, ..
                    },
                ],
            ) = (&self.state.rules.frame, events.as_slice())
            else {
                return Err("copy creation needs a resolving populate choice".into());
            };
            let entry = self.entry(frame.stack_id)?;
            let Some(instruction @ kernel!(Populate { .. })) = self
                .program_for(entry)?
                .instructions
                .get(frame.cursor as usize)
            else {
                return Err("copy creation outside populate instruction".into());
            };
            if *controller != entry.controller
                || *source != entry.source
                || cause != self.instruction_cause(frame, instruction.source_block())?
            {
                return Err("unbound populate token event".into());
            }
        }

        let placing = matches!(&self.state.rules.frame, Frame::CounterPlacement { transaction } if transaction.cause == cause);
        let regeneration_members;
        let land = match &cause {
            BatchCause::LandPlay {
                player,
                source,
                face,
                ..
            } => Some((*source, *player, *face)),
            _ => None,
        };
        let authority = if counter_cost {
            let (BatchCause::AbilityActivation { source, .. }
            | BatchCause::ManaActivation { source, .. }) = cause
            else {
                return Err("counter cost requires activation payment authority".into());
            };
            if events
                .iter()
                .filter_map(|event| match event {
                    ProposedEvent::PayCounterCost { object, .. } => Some(*object),
                    _ => None,
                })
                .ne(std::iter::once(source))
            {
                return Err("counter cost must pay once from the activating source".into());
            }
            BatchAuthority::PayCounters
        } else if !entry_override.is_empty() || land.is_some() {
            BatchAuthority::BattlefieldEntry {
                entries: entry_override,
                land,
            }
        } else if cause == BatchCause::PrecombatMainLore {
            if self.main_phase_lore_group()?.as_ref() != Some(&group) {
                return Err("unauthorized precombat main lore placement".into());
            }
            BatchAuthority::PlaceMainPhaseLore
        } else if placing
            || events.iter().any(|e| {
                matches!(
                    e,
                    ProposedEvent::PutCounters { .. } | ProposedEvent::GivePlayerCounters { .. }
                )
            })
        {
            self.authorize_counter_batch(&cause, &group)?;
            BatchAuthority::PlaceCounters
        } else if events
            .iter()
            .any(|event| matches!(event, ProposedEvent::RemoveLoreCounter { .. }))
        {
            self.authorize_lore_removal(&cause, &group)?;
            BatchAuthority::RemoveLoreCounter
        } else if events
            .iter()
            .any(|event| regeneration::creates_shield(event))
        {
            let Frame::Resolve { frame } = &self.state.rules.frame else {
                return Err("regeneration shield requires a resolving instruction".into());
            };
            let EventGroup::Occurrence {
                proposed: ProposedEvent::CreateEffect { effect },
            } = &group
            else {
                return Err("regeneration shield requires a single occurrence".into());
            };
            self.validate_resolving(frame)?;
            if cause != self.instruction_cause(frame, effect.origin.source_block)? {
                return Err("regeneration shield creation cause mismatch".into());
            }
            BatchAuthority::CreateRegenerationShield
        } else if events
            .iter()
            .any(|event| matches!(event, ProposedEvent::CounterStackObject { .. }))
        {
            self.authorize_counter(&cause, &group)?;
            BatchAuthority::CounterStackObject
        } else if let Some(proof) = &replacement {
            regeneration_members = match &self.state.rules.frame {
                Frame::GraveyardGroup { tx } if tx.is_publication(&cause, &group) => {
                    tx.shields.clone()
                }
                _ => vec![],
            };
            BatchAuthority::Graveyard(proof, &regeneration_members)
        } else if let (
            BatchCause::Casting {
                player,
                source,
                face,
                face_down,
                ..
            },
            EventGroup::Occurrence {
                proposed:
                    ProposedEvent::Move {
                        object,
                        destination: Zone::Stack,
                    },
            },
        ) = (&cause, &group)
            && source == object
        {
            BatchAuthority::SpellToStack {
                player: *player,
                face: *face,
                face_down: *face_down,
            }
        } else {
            BatchAuthority::Ordinary
        };
        let before = self.facts(&events)?;
        let batch_id = self.state.rules.next_batch_id;
        let next_batch = batch_id.checked_add(1).ok_or("batch ID resource limit")?;
        if self.in_rule_step {
            // The enclosing rule-step staging owns rollback. No intermediate
            // state is observable, and every failed batch aborts that step.
            return self.apply_batch(cause, group, before, batch_id, next_batch, authority);
        }
        let mut staged = self.fork();
        match staged.apply_batch(cause, group, before, batch_id, next_batch, authority) {
            Ok(result) => {
                self.commit_staged_state(staged);
                self.state_token = Arc::new(());
                Ok(result)
            }
            Err(error) => {
                self.rules_mut().typed_fault = staged.state.rules.typed_fault.clone();
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
        authority: BatchAuthority<'_>,
    ) -> Result<CommittedBatch, String> {
        self.rules_mut().next_batch_id = next_batch;
        let mut occurrences = vec![];
        self.apply_group(
            &group,
            (batch_id, &cause),
            &mut vec![],
            &mut occurrences,
            Some(&before),
            authority,
        )?;
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
        (batch, cause): (u64, &BatchCause),
        path: &mut Vec<u32>,
        out: &mut Vec<OccurrenceResult>,
        prepared: Option<&PreEventFacts>,
        authority: BatchAuthority<'_>,
    ) -> Result<(), String> {
        match group {
            EventGroup::Occurrence { proposed } => {
                let before_event: PreEvent = self.pre_event();
                let offset = out.len();
                let before = match prepared {
                    Some(before) => before.clone(),
                    None => self.facts(&[proposed])?,
                };
                self.apply_leaf(
                    proposed,
                    (batch, path, cause),
                    before,
                    out,
                    authority,
                    (&before_event, None),
                )?;
                self.publish_actuals(
                    &before_event,
                    cause,
                    None,
                    &out[offset..],
                    changes_control_inputs(proposed),
                )?;
            }
            EventGroup::Ordered { children } => {
                for (i, child) in children.iter().enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_group(child, (batch, cause), path, out, None, authority)?;
                    path.pop();
                }
            }
            EventGroup::Simultaneous { occurrences } => {
                // Validate every leaf against the same pre-state. Collection
                // checks independence and any explicit append authority.
                let common = self.fork();
                let offset = out.len();
                let mut facts = vec![];
                for event in occurrences {
                    let before = common.facts(&[event])?;
                    let mut validation = common.fork();
                    if let Err(error) =
                        validation.apply_prepared_occurrence(event, Some(&before), authority)
                    {
                        // The validation branch is discarded, but its typed
                        // resource failure must reach the enclosing atomic
                        // transaction just as an application failure does.
                        self.rules_mut().typed_fault = validation.state.rules.typed_fault.clone();
                        return Err(error);
                    }
                    facts.push(before);
                }
                for (i, (event, before)) in occurrences.iter().zip(facts).enumerate() {
                    path.push(u32::try_from(i).map_err(|_| "event path resource limit")?);
                    self.apply_leaf(
                        event,
                        (batch, path, cause),
                        before,
                        out,
                        authority,
                        (&common, Some(&common)),
                    )?;
                    path.pop();
                }
                let group = GroupId {
                    batch,
                    path: path.clone(),
                };
                self.publish_actuals(
                    &common,
                    cause,
                    Some(group),
                    &out[offset..],
                    occurrences.iter().any(changes_control_inputs),
                )?;
            }
        }
        Ok(())
    }
    fn apply_leaf(
        &mut self,
        proposed: &ProposedEvent,
        (batch, path, cause): (u64, &[u32], &BatchCause),
        before: PreEventFacts,
        out: &mut Vec<OccurrenceResult>,
        authority: BatchAuthority<'_>,
        (prior, common): (&Self, Option<&Self>),
    ) -> Result<(), String> {
        let actual =
            self.apply_occurrence_with_entry_state(proposed, Some(&before), authority, common)?;
        let id = OccurrenceId {
            batch,
            path: path.to_vec(),
        };
        let counter_changes =
            self.committed_counter_changes(prior, &id, cause, proposed, &actual)?;
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
            id,
            before,
            proposed: proposed.clone(),
            actual,
            counter_changes,
        });
        Ok(())
    }
    pub(crate) fn commit(
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
    pub(crate) fn move_object_for(
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
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn move_object(
        &mut self,
        object: ObjectId,
        destination: Zone,
    ) -> Result<MoveOutcome, String> {
        self.move_object_for(object, destination, BatchCause::Fixture)
    }
}

impl RulesTxn {
    fn entry_tapped(&self, entering: &Object) -> Result<bool, String> {
        if self.entry_abilities_lost(entering.id)? {
            return Ok(false);
        }
        Ok(self
            .copiable_definition(entering.id)?
            .static_abilities
            .iter()
            .any(|ability| match &ability.effect {
                StaticEffect::EntersTapped => true,
                StaticEffect::EntersTappedIf { condition } => {
                    self.entry_condition(entering, condition)
                }
                _ => false,
            }))
    }
    fn apply_move(
        &mut self,
        object: ObjectId,
        destination: Zone,
        (intended, face_down_cast): (Option<(PlayerId, CardFace)>, bool),
        entry_state: Option<&Self>,
        force_tapped: bool,
        entry_accepted: Option<bool>,
    ) -> Result<EventOutcome, String> {
        let object = &object;
        let destination = &destination;

        // Pinned CR 614.1a/6: replace every would-die cause, including
        // costs and SBAs, before any graveyard move or dies observer.
        let replaced = if self.anger_replaces_death(*object, *destination)
            || self.unearth_replaces_departure(*object, *destination)
        {
            Zone::Exile
        } else {
            *destination
        };
        let destination = &replaced;
        let o = self.current(*object)?.clone();
        // CR 712.8a,f, 712.11-14: use the chosen face on a cast or land
        // play, retain it on spell resolution, and otherwise use the front.
        let face = match *destination {
            Zone::Battlefield if o.zone == Zone::Stack => object.face,
            Zone::Battlefield | Zone::Stack => intended.map_or(CardFace::Front, |(_, face)| face),
            _ => CardFace::Front,
        };
        let face_id = ObjectId { face, ..*object };
        // CR 702.37c, 708.3-708.4: a face-down cast is turned face down
        // before it is put on the stack, and the spell becomes a face-down
        // permanent. Every other move turns the card face up (CR 708.9).
        let face_down_entry = match *destination {
            Zone::Stack => face_down_cast,
            Zone::Battlefield => o.zone == Zone::Stack && o.face_down,
            _ => false,
        };
        if face_down_cast && (*destination != Zone::Stack || face != CardFace::Front) {
            return Err("face-down cast outside the stack".into());
        }
        // CR 304.4, 307.4, 712.14b: an instant or sorcery cannot enter.
        if *destination == Zone::Battlefield
            && matches!(
                self.copiable_definition(face_id)?.characteristics.kind,
                CardKind::Instant | CardKind::Sorcery
            )
        {
            return Err("an instant or sorcery cannot enter the battlefield".into());
        }
        // CR 111.8: once a token has left the battlefield it cannot move again.
        if self
            .state
            .rules
            .tokens
            .get(&object.card)
            .is_some_and(|t| t.is_token(&o))
            && o.zone != Zone::Battlefield
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
        self.rules_mut()
            .mana_ability_uses
            .retain(|k| k.source != *object);
        self.rules_mut()
            .activation_uses
            .retain(|k| k.source != *object);
        // CR 400.7: a zone move creates a new incarnation, and an actual
        // outcome explicitly provides its successor. No stale lookup.
        let successor = ObjectId {
            face,
            card: object.card,
            incarnation: object
                .incarnation
                .checked_add(1)
                .ok_or("incarnation resource limit")?,
        };
        self.rules_mut()
            .commander_returns
            .retain(|id| id.card != object.card);
        if self.state.rules.commanders.history(object.card).is_some()
            && matches!(destination, Zone::Graveyard { .. } | Zone::Exile)
        {
            self.rules_mut().commander_returns.insert(successor);
        }
        let intended_controller = intended.map(|(player, _)| player);
        let controller = if *destination == Zone::Battlefield {
            intended_controller.unwrap_or(o.controller)
        } else if *destination == Zone::Stack {
            intended_controller.unwrap_or(owner)
        } else {
            owner
        };
        self.remove_from_combat(*object);
        self.remove_regeneration_shields(*object);
        for library in self.rules_mut().libraries.values_mut() {
            library.retain(|c| *c != object.card);
        }
        if let Zone::Library { owner } = destination {
            self.rules_mut()
                .libraries
                .entry(*owner)
                .or_default()
                .push(object.card);
        }
        // CR 614.1c/614.12: this source replacement modifies entry,
        // rather than tapping the permanent in a subsequent event.
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let enters_tapped = if *destination != Zone::Battlefield {
            false
        } else if force_tapped {
            true
        } else {
            let prior = entry_state.unwrap_or(self);
            let unpaid = if let Some(choice) = prior.entry_choice(face_id)? {
                match entry_accepted {
                    Some(accepted) => !accepted,
                    None if !prior.entry_choice_open(controller, *object, &choice) => true,
                    None => {
                        return Err(
                            "payable entry requires a battlefield entry continuation".into()
                        );
                    }
                }
            } else {
                false
            };
            unpaid
                || prior.entry_tapped(&Object {
                    id: face_id,
                    controller,
                    ..o.clone()
                })?
        };
        // CR 122.6 and 614.1c: entry counters exist on the successor from
        // the moment it enters, including for entry observers.
        let mut plus_one_counters = Quantity::zero();
        if *destination == Zone::Battlefield && !self.entry_abilities_lost(face_id)? {
            for ability in &self.copiable_definition(face_id)?.static_abilities {
                if let StaticEffect::EntersWithPlusOneCounters { count } = &ability.effect {
                    plus_one_counters.add(count);
                }
            }
        }
        // CR 306.5b: loyalty is placed as the permanent enters, even if
        // continuous effects remove its printed abilities.
        let loyalty_counters = if *destination == Zone::Battlefield && !face_down_entry {
            self.copiable_definition(face_id)?
                .characteristics
                .loyalty
                .clone()
                .unwrap_or_default()
        } else {
            Quantity::zero()
        };
        let (turn, batch) = (self.state.rules.turn.number, self.state.rules.next_batch_id);
        let counters = if *destination == Zone::Battlefield && !face_down_entry {
            let prior = entry_state.unwrap_or(self);
            prior.entry_lore_counters(
                face_id,
                batch.checked_sub(1).ok_or("missing lore entry batch")?,
            )?
        } else {
            vec![]
        };
        let keyword_rank = self.apnap_rank(controller);
        let control_upkeeps = (*destination == Zone::Battlefield)
            .then(|| self.state.rules.players[controller.0 as usize].upkeeps);
        self.rules_mut().objects.insert(
            object.card,
            Object {
                hideaway: None,
                phased_out: None,
                attached_to: None,
                attachment_timestamp: None,
                battlefield_entry_turn: (*destination == Zone::Battlefield).then_some(turn),
                control_upkeeps,
                keyword_timestamp: (*destination == Zone::Battlefield).then_some(batch),
                keyword_rank,
                plus_one_counters,
                loyalty_counters,
                loyalty_used_turn: None,
                counters,
                transforms: 0,
                adventure_player: None,
                // CR 702.37c, 708.4: a face-down spell becomes a face-down
                // permanent; any other zone change turns it face up (708.9).
                face_down: face_down_entry,
                marked_damage: Quantity::zero(),
                attack_ready: false,
                deathtouch_damage: false,
                unearthed: false,
                dashed: false,
                id: successor,
                controller,
                default_controller: controller,
                zone: *destination,
                tapped: enters_tapped,
            },
        );
        // CR 708.9: a face-down permanent or spell leaving for any zone other
        // than the battlefield is revealed to all players as it moves. Public
        // zones show it; a hand or library keeps that public knowledge.
        if o.face_down && !face_down_entry {
            match *destination {
                Zone::Hand { owner } => self.remember_revealed_hand(owner, &[successor])?,
                Zone::Library { .. } => self.remember_revealed_library_top(successor),
                _ => {}
            }
        }
        Ok(EventOutcome::Moved {
            movement: MoveOutcome {
                previous: *object,
                successor,
                from: o.zone,
                destination: *destination,
            },
        })
    }
    fn destruction_is_prevented(
        &self,
        object: ObjectId,
        before: Option<&PreEventFacts>,
    ) -> Result<bool, String> {
        let Some(before) = before else {
            return Ok(self.has_keyword(object, KeywordAbility::Indestructible));
        };
        // The leaf's facts are produced against its pre-event fork, not supplied
        // by a caller. Captured characteristics may include layer-4 effects.
        // Missing facts never mean an unprotected target. Restore verifies the
        // complete prior payload (including keywords) through boundary replay.
        let [fact] = before.objects.as_slice() else {
            return Err("missing or ambiguous destruction prior object".into());
        };
        let current = self.current(object)?;
        self.validate_fact_identity(fact)?;
        if fact.object != object
            || fact.zone != Zone::Battlefield
            || current.zone != fact.zone
            || fact.owner != self.state.rules.cards[&object.card].owner
            || fact.controller != current.controller
        {
            return Err("mismatched destruction prior object".into());
        }
        Ok(fact.keywords.contains(&KeywordAbility::Indestructible))
    }
    pub(crate) fn apply_occurrence(
        &mut self,
        proposed: &ProposedEvent,
    ) -> Result<EventOutcome, String> {
        self.apply_occurrence_with_prior(proposed, None)
    }
    // CR 611.3b, 613.1f, 702.12b, 704.3: simultaneous destruction
    // consumes the common prior layer-6 view. Ordered leaves capture fresh facts.
    // No new snapshot state: this authority is already in occurrence evidence.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    pub(crate) fn apply_occurrence_with_prior(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
    ) -> Result<EventOutcome, String> {
        if matches!(
            proposed,
            ProposedEvent::PhaseOutControlled { .. }
                | ProposedEvent::PhaseOutTarget { .. }
                | ProposedEvent::FreezeLifeUntilNextTurn { .. }
        ) {
            return Err("phase-out requires an authorized instruction batch".into());
        }

        self.apply_prepared_occurrence(proposed, before, BatchAuthority::Ordinary)
    }
    /// CR 400.7, 701.23e: one occurrence, with each physical departure from a
    /// publicly known hand accounted once, around any nested leaves (a
    /// discard is a move).
    fn apply_prepared_occurrence(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
        authority: BatchAuthority<'_>,
    ) -> Result<EventOutcome, String> {
        self.apply_occurrence_with_entry_state(proposed, before, authority, None)
    }

    fn apply_occurrence_with_entry_state(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
        authority: BatchAuthority<'_>,
        entry_state: Option<&Self>,
    ) -> Result<EventOutcome, String> {
        let known_hand = self.known_hand_cards();
        let actual = self.apply_occurrence_leaf(proposed, before, authority, entry_state)?;
        self.forget_departed_hand_cards(known_hand)?;
        Ok(actual)
    }

    /// A nested leaf of an occurrence (a discard's move): its hand
    /// departure is accounted by the enclosing occurrence.
    fn apply_nested_leaf(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
        authority: BatchAuthority<'_>,
    ) -> Result<EventOutcome, String> {
        self.apply_occurrence_leaf(proposed, before, authority, None)
    }

    fn apply_occurrence_leaf(
        &mut self,
        proposed: &ProposedEvent,
        before: Option<&PreEventFacts>,
        authority: BatchAuthority<'_>,
        entry_state: Option<&Self>,
    ) -> Result<EventOutcome, String> {
        let replacement = match authority {
            BatchAuthority::Graveyard(proof, _) => Some(proof),
            _ => None,
        };
        let shields = match authority {
            BatchAuthority::Graveyard(_, shields) => shields,
            _ => &[][..],
        };
        if regeneration::creates_shield(proposed)
            && !matches!(authority, BatchAuthority::CreateRegenerationShield)
        {
            return Err("regeneration shield requires authenticated creation".into());
        }

        let actual = match proposed {
            ProposedEvent::Mill {
                player,
                count,
                cards,
            } => self.apply_mill(*player, count, cards, before, replacement)?,

            ProposedEvent::ReorderLibraryTop {
                player,
                looked,
                top,
            } => self.apply_library_top_reorder(*player, looked, top)?,
            ProposedEvent::PutLibraryPrefixOnBottom {
                player,
                looked,
                bottom,
            } => self.apply_library_prefix_to_bottom(*player, looked, bottom)?,
            ProposedEvent::Scry {
                player,
                looked,
                top,
                bottom,
            } => self.apply_scry(*player, looked, top, bottom)?,
            ProposedEvent::Surveil {
                player,
                looked,
                top,
                graveyard,
            } => self.apply_surveil(*player, looked, top, graveyard, before)?,
            ProposedEvent::ShuffleLibrary { player } => {
                self.apply_library_shuffle(*player, None)?
            }
            ProposedEvent::ShuffleLibraryToTop { player, found } => {
                self.apply_library_shuffle(*player, *found)?
            }
            ProposedEvent::EnterTapped { object } => {
                let owner = self.state.rules.cards[&object.card].owner;
                if !matches!(self.current(*object)?.zone, Zone::Library { owner: p } | Zone::Graveyard { owner: p } | Zone::Hand { owner: p } if p == owner)
                {
                    return Err("tapped entry outside owner's library, hand or graveyard".into());
                }
                // CR 614.1c: a forced tapped entry needs no entry payment.
                self.apply_move(
                    *object,
                    Zone::Battlefield,
                    (Some((owner, CardFace::Front)), false),
                    entry_state,
                    true,
                    None,
                )?
            }
            ProposedEvent::RevealFound { player, objects } => {
                self.apply_found_reveal(*player, objects)?
            }
            ProposedEvent::FinishLibraryShuffle { player } => {
                let group_shuffle =
                    replacement.is_some() && self.publish_group_permutation(*player)?;
                if !group_shuffle {
                    return Err("invalid shuffle completion".into());
                }
                let count = self
                    .rules_mut()
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
            ProposedEvent::EnterAttached { object, target } => {
                // CR 303.4a/608.3c/603.6a: the Aura is already legally
                // attached when entry observers and ETB hooks see it.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                let source = self.current(*object)?;
                {
                    let characteristics = self.structural_characteristics(*object)?;
                    // CR 303.4f: an Aura entering without being cast (from its
                    // owner's library, hand or graveyard) enchants a chosen
                    // object it could enchant; that choice does not target.
                    let owner = self.state.rules.cards[&object.card].owner;
                    let chosen = matches!(source.zone, Zone::Library { owner: p } | Zone::Graveyard { owner: p } | Zone::Hand { owner: p } if p == owner);
                    if !(source.zone == Zone::Stack || chosen)
                        || characteristics.kind != CardKind::Enchantment
                        || characteristics.subtypes != ["Aura"]
                        || !self.is_live(source.controller)
                        || !self.attachment_target_type_valid(*object, *target)?
                        || !self
                            .current(*target)
                            .is_ok_and(|o| o.present_on_battlefield() && self.is_live(o.controller))
                        || (!chosen && !self.targetable_by(*target, source.controller))
                        || self.protected_from(*target, &characteristics)
                    {
                        return Err("invalid attached Aura entry".into());
                    }
                }
                let force_tapped = matches!(authority, BatchAuthority::BattlefieldEntry { entries, .. }
                    if entries.iter().any(|(entrant, entry)| entrant == object
                        && matches!(entry, EntryOverride::ForceTapped)));
                // Check allocation before movement/LKI writes, even for direct
                // private application; the batch/rule-step owns full rollback.
                let (attachment_timestamp, next) = self.next_attachment_allocation()?;
                let EventOutcome::Moved { movement } = self.apply_move(
                    *object,
                    Zone::Battlefield,
                    (None, false),
                    entry_state,
                    force_tapped,
                    None,
                )?
                else {
                    return Err("attached entry movement invariant".into());
                };
                if movement.destination != Zone::Battlefield {
                    return Err("unsupported replaced attached entry".into());
                }
                let batch = self.state.rules.next_batch_id;
                let rank = self.apnap_rank(self.current(movement.successor)?.controller);
                let permanent = self
                    .rules_mut()
                    .objects
                    .get_mut(&movement.successor.card)
                    .ok_or("missing entering Aura")?;
                permanent.attached_to = Some(*target);
                permanent.attachment_timestamp = Some(attachment_timestamp);
                permanent.keyword_timestamp = Some(batch);
                permanent.keyword_rank = rank;
                self.rules_mut().next_attachment_timestamp = next;
                EventOutcome::EnteredAttached {
                    movement,
                    target: *target,
                    attachment_timestamp,
                }
            }
            ProposedEvent::Attach { object, target } => {
                let target_valid = match target {
                    Some(t) => {
                        *t != *object
                            && self.current(*t).is_ok_and(|o| {
                                o.present_on_battlefield() && self.is_live(o.controller)
                            })
                            && self.attachment_target_type_valid(*object, *t)?
                            && !self.protected_from(*t, self.color_layer(*object)?.as_ref())
                    }
                    None => true,
                };
                // CR 704.5p: anything attached may become unattached; only an
                // Aura or Equipment attaches.
                if !self.current(*object)?.present_on_battlefield()
                    || !self.is_live(self.current(*object)?.controller)
                    || (target.is_some()
                        && !self
                            .structural_characteristics(*object)?
                            .subtypes
                            .iter()
                            .any(|s| s == "Equipment" || s == "Aura"))
                    || !target_valid
                {
                    return Err("invalid attachment event".into());
                }
                let previous = self.current(*object)?.attached_to;
                let allocation = if previous != *target && target.is_some() {
                    Some(self.next_attachment_allocation()?)
                } else {
                    None
                };
                let batch = self.state.rules.next_batch_id;
                let rank = self.apnap_rank(self.current(*object)?.controller);
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing attachment")?;
                if previous != *target {
                    o.attached_to = *target;
                    o.attachment_timestamp = allocation.map(|(timestamp, _)| timestamp);
                    if target.is_some() {
                        o.keyword_timestamp = Some(batch);
                        o.keyword_rank = rank;
                    }
                    if let Some((_, next)) = allocation {
                        self.rules_mut().next_attachment_timestamp = next;
                    }
                }
                EventOutcome::AttachmentChanged {
                    object: *object,
                    previous,
                    target: *target,
                }
            }
            ProposedEvent::DestroyPermanent { object } => {
                if !self
                    .current(*object)
                    .is_ok_and(|o| o.present_on_battlefield())
                {
                    return Err("invalid destroy permanent event".into());
                }
                // CR 702.12b, 701.8b, 614.17c: a forbidden destruction
                // never reaches Move or its death-to-exile replacement.
                if self.destruction_is_prevented(*object, before)? {
                    return Ok(EventOutcome::DestructionPrevented { object: *object });
                }
                if let Some(actual) = self.replace_destruction(*object, shields, before)? {
                    return Ok(actual);
                }
                self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard {
                            owner: self.state.rules.cards[&object.card].owner,
                        },
                    },
                    before,
                    authority,
                )?
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
                if let Some(actual) = self.replace_destruction(*object, shields, before)? {
                    return Ok(actual);
                }
                self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard {
                            owner: self.state.rules.cards[&object.card].owner,
                        },
                    },
                    before,
                    authority,
                )?
            }

            ProposedEvent::ShuffleLibraryWord { player, index } => {
                self.apply_search_shuffle_word(*player, *index, None)?
            }

            ProposedEvent::GivePlayerCounters {
                player,
                poison,
                energy,
            } => {
                if !matches!(authority, BatchAuthority::PlaceCounters) {
                    return Err("counter placement requires complete batch authority".into());
                }
                let recipient = &mut self.rules_mut().players[player.0 as usize];
                recipient.poison_counters.add(poison);
                recipient.energy_counters.add(energy);
                EventOutcome::CounterAction
            }
            ProposedEvent::PutCounters { object, counters } => {
                if !matches!(
                    authority,
                    BatchAuthority::PlaceCounters | BatchAuthority::PlaceMainPhaseLore
                ) {
                    return Err("counter placement requires complete batch authority".into());
                }
                // The staged allocation checks which permanents this instruction
                // permits. Proliferate can add to a formerly animated land.
                if !self.current(*object)?.present_on_battlefield()
                    || counters.is_empty()
                    || counters.iter().any(|c| c.amount <= Quantity::zero())
                    || counters.windows(2).any(|w| w[0].counter >= w[1].counter)
                {
                    return Err("invalid counter placement".into());
                }
                // CR 613.7c: this placement's batch timestamps every counter
                // of each placed kind on the permanent. The batch has already
                // reserved its successor ID.
                let timestamp = self
                    .state
                    .rules
                    .next_batch_id
                    .checked_sub(1)
                    .ok_or("missing counter placement batch")?;
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing counter recipient")?;
                for c in counters {
                    o.add_counters(&c.counter, &c.amount, timestamp);
                }
                EventOutcome::CounterAction
            }
            ProposedEvent::PayCounterCost { object, cost } => {
                if !matches!(authority, BatchAuthority::PayCounters) {
                    return Err("counter cost requires payment authority".into());
                }
                self.current(*object)?;
                match cost {
                    CounterCost::Remove { counters } => {
                        self.rules_mut()
                            .objects
                            .get_mut(&object.card)
                            .ok_or("missing source")?
                            .pay_counters(&counters.counter, &counters.amount)?;
                    }
                    CounterCost::Loyalty { change } => {
                        self.pay_loyalty_counters(*object, change)?;
                        self.stamp_loyalty_use(*object)?;
                    }
                }
                EventOutcome::CounterAction
            }
            ProposedEvent::RemoveLoreCounter { object } => {
                if !matches!(authority, BatchAuthority::RemoveLoreCounter) {
                    return Err("lore removal requires direct resolution authority".into());
                }
                let recipient = self
                    .rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing lore recipient")?;
                recipient
                    .remove_counters(&mtg_contracts::kir::CounterKind::Lore, &Quantity::from(1));
                EventOutcome::CounterAction
            }
            ProposedEvent::AnnihilateCounters { object, amount } => {
                // CR 122.3, 704.5q: exactly the smaller of the two counts,
                // only as a state-based action.
                let o = self.current(*object)?;
                let plus = o.plus_one_counters.clone();
                let minus = o.counter_count(&mtg_contracts::kir::CounterKind::MinusOneMinusOne);
                if !o.present_on_battlefield()
                    || *amount <= Quantity::zero()
                    || *amount != std::cmp::min(plus, minus)
                {
                    return Err("invalid counter annihilation".into());
                }
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing annihilation object")?;
                o.remove_counters(&mtg_contracts::kir::CounterKind::PlusOnePlusOne, amount);
                o.remove_counters(&mtg_contracts::kir::CounterKind::MinusOneMinusOne, amount);
                EventOutcome::CounterAction
            }
            event @ (ProposedEvent::CreateTokenCopy { .. }
            | ProposedEvent::CreateEncoreTokens { .. }
            | ProposedEvent::CreateMyriadTokens { .. }
            | ProposedEvent::CreateCreatureTokens { .. }
            | ProposedEvent::CreatePredefinedTokens { .. }
            | ProposedEvent::CeaseToken { .. }
            | ProposedEvent::CeaseSpellCopy { .. }) => self.apply_token_event(event)?,
            ProposedEvent::Setup { action } => EventOutcome::Setup {
                actual: self.apply_setup_action(action)?,
            },
            event @ (ProposedEvent::CreateEffect { .. }
            | ProposedEvent::ApplyDamageEffect { .. }
            | ProposedEvent::CommitDamage { .. }
            | ProposedEvent::CleanupEffects) => self.apply_effect_event(event)?,
            ProposedEvent::FreezeLifeUntilNextTurn { player } => {
                if !self.is_live(*player) {
                    return Err("departed frozen life recipient".into());
                }
                self.rules_mut().players[player.0 as usize].life_frozen_until_next_turn = true;
                EventOutcome::LifeFrozenUntilNextTurn { player: *player }
            }
            ProposedEvent::GainLife { player, amount }
            | ProposedEvent::LoseLife { player, amount } => {
                if !self.is_live(*player) || *amount <= Quantity::zero() {
                    return Err("invalid life change".into());
                }
                if self.state.rules.players[player.0 as usize].life_frozen_until_next_turn {
                    return Ok(EventOutcome::LifeChangeProhibited { player: *player });
                }
                let life = &mut self.rules_mut().players[player.0 as usize].life;
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
            ProposedEvent::CounterStackObject { target, .. } => {
                if !matches!(authority, BatchAuthority::CounterStackObject) {
                    return Err("counter requires authenticated resolution".into());
                }
                let index = self
                    .state
                    .rules
                    .stack
                    .iter()
                    .position(|entry| entry.id == *target)
                    .ok_or("missing counter target")?;
                let entry = &self.state.rules.stack[index];
                if !self.stack_entry_complete(entry) {
                    return Err("counter target is incomplete".into());
                }
                if self.spell_cannot_be_countered(entry)? {
                    return Ok(EventOutcome::CounterPrevented { target: *target });
                }
                let spell = entry.kind.is_spell();
                let source = entry.source;
                let movement = if spell {
                    let owner = self
                        .state
                        .rules
                        .cards
                        .get(&source.card)
                        .ok_or("missing countered spell physical card")?
                        .owner;
                    let destination = self.stack_departure(entry, Zone::Graveyard { owner });
                    let EventOutcome::Moved { movement } = self.apply_nested_leaf(
                        &ProposedEvent::Move {
                            object: source,
                            destination,
                        },
                        before,
                        authority,
                    )?
                    else {
                        return Err("missing countered spell movement".into());
                    };
                    Some(movement)
                } else {
                    None
                };
                self.rules_mut().stack.remove(index);
                EventOutcome::StackObjectCountered {
                    target: *target,
                    source,
                    spell,
                    movement,
                }
            }
            ProposedEvent::PayManaSelected {
                player,
                cost,
                spending,
                restricted,
            } => {
                if !self.is_live(*player) || cost.generic.is_negative() {
                    return Err("invalid payment cost or player".into());
                }
                let payer = &self.state.rules.players[player.0 as usize];
                let mut total = spending.clone();
                for unit in restricted {
                    total.entry(unit.color).or_default().add(&unit.amount);
                }
                mtg_view::validate_selected_mana_cost(cost, &total)?;
                let remaining = mtg_view::subtract_selected_mana(payer.mana.clone(), spending)?;
                let buckets =
                    mtg_view::subtract_restricted_mana(payer.restricted_mana.clone(), restricted)?;
                let payer = &mut self.rules_mut().players[player.0 as usize];
                payer.mana = remaining;
                payer.restricted_mana = buckets;
                EventOutcome::ManaPaidSelected {
                    player: *player,
                    cost: cost.clone(),
                    spending: spending.clone(),
                    restricted: restricted.clone(),
                }
            }
            ProposedEvent::Sacrifice { object, player } => {
                if !entry_state.unwrap_or(self).is_live(*player)
                    || !self.current(*object)?.present_on_battlefield()
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
                let EventOutcome::Moved { movement } = self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard { owner },
                    },
                    before,
                    authority,
                )?
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
                let EventOutcome::Moved { movement } = self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Graveyard { owner: *player },
                    },
                    before,
                    authority,
                )?
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
                let EventOutcome::Moved { movement } = self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination: Zone::Library { owner },
                    },
                    before,
                    authority,
                )?
                else {
                    return Err("top-library movement invariant".into());
                };
                // CR 614.1a: a replacement (unearth's exile) can divert the
                // move; only a card that reached the library goes on top.
                if movement.destination == (Zone::Library { owner }) {
                    let cards = self
                        .rules_mut()
                        .libraries
                        .get_mut(&owner)
                        .ok_or("missing owner library")?;
                    cards.retain(|card| *card != object.card);
                    cards.insert(0, object.card);
                }
                EventOutcome::LibraryTopMoved {
                    movement,
                    previous_top,
                }
            }
            ProposedEvent::Lose { player, reason } => {
                if !self.is_live(*player)
                    || match reason {
                        // The singleton instruction and target were checked at the
                        // batch boundary before any occurrence could change state.
                        PlayerLossReason::Life => {
                            self.state.rules.players[player.0 as usize].life > Quantity::zero()
                        }
                        PlayerLossReason::CommanderDamage => !self
                            .state
                            .rules
                            .commanders
                            .combat_loss_candidates()
                            .contains(player),
                        PlayerLossReason::Poison => {
                            self.state.rules.players[player.0 as usize].poison_counters
                                < Quantity::from(10)
                        }
                        PlayerLossReason::FailedDraw => {
                            !self.state.rules.players[player.0 as usize].failed_draw
                        }
                    }
                {
                    return Err("invalid state-based player loss".into());
                }
                self.rules_mut().losing_players.push(*player);
                EventOutcome::Lost {
                    player: *player,
                    reason: *reason,
                }
            }
            ProposedEvent::DeclareAttackers { attacks } => {
                let player = self.state.rules.active.ok_or("missing attacking player")?;
                self.validate_attacks(player, attacks)?;
                for (object, _, _) in attacks {
                    if !self.has_keyword(*object, KeywordAbility::Vigilance) {
                        self.apply_occurrence(&ProposedEvent::Tap { object: *object })?;
                    }
                }
                if !attacks.is_empty() {
                    self.rules_mut().cast_history[player.0 as usize].attacked = true;
                }
                self.rules_mut().combat_declared = !attacks.is_empty();
                let mut defenders: Vec<_> = attacks
                    .iter()
                    .filter_map(|(_, defender, planeswalker)| {
                        planeswalker.is_none().then_some(*defender)
                    })
                    .collect();
                defenders.sort();
                defenders.dedup();
                self.rules_mut().combat_defenders = defenders;
                self.rules_mut().combat = attacks
                    .iter()
                    .map(|(attacker, defender, planeswalker)| Attacker {
                        planeswalker: *planeswalker,
                        defender_removed: false,
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
                self.validate_blocks(*player, blocks)?;
                for (b, a) in blocks {
                    let attacker = self
                        .rules_mut()
                        .combat
                        .iter_mut()
                        .find(|x| x.attacker == *a)
                        .ok_or("missing blocker attacker")?;
                    attacker.blocked = true;
                    attacker.blockers.push(*b);
                    self.rules_mut().blocking_creatures.insert(*b);
                }
                EventOutcome::BlockersDeclared {
                    player: *player,
                    blocks: blocks.clone(),
                }
            }
            ProposedEvent::FailedDraw { player } => {
                if !self.is_live(*player) || self.top_library_card(*player).is_some() {
                    return Err("invalid failed draw".into());
                }
                self.rules_mut().players[player.0 as usize].failed_draw = true;
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
                    self.rules_mut()
                        .objects
                        .get_mut(&object.card)
                        .ok_or("missing checked deathtouch object")?
                        .deathtouch_damage = false;
                }
                EventOutcome::DeathtouchWindowConsumed {
                    objects: objects.clone(),
                }
            }
            ProposedEvent::PhaseOutControlled { controller } => {
                self.phase_out_permanents(phasing::PhaseSelection::Controlled(*controller))?
            }
            ProposedEvent::PhaseOutTarget { object } => {
                self.phase_out_permanents(phasing::PhaseSelection::Target(*object))?
            }
            ProposedEvent::ClearDamage => {
                let mut previous = vec![];
                for o in self.rules_mut().objects.values_mut() {
                    o.deathtouch_damage = false;
                    if o.marked_damage > Quantity::zero() {
                        previous.push((o.id, std::mem::take(&mut o.marked_damage)));
                    }
                }
                EventOutcome::DamageCleared { previous }
            }
            ProposedEvent::StateBasedGraveyard { object, reason } => {
                let o = self.current(*object)?;
                let facts = match before {
                    Some(prior) => prior
                        .objects
                        .iter()
                        .find(|f| f.object == *object)
                        .ok_or("missing common-prior SBA object")?
                        .clone(),
                    None => self.object_before(*object)?,
                };
                let toughness = facts
                    .characteristics
                    .toughness
                    .as_ref()
                    .ok_or("missing SBA toughness")?;
                if !o.present_on_battlefield()
                    || facts.zone != Zone::Battlefield
                    || !facts.characteristics.has_type(CardKind::Creature)
                    || match reason {
                        CreatureLossReason::ZeroToughness => toughness > &Quantity::zero(),
                        CreatureLossReason::Deathtouch => {
                            toughness <= &Quantity::zero() || !facts.deathtouch_damage
                        }
                        CreatureLossReason::LethalDamage => {
                            toughness <= &Quantity::zero() || &facts.marked_damage < toughness
                        }
                    }
                {
                    return Err("invalid creature SBA".into());
                }
                if *reason != CreatureLossReason::ZeroToughness
                    && facts.keywords.contains(&KeywordAbility::Indestructible)
                {
                    return Err("indestructible creature ignores destruction SBA".into());
                }
                if *reason != CreatureLossReason::ZeroToughness
                    && let Some(actual) = self.replace_destruction(*object, shields, before)?
                {
                    return Ok(actual);
                }
                let destination = Zone::Graveyard {
                    owner: self.state.rules.cards[&object.card].owner,
                };
                let EventOutcome::Moved { movement } = self.apply_nested_leaf(
                    &ProposedEvent::Move {
                        object: *object,
                        destination,
                    },
                    before,
                    authority,
                )?
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
                if !o.present_on_battlefield() || !o.tapped {
                    return Err("invalid untap event".into());
                }
                // CR 122.1d, 614.1a: a stun counter is removed instead, on
                // every untap path (untap step, effects and costs).
                let stun = mtg_contracts::kir::CounterKind::Stun;
                if o.counter_count(&stun) > Quantity::zero() {
                    self.rules_mut()
                        .objects
                        .get_mut(&object.card)
                        .ok_or("missing untap object")?
                        .remove_counters(&stun, &Quantity::from(1));
                    return Ok(EventOutcome::CounterAction);
                }
                self.rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing untap object")?
                    .tapped = false;
                self.mark_chosen_cost_untapped(*object);
                EventOutcome::Untapped { object: *object }
            }
            ProposedEvent::ClearMana { player } => {
                if !self.is_live(*player) {
                    return Err("invalid mana clearing player".into());
                }
                let p = &mut self.rules_mut().players[player.0 as usize];
                let previous = std::mem::take(&mut p.mana);
                let previous_restricted = std::mem::take(&mut p.restricted_mana);
                EventOutcome::ManaCleared {
                    player: *player,
                    previous,
                    previous_restricted,
                }
            }
            ProposedEvent::LandPlayed { player } => {
                if !self.main_phase_action(*player) || !self.has_land_play_allowance(*player) {
                    return Err("invalid land-play counting event".into());
                }
                let previous = self.state.rules.turn.lands_played;
                let current = previous.checked_add(1).ok_or("land play resource limit")?;
                self.rules_mut().turn.lands_played = current;
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
                    self.rules_mut().first_strike_participants = self.first_strikers();
                }
                // CR511.3: remove every combatant as the end of combat step
                // ends, before the separately budgeted postcombat entry unit.
                if self.state.rules.turn.step == TurnStep::EndCombat {
                    self.clear_combat();
                }
                // CR 503.1a, 702.30a: count each player's upkeeps as they begin.
                if next.step == TurnStep::Upkeep && next.step != self.state.rules.turn.step {
                    self.rules_mut().players[owner.0 as usize].upkeeps += 1;
                }
                let previous = std::mem::replace(&mut self.rules_mut().turn, next.clone());
                let previous_owner = std::mem::replace(&mut self.rules_mut().turn_owner, *owner);
                self.rules_mut().active = self.is_live(*owner).then_some(*owner);
                if next.number != previous.number {
                    self.expire_player_protection(*owner);
                    self.rules_mut().players[owner.0 as usize].life_frozen_until_next_turn = false;
                    self.rules_mut().mana_ability_uses.clear();
                    self.rules_mut().activation_uses.clear();
                    self.rules_mut().trigger_uses.clear();
                    self.rules_mut().cast_history =
                        std::array::from_fn(|_| triggers::TurnCastHistory::default());
                    let active = self.state.rules.active;
                    for o in self.rules_mut().objects.values_mut() {
                        if o.zone == Zone::Battlefield && Some(o.controller) == active {
                            o.attack_ready = true;
                        }
                    }
                }
                let phased_in = if next.step == TurnStep::Untap {
                    self.phase_in_for_next_untap(previous_owner)?
                } else {
                    vec![]
                };
                EventOutcome::TurnTransitioned {
                    phased_in,
                    previous,
                    current: next.clone(),
                    previous_owner,
                    owner: *owner,
                }
            }
            // CR 701.27a, 712.18: the same object turns over; every effect
            // that applied to it continues to apply.
            ProposedEvent::Transform { object } => {
                let o = self.current(*object)?;
                let definition = self.definition(object.card)?;
                let next = o.transforms.checked_add(1).ok_or("transform count limit")?;
                if !o.present_on_battlefield()
                    || !definition.transforms()
                    || definition
                        .face(state::face_after(object.face, next))
                        .is_none_or(|face| {
                            matches!(
                                face.characteristics.kind,
                                CardKind::Instant | CardKind::Sorcery
                            )
                        })
                {
                    return Err("invalid transform event".into());
                }
                self.rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing transforming object")?
                    .transforms = next;
                EventOutcome::Transformed { object: *object }
            }
            // CR 702.37e, 708.8: the same permanent turns face up; effects
            // that applied to it still apply, and it does not enter. CR
            // 702.37b: a megamorph cost paid this way adds a +1/+1 counter as
            // it turns face up.
            ProposedEvent::TurnFaceUp { object } => {
                let o = self.current(*object)?;
                let face_down = self
                    .definition(object.card)?
                    .face_down
                    .as_ref()
                    .ok_or("turning face up a card without morph")?;
                if !o.present_on_battlefield() || !o.face_down || !object.face.is_front() {
                    return Err("invalid turn face up event".into());
                }
                let counter = face_down.kind == FaceDownKind::Megamorph;
                // CR 613.7f, 613.7m: a new timestamp, ordered by APNAP.
                let (batch, rank) = (
                    self.state.rules.next_batch_id,
                    self.apnap_rank(o.controller),
                );
                let o = self
                    .rules_mut()
                    .objects
                    .get_mut(&object.card)
                    .ok_or("missing face-down object")?;
                o.face_down = false;
                o.keyword_timestamp = Some(batch);
                o.keyword_rank = rank;
                if counter {
                    o.plus_one_counters.add(&Quantity::from(1));
                }
                // CR 506.4: a permanent that stops being a creature, such as
                // a land or enchantment card with morph, is removed from combat.
                if !self.is_battlefield_creature(*object) {
                    self.remove_from_combat(*object);
                }
                EventOutcome::TurnedFaceUp { object: *object }
            }
            ProposedEvent::Tap { object } => {
                let o = self.current(*object)?;
                if !o.present_on_battlefield() || o.tapped {
                    return Err("invalid tap event".into());
                }
                self.rules_mut()
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
                restriction,
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
                let p = self
                    .rules_mut()
                    .players
                    .get_mut(player.0 as usize)
                    .ok_or("missing mana player")?;
                if let Some(restriction) = restriction {
                    if *amount == Quantity::zero() {
                        return Err("empty restricted mana production".into());
                    }
                    restricted_mana::credit(
                        &mut p.restricted_mana,
                        &RestrictedMana {
                            restriction: *restriction,
                            color: *color,
                            amount: amount.clone(),
                        },
                    );
                } else {
                    p.mana.entry(*color).or_default().add(amount);
                }
                EventOutcome::ManaAdded {
                    player: *player,
                    color: *color,
                    amount: amount.clone(),
                    restriction: *restriction,
                }
            }
            ProposedEvent::PayMana { player, cost } => {
                if cost.generic.is_negative() || !self.is_live(*player) {
                    return Err("invalid payment cost or player".into());
                }
                let pool = self.payment_pool(*player, cost)?;
                self.rules_mut()
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
                        || !self
                            .structural_characteristics(*object)?
                            .has_type(CardKind::Land)
                    {
                        return Err("invalid mana reversal source".into());
                    }
                }
                let pool = &mut self
                    .rules_mut()
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
                    self.rules_mut()
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
                self.rules_mut()
                    .commander_returns
                    .retain(|id| id.card != object.card);
                self.remove_from_combat(*object);
                self.remove_regeneration_shields(*object);
                for library in self.rules_mut().libraries.values_mut() {
                    library.retain(|c| *c != object.card);
                }
                self.rules_mut()
                    .mana_ability_uses
                    .retain(|k| k.source != *object);
                self.rules_mut()
                    .activation_uses
                    .retain(|k| k.source != *object);
                self.rules_mut().objects.remove(&object.card);
                EventOutcome::LeftGame {
                    object: *object,
                    owner: *departing_owner,
                }
            }
            ProposedEvent::Move {
                object,
                destination,
            } => {
                let intended = match authority {
                    BatchAuthority::SpellToStack { player, face, .. } => Some((player, face)),
                    BatchAuthority::BattlefieldEntry {
                        land: Some((source, player, face)),
                        ..
                    } if source == *object && *destination == Zone::Battlefield => {
                        Some((player, face))
                    }
                    _ => None,
                };
                let entry = match authority {
                    BatchAuthority::BattlefieldEntry { entries, .. } => entries
                        .iter()
                        .find(|(entrant, _)| entrant == object)
                        .map(|(_, entry)| *entry),
                    _ => None,
                };
                let (force_tapped, payment) = match entry {
                    Some(EntryOverride::ForceTapped) => (true, None),
                    Some(EntryOverride::CreatureController { tapped, .. }) => (tapped, None),
                    Some(EntryOverride::Choice { accepted }) => (false, Some(accepted)),
                    None => (false, None),
                };
                let intended = match entry {
                    Some(EntryOverride::CreatureController { player, .. }) => {
                        Some((player, object.face))
                    }
                    _ => intended,
                };
                let face_down = matches!(
                    authority,
                    BatchAuthority::SpellToStack {
                        face_down: true,
                        ..
                    }
                );
                self.apply_move(
                    *object,
                    *destination,
                    (intended, face_down),
                    entry_state,
                    force_tapped,
                    payment,
                )?
            }
        };
        self.update_library_position_memory(proposed, &actual);
        Ok(actual)
    }
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
