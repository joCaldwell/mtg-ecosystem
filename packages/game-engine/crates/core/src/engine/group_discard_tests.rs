//! CR 101.4a-b, 701.9a-b, 800.4a and 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn setup() -> Prepared {
    let mut rows = vec![
        ("Sign in Blood", 0, 0, true),
        ("Forest", 0, 0, true),
        ("Forest", 1, 1, true),
        ("Island", 1, 1, true),
        ("Caelorna, Coral Tyrant", 1, 1, true),
        ("Island", 2, 2, true),
        ("Forest", 2, 2, true),
        ("Plains", 2, 2, true),
        ("Swamp", 3, 3, true),
    ];
    lands(&mut rows, "Swamp", 0, 4);
    let mut prepared = if let Some(path) = std::env::var_os("MTG_GROUP_DISCARD_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let unnerve = archive
            .lines()
            .find(|line| {
                line.contains("\"name\":\"Unnerve\"")
                    && mtg_contracts::sha256(line.as_bytes())
                        == "576b9e5363d4fdbf05d7f4f1ddc3c793df835f76e948c7cb1353ea9d299e7513"
            })
            .expect("pinned complete Unnerve row");
        rows[0].0 = "Unnerve";
        prepared_with_sources(&rows, &[unnerve])
    } else {
        let mut prepared = prepared_with_sources(&rows, &[]);
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let spell = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Sign in Blood")
            .unwrap();
        spell.name = "Synthetic Group Discard".into();
        spell.oracle_id = "synthetic-group-discard".into();
        spell.source_labels = None;
        let program = spell.spell.as_mut().unwrap();
        program.targets.clear();
        program.instructions = vec![Instruction::DiscardPlayerSet {
            scope: GroupDiscardScope::EachOpponent,
            count: 2.into(),
            source_block: 0,
        }];
        spell.capabilities = spell.requirements();
        prepared.config.cards[0].oracle_id = spell.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared
    };
    prepared.config.commanders = vec![CardId(4)];
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

fn observation(game: &Game, viewer: PlayerId) -> Observation {
    let QueryValue::Observation(view) =
        finish_query(game, game.observe(viewer, Budget::unlimited()))
    else {
        panic!("missing observation")
    };
    view
}

fn choose(game: &mut Game, seat: PlayerId, selected: &[ObjectId]) {
    assert!(matches!(request(game, 1), Request::ResolveDiscard { player, .. } if player == seat));
    let QueryValue::Decision(Some(PlayerRequest::ResolveDiscard {
        token,
        candidates,
        count,
        ..
    })) = finish_query(game, game.player_decision_for(seat, Budget::unlimited()))
    else {
        panic!("private discard choice")
    };
    assert_eq!(count, selected.len());
    let hand = game.action_hand(seat);
    assert_eq!(
        candidates,
        hand.iter()
            .map(|o| game.handle_for(seat, *o).unwrap())
            .collect::<Vec<_>>()
    );
    for viewer in game.live_players().into_iter().filter(|&p| p != seat) {
        let view = observation(game, viewer);
        assert!(view.decision.is_none());
        assert!(
            view.view
                .objects
                .iter()
                .all(|o| o.zone != (Zone::Hand { owner: seat }))
        );
    }
    let response = PlayerResponse::ResolveDiscard {
        objects: selected
            .iter()
            .map(|o| game.handle_for(seat, *o).unwrap())
            .collect(),
    };
    assert!(matches!(
        game.check_player_response(
            PlayerId(0),
            token.clone(),
            response.clone(),
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    if count > 1 {
        let forged = PlayerResponse::ResolveDiscard {
            objects: vec![candidates[0].clone(); count],
        };
        assert!(matches!(
            game.check_player_response(seat, token.clone(), forged, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
    }
    let QueryValue::Validated(receipt) = finish_query(
        game,
        game.check_player_response(seat, token, response, Budget::unlimited()),
    ) else {
        panic!("accepted private selection")
    };
    game.submit(receipt).unwrap();
}

fn discards(game: &Game) -> Vec<&CommittedBatch> {
    batches(game)
        .into_iter()
        .filter(|b| matches!(b.cause, BatchCause::Instruction { .. }))
        .collect()
}

#[test]
fn group_discard_paid_private_apnap_simultaneous_and_restore() {
    let prepared = setup();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    let [caster, first, second, last] = [0, 1, 2, 3].map(PlayerId);
    announce(&mut game, 1, 0, vec![]);
    let spell = game.top().unwrap().clone();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    let hands = [caster, first, second, last].map(|p| game.action_hand(p));
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }

    let a = [hands[1][2], hands[1][0]];
    let b = [hands[2][1], hands[2][2]];
    let mut alternative = game.fork();
    choose(&mut alternative, first, &hands[1][..2]);
    assert!(
        matches!(request(&mut alternative, 1), Request::ResolveDiscard { player, .. } if player == second)
    );
    choose(&mut game, first, &a);
    assert!(
        matches!(request(&mut game, 1), Request::ResolveDiscard { player, .. } if player == second)
    );
    assert!(discards(&game).is_empty());
    assert_eq!(game.action_hand(first), hands[1]);
    for viewer in [caster, second, last] {
        assert_eq!(
            observation(&game, viewer),
            observation(&alternative, viewer)
        );
    }
    // A different legal selection has the same public projection and passes
    // structural validation, but cannot be restored against this input journal.
    let mut forged = game.fork();
    forged.state.rules.frame = alternative.state.rules.frame.clone();
    forged.validate_state().unwrap();
    assert!(Game::restore(prepared.clone(), &forged.snapshot().unwrap()).is_err());

    // Resume with the first private selection still buffered. Both a previous
    // chooser and the current chooser (with a submitted answer) can leave.
    let mut game = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    choose(&mut game, second, &b);
    let mut departed = game.fork();
    departed.concede(first).unwrap();
    departed.concede(second).unwrap();
    choose(&mut departed, last, &hands[3]);
    resolve(&mut departed, 1, |_| {});
    assert!(
        matches!(discards(&departed).as_slice(), [batch] if batch.occurrences.len() == 1
        && matches!(&batch.occurrences[0].actual, EventOutcome::CardDiscarded { player, movement } if *player == last && movement.previous == hands[3][0]))
    );
    assert!(
        a.iter()
            .chain(&b)
            .all(|o| departed.object_id(o.card).is_none())
    );

    assert!(
        matches!(request(&mut game, 1), Request::ResolveDiscard { player, .. } if player == last)
    );
    assert!(discards(&game).is_empty());
    for (seat, hand) in hands.iter().enumerate() {
        assert_eq!(game.action_hand(PlayerId(seat as u8)), *hand);
    }
    choose(&mut game, last, &hands[3]);
    assert!(
        matches!(request(&mut game, 1), Request::CommanderReturn { player, object, .. } if player == first && object.card == a[0].card)
    );
    assert!(game.state.rules.stack.is_empty());
    let effects = discards(&game);
    let [batch] = effects.as_slice() else {
        panic!("one discard group")
    };
    assert!(matches!(batch.group, EventGroup::Simultaneous { .. }));
    assert!(
        matches!(&batch.cause, BatchCause::Instruction { stack_id, player, source, cursor: 0, source_block: 0 } if *stack_id == spell.id && *player == caster && *source == spell.source)
    );
    let expected: Vec<_> = [
        (first, a.as_slice()),
        (second, b.as_slice()),
        (last, hands[3].as_slice()),
    ]
    .into_iter()
    .flat_map(|(player, cards)| cards.iter().map(move |o| (player, *o)))
    .collect();
    let actual: Vec<_> = batch
        .occurrences
        .iter()
        .map(|o| {
            let EventOutcome::CardDiscarded { player, movement } = &o.actual else {
                panic!("discard")
            };
            assert_eq!(movement.destination, Zone::Graveyard { owner: *player });
            assert_eq!(
                game.zone(game.object_id(movement.previous.card).unwrap()),
                Some(movement.destination)
            );
            (*player, movement.previous)
        })
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(game.action_hand(caster), hands[0]);
    answer(&mut game, 1, Response::CommanderReturn { to_command: true });
    resolve(&mut game, 1, |_| {});
    assert_eq!(
        game.zone(object(&game, 4)),
        Some(Zone::Command { owner: first })
    );
}
