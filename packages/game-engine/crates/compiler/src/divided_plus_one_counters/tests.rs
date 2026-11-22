use super::*;
use crate::tests::changed_source;
use std::io::{BufRead, BufReader};

const HEADER: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
const ABZAN_CHARM: &str =
    include_str!("../../../../tests/fixtures/ramos/4137a22c-f793-4e27-a9b2-72740a6e2121.json");
const DIGEST: &str = "8ab640fe165d73efaae0ad2bacb625a5bbadae97df2e4e2d4544fe0332590f8e";

fn synthetic() -> String {
    changed_source(
        include_str!("../../../../tests/fixtures/murder.json"),
        |r| {
            // A different identity, name and mana cost exercise the same source semantics.
            let ast = abilities();
            r["source"]["oracle_text"] = json!(TEXT);
            let result = &mut r["faces"][0]["result"];
            result["abilities"] = ast.clone();
            result["lines"] = json!([{"text":TEXT,"ok":true,"ability":ast[0]}]);
            r["binding"] =
                expected_binding(r["oracleIdentity"].as_str().unwrap(), &r["source"], &ast);
        },
    )
}

fn check(raw: &str) -> Box<Definition> {
    let definition = match compile(HEADER, raw) {
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
    let draw = Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0);
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
fn divided_plus_one_counters_complete_source_and_diagnostics() {
    let raw = synthetic();
    check(&raw);
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
        let bad = changed_source(&raw, |r| {
            *r.pointer_mut(&format!("{step}{field}")).unwrap() = value;
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(HEADER, &bad), CompileResult::Lowered { .. }),
            "{field}"
        );
    }
    for change in 0..8 {
        let bad = changed_source(&raw, |r| {
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
            !matches!(compile(HEADER, &bad), CompileResult::Lowered { .. }),
            "source mutation {change}"
        );
    }
    for (path, value) in [
        ("/binding/executable", json!(true)),
        ("/binding/referencesBound", json!(true)),
        ("/binding/fullyBound", json!(true)),
        ("/binding/counts/bound", json!(4)),
        ("/binding/engineFeatures", json!(["distribute-counters"])),
        ("/binding/nodes/4/cardinality", json!("many")),
        ("/binding/nodes/6/identity", json!("old-objects")),
        ("/binding/nodes/7/inputs", json!([])),
        ("/binding/nodes/8/description", json!({})),
        ("/binding/nodes/11/result", json!("objects")),
        ("/binding/nodes/12/cardinality", json!("two")),
        ("/binding/references/3/binding", json!("bound")),
        ("/binding/references/3/candidates", json!([])),
        ("/binding/diagnostics/0/path", json!("/faces/0")),
        ("/binding/diagnostics/1/rules", json!(["601.2d"])),
        ("/binding/diagnostics/2/code", json!("unsupported-scope")),
        ("/binding/diagnostics", json!([])),
    ] {
        let bad = changed_source(&raw, |r| *r.pointer_mut(path).unwrap() = value);
        assert!(
            matches!(compile(HEADER,&bad),CompileResult::Unsupported{path: p,..} if p == "binding"),
            "{path}"
        );
    }
}

fn check_frozen(raw: &str) {
    assert_eq!(sha256(raw.as_bytes()), DIGEST);
    let d = check(raw);
    assert_eq!(d.name, "Abzan Charm");
    assert_eq!(d.oracle_id, "4137a22c-f793-4e27-a9b2-72740a6e2121");
    assert_eq!(d.provenance.source_index, 9808);
}

#[test]
fn divided_plus_one_counters_complete_frozen_abzan_charm() {
    check_frozen(ABZAN_CHARM.trim_end_matches('\n'));
}

#[test]
fn divided_plus_one_counters_frozen_archive_probe() {
    let Some(path) = std::env::var_os("MTG_DIVIDED_PLUS_ONE_COUNTERS_ARCHIVE_PROBE") else {
        return;
    };
    let mut rows = BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .map(Result::unwrap);
    let first = rows.next().unwrap();
    let header: Value = serde_json::from_str(&first).unwrap();
    let raw = if header["recordType"] == "manifest" {
        assert_eq!(header, serde_json::from_str::<Value>(HEADER).unwrap());
        rows.nth(9810 - 2).unwrap()
    } else {
        assert!(rows.next().is_none());
        first
    };
    check_frozen(&raw);
}
