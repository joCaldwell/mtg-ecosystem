//! CR 601.2c, 608.2b, 707.9, 707.10 and 707.10f (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(crate) const SELECTION: &str = "line:0:1";
const BASE: &str = "/faces/0/abilities/0";
const STEP: &str = "/faces/0/abilities/0/effects/0/steps/0";

pub(crate) fn candidate(source: &Source) -> bool {
    source
        .oracle_text
        .starts_with("Copy target creature spell you control,")
}

pub(crate) fn validate_binding(
    d: &Definition,
    source: &Value,
    binding: &Value,
) -> Result<(), Failure> {
    // The definition has already proved the whole raw AST, including the
    // target operand and controller, before we derive their expected graph.
    let raw: Value = decode(&d.provenance.abilities_json, "abilities")?;
    let effect = &raw[0]["effects"][0]["steps"][0];
    let what = &effect["what"];
    let you = &effect["who"];
    let what_path = format!("{STEP}/what");
    let mut graph = Bindings::new(&d.oracle_id, source);
    graph.ability(BASE, "spell", "spell-controller");
    let scope = graph.id(BASE, "ability");
    let controller = graph.id(BASE, "you");
    graph.reference(&format!("{STEP}/who"), you, &controller, "109.5");
    graph.reference(
        &format!("{what_path}/filter/control"),
        you,
        &controller,
        "109.5",
    );
    let target = graph.target_with_chooser(&scope, &what_path, what, SELECTION, &controller);
    graph.node(
        STEP,
        "event",
        "event",
        json!({
            "scope":scope,"operation":"copy-stack-object","inputs":[target,controller],
            "availability":"when-performed","rules":["608.2c"]
        }),
    );
    graph.result(STEP, "objects", "new-objects", None);
    graph.validate_ordered_with_features(binding, &[json!({
        "feature":"permanent-spell-copy","path":STEP,
        "message":"Recheck the target, copy its spell decisions and copiable values with the printed exception, and resolve the copy as a token permanent.",
        "rules":["601.2c","608.2b","707.9","707.10","707.10f"]
    })], "requires the exact copy controller, announced target and new spell result")
}

#[test]
fn controlled_creature_spell_copy() {
    use crate::tests::changed_source;

    let historical =
        include_str!("../../../../tests/fixtures/ramos/ece44a82-dcf0-4439-bdd9-a09c99a6f159.json");
    assert!(matches!(
        compile(include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"), historical),
        CompileResult::Unsupported { path, .. } if path == "status"
    ));

    const DOUBLE_MAJOR_ARCHIVE_PROBE: &str = "MTG_CONTROLLED_CREATURE_SPELL_COPY_ARCHIVE_PROBE";
    let Some(path) = std::env::var_os(DOUBLE_MAJOR_ARCHIVE_PROBE) else {
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "fb9290e269061249690c5e128652d51af00d3fe13af4cb363f342f8883fe5823"
    );
    let mut lines = archive.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(
        header,
        include_str!(
            "../../../../tests/fixtures/archive-manifest-controlled-creature-spell-copy.json"
        )
        .trim_end()
    );
    let accepted = accepted_manifest(header).unwrap();
    assert_eq!(
        accepted.manifest_sha256,
        "81dac19d638ebe74cfaaf03bc0efc701331a93961fb742c4e3fb330f7148db6d"
    );
    let row_hash = sha256(row.as_bytes());
    assert_eq!(
        row_hash,
        "2fb5407e0742bb508fc968c9254d3edbcb4e67e1ff773c43ad03a4acc339ce84"
    );
    assert_eq!(accepted.record_sha256, BTreeSet::from([row_hash]));
    let original: Record = decode(row, "record").unwrap();
    let result: ParseResult = decode(original.faces[0].result.get(), "result").unwrap();
    let compiled = compile(header, row);
    let CompileResult::Lowered { definition: d } = compiled else {
        panic!("{compiled:?}")
    };
    assert_eq!(
        d.provenance.source_sha256,
        "398a18e953f6d8cc648d5b6c68d4b803d482af56f336efc8bdbd9949ae56c430"
    );
    assert_eq!(d.provenance.source_json, original.source.get());
    assert_eq!(d.provenance.abilities_json, result.abilities.get());
    assert_eq!(d.provenance.binding_sha256, original.binding_sha256);
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(
        spell.targets,
        [TargetSpec {
            slot: 0,
            selection_id: SELECTION.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::ControlledCreatureSpell,
            filter: None,
        }]
    );
    assert_eq!(
        spell.instructions,
        [Instruction::CopyControlledCreatureSpell {
            target_slot: 0,
            source_block: 0,
        }]
    );
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::StackSpellTargetV1,
            Capability::ControlledCreatureSpellCopyV1,
        ])
    );

    let binding_mutations: &[fn(&mut Value)] = &[
        |g| g["fullyBound"] = json!(true),
        |g| g["engineFeatures"] = json!([]),
        |g| g["nodes"][4]["chooser"] = g["nodes"][4]["scope"].clone(),
        |g| g["nodes"][5]["inputs"].as_array_mut().unwrap().reverse(),
        |g| g["nodes"][6]["identity"] = json!("same-objects"),
        |g| g["references"][0]["target"] = json!("wrong controller"),
        |g| g["references"][1]["target"] = json!("wrong control selector"),
        |g| g["references"][2]["target"] = json!("wrong target"),
    ];
    for (index, mutate) in binding_mutations.iter().enumerate() {
        let bad = changed_source(row, |r| mutate(&mut r["binding"]));
        assert!(
            matches!(compile(header, &bad), CompileResult::Unsupported {path, ..} if path == "binding"),
            "binding mutation {index}"
        );
    }

    let ast_mutations: &[fn(&mut Value)] = &[
        |s| s["count"]["value"] = json!(2),
        |s| s["what"]["count"]["value"] = json!(2),
        |s| s["what"]["filter"]["types"] = json!(["artifact"]),
        |s| s["what"]["filter"]["control"] = json!("opponent"),
        |s| s["what"]["selectionId"] = json!("line:0:2"),
        |s| s["who"] = json!({"player":"opponent"}),
        |s| s["copyExceptions"] = json!([]),
        |s| s["chooseNewTargets"] = json!(true),
    ];
    for (index, mutate) in ast_mutations.iter().enumerate() {
        let bad = changed_source(row, |r| {
            let result = &mut r["faces"][0]["result"];
            let ability = &mut result["abilities"][0];
            mutate(&mut ability["effects"][0]["steps"][0]);
            result["lines"][0]["ability"] = ability.clone();
        });
        assert!(
            !matches!(compile(header, &bad), CompileResult::Lowered { .. }),
            "AST mutation {index}"
        );
    }
}
