//! CR 400.7, 603.3d, 608.2b/h and 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::targeted_discard_tests::public_answer;
use super::*;
use std::io::{BufRead, BufReader};

// The contract authenticates this land shape against its archived source and
// AST, so the synthetic definition carries provenance that says the same thing.
fn synthetic_source(land: &mut Definition) {
    let text = [
        "This land enters tapped.",
        "When this land enters, exile target player's graveyard.",
        "{T}: Add {B}.",
    ]
    .join("\n");
    let mut source: serde_json::Value = serde_json::from_str(&land.provenance.source_json).unwrap();
    source["name"] = land.name.clone().into();
    source["oracle_id"] = land.oracle_id.clone().into();
    source["color_identity"] = serde_json::json!(["B"]);
    source["produced_mana"] = serde_json::json!(["B"]);
    source["oracle_text"] = text.into();
    land.provenance.source_json = source.to_string();
    land.provenance.source_sha256 = mtg_contracts::sha256(land.provenance.source_json.as_bytes());
    let this = serde_json::json!({"ref": "this", "noun": "land"});
    land.provenance.abilities_json = serde_json::json!([
        {"kind": "static", "effect": {"static": "enters-tapped", "what": this}},
        {
            "kind": "triggered",
            "trigger": {"trigger": "enters", "what": this},
            "effects": [{"steps": [{
                "effect": "exile",
                "what": {
                    "ref": "zone-cards",
                    "zone": "graveyard",
                    "owner": {"player": "target-player", "selectionId": "line:1:6"}
                }
            }]}]
        },
        {
            "kind": "activated",
            "costs": [{"cost": "tap-self"}],
            "effects": [{"steps": [{
                "effect": "add-mana",
                "who": {"player": "you"},
                "mana": {"mana": "fixed", "symbols": ["B"]}
            }]}]
        }
    ])
    .to_string();
    land.provenance.ast_sha256 =
        mtg_contracts::sha256(format!("[{}]", land.provenance.abilities_json).as_bytes());
}

fn setup() -> Prepared {
    let mut rows = vec![
        ("Command Tower", 0, 0, true),
        ("Isamaru, Hound of Konda", 1, 1, true),
        ("Grizzly Bears", 2, 2, true),
        ("Beast Within", 1, 1, true),
        ("Island", 1, 1, true),
    ];
    lands(&mut rows, "Forest", 1, 3);
    let mut prepared =
        if let Some(path) = std::env::var_os("MTG_ENTRY_GRAVEYARD_EXILE_ARCHIVE_PROBE") {
            let row = BufReader::new(std::fs::File::open(path).expect("open frozen archive probe"))
                .lines()
                .map(|line| line.expect("read archive row"))
                .find(|line| {
                    mtg_contracts::sha256(line.as_bytes())
                        == "9bd9cea6d63676699f88d8116a5ae5a3a3b9f621ab8ba5f380489f17934011e0"
                })
                .expect("pinned complete Bojuka Bog row 755");
            rows[0].0 = "Bojuka Bog";
            prepared_with_sources(&rows, &[&row])
        } else {
            let mut prepared = prepared_with_sources(&rows, &[]);
            let catalog = Arc::make_mut(&mut prepared.catalog);
            let mut trigger = catalog
                .definitions
                .iter()
                .find(|d| d.name == "Reclamation Sage")
                .unwrap()
                .triggered[0]
                .clone();
            let spec = &mut trigger.program.targets[0];
            spec.domain = TargetDomain::AnyPlayer;
            spec.filter = None;
            spec.selection_id = "line:1:6".into();
            trigger.program.instructions = vec![Instruction::exile_target_graveyard(0, 1)];
            let land = catalog
                .definitions
                .iter_mut()
                .find(|d| d.name == "Command Tower")
                .unwrap();
            land.name = "Synthetic graveyard exile land".into();
            land.oracle_id = "synthetic-graveyard-exile-land".into();
            land.source_labels = None;
            land.characteristics.color_identity = vec![Color::B];
            land.static_abilities = vec![StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTapped,
            }];
            land.triggered = vec![trigger];
            land.mana_abilities[0].production = ManaProduction::Fixed {
                symbols: vec![Color::B],
            };
            land.mana_abilities[0].source_block = 2;
            land.capabilities = land.requirements();
            synthetic_source(land);
            prepared.config.cards[0].oracle_id = land.oracle_id.clone();
            catalog
                .definitions
                .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
            catalog.validate().unwrap();
            prepared.catalog_digest = digest(catalog).unwrap();
            prepared
        };
    for card in &mut prepared.config.cards[1..=2] {
        card.zone = Zone::Graveyard { owner: card.owner };
    }
    prepared.config.commanders = vec![CardId(1)];
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn entry_graveyard_exile_uses_chosen_players_current_cards() {
    let prepared = setup();
    let controller = PlayerId(0);
    let chosen = PlayerId(1);
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    let hand = game.handle_for(controller, object(&game, 0)).unwrap();
    public_answer(
        &mut game,
        controller,
        PlayerResponse::PlayLand { object: hand },
    );
    let Request::PlayerTargets {
        player,
        candidates,
        occurrences,
        ..
    } = request(&mut game, 1)
    else {
        panic!("land entry must offer a player target");
    };
    assert_eq!(player, controller);
    assert_eq!(candidates, game.live_players());
    assert!(matches!(
        occurrences.as_slice(),
        [TargetSpec {
            domain: TargetDomain::AnyPlayer,
            slot: 0,
            ..
        }]
    ));
    let source = object(&game, 0);
    assert!(game.current(source).unwrap().tapped);
    assert_eq!(game.zone(source), Some(Zone::Battlefield));
    let id = game.top().unwrap().id;
    let mut game = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();

    // An empty graveyard is still a legal target, including the controller's.
    let mut empty = game.fork();
    public_answer(
        &mut empty,
        controller,
        PlayerResponse::PlayerTargets {
            players: vec![controller],
        },
    );
    resolve(&mut empty, 1, |_| {});
    assert!(empty.events().contains(&Event::TriggeredAbilityCompleted {
        stack_id: id,
        resolved: true
    }));
    assert!(
        batches(&empty).iter().all(
            |b| !matches!(b.cause, BatchCause::Instruction { stack_id, .. } if stack_id == id)
        )
    );

    public_answer(
        &mut game,
        controller,
        PlayerResponse::PlayerTargets {
            players: vec![chosen],
        },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(game.top().unwrap().player_targets, [chosen]);
    let mut departed = game.fork();
    departed.concede(chosen).unwrap();
    resolve(&mut departed, 1, |_| {});
    assert!(
        departed
            .events()
            .contains(&Event::TriggeredAbilityCompleted {
                stack_id: id,
                resolved: false
            })
    );

    // The response removes the trigger source and puts a new card into the
    // chosen player's graveyard after target placement.
    until_player(&mut game, 1, chosen);
    announce(
        &mut game,
        1,
        3,
        vec![TargetEntity::Object { object: source }],
    );
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(
        game.zone(object(&game, 0)),
        Some(Zone::Graveyard { owner: controller })
    );
    let affected = [object(&game, 1), object(&game, 3)];
    let graveyard = Zone::Graveyard { owner: chosen };
    assert!(
        affected
            .iter()
            .all(|old| game.zone(*old) == Some(graveyard))
    );
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(
        matches!(request(&mut game, 1), Request::CommanderReturn { player, object, destination: Zone::Exile, .. } if player == chosen && object == game.object_id(CardId(1)).unwrap())
    );
    assert!(game.state.rules.stack.is_empty());
    let moves: Vec<_> = batches(&game)
        .into_iter()
        .filter(|b| matches!(b.cause, BatchCause::Instruction { stack_id, .. } if stack_id == id))
        .collect();
    let [batch] = moves.as_slice() else {
        panic!("one graveyard exile batch");
    };
    assert!(matches!(batch.group, EventGroup::Simultaneous { .. }));
    assert_eq!(
        batch.cause,
        BatchCause::Instruction {
            stack_id: id,
            player: controller,
            source,
            cursor: 0,
            source_block: 1
        }
    );
    assert_eq!(batch.occurrences.len(), affected.len());
    for (occurrence, old) in batch.occurrences.iter().zip(affected) {
        let EventOutcome::Moved { movement } = &occurrence.actual else {
            panic!("actual exile move");
        };
        assert_eq!(movement.previous, old);
        assert_eq!(movement.from, graveyard);
        assert_eq!(movement.destination, Zone::Exile);
        assert_eq!(game.zone(movement.successor), Some(Zone::Exile));
        assert_eq!(game.state.rules.cards[&old.card].owner, chosen);
    }
    assert_eq!(
        game.zone(object(&game, 2)),
        Some(Zone::Graveyard { owner: PlayerId(2) })
    );
    assert_eq!(
        game.zone(object(&game, 4)),
        Some(Zone::Hand { owner: chosen })
    );
    public_answer(
        &mut game,
        chosen,
        PlayerResponse::CommanderReturn { to_command: true },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(
        game.zone(object(&game, 1)),
        Some(Zone::Command { owner: chosen })
    );
}
