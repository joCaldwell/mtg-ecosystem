//! CR 400.7, 603.3d/603.5 and 608.2b/d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn setup() -> Prepared {
    let mut rows = vec![
        ("Gravedigger", 0, 0, true),
        ("Fog", 0, 0, true),
        ("Fog", 1, 1, true),
        ("Forest", 0, 0, false),
    ];
    let mut prepared = if let Some(path) =
        std::env::var_os("MTG_OWN_GRAVEYARD_CARD_RETURN_ARCHIVE_PROBE")
    {
        use std::io::{BufRead, BufReader};
        let archive = BufReader::new(std::fs::File::open(path).expect("open frozen archive probe"));
        let row = archive
            .lines()
            .map(|line| line.expect("read archive row"))
            .find(|line| {
                line.contains("Eternal Witness")
                    && mtg_contracts::sha256(line.as_bytes())
                        == "feab01f792f10e115969d47513975742beb8087168fedcbae3b79cfdd5417a8e"
            })
            .expect("pinned complete Eternal Witness row 7272");
        rows[0].0 = "Eternal Witness";
        lands(&mut rows, "Forest", 0, 3);
        prepared_with_sources(&rows, &[&row])
    } else {
        lands(&mut rows, "Swamp", 0, 4);
        let mut prepared = prepared(&rows);
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Gravedigger")
            .unwrap();
        d.name = "Synthetic own-graveyard card return".into();
        d.oracle_id = "synthetic-own-graveyard-card-return".into();
        d.source_labels = None;

        let mut source: serde_json::Value =
            serde_json::from_str(&d.provenance.source_json).unwrap();
        source["name"] = d.name.clone().into();
        source["oracle_id"] = d.oracle_id.clone().into();
        source["oracle_text"] = "When this creature enters, you may return target card from your graveyard to your hand.".into();
        d.provenance.source_json = source.to_string();
        d.provenance.source_sha256 = mtg_contracts::sha256(d.provenance.source_json.as_bytes());
        let mut abilities: serde_json::Value =
            serde_json::from_str(&d.provenance.abilities_json).unwrap();
        abilities[0]["effects"][0]["steps"][0]["what"]["filter"]
            .as_object_mut()
            .unwrap()
            .remove("types");
        d.provenance.abilities_json = abilities.to_string();
        d.provenance.ast_sha256 =
            mtg_contracts::sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());

        let p = &mut d.triggered[0].program;
        p.targets[0].domain = TargetDomain::OwnGraveyardCard;
        p.instructions = vec![Instruction::ReturnOwnGraveyardCardToHand {
            target_slot: 0,
            optional: true,
            source_block: 0,
        }];
        d.capabilities = d.requirements();
        prepared.config.cards[0].oracle_id = d.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared
    };
    prepared.config.cards[2].zone = Zone::Graveyard { owner: PlayerId(1) };
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn own_graveyard_card_return_paid_entry_restores_optional_noncreature_return() {
    let prepared = setup();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    let chooser = PlayerId(0);
    let stale = object(&game, 1);
    cast(&mut game, 1, 1, vec![]);
    let target = object(&game, 1);
    assert_ne!(stale, target);
    assert_eq!(game.zone(target), Some(Zone::Graveyard { owner: chooser }));
    announce(&mut game, 1, 0, vec![]);
    let entry = game.top().unwrap();
    assert_eq!(entry.paid.as_ref(), Some(&entry.total_cost));
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    let Request::Targets {
        decision_id,
        player,
        candidates,
        ..
    } = request(&mut game, 1)
    else {
        panic!("entry trigger requires a target before resolution");
    };
    assert_eq!(player, chooser);
    assert_eq!(candidates, [target]);
    for invalid in [object(&game, 2), stale] {
        assert!(
            game.check_response(
                chooser,
                decision_id,
                Response::Targets {
                    objects: vec![invalid]
                }
            )
            .is_err()
        );
    }
    let QueryResult::Incomplete { work, .. } = game.player_decision_for(chooser, Budget::new(0))
    else {
        panic!("target decision query suspends without a budget");
    };
    let saved = game.save_work(&work).unwrap();
    let saved_value: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved_value["work_version"], 44);
    let work = game.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets {
        token,
        occurrences,
        candidates,
        ..
    }))) = game.resume_work(work, Budget::unlimited())
    else {
        panic!("controller receives public object-target request");
    };
    assert_eq!(occurrences[0].domain, TargetDomain::OwnGraveyardCard);
    assert_eq!(candidates, [game.handle_for(chooser, target).unwrap()]);
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        chooser,
        token,
        PlayerResponse::Targets {
            objects: candidates,
        },
        Budget::unlimited(),
    ) else {
        panic!("current graveyard handle validates");
    };
    game.submit(receipt).unwrap();
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(
        matches!(request(&mut game, 1), Request::OptionalRecursion { player, .. } if player == chooser)
    );
    let mut accepted = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    answer(&mut game, 1, Response::OptionalRecursion { perform: false });
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    assert_eq!(game.zone(target), Some(Zone::Graveyard { owner: chooser }));
    answer(
        &mut accepted,
        1,
        Response::OptionalRecursion { perform: true },
    );
    assert!(matches!(
        request(&mut accepted, 1),
        Request::Priority { .. }
    ));
    assert_eq!(accepted.zone(target), None);
    assert_eq!(
        accepted.zone(object(&accepted, 1)),
        Some(Zone::Hand { owner: chooser })
    );
    assert_eq!(
        accepted.zone(object(&accepted, 2)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}
