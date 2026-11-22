use super::*;
use crate::tests::changed_source;
use serde_json::{Value, json};

const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");

fn lowered(raw: &str) -> Box<Definition> {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => definition,
        result => {
            let source: Value = serde_json::from_str(raw).unwrap();
            panic!("{}: {result:?}", source["source"]["name"]);
        }
    }
}

fn changed(raw: &str, edit: impl FnOnce(&mut Value)) -> String {
    changed_source(raw, |r| {
        edit(r);
        let result = &mut r["faces"][0]["result"];
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    })
}

fn synthetic_damage() -> String {
    changed(
        include_str!("../../../tests/fixtures/lightning-bolt.json"),
        |r| {
            r["source"]["oracle_text"] = json!("Lightning Bolt deals X damage to any target.");
            r["source"]["mana_cost"] = json!("{X}{R}");
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["text"] = json!("~ deals X damage to any target.");
            result["abilities"][0]["effects"][0]["steps"][0]["amount"] = json!({"amount":"x"});
            let base = "/faces/0/abilities/0";
            let id = format!("card:{}{base}", r["oracleIdentity"].as_str().unwrap());
            let binding = &mut r["binding"];
            binding["nodes"][1]["manaCost"] = json!("{X}{R}");
            binding["referencesBound"] = json!(true);
            binding["engineFeatures"] = json!([]);
            binding["nodes"].as_array_mut().unwrap().push(json!({
                "id":format!("{id}#announced-x"),"path":base,"node":"announced-number",
                "scope":format!("{id}#ability"),"chooser":format!("{id}#you"),
                "announcement":"cast","rules":["107.3a","601.2b"]
            }));
            binding["references"].as_array_mut().unwrap().push(json!({
                "path":format!("{base}/effects/0/steps/0/amount"),"expression":{"amount":"x"},
                "binding":"bound","target":format!("{id}#announced-x"),"rules":["107.3a"]
            }));
            binding["counts"] = json!({"obligations":3,"bound":3,"unresolved":0});
        },
    )
}

fn assert_target(p: &SpellProgram, domain: TargetDomain, id: &str) {
    assert_eq!(
        p.targets,
        [TargetSpec {
            slot: 0,
            selection_id: id.into(),
            domain,
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None
        }]
    );
}

fn damage() -> Instruction {
    Instruction::damage_target(0, kir::Amount::announced_x(), 0)
}

fn gain(recipient: LifeRecipient) -> Instruction {
    Instruction::announced_x_life(recipient.kernel(Context::Spell).unwrap(), true, 0)
}

fn archive_witness(raw: &str, index: u32) {
    let d = lowered(raw);
    assert_eq!(d.provenance.source_index, index);
    assert!(d.x_mana_cost);
    let p = d.spell.unwrap();
    if index == 4879 {
        assert_eq!(p.modes.len(), 2);
        assert!(p.targets.is_empty() && p.instructions.is_empty());
        let draw = &p.modes[0].program;
        assert!(draw.targets.is_empty());
        assert_eq!(
            draw.instructions,
            [Instruction::act(
                Verb::Draw {
                    player: PlayerExpr::you(Context::Spell),
                    amount: kir::Amount::announced_x()
                },
                0
            )]
        );
        let burn = &p.modes[1].program;
        assert_target(burn, TargetDomain::AnyDamageTarget, "line:2:7");
        assert_eq!(burn.instructions, [damage()]);
        return;
    }

    assert!(p.modes.is_empty());
    let (domain, id) = match index {
        17902 | 21951 => (TargetDomain::BattlefieldCreature, "line:0:5"),
        23936 | 25940 => (TargetDomain::AnyPlayer, "line:0:0"),
        _ => (TargetDomain::AnyDamageTarget, "line:0:6"),
    };
    assert_target(&p, domain, id);
    let target = LifeRecipient::Target { target_slot: 0 };
    let controller = LifeRecipient::Controller;
    let expected = match index {
        883 | 17902 | 19915 => vec![damage()],
        21951 | 24464 => vec![damage(), gain(controller)],
        23936 => vec![gain(target)],
        25940 => vec![
            Instruction::mill(kir::PlayerExpr::target(0), kir::Amount::announced_x(), 0),
            gain(controller),
        ],
        1066 => {
            let draw = Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 0);
            vec![
                damage(),
                draw.clone(),
                draw,
                Instruction::discard(
                    kir::PlayerExpr::you(kir::Context::Spell),
                    kir::Amount::Fixed { value: 1.into() },
                    false,
                    0,
                ),
            ]
        }
        _ => unreachable!(),
    };
    assert_eq!(p.instructions, expected);
}

#[test]
fn announced_x_effects() {
    let synthetic = synthetic_damage();
    let d = lowered(&synthetic);
    let p = d.spell.unwrap();
    assert_target(&p, TargetDomain::AnyDamageTarget, "line:0:6");
    assert_eq!(p.instructions, [damage()]);
    for case in 0..8 {
        let bad = changed(&synthetic, |r| {
            let step = &mut r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0];
            match case {
                0 => step["optional"] = json!(false),
                1 => step["to"]["selectionId"] = json!("line:1:6"),
                2 => r["source"]["mana_cost"] = json!("{R}"),
                3 => r["source"]["mana_cost"] = json!("{X}{X}{R}"),
                4 => r["binding"]["references"][0]["target"] = json!("foreign-source"),
                5 => r["binding"]["references"][2]["target"] = json!("another-x"),
                6 => r["binding"]["nodes"][5]["optional"] = json!(true),
                _ => {
                    r["source"]["oracle_text"] =
                        json!("Lightning Bolt deals X damage to any target. You gain X life.");
                    r["faces"][0]["result"]["lines"][0]["text"] =
                        json!("~ deals X damage to any target. You gain X life.");
                }
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
            "case {case}"
        );
    }

    if let Some(path) = std::env::var_os("MTG_ANNOUNCED_X_EFFECTS_ARCHIVE_PROBE") {
        const PINS: [(u32, &str); 9] = [
            (
                883,
                "bce18ef23c8cb3b0e2109498c17da6eee9107741e0ab1d50ba4c093493730f92",
            ),
            (
                1066,
                "3f888694f101ace022671c8a2d8779cdbdca26fbe9c56760382456f1b8f6b5be",
            ),
            (
                4879,
                "1db96cd11e32a251f2257922a118d4c470ed97a2c30490795e51b61b87bd0e39",
            ),
            (
                17902,
                "5449565d2013894cd3dc2ad411e69ec10c0e45d056afb03b6958defb4af88816",
            ),
            (
                19915,
                "570714f2f5392d9dd9410030166e676e1fffc4c20891d97c1bafc211dffe302e",
            ),
            (
                21951,
                "a1dd7cbdd661547d1ee33d1373185a2064e61cd7fdc30e9ca65cde5f1849470e",
            ),
            (
                23936,
                "e75569a38444b7ecf5f5e0a2637d82c2bb30a75829c67bc6a14cb9b832e6f14c",
            ),
            (
                24464,
                "c0b6d6fd5d8b03f8883a343cef129ef47ed6ae5caddd17104bf3c68184678b4e",
            ),
            (
                25940,
                "05e435067025ee15684fce7f06c895baebf17661d664aba11fba37113636bec5",
            ),
        ];
        let rows = std::fs::read_to_string(path).unwrap();
        assert_eq!(rows.lines().count(), PINS.len());
        for (raw, (index, hash)) in rows.lines().zip(PINS) {
            assert_eq!(sha256(raw.as_bytes()), hash);
            archive_witness(raw, index);
            // Rehash realistic near misses, keeping source and line mirrors intact.
            if matches!(index, 1066 | 4879 | 25940) {
                let bad = changed(raw, |r| {
                    let effects = &mut r["faces"][0]["result"]["abilities"][0]["effects"];
                    match index {
                        1066 => {
                            effects[1]["steps"].as_array_mut().unwrap().pop();
                        }
                        4879 => effects[0]["steps"][0]["options"]
                            .as_array_mut()
                            .unwrap()
                            .swap(0, 1),
                        _ => {
                            let gain_x = r["binding"]["references"]
                                .as_array_mut()
                                .unwrap()
                                .iter_mut()
                                .find(|reference| {
                                    reference["path"]
                                        .as_str()
                                        .unwrap()
                                        .ends_with("/steps/1/amount")
                                })
                                .unwrap();
                            gain_x["target"] = json!("mill-event-quantity");
                        }
                    }
                });
                assert!(
                    !matches!(compile(MANIFEST, &bad), CompileResult::Lowered { .. }),
                    "source {index}"
                );
            }
        }
    }
}
