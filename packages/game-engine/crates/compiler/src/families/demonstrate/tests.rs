use super::*;
use crate::tests::changed_source;

#[test]
fn creative_technique() {
    let historical = include_str!(
        "../../../../../tests/fixtures/ramos/c336b735-3e6b-471e-be7a-84d7402ae2f5.json"
    );
    let historical_header =
        include_str!("../../../../../tests/fixtures/ramos/archive-manifest-current.json");
    assert!(matches!(
        compile(historical_header, historical),
        CompileResult::Unsupported { path, .. } if path == "status"
    ));

    let Some(path) = std::env::var_os("MTG_CREATIVE_TECHNIQUE_ARCHIVE_PROBE") else {
        eprintln!("Creative Technique private archive probe was not supplied");
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "86c721c13151e4ed8f82be4154f0202e8fbd337a3454902f45c33979bc7f7a9e"
    );
    let lines: Vec<_> = archive.lines().collect();
    let [header, row] = lines.as_slice() else {
        panic!("expected the pinned header and a single parsed card");
    };
    assert_eq!(
        *header,
        include_str!("../../../../../tests/fixtures/archive-manifest-creative-technique.json")
            .trim_end()
    );
    let accepted = accepted_manifest(header).unwrap();
    assert_eq!(
        accepted.manifest_sha256,
        "20403105680031ae331c8d8291451ac3194dc11b58956d927fb71776c3babb9e"
    );
    let row_hash = sha256(row.as_bytes());
    assert_eq!(
        row_hash,
        "e7f74eeafcd6a4ba0a23cdeb1f02cc3b6bbeb643b83bdef426f949904639f49b"
    );
    assert_eq!(accepted.record_sha256, BTreeSet::from([row_hash]));
    let historic: Record = decode(historical, "historical").unwrap();
    let parsed: Record = decode(row, "record").unwrap();
    assert_eq!(historic.source.get(), parsed.source.get());
    assert_eq!(historic.source_sha256, parsed.source_sha256);
    assert_eq!(
        parsed.source_sha256,
        "253fa36baba9d86e365ba64a543b0a10d121e0e0759c07f4b73d36970d2e7d34"
    );
    let graph: Value = decode(parsed.binding.get(), "binding").unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 18);
    assert_eq!(graph["references"].as_array().unwrap().len(), 11);
    let features = graph["engineFeatures"].as_array().unwrap();
    assert_eq!(features.len(), 2);
    assert_eq!(features[0]["feature"], "demonstrate");
    assert_eq!(features[1]["feature"], "reveal-and-cast");

    let compiled = compile(header, row);
    let CompileResult::Lowered { definition: d } = compiled else {
        panic!("{compiled:?}");
    };
    assert_eq!(d.characteristics.kind, CardKind::Sorcery);
    assert_eq!(d.characteristics.mana_cost, mana_cost("{4}{R}").unwrap());
    assert_eq!(d.characteristics.color_identity, [Color::R]);
    assert_eq!(d.provenance.source_json, parsed.source.get());
    assert_eq!(d.provenance.binding_sha256, parsed.binding_sha256);
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(spell.timing, SpellTiming::MainPhase);
    assert_eq!(
        spell.instructions,
        [Instruction::RevealAndCast { source_block: 1 }]
    );
    assert!(spell.targets.is_empty());
    let [trigger] = d.triggered.as_slice() else {
        panic!("expected independently indexed Demonstrate trigger");
    };
    assert_eq!(trigger.condition, TriggerCondition::SelfSpellCast);
    assert_eq!(trigger.functioning_zone, FunctioningZone::Stack);
    assert_eq!(trigger.program.timing, SpellTiming::Instant);
    assert_eq!(
        trigger.program.instructions,
        [Instruction::Demonstrate { source_block: 0 }]
    );
    assert!(trigger.program.targets.is_empty());
    assert_eq!(d.source_labels.as_ref().unwrap().triggered, [0]);
    use Capability::*;
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastSorceryV1,
            TriggeredAbilityV1,
            CastTriggerV1,
            ResolutionCastV1,
            RandomBottomV1,
            RevealAndCastV1,
            DemonstrateV1,
        ])
    );

    for index in 0..11 {
        let bad = changed_source(row, |r| {
            r["binding"]["references"][index]["target"] = json!("wrong binding");
        });
        assert!(
            matches!(
                compile(header, &bad),
                CompileResult::Unsupported { path, .. } if path == "binding"
            ),
            "reference {index}"
        );
    }
    for (pointer, value) in [
        ("/nodes/4/role", json!("spell-controller")),
        ("/nodes/8/operation", json!("scan-library")),
        ("/nodes/11/subsetOf", json!("all library cards")),
        ("/nodes/13/identity", json!("same-objects")),
        ("/nodes/14/excluding", json!("all library cards")),
        ("/nodes/17/requiresResult", Value::Null),
        ("/engineFeatures/0/feature", json!("cascade")),
        ("/engineFeatures/1/rules", json!([])),
        ("/fullyBound", json!(true)),
    ] {
        let bad = changed_source(row, |r| {
            *r["binding"].pointer_mut(pointer).unwrap() = value;
        });
        assert!(
            matches!(
                compile(header, &bad),
                CompileResult::Unsupported { path, .. } if path == "binding"
            ),
            "{pointer}"
        );
    }
    for (pointer, value) in [
        ("/effects/0/steps/0/library/owner", json!("opponent")),
        ("/effects/0/steps/1/until/count/value", json!(2)),
        ("/effects/1/steps/0/what/ref", json!("them")),
        ("/effects/1/steps/1/order", json!("any")),
        (
            "/effects/2/steps/0/instruction/what/relation/scope",
            json!("global"),
        ),
        ("/effects/2/steps/0/optional", json!(false)),
    ] {
        let bad = changed_source(row, |r| {
            let result = &mut r["faces"][0]["result"];
            let spell = &mut result["abilities"][1];
            *spell.pointer_mut(pointer).unwrap() = value;
            result["lines"][1]["ability"] = spell.clone();
        });
        assert!(
            matches!(
                compile(header, &bad),
                CompileResult::Unsupported { path, .. } if path == "definition"
            ),
            "{pointer}"
        );
    }
}
