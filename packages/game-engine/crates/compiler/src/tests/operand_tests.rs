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
    bind_graveyard_target(&mut program, &mut BTreeMap::new(), &what, true).unwrap();
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
            CardKind::Artifact
            | CardKind::Creature
            | CardKind::Enchantment
            | CardKind::Land
            | CardKind::Planeswalker => true,
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
    let selector = |v| serde_json::from_value::<ast::PlayerRef>(v).unwrap();
    let player = |v| {
        lower_player_ref(
            &mut program(),
            &mut BTreeMap::new(),
            selector(v),
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
        lower_zone_destination(
            &typed(
                &json!({"zone":"library","owner":"its-owner","position":"top"}),
                "destination"
            )
            .unwrap()
        )
        .unwrap(),
        kir::Destination::OwnerLibraryTop
    );
    assert_eq!(
        code(
            lower_zone_destination(&typed(&json!({"zone":"battlefield"}), "destination").unwrap())
                .unwrap_err()
        ),
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
    assert_eq!(code(failure), "operand:object:unbound_antecedent");
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
fn optional_draw_and_life_require_a_printed_controller_chooser() {
    for effect in ["draw", "gain-life", "lose-life"] {
        let lower = |who, decided_by| {
            let mut step = json!({
                "effect": effect,
                "who": who,
                "amount": {"amount":"fixed","value":2},
                "optional": true
            });
            if let Some(chooser) = decided_by {
                step["decidedBy"] = chooser;
            }
            let sentence: Sentence = serde_json::from_value(json!({"steps":[step]})).unwrap();
            let mut program = program();
            append_spell_effects(
                &mut program,
                &[sentence],
                0,
                &mut BTreeMap::new(),
                PlayerBinding::Unbound,
                Context::Spell,
            )?;
            Ok(program.instructions)
        };
        let you = json!({"player":"you"});
        let target = json!({"player":"target-player","selectionId":"line:0:1"});
        for (who, chooser) in [(target.clone(), None), (you.clone(), Some(target.clone()))] {
            let failure = lower(who, chooser).expect_err("a non-controller would choose");
            assert_eq!(code(failure), "compose.control:optional_chooser");
        }
        for (who, chooser) in [(you.clone(), None), (target, Some(you))] {
            assert!(matches!(
                lower(who, chooser).unwrap().as_slice(),
                [Instruction::Kernel { node: kir::Node::Optional { chooser, .. }, .. }] if chooser.is_you()
            ));
        }
    }
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
        let sentence = crate::compose::decode_sentence(
            &json!({"steps":steps,"bindings":[{"variable":"x","value":value}]}),
        )?;
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
    assert!(lower(json!([mill.clone()]), creatures.clone()).is_ok());
    assert!(
        lower(
            json!([draw.clone()]),
            json!({"amount":"plus","a":{"amount":"fixed","value":1},
                "b":{"amount":"fixed","value":1}}),
        )
        .is_ok()
    );
    for (steps, value, expected) in [
        (
            json!([pump, draw]),
            creatures.clone(),
            "operand:bindings:reread_after_change",
        ),
        (
            json!([mill, draw]),
            json!({"amount":"count","of":{"cls":"card","zone":{"zone":"graveyard","owner":"your"}}}),
            "operand:bindings:reread_after_change",
        ),
        (
            json!([draw]),
            json!({"amount":"plus","a":{"amount":"fixed","value":1},
                "b":{"amount":"x"}}),
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

/// CR 208.2a, 604.3: a printed star is zero until one self CDA defines the
/// stat; `N+*` prints the fixed term of that CDA's sum, and a CDA cannot
/// define a stat printed as a number.
#[test]
fn printed_stars_need_their_characteristic_defining_ability() {
    use crate::assemble::{printed_stat, stars_defined};
    let n = |v: i64| Quantity::from(v);
    assert_eq!(printed_stat("*").unwrap(), (n(0), Some(n(0))));
    assert_eq!(printed_stat("1+*").unwrap(), (n(1), Some(n(1))));
    assert_eq!(printed_stat("*+1").unwrap(), (n(1), Some(n(1))));
    assert_eq!(printed_stat("3").unwrap(), (n(3), None));
    for spelling in ["7-*", "*\u{b2}", "+1", "+*"] {
        assert!(printed_stat(spelling).is_err(), "{spelling}");
    }
    let count = json!({"amount":"count","of":{"types":["creature"],"cls":"card",
        "zone":{"zone":"graveyard","owner":"all-players"}}});
    let lhurgoyf = characteristic_pt(&json!({"static":"group","effects":[
        {"static":"cda-pt","what":{"ref":"self"},"stat":"power","value":count},
        {"static":"cda-pt","what":{"ref":"self"},"stat":"toughness",
         "value":{"amount":"plus","a":count,"b":{"amount":"fixed","value":1}}}]}))
    .unwrap()
    .unwrap();
    let statics = [lhurgoyf];
    stars_defined(&statics, &[Some(n(0)), Some(n(1))]).unwrap();
    for (stars, reason) in [
        (
            [None, Some(n(1))],
            "characteristic-defining ability for a printed number",
        ),
        (
            [Some(n(0)), Some(n(2))],
            "printed fixed term outside the characteristic-defining sum",
        ),
    ] {
        let failure = stars_defined(&statics, &stars).unwrap_err();
        assert_eq!(failure.reason, reason);
    }
    let failure = stars_defined(&[], &[Some(n(0)), None]).unwrap_err();
    assert_eq!(
        (failure.path.as_str(), failure.reason.as_str()),
        (
            "source.power",
            "printed star without a characteristic-defining ability"
        )
    );
    let life = characteristic_pt(&json!({"static":"cda-pt","what":{"ref":"self"},
        "stat":"both","value":{"amount":"life-total","of":{"player":"you"}}}))
    .unwrap()
    .unwrap_err();
    assert_eq!(code(life), "static:cda-pt:amount:life_total");
    for (filter, expected) in [
        (
            json!({"control":"you","supertypes":["snow"]}),
            "static:cda-pt:filter:supertypes:snow",
        ),
        (
            json!({"types":["creature"],"status":["face-down"]}),
            "static:cda-pt:filter:status:face-down",
        ),
    ] {
        let failure = characteristic_pt(&json!({"static":"cda-pt","what":{"ref":"self"},
            "stat":"both","value":{"amount":"count","of":filter}}))
        .unwrap()
        .unwrap_err();
        assert_eq!(code(failure), expected);
    }
}

/// CR 608.2c: "it" and "that <noun>" bind one antecedent the block already
/// introduced, or reject by name; they never fall back to another object.
#[test]
fn pronoun_antecedents_bind_one_admissible_object_or_reject() {
    let tap = |selection: &str| {
        json!({"effect":"tap","what":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":selection}})
    };
    let gain = |of: serde_json::Value| {
        json!({"effect":"gain-life","who":{"player":"you"},
            "amount":{"amount":"attribute","of":of,"attribute":"power"}})
    };
    let lower = |steps: Vec<serde_json::Value>| {
        let sentences: Vec<Sentence> = steps
            .into_iter()
            .map(|step| serde_json::from_value(json!({"steps":[step]})).unwrap())
            .collect();
        let mut program = program();
        append_spell_effects(
            &mut program,
            &sentences,
            0,
            &mut BTreeMap::new(),
            PlayerBinding::Unbound,
            Context::Spell,
        )
        .map(|()| program)
    };
    // The one announced creature is "that creature".
    let program = lower(vec![
        tap("line:0:1"),
        gain(json!({"ref":"that","filter":{"types":["creature"]}})),
    ])
    .unwrap();
    assert_eq!(
        program.instructions[1].verb(),
        Some(&Verb::GainLife {
            player: PlayerExpr::you(Context::Spell),
            amount: Amount::StatOf {
                object: Ref::Target { slot: 0 },
                stat: kir::Stat::Power,
            },
        })
    );
    // Two announced creatures, an earlier untargeted object, or a noun no
    // candidate is, all reject.
    for (steps, expected) in [
        (
            vec![tap("line:0:1"), tap("line:0:2"), gain(json!({"ref":"it"}))],
            "operand:object:ambiguous_antecedent",
        ),
        (
            vec![
                json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}),
                tap("line:0:1"),
                gain(json!({"ref":"it"})),
            ],
            "operand:object:ambiguous_antecedent",
        ),
        (
            vec![
                tap("line:0:1"),
                gain(json!({"ref":"that","filter":{"cls":"card"}})),
            ],
            "operand:object:unbound_antecedent",
        ),
    ] {
        assert_eq!(code(lower(steps).unwrap_err()), expected);
    }
}
