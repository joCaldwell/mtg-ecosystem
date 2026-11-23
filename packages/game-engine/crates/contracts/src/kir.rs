//! Kernel IR v1: one orthogonal algebra for what a resolving spell or ability
//! does.
//!
//! A legacy `Instruction` variant fuses a verb with one recipient shape and one
//! amount shape, so every new combination needs a new variant, a validation
//! arm, a schema branch and a core dispatch arm. The kernel separates them:
//!
//! * a [`Ref`] names an object, player or number the invocation can see;
//! * a [`PlayerExpr`], [`SetExpr`] or [`Amount`] builds players, object sets
//!   and quantities from references and [`Pred`] filters;
//! * a [`Verb`] is one game action over those operands;
//! * a [`Node`] sequences, guards and repeats verbs.
//!
//! [`check`] is the only admission decision for a node. It proves every
//! reference is in scope and of the right kind (CR 608.2b-c, 608.2h), refuses
//! to count a filtered hidden zone (CR 400.2, 401.2, 402.3), and admits a verb
//! only in an audited `(verb, operand kind, context)` cell: a cell the core
//! interpreter executes and whose recovery re-derives the same facts. A
//! well-scoped node outside the audited cells is rejected with
//! [`KirError::Unaudited`], never interpreted approximately.
//!
//! The walk derives the capabilities of each leaf and the continuation
//! surface it uses. The node's wire shape is derived from its Rust type.
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::{
    Capability, CardKind, Color, CreatureToken, KeywordAbility, PredefinedToken, Quantity,
    TargetDomain, TargetSpec, TriggerCondition,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

fn is_false(value: &bool) -> bool {
    !*value
}

/// An internally tagged unit variant carries no operands. Serde's default
/// unit visitor ignores extra fields; the IR rejects them.
fn no_operands<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct NoOperands {}
    NoOperands::deserialize(deserializer).map(|_| ())
}

// ---------------------------------------------------------------------------
// References
// ---------------------------------------------------------------------------

/// Whose "you" a controller reference means (CR 109.5). The role is fixed by
/// where the text is printed, so a reference states it instead of leaving the
/// reader to infer it from the surrounding ability.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ControllerRole {
    /// CR 112.2: the player who cast the resolving spell.
    SpellController,
    /// CR 113.8: the controller of the resolving activated or triggered ability.
    AbilityController,
    /// CR 113.6: the controller of the object a static ability is printed on.
    StaticController,
    /// CR 614.12: the player under whose control a permanent would enter.
    EnteringController,
    /// CR 601.2: the player who would cast the card a permission applies to.
    ProspectiveCaster,
    /// CR 116: the player taking a special action.
    SpecialActionPlayer,
    /// CR 108.3: the owner of the card.
    CardOwner,
    /// The player who holds an ability outside the battlefield and stack.
    AbilityHolder,
}

/// The object an event reference names (CR 603.2, 603.10).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EventObjectRole {
    Entered,
    Died,
    DealtDamage,
    Damaged,
    CastSpell,
    TappedForMana,
    Attacked,
    Blocked,
    Sacrificed,
    LeftBattlefield,
}

/// The player an event reference names.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EventPlayerRole {
    /// CR 601.2: the player who cast the triggering spell.
    Caster,
    /// CR 602.2, 605.1: the player who activated the triggering ability.
    ActivatingPlayer,
    DamagedPlayer,
    ControllerOfEventObject,
    /// CR 102.1, 500: the player whose beginning step triggered the ability.
    ActivePlayer,
}

/// What an earlier step of the same node produced.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StepRole {
    /// The objects the step actually affected (CR 608.2c "this way").
    Objects,
    /// The player who acted in the step.
    Player,
    /// The number the step actually produced (cards drawn, damage dealt).
    Amount,
}

/// The closed reference vocabulary. Every operand that is not a literal is
/// built from these, and the reference binder emits exactly these.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "ref", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ref {
    /// CR 201.5, 400.7, 113.7a: the exact incarnation the ability is printed
    /// on, or its last known information once it has left.
    #[serde(deserialize_with = "no_operands")]
    Source,
    Controller {
        role: ControllerRole,
    },
    /// CR 601.2c, 115.1: the announced target occurrence `slot`.
    Target {
        slot: u32,
    },
    /// CR 301.5, 303.4: the permanent the source is attached to.
    #[serde(deserialize_with = "no_operands")]
    AttachedTo,
    EventObject {
        role: EventObjectRole,
    },
    EventPlayer {
        role: EventPlayerRole,
    },
    /// CR 614.12: the permanent a replacement effect is modifying as it
    /// enters. Never the source: a copy effect's source enters as something
    /// else, and the entering object is what the effect reads.
    #[serde(deserialize_with = "no_operands")]
    EnteringObject,
    StepResult {
        step: u32,
        role: StepRole,
    },
    /// A variable bound by an enclosing [`Node::ForEach`] or [`Node::Choose`].
    BoundVar {
        id: u32,
    },
    /// CR 607: the cards a linked ability of the same source put aside.
    LinkedPool {
        id: u32,
    },
    /// CR 107.3, 601.2b: the value announced for X.
    #[serde(deserialize_with = "no_operands")]
    AnnouncedX,
}

// ---------------------------------------------------------------------------
// Players, predicates, sets and amounts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "player", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerExpr {
    /// The player a reference denotes.
    Is { who: Ref },
    /// CR 109.4-109.5: the current or last known controller of an object.
    ControllerOf { object: Ref },
    /// CR 108.3: the owner of an object.
    OwnerOf { object: Ref },
    /// CR 102.2-102.3: each opponent of the invocation's controller, in APNAP
    /// order (CR 101.4).
    #[serde(deserialize_with = "no_operands")]
    EachOpponent,
    /// Each player, in APNAP order.
    #[serde(deserialize_with = "no_operands")]
    EachPlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cmp {
    AtLeast,
    AtMost,
    Exactly,
}

/// One conjunct of an object predicate. Card types, supertypes and subtypes
/// are the object's current ones (CR 205, 613.1d); colors are current colors
/// (CR 105, 613.1e); power, toughness and keywords are fully derived
/// (CR 613.4, 613.1f).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "atom", rename_all = "snake_case", deny_unknown_fields)]
pub enum Atom {
    Type {
        kind: CardKind,
    },
    NotType {
        kind: CardKind,
    },
    /// Has at least one of the listed card types.
    AnyType {
        kinds: Vec<CardKind>,
    },
    Subtype {
        subtype: String,
    },
    NotSubtype {
        subtype: String,
    },
    #[serde(deserialize_with = "no_operands")]
    Legendary,
    #[serde(deserialize_with = "no_operands")]
    NotLegendary,
    #[serde(deserialize_with = "no_operands")]
    Basic,
    #[serde(deserialize_with = "no_operands")]
    NotBasic,
    Color {
        color: Color,
    },
    NotColor {
        color: Color,
    },
    /// CR 109.5: controlled by one of the denoted players.
    ControlledBy {
        player: PlayerExpr,
    },
    OwnedBy {
        player: PlayerExpr,
    },
    #[serde(deserialize_with = "no_operands")]
    Tapped,
    #[serde(deserialize_with = "no_operands")]
    Untapped,
    /// CR 506.4: an attacking creature.
    #[serde(deserialize_with = "no_operands")]
    Attacking,
    #[serde(deserialize_with = "no_operands")]
    Blocking,
    #[serde(deserialize_with = "no_operands")]
    Token,
    #[serde(deserialize_with = "no_operands")]
    NotToken,
    Keyword {
        keyword: KeywordAbility,
    },
    NotKeyword {
        keyword: KeywordAbility,
    },
    Power {
        cmp: Cmp,
        value: Amount,
    },
    Toughness {
        cmp: Cmp,
        value: Amount,
    },
    ManaValue {
        cmp: Cmp,
        value: Amount,
    },
    /// "Other", "another": not the referenced object.
    IsNot {
        object: Ref,
    },
    /// Satisfies at least one alternative.
    AnyOf {
        alternatives: Vec<Pred>,
    },
}

/// A conjunction of atoms. The empty predicate admits every object of the
/// zone it is evaluated in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Pred(pub Vec<Atom>);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "zone", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneExpr {
    /// CR 110.1, 702.26b: phased-in permanents.
    #[serde(deserialize_with = "no_operands")]
    Battlefield,
    /// Every graveyard (CR 404).
    #[serde(deserialize_with = "no_operands")]
    Graveyards,
    Graveyard {
        of: PlayerExpr,
    },
    Hand {
        of: PlayerExpr,
    },
    Library {
        of: PlayerExpr,
    },
    #[serde(deserialize_with = "no_operands")]
    Exile,
    #[serde(deserialize_with = "no_operands")]
    Stack,
}

impl ZoneExpr {
    /// CR 400.2: hands and libraries are hidden zones.
    pub fn hidden(&self) -> bool {
        matches!(self, Self::Hand { .. } | Self::Library { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "set", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetExpr {
    /// The single object a reference denotes.
    One { object: Ref },
    /// CR 608.2h: every object of `zone` that satisfies `filter` at the
    /// moment the verb is performed.
    Matching { zone: ZoneExpr, filter: Pred },
    /// The cards of a linked pool.
    Pool { pool: Ref },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Power,
    Toughness,
    ManaValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "amount", rename_all = "snake_case", deny_unknown_fields)]
pub enum Amount {
    Fixed {
        value: Quantity,
    },
    /// A number a reference denotes: announced X, a step result, a bound
    /// variable.
    Var {
        of: Ref,
    },
    /// CR 608.2h: the number of objects in a set as the verb is performed.
    Count {
        set: SetExpr,
    },
    /// A characteristic of one object, using last known information if it
    /// has left (CR 608.2h).
    StatOf {
        object: Ref,
        stat: Stat,
    },
    /// The life total of one player.
    LifeOf {
        player: PlayerExpr,
    },
    /// CR 202.3, 601.2i: the total mana value of the spells `caster` cast
    /// this turn other than the resolving spell.
    OtherCastsManaValue {
        caster: PlayerExpr,
    },
}

impl Amount {
    pub fn fixed(value: impl Into<Quantity>) -> Self {
        Self::Fixed {
            value: value.into(),
        }
    }
    pub fn as_fixed(&self) -> Option<&Quantity> {
        match self {
            Self::Fixed { value } => Some(value),
            _ => None,
        }
    }
    /// CR 107.3: the value announced for X.
    pub fn announced_x() -> Self {
        Self::Var {
            of: Ref::AnnouncedX,
        }
    }
    pub fn is_announced_x(&self) -> bool {
        matches!(
            self,
            Self::Var {
                of: Ref::AnnouncedX
            }
        )
    }
}

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

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cond", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cond {
    Compare {
        left: Amount,
        cmp: Cmp,
        right: Amount,
    },
    Exists {
        set: SetExpr,
    },
    /// CR 601.2b, 702.33d: an optional additional cost was paid.
    CostPaid {
        identity: String,
    },
    Not {
        condition: Box<Cond>,
    },
    All {
        conditions: Vec<Cond>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "over", rename_all = "snake_case", deny_unknown_fields)]
pub enum Domain {
    Players { players: PlayerExpr },
    Objects { set: SetExpr },
}

/// CR 118: what a player may pay during resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cost", rename_all = "snake_case", deny_unknown_fields)]
pub enum Payment {
    Generic { amount: Amount },
    Life { amount: Amount },
}

/// Control over verbs. A node is performed in written order (CR 608.2c).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Do(Verb),
    Seq(Vec<Node>),
    If {
        condition: Cond,
        then: Box<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        otherwise: Option<Box<Node>>,
    },
    /// CR 608.2d: `chooser` decides during resolution whether to perform it.
    Optional {
        chooser: PlayerExpr,
        node: Box<Node>,
    },
    /// Performs `body` once per member, binding `BoundVar { id: var }`.
    ForEach {
        var: u32,
        over: Domain,
        body: Box<Node>,
    },
    /// `chooser` picks between `minimum` and `maximum` members of `from`,
    /// bound as `BoundVar { id: var }` while `body` is performed.
    Choose {
        var: u32,
        chooser: PlayerExpr,
        from: SetExpr,
        minimum: Amount,
        maximum: Amount,
        body: Box<Node>,
    },
    /// CR 118.12a: perform `node` unless `payer` pays.
    UnlessPays {
        payer: PlayerExpr,
        payment: Payment,
        node: Box<Node>,
    },
    /// CR 603.7: perform `node` when the delayed event next occurs.
    Delayed {
        event: DelayedEvent,
        node: Box<Node>,
    },
    /// CR 603.12: "when you do": `then` is a reflexive trigger of `first`.
    Reflexive {
        first: Box<Node>,
        then: Box<Node>,
    },
}

impl Node {
    /// The drain pair of [`drain`] for "you" in `context`.
    pub fn drain(context: Context, amount: Amount) -> Self {
        Self::Seq(vec![
            Self::Do(Verb::LoseLife {
                player: PlayerExpr::EachOpponent,
                amount,
            }),
            Self::Do(Verb::GainLife {
                player: PlayerExpr::you(context),
                amount: Amount::Var {
                    of: Ref::StepResult {
                        step: 0,
                        role: StepRole::Amount,
                    },
                },
            }),
        ])
    }
    /// The verb of a single-action node.
    pub fn verb(&self) -> Option<&Verb> {
        match self {
            Self::Do(verb) => Some(verb),
            _ => None,
        }
    }

    pub(crate) fn optional_target_destroy(&self) -> Option<(&Verb, u32, bool)> {
        let Self::Optional { chooser, node } = self else {
            return None;
        };
        match node.verb()? {
            verb @ Verb::Destroy {
                what:
                    SetExpr::One {
                        object: Ref::Target { slot },
                    },
                regeneration_forbidden,
            } if chooser.is_you() => Some((verb, *slot, *regeneration_forbidden)),
            _ => None,
        }
    }

    pub(crate) fn uses_continuation(&self) -> bool {
        match self {
            Self::Seq(_) => drain(self).is_none(),
            Self::Optional { .. } => self.optional_target_destroy().is_none(),
            Self::If { .. }
            | Self::ForEach { .. }
            | Self::Choose { .. }
            | Self::UnlessPays { .. } => true,
            Self::Do(_) | Self::Delayed { .. } | Self::Reflexive { .. } => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// The kind of stack object a node resolves in (CR 113.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Context {
    Spell,
    Activated,
    Triggered,
}

impl Context {
    /// CR 109.5: whose "you" a reference in this context means.
    pub fn controller_role(self) -> ControllerRole {
        match self {
            Self::Spell => ControllerRole::SpellController,
            Self::Activated | Self::Triggered => ControllerRole::AbilityController,
        }
    }
}

impl Ref {
    pub fn controller(context: Context) -> Self {
        Self::Controller {
            role: context.controller_role(),
        }
    }
}

impl PlayerExpr {
    /// "You" in `context` (CR 109.5).
    pub fn you(context: Context) -> Self {
        Self::Is {
            who: Ref::controller(context),
        }
    }
    pub fn target(slot: u32) -> Self {
        Self::Is {
            who: Ref::Target { slot },
        }
    }
    pub fn event(role: EventPlayerRole) -> Self {
        Self::Is {
            who: Ref::EventPlayer { role },
        }
    }
    /// True for "you" in any role.
    pub fn is_you(&self) -> bool {
        matches!(
            self,
            Self::Is {
                who: Ref::Controller { .. }
            }
        )
    }
    pub fn target_slot(&self) -> Option<u32> {
        match self {
            Self::Is {
                who: Ref::Target { slot },
            } => Some(*slot),
            _ => None,
        }
    }
    pub fn is_target(&self) -> bool {
        self.target_slot().is_some()
    }
    pub fn is_event_player(&self) -> bool {
        matches!(
            self,
            Self::Is {
                who: Ref::EventPlayer { .. }
            }
        )
    }
}

impl Pred {
    /// "Lands you control" (CR 205.2a, 109.5).
    pub fn lands_you_control(context: Context) -> Self {
        Self(vec![
            Atom::Type {
                kind: CardKind::Land,
            },
            Atom::ControlledBy {
                player: PlayerExpr::you(context),
            },
        ])
    }
    /// "Creatures" (CR 205.2a).
    pub fn creatures() -> Self {
        Self(vec![Atom::Type {
            kind: CardKind::Creature,
        }])
    }
    /// "Nonland permanent" (CR 110.1, 205.2a).
    pub fn nonland_permanents() -> Self {
        Self(vec![Atom::NotType {
            kind: CardKind::Land,
        }])
    }
    /// True when some conjunct names the invocation's controller.
    pub fn names_you(&self) -> bool {
        self.0.iter().any(|atom| match atom {
            Atom::ControlledBy { player } | Atom::OwnedBy { player } => player.is_you(),
            Atom::AnyOf { alternatives } => alternatives.iter().any(Self::names_you),
            _ => false,
        })
    }
}

impl SetExpr {
    /// CR 701.26b: the lands "you" in `context` control, on the battlefield.
    pub fn lands_you_control(context: Context) -> Self {
        Self::battlefield(Pred::lands_you_control(context))
    }
    /// Every card in one player's graveyard (CR 404.1).
    pub fn graveyard_of(player: PlayerExpr) -> Self {
        Self::Matching {
            zone: ZoneExpr::Graveyard { of: player },
            filter: Pred::default(),
        }
    }
    /// Every card in every graveyard (CR 404).
    pub fn graveyards() -> Self {
        Self::Matching {
            zone: ZoneExpr::Graveyards,
            filter: Pred::default(),
        }
    }
    /// The filter of a board-wide or graveyard-wide set.
    pub fn filter(&self) -> Option<&Pred> {
        match self {
            Self::Matching { filter, .. } => Some(filter),
            _ => None,
        }
    }
    pub fn target(slot: u32) -> Self {
        Self::One {
            object: Ref::Target { slot },
        }
    }
    pub fn source() -> Self {
        Self::One {
            object: Ref::Source,
        }
    }
    pub fn battlefield(filter: Pred) -> Self {
        Self::Matching {
            zone: ZoneExpr::Battlefield,
            filter,
        }
    }
    pub fn target_slot(&self) -> Option<u32> {
        match self {
            Self::One {
                object: Ref::Target { slot },
            } => Some(*slot),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------------

/// Everything outside a node that its references may read.
#[derive(Clone, Copy, Debug)]
pub struct Env<'a> {
    pub context: Context,
    /// The program's announced target occurrences (CR 601.2c).
    pub targets: &'a [TargetSpec],
    /// The event of a triggered ability (CR 603.2).
    pub event: Option<&'a TriggerCondition>,
    /// CR 107.3: the object has {X} in its cost.
    pub announced_x: bool,
}

/// What a reference denotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Object,
    Player,
    /// CR 115.4: "any target" - a creature, player or planeswalker.
    Entity,
    StackObject,
    Number,
    Pool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KirError {
    /// A reference names something this context, event or scope lacks.
    Scope(String),
    /// A reference or literal is of the wrong kind for its operand.
    Type(String),
    /// CR 400.2: counting matching cards of a hidden zone would reveal them.
    HiddenCount,
    /// Well formed, but no audited `(verb, operand kind, context)` cell
    /// executes it.
    Unaudited(String),
}

impl fmt::Display for KirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(detail) => write!(f, "kernel reference out of scope: {detail}"),
            Self::Type(detail) => write!(f, "kernel operand type: {detail}"),
            Self::HiddenCount => f.write_str("kernel count over a filtered hidden zone"),
            Self::Unaudited(detail) => write!(f, "kernel cell is not audited: {detail}"),
        }
    }
}

impl From<KirError> for String {
    fn from(error: KirError) -> Self {
        error.to_string()
    }
}

/// What a checked node needs from its surroundings, derived by the walk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedRequirements {
    pub capabilities: BTreeSet<Capability>,
    /// Target occurrences the node reads (CR 601.2c).
    pub target_slots: BTreeSet<u32>,
    pub event_player: bool,
    pub announced_x: bool,
}

/// The single scope-and-type checker for kernel nodes.
pub fn check(node: &Node, env: &Env<'_>) -> Result<DerivedRequirements, KirError> {
    let mut walk = Walk {
        env,
        scan: false,
        plain_leaf: false,
        bound: vec![],
        steps: 0,
        out: DerivedRequirements::default(),
    };
    walk.node(node, false)?;
    if node.uses_continuation() {
        walk.out.capabilities.insert(Capability::KernelControlV1);
    }
    Ok(walk.out)
}

/// The target occurrences a node names, without judging the node. Program
/// validation uses this to prove every announced target is read (CR 601.2c);
/// [`check`] remains the admission decision.
pub fn target_slots(node: &Node) -> BTreeSet<u32> {
    let env = Env {
        context: Context::Spell,
        targets: &[],
        event: None,
        announced_x: false,
    };
    let mut walk = Walk {
        env: &env,
        scan: true,
        plain_leaf: false,
        bound: vec![],
        steps: 0,
        out: DerivedRequirements::default(),
    };
    // A scan reports no scope or audit error; a malformed literal can only
    // end it early, and `check` reports that literal.
    let _ = walk.node(node, false);
    walk.out.target_slots
}

struct Walk<'a, 'e> {
    env: &'a Env<'e>,
    /// Collect references only: no scope, kind or audit judgement.
    scan: bool,
    /// An ordinary leaf executor reads the invocation, without task bindings.
    plain_leaf: bool,
    /// Variables bound by enclosing `ForEach`/`Choose` nodes, with their kind.
    bound: Vec<(u32, RefKind)>,
    /// Completed steps of the innermost enclosing `Seq`.
    steps: u32,
    out: DerivedRequirements,
}

fn scope<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Scope(detail.into()))
}
fn kind<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Type(detail.into()))
}
fn unaudited<T>(detail: impl Into<String>) -> Result<T, KirError> {
    Err(KirError::Unaudited(detail.into()))
}
/// A missing cell ends a check and is no finding of a scan.
macro_rules! missing_cell {
    ($walk:expr, $detail:expr) => {
        if $walk.scan {
            Ok(())
        } else {
            unaudited($detail)
        }
    };
}

impl Walk<'_, '_> {
    fn domain(&self, slot: u32) -> Result<TargetDomain, KirError> {
        match self.env.targets.get(slot as usize) {
            Some(target) if target.slot == slot => Ok(target.domain),
            _ => scope(format!("target slot {slot} is not announced")),
        }
    }

    fn reference(&mut self, reference: &Ref) -> Result<RefKind, KirError> {
        if self.scan {
            if let Ref::Target { slot } = reference {
                self.out.target_slots.insert(*slot);
            }
            return Ok(RefKind::Object);
        }
        let found = match reference {
            Ref::Source if self.env.context == Context::Spell => {
                return unaudited("spell source as an object operand");
            }
            Ref::Source => RefKind::Object,
            Ref::Controller { role } => {
                if *role != self.env.context.controller_role() {
                    return scope(format!(
                        "controller role {role:?} in a {:?} context",
                        self.env.context
                    ));
                }
                RefKind::Player
            }
            Ref::Target { slot } => {
                let domain = self.domain(*slot)?;
                self.out.target_slots.insert(*slot);
                match domain {
                    TargetDomain::BattlefieldPermanent
                    | TargetDomain::ControlledBattlefieldPermanent
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::CreatureOrPlaneswalker
                    | TargetDomain::OwnGraveyardCreatureCard
                    | TargetDomain::OwnGraveyardCard => RefKind::Object,
                    TargetDomain::AnyDamageTarget => RefKind::Entity,
                    TargetDomain::AnyPlayer
                    | TargetDomain::OpponentPlayer
                    | TargetDomain::PlayerOrPlaneswalker => RefKind::Player,
                    TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility => RefKind::StackObject,
                    TargetDomain::PlayerGraveyardCards { .. } => RefKind::Pool,
                }
            }
            Ref::EventPlayer { role } => {
                let Some(event) = self
                    .env
                    .event
                    .filter(|_| self.env.context == Context::Triggered)
                else {
                    return scope("event player outside a triggered ability");
                };
                if !event_supplies(event, *role) {
                    return scope(format!("event has no {role:?}"));
                }
                self.out.event_player = true;
                RefKind::Player
            }
            Ref::AnnouncedX => {
                if !self.env.announced_x {
                    return scope("X is not announced for this object");
                }
                self.out.announced_x = true;
                RefKind::Number
            }
            Ref::BoundVar { id } => match self.bound.iter().rev().find(|(bound, _)| bound == id) {
                Some((_, kind)) => *kind,
                None => return scope(format!("variable {id} is not bound")),
            },
            Ref::StepResult { step, role } => {
                if *step >= self.steps {
                    return scope(format!("step {step} has not been performed"));
                }
                match role {
                    StepRole::Objects => RefKind::Pool,
                    StepRole::Player => RefKind::Player,
                    StepRole::Amount => RefKind::Number,
                }
            }
            // No audited cell reads these yet. They are in the vocabulary so
            // the binder and the checker agree on scope before the first verb
            // that needs them is audited.
            Ref::AttachedTo => return unaudited("attached-to reference"),
            Ref::EventObject { role } => {
                if self.env.event.is_none() || self.env.context != Context::Triggered {
                    return scope("event object outside a triggered ability");
                }
                return unaudited(format!("event object {role:?}"));
            }
            Ref::EnteringObject => return unaudited("entering-object reference"),
            Ref::LinkedPool { .. } => return unaudited("linked pool reference"),
        };
        if self.plain_leaf {
            match reference {
                Ref::BoundVar { .. } | Ref::StepResult { .. } => {
                    return unaudited(
                        "ordinary kernel leaf reads a control binding or step result",
                    );
                }
                Ref::Target { .. } if self.env.targets.len() != 1 => {
                    return unaudited("nested leaf requires a single announced target");
                }
                _ => {}
            }
        }
        Ok(found)
    }

    fn expect(
        &mut self,
        reference: &Ref,
        wanted: &[RefKind],
        operand: &str,
    ) -> Result<RefKind, KirError> {
        let found = self.reference(reference)?;
        if self.scan || wanted.contains(&found) {
            Ok(found)
        } else {
            kind(format!("{operand} is a {found:?}"))
        }
    }

    fn player(&mut self, player: &PlayerExpr) -> Result<(), KirError> {
        match player {
            PlayerExpr::Is { who } => self.expect(who, &[RefKind::Player], "player").map(drop),
            PlayerExpr::ControllerOf { object } => self
                .expect(
                    object,
                    &[RefKind::Object, RefKind::StackObject],
                    "controlled object",
                )
                .map(drop),
            PlayerExpr::OwnerOf { object } => self
                .expect(object, &[RefKind::Object], "owned object")
                .map(drop),
            PlayerExpr::EachOpponent | PlayerExpr::EachPlayer => Ok(()),
        }
    }

    fn pred(&mut self, pred: &Pred) -> Result<(), KirError> {
        for atom in &pred.0 {
            match atom {
                Atom::Type { .. }
                | Atom::NotType { .. }
                | Atom::Legendary
                | Atom::NotLegendary
                | Atom::Basic
                | Atom::NotBasic
                | Atom::Color { .. }
                | Atom::NotColor { .. }
                | Atom::Tapped
                | Atom::Untapped
                | Atom::Attacking
                | Atom::Blocking
                | Atom::Token
                | Atom::NotToken
                | Atom::Keyword { .. }
                | Atom::NotKeyword { .. } => {}
                Atom::AnyType { kinds } => {
                    if kinds.is_empty() {
                        return kind("empty type alternatives");
                    }
                }
                Atom::Subtype { subtype } | Atom::NotSubtype { subtype } => {
                    if subtype.is_empty() {
                        return kind("empty subtype");
                    }
                }
                Atom::ControlledBy { player } | Atom::OwnedBy { player } => self.player(player)?,
                Atom::Power { value, .. }
                | Atom::Toughness { value, .. }
                | Atom::ManaValue { value, .. } => self.amount(value)?,
                Atom::IsNot { object } => {
                    self.expect(object, &[RefKind::Object], "excluded object")?;
                }
                Atom::AnyOf { alternatives } => {
                    if alternatives.is_empty() {
                        return kind("empty predicate alternatives");
                    }
                    for alternative in alternatives {
                        self.pred(alternative)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn zone(&mut self, zone: &ZoneExpr) -> Result<(), KirError> {
        match zone {
            ZoneExpr::Graveyard { of } | ZoneExpr::Hand { of } | ZoneExpr::Library { of } => {
                self.player(of)
            }
            ZoneExpr::Battlefield | ZoneExpr::Graveyards | ZoneExpr::Exile | ZoneExpr::Stack => {
                Ok(())
            }
        }
    }

    fn set(&mut self, set: &SetExpr) -> Result<(), KirError> {
        match set {
            SetExpr::One { object } => self
                .expect(
                    object,
                    &[RefKind::Object, RefKind::StackObject, RefKind::Entity],
                    "object",
                )
                .map(drop),
            SetExpr::Matching { zone, filter } => {
                self.zone(zone)?;
                self.pred(filter)
            }
            SetExpr::Pool { pool } => self.expect(pool, &[RefKind::Pool], "pool").map(drop),
        }
    }

    fn amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        match amount {
            Amount::Fixed { .. } => Ok(()),
            Amount::Var { of } => self.expect(of, &[RefKind::Number], "amount").map(drop),
            Amount::Count { set } => {
                // CR 400.2, 402.3, 401.2: the size of a hand or library is
                // public, which cards it holds is not. A filtered count of a
                // hidden zone would publish the answer of a private question.
                if let SetExpr::Matching { zone, filter } = set
                    && zone.hidden()
                    && !filter.0.is_empty()
                {
                    return Err(KirError::HiddenCount);
                }
                self.set(set)
            }
            Amount::StatOf { object, .. } => self
                .expect(object, &[RefKind::Object], "measured object")
                .map(drop),
            Amount::LifeOf { player } | Amount::OtherCastsManaValue { caster: player } => {
                self.player(player)
            }
        }
    }

    fn damage_to(&mut self, to: &DamageTo) -> Result<(), KirError> {
        match to {
            DamageTo::Ref { who } => self
                .expect(
                    who,
                    &[RefKind::Object, RefKind::Player, RefKind::Entity],
                    "damage recipient",
                )
                .map(drop),
            DamageTo::Players { players } => self.player(players),
            DamageTo::Objects { set } => self.set(set),
            DamageTo::All { parts } => {
                if parts.is_empty() {
                    return kind("empty damage recipients");
                }
                parts.iter().try_for_each(|part| self.damage_to(part))
            }
        }
    }

    fn destination(&mut self, to: &Destination) -> Result<(), KirError> {
        match to {
            Destination::Battlefield { controller, .. } => self.player(controller),
            Destination::Exile
            | Destination::OwnerHand
            | Destination::OwnerLibraryTop
            | Destination::OwnerLibraryBottom
            | Destination::OwnerGraveyard => Ok(()),
        }
    }

    fn control_player(&mut self, player: &PlayerExpr, single: bool) -> Result<(), KirError> {
        self.player(player)?;
        if self.scan {
            return Ok(());
        }
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. } | Ref::BoundVar { .. },
            } => Ok(()),
            PlayerExpr::Is {
                who: Ref::EventPlayer { role },
            } if Self::recipient_role(*role) => Ok(()),
            PlayerExpr::Is {
                who: Ref::Target { slot },
            } if self.env.targets[..=*slot as usize].iter().all(|target| {
                matches!(
                    target.domain,
                    TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer
                )
            }) =>
            {
                Ok(())
            }
            // CR 108.3: ownership is recorded on the physical card and
            // survives an incarnation's departure. These references always
            // supply one identity, including a singleton iteration binding.
            PlayerExpr::OwnerOf {
                object: Ref::Source | Ref::BoundVar { .. },
            } => Ok(()),
            PlayerExpr::EachPlayer | PlayerExpr::EachOpponent if !single => Ok(()),
            // Object-derived players can need an absent object's last
            // known information (CR 608.2h). The continuation has no such
            // lookup, and a target may already be illegal at resolution.
            _ => unaudited("control player expression"),
        }
    }

    fn control_set(&mut self, set: &SetExpr) -> Result<(), KirError> {
        self.set(set)?;
        self.audit_control_set(set)
    }

    fn audit_control_set(&self, set: &SetExpr) -> Result<(), KirError> {
        if self.scan {
            return Ok(());
        }
        match set {
            SetExpr::Matching {
                zone: zone @ (ZoneExpr::Battlefield | ZoneExpr::Graveyards),
                filter,
            } => Self::audited_filter(zone, filter),
            SetExpr::One {
                object: Ref::Source | Ref::BoundVar { .. },
            }
            | SetExpr::Pool {
                pool: Ref::BoundVar { .. },
            } => Ok(()),
            SetExpr::One {
                object: Ref::Target { slot },
            } if !matches!(
                self.domain(*slot)?,
                TargetDomain::AnyDamageTarget
                    | TargetDomain::StackSpell
                    | TargetDomain::ControlledCreatureSpell
                    | TargetDomain::StackActivatedOrTriggeredAbility
            ) =>
            {
                Ok(())
            }
            _ => unaudited("control object set"),
        }
    }

    fn control_amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        self.amount(amount)?;
        if self.scan {
            return Ok(());
        }
        match amount {
            Amount::Fixed { .. } => Ok(()),
            Amount::Var {
                of: Ref::AnnouncedX,
            } => Ok(()),
            Amount::Count { set } => self.audit_control_set(set),
            // A life subject may be absent or have departed (CR 800.4i).
            // Cast-history amounts require a paid spell, including when a
            // program can also be run by a spell copy (CR 707.10).
            _ => unaudited("control amount"),
        }
    }

    fn nonnegative_control_amount(&mut self, amount: &Amount) -> Result<(), KirError> {
        self.control_amount(amount)?;
        if !self.scan && amount.as_fixed().is_some_and(Quantity::is_negative) {
            return kind("negative control amount");
        }
        Ok(())
    }

    fn condition(&mut self, condition: &Cond) -> Result<(), KirError> {
        match condition {
            Cond::Compare { left, right, .. } => {
                self.control_amount(left)?;
                self.control_amount(right)
            }
            Cond::Exists { set } => self.control_set(set),
            Cond::CostPaid { identity } => {
                if identity.is_empty() {
                    return kind("empty cost identity");
                }
                Ok(())
            }
            Cond::Not { condition } => self.condition(condition),
            Cond::All { conditions } => conditions.iter().try_for_each(|c| self.condition(c)),
        }
    }

    fn delayed(&mut self, event: &DelayedEvent) -> Result<(), KirError> {
        match event {
            DelayedEvent::NextUpkeep { of } => self.player(of),
            DelayedEvent::NextEndStep | DelayedEvent::EndOfCombat | DelayedEvent::Event { .. } => {
                Ok(())
            }
        }
    }

    /// Scope and type of every operand of a verb. Total over [`Verb`].
    fn operands(&mut self, verb: &Verb) -> Result<(), KirError> {
        match verb {
            Verb::Draw { player, amount }
            | Verb::GainLife { player, amount }
            | Verb::LoseLife { player, amount }
            | Verb::Mill { player, amount }
            | Verb::Discard { player, amount, .. }
            | Verb::Scry { player, amount }
            | Verb::Surveil { player, amount } => {
                self.player(player)?;
                self.amount(amount)
            }
            Verb::Damage { source, to, amount } => {
                // CR 120.1: a spell can deal damage as its own source. This
                // attribution does not select the spell as a permanent.
                if *source != Ref::Source {
                    self.expect(source, &[RefKind::Object], "damage source")?;
                }
                self.damage_to(to)?;
                self.amount(amount)
            }
            Verb::Destroy { what, .. }
            | Verb::Tap { what }
            | Verb::Untap { what }
            | Verb::Regenerate { what } => self.set(what),
            Verb::Move { what, to } => {
                self.set(what)?;
                self.destination(to)
            }
            Verb::PutCounters {
                on,
                counter,
                amount,
            } => {
                if matches!(counter, CounterKind::Named { name } if name.is_empty()) {
                    return kind("empty counter name");
                }
                self.set(on)?;
                self.amount(amount)
            }
            Verb::CreateTokens {
                controller,
                count,
                token,
            } => {
                if let TokenSpec::Creature { spec } = token {
                    spec.validate().map_err(KirError::Type)?;
                }
                self.player(controller)?;
                self.amount(count)
            }
            Verb::Sacrifice {
                player,
                count,
                filter,
            } => {
                self.player(player)?;
                self.amount(count)?;
                self.pred(filter)
            }
            Verb::AddMana { player, mana } => {
                self.player(player)?;
                match mana {
                    ManaSpec::Color { amount, .. } => self.amount(amount),
                    ManaSpec::AnyCombination { colors, amount } => {
                        if colors.is_empty() {
                            return kind("empty mana colors");
                        }
                        self.amount(amount)
                    }
                }
            }
            Verb::Search { player, search } => {
                self.player(player)?;
                self.pred(&search.filter)?;
                self.amount(&search.maximum)?;
                self.destination(&search.to)
            }
            Verb::Counter { what } | Verb::CopyStackObject { what } => self
                .expect(what, &[RefKind::StackObject], "stack object")
                .map(drop),
            Verb::GainControl {
                what, controller, ..
            } => {
                self.set(what)?;
                self.player(controller)
            }
            Verb::Attach { what, to } => {
                self.expect(what, &[RefKind::Object], "attachment")?;
                self.expect(to, &[RefKind::Object], "attachment host")
                    .map(drop)
            }
            Verb::ModifyUntil {
                what,
                layer,
                change,
                ..
            } => {
                // CR 613.1: the tag is the layer the change applies in.
                if *layer != change.layer() {
                    return kind(format!("{layer:?} tag on a {:?} change", change.layer()));
                }
                self.set(what)?;
                match change {
                    Change::PowerToughness { power, toughness }
                    | Change::SetPowerToughness { power, toughness } => {
                        self.amount(power)?;
                        self.amount(toughness)
                    }
                    Change::AddKeywords { keywords } | Change::RemoveKeywords { keywords } => {
                        if keywords.is_empty() {
                            return kind("empty keyword change");
                        }
                        Ok(())
                    }
                    Change::SetColors { .. } => Ok(()),
                    Change::AddTypes { kinds } => {
                        if kinds.is_empty() {
                            return kind("empty type change");
                        }
                        Ok(())
                    }
                    Change::Controller { player } => self.player(player),
                }
            }
            Verb::CreateDelayedTrigger { event, node } => {
                self.delayed(event)?;
                self.nested(node, false)
            }
            Verb::PlayPermission { player, what, .. } => {
                self.player(player)?;
                self.set(what)
            }
            Verb::Transform { what } => self.expect(what, &[RefKind::Object], "object").map(drop),
            Verb::SetDesignation { player, .. } | Verb::Shuffle { player } => self.player(player),
            Verb::Builtin { .. } => Ok(()),
        }
    }

    /// A child node with its own step numbering.
    fn nested(&mut self, node: &Node, in_control: bool) -> Result<(), KirError> {
        let steps = std::mem::take(&mut self.steps);
        let result = self.node(node, in_control);
        self.steps = steps;
        result
    }

    fn node(&mut self, node: &Node, in_control: bool) -> Result<(), KirError> {
        match node {
            Node::Do(verb) => {
                match verb {
                    Verb::GainLife { player, amount } | Verb::LoseLife { player, amount }
                        if in_control =>
                    {
                        self.control_player(player, false)?;
                        self.nonnegative_control_amount(amount)?;
                    }
                    _ => {
                        let enclosing_leaf = std::mem::replace(&mut self.plain_leaf, in_control);
                        let operands = self.operands(verb);
                        self.plain_leaf = enclosing_leaf;
                        operands?;
                        if !self.scan {
                            self.audit(verb)?;
                            if in_control {
                                Self::audit_nested_leaf(verb)?;
                            }
                        }
                    }
                }
                if !self.scan {
                    verb_capabilities(verb, &mut self.out.capabilities);
                }
                Ok(())
            }
            Node::Seq(nodes) => {
                // CR 119.3, 608.2c: the drain pair is one resolution step
                // with no decision between its two life events.
                if let Some(amount) = drain(node) {
                    self.amount(amount)?;
                    if let [_, Node::Do(Verb::GainLife { player, .. })] = nodes.as_slice() {
                        self.player(player)?;
                    }
                    if !self.scan {
                        if in_control {
                            return unaudited("nested drain reads a step result");
                        }
                        Self::fixed_or_announced_x(amount, 1, "drain amount")?;
                        self.out.capabilities.extend(DRAIN_CAPABILITIES);
                    }
                    return Ok(());
                }
                let steps = std::mem::take(&mut self.steps);
                let result = nodes.iter().try_for_each(|node| {
                    self.node(node, true)?;
                    self.steps += 1;
                    Ok(())
                });
                self.steps = steps;
                result
            }
            Node::If {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition)?;
                self.nested(then, true)?;
                if let Some(otherwise) = otherwise {
                    self.nested(otherwise, true)?;
                }
                Ok(())
            }
            Node::Optional {
                chooser,
                node: child,
            } => {
                self.control_player(chooser, true)?;
                // CR 608.2d: "you may destroy target permanent", a choice the
                // resolving frame offers at this instruction's cursor.
                if let Some((verb, slot, regeneration_forbidden)) = node.optional_target_destroy() {
                    self.operands(verb)?;
                    if !self.scan {
                        if in_control {
                            return unaudited(
                                "nested optional target destroy uses a different choice route",
                            );
                        }
                        if regeneration_forbidden
                            || self.domain(slot)? != TargetDomain::BattlefieldPermanent
                        {
                            return unaudited("optional destroy target domain");
                        }
                        verb_capabilities(verb, &mut self.out.capabilities);
                    }
                    return Ok(());
                }
                self.nested(child, true)
            }
            Node::ForEach { var, over, body } => {
                let bound = match over {
                    Domain::Players { players } => {
                        self.control_player(players, false)?;
                        RefKind::Player
                    }
                    Domain::Objects { set } => {
                        self.control_set(set)?;
                        RefKind::Object
                    }
                };
                self.bound.push((*var, bound));
                let result = self.nested(body, true);
                self.bound.pop();
                result
            }
            Node::Choose {
                var,
                chooser,
                from,
                minimum,
                maximum,
                body,
            } => {
                self.control_player(chooser, true)?;
                self.control_set(from)?;
                self.nonnegative_control_amount(minimum)?;
                self.nonnegative_control_amount(maximum)?;
                if !self.scan {
                    Self::audit_choice(from, minimum, maximum)?;
                }
                self.bound.push((*var, RefKind::Pool));
                let result = self.nested(body, true);
                self.bound.pop();
                result
            }
            Node::UnlessPays {
                payer,
                payment,
                node,
            } => {
                self.control_player(payer, true)?;
                match payment {
                    Payment::Generic { amount } | Payment::Life { amount } => {
                        self.nonnegative_control_amount(amount)?
                    }
                }
                self.nested(node, true)
            }
            Node::Delayed { event, node } => {
                self.delayed(event)?;
                self.nested_unaudited(node)?;
                missing_cell!(self, "delayed node")
            }
            Node::Reflexive { first, then } => {
                self.nested_unaudited(first)?;
                self.nested_unaudited(then)?;
                missing_cell!(self, "reflexive node")
            }
        }
    }

    /// Scope-check the child of an unsupported trigger control. Only an
    /// audited parent may rely on a successful child audit.
    fn nested_unaudited(&mut self, node: &Node) -> Result<(), KirError> {
        match self.nested(node, true) {
            Err(KirError::Unaudited(_)) => Ok(()),
            result => result,
        }
    }

    fn audit_choice(from: &SetExpr, minimum: &Amount, maximum: &Amount) -> Result<(), KirError> {
        if let (Some(min), Some(max)) = (minimum.as_fixed(), maximum.as_fixed())
            && min > max
        {
            return kind("reversed choice bounds");
        }
        if !matches!(from, SetExpr::Matching { .. }) {
            return unaudited("choice needs a current public object set");
        }
        // CR 800.4a/g: an interactive choice stores candidates and a chooser
        // across concessions. Core cannot replace a departed chooser or
        // refresh candidates. These bounds force the empty or entire set.
        if minimum == maximum
            && (minimum.as_fixed() == Some(&Quantity::zero())
                || matches!(minimum, Amount::Count { set } if set == from))
        {
            return Ok(());
        }
        unaudited("choice can suspend with stale candidates")
    }

    fn audit_nested_leaf(verb: &Verb) -> Result<(), KirError> {
        match verb {
            // These executors authenticate against top-level instructions
            // after the temporary leaf program has been removed.
            Verb::CreateTokens { .. } | Verb::ModifyUntil { .. } => {
                unaudited("nested leaf needs persistent provenance")
            }
            Verb::Damage { to, .. } if !matches!(to, DamageTo::Ref { .. }) => {
                unaudited("nested set damage leaves top-level cursor facts")
            }
            // CR 121.2: counted draws retain attempt progress and finish
            // with an outcome that must match a top-level draw instruction.
            Verb::Draw { player, amount } if !single_draw(player, amount) => {
                unaudited("nested counted draw needs a top-level draw instruction")
            }
            _ => Ok(()),
        }
    }

    /// A fixed amount of at least `minimum`.
    fn fixed_at_least(amount: &Amount, minimum: i64, what: &str) -> Result<(), KirError> {
        match amount.as_fixed() {
            Some(value) if *value >= Quantity::from(minimum) => Ok(()),
            Some(_) => kind(format!("{what} below {minimum}")),
            None => unaudited(format!("computed {what}")),
        }
    }

    /// The event-player roles whose player the life and draw executors read:
    /// a cast trigger's caster and a step trigger's active player.
    fn recipient_role(role: EventPlayerRole) -> bool {
        matches!(
            role,
            EventPlayerRole::Caster | EventPlayerRole::ActivePlayer
        )
    }

    /// A fixed amount of at least `minimum`, or the announced X (CR 107.3).
    /// Returns true for the announced X.
    fn fixed_or_announced_x(amount: &Amount, minimum: i64, what: &str) -> Result<bool, KirError> {
        match amount {
            Amount::Var {
                of: Ref::AnnouncedX,
            } => Ok(true),
            _ => Self::fixed_at_least(amount, minimum, what).map(|()| false),
        }
    }

    /// The target domain of a player expression that names one target.
    fn player_target(&self, player: &PlayerExpr) -> Result<Option<TargetDomain>, KirError> {
        player
            .target_slot()
            .map(|slot| self.domain(slot))
            .transpose()
    }

    /// The audited cells. A cell is opened only where the core interpreter
    /// executes exactly these operands in this context and recovery re-derives
    /// the same facts.
    fn audit(&self, verb: &Verb) -> Result<(), KirError> {
        match verb {
            // CR 119.3: the controller, each opponent, one announced player
            // or the trigger's event player gains or loses a fixed amount.
            // A fixed zero (CR 119.8) is executed only as the reviewed tail
            // of the permanent-grant family; program validation keeps every
            // other life amount positive.
            Verb::GainLife { player, amount } | Verb::LoseLife { player, amount } => {
                // CR 107.3: an announced X is the amount for the controller
                // or one announced player only.
                let announced = Self::fixed_or_announced_x(amount, 0, "life amount")?;
                match player {
                    PlayerExpr::Is {
                        who: Ref::Controller { .. },
                    } => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::EventPlayer { role },
                    } if !announced && Self::recipient_role(*role) => Ok(()),
                    PlayerExpr::EachOpponent if !announced => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if self.player_target(player)? == Some(TargetDomain::AnyPlayer) => Ok(()),
                    _ => unaudited("life recipient"),
                }
            }
            // CR 121.1-121.2, 608.2b: only the single-draw executor skips
            // an illegal announced target and continues the remaining program.
            Verb::Draw { player, amount } => {
                let announced = Self::fixed_or_announced_x(amount, 0, "draw count")?;
                match player {
                    PlayerExpr::Is {
                        who: Ref::Controller { .. },
                    }
                    | PlayerExpr::EachPlayer => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::EventPlayer { role },
                    } if !announced && Self::recipient_role(*role) => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if single_draw(player, amount)
                        && self.player_target(player)? == Some(TargetDomain::AnyPlayer) =>
                    {
                        Ok(())
                    }
                    _ => unaudited("draw recipient or targeted count"),
                }
            }
            // CR 120.1-120.3: the resolving object's source deals damage.
            Verb::Damage { source, to, amount } => {
                if *source != Ref::Source {
                    return unaudited("damage source");
                }
                self.audit_damage(to, amount)
            }
            // CR 701.8, 608.2b: destroy one announced permanent or creature.
            // CR 701.19c: forbidding regeneration is audited for a permanent.
            Verb::Destroy {
                what:
                    SetExpr::One {
                        object: Ref::Target { slot },
                    },
                regeneration_forbidden,
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent => Ok(()),
                TargetDomain::BattlefieldCreature if !regeneration_forbidden => Ok(()),
                _ => unaudited("destroy target domain"),
            },
            // CR 400.3, 400.7, 608.2b: one announced permanent to exile, its
            // owner's hand or the top of its owner's library.
            Verb::Move {
                what:
                    SetExpr::One {
                        object: Ref::Target { slot },
                    },
                to,
            } => match (to, self.domain(*slot)?) {
                (Destination::Exile, TargetDomain::BattlefieldPermanent)
                | (
                    Destination::OwnerHand,
                    TargetDomain::BattlefieldPermanent
                    | TargetDomain::BattlefieldCreature
                    | TargetDomain::CreatureOrPlaneswalker,
                )
                | (Destination::OwnerLibraryTop, TargetDomain::BattlefieldCreature) => Ok(()),
                _ => unaudited("move target destination"),
            },
            // CR 701.26a-b, 608.2b: tap or untap one announced permanent.
            Verb::Tap {
                what:
                    SetExpr::One {
                        object: Ref::Target { slot },
                    },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent | TargetDomain::BattlefieldCreature => Ok(()),
                _ => unaudited("tap target domain"),
            },
            Verb::Untap {
                what:
                    SetExpr::One {
                        object: Ref::Target { slot },
                    },
            } => match self.domain(*slot)? {
                TargetDomain::BattlefieldPermanent => Ok(()),
                _ => unaudited("untap target domain"),
            },
            // CR 701.26b, 608.2h: untap every land you control.
            Verb::Untap { what } if *what == SetExpr::lands_you_control(self.env.context) => Ok(()),
            // CR 404.1, 400.3: exile every card in one announced player's
            // graveyard.
            Verb::Move {
                what:
                    SetExpr::Matching {
                        zone:
                            ZoneExpr::Graveyard {
                                of:
                                    PlayerExpr::Is {
                                        who: Ref::Target { slot },
                                    },
                            },
                        filter,
                    },
                to: Destination::Exile,
            } if filter.0.is_empty() && self.domain(*slot)? == TargetDomain::AnyPlayer => Ok(()),
            // CR 701.8, 608.2h: destroy every battlefield permanent that
            // satisfies the filter as the instruction is performed.
            // CR 701.19c: forbidding regeneration is audited for the nonland
            // permanent wave only.
            Verb::Destroy {
                what:
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    },
                regeneration_forbidden,
            } => {
                Self::audited_filter(&ZoneExpr::Battlefield, filter)?;
                if *regeneration_forbidden && *filter != Pred::nonland_permanents() {
                    return unaudited("regeneration prohibition for this set");
                }
                Ok(())
            }
            // CR 400.3, 400.7: exile, or return to owners' hands, every
            // matching battlefield permanent; or exile every graveyard card.
            Verb::Move {
                what:
                    SetExpr::Matching {
                        zone: ZoneExpr::Battlefield,
                        filter,
                    },
                to: Destination::Exile | Destination::OwnerHand,
            } => Self::audited_filter(&ZoneExpr::Battlefield, filter),
            Verb::Move {
                what:
                    SetExpr::Matching {
                        zone: ZoneExpr::Graveyards,
                        filter,
                    },
                to: Destination::Exile,
            } if filter.0.is_empty() => Ok(()),
            // CR 701.17a: the controller or one announced player mills a
            // fixed count; one announced player mills the announced X.
            Verb::Mill { player, amount } => {
                let announced = Self::fixed_or_announced_x(amount, 0, "mill count")?;
                match player {
                    PlayerExpr::Is {
                        who: Ref::Controller { .. },
                    } if !announced => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if self.player_target(player)? == Some(TargetDomain::AnyPlayer) => Ok(()),
                    _ => unaudited("mill recipient"),
                }
            }
            // CR 701.9a-b: the controller or the damaged player discards a
            // fixed count, one announced player discards a positive count
            // (at random, also the announced X), and each player or each
            // opponent discards a positive count.
            Verb::Discard {
                player,
                amount,
                random,
            } => {
                let announced = Self::fixed_or_announced_x(amount, 0, "discard count")?;
                match player {
                    PlayerExpr::Is {
                        who:
                            Ref::Controller { .. }
                            | Ref::EventPlayer {
                                role: EventPlayerRole::DamagedPlayer,
                            },
                    } if !random && !announced => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if matches!(
                        self.player_target(player)?,
                        Some(TargetDomain::AnyPlayer | TargetDomain::OpponentPlayer)
                    ) && (*random || !announced) =>
                    {
                        if announced {
                            Ok(())
                        } else {
                            Self::fixed_at_least(amount, 1, "discard count")
                        }
                    }
                    PlayerExpr::EachPlayer | PlayerExpr::EachOpponent if !random && !announced => {
                        Self::fixed_at_least(amount, 1, "discard count")
                    }
                    _ => unaudited("discard recipient"),
                }
            }
            // CR 701.21a: the damaged player, one announced opponent, each
            // player or each opponent sacrifices one creature.
            Verb::Sacrifice {
                player,
                count,
                filter,
            } if *filter == Pred::creatures() && count.as_fixed() == Some(&Quantity::from(1)) => {
                match player {
                    PlayerExpr::Is {
                        who:
                            Ref::EventPlayer {
                                role: EventPlayerRole::DamagedPlayer,
                            },
                    }
                    | PlayerExpr::EachPlayer
                    | PlayerExpr::EachOpponent => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if self.player_target(player)? == Some(TargetDomain::OpponentPlayer) => {
                        Ok(())
                    }
                    _ => unaudited("sacrifice recipient"),
                }
            }
            // CR 111.1-111.4, 111.10: "you" create a positive fixed number
            // of tokens of one exact recipe.
            Verb::CreateTokens {
                controller:
                    PlayerExpr::Is {
                        who: Ref::Controller { .. },
                    },
                count,
                ..
            } => {
                Self::fixed_at_least(count, 1, "token count")?;
                if count.as_fixed().and_then(Quantity::to_u32).is_none() {
                    return kind("token count above the supported range");
                }
                Ok(())
            }
            // CR 122.1, 122.6: a positive fixed number of +1/+1 counters on
            // the exact source or one announced creature.
            Verb::PutCounters {
                on: SetExpr::One { object },
                counter: CounterKind::PlusOnePlusOne,
                amount,
            } => {
                Self::fixed_at_least(amount, 1, "counter amount")?;
                match object {
                    Ref::Source => Ok(()),
                    Ref::Target { slot } => match self.domain(*slot)? {
                        TargetDomain::BattlefieldCreature
                        | TargetDomain::ControlledBattlefieldCreature => Ok(()),
                        _ => unaudited("counter target domain"),
                    },
                    _ => unaudited("counter recipient"),
                }
            }
            // CR 611.2c, 613.4c, 514.2: a fixed signed P/T modification of the
            // exact source, one announced creature or a board-wide set until
            // end of turn.
            Verb::ModifyUntil {
                what,
                layer: Layer::PowerToughnessModify,
                change: Change::PowerToughness { power, toughness },
                until: Until::EndOfTurn,
            } if power.as_fixed().is_some() && toughness.as_fixed().is_some() => match what {
                SetExpr::One {
                    object: Ref::Source,
                } => Ok(()),
                SetExpr::One {
                    object: Ref::Target { slot },
                } => match self.domain(*slot)? {
                    TargetDomain::BattlefieldCreature
                    | TargetDomain::BlockingBattlefieldCreature
                    | TargetDomain::ControlledBattlefieldCreature => Ok(()),
                    _ => unaudited("pump target domain"),
                },
                // CR 613.4c: a P/T change applies to creatures; the set is
                // read through the general battlefield query and recovered as
                // a creature-only capture.
                SetExpr::Matching {
                    zone: ZoneExpr::Battlefield,
                    filter,
                } if filter.0.contains(&Atom::Type {
                    kind: CardKind::Creature,
                }) =>
                {
                    Self::audited_filter(&ZoneExpr::Battlefield, filter)
                }
                _ => unaudited("pump set"),
            },
            // CR 611.2c, 613.1f, 514.2: the exact source gains keywords until
            // end of turn.
            Verb::ModifyUntil {
                what: SetExpr::One {
                    object: Ref::Source,
                },
                layer: Layer::Ability,
                change: Change::AddKeywords { keywords },
                until: Until::EndOfTurn,
            } if self.env.context == Context::Activated
                && super::supported_temporary_keywords(keywords) =>
            {
                Ok(())
            }
            _ => unaudited("verb"),
        }
    }

    /// CR 120.2b: noncombat damage to one announced recipient, the
    /// controller or the mana ability's activator, or (CR 608.2h) to every
    /// creature of a board-wide set and optionally each player.
    fn audit_damage(&self, to: &DamageTo, amount: &Amount) -> Result<(), KirError> {
        match to {
            DamageTo::Ref {
                who: Ref::Target { slot },
            } => {
                let announced = Self::fixed_or_announced_x(amount, 1, "damage amount")?;
                match (self.domain(*slot)?, announced) {
                    (TargetDomain::AnyDamageTarget | TargetDomain::BattlefieldCreature, _)
                    | (
                        TargetDomain::OpponentPlayer
                        | TargetDomain::PlayerOrPlaneswalker
                        | TargetDomain::BattlefieldPermanent,
                        false,
                    ) => Ok(()),
                    _ => unaudited("damage target domain"),
                }
            }
            DamageTo::Ref {
                who: Ref::Controller { .. },
            }
            | DamageTo::Ref {
                who:
                    Ref::EventPlayer {
                        role: EventPlayerRole::ActivatingPlayer,
                    },
            } => Self::fixed_at_least(amount, 1, "damage amount"),
            DamageTo::Ref { .. } => unaudited("damage recipient"),
            set => {
                Self::set_damage_parts(set)?;
                match amount {
                    Amount::Fixed { .. } => Self::fixed_at_least(amount, 1, "damage amount"),
                    Amount::Var {
                        of: Ref::AnnouncedX,
                    } => Ok(()),
                    Amount::Count { set } if *set == SetExpr::battlefield(Pred::creatures()) => {
                        Ok(())
                    }
                    Amount::OtherCastsManaValue { caster } if caster.is_you() => Ok(()),
                    _ => unaudited("set damage amount"),
                }
            }
        }
    }

    /// The parts of a set damage recipient: creature sets of the battlefield
    /// and at most one "each player", in that order.
    fn set_damage_parts(to: &DamageTo) -> Result<(), KirError> {
        let parts = match to {
            DamageTo::All { parts } => parts.as_slice(),
            part => std::slice::from_ref(part),
        };
        let mut players = false;
        for part in parts {
            match part {
                DamageTo::Objects {
                    set:
                        SetExpr::Matching {
                            zone: ZoneExpr::Battlefield,
                            filter,
                        },
                } if !players
                    && filter.0.first()
                        == Some(&Atom::Type {
                            kind: CardKind::Creature,
                        }) =>
                {
                    Self::audited_filter(&ZoneExpr::Battlefield, filter)?
                }
                DamageTo::Players {
                    players: PlayerExpr::EachPlayer,
                } if !players => players = true,
                _ => return unaudited("set damage recipient"),
            }
        }
        Ok(())
    }

    /// The atoms the core can evaluate for a set in this zone (CR 608.2h).
    fn audited_filter(zone: &ZoneExpr, filter: &Pred) -> Result<(), KirError> {
        for atom in &filter.0 {
            match atom {
                // CR 110.5d, 506.3–4, 109.4: graveyard cards have no
                // permanent status, combat participation, or controller.
                Atom::Tapped | Atom::Untapped | Atom::Attacking | Atom::Blocking
                | Atom::ControlledBy { .. } if *zone != ZoneExpr::Battlefield => {
                    return unaudited("battlefield predicate outside the battlefield");
                }
                Atom::Type { .. }
                | Atom::NotType { .. }
                | Atom::AnyType { .. }
                | Atom::Subtype { .. }
                | Atom::NotSubtype { .. }
                | Atom::Legendary
                | Atom::NotLegendary
                | Atom::Basic
                | Atom::NotBasic
                | Atom::Color { .. }
                | Atom::NotColor { .. }
                | Atom::Tapped
                | Atom::Untapped
                | Atom::Attacking
                // CR 111.6: tokenhood is identity. Graveyard card sets
                // already exclude tokens regardless of this predicate.
                | Atom::Token
                | Atom::NotToken
                | Atom::Keyword { .. }
                | Atom::NotKeyword { .. } => {}
                Atom::ControlledBy { player } => {
                    if !player.is_you() && *player != PlayerExpr::EachOpponent {
                        return unaudited("set controller");
                    }
                }
                Atom::Power { value, .. }
                | Atom::Toughness { value, .. }
                | Atom::ManaValue { value, .. } => {
                    if value.as_fixed().is_none() {
                        return unaudited("computed set comparison");
                    }
                }
                Atom::AnyOf { alternatives } => {
                    alternatives
                        .iter()
                        .try_for_each(|alternative| Self::audited_filter(zone, alternative))?;
                }
                Atom::Blocking | Atom::OwnedBy { .. } | Atom::IsNot { .. } => {
                    return unaudited("set filter atom");
                }
            }
        }
        Ok(())
    }
}

/// A verb names the reviewed runtime surface it uses. The program adds any
/// requirements that also depend on its targets or family.
fn verb_capabilities(verb: &Verb, out: &mut BTreeSet<Capability>) {
    let event_player = |player: &PlayerExpr, out: &mut BTreeSet<Capability>| {
        if player.is_event_player() {
            out.insert(Capability::EventPlayerRecipientV1);
        }
    };
    match verb {
        Verb::GainLife { player, .. } => {
            out.insert(Capability::LifeGainV1);
            event_player(player, out);
            // The each-opponent life executor shares the reviewed group
            // surface that sacrifice admission gates.
            if *player == PlayerExpr::EachOpponent {
                out.insert(Capability::SacrificeV1);
            }
        }
        Verb::LoseLife { player, .. } => {
            out.insert(Capability::LifeLossV1);
            event_player(player, out);
            if *player == PlayerExpr::EachOpponent {
                out.insert(Capability::SacrificeV1);
            }
        }
        // CR 121.2c, 107.3: the group and announced-X draws name their own
        // surface. A fixed draw by one player is named by the complete
        // program it belongs to (`Definition::requirements`).
        Verb::Draw { player, amount } => {
            event_player(player, out);
            if *player == PlayerExpr::EachPlayer {
                out.insert(Capability::EachPlayerDrawV1);
            } else if amount.is_announced_x() {
                out.insert(Capability::AnnouncedXDrawV1);
            }
        }
        // CR 120.2b: directed noncombat damage; an announced X names its
        // surface too. Board-wide damage shares the reviewed wipe surface, and
        // a cast-history amount names the history surface.
        Verb::Damage { to, amount, .. } => match to {
            DamageTo::Ref { .. } => {
                out.insert(Capability::NoncombatDamageV1);
                if amount.is_announced_x() {
                    out.insert(Capability::AnnouncedXEffectsV1);
                }
            }
            _ => {
                out.insert(Capability::BoardWipesV1);
                if matches!(amount, Amount::OtherCastsManaValue { .. }) {
                    out.extend([Capability::CastManaValueSweepV1, Capability::CastHistoryV1]);
                }
            }
        },
        // CR 701.8: destroying every creature names the reviewed creature
        // destruction surface; every other board-wide destruction and
        // movement shares the reviewed wipe surface.
        Verb::Destroy {
            what: SetExpr::Matching { zone, filter },
            ..
        } if *zone == ZoneExpr::Battlefield && *filter == Pred::creatures() => {
            out.insert(Capability::DestroyCreatureV1);
        }
        // CR 404.1: exiling one announced player's graveyard.
        Verb::Move {
            what:
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyard { .. },
                    ..
                },
            ..
        } => {
            out.insert(Capability::EntryGraveyardExileV1);
        }
        Verb::Destroy {
            what: SetExpr::Matching { .. },
            ..
        }
        | Verb::Move {
            what: SetExpr::Matching { .. },
            ..
        } => {
            out.insert(Capability::BoardWipesV1);
        }
        // One announced permanent: each verb and destination names its
        // reviewed surface. A target destroy's surface depends on its target
        // domain and is named with the program (`Definition::requirements`).
        Verb::Move {
            what: SetExpr::One { .. },
            to,
        } => {
            out.insert(match to {
                Destination::OwnerHand => Capability::OwnerHandMoveV1,
                Destination::OwnerLibraryTop => Capability::OwnerLibraryTopMoveV1,
                _ => Capability::PermanentRemovalV1,
            });
        }
        Verb::Tap {
            what: SetExpr::One { .. },
        } => {
            out.insert(Capability::TapPermanentV1);
        }
        Verb::Untap {
            what: SetExpr::One { .. },
        } => {
            out.insert(Capability::UntapPermanentV1);
        }
        Verb::Untap {
            what: SetExpr::Matching { .. },
        } => {
            out.insert(Capability::UntapControllerLandsV1);
        }
        // CR 111.10: predefined artifact tokens; CR 111.3-111.4: creature
        // tokens, with the reviewed multi-type and hexproof/shroud surfaces.
        Verb::CreateTokens { token, .. } => match token {
            TokenSpec::Predefined { .. } => {
                out.insert(Capability::PredefinedArtifactTokensV1);
            }
            TokenSpec::Creature { spec } => {
                out.insert(Capability::CreatureTokensV1);
                if !spec.additional_types.is_empty() {
                    out.insert(Capability::MultiTypeCreatureTokensV1);
                }
                if spec
                    .keywords
                    .iter()
                    .any(|k| matches!(k, KeywordAbility::Hexproof | KeywordAbility::Shroud))
                {
                    out.insert(Capability::AttachmentsV1);
                }
            }
        },
        // CR 122.1.
        Verb::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            ..
        } => {
            out.insert(Capability::PlusOneCountersV1);
        }
        // CR 613.4c: a targeted pump names the temporary P/T surface and a
        // board-wide pump the reviewed wipe surface. The source pump is named
        // by its complete ability (`Definition::requirements`).
        Verb::ModifyUntil {
            what,
            change: Change::PowerToughness { .. },
            ..
        } => match what {
            SetExpr::One {
                object: Ref::Target { .. },
            } => {
                out.insert(Capability::TemporaryPowerToughnessV1);
            }
            SetExpr::Matching { .. } => {
                out.insert(Capability::BoardWipesV1);
            }
            _ => {}
        },
        // CR 613.1f: the source's own keyword grant.
        Verb::ModifyUntil {
            what: SetExpr::One {
                object: Ref::Source,
            },
            change: Change::AddKeywords { .. },
            ..
        } => {
            out.insert(Capability::ActivatedSourceKeywordGrantV1);
        }
        // CR 701.17a, 107.3.
        Verb::Mill { amount, .. } => {
            out.insert(if amount.is_announced_x() {
                Capability::AnnouncedXEffectsV1
            } else {
                Capability::FixedMillProgramV1
            });
        }
        // CR 701.9: each recipient names its reviewed surface. The
        // controller's discard is named by its complete program
        // (`Definition::requirements`).
        Verb::Discard { player, random, .. } => match player {
            PlayerExpr::EachPlayer | PlayerExpr::EachOpponent => {
                out.insert(Capability::GroupHandDiscardV1);
            }
            _ if *random => {
                out.insert(Capability::RandomPlayerDiscardV1);
            }
            PlayerExpr::Is {
                who: Ref::Target { .. },
            } => {
                out.insert(Capability::TargetPlayerDiscardV1);
            }
            PlayerExpr::Is {
                who: Ref::EventPlayer { .. },
            } => {
                out.insert(Capability::EventPlayerDiscardV1);
            }
            _ => {}
        },
        // CR 701.21a: "each player" shares the reviewed wipe surface and
        // "each opponent" the reviewed group sacrifice surface.
        Verb::Sacrifice { player, .. } => {
            out.insert(match player {
                PlayerExpr::EachPlayer => Capability::BoardWipesV1,
                PlayerExpr::EachOpponent => Capability::SacrificeV1,
                PlayerExpr::Is {
                    who: Ref::Target { .. },
                } => Capability::TargetPlayerSacrificeCreatureV1,
                _ => Capability::EventPlayerSacrificeV1,
            });
        }
        _ => {}
    }
}

/// CR 121.2: a draw of exactly one card by one player is a single draw.
/// Every other draw is performed as a counted sequence of single draws.
pub fn single_draw(player: &PlayerExpr, amount: &Amount) -> bool {
    matches!(player, PlayerExpr::Is { .. }) && amount.as_fixed() == Some(&Quantity::from(1))
}

/// The capabilities a node derives, independent of where it resolves.
pub fn node_capabilities(node: &Node) -> BTreeSet<Capability> {
    let mut out = BTreeSet::new();
    if drain(node).is_some() {
        out.extend(DRAIN_CAPABILITIES);
    } else {
        visit_verbs(node, &mut |verb| verb_capabilities(verb, &mut out));
        if node.uses_continuation() {
            out.insert(Capability::KernelControlV1);
        }
    }
    out
}

/// The drain pair names its own reviewed surface instead of the group life
/// executor's (CR 119.3).
const DRAIN_CAPABILITIES: [Capability; 3] = [
    Capability::OpponentLifeDrainV1,
    Capability::LifeLossV1,
    Capability::LifeGainV1,
];

/// CR 119.3, 608.2c: "each opponent loses N life. You gain life equal to the
/// life lost this way." Returns N when `node` is exactly that pair: one
/// simultaneous loss, then one gain of the total actually lost.
pub fn drain(node: &Node) -> Option<&Amount> {
    let Node::Seq(steps) = node else {
        return None;
    };
    match steps.as_slice() {
        [
            Node::Do(Verb::LoseLife {
                player: PlayerExpr::EachOpponent,
                amount,
            }),
            Node::Do(Verb::GainLife {
                player,
                amount:
                    Amount::Var {
                        of:
                            Ref::StepResult {
                                step: 0,
                                role: StepRole::Amount,
                            },
                    },
            }),
        ] if player.is_you() => Some(amount),
        _ => None,
    }
}

/// Calls `visit` for every verb of a node, in written order.
pub fn visit_verbs<'a>(node: &'a Node, visit: &mut impl FnMut(&'a Verb)) {
    match node {
        Node::Do(verb) => {
            visit(verb);
            if let Verb::CreateDelayedTrigger { node, .. } = verb {
                visit_verbs(node, visit);
            }
        }
        Node::Seq(nodes) => nodes.iter().for_each(|node| visit_verbs(node, visit)),
        Node::If {
            then, otherwise, ..
        } => {
            visit_verbs(then, visit);
            if let Some(otherwise) = otherwise {
                visit_verbs(otherwise, visit);
            }
        }
        Node::Optional { node, .. }
        | Node::UnlessPays { node, .. }
        | Node::Delayed { node, .. } => visit_verbs(node, visit),
        Node::ForEach { body, .. } | Node::Choose { body, .. } => visit_verbs(body, visit),
        Node::Reflexive { first, then } => {
            visit_verbs(first, visit);
            visit_verbs(then, visit);
        }
    }
}

/// The event-player roles an event supplies (CR 603.2).
fn event_player_roles(event: &TriggerCondition) -> &'static [EventPlayerRole] {
    match event {
        // CR 601.2: the caster of the triggering spell.
        TriggerCondition::SpellCast { .. } => &[EventPlayerRole::Caster],
        // CR 500: the player whose step began.
        TriggerCondition::StepBegan { .. } => &[EventPlayerRole::ActivePlayer],
        // CR 605.1a, 106.12: the player who activated the mana ability.
        TriggerCondition::ManaTapped { .. } => &[EventPlayerRole::ActivatingPlayer],
        _ => &[],
    }
}

/// True when `event` supplies the event player in `role` (CR 603.2). A
/// damage event's damaged player is bound only by the actions that read it,
/// so it is not the role lowerers of "that player" choose.
fn event_supplies(event: &TriggerCondition, role: EventPlayerRole) -> bool {
    event_player_roles(event).contains(&role)
        || (role == EventPlayerRole::DamagedPlayer
            && matches!(
                event,
                TriggerCondition::DamageDealt { .. } | TriggerCondition::EquippedCombatDamage
            ))
}

/// The event-player role an event supplies, for lowerers of "that player".
pub fn event_player_role(event: &TriggerCondition) -> Option<EventPlayerRole> {
    event_player_roles(event).first().copied()
}

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

/// Pattern for an instruction that is one kernel verb. `kernel!(Draw { .. })`
/// ignores the source block; `kernel!(Draw { .. } @ 0)` matches it too.
#[macro_export]
macro_rules! kernel {
    ($verb:ident { $($fields:tt)* }) => {
        $crate::Instruction::Kernel {
            node: $crate::kir::Node::Do($crate::kir::Verb::$verb { $($fields)* }),
            ..
        }
    };
    ($verb:ident { $($fields:tt)* } @ $block:pat) => {
        $crate::Instruction::Kernel {
            node: $crate::kir::Node::Do($crate::kir::Verb::$verb { $($fields)* }),
            source_block: $block,
        }
    };
}

/// Pattern for "you" in any controller role.
#[macro_export]
macro_rules! you {
    () => {
        $crate::kir::PlayerExpr::Is {
            who: $crate::kir::Ref::Controller { .. },
        }
    };
}

/// Pattern for the player announced as target `slot`.
#[macro_export]
macro_rules! target_player {
    ($slot:pat) => {
        $crate::kir::PlayerExpr::Is {
            who: $crate::kir::Ref::Target { slot: $slot },
        }
    };
}

/// Pattern for the trigger's event player in any role.
#[macro_export]
macro_rules! event_player {
    () => {
        $crate::kir::PlayerExpr::Is {
            who: $crate::kir::Ref::EventPlayer { .. },
        }
    };
}

/// Pattern for the single object announced as target `slot`.
#[macro_export]
macro_rules! target_object {
    ($slot:pat) => {
        $crate::kir::SetExpr::One {
            object: $crate::kir::Ref::Target { slot: $slot },
        }
    };
}

/// Pattern for the exact source object.
#[macro_export]
macro_rules! source_object {
    () => {
        $crate::kir::SetExpr::One {
            object: $crate::kir::Ref::Source,
        }
    };
}

/// Pattern for the announced X (CR 107.3).
#[macro_export]
macro_rules! announced_x {
    () => {
        $crate::kir::Amount::Var {
            of: $crate::kir::Ref::AnnouncedX,
        }
    };
}

/// Pattern for a fixed amount.
#[macro_export]
macro_rules! fixed {
    ($value:pat) => {
        $crate::kir::Amount::Fixed { value: $value }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginningStep, CasterRelation, TurnRelation, TypePredicate};

    fn spell<'a>(targets: &'a [TargetSpec]) -> Env<'a> {
        Env {
            context: Context::Spell,
            targets,
            event: None,
            announced_x: false,
        }
    }
    fn gain(player: PlayerExpr, amount: Amount) -> Node {
        Node::Do(Verb::GainLife { player, amount })
    }
    fn player_target() -> Vec<TargetSpec> {
        vec![TargetSpec::new(0, "line:0:1", TargetDomain::AnyPlayer)]
    }
    fn creature_target() -> Vec<TargetSpec> {
        vec![TargetSpec::new(
            0,
            "line:0:1",
            TargetDomain::BattlefieldCreature,
        )]
    }

    #[test]
    fn controller_role_must_be_the_one_the_context_defines() {
        // CR 109.5: "you" in a spell is its controller, never an ability's.
        let you = |context| gain(PlayerExpr::you(context), Amount::fixed(3));
        assert!(check(&you(Context::Spell), &spell(&[])).is_ok());
        for wrong in [Context::Activated, Context::Triggered] {
            assert!(matches!(
                check(&you(wrong), &spell(&[])),
                Err(KirError::Scope(_))
            ));
        }
        let mut ability = spell(&[]);
        ability.context = Context::Activated;
        assert!(check(&you(Context::Activated), &ability).is_ok());
        assert!(matches!(
            check(&you(Context::Spell), &ability),
            Err(KirError::Scope(_))
        ));
        for role in [
            ControllerRole::StaticController,
            ControllerRole::EnteringController,
            ControllerRole::ProspectiveCaster,
            ControllerRole::SpecialActionPlayer,
            ControllerRole::CardOwner,
            ControllerRole::AbilityHolder,
        ] {
            let node = gain(
                PlayerExpr::Is {
                    who: Ref::Controller { role },
                },
                Amount::fixed(1),
            );
            assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
        }
    }

    #[test]
    fn target_references_need_an_announced_slot_of_the_right_kind() {
        let node = gain(PlayerExpr::target(0), Amount::fixed(2));
        // CR 601.2c: no announced occurrence.
        assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
        let requirements = check(&node, &spell(&player_target())).unwrap();
        assert_eq!(requirements.target_slots, BTreeSet::from([0]));
        // A creature target is not a player.
        assert!(matches!(
            check(&node, &spell(&creature_target())),
            Err(KirError::Type(_))
        ));
        assert_eq!(target_slots(&node), BTreeSet::from([0]));
    }

    #[test]
    fn event_players_exist_only_for_events_that_supply_the_role() {
        let caster = gain(PlayerExpr::event(EventPlayerRole::Caster), Amount::fixed(1));
        let cast = TriggerCondition::SpellCast {
            caster: CasterRelation::AnyPlayer,
            spell: TypePredicate::Any {},
        };
        let step = TriggerCondition::StepBegan {
            step: BeginningStep::Upkeep,
            turn_relation: TurnRelation::AnyPlayer,
        };
        let triggered = |event| Env {
            context: Context::Triggered,
            targets: &[],
            event,
            announced_x: false,
        };
        let requirements = check(&caster, &triggered(Some(&cast))).unwrap();
        assert!(requirements.event_player);
        assert!(
            requirements
                .capabilities
                .contains(&Capability::EventPlayerRecipientV1)
        );
        // CR 603.2: a beginning-of-step event has an active player, no caster.
        assert!(matches!(
            check(&caster, &triggered(Some(&step))),
            Err(KirError::Scope(_))
        ));
        let active = gain(
            PlayerExpr::event(EventPlayerRole::ActivePlayer),
            Amount::fixed(1),
        );
        assert!(check(&active, &triggered(Some(&step))).is_ok());
        assert!(matches!(
            check(&caster, &triggered(Some(&TriggerCondition::SelfDies))),
            Err(KirError::Scope(_))
        ));
        assert!(matches!(
            check(&caster, &triggered(None)),
            Err(KirError::Scope(_))
        ));
        assert!(matches!(
            check(&caster, &spell(&[])),
            Err(KirError::Scope(_))
        ));
    }

    #[test]
    fn announced_x_is_in_scope_only_when_the_object_has_x() {
        let node = gain(
            PlayerExpr::you(Context::Spell),
            Amount::Var {
                of: Ref::AnnouncedX,
            },
        );
        assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
        let mut with_x = spell(&[]);
        with_x.announced_x = true;
        let requirements = check(&node, &with_x).unwrap();
        assert!(requirements.announced_x);
        // CR 107.3: X is the amount for the controller or one announced
        // player; a group or event player has no audited X cell.
        let targets = player_target();
        let mut targeted = spell(&targets);
        targeted.announced_x = true;
        let x = |player| {
            Node::Do(Verb::LoseLife {
                player,
                amount: Amount::announced_x(),
            })
        };
        check(&x(PlayerExpr::target(0)), &targeted).unwrap();
        assert!(matches!(
            check(&x(PlayerExpr::EachOpponent), &targeted),
            Err(KirError::Unaudited(_))
        ));
        // Any other computed amount stays unaudited.
        let counted = gain(
            PlayerExpr::you(Context::Spell),
            Amount::Count {
                set: SetExpr::battlefield(Pred::default()),
            },
        );
        assert!(matches!(
            check(&counted, &with_x),
            Err(KirError::Unaudited(_))
        ));
    }

    #[test]
    fn the_drain_pair_uses_the_top_level_executor() {
        let pair = |amount| Node::drain(Context::Spell, amount);
        let requirements = check(&pair(Amount::fixed(2)), &spell(&[])).unwrap();
        assert_eq!(
            requirements.capabilities,
            BTreeSet::from([
                Capability::OpponentLifeDrainV1,
                Capability::LifeLossV1,
                Capability::LifeGainV1
            ])
        );
        assert_eq!(
            node_capabilities(&pair(Amount::fixed(2))),
            requirements.capabilities
        );
        assert_eq!(drain(&pair(Amount::fixed(2))), Some(&Amount::fixed(2)));
        // CR 119.3: a fixed drain is positive; X needs an announced X.
        assert!(matches!(
            check(&pair(Amount::fixed(0)), &spell(&[])),
            Err(KirError::Type(_))
        ));
        assert!(matches!(
            check(&pair(Amount::announced_x()), &spell(&[])),
            Err(KirError::Scope(_))
        ));
        let mut with_x = spell(&[]);
        with_x.announced_x = true;
        assert!(
            check(&pair(Amount::announced_x()), &with_x)
                .unwrap()
                .announced_x
        );
        // "You" must be the role of the resolving context (CR 109.5).
        assert!(matches!(
            check(
                &Node::drain(Context::Triggered, Amount::fixed(1)),
                &spell(&[])
            ),
            Err(KirError::Scope(_))
        ));
        let Node::Seq(steps) = pair(Amount::fixed(2)) else {
            unreachable!()
        };
        for ordinary in [
            Node::Seq(vec![steps[0].clone()]),
            Node::Seq(vec![steps[0].clone(), steps[0].clone()]),
        ] {
            assert_eq!(drain(&ordinary), None);
            let required = check(&ordinary, &spell(&[])).unwrap();
            assert_eq!(required.capabilities, node_capabilities(&ordinary));
            assert!(
                !required
                    .capabilities
                    .contains(&Capability::OpponentLifeDrainV1)
            );
        }
        for unaudited in [
            Node::Seq(vec![steps[0].clone(), steps[1].clone(), steps[0].clone()]),
            Node::Seq(vec![pair(Amount::fixed(2))]),
        ] {
            assert!(matches!(
                check(&unaudited, &spell(&[])),
                Err(KirError::Unaudited(_))
            ));
        }
    }

    #[test]
    fn bound_variables_and_step_results_follow_lexical_scope() {
        let you = PlayerExpr::you(Context::Spell);
        let each = |body| Node::ForEach {
            var: 7,
            over: Domain::Players {
                players: PlayerExpr::EachOpponent,
            },
            body: Box::new(body),
        };
        let bound = gain(
            PlayerExpr::Is {
                who: Ref::BoundVar { id: 7 },
            },
            Amount::fixed(1),
        );
        // Unbound outside the loop, bound inside it.
        assert!(matches!(
            check(&bound, &spell(&[])),
            Err(KirError::Scope(_))
        ));
        assert!(check(&each(bound), &spell(&[])).is_ok());
        let bound_draw = Node::Do(Verb::Draw {
            player: PlayerExpr::Is {
                who: Ref::BoundVar { id: 7 },
            },
            amount: Amount::fixed(1),
        });
        assert!(matches!(
            check(&each(bound_draw), &spell(&[])),
            Err(KirError::Unaudited(_))
        ));
        let other = gain(
            PlayerExpr::Is {
                who: Ref::BoundVar { id: 8 },
            },
            Amount::fixed(1),
        );
        assert!(matches!(
            check(&each(other), &spell(&[])),
            Err(KirError::Scope(_))
        ));
        // A step may read only steps performed before it (CR 608.2c).
        let result = |step| {
            gain(
                you.clone(),
                Amount::Var {
                    of: Ref::StepResult {
                        step,
                        role: StepRole::Amount,
                    },
                },
            )
        };
        let first = gain(you.clone(), Amount::fixed(1));
        assert!(matches!(
            check(&Node::Seq(vec![first.clone(), result(0)]), &spell(&[])),
            Err(KirError::Unaudited(_))
        ));
        let step_draw = Node::Do(Verb::Draw {
            player: you.clone(),
            amount: Amount::Var {
                of: Ref::StepResult {
                    step: 0,
                    role: StepRole::Amount,
                },
            },
        });
        assert!(matches!(
            check(&Node::Seq(vec![first.clone(), step_draw]), &spell(&[])),
            Err(KirError::Unaudited(_))
        ));
        for forward in [
            Node::Seq(vec![result(0), first.clone()]),
            Node::Seq(vec![first.clone(), result(1)]),
            result(0),
        ] {
            assert!(matches!(
                check(&forward, &spell(&[])),
                Err(KirError::Scope(_))
            ));
        }
    }

    #[test]
    fn control_checks_branches_bindings_payments_and_capabilities() {
        let you = PlayerExpr::you(Context::Spell);
        let set = SetExpr::Matching {
            zone: ZoneExpr::Graveyards,
            filter: Pred::default(),
        };
        let size = Amount::Count { set: set.clone() };
        let body = Node::ForEach {
            var: 2,
            over: Domain::Objects {
                set: SetExpr::Pool {
                    pool: Ref::BoundVar { id: 1 },
                },
            },
            body: Box::new(gain(
                PlayerExpr::OwnerOf {
                    object: Ref::BoundVar { id: 2 },
                },
                Amount::Count {
                    set: SetExpr::One {
                        object: Ref::BoundVar { id: 2 },
                    },
                },
            )),
        };
        let choose = Node::Choose {
            var: 1,
            chooser: you.clone(),
            from: set.clone(),
            minimum: size.clone(),
            maximum: size,
            body: Box::new(body),
        };
        let pay = |payment| Node::UnlessPays {
            payer: you.clone(),
            payment,
            node: Box::new(Node::Do(Verb::LoseLife {
                player: you.clone(),
                amount: Amount::fixed(1),
            })),
        };
        let control = Node::If {
            condition: Cond::All {
                conditions: vec![
                    Cond::Exists { set },
                    Cond::Not {
                        condition: Box::new(Cond::CostPaid {
                            identity: "kicker".into(),
                        }),
                    },
                    Cond::Compare {
                        left: Amount::announced_x(),
                        cmp: Cmp::AtLeast,
                        right: Amount::fixed(1),
                    },
                ],
            },
            then: Box::new(Node::Optional {
                chooser: you.clone(),
                node: Box::new(choose),
            }),
            otherwise: Some(Box::new(Node::Seq(vec![
                pay(Payment::Life {
                    amount: Amount::fixed(1),
                }),
                pay(Payment::Generic {
                    amount: Amount::announced_x(),
                }),
            ]))),
        };
        let env = Env {
            announced_x: true,
            ..spell(&[])
        };
        let requirements = check(&control, &env).unwrap();
        assert!(requirements.announced_x);
        assert_eq!(
            requirements.capabilities,
            BTreeSet::from([
                Capability::KernelControlV1,
                Capability::LifeGainV1,
                Capability::LifeLossV1,
            ])
        );
        assert_eq!(requirements.capabilities, node_capabilities(&control));
    }

    #[test]
    fn control_rejects_unsupported_operands_and_children() {
        let you = PlayerExpr::you(Context::Spell);
        let child = || Box::new(gain(you.clone(), Amount::fixed(1)));
        let choice = |minimum, maximum| Node::Choose {
            var: 0,
            chooser: you.clone(),
            from: SetExpr::battlefield(Pred::creatures()),
            minimum: Amount::fixed(minimum),
            maximum: Amount::fixed(maximum),
            body: child(),
        };
        assert!(check(&choice(0, 0), &spell(&[])).is_ok());
        assert!(matches!(
            check(&choice(2, 1), &spell(&[])),
            Err(KirError::Type(_))
        ));
        assert!(matches!(
            check(
                &Node::UnlessPays {
                    payer: you.clone(),
                    payment: Payment::Life {
                        amount: Amount::fixed(-1)
                    },
                    node: child(),
                },
                &spell(&[])
            ),
            Err(KirError::Type(_))
        ));
        for node in [
            Node::Delayed {
                event: DelayedEvent::NextEndStep,
                node: child(),
            },
            Node::Reflexive {
                first: child(),
                then: child(),
            },
            Node::ForEach {
                var: 0,
                over: Domain::Objects {
                    set: SetExpr::Matching {
                        zone: ZoneExpr::Exile,
                        filter: Pred::default(),
                    },
                },
                body: child(),
            },
            Node::Optional {
                chooser: PlayerExpr::EachOpponent,
                node: child(),
            },
            Node::If {
                condition: Cond::Exists {
                    set: SetExpr::source(),
                },
                then: child(),
                otherwise: Some(Box::new(Node::Do(Verb::Shuffle {
                    player: you.clone(),
                }))),
            },
            Node::Seq(vec![gain(
                you.clone(),
                Amount::LifeOf {
                    player: you.clone(),
                },
            )]),
            Node::Seq(vec![gain(
                PlayerExpr::ControllerOf {
                    object: Ref::Source,
                },
                Amount::fixed(1),
            )]),
            Node::Seq(vec![Node::Do(Verb::ModifyUntil {
                what: SetExpr::source(),
                layer: Layer::PowerToughnessModify,
                change: Change::PowerToughness {
                    power: Amount::fixed(1),
                    toughness: Amount::fixed(1),
                },
                until: Until::EndOfTurn,
            })]),
            choice(1, 1),
        ] {
            assert!(
                matches!(check(&node, &spell(&[])), Err(KirError::Unaudited(_))),
                "{node:?}"
            );
        }
        let optional_destroy = Node::Optional {
            chooser: you,
            node: Box::new(Node::Do(Verb::Destroy {
                what: SetExpr::target(0),
                regeneration_forbidden: false,
            })),
        };
        let targets = [TargetSpec::new(
            0,
            "line:0:0",
            TargetDomain::BattlefieldPermanent,
        )];
        let env = spell(&targets);
        assert!(
            !check(&optional_destroy, &env)
                .unwrap()
                .capabilities
                .contains(&Capability::KernelControlV1)
        );
        assert!(matches!(
            check(&Node::Seq(vec![optional_destroy]), &env),
            Err(KirError::Unaudited(_))
        ));
    }

    #[test]
    fn permanent_source_modifications_require_an_executable_context() {
        let pump = Node::Do(Verb::ModifyUntil {
            what: SetExpr::source(),
            layer: Layer::PowerToughnessModify,
            change: Change::PowerToughness {
                power: Amount::fixed(1),
                toughness: Amount::fixed(1),
            },
            until: Until::EndOfTurn,
        });
        let grant = |keyword| {
            Node::Do(Verb::ModifyUntil {
                what: SetExpr::source(),
                layer: Layer::Ability,
                change: Change::AddKeywords {
                    keywords: vec![keyword],
                },
                until: Until::EndOfTurn,
            })
        };
        let activated = Env {
            context: Context::Activated,
            ..spell(&[])
        };
        let triggered = Env {
            context: Context::Triggered,
            ..spell(&[])
        };
        assert!(check(&pump, &spell(&[])).is_err());
        assert!(check(&pump, &activated).is_ok());
        assert!(check(&grant(KeywordAbility::Flying), &activated).is_ok());
        assert!(check(&grant(KeywordAbility::Flying), &triggered).is_err());
        assert!(check(&grant(KeywordAbility::Flying), &spell(&[])).is_err());
        for keyword in [
            KeywordAbility::Menace,
            KeywordAbility::Shroud,
            KeywordAbility::Defender,
            KeywordAbility::ProtectionFromEachColor,
        ] {
            assert!(check(&grant(keyword), &activated).is_err());
        }
    }

    #[test]
    fn control_rejects_counted_draw_leaves() {
        let targets = [TargetSpec::new(0, "line:0:0", TargetDomain::AnyPlayer)];
        let env = Env {
            announced_x: true,
            ..spell(&targets)
        };
        let you = PlayerExpr::you(Context::Spell);
        let target = PlayerExpr::Is {
            who: Ref::Target { slot: 0 },
        };
        for (player, amount) in [
            (you.clone(), Amount::fixed(0)),
            (you.clone(), Amount::fixed(2)),
            (you.clone(), Amount::announced_x()),
            (PlayerExpr::EachPlayer, Amount::fixed(1)),
        ] {
            let leaf = Node::Do(Verb::Draw { player, amount });
            assert!(check(&leaf, &env).is_ok());
            assert!(matches!(
                check(&Node::Seq(vec![leaf]), &env),
                Err(KirError::Unaudited(reason)) if reason.contains("nested counted draw")
            ));
        }
        for amount in [Amount::fixed(0), Amount::fixed(2), Amount::announced_x()] {
            let leaf = Node::Do(Verb::Draw {
                player: target.clone(),
                amount,
            });
            assert!(matches!(check(&leaf, &env), Err(KirError::Unaudited(_))));
        }
        for player in [you, target] {
            let leaf = Node::Do(Verb::Draw {
                player,
                amount: Amount::fixed(1),
            });
            assert!(check(&Node::Seq(vec![leaf]), &env).is_ok());
        }
    }

    #[test]
    fn graveyard_control_sets_reject_battlefield_predicates() {
        let you = PlayerExpr::you(Context::Spell);
        let child = || Box::new(gain(you.clone(), Amount::fixed(1)));
        for atom in [
            Atom::Tapped,
            Atom::Untapped,
            Atom::Attacking,
            Atom::Blocking,
            Atom::ControlledBy {
                player: you.clone(),
            },
        ] {
            let set = SetExpr::Matching {
                zone: ZoneExpr::Graveyards,
                filter: Pred(vec![Atom::AnyOf {
                    alternatives: vec![Pred::creatures(), Pred(vec![atom.clone()])],
                }]),
            };
            let amount = Amount::Count { set: set.clone() };
            for node in [
                Node::If {
                    condition: Cond::Exists { set: set.clone() },
                    then: child(),
                    otherwise: None,
                },
                Node::ForEach {
                    var: 0,
                    over: Domain::Objects { set: set.clone() },
                    body: child(),
                },
                Node::Seq(vec![gain(you.clone(), amount.clone())]),
                Node::Choose {
                    var: 0,
                    chooser: you.clone(),
                    from: set,
                    minimum: amount.clone(),
                    maximum: amount,
                    body: child(),
                },
            ] {
                assert!(
                    matches!(check(&node, &spell(&[])), Err(KirError::Unaudited(reason))
                        if reason == "battlefield predicate outside the battlefield"),
                    "{atom:?} in {node:?}"
                );
            }
        }
    }

    #[test]
    fn a_filtered_count_of_a_hidden_zone_is_rejected() {
        let you = PlayerExpr::you(Context::Spell);
        let count = |zone, filter| {
            gain(
                you.clone(),
                Amount::Count {
                    set: SetExpr::Matching { zone, filter },
                },
            )
        };
        let creatures = || {
            Pred(vec![Atom::Type {
                kind: CardKind::Creature,
            }])
        };
        // CR 400.2: which cards a hand or library holds is hidden.
        for hidden in [
            ZoneExpr::Hand { of: you.clone() },
            ZoneExpr::Library { of: you.clone() },
        ] {
            assert_eq!(
                check(&count(hidden.clone(), creatures()), &spell(&[])),
                Err(KirError::HiddenCount)
            );
            // CR 402.3, 401.2: its size is public.
            assert!(matches!(
                check(&count(hidden, Pred::default()), &spell(&[])),
                Err(KirError::Unaudited(_))
            ));
        }
        for public in [
            ZoneExpr::Battlefield,
            ZoneExpr::Graveyard { of: you.clone() },
            ZoneExpr::Graveyards,
            ZoneExpr::Exile,
        ] {
            assert!(matches!(
                check(&count(public, creatures()), &spell(&[])),
                Err(KirError::Unaudited(_))
            ));
        }
    }

    #[test]
    fn a_continuous_change_carries_its_own_layer() {
        let pump = |layer| {
            Node::Do(Verb::ModifyUntil {
                what: SetExpr::target(0),
                layer,
                change: Change::PowerToughness {
                    power: Amount::fixed(2),
                    toughness: Amount::fixed(2),
                },
                until: Until::EndOfTurn,
            })
        };
        // CR 613.4c: a modification is not a layer-6 ability change.
        assert!(matches!(
            check(&pump(Layer::Ability), &spell(&creature_target())),
            Err(KirError::Type(_))
        ));
        assert!(!matches!(
            check(
                &pump(Layer::PowerToughnessModify),
                &spell(&creature_target())
            ),
            Err(KirError::Type(_) | KirError::Scope(_))
        ));
        for (change, layer) in [
            (
                Change::AddKeywords {
                    keywords: vec![KeywordAbility::Flying],
                },
                Layer::Ability,
            ),
            (Change::SetColors { colors: vec![] }, Layer::Color),
            (
                Change::AddTypes {
                    kinds: vec![CardKind::Artifact],
                },
                Layer::Type,
            ),
            (
                Change::Controller {
                    player: PlayerExpr::EachOpponent,
                },
                Layer::Control,
            ),
        ] {
            assert_eq!(change.layer(), layer);
        }
    }

    #[test]
    fn life_cells_admit_exactly_the_audited_recipients_and_amounts() {
        let targets = player_target();
        let env = spell(&targets);
        for player in [
            PlayerExpr::you(Context::Spell),
            PlayerExpr::EachOpponent,
            PlayerExpr::target(0),
        ] {
            for node in [
                gain(player.clone(), Amount::fixed(0)),
                Node::Do(Verb::LoseLife {
                    player: player.clone(),
                    amount: Amount::fixed(4),
                }),
            ] {
                check(&node, &env).unwrap();
            }
            assert!(matches!(
                check(&gain(player, Amount::fixed(-1)), &env),
                Err(KirError::Type(_))
            ));
        }
        // CR 119.3: an opponent-only target domain has no audited life cell.
        let opponent = [TargetSpec::new(0, "line:0:1", TargetDomain::OpponentPlayer)];
        assert!(matches!(
            check(
                &gain(PlayerExpr::target(0), Amount::fixed(1)),
                &spell(&opponent)
            ),
            Err(KirError::Unaudited(_))
        ));
        assert!(matches!(
            check(&gain(PlayerExpr::EachPlayer, Amount::fixed(1)), &env),
            Err(KirError::Unaudited(_))
        ));
    }

    #[test]
    fn board_wide_cells_audit_the_zone_destination_and_every_filter_atom() {
        let creatures = Pred(vec![Atom::Type {
            kind: CardKind::Creature,
        }]);
        let destroy = |filter: Pred, regeneration_forbidden| {
            Node::Do(Verb::Destroy {
                what: SetExpr::battlefield(filter),
                regeneration_forbidden,
            })
        };
        let moved = |what, to| Node::Do(Verb::Move { what, to });
        let env = spell(&[]);
        // CR 701.8: destroying every creature names the creature
        // destruction surface; every other board-wide set, the wipe surface.
        let requirements = check(&destroy(creatures.clone(), false), &env).unwrap();
        assert_eq!(
            requirements.capabilities,
            BTreeSet::from([Capability::DestroyCreatureV1])
        );
        let artifacts = Pred(vec![Atom::Type {
            kind: CardKind::Artifact,
        }]);
        assert_eq!(
            check(&destroy(artifacts, false), &env)
                .unwrap()
                .capabilities,
            BTreeSet::from([Capability::BoardWipesV1])
        );
        for to in [Destination::Exile, Destination::OwnerHand] {
            check(&moved(SetExpr::battlefield(creatures.clone()), to), &env).unwrap();
        }
        check(&moved(SetExpr::graveyards(), Destination::Exile), &env).unwrap();
        // CR 701.19c: the prohibition is audited for the nonland wave only.
        check(&destroy(Pred::nonland_permanents(), true), &env).unwrap();
        for node in [
            destroy(creatures.clone(), true),
            moved(
                SetExpr::battlefield(creatures.clone()),
                Destination::OwnerLibraryTop,
            ),
            moved(SetExpr::graveyards(), Destination::OwnerHand),
            moved(
                SetExpr::Matching {
                    zone: ZoneExpr::Graveyards,
                    filter: creatures.clone(),
                },
                Destination::Exile,
            ),
            moved(
                SetExpr::Matching {
                    zone: ZoneExpr::Exile,
                    filter: Pred::default(),
                },
                Destination::OwnerHand,
            ),
            // Atoms the board evaluator does not read.
            destroy(Pred(vec![Atom::Blocking]), false),
            destroy(
                Pred(vec![Atom::IsNot {
                    object: Ref::Source,
                }]),
                false,
            ),
            destroy(
                Pred(vec![Atom::ControlledBy {
                    player: PlayerExpr::EachPlayer,
                }]),
                false,
            ),
            destroy(
                Pred(vec![Atom::AnyOf {
                    alternatives: vec![Pred(vec![Atom::Blocking])],
                }]),
                false,
            ),
        ] {
            assert!(
                matches!(check(&node, &env), Err(KirError::Unaudited(_))),
                "{node:?}"
            );
        }
        // A computed comparison is in scope only with X, and unaudited then.
        let by_x = destroy(
            Pred(vec![Atom::ManaValue {
                cmp: Cmp::AtMost,
                value: Amount::Var {
                    of: Ref::AnnouncedX,
                },
            }]),
            false,
        );
        assert!(matches!(check(&by_x, &env), Err(KirError::Scope(_))));
        let mut with_x = spell(&[]);
        with_x.announced_x = true;
        assert!(matches!(check(&by_x, &with_x), Err(KirError::Unaudited(_))));
        // "You control" in a filter obeys the same controller-role rule.
        let yours = |context| {
            destroy(
                Pred(vec![Atom::ControlledBy {
                    player: PlayerExpr::you(context),
                }]),
                false,
            )
        };
        check(&yours(Context::Spell), &env).unwrap();
        assert!(matches!(
            check(&yours(Context::Triggered), &env),
            Err(KirError::Scope(_))
        ));
        // Creature pump sets admit creature predicates regardless of conjunct order.
        let reordered_pump = Node::Do(Verb::ModifyUntil {
            what: SetExpr::battlefield(Pred(vec![
                Atom::Color { color: Color::B },
                Atom::Type {
                    kind: CardKind::Creature,
                },
            ])),
            layer: Layer::PowerToughnessModify,
            change: Change::PowerToughness {
                power: Amount::fixed(1),
                toughness: Amount::fixed(1),
            },
            until: Until::EndOfTurn,
        });
        check(&reordered_pump, &env).unwrap();
        assert!(
            Pred(vec![Atom::ControlledBy {
                player: PlayerExpr::you(Context::Spell)
            }])
            .names_you()
        );
        assert!(!creatures.names_you());
    }

    #[test]
    fn capabilities_include_the_continuation_and_its_leaves() {
        let caps = |node: &Node| node_capabilities(node);
        assert_eq!(
            caps(&gain(PlayerExpr::you(Context::Spell), Amount::fixed(1))),
            BTreeSet::from([Capability::LifeGainV1])
        );
        assert_eq!(
            caps(&Node::Do(Verb::LoseLife {
                player: PlayerExpr::EachOpponent,
                amount: Amount::fixed(1)
            })),
            BTreeSet::from([Capability::LifeLossV1, Capability::SacrificeV1])
        );
        // The walker reaches verbs under every control node.
        let nested = Node::Optional {
            chooser: PlayerExpr::you(Context::Spell),
            node: Box::new(Node::Seq(vec![
                gain(PlayerExpr::target(0), Amount::fixed(1)),
                Node::If {
                    condition: Cond::CostPaid {
                        identity: "kicker".into(),
                    },
                    then: Box::new(Node::Do(Verb::LoseLife {
                        player: PlayerExpr::target(0),
                        amount: Amount::fixed(1),
                    })),
                    otherwise: None,
                },
            ])),
        };
        assert_eq!(
            caps(&nested),
            BTreeSet::from([
                Capability::KernelControlV1,
                Capability::LifeGainV1,
                Capability::LifeLossV1,
            ])
        );
    }

    #[test]
    fn unit_operands_reject_unknown_fields() {
        // A unit operand's only field is its tag.
        assert!(serde_json::from_value::<Destination>(serde_json::json!({"to":"exile"})).is_ok());
        assert!(
            serde_json::from_value::<Destination>(serde_json::json!({"to":"exile","x":1})).is_err()
        );
        assert!(serde_json::from_value::<Ref>(serde_json::json!({"ref":"source","x":1})).is_err());
        assert!(
            serde_json::from_value::<PlayerExpr>(
                serde_json::json!({"player":"each_opponent","x":1})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<ZoneExpr>(serde_json::json!({"zone":"battlefield","x":1}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<Atom>(serde_json::json!({"atom":"tapped","x":1})).is_err()
        );
        assert!(
            serde_json::from_value::<CounterKind>(
                serde_json::json!({"counter":"plus_one_plus_one","x":1})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<DelayedEvent>(serde_json::json!({"at":"next_end_step","x":1}))
                .is_err()
        );
        assert_eq!(
            serde_json::from_value::<Ref>(serde_json::json!({"ref":"announced_x"})).unwrap(),
            Ref::AnnouncedX
        );
    }

    #[test]
    fn nodes_round_trip_and_reject_unknown_fields() {
        let node = Node::Seq(vec![
            gain(PlayerExpr::target(0), Amount::fixed(3)),
            Node::Do(Verb::Destroy {
                what: SetExpr::battlefield(Pred(vec![
                    Atom::Type {
                        kind: CardKind::Creature,
                    },
                    Atom::Power {
                        cmp: Cmp::AtLeast,
                        value: Amount::fixed(4),
                    },
                    Atom::ControlledBy {
                        player: PlayerExpr::EachOpponent,
                    },
                ])),
                regeneration_forbidden: false,
            }),
        ]);
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(
            json["seq"][0],
            serde_json::json!({"do":{"verb":"gain_life",
                "player":{"player":"is","who":{"ref":"target","slot":0}},
                "amount":{"amount":"fixed","value":"3"}}})
        );
        assert_eq!(serde_json::from_value::<Node>(json.clone()).unwrap(), node);
        let mut unknown = json;
        unknown["seq"][0]["do"]["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<Node>(unknown).is_err());
        assert!(
            serde_json::from_value::<Node>(serde_json::json!({"do":{"verb":"levitate"}})).is_err()
        );
    }
}
