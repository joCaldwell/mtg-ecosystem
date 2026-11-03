//! Complete archived source programs, not a new supported profile.
use super::*;

const IDS: [&str; 8] = [
    BLOOD_ARTIST,
    WARDEN,
    VISIONARY,
    choices::BOUNCER,
    FURNACE,
    GROWTH,
    DISFIGURE,
    FOG,
];
#[derive(Default, Serialize)]
struct Work {
    accepted_inputs: usize,
    advance_calls: usize,
    typed_fixture_moves: usize,
    samples: Vec<Sample>,
}
fn advance(game: &mut Game, work: &mut Work, rep: usize, collect: bool) -> Request {
    for _ in 0..1000 {
        work.advance_calls += 1;
        match timed(
            &mut work.samples,
            collect,
            "actual_advance",
            rep,
            0,
            0,
            || game.advance(Budget::new(64)),
        ) {
            Advance::AwaitingDecision(request) => return request,
            Advance::Suspended { .. } => (),
            other => panic!("finite heterogeneous advance: {other:?}"),
        }
    }
    panic!("heterogeneous advance call guard")
}
fn answer(
    game: &mut Game,
    work: &mut Work,
    rep: usize,
    collect: bool,
    request: &Request,
    response: Response,
) {
    timed(
        &mut work.samples,
        collect,
        "actual_respond",
        rep,
        0,
        0,
        || game.respond(request.player(), request.decision_id(), response),
    )
    .unwrap();
    work.accepted_inputs += 1;
}
fn game(enabled: bool, empty_hand: bool) -> Game {
    let catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/expected-choices-ir.json"
    ))
    .unwrap();
    let mut prepared = Prepared::admit(catalog.clone(), ScenarioConfig::choices()).unwrap();
    let mut config = prepared.config.clone();
    config.commanders.clear();
    config.cards.clear();
    for (index, id) in IDS.iter().enumerate() {
        let owner = PlayerId(match index {
            1 => 1,
            2 => 2,
            3 => 3,
            _ => 0,
        });
        config.cards.push(CardSetup {
            id: CardId(u32::try_from(index).unwrap()),
            oracle_id: if enabled { *id } else { FOREST }.into(),
            owner,
            controller: owner,
            zone: if index == 2 || index >= 5 {
                Zone::Hand { owner }
            } else {
                Zone::Battlefield
            },
        });
    }
    for player in 0..4u8 {
        config.cards.push(CardSetup {
            id: CardId(8 + u32::from(player)),
            oracle_id: BEARS.into(),
            owner: PlayerId(player),
            controller: PlayerId(player),
            zone: Zone::Battlefield,
        });
    }
    for (index, id) in [FOREST, SWAMP, FOREST].iter().enumerate() {
        config.cards.push(CardSetup {
            id: CardId(12 + u32::try_from(index).unwrap()),
            oracle_id: (*id).into(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: Zone::Battlefield,
        });
    }
    config.cards.push(CardSetup {
        id: CardId(15),
        oracle_id: FOREST.into(),
        owner: PlayerId(0),
        controller: PlayerId(0),
        zone: Zone::Hand { owner: PlayerId(0) },
    });
    config.cards.push(CardSetup {
        id: CardId(16),
        oracle_id: FOREST.into(),
        owner: PlayerId(3),
        controller: PlayerId(3),
        zone: if empty_hand {
            Zone::Library { owner: PlayerId(3) }
        } else {
            Zone::Hand { owner: PlayerId(3) }
        },
    });
    for index in 17..57u32 {
        let owner = PlayerId((index % 4) as u8);
        config.cards.push(CardSetup {
            id: CardId(index),
            oracle_id: FOREST.into(),
            owner,
            controller: owner,
            zone: Zone::Library { owner },
        });
    }
    assert!(
        Prepared::admit(catalog, config.clone()).is_err(),
        "modified table is never public admission"
    );
    prepared.profile_digest = digest(&(
        "synthetic-heterogeneous-eight-v1",
        sha256(CONTRACT.as_bytes()),
        enabled,
        empty_hand,
        &config,
    ))
    .unwrap();
    prepared.config = config;
    assert!(prepared.match_config.is_none());
    Game::with_trace(prepared, TraceMode::Detailed)
}
fn object(game: &Game, index: u32) -> ObjectId {
    game.object_id(CardId(index)).unwrap()
}
struct Cast {
    index: u32,
    land: u32,
    color: Color,
    target: Option<ObjectId>,
}
fn cast(game: &mut Game, work: &mut Work, rep: usize, collect: bool, plan: Cast) {
    let Cast {
        index,
        land,
        color,
        target,
    } = plan;
    let request = advance(game, work, rep, collect);
    assert!(matches!(
        request,
        Request::Priority {
            player: PlayerId(0),
            ..
        }
    ));
    answer(
        game,
        work,
        rep,
        collect,
        &request,
        Response::Cast {
            card: CardId(index),
        },
    );
    let mut request = advance(game, work, rep, collect);
    if let Some(target) = target {
        let Request::Targets {
            candidates,
            occurrences,
            ..
        } = &request
        else {
            panic!("archived one target")
        };
        assert_eq!(occurrences.len(), 1);
        assert!(candidates.contains(&target));
        answer(
            game,
            work,
            rep,
            collect,
            &request,
            Response::Targets {
                objects: vec![target],
            },
        );
        request = advance(game, work, rep, collect);
    }
    assert!(matches!(request, Request::Mana { .. }));
    let mana_object = object(game, land);
    answer(
        game,
        work,
        rep,
        collect,
        &request,
        Response::ActivateMana {
            object: mana_object,
        },
    );
    let request = advance(game, work, rep, collect);
    assert!(matches!(request, Request::Mana { .. }));
    answer(
        game,
        work,
        rep,
        collect,
        &request,
        Response::FinishMana { undo_mana: false },
    );
    let request = advance(game, work, rep, collect);
    assert!(matches!(request, Request::SpendMana { .. }));
    answer(
        game,
        work,
        rep,
        collect,
        &request,
        Response::SpendMana {
            spending: [(color, 1.into())].into(),
        },
    );
    for _ in 0..1000 {
        let request = advance(game, work, rep, collect);
        assert!(matches!(request, Request::Priority { .. }));
        if game.state.stack.is_empty() {
            assert_eq!(request.player(), PlayerId(0));
            return;
        }
        assert!(game.state.stack.last().unwrap().paid.is_some());
        answer(game, work, rep, collect, &request, Response::Pass);
    }
    panic!("finite complete-spell resolution guard")
}
fn move_card(
    game: &mut Game,
    work: &mut Work,
    rep: usize,
    collect: bool,
    index: u32,
    destination: Zone,
) -> ObjectId {
    let previous = object(game, index);
    let result = timed(
        &mut work.samples,
        collect,
        "actual_typed_fixture_move_and_trigger_collection",
        rep,
        0,
        0,
        || {
            game.commit(
                ProposedEvent::Move {
                    object: previous,
                    destination,
                },
                BatchCause::Fixture,
            )
        },
    )
    .unwrap();
    assert!(matches!(result, EventOutcome::Moved { .. }));
    work.typed_fixture_moves += 1;
    previous
}
fn program_manifest(game: &Game) -> Vec<serde_json::Value> {
    let entries=IDS.iter().map(|id| {
        let d=game.prepared.catalog.definitions.iter().find(|d|d.oracle_id==*id).unwrap();
        serde_json::json!({"oracle_id":d.oracle_id,"source_provenance":d.provenance,
            "complete_spell":d.spell,"complete_triggered":d.triggered,"complete_static":d.static_abilities})
    }).collect::<Vec<_>>();
    assert_eq!(
        entries
            .iter()
            .map(|e| e["oracle_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
    assert_eq!(
        entries
            .iter()
            .map(|e| canonical(&serde_json::json!([
                e["complete_spell"],
                e["complete_triggered"],
                e["complete_static"]
            ]))
            .unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        8
    );
    entries
}
fn damage_probe(
    game: &Game,
    enabled: bool,
    work: &mut Work,
    rep: usize,
    collect: bool,
) -> serde_json::Value {
    let mut probe = game.fork();
    let source = object(&probe, 8);
    // Archived Bears have base power2. The enabled row paid and resolved
    // Growth(+3) and Disfigure(-2), so CR510.1a/613.4c require power3 here.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let original_amount = if enabled { 3 } else { 2 };
    assert_eq!(
        probe.definition(source.card).unwrap().characteristics.power,
        Some(2.into())
    );
    assert_eq!(
        probe.characteristics(source).unwrap().power,
        Some(original_amount.into())
    );
    timed(
        &mut work.samples,
        collect,
        "prepare_fixture_combat_damage",
        rep,
        0,
        0,
        || {
            probe.prepare_combat_damage(vec![DamageOccurrence {
                source,
                controller: PlayerId(0),
                recipient: DamageRecipient::Player {
                    player: PlayerId(1),
                },
                amount: original_amount.into(),
            }])
        },
    )
    .unwrap();
    let group = timed(
        &mut work.samples,
        collect,
        "actual_damage_replacement_candidate_query",
        rep,
        0,
        0,
        || probe.damage_effect_group(),
    )
    .unwrap();
    if !enabled {
        assert!(group.is_none());
        return serde_json::json!({"eligible_pairs":0,"applied":0,"original_amount":original_amount});
    }
    let (chooser, recipient, pairs) = group.unwrap();
    assert_eq!(chooser, PlayerId(1));
    assert_eq!(
        recipient,
        TargetEntity::Player {
            player: PlayerId(1)
        }
    );
    let furnace = EffectKey::Static {
        source: object(&probe, 4),
        ability: 0,
    };
    let fog = EffectKey::Resolved { id: EffectId(2) };
    assert_eq!(pairs.len(), 2);
    assert!(pairs.iter().any(|p| p.effect == furnace));
    assert!(pairs.iter().any(|p| p.effect == fog));
    let transaction = probe.damage_transaction().unwrap().id;
    for (effect, expected) in [(furnace, 6), (fog, 0)] {
        let result = timed(
            &mut work.samples,
            collect,
            "actual_damage_replacement_application",
            rep,
            0,
            0,
            || {
                probe.commit(
                    ProposedEvent::ApplyDamageEffect {
                        transaction,
                        damage: DamageId(0),
                        effect,
                    },
                    BatchCause::Fixture,
                )
            },
        )
        .unwrap();
        let EventOutcome::DamageEffectApplied { application } = result else {
            panic!("actual replacement outcome")
        };
        assert_eq!(application.after, expected.into());
        assert_eq!(
            probe.damage_transaction().unwrap().entries[0].amount,
            expected.into()
        );
    }
    assert!(probe.damage_effect_group().unwrap().is_none());
    serde_json::json!({"eligible_pairs":2,"applied":2,"original_amount":original_amount,"amounts_after":[6,0],"actual_damage_commit_claim":false})
}
pub(super) fn row(enabled: bool, empty_hand: bool, rep: usize, collect: bool) -> serde_json::Value {
    let mut game = game(enabled, empty_hand);
    let manifest = program_manifest(&game);
    let mut work = Work::default();
    let base = game.definition(CardId(8)).unwrap().characteristics.clone();
    if enabled {
        // Pin complete archived programs before using actual announcement/payment.
        for (index, id, expected_power) in [(5u32, GROWTH, 3i64), (6u32, DISFIGURE, -2i64)] {
            let d = game.definition(CardId(index)).unwrap();
            assert_eq!(d.oracle_id, id);
            let p = d.spell.as_ref().unwrap();
            assert_eq!(p.instructions.len(), 1);
            assert_eq!(p.targets.len(), 1);
            assert!(
                matches!(&p.instructions[0],Instruction::ModifyPowerToughness{power,toughness,..}if *power==expected_power.into()&&*toughness==expected_power.into())
            );
        }
        let fog = game.definition(CardId(7)).unwrap().spell.as_ref().unwrap();
        assert_eq!(fog.instructions.len(), 1);
        assert!(matches!(
            fog.instructions[0],
            Instruction::PreventCombatDamage { .. }
        ));
        let target = object(&game, 8);
        for plan in [
            Cast {
                index: 5,
                land: 12,
                color: Color::G,
                target: Some(target),
            },
            Cast {
                index: 6,
                land: 13,
                color: Color::B,
                target: Some(target),
            },
            Cast {
                index: 7,
                land: 14,
                color: Color::G,
                target: None,
            },
        ] {
            cast(&mut game, &mut work, rep, collect, plan);
        }
        assert_eq!(game.state.effects.len(), 3);
        assert_eq!(
            game.events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCast { .. }))
                .count(),
            3
        );
        assert_eq!(
            game.events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCompleted { .. }))
                .count(),
            3
        );
    }
    let mut expected = base.clone();
    if enabled {
        expected.power.as_mut().unwrap().add(&1.into());
        expected.toughness.as_mut().unwrap().add(&1.into());
    }
    assert_eq!(game.characteristics(object(&game, 8)).unwrap(), expected);
    assert_eq!(game.static_damage_effects().len(), usize::from(enabled));
    let probe = damage_probe(&game, enabled, &mut work, rep, collect);
    let mut expected_pairs = BTreeSet::new();
    move_card(&mut game, &mut work, rep, collect, 2, Zone::Battlefield);
    if enabled {
        expected_pairs.insert((WARDEN.to_string(), CardId(2)));
        expected_pairs.insert((VISIONARY.to_string(), CardId(2)));
    }
    let peak_trigger_definitions = game
        .state
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield)
        .map(|o| game.definition(o.id.card).unwrap().triggered.len())
        .sum::<usize>();
    let peak_static_definitions = game.static_damage_effects().len();
    let peak_resolved_instances = game.state.effects.len();
    assert_eq!(peak_trigger_definitions, if enabled { 4 } else { 0 });
    assert_eq!(peak_static_definitions, usize::from(enabled));
    assert_eq!(peak_resolved_instances, if enabled { 3 } else { 0 });
    let before_nonmatch = game.state.pending_triggers.len();
    move_card(&mut game, &mut work, rep, collect, 15, Zone::Battlefield);
    assert_eq!(
        game.state.pending_triggers.len(),
        before_nonmatch,
        "land ETB never matches creature triggers"
    );
    move_card(
        &mut game,
        &mut work,
        rep,
        collect,
        9,
        Zone::Graveyard { owner: PlayerId(1) },
    );
    if enabled {
        expected_pairs.insert((BLOOD_ARTIST.to_string(), CardId(9)));
    }
    move_card(
        &mut game,
        &mut work,
        rep,
        collect,
        3,
        Zone::Graveyard { owner: PlayerId(3) },
    );
    if enabled {
        expected_pairs.insert((BLOOD_ARTIST.to_string(), CardId(3)));
        if empty_hand {
            expected_pairs.insert((choices::BOUNCER.to_string(), CardId(3)));
        }
    }
    let actual_pairs = game
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::TriggerCollected {
                source,
                witness: subject,
                ability,
                ..
            } => {
                assert_eq!(*ability, 0);
                Some((
                    game.definition(source.card).unwrap().oracle_id.clone(),
                    subject.subject().unwrap().card,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual_pairs.iter().cloned().collect::<BTreeSet<_>>(),
        expected_pairs
    );
    assert_eq!(actual_pairs.len(), expected_pairs.len());
    assert_eq!(game.state.pending_triggers.len(), expected_pairs.len());
    game.validate_trigger_state().unwrap();
    let checks = game
        .events()
        .iter()
        .filter_map(|e| {
            if let Event::TriggerConditionChecked { check, .. } = e {
                Some(check.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(checks.len(), usize::from(enabled));
    if enabled {
        assert_eq!(checks[0].controller, PlayerId(3));
        assert_eq!(checks[0].hand_count, u32::from(!empty_hand));
        assert_eq!(checks[0].passed, empty_hand);
    }
    let prior = canonical_state(&game.state);
    let expired = game.state.effects.iter().map(|e| e.id).collect::<Vec<_>>();
    let outcome = timed(
        &mut work.samples,
        collect,
        "actual_cleanup_expiry",
        rep,
        0,
        0,
        || game.commit(ProposedEvent::CleanupEffects, BatchCause::Fixture),
    )
    .unwrap();
    let EventOutcome::EffectsCleaned {
        expired: actual, ..
    } = outcome
    else {
        panic!("cleanup")
    };
    assert_eq!(actual, expired);
    assert!(game.state.effects.is_empty());
    assert_eq!(game.characteristics(object(&game, 8)).unwrap(), base);
    assert_eq!(
        game.static_damage_effects().len(),
        usize::from(enabled),
        "static provider does not expire with resolved effects"
    );
    let after = canonical_state(&game.state);
    serde_json::json!({"row":"source_backed_heterogeneous","repetition":rep,"warmup":rep==0,"enabled":enabled,"bouncer_controller_hand_empty":empty_hand,
        "unique_authentic_source_programs":if enabled{8}else{0},"complete_source_manifest":manifest,
        "peak_trigger_definition_instances":peak_trigger_definitions,"peak_static_definition_instances":peak_static_definitions,"peak_resolved_effect_instances":peak_resolved_instances,
        "unique_synthetic_generated_payloads":0,"unique_authentic_complete_semantic_payloads":if enabled{8}else{0},"actually_paid_completed_spells":if enabled{3}else{0},"resolved_effect_instances_before_cleanup":expired.len(),
        "static_provider_instances":usize::from(enabled),"actual_trigger_matches":actual_pairs.len(),"matched_source_subject_pairs":actual_pairs,
        "intervening_condition_checks":checks.len(),"condition_result":if enabled{Some(empty_hand)}else{None},"noncreature_ETB_trigger_matches":0,
        "matcher_internal_comparisons_not_instrumented":true,"damage_replacement_probe":probe,
        "cleanup_expired_instances":expired.len(),"static_provider_remains_after_cleanup":enabled,
        "before_cleanup_full_State_sha256":sha256(&prior),"after_cleanup_full_State_sha256":sha256(&after),
        "typed_fixture_move_batches_not_normal_game_reachability":true,"pending_trigger_placement_or_resolution_claim":false,
        "synthetic_configuration_admission_rejected":true,"work":work})
}
