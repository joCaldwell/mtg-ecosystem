//! Verb and block lowerers composition drives block by block.
use super::*;

mod activations;
mod effects;
mod procedures;
mod statics;
mod triggers;
pub(crate) use activations::*;
pub(crate) use effects::*;
pub(crate) use procedures::*;
pub(crate) use statics::*;
pub(crate) use triggers::*;
