use super::*;
use serde_json::json;

const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
const GLOVES: &str =
    include_str!("../../../tests/fixtures/308d7868-49f9-47a3-a7b1-4b0332d610f1.json");
const CLAMP: &str =
    include_str!("../../../tests/fixtures/65986c1b-8e51-4604-b685-d82fa7d1263a.json");
const BLADE: &str =
    include_str!("../../../tests/fixtures/2ccdc60a-49a9-44b9-a7af-0ebf18b26785.json");
const FEAST: &str =
    include_str!("../../../tests/fixtures/d0901053-6de0-46d0-9ee3-8d40510236c1.json");
const TEMPLE: &str =
    include_str!("../../../tests/fixtures/79f94050-d850-41ca-b1db-5ae0cf743f0a.json");
const OMEN: &str =
    include_str!("../../../tests/fixtures/41960d32-ddb5-42be-94b2-3a2e77ca148d.json");

fn compiled(raw: &str) -> Definition {
    match compile(HEADER, raw) {
        CompileResult::Lowered { definition } => *definition,
        err => panic!(
            "{}: {err:?}",
            serde_json::from_str::<serde_json::Value>(raw).unwrap()["source"]["name"]
        ),
    }
}

#[test]
fn slice4_equipment_binds_distinct_sources_and_ordered_steps() {
    for (raw, condition) in [
        (GLOVES, TriggerCondition::EquippedCombatDamage),
        (BLADE, TriggerCondition::EquippedCombatDamage),
        (FEAST, TriggerCondition::EquippedCombatDamage),
        (CLAMP, TriggerCondition::EquippedDies),
    ] {
        let d = compiled(raw);
        assert_eq!(d.triggered[0].condition, condition);
        assert!(
            d.capabilities
                .contains(&Capability::EquippedCreatureTriggerV1)
        );
    }
    assert!(matches!(
        compiled(BLADE).triggered[0].program.instructions.as_slice(),
        [
            kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::Target { slot: 0 }
                },
                amount: fixed!(_),
                ..
            }),
            kernel!(Draw {
                player: you!(),
                amount: fixed!(_)
            })
        ]
    ));
    assert!(matches!(
        compiled(FEAST).triggered[0].program.instructions.as_slice(),
        [
            kernel!(Discard {
                player: event_player!(),
                amount: fixed!(_),
                random: false
            }),
            kernel!(Untap {
                what: kir::SetExpr::Matching { .. }
            })
        ]
    ));
    let d = compiled(CLAMP);
    assert!(
        matches!(&d.static_abilities[0].effect, StaticEffect::ModifyCreatures{scope:CreatureScope::Equipped,power,toughness,..}
        if *power==1.into() && *toughness==(-1).into())
    );
}

#[test]
fn slice4_look_steps_reuse_entry_attack_and_paid_sacrifice_contexts() {
    for raw in [
        TEMPLE,
        include_str!("../../../tests/fixtures/0e2f8758-7457-4b54-ae61-a9c68cea4a23.json"),
        include_str!("../../../tests/fixtures/32ebc862-3bf6-4754-b7b8-8bb73f1651cf.json"),
        include_str!("../../../tests/fixtures/8ac23863-0aa7-41bc-b9b7-c5f0566172cf.json"),
        include_str!("../../../tests/fixtures/7c8e4f29-53e0-4242-8ef8-7a9476dce178.json"),
        OMEN,
    ] {
        let d = compiled(raw);
        assert!(matches!(
            d.triggered[0].program.instructions[0],
            Instruction::Scry { .. } | Instruction::Surveil { .. }
        ));
        d.validate().unwrap();
    }
    let omen = compiled(OMEN);
    assert!(
        matches!(&omen.activated[0].cost,ActivationCost::Composite{costs}
        if costs.sacrifice && !costs.tap && costs.mana.as_ref().is_some_and(|m| m.generic==2.into() && m.colored==[Color::U]))
    );
    assert!(matches!(
        omen.triggered[0].program.instructions.as_slice(),
        [
            Instruction::Scry { .. },
            kernel!(Draw {
                player: kir::PlayerExpr::Is { .. },
                amount: fixed!(_)
            })
        ]
    ));
}

#[test]
fn slice4_rejects_unconsumed_source_and_wrong_binding() {
    let wrong_object = super::tests::changed_source(GLOVES, |r| {
        let edge = r["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e["path"].as_str().unwrap().ends_with("/trigger/what"))
            .unwrap();
        edge["target"] = json!("not-the-equipped-bearer");
    });
    let wrong_trigger = super::tests::changed_source(GLOVES, |r| {
        let source = r["source"]["oracle_text"]
            .as_str()
            .unwrap()
            .replace("to a player", "to a creature");
        r["source"]["oracle_text"] = json!(source);
        let line = &mut r["faces"][0]["result"]["lines"][0]["text"];
        *line = json!(
            line.as_str()
                .unwrap()
                .replace("to a player", "to a creature")
        );
    });
    for record in [&wrong_object, &wrong_trigger] {
        assert!(!matches!(
            compile(HEADER, record),
            CompileResult::Lowered { .. }
        ));
    }
    let mut erased = compiled(TEMPLE);
    erased.triggered.clear();
    erased.source_labels = None;
    erased.capabilities = erased.requirements();
    assert!(erased.validate().is_err());
}

#[test]
fn slice4_predefined_tokens_preserve_the_template_and_creation_context() {
    for (raw, kind) in [
        (
            include_str!("../../../tests/fixtures/c4efae03-99de-4439-925b-504602623bfd.json"),
            PredefinedToken::Treasure,
        ),
        (
            include_str!("../../../tests/fixtures/c8c88c33-b8df-4b19-b12c-79c9709af8f1.json"),
            PredefinedToken::Food,
        ),
        (
            include_str!("../../../tests/fixtures/a5a05edd-2a30-414b-b766-f0651d5783f5.json"),
            PredefinedToken::Clue,
        ),
        (
            include_str!("../../../tests/fixtures/c1624d10-8838-4af8-aea1-a96c0fe6fd6b.json"),
            PredefinedToken::Treasure,
        ),
    ] {
        let d = compiled(raw);
        assert!(
            d.capabilities
                .contains(&Capability::PredefinedArtifactTokensV1)
        );
        assert!(
            matches!(d.triggered[0].program.instructions.as_slice(),[Instruction::CreatePredefinedTokens{kind:k,count:1,..}] if *k==kind)
        );
    }
    let activated = compiled(include_str!(
        "../../../tests/fixtures/184c763c-b52a-471c-bd87-d01729ebe5e4.json"
    ));
    assert_eq!(activated.activated[0].cost, ActivationCost::TapSource);
    assert!(matches!(
        activated.activated[0].program.instructions.as_slice(),
        [Instruction::CreatePredefinedTokens {
            kind: PredefinedToken::Treasure,
            count: 1,
            ..
        }]
    ));
}

#[test]
fn slice4_rejects_unimplemented_mana_events_and_extra_compound_effects() {
    for raw in [
        include_str!("../../../tests/fixtures/706ae742-1807-44b7-a4fa-f2e26f61519a.json"),
        include_str!("../../../tests/fixtures/fac42229-4f5f-4d04-85dd-5031d4e435aa.json"),
    ] {
        assert!(matches!(
            compile(HEADER, raw),
            CompileResult::Unsupported { .. }
        ));
    }
}
