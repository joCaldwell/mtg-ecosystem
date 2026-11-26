//! Control nodes: sequencing, guards, choices and repetition.
use super::*;
use crate::{ManaCost, SpellProgram};

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
    /// CR 608.2d: the controller chooses one object from the current public set.
    ChooseObject {
        from: SetExpr,
        action: ObjectChoiceAction,
    },
    /// CR 115.3, 608.2b, 701.26: tap or untap all legal announced members
    /// of the single "up to N" target occurrence simultaneously.
    TapTargets {
        count: u32,
        tap: bool,
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
            | Self::UnlessPays { .. }
            | Self::IfPays { .. } => true,
            Self::Reflexive { .. } => true,
            Self::Do(_)
            | Self::Delayed { .. }
            | Self::ChooseObject { .. }
            | Self::TapTargets { .. } => false,
        }
    }
}
