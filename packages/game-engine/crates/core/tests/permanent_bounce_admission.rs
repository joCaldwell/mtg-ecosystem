//! Genuine accepted legal-pod prefixes with actual mana, land plays and casts.
//! Setup-only seed selection never executes gameplay or selects an outcome.
use mtg_contracts::{Catalog, Color, ManaCost, Quantity, sha256};
use mtg_core::*;
use std::collections::BTreeMap;

const PROFILE: &str = "commander-sacrifice-v1";
fn catalog() -> Catalog {
    let c = match_profile(PROFILE).unwrap().catalog().clone();
    assert_eq!(c.definitions.len(), 158);
    c
}
fn id(c: &Catalog, name: &str) -> String {
    c.definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
fn config(c: &Catalog, seed: [u8; 32], spell: &str) -> MatchConfig {
    let commander = id(c, "Caelorna, Coral Tyrant");
    let mut config = MatchConfig::for_profile(PROFILE, seed, PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|seat| Decklist {
        commander: commander.clone(),
        cards: if seat == 0 {
            vec![
                DeckEntry {
                    oracle_id: commander.clone(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, spell),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Sol Ring"),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Island"),
                    count: 97,
                },
            ]
        } else {
            vec![
                DeckEntry {
                    oracle_id: commander.clone(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Island"),
                    count: 99,
                },
            ]
        },
    });
    let pod = validate_pod(c, &config).unwrap();
    // Both exact source IDs sort before Island; only their relative order with
    // Ring changes. The two special cards occupy the same physical slot set.
    let mut special = pod
        .physical_config()
        .cards
        .iter()
        .filter(|r| r.id == CardId(1) || r.id == CardId(2))
        .map(|r| r.oracle_id.clone())
        .collect::<Vec<_>>();
    let mut expected = vec![id(c, spell), id(c, "Sol Ring")];
    special.sort();
    expected.sort();
    assert_eq!(special, expected);
    config
}
fn settle(g: &mut Game, quantum: u64) {
    for _ in 0..20000 {
        match g.advance(Budget::new(quantum)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => return,
            other => panic!("permanent bounce pod {other:?}"),
        }
    }
    panic!("automatic work bound");
}
fn query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        match result {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(64)),
            QueryResult::Rejected(e) => panic!("query rejected {e:?}"),
        }
    }
    panic!("query bound");
}
fn observation(g: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(o) = query(g, g.observe(player, Budget::new(10000))) else {
        panic!("observation");
    };
    o
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    let actor = g
        .live_players()
        .into_iter()
        .find(|p| g.decision_for(*p).unwrap().is_some())
        .unwrap();
    let QueryValue::Decision(Some(r)) = query(g, g.player_decision_for(actor, Budget::new(10000)))
    else {
        panic!("decision");
    };
    (actor, r)
}
fn submit(g: &mut Game, response: PlayerResponse, quantum: u64) {
    let (actor, r) = request(g);
    let QueryValue::Validated(receipt) = query(
        g,
        g.check_player_response(actor, r.token().clone(), response, Budget::new(10000)),
    ) else {
        panic!("receipt");
    };
    g.submit(receipt).unwrap();
    settle(g, quantum);
}
fn reject(g: &Game, actor: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = g.snapshot().unwrap();
    let mut result = g.check_player_response(actor, token, response, Budget::new(10000));
    for _ in 0..10000 {
        match result {
            QueryResult::Rejected(_) => {
                assert_eq!(g.snapshot().unwrap(), before);
                return;
            }
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(64)),
            QueryResult::Complete(_) => panic!("illegal land target was accepted"),
        }
    }
    panic!("negative query bound");
}
fn setup(c: &Catalog, seed: [u8; 32], spell: &str, quantum: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c.clone(), config(c, seed, spell)).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    settle(&mut g, quantum);
    for _ in 0..4 {
        assert!(matches!(request(&g).1, PlayerRequest::Mulligan { .. }));
        submit(&mut g, PlayerResponse::Mulligan { take: false }, quantum);
    }
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_eq!(
        observation(&g, PlayerId(0)).view.turn.step,
        TurnStep::Upkeep
    );
    (p, g)
}
fn opening(g: &Game, c: &Catalog, spell: &str) -> bool {
    let view = observation(g, PlayerId(0)).view;
    [id(c, spell), id(c, "Sol Ring")].iter().all(|id| {
        view.objects
            .iter()
            .any(|o| o.oracle_id == *id && o.zone == (Zone::Hand { owner: PlayerId(0) }))
    })
}
fn seed(index: u32) -> [u8; 32] {
    let hex = sha256(format!("commander-sacrifice-v1/opening-permanent-bounce/{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
#[test]
#[ignore = "bounded setup-only opening scan, not gameplay/outcome search"]
fn permanent_bounce_setup_only_seed_probe() {
    let c = catalog();
    for index in 0..2048 {
        let (_, g) = setup(&c, seed(index), "Regress", 4096);
        if opening(&g, &c, "Regress") {
            println!(
                "first opening-only success index={index} seed={:?}",
                seed(index)
            );
            return;
        }
    }
    panic!("no pair opening in predeclared setup-only range0..2048");
}
fn recover(g: &Game, p: &Prepared, verifier: &mut SnapshotVerifier) {
    let snapshot = g.snapshot().unwrap();
    verifier.verify(&snapshot).unwrap();
    assert_eq!(
        Game::restore(p.clone(), &snapshot)
            .unwrap()
            .state_digest()
            .unwrap(),
        g.state_digest().unwrap()
    );
    assert_eq!(
        Game::replay_with_trace(p.clone(), &g.replay_journal(), TraceMode::Off)
            .unwrap()
            .state_digest()
            .unwrap(),
        g.state_digest().unwrap()
    );
}
fn run(seed: [u8; 32], spell: &str, quantum: u64) -> Game {
    let c = catalog();
    let source = id(&c, spell);
    let ring = id(&c, "Sol Ring");
    let island = id(&c, "Island");
    let generic = if spell == "Regress" { 2 } else { 1 };
    let source_definition = c.definition_by_oracle_id(&source).unwrap();
    assert_eq!(
        source_definition.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::from(generic),
            colored: vec![Color::U]
        }
    );
    let (p, mut game) = setup(&c, seed, spell, quantum);
    assert!(
        opening(&game, &c, spell),
        "actual pair must occur in this source's opening"
    );
    let source_card = p
        .config()
        .cards
        .iter()
        .find(|r| r.owner == PlayerId(0) && r.oracle_id == source)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut ring_cast, mut cast, mut paid, mut targeted, mut rejected_land) =
        (false, false, false, false, false);
    let (mut ring_mana, mut blue_mana) = (0, 0);
    let owner = if spell == "Regress" {
        PlayerId(1)
    } else {
        PlayerId(0)
    };
    let mut owner_hand_before = 0;
    for _ in 0..1200 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        if paid
            && view.stack.is_empty()
            && view.objects.iter().any(|o| {
                o.oracle_id == source && o.zone == (Zone::Graveyard { owner: PlayerId(0) })
            })
        {
            assert!(targeted);
            assert_eq!((ring_mana, blue_mana), (1, 1));
            let own = observation(&game, owner).view;
            assert_eq!(
                own.players
                    .iter()
                    .find(|p| p.player == owner)
                    .unwrap()
                    .hand_count,
                owner_hand_before + 1
            );
            let returned = if spell == "Regress" { &island } else { &ring };
            assert!(
                own.objects
                    .iter()
                    .any(|o| o.oracle_id == *returned && o.zone == (Zone::Hand { owner }))
            );
            if spell == "Regress" {
                assert!(!own.objects.iter().any(|o| o.oracle_id == island
                    && o.controller == owner
                    && o.zone == Zone::Battlefield));
            } else {
                assert!(rejected_land);
            }
            let caster = observation(&game, PlayerId(0)).view;
            let mana = &caster
                .players
                .iter()
                .find(|p| p.player == PlayerId(0))
                .unwrap()
                .mana;
            assert_eq!(
                mana.get(&Color::C).cloned().unwrap_or_default(),
                Quantity::from(2 - generic)
            );
            assert_eq!(
                mana.get(&Color::U).cloned().unwrap_or_default(),
                Quantity::zero()
            );
            let stack = game
                .events()
                .iter()
                .find_map(|e| match e {
                    Event::SpellProposed { card, stack_id } if *card == source_card => {
                        Some(*stack_id)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(game.events().iter().any(|e|matches!(e,Event::SpellCompleted { stack_id,resolved:true } if *stack_id == stack)));
            assert_eq!(game.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Moved { movement },.. } if movement.from == Zone::Battlefield && movement.destination == (Zone::Hand { owner }))).count(),1);
            recover(&game, &p, &mut verifier);
            return game;
        }
        let response = match request {
            PlayerRequest::Priority { .. } => {
                let main = view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == island
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                if main && actor.0 < 2 && !cast {
                    if lands.len() < if actor == PlayerId(0) { 2 } else { 1 }
                        && view.turn.lands_played == 0
                        && let Some(land) = view.objects.iter().find(|o| {
                            o.oracle_id == island && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && !ring_cast
                        && lands.iter().any(|o| !o.tapped)
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == ring && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        ring_cast = true;
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && view.objects.iter().any(|o| {
                            o.oracle_id == ring && o.zone == Zone::Battlefield && !o.tapped
                        })
                        && lands.iter().any(|o| !o.tapped)
                        && view.objects.iter().any(|o| {
                            o.oracle_id == island
                                && o.controller == PlayerId(1)
                                && o.zone == Zone::Battlefield
                        })
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == source && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        cast = true;
                        recover(&game, &p, &mut verifier);
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Targets {
                token,
                candidates,
                occurrences,
                ..
            } => {
                assert!(cast && !targeted);
                assert_eq!(occurrences.len(), 1);
                let filter = occurrences[0].filter.as_ref().unwrap();
                assert!(filter.types.is_empty());
                let target = if spell == "Regress" {
                    assert!(filter.non_types.is_empty());
                    view.objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == island
                                && o.controller == PlayerId(1)
                                && o.zone == Zone::Battlefield
                        })
                        .unwrap()
                } else {
                    assert_eq!(filter.non_types, vec![mtg_contracts::CardKind::Land]);
                    let land = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == island
                                && o.controller == PlayerId(0)
                                && o.zone == Zone::Battlefield
                        })
                        .unwrap();
                    assert!(!candidates.contains(&land.handle));
                    reject(
                        &game,
                        actor,
                        token,
                        PlayerResponse::Targets {
                            objects: vec![land.handle.clone()],
                        },
                    );
                    rejected_land = true;
                    view.objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == ring && o.owner == owner && o.zone == Zone::Battlefield
                        })
                        .unwrap()
                };
                assert!(candidates.contains(&target.handle));
                owner_hand_before = view
                    .players
                    .iter()
                    .find(|p| p.player == owner)
                    .unwrap()
                    .hand_count;
                targeted = true;
                recover(&game, &p, &mut verifier);
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let mana = &view
                    .players
                    .iter()
                    .find(|p| p.player == actor)
                    .unwrap()
                    .mana;
                let amount = |c| mana.get(&c).cloned().unwrap_or_default();
                if cast && amount(Color::C) < Quantity::from(generic) {
                    let o = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == ring && o.zone == Zone::Battlefield && !o.tapped)
                        .unwrap();
                    ring_mana += 1;
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else if amount(Color::U)
                    < if cast {
                        Quantity::from(1)
                    } else {
                        cost.generic
                    }
                {
                    let o = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == island
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .unwrap();
                    if cast {
                        blue_mana += 1;
                    }
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana { cost, .. } => {
                if cast {
                    assert_eq!(
                        cost,
                        ManaCost {
                            generic: Quantity::from(generic),
                            colored: vec![Color::U]
                        }
                    );
                    paid = true;
                    PlayerResponse::SpendMana {
                        spending: BTreeMap::from([
                            (Color::C, Quantity::from(generic)),
                            (Color::U, 1.into()),
                        ]),
                    }
                } else {
                    PlayerResponse::SpendMana {
                        spending: BTreeMap::from([(Color::U, 1.into())]),
                    }
                }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count,
                mut candidates,
                ..
            } => {
                candidates.sort_by_key(|h| {
                    view.objects
                        .iter()
                        .find(|o| o.handle == *h)
                        .is_some_and(|o| o.oracle_id == source || o.oracle_id == ring)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected permanent bounce decision {other:?}"),
        };
        submit(&mut game, response, quantum);
        if paid {
            recover(&game, &p, &mut verifier);
        }
    }
    panic!("permanent bounce predeclared1200 decision bound");
}
#[test]
fn permanent_bounce_accepted_pods_pay_regress_and_disperse_with_replay_across_quanta() {
    // First pair in the declared setup-only scan; no gameplay chose this seed.
    let seed = seed(10);
    assert_eq!(
        seed,
        [
            237, 69, 34, 158, 97, 143, 106, 220, 165, 203, 230, 44, 176, 177, 120, 59, 244, 104,
            122, 130, 154, 146, 22, 183, 82, 201, 171, 55, 255, 55, 29, 173
        ]
    );
    for spell in ["Regress", "Disperse"] {
        let a = run(seed, spell, 1);
        let b = run(seed, spell, 4096);
        assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
        assert_eq!(a.replay_journal(), b.replay_journal());
    }
}
