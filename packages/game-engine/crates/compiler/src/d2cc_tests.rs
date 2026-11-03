//! Archived whole-card positives, paired structured-AST negatives and synthetic
//! predicate/binding boundaries; synthetic mutations are not printed card facts.
use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const STUDENT: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/0adcf8d2-6eb9-4679-8ad5-f28e2f4aba39.json"
);
const SYTHIS: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/0fc64fd6-f057-4056-9dca-47accb7ff036.json"
);
const ARCHER: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/2e9289d6-dbc6-456d-88cf-d1f534e731d6.json"
);
const SIGIL: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/3b9b5b22-5a7d-4e37-a870-ca0f0efa4f36.json"
);
const WIZARD: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/321e101c-a426-4d18-be0b-c26c5348b70d.json"
);
const VISITOR: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/0629747e-8dd8-4150-95ee-9d860264ccb0.json"
);
const PYRE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/0a6f45e9-5c7a-4e2c-bf8a-fc5fe0ae0793.json"
);
const KAMBAL: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/4987c458-604a-4727-b360-170616e91e67.json"
);
const HYDRA: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2cc/b3f2265b-dd65-4b74-8b74-35ee0b147617.json"
);
fn lower(raw: &str) -> Definition {
    match compile(MANIFEST, raw) {
        CompileResult::Lowered { definition } => *definition,
        x => panic!("{x:?}"),
    }
}
fn changed(f: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(STUDENT, |r| {
        f(&mut r["faces"][0]["result"]["abilities"][0]);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2cc_archived_whole_programs_min_ir_capability_closure() {
    for raw in [STUDENT, SYTHIS, ARCHER, SIGIL, KAMBAL, PYRE, VISITOR, HYDRA] {
        let d = lower(raw);
        assert_eq!(d.capabilities, d.requirements());
        assert!(d.capabilities.contains(&Capability::CastTriggerV1));
        let mut bad = d.clone();
        bad.capabilities.remove(&Capability::CastTriggerV1);
        assert!(bad.validate().is_err());
        for version in [22, 23, 24, 25] {
            let c = Catalog {
                schema_version: version,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            assert_eq!(
                c.validate().is_ok(),
                matches!(version, 23 | 24),
                "{} v{version}",
                d.name
            );
        }
    }
    assert!(
        matches!(lower(STUDENT).triggered[0].program.instructions.as_slice(), [Instruction::GainLife { recipient:LifeRecipient::Controller,amount,.. }] if *amount == 2.into())
    );
    assert!(matches!(
        lower(SYTHIS).triggered[0].program.instructions.as_slice(),
        [Instruction::GainLife { .. }, Instruction::DrawCards { .. }]
    ));
    assert!(matches!(
        lower(ARCHER).triggered[0].program.instructions.as_slice(),
        [Instruction::SacrificeDamage {
            recipient: Some(LifeRecipient::EachOpponent),
            ..
        }]
    ));
    assert!(matches!(
        lower(SIGIL).triggered[0].program.instructions.as_slice(),
        [Instruction::CreateCreatureTokens { .. }]
    ));
    assert!(matches!(
        compile(MANIFEST, WIZARD),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn d2cc_synthetic_boolean_type_membership_and_independent_casters() {
    for who in ["you", "a-player", "an-opponent"] {
        for (filter, accepts) in [
            (
                serde_json::json!({"cls":"spell","types":["artifact","creature"]}),
                [false, false, true, false],
            ),
            (
                serde_json::json!({"cls":"spell","nonTypes":["creature"]}),
                [true, true, false, true],
            ),
            (
                serde_json::json!({"cls":"spell","allOf":[{"anyOf":[{"types":["instant"]},{"types":["sorcery"]}]}]}),
                [true, false, false, true],
            ),
            (
                serde_json::json!({"cls":"spell","anyOf":[{"types":["artifact"]},{"types":["enchantment"]}],"nonTypes":["creature"]}),
                [false, true, false, false],
            ),
        ] {
            let d = lower(&changed(|a| {
                a["trigger"]["who"]["player"] = who.into();
                a["trigger"]["what"] = filter;
            }));
            let TriggerCondition::SpellCast { caster, spell } = &d.triggered[0].condition else {
                panic!("cast")
            };
            assert_eq!(
                *caster,
                match who {
                    "you" => CasterRelation::Controller,
                    "a-player" => CasterRelation::AnyPlayer,
                    _ => CasterRelation::Opponent,
                }
            );
            for ((kind, extra), accept) in [
                (CardKind::Instant, vec![]),
                (CardKind::Artifact, vec![]),
                (CardKind::Creature, vec![CardKind::Artifact]),
                (CardKind::Sorcery, vec![]),
            ]
            .into_iter()
            .zip(accepts)
            {
                let mut c = d.characteristics.clone();
                c.kind = kind;
                c.additional_types = extra;
                assert_eq!(spell.matches(&c), accept, "{who}/{spell:?}/{c:?}");
            }
        }
    }
}
#[test]
fn d2cc_unknown_qualifiers_self_cast_copy_ordinals_color_and_optional_reject() {
    let mut mutations = vec![];
    for (field, value) in [
        ("from", serde_json::json!({"zone":"hand","owner":"your"})),
        ("notFrom", serde_json::json!({"zone":"hand","owner":"your"})),
        ("during", serde_json::json!("your")),
        (
            "occurrence",
            serde_json::json!({"ordinal":1,"period":"turn"}),
        ),
        ("unknown", serde_json::json!(true)),
    ] {
        mutations.push(changed(|a| a["trigger"][field] = value));
    }
    for what in [
        serde_json::json!({"ref":"this","noun":"spell"}),
        serde_json::json!({"cls":"spell","targets":{"ref":"this","noun":"creature"}}),
        serde_json::json!({"cls":"spell","colors":["blue"]}),
        serde_json::json!({"cls":"spell","own":true}),
        serde_json::json!({"cls":"spell","types":["planeswalker"]}),
        serde_json::json!({"cls":"spell","allOf":[]}),
        serde_json::json!({"cls":"spell","anyOf":[{"types":["artifact"]},{"unknown":true}]}),
        serde_json::json!({"cls":"spell","properties":[{"property":"mana-value","comparison":{"op":"ge","value":{"amount":"fixed","value":4}}}]}),
    ] {
        mutations.push(changed(|a| a["trigger"]["what"] = what));
    }
    mutations.push(changed(|a| {
        a["trigger"].as_object_mut().unwrap().remove("who");
    }));
    mutations.push(changed(|a| {
        a["trigger"].as_object_mut().unwrap().remove("what");
    }));
    mutations.push(changed(|a| a["trigger"]["trigger"] = "cast-or-copy".into()));
    mutations.push(changed(|a| {
        a["limit"] = serde_json::json!({"maximum":1,"period":"turn"})
    }));
    mutations.push(changed(|a| {
        a["effects"][0]["steps"][0]["optional"] = true.into()
    }));
    mutations.push(changed(|a| {
        a["trigger"]["who"]["of"] = serde_json::json!({"ref":"this"})
    }));
    for raw in mutations {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
}
#[test]
fn d2cc_actor_binding_is_cast_only_and_no_intervening_extension() {
    for op in ["draw", "gain-life", "lose-life"] {
        let raw = changed(|a| {
            a["trigger"]["who"]["player"] = "a-player".into();
            a["effects"][0]["steps"][0] = serde_json::json!({"effect":op,"who":{"player":"that-player"},"amount":{"amount":"fixed","value":1}});
        });
        let mut d = lower(&raw);
        assert!(d.capabilities.contains(&Capability::EventPlayerRecipientV1));
        d.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    let raw = changed(
        |a| a["condition"] = serde_json::json!({"condition":"cards-in-hand","who":{"player":"you"},"comparison":{"op":"eq","value":{"amount":"fixed","value":0}}}),
    );
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
    for raw in [
        serde_json::json!({"predicate":"any","unknown":true}),
        serde_json::json!({"predicate":"type","kind":"creature","unknown":true}),
        serde_json::json!({"predicate":"all","terms":[{"predicate":"any"}],"unknown":true}),
    ] {
        assert!(serde_json::from_value::<TypePredicate>(raw).is_err());
    }
    for predicate in [
        TypePredicate::All { terms: vec![] },
        TypePredicate::AnyOf {
            terms: vec![TypePredicate::Any {}; 33],
        },
    ] {
        assert!(predicate.validate().is_err());
    }
}

#[test]
fn d2cc_damage_actor_and_multiplayer_draw_guards_remain_closed() {
    let raw = changed(|a| {
        a["effects"][0]["steps"][0] = serde_json::json!({
            "effect":"damage", "source":{"ref":"this","noun":"creature"},
            "amount":{"amount":"fixed","value":1},
            "to":{"to":"player","ref":{"player":"that-player"}}
        })
    });
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
    let raw = changed(|a| {
        a["effects"][0]["steps"][0] = serde_json::json!({
            "effect":"draw", "who":{"player":"each-opponent"}, "amount":{"amount":"fixed","value":1}
        })
    });
    assert!(matches!(
        compile(MANIFEST, &raw),
        CompileResult::Unsupported { .. }
    ));
    let mut d = lower(ARCHER);
    if let Instruction::SacrificeDamage { recipient, .. } =
        &mut d.triggered[0].program.instructions[0]
    {
        *recipient = Some(LifeRecipient::EventPlayer);
    } else {
        panic!("damage")
    }
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
