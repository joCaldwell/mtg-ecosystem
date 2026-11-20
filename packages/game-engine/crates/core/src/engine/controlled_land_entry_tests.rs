use super::*;

const NECTARPOT: &str =
    include_str!("../../../../tests/fixtures/263b526c-8b81-44ee-a7c4-5c24bf9b42a4.json");

const TATYOVA: &str =
    include_str!("../../../../tests/fixtures/0715e860-3b3b-4331-9718-207973e94fee.json");

fn with_landfall(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let mut catalog = land_catalog();
    catalog
        .definitions
        .extend([lower(NECTARPOT), lower(TATYOVA)]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    midgame_with_catalog(rows, catalog)
}

#[test]
fn controlled_land_entry_queues_each_land_and_keeps_frozen_controller() {
    let mut game = Game::with_trace(
        with_landfall(&[
            ("Tatyova, Benthic Druid", 0, 0, false),
            ("Command Tower", 0, 0, true),
        ]),
        TraceMode::Detailed,
    );
    let _ = request(&mut game);
    let land = named(&game, "Command Tower");
    let handle = game.handle_for(PlayerId(0), land).unwrap();
    let before = game.state.rules.players[0].life.clone();
    let library_before = game.state.rules.libraries[&PlayerId(0)].len();
    public(&mut game, PlayerResponse::PlayLand { object: handle });
    let mut decision = request(&mut game);
    assert_eq!(game.state.rules.players[0].life, before);
    let trigger = game.state.rules.stack.last().unwrap();
    assert!(matches!(&trigger.kind, StackKind::Triggered {
        witness: TriggerEventWitness::ControlledLandEntered { entrant, .. }, ..
    } if entrant.object.card == land.card && entrant.controller == PlayerId(0)));
    check(&game);
    for _ in 0..4 {
        game.respond(decision.player(), decision.decision_id(), Response::Pass)
            .unwrap();
        decision = request(&mut game);
    }
    let mut total = before;
    total.add(&1.into());
    assert_eq!(game.state.rules.players[0].life, total);
    assert_eq!(
        game.state.rules.libraries[&PlayerId(0)].len(),
        library_before - 1
    );
    check(&game);
}

#[test]
fn controlled_land_entry_uses_common_post_entry_facts_per_land() {
    let mut game = Game::new(with_landfall(&[
        ("Kazandu Nectarpot", 0, 0, false),
        ("Kazandu Nectarpot", 1, 1, false),
        ("Island", 0, 0, true),
        ("Mountain", 0, 0, true),
        ("Plains", 1, 1, true),
    ]));
    let objects = ["Island", "Mountain", "Plains"].map(|name| named(&game, name));
    game.commit_batch(
        BatchCause::Fixture,
        EventGroup::Simultaneous {
            occurrences: objects
                .into_iter()
                .map(|object| ProposedEvent::Move {
                    object,
                    destination: Zone::Battlefield,
                })
                .collect(),
        },
    )
    .unwrap();
    let pending = game
        .state
        .rules
        .pending_triggers
        .iter()
        .filter_map(|trigger| {
            let TriggerEventWitness::ControlledLandEntered { entrant, .. } = &trigger.witness
            else {
                return None;
            };
            Some((entrant.object.card, trigger.controller))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        pending,
        [
            (objects[0].card, PlayerId(0)),
            (objects[1].card, PlayerId(0)),
            (objects[2].card, PlayerId(1))
        ]
        .into_iter()
        .collect()
    );
    game.stabilize().unwrap();
    let decision = request(&mut game);
    let Request::OrderTriggers {
        player: PlayerId(0),
        ref candidates,
        ..
    } = decision
    else {
        panic!("{decision:?}")
    };
    let order = candidates.iter().copied().rev().collect();
    game.respond(
        PlayerId(0),
        decision.decision_id(),
        Response::OrderTriggers { order },
    )
    .unwrap();
    let _ = request(&mut game);
    assert_eq!(
        game.state
            .rules
            .stack
            .iter()
            .map(|entry| entry.controller)
            .collect::<Vec<_>>(),
        [PlayerId(0), PlayerId(0), PlayerId(1)]
    );
}
