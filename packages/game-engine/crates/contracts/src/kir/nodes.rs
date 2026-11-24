//! Control nodes: sequencing, guards, choices and repetition.
use super::*;

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
