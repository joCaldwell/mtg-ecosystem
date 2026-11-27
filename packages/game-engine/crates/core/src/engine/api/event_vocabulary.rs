//! The vocabulary of committed rule events, their outcomes and the
//! instruction outcomes a resolving invocation records.
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proposal", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedEvent {
    Transform {
        object: ObjectId,
    },
    /// CR 702.37e, 708.8: a face-down permanent turns face up.
    TurnFaceUp {
        object: ObjectId,
    },

    FreezeLifeUntilNextTurn {
        player: PlayerId,
    },
    PhaseOutControlled {
        controller: PlayerId,
    },
    PhaseOutTarget {
        object: ObjectId,
    },
    Mill {
        player: PlayerId,
        count: Quantity,
        cards: Vec<ObjectId>,
    },
    /// CR 401.2, 701.20e: a private look followed by ordering the prefix.
    ReorderLibraryTop {
        player: PlayerId,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
    },
    /// CR 401.2, 401.4: move the remaining privately inspected prefix to
    /// the bottom, still in the library and in the selected order.
    PutLibraryPrefixOnBottom {
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
    },
    /// CR 701.22a: `looked` is the library's top; `top` goes back on top in
    /// that order and `bottom` beneath the rest in that order.
    Scry {
        player: PlayerId,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
    },
    /// CR 701.25a: `graveyard` moves to its owner's graveyard and `top` goes
    /// back on top in that order.
    Surveil {
        player: PlayerId,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
        graveyard: Vec<ObjectId>,
    },
    /// CR 701.24a: a whole resolving shuffle of one library.
    ShuffleLibrary {
        player: PlayerId,
    },
    /// CR 701.24b: shuffle every card except the found singleton, then
    /// put it on top. An unsuccessful search still shuffles.
    ShuffleLibraryToTop {
        player: PlayerId,
        found: Option<ObjectId>,
    },
    /// CR 110.2, 614.1c: a card from a library or graveyard enters under its owner's control,
    /// already tapped.
    EnterTapped {
        object: ObjectId,
    },
    /// CR 701.23e: the found cards, now in their owner's hand, were revealed.
    RevealFound {
        player: PlayerId,
        objects: Vec<ObjectId>,
    },

    /// CR 303.4a/608.3c: entry and legal attachment are one occurrence.
    EnterAttached {
        object: ObjectId,
        target: ObjectId,
    },
    Attach {
        object: ObjectId,
        target: Option<ObjectId>,
    },
    ReversePermanentMana {
        receipt: Box<PermanentManaUndo>,
    },
    FinishLibraryShuffle {
        player: PlayerId,
    },
    DestroyPermanent {
        object: ObjectId,
    },
    DestroyCreature {
        object: ObjectId,
    },

    ShuffleLibraryWord {
        player: PlayerId,
        index: u32,
    },
    CreateTokenCopy {
        controller: PlayerId,
        source: ObjectId,
        original: ObjectId,
    },
    /// CR 702.141a: one token copy of the encore card `source` for each
    /// listed opponent, created at once.
    CreateEncoreTokens {
        controller: PlayerId,
        source: ObjectId,
        defenders: Vec<PlayerId>,
    },
    /// CR 702.116a, 508.4, 608.2f: one token copy of the myriad source
    /// `source` for each accepted defender, tapped and attacking that player
    /// or their chosen planeswalker, all created at once for `creator`.
    CreateMyriadTokens {
        controller: PlayerId,
        source: ObjectId,
        defenders: Vec<MyriadDefender>,
        creator: StackId,
    },
    CreatePredefinedTokens {
        controller: PlayerId,
        source: ObjectId,
        count: u32,
        kind: mtg_contracts::PredefinedToken,
        /// CR 614.1c: the tokens enter tapped.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        tapped: bool,
    },
    CreateCreatureTokens {
        controller: PlayerId,
        source: ObjectId,
        count: u32,
        token: Box<mtg_contracts::CreatureToken>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        tapped: bool,
    },
    GivePlayerCounters {
        player: PlayerId,
        poison: Quantity,
        energy: Quantity,
    },
    /// CR 122.1, 701.34a: one atomic placement of one or more kinds on a
    /// permanent, in strictly increasing kind order.
    PutCounters {
        object: ObjectId,
        counters: Vec<CounterAmount>,
    },
    RemoveLoreCounter {
        object: ObjectId,
    },
    /// CR 122.3, 704.5q: remove `amount` +1/+1 and `amount` -1/-1 counters.
    AnnihilateCounters {
        object: ObjectId,
        amount: Quantity,
    },
    CeaseSpellCopy {
        object: ObjectId,
    },
    CeaseToken {
        object: ObjectId,
    },
    Setup {
        action: SetupAction,
    },

    CreateEffect {
        effect: Box<ResolvedEffect>,
    },
    ApplyDamageEffect {
        transaction: DamageTransactionId,
        damage: DamageId,
        effect: EffectKey,
    },
    CommitDamage {
        transaction: DamageTransactionId,
    },
    CleanupEffects,
    GainLife {
        player: PlayerId,
        amount: Quantity,
    },
    LoseLife {
        player: PlayerId,
        amount: Quantity,
    },
    CounterStackObject {
        target: StackId,
        countering: StackId,
        domain: TargetDomain,
    },
    Sacrifice {
        object: ObjectId,
        player: PlayerId,
    },
    DiscardCard {
        object: ObjectId,
        player: PlayerId,
    },
    PayManaSelected {
        player: PlayerId,
        cost: ManaCost,
        spending: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        restricted: Vec<RestrictedMana>,
    },
    PutOnTopOwnerLibrary {
        object: ObjectId,
    },

    Lose {
        player: PlayerId,
        reason: PlayerLossReason,
    },
    DeclareAttackers {
        attacks: Vec<(ObjectId, PlayerId, Option<ObjectId>)>,
    },
    DeclareBlockers {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    FailedDraw {
        player: PlayerId,
    },
    ClearDamage,
    ConsumeDeathtouchWindow {
        objects: Vec<ObjectId>,
    },
    StateBasedGraveyard {
        object: ObjectId,
        reason: CreatureLossReason,
    },
    Untap {
        object: ObjectId,
    },
    ClearMana {
        player: PlayerId,
    },
    LandPlayed {
        player: PlayerId,
    },
    TurnTransition {
        next: TurnState,
        owner: PlayerId,
    },
    ReverseMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        remove_zero: bool,
    },
    Tap {
        object: ObjectId,
    },
    AddMana {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: ObjectId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restriction: Option<ManaSpendRestriction>,
    },
    PayMana {
        player: PlayerId,
        cost: ManaCost,
    },
    Move {
        object: ObjectId,
        destination: Zone,
    },
    LeaveGame {
        object: ObjectId,
        departing_owner: PlayerId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveOutcome {
    pub previous: ObjectId,
    pub successor: ObjectId,
    pub from: Zone,
    pub destination: Zone,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventOutcome {
    Transformed {
        object: ObjectId,
    },
    /// CR 702.37b: `counter` records the megamorph +1/+1 counter.
    TurnedFaceUp {
        object: ObjectId,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        counter: bool,
    },
    LifeFrozenUntilNextTurn {
        player: PlayerId,
    },
    LifeChangeProhibited {
        player: PlayerId,
    },
    PermanentsPhasedOut {
        /// Controller of the directly phased permanents at phase-out (CR 702.26a).
        controller: PlayerId,
        members: Vec<(ObjectId, PhaseReturn)>,
    },
    Milled {
        player: PlayerId,
        movements: Vec<MoveOutcome>,
    },
    LibraryTopReordered {
        player: PlayerId,
        looked: Vec<ObjectId>,
        top: Vec<ObjectId>,
    },
    LibraryPrefixBottomed {
        player: PlayerId,
        looked: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
    },
    Scried {
        player: PlayerId,
        top: Vec<ObjectId>,
        bottom: Vec<ObjectId>,
    },
    Surveilled {
        player: PlayerId,
        top: Vec<ObjectId>,
        movements: Vec<MoveOutcome>,
    },
    FoundRevealed {
        player: PlayerId,
        objects: Vec<ObjectId>,
    },

    EnteredAttached {
        movement: MoveOutcome,
        target: ObjectId,
        attachment_timestamp: u64,
    },
    Regenerated {
        object: ObjectId,
        shield: EffectId,
        tapped: bool,
    },
    DestructionPrevented {
        object: ObjectId,
    },
    CounterPrevented {
        target: StackId,
    },
    /// CR 702.116a, 608.2d: the controller accepted a myriad copy for this
    /// opponent; the copies are created together afterwards (CR 608.2f).
    MyriadAccepted {
        defender: MyriadDefender,
    },
    AttachmentChanged {
        object: ObjectId,
        previous: Option<ObjectId>,
        target: Option<ObjectId>,
    },
    PermanentManaReversed {
        player: PlayerId,
        source: ObjectId,
    },
    LibraryShuffled {
        player: PlayerId,
        count: u64,
    },
    LibraryShuffledWord {
        player: PlayerId,
        index: u32,
        word: u32,
        selected: Option<u32>,
        position: OpenStream,
    },
    TokenCopied {
        object: ObjectId,
        controller: PlayerId,
        original: ObjectId,
    },
    EncoreTokensCreated {
        objects: Vec<ObjectId>,
        controller: PlayerId,
        original: ObjectId,
    },
    MyriadTokensCreated {
        objects: Vec<ObjectId>,
        controller: PlayerId,
        original: ObjectId,
        /// Each token's defender, or `None` when it entered without attacking
        /// (CR 506.3b, 508.4a).
        attacking: Vec<Option<MyriadDefender>>,
    },
    PredefinedTokensCreated {
        objects: Vec<ObjectId>,
        controller: PlayerId,
        kind: mtg_contracts::PredefinedToken,
    },
    TokensCreated {
        objects: Vec<ObjectId>,
        controller: PlayerId,
        token: Box<mtg_contracts::CreatureToken>,
    },

    PlayerCountersPlaced {
        player: PlayerId,
        poison: Quantity,
        energy: Quantity,
        previous_poison: Quantity,
        previous_energy: Quantity,
    },
    CountersPlaced {
        object: ObjectId,
        placed: Vec<CounterChange>,
    },
    LoreCounterRemoved {
        object: ObjectId,
        previous: Quantity,
    },
    CountersAnnihilated {
        object: ObjectId,
        amount: Quantity,
    },
    /// CR 122.1d: the untap was replaced by removing one stun counter.
    StunCounterRemoved {
        object: ObjectId,
        previous: Quantity,
    },
    SpellCopyCeased {
        object: ObjectId,
    },
    TokenCeased {
        object: ObjectId,
    },
    Setup {
        actual: SetupOutcome,
    },

    EffectCreated {
        effect: Box<ResolvedEffect>,
    },
    DamageEffectApplied {
        application: Box<DamageApplication>,
    },
    DamageCommitted {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        transaction: DamageTransactionId,
        actual: Vec<ActualDamage>,
        results: Vec<DamageResult>,
        commander_damage: Vec<CommanderDamageResult>,
        cancelled: Vec<DamageId>,
    },
    EffectsCleaned {
        expired: Vec<EffectId>,
        previous: Vec<(ObjectId, Quantity)>,
    },
    LifeGained {
        player: PlayerId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
    },
    LifeLost {
        player: PlayerId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
    },
    StackObjectCountered {
        target: StackId,
        source: ObjectId,
        spell: bool,
        movement: Option<MoveOutcome>,
    },
    Sacrificed {
        player: PlayerId,
        movement: MoveOutcome,
    },
    CardDiscarded {
        player: PlayerId,
        movement: MoveOutcome,
    },
    ManaPaidSelected {
        player: PlayerId,
        cost: ManaCost,
        spending: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        restricted: Vec<RestrictedMana>,
    },
    LibraryTopMoved {
        movement: MoveOutcome,
        previous_top: Option<CardId>,
    },
    Lost {
        player: PlayerId,
        reason: PlayerLossReason,
    },
    AttackersDeclared {
        attacks: Vec<(ObjectId, PlayerId, Option<ObjectId>)>,
    },
    BlockersDeclared {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    DrawFailed {
        player: PlayerId,
    },
    DeathtouchWindowConsumed {
        objects: Vec<ObjectId>,
    },
    DamageCleared {
        previous: Vec<(ObjectId, Quantity)>,
    },
    StateBasedMoved {
        movement: MoveOutcome,
        reason: CreatureLossReason,
    },
    Untapped {
        object: ObjectId,
    },
    ManaCleared {
        player: PlayerId,
        previous: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        previous_restricted: Vec<RestrictedMana>,
    },
    LandPlayCounted {
        player: PlayerId,
        previous: u32,
        current: u32,
    },
    TurnTransitioned {
        phased_in: Vec<ObjectId>,
        previous: TurnState,
        current: TurnState,
        previous_owner: PlayerId,
        owner: PlayerId,
    },
    ManaReversed {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        source: Option<ObjectId>,
        removed_entry: bool,
    },
    LeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    Tapped {
        object: ObjectId,
    },
    ManaAdded {
        player: PlayerId,
        color: Color,
        amount: Quantity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restriction: Option<ManaSpendRestriction>,
    },
    ManaPaid {
        player: PlayerId,
        cost: ManaCost,
    },
    Moved {
        movement: MoveOutcome,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstructionOutcome {
    Kernel {
        outcomes: Vec<InstructionOutcome>,
    },

    CounterPlacements {
        actual: Vec<EventOutcome>,
    },
    CounterPrevented {
        target: StackId,
    },
    /// CR 702.116a, 608.2d: the controller accepted a myriad copy for this
    /// opponent; the copies are created together afterwards (CR 608.2f).
    MyriadAccepted {
        defender: MyriadDefender,
    },
    SetMoved {
        movements: Vec<MoveOutcome>,
        destroyed_creatures: usize,
    },
    Effect {
        actual: Option<EventOutcome>,
    },
    Damage {
        actual: Option<EventOutcome>,
    },
    Life {
        actual: Option<EventOutcome>,
    },
    StackTargetIllegal {
        target: StackId,
    },
    StackObjectNoLongerPresent {
        target: StackId,
    },
    Countered {
        target: StackId,
        movement: Option<MoveOutcome>,
    },
    PlayerActionSkipped {
        player: PlayerId,
    },
    Sacrificed {
        player: PlayerId,
        movement: Option<MoveOutcome>,
    },
    Drawn {
        player: PlayerId,
        movement: Option<MoveOutcome>,
        failed: bool,
    },
    EachPlayerDrawAttempts {
        players: Vec<PlayerDrawAttempts>,
    },
    DrawAttempts {
        player: PlayerId,
        attempted: Quantity,
        failed: Quantity,
    },
    Discarded {
        player: PlayerId,
        movements: Vec<MoveOutcome>,
    },
    Moved {
        movement: MoveOutcome,
    },
    IllegalTarget {
        target: ObjectId,
    },
    ObjectNoLongerPresent {
        target: ObjectId,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    GraveyardReplacementRevealed {
        cause: BatchCause,
        objects: Vec<ObjectId>,
    },
    SpellCopied {
        cause: BatchCause,
        target: StackId,
        copy: StackId,
        object: ObjectId,
    },
    XAnnounced {
        stack_id: StackId,
        player: PlayerId,
        value: Quantity,
    },
    ModeSubsetSelected {
        stack_id: StackId,
        player: PlayerId,
        modes: Vec<u32>,
    },
    ModeSelected {
        stack_id: StackId,
        player: PlayerId,
        mode: u32,
    },
    TriggerConditionChecked {
        source: ObjectId,
        controller: PlayerId,
        ability: EffectiveTriggerAbility,
        stage: InterveningStage,
        check: InterveningCheck,
    },
    TriggeredAbilityNotPlaced {
        trigger_id: TriggerId,
        stack_id: StackId,
        reason: TriggerPlacementFailure,
    },
    TriggerCollected {
        trigger_id: TriggerId,
        source: ObjectId,
        player: PlayerId,
        ability: EffectiveTriggerAbility,
        occurrence: TriggerOccurrenceId,
        witness: TriggerEventWitness,
        source_facts: Box<ObjectBefore>,
    },
    TriggersOrdered {
        player: PlayerId,
        order: Vec<TriggerId>,
    },
    TriggerPlaced {
        trigger_id: TriggerId,
        stack_id: StackId,
        player: PlayerId,
    },
    TriggeredAbilityCompleted {
        stack_id: StackId,
        resolved: bool,
    },
    AbilityActivated {
        stack_id: StackId,
        source: ObjectId,
        player: PlayerId,
        ability: u32,
    },
    AbilityCompleted {
        stack_id: StackId,
    },
    GameDrawn,
    PlayersLost {
        losses: Vec<(PlayerId, PlayerLossReason)>,
    },
    PlayerDeparted {
        player: PlayerId,
    },
    ObjectLeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    ControlEffectEnded {
        effect_id: u32,
    },
    ControlChanged {
        object: ObjectId,
        previous: PlayerId,
        controller: PlayerId,
    },
    StackRemovedForDeparture {
        stack_id: StackId,
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
    },
    Committed {
        proposed: ProposedEvent,
        actual: EventOutcome,
    },
    SpellProposed {
        card: CardId,
        stack_id: StackId,
    },
    SpellCast {
        stack_id: StackId,
    },
    PriorityPassed {
        player: PlayerId,
    },
    SpellCompleted {
        stack_id: StackId,
        resolved: bool,
    },
    CastReversed {
        stack_id: StackId,
        reason: String,
    },
}

/// CR 122.1: `amount` counters of one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterAmount {
    pub counter: mtg_contracts::kir::CounterKind,
    pub amount: Quantity,
}

/// CR 122.1: one kind's placement and the counts around it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterChange {
    pub counter: mtg_contracts::kir::CounterKind,
    pub amount: Quantity,
    pub previous: Quantity,
    pub current: Quantity,
}

/// CR 702.116a: an opponent, or a chosen planeswalker they control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MyriadDefender {
    pub player: PlayerId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planeswalker: Option<ObjectId>,
}
