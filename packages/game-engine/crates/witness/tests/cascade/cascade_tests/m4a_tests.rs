//! Bounded complete Apex source witnesses. Prepared midgame roots remain
//! distinct from real trusted-RNG setup and from source-admitted pods.
use super::*;
#[test]
fn m4a_producer_identity() {
    eprintln!(
        "M4A_PRODUCER {}",
        serde_json::json!({"build_id":mtg_core::testkit::build_id(),"opt_level":TEST_OPT_LEVEL,"debug_assertions":cfg!(debug_assertions)})
    );
    assert_eq!(TEST_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}
