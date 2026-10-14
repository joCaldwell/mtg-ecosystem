//! Deterministic scenario kernel. See frozen first-scenario brief for admission.
pub const BUILD_ID: &str = env!("MTG_BUILD_ID");
pub const BUILD_PROFILE: &str = env!("MTG_PROFILE");
pub const BUILD_TARGET: &str = env!("MTG_TARGET");
pub const BUILD_RUSTFLAGS: &str = env!("MTG_CARGO_ENCODED_RUSTFLAGS");
pub const BUILD_RUSTC: &str = env!("MTG_RUSTC");
mod engine;
pub use engine::*;
