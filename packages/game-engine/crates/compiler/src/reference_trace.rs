//! Sweep-time transition differential for pronoun antecedents.
//!
//! The Rust antecedent rule is the only source of meaning. While Stage 1
//! still reads the unchanged R5 archive, the sweep compares each antecedent
//! the rule chose with the frozen TypeScript resolution and fails on any
//! unexplained disagreement. Recording is off unless [`traced`] enables it
//! on the current thread, and never changes lowering.
use serde::Serialize;
use std::cell::RefCell;

/// One pronoun the lowerer bound, keyed by its printed block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Bound {
    pub block: u32,
    /// The printed pronoun: `it` or `that`.
    pub word: &'static str,
    pub resolved: Resolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "ref", rename_all = "kebab-case")]
pub enum Resolved {
    Source,
    Target {
        selection: String,
    },
    EventObject {
        role: String,
        /// The trigger's event object is the source itself (CR 201.5).
        is_source: bool,
    },
    Other {
        reference: String,
    },
}

thread_local! {
    static TRACE: RefCell<Option<Vec<Bound>>> = const { RefCell::new(None) };
}

pub(crate) fn record(bound: impl FnOnce() -> Bound) {
    TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.push(bound());
        }
    });
}

/// Run `work` with recording enabled and return what it recorded.
pub fn traced<T>(work: impl FnOnce() -> T) -> (T, Vec<Bound>) {
    let previous = TRACE.with(|trace| trace.replace(Some(Vec::new())));
    let result = work();
    let recorded = TRACE
        .with(|trace| trace.replace(previous))
        .unwrap_or_default();
    (result, recorded)
}
