//! Complete frozen cycling sources. Private seeded/midgame fixtures only;
//! no profile, catalog, search, host or policy admission. Native tests UNRUN
//! by the source owner; parent qualifies the stable union once.
//! CR602.2a-b/701.20a/702.29a/733.1/800.4a, frozen2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::typed_land_search_tests::{observation_work, typed_prepared, validate};
use super::*;
const KETRIA: &str =
    include_str!("../../../../tests/fixtures/ramos/6bae00e8-06cf-4ac4-a1cc-757e454109fe.json");
const SPARA: &str =
    include_str!("../../../../tests/fixtures/ramos/3123ec89-8e95-4761-ba17-747ec667509f.json");
const ZIATORA: &str =
    include_str!("../../../../tests/fixtures/ramos/f7e7b78c-c769-4720-8585-1874773eb342.json");
const RAMOS: &str =
    include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");
fn lower(raw: &str) -> Definition {
    match mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    ) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn prepared(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    let removal = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap();
    for d in removal
        .definitions
        .into_iter()
        .filter(|d| d.name == "Stifle")
        .chain([lower(KETRIA), lower(SPARA), lower(ZIATORA), lower(RAMOS)])
    {
        c.definitions.retain(|old| old.oracle_id != d.oracle_id);
        c.definitions.push(d);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut cards: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, owner, controller, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
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
    let mut commanders = vec![];
    for seat in 0..4 {
        let owner = PlayerId(seat);
        commanders.push(CardId(cards.len() as u32));
        cards.push(CardSetup {
            id: CardId(cards.len() as u32),
            oracle_id: lower(RAMOS).oracle_id,
            owner,
            controller: owner,
            zone: Zone::Command { owner },
        });
        while cards.iter().filter(|c| c.owner == owner).count() < 100 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner,
                controller: owner,
                zone: Zone::Library { owner },
            });
        }
        assert_eq!(cards.iter().filter(|c| c.owner == owner).count(), 100);
        let mut names = BTreeSet::new();
        for row in cards.iter().filter(|c| c.owner == owner) {
            let d = c.definition_by_oracle_id(&row.oracle_id).unwrap();
            assert!(d.characteristics.basic || names.insert(d.name.clone()));
        }
    }
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::PrecombatMain,
            ..Default::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "cycling-private-midgame".into(),
        commanders,
        cards,
    };
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
fn public(g: &mut Game, response: PlayerResponse) {
    let receipt = validate(g, response);
    g.submit(receipt).unwrap();
}
fn announce_cycle(g: &mut Game, source: ObjectId) {
    let _ = request(g);
    let handle = g.handle_for(PlayerId(0), source).unwrap();
    public(
        g,
        PlayerResponse::ActivateAbility {
            object: handle,
            ability: 0,
        },
    );
    // An accepted but unexecuted input reveals nothing and survives recovery.
    assert!(g.state.rules.stack.is_empty());
    check(g);
    assert!(matches!(
        request(g),
        Request::Mana {
            player: PlayerId(0),
            ..
        }
    ));
    assert_eq!(g.zone(source), Some(Zone::Hand { owner: PlayerId(0) }));
    assert!(g.top().unwrap().paid.is_none());
    let serialized = serde_json::to_value(g.top().unwrap()).unwrap();
    assert_eq!(
        serialized["cast_from"],
        serde_json::json!({"zone":"hand","owner":0})
    );
    assert_eq!(serialized["source_facts"]["zone"], serialized["cast_from"]);
    assert_eq!(serialized["kind"]["entry"], "activated");
    assert_eq!(observation(g, 0).observation_version, 41);
    for viewer in 0..4 {
        let view = observation(g, viewer).view;
        assert!(
            view.objects
                .iter()
                .any(|o| o.oracle_id == lower(KETRIA).oracle_id
                    && o.zone == (Zone::Hand { owner: PlayerId(0) }))
        );
        assert_eq!(view.stack.len(), 1);
        assert!(
            view.stack[0]
                .ability
                .as_ref()
                .unwrap()
                .source
                .current
                .is_some()
        );
    }
    check(g);
}
fn activate_mana(g: &mut Game, source: ObjectId) {
    let handle = g.handle_for(PlayerId(0), source).unwrap();
    public(g, PlayerResponse::ActivateMana { object: handle });
    let _ = request(g);
}
fn pay_cycle(g: &mut Game, spending: BTreeMap<Color, Quantity>) {
    public(g, PlayerResponse::FinishMana { undo_mana: true });
    assert!(matches!(request(g), Request::SpendMana { .. }));
    check(g);
    let before = g.snapshot().unwrap();
    let token = observation(g, 0).decision.unwrap().token().clone();
    let invalid = g.check_player_response(
        PlayerId(0),
        token,
        PlayerResponse::SpendMana {
            spending: BTreeMap::new(),
        },
        Budget::unlimited(),
    );
    assert!(!matches!(
        invalid,
        QueryResult::Complete(QueryValue::Validated(_))
    ));
    assert_eq!(g.snapshot().unwrap(), before);
    public(g, PlayerResponse::SpendMana { spending });
    // Full payment and discard remain pending together until one staged step.
    assert!(g.top().unwrap().paid.is_none());
    let _ = g.advance(Budget::new(1));
    assert!(g.top().unwrap().paid.is_some());
    let _ = request(g);
    check(g);
}
fn finish(g: &mut Game) {
    for _ in 0..40 {
        let r = request(g);
        if g.state.rules.stack.is_empty() {
            return;
        }
        assert!(matches!(r, Request::Priority { .. }));
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("bounded cycling completion");
}
fn query(g: &Game, source: ObjectId) -> Vec<ActionBeginning> {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let token = observation(g, 0).decision.unwrap().token().clone();
    let (outcome, continuation) = g
        .player_session(PlayerId(0))
        .action_beginnings(
            token,
            ActionScope::Object {
                object: g.handle_for(PlayerId(0), source).unwrap(),
            },
            64,
            Budget::unlimited(),
        )
        .into_parts();
    assert!(continuation.is_none());
    let ActionQueryOutcome::Complete { page } = outcome else {
        panic!("action query");
    };
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    page.beginnings
}

#[test]
fn cycling_public_api_paid_cycle_exact_cost_draw_and_replay() {
    // One bounded real public-API path from a seeded opening and legal100 deck.
    // No direct state mutation supplies cards/mana/control after Game creation.
    let p = typed_prepared(KETRIA);
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let card = develop_search(&mut g, "Ketria Triome", 3);
    let source = g.object_id(card).unwrap();
    let before = g.state.clone();
    assert!(query(&g, source).iter().any(|b| matches!(
        b.initiation,
        ActionInitiation::ActivateAbility { ability: 0, .. }
    )));
    assert_eq!(g.state, before);
    for seat in 1..4 {
        assert!(
            !observation(&g, seat)
                .view
                .objects
                .iter()
                .any(|o| o.oracle_id == lower(KETRIA).oracle_id)
        );
    }
    assert!(g.validate_mana_activation(PlayerId(0), source).is_err());
    announce_cycle(&mut g, source);
    observation_work(&g, PlayerId(0));
    observation_work(&g, PlayerId(1));
    let forests: Vec<_> = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
        .map(|o| o.id)
        .collect();
    assert_eq!(forests.len(), 3);
    for forest in forests {
        activate_mana(&mut g, forest);
    }
    let library = g.state.rules.libraries[&PlayerId(0)].len();
    let other_hand = g
        .action_hand(PlayerId(0))
        .into_iter()
        .filter(|o| *o != source)
        .collect::<BTreeSet<_>>();
    let bad = g.snapshot().unwrap();
    let wrong = PlayerResponse::ActivateAbility {
        object: g.handle_for(PlayerId(0), source).unwrap(),
        ability: 0,
    };
    let token = observation(&g, 0).decision.unwrap().token().clone();
    assert!(!matches!(
        g.check_player_response(PlayerId(0), token, wrong, Budget::unlimited()),
        QueryResult::Complete(QueryValue::Validated(_))
    ));
    assert_eq!(g.snapshot().unwrap(), bad);
    pay_cycle(&mut g, BTreeMap::from([(Color::G, 3.into())]));
    assert!(g.current(source).is_err());
    let successor = g.object_id(card).unwrap();
    assert_eq!(
        g.zone(successor),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(
        g.top().unwrap().source_facts.as_ref().unwrap().zone,
        Zone::Hand { owner: PlayerId(0) }
    );
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library);
    assert!(other_hand.is_subset(&g.action_hand(PlayerId(0)).into_iter().collect()));
    let discards = g.events().iter().filter(|e| matches!(e, Event::Committed { proposed: ProposedEvent::DiscardCard { object, .. }, .. } if *object == source)).count();
    assert_eq!(discards, 1);
    let mut tiny = g.fork();
    for _ in 0..200 {
        let a = tiny.advance(Budget::new(1));
        let restored =
            Game::restore(tiny.prepared.as_ref().clone(), &tiny.snapshot().unwrap()).unwrap();
        assert_eq!(restored.state, tiny.state);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) if tiny.state.rules.stack.is_empty() => break,
            Advance::AwaitingDecision(r) => {
                assert!(matches!(r, Request::Priority { .. }));
                tiny.respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap();
            }
            other => panic!("bounded single-step cycling {other:?}"),
        }
    }
    assert!(tiny.state.rules.stack.is_empty());
    finish(&mut g);
    assert_eq!(tiny.state, g.state);
    assert_eq!(tiny.trace, g.trace);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library - 1);
    assert_eq!(g.object_id(card), Some(successor)); // ability completion never moves the card
    assert_eq!(
        g.events()
            .iter()
            .filter(|e| matches!(e, Event::AbilityActivated { source: o, .. } if *o == source))
            .count(),
        1
    );
    check(&g);
}

#[test]
fn cycling_paid_countered_ability_retains_exact_discard_and_no_draw() {
    let p = prepared(&[
        ("Ketria Triome", 0, 0, true),
        ("Sol Ring", 0, 0, false),
        ("Forest", 0, 0, false),
        ("Stifle", 1, 1, true),
        ("Island", 1, 1, false),
    ]);
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let _ = request(&mut g);
    let source = object(&g, 0);
    let ring = object(&g, 1);
    let forest = object(&g, 2);
    announce_cycle(&mut g, source);
    activate_mana(&mut g, ring);
    activate_mana(&mut g, forest);
    pay_cycle(
        &mut g,
        BTreeMap::from([(Color::C, 2.into()), (Color::G, 1.into())]),
    );
    let ability = g.top().unwrap().id;
    let successor = object(&g, 0);
    let library = g.state.rules.libraries[&PlayerId(0)].len();
    answer(&mut g, Response::Pass);
    assert_eq!(request(&mut g).player(), PlayerId(1));
    answer(&mut g, Response::Cast { card: CardId(3) });
    assert!(matches!(request(&mut g), Request::StackTargets { .. }));
    answer(
        &mut g,
        Response::StackTargets {
            entries: vec![ability],
        },
    );
    assert!(matches!(request(&mut g), Request::Mana { .. }));
    let island = object(&g, 4);
    answer(&mut g, Response::ActivateMana { object: island });
    answer(&mut g, Response::FinishMana { undo_mana: true });
    assert!(matches!(request(&mut g), Request::SpendMana { .. }));
    answer(
        &mut g,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::U, 1.into())]),
        },
    );
    let _ = request(&mut g);
    check(&g);
    finish(&mut g);
    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), library);
    assert_eq!(object(&g, 0), successor);
    assert_eq!(
        g.zone(successor),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    assert_eq!(g.mana(PlayerId(0), Color::C), Some(0.into()));
    assert!(g.current(ring).unwrap().tapped);
    assert_eq!(g.events().iter().filter(|e| matches!(e,Event::Committed { proposed:ProposedEvent::DiscardCard { object:o,.. },.. } if *o==source)).count(),1);
    // Ordinary cleanup and later source observation cannot discard or draw a
    // second time for the already-countered invocation.
    let turn = g.turn().number;
    for _ in 0..100 {
        let r = request(&mut g);
        if g.turn().number > turn {
            break;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            other => panic!("cleanup {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert!(g.turn().number > turn);
    assert_eq!(object(&g, 0), successor);
    check(&g);
}

#[test]
fn cycling_owner_not_stale_controller_private_eligibility_and_cancel_memory() {
    let p = prepared(&[
        ("Ketria Triome", 0, 1, true),
        ("Spara's Headquarters", 1, 0, true),
        ("Sol Ring", 0, 0, false),
        ("Forest", 0, 0, false),
    ]);
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let _ = request(&mut g);
    let source = object(&g, 0);
    let opponent = object(&g, 1);
    assert_eq!(g.current(source).unwrap().controller, PlayerId(1));
    assert!(g.validate_activation(PlayerId(0), source, 0).is_ok());
    assert!(g.validate_activation(PlayerId(1), source, 0).is_err());
    assert!(g.validate_activation(PlayerId(0), opponent, 0).is_err());
    assert!(g.handle_for(PlayerId(0), opponent).is_err());
    for viewer in 1..4 {
        assert!(g.handle_for(PlayerId(viewer), source).is_err());
    }
    let before = g.snapshot().unwrap();
    assert!(
        g.announce_ability(PlayerId(1), g.state.rules.revision, source, 0)
            .is_err()
    );
    assert_eq!(g.snapshot().unwrap(), before);
    assert!(
        query(&g, source)
            .iter()
            .any(|b| matches!(b.initiation, ActionInitiation::ActivateAbility { .. }))
    );
    for viewer in 1..4 {
        assert!(g.handle_for(PlayerId(viewer), source).is_err());
    }
    announce_cycle(&mut g, source);
    let announced = observation(&g, 1).view;
    assert_eq!(g.top().unwrap().controller, PlayerId(0));
    let mut canceled = g.fork();
    public(
        &mut canceled,
        PlayerResponse::FinishMana { undo_mana: true },
    );
    let _ = request(&mut canceled);
    assert!(canceled.state.rules.stack.is_empty());
    assert_eq!(canceled.object_id(source.card), Some(source));
    assert!(canceled.handle_for(PlayerId(1), source).is_err());
    assert!(
        canceled.state.knowledge.visibility[1]
            .history
            .iter()
            .any(|h| h.matches_public(&announced))
    );
    check(&canceled);
    // Original still has its revealed unpaid source; canceling another fork
    // cannot erase or alter the actual announcement authority.
    assert_eq!(g.zone(source), Some(Zone::Hand { owner: PlayerId(0) }));
    let ring = object(&g, 2);
    let forest = object(&g, 3);
    activate_mana(&mut g, ring);
    activate_mana(&mut g, forest);
    let libraries = [
        g.state.rules.libraries[&PlayerId(0)].len(),
        g.state.rules.libraries[&PlayerId(1)].len(),
    ];
    pay_cycle(
        &mut g,
        BTreeMap::from([(Color::C, 2.into()), (Color::G, 1.into())]),
    );
    assert_eq!(g.top().unwrap().controller, PlayerId(0));
    assert_eq!(
        g.top().unwrap().source_facts.as_ref().unwrap().controller,
        PlayerId(1)
    );
    finish(&mut g);
    assert_eq!(
        g.state.rules.libraries[&PlayerId(0)].len(),
        libraries[0] - 1
    );
    assert_eq!(g.state.rules.libraries[&PlayerId(1)].len(), libraries[1]);
    check(&g);
}

#[test]
fn cycling_snapshot_hand_authority_source_and_payment_forgeries_reject() {
    let mut g = Game::new(prepared(&[("Ketria Triome", 0, 0, true)]));
    let _ = request(&mut g);
    let source = object(&g, 0);
    announce_cycle(&mut g, source);
    for case in 0..5 {
        let mut forged = g.fork();
        match case {
            0 => forged.state.rules.stack[0].cast_from = Zone::Battlefield,
            1 => {
                forged.state.rules.stack[0]
                    .source_facts
                    .as_mut()
                    .unwrap()
                    .zone = Zone::Battlefield
            }
            2 => forged.state.rules.stack[0].controller = PlayerId(1),
            3 => {
                forged.state.rules.stack[0].paid =
                    Some(forged.state.rules.stack[0].total_cost.clone())
            }
            _ => {
                if let Frame::Mana { cast } = &mut forged.state.rules.frame {
                    cast.original_object.incarnation += 1;
                } else {
                    panic!("root announcement");
                }
            }
        }
        assert!(
            Game::restore(
                forged.prepared.as_ref().clone(),
                &forged.snapshot().unwrap()
            )
            .is_err(),
            "{case}"
        );
    }
    check(&g);
}

#[test]
fn cycling_failed_payment_nested_mana_keep_undo_and_owner_departure() {
    for undo in [false, true] {
        let p = prepared(&[
            ("Ketria Triome", 0, 0, true),
            ("Forest", 0, 0, false),
            ("Golgari Signet", 0, 0, false),
            ("Spara's Headquarters", 1, 0, true),
        ]);
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        let _ = request(&mut g);
        let source = object(&g, 0);
        let forest = object(&g, 1);
        let signet = object(&g, 2);
        announce_cycle(&mut g, source);
        activate_mana(&mut g, signet);
        assert!(matches!(g.state.rules.frame, Frame::ManaAbility { .. }));
        activate_mana(&mut g, forest);
        check(&g);
        public(&mut g, PlayerResponse::FinishMana { undo_mana: false });
        let _ = request(&mut g);
        public(
            &mut g,
            PlayerResponse::SpendMana {
                spending: BTreeMap::from([(Color::G, 1.into())]),
            },
        );
        let _ = request(&mut g);
        assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
        assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
        check(&g);
        let stale = validate(&g, PlayerResponse::FinishMana { undo_mana: undo });
        public(&mut g, PlayerResponse::FinishMana { undo_mana: undo });
        let _ = request(&mut g);
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(g.object_id(source.card), Some(source));
        assert_eq!(g.current(forest).unwrap().tapped, !undo);
        assert_eq!(g.current(signet).unwrap().tapped, !undo);
        if undo {
            assert!(g.state.rules.players[0].mana.is_empty());
        } else {
            assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
        }
        let before = g.snapshot().unwrap();
        assert!(g.submit(stale).is_err());
        assert_eq!(g.snapshot().unwrap(), before);
        assert!(!g.events().iter().any(|e| matches!(e,Event::Committed { proposed:ProposedEvent::DiscardCard { object:o,.. },.. } if *o==source)));
        check(&g);
    }
    let mut g = Game::with_trace(
        prepared(&[("Ketria Triome", 0, 0, true)]),
        TraceMode::Detailed,
    );
    let _ = request(&mut g);
    let source = object(&g, 0);
    let receipt = validate(
        &g,
        PlayerResponse::ActivateAbility {
            object: g.handle_for(PlayerId(0), source).unwrap(),
            ability: 0,
        },
    );
    g.submit(receipt).unwrap();
    g.concede(PlayerId(0)).unwrap();
    let _ = request(&mut g);
    assert!(g.state.rules.stack.is_empty());
    assert!(g.object_id(source.card).is_none());
    check(&g);
}

#[test]
fn cycling_nested_mana_departure_preserves_hand_announcement_and_pending_replay() {
    let mut g = Game::with_trace(
        prepared(&[
            ("Ketria Triome", 0, 0, true),
            ("Golgari Signet", 1, 0, false),
            ("Forest", 0, 0, false),
        ]),
        TraceMode::Detailed,
    );
    let _ = request(&mut g);
    let source = object(&g, 0);
    let signet = object(&g, 1);
    announce_cycle(&mut g, source);
    let handle = g.handle_for(PlayerId(0), signet).unwrap();
    public(&mut g, PlayerResponse::ActivateMana { object: handle });
    check(&g); // accepted nested activation, not executed
    let _ = request(&mut g);
    assert!(matches!(g.state.rules.frame, Frame::ManaAbility { .. }));
    g.concede(PlayerId(1)).unwrap();
    let _ = request(&mut g);
    assert!(matches!(g.state.rules.frame, Frame::Mana { .. }));
    assert_eq!(g.top().unwrap().source, source);
    assert_eq!(g.zone(source), Some(Zone::Hand { owner: PlayerId(0) }));
    assert!(g.top().unwrap().paid.is_none());
    assert!(
        observation(&g, 2)
            .view
            .objects
            .iter()
            .any(|o| o.oracle_id == lower(KETRIA).oracle_id)
    );
    check(&g);
    // Failed root payment cancels only the announced invocation; the source
    // never moved and no discarded/cycled/draw occurrence can be replayed.
    public(&mut g, PlayerResponse::FinishMana { undo_mana: true });
    let _ = request(&mut g);
    assert!(g.state.rules.stack.is_empty());
    assert_eq!(object(&g, 0), source);
    check(&g);
}

#[test]
fn cycling_played_typed_lands_stay_tapped_all_intrinsic_colors_and_no_battlefield_cycling() {
    for (raw, colors) in [
        (KETRIA, vec![Color::G, Color::U, Color::R]),
        (SPARA, vec![Color::G, Color::W, Color::U]),
        (ZIATORA, vec![Color::B, Color::R, Color::G]),
    ] {
        let name = lower(raw).name;
        let p = prepared(&[(&name, 0, 0, true)]);
        let mut g = Game::new(p);
        let _ = request(&mut g);
        let old = object(&g, 0);
        let handle = g.handle_for(PlayerId(0), old).unwrap();
        public(&mut g, PlayerResponse::PlayLand { object: handle });
        let _ = request(&mut g);
        let entered = object(&g, 0);
        assert_ne!(old, entered);
        assert!(g.current(entered).unwrap().tapped);
        assert_eq!(g.mana_abilities(entered).unwrap(), colors);
        assert!(g.validate_activation(PlayerId(0), entered, 0).is_err());
        assert!(
            query(&g, entered)
                .iter()
                .all(|b| !matches!(b.initiation, ActionInitiation::ActivateAbility { .. }))
        );
        check(&g);
        // Private declared battlefield setup reaches an actual choice for each
        // color; play/entry algorithm above remains untouched and always taps.
        for color in colors {
            let mut mana = Game::new(prepared(&[(&name, 0, 0, false)]));
            let _ = request(&mut mana);
            let source = object(&mana, 0);
            activate_mana(&mut mana, source);
            let Request::ManaColor {
                colors: offered, ..
            } = request(&mut mana)
            else {
                panic!("intrinsic triple choice");
            };
            assert_eq!(offered.len(), 3);
            assert!(offered.contains(&color));
            public(&mut mana, PlayerResponse::ManaColor { color });
            let _ = request(&mut mana);
            assert_eq!(mana.mana(PlayerId(0), color), Some(1.into()));
            check(&mana);
        }
    }
}
