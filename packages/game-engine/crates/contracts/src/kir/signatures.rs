//! Checked slots for the verbs using the resolving evaluator.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatedVerb {
    Draw,
    GainLife,
    LoseLife,
    Damage,
    PutCounters,
    Sacrifice,
    Discard,
    Mill,
    CreateTokens,
    /// CR 400.3, 400.7: a zone change.
    Move,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandClass {
    Source,
    Controller,
    Target,
    TargetRelation,
    EventPlayer,
    EventObject,
    /// CR 603.2, 608.2h: the number the triggering event carried ("that much").
    EventAmount,
    ObjectRelation,
    Bound,
    PlayerDomain,
    Fixed,
    AnnouncedX,
    Measure,
    StepResult,
    CostResult,
}

impl OperandClass {
    pub fn reference(reference: &Ref) -> Option<Self> {
        Some(match reference {
            Ref::Source => Self::Source,
            Ref::Controller { .. } => Self::Controller,
            Ref::Target { .. } => Self::Target,
            Ref::EventPlayer { .. } => Self::EventPlayer,
            Ref::EventObject { .. } => Self::EventObject,
            Ref::EventAmount => Self::EventAmount,
            Ref::BoundVar { .. } => Self::Bound,
            Ref::AnnouncedX => Self::AnnouncedX,
            Ref::StepResult { .. } => Self::StepResult,
            Ref::CostResult { .. } => Self::CostResult,
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

    /// A set of objects: one referenced object, or a set computed when the
    /// step is performed (CR 608.2h).
    pub fn set(set: &SetExpr) -> Option<Self> {
        match set {
            SetExpr::One { object } => Self::reference(object),
            SetExpr::Matching { .. } => Some(Self::Measure),
            SetExpr::Pool { pool } => Self::reference(pool),
            SetExpr::Targets { .. } => Some(Self::Target),
            SetExpr::Result { .. } => Some(Self::StepResult),
        }
    }

    /// Every recipient part of a damage step (CR 120.2).
    pub fn recipients(to: &DamageTo) -> Vec<Option<Self>> {
        match to {
            DamageTo::Ref { who } => vec![Self::reference(who)],
            DamageTo::Players { players } => vec![Self::player(players)],
            DamageTo::Objects { set } => vec![Self::set(set)],
            DamageTo::All { parts } => parts.iter().flat_map(Self::recipients).collect(),
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
    pub object_result_read: ObjectUse,
    pub projections: &'static [StepRole],
}

const PLAYER_CLASSES: &[OperandClass] = &[
    OperandClass::Controller,
    OperandClass::Target,
    OperandClass::TargetRelation,
    OperandClass::EventPlayer,
    OperandClass::ObjectRelation,
    OperandClass::Bound,
    OperandClass::PlayerDomain,
    OperandClass::StepResult,
    OperandClass::CostResult,
];

const QUANTITY_CLASSES: &[OperandClass] = &[
    OperandClass::Fixed,
    OperandClass::AnnouncedX,
    OperandClass::Measure,
    OperandClass::StepResult,
    OperandClass::CostResult,
    OperandClass::EventAmount,
];

const fn quantity(domain: QuantityDomain) -> OperandSlot {
    OperandSlot {
        field: "amount",
        role: SlotRole::Amount,
        sort: Sort::Quantity,
        accepts: QUANTITY_CLASSES,
        quantity: Some(domain),
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    }
}

const PLAYER_AMOUNT: &[OperandSlot] = &[
    OperandSlot {
        field: "player",
        role: SlotRole::Recipient,
        sort: Sort::Player,
        accepts: PLAYER_CLASSES,
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
    quantity(QuantityDomain::NonNegative),
];

/// CR 120.1-120.3, 608.2h: the object that deals the damage, its recipients
/// and the amount. A source that left reads last-known information.
const DAMAGE_SLOTS: &[OperandSlot] = &[
    OperandSlot {
        field: "source",
        role: SlotRole::Source,
        sort: Sort::Object,
        accepts: &[
            OperandClass::Source,
            OperandClass::Target,
            OperandClass::EventObject,
            OperandClass::Bound,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
    OperandSlot {
        field: "to",
        role: SlotRole::Recipient,
        sort: Sort::Set,
        accepts: &[
            OperandClass::Source,
            OperandClass::Controller,
            OperandClass::Target,
            OperandClass::TargetRelation,
            OperandClass::EventPlayer,
            OperandClass::EventObject,
            OperandClass::ObjectRelation,
            OperandClass::Bound,
            OperandClass::PlayerDomain,
            OperandClass::Measure,
            OperandClass::StepResult,
            OperandClass::CostResult,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ActOnLiveIncarnation,
        absent: Absent::SkipMember,
    },
    quantity(QuantityDomain::Positive),
];

/// CR 122.1, 122.6: the permanents that receive counters and how many each.
const COUNTER_SLOTS: &[OperandSlot] = &[
    OperandSlot {
        field: "on",
        role: SlotRole::Recipient,
        sort: Sort::Set,
        accepts: &[
            OperandClass::Source,
            OperandClass::Target,
            OperandClass::EventObject,
            OperandClass::Bound,
            OperandClass::Measure,
            OperandClass::StepResult,
            OperandClass::CostResult,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ActOnLiveIncarnation,
        absent: Absent::SkipMember,
    },
    quantity(QuantityDomain::Positive),
];

/// CR 701.21a, 609.3: who sacrifices, how many and which permanents: one
/// designated permanent ("sacrifice it") or a choice among the permanents
/// that player controls.
const SACRIFICE_SLOTS: &[OperandSlot] = &[
    OperandSlot {
        field: "player",
        role: SlotRole::Subject,
        sort: Sort::Player,
        accepts: PLAYER_CLASSES,
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
    quantity(QuantityDomain::Positive),
    OperandSlot {
        field: "filter",
        role: SlotRole::Recipient,
        sort: Sort::Set,
        accepts: &[
            OperandClass::Source,
            OperandClass::EventObject,
            OperandClass::Measure,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ActOnLiveIncarnation,
        absent: Absent::SkipMember,
    },
];

/// CR 400.3, 400.7, 608.2b-c: the objects that change zones and, for
/// battlefield entry, the player under whose control they enter (CR 110.2a).
/// Each object goes to its owner's zone; a new incarnation is never moved.
const MOVE_SLOTS: &[OperandSlot] = &[
    OperandSlot {
        field: "what",
        role: SlotRole::Subject,
        sort: Sort::Set,
        accepts: &[
            OperandClass::Source,
            OperandClass::Target,
            OperandClass::Measure,
        ],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ActOnLiveIncarnation,
        absent: Absent::SkipMember,
    },
    OperandSlot {
        field: "controller",
        role: SlotRole::Recipient,
        sort: Sort::Player,
        accepts: &[OperandClass::Controller],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
];

const DRAW: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Draw,
    slots: PLAYER_AMOUNT,
    distribution: Distribution::EachSequential,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[StepRole::Amount, StepRole::Objects, StepRole::Player],
};
const GAIN: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::GainLife,
    slots: PLAYER_AMOUNT,
    distribution: Distribution::EachSimultaneous,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[StepRole::Amount, StepRole::Player],
};
const LOSS: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::LoseLife,
    ..GAIN
};
const DAMAGE: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Damage,
    slots: DAMAGE_SLOTS,
    distribution: Distribution::EachSimultaneous,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[StepRole::Amount, StepRole::Objects, StepRole::Player],
};
const COUNTERS: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::PutCounters,
    slots: COUNTER_SLOTS,
    distribution: Distribution::EachSimultaneous,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[StepRole::Amount, StepRole::Objects],
};
const MILL: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Mill,
    slots: PLAYER_AMOUNT,
    distribution: Distribution::EachSimultaneous,
    object_result_read: ObjectUse::ReadCurrentOrLastKnown,
    projections: &[StepRole::Amount, StepRole::Objects, StepRole::Player],
};
const DISCARD: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Discard,
    distribution: Distribution::CollectThenSimultaneous,
    ..MILL
};
/// CR 400.7, 608.2c: the moved objects and their number. Characteristics of
/// a moved card are its public successor's or its last-known ones.
const MOVE: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Move,
    slots: MOVE_SLOTS,
    distribution: Distribution::EachSimultaneous,
    object_result_read: ObjectUse::ReadCurrentOrLastKnown,
    projections: &[StepRole::Amount, StepRole::Objects],
};
const SACRIFICE: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::Sacrifice,
    slots: SACRIFICE_SLOTS,
    distribution: Distribution::CollectThenSimultaneous,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[StepRole::Amount, StepRole::Objects, StepRole::Player],
};

/// CR 111.1-111.2, 608.2h: who creates the tokens and how many, read as the
/// step is performed. The creating player is the ability's controller or the
/// current or last-known controller of an announced target.
const TOKEN_SLOTS: &[OperandSlot] = &[
    OperandSlot {
        field: "controller",
        role: SlotRole::Recipient,
        sort: Sort::Player,
        accepts: &[OperandClass::Controller, OperandClass::TargetRelation],
        quantity: None,
        read: ReadTime::StepStart,
        use_: ObjectUse::ReadCurrentOrLastKnown,
        absent: Absent::SkipVerb,
    },
    quantity(QuantityDomain::NonNegative),
];

const TOKENS: VerbSignature = VerbSignature {
    verb: EvaluatedVerb::CreateTokens,
    slots: TOKEN_SLOTS,
    distribution: Distribution::Single,
    object_result_read: ObjectUse::ActOnLiveIncarnation,
    projections: &[],
};

/// The operands of an evaluated verb, in signature slot order.
#[derive(Clone, Copy, Debug)]
pub enum Evaluated<'a> {
    PlayerAmount {
        player: &'a PlayerExpr,
        amount: &'a Amount,
    },
    Damage {
        source: &'a Ref,
        to: &'a DamageTo,
        amount: &'a Amount,
    },
    Counters {
        on: &'a SetExpr,
        counter: &'a CounterKind,
        amount: &'a Amount,
    },
    Discard {
        player: &'a PlayerExpr,
        amount: Option<&'a Amount>,
        random: bool,
    },
    Sacrifice {
        player: &'a PlayerExpr,
        count: &'a Amount,
        filter: &'a Pred,
        greatest_power: bool,
    },
    Tokens {
        controller: &'a PlayerExpr,
        count: &'a Amount,
        token: &'a TokenSpec,
        tapped: bool,
    },
    Move {
        what: &'a SetExpr,
        to: &'a Destination,
    },
}

impl<'a> Evaluated<'a> {
    /// The quantity slot every evaluated verb has.
    pub fn amount(&self) -> Option<&'a Amount> {
        match *self {
            Self::PlayerAmount { amount, .. }
            | Self::Damage { amount, .. }
            | Self::Counters { amount, .. }
            | Self::Sacrifice { count: amount, .. }
            | Self::Tokens { count: amount, .. } => Some(amount),
            Self::Discard { amount, .. } => amount,
            Self::Move { .. } => None,
        }
    }

    /// CR 701.21a: "sacrifice it" names one exact permanent instead of a
    /// choice among the player's permanents.
    pub fn designated(&self) -> Option<&'a Ref> {
        match *self {
            Self::Sacrifice { filter, .. } => match filter.0.as_slice() {
                [Atom::Is { object }] => Some(object),
                _ => None,
            },
            _ => None,
        }
    }

    /// Each slot with the classes of the operands it holds, in slot order.
    pub fn classes(
        &self,
        signature: &'static VerbSignature,
    ) -> Vec<(&'static OperandSlot, Option<OperandClass>)> {
        let slot = |index: usize| &signature.slots[index];
        match *self {
            Self::PlayerAmount { player, amount } => vec![
                (slot(0), OperandClass::player(player)),
                (slot(1), OperandClass::amount(amount)),
            ],
            Self::Discard { player, amount, .. } => {
                let mut out = vec![(slot(0), OperandClass::player(player))];
                if let Some(amount) = amount {
                    out.push((slot(1), OperandClass::amount(amount)));
                }
                out
            }
            Self::Damage { source, to, amount } => {
                let mut out = vec![(slot(0), OperandClass::reference(source))];
                out.extend(
                    OperandClass::recipients(to)
                        .into_iter()
                        .map(|c| (slot(1), c)),
                );
                out.push((slot(2), OperandClass::amount(amount)));
                out
            }
            Self::Counters { on, amount, .. } => vec![
                (slot(0), OperandClass::set(on)),
                (slot(1), OperandClass::amount(amount)),
            ],
            Self::Tokens {
                controller, count, ..
            } => vec![
                (slot(0), OperandClass::player(controller)),
                (slot(1), OperandClass::amount(count)),
            ],
            Self::Move { what, to } => {
                let mut out = vec![(slot(0), OperandClass::set(what))];
                if let Destination::Battlefield { controller, .. } = to {
                    out.push((slot(1), OperandClass::player(controller)));
                }
                out
            }
            Self::Sacrifice { player, count, .. } => vec![
                (slot(0), OperandClass::player(player)),
                (slot(1), OperandClass::amount(count)),
                (
                    slot(2),
                    match self.designated() {
                        Some(object) => OperandClass::reference(object),
                        None => Some(OperandClass::Measure),
                    },
                ),
            ],
        }
    }
}

impl Verb {
    /// The signature and operands of a verb the resolving evaluator performs.
    pub fn evaluated(&self) -> Option<(&'static VerbSignature, Evaluated<'_>)> {
        match self {
            Self::Mill { player, amount } => {
                Some((&MILL, Evaluated::PlayerAmount { player, amount }))
            }
            // CR 608.2n: a resolving spell shuffling itself is its own
            // departure from the stack, part of its resolution.
            Self::Move {
                what: SetExpr::One {
                    object: Ref::Source,
                },
                to: Destination::OwnerLibraryShuffled,
            } => None,
            Self::Move { what, to } => Some((&MOVE, Evaluated::Move { what, to })),
            Self::Discard {
                player,
                amount,
                random,
            } => Some((
                &DISCARD,
                Evaluated::Discard {
                    player,
                    amount: amount.as_ref(),
                    random: *random,
                },
            )),
            Self::Draw { player, amount } => {
                Some((&DRAW, Evaluated::PlayerAmount { player, amount }))
            }
            Self::GainLife { player, amount } => {
                Some((&GAIN, Evaluated::PlayerAmount { player, amount }))
            }
            Self::LoseLife { player, amount } => {
                Some((&LOSS, Evaluated::PlayerAmount { player, amount }))
            }
            Self::Damage { source, to, amount } => {
                Some((&DAMAGE, Evaluated::Damage { source, to, amount }))
            }
            Self::PutCounters {
                on,
                counter,
                amount,
            } => Some((
                &COUNTERS,
                Evaluated::Counters {
                    on,
                    counter,
                    amount,
                },
            )),
            Self::Sacrifice {
                player,
                count,
                filter,
                greatest_power,
            } => Some((
                &SACRIFICE,
                Evaluated::Sacrifice {
                    player,
                    count,
                    filter,
                    greatest_power: *greatest_power,
                },
            )),
            Self::CreateTokens {
                controller,
                count,
                token,
                tapped,
            } => Some((
                &TOKENS,
                Evaluated::Tokens {
                    controller,
                    count,
                    token,
                    tapped: *tapped,
                },
            )),
            _ => None,
        }
    }

    pub fn is_evaluated(&self) -> bool {
        self.evaluated().is_some()
    }
}

/// The leaf that supplies one unambiguous projection within a Seq sibling.
/// Stage 1 allows a direct action or a single guarded body.
pub fn step_producer(node: &Node) -> Option<(&'static VerbSignature, Option<u32>)> {
    match node {
        Node::Do(verb) => Some((verb.evaluated()?.0, None)),
        Node::Optional { node, .. }
        | Node::IfPays { node, .. }
        | Node::If {
            then: node,
            otherwise: None,
            ..
        } => Some((node.verb()?.evaluated()?.0, Some(0))),
        _ => None,
    }
}
