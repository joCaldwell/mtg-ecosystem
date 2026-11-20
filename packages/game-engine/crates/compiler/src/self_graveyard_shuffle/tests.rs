use super::*;
use crate::tests::changed_source;

const HISTORICAL: &str =
    include_str!("../../../../tests/fixtures/ramos/2d7e00b6-12f0-4b03-82a6-50e3d1b5395e.json");
const HISTORICAL_HEADER: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const HEADER: &str =
    include_str!("../../../../tests/fixtures/archive-manifest-progenitus-replacement.json");

#[test]
fn self_graveyard_reveal_shuffle_exact_archive() {
    assert_eq!(
        sha256(HISTORICAL.trim_end().as_bytes()),
        "7198721dd72cb5c1acf6181a87eaaf40101b9705a5f6e24ff5506934216c9aca"
    );
    assert!(matches!(
        compile(HISTORICAL_HEADER, HISTORICAL),
        CompileResult::Unsupported { path, .. } if path == "binding"
    ));

    let Some(path) = std::env::var_os("MTG_SELF_GRAVEYARD_SHUFFLE_ARCHIVE_PROBE") else {
        eprintln!("Self graveyard shuffle private archive probe was not supplied");
        return;
    };
    let archive = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(archive.as_bytes()),
        "c5fbc3e89be7b8c69fa6b4e2a74deec2509b214a2484e1200f7ef86cf7f64ab2"
    );
    let mut lines = archive.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    assert!(lines.next().is_none());
    assert_eq!(header, HEADER.trim_end());
    let accepted = accepted_manifest(header).unwrap();
    assert_eq!(
        accepted.manifest_sha256,
        "2e336b680c306296fc5d57d75c5b49a5f3e2b1de36cde7c6c7f2d6a7309ffe4d"
    );
    let row_hash = sha256(row.as_bytes());
    assert_eq!(
        row_hash,
        "5a438fee440e791120c3dad3722c39d999ba1046e9915c0f49ff82bd4177f5cf"
    );
    assert_eq!(accepted.record_sha256, BTreeSet::from([row_hash]));
    let historical: Record = decode(HISTORICAL, "historical").unwrap();
    let parsed: Record = decode(row, "record").unwrap();
    assert_eq!(historical.source.get(), parsed.source.get());
    assert_eq!(historical.ast_sha256, parsed.ast_sha256);
    assert_eq!(
        parsed.source_sha256,
        "a5963cce5be340c2eeb5ff539fc3edfca834414cdeded0b13339fa91598bc9de"
    );
    assert_eq!(
        parsed.binding_sha256,
        "03b9479bf5ef63e32cd4f3e899ef67f8aff5e62bb973eef83e786f0ad3206f88"
    );
    let graph: Value = decode(parsed.binding.get(), "binding").unwrap();
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 11);
    assert_eq!(graph["references"].as_array().unwrap().len(), 6);

    let compiled = compile(header, row);
    let CompileResult::Lowered { definition: d } = compiled else {
        panic!("{compiled:?}");
    };
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(
        d.characteristics.mana_cost,
        mana_cost("{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}").unwrap()
    );
    assert_eq!(d.keywords, [KeywordAbility::ProtectionFromEverything]);
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Any,
            effect: StaticEffect::SelfGraveyardRevealShuffle,
        }]
    );
    assert_eq!(d.source_labels.as_ref().unwrap().statics, [1]);
    assert_eq!(d.provenance.source_json, parsed.source.get());
    assert_eq!(d.provenance.binding_sha256, parsed.binding_sha256);
    use Capability::*;
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            BaseCreatureV1,
            CastCreatureV1,
            CommanderIdentityV1,
            ProtectionFromEverythingV1,
            SelfGraveyardRevealShuffleV1,
        ])
    );

    for index in 0..6 {
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
    for collection in ["nodes", "references", "engineFeatures"] {
        let bad = changed_source(row, |r| {
            r["binding"][collection].as_array_mut().unwrap().swap(0, 1);
        });
        assert!(
            matches!(
                compile(header, &bad),
                CompileResult::Unsupported { path, .. } if path == "binding"
            ),
            "{collection}"
        );
    }
    for (index, pointer, value) in [
        (0, "/keywords/0/from/0/scope", json!("each-color")),
        (
            1,
            "/effect/replacement/event/from",
            json!({"zone":"battlefield"}),
        ),
        (
            1,
            "/effect/replacement/replace/effects/0/steps/0/what/role",
            json!("source"),
        ),
        (
            1,
            "/effect/replacement/replace/effects/0/steps/1/library/owner",
            json!("you"),
        ),
    ] {
        let bad = changed_source(row, |r| {
            let result = &mut r["faces"][0]["result"];
            let ability = &mut result["abilities"][index];
            *ability.pointer_mut(pointer).unwrap() = value;
            result["lines"][index]["ability"] = ability.clone();
        });
        assert!(
            matches!(
                compile(header, &bad),
                CompileResult::Unsupported { path, .. } if path == "abilities.self_graveyard_shuffle"
            ),
            "{pointer}"
        );
    }
    let bad = changed_source(row, |r| r["source"]["power"] = json!("9"));
    assert!(matches!(
        compile(header, &bad),
        CompileResult::Unsupported { path, .. } if path == "abilities.self_graveyard_shuffle"
    ));
    let bad = changed_source(row, |r| {
        r["faces"][0]["result"]["lines"][1]["ability"]["effect"]["replacement"]["occurrence"] =
            json!("first");
    });
    assert!(matches!(
        compile(header, &bad),
        CompileResult::Invalid { path, .. } if path == "lines"
    ));
    assert!(matches!(
        compile(HISTORICAL_HEADER, row),
        CompileResult::Unsupported { path, .. } if path == "record"
    ));
}
