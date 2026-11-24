//! The decision protocol: what a player is asked and may answer.
use crate::engine::*;

/// The intrinsic ability for a basic land type or an index into this source's printed mana abilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "ability", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaAbilityIdentity {
    Intrinsic { color: Color },
    Printed { index: u32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    EntryCopy {
        object: Option<ObjectId>,
    },
    EntryAttachment {
        object: ObjectId,
    },
    OpeningPlacement {
        object: Option<ObjectId>,
    },
    AdditionalCosts {
        selected: Vec<String>,
    },

    LegendRule {
        keep: ObjectId,
    },
    AnnounceX {
        value: Quantity,
    },
    HybridCost {
        colors: Vec<HybridPayment>,
    },
    SacrificeCost {
        object: ObjectId,
    },
    ForcedSacrifice {
        object: ObjectId,
    },
    ChooseManaAbility {
        ability: ManaAbilityIdentity,
    },
    ManaCombination {
        amounts: [Quantity; 5],
    },
    ManaColor {
        color: Color,
    },

    EntryReveal {
        card: Option<ObjectId>,
    },

    OptionalEffect {
        perform: bool,
    },

    Mulligan {
        take: bool,
    },
    BottomOpening {
        objects: Vec<ObjectId>,
    },

    ModeSubset {
        modes: Vec<u32>,
    },
    Modes {
        mode: u32,
    },
    CounterPlacementEffect {
        effect: CounterPlacementEffectKey,
    },
    DamageEffect {
        effect: EffectKey,
        damage: DamageId,
    },
    OrderTriggers {
        order: Vec<TriggerId>,
    },
    EntityTargets {
        entities: Vec<TargetEntity>,
    },
    StackTargets {
        entries: Vec<StackId>,
    },
    ActivateAbility {
        object: ObjectId,
        ability: u32,
    },
    PlayerTargets {
        players: Vec<PlayerId>,
    },
    Sacrifice {
        object: ObjectId,
    },
    ResolveDiscard {
        objects: Vec<ObjectId>,
    },
    SpendMana {
        spending: BTreeMap<Color, Quantity>,
    },
    SpendManaAllocated {
        spending: BTreeMap<Color, Quantity>,
        restricted: Vec<RestrictedMana>,
    },
    CommanderReturn {
        to_command: bool,
    },
    Attackers {
        attacks: Vec<(ObjectId, PlayerId)>,
    },
    Blockers {
        blocks: Vec<(ObjectId, ObjectId)>,
    },
    Damage {
        assignments: Vec<(ObjectId, Quantity)>,
    },
    TrampleDamage {
        assignments: Vec<(ObjectId, Quantity)>,
        player: PlayerId,
        amount: Quantity,
    },
    Discard {
        objects: Vec<ObjectId>,
    },
    PlayLand {
        card: CardId,
    },
    PlayBackLand {
        card: CardId,
    },

    ActivateMana {
        object: ObjectId,
    },
    Bestow {
        card: CardId,
    },
    Cast {
        card: CardId,
    },
    Pass,
    FinishMana {
        undo_mana: bool,
    },
    Targets {
        objects: Vec<ObjectId>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedInput {
    pub decision_id: u64,
    pub player: PlayerId,
    pub response: Response,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GraveyardOrderOption {
    Regenerate,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    EntryCopy {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
        candidates: Vec<ObjectId>,
    },
    EntryAttachment {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
        candidates: Vec<ObjectId>,
    },
    OpeningPlacement {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
    },
    GraveyardReplacementOrder {
        /// OptionalEffect(true) selects this alternative to exile. Omission selects the printed shuffle.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offered: Option<GraveyardOrderOption>,
        decision_id: u64,
        player: PlayerId,
        object: ObjectId,
    },

    AdditionalCosts {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<mtg_contracts::OptionalAdditionalManaCost>,
    },

    LegendRule {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        previous_choices: Vec<ObjectId>,
    },
    AnnounceX {
        decision_id: u64,
        player: PlayerId,
    },
    HybridCost {
        decision_id: u64,
        player: PlayerId,
        symbols: Vec<ColorHybridSymbol>,
    },
    SacrificeCost {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
    },
    ForcedSacrifice {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        previous_choices: Vec<(PlayerId, ObjectId)>,
    },
    ChooseManaAbility {
        decision_id: u64,
        player: PlayerId,
        choices: Vec<ManaAbilityIdentity>,
    },
    ManaCombination {
        decision_id: u64,
        player: PlayerId,
        total: Quantity,
        colors: Vec<Color>,
    },
    ManaColor {
        decision_id: u64,
        player: PlayerId,
        colors: Vec<Color>,
    },

    EntryReveal {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
        candidates: Vec<ObjectId>,
    },

    OptionalEffect {
        decision_id: u64,
        player: PlayerId,
    },

    Mulligan {
        decision_id: u64,
        player: PlayerId,
        mulligans_taken: u8,
        bottom_if_mulligan: u8,
        can_mulligan: bool,
    },
    BottomOpening {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },

    Modes {
        decision_id: u64,
        player: PlayerId,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        one_or_more: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        allow_repeat: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exactly: Option<u32>,
        candidates: Vec<u32>,
    },
    CounterPlacementEffect {
        decision_id: u64,
        player: PlayerId,
        recipient: ObjectId,
        candidates: Vec<CounterPlacementEffectKey>,
    },
    DamageEffect {
        decision_id: u64,
        player: PlayerId,
        recipient: TargetEntity,
        candidates: Vec<DamageEffectCandidate>,
    },
    OrderTriggers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<TriggerId>,
    },
    EntityTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<TargetEntity>,
    },
    StackTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<StackId>,
    },
    PlayerTargets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        candidates: Vec<PlayerId>,
    },
    Sacrifice {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
    },
    ResolveDiscard {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },
    SpendMana {
        decision_id: u64,
        player: PlayerId,
        cost: ManaCost,
        available: BTreeMap<Color, Quantity>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        restricted_available: Vec<RestrictedMana>,
    },
    CommanderReturn {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        previous_choices: Vec<(ObjectId, bool)>,
        decision_id: u64,
        player: PlayerId,
        object: ObjectId,
        destination: Zone,
    },
    Attackers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        defenders: Vec<PlayerId>,
    },
    Blockers {
        decision_id: u64,
        player: PlayerId,
        candidates: Vec<ObjectId>,
        attackers: Vec<ObjectId>,
    },
    Damage {
        decision_id: u64,
        player: PlayerId,
        source: ObjectId,
        power: Quantity,
        recipients: Vec<ObjectId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        player_recipient: Option<PlayerId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lethal_requirements: Vec<(ObjectId, Quantity)>,
    },
    Discard {
        decision_id: u64,
        player: PlayerId,
        count: usize,
        candidates: Vec<ObjectId>,
    },
    Mana {
        decision_id: u64,
        player: PlayerId,
        cost: ManaCost,
    },
    Priority {
        decision_id: u64,
        player: PlayerId,
    },
    Targets {
        decision_id: u64,
        player: PlayerId,
        occurrences: Vec<TargetSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        counter_division: Option<CounterDivision>,
        candidates: Vec<ObjectId>,
    },
}
impl Request {
    pub fn decision_id(&self) -> u64 {
        match self {
            Self::AdditionalCosts { decision_id, .. }
            | Self::HybridCost { decision_id, .. }
            | Self::AnnounceX { decision_id, .. }
            | Self::ChooseManaAbility { decision_id, .. }
            | Self::ManaCombination { decision_id, .. }
            | Self::ManaColor { decision_id, .. }
            | Self::EntryCopy { decision_id, .. }
            | Self::EntryAttachment { decision_id, .. }
            | Self::EntryReveal { decision_id, .. }
            | Self::OptionalEffect { decision_id, .. }
            | Self::Mulligan { decision_id, .. }
            | Self::OpeningPlacement { decision_id, .. }
            | Self::BottomOpening { decision_id, .. }
            | Self::Modes { decision_id, .. }
            | Self::CounterPlacementEffect { decision_id, .. }
            | Self::DamageEffect { decision_id, .. }
            | Self::OrderTriggers { decision_id, .. }
            | Self::EntityTargets { decision_id, .. }
            | Self::StackTargets { decision_id, .. }
            | Self::PlayerTargets { decision_id, .. }
            | Self::Sacrifice { decision_id, .. }
            | Self::SacrificeCost { decision_id, .. }
            | Self::ForcedSacrifice { decision_id, .. }
            | Self::LegendRule { decision_id, .. }
            | Self::ResolveDiscard { decision_id, .. }
            | Self::SpendMana { decision_id, .. }
            | Self::GraveyardReplacementOrder { decision_id, .. }
            | Self::CommanderReturn { decision_id, .. }
            | Self::Attackers { decision_id, .. }
            | Self::Blockers { decision_id, .. }
            | Self::Damage { decision_id, .. }
            | Self::Discard { decision_id, .. }
            | Self::Priority { decision_id, .. }
            | Self::Targets { decision_id, .. }
            | Self::Mana { decision_id, .. } => *decision_id,
        }
    }
    pub fn player(&self) -> PlayerId {
        match self {
            Self::AdditionalCosts { player, .. }
            | Self::HybridCost { player, .. }
            | Self::AnnounceX { player, .. }
            | Self::ChooseManaAbility { player, .. }
            | Self::ManaCombination { player, .. }
            | Self::ManaColor { player, .. }
            | Self::EntryCopy { player, .. }
            | Self::EntryAttachment { player, .. }
            | Self::EntryReveal { player, .. }
            | Self::OptionalEffect { player, .. }
            | Self::Mulligan { player, .. }
            | Self::OpeningPlacement { player, .. }
            | Self::BottomOpening { player, .. }
            | Self::Modes { player, .. }
            | Self::CounterPlacementEffect { player, .. }
            | Self::DamageEffect { player, .. }
            | Self::OrderTriggers { player, .. }
            | Self::EntityTargets { player, .. }
            | Self::StackTargets { player, .. }
            | Self::PlayerTargets { player, .. }
            | Self::Sacrifice { player, .. }
            | Self::SacrificeCost { player, .. }
            | Self::ForcedSacrifice { player, .. }
            | Self::LegendRule { player, .. }
            | Self::ResolveDiscard { player, .. }
            | Self::SpendMana { player, .. }
            | Self::GraveyardReplacementOrder { player, .. }
            | Self::CommanderReturn { player, .. }
            | Self::Attackers { player, .. }
            | Self::Blockers { player, .. }
            | Self::Damage { player, .. }
            | Self::Discard { player, .. }
            | Self::Priority { player, .. }
            | Self::Targets { player, .. }
            | Self::Mana { player, .. } => *player,
        }
    }
}
