//! Sweep-time transition differential for Rust reference selections.
//!
//! The Rust antecedent rule is the only source of meaning. While Stage 1
//! still reads the unchanged R5 archive, the sweep compares each antecedent
//! the rule chose with the frozen TypeScript resolution and fails on any
//! unexplained disagreement. Recording is off unless [`traced`] enables it
//! on the current thread, and never changes lowering.
use mtg_contracts::TargetDomain;
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
    StepResult {
        step: String,
        role: &'static str,
    },
    Other {
        reference: String,
    },
}

/// One whole-card source-authority decision: the registered family and every
/// selection it made, each at its block-relative AST path.
#[derive(Serialize)]
pub(crate) struct SourceChoice {
    pub family: &'static str,
    pub face: usize,
    pub block: usize,
    pub selections: Vec<SourceSelection>,
}

#[derive(Serialize)]
pub(crate) struct SourceSelection {
    pub path: String,
    #[serde(flatten)]
    pub selection: Selection,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum Selection {
    /// CR 109.5.
    SpellController,
    /// CR 608.2h: evaluated in APNAP order at resolution.
    EachPlayer,
    EachOpponent,
    /// CR 601.2c.
    Target {
        slot: u32,
        selection_id: String,
        domain: TargetDomain,
    },
    /// CR 107.3a, 601.2b.
    AnnouncedX,
    /// CR 608.2h, 701.9b: the recipient, or a random process, chooses the
    /// cards at resolution.
    ResolutionCards {
        cardinality: Cardinality,
        random: bool,
    },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Cardinality {
    One,
    Many,
    /// The announced X may be zero or one.
    Unknown,
}

#[derive(Default)]
pub(crate) struct Trace {
    pub bound: Vec<Bound>,
    pub source: Vec<SourceChoice>,
}

thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

pub(crate) fn record(bound: impl FnOnce() -> Bound) {
    TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.bound.push(bound());
        }
    });
}

pub(crate) fn record_source(choice: impl FnOnce() -> SourceChoice) {
    TRACE.with(|trace| {
        if let Some(trace) = trace.borrow_mut().as_mut() {
            trace.source.push(choice());
        }
    });
}

/// Run `work` with recording enabled and return what it recorded.
pub fn traced<T>(work: impl FnOnce() -> T) -> (T, Vec<Bound>) {
    let (result, recorded) = traced_with_source(work);
    (result, recorded.bound)
}

pub(crate) fn traced_with_source<T>(work: impl FnOnce() -> T) -> (T, Trace) {
    let previous = TRACE.with(|trace| trace.replace(Some(Trace::default())));
    let result = work();
    let recorded = TRACE
        .with(|trace| trace.replace(previous))
        .unwrap_or_default();
    (result, recorded)
}
