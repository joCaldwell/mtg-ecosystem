//! CR 603.2c/603.3a/603.6a/603.10: each qualifying entrant freezes its
//! post-event appearance and the observer's controller before trigger placement.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn setup() -> Prepared {
    let mut rows = vec![
        ("Elvish Visionary", 0, 0, true),
        ("Elvish Visionary", 0, 1, false),
        ("Glorious Anthem", 0, 0, false),
        ("Glorious Anthem", 0, 0, false),
        ("Wall of Wood", 0, 0, true),
        ("Grizzly Bears", 0, 0, true),
        ("Spore Swarm", 0, 0, true),
        ("Naturalize", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 0, 14);
    if let Some(path) = std::env::var_os("MTG_POWER_ENTRY_DRAW_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let row = archive
            .lines()
            .find(|line| {
                mtg_contracts::sha256(line.as_bytes())
                    == "e137e566b5055f3fcdccbdef0a8ce692f43582b0ab1ae679178b624d4fe2cc14"
            })
            .expect("pinned complete Garruk's Packleader row");
        rows[0].0 = "Garruk's Packleader";
        rows[1].0 = "Garruk's Packleader";
        return prepared_with_sources(&rows, &[row]);
    }
    let mut p = prepared_with_sources(&rows, &[]);
    let catalog = Arc::make_mut(&mut p.catalog);
    let mut observer = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Elvish Visionary")
        .unwrap()
        .clone();
    observer.oracle_id = "synthetic-power-entry-observer".into();
    observer.name = "Synthetic power entry observer".into();
    observer.source_labels = None;
    observer.triggered[0].condition = TriggerCondition::ControlledCreatureEntersWithPower {
        minimum_power: 3.into(),
        other: true,
    };
    observer.triggered[0].program.instructions =
        vec![Instruction::OptionalDrawCards { source_block: 0 }];
    observer.capabilities = observer.requirements();
    for card in &mut p.config.cards[..2] {
        card.oracle_id = observer.oracle_id.clone();
    }
    catalog.definitions.push(observer);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}

fn resolve_top(g: &mut Game) -> Request {
    for _ in 0..4 {
        answer(g, 1, Response::Pass);
    }
    request(g, 1)
}

#[test]
fn power_entry_draw_freezes_each_controlled_entrant_before_optional_resolution() {
    let mut g = Game::with_trace(setup(), TraceMode::Off);
    // The opposing observer is owned by seat 0. Neither it nor the entering
    // observer triggers on this cast; the Wall subsequently enters at power 2.
    cast(&mut g, 1, 0, vec![]);
    cast(&mut g, 1, 4, vec![]);
    assert_eq!(g.state.rules.next_trigger_id, 0);

    announce(&mut g, 1, 5, vec![]);
    let previous = object(&g, 5);
    assert!(matches!(resolve_top(&mut g), Request::Priority { .. }));
    let entrant = object(&g, 5);
    let observer = object(&g, 0);
    let [trigger] = g.state.rules.stack.as_slice() else {
        panic!("one controlled entry")
    };
    assert_eq!(
        (trigger.source, trigger.controller),
        (observer, PlayerId(0))
    );
    assert!(matches!(&trigger.kind, StackKind::Triggered {
        witness: TriggerEventWitness::CreatureEnteredWithPower { entrant: fact, movement: Some(m), .. }, ..
    } if fact.object == entrant && fact.characteristics.power == Some(4.into())
        && m.previous == previous && m.successor == entrant && m.from == Zone::Stack));
    let mut forged = trigger.clone();
    let StackKind::Triggered {
        witness: TriggerEventWitness::CreatureEnteredWithPower { movement, .. },
        ..
    } = &mut forged.kind
    else {
        unreachable!()
    };
    *movement = None;
    assert!(
        g.validate_trigger_entry(&forged).is_err(),
        "a moved card cannot be a created token"
    );
    assert!(matches!(
        resolve_top(&mut g),
        Request::OptionalEffect {
            player: PlayerId(0),
            ..
        }
    ));
    let library = g.state.rules.libraries[&PlayerId(0)].len();
    answer(&mut g, 1, Response::OptionalEffect { perform: true });
    request(&mut g, 1);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library - 1);

    announce(&mut g, 1, 6, vec![]);
    let Request::OrderTriggers {
        player: PlayerId(0),
        candidates,
        ..
    } = resolve_top(&mut g)
    else {
        panic!("order one trigger per token")
    };
    assert_eq!(candidates.len(), 3);
    recover(&g);
    answer(&mut g, 1, Response::OrderTriggers { order: candidates });
    request(&mut g, 1);
    let mut tokens = BTreeSet::new();
    for trigger in &g.state.rules.stack {
        assert_eq!(
            (trigger.source, trigger.controller),
            (observer, PlayerId(0))
        );
        let StackKind::Triggered {
            witness:
                TriggerEventWitness::CreatureEnteredWithPower {
                    entrant,
                    movement: None,
                    ..
                },
            ..
        } = &trigger.kind
        else {
            panic!("actual token witness")
        };
        assert_eq!(entrant.characteristics.power, Some(3.into()));
        assert!(tokens.insert(entrant.object));
    }
    assert_eq!(tokens.len(), 3);
    let anthem = target(&g, 2);
    announce(&mut g, 1, 7, anthem);
    assert!(matches!(resolve_top(&mut g), Request::Priority { .. }));
    for token in tokens {
        assert_eq!(g.characteristics(token).unwrap().power, Some(2.into()));
    }
    recover(&g);
    for perform in [false, true, false] {
        assert!(matches!(
            resolve_top(&mut g),
            Request::OptionalEffect {
                player: PlayerId(0),
                ..
            }
        ));
        answer(&mut g, 1, Response::OptionalEffect { perform });
        request(&mut g, 1);
    }
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library - 2);
}

#[test]
fn greatest_power_paid_allocation_and_entrant_draw() {
    let rows = [
        ("Copperleaf Surveyor", 0, 0, false),
        ("Island", 0, 0, false),
        ("Forest", 0, 0, false),
        ("Grizzly Bears", 0, 0, true),
    ];
    let p = prepared_with_sources(
        &rows,
        &[include_str!(
            "../../../../../tests/fixtures/synthetic-copperleaf-surveyor.json"
        )],
    );
    let mut g = Game::with_trace(p, TraceMode::Off);
    announce(&mut g, 1, 3, vec![]);
    assert!(matches!(resolve_top(&mut g), Request::Priority { .. }));
    assert!(matches!(
        resolve_top(&mut g),
        Request::OptionalEffect {
            player: PlayerId(0),
            ..
        }
    ));
    recover(&g);
    let mut forged = g.state.rules.frame.clone();
    let Frame::ResolutionChoice {
        frame,
        choice: actions::ResolutionChoice::OptionalDraw { player, .. },
    } = &mut forged
    else {
        panic!("expected optional draw")
    };
    *player = PlayerId(1);
    frame.entrant_draw.as_mut().unwrap().controller = *player;
    assert!(g.validate_resolving(frame).is_err());
    let before = g.state.rules.libraries[&PlayerId(0)].len();
    answer(&mut g, 1, Response::OptionalEffect { perform: true });
    request(&mut g, 1);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), before - 1);
    let turn = g.turn().number;
    own_main(&mut g, 1, 0, turn);
    let source = object(&g, 0);
    let island = object(&g, 1);
    answer(&mut g, 1, Response::ActivateMana { object: source });
    answer(&mut g, 1, Response::ActivateMana { object: island });
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    let r = request(&mut g, 1);
    assert!(matches!(&r,Request::ManaCombination{total,..} if *total==2.into()));
    recover(&g);
    let mut forged = g.state.rules.frame.clone();
    let Frame::ManaAbility { activation } = &mut forged else {
        panic!("expected paid mana frame")
    };
    activation.stage = mana_permanents::ManaStage::Combination { total: 3.into() };
    assert!(g.validate_nested_mana_frame(&forged).is_err());
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::ManaCombination {
                amounts: [0_i64, 0, 0, 0, 1].map(Quantity::from)
            }
        )
        .is_err()
    );
    answer(
        &mut g,
        1,
        Response::ManaCombination {
            amounts: [1_i64, 0, 0, 0, 1].map(Quantity::from),
        },
    );
    request(&mut g, 1);
    assert!(g.current(source).unwrap().tapped && g.current(island).unwrap().tapped);
    assert_eq!(g.mana(PlayerId(0), Color::W), Some(1.into()));
    assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
    recover(&g);
}
