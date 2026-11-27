//! Single owner of mtg-core wire and persistence format versions. Every
//! producer stamps, and every admission check compares, these constants; tests
//! import them instead of repeating literals (historical-forgery literals stay
//! explicit). Bump a version whenever its serialized shape changes: the
//! `formats_tests` fingerprints fail with the constant to bump.

/// `Game::snapshot` envelope and its complete private `State`.
pub const SNAPSHOT_VERSION: u32 = 131;
/// `Observation`, the permitted per-viewer view and decision.
pub const OBSERVATION_VERSION: u32 = 101;
/// `Game::save_work` envelope for suspended queries.
pub const WORK_VERSION: u32 = 65;
/// `SearchContext`, the permitted information set a search query returns.
pub const SEARCH_CONTEXT_VERSION: u32 = 57;
/// Search sample snapshot envelope.
pub const SAMPLE_SNAPSHOT_VERSION: u32 = 125;
/// Search capture profile.
pub const CAPTURE_VERSION: u32 = 1;
/// Search world proposal.
pub const PROPOSAL_VERSION: u32 = 1;
