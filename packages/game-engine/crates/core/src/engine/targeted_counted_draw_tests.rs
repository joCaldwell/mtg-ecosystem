//! Private paid draw witness. CR 121.2, 608.2b, 800.4, 903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const OPPORTUNITY: &str =
    include_str!("../../../../tests/fixtures/1f544a9f-c238-4858-be19-d6cd7d023dcc.json");

#[test]
fn targeted_counted_draw_paid_opportunity_commander_and_departure() {
    let caster = PlayerId(0);
    let recipient = PlayerId(1);
    let mut rows = vec![("Opportunity", 0, 0, true)];
    lands(&mut rows, "Island", 0, 6);
    rows.push(("Forest", 0, 0, true));
    let mut prepared = prepared_with_sources(&rows, &[OPPORTUNITY]);

    // The scenario starts with a tucked commander. It exercises the draw
    // replacement from that zone.
    let commander = prepared.config.initial_libraries()[&recipient][0];
    let oracle = &prepared
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Caelorna, Coral Tyrant")
        .unwrap()
        .oracle_id;
    prepared
        .config
        .cards
        .iter_mut()
        .find(|c| c.id == commander)
        .unwrap()
        .oracle_id = oracle.clone();
    prepared.config.commanders = vec![commander];
    prepared.profile_digest = digest(&prepared.config).unwrap();

    let mut game = Game::with_trace(prepared, TraceMode::Detailed);
    let q = 1;
    let caster_library = game.state.rules.libraries[&caster].clone();
    let retained_hand_card = object(&game, 7);
    let prefix = game.state.rules.libraries[&recipient]
        .iter()
        .map(|id| game.object_id(*id).unwrap())
        .collect::<Vec<_>>();
    let life = game
        .state
        .rules
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    answer(&mut game, q, Response::Cast { card: CardId(0) });
    answer(
        &mut game,
        q,
        Response::PlayerTargets {
            players: vec![recipient],
        },
    );
    fund(&mut game, q, "Island", 6);
    answer(&mut game, q, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 6.into())]),
        },
    );
    assert!(matches!(request(&mut game, q), Request::Priority { .. }));
    let paid = game
        .state
        .rules
        .stack
        .last()
        .unwrap()
        .paid
        .as_ref()
        .unwrap();
    assert_eq!(paid.generic, 4.into());
    assert_eq!(paid.colored, [Color::U, Color::U]);
    assert!((1..=6).all(|id| game.current(object(&game, id)).unwrap().tapped));
    assert_eq!(game.mana(caster, Color::U), Some(0.into()));

    let choice = (0..8)
        .find_map(|_| {
            let r = request(&mut game, q);
            if matches!(r, Request::CommanderReturn { .. }) {
                return Some(r);
            }
            answer(&mut game, q, pass(&r));
            None
        })
        .expect("commander replacement after priority passes");
    assert!(matches!(
        choice,
        Request::CommanderReturn { player, object, destination: Zone::Hand { owner }, .. }
            if player == recipient && object == prefix[0] && owner == recipient
    ));
    recover(&game);

    let state = game.state.clone();
    let trace = game.trace.clone();
    let QueryValue::Observation(own) = finish_query(&game, game.observe(recipient, Budget::new(q)))
    else {
        panic!("recipient observation")
    };
    let Some(PlayerRequest::CommanderReturn {
        token,
        object: None,
        ..
    }) = own.decision
    else {
        panic!("owner choice without a hidden object handle")
    };
    let QueryValue::Observation(other) = finish_query(&game, game.observe(caster, Budget::new(q)))
    else {
        panic!("caster observation")
    };
    assert!(other.decision.is_none());
    let QueryValue::Validated(receipt) = finish_query(
        &game,
        game.check_player_response(
            recipient,
            token,
            PlayerResponse::CommanderReturn { to_command: true },
            Budget::new(q),
        ),
    ) else {
        panic!("owner response")
    };
    assert_eq!(game.state, state);
    assert_eq!(game.trace, trace);
    game.submit(receipt).unwrap();
    assert!(matches!(
        game.advance(Budget::new(q)),
        Advance::Suspended { .. }
    ));
    assert!(matches!(
        &game.state.rules.frame,
        Frame::Resolve { frame } if frame.draw_progress.as_ref().is_some_and(|p|
            p.completed == 1.into() && p.remaining == 3.into() && p.failed == 0.into())
    ));
    assert_eq!(game.state.rules.libraries[&recipient][0], prefix[1].card);
    recover(&game);

    let mut departed = game.fork();
    departed.concede(recipient).unwrap();
    resolve(&mut departed, q, |_| {});
    assert!(!departed.is_live(recipient));
    assert_eq!(departed.state.rules.libraries[&caster], caster_library);
    assert_eq!(departed.action_hand(caster), [retained_hand_card]);
    recover(&departed);

    let mut attempts = None;
    resolve(&mut game, q, |g| {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && let Some(outcome @ InstructionOutcome::DrawAttempts { .. }) = frame.outcomes.first()
        {
            attempts = Some(outcome.clone());
        }
    });
    assert_eq!(
        attempts,
        Some(InstructionOutcome::DrawAttempts {
            player: recipient,
            attempted: 4.into(),
            failed: 0.into(),
        })
    );
    assert_eq!(game.state.rules.libraries[&caster], caster_library);
    assert_eq!(game.action_hand(caster), [retained_hand_card]);
    assert_eq!(game.action_hand(recipient).len(), 3);
    assert_eq!(
        game.zone(game.object_id(commander).unwrap()),
        Some(Zone::Command { owner: recipient })
    );
    for old in &prefix[1..4] {
        let now = game.object_id(old.card).unwrap();
        assert_eq!(now.incarnation, old.incarnation + 1);
        assert_eq!(game.zone(now), Some(Zone::Hand { owner: recipient }));
        assert!(game.handle_for(recipient, now).is_ok());
        for other in [caster, PlayerId(2), PlayerId(3)] {
            assert!(game.handle_for(other, now).is_err());
        }
    }
    let tail = game.state.rules.libraries[&recipient]
        .iter()
        .map(|id| game.object_id(*id).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(tail, prefix[4..]);
    assert!(game.state.rules.players.iter().map(|p| &p.life).eq(&life));
}
