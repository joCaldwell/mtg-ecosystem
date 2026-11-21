//! The suspended frame: where the rules stopped and the progress needed to
//! continue. A frame names invocations by `StackId` and owns none of them.
use crate::engine::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Casting {
    pub completion: CastCompletion,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sacrifices: Vec<ObjectId>,
    #[serde(default, skip_serializing_if = "AnnouncementOrigin::is_spell")]
    pub origin: AnnouncementOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spending: Option<BTreeMap<Color, Quantity>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spending_restricted: Vec<RestrictedMana>,
    pub stack_id: StackId,
    pub original_object: ObjectId,
    pub prior_passes: Vec<PlayerId>,
    pub mana_activations: Vec<ManaActivationUndo>,
    pub undo_mana: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManaActivationUndo {
    pub object: ObjectId,
    pub player: PlayerId,
    pub color: Color,
    pub amount: Quantity,
    pub mana_was_absent: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(in crate::engine) tap_triggers: Vec<TriggerId>,
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
    GraveyardGroup {
        tx: Box<GraveyardGroup>,
    },
    GraveyardReplacement {
        transaction: Box<GraveyardReplacement>,
    },
    PlayHideawayLinked {
        progress: Box<LinkedPlayProgress>,
    },
    LandEntry {
        origin: EntryOrigin,
        objects: Vec<ObjectId>,
        choices: Vec<land_entry::EntryChoice>,
    },
    /// CR305.6/605.3: select an intrinsic ability before activation or costs.
    ManaSourceChoice {
        player: PlayerId,
        source: ObjectId,
        scope: ManaSourceScope,
        resume: Box<Frame>,
    },
    RevealAndCast {
        progress: Box<RevealAndCastProgress>,
    },
    HandCast {
        frame: Box<Resolving>,
        child: Option<StackId>,
    },
    Cascade {
        progress: Box<CascadeProgress>,
    },
    LegendRule {
        resume: Box<Frame>,
        groups: Vec<Vec<ObjectId>>,
        kept: Vec<ObjectId>,
    },
    AdditionalCosts {
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
    OptionalDamage {
        frame: Resolving,
    },
    ManaAbility {
        activation: Box<ManaActivation>,
    },
    Setup,
    DamageAllocation {
        transaction: DamageTransaction,
        effect: EffectKey,
    },
    Modes {
        cast: Casting,
    },
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
        continuation: Box<CommanderContinuation>,
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
        resume: Box<Frame>,
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
