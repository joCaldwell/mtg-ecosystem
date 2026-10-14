//! Deterministic scenario kernel. See frozen first-scenario brief for admission.
pub const BUILD_ID: &str = env!("MTG_BUILD_ID");
pub const BUILD_PROFILE: &str = env!("MTG_PROFILE");
pub const BUILD_OPT_LEVEL: &str = env!("MTG_OPT_LEVEL");
pub const BUILD_DEBUG: &str = env!("MTG_DEBUG");
pub const BUILD_DEBUG_ASSERTIONS: &str = env!("MTG_DEBUG_ASSERTIONS");
pub const BUILD_TARGET: &str = env!("MTG_TARGET");
pub const BUILD_RUSTFLAGS: &str = env!("MTG_CARGO_ENCODED_RUSTFLAGS");
pub const BUILD_RUSTC: &str = env!("MTG_RUSTC");
mod engine;
pub use engine::*;

#[cfg(test)]
mod build_tests {
    #[test]
    fn recorded_debug_assertions_match_the_target_configuration() {
        assert_eq!(
            super::BUILD_DEBUG_ASSERTIONS,
            cfg!(debug_assertions).to_string()
        );
    }
}
