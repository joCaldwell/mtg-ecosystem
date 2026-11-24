//! Independent exact-admission and untrusted readback checks for the new pools.
use crate::support;
use mtg_contracts::{Catalog, Color, Instruction, Quantity};
use mtg_core::*;

fn profiles() -> [(ScenarioConfig, Catalog); 3] {
    [
        (
            ScenarioConfig::commander_damage(),
            support::action_catalog(),
        ),
        (
            ScenarioConfig::commander_damage_borrowed(),
            support::action_catalog(),
        ),
        (
            ScenarioConfig::commander_library(),
            support::action_catalog(),
        ),
    ]
}

#[test]
fn each_scenario_requires_its_exact_pool_and_current_catalog_schema() {
    for (config, catalog) in profiles() {
        assert!(Prepared::admit(catalog.clone(), config.clone()).is_ok());
        let base = support::base_catalog();
        assert!(Prepared::admit(base, config.clone()).is_err());
        for index in 0..catalog.definitions.len() {
            let mut missing = catalog.clone();
            missing.definitions.remove(index);
            assert!(Prepared::admit(missing, config.clone()).is_err());
        }
    }
    for unsupported in [0, mtg_contracts::CURRENT_IR_VERSION + 1] {
        let mut invalid = support::action_catalog();
        invalid.schema_version = unsupported;
        assert!(Catalog::read(&mtg_contracts::canonical(&invalid).unwrap()).is_err());
        assert!(Prepared::admit(invalid, ScenarioConfig::commander_library()).is_err());
    }
}

#[test]
fn new_profiles_reject_structurally_valid_card_or_setup_widening() {
    for (config, catalog) in profiles() {
        for mutate in [
            (|c: &mut ScenarioConfig| c.mode = Mode::CommanderMatch) as fn(&mut ScenarioConfig),
            |c| c.commanders.clear(),
            |c| c.commanders = vec![CardId(999)],
            |c| c.commanders.push(CardId(1)),
            |c| c.cards[1].oracle_id = "b2c6aa39-2d2a-459c-a555-fb48ba993373".into(),
            |c| c.cards[1].oracle_id = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6".into(),
            |c| c.cards[0].owner = PlayerId(2),
            |c| c.cards[0].zone = Zone::Exile,
            |c| c.cards[0].id = CardId(999),
            |c| c.cards.push(c.cards[1].clone()),
            |c| c.cards.reverse(),
        ] {
            let mut bad = config.clone();
            mutate(&mut bad);
            if bad != config {
                assert!(Prepared::admit(catalog.clone(), bad).is_err());
            }
        }
        for mutate in [
            (|c: &mut Catalog| c.definitions.last_mut().unwrap().provenance.source_index += 1)
                as fn(&mut Catalog),
            |c| {
                c.definitions
                    .iter_mut()
                    .find(|d| d.characteristics.power.is_some())
                    .unwrap()
                    .characteristics
                    .power = Some(Quantity::from(7))
            },
            |c| {
                c.definitions
                    .iter_mut()
                    .find(|d| d.characteristics.power.is_some())
                    .unwrap()
                    .characteristics
                    .mana_cost
                    .generic = Quantity::zero()
            },
        ] {
            let mut bad = catalog.clone();
            mutate(&mut bad);
            bad.validate().unwrap();
            assert!(Prepared::admit(bad, config.clone()).is_err());
        }
    }
    let mut changed = support::action_catalog();
    let ebb = changed
        .definitions
        .iter_mut()
        .find(|d| d.name == "Time Ebb")
        .unwrap();
    ebb.spell.as_mut().unwrap().instructions = vec![Instruction::move_target(
        0,
        mtg_contracts::kir::Destination::OwnerHand,
        0,
    )];
    ebb.capabilities = ebb.requirements();
    changed.validate().unwrap();
    assert!(Prepared::admit(changed, ScenarioConfig::commander_library()).is_err());
}

fn request(game: &mut Game) -> Decision<Rules> {
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(r) => r,
        other => panic!("expected decision, got {other:?}"),
    }
}
fn answer(game: &mut Game, response: Answer) {
    let finish = matches!(
        response,
        Answer::Pay {
            pay: PayAnswer::Finish { .. }
        }
    );
    let r = request(game);
    game.respond(r.player, r.id, response).unwrap();
    if finish {
        support::complete_mana(game);
    }
}
fn restored(game: &Game, prepared: &Prepared) {
    let other = Game::restore(prepared.clone(), &game.snapshot().unwrap()).unwrap();
    assert_eq!(other.state_digest().unwrap(), game.state_digest().unwrap());
    assert_eq!(other.trace(), game.trace());
}

#[test]
fn authentic_multicolor_printed_cost_fails_with_monochromatic_sources_without_growing_tax() {
    for config in [
        ScenarioConfig::commander_damage(),
        ScenarioConfig::commander_damage_borrowed(),
    ] {
        let prepared = Prepared::admit(support::action_catalog(), config).unwrap();
        let mut game = Game::new(prepared.clone());
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            answer(&mut game, selected_answer)
        };
        {
            let selected_answer = game.next_test_decision().cast(CardId(3));
            answer(&mut game, selected_answer)
        };
        let commander = game.object_id(CardId(0)).unwrap();
        {
            let selected_answer = game.next_test_decision().targets(vec![commander]);
            answer(&mut game, selected_answer)
        };
        let island = game.object_id(CardId(2)).unwrap();
        {
            let selected_answer = game.next_test_decision().activate_mana(island);
            answer(&mut game, selected_answer)
        };
        {
            let selected_answer = Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: false,
                },
            };
            answer(&mut game, selected_answer)
        };
        for _ in 0..4 {
            {
                let selected_answer = Answer::Pick {
                    picks: vec![vec![0]],
                };
                answer(&mut game, selected_answer)
            };
        }
        assert!(matches!(
            request(&mut game),
            Decision {
                player: PlayerId(0),
                reason: Reason::CommanderReturn { .. },
                ..
            }
        ));
        {
            let selected_answer = game.next_test_decision().select(Candidate::Flag {
                flag: Flag::Destination,
            });
            answer(&mut game, selected_answer)
        };
        request(&mut game);
        let original = game.object_id(CardId(0)).unwrap();
        assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
        for undo in [false, true] {
            {
                let selected_answer = game.next_test_decision().cast(CardId(0));
                answer(&mut game, selected_answer)
            };
            let Decision {
                shape: Shape::Pay { cost, .. },
                reason: Reason::Mana { .. },
                ..
            } = request(&mut game)
            else {
                panic!("mana request")
            };
            assert_eq!(cost.generic, Quantity::from(3));
            assert_eq!(cost.colored, [Color::B, Color::B, Color::G]);
            restored(&game, &prepared);
            let forest = game.object_id(CardId(1)).unwrap();
            if game.mana(PlayerId(0), Color::G).unwrap() == Quantity::zero() {
                {
                    let selected_answer = game.next_test_decision().activate_mana(forest);
                    answer(&mut game, selected_answer)
                };
            }
            {
                let selected_answer = Answer::Pay {
                    pay: PayAnswer::Finish {
                        undo_on_failure: undo,
                    },
                };
                answer(&mut game, selected_answer)
            };
            request(&mut game);
            assert_eq!(game.object_id(CardId(0)), Some(original));
            assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
            assert_eq!(
                game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
                Quantity::zero()
            );
            restored(&game, &prepared);
        }
    }
}

fn draw_choice() -> (Prepared, Game) {
    let prepared = Prepared::admit(
        support::action_catalog(),
        ScenarioConfig::commander_library(),
    )
    .unwrap();
    let mut game = Game::new(prepared.clone());
    {
        let selected_answer = game.next_test_decision().cast(CardId(0));
        answer(&mut game, selected_answer)
    };
    let commander = game.object_id(CardId(4)).unwrap();
    {
        let selected_answer = game.next_test_decision().targets(vec![commander]);
        answer(&mut game, selected_answer)
    };
    for id in 1..=3 {
        let island = game.object_id(CardId(id)).unwrap();
        {
            let selected_answer = game.next_test_decision().activate_mana(island);
            answer(&mut game, selected_answer)
        };
    }
    {
        let selected_answer = Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        };
        answer(&mut game, selected_answer)
    };
    for _ in 0..4 {
        {
            let selected_answer = Answer::Pick {
                picks: vec![vec![0]],
            };
            answer(&mut game, selected_answer)
        };
    }
    assert!(matches!(
        request(&mut game),
        Decision {
            reason: Reason::CommanderReturn {
                destination: Zone::Library { owner: PlayerId(1) },
                ..
            },
            ..
        }
    ));
    {
        let selected_answer = game.next_test_decision().select(Candidate::Flag {
            flag: Flag::Destination,
        });
        answer(&mut game, selected_answer)
    };
    for _ in 0..128 {
        let req = request(&mut game);
        if matches!(
            req,
            Decision {
                reason: Reason::CommanderReturn {
                    destination: Zone::Hand { owner: PlayerId(1) },
                    ..
                },
                ..
            }
        ) {
            return (prepared, game);
        }
        let response = match req.clone() {
            Decision {
                reason: Reason::Priority,
                ..
            } => Answer::Pick {
                picks: vec![vec![0]],
            },
            Decision {
                reason: Reason::Attackers,
                ..
            } => req.attack(vec![]),
            _ => panic!("unexpected passive request {req:?}"),
        };
        game.respond(req.player, req.id, response).unwrap();
    }
    panic!("commander draw choice unreachable")
}

#[test]
fn nested_hidden_draw_snapshots_reject_forged_identity_owner_top_and_context_without_panics() {
    let (prepared, game) = draw_choice();
    restored(&game, &prepared);
    let snapshot: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    for mutate in [
        (|v: &mut serde_json::Value| v["state"]["frame"]["object"]["card"] = 999.into())
            as fn(&mut serde_json::Value),
        |v| v["state"]["frame"]["object"]["incarnation"] = 999.into(),
        |v| v["state"]["frame"]["continuation"]["player"] = 255.into(),
        |v| v["state"]["frame"]["destination"] = serde_json::json!({"zone":"library","owner":1}),
        |v| v["state"]["libraries"]["1"] = serde_json::json!([]),
        |v| v["state"]["libraries"]["1"] = serde_json::json!([999, 10, 11]),
        |v| v["state"]["libraries"]["1"] = serde_json::json!([10, 4, 11]),
        |v| v["state"]["libraries"]["1"] = serde_json::json!([4, 4, 10, 11]),
        |v| v["state"]["turn"]["step"] = "precombat_main".into(),
        |v| {
            v["state"]["objects"]
                .as_object_mut()
                .unwrap()
                .remove("4")
                .map(|_| ())
                .unwrap()
        },
    ] {
        let mut forged = snapshot.clone();
        mutate(&mut forged);
        forged["state_sha256"] = mtg_contracts::digest(&forged["state"]).unwrap().into();
        let raw = serde_json::to_string(&forged).unwrap();
        let result = std::panic::catch_unwind(|| Game::restore(prepared.clone(), &raw));
        assert!(result.is_ok(), "malformed nested draw snapshot panicked");
        assert!(result.unwrap().is_err());
    }
    assert_eq!(
        game.snapshot().unwrap(),
        serde_json::to_string(&snapshot).unwrap()
    );
}
