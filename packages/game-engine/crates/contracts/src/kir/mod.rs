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

mod activation;
mod amount;
mod check;
mod events;
mod nodes;
mod pred;
mod refs;
mod signatures;
mod statics;
mod tiers;
mod verbs;

pub use activation::*;
pub use amount::*;
pub use check::*;
pub use events::*;
pub use nodes::*;
pub use pred::*;
pub use refs::*;
pub use signatures::*;
pub use statics::*;
pub(crate) use tiers::executes;
pub use tiers::*;
pub use verbs::*;
mod creature_types;
pub use creature_types::{
    CreatureType, is_basic_land_type, is_creature_subtype, is_land_subtype, subtype_card_type,
};

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

/// Pattern for the single object a trigger event names (CR 603.10).
#[macro_export]
macro_rules! event_object {
    () => {
        $crate::kir::SetExpr::One {
            object: $crate::kir::Ref::EventObject { .. },
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
#[path = "../kir_tests.rs"]
mod tests;
