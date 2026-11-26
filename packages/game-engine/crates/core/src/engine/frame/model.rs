//! The suspended frame: where the rules stopped and the progress needed to
//! continue. A frame names invocations by `StackId` and owns none of them.
use crate::engine::*;

/// Index of the immediate parent in the saved frame stack, bottom first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameLink(pub usize);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Casting {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sacrifices: Vec<ObjectId>,
    #[serde(default, skip_serializing_if = "AnnouncementOrigin::is_spell")]
    pub origin: AnnouncementOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spending: Option<BTreeMap<Color, Quantity>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delve: Option<Vec<ObjectId>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spending_restricted: Vec<RestrictedMana>,
    pub stack_id: StackId,
    pub original_object: ObjectId,
    pub prior_passes: Vec<PlayerId>,
    pub mana_activations: Vec<ManaActivationUndo>,
    pub undo_mana: bool,
    /// CR 701.23e: announcing this spell from its owner's hand consumed one
    /// publicly known copy of its identity there; a reversal restores it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub revealed_hand_consumed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaActivationUndo {
    pub object: ObjectId,
    pub player: PlayerId,
    pub color: Color,
    pub amount: Quantity,
    pub mana_was_absent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permanent: Option<Box<PermanentManaUndo>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetDamageFacts {
    pub(in crate::engine) cursor: u32,
    pub(in crate::engine) amount: Quantity,
    pub(in crate::engine) targets: Vec<TargetEntity>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawProgress {
    pub cursor: u32,
    pub source_block: u32,
    pub completed: Quantity,
    pub remaining: Quantity,
    pub failed: Quantity,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolving {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrant_draw: Option<Box<ObjectBefore>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_discard: Option<group_discard::GroupDiscardProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_discard: Option<random_discard::RandomDiscardProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draw_progress: Option<DrawProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub each_player_draw: Option<EachPlayerDrawProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_damage: Option<SetDamageFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fight: Option<Box<[ObjectBefore; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_check: Option<InterveningCheck>,
    pub stack_id: StackId,
    pub cursor: u32,
    pub legal_targets: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_facts: Vec<Option<TargetFacts>>,
    pub outcomes: Vec<InstructionOutcome>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum Frame {
    BattlefieldEntry {
        entry: Box<battlefield_entry::PendingEntry>,
    },
    Kernel {
        task: Box<kir::Task>,
    },
    GraveyardGroup {
        tx: Box<GraveyardGroup>,
    },

    /// CR305.6/605.3: select an intrinsic ability before activation or costs.
    ManaSourceChoice {
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
        resume: FrameLink,
    },

    LegendRule {
        resume: FrameLink,
        groups: Vec<Vec<ObjectId>>,
        kept: Vec<ObjectId>,
    },
    AdditionalCosts {
        cast: Casting,
    },
    ChooseModes {
        cast: Casting,
    },
    AnnounceX {
        cast: Casting,
    },
    HybridCost {
        cast: Casting,
    },
    SacrificeCost {
        cast: Casting,
    },
    ChosenObjectCost {
        cast: Casting,
        selected: Vec<ObjectId>,
    },
    ForcedSacrifice {
        frame: Resolving,
        remaining: Vec<PlayerId>,
        choices: Vec<(PlayerId, ObjectId)>,
        source_block: u32,
    },
    ResolutionPayment {
        frame: Resolving,
        payment: ResolutionPayment,
    },

    ManaAbility {
        activation: Box<ManaActivation>,
    },
    Setup,

    CounterPlacement {
        transaction: CounterPlacementTransaction,
    },
    DamageEffects {
        transaction: DamageTransaction,
    },
    TriggerOrder {
        placement: TriggerPlacement,
    },
    TriggerTargets {
        placement: TriggerPlacement,
        stack_id: StackId,
    },
    EntityTargets {
        cast: Casting,
    },
    StackTargets {
        cast: Casting,
    },
    PlayerTargets {
        cast: Casting,
    },
    SpendMana {
        cast: Casting,
    },
    ResolutionChoice {
        frame: Resolving,
        choice: ResolutionChoice,
    },
    CommanderReturn {
        object: ObjectId,
        destination: Zone,
        continuation: CommanderContinuation,
    },
    Attackers,
    Blockers {
        remaining: Vec<PlayerId>,
    },
    Damage,
    DamageChoice {
        source: ObjectId,
    },
    Discard {
        player: PlayerId,
        count: usize,
    },
    CleanupClear,
    Stabilize {
        resume: FrameLink,
        cleanup: bool,
        changed: bool,
    },
    Draw,
    Priority,
    Transition,
    EnterStep,
    Targets {
        cast: Casting,
    },
    Mana {
        cast: Casting,
    },
    Payment {
        cast: Casting,
    },
    Resolve {
        frame: Resolving,
    },
    Fault {
        reason: String,
    },
    Finished {
        winner: PlayerId,
    },
}
