//! Exact archive-backed contract witnesses. The compiler tests separately
//! lower these records and check the source bindings.
use super::*;
use serde_json::{Value, value::RawValue};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    source_index: u32,
    source_sha256: String,
    ast_sha256: String,
    binding_sha256: String,
    source: Box<RawValue>,
    faces: [Face; 1],
}

#[derive(Deserialize)]
struct Face {
    result: Parse,
}

#[derive(Deserialize)]
struct Parse {
    abilities: Box<RawValue>,
}

fn from_source(mut d: Definition, record: &str) -> Definition {
    let record: Record = strict_read(record).unwrap();
    let source: Value = strict_read(record.source.get()).unwrap();
    let archive = accepted_manifest(include_str!(
        "../../../tests/fixtures/archive-manifest-current.json"
    ))
    .unwrap();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.name = source["name"].as_str().unwrap().into();
    d.source_labels = None;
    d.characteristics.color_identity =
        serde_json::from_value(source["color_identity"].clone()).unwrap();
    d.provenance = SourceProvenance {
        printing_id: source["id"].as_str().unwrap().into(),
        source_index: record.source_index,
        source_sha256: record.source_sha256,
        ast_sha256: record.ast_sha256,
        binding_sha256: record.binding_sha256,
        archive_manifest_sha256: archive.manifest_sha256.clone(),
        parser_sha256: archive.parser_source_sha256.clone(),
        source_json: record.source.get().into(),
        abilities_json: record.faces[0].result.abilities.get().into(),
    };
    d.capabilities = d.requirements();
    d
}

pub(super) fn definitions() -> [Definition; 3] {
    let mut checkland = listed_mana_choice::witness();
    checkland.static_abilities = vec![StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessControllerLandTypes {
            subtypes: [BasicLandType::Island, BasicLandType::Mountain],
        },
    }];
    checkland.mana_abilities[0].production = ManaProduction::Choice {
        colors: vec![Color::U, Color::R],
    };
    checkland.mana_abilities[0].source_block = 1;

    let mut fetch = listed_mana_choice::witness();
    fetch.mana_abilities.clear();
    fetch.activated = vec![ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                tap: true,
                sacrifice: true,
                mana: None,
                remove_source_plus_one_counters: None,
            },
        },
        program: basic_land_fetch_program(true),
    }];

    let mut mana = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Sol Ring")
    .unwrap();
    mana.characteristics.mana_cost.generic = 5.into();
    mana.mana_abilities[0].production = ManaProduction::AnyOneColor {
        amount: SingleColorManaAmount::Fixed { quantity: 3.into() },
    };

    [
        from_source(
            checkland,
            include_str!("../../../tests/fixtures/6a6c5e17-6465-4a1f-9d63-8a3ce2edc522.json"),
        ),
        from_source(
            fetch,
            include_str!("../../../tests/fixtures/a75445d3-1303-4bb5-89ad-26ea93fecd48.json"),
        ),
        from_source(
            mana,
            include_str!("../../../tests/fixtures/9a02a9a7-39d9-4763-85d3-747a0540b60b.json"),
        ),
    ]
}

#[test]
fn wave1_mana_witnesses_cover_each_capability_in_schema_one() {
    for (definition, capability) in definitions().into_iter().zip([
        Capability::ConditionalLandTypeEntryV1,
        Capability::BasicLandFetchV1,
        Capability::SameColorManaV1,
    ]) {
        definition.validate().unwrap();
        assert!(definition.capabilities.contains(&capability));
        let roundtrip: Definition =
            strict_read(&serde_json::to_string(&definition).unwrap()).unwrap();
        assert_eq!(roundtrip, definition);
    }
}
