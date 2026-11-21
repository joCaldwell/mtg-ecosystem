//! The transaction every change to a game commits through.
//!
//! A change runs on a staged copy that shares the game's tables and copies
//! only what it writes. `transact` then publishes the staged state to all
//! four viewers, checks the retained-history capacity and, only if all of
//! that succeeded, replaces the game's state and issues a new state token.
//! A failed change leaves the game exactly as it was. Which tables changed
//! is read from table identity: a written table or object chunk is a new
//! allocation, so publication needs no separate dirty set.
use crate::engine::*;
use std::ops::{Deref, DerefMut};

/// The staged copy one change runs on. Rule routines are methods of the game;
/// through this handle they act on the staged copy only.
pub struct RulesTxn<'a> {
    staged: &'a mut Game,
}

impl Deref for RulesTxn<'_> {
    type Target = Game;
    fn deref(&self) -> &Game {
        self.staged
    }
}
impl DerefMut for RulesTxn<'_> {
    fn deref_mut(&mut self) -> &mut Game {
        self.staged
    }
}

impl Game {
    /// Runs one rule change atomically: stage, change, publish, check the
    /// history capacity, commit. On failure nothing is committed, and a
    /// capacity fault the change recorded travels with the error.
    pub(crate) fn transact<T>(
        &mut self,
        change: impl FnOnce(&mut RulesTxn<'_>) -> Result<T, RuleError>,
    ) -> Result<T, RuleError> {
        self.commit_staged(true, change)
    }

    /// Accepts an input atomically. Retaining an input is charged to history
    /// but never refused here: the next rule step's capacity check faults
    /// instead, so an accepted input is always journaled.
    pub(crate) fn transact_input<T>(
        &mut self,
        change: impl FnOnce(&mut RulesTxn<'_>) -> Result<T, RuleError>,
    ) -> Result<T, RuleError> {
        self.commit_staged(false, change)
    }

    fn commit_staged<T>(
        &mut self,
        metered: bool,
        change: impl FnOnce(&mut RulesTxn<'_>) -> Result<T, RuleError>,
    ) -> Result<T, RuleError> {
        let mut staged = self.fork();
        let outcome = change(&mut RulesTxn {
            staged: &mut staged,
        })
        .and_then(|value| {
            staged.refresh_visibility()?;
            if metered {
                staged.check_history_capacity()?;
            }
            Ok(value)
        });
        match outcome {
            Ok(value) => {
                #[cfg(debug_assertions)]
                if let Err(violation) = staged.validate_state() {
                    panic!("committed state violates its invariant: {violation}");
                }
                self.state = staged.state;
                self.trace = staged.trace;
                self.published = staged.published;
                self.state_token = Arc::new(());
                Ok(value)
            }
            Err(error) => Err(error.with_fault(staged.state.rules.typed_fault.clone())),
        }
    }
}
