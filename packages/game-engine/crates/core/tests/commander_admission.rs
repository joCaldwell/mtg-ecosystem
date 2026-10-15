//! Independent closed-pool admission and retained-input checks. These fixtures
//! are reviewed archive-derived IR, never a dependency on the compiler in core.
use mtg_contracts::{Catalog, Color, Quantity};
use mtg_core::*;

fn original_catalog() -> Catalog {
    Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap()
}
fn commander_catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../tests/fixtures/expected-commander-ir.json"
    ))
    .unwrap()
}
fn commander_profiles() -> [ScenarioConfig; 3] {
    [
        ScenarioConfig::command_casting(),
        ScenarioConfig::commander_lifecycle(),
        ScenarioConfig::commander_exile(),
    ]
}

#[test]
fn each_profile_requires_its_exact_reviewed_catalog_closure() {
    for config in commander_profiles() {
        assert!(Prepared::admit(commander_catalog(), config.clone()).is_ok());
        assert!(Prepared::admit(original_catalog(), config.clone()).is_err());
        for index in 0..commander_catalog().definitions.len() {
            let mut missing = commander_catalog();
            missing.definitions.remove(index);
            assert!(Prepared::admit(missing, config.clone()).is_err());
        }
        let mut duplicate = commander_catalog();
        duplicate.definitions.push(duplicate.definitions[0].clone());
        assert!(Prepared::admit(duplicate, config).is_err());
    }
    for config in [
        ScenarioConfig::land_return(),
        ScenarioConfig::commander_foundation(),
        ScenarioConfig::stack_response(),
        ScenarioConfig::casting_reversal(),
        ScenarioConfig::turns_permanents(),
        ScenarioConfig::combat_game(),
        ScenarioConfig::combat_interactions(),
    ] {
        assert!(Prepared::admit(original_catalog(), config.clone()).is_ok());
        assert!(Prepared::admit(commander_catalog(), config).is_err());
    }
}

#[test]
fn structural_ir_validity_never_substitutes_for_definition_admission() {
    type Mutation = fn(&mut Catalog);
    let mutations: Vec<Mutation> = vec![
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name == "Plains")
                .unwrap()
                .name
                .push_str(" renamed")
        },
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name == "Plains")
                .unwrap()
                .characteristics
                .color_identity = vec![Color::U]
        },
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name.starts_with("Isamaru"))
                .unwrap()
                .characteristics
                .mana_cost
                .generic = Quantity::from(2)
        },
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name.starts_with("Isamaru"))
                .unwrap()
                .characteristics
                .mana_cost
                .colored = vec![Color::U]
        },
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name.starts_with("Isamaru"))
                .unwrap()
                .characteristics
                .power = Some(Quantity::from(3))
        },
        |c| {
            c.definitions
                .iter_mut()
                .find(|d| d.name == "Plains")
                .unwrap()
                .provenance
                .source_index += 1
        },
    ];
    for config in commander_profiles() {
        for mutate in &mutations {
            let mut catalog = commander_catalog();
            mutate(&mut catalog);
            catalog.validate().unwrap();
            assert!(Prepared::admit(catalog, config.clone()).is_err());
        }
    }
}

#[test]
fn commander_profiles_reject_unreviewed_designations_mana_and_setup() {
    type Mutation = fn(&mut ScenarioConfig);
    let mutations: Vec<Mutation> = vec![
        |c| c.mode = Mode::CommanderMatch,
        |c| c.players = 3,
        |c| c.profile.push_str("-unreviewed"),
        |c| c.commanders.clear(),
        |c| c.commanders.push(CardId(0)),
        |c| c.commanders = vec![CardId(1)],
        |c| c.commanders = vec![CardId(999)],
        |c| c.cards[0].oracle_id = "14c8f55d-d177-4c25-a931-ebeb9e6062a0".into(),
        |c| c.cards[0].owner = PlayerId(1),
        |c| c.cards[0].zone = Zone::Hand { owner: PlayerId(0) },
        |c| c.cards[1].oracle_id = "b2c6aa39-2d2a-459c-a555-fb48ba993373".into(),
        |c| c.cards[1].controller = PlayerId(1),
        |c| c.cards[1].owner = PlayerId(1),
        |c| c.cards[1].zone = Zone::Graveyard { owner: PlayerId(0) },
        |c| c.cards[1].id = CardId(999),
        |c| {
            c.cards.pop();
        },
        |c| c.cards.push(c.cards[1].clone()),
        |c| c.cards.swap(1, 2),
    ];
    for config in commander_profiles() {
        for mutate in &mutations {
            let mut changed = config.clone();
            mutate(&mut changed);
            assert_ne!(changed, config);
            assert!(Prepared::admit(commander_catalog(), changed).is_err());
        }
    }
}

fn reject_forged_snapshot(
    prepared: &Prepared,
    snapshot: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let mut value: serde_json::Value = serde_json::from_str(snapshot).unwrap();
    mutate(&mut value);
    value["state_sha256"] = mtg_contracts::digest(&value["state"]).unwrap().into();
    let raw = serde_json::to_string(&value).unwrap();
    let result = std::panic::catch_unwind(|| Game::restore(prepared.clone(), &raw));
    assert!(
        result.is_ok(),
        "malformed snapshots must reject without panicking"
    );
    assert!(result.unwrap().is_err(), "forged snapshot was accepted");
}

#[test]
fn commander_snapshot_versions_and_physical_designations_are_validated() {
    let prepared = Prepared::admit(commander_catalog(), ScenarioConfig::command_casting()).unwrap();
    let game = Game::new(prepared.clone());
    let snapshot = game.snapshot().unwrap();
    assert!(Game::restore(prepared.clone(), &snapshot).is_ok());
    reject_forged_snapshot(&prepared, &snapshot, |v| v["snapshot_version"] = 8.into());
    reject_forged_snapshot(&prepared, &snapshot, |v| {
        v["state"]["unexpected_commander_semantics"] = true.into()
    });
    reject_forged_snapshot(&prepared, &snapshot, |v| {
        v["state"]["commanders"]["records"]["0"]["owner"] = 1.into();
    });
    reject_forged_snapshot(&prepared, &snapshot, |v| {
        v["state"]["commanders"]["records"]["0"]["command_casts"]["0"] = "1".into();
    });
    reject_forged_snapshot(&prepared, &snapshot, |v| {
        let mut rogue_object = v["state"]["objects"]["0"].clone();
        rogue_object["id"]["card"] = 999.into();
        rogue_object["zone"] = serde_json::json!({"zone":"graveyard","owner":0});
        v["state"]["objects"]["999"] = rogue_object;
        v["state"]["commanders"]["records"]["999"] =
            v["state"]["commanders"]["records"]["0"].clone();
        v["state"]["frame"] = serde_json::json!({
            "stage":"commander_return", "object":{"card":999,"incarnation":0},
            "destination":{"zone":"graveyard","owner":0},
            "continuation":{"continuation":"state_based","resume":{"stage":"priority"}}
        });
    });
}

fn answer(game: &mut Game, response: Response) {
    let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
        panic!("expected a decision");
    };
    game.respond(request.player(), request.decision_id(), response)
        .unwrap();
}

#[test]
fn command_tax_is_frozen_before_completed_cast_history_changes() {
    let prepared = Prepared::admit(commander_catalog(), ScenarioConfig::command_casting()).unwrap();
    let mut game = Game::new(prepared.clone());
    answer(&mut game, Response::Cast { card: CardId(0) });
    let Advance::AwaitingDecision(Request::Mana { cost, .. }) = game.advance(Budget::unlimited())
    else {
        panic!("expected casting mana request");
    };
    assert_eq!(cost.generic, Quantity::zero());
    assert_eq!(cost.colored, [Color::W]);
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::zero()
    );
    let announcement = game.snapshot().unwrap();
    assert!(Game::restore(prepared.clone(), &announcement).is_ok());
    reject_forged_snapshot(&prepared, &announcement, |v| {
        v["state"]["stack"][0]["total_cost"]["generic"] = "2".into();
    });
    reject_forged_snapshot(&prepared, &announcement, |v| {
        v["state"]["stack"][0]["cast_from"] = serde_json::json!({"zone":"hand","owner":0});
    });
    let plains = game.object_id(CardId(1)).unwrap();
    answer(&mut game, Response::ActivateMana { object: plains });
    answer(&mut game, Response::FinishMana { undo_mana: false });
    assert!(matches!(
        game.advance(Budget::unlimited()),
        Advance::AwaitingDecision(Request::Priority { .. })
    ));
    assert_eq!(
        game.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
        Quantity::from(2)
    );
    let completed = game.snapshot().unwrap();
    assert!(Game::restore(prepared.clone(), &completed).is_ok());
    let value: serde_json::Value = serde_json::from_str(&completed).unwrap();
    assert_eq!(value["state"]["stack"][0]["total_cost"]["generic"], "0");
    assert_eq!(value["state"]["stack"][0]["paid"]["generic"], "0");
    reject_forged_snapshot(&prepared, &completed, |v| {
        v["state"]["stack"][0]["total_cost"]["generic"] = "2".into();
        v["state"]["stack"][0]["paid"]["generic"] = "2".into();
    });
}
