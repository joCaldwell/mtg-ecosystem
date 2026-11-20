use super::*;
use serde_json::value::RawValue;

const HISTORICAL: &str =
    include_str!("../../../../tests/fixtures/ramos/2d7e00b6-12f0-4b03-82a6-50e3d1b5395e.json");
const HEADER: &str =
    include_str!("../../../../tests/fixtures/archive-manifest-progenitus-replacement.json");
const ARCHIVE_SHA: &str = "c5fbc3e89be7b8c69fa6b4e2a74deec2509b214a2484e1200f7ef86cf7f64ab2";
const BINDING_SHA: &str = "03b9479bf5ef63e32cd4f3e899ef67f8aff5e62bb973eef83e786f0ad3206f88";

#[derive(Deserialize)]
struct ArchiveOperands {
    source: Box<RawValue>,
    faces: Vec<Face>,
}

#[derive(Deserialize)]
struct Face {
    result: ParseResult,
}

#[derive(Deserialize)]
struct ParseResult {
    abilities: Box<RawValue>,
}

fn operands(record: &str) -> (String, String) {
    let ArchiveOperands { source, faces } = strict_read(record).unwrap();
    let [face] = faces.as_slice() else {
        panic!("expected one face");
    };
    (source.get().into(), face.result.abilities.get().into())
}

// The committed historical source and AST are byte-identical in the new archive.
// The compiler witness independently proves its new binding bytes and lowering.
pub(crate) fn definition() -> Definition {
    let (source_json, abilities_json) = operands(HISTORICAL);
    let source: Value = strict_read(&source_json).unwrap();
    let archive = accepted_manifest(HEADER).unwrap();
    let name = source["name"].as_str().unwrap();
    Definition {
        modal_land: None,
        transform_faces: None,
        source_labels: Some(SourceLabels {
            text: printed_text(name),
            blocks: vec![PROTECTION_TEXT.into(), REPLACEMENT_TEXT.into()],
            statics: vec![1],
            ..SourceLabels::default()
        }),
        x_mana_cost: false,
        oracle_id: source["oracle_id"].as_str().unwrap().into(),
        name: name.into(),
        characteristics: characteristics(),
        intrinsic_zone: None,
        spell: Some(spell()),
        activated: vec![],
        mana_abilities: vec![],
        triggered: vec![],
        static_abilities: vec![static_ability()],
        keywords: vec![KeywordAbility::ProtectionFromEverything],
        capabilities: BTreeSet::from(CAPABILITIES),
        provenance: SourceProvenance {
            printing_id: source["id"].as_str().unwrap().into(),
            source_index: 0,
            source_sha256: sha256(source_json.as_bytes()),
            ast_sha256: sha256(format!("[{abilities_json}]").as_bytes()),
            binding_sha256: BINDING_SHA.into(),
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json,
            abilities_json,
        },
    }
}

#[test]
fn self_graveyard_reveal_shuffle_requires_complete_intrinsic_source() {
    let exact = definition();
    if let Some(path) = std::env::var_os("MTG_SELF_GRAVEYARD_SHUFFLE_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).unwrap();
        assert_eq!(sha256(archive.as_bytes()), ARCHIVE_SHA);
        let mut lines = archive.lines();
        assert_eq!(lines.next(), Some(HEADER.trim_end()));
        let row = lines.next().unwrap();
        assert!(lines.next().is_none());
        let (source, ast) = operands(row);
        assert_eq!(exact.provenance.source_json, source);
        assert_eq!(exact.provenance.abilities_json, ast);
        let record: Value = strict_read(row).unwrap();
        assert_eq!(record["bindingSha256"], BINDING_SHA);
        assert_eq!(record["sourceIndex"], exact.provenance.source_index);
    }
    exact.validate().unwrap();
    assert_eq!(exact.requirements(), BTreeSet::from(CAPABILITIES));
    assert_eq!(
        strict_read::<Definition>(&canonical(&exact).unwrap()).unwrap(),
        exact
    );
    for change in [
        (|d: &mut Definition| d.keywords.clear()) as fn(&mut Definition),
        |d| d.static_abilities.clear(),
        |d| {
            d.keywords.clear();
            d.static_abilities.clear();
        },
        |d| d.static_abilities[0].functioning_zone = FunctioningZone::Battlefield,
        |d| d.static_abilities[0].effect = StaticEffect::EntersTapped,
        |d| d.characteristics.power = Some(9.into()),
        |d| d.spell.as_mut().unwrap().functioning_zone = FunctioningZone::Any,
        |d| d.source_labels.as_mut().unwrap().statics = vec![0],
    ] {
        let mut bad = exact.clone();
        change(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    for cap in [
        Capability::ProtectionFromEverythingV1,
        Capability::SelfGraveyardRevealShuffleV1,
    ] {
        let mut bad = exact.clone();
        bad.capabilities.remove(&cap);
        assert!(bad.validate().is_err());
    }
    let existing: Catalog = strict_read(include_str!(
        "../../../../tests/fixtures/cards-data-d2b/expected-ir.json"
    ))
    .unwrap();
    let mut entering_tapped = existing
        .definitions
        .into_iter()
        .find(|d| d.name == "Moss Diamond")
        .unwrap();
    entering_tapped.validate().unwrap();
    let entry = entering_tapped
        .static_abilities
        .iter_mut()
        .find(|a| a.effect == StaticEffect::EntersTapped)
        .unwrap();
    entry.functioning_zone = FunctioningZone::Any;
    assert!(entering_tapped.validate().is_err());

    let keyword = KeywordAbility::ProtectionFromEverything;
    assert!(!supported_temporary_keywords(&[keyword]));
    let token = CreatureToken {
        additional_types: vec![],
        colors: vec![],
        subtypes: vec!["Spirit".into()],
        power: 1.into(),
        toughness: 1.into(),
        keywords: vec![keyword],
    };
    assert!(token.validate().is_err());
    assert!(!static_creature_modification_valid(
        CardKind::Creature,
        false,
        false,
        &StaticEffect::ModifyCreatures {
            scope: CreatureScope::Controller,
            power: Quantity::zero(),
            toughness: Quantity::zero(),
            keywords: vec![keyword],
            restrictions: vec![],
        },
    ));
}
