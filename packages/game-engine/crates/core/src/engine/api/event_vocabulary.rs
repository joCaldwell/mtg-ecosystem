//! The vocabulary of committed rule events, their outcomes and the
//! instruction outcomes a resolving invocation records.
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "proposal", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedEvent {
    Transform {
        object: ObjectId,
    },
    RevealLibraryCard {
        player: PlayerId,
        object: ObjectId,
    },
    BottomRevealedCards {
        player: PlayerId,
        order: Vec<ObjectId>,
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
    RevealHandCardForEntry {
        player: PlayerId,
        card: ObjectId,
    },

    Mill {
        player: PlayerId,
        count: Quantity,
        cards: Vec<ObjectId>,
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
    CascadeLandEntry {
        object: ObjectId,
        player: PlayerId,
    },

    ShuffleLibraryWord {
        player: PlayerId,
        index: u32,
    },
    CreatePredefinedTokens {
        controller: PlayerId,
        source: ObjectId,
        count: u32,
        kind: mtg_contracts::PredefinedToken,
    },
    CreateCreatureTokens {
        controller: PlayerId,
        source: ObjectId,
        count: u32,
        token: Box<mtg_contracts::CreatureToken>,
    },
    ApplyCounterPlacementEffect {
        effect: CounterPlacementEffectKey,
    },

    PutPlusOneCounters {
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
    NoncombatDamage {
        damage: Box<NoncombatDamageOccurrence>,
    },
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
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    DeclareBlockers {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    CombatDamage {
        damage: Vec<DamageOccurrence>,
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tap_triggers: Vec<TriggerId>,
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
    CounterPlacementModified {
        application: CounterPlacementApplication,
    },
    LibraryCardRevealed {
        player: PlayerId,
        object: ObjectId,
    },
    RevealedCardsBottomed {
        player: PlayerId,
        order: Vec<ObjectId>,
    },
    HandCardRevealedForEntry {
        player: PlayerId,
        card: ObjectId,
    },

    Milled {
        player: PlayerId,
        movements: Vec<MoveOutcome>,
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
        before: RngPosition,
        after: RngPosition,
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

    PlusOneCountersPlaced {
        object: ObjectId,
        amount: Quantity,
        previous: Quantity,
        current: Quantity,
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
    NoncombatDamageDealt {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        damage: Box<NoncombatDamageOccurrence>,
        result: DamageResult,
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
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    BlockersDeclared {
        player: PlayerId,
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    DamageDealt {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        life_gains: Vec<LifelinkGain>,
        damage: Vec<DamageOccurrence>,
        results: Vec<DamageResult>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        commander_damage: Vec<CommanderDamageResult>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retained_as: Option<Color>,
    },
    LandPlayCounted {
        player: PlayerId,
        previous: u32,
        current: u32,
    },
    TurnTransitioned {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lore: Vec<LoreChange>,
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

    DelayedChapterRegistered {
        issuer: StackId,
    },

    CounterPlacements {
        actual: Vec<EventOutcome>,
    },
    CounterPrevented {
        target: StackId,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        grant: Option<CascadeGrantProvenance>,
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
