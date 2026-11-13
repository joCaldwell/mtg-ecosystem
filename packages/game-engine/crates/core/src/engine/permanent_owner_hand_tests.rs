//! Complete Regress/Disperse sources in private configurations, not legal pods.
//! Accepted spell payments/concessions; initial ownership/control is explicit.
//! Pinned CR400.3/7,608.2b,800.4a,903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::profile_data::{Feature, ProfileError};

fn prepared(rows: &[(&str, u8, u8, bool)], commander: bool) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../../tests/fixtures/2fa763ad-4d94-4c3a-a099-13ac22c09ce4.json"),
        include_str!("../../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete permanent owner-hand source must lower");
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::PermanentOwnerHandMoveV1)
        );
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "permanent-owner-hand-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(id, (name, owner, controller, hand))| CardSetup {
            id: CardId(id as u32),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*owner),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    if commander {
        config.commanders = vec![CardId(0)];
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
fn object(g: &Game, card: u32) -> ObjectId {
    g.object_id(CardId(card)).unwrap()
}
fn request(g: &mut Game, quantum: u64) -> Request {
    for _ in 0..500 {
        match g.advance(Budget::new(quantum)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            other => panic!("permanent owner-hand {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("permanent owner-hand work bound");
}
fn answer(g: &mut Game, quantum: u64, response: Response) {
    let r = request(g, quantum);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn announce(g: &mut Game, quantum: u64, card: u32) -> Request {
    answer(g, quantum, Response::Cast { card: CardId(card) });
    let r = request(g, quantum);
    assert!(matches!(r, Request::Targets { .. }));
    r
}
fn pay(g: &mut Game, quantum: u64, target: ObjectId, lands: &[u32]) -> StackId {
    answer(
        g,
        quantum,
        Response::Targets {
            objects: vec![target],
        },
    );
    for land in lands {
        let land = object(g, *land);
        answer(g, quantum, Response::ActivateMana { object: land });
    }
    answer(g, quantum, Response::FinishMana { undo_mana: false });
    let r = request(g, quantum);
    let Request::SpendMana { cost, .. } = r else {
        panic!("spending request");
    };
    assert_eq!(cost.colored, vec![Color::U]);
    assert_eq!(cost.generic, Quantity::from(lands.len() as i64 - 1));
    answer(
        g,
        quantum,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, Quantity::from(lands.len() as i64))]),
        },
    );
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    for land in lands {
        assert!(g.current(object(g, *land)).unwrap().tapped);
    }
    g.top().unwrap().id
}
fn finish(g: &mut Game, quantum: u64, commander_choice: Option<bool>) {
    for _ in 0..30 {
        let r = request(g, quantum);
        if g.state.rules.stack.is_empty() {
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::CommanderReturn { .. } => Response::CommanderReturn {
                to_command: commander_choice.expect("explicit Commander choice"),
            },
            other => panic!("unexpected owner-hand continuation {other:?}"),
        };
        answer(g, quantum, response);
    }
    panic!("owner-hand completion bound");
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
fn reject(g: &mut Game, r: &Request, target: ObjectId) {
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![target]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
}
fn completed(g: &Game, stack: StackId, resolved: bool) {
    if g.trace.mode == TraceMode::Detailed {
        assert!(g.events().iter().any(|e|matches!(e,Event::SpellCompleted { stack_id,resolved:r } if *stack_id == stack && *r == resolved)));
    }
}

#[test]
fn permanent_owner_hand_regress_returns_land_and_borrowed_artifact_to_owner_with_new_identity() {
    for name in ["Island", "Sol Ring"] {
        let p = prepared(
            &[
                (name, 1, 2, false),
                ("Regress", 0, 0, true),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
                ("Regress", 0, 0, true),
            ],
            false,
        );
        let mut baseline = None;
        for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
            let mut g = Game::with_trace(p.clone(), mode);
            let target = object(&g, 0);
            let r = announce(&mut g, quantum, 1);
            assert!(matches!(r,Request::Targets { candidates,.. } if candidates.contains(&target)));
            let stack = pay(&mut g, quantum, target, &[2, 3, 4]);
            recover(&g);
            finish(&mut g, quantum, None);
            completed(&g, stack, true);
            let successor = object(&g, 0);
            assert_ne!(successor, target);
            assert!(g.current(target).is_err());
            assert_eq!(
                g.current(successor).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            assert_eq!(g.current(successor).unwrap().controller, PlayerId(1));
            recover(&g);
            // A second genuine spell proposal has legal Island alternatives.
            // Neither the old ID nor the new hand incarnation is a target.
            let r = announce(&mut g, quantum, 5);
            reject(&mut g, &r, target);
            reject(&mut g, &r, successor);
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
fn permanent_owner_hand_disperse_rejects_land_atomically_and_returns_artifact_or_enchantment() {
    for selected in [4, 5] {
        let p = prepared(
            &[
                ("Island", 1, 1, false),
                ("Disperse", 0, 0, true),
                ("Island", 0, 0, false),
                ("Island", 0, 0, false),
                ("Sol Ring", 1, 1, false),
                ("Glorious Anthem", 1, 1, false),
            ],
            false,
        );
        let mut baseline = None;
        for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
            let mut g = Game::with_trace(p.clone(), mode);
            let land = object(&g, 0);
            let artifact = object(&g, 4);
            let enchantment = object(&g, 5);
            let r = announce(&mut g, quantum, 1);
            assert!(
                matches!(&r,Request::Targets { candidates,.. } if !candidates.contains(&land) && candidates.contains(&artifact) && candidates.contains(&enchantment))
            );
            reject(&mut g, &r, land);
            let target = object(&g, selected);
            let stack = pay(&mut g, quantum, target, &[2, 3]);
            recover(&g);
            finish(&mut g, quantum, None);
            completed(&g, stack, true);
            assert_eq!(
                g.current(object(&g, selected)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            assert!(g.current(target).is_err());
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
fn permanent_owner_hand_commander_owner_chooses_hand_or_command_and_feature_guards_hold() {
    let p = prepared(
        &[
            ("Isamaru, Hound of Konda", 1, 1, false),
            ("Disperse", 0, 0, true),
            ("Island", 0, 0, false),
            ("Island", 0, 0, false),
        ],
        true,
    );
    for to_command in [false, true] {
        let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
        let target = object(&g, 0);
        announce(&mut g, 1, 1);
        let stack = pay(&mut g, 1, target, &[2, 3]);
        for _ in 0..10 {
            let r = request(&mut g, 1);
            if matches!(r, Request::CommanderReturn { .. }) {
                assert_eq!(r.player(), PlayerId(1));
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
        finish(&mut g, 1, Some(to_command));
        completed(&g, stack, true);
        let zone = if to_command {
            Zone::Command { owner: PlayerId(1) }
        } else {
            Zone::Hand { owner: PlayerId(1) }
        };
        assert_eq!(g.current(object(&g, 0)).unwrap().zone, zone);
        recover(&g);
    }
    for feature in [Feature::Hosting, Feature::Search] {
        let mut profile = match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted = false;
        profile.search = false;
        match feature {
            Feature::Hosting => profile.hosted = true,
            Feature::Search => profile.search = true,
        }
        assert!(
            matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities { feature:actual,capabilities }) if actual == feature && capabilities.contains(&Capability::PermanentOwnerHandMoveV1))
        );
    }
}
#[test]
fn permanent_owner_hand_departed_target_fizzles_without_returning_or_rebinding() {
    let p = prepared(
        &[
            ("Sol Ring", 1, 1, false),
            ("Disperse", 0, 0, true),
            ("Island", 0, 0, false),
            ("Island", 0, 0, false),
        ],
        false,
    );
    let mut baseline = None;
    for (quantum, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        let target = object(&g, 0);
        announce(&mut g, quantum, 1);
        let stack = pay(&mut g, quantum, target, &[2, 3]);
        g.concede(PlayerId(1)).unwrap();
        assert!(matches!(request(&mut g, quantum), Request::Priority { .. }));
        assert!(g.current(target).is_err());
        recover(&g);
        finish(&mut g, quantum, None);
        completed(&g, stack, false);
        assert_eq!(
            g.current(object(&g, 1)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        recover(&g);
        if let Some(old) = &baseline {
            assert_eq!(old, &g.state);
        } else {
            baseline = Some(g.state.clone());
        }
    }
}
