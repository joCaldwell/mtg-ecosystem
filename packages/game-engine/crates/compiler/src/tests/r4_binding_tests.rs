//! Exact frozen R4 records exercise both refreshed operand graphs and row pins.
use super::*;
use serde_json::{Value, json};

/// One triggered block whose "that player" resolves to an event player.
fn event_player_block(role: &str, scope: Option<&str>) -> resolution::Resolution {
    let mut entry =
        json!({"path":"/effects/0/steps/0/who","ref":{"ref":"event-player","role":role}});
    if let Some(scope) = scope {
        entry["scope"] = json!(scope);
    }
    let card = json!({"artifactKind":"oracle-resolution","schemaVersion":1,"faces":[{"sourceFaceIndex":0,
        "blocks":[{"refs":[entry],"steps":[],"spans":{"unsupported":"synthetic"},"engineFeatures":[],
        "residuals":[]}]}],"residuals":[],"complete":true})
    .to_string();
    let raw = serde_json::value::RawValue::from_string(card.clone()).unwrap();
    resolution::Resolution::read(Some(raw.as_ref()), Some(&sha256(card.as_bytes()))).unwrap()
}

#[test]
fn kernel_event_players_belong_to_the_trigger_that_supplies_them() {
    let block = |trigger: Value| {
        json!([{"kind":"triggered","trigger":trigger,"effects":[{"steps":[{"effect":"lose-life",
            "who":{"player":"that-player"},"amount":{"amount":"fixed","value":1}}]}]}])
    };
    let upkeep = block(json!({"trigger":"phase","phase":"upkeep","whose":"each-player"}));
    let cast = block(json!({"trigger":"cast","who":{"player":"an-opponent"}}));
    let draws = block(json!({"trigger":"draws","who":{"player":"an-opponent"}}));
    // CR 102.1, 603.2: the binder's spelling is the kernel role the trigger supplies.
    for (role, trigger, condition) in [
        (
            "active-player",
            "phase",
            TriggerCondition::StepBegan {
                step: BeginningStep::Upkeep,
                turn_relation: TurnRelation::AnyPlayer,
            },
        ),
        (
            "caster",
            "cast",
            TriggerCondition::SpellCast {
                caster: CasterRelation::Opponent,
                spell: TypePredicate::Any {},
            },
        ),
    ] {
        let (kernel, supplier) = resolution::kernel_event_player(role).unwrap();
        assert_eq!(
            (Some(kernel), supplier),
            (kir::event_player_role(&condition), trigger)
        );
    }
    assert_eq!(
        resolution::kernel_event_player("activating-player").map(|(role, _)| role),
        Some(kir::EventPlayerRole::ActivatingPlayer)
    );
    assert!(
        event_player_block("active-player", None)
            .ordinary_antecedents(&upkeep)
            .is_ok()
    );
    assert!(
        event_player_block("caster", None)
            .ordinary_antecedents(&cast)
            .is_ok()
    );
    // A kernel role under another trigger, or borrowed from a nested ability.
    for (role, scope, ability) in [
        ("active-player", None, &cast),
        ("caster", None, &upkeep),
        ("active-player", Some("/effects/0/steps/0"), &upkeep),
        ("activating-player", None, &draws),
    ] {
        assert!(
            matches!(
                event_player_block(role, scope).ordinary_antecedents(ability),
                Err(f) if f.path == "resolution"
            ),
            "{role} {scope:?}"
        );
    }
    // An event player the kernel does not name keeps its trigger discriminant;
    // the lowerer that reads it decides (composition reads no such role yet).
    assert!(
        event_player_block("draws", None)
            .ordinary_antecedents(&draws)
            .is_ok()
    );
    assert_eq!(resolution::kernel_event_player("draws"), None);
}
