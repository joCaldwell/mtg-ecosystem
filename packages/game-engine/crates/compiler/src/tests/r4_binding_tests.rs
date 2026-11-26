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
            TriggerCondition::on(EventPattern::StepBegins {
                step: BeginningStep::Upkeep,
                active: PlayerExpr::EachPlayer,
            }),
        ),
        (
            "caster",
            "cast",
            TriggerCondition::on(EventPattern::Cast {
                caster: PlayerExpr::EachOpponent,
                spell: Pred::default(),
                nth: None,
            }),
        ),
        (
            "draws",
            "draws",
            TriggerCondition::on(EventPattern::Draws {
                player: PlayerExpr::EachOpponent,
                nth: None,
            }),
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
            .references(&upkeep)
            .is_ok()
    );
    assert!(event_player_block("caster", None).references(&cast).is_ok());
    // A kernel role under another trigger, or borrowed from a nested ability.
    for (role, scope, ability) in [
        ("active-player", None, &cast),
        ("caster", None, &upkeep),
        ("draws", None, &cast),
        ("active-player", Some("/effects/0/steps/0"), &upkeep),
        ("activating-player", None, &draws),
    ] {
        assert!(
            matches!(
                event_player_block(role, scope).references(ability),
                Err(f) if f.path == "resolution"
            ),
            "{role} {scope:?}"
        );
    }
    assert!(event_player_block("draws", None).references(&draws).is_ok());
}

/// One block with a single resolved reference entry.
fn reference_block(path: &str, resolved: Value) -> resolution::Resolution {
    let card = json!({"artifactKind":"oracle-resolution","schemaVersion":1,"faces":[{"sourceFaceIndex":0,
        "blocks":[{"refs":[{"path":path,"ref":resolved}],"steps":[],"spans":{"unsupported":"synthetic"},
        "engineFeatures":[],"residuals":[]}]}],"residuals":[],"complete":true})
    .to_string();
    let raw = serde_json::value::RawValue::from_string(card.clone()).unwrap();
    resolution::Resolution::read(Some(raw.as_ref()), Some(&sha256(card.as_bytes()))).unwrap()
}

#[test]
fn reference_table_rows_name_the_role_and_block_they_reject() {
    let code = |r: Result<(), Failure>| {
        r.err()
            .map(|f| crate::coverage::reason_code(&f.path, &f.reason, &Value::Null))
    };
    let pump = |what: Value| json!([{"kind":"static","effect":{"static":"modify","what":what,"modifiers":[]}}]);
    // CR 109.5: "you" is the block's own controller and no other role.
    let gain = json!([{"kind":"activated","costs":[],"effects":[{"steps":[{"effect":"gain-life",
        "who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}]);
    let you = |role: &str| {
        reference_block(
            "/effects/0/steps/0/who",
            json!({"ref":"controller","role":role}),
        )
        .references(&gain)
    };
    assert_eq!(code(you("ability-controller")), None);
    assert_eq!(
        code(you("entering-controller")),
        Some("ref:controller:entering-controller:activated".into())
    );
    // An attachment host must be printed as enchanted/equipped. A pronoun cannot stand in.
    let attached = reference_block("/effect/what", json!({"ref":"attached-to"}))
        .references(&pump(json!({"ref":"it"})));
    assert_eq!(code(attached), Some("ref:attached-to:static".into()));
    // CR 201.5: "this" is the source, never a target.
    let this = |resolved: Value| {
        reference_block("/effect/what", resolved)
            .references(&pump(json!({"ref":"this","noun":"creature"})))
    };
    assert_eq!(code(this(json!({"ref":"source"}))), None);
    assert_eq!(
        code(this(json!({"ref":"target","slot":0}))),
        Some("ref:target:static".into())
    );
}
