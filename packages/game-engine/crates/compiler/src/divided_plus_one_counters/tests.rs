use super::*;
use crate::tests::changed_source;
use std::io::{BufRead, BufReader};

const R4: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
const ABZAN_CHARM: &str = include_str!("../../../../tests/fixtures/phase1-r4/abzan-charm.json");
const DIGEST: &str = "c4928e278408c6fbd431722629153931a67de14a7cbd2889512235a1a8572c67";
const CURRENT: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
/// The revision-17 Ramos row, archived before the binder could announce the
/// division: its graph and missing resolution leave the operand unresolved.
const LEGACY_ROW: &str =
    include_str!("../../../../tests/fixtures/ramos/4137a22c-f793-4e27-a9b2-72740a6e2121.json");

fn compile_r4(raw: &str) -> CompileResult {
    compile(R4, raw)
}

fn check(raw: &str) -> Box<Definition> {
    let definition = match compile_r4(raw) {
        CompileResult::Lowered { definition } => definition,
        other => panic!("expected complete divided-counter modal instant: {other:?}"),
    };
    let p = definition.spell.as_ref().unwrap();
    let [exile, draw_loss, distribute] = p.modes.as_slice() else {
        panic!("expected three modes")
    };
    let exile_target = &exile.program.targets[0];
    assert_eq!(exile_target.selection_id, "line:1:2");
    assert_eq!(
        exile_target.filter.as_ref().unwrap().power,
        Some(CreaturePowerFilter::AtLeast { value: 3.into() })
    );
    assert_eq!(
        exile.program.instructions,
        [Instruction::ExilePermanent {
            target_slot: 0,
            source_block: 0
        }]
    );
    assert!(draw_loss.program.targets.is_empty());
    let draw = Instruction::DrawCards {
        recipient: None,
        count: 1.into(),
        source_block: 0,
    };
    assert_eq!(
        draw_loss.program.instructions,
        [
            draw.clone(),
            draw,
            Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(2)
                },
                0
            )
        ]
    );
    assert_eq!(
        distribute.program.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:3:13".into(),
            filter: None,
            stage: EvaluationStage::Announcement,
            chooser: Chooser::Controller,
            domain: TargetDomain::BattlefieldCreature
        }]
    );
    assert_eq!(
        distribute.program.instructions,
        [Instruction::DistributePlusOneCounters {
            count: 2.into(),
            minimum_targets: 1,
            maximum_targets: 2,
            source_block: 0,
        }]
    );
    let labels = definition.source_labels.as_ref().unwrap();
    assert_eq!(labels.text, TEXT);
    assert_eq!(labels.blocks, [TEXT]);
    assert_eq!(labels.spell_modes, TEXT.lines().skip(1).collect::<Vec<_>>());
    use Capability::*;
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastInstantV1,
            ModalChoiceV1,
            CreatureTargetV1,
            PermanentRemovalV1,
            PowerFilteredRemovalV1,
            ControllerDrawV1,
            LifeLossV1,
            DividedPlusOneCountersV1,
            PlusOneCountersV1
        ])
    );
    definition
}

#[test]
fn divided_plus_one_counters_complete_r4_abzan_charm() {
    let raw = ABZAN_CHARM.trim_end_matches('\n');
    assert_eq!(sha256(raw.as_bytes()), DIGEST);
    let d = check(raw);
    assert_eq!(d.name, "Abzan Charm");
    assert_eq!(d.oracle_id, "4137a22c-f793-4e27-a9b2-72740a6e2121");
    assert_eq!(d.provenance.source_index, 9808);
    assert_eq!(
        d.provenance.archive_manifest_sha256,
        accepted_manifest(R4).unwrap().manifest_sha256
    );
}

#[test]
fn divided_plus_one_counters_rejects_an_unresolved_division() {
    // No diagnostic graph substitutes for the resolution (CR 601.2d).
    assert!(matches!(
        compile(CURRENT, LEGACY_ROW),
        CompileResult::Unsupported { path, .. } if path == "resolution"
    ));
}

#[test]
fn divided_plus_one_counters_complete_source_only() {
    let raw = ABZAN_CHARM.trim_end_matches('\n');
    let step = "/faces/0/result/abilities/0/effects/0/steps/0";
    for (field, value) in [
        ("/count/exactly", json!(2)),
        (
            "/options/0/effects/0/steps/0/what/filter/properties/0/comparison/op",
            json!("le"),
        ),
        (
            "/options/1/effects/0/steps/1/who/player",
            json!("target-player"),
        ),
        ("/options/2/effects/0/steps/0/counter/type", json!("-1/-1")),
        ("/options/2/effects/0/steps/0/count/value", json!(3)),
        ("/options/2/effects/0/steps/0/among/count/minimum", json!(0)),
        (
            "/options/2/effects/0/steps/0/among/count/maximum/value",
            json!(3),
        ),
        (
            "/options/2/effects/0/steps/0/among/selectionId",
            json!(EXILE),
        ),
    ] {
        let bad = changed_source(raw, |r| {
            *r.pointer_mut(&format!("{step}{field}")).unwrap() = value;
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile_r4(&bad), CompileResult::Lowered { .. }),
            "{field}"
        );
    }
    for change in 0..8 {
        let bad = changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            let modal = &mut result["abilities"][0]["effects"][0]["steps"][0];
            match change {
                0 => modal["options"].as_array_mut().unwrap().swap(0, 2),
                1 => modal["options"][2]["effects"][0]["steps"][0]["optional"] = Value::Null,
                2 => modal["options"][2]["effects"][0]["bindings"] = json!([]),
                3 => modal["count"]["upTo"] = json!(1),
                4 => {
                    modal["options"].as_array_mut().unwrap().pop();
                }
                5 => r["source"]["mana_cost"] = json!("{X}{G}"),
                6 => r["source"]["type_line"] = json!("Sorcery"),
                _ => {
                    let text = TEXT.replace("one or two", "up to two");
                    r["source"]["oracle_text"] = json!(text);
                    r["faces"][0]["result"]["lines"][0]["text"] = json!(text);
                }
            }
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile_r4(&bad), CompileResult::Lowered { .. }),
            "source mutation {change}"
        );
    }
}

#[test]
fn divided_plus_one_counters_requires_the_exact_resolution() {
    let raw = ABZAN_CHARM.trim_end_matches('\n');
    let block = "/resolution/faces/0/blocks/0";
    let among = "/effects/0/steps/0/options/2/effects/0/steps/0/among";
    let changes: [fn(&mut Value); 7] = [
        // The division's recipients are the third mode's own target.
        |r| r["resolution"]["faces"][0]["blocks"][0]["refs"][3]["ref"]["slot"] = json!(0),
        |r| {
            r["resolution"]["faces"][0]["blocks"][0]["refs"][1]["ref"]["role"] =
                json!("ability-controller")
        },
        |r| {
            r["resolution"]["faces"][0]["blocks"][0]["refs"]
                .as_array_mut()
                .unwrap()
                .pop();
        },
        |r| {
            r["resolution"]["faces"][0]["blocks"][0]["residuals"] =
                json!(["diagnostic:unsupported-number-binding"]);
            r["resolution"]["complete"] = json!(false);
        },
        |r| {
            r["resolution"]["faces"][0]["blocks"][0]["steps"][4] = json!({"path":
                "/effects/0/steps/0/options/2/effects/0/steps/0","astEffect":"distribute-counters","unaccounted":true});
        },
        |r| r["resolution"]["faces"][0]["blocks"][0]["spans"]["certificate"]["bytes"] = json!(1),
        |r| {
            r.as_object_mut().unwrap().remove("resolution");
            r.as_object_mut().unwrap().remove("resolutionSha256");
        },
    ];
    assert_eq!(
        raw.parse::<Value>()
            .unwrap()
            .pointer(&format!("{block}/refs/3/path")),
        Some(&json!(among))
    );
    for (index, change) in changes.into_iter().enumerate() {
        let bad = changed_source(raw, change);
        assert!(
            matches!(compile_r4(&bad), CompileResult::Unsupported { path, .. }
                | CompileResult::Invalid { path, .. } if path.starts_with("resolution")),
            "resolution change {index}: {:?}",
            compile_r4(&bad)
        );
    }
    // A forged hash is an integrity failure, not a fallback.
    let pinned: Value = serde_json::from_str(raw).unwrap();
    let pinned = pinned["resolutionSha256"].as_str().unwrap();
    let forged = raw.replacen(pinned, DIGEST, 1);
    assert!(matches!(
        compile_r4(&forged),
        CompileResult::Invalid { path, .. } if path == "resolution"
    ));
}

#[test]
fn divided_plus_one_counters_frozen_archive_probe() {
    let Some(path) = std::env::var_os("MTG_DIVIDED_PLUS_ONE_COUNTERS_ARCHIVE_PROBE") else {
        return;
    };
    let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .map(Result::unwrap);
    let header: Value = serde_json::from_str(&rows.next().unwrap()).unwrap();
    assert_eq!(header, serde_json::from_str::<Value>(R4).unwrap());
    let raw = rows.nth(9810 - 2).unwrap();
    assert_eq!(raw, ABZAN_CHARM.trim_end_matches('\n'));
    check(&raw);
}
