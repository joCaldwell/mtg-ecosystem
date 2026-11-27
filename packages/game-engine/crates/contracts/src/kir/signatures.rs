//! Checked slots for the verbs using the resolving evaluator.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatedVerb {
    Draw,
    GainLife,
    LoseLife,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandClass {
    Source,
    Controller,
    Target,
    TargetRelation,
    EventPlayer,
    EventObject,
    ObjectRelation,
    Bound,
    PlayerDomain,
    Fixed,
    AnnouncedX,
    Measure,
    StepResult,
}

impl OperandClass {
    pub fn reference(reference: &Ref) -> Option<Self> {
        Some(match reference {
            Ref::Source => Self::Source,
            Ref::Controller { .. } => Self::Controller,
            Ref::Target { .. } => Self::Target,
            Ref::EventPlayer { .. } => Self::EventPlayer,
            Ref::EventObject { .. } => Self::EventObject,
            Ref::BoundVar { .. } => Self::Bound,
            Ref::AnnouncedX => Self::AnnouncedX,
            Ref::StepResult { .. } => Self::StepResult,
            _ => return None,
        })
    }

    pub fn player(player: &PlayerExpr) -> Option<Self> {
        match player {
            PlayerExpr::Is { who } => Self::reference(who),
            PlayerExpr::ControllerOf { object } | PlayerExpr::OwnerOf { object } => {
                Self::reference(object).map(|class| match class {
                    Self::Target => Self::TargetRelation,
                    _ => Self::ObjectRelation,
                })
            }
            PlayerExpr::EachOpponent | PlayerExpr::EachPlayer => Some(Self::PlayerDomain),
        }
    }

    pub fn amount(amount: &Amount) -> Option<Self> {
        match amount {
            Amount::Fixed { .. } => Some(Self::Fixed),
            Amount::Var { of } => Self::reference(of),
            _ => Some(Self::Measure),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRole {
    Subject,
    Recipient,
    Amount,
    Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Player,
    Object,
    Set,
    Quantity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityDomain {
    NonNegative,
    Positive,
    Signed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadTime {
    Announcement,
    ResolutionStart,
    StepStart,
    EventBefore,
    EventAfter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectUse {
    ActOnLiveIncarnation,
    ReadEventSnapshot,
    ReadCurrentOrLastKnown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Absent {
    SkipMember,
    SkipVerb,
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbsentReason {
    IllegalTarget,
    NotChosen,
    DepartedPlayer,
    NoEventMember,
    NotPerformed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved<T> {
    Present(T),
    Absent(AbsentReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distribution {
    Single,
    EachSequential,
    EachSimultaneous,
    CollectThenSimultaneous,
    DividedAtAnnouncement,
}

pub struct OperandSlot {
    pub field: &'static str,
    pub role: SlotRole,
    pub sort: Sort,
    pub accepts: &'static [OperandClass],
    pub quantity: Option<QuantityDomain>,
    pub read: ReadTime,
    pub use_: ObjectUse,
    pub absent: Absent,
}

pub struct VerbSignature {
    pub verb: EvaluatedVerb,
    pub slots: &'static [OperandSlot],
    pub distribution: Distribution,
    pub projections: &'static [StepRole],
}

const PLAYER_AMOUNT: &[OperandSlot] = &[
    OperandSlot {
        field: "player",
        role: SlotRole::Recipient,
        sort: Sort::Player,
        accepts: &[
            OperandClass::Controller,
            OperandClass::Target,
            OperandClass::TargetRelation,
            OperandClass::EventPlayer,
            OperandClass::ObjectRelation,
            OperandClass::Bound,
            OperandClass::PlayerDomain,
            OperandClass::StepResult,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
    OperandSlot {
        field: "amount",
        role: SlotRole::Amount,
        sort: Sort::Quantity,
        accepts: &[
            OperandClass::Fixed,
            OperandClass::AnnouncedX,
            OperandClass::Measure,
            OperandClass::StepResult,
        ],
        quantity: Some(QuantityDomain::NonNegative),
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
];

const DRAW: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Draw,
    slots: PLAYER_AMOUNT,
    distribution: Distribution::EachSequential,
    projections: &[StepRole::Amount, StepRole::Objects, StepRole::Player],
};
const GAIN: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::GainLife,
    slots: PLAYER_AMOUNT,
    distribution: Distribution::EachSimultaneous,
    projections: &[StepRole::Amount, StepRole::Player],
};
const LOSS: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::LoseLife,
    ..GAIN
};

impl Verb {
    pub fn evaluated_operands(&self) -> Option<(&'static VerbSignature, &PlayerExpr, &Amount)> {
        match self {
            Self::Draw { player, amount } => Some((&DRAW, player, amount)),
            Self::GainLife { player, amount } => Some((&GAIN, player, amount)),
            Self::LoseLife { player, amount } => Some((&LOSS, player, amount)),
            _ => None,
        }
    }
}

/// The leaf that supplies one unambiguous projection within a Seq sibling.
/// Stage 1 allows a direct action or a single guarded body.
pub fn step_producer(node: &Node) -> Option<(&VerbSignature, Option<u32>)> {
    match node {
        Node::Do(verb) => Some((verb.evaluated_operands()?.0, None)),
        Node::Optional { node, .. }
        | Node::IfPays { node, .. }
        | Node::If {
            then: node,
            otherwise: None,
            ..
        } => Some((node.verb()?.evaluated_operands()?.0, Some(0))),
        _ => None,
    }
}
