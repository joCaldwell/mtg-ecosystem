//! Verbs and their operands: one game action each.
use super::*;

// ---------------------------------------------------------------------------
// Verb operands
// ---------------------------------------------------------------------------

/// Who or what damage is dealt to (CR 120.1-120.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "to", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageTo {
    /// One object, player or any-target occurrence.
    Ref {
        who: Ref,
    },
    Players {
        players: PlayerExpr,
    },
    Objects {
        set: SetExpr,
    },
    /// CR 120.2: every listed recipient is dealt the damage simultaneously.
    All {
        parts: Vec<DamageTo>,
    },
}

/// CR 611.2c: a rules restriction can name one exact object or apply to a
/// live class of creatures. It does not grant or remove an ability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum CombatProhibitionScope<T> {
    Target { target: T },
    Creatures {},
    CreaturesWithoutFlying {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "to", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destination {
    #[serde(deserialize_with = "no_operands")]
    Exile,
    /// CR 400.3: an object goes to its owner's hand, library or graveyard.
    #[serde(deserialize_with = "no_operands")]
    OwnerHand,
    #[serde(deserialize_with = "no_operands")]
    OwnerLibraryTop,
    #[serde(deserialize_with = "no_operands")]
    OwnerLibraryBottom,
    #[serde(deserialize_with = "no_operands")]
    OwnerGraveyard,
    Battlefield {
        controller: PlayerExpr,
        #[serde(default, skip_serializing_if = "is_false")]
        tapped: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "counter", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterKind {
    /// CR 122.1a.
    #[serde(deserialize_with = "no_operands")]
    PlusOnePlusOne,
    #[serde(deserialize_with = "no_operands")]
    MinusOneMinusOne,
    Named {
        name: String,
    },
}

/// Player counters for poison and energy (CR 107.14, 122.1f, 704.5c).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayerCounterKind {
    Poison,
    Energy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "token", rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenSpec {
    /// CR 111.3-111.4: a creature token with exactly these characteristics.
    Creature { spec: CreatureToken },
    /// CR 111.10: a predefined token.
    Predefined { kind: PredefinedToken },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mana", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaSpec {
    /// `amount` mana of one color (CR 106.1).
    Color { color: Color, amount: Amount },
    /// `amount` mana in any combination of the listed colors.
    AnyCombination { colors: Vec<Color>, amount: Amount },
}

/// CR 613.1: the layer a continuous change applies in.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// 613.1a copy effects.
    Copy,
    /// 613.1b control-changing effects.
    Control,
    /// 613.1c text-changing effects.
    Text,
    /// 613.1d type-changing effects.
    Type,
    /// 613.1e color-changing effects.
    Color,
    /// 613.1f ability-adding and ability-removing effects.
    Ability,
    /// 613.4b power/toughness-setting effects.
    PowerToughnessSet,
    /// 613.4c power/toughness modifications.
    PowerToughnessModify,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// Layer 7c: a signed modification.
    PowerToughness { power: Amount, toughness: Amount },
    /// Layer 7c: a signed modification bound to the invocation's announced X.
    PowerToughnessX {
        power: PumpDelta,
        toughness: PumpDelta,
    },
    /// Layer 7b.
    SetPowerToughness { power: Amount, toughness: Amount },
    /// Layer 6.
    AddKeywords { keywords: Vec<KeywordAbility> },
    /// Layer 6.
    RemoveKeywords { keywords: Vec<KeywordAbility> },
    /// Layer 6 (CR 113.10, 113.10a, 613.1f): quoted mana abilities the
    /// recipients have. Each belongs to the object that has it: that object
    /// pays its costs and "you" is whoever activates it (CR 109.5).
    AddManaAbilities { abilities: Vec<crate::ManaAbility> },
    /// Layer 5.
    SetColors { colors: Vec<Color> },
    /// Layer 4.
    AddTypes { kinds: Vec<CardKind> },
    /// Layer 2.
    Controller { player: PlayerExpr },
}

impl Change {
    /// The one layer this change belongs to (CR 613.1, 613.4).
    pub fn layer(&self) -> Layer {
        match self {
            Self::PowerToughness { .. } | Self::PowerToughnessX { .. } => {
                Layer::PowerToughnessModify
            }
            Self::SetPowerToughness { .. } => Layer::PowerToughnessSet,
            Self::AddKeywords { .. }
            | Self::RemoveKeywords { .. }
            | Self::AddManaAbilities { .. } => Layer::Ability,
            Self::SetColors { .. } => Layer::Color,
            Self::AddTypes { .. } => Layer::Type,
            Self::Controller { .. } => Layer::Control,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "delta", rename_all = "snake_case", deny_unknown_fields)]
pub enum PumpDelta {
    Fixed {
        value: Quantity,
    },
    X {
        negative: bool,
    },
    /// CR 608.2h, 611.2c: a resolution count read once as the effect
    /// begins; the change keeps that value for its duration.
    Counted {
        amount: Amount,
        negative: bool,
    },
}

impl PumpDelta {
    /// The signed change once the change's one X is known: the announced X,
    /// or the value its counted deltas read.
    pub fn with_x(&self, x: &Quantity) -> Quantity {
        match self {
            Self::Fixed { value } => value.clone(),
            Self::X { negative: false }
            | Self::Counted {
                negative: false, ..
            } => x.clone(),
            Self::X { negative: true } | Self::Counted { negative: true, .. } => {
                let mut result = Quantity::zero();
                result.subtract(x);
                result
            }
        }
    }
    /// The resolution count this delta reads, if any.
    pub fn counted(&self) -> Option<&Amount> {
        match self {
            Self::Counted { amount, .. } => Some(amount),
            _ => None,
        }
    }
}

impl Change {
    /// CR 608.2h: the one resolution count a P/T change reads. Both
    /// counted deltas of one change read the same number.
    pub fn counted_x(&self) -> Option<&Amount> {
        match self {
            Self::PowerToughnessX { power, toughness } => power.counted().or(toughness.counted()),
            _ => None,
        }
    }
}

/// Fixed values installed by one resolving effect (CR 611.2c, 613.6).
/// An omitted characteristic retains its value from the other effects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CharacteristicSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<Color>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<KeywordAbility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_power: Option<Quantity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_toughness: Option<Quantity>,
}

/// CR 205.1b: this permanent becomes a creature while retaining its
/// card types and supertypes. Only its creature subtypes can be replaced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub add_artifact: bool,
    pub replace_creature_subtypes: bool,
    pub creature_subtypes: Vec<String>,
}

/// How long a continuous effect lasts (CR 611.2a-b).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Until {
    /// CR 514.2: until the cleanup step of the turn it was created in.
    EndOfTurn,
    EndOfCombat,
    /// Until the controller's next turn begins.
    ControllerNextTurn,
    /// For as long as the source remains on the battlefield.
    SourceLeaves,
    /// No stated duration (CR 611.2a).
    Indefinite,
}

/// CR 701.23: what a search looks for and where the found cards go. An
/// empty filter is a search "simply for a quantity of cards": the searcher
/// must find `maximum`, or as many as the library holds (CR 701.23d). With a
/// stated quality they may find any number up to `maximum` (CR 701.23b). The
/// found cards move together to `to`; `reveal` shows them first (CR 701.23e).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchSpec {
    pub filter: Pred,
    pub maximum: Amount,
    pub to: Destination,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reveal: bool,
}

/// CR 603.7: when a delayed triggered ability triggers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "at", rename_all = "snake_case", deny_unknown_fields)]
pub enum DelayedEvent {
    #[serde(deserialize_with = "no_operands")]
    NextEndStep,
    #[serde(deserialize_with = "no_operands")]
    NextTurnUpkeep,
    /// CR 400.7j: exile this one announced creature and remember its successor.
    NextEndStepAfterExile {
        slot: u32,
    },
    /// CR 400.7e/j, 603.10a: exile this source's death successor and watch one permanent.
    TargetLeavesAfterExilingSource {
        slot: u32,
    },
    NextUpkeep {
        of: PlayerExpr,
    },
    #[serde(deserialize_with = "no_operands")]
    EndOfCombat,
    /// An ordinary event pattern, watched until `until` (CR 603.7c).
    Event {
        event: Box<TriggerCondition>,
        until: Until,
    },
}

/// What a play permission allows (CR 601.3, 305.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Play,
    Cast,
    CastWithoutPayingManaCost,
}

/// A designation a player or object carries (CR 700.6, 725, 726).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Designation {
    Monarch,
    Initiative,
    CityBlessing,
    Monstrous,
    Renowned,
}

/// A whole reviewed procedure the algebra does not decompose. Each id names
/// one Comprehensive Rules keyword action, never a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinId {
    /// CR 701.34.
    Proliferate,
    /// CR 702.85.
    Cascade,
    /// CR 702.144.
    Demonstrate,
    /// CR 702.75.
    Hideaway,
}

/// One game action. A verb acts on every member of its operands as one
/// simultaneous event unless its rule says otherwise (CR 608.2c, 101.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verb", rename_all = "snake_case", deny_unknown_fields)]
pub enum Verb {
    /// CR 121.1-121.2.
    Draw {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 119.3.
    GainLife {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 119.3.
    LoseLife {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 120: `source` deals `amount` damage to each recipient.
    Fight {
        first: Ref,
        second: Ref,
    },
    Populate {
        player: PlayerExpr,
    },
    Proliferate {
        player: PlayerExpr,
    },
    Damage {
        source: Ref,
        to: DamageTo,
        amount: Amount,
    },
    /// CR 701.8.
    Destroy {
        what: SetExpr,
        /// CR 701.19c.
        #[serde(default, skip_serializing_if = "is_false")]
        regeneration_forbidden: bool,
    },
    /// CR 400.3, 400.7: a zone change.
    Move {
        what: SetExpr,
        to: Destination,
    },
    /// CR 701.26a.
    Tap {
        what: SetExpr,
    },
    /// CR 701.26b.
    Untap {
        what: SetExpr,
    },
    /// CR 122.
    GiveCounters {
        player: PlayerExpr,
        counter: PlayerCounterKind,
        amount: Amount,
    },
    PutCounters {
        on: SetExpr,
        counter: CounterKind,
        amount: Amount,
    },
    /// CR 111.
    CreateTokens {
        controller: PlayerExpr,
        count: Amount,
        token: TokenSpec,
    },
    /// CR 701.17.
    Mill {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 701.9.
    Discard {
        player: PlayerExpr,
        amount: Amount,
        #[serde(default, skip_serializing_if = "is_false")]
        random: bool,
    },
    /// CR 701.21: each denoted player chooses and sacrifices.
    Sacrifice {
        player: PlayerExpr,
        count: Amount,
        filter: Pred,
    },
    /// CR 106.4.
    AddMana {
        player: PlayerExpr,
        mana: ManaSpec,
    },
    /// CR 701.23.
    Search {
        player: PlayerExpr,
        search: SearchSpec,
    },
    /// CR 701.6.
    Counter {
        what: Ref,
    },
    /// CR 613.1b.
    GainControl {
        what: SetExpr,
        controller: PlayerExpr,
        until: Until,
    },
    /// CR 611.2c, 613.1d-f, 613.4b and 613.7b.
    SetCharacteristics {
        what: SetExpr,
        values: CharacteristicSet,
        until: Until,
    },
    /// CR 701.3.
    Attach {
        what: Ref,
        to: Ref,
    },
    /// CR 611.2c: one continuous effect over the set captured as the verb is
    /// performed. `layer` states where `change` applies (CR 613).
    ModifyUntil {
        what: SetExpr,
        layer: Layer,
        change: Change,
        until: Until,
    },
    /// CR 508.1c, 509.1b: a combat declaration restriction.
    RestrictCombat {
        affects: CombatProhibitionScope<Ref>,
        restriction: CombatRequirement,
        until: Until,
    },
    /// CR 603.7.
    ReturnCapturedExile {},
    CreateDelayedTrigger {
        event: DelayedEvent,
        node: Box<Node>,
    },
    /// CR 707.10.
    CopyStackObject {
        what: Ref,
    },
    /// CR 601.3: `player` may play or cast the cards of `what` until `until`.
    PlayPermission {
        player: PlayerExpr,
        what: SetExpr,
        permission: Permission,
        until: Until,
    },
    /// CR 701.27.
    Transform {
        what: Ref,
    },
    SetDesignation {
        player: PlayerExpr,
        designation: Designation,
    },
    /// CR 701.22.
    ReorderLibraryTop {
        player: PlayerExpr,
        amount: Amount,
    },
    Scry {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 701.25.
    Surveil {
        player: PlayerExpr,
        amount: Amount,
    },
    /// CR 701.24.
    Shuffle {
        player: PlayerExpr,
    },
    /// CR 701.19.
    Regenerate {
        what: SetExpr,
    },
    Builtin {
        id: BuiltinId,
    },
}
