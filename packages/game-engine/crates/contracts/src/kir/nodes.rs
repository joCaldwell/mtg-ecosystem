//! Control nodes: sequencing, guards, choices and repetition.
use super::*;
use crate::{ManaCost, SpellProgram};

// ---------------------------------------------------------------------------
// Control
// ---------------------------------------------------------------------------

/// The simultaneous battlefield removal a counted target group performs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupRemoval {
    Destroy,
    Exile,
    OwnerHand,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "cond", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cond {
    TurnHistory {
        fact: super::TurnFact,
        cmp: Cmp,
        value: Quantity,
    },
    Compare {
        left: Amount,
        cmp: Cmp,
        right: Amount,
    },
    /// CR 102.1: whether the active player is among these players.
    ActivePlayer {
        player: PlayerExpr,
    },
    Exists {
        set: SetExpr,
    },
    /// CR 601.2b, 702.33d: an optional additional cost was paid.
    CostPaid {
        identity: String,
    },
    /// CR 608.2h: the referenced object satisfies `filter`, using its last
    /// known information once it has left its zone.
    Satisfies {
        object: Ref,
        filter: Pred,
    },
    /// CR 603.4, 400.7: the referenced permanent entered the battlefield
    /// from a spell `caster` cast.
    CastBy {
        object: Ref,
        caster: PlayerExpr,
    },
    /// CR 903.3, 603.4: the ability controller controls a commander
    /// designated for their own deck.
    ControlsOwnCommander {},
    /// CR 603.4, 102.3: some individual opponent controls more matching
    /// permanents than the ability controller.
    OpponentControlsMore {
        kind: CardKind,
    },
    /// CR 603.4: compare with the opponent identified by the trigger event.
    EventOpponentControlsMore {
        kind: CardKind,
    },
    /// CR 107.1b, 603.4: total signed power of creatures you control.
    ControllerCreatureTotalPower {
        cmp: Cmp,
        value: Quantity,
    },
    /// CR 702.30a, 603.4: the source permanent came under the ability
    /// controller's control since the beginning of that player's last upkeep.
    ControlledSinceLastUpkeep {},
    /// CR 602.5: a measurement only an activation condition reads, compared
    /// with a fixed number ([`super::ActivationCondition`]).
    Measured {
        measure: super::Measure,
        cmp: Cmp,
        value: Quantity,
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
    Mana { mana: ManaCost },
    Sacrifice { count: u32, filter: Pred },
    DiscardArtifact {},
}

/// The instruction performed on a single public object selected at resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectChoiceAction {
    ReturnToOwnerHand,
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
    /// CR 608.2d, 205.3m: choose a creature type while resolving.
    ChooseCreatureType {
        var: u32,
        chooser: PlayerExpr,
        body: Box<Node>,
    },
    /// CR 701.20a, 701.20b: publicly reveal a library prefix in place and
    /// bind the cards named by the following instructions.
    RevealTop {
        var: u32,
        player: PlayerExpr,
        count: u32,
        body: Box<Node>,
    },
    /// CR 608.2d: the controller chooses one object from the current public set.
    ChooseObject {
        from: SetExpr,
        action: ObjectChoiceAction,
    },
    /// CR 115.3, 608.2b, 701.26: tap or untap all legal announced members
    /// of the single "up to N" target occurrence simultaneously.
    TapTargets {
        count: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variable: Option<VariableTargetCount>,
        tap: bool,
    },
    /// CR 115.3, 608.2b, 701.8, 400.3: destroy, exile or return to owners'
    /// hands every legal announced member of the single counted battlefield
    /// target occurrence in one simultaneous event.
    RemoveTargets {
        count: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variable: Option<VariableTargetCount>,
        removal: GroupRemoval,
    },
    /// CR 118.12, 608.2b: sacrifice one controlled creature if able;
    /// on doing so return the legal announced creature cards together.
    SacrificeForTappedReturn {
        count: u32,
    },
    /// CR 118.12a: perform `node` unless `payer` pays.
    UnlessPays {
        payer: PlayerExpr,
        payment: Payment,
        node: Box<Node>,
    },
    /// CR 118.12, 608.2c: "[payer] may pay [cost]. If they do, [node]":
    /// perform `node` only if `payer` pays.
    IfPays {
        payer: PlayerExpr,
        payment: Payment,
        node: Box<Node>,
    },
    /// CR 603.7: perform `node` when the delayed event next occurs.
    Delayed {
        event: DelayedEvent,
        program: Box<SpellProgram>,
    },
    /// CR 603.12: paying this optional fixed cost creates a separate trigger.
    Reflexive {
        payment: ManaCost,
        program: Box<SpellProgram>,
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

    /// CR 608.2b: the one announced object a target-group leaf acts on.
    /// The leaf's whole action applies to that object, so it is skipped
    /// when the object was not chosen or is an illegal target.
    pub fn group_leaf_slot(&self) -> Option<u32> {
        let one = |set: &SetExpr| match set {
            SetExpr::One {
                object: Ref::Target { slot },
            } => Some(*slot),
            _ => None,
        };
        let slot = match self.verb()? {
            Verb::Destroy { what, .. }
            | Verb::Move { what, .. }
            | Verb::Tap { what }
            | Verb::Untap { what }
            | Verb::PutCounters { on: what, .. }
            | Verb::ModifyUntil { what, .. } => one(what)?,
            Verb::Damage {
                source: Ref::Source,
                to: DamageTo::Ref {
                    who: Ref::Target { slot },
                },
                ..
            } => *slot,
            Verb::RestrictCombat {
                affects:
                    CombatProhibitionScope::Target {
                        target: Ref::Target { slot },
                    },
                ..
            } => *slot,
            _ => return None,
        };
        (crate::kir::target_slots(self) == std::collections::BTreeSet::from([slot])).then_some(slot)
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
            Self::Seq(_) => true,
            Self::Optional { .. } => self.optional_target_destroy().is_none(),
            Self::If { .. }
            | Self::ForEach { .. }
            | Self::Choose { .. }
            | Self::ChooseCreatureType { .. }
            | Self::RevealTop { .. }
            | Self::UnlessPays { .. }
            | Self::IfPays { .. } => true,
            Self::Reflexive { .. } => true,
            Self::Do(_)
            | Self::Delayed { .. }
            | Self::ChooseObject { .. }
            | Self::TapTargets { .. }
            | Self::SacrificeForTappedReturn { .. }
            | Self::RemoveTargets { .. } => false,
        }
    }
}

/// CR 601.2b-c: the sole occurrence's cardinality at announcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VariableTargetCount {
    Any,
    ExactlyX,
    UpToX,
}
