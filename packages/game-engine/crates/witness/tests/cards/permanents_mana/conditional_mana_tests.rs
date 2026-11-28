//! Conditional mana remains outside the checked production path (CR 614.15).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_compiler::CompileResult;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

#[test]
fn conditional_mana_reports_the_unavailable_production_path() {
    for record in [
        record!("55eb9310-f24b-410b-850b-a7d0d5117946"), // Leafkin Druid
        record!("5be6f245-31cf-4294-ace0-ca6ce3463fed"), // Raucous Audience
        record!("5f2be3c2-060a-43e1-b63b-9cd3c78ffcb0"), // Ilysian Caryatid
        record!("8a83d284-75a0-4901-b7d9-c4b7586ee327"), // River of Tears
    ] {
        let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, record);
        assert!(
            matches!(&result, CompileResult::Unsupported { path, .. } if path.contains("compose.mana-replacement")),
            "{result:?}"
        );
    }
}
