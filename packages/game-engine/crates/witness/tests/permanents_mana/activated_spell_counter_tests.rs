//! Full Ertai source with real creature casting, spell targets and mana costs.
//! Private configurations, not pods; all stack/tap changes use accepted inputs.
//! Pinned CR113.7a,302.6,602.2b,608.2b,701.6a/b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(hand: bool) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/f2902dfd-888f-4b02-b8ad-1056f0059fc3.json"),
        include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete activated counter witness must lower");
        };
        if definition.name == "Ertai, Wizard Adept" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::ActivatedSpellCounterV1)
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "activated-spell-counter-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Ertai, Wizard Adept", 0, hand),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Island", 0, false),
        ("Sol Ring", 1, true),
        ("Island", 1, false),
        ("Regress", 2, true),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Counterspell", 3, true),
        ("Island", 3, false),
        ("Island", 3, false),
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
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game, q: u64) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!("activated counter {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("automatic work bound");
}
fn answer(g: &mut Game, q: u64, response: Response) {
    let r = request(g, q);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, q: u64, seat: u8) {
    for _ in 0..8 {
        let r = request(g, q);
        assert!(matches!(r, Request::Priority { .. }));
        if r.player() == PlayerId(seat) {
            return;
        }
        answer(g, q, Response::Pass);
    }
    panic!("priority bound");
}
fn pay(g: &mut Game, q: u64, lands: &[u32]) {
    for id in lands {
        let land = object(g, *id);
        answer(g, q, Response::ActivateMana { object: land });
    }
    answer(g, q, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, q), Request::SpendMana { .. }));
    answer(
        g,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(lands.len() as i64))]),
        },
    );
    assert!(matches!(request(g, q), Request::Priority { .. }));
}
fn finish_to(g: &mut Game, q: u64, len: usize) {
    for _ in 0..30 {
        assert!(matches!(request(g, q), Request::Priority { .. }));
        if g.state.rules.stack.len() == len {
            return;
        }
        answer(g, q, Response::Pass);
    }
    panic!("completion bound");
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
fn reject(g: &mut Game, r: &Request, player: PlayerId, response: Response) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(player, r.decision_id(), response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}
fn ring_spell(g: &mut Game, q: u64) -> StackId {
    // Ring is an artifact: reach its controller's main phase using real turns.
    for _ in 0..400 {
        let r = request(g, q);
        if matches!(r, Request::Priority { .. })
            && r.player() == PlayerId(1)
            && g.state.rules.active == Some(PlayerId(1))
            && g.state.rules.turn.step == TurnStep::PrecombatMain
            && g.state.rules.stack.is_empty()
        {
            break;
        }
        let response = match &r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates[..*count].to_vec(),
            },
            other => panic!("unexpected opponent-main setup {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert_eq!(g.state.rules.active, Some(PlayerId(1)));
    assert_eq!(g.state.rules.turn.step, TurnStep::PrecombatMain);
    answer(g, q, Response::Cast { card: CardId(8) });
    pay(g, q, &[9]);
    let target = g.top().unwrap().id;
    priority(g, q, 0);
    target
}
fn announce(g: &mut Game, q: u64, target: StackId) -> StackId {
    let source = object(g, 0);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let r = request(g, q);
    let own = g.top().unwrap().id;
    assert!(matches!(&r,Request::StackTargets { candidates,.. } if candidates==&vec![target]));
    for invalid in [own, StackId(999)] {
        reject(
            g,
            &r,
            r.player(),
            Response::StackTargets {
                entries: vec![invalid],
            },
        );
    }
    reject(
        g,
        &r,
        PlayerId(1),
        Response::StackTargets {
            entries: vec![target],
        },
    );
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::StackTargets {
        token,
        ..
    }))) = g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("public stack targets");
    };
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::StackTargets {
                entries: vec![StackHandle("foreign".into())]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
    recover(g);
    answer(
        g,
        q,
        Response::StackTargets {
            entries: vec![target],
        },
    );
    assert!(matches!(request(g, q), Request::Mana { .. }));
    recover(g);
    own
}
fn forged_bindings_reject(g: &Game) {
    for mutation in 0..3 {
        let mut s: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let entry = s.state.rules.stack.last_mut().unwrap();
        match mutation {
            0 => entry.stack_targets[0] = entry.id,
            1 => entry.targets.push(object(g, 1)),
            _ => entry.announced_stack_targets[0].clear(),
        };
        s.state_sha256 = digest(&s.state).unwrap();
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&s).unwrap()
            )
            .is_err()
        );
    }
}
fn fund_counter(g: &mut Game, q: u64) {
    for id in 1..=4 {
        let land = object(g, id);
        answer(g, q, Response::ActivateMana { object: land });
    }
    answer(g, q, Response::FinishMana { undo_mana: false });
    let r = request(g, q);
    assert!(
        matches!(&r,Request::SpendMana { cost,.. } if *cost==ManaCost { generic:2.into(),colored:vec![Color::U,Color::U] })
    );
    recover(g);
    answer(
        g,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 4.into())]),
        },
    );
    assert!(matches!(request(g, q), Request::Priority { .. }));
    let source = object(g, 0);
    assert!(g.current(source).unwrap().tapped);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 2.into(),
            colored: vec![Color::U, Color::U]
        })
    );
    assert_eq!(g.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    recover(g);
    let r = request(g, q);
    reject(
        g,
        &r,
        PlayerId(0),
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
}
fn resolution_start(g: &mut Game) {
    for _ in 0..30 {
        if matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.cursor==0) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) if matches!(r, Request::Priority { .. }) => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("counter resolution boundary {other:?}"),
        }
    }
    panic!("resolution boundary bound");
}
#[derive(Clone, Copy)]
enum Case {
    Counter,
    BounceSource,
    RemoveTarget,
}
fn run(case: Case, q: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(prepared(false), mode);
    let source = object(&g, 0);
    let r = request(&mut g, q);
    reject(
        &mut g,
        &r,
        PlayerId(0),
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let target = ring_spell(&mut g, q);
    let ability = announce(&mut g, q, target);
    forged_bindings_reject(&g);
    fund_counter(&mut g, q);
    match case {
        Case::Counter => (),
        Case::BounceSource => {
            priority(&mut g, q, 2);
            answer(&mut g, q, Response::Cast { card: CardId(10) });
            answer(
                &mut g,
                q,
                Response::Targets {
                    objects: vec![source],
                },
            );
            pay(&mut g, q, &[11, 12, 13]);
            finish_to(&mut g, q, 2);
            assert!(g.current(source).is_err());
            assert_eq!(
                g.current(object(&g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
        }
        Case::RemoveTarget => {
            priority(&mut g, q, 3);
            answer(&mut g, q, Response::Cast { card: CardId(14) });
            answer(
                &mut g,
                q,
                Response::StackTargets {
                    entries: vec![target],
                },
            );
            pay(&mut g, q, &[15, 16]);
            finish_to(&mut g, q, 1);
            assert_eq!(
                g.current(object(&g, 8)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(1) }
            );
        }
    }
    recover(&g);
    resolution_start(&mut g);
    let legal = !matches!(case, Case::RemoveTarget);
    assert!(
        matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.spell.id==ability && frame.legal_targets==[legal])
    );
    recover(&g);
    finish_to(&mut g, q, 0);
    assert_eq!(
        g.current(object(&g, 8)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    if !matches!(case, Case::BounceSource) {
        assert!(g.current(source).unwrap().tapped);
    }
    assert_eq!(g.mana(PlayerId(0), Color::U), Some(Quantity::zero()));
    if mode == TraceMode::Detailed {
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id==ability))
        );
        assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if *object==source)).count(),1);
        if legal {
            assert!(g.events().iter().any(|e|matches!(e,Event::Committed { actual:EventOutcome::StackObjectCountered { target:t,source:s,.. },.. } if *t==target && s.card==CardId(8))));
        }
    }
    recover(&g);
    g
}
fn quanta(case: Case) {
    let a = run(case, 1, TraceMode::Detailed);
    let b = run(case, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn activated_spell_counter_pays_four_and_tap_then_counters_real_spell() {
    quanta(Case::Counter);
}
#[test]
fn activated_spell_counter_paid_ability_survives_regress_source_bounce() {
    quanta(Case::BounceSource);
}
#[test]
fn activated_spell_counter_target_countered_first_fizzles_without_refunding_costs() {
    quanta(Case::RemoveTarget);
}
#[test]
fn activated_spell_counter_real_source_cast_is_sick_and_unpayable_attempt_reverses() {
    let mut fresh = Game::new(prepared(true));
    answer(&mut fresh, 1, Response::Cast { card: CardId(0) });
    pay(&mut fresh, 1, &[1, 2, 3]);
    assert_eq!(
        fresh.top().unwrap().paid,
        Some(ManaCost {
            generic: 2.into(),
            colored: vec![Color::U]
        })
    );
    finish_to(&mut fresh, 1, 0);
    let source = object(&fresh, 0);
    assert!(!fresh.current(source).unwrap().attack_ready);
    ring_spell(&mut fresh, 1);
    let r = request(&mut fresh, 1);
    let before = fresh.snapshot().unwrap();
    assert!(
        fresh
            .respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: source,
                    ability: 0
                }
            )
            .unwrap_err()
            .contains("continuously controlled")
    );
    assert_eq!(fresh.snapshot().unwrap(), before);
    recover(&fresh);
    let mut g = Game::new(prepared(false));
    let target = ring_spell(&mut g, 1);
    announce(&mut g, 1, target);
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    assert!(matches!(
        g.advance(Budget::new(1)),
        Advance::Suspended { .. }
    ));
    assert!(matches!(g.state.rules.frame, Frame::Payment { .. }));
    recover(&g);
    assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
    assert_eq!(g.state.rules.stack.len(), 1);
    assert_eq!(g.top().unwrap().id, target);
    assert!(!g.current(object(&g, 0)).unwrap().tapped);
    recover(&g);
}
