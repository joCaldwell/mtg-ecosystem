//! CR 601.2b, 608.2b, 700.2a, 702.16b, 903.3 (frozen 20260925).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::targeted_discard_tests::public_answer;
use super::*;

const WILL: &str =
    include_str!("../../../../../tests/fixtures/ramos/fd949f82-fc10-4e37-8aa9-6c7569fe3c55.json");
const CASTER: PlayerId = PlayerId(0);

fn mode_offer(game: &mut Game, expected: &[u32]) -> DecisionToken {
    assert!(
        matches!(request(game, 4096), Request::Modes { candidates, .. } if candidates == expected)
    );
    let QueryResult::Complete(QueryValue::Observation(obs)) =
        game.observe(CASTER, Budget::unlimited())
    else {
        panic!("expected modal observation");
    };
    let Some(PlayerRequest::Modes {
        token, candidates, ..
    }) = obs.decision
    else {
        panic!("expected public mode offer");
    };
    assert_eq!(candidates, expected);
    let labels = obs.decision_context.unwrap().modes;
    assert!(labels.iter().all(|label| !label.text.is_empty()));
    assert_eq!(
        labels.iter().map(|label| label.mode).collect::<Vec<_>>(),
        expected
    );
    token
}

#[test]
fn commander_modal_keyword_paid_borrowed_commander_and_protection() {
    let mut rows = vec![
        ("Akroma's Will", 0, 0, true),
        ("Caelorna, Coral Tyrant", 1, 0, false), // Borrowed commander.
        ("Isamaru, Hound of Konda", 0, 0, false), // Undesignated physical copy.
        ("Isamaru, Hound of Konda", 0, 0, false), // Own commander in command zone.
        ("Unsummon", 0, 0, true),
        ("Bonesplitter", 0, 0, false),
        ("Island", 0, 0, false),
        ("Pyroclasm", 0, 0, true),
        ("Mountain", 0, 0, false),
        ("Grizzly Bears", 2, 2, false),
        ("Island", 0, 0, false),
    ];
    lands(&mut rows, "Plains", 0, 5);
    let mut prepared = prepared_with_sources(&rows, &[WILL]);
    prepared.config.commanders = vec![CardId(1), CardId(3)];
    prepared.config.cards[3].zone = Zone::Command { owner: CASTER };
    prepared.profile_digest = digest(&prepared.config).unwrap();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);

    announce(&mut game, 4096, 7, vec![]);
    let recipient = object(&game, 2);
    announce(
        &mut game,
        4096,
        4,
        vec![TargetEntity::Object { object: recipient }],
    );
    let bounce = game.top().unwrap().id;
    let card = game.handle_for(CASTER, object(&game, 0)).unwrap();
    public_answer(&mut game, CASTER, PlayerResponse::Cast { card });
    mode_offer(&mut game, &[0, 1, 2]);

    // Restore with the choice open and a public decision query partway through its candidates.
    let QueryResult::Incomplete { work, .. } = game.player_decision_for(CASTER, Budget::new(2))
    else {
        panic!("expected a partial mode list");
    };
    let saved = game.save_work(&work).unwrap();
    let mut game = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    let work = game.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Modes {
        candidates,
        token,
        ..
    }))) = game.resume_work(work, Budget::unlimited())
    else {
        panic!("expected restored mode candidates");
    };
    assert_eq!(candidates, [0, 1, 2]);
    let QueryResult::Complete(QueryValue::Validated(choice)) = game.check_player_response(
        CASTER,
        token,
        PlayerResponse::Modes { mode: 2 },
        Budget::unlimited(),
    ) else {
        panic!("borrowed commander permits both modes");
    };
    game.submit(choice).unwrap();

    // If the owner leaves before binding, only the two single choices remain.
    let mut unbound = game.fork();
    unbound.concede(PlayerId(1)).unwrap();
    let token = mode_offer(&mut unbound, &[0, 1]);
    let before = unbound.state_digest().unwrap();
    assert!(matches!(
        unbound.check_player_response(
            CASTER,
            token,
            PlayerResponse::Modes { mode: 2 },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(unbound.state_digest().unwrap(), before);

    // Once bound, the same departure before payment preserves the choice.
    assert!(matches!(request(&mut game, 4096), Request::Mana { .. }));
    assert_eq!(game.top().unwrap().mode, Some(2));
    assert!(game.top().unwrap().paid.is_none());
    game.concede(PlayerId(1)).unwrap();
    fund(&mut game, 4096, "Plains", 4);
    public_answer(
        &mut game,
        CASTER,
        PlayerResponse::FinishMana { undo_mana: false },
    );
    public_answer(
        &mut game,
        CASTER,
        PlayerResponse::SpendMana {
            spending: [(Color::W, 4.into())].into(),
        },
    );
    assert!(matches!(request(&mut game, 4096), Request::Priority { .. }));
    assert_eq!(
        game.top().unwrap().paid,
        Some(ManaCost {
            generic: 3.into(),
            colored: vec![Color::W]
        })
    );
    assert!(game.object_id(CardId(1)).is_none());
    resolve(&mut game, 4096, |_| {});
    for keyword in [
        KeywordAbility::Flying,
        KeywordAbility::Vigilance,
        KeywordAbility::DoubleStrike,
        KeywordAbility::Lifelink,
        KeywordAbility::Indestructible,
        KeywordAbility::ProtectionFromEachColor,
    ] {
        assert!(game.has_keyword(recipient, keyword));
    }
    assert!(game.events().iter().any(|event| matches!(
        event, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == bounce
    )));
    assert_eq!(game.zone(recipient), Some(Zone::Battlefield));
    assert_eq!(
        game.current(recipient).unwrap().marked_damage,
        Quantity::zero()
    );
    assert_eq!(
        game.zone(object(&game, 9)),
        Some(Zone::Graveyard { owner: PlayerId(2) })
    );
    equip(&mut game, 4096, 5, 2);
}
