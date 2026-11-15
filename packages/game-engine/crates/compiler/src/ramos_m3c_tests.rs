//! Two exact retained d705 records outside the frozen Ramos target.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const BLOW: &str = include_str!("../../../tests/fixtures/dismantling-blow.json");
const RECLAMATION: &str = include_str!("../../../tests/fixtures/natural-reclamation.json");
fn lower(raw: &str) -> Box<Definition> {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => definition,
        other => panic!("{other:?}"),
    }
}
#[test]
fn ramos_m3c_exact_complete_records_preserve_source_and_linked_semantics() {
    for (raw, sha) in [
        (
            BLOW,
            "efa9a2f7a10a86eeb8518514fb4c9fc5faf71386b2f420c80161b51aa5986289",
        ),
        (
            RECLAMATION,
            "321ec3069c9acee2fd968d11665e58b29ab2bc089b354e5b1a5a6a2741a6ae4f",
        ),
    ] {
        assert_eq!(sha256(raw.as_bytes()), sha);
        lower(raw).validate().unwrap();
    }
    let blow = lower(BLOW);
    let p = blow.spell.as_ref().unwrap();
    assert_eq!(
        blow.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::W]
        }
    );
    assert_eq!(p.optional_additional_costs.len(), 1);
    assert_eq!(
        p.optional_additional_costs[0].mana,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::U]
        }
    );
    assert!(
        matches!(p.instructions.as_slice(),[Instruction::DestroyPermanent {optional:false,target_slot:0,..},Instruction::DrawCardsIfAdditionalCost {count,cost_identity,..}]
        if *count==2.into() && *cost_identity==p.optional_additional_costs[0].identity)
    );
    assert!(
        blow.capabilities
            .contains(&Capability::OptionalAdditionalManaCostV1)
    );
    let reclamation = lower(RECLAMATION);
    assert_eq!(
        reclamation.triggered[0].condition,
        TriggerCondition::SelfSpellCast
    );
    assert_eq!(
        reclamation.triggered[0].functioning_zone,
        FunctioningZone::Stack
    );
    assert_eq!(
        reclamation.spell.as_ref().unwrap().timing,
        SpellTiming::Instant
    );
    assert!(matches!(
        reclamation.spell.as_ref().unwrap().instructions.as_slice(),
        [Instruction::DestroyPermanent {
            optional: false,
            ..
        }]
    ));
}
#[test]
fn ramos_m3c_contract_missing_cost_identity_link_capability_and_other_contexts_reject() {
    let d = lower(BLOW);
    for case in 0..11 {
        let mut bad = (*d).clone();
        let p = bad.spell.as_mut().unwrap();
        match case {
            0 => p.optional_additional_costs.clear(),
            1 => p.optional_additional_costs[0].identity.clear(),
            2 => {
                p.optional_additional_costs[0].identity =
                    "card:foreign/faces/0/abilities/0/keywords/0#keyword".into()
            }
            3 => p.optional_additional_costs[0].mana.generic = (-1).into(),
            4 => p
                .optional_additional_costs
                .push(p.optional_additional_costs[0].clone()),
            5 => {
                if let Instruction::DrawCardsIfAdditionalCost { cost_identity, .. } =
                    &mut p.instructions[1]
                {
                    *cost_identity = "wrong-link".into();
                }
            }
            6 => {
                if let Instruction::DrawCardsIfAdditionalCost { count, .. } = &mut p.instructions[1]
                {
                    *count = 0.into();
                }
            }
            7 => p.instructions.push(p.instructions[0].clone()),
            8 => {
                bad.triggered = vec![TriggeredAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    condition: TriggerCondition::SelfEntersBattlefield,
                    intervening_if: None,
                    death_filter: None,
                    program: p.clone(),
                }];
            }
            9 => {
                bad.characteristics.kind = CardKind::Creature;
            }
            10 => {
                bad.capabilities
                    .remove(&Capability::OptionalAdditionalManaCostV1);
            }
            _ => unreachable!(),
        }
        if case != 10 {
            bad.capabilities = bad.requirements();
        }
        assert!(bad.validate().is_err(), "case {case}");
    }
}
#[test]
fn ramos_m3c_complete_source_near_misses_cannot_discard_cost_or_condition() {
    for case in 0..16 {
        let raw = tests::changed_source(BLOW, |r| {
            let abilities = &mut r["faces"][0]["result"]["abilities"];
            match case {
                0 => abilities[0]["keywords"][0]["keyword"] = "multikicker".into(),
                1 => abilities[0]["keywords"][0]["cost"][0]["symbols"] = serde_json::json!(["X"]),
                2 => abilities[0]["keywords"][0]["cost"] = serde_json::json!([{"cost":"tap-self"}]),
                3 => {
                    abilities[0]["keywords"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("cost");
                }
                4 => abilities[0]["keywords"][0]["extra"] = true.into(),
                5 => abilities[1]["effects"][1]["conditionKind"] = "unless".into(),
                6 => {
                    abilities[1]["effects"][1]["condition"]["comparison"]["value"]["value"] =
                        2.into()
                }
                7 => abilities[1]["effects"][1]["condition"]["what"]["noun"] = "creature".into(),
                8 => {
                    abilities[1]["effects"][1]
                        .as_object_mut()
                        .unwrap()
                        .remove("condition");
                }
                9 => abilities[1]["effects"][1]["steps"][0]["optional"] = true.into(),
                10 => {
                    let n = r["binding"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|n| n["node"] == "linked-ability")
                        .unwrap();
                    n["ability"] = "wrong".into();
                }
                11 => {
                    r["source"]["oracle_text"]="Kicker {2}{U}\nDestroy target artifact or enchantment. If this spell was kicked, draw two cards. Gain 1 life.".into();
                }
                12 => {
                    let r = r["binding"]["references"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|r| {
                            r["path"]
                                .as_str()
                                .is_some_and(|p| p.ends_with("/condition/what"))
                        })
                        .unwrap();
                    r["target"] = "foreign-source".into();
                }
                13 => {
                    let r = r["binding"]["references"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|r| r["path"].as_str().is_some_and(|p| p.ends_with("/payer")))
                        .unwrap();
                    r["target"] = "foreign-caster".into();
                }
                14 => {
                    r["faces"][0]["result"]["abilities"][1]["effects"][1]["condition"] =
                        serde_json::Value::Null
                }
                15 => {
                    r["faces"][0]["result"]["abilities"][1]["effects"][1]["conditionKind"] =
                        serde_json::Value::Null
                }
                _ => unreachable!(),
            }
            let result = &mut r["faces"][0]["result"];
            for i in 0..2 {
                result["lines"][i]["ability"] = result["abilities"][i].clone();
            }
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "case {case}"
        );
    }
}

#[test]
fn ramos_m3c_nonempty_source_line_with_empty_effects_rejects() {
    lower(BLOW);
    let raw = tests::changed_source(BLOW, |r| {
        let text = "Draw a card.";
        let source = r["source"]["oracle_text"].as_str().unwrap();
        r["source"]["oracle_text"] = format!("{source}\n{text}").into();
        let result = &mut r["faces"][0]["result"];
        let ability = serde_json::json!({"kind":"spell","effects":[]});
        result["abilities"]
            .as_array_mut()
            .unwrap()
            .push(ability.clone());
        result["lines"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"text":text,"ok":true,"ability":ability}));
    });
    let result = compile(MANIFEST, &raw);
    assert!(
        matches!(&result, CompileResult::Unsupported { path, .. } if path == "effects"),
        "complete source must not discard an empty AST block: {result:?}"
    );
}
#[test]
fn ramos_m3c_fixed_mana_and_draw_quantities_are_parameters() {
    let raw = tests::changed_source(BLOW, |r| {
        let result = &mut r["faces"][0]["result"];
        result["abilities"][0]["keywords"][0]["cost"][0]["symbols"] =
            serde_json::json!(["17", "U", "U"]);
        result["abilities"][1]["effects"][1]["steps"][0]["amount"]["value"] = 19.into();
        for i in 0..2 {
            result["lines"][i]["ability"] = result["abilities"][i].clone();
        }
    });
    // Synthetic AST operand control, not an additional admitted source.
    let d = lower(&raw);
    assert_eq!(
        d.spell.as_ref().unwrap().optional_additional_costs[0]
            .mana
            .generic,
        17.into()
    );
    assert!(
        matches!(&d.spell.as_ref().unwrap().instructions[1],Instruction::DrawCardsIfAdditionalCost {count,..} if *count==19.into())
    );
}
#[test]
fn ramos_m3c_instant_cascade_extra_keyword_arguments_and_battlefield_triggers_reject() {
    for case in 0..3 {
        let raw = tests::changed_source(RECLAMATION, |r| {
            let result = &mut r["faces"][0]["result"];
            match case {
                0 => result["abilities"][0]["keywords"][0]["cost"] = serde_json::json!([]),
                1 => result["abilities"][0]["keywords"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"keyword":"cascade"})),
                2 => {
                    result["abilities"][0] = serde_json::json!({"kind":"triggered","trigger":{"event":"enters-battlefield","what":{"ref":"this","noun":"spell"}},"effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]})
                }
                _ => unreachable!(),
            }
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(MANIFEST, &raw), CompileResult::Lowered { .. }),
            "case {case}"
        );
    }
}

#[test]
fn ramos_m3c_compound_grants_reject_unconsumed_sentence_conditions() {
    let blow: serde_json::Value = serde_json::from_str(BLOW).unwrap();
    let condition = blow["faces"][0]["result"]["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["effects"].as_array())
        .flatten()
        .find_map(|sentence| sentence.get("condition"))
        .unwrap()
        .clone();
    for raw in [
        include_str!("../../../tests/fixtures/058a8475-649f-433a-b258-42bc03bf40ba.json"),
        include_str!("../../../tests/fixtures/0cd28580-d718-4d0b-96e3-a836c2911c27.json"),
    ] {
        lower(raw).validate().unwrap();
        for variant in 0..3 {
            let changed = tests::changed_source(raw, |r| {
                let sentence = &mut r["faces"][0]["result"]["abilities"][0]["effects"][0];
                if variant != 1 {
                    sentence["conditionKind"] = "if".into();
                }
                if variant != 0 {
                    sentence["condition"] = condition.clone();
                }
            });
            assert!(matches!(
                compile(MANIFEST, &changed),
                CompileResult::Unsupported { .. }
            ));
        }
    }
}
