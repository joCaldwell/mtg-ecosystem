use super::*;
use crate::tests::changed_source;
use std::io::{BufRead, BufReader};

const MANIFEST: &str = include_str!("../../../../../tests/fixtures/archive-manifest-current.json");
const ROW: &str =
    include_str!("../../../../../tests/fixtures/ramos/53236dd7-845a-444c-96d5-f41ed7325d8f.json");
const DIGEST: &str = "0e5a727566c9304a0694c2dd90ab89739e8e973521d0de53a75617a43a4b2aa0";

fn check(row: &str, symbol: &str) -> Box<Definition> {
    let result = compile(MANIFEST, row);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(definition.triggered.len(), 1);
    let t = &definition.triggered[0];
    assert_eq!(
        t.condition,
        TriggerCondition::SpellCast {
            caster: CasterRelation::Opponent,
            spell: TypePredicate::Any {}
        }
    );
    assert_eq!(
        t.program.instructions,
        [Instruction::OptionalDrawUnlessCasterPays {
            generic: symbol.parse().unwrap(),
            source_block: 0
        }]
    );
    assert_eq!(definition.source_labels.as_ref().unwrap().triggered, [0]);
    use Capability::*;
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            NormalLayoutV1,
            CastEnchantmentV1,
            TriggeredAbilityV1,
            CastTriggerV1,
            OptionalControllerDrawV1,
            CastPayerDrawV1
        ])
    );
    definition
}

#[test]
fn cast_payer_draw() {
    // Physical 12509 is also retained in the revision-17 Ramos sources.
    let row = if let Some(path) = std::env::var_os("MTG_CAST_PAYER_DRAW_ARCHIVE_PROBE") {
        let mut rows = BufReader::new(std::fs::File::open(path).unwrap()).lines();
        let first = rows.next().unwrap().unwrap();
        if serde_json::from_str::<Value>(&first).unwrap()["recordType"] == "manifest" {
            rows.nth(12509 - 2)
                .expect("missing Rhystic Study physical 12509")
                .unwrap()
        } else {
            assert!(
                rows.next().is_none(),
                "expected full archive or one pinned row"
            );
            first
        }
    } else {
        ROW.trim_end().to_owned()
    };
    assert_eq!(sha256(row.as_bytes()), DIGEST);
    let actual = check(&row, "1");
    assert_eq!(actual.name, "Rhystic Study");
    assert_eq!(actual.provenance.source_index + 2, 12509);

    // Independent of witness name, mana value, and machine-width payment bounds.
    let symbol = "18446744073709551617";
    let synthetic = changed_source(&row, |r| {
        let name = "Synthetic cast payment enchantment";
        let text = format!(
            "Whenever an opponent casts a spell, you may draw a card unless that player pays {{{symbol}}}."
        );
        r["source"]["name"] = json!(name);
        r["source"]["mana_cost"] = json!("{U}");
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        result["name"] = json!(name);
        result["abilities"][0]["effects"][0]["condition"]["cost"][0]["symbols"] = json!([symbol]);
        result["lines"][0]["text"] = json!(text);
        result["lines"][0]["ability"] = result["abilities"][0].clone();
        r["binding"]["nodes"][1]["name"] = json!(name);
        r["binding"]["nodes"][1]["manaCost"] = json!("{U}");
    });
    check(&synthetic, symbol);
    for (path, value) in [
        ("/trigger/who", json!({"player":"an-opponent","of":null})),
        (
            "/trigger/what",
            json!({"cls":"spell","nonTypes":["creature"]}),
        ),
        ("/trigger/notFrom", Value::Null),
        ("/condition", Value::Null),
        ("/effects/0/conditionKind", json!("if")),
        ("/effects/0/condition/who", json!({"player":"you"})),
        ("/effects/0/steps/0/optional", json!(false)),
        ("/effects/0/steps/0/amount/value", json!(2)),
        ("/effects/0/condition/cost/0/symbols", json!(["1", "1"])),
        ("/effects/0/condition/cost/0/symbols", json!(["0"])),
        ("/effects/0/condition/cost/0/symbols", json!(["U"])),
        ("/effects/0/condition/cost/0/symbols", json!(["X"])),
        ("/effects/0/condition/cost/0/symbols", json!(["01"])),
        ("/effects/0/condition/cost/0/symbols", json!(["7"])),
    ] {
        let bad = changed_source(&synthetic, |r| {
            let result = &mut r["faces"][0]["result"];
            let (parent, key) = path.rsplit_once('/').unwrap();
            result["abilities"][0].pointer_mut(parent).unwrap()[key] = value;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &bad), CompileResult::Unsupported { .. }),
            "{path}"
        );
    }
    for (field, value) in [
        ("type_line", json!("Enchantment — Shrine")),
        ("type_line", json!("Legendary Enchantment")),
        ("type_line", json!("Artifact Enchantment")),
        ("keywords", json!(["Cumulative upkeep"])),
        (
            "oracle_text",
            json!(
                "Whenever an opponent casts a noncreature spell, you may draw a card unless that player pays {1}."
            ),
        ),
        (
            "oracle_text",
            json!(
                "Whenever an opponent casts a spell, you may draw a card unless that player pays {1}. Then draw a card."
            ),
        ),
    ] {
        let bad = changed_source(&row, |r| {
            r["source"][field] = value.clone();
            if field == "oracle_text" {
                r["faces"][0]["result"]["lines"][0]["text"] = value;
            } else if field == "type_line" {
                r["binding"]["nodes"][1]["typeLine"] = value;
            }
        });
        assert!(
            matches!(compile(MANIFEST, &bad), CompileResult::Unsupported { .. }),
            "{field}"
        );
    }
    for (path, target) in [
        ("/binding/references/0/target", "/faces/0/abilities/0#you"),
        (
            "/binding/references/1/target",
            "/faces/0/abilities/0/trigger#player",
        ),
        (
            "/binding/references/2/target",
            "/faces/0/abilities/0/trigger/who#query",
        ),
        ("/binding/references/3/target", "/faces/0/abilities/0#you"),
        (
            "/binding/nodes/8/decider",
            "/faces/0/abilities/0/trigger#player",
        ),
    ] {
        let bad = changed_source(&row, |r| {
            *r.pointer_mut(path).unwrap() = json!(format!("card:{}{target}", actual.oracle_id));
        });
        assert!(
            matches!(compile(MANIFEST,&bad),CompileResult::Unsupported { path,.. } if path == "binding")
        );
    }
    let stale = changed_source(&row, |r| {
        let text = "Whenever an opponent casts a spell, you may draw a card.";
        r["source"]["oracle_text"] = json!(text);
        let result = &mut r["faces"][0]["result"];
        let a = &mut result["abilities"][0];
        a["effects"][0].as_object_mut().unwrap().remove("condition");
        a["effects"][0]
            .as_object_mut()
            .unwrap()
            .remove("conditionKind");
        result["lines"][0] = json!({"text":text,"ok":true,"ability":a});
    });
    assert!(matches!(
        compile(MANIFEST, &stale),
        CompileResult::Unsupported { .. }
    ));
}
