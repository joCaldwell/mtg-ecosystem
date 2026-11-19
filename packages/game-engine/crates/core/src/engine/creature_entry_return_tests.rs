//! CR 400.3/7, 603.3d/5, 608.2b and 903.9b.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn setup() -> Prepared {
    let mut rows = vec![
        ("Reclamation Sage", 0, 0, true),
        ("Isamaru, Hound of Konda", 2, 1, false),
        ("Grizzly Bears", 3, 0, false),
        ("Island", 1, 1, false),
    ];
    let mut prepared =
        if let Some(path) = std::env::var_os("MTG_CREATURE_ENTRY_RETURN_ARCHIVE_PROBE") {
            let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
            let row = archive
                .lines()
                .find(|line| {
                    mtg_contracts::sha256(line.as_bytes())
                        == "f14f7af722c4af9eb3764eed58cf36d1253488b6de36376874e319b016543506"
                })
                .expect("pinned complete Riddlemaster Sphinx row");
            rows[0].0 = "Riddlemaster Sphinx";
            lands(&mut rows, "Island", 0, 6);
            prepared_with_sources(&rows, &[row])
        } else {
            lands(&mut rows, "Forest", 0, 3);
            let mut prepared = prepared_with_sources(&rows, &[]);
            let catalog = Arc::make_mut(&mut prepared.catalog);
            let source = catalog
                .definitions
                .iter_mut()
                .find(|d| d.name == "Reclamation Sage")
                .unwrap();
            source.name = "Synthetic creature entry return".into();
            source.oracle_id = "synthetic-creature-entry-return".into();
            source.source_labels = None;
            let program = &mut source.triggered[0].program;
            let filter = program.targets[0].filter.as_mut().unwrap();
            filter.types = vec![CardKind::Creature];
            filter.opponent = true;
            program.instructions = vec![Instruction::ReturnEntryTargetCreature {
                target_slot: 0,
                optional: true,
                source_block: 0,
            }];
            source.capabilities = source.requirements();
            prepared.config.cards[0].oracle_id = source.oracle_id.clone();
            catalog
                .definitions
                .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
            catalog.validate().unwrap();
            prepared.catalog_digest = digest(catalog).unwrap();
            prepared
        };
    prepared.config.commanders = vec![CardId(1)];
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn creature_entry_return_paid_opponent_target_optional_owner_hand() {
    let prepared = setup();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    let target = object(&game, 1);
    announce(&mut game, 1, 0, vec![]);
    let creature = game.top().unwrap();
    assert_eq!(creature.paid.as_ref(), Some(&creature.total_cost));
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    let Request::Targets {
        player: PlayerId(0),
        candidates,
        ..
    } = request(&mut game, 1)
    else {
        panic!("trigger controller chooses at placement");
    };
    assert_eq!(candidates, [target]);
    answer(
        &mut game,
        1,
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let trigger = game.top().unwrap();
    assert!(matches!(trigger.kind, StackKind::Triggered { .. }));
    assert_eq!(trigger.source, object(&game, 0));
    assert_eq!(trigger.targets, [target]);
    for _ in 0..4 {
        answer(&mut game, 1, Response::Pass);
    }
    assert!(matches!(
        request(&mut game, 1),
        Request::OptionalEffect {
            player: PlayerId(0),
            ..
        }
    ));
    let mut departed = game.fork();
    departed.concede(PlayerId(2)).unwrap();
    assert!(matches!(
        request(&mut departed, 1),
        Request::Priority { .. }
    ));
    assert!(departed.state.rules.stack.is_empty());
    assert!(departed.current(target).is_err());

    answer(&mut game, 1, Response::OptionalEffect { perform: true });
    assert!(matches!(request(&mut game, 1), Request::CommanderReturn {
        player: PlayerId(2), object, destination: Zone::Hand { owner: PlayerId(2) }, ..
    } if object == target));
    let mut restored = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    answer(
        &mut restored,
        1,
        Response::CommanderReturn { to_command: false },
    );
    assert!(matches!(
        request(&mut restored, 1),
        Request::Priority { .. }
    ));
    assert!(restored.state.rules.stack.is_empty());
    assert!(restored.current(target).is_err());
    assert_eq!(
        restored.zone(object(&restored, 1)),
        Some(Zone::Hand { owner: PlayerId(2) })
    );
}
