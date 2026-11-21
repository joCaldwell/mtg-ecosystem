//! Full Temporal Adept source in private configurations, not admitted pods.
//! Creature casting, mana, readiness, responses and movement use accepted input.
//! Pinned CR113.7a,302.6,400.3/7,602.2b,608.2b,903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared(hand: bool, target: &str, commander: bool) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for (header, raw) in [
        (
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!("../../../../tests/fixtures/fd9b5462-6578-4bf6-8026-23eaf5af3eee.json"),
        ),
        (
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
        ),
        (
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"
            ),
            include_str!(
                "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
            ),
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete activated permanent-return source must lower");
        };
        if definition.name == "Temporal Adept" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::ActivatedPermanentOwnerHandV1)
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "activated-permanent-return-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Temporal Adept", 0, 0, hand),
        ("Island", 0, 0, false),
        ("Island", 0, 0, false),
        ("Island", 0, 0, false),
        ("Island", 0, 0, false),
        ("Island", 0, 0, false),
        ("Island", 0, 0, false),
        (target, 2, if target == "Sol Ring" { 1 } else { 2 }, false),
        ("Regress", 1, 1, true),
        ("Island", 1, 1, false),
        ("Island", 1, 1, false),
        ("Island", 1, 1, false),
        ("Island", 2, 2, false),
        ("Island", 2, 2, false),
        ("Island", 2, 2, false),
        ("Island", 2, 2, false),
        ("Island", 2, 2, false),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (name, owner, controller, hand))| CardSetup {
        id: CardId(id as u32),
        oracle_id: catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone(),
        owner: PlayerId(owner),
        controller: PlayerId(controller),
        zone: if hand {
            Zone::Hand {
                owner: PlayerId(owner),
            }
        } else {
            Zone::Battlefield
        },
    })
    .collect();
    if commander {
        config.commanders = vec![CardId(7)];
    }
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
            other => panic!("permanent return {:?}: {other:?}", g.state.rules.frame),
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
fn passive(g: &mut Game, q: u64) {
    let r = request(g, q);
    let response = match &r {
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates[..*count].to_vec(),
        },
        other => panic!("aging request {other:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
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
fn finish_to(g: &mut Game, q: u64, len: usize, choice: Option<bool>) {
    for _ in 0..30 {
        let r = request(g, q);
        if g.state.rules.stack.len() == len {
            return;
        }
        match r {
            Request::Priority { .. } => answer(g, q, Response::Pass),
            Request::CommanderReturn { .. } => answer(
                g,
                q,
                Response::CommanderReturn {
                    to_command: choice.expect("explicit Commander replacement"),
                },
            ),
            other => panic!("return continuation {other:?}"),
        }
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
fn reject(g: &mut Game, r: &Request, response: Response) {
    let before = g.snapshot().unwrap();
    assert!(g.respond(r.player(), r.decision_id(), response).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}
fn announce(g: &mut Game, q: u64, target: ObjectId) {
    let source = object(g, 0);
    priority(g, q, 0);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    let r = request(g, q);
    assert!(matches!(&r,Request::Targets{candidates,..}if candidates.contains(&target)));
    let hand = object(g, 8);
    reject(
        g,
        &r,
        Response::Targets {
            objects: vec![hand],
        },
    );
    reject(
        g,
        &r,
        Response::Targets {
            objects: vec![ObjectId {
                card: CardId(999),
                incarnation: 0,
            }],
        },
    );
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets { token, .. }))) =
        g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("public return targets");
    };
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("unknown".into())]
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
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(request(g, q), Request::Mana { .. }));
    recover(g);
}
fn activate(g: &mut Game, q: u64, target: ObjectId) -> StackId {
    announce(g, q, target);
    pay(g, q, &[1, 2, 3]);
    let source = object(g, 0);
    assert!(g.current(source).unwrap().tapped);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![Color::U, Color::U, Color::U]
        })
    );
    let ability = g.top().unwrap().id;
    let r = request(g, q);
    reject(
        g,
        &r,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    recover(g);
    ability
}
fn tap_once(g: &Game, source: ObjectId, ability: StackId) {
    if g.trace.mode == TraceMode::Detailed {
        assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed{actual:EventOutcome::Tapped{object},..}if*object==source)).count(),1);
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted{stack_id}if*stack_id==ability))
        );
    }
}
fn resolve_start(g: &mut Game) {
    for _ in 0..30 {
        if matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 0) {
            return;
        }
        match g.advance(Budget::new(1)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) if matches!(r, Request::Priority { .. }) => {
                g.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("return resolution boundary {other:?}"),
        }
    }
    panic!("return resolution boundary bound");
}

#[test]
fn activated_permanent_return_casts_ages_and_pays_three_blue_to_return_true_owners_artifact() {
    let p = prepared(true, "Sol Ring", false);
    let mut baseline = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        answer(&mut g, q, Response::Cast { card: CardId(0) });
        pay(&mut g, q, &[1, 2, 3]);
        assert_eq!(
            g.top().unwrap().paid,
            Some(ManaCost {
                generic: 1.into(),
                colored: vec![Color::U, Color::U]
            })
        );
        finish_to(&mut g, q, 0, None);
        let source = object(&g, 0);
        assert!(!g.current(source).unwrap().attack_ready);
        for id in 4..=6 {
            assert!(!g.current(object(&g, id)).unwrap().tapped);
        }
        let r = request(&mut g, q);
        let before = g.snapshot().unwrap();
        assert!(
            g.respond(
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
        assert_eq!(g.snapshot().unwrap(), before);
        recover(&g);
        let turn = g.turn().number;
        for _ in 0..300 {
            let r = request(&mut g, q);
            if g.turn().number > turn
                && g.active_player() == Some(PlayerId(0))
                && g.turn().step == TurnStep::PrecombatMain
                && matches!(r, Request::Priority { .. })
            {
                break;
            }
            passive(&mut g, q);
        }
        assert!(g.turn().number > turn && g.current(source).unwrap().attack_ready);
        let target = object(&g, 7);
        assert_eq!(g.current(target).unwrap().controller, PlayerId(1));
        announce(&mut g, q, target);
        answer(&mut g, q, Response::FinishMana { undo_mana: false });
        assert!(matches!(
            g.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
        assert!(matches!(g.state.rules.frame, Frame::Payment { .. }));
        recover(&g);
        assert!(matches!(request(&mut g, q), Request::Priority { .. }));
        assert!(g.state.rules.stack.is_empty());
        assert!(!g.current(source).unwrap().tapped);
        assert_eq!(g.current(target).unwrap().zone, Zone::Battlefield);
        recover(&g);
        let ability = activate(&mut g, q, target);
        finish_to(&mut g, q, 0, None);
        assert!(g.current(target).is_err());
        assert_eq!(
            g.current(object(&g, 7)).unwrap().zone,
            Zone::Hand { owner: PlayerId(2) }
        );
        assert_eq!(g.current(object(&g, 7)).unwrap().controller, PlayerId(2));
        tap_once(&g, source, ability);
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}
#[test]
fn activated_permanent_return_self_bounce_pays_tap_and_returns_new_incarnation() {
    let p = prepared(false, "Sol Ring", false);
    let mut baseline = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        let source = object(&g, 0);
        let ability = activate(&mut g, q, source);
        finish_to(&mut g, q, 0, None);
        assert!(g.current(source).is_err());
        assert_eq!(
            g.current(object(&g, 0)).unwrap().zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        tap_once(&g, source, ability);
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}
#[test]
fn activated_permanent_return_paid_ability_survives_real_regress_source_bounce() {
    let p = prepared(false, "Sol Ring", false);
    let mut baseline = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        let source = object(&g, 0);
        let target = object(&g, 7);
        let ability = activate(&mut g, q, target);
        priority(&mut g, q, 1);
        answer(&mut g, q, Response::Cast { card: CardId(8) });
        answer(
            &mut g,
            q,
            Response::Targets {
                objects: vec![source],
            },
        );
        pay(&mut g, q, &[9, 10, 11]);
        finish_to(&mut g, q, 1, None);
        assert!(g.current(source).is_err());
        assert_eq!(
            g.current(object(&g, 0)).unwrap().zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        recover(&g);
        finish_to(&mut g, q, 0, None);
        assert_eq!(
            g.current(object(&g, 7)).unwrap().zone,
            Zone::Hand { owner: PlayerId(2) }
        );
        tap_once(&g, source, ability);
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}
#[test]
fn activated_permanent_return_target_bounce_and_genuine_flash_reentry_never_rebind() {
    for reenter in [false, true] {
        let p = prepared(false, "Research Thief", false);
        let mut baseline = None;
        for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
            let mut g = Game::with_trace(p.clone(), mode);
            let source = object(&g, 0);
            let target = object(&g, 7);
            let ability = activate(&mut g, q, target);
            priority(&mut g, q, 1);
            answer(&mut g, q, Response::Cast { card: CardId(8) });
            answer(
                &mut g,
                q,
                Response::Targets {
                    objects: vec![target],
                },
            );
            pay(&mut g, q, &[9, 10, 11]);
            finish_to(&mut g, q, 1, None);
            assert!(g.current(target).is_err());
            assert_eq!(
                g.current(object(&g, 7)).unwrap().zone,
                Zone::Hand { owner: PlayerId(2) }
            );
            if reenter {
                priority(&mut g, q, 2);
                answer(&mut g, q, Response::Cast { card: CardId(7) });
                pay(&mut g, q, &[12, 13, 14, 15, 16]);
                finish_to(&mut g, q, 1, None);
                assert_eq!(g.current(object(&g, 7)).unwrap().zone, Zone::Battlefield);
                assert_ne!(object(&g, 7), target);
            }
            let successor = object(&g, 7);
            recover(&g);
            resolve_start(&mut g);
            assert!(
                matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.spell.id == ability && frame.legal_targets == [false])
            );
            recover(&g);
            finish_to(&mut g, q, 0, None);
            assert_eq!(object(&g, 7), successor);
            assert_eq!(
                g.current(object(&g, 7)).unwrap().zone,
                if reenter {
                    Zone::Battlefield
                } else {
                    Zone::Hand { owner: PlayerId(2) }
                }
            );
            assert!(g.current(source).unwrap().tapped);
            tap_once(&g, source, ability);
            recover(&g);
            if let Some(old) = &baseline {
                assert_eq!(old, &g.state);
            } else {
                baseline = Some(g.state.clone());
            }
        }
    }
}
#[test]
fn activated_permanent_return_commander_owner_chooses_hand_or_command() {
    let p = prepared(false, "Isamaru, Hound of Konda", true);
    for to_command in [false, true] {
        let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
        let target = object(&g, 7);
        activate(&mut g, 1, target);
        for _ in 0..8 {
            let r = request(&mut g, 1);
            if matches!(r, Request::CommanderReturn { .. }) {
                assert_eq!(r.player(), PlayerId(2));
                break;
            }
            assert!(matches!(r, Request::Priority { .. }));
            answer(&mut g, 1, Response::Pass);
        }
        assert!(matches!(
            request(&mut g, 1),
            Request::CommanderReturn { .. }
        ));
        recover(&g);
        finish_to(&mut g, 1, 0, Some(to_command));
        assert_eq!(
            g.current(object(&g, 7)).unwrap().zone,
            if to_command {
                Zone::Command { owner: PlayerId(2) }
            } else {
                Zone::Hand { owner: PlayerId(2) }
            }
        );
        recover(&g);
    }
}
