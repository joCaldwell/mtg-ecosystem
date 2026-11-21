//! Why a rules operation stopped.
//!
//! A rule step commits atomically or not at all. Its failure is one of three
//! kinds, and none of them is a rules outcome (no win, loss or draw):
//! an invariant of the state or of a snapshot does not hold; a declared
//! capacity or counter is exhausted; or a proposed input is not a legal
//! choice in this state. Callers that still report text convert with `From`.
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleError {
    /// A state or snapshot invariant does not hold. The step faults.
    Invariant(String),
    /// A declared capacity or counter is exhausted. The step faults, and the
    /// staged state carries the typed `RuleFault` when one applies.
    Resource(String),
    /// A proposed input is not a legal choice in this state. Nothing changes.
    Illegal(String),
}

impl RuleError {
    pub fn invariant(reason: impl Into<String>) -> Self {
        Self::Invariant(reason.into())
    }
    pub fn resource(reason: impl Into<String>) -> Self {
        Self::Resource(reason.into())
    }
    pub fn illegal(reason: impl Into<String>) -> Self {
        Self::Illegal(reason.into())
    }
    pub fn reason(&self) -> &str {
        match self {
            Self::Invariant(reason) | Self::Resource(reason) | Self::Illegal(reason) => reason,
        }
    }
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}
impl std::error::Error for RuleError {}

// Text raised by a rules routine that is not yet typed is an invariant
// failure: illegal input and exhausted capacity are named at their source.
impl From<String> for RuleError {
    fn from(reason: String) -> Self {
        Self::Invariant(reason)
    }
}
impl From<&str> for RuleError {
    fn from(reason: &str) -> Self {
        Self::Invariant(reason.into())
    }
}
impl From<RuleError> for String {
    fn from(error: RuleError) -> Self {
        match error {
            RuleError::Invariant(reason)
            | RuleError::Resource(reason)
            | RuleError::Illegal(reason) => reason,
        }
    }
}
