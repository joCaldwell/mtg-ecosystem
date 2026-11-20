use super::*;
use serde_json::value::RawValue;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveRow {
    source_index: u32,
    oracle_identity: String,
    source_sha256: String,
    ast_sha256: String,
    binding_sha256: String,
    source: Box<RawValue>,
    faces: Vec<Face>,
    binding: Box<RawValue>,
}

#[derive(Deserialize)]
struct Face {
    result: FaceResult,
}

#[derive(Deserialize)]
struct FaceResult {
    abilities: Box<RawValue>,
}

fn definition_from_row(row: ArchiveRow) -> Definition {
    let [face] = row.faces.as_slice() else {
        panic!("expected the single printed face");
    };
    let source: Value = strict_read(row.source.get()).unwrap();
    Definition {
        modal_land: None,
        source_labels: Some(labels()),
        x_mana_cost: false,
        oracle_id: row.oracle_identity,
        name: source["name"].as_str().unwrap().into(),
        characteristics: characteristics(),
        intrinsic_zone: None,
        spell: Some(permanent_spell()),
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![cascade_trigger()],
        static_abilities: vec![entry_static()],
        keywords: vec![KeywordAbility::Flash],
        capabilities: CAPABILITIES.into_iter().collect(),
        provenance: SourceProvenance {
            printing_id: source["id"].as_str().unwrap().into(),
            source_index: row.source_index,
            source_sha256: row.source_sha256,
            ast_sha256: row.ast_sha256,
            binding_sha256: row.binding_sha256,
            archive_manifest_sha256: MANIFEST_SHA.into(),
            parser_sha256: PARSER_SHA.into(),
            source_json: row.source.get().into(),
            abilities_json: face.result.abilities.get().into(),
        },
    }
}

pub(in crate::entry_copy) fn registry_witness() -> Definition {
    // The historical fixture has the same source and AST. Use the
    // registered reparse pins for this executable-shape registry check.
    let mut row: ArchiveRow = strict_read(include_str!(
        "../../../../tests/fixtures/ramos/cf127842-5fb1-44df-8d91-8cc0f48c559a.json"
    ))
    .unwrap();
    assert_eq!(row.source_sha256, SOURCE_SHA);
    assert_eq!(row.ast_sha256, AST_SHA);
    row.source_index = 0;
    row.binding_sha256 = BINDING_SHA.into();
    definition_from_row(row)
}

#[test]
fn protege_entry_copy_archive_probe() {
    let Some(path) = std::env::var_os("MTG_PROTEGE_ENTRY_COPY_ARCHIVE_PROBE") else {
        eprintln!("Sakashima's Protege private archive probe was not supplied");
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    let lines: Vec<_> = archive.lines().collect();
    let [header, record] = lines.as_slice() else {
        panic!("expected the pinned header and a single parsed card");
    };
    let accepted = accepted_manifest(header).unwrap();
    assert_eq!(accepted.manifest_sha256, MANIFEST_SHA);
    assert_eq!(accepted.file_sha256, Some(sha256(archive.as_bytes())));
    assert_eq!(
        accepted.record_sha256,
        BTreeSet::from([sha256(record.as_bytes())])
    );
    let row: ArchiveRow = strict_read(record).unwrap();
    assert_eq!(row.binding_sha256, sha256(row.binding.get().as_bytes()));
    let d = definition_from_row(row);
    assert_eq!(d.requirements(), d.capabilities);
    d.validate().unwrap();
    let decoded: Definition = strict_read(&canonical(&d).unwrap()).unwrap();
    assert_eq!(decoded, d);
    decoded.validate().unwrap();
    assert_eq!(
        json!(EFFECT),
        "enter_as_copy_of_permanent_entered_this_turn"
    );
    assert_eq!(
        json!(CAPABILITY),
        "enter_as_copy_of_permanent_entered_this_turn_v1"
    );
    assert!(
        serde_json::from_value::<StaticEffect>(
            json!({"enter_as_copy_of_permanent_entered_this_turn":{"target":true}})
        )
        .is_err()
    );

    let invalid: &[fn(&mut Definition)] = &[
        |d| d.static_abilities.clear(),
        |d| d.static_abilities[0].functioning_zone = FunctioningZone::Stack,
        |d| d.static_abilities.push(entry_static()),
        |d| d.static_abilities.push(opening_placement_ability()),
        |d| d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase,
        |d| d.keywords.clear(),
        |d| d.triggered.clear(),
        |d| d.triggered.push(cascade_trigger()),
        |d| d.triggered[0].program = canonical_cascade_program(0),
        |d| {
            d.characteristics
                .additional_types
                .push(CardKind::Enchantment)
        },
        |d| {
            d.characteristics.hybrid_mana.push(ColorHybridSymbol {
                colors: [Color::U, Color::B].map(HybridPayment::Mana),
            })
        },
        |d| d.provenance.binding_sha256 = "0".repeat(64),
        |d| d.source_labels.as_mut().unwrap().statics = vec![1],
    ];
    for (index, mutate) in invalid.iter().enumerate() {
        let mut bad = d.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err(), "mutation {index}");
    }
    let mut bad = d;
    bad.capabilities.remove(&CAPABILITY);
    assert!(bad.validate().is_err());
}
