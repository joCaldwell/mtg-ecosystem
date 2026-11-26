//! References: the objects, players and numbers an invocation can see, and
//! the player expressions built from them (CR 109.5, 608.2b-c).
use super::*;

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
    /// CR 121.1: the player who drew one card.
    DrawingPlayer,
    /// CR 509.3g: the player the unblocked creature is attacking.
    DefendingPlayer,
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
