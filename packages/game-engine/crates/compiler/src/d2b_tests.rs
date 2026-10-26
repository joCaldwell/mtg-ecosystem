use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const SOURCE_SELF: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2b/fec77e0a-433e-4d32-9260-e26a37f6ad23.json"
);

#[test]
fn bound_self_reference_equals_this_source_and_rejects_wrong_or_missing_bindings() {
    let CompileResult::Lowered { mut definition } = compile(MANIFEST, SOURCE_SELF) else {
        panic!("bound source")
    };
    let this = tests::changed_source(SOURCE_SELF, |r| {
        r["source"]["oracle_text"] = r["source"]["oracle_text"]
            .as_str()
            .unwrap()
            .replacen("Timeless Lotus", "This artifact", 1)
            .into();
        r["faces"][0]["result"]["lines"][0]["text"] = "This artifact enters tapped.".into();
        r["faces"][0]["result"]["abilities"][0]["effect"]["what"] =
            serde_json::json!({"ref":"this","noun":"artifact"});
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    });
    let CompileResult::Lowered { definition: other } = compile(MANIFEST, &this) else {
        panic!("this source")
    };
    definition.provenance = other.provenance.clone();
    assert_eq!(definition, other);
    for mutation in [
        "wrong-target",
        "unresolved",
        "missing",
        "wrong-expression",
        "duplicate",
    ] {
        let raw = tests::changed_source(SOURCE_SELF, |r| {
            let refs = r["binding"]["references"].as_array_mut().unwrap();
            let i = refs
                .iter()
                .position(|r| r["path"] == "/faces/0/abilities/0/effect/what")
                .unwrap();
            match mutation {
                "wrong-target" => refs[i]["target"] = "another-object".into(),
                "unresolved" => refs[i]["binding"] = "unresolved".into(),
                "missing" => {
                    refs.remove(i);
                }
                "wrong-expression" => refs[i]["expression"] = serde_json::json!({"ref":"it"}),
                "duplicate" => refs.push(refs[i].clone()),
                _ => unreachable!(),
            }
        });
        assert!(
            matches!(compile(MANIFEST,&raw),CompileResult::Unsupported {reason,..} if reason=="ref:self:unbound_source"),
            "{mutation}"
        );
    }
}

#[test]
fn new_intrinsics_require_capability_closure_version_and_exact_shapes() {
    let catalog = catalog::build_catalog(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalogs/cards-data-d2b.json"
    )))
    .unwrap();
    for capability in [Capability::FlashV1, Capability::EntersTappedV1] {
        let d = catalog
            .definitions
            .iter()
            .find(|d| d.capabilities.contains(&capability))
            .unwrap();
        let mut bad = d.clone();
        bad.capabilities.remove(&capability);
        assert!(bad.validate().is_err());
        let mut old = catalog.clone();
        old.schema_version = 19;
        assert!(old.validate().is_err());
    }
    let mut bad = catalog
        .definitions
        .iter()
        .find(|d| d.keywords.contains(&KeywordAbility::Flash))
        .unwrap()
        .clone();
    bad.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
    assert!(bad.validate().is_err());
    let raw = tests::changed_source(SOURCE_SELF, |r| {
        for field in ["abilities", "lines"] {
            let effect = if field == "abilities" {
                &mut r["faces"][0]["result"][field][0]["effect"]
            } else {
                &mut r["faces"][0]["result"][field][0]["ability"]["effect"]
            };
            effect["conditionKind"] = "unless".into();
            effect["condition"] = serde_json::json!({"condition":"unsupported"});
        }
    });
    assert!(
        matches!(compile(MANIFEST,&raw),CompileResult::Unsupported {reason,..} if reason=="static:enters-tapped:conditional")
    );
}

#[test]
fn ability_word_labels_cannot_hide_an_unbound_player_relation() {
    let raw = include_str!(
        "../../../tests/fixtures/cards-data-d2b/95d018f4-7f97-4b2c-abb4-cef69031caa1.json"
    );
    assert!(
        matches!(compile(MANIFEST,raw),CompileResult::Unsupported {reason,..} if reason=="unbound death controller")
    );
}

#[test]
fn bound_self_trigger_uses_the_same_source_incarnation_ir_as_this_creature() {
    let raw = r#"[{"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"self"}},"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]"#;
    let binding = serde_json::json!({"identity":"synthetic", "nodes":[{"node":"source","id":"card:synthetic/source#object","identity":"synthetic","path":"/source"}],"references":[{"path":"/faces/0/abilities/0/trigger/what","expression":{"ref":"self"},"binding":"bound","target":"card:synthetic/source#object"}]});
    let canonical =
        self_reference::canonicalize(raw, &binding, "synthetic", "creature", "/faces/0/abilities")
            .unwrap();
    let abilities = read_abilities(&canonical).unwrap();
    let Ability::Triggered {
        trigger, effects, ..
    } = &abilities[0]
    else {
        panic!("trigger")
    };
    let ir = lower_trigger(trigger, effects, None, 0).unwrap();
    let direct = lower_trigger(
        &SourceTrigger::Enters {
            what: TriggerObject::This {
                noun: "creature".into(),
            },
        },
        effects,
        None,
        0,
    )
    .unwrap();
    assert_eq!(ir, direct);
    assert_eq!(ir.condition, TriggerCondition::SelfEntersBattlefield);
    assert!(
        matches!(ir.program.instructions.as_slice(),[Instruction::DrawCards {count,recipient:None,..}] if *count == Quantity::from(1))
    );
}
