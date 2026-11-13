//! Actual paid spells in a private source configuration, not legal-pod proof.
//! Accepted concessions bracket Swords' exile instruction; no runtime mutation.
//! Pinned CR104.3a,608.2c/h,611.3a/b,800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared() -> Prepared {
    let catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-sacrifice-ir.json"
    ))
    .unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "removal-information-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Grizzly Bears", 0, false),
        ("Rancor", 2, true),
        ("Forest", 2, false),
        ("Swords to Plowshares", 1, true),
        ("Plains", 1, false),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (name, seat, hand))| CardSetup {
        id: CardId(id as u32),
        oracle_id: catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone(),
        owner: PlayerId(seat),
        controller: PlayerId(seat),
        zone: if hand {
            Zone::Hand {
                owner: PlayerId(seat),
            }
        } else {
            Zone::Battlefield
        },
    })
    .collect();
    for seat in 0..4 {
        for _ in 0..4 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("removal information {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("removal information work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn passive(g: &mut Game, quantum: u64) {
    let r = request(g, quantum);
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        other => panic!("unexpected passive request {other:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, seat: u8, main: bool) {
    for _ in 0..200 {
        let r = request(g, quantum);
        if matches!(r, Request::Priority { .. })
            && r.player() == PlayerId(seat)
            && (!main
                || (g.active_player() == Some(PlayerId(seat))
                    && g.turn().step == TurnStep::PrecombatMain))
        {
            return;
        }
        passive(g, quantum);
    }
    panic!("priority work bound");
}
fn cast(
    g: &mut Game,
    quantum: u64,
    card: u32,
    land: u32,
    color: Color,
    target: ObjectId,
) -> StackId {
    answer(g, quantum, Response::Cast { card: CardId(card) });
    assert!(matches!(request(g, quantum), Request::Targets { .. }));
    answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    let land = object(g, land);
    assert!(matches!(request(g, quantum), Request::Mana { .. }));
    answer(g, quantum, Response::ActivateMana { object: land });
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
    answer(
        g,
        quantum,
        Response::SpendMana {
            spending: BTreeMap::from([(color, 1.into())]),
        },
    );
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    assert!(g.current(land).unwrap().tapped);
    g.top().unwrap().id
}
fn settle_spell(g: &mut Game, quantum: u64) {
    for _ in 0..20 {
        assert!(matches!(request(g, quantum), Request::Priority { .. }));
        if g.state.rules.stack.is_empty() {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("spell resolution bound");
}
fn cursor(g: &mut Game, expected: u32) {
    for _ in 0..40 {
        if matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == expected) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) if matches!(r, Request::Priority { .. }) => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("cursor{expected}: {other:?}"),
        }
    }
    panic!("instruction boundary work bound");
}
fn recover(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replay.state, g.state);
    assert_eq!(replay.trace(), g.trace());
}
#[derive(Clone, Copy)]
enum Departure {
    None,
    BeforeExile,
    AfterExile,
}
fn run(departure: Departure, quantum: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(prepared(), mode);
    let bears = object(&g, 0);
    priority(&mut g, quantum, 2, true);
    cast(&mut g, quantum, 1, 2, Color::G, bears);
    settle_spell(&mut g, quantum);
    assert_eq!(g.current(object(&g, 1)).unwrap().attached_to, Some(bears));
    assert_eq!(g.characteristics(bears).unwrap().power, Some(4.into()));
    priority(&mut g, quantum, 1, false);
    let swords = cast(&mut g, quantum, 3, 4, Color::W, bears);
    cursor(&mut g, 0);
    assert_eq!(g.current(bears).unwrap().zone, Zone::Battlefield);
    recover(&g);
    match departure {
        Departure::BeforeExile => {
            g.concede(PlayerId(2)).unwrap();
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
            assert!(matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 0));
            assert_eq!(g.characteristics(bears).unwrap().power, Some(2.into()));
            recover(&g);
        }
        Departure::AfterExile => {
            cursor(&mut g, 1);
            assert_eq!(g.current(object(&g, 0)).unwrap().zone, Zone::Exile);
            g.concede(PlayerId(2)).unwrap();
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
            assert!(matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 1));
            recover(&g);
        }
        Departure::None => (),
    }
    assert!(matches!(request(&mut g, quantum), Request::Priority { .. }));
    // In the uninterrupted control, Rancor's own graveyard-return trigger
    // may already be on the stack. This witness stops after Swords completes.
    assert!(!g.state.rules.stack.iter().any(|entry| entry.id == swords));
    assert_eq!(g.current(object(&g, 0)).unwrap().zone, Zone::Exile);
    let expected_life = if matches!(departure, Departure::BeforeExile) {
        42
    } else {
        44
    };
    assert_eq!(g.state.rules.players[0].life, expected_life.into());
    assert_eq!(
        g.current(object(&g, 3)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    if mode == TraceMode::Detailed {
        assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: true } if *stack_id == swords)));
    }
    recover(&g);
    g
}
fn quanta(departure: Departure) {
    let a = run(departure, 1, TraceMode::Detailed);
    let b = run(departure, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn removal_information_aura_owner_departure_before_exile_uses_last_battlefield_power() {
    quanta(Departure::BeforeExile);
}
#[test]
fn removal_information_aura_owner_departure_after_exile_preserves_target_lki() {
    quanta(Departure::AfterExile);
}
#[test]
fn removal_information_uninterrupted_swords_uses_enchanted_power() {
    quanta(Departure::None);
}
