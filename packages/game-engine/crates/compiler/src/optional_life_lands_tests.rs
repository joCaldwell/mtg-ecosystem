//! Five exact frozen whole sources; all mutations are synthetic, no admission.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
const SOURCES: &[(&str, &str, &str, &str, &str)] = &[
    (
        include_str!("../../../tests/fixtures/ramos/20283c4a-f1f0-42f0-bc08-6da87474426b.json"),
        "deaacd6676e2f1fee5d93de8347b707f94e1a1fd1fdafca66b42eb6bf249720e",
        "2a06b6e30969387e5f7aa8da516e63a7fdd1019321062fcf114904423e9ddfb9",
        "6404d85dec10be76db49503610f19d4bf09d98c87c68abfab58c7e2e0c4e0b23",
        "Forest Island",
    ),
    (
        include_str!("../../../tests/fixtures/ramos/f1750962-a87c-49f6-b731-02ae971ac6ea.json"),
        "623a921ae25fa43a1d3a6a94e0047c8280747478ffe14d06f6d76a53e87e2b34",
        "84264d198bedab7e06876d049dd6933c50535925a57d6da1df379e4dc4d8b1d1",
        "12bf14244cead824688843e2d44ec96ae3d64a436ddf487a7d3cd6a10158c949",
        "Plains Island",
    ),
    (
        include_str!("../../../tests/fixtures/ramos/975ec9a3-6f20-4177-8211-82526e092538.json"),
        "43a7fda9553b0f6958fb3f5176ac919ba8ed4b327cde2109844a42a7edb2928e",
        "ff7334439be5ec451aae6d59af67dca067f861cf3ad1ffcc19a94fd77ee698cd",
        "fa973c626aebb69fc81e66fbd589057c91f33f0db9867f2a33e0d2ec2103e0e9",
        "Swamp Forest",
    ),
    (
        include_str!("../../../tests/fixtures/ramos/16052b52-ade1-406f-a06b-ce7ea607fb63.json"),
        "701a12bca1b3da0bee34cded6b41da315b3dbdb909ddc5bf0d7005d1fc56ca79",
        "30bb8de801d0a2c2a705bd8fb51d50f18a519d92347d934875ef5d0e49dbe40f",
        "dbb6a1bb7a90f5c0654694ba06e8852647d59a4bb92509c1cc8360d67f217560",
        "Mountain Forest",
    ),
    (
        include_str!("../../../tests/fixtures/ramos/f413a83d-a40d-434c-b20a-4c707c0527fa.json"),
        "2244672ca2625085989c4b5ce62ac8936e664b191011e8e8ac0ee9e906d230d4",
        "ba93ada5c3bdb91d224475ccaaacadc04cdc0fd3f8b3faceeef4e34d42910b2c",
        "424ee03d80522a76938336addbed1c1947d52a510e9d082efa34c02bac4c8acd",
        "Forest Plains",
    ),
];
fn lowered(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn changed(raw: &str, change: impl FnOnce(&mut Value)) -> String {
    crate::tests::changed_source(raw, |r| {
        change(r);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn optional_life_lands_five_complete_pinned_sources() {
    let mut definitions = vec![];
    for &(raw, file_hash, source_hash, binding_hash, subtypes) in SOURCES {
        assert_eq!(sha256(raw.as_bytes()), file_hash);
        let record: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(record["sourceSha256"], source_hash);
        assert_eq!(record["bindingSha256"], binding_hash);
        assert_eq!(
            record["astSha256"],
            "e82812e47ace0b9c5b16a76e902172265bb59eb06f8de9a07122b54a4d249faa"
        );
        assert_eq!(
            record["binding"]["counts"],
            json!({"obligations":3,"bound":3,"unresolved":0})
        );
        assert_eq!(record["binding"]["nodes"].as_array().unwrap().len(), 5);
        let d = lowered(raw);
        d.validate().unwrap();
        assert_eq!(
            d.characteristics.subtypes,
            subtypes.split(' ').collect::<Vec<_>>()
        );
        assert_eq!(
            d.static_abilities,
            [StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTappedUnlessPayLife { amount: 2.into() },
            }]
        );
        assert!(d.mana_abilities.is_empty());
        assert!(d.spell.is_none());
        assert_eq!(d.source_labels.as_ref().unwrap().statics, [0]);
        assert_eq!(d.source_labels.as_ref().unwrap().blocks.len(), 1);
        assert_eq!(
            d.capabilities,
            BTreeSet::from([
                Capability::NormalLayoutV1,
                Capability::LandPlayV1,
                Capability::TypedNonbasicIntrinsicManaV1,
                Capability::OptionalLifeLandEntryV1,
            ])
        );
        definitions.push(d);
    }
    if let Some(directory) = std::env::var_os("MTG_OPTIONAL_LIFE_LANDS_EXPORT_DIR") {
        use std::io::Write;
        definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions,
        };
        catalog.validate().unwrap();
        for (name, value) in [
            ("catalog.json", serde_json::to_value(catalog).unwrap()),
            ("schema.json", catalog_schema()),
        ] {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(std::path::Path::new(&directory).join(name))
                .unwrap();
            file.write_all(serde_json::to_string_pretty(&value).unwrap().as_bytes())
                .unwrap();
        }
    }
}
#[test]
fn optional_life_lands_exact_source_parameters_and_identity_independence() {
    let raw = SOURCES[0].0;
    for n in [0u64, 1, 2, 3, u64::MAX] {
        let changed = changed(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["effect"]["costs"][0]["amount"]["value"] =
                json!(n)
        });
        assert_eq!(
            lowered(&changed).static_abilities[0].effect,
            StaticEffect::EntersTappedUnlessPayLife {
                amount: n.to_string().parse().unwrap()
            }
        );
    }
    let renamed = changed(raw, |r| {
        r["source"]["name"] = json!("Generic optional life entry");
        r["faces"][0]["result"]["name"] = json!("Generic optional life entry");
        r["binding"]["nodes"][1]["name"] = json!("Generic optional life entry");
    });
    lowered(&renamed).validate().unwrap();
    let rebound = changed(raw, |r| {
        let old = r["oracleIdentity"].as_str().unwrap();
        let text = serde_json::to_string(r)
            .unwrap()
            .replace(old, "00000000-0000-0000-0000-000000000037");
        *r = serde_json::from_str(&text).unwrap();
    });
    lowered(&rebound).validate().unwrap();
}
#[test]
fn optional_life_lands_rehashed_operand_and_binding_negatives() {
    let raw = SOURCES[0].0;
    for (path, value) in [
        ("/what", json!({"ref":"this","noun":"creature"})),
        ("/what", json!({"ref":"each","noun":"land"})),
        ("/who", json!({"player":"opponent"})),
        ("/who", json!(null)),
        ("/costs", json!([])),
        ("/costs", json!(null)),
        ("/costs/0/cost", json!("mana")),
        ("/costs/0/payer", json!({"player":"you"})),
        ("/costs/0/amount", json!({"amount":"x"})),
        ("/costs/0/amount/value", json!(-1)),
        ("/costs/0/amount/value", json!(2.5)),
        ("/costs/0/amount/value", json!(2.0)),
        ("/costs/0/amount/value", json!("2")),
        ("/costs/0/amount/value", json!(null)),
        ("/costs/0/amount/value", json!(18446744073709551616.0)),
        ("/costs/0/amount/extra", json!(true)),
        ("/declined", json!({"tapped":false})),
        ("/declined", json!(null)),
        ("/declined/extra", json!(true)),
        ("/extra", json!(true)),
    ] {
        let mutated = changed(raw, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0]["effect"];
            let (parent, key) = path.rsplit_once('/').unwrap();
            a.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value.clone());
        });
        assert!(
            !matches!(compile(MANIFEST, &mutated), CompileResult::Lowered { .. }),
            "{path}:{value}"
        );
    }
    for (path, value) in [
        ("/binding/nodes/0/identity", json!("foreign")),
        ("/binding/nodes/1/typeLine", json!("Land")),
        ("/binding/nodes/2/you", json!("foreign")),
        ("/binding/nodes/3/role", json!("entering-controller")),
        ("/binding/nodes/4/role", json!("static-source-controller")),
        ("/binding/nodes/4/scope", json!("foreign")),
        ("/binding/references/0/target", json!("foreign")),
        ("/binding/references/1/target", json!("foreign")),
        ("/binding/references/2/target", json!("foreign")),
        (
            "/binding/references/2/expression",
            json!({"player":"owner"}),
        ),
        ("/binding/references/2/binding", json!("unresolved")),
        ("/binding/counts/obligations", json!(2)),
        ("/binding/counts/bound", json!(2)),
        ("/binding/counts/unresolved", json!(1)),
        ("/binding/counts/extra", json!(true)),
        ("/binding/fullyBound", json!(false)),
        ("/binding/referencesBound", json!(false)),
    ] {
        let mutated = changed(raw, |r| *r.pointer_mut(path).unwrap() = value);
        assert!(
            !matches!(compile(MANIFEST, &mutated), CompileResult::Lowered { .. }),
            "{path}"
        );
    }
    for case in 0..8 {
        let mutated = changed(raw, |r| match case {
            0 => {
                r["binding"]["references"].as_array_mut().unwrap().pop();
            }
            1 => {
                let x = r["binding"]["references"][0].clone();
                r["binding"]["references"][2] = x;
            }
            2 => {
                let mut x = r["binding"]["references"][2].clone();
                x["path"] = json!("/foreign");
                r["binding"]["references"].as_array_mut().unwrap().push(x);
            }
            3 => {
                let x = r["binding"]["nodes"][0].clone();
                r["binding"]["nodes"].as_array_mut().unwrap().push(x);
            }
            4 => {
                r["binding"]["nodes"].as_array_mut().unwrap().pop();
            }
            5 => {
                let x = r["faces"][0]["result"]["abilities"][0]["effect"]["costs"][0].clone();
                r["faces"][0]["result"]["abilities"][0]["effect"]["costs"]
                    .as_array_mut()
                    .unwrap()
                    .push(x);
            }
            6 => {
                r["binding"]["references"][2]["extra"] = json!(true);
            }
            _ => {
                let x = r["faces"][0]["result"]["abilities"][0].clone();
                r["faces"][0]["result"]["abilities"]
                    .as_array_mut()
                    .unwrap()
                    .push(x);
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &mutated), CompileResult::Lowered { .. }),
            "{case}"
        );
    }
}
