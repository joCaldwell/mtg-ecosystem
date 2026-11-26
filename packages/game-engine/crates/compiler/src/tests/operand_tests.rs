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

/// CR 110.4: a permanent-card target covers every admitted permanent type.
#[test]
fn own_graveyard_permanent_target_covers_all_permanent_card_kinds() {
    use std::collections::BTreeSet;

    let what = object(&json!({
        "ref": "target",
        "filter": {"cls": "card", "permanent": true, "zone": {"zone": "graveyard", "owner": "your"}},
        "count": {"amount": "fixed", "value": 1},
        "selectionId": "line:0:1",
    })).unwrap();
    let mut program = program();
    bind_own_graveyard_target(&mut program, &mut BTreeMap::new(), &what, false).unwrap();
    let predicate = program.targets[0]
        .filter
        .as_ref()
        .unwrap()
        .predicate
        .as_ref()
        .unwrap();
    let [mtg_contracts::kir::Atom::AnyType { kinds }] = predicate.0.as_slice() else {
        panic!("unexpected permanent-card predicate {predicate:?}");
    };

    let schema = mtg_contracts::catalog_schema();
    let all: Vec<CardKind> =
        serde_json::from_value(schema["$defs"]["CardKind"]["enum"].clone()).unwrap();
    let expected = all
        .into_iter()
        .filter(|kind| match kind {
            CardKind::Instant | CardKind::Sorcery => false,
            CardKind::Artifact | CardKind::Creature | CardKind::Enchantment | CardKind::Land => {
                true
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(kinds.len(), expected.len());
    assert_eq!(kinds.iter().copied().collect::<BTreeSet<_>>(), expected);
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
    let mut optional = program();
    bind_creature_target(
        &mut optional,
        &mut BTreeMap::new(),
        &target(json!({"upTo":true})),
    )
    .unwrap();
    assert!(optional.optional_target);
    let mut controlled = program();
    bind_creature_target(
        &mut controlled,
        &mut BTreeMap::new(),
        &target(json!({"filter":{"types":["creature"],"control":"you"}})),
    )
    .unwrap();
    assert_eq!(
        controlled.targets[0].domain,
        TargetDomain::ControlledBattlefieldPermanent
    );
    assert_eq!(
        controlled.targets[0].filter.as_ref().unwrap().types,
        [CardKind::Creature]
    );
    let mut grounded = program();
    bind_creature_target(
        &mut grounded,
        &mut BTreeMap::new(),
        &target(
            json!({"filter":{"types":["creature"],"control":"you","withoutAbility":["flying"]}}),
        ),
    )
    .unwrap();
    assert_eq!(
        grounded.targets[0].filter.as_ref().unwrap().predicate,
        Some(Pred(vec![Atom::NotKeyword {
            keyword: KeywordAbility::Flying
        }]))
    );
    for (extra, expected) in [
        (json!({"upTo":false}), "operand:target:up_to"),
        (
            json!({"upTo":true,"count":{"amount":"fixed","value":2}}),
            "operand:target:up_to",
        ),
        (
            json!({"count":{"amount":"fixed","value":2}}),
            "operand:target:count",
        ),
        (
            json!({"filter":{"types":["creature"],"control":"you","withoutAbility":["banding"]}}),
            "operand:target_filter:without_ability:banding",
        ),
    ] {
        let failure = bind_creature_target(&mut program(), &mut BTreeMap::new(), &target(extra))
            .expect_err("qualified target");
        assert_eq!(code(failure), expected);
    }
    // An explicit null is never an absent term (CR 601.2c: no printed
    // qualifier is dropped): "target creature" with a null controller or
    // "other" term rejects instead of lowering as an unqualified creature.
    for (field, expected) in [
        ("control", "operand:object:null:control"),
        ("other", "operand:object:null:other"),
    ] {
        let mut what = json!({"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":"line:0:1"});
        what["filter"][field] = serde_json::Value::Null;
        assert_eq!(code(object(&what).unwrap_err()), expected);
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
    // CR 107.3, 608.2h: X is the announced X, or the sentence's bound X.
    let x = amount(json!({"amount":"x"}));
    assert_eq!(
        lower_damage_amount(&x, None, Context::Spell).unwrap(),
        kir::Amount::announced_x()
    );
    let creatures = kir::Amount::Count {
        set: SetExpr::battlefield(kir::Pred::creatures()),
    };
    assert_eq!(
        lower_damage_amount(&x, Some(&creatures), Context::Spell).unwrap(),
        creatures
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
        {"effect":"lose-game","who":{"player":"you"}}]}))
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
    assert_eq!(code(failure), "verb:lose-game:unlowered");
}

#[test]
fn counter_recipients_decode_as_generated_objects() {
    // CR 608.2c: "put a +1/+1 counter on it" names a pronoun recipient; the
    // step reaches its verb and rejects under the recipient's own code.
    let sentence: Sentence = serde_json::from_value(json!({"steps":[
        {"effect":"put-counters","on":{"ref":"it"},"counter":{"type":"+1/+1"},
         "count":{"amount":"fixed","value":1}}]}))
    .unwrap();
    let failure = append_spell_effects(
        &mut program(),
        &[sentence],
        0,
        &mut BTreeMap::new(),
        PlayerBinding::Unbound,
        Context::Activated,
    )
    .unwrap_err();
    assert_eq!(code(failure), "operand:target:ref_it");
}

#[test]
fn null_array_elements_name_their_array() {
    let what = json!({"ref":"target","filter":{"types":["creature", null]},
        "count":{"amount":"fixed","value":1},"selectionId":"line:0:1"});
    assert_eq!(
        code(object(&what).unwrap_err()),
        "operand:object:null:types"
    );
}

/// CR 115.3, 608.2b: a counted occurrence lowers each step once per member,
/// in slot order, so every member reads the same actions. A repeated step on
/// the same occurrence rebinds the same slots (Windborne Charge, frozen R5
/// source), and a non-spell damage group rejects by name.
#[test]
fn counted_target_groups_lower_one_leaf_per_member_and_step() {
    let target = json!({"ref":"target","filter":{"types":["creature"],"control":"you"},
        "count":{"amount":"fixed","value":2},"selectionId":"line:0:1"});
    let sentence: Sentence = serde_json::from_value(json!({"steps":[
        {"effect":"pump","what":target,
         "power":{"sign":1,"amount":{"amount":"fixed","value":2}},
         "toughness":{"sign":1,"amount":{"amount":"fixed","value":2}},
         "duration":{"duration":"end-of-turn"}},
        {"effect":"gain-abilities","what":target,
         "abilities":[{"gained":"keyword","keyword":{"keyword":"flying"}}],
         "duration":{"duration":"end-of-turn"}}]}))
    .unwrap();
    let mut p = program();
    append_spell_effects(
        &mut p,
        &[sentence],
        0,
        &mut BTreeMap::new(),
        PlayerBinding::Unbound,
        Context::Spell,
    )
    .unwrap();
    assert_eq!(p.target_group(), Some(2));
    assert!(!p.optional_target);
    let slots: Vec<_> = p
        .instructions
        .iter()
        .map(|Instruction::Kernel { node, .. }| node.group_leaf_slot())
        .collect();
    assert_eq!(slots, [Some(0), Some(1), Some(0), Some(1)]);
    let damage: Sentence = serde_json::from_value(json!({"steps":[
        {"effect":"damage","source":{"ref":"this","noun":"creature"},
         "amount":{"amount":"fixed","value":3},
         "to":{"to":"object","ref":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":2},"selectionId":"line:0:8"}}}]}))
    .unwrap();
    let failure = append_spell_effects(
        &mut program(),
        &[damage],
        0,
        &mut BTreeMap::new(),
        PlayerBinding::Unbound,
        Context::Triggered,
    )
    .unwrap_err();
    assert_eq!(code(failure), "operand:target:count:damage_source");
}

#[test]
fn bound_x_rejects_by_name_where_its_reads_are_not_exact() {
    // CR 107.3, 608.2h: "where X is [a number]" is read as each step that
    // prints X is performed. A step that cannot read it, a re-read after a
    // step that could change the number, and an unsupported quantity reject.
    let creatures = json!({"amount":"count","of":{"types":["creature"],"control":"you"}});
    let pump = json!({"effect":"pump","what":{"ref":"target","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":"line:0:0"},
        "power":{"sign":1,"amount":{"amount":"x"}},
        "toughness":{"sign":1,"amount":{"amount":"fixed","value":0}},
        "duration":{"duration":"end-of-turn"}});
    let draw = json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"x"}});
    let mill = json!({"effect":"mill","who":{"player":"you"},"amount":{"amount":"x"}});
    let lower = |steps: serde_json::Value, value: serde_json::Value| {
        let sentence: Sentence = serde_json::from_value(
            json!({"steps":steps,"bindings":[{"variable":"x","value":value}]}),
        )
        .unwrap();
        append_spell_effects(
            &mut program(),
            &[sentence],
            0,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Spell,
        )
    };
    assert!(lower(json!([draw.clone()]), creatures.clone()).is_ok());
    for (steps, value, expected) in [
        (
            json!([pump, draw]),
            creatures.clone(),
            "operand:bindings:reread_after_change",
        ),
        (
            json!([mill]),
            creatures.clone(),
            "bindings:unconsumed_count_binding",
        ),
        (
            json!([draw]),
            json!({"amount":"plus","a":{"amount":"fixed","value":1},
                "b":{"amount":"fixed","value":1}}),
            "operand:bindings:plus",
        ),
        (
            json!([draw]),
            json!({"amount":"attribute-extremum","extremum":"minimum","attribute":"power",
                "among":{"types":["creature"],"control":"you"}}),
            "operand:bindings:attribute_extremum:minimum",
        ),
    ] {
        assert_eq!(code(lower(steps, value).unwrap_err()), expected);
    }
}

/// CR 614.12, 701.20a: "you may reveal a [filter] card from your hand"
/// lowers to its hidden-card filter; every other reveal shape rejects by name.
#[test]
fn entry_reveal_lowers_one_filtered_hand_card_or_rejects() {
    let reveal = |what: serde_json::Value| {
        json!({"static":"entry-choice","what":{"ref":"this","noun":"land"},"who":{"player":"you"},
            "costs":[{"cost":"reveal","what":what}],"declined":{"tapped":true}})
    };
    let hand = |filter: serde_json::Value, count: i64| {
        let mut filter = filter;
        filter["cls"] = json!("card");
        filter["zone"] = json!({"zone":"hand","owner":"your"});
        json!({"ref":"filter","filter":filter,"count":{"amount":"fixed","value":count}})
    };
    let site = StaticSite {
        quoted: &|_| unreachable!("no quoted ability"),
    };
    let lowered = lower_static(
        &reveal(hand(
            json!({"allOf":[{"anyOf":[{"subtypes":["Plains"]},{"subtypes":["Island"]}]}]}),
            1,
        )),
        &site,
    )
    .unwrap();
    let [
        StaticAbility {
            effect: StaticEffect::RevealOrEnterTapped { filter },
            ..
        },
    ] = lowered.as_slice()
    else {
        panic!("{lowered:?}")
    };
    assert_eq!(
        filter,
        &Pred(vec![Atom::AnyOf {
            alternatives: vec![
                Pred(vec![Atom::Subtype {
                    subtype: "Plains".into()
                }]),
                Pred(vec![Atom::Subtype {
                    subtype: "Island".into()
                }]),
            ]
        }])
    );
    for (what, expected) in [
        (
            hand(json!({"subtypes":["Elf"]}), 2),
            "static:entry-choice:reveal:count",
        ),
        (hand(json!({}), 1), "static:entry-choice:reveal:unfiltered"),
        (
            json!({"ref":"filter","filter":{"subtypes":["Elf"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
                "count":{"amount":"fixed","value":1}}),
            "static:entry-choice:reveal:zone",
        ),
        (
            hand(json!({"subtypes":["Elf"],"control":"you"}), 1),
            "operand:filter:library_card:control",
        ),
    ] {
        let failure = lower_static(&reveal(what.clone()), &site).expect_err("rejected reveal");
        assert_eq!(code(failure), expected, "{what}");
    }
}
