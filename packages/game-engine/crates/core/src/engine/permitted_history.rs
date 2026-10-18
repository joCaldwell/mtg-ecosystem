//! Complete public history values backed only by already permitted immutable
//! rows. No Game, Knowledge handle map, Prepared, hidden identity or RNG is held.
use super::stored_history::{StoredHistory, StoredPermittedView};
use super::*;

/// An ordered, complete history of permitted observations.
///
/// Serialization is exactly the historical `Vec<PermittedView>` JSON array.
/// Clones share immutable entries; each getter/iterator item is an independent
/// owned public view. Collecting the iterator explicitly allocates all views.
/// Construction/deserialization is data only and never seals a SearchCapture.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermittedHistory(StoredHistory);
impl PermittedHistory {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<PermittedView> {
        self.stored(index).map(StoredPermittedView::to_public)
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = PermittedView> + ExactSizeIterator {
        self.0.iter().map(StoredPermittedView::to_public)
    }
    pub fn from_views(views: impl IntoIterator<Item = PermittedView>) -> Self {
        let mut history = Self::default();
        for view in views {
            history.0.push_public(view);
        }
        history
    }
    // This borrow is private: capture's per-object cursor must not invoke the
    // owned getter and repeatedly expand all rows in the current history view.
    pub(super) fn stored(&self, index: usize) -> Option<&StoredPermittedView> {
        (index < self.len()).then(|| &self.0[index])
    }
    // Exactly one stored entry for the original Task::Search history unit.
    // No whole-history prefill, bulk cursor advance or omitted public rows.
    pub(super) fn push_stored(&mut self, view: StoredPermittedView) {
        self.0.push(view);
    }
    #[cfg(test)]
    pub(super) fn push_public(&mut self, view: PermittedView) {
        self.0.push_public(view);
    }
}

#[cfg(test)]
#[path = "permitted_history_tests.rs"]
mod tests;
