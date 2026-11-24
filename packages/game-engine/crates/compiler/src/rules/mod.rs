//! Verb and block lowerers composition drives block by block.
use super::*;

mod activations;
mod procedures;
mod statics;
mod triggers;
mod verbs;
pub(crate) use activations::*;
pub(crate) use procedures::*;
pub(crate) use statics::*;
pub(crate) use triggers::*;
pub(crate) use verbs::append_spell_effects;
