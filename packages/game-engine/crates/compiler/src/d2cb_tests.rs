use super::*;
const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const MINE: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2c/d26b27db-a567-4631-b4b6-7294222fbdd1.json"
);
const SEIZAN: &str = include_str!(
    "../../../tests/fixtures/cards-data-d2c/04d0d20f-720e-4cb6-a3ad-ea2b57bb7efa.json"
);
fn lower(raw: &str) -> Definition {
    let CompileResult::Lowered { definition } = compile(MANIFEST, raw) else {
        panic!("{:?}", compile(MANIFEST, raw))
    };
    *definition
}
fn changed(raw: &str, f: impl FnOnce(&mut serde_json::Value)) -> String {
    tests::changed_source(raw, |r| {
        let a = &mut r["faces"][0]["result"]["abilities"][0];
        f(a);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn d2cb_archive_programs_and_exact_minimum_capability_schema_closure() {
    let mine = lower(MINE);
    let seizan = lower(SEIZAN);
    assert_eq!(
        mine.triggered[0].intervening_if,
        Some(InterveningCondition::SourceStatus {
            status: TapStatus::Untapped
        })
    );
    assert!(
        matches!(mine.triggered[0].program.instructions.as_slice(), [Instruction::DrawCards { recipient: Some(LifeRecipient::EventPlayer), count, .. }] if *count == 1.into())
    );
    assert!(
        matches!(seizan.triggered[0].program.instructions.as_slice(), [Instruction::LoseLife {recipient: LifeRecipient::EventPlayer, amount, ..}, Instruction::DrawCards {recipient: Some(LifeRecipient::EventPlayer), ..}, Instruction::DrawCards {recipient: Some(LifeRecipient::EventPlayer), ..}] if *amount == 2.into())
    );
    let mut status_only = mine.clone();
    if let Instruction::DrawCards { recipient, .. } =
        &mut status_only.triggered[0].program.instructions[0]
    {
        *recipient = None;
    }
    status_only.capabilities = status_only.requirements();
    let mut actor_only = mine.clone();
    actor_only.triggered[0].intervening_if = None;
    actor_only.capabilities = actor_only.requirements();
    for d in [mine, seizan, status_only, actor_only] {
        assert_eq!(d.capabilities, d.requirements());
        assert!(
            d.capabilities.contains(&Capability::EventPlayerRecipientV1)
                || d.capabilities
                    .contains(&Capability::SourceStatusConditionV1)
        );
        for cap in [
            Capability::EventPlayerRecipientV1,
            Capability::SourceStatusConditionV1,
        ] {
            if d.capabilities.contains(&cap) {
                let mut bad = d.clone();
                bad.capabilities.remove(&cap);
                assert!(bad.validate().is_err());
            }
        }
        for version in [21, 22, 23] {
            let c = Catalog {
                schema_version: version,
                rules_edition: RULES_EDITION.into(),
                rules_sha256: RULES_SHA256.into(),
                definitions: vec![d.clone()],
            };
            assert_eq!(c.validate().is_ok(), version == 22);
        }
    }
}
#[test]
fn d2cb_condition_composition_and_unbound_actor_whole_card_rejections() {
    for status in ["tapped", "untapped"] {
        lower(&changed(MINE, |a| a["condition"]["status"] = status.into()));
    }
    let hand = lower(&changed(
        MINE,
        |a| a["condition"] = serde_json::json!({"condition":"cards-in-hand","who":{"player":"you"},"comparison":{"op":"eq","value":{"amount":"fixed","value":0}}}),
    ));
    assert_eq!(
        hand.triggered[0].intervening_if,
        Some(InterveningCondition::ControllerHandEmpty)
    );
    for raw in [
        changed(MINE, |a| a["condition"]["status"] = "face-down".into()),
        changed(MINE, |a| {
            a["condition"]["what"] = serde_json::json!({"ref":"it"})
        }),
        changed(MINE, |a| a["condition"]["extra"] = true.into()),
        changed(MINE, |a| {
            a["effects"][0]["steps"][0]["who"]["selectionId"] = "unbound".into()
        }),
        changed(
            SEIZAN,
            |a| {
                a["trigger"] =
                    serde_json::json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}})
            },
        ),
        changed(SEIZAN, |a| {
            a["kind"] = "spell".into();
            a.as_object_mut().unwrap().remove("trigger");
        }),
        changed(SEIZAN, |a| {
            a["kind"] = "activated".into();
            a.as_object_mut().unwrap().remove("trigger");
            a["costs"] = serde_json::json!([{"cost":"tap-self"}]);
        }),
        changed(SEIZAN, |a| {
            a["effects"][0]["steps"][1]["optional"] = true.into()
        }),
    ] {
        assert!(
            matches!(compile(MANIFEST, &raw), CompileResult::Unsupported { .. }),
            "{:?}",
            compile(MANIFEST, &raw)
        );
    }
    let mut d = lower(SEIZAN);
    d.triggered[0].condition = TriggerCondition::SelfEntersBattlefield;
    d.capabilities = d.requirements();
    assert!(d.validate().unwrap_err().contains("bound step trigger"));
    let mut d = lower(MINE);
    d.triggered[0].condition = TriggerCondition::SelfDies;
    d.triggered[0].program.instructions = vec![Instruction::GainLife {
        recipient: LifeRecipient::Controller,
        amount: 1.into(),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    assert!(d.validate().is_err());
}
