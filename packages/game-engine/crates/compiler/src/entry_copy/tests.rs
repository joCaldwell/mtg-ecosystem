use super::*;
use crate::tests::changed_source;

#[test]
fn protege_entry_copy_archive_probe() {
    let historical_header =
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let historical =
        include_str!("../../../../tests/fixtures/ramos/cf127842-5fb1-44df-8d91-8cc0f48c559a.json");
    assert!(matches!(
        compile(historical_header, historical),
        CompileResult::Unsupported { path, .. } if path == "binding"
    ));

    let Some(path) = std::env::var_os("MTG_PROTEGE_ENTRY_COPY_ARCHIVE_PROBE") else {
        eprintln!("Sakashima's Protege private archive probe was not supplied");
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "e8b63300e5669872bf113bb6be2cbf6a4856b4ce7281c76bfdc8a98680a6d78b"
    );
    let lines: Vec<_> = archive.lines().collect();
    let [header, row] = lines.as_slice() else {
        panic!("expected the pinned header and a single parsed card");
    };
    assert_eq!(
        *header,
        include_str!("../../../../tests/fixtures/archive-manifest-protege-entry-copy.json")
            .trim_end()
    );
    let accepted = accepted_manifest(header).unwrap();
    assert_eq!(
        accepted.record_sha256,
        BTreeSet::from([sha256(row.as_bytes())])
    );
    let old: Record = decode(historical, "historical").unwrap();
    let parsed: Record = decode(row, "record").unwrap();
    assert_eq!(parsed.source.get(), old.source.get());
    assert_eq!(parsed.ast_sha256, old.ast_sha256);
    assert_ne!(parsed.binding_sha256, old.binding_sha256);
    let result = compile(header, row);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}");
    };
    assert_eq!(d.provenance.source_json, parsed.source.get());
    assert_eq!(d.provenance.binding_sha256, parsed.binding_sha256);
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(d.keywords, [KeywordAbility::Flash]);
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(spell.resolution, SpellResolution::Permanent);
    assert_eq!(spell.timing, SpellTiming::Instant);
    assert!(spell.targets.is_empty());
    let [trigger] = d.triggered.as_slice() else {
        panic!("expected a single intrinsic cascade");
    };
    assert_eq!(trigger.condition, TriggerCondition::SelfSpellCast);
    assert_eq!(trigger.program, canonical_cascade_program(1));
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn,
        }]
    );
    let labels = d.source_labels.as_ref().unwrap();
    assert_eq!(labels.triggered, [1]);
    assert_eq!(labels.statics, [2]);
    assert!(
        d.capabilities
            .contains(&Capability::EnterAsCopyOfPermanentEnteredThisTurnV1)
    );

    for (pointer, value) in [
        ("/nodes/4/role", json!("trigger-controller")),
        ("/nodes/8/role", json!("prospective-caster")),
        ("/nodes/11/role", json!("owner")),
        ("/nodes/13/selection", json!("target")),
        ("/nodes/13/count", json!("one")),
        ("/nodes/13/historyIdentity", json!("physical-card")),
        ("/nodes/14/values", json!("all-live-characteristics")),
        ("/references/2/binding", json!("unresolved")),
        ("/fullyBound", json!(true)),
    ] {
        let bad = changed_source(row, |r| {
            *r["binding"].pointer_mut(pointer).unwrap() = value;
        });
        assert!(
            matches!(compile(header, &bad), CompileResult::Unsupported { path, .. } if path == "binding"),
            "{pointer}"
        );
    }
    let reordered = changed_source(row, |r| {
        r["binding"]["nodes"].as_array_mut().unwrap().swap(12, 13);
    });
    assert!(matches!(
        compile(header, &reordered),
        CompileResult::Unsupported { path, .. } if path == "binding"
    ));
    let bad = changed_source(row, |r| {
        let result = &mut r["faces"][0]["result"];
        result["abilities"][2]["effect"]["model"]["when"] = json!("after-entry");
        result["lines"][2]["ability"] = result["abilities"][2].clone();
    });
    assert!(matches!(
        compile(header, &bad),
        CompileResult::Unsupported { path, .. } if path == "definition"
    ));

    let unregistered = format!("{{ {}", row.strip_prefix('{').unwrap());
    assert!(matches!(
        compile(header, &unregistered),
        CompileResult::Invalid { path, .. } if path == "record"
    ));
}
