//! The executor's public surface: admission, the decision protocol, the game
//! handle with its accessors, play (submit and advance) and recovery.
//!
//! Public results keep their text errors; the rule machinery beneath
//! (`state`, `frame`, `step`) reports `RuleError`.
mod decision;
mod event_vocabulary;
mod game;
mod play;
mod protocol;
mod scenario;
mod snapshot;
#[cfg(test)]
mod tests;

pub use decision::*;
pub use event_vocabulary::*;
pub use game::*;
pub use protocol::*;
pub use scenario::*;
