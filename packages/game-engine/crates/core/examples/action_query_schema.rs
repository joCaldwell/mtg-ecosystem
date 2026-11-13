//! Explicit native schema/identity export; never implicitly run by SDK/client.
fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {
            let schema = schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<mtg_core::ActionQueryOutcome>();
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        }
        Some("--identity") => println!(
            "{}",
            serde_json::json!({
                "engine_build_id": mtg_core::BUILD_ID,
                "current_ir_version": mtg_contracts::CURRENT_IR_VERSION,
                "build_profile": mtg_core::BUILD_PROFILE,
                "opt_level": mtg_core::BUILD_OPT_LEVEL,
                "debug_assertions": mtg_core::BUILD_DEBUG_ASSERTIONS,
                "target": mtg_core::BUILD_TARGET,
                "rustc": mtg_core::BUILD_RUSTC,
                "page_max": mtg_core::ACTION_PAGE_MAX,
            })
        ),
        Some(_) => panic!("usage: action_query_schema [--identity]"),
    }
}
