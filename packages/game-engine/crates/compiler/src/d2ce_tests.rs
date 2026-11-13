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
const LOCKE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/bdcb7aed-3595-4c7e-b9da-543e92de919a.json"
);
const FEAR: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/34fd461b-a913-4a60-9477-ce2bd5c6fc5a.json"
);
const OHRAN: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/b99ada26-9a61-4175-9fb8-15a106960220.json"
);
const KUTZIL: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2ce/3233af43-7826-4895-b67e-03c6102c2cd5.json"
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
        // The complete Thieving Otter source at archive line 14102:
        // no combat restriction, exactly an opponent, and one mandatory draw.
        // This changes its former unsupported disposition for reviewed semantics;
        // the original seven combat witnesses retain their distinct requirements.
        // Neurok Commando is supported (line 10885): its complete
        // shroud plus combat/player trigger with a resolution-time optional draw.
        // Dimir Cutpurse is supported (line 13027): the damaged player discards
        // one card, followed by the invocation controller's mandatory draw.
        // Demon of Loathing is supported (line 6875): the damaged player chooses
        // and sacrifices one controlled creature; flying/trample are retained.
        // Research Thief is supported (line 28052): a controlled artifact
        // creature deals damage; complete flash/flying source remains intact.
        // Locke Cole is supported (line 28579): its combat damage trigger draws
        // then discards for the controller, retaining deathtouch and lifelink.
        let otter = row["archive_line"] == 14102;
        let research = row["archive_line"] == 28052;
        // Fear of Failed Tests is supported (line7911): its standalone
        // mandatory controller draw uses the actual triggering damage quantity.
        // The complete Ohran Frostfang is supported (line27940): preserve Snow
        // and the dynamic attacking-creature grant, plus its controlled-creature damage trigger.
        let ohran = row["archive_line"] == 27940;
        // The unreleased capabilities complete Kutzil (line7477): the casting
        // prohibition and candidate-local grouped power predicate have Rust binders.
        // Its original diagnostic artifact remains fullyBound:false unchanged.
        let kutzil = row["archive_line"] == 7477;
        if ![
            4538, 6875, 7477, 7911, 9081, 10885, 12306, 13027, 14102, 14941, 19442, 21636, 27940,
            28052, 28189, 28579,
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
        let expected = if kutzil {
            TriggerCondition::GroupedDamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Type {
                        kind: CardKind::Creature,
                    },
                },
                kind: TriggerDamageKind::Combat,
                recipient: DamageTriggerRecipient::Player,
                source_power: DamageSourcePowerPredicate::GreaterThanBase,
            }
        } else if research {
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
        } else if ohran {
            TriggerCondition::DamageDealt {
                source: DamageTriggerSource::CreatureFilter {
                    controller: DamageSourceRelation::Controller,
                    types: TypePredicate::Type {
                        kind: CardKind::Creature,
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
        let capability = if kutzil {
            Capability::GroupedCreatureDamageTriggerV1
        } else if otter {
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
        Catalog {
            schema_version: mtg_contracts::CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![d.clone()],
        }
        .validate()
        .unwrap();
    }
    assert_eq!(admitted, 16);
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
        // An unrestricted controller relation remains outside the controlled-creature source scope.
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
        // Controller optional draw is supported; optional draw of two
        // cards remains outside the complete supported instruction shape.
        a["effects"][0]["steps"][0]["optional"] = true.into();
        a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["amount"] =
            serde_json::json!({"amount":"that-much", "of":{"ref":"it"}})
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
fn d2ce_ir_strict_dimensions_mandatory_program_and_capability_closure() {
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
        |a| {
            a["effects"][0]["steps"][0]["amount"] =
                serde_json::json!({"amount":"that-much", "of":{"ref":"it"}})
        },
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
        // Unqualified controller discard is supported in damage triggers;
        // a qualified controller selector still has no supported meaning.
        |a| {
            a["effects"][0]["steps"][0]["who"] =
                serde_json::json!({"player":"you","of":{"ref":"it"}})
        },
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
        // Keep the contextual rejection: controller discard is admitted only
        // for damage triggers, never an unversioned general entry trigger.
        controller.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
        controller.capabilities = controller.requirements();
        assert!(
            controller.validate().is_err(),
            "controller discard entry trigger is not admitted"
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
    // The complete Frostfang source retains Snow, the attacking keyword grant,
    // and its filtered damage trigger with their independent capabilities.
    let frostfang = lower(OHRAN);
    assert!(frostfang.characteristics.snow);
    assert!(
        frostfang
            .capabilities
            .contains(&Capability::FilteredDamageTriggerV1)
    );
    assert!(
        frostfang
            .capabilities
            .contains(&Capability::SnowSupertypeV1)
    );
    assert!(
        frostfang
            .capabilities
            .contains(&Capability::AttackingCreatureKeywordsV1)
    );
    assert_eq!(
        frostfang.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![]
            },
        }]
    );
    Catalog {
        schema_version: mtg_contracts::CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions: vec![frostfang],
    }
    .validate()
    .unwrap();
}

#[test]
fn damage_controller_discard_exact_locke_source_provenance_and_order() {
    let d = lower(LOCKE);
    let source: serde_json::Value = serde_json::from_str(LOCKE).unwrap();
    assert_eq!(d.name, "Locke Cole");
    assert!(d.characteristics.legendary);
    assert_eq!(
        d.keywords,
        [KeywordAbility::Deathtouch, KeywordAbility::Lifelink]
    );
    assert_eq!(
        source["source"]["oracle_text"],
        "Deathtouch, lifelink\nWhenever Locke Cole deals combat damage to a player, draw a card, then discard a card."
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
        [
            Instruction::DrawCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 1
            },
            Instruction::DiscardCards {
                recipient: None,
                count: Quantity::from(1),
                source_block: 1
            },
        ]
    );
    assert!(
        a.program.targets.is_empty() && a.program.modes.is_empty() && a.intervening_if.is_none()
    );
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    for capability in [
        Capability::DamageTriggerV1,
        Capability::ControllerDrawV1,
        Capability::ControllerDiscardV1,
        Capability::DamageControllerDiscardV1,
    ] {
        assert!(d.capabilities.contains(&capability));
    }
    assert!(!d.capabilities.contains(&Capability::EventPlayerDiscardV1));
    let mut missing = d;
    missing
        .capabilities
        .remove(&Capability::DamageControllerDiscardV1);
    assert!(missing.validate().is_err());
    let unbound = tests::changed_source(LOCKE, |r| {
        r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["path"] != "/faces/0/abilities/1/trigger/what");
    });
    assert!(matches!(
        compile(MANIFEST, &unbound),
        CompileResult::Unsupported { .. }
    ));
}

#[test]
fn damage_controller_discard_rejects_other_contexts_qualifiers_filters_and_counts() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| {
            a["trigger"] =
                serde_json::json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}})
        }) as Mutation,
        |a| a["trigger"] = serde_json::json!({"trigger":"phase","phase":"upkeep","whose":"your"}),
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"cast","who":{"player":"you"},"what":{"cls":"spell"}})
        },
        |a| {
            a["effects"][0]["steps"][1]["who"] =
                serde_json::json!({"player":"target-player","selectionId":"target"})
        },
        |a| a["effects"][0]["steps"][1]["who"]["of"] = serde_json::json!({"ref":"it"}),
        |a| a["effects"][0]["steps"][1]["who"]["selectionId"] = "qualified".into(),
        |a| a["effects"][0]["steps"][1]["what"]["count"]["value"] = 0.into(),
        |a| a["effects"][0]["steps"][1]["what"]["count"]["value"] = 2.into(),
        |a| a["effects"][0]["steps"][1]["what"]["count"]["amount"] = "x".into(),
        |a| a["effects"][0]["steps"][1]["what"]["filter"]["cls"] = "creature".into(),
        |a| {
            a["effects"][0]["steps"][1]["what"]["filter"]["types"] = serde_json::json!(["creature"])
        },
        |a| a["effects"][0]["steps"][1]["what"]["ref"] = "target".into(),
        |a| a["effects"][0]["steps"][1]["optional"] = true.into(),
    ] {
        let raw = tests::changed_source(LOCKE, |r| {
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
fn damage_quantity_draw_exact_fear_source_and_diagnostic_provenance() {
    let d = lower(FEAR);
    let source: serde_json::Value = serde_json::from_str(FEAR).unwrap();
    assert_eq!(d.name, "Fear of Failed Tests");
    assert_eq!(
        source["source"]["oracle_text"],
        "Whenever this creature deals combat damage to a player, draw that many cards."
    );
    assert_eq!(source["source"]["power"], "2");
    assert_eq!(source["source"]["toughness"], "7");
    assert!(d.characteristics.has_type(CardKind::Enchantment));
    assert!(d.characteristics.has_type(CardKind::Creature));
    assert!(d.keywords.is_empty() && d.static_abilities.is_empty() && d.activated.is_empty());
    assert_eq!(d.triggered.len(), 1);
    assert_eq!(
        d.triggered[0].program.instructions,
        [Instruction::DrawDamageAmount { source_block: 0 }]
    );
    assert!(d.triggered[0].program.targets.is_empty());
    assert!(d.triggered[0].program.modes.is_empty());
    assert!(d.capabilities.contains(&Capability::DamageTriggerV1));
    assert!(d.capabilities.contains(&Capability::DamageQuantityDrawV1));
    assert!(!d.capabilities.contains(&Capability::ControllerDrawV1));
    assert_eq!(d.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, source["bindingSha256"]);
    let reference = source["binding"]["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == "/faces/0/abilities/0/effects/0/steps/0/amount")
        .unwrap();
    assert_eq!(
        reference["expression"],
        serde_json::json!({"amount":"that-much"})
    );
    assert_eq!(
        reference["target"],
        "card:34fd461b-a913-4a60-9477-ce2bd5c6fc5a/faces/0/abilities/0/trigger#quantity"
    );
    // Semantic scope is proved from the complete typed AST. The diagnostic
    // graph is retained provenance, not authority for executable quantities.
    let diagnostic = tests::changed_source(FEAR, |r| {
        r["binding"]["references"] = serde_json::json!([]);
    });
    assert_eq!(
        lower(&diagnostic).triggered[0].program,
        d.triggered[0].program
    );
    for optional in [None, Some(false)] {
        let mandatory = tests::changed_source(FEAR, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            if let Some(v) = optional {
                a["effects"][0]["steps"][0]["optional"] = v.into();
            }
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert_eq!(
            lower(&mandatory).triggered[0].program,
            d.triggered[0].program
        );
    }
}

#[test]
fn damage_quantity_draw_rejects_unbound_ambiguous_and_qualified_source_shapes() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| a["effects"][0]["steps"][0]["optional"] = true.into())
            as Mutation,
        |a| a["effects"][0]["steps"][0]["optional"] = serde_json::Value::Null,
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"}),
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"each-opponent"}),
        |a| {
            a["effects"][0]["steps"][0]["who"] =
                serde_json::json!({"player":"target-player","selectionId":"target"})
        },
        |a| a["effects"][0]["steps"][0]["who"]["of"] = serde_json::json!({"ref":"it"}),
        |a| a["effects"][0]["steps"][0]["who"]["selectionId"] = "qualified".into(),
        |a| a["effects"][0]["steps"][0]["amount"]["value"] = 2.into(),
        |a| a["effects"][0]["steps"][0]["amount"]["of"] = serde_json::json!({"ref":"it"}),
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"that-many"}),
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"fixed"}),
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::Value::Null,
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"fixed","value":0}),
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"fixed","value":3}),
        |a| a["effects"][0]["bindings"] = serde_json::json!([{"amount":"that-much"}]),
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}})
        },
        |a| a["trigger"] = serde_json::json!({"trigger":"phase","phase":"upkeep","whose":"your"}),
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"cast","who":{"player":"you"},"what":{"cls":"spell"}})
        },
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"dies","what":{"ref":"this","noun":"creature"}})
        },
        |a| {
            a["trigger"] =
                serde_json::json!({"trigger":"attacks","what":{"ref":"this","noun":"creature"}})
        },
        |a| a["trigger"]["what"] = serde_json::json!({"ref":"it"}),
        |a| a["trigger"]["combat"] = false.into(),
        |a| {
            a["effects"][0]["steps"].as_array_mut().unwrap().insert(0,
            serde_json::json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}));
        },
        |a| {
            a["effects"][0]["steps"].as_array_mut().unwrap().insert(0,
            serde_json::json!({"effect":"damage","source":{"ref":"self"},"amount":{"amount":"fixed","value":1},"to":{"to":"any-target","selectionId":"damage"}}));
        },
        |a| {
            a["effects"].as_array_mut().unwrap().insert(0,
            serde_json::json!({"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}));
        },
        |a| {
            let draw = a["effects"][0]["steps"][0].clone();
            a["effects"] = serde_json::json!([{"steps":[{"effect":"modal","count":{"exactly":1},"options":[{"effects":[{"steps":[draw.clone()]}]},{"effects":[{"steps":[draw]}]}]}]}]);
        },
        |a| {
            a["kind"] = "spell".into();
            a.as_object_mut().unwrap().remove("trigger");
        },
        |a| {
            a["kind"] = "activated".into();
            a["costs"] = serde_json::json!([{"cost":"tap-self"}]);
            a.as_object_mut().unwrap().remove("trigger");
        },
    ] {
        let raw = tests::changed_source(FEAR, |r| {
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
    // Existing fixed-one/two lowering and byte shape remain exactly ordinary
    // DrawCards: dynamic quantities do not relax fixed cardinality checks.
    for count in [1, 2] {
        let raw = tests::changed_source(FEAR, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            a["effects"][0]["steps"][0]["amount"] =
                serde_json::json!({"amount":"fixed","value":count});
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let d = lower(&raw);
        assert_eq!(
            d.triggered[0].program.instructions,
            vec![
                Instruction::DrawCards {
                    recipient: None,
                    count: Quantity::from(1),
                    source_block: 0
                };
                count as usize
            ]
        );
        assert!(!d.capabilities.contains(&Capability::DamageQuantityDrawV1));
    }
}

#[test]
fn attacking_creature_keywords_exact_ohran_source_preserves_snow_and_both_abilities() {
    let d = lower(OHRAN);
    let r: serde_json::Value = serde_json::from_str(OHRAN).unwrap();
    assert_eq!(d.name, "Ohran Frostfang");
    assert_eq!(
        r["source"]["oracle_text"],
        "Attacking creatures you control have deathtouch.\nWhenever a creature you control deals combat damage to a player, draw a card."
    );
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.snow);
    assert_eq!(d.characteristics.subtypes, ["Snake"]);
    assert_eq!(d.characteristics.power, Some(Quantity::from(2)));
    assert_eq!(d.characteristics.toughness, Some(Quantity::from(6)));
    assert_eq!(d.characteristics.mana_cost.generic, Quantity::from(3));
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G, Color::G]);
    assert!(d.keywords.is_empty());
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifyCreatures {
                scope: CreatureScope::AttackingController,
                power: Quantity::zero(),
                toughness: Quantity::zero(),
                keywords: vec![KeywordAbility::Deathtouch],
                restrictions: vec![]
            },
        }]
    );
    assert_eq!(d.triggered.len(), 1);
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
    assert_eq!(
        d.triggered[0].program.instructions,
        [Instruction::DrawCards {
            recipient: None,
            count: Quantity::from(1),
            source_block: 1
        }]
    );
    for cap in [
        Capability::SnowSupertypeV1,
        Capability::AttackingCreatureKeywordsV1,
        Capability::FilteredDamageTriggerV1,
        Capability::DamageTriggerV1,
        Capability::ControllerDrawV1,
    ] {
        assert!(d.capabilities.contains(&cap));
        let mut missing = d.clone();
        missing.capabilities.remove(&cap);
        assert!(missing.validate().is_err());
    }
    assert!(!d.capabilities.contains(&Capability::AttachmentsV1));
    assert_eq!(d.provenance.source_sha256, r["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, r["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, r["bindingSha256"]);
    let nonsnow = tests::changed_source(OHRAN, |r| {
        r["source"]["type_line"] = "Creature — Snake".into()
    });
    let nonsnow = lower(&nonsnow);
    assert!(!nonsnow.characteristics.snow);
    assert!(!nonsnow.capabilities.contains(&Capability::SnowSupertypeV1));
    assert_eq!(nonsnow.static_abilities, d.static_abilities);
}

#[test]
fn attacking_creature_keywords_rejects_unconsumed_status_scope_and_modifier_semantics() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|e: &mut serde_json::Value| {
            e["what"]["filter"]["status"] = serde_json::json!(["blocking"])
        }) as Mutation,
        |e| e["what"]["filter"]["status"] = serde_json::json!(["attacking", "tapped"]),
        |e| e["what"]["filter"]["status"] = serde_json::json!(["attacking", "attacking"]),
        |e| e["what"]["filter"]["status"] = serde_json::json!([]),
        |e| e["what"]["filter"]["status"] = serde_json::Value::Null,
        |e| {
            e["what"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("status");
        },
        |e| e["what"]["filter"]["control"] = "opponent".into(),
        |e| {
            e["what"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("control");
        },
        |e| e["what"]["filter"]["owner"] = "you".into(),
        |e| e["what"]["filter"]["other"] = true.into(),
        |e| e["what"]["filter"]["types"] = serde_json::json!(["artifact", "creature"]),
        |e| e["what"]["filter"]["supertypes"] = serde_json::json!(["snow"]),
        |e| e["what"]["ref"] = "filter".into(),
        |e| e["what"]["count"] = serde_json::json!({"amount":"fixed","value":1}),
        |e| e["what"]["selectionId"] = "qualified".into(),
        |e| e["modifiers"][0]["optional"] = true.into(),
        |e| e["modifiers"][0]["abilities"][0]["gained"] = "activated".into(),
        |e| e["modifiers"][0]["abilities"][0]["keyword"]["keyword"] = "flying".into(),
        |e| e["modifiers"][0]["abilities"][0]["keyword"]["keyword"] = "flash".into(),
        |e| e["modifiers"][0]["abilities"][0]["keyword"]["cost"] = serde_json::json!([]),
        |e| e["modifiers"][0]["abilities"][0]["keyword"]["filter"] = serde_json::Value::Null,
        |e| {
            let grant = e["modifiers"][0]["abilities"][0].clone();
            e["modifiers"][0]["abilities"]
                .as_array_mut()
                .unwrap()
                .push(grant);
        },
        |e| {
            e["modifiers"][0]["abilities"] = serde_json::json!([]);
        },
        |e| {
            e["modifiers"].as_array_mut().unwrap().push(serde_json::json!({"modifier":"pt","power":{"sign":1,"amount":{"amount":"fixed","value":1}},"toughness":{"sign":1,"amount":{"amount":"fixed","value":1}}}));
        },
        |e| {
            e["modifiers"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"modifier":"cant","action":"be-blocked"}));
        },
        |e| {
            let grant = e["modifiers"][0].clone();
            e["modifiers"].as_array_mut().unwrap().push(grant);
        },
    ] {
        let raw = tests::changed_source(OHRAN, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][0]["effect"]);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}

#[test]
fn snow_supertype_rejects_duplicate_world_other_kinds_and_untracked_mana() {
    let duplicate = tests::changed_source(OHRAN, |r| {
        r["source"]["type_line"] = "Snow Snow Creature — Snake".into()
    });
    assert!(matches!(
        compile(MANIFEST, &duplicate),
        CompileResult::Invalid { .. }
    ));
    for line in [
        "Snow Land — Forest",
        "Basic Snow Land — Forest",
        "Snow Artifact",
        "Snow Enchantment",
        "Snow Instant",
        "World Creature — Snake",
    ] {
        let raw = tests::changed_source(OHRAN, |r| r["source"]["type_line"] = line.into());
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{line}: {:?}",
            compile(MANIFEST, &raw)
        );
    }
    let snow_cost = tests::changed_source(OHRAN, |r| r["source"]["mana_cost"] = "{3}{S}{G}".into());
    assert!(matches!(
        compile(MANIFEST, &snow_cost),
        CompileResult::Unsupported { .. }
    ));
    let mana = tests::changed_source(OHRAN, |r| {
        let result = &mut r["faces"][0]["result"];
        result["abilities"][0] = serde_json::json!({"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}}]}]});
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    });
    assert!(matches!(
        compile(MANIFEST, &mana),
        CompileResult::Unsupported { reason, .. } if reason.contains("non-mana creature")
    ));
}

#[test]
fn grouped_damage_exact_kutzil_source_binds_complete_scope_and_candidate_power() {
    let d = lower(KUTZIL);
    let r: serde_json::Value = serde_json::from_str(KUTZIL).unwrap();
    assert_eq!(d.name, "Kutzil, Malamet Exemplar");
    assert_eq!(
        r["source"]["oracle_text"],
        "Your opponents can't cast spells during your turn.\nWhenever one or more creatures you control each with power greater than its base power deals combat damage to a player, draw a card."
    );
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.legendary);
    assert_eq!(d.characteristics.subtypes, ["Cat", "Warrior"]);
    assert_eq!(d.characteristics.power, Some(3.into()));
    assert_eq!(d.characteristics.toughness, Some(3.into()));
    assert_eq!(d.characteristics.mana_cost.generic, 1.into());
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G, Color::W]);
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::OpponentsCantCastDuringControllerTurn
        }]
    );
    assert_eq!(d.triggered.len(), 1);
    assert_eq!(
        d.triggered[0].condition.reporting_key(),
        "grouped_damage_dealt"
    );
    assert_eq!(
        d.triggered[0].condition,
        TriggerCondition::GroupedDamageDealt {
            source: DamageTriggerSource::CreatureFilter {
                controller: DamageSourceRelation::Controller,
                types: TypePredicate::Type {
                    kind: CardKind::Creature
                }
            },
            kind: TriggerDamageKind::Combat,
            recipient: DamageTriggerRecipient::Player,
            source_power: DamageSourcePowerPredicate::GreaterThanBase
        }
    );
    assert_eq!(
        d.triggered[0].program.instructions,
        [Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 1
        }]
    );
    assert!(d.triggered[0].program.targets.is_empty() && d.triggered[0].program.modes.is_empty());
    for cap in [
        Capability::OpponentCastProhibitionV1,
        Capability::GroupedCreatureDamageTriggerV1,
        Capability::ControllerDrawV1,
    ] {
        assert!(d.capabilities.contains(&cap));
        let mut missing = d.clone();
        missing.capabilities.remove(&cap);
        assert!(missing.validate().is_err());
    }
    assert_eq!(d.provenance.source_sha256, r["sourceSha256"]);
    assert_eq!(d.provenance.ast_sha256, r["astSha256"]);
    assert_eq!(d.provenance.binding_sha256, r["bindingSha256"]);
    assert_eq!(r["binding"]["fullyBound"], false);
    assert!(!r["binding"]["diagnostics"].as_array().unwrap().is_empty());
    // The diagnostic graph is retained without repairing the unresolved action
    // scope or its incorrect source-level "it" target. Executable Rust binds
    // the exact prohibition and EACH queried creature's own base power.
    assert_eq!(
        r["binding"]["references"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["path"]
                == "/faces/0/abilities/1/trigger/what/filter/properties/0/comparison/value/of")
            .unwrap()["target"],
        "card:3233af43-7826-4895-b67e-03c6102c2cd5/source#object"
    );
    let changed_diagnostic =
        tests::changed_source(KUTZIL, |r| r["binding"]["fullyBound"] = true.into());
    let diagnostic = lower(&changed_diagnostic);
    assert_eq!(diagnostic.static_abilities, d.static_abilities);
    assert_eq!(diagnostic.triggered, d.triggered);
}

#[test]
fn opponent_cast_prohibition_rejects_partial_unknown_and_qualified_action_scopes() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|p: &mut serde_json::Value| p["who"] = serde_json::json!({"player":"you"})) as Mutation,
        |p| p["who"] = serde_json::json!({"player":"a-player"}),
        |p| p["who"]["of"] = serde_json::json!({"ref":"it"}),
        |p| p["who"]["selectionId"] = "unbound".into(),
        |p| p["what"]["ref"] = "this".into(),
        |p| p["what"]["filter"]["cls"] = "card".into(),
        |p| p["what"]["filter"]["types"] = serde_json::json!(["creature"]),
        |p| p["what"]["filter"]["owner"] = "you".into(),
        |p| p["what"]["zone"] = "hand".into(),
        |p| p["what"]["count"] = serde_json::json!({"amount":"fixed","value":1}),
        |p| p["when"]["during"] = "phase".into(),
        |p| p["when"]["whose"] = serde_json::json!({"player":"each-opponent"}),
        |p| p["when"]["whose"]["of"] = serde_json::json!({"ref":"it"}),
        |p| p["when"] = serde_json::Value::Null,
        |p| {
            p.as_object_mut().unwrap().remove("when");
        },
        |p| p["prohibition"] = "activate".into(),
        |p| p["optional"] = false.into(),
        |p| p["once"] = true.into(),
        |p| p["unknown"] = true.into(),
    ] {
        let raw = tests::changed_source(KUTZIL, |r| {
            let result = &mut r["faces"][0]["result"];
            mutate(&mut result["abilities"][0]["effect"]["prohibition"]);
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}

#[test]
fn grouped_damage_rejects_partial_grouping_properties_and_unbound_draw_recipients() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| {
            a["trigger"].as_object_mut().unwrap().remove("grouping");
        }) as Mutation,
        |a| a["trigger"]["grouping"] = serde_json::Value::Null,
        |a| a["trigger"]["grouping"] = "each".into(),
        |a| a["trigger"]["grouping"] = "once-per-turn".into(),
        |a| a["trigger"]["grouping"] = false.into(),
        |a| a["trigger"]["what"]["count"]["value"] = 0.into(),
        |a| a["trigger"]["what"]["count"]["value"] = 2.into(),
        |a| a["trigger"]["what"]["filter"]["properties"] = serde_json::json!([]),
        |a| a["trigger"]["what"]["filter"]["properties"] = serde_json::Value::Null,
        |a| {
            a["trigger"]["what"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("properties");
        },
        |a| a["trigger"]["what"]["filter"]["properties"][0]["property"] = "toughness".into(),
        |a| a["trigger"]["what"]["filter"]["properties"][0]["comparison"]["op"] = "gte".into(),
        |a| {
            a["trigger"]["what"]["filter"]["properties"][0]["comparison"]["value"]["attribute"] =
                "power".into()
        },
        |a| {
            a["trigger"]["what"]["filter"]["properties"][0]["comparison"]["value"]["of"] =
                serde_json::json!({"ref":"this","noun":"creature"})
        },
        |a| {
            a["trigger"]["what"]["filter"]["properties"][0]["comparison"]["value"]["of"] =
                serde_json::json!({"ref":"it","owner":"you"})
        },
        |a| {
            let p = a["trigger"]["what"]["filter"]["properties"][0].clone();
            a["trigger"]["what"]["filter"]["properties"]
                .as_array_mut()
                .unwrap()
                .push(p);
        },
        |a| a["trigger"]["what"]["filter"]["properties"][0]["unknown"] = true.into(),
        |a| a["trigger"]["what"]["filter"]["control"] = "opponent".into(),
        |a| a["trigger"]["what"]["filter"]["types"] = serde_json::json!(["artifact", "creature"]),
        |a| a["trigger"]["what"]["filter"]["nonTypes"] = serde_json::json!([]),
        |a| a["trigger"]["what"]["filter"]["other"] = true.into(),
        |a| a["trigger"]["combat"] = false.into(),
        |a| a["trigger"]["to"] = serde_json::json!({"player":"an-opponent"}),
        |a| a["effects"][0]["steps"][0]["who"] = serde_json::json!({"player":"that-player"}),
        |a| a["effects"][0]["steps"][0]["who"]["of"] = serde_json::json!({"ref":"it"}),
        |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
        |a| a["effects"][0]["steps"][0]["amount"]["value"] = 2.into(),
        |a| a["effects"][0]["steps"][0]["amount"] = serde_json::json!({"amount":"that-much"}),
        |a| {
            let draw = a["effects"][0]["steps"][0].clone();
            a["effects"][0]["steps"].as_array_mut().unwrap().push(draw);
        },
        |a| a["condition"] = serde_json::json!({"condition":"status","what":{"ref":"this","noun":"creature"},"status":"tapped"}),
    ] {
        let raw = tests::changed_source(KUTZIL, |r| {
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
