//! Private complete-source paid witnesses. No reviewed profile/pod admission.
//! Midgame roots below are seedless; randomized multi-card groups require the
//! separate real-setup witnesses, never an injected midgame recovery seed.
use super::*;

#[test]
fn cascade_producer_identity() {
    eprintln!(
        "M3_PRODUCER {}",
        serde_json::json!({"build_id":mtg_core::testkit::build_id(),"opt_level":TEST_OPT_LEVEL,"debug_assertions":cfg!(debug_assertions)})
    );
    assert_eq!(TEST_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}

mod m4a_tests;

mod m4e_tests;
