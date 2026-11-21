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
//! The capabilities a node needs are derived by the same walk, so adding a
//! combination of existing operands adds no capability, schema branch or
//! validation arm.
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
    Source,
    Controller {
        role: ControllerRole,
    },
    /// CR 601.2c, 115.1: the announced target occurrence `slot`.
    Target {
        slot: u32,
    },
    /// CR 301.5, 303.4: the permanent the source is attached to.
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
    EachOpponent,
    /// Each player, in APNAP order.
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
    Legendary,
    NotLegendary,
    Basic,
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
    Tapped,
    Untapped,
    /// CR 506.4: an attacking creature.
    Attacking,
    Blocking,
    Token,
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
    Battlefield,
    /// Every graveyard (CR 404).
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
    Exile,
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
    Exile,
    /// CR 400.3: an object goes to its owner's hand, library or graveyard.
    OwnerHand,
    OwnerLibraryTop,
    OwnerLibraryBottom,
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
    PlusOnePlusOne,
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
    NextEndStep,
    NextUpkeep {
        of: PlayerExpr,
    },
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
    /// The verb of a single-action node.
    pub fn verb(&self) -> Option<&Verb> {
        match self {
            Self::Do(verb) => Some(verb),
            _ => None,
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
        bound: vec![],
        steps: 0,
        out: DerivedRequirements::default(),
    };
    walk.node(node)?;
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
        bound: vec![],
        steps: 0,
        out: DerivedRequirements::default(),
    };
    // A scan reports no scope or audit error; a malformed literal can only
    // end it early, and `check` reports that literal.
    let _ = walk.node(node);
    walk.out.target_slots
}

struct Walk<'a, 'e> {
    env: &'a Env<'e>,
    /// Collect references only: no scope, kind or audit judgement.
    scan: bool,
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
        Ok(match reference {
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
                if !event_player_roles(event).contains(role) {
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
        })
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
            Amount::LifeOf { player } => self.player(player),
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

    fn condition(&mut self, condition: &Cond) -> Result<(), KirError> {
        match condition {
            Cond::Compare { left, right, .. } => {
                self.amount(left)?;
                self.amount(right)
            }
            Cond::Exists { set } => self.set(set),
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
                self.expect(source, &[RefKind::Object], "damage source")?;
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
                self.nested(node)
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
    fn nested(&mut self, node: &Node) -> Result<(), KirError> {
        let steps = std::mem::take(&mut self.steps);
        let result = self.node(node);
        self.steps = steps;
        result
    }

    fn node(&mut self, node: &Node) -> Result<(), KirError> {
        match node {
            Node::Do(verb) => {
                self.operands(verb)?;
                if !self.scan {
                    self.audit(verb)?;
                    verb_capabilities(verb, &mut self.out.capabilities);
                }
                Ok(())
            }
            // Control nodes are scope-checked completely, then rejected: the
            // resolving frame carries one cursor per instruction and no
            // nested position, so the interpreter cannot suspend and resume
            // inside a kernel node yet.
            Node::Seq(nodes) => {
                let steps = std::mem::take(&mut self.steps);
                for node in nodes {
                    self.node_unaudited(node)?;
                    self.steps += 1;
                }
                self.steps = steps;
                missing_cell!(self, "sequence node")
            }
            Node::If {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition)?;
                self.nested_unaudited(then)?;
                if let Some(otherwise) = otherwise {
                    self.nested_unaudited(otherwise)?;
                }
                missing_cell!(self, "conditional node")
            }
            Node::Optional { chooser, node } => {
                self.player(chooser)?;
                self.nested_unaudited(node)?;
                missing_cell!(self, "optional node")
            }
            Node::ForEach { var, over, body } => {
                let bound = match over {
                    Domain::Players { players } => {
                        self.player(players)?;
                        RefKind::Player
                    }
                    Domain::Objects { set } => {
                        self.set(set)?;
                        RefKind::Object
                    }
                };
                self.bound.push((*var, bound));
                let result = self.nested_unaudited(body);
                self.bound.pop();
                result?;
                missing_cell!(self, "for-each node")
            }
            Node::Choose {
                var,
                chooser,
                from,
                minimum,
                maximum,
                body,
            } => {
                self.player(chooser)?;
                self.set(from)?;
                self.amount(minimum)?;
                self.amount(maximum)?;
                self.bound.push((*var, RefKind::Object));
                let result = self.nested_unaudited(body);
                self.bound.pop();
                result?;
                missing_cell!(self, "choose node")
            }
            Node::UnlessPays {
                payer,
                payment,
                node,
            } => {
                self.player(payer)?;
                match payment {
                    Payment::Generic { amount } | Payment::Life { amount } => {
                        self.amount(amount)?
                    }
                }
                self.nested_unaudited(node)?;
                missing_cell!(self, "unless-pays node")
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

    /// Scope-checks a child of a control node. The child's own audit result
    /// is dropped: the enclosing control node reports the missing cell.
    fn node_unaudited(&mut self, node: &Node) -> Result<(), KirError> {
        match self.node(node) {
            Err(KirError::Unaudited(_)) => Ok(()),
            other => other,
        }
    }
    fn nested_unaudited(&mut self, node: &Node) -> Result<(), KirError> {
        let steps = std::mem::take(&mut self.steps);
        let result = self.node_unaudited(node);
        self.steps = steps;
        result
    }

    /// A fixed amount of at least `minimum`.
    fn fixed_at_least(amount: &Amount, minimum: i64, what: &str) -> Result<(), KirError> {
        match amount.as_fixed() {
            Some(value) if *value >= Quantity::from(minimum) => Ok(()),
            Some(_) => kind(format!("{what} below {minimum}")),
            None => unaudited(format!("computed {what}")),
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
            // Zero is a legal amount that changes nothing (CR 119.8, 107.1b).
            Verb::GainLife { player, amount } | Verb::LoseLife { player, amount } => {
                Self::fixed_at_least(amount, 0, "life amount")?;
                match player {
                    PlayerExpr::Is {
                        who: Ref::Controller { .. } | Ref::EventPlayer { .. },
                    }
                    | PlayerExpr::EachOpponent => Ok(()),
                    PlayerExpr::Is {
                        who: Ref::Target { .. },
                    } if self.player_target(player)? == Some(TargetDomain::AnyPlayer) => Ok(()),
                    _ => unaudited("life recipient"),
                }
            }
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
                Self::audited_filter(filter)?;
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
            } => Self::audited_filter(filter),
            Verb::Move {
                what:
                    SetExpr::Matching {
                        zone: ZoneExpr::Graveyards,
                        filter,
                    },
                to: Destination::Exile,
            } if filter.0.is_empty() => Ok(()),
            _ => unaudited("verb"),
        }
    }

    /// The atoms the core evaluates for a board-wide set (CR 608.2h). Every
    /// one reads only the object and the invocation's controller.
    fn audited_filter(filter: &Pred) -> Result<(), KirError> {
        for atom in &filter.0 {
            match atom {
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
                    alternatives.iter().try_for_each(Self::audited_filter)?
                }
                Atom::Blocking | Atom::OwnedBy { .. } | Atom::IsNot { .. } => {
                    return unaudited("set filter atom");
                }
            }
        }
        Ok(())
    }
}

/// The capabilities one verb needs. This is the whole capability mapping for
/// kernel nodes: a verb names the reviewed runtime surface it uses, and no
/// card, family or operand combination adds a capability of its own.
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
        // Board-wide destruction and movement share the reviewed wipe surface.
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
        _ => {}
    }
}

/// The capabilities a node derives, independent of where it resolves.
pub fn node_capabilities(node: &Node) -> BTreeSet<Capability> {
    let mut out = BTreeSet::new();
    visit_verbs(node, &mut |verb| verb_capabilities(verb, &mut out));
    out
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
        _ => &[],
    }
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
        // In scope and well typed; the computed life amount has no cell yet.
        assert!(matches!(check(&node, &with_x), Err(KirError::Unaudited(_))));
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
        assert!(matches!(
            check(&each(bound.clone()), &spell(&[])),
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
        let requirements = check(&destroy(creatures.clone(), false), &env).unwrap();
        assert_eq!(
            requirements.capabilities,
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
        assert!(
            Pred(vec![Atom::ControlledBy {
                player: PlayerExpr::you(Context::Spell)
            }])
            .names_you()
        );
        assert!(!creatures.names_you());
    }

    #[test]
    fn capabilities_are_derived_from_verbs_alone() {
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
            BTreeSet::from([Capability::LifeGainV1, Capability::LifeLossV1])
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
