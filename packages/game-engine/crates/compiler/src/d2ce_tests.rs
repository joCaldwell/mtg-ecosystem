//! Source-only candidate tests. Complete bounded archive rows, never expected IR.
use super::*;
const MANIFEST: &str =
    include_str!("../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json");
const FOX: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/51f4eb1f-abf2-425a-b78e-cafac177da89.json"
);
const OTTER: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/5dee663f-9e2e-4503-8086-fc51664fdf29.json"
);
const COMMANDO: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/48936751-429b-4c40-88f5-72acde6278cc.json"
);
const CUTPURSE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/56c1a548-c80c-432b-bf92-d4ef6117b298.json"
);
const DEMON: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/2e55fc03-779c-4a71-beb7-aa0ade738c24.json"
);
const RESEARCH: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
);
fn rows() -> Vec<(serde_json::Value, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/cards-data-d2ce");
    let p: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("provenance.json")).unwrap())
            .unwrap();
    p["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r.clone(),
                std::fs::read_to_string(root.join(r["file"].as_str().unwrap())).unwrap(),
            )
        })
        .collect()
}
fn lower(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn changed(f: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(FOX, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        f(a);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2ce_complete_archive_rows_provenance_programs_and_whole_card_negatives() {
    let mut admitted = 0;
    for (row, raw) in rows() {
        assert_eq!(sha256(raw.trim_end().as_bytes()), row["record_sha256"]);
        let source: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(source["source"]["legalities"]["commander"], "legal");
        assert_eq!(source["sourceSha256"], row["source_sha256"]);
        assert_eq!(source["astSha256"], row["ast_sha256"]);
        // IR26 adds the complete Thieving Otter source at archive line 14102:
        // no combat restriction, exactly an opponent, and one mandatory draw.
        // This changes its former unsupported disposition for reviewed semantics;
        // the original seven IR25 witnesses retain their original requirements.
        // IR27 additionally admits Neurok Commando (line 10885): its complete
        // shroud plus combat/player trigger with a resolution-time optional draw.
        // IR28 admits Dimir Cutpurse (line 13027): the damaged player discards
        // one card, followed by the invocation controller's mandatory draw.
        // IR29 admits Demon of Loathing (line 6875): the damaged player chooses
        // and sacrifices one controlled creature; flying/trample are retained.
        // IR30 admits Research Thief (line 28052): a controlled artifact
        // creature deals damage; complete flash/flying source remains intact.
        let otter = row["archive_line"] == 14102;
        let commando = row["archive_line"] == 10885;
        let cutpurse = row["archive_line"] == 13027;
        let demon = row["archive_line"] == 6875;
        let research = row["archive_line"] == 28052;
        if ![
            4538, 6875, 9081, 10885, 12306, 13027, 14102, 14941, 19442, 21636, 28052, 28189,
        ]
        .contains(&row["archive_line"].as_u64().unwrap())
        {
            assert!(
                matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
                "{}: {:?}",
                source["source"]["name"],
                compile(MANIFEST, &raw)
            );
            continue;
        }
        admitted += 1;
        let d = lower(&raw);
        assert_eq!(d.triggered.len(), 1);
        let expected = if research {
            TriggerCondition::DamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::All {
                        terms: vec![
                            TypePredicate::Type {
                                kind: CardKind::Artifact,
                            },
                            TypePredicate::Type {
                                kind: CardKind::Creature,
                            },
                        ],
                    },
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            }
        } else if otter {
            TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Any,
                recipient: DamageTriggerRecipient::OpponentPlayer,
            }
        } else {
            TriggerCondition::DamageDealt {
                source: DamageTriggerSource::Source {},
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
            }
        };
        assert_eq!(d.triggered[0].condition, expected);
        assert_eq!(d.capabilities, d.requirements());
        let capability = if otter {
            Capability::SourceDamageTriggerV2
        } else {
            Capability::DamageTriggerV1
        };
        assert!(d.capabilities.contains(&capability));
        assert_eq!(d.provenance.source_sha256, row["source_sha256"]);
        assert_eq!(d.provenance.ast_sha256, row["ast_sha256"]);
        let mut bad = d.clone();
        bad.capabilities.remove(&capability);
        assert!(bad.validate().is_err());
        for version in 24..=mtg_contracts::CURRENT_IR_VERSION + 1 {
            let c = Catalog {
                schema_version: version,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            assert_eq!(
                c.validate().is_ok(),
                ((if research {
                    30
                } else if demon {
                    29
                } else if cutpurse {
                    28
                } else if commando {
                    27
                } else if otter {
                    26
                } else {
                    25
                })..=mtg_contracts::CURRENT_IR_VERSION)
                    .contains(&version),
                "{} v{version}",
                d.name
            );
        }
    }
    assert_eq!(admitted, 12);
}
#[test]
fn d2ce_rejects_partial_kind_source_recipient_optional_quantity_and_condition_shapes() {
    let mut mutations = vec![];
    for v in [
        serde_json::json!(false),
        serde_json::json!(null),
        serde_json::json!("true"),
    ] {
        mutations.push(changed(|a| a["trigger"]["combat"] = v));
    }
    mutations.push(changed(|a| {
        a["trigger"].as_object_mut().unwrap().remove("combat");
    }));
    for v in [
        serde_json::json!({"player":"an-opponent"}),
        serde_json::json!({"player":"a-player"}),
        serde_json::json!({"ref":"this","noun":"creature"}),
        serde_json::json!(null),
    ] {
        mutations.push(changed(|a| a["trigger"]["to"] = v));
    }
    for v in [
        serde_json::json!({"ref":"it"}),
        // An unrestricted controller relation remains outside the IR30 slice.
        serde_json::json!({"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}),
        serde_json::json!({"ref":"this","noun":"permanent"}),
        serde_json::json!({"ref":"this","noun":"creature","extra":true}),
    ] {
        mutations.push(changed(|a| a["trigger"]["what"] = v));
    }
    for (k, v) in [
        ("grouping", serde_json::json!("one-or-more")),
        ("unknown", serde_json::json!(true)),
        ("amount", serde_json::json!({"amount":"fixed","value":2})),
        (
            "occurrence",
            serde_json::json!({"ordinal":1,"period":"turn"}),
        ),
    ] {
        mutations.push(changed(|a| a["trigger"][k] = v));
    }
    mutations.push(changed(|a| {
        // Controller optional draw is supported at IR27; optional draw of two
        // cards remains outside the complete supported instruction shape.
        a["effects"][0]["steps"][0]["optional"] = true.into();
        a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"that-much"})
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"})
    }));
    mutations.push(changed(|a| a["condition"] = serde_json::json!({"condition":"status","what":{"ref":"this","noun":"creature"},"status":"tapped"})));
    mutations.push(changed(|a| a["functioningZone"] = "graveyard".into()));
    for raw in mutations {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    // Source-name canonicalization requires the exact archive binding. These
    // are explicitly synthetic source/binding mutations, not archived cards.
    let unbound = changed(|a| a["trigger"]["what"] = serde_json::json!({"ref":"self"}));
    assert!(matches!(
        compile(MANIFEST, &unbound),
        CompileResult::Unsupported { .. }
    ));
    let bound = tests::changed_source(&unbound, |r| {
        let reference = r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["path"] == "/faces/0/abilities/0/trigger/what")
            .unwrap();
        reference["expression"] = serde_json::json!({"ref":"self"});
    });
    assert!(
        lower(&bound)
            .capabilities
            .contains(&Capability::DamageTriggerV1)
    );
}
#[test]
fn d2ce_ir_strict_dimensions_mandatory_program_and_minimum_version() {
    let d = lower(FOX);
    for condition in [
        serde_json::json!({"damage_dealt":{"source":{"subject":"source","extra":true},"kind":"combat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"filter"},"kind":"combat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"noncombat","recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":true,"recipient":"player"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"combat","recipient":"opponent"}}),
        serde_json::json!({"damage_dealt":{"source":{"subject":"source"},"kind":"combat","recipient":"player","unknown":1}}),
    ] {
        assert!(serde_json::from_value::<TriggerCondition>(condition).is_err());
    }
    let mut event = d.clone();
    if let Instruction::DrawCards { recipient, .. } =
        &mut event.triggered[0].program.instructions[0]
    {
        *recipient = Some(LifeRecipient::EventPlayer);
    } else {
        panic!("draw");
    }
    event.capabilities = event.requirements();
    assert!(event.validate().is_err());
    let mut optional = d.clone();
    optional.triggered[0].intervening_if = Some(InterveningCondition::ControllerHandEmpty);
    optional.capabilities = optional.requirements();
    assert!(optional.validate().is_err());
}

#[test]
fn source_damage_v2_exact_otter_source_and_provenance() {
    let d = lower(OTTER);
    let source: serde_json::Value = serde_json::from_str(OTTER).unwrap();
    assert_eq!(d.name, "Thieving Otter");
    assert_eq!(
        source["source"]["oracle_text"],
        "Whenever this creature deals damage to an opponent, draw a card."
    );
    assert_eq!(
        source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(d.triggered.len(), 1);
    let a = &d.triggered[0];
    assert_eq!(
        a.condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Any,
            recipient: DamageTriggerRecipient::OpponentPlayer,
        }
    );
    assert!(a.program.targets.is_empty());
    assert!(a.program.modes.is_empty());
    assert!(a.intervening_if.is_none());
    assert!(
        matches!(a.program.instructions.as_slice(), [Instruction::DrawCards {
        count, recipient: None, source_block: 0
    }] if *count == Quantity::from(1))
    );
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    assert!(d.capabilities.contains(&Capability::SourceDamageTriggerV2));
    assert!(!d.capabilities.contains(&Capability::DamageTriggerV1));
    let mut missing = d.clone();
    missing
        .capabilities
        .remove(&Capability::SourceDamageTriggerV2);
    assert!(missing.validate().is_err());
    missing.capabilities.insert(Capability::DamageTriggerV1);
    assert!(missing.validate().is_err());
}

#[test]
fn source_damage_v2_rejects_unreviewed_source_kind_recipient_and_program_shapes() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| a["trigger"]["combat"] = false.into()) as Mutation,
        |a| a["trigger"]["combat"] = true.into(),
        |a| a["trigger"]["combat"] = serde_json::Value::Null,
        |a| a["trigger"]["combat"] = "false".into(),
        |a| a["trigger"]["to"] = "any".into(),
        |a| a["trigger"]["to"] = serde_json::json!({"player":"a-player"}),
        |a| a["trigger"]["to"] = serde_json::json!({"player":"an-opponent","of":{"ref":"it"}}),
        |a| {
            a["trigger"]["to"] = serde_json::json!({"player":"an-opponent","selectionId":"unbound"})
        },
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"it"}),
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"this","noun":"permanent"}),
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}),
        |a| a["trigger"]["grouping"] = "one-or-more".into(),
        |a| {
            a["effects"][0]["steps"][0]["optional"] = true.into();
            a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
        },
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"that-much"}),
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"}),
        |a| a["condition"] = serde_json::json!({"condition":"status","what":{"ref":"this","noun":"creature"},"status":"tapped"}),
    ] {
        let raw = tests::changed_source(OTTER, |r| {
            mutate(&mut r["faces"][0]["result"]["abilities"][0]);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}

#[test]
fn optional_draw_exact_commando_source_and_provenance() {
    let d = lower(COMMANDO);
    let source: serde_json::Value = serde_json::from_str(COMMANDO).unwrap();
    assert_eq!(d.name, "Neurok Commando");
    assert_eq!(d.keywords, [KeywordAbility::Shroud]);
    assert_eq!(
        source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(d.triggered.len(), 1);
    let a = &d.triggered[0];
    assert_eq!(
        a.condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        }
    );
    assert_eq!(
        a.program.instructions,
        [Instruction::OptionalDrawCards { source_block: 1 }]
    );
    assert!(a.program.targets.is_empty());
    assert!(a.program.modes.is_empty());
    assert!(a.intervening_if.is_none());
    assert!(
        d.capabilities
            .contains(&Capability::OptionalControllerDrawV1)
    );
    assert!(d.capabilities.contains(&Capability::DamageTriggerV1));
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    let mut missing = d.clone();
    missing
        .capabilities
        .remove(&Capability::OptionalControllerDrawV1);
    assert!(missing.validate().is_err());

    // Missing/false retains the existing mandatory one-card instruction, with
    // every source block and trigger dimension otherwise unchanged.
    for optional in [None, Some(false)] {
        let raw = tests::changed_source(COMMANDO, |r| {
            let result = &mut r["faces"][0]["result"];
            let step = &mut result["abilities"][1]["effects"][0]["steps"][0];
            if let Some(value) = optional {
                step["optional"] = value.into();
            } else {
                step.as_object_mut().unwrap().remove("optional");
            }
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        let mandatory = lower(&raw);
        assert_eq!(
            mandatory.triggered[0].program.instructions,
            [Instruction::DrawCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 1
            }]
        );
        assert!(
            !mandatory
                .capabilities
                .contains(&Capability::OptionalControllerDrawV1)
        );
    }
}

#[test]
fn optional_draw_rejects_other_recipients_quantities_and_malformed_options() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|s: &mut serde_json::Value| s["amount"]["value"] = 0.into()) as Mutation,
        |s| s["amount"]["value"] = 2.into(),
        |s| s["amount"] = serde_json::json!({"amount":"that-much"}),
        |s| s["who"] = serde_json::json!({"player":"target-player","selectionId":"target"}),
        |s| s["who"] = serde_json::json!({"player":"that-player"}),
        |s| s["who"] = serde_json::json!({"player":"each-opponent"}),
        |s| s["who"] = serde_json::json!({"player":"you","of":{"ref":"it"}}),
        |s| s["who"] = serde_json::json!({"player":"you","selectionId":"qualified"}),
        |s| s["optional"] = serde_json::Value::Null,
        |s| s["optional"] = "true".into(),
        |s| s["optional"] = serde_json::json!({"may":true}),
        |s| s["unknown"] = true.into(),
    ] {
        let raw = tests::changed_source(COMMANDO, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][1]["effects"][0]["steps"][0]);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}

#[test]
fn event_player_discard_exact_cutpurse_source_provenance_and_binding() {
    let d = lower(CUTPURSE);
    let source: serde_json::Value = serde_json::from_str(CUTPURSE).unwrap();
    assert_eq!(d.name, "Dimir Cutpurse");
    assert_eq!(
        source["source"]["oracle_text"],
        "Whenever this creature deals combat damage to a player, that player discards a card and you draw a card."
    );
    assert_eq!(d.triggered.len(), 1);
    let a = &d.triggered[0];
    assert_eq!(
        a.condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        }
    );
    assert_eq!(
        a.program.instructions,
        [
            Instruction::DiscardCards {
                recipient: Some(ActionRecipient::EventPlayer),
                count: Quantity::from(1),
                source_block: 0
            },
            Instruction::DrawCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 0
            },
        ]
    );
    assert!(a.program.targets.is_empty());
    assert!(a.program.modes.is_empty());
    assert!(a.intervening_if.is_none());
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    assert!(d.capabilities.contains(&Capability::EventPlayerDiscardV1));
    assert!(!d.capabilities.contains(&Capability::ControllerDiscardV1));
    assert!(!d.capabilities.contains(&Capability::EventPlayerRecipientV1));
    let mut missing = d.clone();
    missing
        .capabilities
        .remove(&Capability::EventPlayerDiscardV1);
    assert!(missing.validate().is_err());
}

#[test]
fn event_player_discard_rejects_unbound_qualified_and_unreviewed_programs() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| a["trigger"]["combat"] = false.into()) as Mutation,
        |a| {
            a["trigger"].as_object_mut().unwrap().remove("combat");
        },
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"it"}),
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"this","noun":"permanent"}),
        |a| a["trigger"]["to"] = serde_json::json!({"player":"a-player"}),
        |a| a["trigger"] = serde_json::json!({"trigger":"phase","phase":"upkeep","whose":"your"}),
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}})
        },
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"you"}),
        |a| {
            a["effects"][0]["steps"][0]["who"] =
                serde_json::json!({"player":"target-player","selectionId":"target"})
        },
        |a| a["effects"][0]["steps"][0]["who"]["of"] = serde_json::json!({"ref":"it"}),
        |a| a["effects"][0]["steps"][0]["who"]["selectionId"] = "qualified".into(),
        |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 0.into(),
        |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 2.into(),
        |a| a["effects"][0]["steps"][0]["what"]["filter"]["cls"] = "creature".into(),
        |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
        |a| a["effects"][0]["steps"][1]["who"] = serde_json::json!({"player":"that-player"}),
    ] {
        let raw = tests::changed_source(CUTPURSE, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][0]);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    // Damage recipient discard does not extend the older Draw/Life event-player
    // binding, which remains restricted to supported step and cast triggers.
    let mut damage_draw = lower(FOX);
    damage_draw.triggered[0].program.instructions = vec![Instruction::DrawCards {
        recipient: Some(LifeRecipient::EventPlayer),
        count: Quantity::from(1),
        source_block: 0,
    }];
    damage_draw.capabilities = damage_draw.requirements();
    assert!(damage_draw.validate().is_err());
    let mut damage_life = damage_draw;
    damage_life.triggered[0].program.instructions = vec![Instruction::GainLife {
        recipient: LifeRecipient::EventPlayer,
        amount: Quantity::from(1),
        source_block: 0,
    }];
    damage_life.capabilities = damage_life.requirements();
    assert!(damage_life.validate().is_err());
    for recipient in [None, Some(ActionRecipient::Controller)] {
        let mut controller = lower(CUTPURSE);
        if let Instruction::DiscardCards { recipient: r, .. } =
            &mut controller.triggered[0].program.instructions[0]
        {
            *r = recipient;
        }
        controller.capabilities = controller.requirements();
        assert!(
            controller.validate().is_err(),
            "controller discard trigger is not admitted"
        );
    }
}

#[test]
fn event_player_sacrifice_exact_demon_source_provenance_and_binding() {
    let d = lower(DEMON);
    let source: serde_json::Value = serde_json::from_str(DEMON).unwrap();
    assert_eq!(d.name, "Demon of Loathing");
    assert_eq!(
        d.keywords,
        [KeywordAbility::Flying, KeywordAbility::Trample]
    );
    assert_eq!(
        source["source"]["oracle_text"],
        "Flying, trample\nWhenever this creature deals combat damage to a player, that player sacrifices a creature of their choice."
    );
    assert_eq!(
        source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(d.triggered.len(), 1);
    let a = &d.triggered[0];
    assert_eq!(
        a.condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::Source {},
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        }
    );
    assert_eq!(
        a.program.instructions,
        [Instruction::SacrificeCreature {
            recipient: ActionRecipient::EventPlayer,
            source_block: 1,
        }]
    );
    assert!(a.program.targets.is_empty());
    assert!(a.program.modes.is_empty());
    assert!(a.intervening_if.is_none());
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    assert!(d.capabilities.contains(&Capability::EventPlayerSacrificeV1));
    assert!(!d.capabilities.contains(&Capability::EventPlayerDiscardV1));
    assert!(!d.capabilities.contains(&Capability::EventPlayerRecipientV1));
    let mut missing = d.clone();
    missing
        .capabilities
        .remove(&Capability::EventPlayerSacrificeV1);
    assert!(missing.validate().is_err());
    let mut controller = d;
    controller.triggered[0].program.instructions = vec![Instruction::SacrificeCreature {
        recipient: ActionRecipient::Controller,
        source_block: 1,
    }];
    controller.capabilities = controller.requirements();
    assert!(
        controller.validate().is_err(),
        "controller sacrifice is not admitted"
    );
}

#[test]
fn event_player_sacrifice_rejects_qualified_choosers_cardinality_and_unbound_sources() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| a["trigger"]["combat"] = false.into()) as Mutation,
        |a| {
            a["trigger"].as_object_mut().unwrap().remove("combat");
        },
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"it"}),
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"this","noun":"permanent"}),
        |a| a["trigger"] = serde_json::json!({"trigger":"phase","phase":"upkeep","whose":"your"}),
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"cast","who":{"player":"you"},"what":{"cls":"spell"}})
        },
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}})
        },
        |a| a["effects"][0]["steps"][0]["chosenBy"] = serde_json::json!({"player":"you"}),
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"you"}),
        |a| {
            for actor in ["who", "chosenBy"] {
                a["effects"][0]["steps"][0][actor]["of"] = serde_json::json!({"ref":"it"});
            }
        },
        |a| {
            for actor in ["who", "chosenBy"] {
                a["effects"][0]["steps"][0][actor]["selectionId"] = "qualified".into();
            }
        },
        |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 0.into(),
        |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 2.into(),
        |a| {
            a["effects"][0]["steps"][0]["what"]["filter"] =
                serde_json::json!({"types":["artifact"]})
        },
        |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
    ] {
        let raw = tests::changed_source(DEMON, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][1]);
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}

#[test]
fn filtered_damage_exact_research_source_provenance_and_conjunctive_types() {
    let d = lower(RESEARCH);
    let source: serde_json::Value = serde_json::from_str(RESEARCH).unwrap();
    assert_eq!(d.name, "Research Thief");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert_eq!(d.characteristics.additional_types, [CardKind::Artifact]);
    assert_eq!(d.keywords, [KeywordAbility::Flash, KeywordAbility::Flying]);
    assert_eq!(d.spell.as_ref().unwrap().timing, SpellTiming::Instant);
    assert_eq!(
        source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(d.triggered.len(), 1);
    let a = &d.triggered[0];
    assert_eq!(
        a.condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::CreatureFilter {
                controller: DamageSourceRelation::Controller,
                types: TypePredicate::All {
                    terms: vec![
                        TypePredicate::Type {
                            kind: CardKind::Artifact
                        },
                        TypePredicate::Type {
                            kind: CardKind::Creature
                        },
                    ]
                },
            },
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        }
    );
    assert_eq!(
        a.program.instructions,
        [Instruction::DrawCards {
            recipient: None,
            count: Quantity::from(1),
            source_block: 2
        }]
    );
    assert!(
        a.program.targets.is_empty() && a.program.modes.is_empty() && a.intervening_if.is_none()
    );
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    assert!(d.capabilities.contains(&Capability::DamageTriggerV1));
    assert!(
        d.capabilities
            .contains(&Capability::FilteredDamageTriggerV1)
    );
    assert!(!d.capabilities.contains(&Capability::SourceDamageTriggerV2));
    let mut missing = d;
    missing
        .capabilities
        .remove(&Capability::FilteredDamageTriggerV1);
    assert!(missing.validate().is_err());
}

#[test]
fn filtered_damage_preserves_controlled_sources_and_boolean_types() {
    let raw = tests::changed_source(RESEARCH, |r| {
        let result = &mut r["faces"][0]["result"];
        let filter = &mut result["abilities"][2]["trigger"]["what"]["filter"];
        filter["nonTypes"] = serde_json::json!(["enchantment"]);
        filter["anyOf"] = serde_json::json!([{"types":["artifact"]},{"types":["enchantment"]}]);
        result["lines"][2]["ability"] = result["abilities"][2].clone();
    });
    let d = lower(&raw);
    let TriggerCondition::DamageDealt {
        source: DamageTriggerSource::CreatureFilter { controller, types },
        ..
    } = &d.triggered[0].condition
    else {
        panic!("filter");
    };
    assert_eq!(*controller, DamageSourceRelation::Controller);
    types.validate().unwrap();
    let mut characteristics = d.characteristics.clone();
    assert!(types.matches(&characteristics));
    characteristics.additional_types.clear();
    assert!(
        !types.matches(&characteristics),
        "artifact+creature is conjunctive"
    );
    characteristics.additional_types = vec![CardKind::Artifact, CardKind::Enchantment];
    assert!(
        !types.matches(&characteristics),
        "negative type is retained"
    );
    // The complete controlled creature filter is supported, whereas an absent
    // controller relation in the historical negatives remains unsupported.
    let d = lower(&changed(
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"filter","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1}}),
    ));
    assert_eq!(
        d.triggered[0].condition,
        TriggerCondition::DamageDealt {
            source: DamageTriggerSource::CreatureFilter {
                controller: DamageSourceRelation::Controller,
                types: TypePredicate::Type {
                    kind: CardKind::Creature
                }
            },
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
        }
    );
}

#[test]
fn filtered_damage_rejects_partial_qualifiers_noncreature_narrowing_and_predicate_exhaustion() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|w: &mut serde_json::Value| w["filter"]["types"] = serde_json::json!(["artifact"]))
            as Mutation,
        |w| w["filter"]["types"] = serde_json::json!([]),
        |w| w["filter"]["types"] = serde_json::json!(["creature", "future-kind"]),
        |w| w["count"]["value"] = 0.into(),
        |w| w["count"]["value"] = 2.into(),
        |w| {
            w["filter"].as_object_mut().unwrap().remove("control");
        },
        |w| w["filter"]["control"] = "opponent".into(),
        |w| w["filter"]["control"] = "any_player".into(),
        |w| w["filter"]["control"] = "owner".into(),
        |w| w["filter"]["control"] = serde_json::Value::Null,
        |w| w["filter"]["nonTypes"] = serde_json::Value::Null,
        |w| w["filter"]["owner"] = "you".into(),
        |w| w["filter"]["other"] = true.into(),
        |w| w["filter"]["status"] = serde_json::json!(["attacking"]),
        |w| w["grouping"] = "one-or-more".into(),
        |w| w["amount"] = serde_json::json!({"amount":"fixed","value":1}),
        |w| w["filter"]["anyOf"] = serde_json::json!([]),
        |w| w["filter"]["types"] = serde_json::json!(vec!["creature"; 33]),
        |w| {
            let mut child = serde_json::json!({"types":["creature"]});
            for _ in 0..10 {
                child = serde_json::json!({"allOf":[child]});
            }
            w["filter"]["allOf"] = serde_json::json!([child]);
        },
    ] {
        let raw = tests::changed_source(RESEARCH, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][2]["trigger"]["what"]);
            result["lines"][2]["ability"] = result["abilities"][2].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    let frostfang = include_str!(
        "../../../tests/fixtures/cards-data-d2ce/b99ada26-9a61-4175-9fb8-15a106960220.json"
    );
    assert!(matches!(
        compile(MANIFEST, frostfang),
        CompileResult::Unsupported { .. }
    ));
}
