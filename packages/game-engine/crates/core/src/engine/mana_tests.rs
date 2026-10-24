//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let c = catalog();
    let mut cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, p, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*p),
            controller: PlayerId(*p),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*p),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect::<Vec<_>>();
    for p in 0..4 {
        for _ in 0..4 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(p),
                controller: PlayerId(p),
                zone: Zone::Library { owner: PlayerId(p) },
            });
        }
    }
    let config = ScenarioConfig {
        mode: Mode::CommanderMatch,
        players: 4,
        profile: MANA_PROFILE.into(),
        commanders: rows
            .iter()
            .enumerate()
            .filter(|(_, (name, _, _))| {
                matches!(*name, "Isamaru, Hound of Konda" | "Yargle and Multani")
            })
            .map(|(i, _)| CardId(i as u32))
            .collect(),
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
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game) -> Request {
    match g.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(r) => r,
        x => panic!("{x:?}"),
    }
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn reach(g: &mut Game, step: TurnStep) {
    for _ in 0..200 {
        let r = request(g);
        if g.turn().step == step && matches!(r, Request::Priority { .. }) {
            return;
        }
        let response = match r {
            Request::OptionalRecursion { .. } => Response::OptionalRecursion { perform: true },
            Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: false },
            Request::Priority { .. } => Response::Pass,
            x => panic!("unexpected {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("step not reached")
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let mut replayed = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    if matches!(g.state.frame, Frame::Fault { .. }) {
        let _ = replayed.advance(Budget::new(1));
    }
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace(), g.trace());
}
fn traces(p: Prepared, run: impl Fn(&mut Game)) {
    let mut digest0 = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(p.clone(), mode);
        run(&mut g);
        check(&g);
        let d = g.state_digest().unwrap();
        if let Some(old) = &digest0 {
            assert_eq!(&d, old)
        } else {
            digest0 = Some(d)
        }
    }
}

fn mana(g: &mut Game, id: u32) {
    let object = object(g, id);
    answer(g, Response::ActivateMana { object });
}
fn spend(g: &mut Game, colors: &[(Color, i64)]) {
    answer(
        g,
        Response::SpendMana {
            spending: colors
                .iter()
                .map(|(c, n)| (*c, Quantity::from(*n)))
                .collect(),
        },
    );
}
fn finish(g: &mut Game, undo: bool) {
    answer(g, Response::FinishMana { undo_mana: undo });
}
fn choose(g: &mut Game, color: Color) {
    answer(g, Response::ManaColor { color });
}
fn pass(g: &mut Game, n: usize) {
    for _ in 0..n {
        answer(g, Response::Pass);
    }
    let _ = request(g);
}

#[test]
fn sol_ring_pays_generic_and_artifact_casts_normally() {
    // CR 106.1b/107.4c/601.2h: two C pay {2}, without a colored symbol.
    traces(
        prepared(&[("Sol Ring", 0, false), ("Mind Stone", 0, true)]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(g, Response::Cast { card: CardId(1) });
            mana(g, 0);
            assert!(matches!(request(g), Request::Mana { .. }));
            assert_eq!(g.mana(PlayerId(0), Color::C), Some(2.into()));
            assert_eq!(g.state.stack.len(), 1);
            finish(g, false);
            spend(g, &[(Color::C, 2)]);
            pass(g, 4);
            assert_eq!(g.zone(object(g, 1)), Some(Zone::Battlefield));
            assert_eq!(g.mana(PlayerId(0), Color::C), Some(0.into()));
        },
    );
}
#[test]
fn commander_identity_colors_and_birds_five_colors_are_controller_choices() {
    // CR 903.4a: immutable game-beginning commander definition, not Tower's
    // own empty identity, and never a basic-land subtype.
    for commander in ["Isamaru, Hound of Konda", "Yargle and Multani"] {
        for source in ["Arcane Signet", "Command Tower", "Commander's Sphere"] {
            traces(
                prepared(&[(commander, 0, false), (source, 0, false)]),
                |g| {
                    reach(g, TurnStep::PrecombatMain);
                    mana(g, 1);
                    let Request::ManaColor { colors, .. } = request(g) else {
                        panic!()
                    };
                    let expected = if commander.starts_with("Isamaru") {
                        vec![Color::W]
                    } else {
                        vec![Color::B, Color::G]
                    };
                    assert_eq!(colors, expected);
                    check(g);
                    assert!(g.decision_for(PlayerId(1)).unwrap().is_none());
                    choose(g, colors[0]);
                    let _ = request(g);
                    assert!(g.state.stack.is_empty());
                    assert_eq!(g.mana(PlayerId(0), colors[0]), Some(1.into()));
                },
            );
        }
    }
    traces(prepared(&[("Birds of Paradise", 0, false)]), |g| {
        reach(g, TurnStep::PrecombatMain);
        mana(g, 0);
        assert!(
            matches!(request(g),Request::ManaColor {colors,..} if colors==vec![Color::W,Color::U,Color::B,Color::R,Color::G])
        );
        let revision = g.state.revision;
        assert!(
            g.respond(
                PlayerId(0),
                revision,
                Response::ManaColor { color: Color::C }
            )
            .is_err()
        );
        choose(g, Color::U);
        let _ = request(g);
        assert_eq!(g.mana(PlayerId(0), Color::U), Some(1.into()));
    });
}

#[test]
fn signet_nested_payment_cast_and_lifo_undo_preserve_exact_pool_and_sources() {
    for undo in [true, false] {
        traces(
            prepared(&[
                ("Golgari Signet", 0, false),
                ("Forest", 0, false),
                ("Yargle and Multani", 0, true),
            ]),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                answer(g, Response::Cast { card: CardId(2) });
                mana(g, 0);
                assert!(matches!(request(g),Request::Mana {cost,..} if cost.generic==1.into()));
                check(g);
                mana(g, 1);
                let _ = request(g);
                check(g);
                assert!(!g.current(object(g, 0)).unwrap().tapped);
                finish(g, false);
                assert!(matches!(request(g), Request::SpendMana { .. }));
                check(g);
                spend(g, &[(Color::G, 1)]);
                let _ = request(g);
                assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
                assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
                assert_eq!(g.state.stack.len(), 1);
                check(g);
                finish(g, undo);
                let _ = request(g);
                assert_eq!(
                    g.zone(object(g, 2)),
                    Some(Zone::Hand { owner: PlayerId(0) })
                );
                if undo {
                    assert!(g.state.players[0].mana.is_empty());
                    assert!(
                        !g.current(object(g, 0)).unwrap().tapped
                            && !g.current(object(g, 1)).unwrap().tapped
                    );
                } else {
                    assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
                    assert!(
                        g.current(object(g, 0)).unwrap().tapped
                            && g.current(object(g, 1)).unwrap().tapped
                    );
                }
            },
        );
    }
    // CR 605.3a/601.2g: land pays Signet inside the same cast; its B/G
    // pays the Bears' reviewed {1}{G} cost (B pays generic).
    traces(
        prepared(&[
            ("Golgari Signet", 0, false),
            ("Forest", 0, false),
            ("Grizzly Bears", 0, true),
        ]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(g, Response::Cast { card: CardId(2) });
            mana(g, 0);
            mana(g, 1);
            finish(g, false);
            spend(g, &[(Color::G, 1)]);
            let _ = request(g);
            check(g);
            finish(g, false);
            spend(g, &[(Color::B, 1), (Color::G, 1)]);
            pass(g, 4);
            assert_eq!(g.zone(object(g, 2)), Some(Zone::Battlefield));
            assert_eq!(g.mana(PlayerId(0), Color::B), Some(0.into()));
            assert_eq!(g.mana(PlayerId(0), Color::G), Some(0.into()));
        },
    );
}
#[test]
fn cancelled_unfunded_nested_payment_has_no_source_side_effects() {
    traces(prepared(&[("Golgari Signet", 0, false)]), |g| {
        reach(g, TurnStep::PrecombatMain);
        let before = g.state.players[0].mana.clone();
        mana(g, 0);
        let _ = request(g);
        check(g);
        finish(g, true);
        let _ = request(g);
        assert_eq!(g.state.players[0].mana, before);
        assert!(!g.current(object(g, 0)).unwrap().tapped);
    });
}
#[test]
fn llanowar_tap_ability_waits_until_next_turn_and_mana_empties_each_step() {
    // CR 302.6/602.5a,106.4: creature {T} costs require continuous control.
    traces(
        prepared(&[("Llanowar Elves", 0, true), ("Forest", 0, false)]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(g, Response::Cast { card: CardId(0) });
            mana(g, 1);
            finish(g, false);
            spend(g, &[(Color::G, 1)]);
            pass(g, 4);
            let r = request(g);
            let source = object(g, 0);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::ActivateMana { object: source }
                )
                .is_err()
            );
            for _ in 0..400 {
                let r = request(g);
                if g.state.turn_owner == PlayerId(0)
                    && g.turn().number > 1
                    && g.turn().step == TurnStep::PrecombatMain
                    && matches!(r, Request::Priority { .. })
                {
                    break;
                }
                let response = match r {
                    Request::Priority { .. } => Response::Pass,
                    Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                    Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                    x => panic!("{x:?}"),
                };
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            assert!(g.turn().number > 1);
            mana(g, 0);
            let _ = request(g);
            assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
            reach(g, TurnStep::BeginningCombat);
            assert!(g.state.players[0].mana.is_empty());
        },
    );
}
#[test]
fn sphere_and_mind_stone_draw_use_stack_and_survive_murder_response() {
    // CR 602.2/701.21a/113.7a: sacrifice precedes response priority;
    // Murder targets another creature, since the artifact is already gone.
    for name in ["Commander's Sphere", "Mind Stone"] {
        traces(
            prepared(&[
                (name, 0, false),
                ("Forest", 0, false),
                ("Murder", 1, true),
                ("Swamp", 1, false),
                ("Swamp", 1, false),
                ("Swamp", 1, false),
                ("Grizzly Bears", 0, false),
            ]),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                let source = object(g, 0);
                answer(
                    g,
                    Response::ActivateAbility {
                        object: source,
                        ability: 0,
                    },
                );
                if name == "Mind Stone" {
                    let _ = request(g);
                    assert!(g.validate_mana_activation(PlayerId(0), source).is_err());
                    mana(g, 1);
                    finish(g, false);
                    spend(g, &[(Color::G, 1)]);
                }
                let _ = request(g);
                assert_eq!(
                    g.zone(object(g, 0)),
                    Some(Zone::Graveyard { owner: PlayerId(0) })
                );
                assert_eq!(g.state.stack.len(), 1);
                check(g);
                pass(g, 1);
                answer(g, Response::Cast { card: CardId(2) });
                let target = object(g, 6);
                answer(
                    g,
                    Response::Targets {
                        objects: vec![target],
                    },
                );
                for id in [3, 4, 5] {
                    mana(g, id);
                }
                finish(g, false);
                spend(g, &[(Color::B, 3)]);
                pass(g, 4);
                assert_eq!(
                    g.zone(object(g, 6)),
                    Some(Zone::Graveyard { owner: PlayerId(0) })
                );
                let before = g
                    .state
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                    .count();
                pass(g, 4);
                assert_eq!(
                    g.state
                        .objects
                        .values()
                        .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                        .count(),
                    before + 1
                );
                assert!(g.state.stack.is_empty());
            },
        );
    }
}
#[test]
fn mana_decks_identity_singleton_and_capacity_bounds() {
    let c = catalog();
    let config = MatchConfig::for_profile(MANA_PROFILE, [0; 32], PlayerId(0)).unwrap();
    validate_pod(&c, &config).unwrap();
    let counts = |deck: Decklist| deck.cards.iter().map(|r| r.count).sum::<u32>();
    assert_eq!(counts(mana_white_deck()), 100);
    assert_eq!(counts(mana_black_green_deck()), 100);
    let tower = c
        .definitions
        .iter()
        .find(|d| d.name == "Command Tower")
        .unwrap();
    let signet = c
        .definitions
        .iter()
        .find(|d| d.name == "Golgari Signet")
        .unwrap();
    let mut bad = config.clone();
    let white = &mut bad.decks[0];
    white
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == tower.oracle_id)
        .unwrap()
        .oracle_id = signet.oracle_id.clone();
    assert!(matches!(
        validate_pod(&c, &bad),
        Err(MatchAdmissionError::ColorIdentity { .. })
    ));
    let mut bad = config;
    let white = &mut bad.decks[0];
    white
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == tower.oracle_id)
        .unwrap()
        .count = 2;
    white
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == PLAINS)
        .unwrap()
        .count -= 1;
    assert!(matches!(
        validate_pod(&c, &bad),
        Err(MatchAdmissionError::SingletonName { .. })
    ));
    let creature_max = [mana_white_deck(), mana_black_green_deck()]
        .iter()
        .map(|d| {
            d.cards
                .iter()
                .filter(|r| {
                    c.definitions
                        .iter()
                        .find(|x| x.oracle_id == r.oracle_id)
                        .unwrap()
                        .characteristics
                        .kind
                        == CardKind::Creature
                })
                .map(|r| r.count as usize)
                .sum::<usize>()
        })
        .max()
        .unwrap();
    assert_eq!(creature_max, 31);
    let g = Game::new(prepared(&[]));
    assert_eq!(g.damage_limit(), 760);
    assert_eq!(g.stack_limit(), 3868);
    assert_eq!(g.event_group_limit(), 660);
    assert_eq!(g.token_capacity(), 256);
}

#[test]
fn signet_undo_preserves_floated_colorless_mana_and_exact_nested_spending() {
    for paid in [Color::C, Color::G] {
        traces(
            prepared(&[
                ("Sol Ring", 0, false),
                ("Golgari Signet", 0, false),
                ("Forest", 0, false),
                ("Yargle and Multani", 0, true),
            ]),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                mana(g, 0);
                let _ = request(g);
                answer(g, Response::Cast { card: CardId(3) });
                mana(g, 1);
                mana(g, 2);
                finish(g, false);
                spend(g, &[(paid, 1)]);
                let _ = request(g);
                check(g);
                finish(g, true);
                let _ = request(g);
                assert_eq!(
                    g.state.players[0].mana,
                    BTreeMap::from([(Color::C, 2.into())])
                );
                assert!(g.current(object(g, 0)).unwrap().tapped);
                assert!(
                    !g.current(object(g, 1)).unwrap().tapped
                        && !g.current(object(g, 2)).unwrap().tapped
                );
            },
        );
    }
}
#[test]
fn haste_exception_and_nested_payment_reject_wrong_timing_seat_and_reserved_source() {
    let mut p = prepared(&[("Llanowar Elves", 0, true), ("Forest", 0, false)]);
    let mut c = (*p.catalog).clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Llanowar Elves")
        .unwrap();
    d.keywords.push(KeywordAbility::Haste);
    d.capabilities = d.requirements();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    traces(p, |g| {
        reach(g, TurnStep::PrecombatMain);
        answer(g, Response::Cast { card: CardId(0) });
        mana(g, 1);
        finish(g, false);
        spend(g, &[(Color::G, 1)]);
        pass(g, 4);
        mana(g, 0);
        let _ = request(g);
        assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
    });
    traces(
        prepared(&[("Golgari Signet", 0, false), ("Forest", 0, false)]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            mana(g, 0);
            let r = request(g);
            for response in [
                Response::Pass,
                Response::ActivateMana {
                    object: object(g, 0),
                },
                Response::Cast { card: CardId(0) },
                Response::SpendMana {
                    spending: BTreeMap::new(),
                },
            ] {
                let before = g.snapshot().unwrap();
                assert!(g.respond(r.player(), r.decision_id(), response).is_err());
                assert_eq!(g.snapshot().unwrap(), before);
            }
            assert!(
                g.respond(
                    PlayerId(1),
                    r.decision_id(),
                    Response::FinishMana { undo_mana: true }
                )
                .is_err()
            );
            finish(g, true);
            let _ = request(g);
        },
    );
}
#[test]
fn nested_payment_concessions_preserve_live_payer_and_departed_last_known_facts() {
    for payer_leaves in [false, true] {
        traces(
            prepared(&[
                ("Golgari Signet", 0, false),
                ("Forest", 0, false),
                ("Yargle and Multani", 0, true),
            ]),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                answer(g, Response::Cast { card: CardId(2) });
                mana(g, 0);
                mana(g, 1);
                let _ = request(g);
                g.concede(PlayerId(if payer_leaves { 0 } else { 2 }))
                    .unwrap();
                let _ = request(g);
                check(g);
                if payer_leaves {
                    assert_eq!(
                        g.state.players[0].mana,
                        BTreeMap::from([(Color::G, 1.into())])
                    );
                    assert_eq!(
                        g.state.players[0].last_known.as_ref().unwrap().mana,
                        g.state.players[0].mana
                    );
                } else {
                    finish(g, false);
                    spend(g, &[(Color::G, 1)]);
                    let _ = request(g);
                    finish(g, true);
                    let _ = request(g);
                    assert!(g.state.players[0].mana.is_empty());
                }
            },
        );
    }
}

#[test]
fn optional_sacrifice_mana_cost_and_cost_triggers_reverse_with_source_and_pool() {
    // Private IR composition: the production pool has no sacrifice-mana source.
    // CR 733.1-2 reverses the cost and its pending triggers before priority.
    let mut p = prepared(&[
        ("Llanowar Elves", 0, false),
        ("Blood Artist", 1, false),
        ("Yargle and Multani", 0, true),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions
        .iter_mut()
        .find(|d| d.name == "Llanowar Elves")
        .unwrap()
        .mana_abilities[0]
        .costs
        .sacrifice = true;
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        reach(g, TurnStep::PrecombatMain);
        let source = object(g, 0);
        answer(g, Response::Cast { card: CardId(2) });
        mana(g, 0);
        let _ = request(g);
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert_eq!(g.state.pending_triggers.len(), 1);
        check(g);
        finish(g, true);
        let _ = request(g);
        assert_eq!(object(g, 0), source);
        assert_eq!(g.zone(source), Some(Zone::Battlefield));
        assert!(!g.current(source).unwrap().tapped);
        assert!(g.state.players[0].mana.is_empty());
        assert!(g.state.pending_triggers.is_empty());
    });
}
#[test]
fn nested_and_completed_mana_receipt_forgery_fails_replay_proof() {
    let p = prepared(&[
        ("Golgari Signet", 0, false),
        ("Forest", 0, false),
        ("Yargle and Multani", 0, true),
    ]);
    let mut g = Game::new(p.clone());
    reach(&mut g, TurnStep::PrecombatMain);
    answer(&mut g, Response::Cast { card: CardId(2) });
    mana(&mut g, 0);
    mana(&mut g, 1);
    let _ = request(&mut g);
    let snapshot = g.snapshot().unwrap();
    let mut forged: Snapshot = strict_read(&snapshot).unwrap();
    let Frame::ManaAbility { activation } = &mut forged.state.frame else {
        panic!()
    };
    activation.source.incarnation += 1;
    forged.state_sha256 = digest(&forged.state).unwrap();
    assert!(Game::restore(p.clone(), &canonical(&forged).unwrap()).is_err());
    finish(&mut g, false);
    spend(&mut g, &[(Color::G, 1)]);
    let _ = request(&mut g);
    let snapshot = g.snapshot().unwrap();
    for mutation in 0..4 {
        let mut forged: Snapshot = strict_read(&snapshot).unwrap();
        let Frame::Mana { cast } = &mut forged.state.frame else {
            panic!()
        };
        let receipt = cast
            .mana_activations
            .last_mut()
            .unwrap()
            .permanent
            .as_mut()
            .unwrap();
        match mutation {
            0 => receipt.produced.insert(Color::B, 2.into()),
            1 => receipt.spent.insert(Color::G, 2.into()),
            2 => {
                receipt.absent.clear();
                None
            }
            _ => {
                receipt.source.tapped = true;
                None
            }
        };
        forged.state_sha256 = digest(&forged.state).unwrap();
        assert!(Game::restore(p.clone(), &canonical(&forged).unwrap()).is_err());
    }
}

#[test]
fn nested_mana_and_color_public_queries_resume_and_recover_without_version_changes() {
    fn complete(g: &Game, mut q: QueryResult) -> QueryValue {
        loop {
            match q {
                QueryResult::Complete(v) => return v,
                QueryResult::Incomplete { work, .. } => q = g.resume_work(work, Budget::new(1)),
                QueryResult::Rejected(r) => panic!("{r:?}"),
            }
        }
    }
    fn validate(g: &mut Game, response: PlayerResponse) {
        let QueryValue::Observation(o) = complete(g, g.observe(PlayerId(0), Budget::new(1))) else {
            panic!()
        };
        let token = o.decision.as_ref().unwrap().token().clone();
        let QueryResult::Incomplete { work, .. } =
            g.check_player_response(PlayerId(0), token, response, Budget::new(1))
        else {
            panic!("expected budget suspension")
        };
        let saved = g.save_work(&work).unwrap();
        let restored = Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
        let restored_work = restored.restore_work(&saved).unwrap();
        assert!(matches!(
            complete(
                &restored,
                restored.resume_work(restored_work, Budget::new(1))
            ),
            QueryValue::Validated(_)
        ));
        let QueryValue::Validated(receipt) = complete(g, g.resume_work(work, Budget::new(1)))
        else {
            panic!()
        };
        g.submit(receipt).unwrap();
    }
    traces(
        prepared(&[
            ("Golgari Signet", 0, false),
            ("Forest", 0, false),
            ("Arcane Signet", 0, false),
            ("Isamaru, Hound of Konda", 0, false),
        ]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            mana(g, 0);
            let _ = request(g);
            let QueryValue::Observation(o) =
                complete(g, g.observe(PlayerId(0), Budget::unlimited()))
            else {
                panic!()
            };
            let source = o
                .view
                .objects
                .iter()
                .find(|o| o.oracle_id == FOREST && o.zone == Zone::Battlefield)
                .unwrap()
                .handle
                .clone();
            validate(g, PlayerResponse::ActivateMana { object: source });
            let _ = request(g);
            finish(g, false);
            spend(g, &[(Color::G, 1)]);
            let _ = request(g);
            mana(g, 2);
            let _ = request(g);
            validate(g, PlayerResponse::ManaColor { color: Color::W });
            let _ = request(g);
            assert_eq!(g.mana(PlayerId(0), Color::W), Some(1.into()));
        },
    );
}
