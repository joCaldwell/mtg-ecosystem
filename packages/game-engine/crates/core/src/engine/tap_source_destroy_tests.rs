//! Full Royal Assassin source, actual creature casting and accepted responses.
//! Private configuration, not pod admission; no tap/readiness state injection.
//! Pinned CR113.7a,302.6,602.2b,608.2b,701.26a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::profile_data::{Feature, ProfileError};

fn prepared() -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/9ed6f28f-a3db-48c5-9ab0-b90a7fba5f57.json"),
        include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
        include_str!("../../../../tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete tap-source destruction witness must lower");
        };
        if definition.name == "Royal Assassin" {
            assert!(
                definition
                    .capabilities
                    .contains(&Capability::TapSourceDestroyV1)
            );
            assert_eq!(definition.activated[0].cost, ActivationCost::TapSource);
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "tap-source-destroy-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Royal Assassin", 0, true),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
        ("Swamp", 0, false),
        ("Llanowar Elves", 1, false),
        ("Grizzly Bears", 1, false),
        ("Island", 1, false),
        ("Burst of Energy", 1, true),
        ("Plains", 1, false),
        ("Regress", 2, true),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Island", 2, false),
        ("Llanowar Elves", 1, true),
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
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!(
                "tap-source destruction {:?}: {other:?}",
                g.state.rules.frame
            ),
        }
    }
    panic!("tap-source destruction work bound");
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
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates[..*count].to_vec(),
        },
        other => panic!("unexpected aging request {other:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn priority(g: &mut Game, quantum: u64, seat: u8) {
    for _ in 0..8 {
        let r = request(g, quantum);
        assert!(matches!(r, Request::Priority { .. }));
        if r.player() == PlayerId(seat) {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("priority bound");
}
fn pay_spell(g: &mut Game, quantum: u64, lands: &[u32], color: Color) {
    for id in lands {
        let land = object(g, *id);
        answer(g, quantum, Response::ActivateMana { object: land });
    }
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
    answer(
        g,
        quantum,
        Response::SpendMana {
            spending: BTreeMap::from([(color, Quantity::from(lands.len() as i64))]),
        },
    );
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
}
fn finish_to(g: &mut Game, quantum: u64, remaining: usize) {
    for _ in 0..30 {
        assert!(matches!(request(g, quantum), Request::Priority { .. }));
        if g.state.rules.stack.len() == remaining {
            return;
        }
        answer(g, quantum, Response::Pass);
    }
    panic!("spell completion bound");
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
fn tap_elf(g: &mut Game, quantum: u64) {
    priority(g, quantum, 1);
    let elf = object(g, 4);
    assert!(!g.current(elf).unwrap().tapped);
    answer(g, quantum, Response::ActivateMana { object: elf });
    request(g, quantum);
    assert!(g.current(elf).unwrap().tapped);
    priority(g, quantum, 0);
}
fn cast_and_age(g: &mut Game, quantum: u64) -> ObjectId {
    answer(g, quantum, Response::Cast { card: CardId(0) });
    pay_spell(g, quantum, &[1, 2, 3], Color::B);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 1.into(),
            colored: vec![Color::B, Color::B]
        })
    );
    finish_to(g, quantum, 0);
    let royal = object(g, 0);
    assert!(!g.current(royal).unwrap().attack_ready);
    tap_elf(g, quantum);
    let r = request(g, quantum);
    let before = g.snapshot().unwrap();
    let error = g
        .respond(
            r.player(),
            r.decision_id(),
            Response::ActivateAbility {
                object: royal,
                ability: 0,
            },
        )
        .unwrap_err();
    assert!(error.contains("continuously controlled"));
    assert_eq!(g.snapshot().unwrap(), before);
    recover(g);
    let turn = g.turn().number;
    for _ in 0..300 {
        let r = request(g, quantum);
        if g.turn().number > turn
            && g.active_player() == Some(PlayerId(0))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { .. })
        {
            break;
        }
        passive(g, quantum);
    }
    assert!(g.turn().number > turn);
    assert!(g.current(royal).unwrap().attack_ready);
    tap_elf(g, quantum);
    royal
}
fn activate(g: &mut Game, quantum: u64, royal: ObjectId, target: ObjectId) -> StackId {
    answer(
        g,
        quantum,
        Response::ActivateAbility {
            object: royal,
            ability: 0,
        },
    );
    let r = request(g, quantum);
    assert!(
        matches!(&r,Request::Targets { candidates,.. } if candidates.contains(&target) && !candidates.contains(&object(g,5)) && !candidates.contains(&object(g,6)) && !candidates.contains(&object(g,13)))
    );
    for id in [5, 6, 13] {
        let invalid = object(g, id);
        reject(
            g,
            &r,
            r.player(),
            Response::Targets {
                objects: vec![invalid],
            },
        );
    }
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets { token, .. }))) =
        g.player_decision_for(PlayerId(0), Budget::unlimited())
    else {
        panic!("public target decision");
    };
    let before = g.snapshot().unwrap();
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token,
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("foreign".into())]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert_eq!(g.snapshot().unwrap(), before);
    answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    // The generic object-target announcement currently uses a zero-mana
    // continuation. If it is completed directly, the same paid tap is required.
    if matches!(request(g, quantum), Request::Mana { .. }) {
        answer(g, quantum, Response::FinishMana { undo_mana: false });
        assert!(matches!(request(g, quantum), Request::SpendMana { .. }));
        answer(
            g,
            quantum,
            Response::SpendMana {
                spending: BTreeMap::new(),
            },
        );
    }
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    assert!(
        g.current(royal).unwrap().tapped,
        "the printed tap cost must actually be paid"
    );
    let stack = g.top().unwrap().id;
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![]
        })
    );
    let r = request(g, quantum);
    reject(
        g,
        &r,
        PlayerId(0),
        Response::ActivateAbility {
            object: royal,
            ability: 0,
        },
    );
    recover(g);
    stack
}
fn resolve_start(g: &mut Game) {
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
            other => panic!("resolution boundary {other:?}"),
        }
    }
    panic!("resolution boundary bound");
}
#[derive(Clone, Copy)]
enum ResponseCase {
    None,
    UntapTarget,
    BounceSource,
}
fn run(case: ResponseCase, quantum: u64, mode: TraceMode) -> Game {
    let mut g = Game::with_trace(prepared(), mode);
    let royal = cast_and_age(&mut g, quantum);
    let target = object(&g, 4);
    let ability = activate(&mut g, quantum, royal, target);
    match case {
        ResponseCase::None => (),
        ResponseCase::UntapTarget => {
            priority(&mut g, quantum, 1);
            answer(&mut g, quantum, Response::Cast { card: CardId(7) });
            answer(
                &mut g,
                quantum,
                Response::Targets {
                    objects: vec![target],
                },
            );
            pay_spell(&mut g, quantum, &[8], Color::W);
            finish_to(&mut g, quantum, 1);
            assert!(!g.current(target).unwrap().tapped);
        }
        ResponseCase::BounceSource => {
            priority(&mut g, quantum, 2);
            answer(&mut g, quantum, Response::Cast { card: CardId(9) });
            answer(
                &mut g,
                quantum,
                Response::Targets {
                    objects: vec![royal],
                },
            );
            pay_spell(&mut g, quantum, &[10, 11, 12], Color::U);
            finish_to(&mut g, quantum, 1);
            assert!(g.current(royal).is_err());
            assert_eq!(
                g.current(object(&g, 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
        }
    }
    recover(&g);
    resolve_start(&mut g);
    let legal = !matches!(case, ResponseCase::UntapTarget);
    assert!(
        matches!(&g.state.rules.frame,Frame::Resolve { frame } if frame.spell.id==ability && frame.legal_targets==[legal])
    );
    recover(&g);
    finish_to(&mut g, quantum, 0);
    if legal {
        assert!(g.current(target).is_err());
        assert_eq!(
            g.current(object(&g, 4)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
    } else {
        assert_eq!(g.current(target).unwrap().zone, Zone::Battlefield);
        assert!(!g.current(target).unwrap().tapped);
    }
    if !matches!(case, ResponseCase::BounceSource) {
        assert!(g.current(royal).unwrap().tapped);
    }
    if mode == TraceMode::Detailed {
        assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if *object==royal)).count(),1);
        assert!(
            g.events()
                .iter()
                .any(|e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id==ability))
        );
    }
    recover(&g);
    g
}
fn quanta(case: ResponseCase) {
    let a = run(case, 1, TraceMode::Detailed);
    let b = run(case, 4096, TraceMode::Off);
    assert_eq!(a.state, b.state);
    assert_eq!(a.replay_journal(), b.replay_journal());
}
#[test]
fn tap_source_destroy_cast_creature_ages_then_pays_tap_and_destroys_real_tapped_elf() {
    quanta(ResponseCase::None);
}
#[test]
fn tap_source_destroy_real_burst_untaps_target_and_fizzles_with_cost_spent() {
    quanta(ResponseCase::UntapTarget);
}
#[test]
fn tap_source_destroy_paid_ability_survives_real_regress_source_bounce() {
    quanta(ResponseCase::BounceSource);
}
#[test]
fn tap_source_destroy_new_capability_remains_outside_host_and_search() {
    let p = prepared();
    let mut c = p.catalog.as_ref().clone();
    c.definitions.retain(|d| d.name == "Royal Assassin");
    c.validate().unwrap();
    for feature in [Feature::Hosting, Feature::Search] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = false;
        profile.search = false;
        match feature {
            Feature::Hosting => profile.hosted = true,
            Feature::Search => profile.search = true,
        }
        assert!(
            matches!(profile.validate_requested_features(&c),Err(ProfileError::UnreviewedCapabilities { feature:actual,capabilities }) if actual==feature && capabilities.contains(&Capability::TapSourceDestroyV1))
        );
    }
}
