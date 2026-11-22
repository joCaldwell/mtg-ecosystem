//! Exact frozen sources in private seedless paid roots; no pod admission.
use super::*;
#[test]
fn m4e_producer_identity() {
    eprintln!(
        "M4E_PRODUCER {}",
        serde_json::json!({"build_id":mtg_core::testkit::build_id(),"opt_level":TEST_OPT_LEVEL,"debug_assertions":cfg!(debug_assertions)})
    );
    assert_eq!(TEST_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}
