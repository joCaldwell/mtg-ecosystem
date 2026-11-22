//! Frozen complete Escort witness; all changed AST/IR inputs are synthetic.
//! Source-only draft: native execution and admission remain parent-owned.
use super::*;
use serde_json::{Value, json};
const HEADER: &str = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const RAW: &str =
    include_str!("../../../../tests/fixtures/ramos/c0516c2c-d7c9-4763-9180-980f60205a28.json");
fn lowered(raw: &str) -> Box<Definition> {
    match compile(HEADER, raw) {
        CompileResult::Lowered { definition } => {
            definition.validate().unwrap();
            definition
        }
        other => panic!("complete Escort composition: {other:?}"),
    }
}
fn changed(change: impl FnOnce(&mut Value)) -> String {
    tests::changed_source(RAW, |r| {
        change(r);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
fn reject(raw: &str) {
    assert!(
        !matches!(compile(HEADER, raw), CompileResult::Lowered { .. }),
        "synthetic source must reject"
    );
}
#[test]
fn escort_set_exact_source_ast_binding_provenance_cost_and_program() {
    assert_eq!(
        sha256(RAW.as_bytes()),
        "0828c19386b8bdbe4f5fa9187e6ccd3ecc4812734cc05875ff9240b439a9af41"
    );
    let r: Value = serde_json::from_str(RAW).unwrap();
    assert_eq!(
        r["source"]["oracle_text"],
        "Sacrifice this creature: Creatures you control gain indestructible until end of turn."
    );
    assert_eq!(r["source"]["legalities"]["commander"], "legal");
    assert_eq!(
        r["binding"]["counts"],
        json!({"obligations":4,"bound":4,"unresolved":0})
    );
    assert_eq!(r["binding"]["referencesBound"], true);
    assert_eq!(r["binding"]["fullyBound"], true);
    assert_eq!(r["binding"]["references"].as_array().unwrap().len(), 4);
    assert!(
        r["binding"]["references"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["binding"] == "bound")
    );
    assert_eq!(
        r["faces"][0]["result"]["abilities"],
        json!([{
            "kind":"activated","costs":[{"cost":"sacrifice","what":{"ref":"this","noun":"creature"}}],
            "effects":[{"steps":[{"effect":"gain-abilities","what":{"ref":"each","filter":{"types":["creature"],"control":"you"}},
                "abilities":[{"gained":"keyword","keyword":{"keyword":"indestructible"}}],"duration":{"duration":"end-of-turn"}}]}]
        }])
    );
    let d = lowered(RAW);
    assert_eq!(d.oracle_id, "c0516c2c-d7c9-4763-9180-980f60205a28");
    assert_eq!(d.name, "Dauntless Escort");
    assert_eq!(d.provenance.source_index, 28979);
    assert_eq!(
        d.provenance.source_sha256,
        "637bbbc9abd468b8f2cbdd1846cc891608745c0c35f364947be49836bb841713"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "a19ee156bf73bcac8d78d67fe172111faf9384e15e2ff9ab62efade6c9789157"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "8230ee3c8f5c2aa9cb5b9bd19373bbe3ea4ac36beb896e75c6f34c6628207475"
    );
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::G, Color::W]
        }
    );
    assert_eq!(d.characteristics.power, Some(3.into()));
    assert_eq!(d.characteristics.toughness, Some(3.into()));
    assert!(
        d.spell
            .as_ref()
            .is_some_and(|p| p.resolution == SpellResolution::Permanent
                && p.timing == SpellTiming::MainPhase
                && p.instructions.is_empty()
                && p.targets.is_empty()
                && p.optional_additional_costs.is_empty())
            && d.keywords.is_empty()
            && d.triggered.is_empty()
            && d.static_abilities.is_empty()
            && d.mana_abilities.is_empty()
    );
    assert_eq!(d.activated.len(), 1);
    let a = &d.activated[0];
    assert_eq!(a.cost, ActivationCost::SacrificeSource);
    assert_eq!(
        a.program.instructions,
        vec![Instruction::GrantKeywordsSet {
            set: ObjectSet::ControllerCreatures,
            modifier: None,
            keywords: vec![KeywordAbility::Indestructible],
            duration: Duration::UntilEndOfTurn,
            source_block: 0,
        }]
    );
    let program = sacrifice_source_creature_set_keyword_ability(a).unwrap();
    assert_eq!(program.keywords, [KeywordAbility::Indestructible]);
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::BaseCreatureV1,
            Capability::CastCreatureV1,
            Capability::ActivatedAbilityV1,
            Capability::SacrificeSourceCostV1,
            Capability::BoardWipesV1,
            Capability::TemporaryCreatureSetKeywordProgramV1,
            Capability::IndestructibleV1
        ])
    );
    assert_eq!(d.capabilities, d.requirements());
}
#[test]
fn escort_set_synthetic_parameterized_keywords_and_characteristics_use_same_composition() {
    let raw = changed(|r| {
        for n in r["binding"]["nodes"].as_array_mut().unwrap() {
            if n["node"] == "keyword-ability" {
                n["keyword"] = json!("flying");
            }
        }
        r["source"]["power"] = "7".into();
        r["source"]["toughness"] = "9".into();
        r["source"]["mana_cost"] = "{5}{G}{W}".into();
        r["binding"]["nodes"][1]["manaCost"] = "{5}{G}{W}".into();
        let nodes = r["binding"]["nodes"].as_array_mut().unwrap();
        let template: Vec<_> = nodes
            .iter()
            .filter(|n| {
                n["path"].as_str().is_some_and(|p| {
                    p.starts_with("/faces/0/abilities/0/effects/0/steps/0/abilities/0")
                })
            })
            .cloned()
            .collect();
        for n in template {
            let text = serde_json::to_string(&n).unwrap().replace(
                "/effects/0/steps/0/abilities/0",
                "/effects/0/steps/0/abilities/1",
            );
            let mut node: Value = serde_json::from_str(&text).unwrap();
            if node["node"] == "keyword-ability" {
                node["keyword"] = json!("lifelink");
            }
            nodes.push(node);
        }
        r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["abilities"] = json!([
            {"gained":"keyword","keyword":{"keyword":"flying"}},
            {"gained":"keyword","keyword":{"keyword":"lifelink"}}
        ]);
    });
    let d = lowered(&raw);
    assert_eq!(d.characteristics.power, Some(7.into()));
    assert_eq!(d.characteristics.toughness, Some(9.into()));
    assert_eq!(
        sacrifice_source_creature_set_keyword_ability(&d.activated[0])
            .unwrap()
            .keywords,
        [KeywordAbility::Flying, KeywordAbility::Lifelink]
    );
}
#[test]
fn escort_set_complete_source_negatives_do_not_admit_general_activated_grants() {
    for variant in 0..22 {
        reject(&changed(|r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            match variant {
                0 => a["costs"] = json!([{"cost":"tap-self"}]),
                1 => a["costs"] = json!([{"cost":"mana","symbols":["G"]}]),
                2 => a["costs"][0]["what"] = json!({"ref":"filter","filter":{"types":["creature"]}}),
                3 => a["costs"].as_array_mut().unwrap().push(json!({"cost":"tap-self"})),
                4 => a["effects"][0]["condition"] = json!({"condition":"you-control-creature"}),
                5 => a["effects"][0]["conditionKind"] = "if".into(),
                6 => a["effects"][0]["bindings"] = json!([{"binding":"unsupported"}]),
                7 => { let s = a["effects"][0].clone(); a["effects"].as_array_mut().unwrap().push(s); },
                8 => { let s = a["effects"][0]["steps"][0].clone(); a["effects"][0]["steps"].as_array_mut().unwrap().push(s); },
                9 => a["effects"][0]["steps"][0]["what"] = json!({"ref":"each","filter":{"cls":"permanent","control":"you"}}),
                10 => a["effects"][0]["steps"][0]["what"] = json!({"ref":"each","filter":{"types":["creature"],"status":["attacking"]}}),
                11 => a["effects"][0]["steps"][0]["what"]["filter"]["control"] = "opponent".into(),
                12 => a["effects"][0]["steps"][0]["what"]["filter"]["owner"] = "you".into(),
                13 => a["effects"][0]["steps"][0]["what"] = json!({"ref":"target","selectionId":"synthetic:0","count":{"amount":"fixed","value":1},"filter":{"types":["creature"]}}),
                14 => a["effects"][0]["steps"][0]["duration"]["duration"] = "forever".into(),
                15 => a["effects"][0]["steps"][0]["abilities"].as_array_mut().unwrap().push(json!({"gained":"keyword","keyword":{"keyword":"indestructible"}})),
                16 => a["effects"][0]["steps"][0]["abilities"][0]["keyword"]["keyword"] = "shroud".into(),
                17 => a["restriction"] = json!({"restriction":"only-as-sorcery"}),
                18 => { let a = a.clone(); r["faces"][0]["result"]["abilities"].as_array_mut().unwrap().push(a); },
                19 => r["source"]["type_line"] = "Artifact".into(),
                20 => { a["kind"] = "triggered".into(); a["trigger"] = json!({"trigger":"self-dies"}); a.as_object_mut().unwrap().remove("costs"); },
                _ => a["effects"][0]["steps"].as_array_mut().unwrap().push(json!({"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":1}})),
            }
        }));
    }
}

#[test]
fn escort_set_typed_condition_guards_preserve_none_none_closure() {
    let r: Value = serde_json::from_str(RAW).unwrap();
    let Ability::Activated { costs, effects, .. } =
        serde_json::from_value::<Ability>(r["faces"][0]["result"]["abilities"][0].clone()).unwrap()
    else {
        unreachable!()
    };
    assert!(lower_activation(&costs, &effects, 0, CardKind::Creature).is_ok());
    let condition: KickedCondition = serde_json::from_value(json!({
        "condition":"kicked","what":{"ref":"this","noun":"spell"},
        "comparison":{"op":"ge","value":{"amount":"fixed","value":1}}
    }))
    .unwrap();
    for (condition, kind) in [
        (Some(condition.clone()), None),
        (None, Some("if".into())),
        (Some(condition), Some("if".into())),
    ] {
        let mut effects = effects.clone();
        effects[0].condition = condition;
        effects[0].condition_kind = kind;
        assert!(matches!(
            lower_activation(&costs, &effects, 0, CardKind::Creature),
            Err(Failure {
                unsupported: true,
                ..
            })
        ));
    }
}

#[test]
fn escort_set_binding_only_foreign_missing_duplicate_and_unresolved_reject() {
    lowered(RAW);
    for index in 0..4 {
        for variant in 0..4 {
            reject(&changed(|r| {
                let refs = r["binding"]["references"].as_array_mut().unwrap();
                match variant {
                    0 => refs[index]["target"] = json!("foreign"),
                    1 => {
                        refs.remove(index);
                    }
                    2 => {
                        let other = refs[index].clone();
                        refs.push(other);
                    }
                    _ => refs[index]["binding"] = json!("unresolved"),
                }
            }));
        }
    }
    for index in 0..13 {
        reject(&changed(|r| {
            r["binding"]["nodes"].as_array_mut().unwrap().remove(index);
        }));
    }
}
