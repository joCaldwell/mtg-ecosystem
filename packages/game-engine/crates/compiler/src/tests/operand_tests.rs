//! Typed operand lowering: every operand kind reads its generated AST type
//! and an unrepresented shape rejects under its own `operand:<kind>:<shape>`
//! code, never an opaque fall-through. CR 115, 601.2c, 608.2b-h.
use super::*;
use serde_json::json;

fn code(failure: Failure) -> String {
    crate::coverage::reason_code(&failure.path, &failure.reason, &serde_json::Value::Null)
}

fn program() -> SpellProgram {
    SpellProgram::instructions(SpellTiming::Instant)
}

#[test]
fn unrepresented_filter_terms_name_their_field() {
    for (filter, expected) in [
        (
            json!({"types":["creature"],"chosenColor":{"choice":"chosen-value","category":"color","which":"chosen"}}),
            "operand:filter:chosen_color",
        ),
        (
            json!({"types":["creature"],"other":true}),
            "operand:filter:other",
        ),
        (
            json!({"types":["creature"],"control":"that-player"}),
            "operand:filter:control:that_player",
        ),
        (
            json!({"types":["creature"],"status":["blocking"]}),
            "operand:filter:status:blocking",
        ),
    ] {
        let what = json!({"ref":"all","filter":filter});
        let failure = object(&what)
            .and_then(|what| lower_board_set(&what, Context::Spell))
            .expect_err("unrepresented term");
        assert_eq!(code(failure), expected, "{filter}");
    }
}

#[test]
fn target_occurrences_name_their_qualifier() {
    let target = |extra: serde_json::Value| {
        let mut what = json!({"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":"line:0:1"});
        what.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        object(&what).unwrap()
    };
    let mut slots = BTreeMap::new();
    let mut p = program();
    assert_eq!(
        bind_creature_target(&mut p, &mut slots, &target(json!({}))).unwrap(),
        0
    );
    for (extra, expected) in [
        (json!({"upTo":true}), "operand:target:up_to"),
        (
            json!({"count":{"amount":"fixed","value":2}}),
            "operand:target:count",
        ),
        (
            json!({"filter":{"types":["creature"],"control":"you"}}),
            "operand:creature_target:control",
        ),
    ] {
        let failure = bind_creature_target(&mut program(), &mut BTreeMap::new(), &target(extra))
            .expect_err("qualified target");
        assert_eq!(code(failure), expected);
    }
    // A set is not a target occurrence.
    let all = object(&json!({"ref":"all","filter":{"types":["creature"]}})).unwrap();
    let failure = bind_permanent_target(&mut program(), &mut BTreeMap::new(), &all).unwrap_err();
    assert_eq!(code(failure), "operand:target:ref_all");
}

#[test]
fn amounts_players_destinations_and_durations_are_typed() {
    let amount = |v| typed::<ast::Amount>(&v, "amount").unwrap();
    assert_eq!(
        fixed_positive_amount(&amount(json!({"amount":"fixed","value":3}))).unwrap(),
        Quantity::from(3)
    );
    for (v, expected) in [
        (
            json!({"amount":"fixed","value":0}),
            "operand:amount:fixed:range",
        ),
        (json!({"amount":"x"}), "operand:amount:x"),
    ] {
        assert_eq!(
            code(fixed_positive_amount(&amount(v)).unwrap_err()),
            expected
        );
    }
    // CR 107.3, 608.2h: X is the announced X, or the counted creatures.
    let x = amount(json!({"amount":"x"}));
    assert_eq!(
        lower_damage_amount(&x, false, Context::Spell).unwrap(),
        kir::Amount::announced_x()
    );
    assert_eq!(
        lower_damage_amount(&x, true, Context::Spell).unwrap(),
        kir::Amount::Count {
            set: SetExpr::battlefield(kir::Pred::creatures())
        }
    );
    let selector = |v| serde_json::from_value::<PlayerSelector>(v).unwrap();
    let player = |v| {
        lower_player(
            &mut program(),
            &mut BTreeMap::new(),
            &selector(v),
            Context::Activated,
            None,
        )
    };
    assert_eq!(
        player(json!({"player":"you"})).unwrap(),
        PlayerExpr::you(Context::Activated)
    );
    assert_eq!(
        code(player(json!({"player":"that-player"})).unwrap_err()),
        "operand:player:that_player:unbound"
    );
    assert_eq!(
        code(player(json!({"player":"each-player"})).unwrap_err()),
        "operand:player:each_player"
    );
    assert_eq!(
        lower_destination(&json!({"zone":"library","owner":"its-owner","position":"top"})).unwrap(),
        kir::Destination::OwnerLibraryTop
    );
    assert_eq!(
        code(lower_destination(&json!({"zone":"battlefield"})).unwrap_err()),
        "operand:destination:battlefield"
    );
    let duration =
        |d: &str| serde_json::from_value::<SourceDuration>(json!({ "duration": d })).unwrap();
    until_end_of_turn(&duration("end-of-turn")).unwrap();
    assert_eq!(
        code(until_end_of_turn(&duration("end-of-combat")).unwrap_err()),
        "operand:duration:end_of_combat"
    );
}

#[test]
fn verbs_without_a_module_name_their_effect_tag() {
    let sentence: Sentence = serde_json::from_value(json!({"steps":[
        {"effect":"scry","amount":{"amount":"fixed","value":2}}]}))
    .unwrap();
    let failure = append_spell_effects(
        &mut program(),
        &[sentence],
        0,
        &mut BTreeMap::new(),
        PlayerBinding::Unbound,
        Context::Spell,
    )
    .unwrap_err();
    assert_eq!(code(failure), "step:scry:unlowered");
}
