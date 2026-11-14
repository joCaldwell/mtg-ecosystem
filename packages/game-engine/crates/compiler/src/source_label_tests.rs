use super::*;
#[test]
fn source_labels_bind_complete_frozen_blocks_abilities_and_modes() {
    for raw in [
        include_str!("../../../tests/fixtures/healing-salve.json"),
        include_str!("../../../tests/fixtures/prodigal-sorcerer.json"),
    ] {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("frozen source rejected");
        };
        let record: serde_json::Value = serde_json::from_str(raw).unwrap();
        let labels = definition.source_labels.as_ref().unwrap();
        assert_eq!(
            labels.text,
            record["source"]["oracle_text"].as_str().unwrap()
        );
        assert_eq!(
            labels.blocks,
            record["faces"][0]["result"]["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["text"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(labels.activated.len(), definition.activated.len());
        assert_eq!(labels.triggered.len(), definition.triggered.len());
        assert_eq!(labels.mana.len(), definition.mana_abilities.len());
        assert_eq!(
            labels.spell_modes.len(),
            definition.spell.as_ref().unwrap().modes.len()
        );
        let mut altered = definition.clone();
        altered.source_labels.as_mut().unwrap().text.push('!');
        assert_eq!(
            altered.validate().unwrap_err(),
            "invalid source label provenance"
        );
    }
}
