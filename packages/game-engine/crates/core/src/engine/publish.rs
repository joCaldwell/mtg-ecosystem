//! Publication of each viewer's permitted knowledge after a state change.
//!
//! A publication updates every viewer's handles and decision token, projects
//! the permitted view and appends it to that viewer's history when it differs
//! from the last one. The complete projection (`publish_full`, in
//! `visibility.rs`) is the definition. This module adds the coarse
//! incremental path: when nothing a view row, handle or public table is
//! derived from has changed since the last publication, the last view's rows
//! are reused and only the priority and turn fields are refreshed.
//!
//! In builds with `debug_assertions`, every publication is also run as a
//! complete projection of the same prior state and the two results must be
//! equal, so every test proves the incremental path against its definition.
use super::*;

mod handles;
pub(in crate::engine) use handles::HandleId;
pub mod permitted_history;
mod reuse;
pub mod stored_history;
pub mod visibility;

pub use reuse::Published;

impl Game {
    /// Publishes the current rules state to all four viewers.
    pub(crate) fn refresh_visibility(&mut self) -> Result<(), String> {
        #[cfg(debug_assertions)]
        let reference = self.complete_publication();
        let published = self.publish();
        #[cfg(debug_assertions)]
        self.assert_matches_complete_publication(reference, &published);
        published
    }

    fn publish(&mut self) -> Result<(), String> {
        if self.setup_automatic() {
            return Ok(());
        }
        let decider = self.state.rules.frame.decider(self)?;
        let cache = if self.republish_rows(decider)? {
            self.published
                .as_ref()
                .expect("reused a published view")
                .cache
                .clone()
        } else {
            self.publish_full(decider, false)?
        };
        self.published = Some(Arc::new(Published::of(&self.state, cache)));
        Ok(())
    }

    /// The prior state, to be published by the complete projection alone.
    #[cfg(debug_assertions)]
    fn complete_publication(&self) -> Game {
        let mut reference = self.fork();
        reference.published = None;
        reference
    }

    /// Proves this publication against the complete projection of the same
    /// prior state: the same outcome, and on success the same knowledge and
    /// history retention charge for every viewer.
    #[cfg(debug_assertions)]
    fn assert_matches_complete_publication(
        &self,
        mut reference: Game,
        published: &Result<(), String>,
    ) {
        // The reference publishes nothing incrementally and builds each view
        // row by row in table order.
        let complete = if reference.setup_automatic() {
            Ok(())
        } else {
            reference
                .state
                .rules
                .frame
                .decider(&reference)
                .map_err(String::from)
                .and_then(|decider| reference.publish_full(decider, true).map(|_| ()))
        };
        assert_eq!(
            *published, complete,
            "incremental publication outcome differs from the complete projection"
        );
        if complete.is_err() {
            return;
        }
        for (v, (incremental, complete)) in self
            .state
            .knowledge
            .visibility
            .iter()
            .zip(&reference.state.knowledge.visibility)
            .enumerate()
        {
            assert!(
                incremental == complete,
                "incremental publication differs from the complete projection for viewer {v}\n\
                 incremental: {:?}\ncomplete: {:?}",
                incremental.history.last(),
                complete.history.last(),
            );
            assert_eq!(
                incremental.history.retention_charge(),
                complete.history.retention_charge(),
                "history retention charge differs from the complete projection for viewer {v}"
            );
        }
    }
}
