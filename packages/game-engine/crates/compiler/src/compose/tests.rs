use super::*;
use serde_json::json;

fn card(kind: CardKind) -> Card<'static> {
    Card {
        kind,
        additional_types: &[],
    }
}

fn costs(raw: serde_json::Value) -> Vec<SourceCost> {
    serde_json::from_value(raw).unwrap()
}

#[test]
fn cost_terms_select_one_complete_runtime_cost_and_never_drop_a_term() {
    let artifact = card(CardKind::Artifact);
    let this = |noun: &str| json!({"cost":"sacrifice","what":{"ref":"this","noun":noun}});
    assert_eq!(
        costs::lower(&artifact, &costs(json!([{"cost":"tap-self"}]))).unwrap(),
        ActivationCost::TapSource
    );
    assert_eq!(
        costs::lower(
            &artifact,
            &costs(json!([{"cost":"mana","symbols":["2"]},{"cost":"tap-self"},this("artifact")]))
        )
        .unwrap(),
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: Some(mana_cost("{2}").unwrap()),
                sacrifice: true,
            }
        }
    );
    // CR 701.21a: the exact creature source keeps its dedicated cost.
    assert_eq!(
        costs::lower(&card(CardKind::Creature), &costs(json!([this("creature")]))).unwrap(),
        ActivationCost::SacrificeSource
    );
    assert!(matches!(
        costs::lower(&card(CardKind::Enchantment), &costs(json!([this("enchantment")]))).unwrap(),
        ActivationCost::Composite { costs } if costs.sacrifice && !costs.tap && costs.mana.is_none()
    ));
    // A self reference must name one of the card's own types (CR 201.5).
    assert!(costs::lower(&artifact, &costs(json!([this("creature")]))).is_err());
    let another = json!({"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1}}});
    assert!(matches!(
        costs::lower(
            &artifact,
            &costs(json!([{"cost":"mana","symbols":["1"]},another]))
        )
        .unwrap(),
        ActivationCost::SacrificePermanent {
            count: 1,
            mana: Some(_),
            ..
        }
    ));
    for unsupported in [
        json!([]),
        json!([{"cost":"tap-self"},{"cost":"tap-self"}]),
        json!([{"cost":"mana","symbols":["1"]},{"cost":"mana","symbols":["1"]}]),
        json!([{"cost":"mana","symbols":["C"]}]),
        json!([{"cost":"mana","symbols":["{1}"]}]),
        json!([{"cost":"tap-self"},another]),
        json!([{"cost":"tap-self"},{"cost":"pay-life","amount":{"amount":"fixed","value":1}}]),
    ] {
        assert!(
            costs::lower(&artifact, &costs(unsupported.clone())).is_err(),
            "{unsupported}"
        );
    }
}

#[test]
fn event_rows_only_add_self_entry_for_the_cards_own_type() {
    let enters = |noun: &str| -> SourceTrigger {
        serde_json::from_value(json!({"trigger":"enters","what":{"ref":"this","noun":noun}}))
            .unwrap()
    };
    assert_eq!(
        events::lower(&card(CardKind::Artifact), &enters("artifact")),
        Some(TriggerCondition::SelfEntersBattlefield)
    );
    assert_eq!(
        events::lower(&card(CardKind::Land), &enters("land")),
        Some(TriggerCondition::SelfEntersBattlefield)
    );
    // Another object's type is not a self reference.
    assert_eq!(
        events::lower(&card(CardKind::Land), &enters("artifact")),
        None
    );
    // The shared trigger lowerer keeps the spellings it already owns.
    assert_eq!(
        events::lower(&card(CardKind::Creature), &enters("creature")),
        None
    );
    let dies: SourceTrigger =
        serde_json::from_value(json!({"trigger":"dies","what":{"ref":"this","noun":"creature"}}))
            .unwrap();
    assert_eq!(events::lower(&card(CardKind::Creature), &dies), None);
}

#[test]
fn keyword_rows_reject_parameters_and_unknown_keywords() {
    let keywords =
        |raw: serde_json::Value| -> Vec<SourceKeyword> { serde_json::from_value(raw).unwrap() };
    let creature = card(CardKind::Creature);
    assert_eq!(
        keywords::lower(
            &creature,
            &keywords(json!([{"keyword":"flying"},{"keyword":"first-strike"}]))
        )
        .unwrap(),
        [KeywordAbility::Flying, KeywordAbility::FirstStrike]
    );
    assert!(keywords::lower(&creature, &[]).is_err());
    assert!(
        keywords::lower(
            &creature,
            &keywords(json!([{"keyword":"cycling","cost":[{"cost":"mana","symbols":["2"]}]}]))
        )
        .is_err()
    );
    assert!(keywords::lower(&creature, &keywords(json!([{"keyword":"cascade"}]))).is_err());
}

const R3: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json");
const STRIP: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-strip-mine.json");
const SEAL: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-seal-of-primordium.json");
const MARSH: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-piranha-marsh.json");
const DRAGON: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-destructor-dragon.json");
const CREW: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-hire-a-crew.json");
const WITNESSES: [&str; 9] = [
    include_str!("../../../../tests/fixtures/phase1-r3/compose-marble-chalice.json"),
    include_str!("../../../../tests/fixtures/phase1-r3/compose-rishadan-port.json"),
    STRIP,
    SEAL,
    include_str!("../../../../tests/fixtures/phase1-r3/compose-kamahl-pit-fighter.json"),
    include_str!("../../../../tests/fixtures/phase1-r3/compose-icatian-priest.json"),
    MARSH,
    DRAGON,
    CREW,
];

fn composed(raw: &str) -> Definition {
    let CompileResult::Lowered { definition } = compile(R3, raw) else {
        panic!("composed frozen source must lower: {:?}", compile(R3, raw));
    };
    assert!(definition.composed);
    assert!(
        definition
            .capabilities
            .contains(&Capability::ComposedAbilityV1)
    );
    *definition
}

/// Re-hash a changed archive row, keeping each line's AST equal to the aggregate.
fn changed(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    crate::tests::changed_source(raw, |row| {
        change(row);
        let result = &mut row["faces"][0]["result"];
        let abilities = result["abilities"].as_array().unwrap().clone();
        for (i, ability) in abilities.into_iter().enumerate() {
            result["lines"][i]["ability"] = ability;
        }
    })
}

#[test]
fn frozen_witnesses_lower_block_by_block_and_round_trip_executable_ir() {
    for raw in WITNESSES {
        let definition = composed(raw);
        definition.validate().unwrap();
        // Executable IR v1 round trip: the marker and capability survive, and
        // the strict reader accepts exactly what the compiler emitted.
        let catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![definition.clone()],
        };
        let json = serde_json::to_string(&catalog).unwrap();
        assert!(json.contains("\"composed\":true"));
        assert_eq!(Catalog::read(&json).unwrap().definitions, [definition]);
    }
}

#[test]
fn composed_blocks_keep_their_printed_order_cost_and_operands() {
    // A land: one mana block and one tap-and-sacrifice activation.
    let strip = composed(STRIP);
    assert_eq!(strip.mana_abilities.len(), 1);
    let [ability] = strip.activated.as_slice() else {
        panic!("one activation")
    };
    assert!(matches!(
        &ability.cost,
        ActivationCost::Composite { costs }
            if costs.tap && costs.sacrifice && costs.mana.is_none()
    ));
    assert!(matches!(
        ability.program.instructions.as_slice(),
        [Instruction::DestroyPermanent {
            target_slot: 0,
            optional: false,
            source_block: 1,
            ..
        }]
    ));
    assert_eq!(
        ability.program.targets[0].domain,
        TargetDomain::BattlefieldPermanent
    );
    // An enchantment sacrifices itself; the self noun is the card's own type.
    let seal = composed(SEAL);
    assert!(matches!(
        &seal.activated[0].cost,
        ActivationCost::Composite { costs } if costs.sacrifice && !costs.tap && costs.mana.is_none()
    ));
    // A land's own entry replacement, entry trigger and mana ability.
    let marsh = composed(MARSH);
    assert_eq!(marsh.static_abilities[0].effect, StaticEffect::EntersTapped);
    assert_eq!(
        marsh.triggered[0].condition,
        TriggerCondition::SelfEntersBattlefield
    );
    assert_eq!(
        marsh.triggered[0].program.targets[0].domain,
        TargetDomain::AnyPlayer
    );
    let labels = marsh.source_labels.as_ref().unwrap();
    assert_eq!(
        (labels.statics.as_slice(), labels.triggered.as_slice()),
        (&[0][..], &[1][..])
    );
    assert_eq!(labels.mana, [2]);
    // A spell sequence stays in printed order.
    let crew = composed(CREW);
    assert!(matches!(
        crew.spell.as_ref().unwrap().instructions.as_slice(),
        [
            Instruction::CreateCreatureTokens { count: 1, .. },
            Instruction::PumpSet {
                set: ObjectSet::ControllerCreatures,
                ..
            }
        ]
    ));
    // A creature's dies trigger on any noncreature permanent.
    let dragon = composed(DRAGON);
    assert_eq!(dragon.keywords, [KeywordAbility::Flying]);
    assert_eq!(dragon.triggered[0].condition, TriggerCondition::SelfDies);
}

#[test]
fn families_keep_precedence_and_composition_never_relabels_them() {
    let raw = include_str!("../../../../tests/fixtures/phase1-r3/bond-land.json");
    let CompileResult::Lowered { definition } = compile(R3, raw) else {
        panic!("family source must lower")
    };
    assert!(!definition.composed);
    assert!(
        !definition
            .capabilities
            .contains(&Capability::ComposedAbilityV1)
    );
}

#[test]
fn changed_sources_reject_instead_of_dropping_or_guessing() {
    let rejected = |raw: &str| !matches!(compile(R3, raw), CompileResult::Lowered { .. });
    // An unconsumed step field.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["unless"] = json!(true);
    })));
    // An extra cost term the runtime cannot pay atomically.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["costs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"cost":"pay-life","amount":{"amount":"fixed","value":1}}));
    })));
    // A self reference naming a type the card does not have (CR 201.5).
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["costs"][0]["what"]["noun"] = json!("creature");
    })));
    // An activation restriction outside the timing table.
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["restriction"] =
            json!({"restriction":"timing","turn":"your"});
    })));
    // The reference graph must be completely bound.
    assert!(rejected(&changed(DRAGON, |row| {
        row["binding"]["referencesBound"] = json!(false)
    })));
    assert!(rejected(&changed(DRAGON, |row| {
        row["binding"]["diagnostics"] = json!([{"code":"missing-antecedent"}]);
    })));
    assert!(rejected(&changed(DRAGON, |row| {
        row["binding"]["counts"]["unresolved"] = json!(1)
    })));
    // A pronoun bound to the source instead of the block's announced target.
    assert!(rejected(&changed(DRAGON, |row| {
        let id = row["oracleIdentity"].as_str().unwrap().to_owned();
        row["binding"]["references"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "path":"/faces/0/abilities/1/effects/0/steps/0/who/of",
                "expression":{"ref":"it"},"binding":"bound",
                "target":format!("card:{id}/source#object")
            }));
    })));
    // A clause removed from the AST leaves its bound references dangling,
    // and an injected clause has no bound event.
    // (A family may itself lower the shortened spell, so ask the pipeline.)
    let shortened = changed(CREW, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    assert!(lower(accepted_manifest(R3).unwrap(), CREW).is_ok());
    assert!(lower(accepted_manifest(R3).unwrap(), &shortened).is_err());
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array_mut()
            .unwrap()
            .push(json!({"effect":"draw","who":{"player":"you"},
                "amount":{"amount":"fixed","value":1}}));
    })));
    // A reference bound to a node that does not exist, a "you" bound to
    // another player, and a repeated node identity.
    assert!(rejected(&changed(DRAGON, |row| {
        row["binding"]["references"][0]["target"] = json!("absent");
    })));
    assert!(rejected(&changed(MARSH, |row| {
        let refs = row["binding"]["references"].as_array_mut().unwrap();
        let you = refs
            .iter()
            .position(|r| r["expression"] == json!({"player":"you"}))
            .unwrap();
        let other = refs
            .iter()
            .position(|r| r["expression"] != json!({"player":"you"}))
            .unwrap();
        refs[you]["target"] = refs[other]["target"].clone();
    })));
    assert!(rejected(&changed(DRAGON, |row| {
        let nodes = row["binding"]["nodes"].as_array_mut().unwrap();
        nodes.push(nodes.last().unwrap().clone());
    })));
    // A printed clause the AST does not carry.
    assert!(rejected(&changed(SEAL, |row| {
        let text = format!(
            "{} Draw a card.",
            row["source"]["oracle_text"].as_str().unwrap()
        );
        row["source"]["oracle_text"] = json!(text);
        row["faces"][0]["result"]["lines"][0]["text"] = json!(text);
    })));
    // A changed cost term that reference binding does not cover.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["costs"][0] =
            json!({"cost":"mana","symbols":["1"]});
    })));
    // Variable costs keep their reviewed families.
    assert!(rejected(
        &changed(CREW, |row| row["source"]["mana_cost"] = json!("{X}{R}"))
    ));
    // An intervening condition on a table event.
    assert!(rejected(&changed(MARSH, |row| {
        row["faces"][0]["result"]["abilities"][1]["condition"] = json!({"condition":"cards-in-hand","who":{"player":"you"},
                "comparison":{"op":"eq","value":{"amount":"fixed","value":0}}});
    })));
}

#[test]
fn timing_and_step_tables_lower_sorcery_speed_and_combat_restrictions() {
    // CR 602.5d: a sorcery-speed activation announces at main-phase timing.
    let restrict = |row: &mut serde_json::Value| {
        row["faces"][0]["result"]["abilities"][0]["restriction"] = json!({"restriction":"sorcery"});
    };
    // The restriction is one more printed sentence; without it the row's
    // text and AST disagree.
    assert!(!matches!(
        compile(R3, &changed(SEAL, restrict)),
        CompileResult::Lowered { .. }
    ));
    let sorcery = changed(SEAL, |row| {
        restrict(row);
        let text = format!(
            "{} Activate only as a sorcery.",
            row["source"]["oracle_text"].as_str().unwrap()
        );
        row["source"]["oracle_text"] = json!(text);
        row["faces"][0]["result"]["lines"][0]["text"] = json!(text);
    });
    let CompileResult::Lowered { definition } = compile(R3, &sorcery) else {
        panic!("timing row must lower")
    };
    assert!(definition.composed);
    assert_eq!(
        definition.activated[0].program.timing,
        SpellTiming::MainPhase
    );
    assert_eq!(
        composed(SEAL).activated[0].program.timing,
        SpellTiming::Instant
    );
    assert_eq!(
        timing(Some(json!({"restriction":"sorcery"}))).unwrap().0,
        SpellTiming::MainPhase
    );
    assert_eq!(timing(None).unwrap().0, SpellTiming::Instant);
    assert!(timing(Some(json!({"restriction":"sorcery","turn":"your"}))).is_err());
    // CR 509.1b: one announced creature, restricted until cleanup.
    let step = json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},
        "what":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":"line:0:3"}});
    assert!(steps::claims(&step));
    let mut program = instructions_program(SpellTiming::Instant);
    let instruction = steps::lower(&step, &mut program, &mut BTreeMap::new(), 0).unwrap();
    assert_eq!(
        instruction,
        Instruction::GrantCreatureRestriction {
            target_slot: 0,
            restriction: CreatureRestriction::CantBlock,
            source_block: 0
        }
    );
    assert_eq!(program.targets[0].domain, TargetDomain::BattlefieldCreature);
    for unclaimed in [
        json!({"effect":"cant","action":"attack","duration":{"duration":"this-turn"},"what":step["what"]}),
        json!({"effect":"cant","action":"block","duration":{"duration":"end-of-turn"},"what":step["what"]}),
        json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},"what":step["what"],"extra":1}),
        json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},
            "what":{"ref":"target","filter":{"types":["creature"],"control":"opponent"},
                "count":{"amount":"fixed","value":1},"selectionId":"line:0:3"}}),
    ] {
        assert!(!steps::claims(&unclaimed), "{unclaimed}");
    }
}
