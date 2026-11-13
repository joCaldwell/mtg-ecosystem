//! Source-backed private witnesses, not an expansion of a public match pool.
//! Pinned CR 506.4, 509.1g, 511.3, 608.2b and 514.2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn prepared() -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        mtg_compiler::fixtures::MANIFEST,
        include_str!("../../../../tests/fixtures/righteousness.json"),
    ) else {
        panic!("complete retained Righteousness must lower");
    };
    assert_eq!(definition.name, "Righteousness");
    assert!(
        definition
            .capabilities
            .contains(&Capability::BlockingCreatureTargetV1)
    );
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "blocking-target-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards = [
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
        ("Grizzly Bears", 1, false),
        ("Plains", 1, false),
        ("Righteousness", 1, true),
        ("Unsummon", 0, true),
        ("Island", 0, false),
        ("Righteousness", 0, true),
        ("Plains", 0, false),
    ]
    .iter()
    .enumerate()
    .map(|(id, (name, seat, hand))| CardSetup {
        id: CardId(id as u32),
        oracle_id: catalog
            .definitions
            .iter()
            .find(|d| d.name == *name)
            .unwrap()
            .oracle_id
            .clone(),
        owner: PlayerId(*seat),
        controller: PlayerId(*seat),
        zone: if *hand {
            Zone::Hand {
                owner: PlayerId(*seat),
            }
        } else {
            Zone::Battlefield
        },
    })
    .collect();
    let land = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Plains")
        .unwrap()
        .oracle_id
        .clone();
    for seat in 0..4 {
        for _ in 0..8 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: land.clone(),
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
    for _ in 0..1000 {
        match g.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(r) => return r,
            Advance::Suspended { .. } => (),
            other => panic!("blocking witness {:?}: {other:?}", g.state.rules.frame),
        }
    }
    panic!("blocking witness exceeded bounded advance count")
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
        Request::Damage {
            power, recipients, ..
        } => {
            assert_eq!(recipients.len(), 1);
            Response::Damage {
                assignments: vec![(recipients[0], power.clone())],
            }
        }
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates.iter().take(*count).copied().collect(),
        },
        other => panic!("unexpected passive blocking request {other:?}"),
    };
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn declare(g: &mut Game, quantum: u64) {
    for _ in 0..100 {
        if matches!(request(g, quantum), Request::Attackers { .. }) {
            break;
        }
        passive(g, quantum);
    }
    let attacker = object(g, 0);
    answer(
        g,
        quantum,
        Response::Attackers {
            attacks: vec![(attacker, PlayerId(1))],
        },
    );
    for _ in 0..100 {
        if matches!(request(g, quantum), Request::Blockers { .. }) {
            break;
        }
        passive(g, quantum);
    }
    let blocker = object(g, 1);
    answer(
        g,
        quantum,
        Response::Blockers {
            blocks: vec![(blocker, attacker)],
        },
    );
    assert!(matches!(request(g, quantum), Request::Priority { .. }));
    assert!(g.is_blocking_creature(blocker));
    assert!(!g.is_blocking_creature(object(g, 2)));
}
fn priority(g: &mut Game, quantum: u64, player: PlayerId) {
    for _ in 0..20 {
        let r = request(g, quantum);
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        if r.player() == player {
            return;
        }
        passive(g, quantum);
    }
    panic!("missing priority")
}
fn announce(g: &mut Game, quantum: u64, card: u32, target: ObjectId) {
    let caster = g.current(object(g, card)).unwrap().controller;
    priority(g, quantum, caster);
    answer(g, quantum, Response::Cast { card: CardId(card) });
    for _ in 0..30 {
        let r = request(g, quantum);
        let response = match &r {
            Request::Targets { candidates, .. } => {
                assert!(candidates.contains(&target));
                Response::Targets {
                    objects: vec![target],
                }
            }
            Request::Mana { cost, .. } => {
                if g.payment_pool(caster, cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == caster
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                assert_eq!(cost.generic, Quantity::zero());
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for color in &cost.colored {
                    assert!(available.get(color).is_some_and(|q| *q > Quantity::zero()));
                    spending.entry(*color).or_default().add(&1.into());
                }
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => return,
            other => panic!("unexpected announcement {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("announcement bound")
}
fn resolve(g: &mut Game, quantum: u64) {
    for _ in 0..60 {
        request(g, quantum);
        if g.state.rules.stack.is_empty() {
            return;
        }
        passive(g, quantum);
    }
    panic!("resolution bound")
}
fn reach(g: &mut Game, quantum: u64, step: TurnStep) {
    for _ in 0..200 {
        let r = request(g, quantum);
        if g.turn().step == step && matches!(r, Request::Priority { .. }) {
            return;
        }
        passive(g, quantum);
    }
    panic!("step {step:?} not reached")
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    let replayed = Game::replay_with_trace(p, &g.replay_journal(), g.trace.mode).unwrap();
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace(), g.trace());
}

#[test]
fn blocking_target_real_spell_combat_cleanup_and_replay_all_traces_and_budgets() {
    let mut expected = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for quantum in [1, u64::MAX] {
            let mut g = Game::with_trace(prepared(), mode);
            declare(&mut g, quantum);
            let casting_turn = g.turn().number;
            check(&g);
            let blocker = object(&g, 1);
            announce(&mut g, quantum, 4, blocker);
            check(&g);
            resolve(&mut g, quantum);
            assert_eq!(g.characteristics(blocker).unwrap().power, Some(9.into()));
            assert_eq!(
                g.characteristics(blocker).unwrap().toughness,
                Some(9.into())
            );
            check(&g);
            reach(&mut g, quantum, TurnStep::EndCombat);
            assert!(g.is_blocking_creature(blocker));
            assert_ne!(g.current(object(&g, 0)).unwrap().zone, Zone::Battlefield);
            assert_eq!(g.current(blocker).unwrap().marked_damage, 2.into());
            reach(&mut g, quantum, TurnStep::PostcombatMain);
            assert!(!g.is_blocking_creature(blocker));
            assert_eq!(g.characteristics(blocker).unwrap().power, Some(9.into()));
            check(&g);
            for _ in 0..100 {
                if g.turn().number > casting_turn {
                    break;
                }
                passive(&mut g, quantum);
            }
            request(&mut g, quantum);
            assert!(g.turn().number > casting_turn);
            assert_eq!(g.characteristics(blocker).unwrap().power, Some(2.into()));
            assert_eq!(g.current(blocker).unwrap().marked_damage, Quantity::zero());
            check(&g);
            let hash = g.state_digest().unwrap();
            if let Some(old) = &expected {
                assert_eq!(old, &hash);
            } else {
                expected = Some(hash);
            }
        }
    }
}

#[test]
fn blocking_target_orphan_survives_attacker_bounce_until_combat_ends() {
    let mut g = Game::new(prepared());
    declare(&mut g, 1);
    let attacker = object(&g, 0);
    let blocker = object(&g, 1);
    announce(&mut g, 1, 5, attacker);
    resolve(&mut g, 1);
    assert!(g.state.rules.combat.is_empty());
    assert!(g.is_blocking_creature(blocker));
    check(&g);
    reach(&mut g, 1, TurnStep::EndCombat);
    assert!(g.is_blocking_creature(blocker));
    announce(&mut g, 1, 4, blocker);
    resolve(&mut g, 1);
    assert_eq!(g.characteristics(blocker).unwrap().power, Some(9.into()));
    check(&g);
    reach(&mut g, 1, TurnStep::PostcombatMain);
    assert!(!g.is_blocking_creature(blocker));
    assert!(g.state.rules.blocking_creatures.is_empty());
    priority(&mut g, 1, PlayerId(0));
    let before = g.snapshot().unwrap();
    let r = request(&mut g, 1);
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Cast { card: CardId(7) }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    check(&g);
}

#[test]
fn blocking_target_offered_handles_invalid_input_and_queries_are_private_and_atomic() {
    let mut g = Game::new(prepared());
    priority(&mut g, 1, PlayerId(1));
    let initial = g.snapshot().unwrap();
    let r = request(&mut g, 1);
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Cast { card: CardId(4) }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), initial);
    declare(&mut g, 1);
    priority(&mut g, 1, PlayerId(1));
    answer(&mut g, 1, Response::Cast { card: CardId(4) });
    let r = request(&mut g, 1);
    assert!(
        matches!(&r, Request::Targets { candidates, .. } if candidates == &vec![object(&g, 1)])
    );
    let before = g.snapshot().unwrap();
    assert!(
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![object(&g, 2)]
            }
        )
        .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    check(&g);
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::Targets {
        token,
        candidates,
        ..
    }))) = g.player_decision_for(PlayerId(1), Budget::unlimited())
    else {
        panic!("permitted targets")
    };
    assert_eq!(candidates.len(), 1);
    let selected = candidates[0].clone();
    assert!(matches!(
        g.check_player_response(
            PlayerId(1),
            token.clone(),
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("foreign".into())]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    let response = PlayerResponse::Targets {
        objects: candidates,
    };
    assert!(matches!(
        g.check_player_response(
            PlayerId(0),
            token.clone(),
            response.clone(),
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
    assert!(matches!(
        g.check_player_response(PlayerId(1), token.clone(), response, Budget::unlimited()),
        QueryResult::Complete(QueryValue::Validated(_))
    ));
    let QueryResult::Complete(QueryValue::Observation(other)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("other view")
    };
    assert!(other.decision.is_none());
    let wire = serde_json::to_string(&other).unwrap();
    for private in [
        "blocking_creatures",
        "state_sha256",
        "profile_digest",
        "journal",
        "incarnation",
    ] {
        assert!(!wire.contains(private), "{private}");
    }
    assert_eq!(g.snapshot().unwrap(), before);
    let blocker = object(&g, 1);
    answer(
        &mut g,
        1,
        Response::Targets {
            objects: vec![blocker],
        },
    );
    request(&mut g, 1);
    assert!(matches!(
        g.check_player_response(
            PlayerId(1),
            token,
            PlayerResponse::Targets {
                objects: vec![selected]
            },
            Budget::unlimited()
        ),
        QueryResult::Rejected(QueryRejection::InvalidResponse)
    ));
}

#[test]
fn blocking_target_resolution_rechecks_departed_exact_incarnation() {
    let mut g = Game::new(prepared());
    declare(&mut g, 1);
    let blocker = object(&g, 1);
    announce(&mut g, 1, 4, blocker);
    announce(&mut g, 1, 5, blocker);
    check(&g);
    resolve(&mut g, 1);
    let successor = object(&g, 1);
    assert_ne!(successor, blocker);
    assert_eq!(
        g.current(successor).unwrap().zone,
        Zone::Hand { owner: PlayerId(1) }
    );
    assert!(!g.is_blocking_creature(blocker));
    assert!(!g.is_blocking_creature(successor));
    assert_eq!(g.characteristics(successor).unwrap().power, Some(2.into()));
    assert!(
        !g.state
            .rules
            .effects
            .iter()
            .any(|e| matches!(e.kind, ResolvedEffectKind::ModifyPowerToughness { .. }))
    );
    check(&g);
}

#[test]
fn blocking_target_private_control_return_ends_status_without_leaving_battlefield() {
    // The pool admits no control-grant spell. This explicitly synthetic kernel
    // setup exercises the real control-return/concession path; ordinary replay
    // is asserted by the other witnesses, not for this injected initial grant.
    let mut p = prepared();
    p.config.cards[1].owner = PlayerId(0);
    p.config.cards[2].owner = PlayerId(2);
    p.config.cards[2].controller = PlayerId(2);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::new(p);
    let blocker = object(&g, 1);
    let source = object(&g, 2);
    g.state
        .rules
        .objects
        .get_mut(&CardId(1))
        .unwrap()
        .default_controller = PlayerId(0);
    g.state.rules.control_effects = vec![ControlEffect {
        id: 1,
        object: blocker,
        controller: PlayerId(1),
        source: ControlSource::Static { object: source },
    }];
    declare(&mut g, 1);
    announce(&mut g, 1, 4, blocker);
    g.concede(PlayerId(2)).unwrap();
    request(&mut g, 1);
    assert_eq!(g.current(blocker).unwrap().zone, Zone::Battlefield);
    assert_eq!(g.current(blocker).unwrap().controller, PlayerId(0));
    assert!(!g.is_blocking_creature(blocker));
    assert!(g.state.rules.combat[0].blocked);
    assert!(g.state.rules.combat[0].blockers.is_empty());
    resolve(&mut g, 1);
    assert_eq!(g.characteristics(blocker).unwrap().power, Some(2.into()));
    assert!(
        !g.state
            .rules
            .effects
            .iter()
            .any(|e| matches!(e.kind, ResolvedEffectKind::ModifyPowerToughness { .. }))
    );
}

#[test]
fn blocking_target_snapshot_membership_is_structural_and_replay_bound() {
    let mut g = Game::new(prepared());
    declare(&mut g, 1);
    let raw = g.snapshot().unwrap();
    for members in [
        serde_json::json!([]),
        serde_json::json!([object(&g, 1), object(&g, 2)]),
    ] {
        let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
        bad["state"]["blocking_creatures"] = members;
        bad["state_sha256"] = digest(&bad["state"]).unwrap().into();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
    }
    let profile = match_profile("commander-foundations-v1").unwrap();
    let mut single = g.prepared.catalog.as_ref().clone();
    single.definitions.retain(|d| d.name == "Righteousness");
    single.validate().unwrap();
    let crate::profile_data::ProfileError::UnreviewedCapabilities { capabilities, .. } =
        profile.validate_requested_features(&single).unwrap_err()
    else {
        panic!("new target capability must remain unreviewed")
    };
    assert!(capabilities.contains(&Capability::BlockingCreatureTargetV1));
    check(&g);
}
