//! Explicit native action-query schema export; never implicitly run by SDK/client.
//! The native build identity belongs to executables (`mtg-server native-info`).
fn main() {
    assert!(
        std::env::args().nth(1).is_none(),
        "usage: action_query_schema"
    );
    let schema = schemars::generate::SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<mtg_core::ActionQueryOutcome>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
