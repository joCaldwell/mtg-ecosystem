//! Paid, bounded stack-response witness. The archive probe pins physical
//! row 29840 of d705222b280a71cb, or an unchanged extraction of that row.
use super::*;
use std::io::{BufRead, BufReader};

const PAYER: PlayerId = PlayerId(0);
const CASTER: PlayerId = PlayerId(1);

fn scenario() -> Prepared {
    let mut rows = vec![
        ("Counterspell", 1, 1, true),
        ("Sol Ring", 0, 0, true),
        ("Golgari Signet", 0, 0, false),
        ("Birds of Paradise", 0, 0, false),
        ("Volcanic Fallout", 0, 0, true),
    ];
    lands(&mut rows, "Mountain", 0, 3);
    lands(&mut rows, "Island", 1, 2);
    if let Some(path) = std::env::var_os("MTG_COUNTER_UNLESS_GENERIC_ARCHIVE_PROBE") {
        let file = std::fs::File::open(path).expect("open Mana Leak archive probe");
        let row = BufReader::new(file)
            .lines()
            .map(|l| l.expect("read archive record"))
            .find(|l| {
                l.contains("c61fe162-2202-4e56-9ba0-393547f9875f")
                    && sha256(l.as_bytes())
                        == "0c374ab42c858b02e9ffc9dce9bc025c88eb86459eb9287de4a1f0692eba0ae0"
            })
            .expect("exact Mana Leak archive record");
        rows[0].0 = "Mana Leak";
        return prepared_with_sources(&rows, &[&row]);
    }
    let mut pod = prepared(&rows);
    let catalog = Arc::make_mut(&mut pod.catalog);
    let spell = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Counterspell")
        .unwrap();
    spell.name = "Synthetic conditional counter".into();
    spell.oracle_id = "synthetic-conditional-counter".into();
    spell.source_labels = None;
    spell.characteristics.mana_cost = ManaCost {
        generic: 1.into(),
        colored: vec![Color::U],
    };
    spell.spell.as_mut().unwrap().instructions = vec![Instruction::CounterSpellUnlessPays {
        target_slot: 0,
        generic: 3.into(),
        source_block: 0,
    }];
    spell.capabilities = spell.requirements();
    pod.config.cards[0].oracle_id = spell.oracle_id.clone();
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

fn cast_response(g: &mut Game, card: u32) -> (StackInstance, StackInstance) {
    announce(g, 1, card, vec![]);
    let target = g.top().unwrap().clone();
    until_player(g, 1, CASTER);
    answer(g, 1, Response::Cast { card: CardId(0) });
    answer(
        g,
        1,
        Response::StackTargets {
            entries: vec![target.id],
        },
    );
    fund(g, 1, "Island", 2);
    answer(g, 1, Response::FinishMana { undo_mana: false });
    answer(
        g,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 2.into())]),
        },
    );
    assert!(matches!(request(g, 1), Request::Priority { .. }));
    let spell = g.top().unwrap().clone();
    assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
    assert_eq!(target.paid.as_ref(), Some(&target.total_cost));
    (spell, target)
}

fn offer(g: &mut Game) {
    for _ in 0..4 {
        answer(g, 1, Response::Pass);
    }
    assert!(matches!(
        request(g, 1),
        Request::OptionalEffect { player: PAYER, .. }
    ));
    assert!(g.request_header(CASTER).unwrap().is_none());
}

fn submit_payment(g: &mut Game, response: PlayerResponse) {
    assert_eq!(request(g, 1).player(), PAYER);
    let token = g.request_header(PAYER).unwrap().unwrap().token().clone();
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        g.check_player_response(PAYER, token, response, Budget::unlimited())
    else {
        panic!("expected an authenticated payment response");
    };
    g.submit(receipt).unwrap();
}

fn activate(g: &mut Game, card: u32) {
    let handle = g.handle_for(PAYER, object(g, card)).unwrap();
    submit_payment(g, PlayerResponse::ActivateMana { object: handle });
}

fn completed(g: &mut Game, spell: &StackInstance) {
    assert!(matches!(request(g, 1), Request::Priority { .. }));
    assert!(g.events().iter().any(
        |e| matches!(e, Event::SpellCompleted { stack_id, resolved: true } if *stack_id == spell.id)
    ));
}

#[test]
fn counter_unless_generic_paid_resolution() {
    let pod = scenario();
    let mut g = Game::with_trace(pod.clone(), TraceMode::Detailed);
    let (spell, target) = cast_response(&mut g, 1);
    let cause = BatchCause::Instruction {
        stack_id: spell.id,
        player: CASTER,
        source: spell.source,
        cursor: 0,
        source_block: 0,
    };
    // Rule-step probe: normal fixture casts come from their owner's hand.
    // Change only the target spell controller, before resolution freezes it.
    let mut borrowed = g.fork();
    borrowed
        .state
        .rules
        .stack
        .iter_mut()
        .find(|s| s.id == target.id)
        .unwrap()
        .controller = PlayerId(2);
    for _ in 0..4 {
        answer(&mut borrowed, 1, Response::Pass);
    }
    assert!(matches!(
        request(&mut borrowed, 1),
        Request::OptionalEffect {
            player: PlayerId(2),
            ..
        }
    ));
    assert!(borrowed.request_header(PAYER).unwrap().is_none());

    let mut illegal = g.fork();
    illegal.concede(PAYER).unwrap();
    resolve(&mut illegal, 1, |_| {});
    assert!(illegal.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == spell.id)));
    assert!(batches(&illegal).iter().all(|b| b.cause != cause));

    offer(&mut g);
    let Frame::ResolutionPayment { frame, payment } = &g.state.rules.frame else {
        panic!("expected retained counter payment offer");
    };
    let mut forged = payment.clone();
    forged.payer = PlayerId(2);
    assert!(g.validate_resolution_payment(frame, &forged).is_err());
    let mut detached = g.fork();
    detached.concede(CASTER).unwrap();
    assert!(matches!(
        request(&mut detached, 1),
        Request::OptionalEffect { player: PAYER, .. }
    ));
    submit_payment(
        &mut detached,
        PlayerResponse::OptionalEffect { perform: false },
    );
    completed(&mut detached, &spell);
    assert!(detached.state.rules.stack.is_empty());

    let mut decline = g.fork();
    submit_payment(
        &mut decline,
        PlayerResponse::OptionalEffect { perform: false },
    );
    completed(&mut decline, &spell);
    assert!(batches(&decline).iter().any(|b| b.cause == cause && matches!(b.result(&[]).unwrap(), EventOutcome::StackObjectCountered { target: id, .. } if *id == target.id)));

    submit_payment(&mut g, PlayerResponse::OptionalEffect { perform: true });
    activate(&mut g, 2); // Signet asks for {1}; Birds produces the chosen W.
    activate(&mut g, 3);
    submit_payment(&mut g, PlayerResponse::ManaColor { color: Color::W });
    submit_payment(&mut g, PlayerResponse::FinishMana { undo_mana: false });
    submit_payment(
        &mut g,
        PlayerResponse::SpendMana {
            spending: BTreeMap::from([(Color::W, 1.into())]),
        },
    );
    assert!(
        matches!(request(&mut g, 1), Request::Mana { player: PAYER, cost, .. } if cost.generic == 3.into())
    );

    let mut failed = g.fork();
    submit_payment(&mut failed, PlayerResponse::FinishMana { undo_mana: true });
    completed(&mut failed, &spell);
    assert!(!failed.current(object(&failed, 2)).unwrap().tapped);
    assert!(!failed.current(object(&failed, 3)).unwrap().tapped);
    assert!(batches(&failed).iter().any(|b| b.cause == cause && matches!(b.result(&[]).unwrap(), EventOutcome::StackObjectCountered { target: id, .. } if *id == target.id)));

    fund(&mut g, 1, "Mountain", 2);
    submit_payment(&mut g, PlayerResponse::FinishMana { undo_mana: false });
    assert!(
        matches!(request(&mut g, 1), Request::SpendMana { player: PAYER, cost, .. } if cost.generic == 3.into() && cost.colored.is_empty())
    );
    recover(&g);
    let mut gone = g.fork();
    gone.concede(PAYER).unwrap();
    completed(&mut gone, &spell);
    assert!(batches(&gone).iter().all(|b| b.cause != cause));

    let token = g.request_header(PAYER).unwrap().unwrap().token().clone();
    assert!(matches!(
        g.check_player_response(
            PAYER,
            token,
            PlayerResponse::SpendMana {
                spending: BTreeMap::from([(Color::G, 1.into())])
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(_)
    ));
    let chosen = BTreeMap::from([(Color::G, 1.into()), (Color::R, 2.into())]);
    submit_payment(
        &mut g,
        PlayerResponse::SpendMana {
            spending: chosen.clone(),
        },
    );
    completed(&mut g, &spell);
    assert_eq!(g.top().unwrap(), &target);
    assert_eq!(
        g.state.rules.players[0].mana.get(&Color::B),
        Some(&Quantity::from(1))
    );
    let effects: Vec<_> = batches(&g)
        .into_iter()
        .filter(|b| b.cause == cause)
        .map(|b| b.result(&[]).unwrap())
        .collect();
    assert!(
        matches!(effects.as_slice(), [EventOutcome::ManaPaidSelected { player: PAYER, spending, .. }] if spending == &chosen)
    );

    let mut protected = Game::with_trace(pod, TraceMode::Detailed);
    let (spell, target) = cast_response(&mut protected, 4);
    offer(&mut protected);
    submit_payment(
        &mut protected,
        PlayerResponse::OptionalEffect { perform: false },
    );
    completed(&mut protected, &spell);
    assert_eq!(protected.top().unwrap(), &target);
}
