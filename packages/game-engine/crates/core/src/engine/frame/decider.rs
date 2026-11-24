//! Who decides at the suspended frame.
//!
//! Exactly one player can owe a decision, and the frame alone selects the
//! one request generator that can produce it. Each viewer's decision token is
//! keyed by the revision and the numeric kind returned here; the kinds are
//! part of each viewer's stored knowledge and never change meaning.
use super::*;

/// The player who owes the pending decision and the kind of its token key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decider {
    pub player: PlayerId,
    pub reason: Reason<Rules>,
}

impl Frame {
    /// The decider at this frame for `game`, whose suspended frame it is.
    /// Every variant is named: a new frame must say who decides at it.
    pub fn decider(&self, game: &RulesTxn) -> Result<Option<Decider>, RuleError> {
        if game.state.rules.pending.is_some() || !game.state.rules.pending_concessions.is_empty() {
            return Ok(None);
        }
        // Priority is frequent and has no conditional chooser. Publishing its
        // token precedes enumerating actions against the committed visible rows.
        if matches!(self, Frame::Priority) {
            return Ok(Some(Decider {
                player: game.state.rules.priority,
                reason: Reason::Priority,
            }));
        }
        Ok(game.frame_decision()?.map(|decision| Decider {
            player: decision.player,
            reason: decision.reason,
        }))
    }
}
