//! Four exact complete archived sources and synthetic strict source controls.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const CASES: &[(&str, &str, &str, u32, &str, &str, &str)] = &[
    (
        "Loran's Escape",
        include_str!("../../../tests/fixtures/1a9048d6-f3fa-4f3b-9baa-35318177bc6e.json"),
        "5a1488a883a71b471003fc2c2909ff25e45fd25c9c77a8636732ff9885516571",
        4057,
        "19e472477a351e8baf5f61cba4c1b92e16ac82cac3543cbe3bc626ca8069fa71",
        "0e36b6d3b16e0386b5c94f2cc1db737c4906cf5072b0e4e60e550a54cb7c9e45",
        "2249eb458d8f7aa32c652eb3841a00cb7e8bb70119ee288cf1c6792a5202cf2e",
    ),
    (
        "Serum Visions",
        include_str!("../../../tests/fixtures/56956afd-db53-4542-816b-490c8b0bbcf7.json"),
        "a95b4ac8017fac4d632079fa696d3e89f8545277600fa9512a9ae66391334f90",
        13005,
        "052e4ac325d97cf9e084bf9a07381965d3b8eab22a463750bf74dff225c06ea3",
        "0511dad10956edc72c0f39e9114508cac382942d7de366682f33cb09a6bde27c",
        "29257b1aa53bd09f46679d2183753b403fed8099295654adb0ff52a45824e613",
    ),
    (
        "Opt",
        include_str!("../../../tests/fixtures/713332c1-5bd8-400f-bfff-c1ca0697a043.json"),
        "3627b05641eea07780e6fa8fc2691352a5dcfdf26ce1374be7b5739aace979f7",
        17037,
        "9bc1edc63bdad6b1b08e0fbbf0269fed7181fb674b9542c7bfcc1359a76d8b55",
        "8053ea35233d5b3168e61cba98ae5ab21b04db3f56148b1173eea69b2b34e3ff",
        "6ed0eef38d6ad18faa87c0cece73381beff6f7a9614f31aad236972d7ee9ed5d",
    ),
    (
        "Preordain",
        include_str!("../../../tests/fixtures/ac641490-ca14-48d7-8cc4-b69ce984befa.json"),
        "0185b470d802635525fe9e6070363be7450be55c73669940ef537a78fdac45f7",
        25949,
        "277d1648e22306a768023eabf7f890007238994c3de816998e649602fafc35d5",
        "c4abd40f9475a271acf375110d731eb6110a9fc2716e9bbb2acc1eb6e2168f91",
        "94d4f6c35ff6f17ccdc276619237a3275b39b70a42fea0a8805a95165930c38f",
    ),
];
fn lowered(raw: &str) -> Box<Definition> {
    let result = compile(HEADER, raw);
    let CompileResult::Lowered { definition } = result else {
        panic!("complete scry source required: {result:?}")
    };
    definition.validate().unwrap();
    definition
}
fn changed(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(raw, |r| {
        change(r);
        let abilities = r["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .clone();
        let lines = r["faces"][0]["result"]["lines"].as_array_mut().unwrap();
        for (line, ability) in lines
            .iter_mut()
            .filter(|l| l.get("ability").is_some())
            .zip(abilities)
        {
            line["ability"] = ability;
        }
    })
}
fn edit_scry(r: &mut Value, change: impl Fn(&mut Value)) {
    for ability in r["faces"][0]["result"]["abilities"].as_array_mut().unwrap() {
        for sentence in ability["effects"].as_array_mut().unwrap() {
            for step in sentence["steps"].as_array_mut().unwrap() {
                if step["effect"] == "scry" {
                    change(step);
                }
            }
        }
    }
}
fn rejected(raw: &str) {
    assert!(
        !matches!(compile(HEADER, raw), CompileResult::Lowered { .. }),
        "unexpected complete lowering: {raw}"
    );
}
#[test]
fn controller_scry_compiler_four_frozen_sources_preserve_full_order_provenance_and_costs() {
    for (name, raw, record, index, source, ast, binding) in CASES {
        assert_eq!(sha256(raw.trim_end().as_bytes()), *record);
        let r: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(r["source"]["layout"], "normal");
        assert_eq!(r["source"]["legalities"]["commander"], "legal");
        assert_eq!(r["binding"]["fullyBound"], true);
        let d = lowered(raw);
        assert_eq!(d.name, *name);
        assert_eq!(d.provenance.source_index, *index);
        assert_eq!(d.provenance.source_sha256, *source);
        assert_eq!(d.provenance.ast_sha256, *ast);
        assert_eq!(d.provenance.binding_sha256, *binding);
        assert_eq!(d.characteristics.mana_cost.generic, 0.into());
        let p = d.spell.as_ref().unwrap();
        let scry = |n, b| Instruction::Scry {
            amount: Quantity::from(n),
            source_block: b,
        };
        let draw = |b| Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), b);
        let expected = match *name {
            "Opt" => vec![scry(1, 0), draw(1)],
            "Preordain" => vec![scry(2, 0), draw(0)],
            "Serum Visions" => vec![draw(0), scry(2, 0)],
            _ => vec![
                Instruction::GrantKeywords {
                    target_slot: 0,
                    modifier: None,
                    keywords: vec![KeywordAbility::Hexproof, KeywordAbility::Indestructible],
                    duration: Duration::UntilEndOfTurn,
                    source_block: 0,
                },
                scry(1, 0),
            ],
        };
        assert_eq!(p.instructions, expected);
        let mut caps = BTreeSet::from([Capability::NormalLayoutV1, Capability::ControllerScryV1]);
        if *name == "Loran's Escape" {
            assert_eq!(d.characteristics.mana_cost.colored, [Color::W]);
            assert_eq!(p.targets[0].domain, TargetDomain::BattlefieldPermanent);
            assert_eq!(
                p.targets[0].filter.as_ref().unwrap().types,
                [CardKind::Creature, CardKind::Artifact]
            );
            assert!(
                temporary_permanent_keyword_program(p)
                    .unwrap()
                    .controller_scry
                    .is_some()
            );
            assert!(temporary_creature_keyword_program(p).is_none());
            caps.extend([
                Capability::TemporaryPermanentKeywordProgramV1,
                Capability::PermanentRemovalV1,
                Capability::AttachmentsV1,
                Capability::IndestructibleV1,
            ]);
        } else {
            assert_eq!(d.characteristics.mana_cost.colored, [Color::U]);
            assert!(p.targets.is_empty());
            caps.insert(Capability::ControllerDrawV1);
            assert!(mtg_contracts::controller_scry_program(p));
        }
        if matches!(*name, "Opt" | "Loran's Escape") {
            caps.insert(Capability::CastInstantV1);
            assert_eq!(p.timing, SpellTiming::Instant);
        } else {
            caps.insert(Capability::CastSorceryV1);
            assert_eq!(p.timing, SpellTiming::MainPhase);
        }
        assert_eq!(d.capabilities, caps);
    }
}
#[test]
fn controller_scry_compiler_fixed_parameterization_and_commutative_selector_canonicalization() {
    for (_, raw, ..) in CASES {
        for value in [0, 1, 2, 17, u32::MAX] {
            let raw = changed(raw, |r| {
                edit_scry(r, |s| s["amount"]["value"] = value.into())
            });
            let d = lowered(&raw);
            assert!(d.spell.as_ref().unwrap().instructions.iter().any(|i|matches!(i,Instruction::Scry {amount,..} if *amount==Quantity::from(i64::from(value)))));
        }
    }
    let raw = CASES[0].1;
    let reversed = changed(raw, |r| {
        r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["allOf"][0]["anyOf"].as_array_mut().unwrap().reverse();
        let target =
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
        for reference in r["binding"]["references"].as_array_mut().unwrap() {
            reference["expression"] = target.clone();
        }
        for node in r["binding"]["nodes"].as_array_mut().unwrap() {
            if node["node"] == "selection" {
                node["expression"] = target.clone();
            }
        }
    });
    assert_eq!(lowered(raw).spell, lowered(&reversed).spell);
}
#[test]
fn controller_scry_compiler_rejects_unconsumed_operands_qualifiers_and_sentences() {
    for (_, raw, ..) in CASES {
        for case in 0..10 {
            let raw = changed(raw, |r| {
                edit_scry(r, |s| match case {
                    0 => s["amount"] = Value::Null,
                    1 => s["amount"]["value"] = json!(-1),
                    2 => s["amount"]["amount"] = json!("x"),
                    3 => s["amount"]["extra"] = Value::Null,
                    4 => s["amount"]["value"] = json!(1.5),
                    5 => s["who"] = json!({"player":"you"}),
                    6 => s["forEach"] = Value::Null,
                    7 => s["optional"] = json!(false),
                    8 => s["condition"] = Value::Null,
                    _ => s["amount"]["value"] = json!("2"),
                })
            });
            rejected(&raw);
        }
        for field in ["condition", "conditionKind"] {
            rejected(&changed(raw, |r| {
                r["faces"][0]["result"]["abilities"][0]["effects"][0][field] = Value::Null
            }));
        }
        for kind in ["Artifact", "Creature", "Enchantment", "Land"] {
            rejected(&tests::changed_source(raw, |r| {
                r["source"]["type_line"] = kind.into()
            }));
        }
    }
    for (_, raw, ..) in &CASES[1..] {
        for field in ["selectionId", "of"] {
            rejected(&changed(raw, |r| {
                for a in r["faces"][0]["result"]["abilities"].as_array_mut().unwrap() {
                    for e in a["effects"].as_array_mut().unwrap() {
                        for s in e["steps"].as_array_mut().unwrap() {
                            if s["effect"] == "draw" {
                                s["who"][field] = Value::Null;
                            }
                        }
                    }
                }
            }));
        }
    }
}
#[test]
fn controller_scry_compiler_rejects_filtered_grant_scope_and_complete_binding_near_misses() {
    for case in 0..10 {
        let raw = changed(CASES[0].1, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            match case {
                0 => a["effects"][0]["steps"][0]["what"]["filter"]["control"] = json!("opponent"),
                1 => a["effects"][0]["steps"][0]["what"]["count"]["value"] = json!(2),
                2 => a["effects"][0]["steps"][0]["what"]["upTo"] = json!(false),
                3 => a["effects"][0]["steps"][0]["what"]["filter"] = json!({"types":["land"]}),
                4 => a["effects"][0]["steps"][0]["what"]["filter"]["allOf"][0]["anyOf"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"types":["enchantment"]})),
                5 => {
                    a["effects"][0]["steps"][0]["abilities"][0]["keyword"]["keyword"] =
                        json!("shroud")
                }
                6 => {
                    a["effects"][1]["steps"][0] = json!({"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":2}})
                }
                7 => {
                    let tail = a["effects"][1]["steps"][0].clone();
                    a["effects"].as_array_mut().unwrap().pop();
                    a["effects"][0]["steps"].as_array_mut().unwrap().push(tail);
                }
                8 => r["binding"]["fullyBound"] = json!(false),
                _ => r["binding"]["references"][0]["expression"] = Value::Null,
            }
        });
        rejected(&raw);
    }
    for (_, raw, ..) in &CASES[1..] {
        rejected(&tests::changed_source(raw, |r| {
            r["binding"]["references"][0]["expression"] = Value::Null
        }));
        rejected(&tests::changed_source(raw, |r| {
            r["binding"]["diagnostics"] = json!([{"code":"unsupported-reference"}])
        }));
    }
}
