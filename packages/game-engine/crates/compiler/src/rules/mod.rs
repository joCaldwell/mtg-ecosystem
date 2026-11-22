//! Verb and block lowerers shared by composition and the residual families.
use super::*;

mod activations;
mod effects;
mod statics;
mod triggers;
pub(crate) use activations::*;
pub(crate) use effects::*;
pub(crate) use statics::*;
pub(crate) use triggers::*;
