//! Independent exact-admission and untrusted readback checks for the new pools.
mod support;
use mtg_contracts::{Catalog, Color, Instruction, Quantity};
use mtg_core::*;

fn old() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-commander-ir.json"
    ))
    .unwrap()
}
fn damage() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-damage-ir.json"
    ))
    .unwrap()
}
fn library() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-library-ir.json"
    ))
    .unwrap()
}
fn profiles() -> [(ScenarioConfig, Catalog); 3] {
    [
        (ScenarioConfig::commander_damage(), damage()),
        (ScenarioConfig::commander_damage_borrowed(), damage()),
        (ScenarioConfig::commander_library(), library()),
    ]
}

#[test]
fn profile_pool_and_ir_vocabulary_versions_are_independent_admission_requirements() {
    for (config, catalog) in profiles() {
        assert!(Prepared::admit(catalog.clone(), config.clone()).is_ok());
        for wrong in [
            old(),
            if config == ScenarioConfig::commander_library() {
                damage()
            } else {
                library()
            },
        ] {
            assert!(Prepared::admit(wrong, config.clone()).is_err());
        }
        for index in 0..catalog.definitions.len() {
            let mut missing = catalog.clone();
            missing.definitions.remove(index);
            assert!(Prepared::admit(missing, config.clone()).is_err());
        }
    }
    let mut downgrade = library();
    downgrade.schema_version = 3;
    assert!(downgrade.validate().is_err());
    assert!(Catalog::read(&mtg_contracts::canonical(&downgrade).unwrap()).is_err());
    let mut wrong_version = damage();
    wrong_version.schema_version = 4;
    wrong_version.validate().unwrap();
    assert!(Prepared::admit(wrong_version, ScenarioConfig::commander_damage()).is_err());
    assert!(Prepared::admit(damage(), ScenarioConfig::command_casting()).is_err());
    assert!(Prepared::admit(library(), ScenarioConfig::command_casting()).is_err());
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
    let mut changed = library();
    let ebb = changed
        .definitions
        .iter_mut()
        .find(|d| d.name == "Time Ebb")
        .unwrap();
    ebb.spell.as_mut().unwrap().instructions = vec![Instruction::ReturnToOwnerHand {
        target_slot: 0,
        source_block: 0,
    }];
    ebb.capabilities = ebb.requirements();
    changed.validate().unwrap();
    assert!(Prepared::admit(changed, ScenarioConfig::commander_library()).is_err());
}

fn request(game: &mut Game) -> Request {
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(r) => r,
        other => panic!("expected decision, got {other:?}"),
    }
}
fn answer(game: &mut Game, response: Response) {
    let finish = matches!(response, Response::FinishMana { .. });
    let r = request(game);
    game.respond(r.player(), r.decision_id(), response).unwrap();
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
        let prepared = Prepared::admit(damage(), config).unwrap();
        let mut game = Game::new(prepared.clone());
        answer(&mut game, Response::Pass);
        answer(&mut game, Response::Cast { card: CardId(3) });
        let commander = game.object_id(CardId(0)).unwrap();
        answer(
            &mut game,
            Response::Targets {
                objects: vec![commander],
            },
        );
        let island = game.object_id(CardId(2)).unwrap();
        answer(&mut game, Response::ActivateMana { object: island });
        answer(&mut game, Response::FinishMana { undo_mana: false });
        for _ in 0..4 {
            answer(&mut game, Response::Pass);
        }
        assert!(matches!(
            request(&mut game),
            Request::CommanderReturn {
                player: PlayerId(0),
                ..
            }
        ));
        answer(&mut game, Response::CommanderReturn { to_command: false });
        request(&mut game);
        let original = game.object_id(CardId(0)).unwrap();
        assert_eq!(game.zone(original), Some(Zone::Hand { owner: PlayerId(0) }));
        for undo in [false, true] {
            answer(&mut game, Response::Cast { card: CardId(0) });
            let Request::Mana { cost, .. } = request(&mut game) else {
                panic!("mana request")
            };
            assert_eq!(cost.generic, Quantity::from(3));
            assert_eq!(cost.colored, [Color::B, Color::B, Color::G]);
            restored(&game, &prepared);
            let forest = game.object_id(CardId(1)).unwrap();
            if game.mana(PlayerId(0), Color::G).unwrap() == Quantity::zero() {
                answer(&mut game, Response::ActivateMana { object: forest });
            }
            answer(&mut game, Response::FinishMana { undo_mana: undo });
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
    let prepared = Prepared::admit(library(), ScenarioConfig::commander_library()).unwrap();
    let mut game = Game::new(prepared.clone());
    answer(&mut game, Response::Cast { card: CardId(0) });
    let commander = game.object_id(CardId(4)).unwrap();
    answer(
        &mut game,
        Response::Targets {
            objects: vec![commander],
        },
    );
    for id in 1..=3 {
        let island = game.object_id(CardId(id)).unwrap();
        answer(&mut game, Response::ActivateMana { object: island });
    }
    answer(&mut game, Response::FinishMana { undo_mana: false });
    for _ in 0..4 {
        answer(&mut game, Response::Pass);
    }
    assert!(matches!(
        request(&mut game),
        Request::CommanderReturn {
            destination: Zone::Library { owner: PlayerId(1) },
            ..
        }
    ));
    answer(&mut game, Response::CommanderReturn { to_command: false });
    for _ in 0..128 {
        let req = request(&mut game);
        if matches!(
            req,
            Request::CommanderReturn {
                destination: Zone::Hand { owner: PlayerId(1) },
                ..
            }
        ) {
            return (prepared, game);
        }
        let response = match req {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            _ => panic!("unexpected passive request {req:?}"),
        };
        game.respond(req.player(), req.decision_id(), response)
            .unwrap();
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
