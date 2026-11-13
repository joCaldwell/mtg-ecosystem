//! Retained counters and hash-consistent malformed-filter probes. Source card
//! facts and Swan Song's Bird follow-up remain unchanged in every mutation.
use super::*;
use serde_json::{Value, json};

const SWAN: &str = include_str!("../../../tests/fixtures/swan-song.json");
const COUNTERSPELL: &str = include_str!("../../../tests/fixtures/counterspell.json");
const TARGET_PATH: &str = "/faces/0/abilities/0/effects/0/steps/0/what";

fn changed_all_of(value: Value) -> String {
    tests::changed_source(SWAN, |record| {
        let result = &mut record["faces"][0]["result"];
        result["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["allOf"] = value;
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        let target = result["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        for node in record["binding"]["nodes"].as_array_mut().unwrap() {
            if node["node"] == "selection" && node["path"] == TARGET_PATH {
                node["expression"] = target.clone();
            }
        }
        for reference in record["binding"]["references"].as_array_mut().unwrap() {
            if reference["path"] == TARGET_PATH {
                reference["expression"] = target.clone();
            }
        }
    })
}

fn assert_complete_mutant(raw: &str) {
    let record: Value = strict_read(raw).unwrap();
    let original: Value = strict_read(SWAN).unwrap();
    assert_eq!(record["source"], original["source"]);
    let result = &record["faces"][0]["result"];
    assert_eq!(result["lines"][0]["ability"], result["abilities"][0]);
    assert_eq!(
        result["abilities"][0]["effects"][1],
        original["faces"][0]["result"]["abilities"][0]["effects"][1]
    );
    let target = &result["abilities"][0]["effects"][0]["steps"][0]["what"];
    for entries in [
        &record["binding"]["nodes"],
        &record["binding"]["references"],
    ] {
        let node = entries
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["path"] == TARGET_PATH)
            .unwrap();
        assert_eq!(&node["expression"], target);
    }
    assert_eq!(
        record["sourceSha256"],
        sha256(canonical(&record["source"]).unwrap().as_bytes())
    );
    assert_eq!(
        record["bindingSha256"],
        sha256(canonical(&record["binding"]).unwrap().as_bytes())
    );
    assert_eq!(
        record["astSha256"],
        sha256(canonical(&json!([result["abilities"]])).unwrap().as_bytes())
    );
}

#[test]
fn retained_counter_sources_preserve_typed_and_omitted_spell_filters() {
    let catalog = embedded_catalog("wipes").unwrap();
    for (raw, typed) in [(SWAN, true), (COUNTERSPELL, false)] {
        let result = compile(fixtures::MANIFEST, raw);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert_eq!(
            catalog
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
        let p = definition.spell.as_ref().unwrap();
        assert_eq!(p.targets[0].filter.is_some(), typed);
        if typed {
            assert_eq!(
                p.targets[0].filter.as_ref().unwrap().types,
                vec![CardKind::Enchantment, CardKind::Instant, CardKind::Sorcery]
            );
            assert_eq!(
                p.instructions.len(),
                2,
                "the complete Bird follow-up remains present"
            );
        } else {
            let record: Value = strict_read(raw).unwrap();
            assert!(record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"].get("allOf").is_none());
        }
    }
}

#[test]
fn rehashed_swan_counter_filter_preserves_its_valid_array_and_complete_program() {
    let original: Value = strict_read(SWAN).unwrap();
    let raw = changed_all_of(
        original["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
            ["allOf"]
            .clone(),
    );
    assert_complete_mutant(&raw);
    let result = compile(fixtures::MANIFEST, &raw);
    let CompileResult::Lowered {
        definition: rebuilt,
    } = result
    else {
        panic!("{result:?}")
    };
    let CompileResult::Lowered {
        definition: original,
    } = compile(fixtures::MANIFEST, SWAN)
    else {
        panic!("complete retained Swan Song")
    };
    assert_eq!(rebuilt.characteristics, original.characteristics);
    assert_eq!(rebuilt.spell, original.spell);
    assert_eq!(rebuilt.capabilities, original.capabilities);
    assert_eq!(
        rebuilt.provenance.source_index,
        original.provenance.source_index
    );
}

#[test]
fn swan_counter_filter_rejects_explicit_null_and_other_nonarrays_without_erasing_restrictions() {
    for all_of in [
        Value::Null,
        json!("not-an-array"),
        json!({}),
        json!(true),
        json!(7),
    ] {
        let raw = changed_all_of(all_of.clone());
        assert_complete_mutant(&raw);
        let result = compile(fixtures::MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { ref path, ref reason }
                if path == "abilities[0].effects[0].steps[0].what.filter" && reason.ends_with("expected_a_sequence")),
            "allOf={all_of}: malformed presence must reject at the typed filter, not become an unrestricted counter: {result:?}"
        );
    }
}
