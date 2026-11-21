use super::*;
use mtg_witness::equipped::{equip, object, prepared};

fn synthetic_game(alternate_spell: Option<&str>) -> Game {
    let mut catalog = Catalog::read(include_str!(
        "../../../../../tests/fixtures/synthetic-controlled-phasing-ir.json"
    ))
    .unwrap();
    if let Some(source) = alternate_spell {
        let life = Catalog::read(source).unwrap().definitions.remove(0);
        catalog.definitions[0] = life;
    }
    let spell = catalog.definitions[0].oracle_id.clone();
    let land = catalog.definitions[1].oracle_id.clone();
    let attacker = if catalog.definitions[0]
        .capabilities
        .contains(&Capability::ControllerProtectionUntilNextTurnV1)
    {
        let base = Catalog::read(include_str!(
            "../../../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap();
        let creature = base
            .definitions
            .into_iter()
            .find(|d| d.name == "Grizzly Bears")
            .unwrap();
        let id = creature.oracle_id.clone();
        catalog.definitions.push(creature);
        Some(id)
    } else {
        None
    };
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "synthetic-controlled-phasing".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.commanders.clear();
    config.cards.clear();
    for (seat, zone, definition) in [
        (0, Zone::Hand { owner: PlayerId(0) }, 0),
        (0, Zone::Battlefield, 1),
        (1, Zone::Battlefield, 1),
    ] {
        config.cards.push(CardSetup {
            id: CardId(config.cards.len() as u32),
            oracle_id: if definition == 0 {
                spell.clone()
            } else {
                land.clone()
            },
            owner: PlayerId(seat),
            controller: PlayerId(seat),
            zone,
        });
    }
    if let Some(oracle_id) = attacker {
        config.cards.push(CardSetup {
            id: CardId(3),
            oracle_id,
            owner: PlayerId(1),
            controller: PlayerId(1),
            zone: Zone::Battlefield,
        });
    }
    for seat in (0..4).map(PlayerId) {
        for _ in 0..4 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: land.clone(),
                owner: seat,
                controller: seat,
                zone: Zone::Library { owner: seat },
            });
        }
    }
    let prepared = Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    };
    Game::with_trace(prepared, TraceMode::Detailed)
}

#[test]
fn paid_life_freeze_retains_damage_blocks_positive_payment_and_expires_on_next_turn() {
    use mtg_witness::equipped::{answer, cast, recover, request};
    let mut g = synthetic_game(Some(include_str!(
        "../../../../../tests/fixtures/synthetic-controller-life-freeze-ir.json"
    )));
    request(&mut g, 1);
    cast(&mut g, 1, 0, vec![]);
    let player = PlayerId(0);
    assert!(
        g.visible_player(player)
            .unwrap()
            .life_frozen_until_next_turn
    );
    assert!(
        !g.visible_player(PlayerId(1))
            .unwrap()
            .life_frozen_until_next_turn
    );
    recover(&g);
    let mut probe = g.fork();
    let amount = Quantity::from(3);
    let source = probe.object_before(object(&probe, 2)).unwrap();
    let outcome = probe
        .apply_noncombat_damage(&NoncombatDamageOccurrence {
            source,
            controller: PlayerId(1),
            recipient: DamageRecipient::Player { player },
            amount: amount.clone(),
        })
        .unwrap();
    assert!(
        matches!(outcome,EventOutcome::NoncombatDamageDealt{result,..} if result.previous==result.current)
    );
    for event in [
        ProposedEvent::GainLife {
            player,
            amount: amount.clone(),
        },
        ProposedEvent::LoseLife {
            player,
            amount: amount.clone(),
        },
    ] {
        assert_eq!(
            probe.commit(event, BatchCause::Fixture).unwrap(),
            EventOutcome::LifeChangeProhibited { player }
        );
    }
    assert!(!probe.state.rules.players[0].can_pay_life(&amount));
    assert!(probe.state.rules.players[0].can_pay_life(&Quantity::zero()));
    for _ in 0..600 {
        let r = request(&mut g, 1);
        if !g
            .visible_player(player)
            .unwrap()
            .life_frozen_until_next_turn
        {
            break;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            other => panic!("unexpected turn request {other:?}"),
        };
        answer(&mut g, 1, response);
    }
    assert!(
        !g.visible_player(player)
            .unwrap()
            .life_frozen_until_next_turn
    );
    assert_eq!(g.state.rules.turn_owner, player);
    assert!(g.state.rules.players[0].can_pay_life(&amount));
    recover(&g);
}

#[test]
fn paid_controller_window_compiles_phases_prevents_and_exiles_its_stack_source() {
    use mtg_witness::equipped::{cast, recover, request};
    let raw = include_str!(
        "../../../../../tests/fixtures/ramos/0d4ecdb1-ec90-497f-a7a4-1c68092b8757.json"
    );
    let compiled = mtg_compiler::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-current.json"),
        raw,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}")
    };
    let mut p = prepared(
        &[
            ("Plains", true),
            ("Plains", false),
            ("Plains", false),
            ("Plains", false),
            ("Forest", false),
        ],
        false,
    );
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    p.config.cards[4].owner = PlayerId(1);
    p.config.cards[4].controller = PlayerId(1);
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(&*p.catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut game = Game::with_trace(p, TraceMode::Detailed);
    request(&mut game, 1);
    cast(&mut game, 1, 0, vec![]);
    assert_eq!(game.current(object(&game, 0)).unwrap().zone, Zone::Exile);
    assert!(game.player_protected(PlayerId(0)));
    assert!(
        game.visible_player(PlayerId(0))
            .unwrap()
            .life_frozen_until_next_turn
    );
    for i in 1..=3 {
        assert!(game.current(object(&game, i)).unwrap().phased_out.is_some());
    }
    assert!(
        game.current(object(&game, 4))
            .unwrap()
            .present_on_battlefield()
    );
    recover(&game);
}

#[test]
fn paid_self_exile_moves_the_exact_spell_on_completion() {
    use mtg_witness::equipped::{cast, recover, request};
    let mut game = synthetic_game(Some(include_str!(
        "../../../../../tests/fixtures/synthetic-self-exile-ir.json"
    )));
    request(&mut game, 1);
    cast(&mut game, 1, 0, vec![]);
    assert!(game.state.rules.stack.is_empty());
    let completed = object(&game, 0);
    assert_eq!(game.current(completed).unwrap().zone, Zone::Exile);
    recover(&game);
}

#[test]
fn paid_controller_protection_excludes_targets_and_recovers_until_next_turn() {
    use mtg_witness::equipped::{answer, cast, recover, request};
    let mut g = synthetic_game(Some(include_str!(
        "../../../../../tests/fixtures/synthetic-controller-protection-ir.json"
    )));
    let player = PlayerId(0);
    request(&mut g, 1);
    cast(&mut g, 1, 0, vec![]);
    assert!(g.player_protected(player));
    assert!(
        !g.eligible_players(TargetDomain::AnyPlayer, PlayerId(1))
            .contains(&player)
    );
    assert!(g.targetable_player(PlayerId(1)));
    recover(&g);
    let mut attacked = false;
    for _ in 0..600 {
        let r = request(&mut g, 1);
        if !g.player_protected(player) {
            break;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers {
                player: seat,
                candidates,
                defenders,
                ..
            } if seat == PlayerId(1) => {
                let attacker = object(&g, 3);
                assert!(candidates.contains(&attacker) && defenders.contains(&player));
                attacked = true;
                Response::Attackers {
                    attacks: vec![(attacker, player)],
                }
            }
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::DamageEffect { candidates, .. } => Response::DamageEffect {
                damage: candidates[0].damage,
                effect: candidates[0].effect.clone(),
            },
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            other => panic!("unexpected turn request {other:?}"),
        };
        answer(&mut g, 1, response);
    }
    assert!(attacked);
    let outcomes = g
        .trace
        .records
        .iter()
        .filter_map(|r| {
            if let EvidenceRecord::Batch { batch } = &r.record {
                Some(&batch.occurrences)
            } else {
                None
            }
        })
        .flatten()
        .map(|o| &o.actual)
        .collect::<Vec<_>>();
    assert!(outcomes.iter().any(|o| matches!(o,EventOutcome::DamageEffectApplied {application} if application.prevented == Quantity::from(2) && application.after == Quantity::zero())));
    assert!(outcomes.iter().any(|o| matches!(o, EventOutcome::DamageCommitted {actual,commander_damage,..} if actual.is_empty() && commander_damage.is_empty())));
    assert!(g.targetable_player(player));
    assert_eq!(g.state.rules.turn_owner, player);
    recover(&g);
}

#[test]
fn paid_controller_phasing_replays_status_and_normal_untap_with_stable_handles() {
    use mtg_witness::equipped::{answer, cast, recover, request};
    let mut g = synthetic_game(None);
    request(&mut g, 1);
    let land = object(&g, 1);
    let opposing = object(&g, 2);
    let handle = g.handle_for(PlayerId(1), land).unwrap();
    cast(&mut g, 1, 0, vec![]);
    assert!(g.current(land).unwrap().tapped);
    assert_eq!(
        g.current(land).unwrap().phased_out,
        Some(PhaseReturn::Direct {
            controller_at_phase_out: PlayerId(0)
        })
    );
    assert!(g.current(opposing).unwrap().present_on_battlefield());
    assert!(g.mana_abilities(land).unwrap().is_empty());
    let filter = SacrificeFilter {
        types: vec![CardKind::Land],
        subtypes: vec![],
    };
    assert!(g.sacrifice_candidates(PlayerId(0), &filter).is_empty());
    assert_eq!(g.sacrifice_candidates(PlayerId(1), &filter), [opposing]);
    assert!(
        g.commit(
            ProposedEvent::Sacrifice {
                player: PlayerId(0),
                object: land
            },
            BatchCause::Fixture
        )
        .is_err()
    );

    assert_eq!(g.handle_for(PlayerId(1), land).unwrap(), handle);
    recover(&g);
    for _ in 0..600 {
        if g.current(land).unwrap().present_on_battlefield() {
            break;
        }
        let r = request(&mut g, 1);
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            other => panic!("unexpected turn request: {other:?}"),
        };
        answer(&mut g, 1, response);
    }
    request(&mut g, 1);
    assert_eq!(g.state.rules.turn_owner, PlayerId(0));
    assert!(g.current(land).unwrap().present_on_battlefield());
    assert!(!g.current(land).unwrap().tapped);
    assert_eq!(g.sacrifice_candidates(PlayerId(0), &filter), [land]);
    assert_eq!(g.handle_for(PlayerId(1), land).unwrap(), handle);
    recover(&g);
}

#[test]
fn targeted_phasing_changes_only_the_selected_incarnation_and_its_attachments() {
    let mut game = Game::with_trace(
        prepared(
            &[
                ("Darksteel Plate", false),
                ("Grizzly Bears", false),
                ("Forest", false),
                ("Forest", false),
            ],
            false,
        ),
        TraceMode::Detailed,
    );
    equip(&mut game, 1, 0, 1);
    let plate = object(&game, 0);
    let bear = object(&game, 1);
    let forest = object(&game, 2);
    let before = game.state.clone();
    let event = ProposedEvent::PhaseOutTarget { object: bear };
    assert!(game.commit(event.clone(), BatchCause::Fixture).is_err());
    assert!(game.apply_occurrence(&event).is_err());
    assert_eq!(game.state, before);

    let selection = PhaseSelection::Target(bear);
    let outcome = game.phase_out_permanents(selection).unwrap();
    assert_eq!(
        outcome,
        EventOutcome::PermanentsPhasedOut {
            controller: PlayerId(0),
            members: vec![
                (plate, PhaseReturn::Indirect { attached_to: bear }),
                (
                    bear,
                    PhaseReturn::Direct {
                        controller_at_phase_out: PlayerId(0)
                    }
                ),
            ],
        }
    );
    assert!(game.current(forest).unwrap().present_on_battlefield());
    assert!(game.phase_out_permanents(selection).is_err());
    assert!(
        game.phase_in_for_next_untap(PlayerId(0))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        game.phase_in_for_next_untap(PlayerId(3)).unwrap(),
        [plate, bear]
    );
    assert!(game.current(bear).unwrap().present_on_battlefield());
    assert_eq!(game.current(plate).unwrap().attached_to, Some(bear));
}

#[test]
fn controlled_phasing_freezes_attachment_identity_and_return_seat_on_live_table() {
    let mut p = prepared(
        &[
            ("Darksteel Plate", false),
            ("Grizzly Bears", false),
            ("Forest", false),
            ("Forest", false),
        ],
        false,
    );
    p.config.cards[1].owner = PlayerId(2);
    p.profile_digest = digest(&p.config).unwrap();
    let mut game = Game::with_trace(p, TraceMode::Detailed);
    equip(&mut game, 1, 0, 1);
    let plate = object(&game, 0);
    let bear = object(&game, 1);
    let mut schedule = PhasingSchedule::default();
    let before = game.state.clone();
    let ids = schedule.phase_out_controlled(&game, PlayerId(0)).unwrap();
    assert_eq!(ids, (0..4).map(|i| object(&game, i)).collect::<Vec<_>>());
    assert_eq!(
        schedule.return_condition(plate),
        Some(PhaseReturn::Indirect { attached_to: bear })
    );
    assert_eq!(
        schedule.return_condition(bear),
        Some(PhaseReturn::Direct {
            controller_at_phase_out: PlayerId(0)
        })
    );
    assert!(
        schedule
            .phase_out_controlled(&game, PlayerId(0))
            .unwrap()
            .is_empty()
    );
    let successor = ObjectId {
        card: bear.card,
        incarnation: bear.incarnation + 1,
    };
    assert_eq!(schedule.return_condition(successor), None);
    assert!(
        schedule
            .phase_in_at_untap(&game, PlayerId(0))
            .unwrap()
            .is_empty()
    );
    assert_eq!(schedule.phase_in_at_untap(&game, PlayerId(3)).unwrap(), ids);
    assert_eq!(game.state, before);
    assert!(
        game.commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::PhaseOutControlled {
                    controller: PlayerId(0)
                }
            }
        )
        .is_err()
    );
    game.phase_out_permanents(PhaseSelection::Controlled(PlayerId(0)))
        .unwrap();
    assert!(!game.controlled_creatures(PlayerId(0)).contains(&bear));
    for viewer in (0..4).map(PlayerId) {
        assert!(
            game.visible_object(viewer, game.current(bear).unwrap())
                .unwrap()
                .unwrap()
                .phased_out
        );
    }
    assert_eq!(game.zone(bear), Some(Zone::Battlefield));
    assert!(!game.current(bear).unwrap().present_on_battlefield());
    assert_eq!(game.current(plate).unwrap().attached_to, Some(bear));
    game.phase_in_for_next_untap(PlayerId(3)).unwrap();
    assert!(game.current(bear).unwrap().present_on_battlefield());
    assert_eq!(game.current(plate).unwrap().attached_to, Some(bear));
}
