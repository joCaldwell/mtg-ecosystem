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
    /// Layer 7b.
    SetPowerToughness { power: Amount, toughness: Amount },
    /// Layer 6.
    AddKeywords { keywords: Vec<KeywordAbility> },
    /// Layer 6.
    RemoveKeywords { keywords: Vec<KeywordAbility> },
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
            Self::PowerToughness { .. } => Layer::PowerToughnessModify,
            Self::SetPowerToughness { .. } => Layer::PowerToughnessSet,
            Self::AddKeywords { .. } | Self::RemoveKeywords { .. } => Layer::Ability,
            Self::SetColors { .. } => Layer::Color,
            Self::AddTypes { .. } => Layer::Type,
            Self::Controller { .. } => Layer::Control,
        }
    }
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

/// CR 701.23: what a search looks for and where the found cards go.
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
    /// CR 603.7.
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
