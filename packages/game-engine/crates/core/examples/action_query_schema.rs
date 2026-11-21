//! Explicit native schema/identity export; never implicitly run by SDK/client.
fn main() {
    match std::env::args().nth(1).as_deref() {
        None => {
            let schema = schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<mtg_core::ActionQueryOutcome>();
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        }
        // Examples install no executable identity (test-only sources are outside
        // it), so this reports `BuildIdentity::TEST`; mtg-server reports the
        // installed native identity.
        Some("--identity") => {
            let build = mtg_core::build_identity();
            println!(
                "{}",
                serde_json::json!({
                    "engine_build_id": build.id,
                    "current_ir_version": mtg_contracts::CURRENT_IR_VERSION,
                    "build_profile": build.profile,
                    "opt_level": build.opt_level,
                    "debug_assertions": build.debug_assertions,
                    "target": build.target,
                    "rustc": build.rustc,
                    "page_max": mtg_core::ACTION_PAGE_MAX,
                })
            )
        }
        Some(_) => panic!("usage: action_query_schema [--identity]"),
    }
}
