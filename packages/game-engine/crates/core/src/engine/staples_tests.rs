//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-staples-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let mut c = catalog();
    if rows.iter().any(|(name, _, _)| *name == "Unsummon") {
        let old =
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
        c.definitions.push(
            old.definitions
                .into_iter()
                .find(|d| d.name == "Unsummon")
                .unwrap(),
        );
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    }
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
        profile: STAPLES_PROFILE.into(),
        commanders: vec![],
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
#[allow(dead_code)]
fn attackers(g: &mut Game) -> Vec<ObjectId> {
    for _ in 0..100 {
        let r = request(g);
        if let Request::Attackers { candidates, .. } = r {
            return candidates;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("attackers not reached")
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
fn announce(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    if g.turn().step == TurnStep::Untap {
        reach(g, TurnStep::PrecombatMain);
    }
    let obj = object(g, id);
    answer(g, Response::Cast { card: obj.card });
    loop {
        let r = request(g);
        let response = match &r {
            Request::PlayerTargets { .. } => Response::PlayerTargets {
                players: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Player { player } => *player,
                        _ => panic!("player target"),
                    })
                    .collect(),
            },
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .objects
                        .values()
                        .filter(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .min_by_key(|o| {
                            usize::from(
                                !g.mana_abilities(o.id)
                                    .unwrap()
                                    .iter()
                                    .any(|c| Some(c) == cost.colored.first()),
                            )
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}

fn automatic(r: &Request, targets: &[TargetEntity]) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::Targets { .. } => Response::Targets {
            objects: targets
                .iter()
                .map(|t| match t {
                    TargetEntity::Object { object } => *object,
                    _ => panic!("creature target"),
                })
                .collect(),
        },
        Request::PlayerTargets { .. } => Response::PlayerTargets {
            players: vec![PlayerId(1)],
        },
        x => panic!("unexpected resolution request {x:?}"),
    }
}
fn resolve_with(g: &mut Game, targets: Vec<TargetEntity>, mut inspect: impl FnMut(&Game)) {
    for _ in 0..2000 {
        let advance = g.advance(Budget::new(1));
        inspect(g);
        match advance {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.stack.is_empty()
                    && g.state.pending_triggers.is_empty()
                {
                    return;
                }
                let response = automatic(&r, &targets);
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("resolve {x:?}"),
        }
    }
    panic!("resolution budget")
}
fn resolve(g: &mut Game, targets: Vec<TargetEntity>) {
    resolve_with(g, targets, |_| {});
}
fn cast(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    announce(g, id, targets.clone());
    resolve(g, targets);
}
fn target(o: ObjectId) -> Vec<TargetEntity> {
    vec![TargetEntity::Object { object: o }]
}
fn token_objects(g: &Game) -> Vec<ObjectId> {
    g.state
        .objects
        .values()
        .filter(|o| g.state.tokens.contains_key(&o.id.card))
        .map(|o| o.id)
        .collect()
}
fn observation(g: &Game, viewer: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(viewer), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}
fn lands(name: &str, n: usize) -> Vec<(&str, u8, bool)> {
    vec![(name, 0, false); n]
}
fn graveyard(mut p: Prepared, id: usize) -> Prepared {
    let owner = p.config.cards[id].owner;
    p.config.cards[id].zone = Zone::Graveyard { owner };
    p.profile_digest = digest(&p.config).unwrap();
    p
}
#[test]
fn murder_destroy_fizzle_and_simultaneous_day_lookback() {
    // CR701.8a/608.2b: destruction is an actual graveyard move; stale
    // incarnations fizzle the entire spell. CR603.10a: an Artist dying with
    // two Soldiers sees all THREE deaths before 704.5d ceases the tokens.
    let mut rows = vec![("Murder", 0, true), ("Grizzly Bears", 1, false)];
    rows.extend(lands("Swamp", 3));
    traces(prepared(&rows), |g| {
        let t = object(g, 1);
        cast(g, 0, target(t));
        assert_eq!(
            g.current(object(g, 1)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
    });
    let mut rows = vec![
        ("Murder", 0, true),
        ("Murder", 0, true),
        ("Grizzly Bears", 1, false),
    ];
    rows.extend(lands("Swamp", 6));
    traces(prepared(&rows), |g| {
        let t = object(g, 2);
        announce(g, 0, target(t));
        announce(g, 1, target(t));
        resolve(g, vec![]);
        assert_eq!(
            g.current(object(g, 2)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        if g.trace.mode == TraceMode::Detailed {
            let completed = g
                .events()
                .iter()
                .filter_map(|e| {
                    if let Event::SpellCompleted { resolved, .. } = e {
                        Some(*resolved)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(completed, vec![true, false]);
        }
    });
    let mut rows = vec![
        ("Raise the Alarm", 0, true),
        ("Day of Judgment", 0, true),
        ("Blood Artist", 0, false),
    ];
    rows.extend(lands("Plains", 6));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        assert_eq!(token_objects(g).len(), 2);
        cast(g, 1, vec![]);
        assert!(token_objects(g).is_empty());
        assert_eq!(g.state.players[0].life, 43.into());
        assert_eq!(g.state.players[1].life, 37.into());
        assert_eq!(g.state.pending_triggers.len(), 0);
    });
}
#[test]
fn recursion_targets_own_cards_optional_yes_no_and_commander() {
    for optional in [false, true] {
        for yes in [false, true] {
            if !optional && !yes {
                continue;
            }
            let name = if optional {
                "Gravedigger"
            } else {
                "Raise Dead"
            };
            let mut rows = vec![
                (name, 0, true),
                ("Grizzly Bears", 0, false),
                ("Grizzly Bears", 1, false),
            ];
            rows.extend(lands("Swamp", 4));
            traces(graveyard(graveyard(prepared(&rows), 1), 2), |g| {
                let old = object(g, 1);
                announce(g, 0, if optional { vec![] } else { target(old) });
                let mut asked = false;
                for _ in 0..100 {
                    let r = request(g);
                    if matches!(r, Request::Priority { .. })
                        && g.state.stack.is_empty()
                        && g.state.pending_triggers.is_empty()
                    {
                        break;
                    }
                    let response = if matches!(r, Request::OptionalRecursion { .. }) {
                        asked = true;
                        check(g);
                        Response::OptionalRecursion { perform: yes }
                    } else {
                        automatic(&r, &target(old))
                    };
                    if let Request::Targets { ref candidates, .. } = r {
                        assert_eq!(candidates, &vec![old]);
                    }
                    g.respond(r.player(), r.decision_id(), response).unwrap();
                }
                assert_eq!(asked, optional);
                assert_eq!(
                    g.current(object(g, 1)).unwrap().zone,
                    if yes {
                        Zone::Hand { owner: PlayerId(0) }
                    } else {
                        Zone::Graveyard { owner: PlayerId(0) }
                    }
                );
                assert_eq!(
                    object(g, 1).incarnation,
                    if yes {
                        old.incarnation + 1
                    } else {
                        old.incarnation
                    }
                );
            });
        }
    }
    for name in ["Raise Dead", "Gravedigger"] {
        for perform in [false, true] {
            if name == "Raise Dead" && !perform {
                continue;
            }
            for to_command in [false, true] {
                let mut rows = vec![(name, 0, true), ("Isamaru, Hound of Konda", 0, false)];
                rows.extend(lands("Swamp", 4));
                let mut p = graveyard(prepared(&rows), 1);
                p.config.commanders = vec![CardId(1)];
                p.profile_digest = digest(&p.config).unwrap();
                traces(p, |g| {
                    let old = object(g, 1);
                    announce(
                        g,
                        0,
                        if name == "Raise Dead" {
                            target(old)
                        } else {
                            vec![]
                        },
                    );
                    let mut offered = false;
                    for _ in 0..50 {
                        let r = request(g);
                        if matches!(r, Request::Priority { .. })
                            && g.state.stack.is_empty()
                            && g.state.pending_triggers.is_empty()
                        {
                            break;
                        }
                        let response = match r {
                            Request::OptionalRecursion { .. } => {
                                check(g);
                                Response::OptionalRecursion { perform }
                            }
                            Request::CommanderReturn { .. } => {
                                offered = true;
                                check(g);
                                Response::CommanderReturn { to_command }
                            }
                            _ => automatic(&r, &target(old)),
                        };
                        g.respond(r.player(), r.decision_id(), response).unwrap();
                    }
                    assert_eq!(offered, perform);
                    assert_eq!(
                        g.current(object(g, 1)).unwrap().zone,
                        if !perform {
                            Zone::Graveyard { owner: PlayerId(0) }
                        } else if to_command {
                            Zone::Command { owner: PlayerId(0) }
                        } else {
                            Zone::Hand { owner: PlayerId(0) }
                        }
                    );
                });
            }
        }
    }
    let mut rows = vec![("Gravedigger", 0, true)];
    rows.extend(lands("Swamp", 4));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        assert!(g.state.stack.is_empty());
        assert!(g.state.pending_triggers.is_empty());
    });
}

#[test]
fn destroyed_commander_offers_owner_graveyard_return() {
    for (name, land, cost) in [("Murder", "Swamp", 3), ("Day of Judgment", "Plains", 4)] {
        let mut rows = vec![(name, 0, true), ("Isamaru, Hound of Konda", 1, false)];
        rows.extend(lands(land, cost));
        let mut p = prepared(&rows);
        p.config.commanders = vec![CardId(1)];
        p.profile_digest = digest(&p.config).unwrap();
        traces(p, |g| {
            let t = object(g, 1);
            announce(g, 0, if name == "Murder" { target(t) } else { vec![] });
            let mut offered = false;
            for _ in 0..50 {
                let r = request(g);
                if let Request::CommanderReturn { player, .. } = r {
                    offered = true;
                    assert_eq!(player, PlayerId(1));
                    assert_eq!(
                        g.current(object(g, 1)).unwrap().zone,
                        Zone::Graveyard { owner: player }
                    );
                    check(g);
                    g.respond(
                        player,
                        r.decision_id(),
                        Response::CommanderReturn { to_command: true },
                    )
                    .unwrap();
                    break;
                }
                let response = automatic(&r, &[]);
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            assert!(offered);
            resolve(g, vec![]);
            assert_eq!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Command { owner: PlayerId(1) }
            );
        });
    }
}

#[test]
fn draw_life_programs_and_continuous_anthem_layer_7c() {
    for name in ["Night's Whisper", "Sign in Blood"] {
        let mut rows = vec![(name, 0, true)];
        rows.extend(lands("Swamp", 2));
        traces(prepared(&rows), |g| {
            let player = if name == "Sign in Blood" {
                PlayerId(1)
            } else {
                PlayerId(0)
            };
            cast(
                g,
                0,
                if name == "Sign in Blood" {
                    vec![TargetEntity::Player { player }]
                } else {
                    vec![]
                },
            );
            assert_eq!(g.state.players[player.0 as usize].life, 38.into());
            assert_eq!(
                g.state.libraries[&player].len(),
                if player == PlayerId(0) { 1 } else { 2 }
            ); // active draw step already drew one
            assert_eq!(
                g.state.players[if player == PlayerId(0) { 1 } else { 0 }].life,
                40.into()
            );
        });
    }
    let mut rows = vec![
        ("Glorious Anthem", 0, true),
        ("Grizzly Bears", 0, true),
        ("Battlegrowth", 0, true),
        ("Giant Growth", 0, true),
        ("Grizzly Bears", 1, false),
    ];
    rows.extend(lands("Plains", 3));
    rows.extend(lands("Forest", 4));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
        cast(g, 1, vec![]);
        let bear = object(g, 1);
        assert_eq!(g.characteristics(bear).unwrap().power, Some(3.into()));
        assert_eq!(
            g.characteristics(object(g, 4)).unwrap().power,
            Some(2.into())
        );
        cast(g, 2, target(bear));
        cast(g, 3, target(bear));
        let c = g.characteristics(bear).unwrap();
        assert_eq!((c.power, c.toughness), (Some(7.into()), Some(7.into())));
    });
    let mut p = prepared(&[("Glorious Anthem", 1, false), ("Grizzly Bears", 0, false)]);
    p.config.cards[0].controller = PlayerId(0);
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        reach(g, TurnStep::PrecombatMain);
        let bear = object(g, 1);
        assert_eq!(g.characteristics(bear).unwrap().power, Some(3.into()));
        g.concede(PlayerId(1)).unwrap();
        let _ = request(g);
        assert_eq!(g.characteristics(bear).unwrap().power, Some(2.into()));
    });
}
fn opening(g: &mut Game) {
    for _ in 0..20 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) {
            return;
        }
        let response = match r {
            Request::Mulligan { .. } => Response::Mulligan { take: false },
            x => panic!("opening {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("opening declarations")
}
fn search_prepared(name: &str) -> Prepared {
    let catalog = catalog();
    let spell = catalog
        .definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone();
    for seed in 0..128u8 {
        let mut config =
            MatchConfig::for_profile(STAPLES_PROFILE, [seed; 32], PlayerId(0)).unwrap();
        config.decks = std::array::from_fn(|seat| Decklist {
            commander: YARGLE.into(),
            cards: if seat == 0 {
                vec![
                    DeckEntry {
                        oracle_id: YARGLE.into(),
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: FOREST.into(),
                        count: 98,
                    },
                    DeckEntry {
                        oracle_id: spell.clone(),
                        count: 1,
                    },
                ]
            } else {
                vec![
                    DeckEntry {
                        oracle_id: YARGLE.into(),
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: FOREST.into(),
                        count: 99,
                    },
                ]
            },
        });
        let p = Prepared::admit_pod(catalog.clone(), config).unwrap();
        let mut g = Game::new(p.clone());
        opening(&mut g);
        if g.state.objects.values().any(|o| {
            o.zone == (Zone::Hand { owner: PlayerId(0) })
                && g.definition(o.id.card).unwrap().name == name
        }) {
            return p;
        }
    }
    panic!("no reviewed opening seed")
}
fn develop_search(g: &mut Game, name: &str, cost: usize) -> CardId {
    opening(g);
    for _ in 0..1000 {
        let r = request(g);
        if g.active_player() == Some(PlayerId(0))
            && matches!(
                r,
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            )
            && matches!(
                g.turn().step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
        {
            let forests = g
                .state
                .objects
                .values()
                .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(0))
                .count();
            if forests >= cost {
                return g
                    .state
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == (Zone::Hand { owner: PlayerId(0) })
                            && g.definition(o.id.card).unwrap().name == name
                    })
                    .unwrap()
                    .id
                    .card;
            }
            if g.turn().lands_played == 0 {
                let card = g
                    .state
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == (Zone::Hand { owner: PlayerId(0) })
                            && g.definition(o.id.card).unwrap().name == "Forest"
                    })
                    .unwrap()
                    .id
                    .card;
                g.respond(r.player(), r.decision_id(), Response::PlayLand { card })
                    .unwrap();
                continue;
            }
        }
        let response = match &r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates
                    .iter()
                    .filter(|o| g.definition(o.card).unwrap().name != name)
                    .take(*count)
                    .copied()
                    .collect(),
            },
            x => panic!("develop {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("search development bound")
}
#[test]
fn search_zero_one_two_private_handles_shuffle_and_pending_restore() {
    for (name, maximum, cost) in [("Rampant Growth", 1, 2), ("Explosive Vegetation", 2, 4)] {
        let p = search_prepared(name);
        for count in 0..=maximum {
            traces(p.clone(), |g| {
                let spell = develop_search(g, name, cost);
                announce(g, spell.0, vec![]);
                for _ in 0..10 {
                    let r = request(g);
                    if matches!(r, Request::SearchLands { .. }) {
                        break;
                    }
                    g.respond(r.player(), r.decision_id(), Response::Pass)
                        .unwrap();
                }
                let r = request(g);
                let Request::SearchLands {
                    player,
                    candidates,
                    maximum: m,
                    ..
                } = &r
                else {
                    panic!("search choice")
                };
                assert_eq!(*m, maximum);
                assert_eq!(*player, PlayerId(0));
                assert!(candidates.len() > 2);
                check(g); // independently replay from setup to this private choice
                let before = g.state.match_state.as_ref().unwrap().random.word_position;
                let library_before = g.state.libraries[&PlayerId(0)].clone();
                let mut alternate = g.fork();
                alternate
                    .state
                    .libraries
                    .get_mut(&PlayerId(0))
                    .unwrap()
                    .reverse();
                alternate
                    .state
                    .cards
                    .get_mut(&candidates[0].card)
                    .unwrap()
                    .oracle_id = SWAMP.into();
                for seat in 1..4 {
                    assert_eq!(observation(g, seat), observation(&alternate, seat));
                }
                let own = observation(g, 0);
                let Some(PlayerRequest::SearchLands {
                    candidates: visible,
                    ..
                }) = &own.decision
                else {
                    panic!("private request")
                };
                assert_eq!(visible.len(), candidates.len());
                assert!(
                    own.view
                        .objects
                        .iter()
                        .all(|o| !matches!(o.zone, Zone::Library { .. }))
                );
                for seat in 1..4 {
                    let other = observation(g, seat);
                    assert!(other.decision.is_none());
                    assert!(
                        other
                            .view
                            .objects
                            .iter()
                            .all(|o| !matches!(o.zone, Zone::Library { .. }))
                    );
                    for c in visible {
                        assert!(g.resolve_search_handle(PlayerId(seat), &c.handle).is_err());
                    }
                }
                assert!(
                    g.check_response(
                        PlayerId(1),
                        r.decision_id(),
                        Response::SearchLands { objects: vec![] }
                    )
                    .is_err()
                );
                assert!(
                    g.check_response(
                        PlayerId(0),
                        r.decision_id(),
                        Response::SearchLands {
                            objects: vec![candidates[0]; 2]
                        }
                    )
                    .is_err()
                );
                let handles = visible
                    .iter()
                    .take(count as usize)
                    .map(|c| c.handle.clone())
                    .collect::<Vec<_>>();
                let response = PlayerResponse::SearchLands {
                    objects: handles.clone(),
                };
                let mut work = match g.check_player_response(
                    PlayerId(0),
                    own.decision.as_ref().unwrap().token().clone(),
                    response,
                    Budget::new(1),
                ) {
                    QueryResult::Incomplete { work, .. } => work,
                    _ => panic!("query work"),
                };
                let saved = g.save_work(&work).unwrap();
                work = g.restore_work(&saved).unwrap();
                let QueryResult::Complete(QueryValue::Validated(validated)) =
                    g.resume_work(work, Budget::unlimited())
                else {
                    panic!("search validation")
                };
                let found = candidates
                    .iter()
                    .take(count as usize)
                    .copied()
                    .collect::<Vec<_>>();
                g.submit(validated).unwrap();
                resolve(g, vec![]);
                assert_eq!(
                    g.state.libraries[&PlayerId(0)].len(),
                    library_before.len() - count as usize
                );
                assert!(g.state.match_state.as_ref().unwrap().random.word_position.0 > before.0);
                for id in found {
                    let o = g.current(g.object_id(id.card).unwrap()).unwrap();
                    assert_eq!(o.zone, Zone::Battlefield);
                    assert!(o.tapped);
                }
                for h in handles {
                    assert!(g.resolve_search_handle(PlayerId(0), &h).is_err());
                }
                assert_ne!(g.state.libraries[&PlayerId(0)], library_before);
                for seat in 0..4 {
                    assert_eq!(observation(g, seat).view.players[0].library_shuffles, 1);
                }
            });
        }
    }
}
#[test]
fn reviewed_staples_resource_bounds_live_capacity_and_deck_counts() {
    let c = catalog();
    let mut most_creatures = 0;
    let mut most_new_tokens = 0;
    for colors in [vec![Color::W], vec![Color::B, Color::G]] {
        let pool = c
            .definitions
            .iter()
            .filter(|d| {
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|color| colors.contains(color))
            })
            .collect::<Vec<_>>();
        most_creatures = most_creatures.max(
            pool.iter()
                .filter(|d| d.characteristics.kind == CardKind::Creature)
                .count(),
        );
        let created = pool
            .iter()
            .flat_map(|d| d.spell.iter().chain(d.triggered.iter().map(|a| &a.program)))
            .flat_map(|p| &p.instructions)
            .map(|i| match i {
                Instruction::CreateCreatureTokens { count, .. } => *count as usize,
                _ => 0,
            })
            .sum::<usize>();
        most_new_tokens = most_new_tokens.max(created);
        assert!(
            pool.iter()
                .flat_map(|d| &d.spell)
                .all(|p| p.resolution != SpellResolution::Permanent
                    || p.timing == SpellTiming::MainPhase)
        );
    }
    assert_eq!((most_creatures, most_new_tokens), (28, 9));
    assert_eq!(staples::CREATURES, 4 * most_creatures + 256);
    assert_eq!(staples::DAMAGE_BOUND, 736);
    assert_eq!(
        staples::STACK_BOUND,
        400 + 4 + 8 * (4 * most_creatures + 256 + 4 * most_new_tokens) + 4 * most_creatures + 4
    );
    assert_eq!(staples::STACK_BOUND, 3752);
    assert_eq!(staples::EVENT_GROUP_BOUND, 660);
    let config = MatchConfig::for_profile(STAPLES_PROFILE, [0; 32], PlayerId(0)).unwrap();
    assert!(
        config
            .decks
            .iter()
            .all(|d| d.cards.iter().map(|r| r.count).sum::<u32>() == 100)
    );
    assert_eq!(
        config.decks[0]
            .cards
            .iter()
            .find(|c| c.oracle_id == PLAINS)
            .unwrap()
            .count,
        76
    );
    assert_eq!(
        config.decks[1]
            .cards
            .iter()
            .find(|c| c.oracle_id == FOREST)
            .unwrap()
            .count,
        29
    );
    assert_eq!(
        config.decks[1]
            .cards
            .iter()
            .find(|c| c.oracle_id == SWAMP)
            .unwrap()
            .count,
        30
    );
    assert_eq!(
        Prepared::admit_pod(c, config).unwrap().config.cards.len(),
        400
    );
    let mut g = Game::new(prepared(&[("Hornet Queen", 0, false)]));
    let spec = match &g.definition(CardId(0)).unwrap().triggered[0]
        .program
        .instructions[0]
    {
        Instruction::CreateCreatureTokens { token, .. } => token.clone(),
        _ => panic!("token producer"),
    };
    let create = |count| ProposedEvent::CreateCreatureTokens {
        controller: PlayerId(0),
        source: ObjectId {
            card: CardId(0),
            incarnation: 0,
        },
        count,
        token: Box::new(spec.clone()),
    };
    for _ in 0..64 {
        g.commit(create(4), BatchCause::StateBased).unwrap();
    }
    assert_eq!(g.allocated_capacity_count(), 256);
    assert_eq!(g.state.tokens.len(), 256);
    let ceased = token_objects(&g)[0];
    g.commit(
        ProposedEvent::Move {
            object: ceased,
            destination: Zone::Graveyard { owner: PlayerId(0) },
        },
        BatchCause::StateBased,
    )
    .unwrap();
    g.commit(
        ProposedEvent::CeaseToken {
            object: g.object_id(ceased.card).unwrap(),
        },
        BatchCause::StateBased,
    )
    .unwrap();
    g.commit(create(1), BatchCause::StateBased).unwrap();
    assert_eq!(g.state.tokens.len(), 257);
    assert_eq!(g.allocated_capacity_count(), 256);
    assert!(g.object_id(ceased.card).is_none());
    assert!(g.state.tokens.last_key_value().unwrap().0.0 > ceased.card.0);
    assert!(g.commit(create(1), BatchCause::StateBased).is_err());
    assert_eq!(
        g.typed_fault(),
        Some(&RuleFault::TokenCapacityExceeded {
            capacity: 256,
            created: 256,
            requested: 1
        })
    );
    assert_eq!(g.allocated_capacity_count(), 256);
}

#[test]
fn staples_token_capacity_is_a_typed_atomic_replayable_truncation() {
    let mut p = prepared(&[
        ("Raise the Alarm", 0, true),
        ("Plains", 0, false),
        ("Plains", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Raise the Alarm")
        .unwrap();
    let Instruction::CreateCreatureTokens { count, .. } =
        &mut d.spell.as_mut().unwrap().instructions[0]
    else {
        panic!()
    };
    *count = staples::LIVE_TOKEN_CAPACITY + 1;
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        announce(g, 0, vec![]);
        for _ in 0..4 {
            answer(g, Response::Pass);
        }
        assert!(
            matches!(g.advance(Budget::unlimited()),Advance::Fault(ref s) if s=="token capacity exceeded")
        );
        assert_eq!(
            g.typed_fault(),
            Some(&RuleFault::TokenCapacityExceeded {
                capacity: staples::LIVE_TOKEN_CAPACITY,
                created: 0,
                requested: staples::LIVE_TOKEN_CAPACITY + 1
            })
        );
        assert!(g.state.tokens.is_empty());
    });
}

#[test]
fn compiled_pool_lifetime_token_bound_requires_review_on_recursion_changes() {
    // CR603.3d/603.5: Gravedigger targets when its enters trigger is placed,
    // while it is on the battlefield, so it cannot return itself. CR400.7:
    // old incarnation-bound Disfigure/Growth effects do not follow a recast.
    // Singleton construction plus own-graveyard creature-card targets excludes
    // another seat's Gravedigger, and excludes Spore Swarm/other noncreatures.
    let c = catalog();
    let profile = match_profile(STAPLES_PROFILE).unwrap();
    assert_eq!(profile.commanders, &[ISAMARU, YARGLE]);
    fn instructions(p: &SpellProgram) -> Vec<&Instruction> {
        p.instructions
            .iter()
            .chain(p.modes.iter().flat_map(|m| instructions(&m.program)))
            .collect()
    }
    fn tokens(p: &SpellProgram) -> usize {
        instructions(p)
            .iter()
            .map(|i| match i {
                Instruction::CreateCreatureTokens { count, .. } => *count as usize,
                _ => 0,
            })
            .sum()
    }
    // A new instruction can move/recur producers or repeatedly create tokens.
    // Keep the complete reviewed vocabulary explicit: future operators require
    // auditing this lifetime proof rather than disappearing through a wildcard.
    for d in &c.definitions {
        for p in d
            .spell
            .iter()
            .chain(d.triggered.iter().map(|a| &a.program))
            .chain(d.activated.iter().map(|a| &a.program))
        {
            for i in instructions(p) {
                match i {
                    Instruction::SacrificeDamage { .. }
                    | Instruction::EachOpponentSacrifice { .. }
                    | Instruction::ModifySourcePowerToughness { .. }
                    | Instruction::DestroyPermanent { .. }
                    | Instruction::ExilePermanent { .. }
                    | Instruction::ChangeLifeFromTarget { .. }
                    | Instruction::CreateTokensFor { .. }
                    | Instruction::DamageTargetController { .. }
                    | Instruction::AttachEquipment { .. }
                    | Instruction::ReturnAuraFromGraveyard { .. } => {
                        panic!("attachment outside historical staples proof")
                    }
                    Instruction::CreateCreatureTokens { .. }
                    | Instruction::PutPlusOneCounters { .. }
                    | Instruction::ModifyPowerToughness { .. }
                    | Instruction::PreventCombatDamage { .. }
                    | Instruction::PreventNextDamage { .. }
                    | Instruction::ReturnToOwnerHand { .. }
                    | Instruction::PutOnTopOwnerLibrary { .. }
                    | Instruction::TargetPlayerSacrificeCreature { .. }
                    | Instruction::DrawCards { .. }
                    | Instruction::DiscardCards { .. }
                    | Instruction::CounterStackObject { .. }
                    | Instruction::DealDamage { .. }
                    | Instruction::GainLife { .. }
                    | Instruction::LoseLife { .. }
                    | Instruction::DestroyTargetCreature { .. }
                    | Instruction::DestroyAllCreatures { .. }
                    | Instruction::ReturnOwnGraveyardCreatureToHand { .. }
                    | Instruction::SearchBasicLands { .. } => {}
                }
            }
        }
    }
    for id in profile.commanders {
        let d = c.definitions.iter().find(|d| d.oracle_id == *id).unwrap();
        assert!(d.activated.is_empty() && d.triggered.is_empty());
        assert_eq!(
            tokens(d.spell.as_ref().unwrap()),
            0,
            "commanders make no tokens"
        );
    }
    let mut seat_bounds = vec![];
    for colors in [vec![Color::W], vec![Color::B, Color::G]] {
        let pool = c
            .definitions
            .iter()
            .filter(|d| {
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|color| colors.contains(color))
            })
            .collect::<Vec<_>>();
        let mut base = 0;
        let mut largest_creature_entry = 0;
        let mut recursion_cards = vec![];
        for d in pool {
            assert!(
                d.keywords.iter().all(|k| [
                    KeywordAbility::Defender,
                    KeywordAbility::Flying,
                    KeywordAbility::Reach,
                    KeywordAbility::Vigilance,
                    KeywordAbility::Haste,
                    KeywordAbility::Lifelink,
                    KeywordAbility::Deathtouch,
                    KeywordAbility::Trample,
                    KeywordAbility::FirstStrike,
                    KeywordAbility::DoubleStrike,
                    KeywordAbility::Menace
                ]
                .contains(k)),
                "new timing/recursion keyword needs review"
            );
            for a in &d.static_abilities {
                match &a.effect {
                    StaticEffect::ModifyCreatures { .. } => {
                        panic!("grants outside historical staples proof")
                    }
                    StaticEffect::DoubleDamageToPermanentOrPlayer => {}
                    StaticEffect::ControllerCreatureAnthem { power, toughness } => assert!(
                        *power > Quantity::zero() && *toughness > Quantity::zero(),
                        "an enters-time toughness loss needs review"
                    ),
                }
            }
            assert!(d.activated.iter().all(|a| tokens(&a.program) == 0));
            let mut entry_tokens = 0;
            for a in &d.triggered {
                if tokens(&a.program) > 0 {
                    assert_eq!(d.characteristics.kind, CardKind::Creature);
                    assert_eq!(a.condition, TriggerCondition::SelfEntersBattlefield);
                    entry_tokens += tokens(&a.program);
                }
            }
            if d.characteristics.kind == CardKind::Creature {
                assert!(d.characteristics.toughness.as_ref().unwrap() > &Quantity::zero());
                assert_eq!(d.spell.as_ref().unwrap().timing, SpellTiming::MainPhase);
                largest_creature_entry = largest_creature_entry.max(entry_tokens);
            }
            base += entry_tokens + d.spell.as_ref().map_or(0, tokens);
            for p in d
                .spell
                .iter()
                .chain(d.triggered.iter().map(|a| &a.program))
                .chain(d.activated.iter().map(|a| &a.program))
            {
                for i in instructions(p) {
                    match i {
                        Instruction::ReturnOwnGraveyardCreatureToHand { target_slot, .. } => {
                            assert_eq!(
                                p.targets[*target_slot as usize].domain,
                                TargetDomain::OwnGraveyardCreatureCard
                            );
                            recursion_cards.push(d.name.as_str());
                        }
                        Instruction::ReturnToOwnerHand { .. }
                        | Instruction::PutOnTopOwnerLibrary { .. } => {
                            panic!("new repeat-entry route in admitted colors: {}", d.name)
                        }
                        _ => {}
                    }
                }
            }
        }
        recursion_cards.sort();
        assert_eq!(base, 9);
        let recursions = if colors == [Color::W] {
            assert!(recursion_cards.is_empty());
            0
        } else {
            assert_eq!(recursion_cards, vec!["Gravedigger", "Raise Dead"]);
            let raise = c
                .definitions
                .iter()
                .find(|d| d.name == "Raise Dead")
                .unwrap();
            assert_eq!(raise.characteristics.kind, CardKind::Sorcery);
            assert!(matches!(
                raise.spell.as_ref().unwrap().instructions.as_slice(),
                [Instruction::ReturnOwnGraveyardCreatureToHand {
                    optional: false,
                    ..
                }]
            ));
            let digger = c
                .definitions
                .iter()
                .find(|d| d.name == "Gravedigger")
                .unwrap();
            assert_eq!(digger.characteristics.kind, CardKind::Creature);
            assert_eq!(digger.triggered.len(), 1);
            assert_eq!(
                digger.triggered[0].condition,
                TriggerCondition::SelfEntersBattlefield
            );
            assert!(matches!(
                digger.triggered[0].program.instructions.as_slice(),
                [Instruction::ReturnOwnGraveyardCreatureToHand { optional: true, .. }]
            ));
            assert!(digger.activated.is_empty());
            assert_eq!(largest_creature_entry, 4);
            let queen = c
                .definitions
                .iter()
                .find(|d| d.name == "Hornet Queen")
                .unwrap();
            assert_eq!(
                queen
                    .triggered
                    .iter()
                    .map(|a| tokens(&a.program))
                    .sum::<usize>(),
                4
            );
            // Raise Dead either returns a producer directly, or is spent on
            // Gravedigger for one extra enters trigger: TWO recursions total.
            2
        };
        seat_bounds.push(base + recursions * largest_creature_entry);
    }
    assert_eq!(
        c.definitions
            .iter()
            .find(|d| d.name == "Spore Swarm")
            .unwrap()
            .characteristics
            .kind,
        CardKind::Instant
    );
    assert_eq!(seat_bounds, vec![9, 17]);
    assert_eq!(4 * seat_bounds.into_iter().max().unwrap(), 68);
}
#[test]
fn concession_keeps_legal_pending_graveyard_target() {
    // CR 800.4a: another player's departure revalidates pending targets;
    // a creature card in the caster's own graveyard (115.1) stays legal, so the
    // Raise Dead cast continues to its mana payment exactly as without it.
    for concede in [false, true] {
        let mut rows = vec![("Raise Dead", 0, true), ("Grizzly Bears", 0, false)];
        rows.extend(lands("Swamp", 2));
        let mut g = Game::new(graveyard(prepared(&rows), 1));
        reach(&mut g, TurnStep::PrecombatMain);
        let bear = object(&g, 1);
        let raise_dead = object(&g, 0).card;
        answer(&mut g, Response::Cast { card: raise_dead });
        let r = request(&mut g);
        assert!(matches!(r, Request::Targets { .. }), "{r:?}");
        g.respond(
            r.player(),
            r.decision_id(),
            Response::Targets {
                objects: vec![bear],
            },
        )
        .unwrap();
        if concede {
            g.concede(PlayerId(2)).unwrap();
        }
        let r = request(&mut g);
        assert!(
            matches!(r, Request::Mana { .. }),
            "concede={concede}: {r:?}"
        );
        assert_eq!(g.state.stack.len(), 1);
    }
}
