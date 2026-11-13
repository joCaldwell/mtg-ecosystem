//! Accepted-pod prefix: real setup and paid Swift/Burst responses before blockers.
//! This witness stops before combat damage; private genuine-source tests cover
//! the later damage. No source, mana, hand, zone or combat state is injected.
use super::*;

fn defense_catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 39);
    c
}
fn defense_config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let commander = id("Isamaru, Hound of Konda");
    let mut config =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|seat| Decklist {
        commander: commander.clone(),
        cards: vec![
            DeckEntry {
                oracle_id: commander.clone(),
                count: 1,
            },
            DeckEntry {
                oracle_id: id(match seat {
                    0 => "Burst of Energy",
                    2 => "Righteousness",
                    _ => "Swift Response",
                }),
                count: 1,
            },
            DeckEntry {
                oracle_id: id("Plains"),
                count: 98,
            },
        ],
    });
    let normalized = validate_pod(c, &config).unwrap();
    // Commander allocation is forced first; each sole nonbasic library card
    // sorts before Plains. This is the setup proxy's physical slot mapping.
    for (seat, spell) in [
        (0, "Burst of Energy"),
        (1, "Swift Response"),
        (2, "Righteousness"),
        (3, "Swift Response"),
    ] {
        let card = normalized
            .physical_config()
            .cards
            .iter()
            .find(|card| card.id == CardId(seat * 100 + 1))
            .unwrap();
        assert_eq!(card.oracle_id, id(spell));
    }
    config
}
fn completion(game: &Game, card: CardId, expected: bool) {
    let stack = game
        .events()
        .iter()
        .find_map(|event| match event {
            Event::SpellProposed {
                card: proposed,
                stack_id,
            } if *proposed == card => Some(*stack_id),
            _ => None,
        })
        .expect("the actual source was proposed");
    assert!(
        game.events().iter().any(
            |event| matches!(event,Event::SpellCompleted {stack_id,resolved}
        if *stack_id==stack && *resolved==expected)
        ),
        "source {card:?} resolved={expected}"
    );
}
fn run_defense(seed: [u8; 32], quantum: u64) -> Game {
    let c = defense_catalog();
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let commander = id("Isamaru, Hound of Konda");
    let plains = id("Plains");
    let burst = id("Burst of Energy");
    let swift = id("Swift Response");
    let configuration = defense_config(&c, seed);
    let prepared = Prepared::admit_pod(c.clone(), configuration).unwrap();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    assert!(matches!(
        settle_match(&mut game, quantum),
        Advance::AwaitingDecision(_)
    ));
    for (seat, source) in [(PlayerId(0), &burst), (PlayerId(1), &swift)] {
        let own = observation(&game, seat);
        assert_eq!(
            own.view
                .players
                .iter()
                .find(|p| p.player == seat)
                .unwrap()
                .hand_count,
            7
        );
        assert!(
            own.view
                .objects
                .iter()
                .any(|o| &o.oracle_id == source && o.zone == (Zone::Hand { owner: seat })),
            "actual opening source for {seat:?}"
        );
    }
    let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Detailed);
    let (
        mut attacked,
        mut swift_cast,
        mut swift_paid,
        mut burst_cast,
        mut burst_paid,
        mut untapped,
    ) = (false, false, false, false, false, false);
    let (mut swift_mana, mut burst_mana) = (0, 0);
    for _ in 0..1800 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        let attacker = view.objects.iter().find(|o| {
            o.oracle_id == commander && o.owner == PlayerId(0) && o.zone == Zone::Battlefield
        });
        let defender = view.objects.iter().find(|o| {
            o.oracle_id == commander && o.owner == PlayerId(1) && o.zone == Zone::Battlefield
        });
        if burst_paid
            && let Some(attacker) = attacker
            && !attacker.tapped
        {
            assert!(attacked && swift_paid && burst_cast);
            assert!(view.combat.iter().any(|a| a.attacker == attacker.handle));
            assert!(view.objects.iter().any(|o| o.oracle_id == burst
                && o.owner == PlayerId(0)
                && o.zone == (Zone::Graveyard { owner: PlayerId(0) })));
            completion(&game, CardId(1), true);
            if !untapped {
                assert_eq!(
                    view.stack.len(),
                    1,
                    "Burst resolved above the original Swift"
                );
                verifier.verify(&game.snapshot().unwrap()).unwrap();
                untapped = true;
            }
            if untapped && view.stack.is_empty() {
                assert_eq!(
                    view.turn.step,
                    TurnStep::DeclareAttackers,
                    "prefix ends before blockers"
                );
                assert!(view.objects.iter().any(|o| o.oracle_id == swift
                    && o.owner == PlayerId(1)
                    && o.zone == (Zone::Graveyard { owner: PlayerId(1) })));
                completion(&game, CardId(101), false);
                assert_eq!((swift_mana, burst_mana), (2, 1));
                assert!(!defender.expect("surviving untapped defender").tapped);
                assert_eq!(
                    view.players
                        .iter()
                        .find(|p| p.player == PlayerId(1))
                        .unwrap()
                        .life,
                    40.into()
                );
                for (seat, tapped) in [(PlayerId(0), 1), (PlayerId(1), 2)] {
                    assert_eq!(
                        view.objects
                            .iter()
                            .filter(|o| o.oracle_id == plains
                                && o.controller == seat
                                && o.zone == Zone::Battlefield
                                && o.tapped)
                            .count(),
                        tapped
                    );
                    assert!(
                        view.players
                            .iter()
                            .find(|p| p.player == seat)
                            .unwrap()
                            .mana
                            .values()
                            .all(|n| *n == Quantity::zero())
                    );
                }
                assert!(
                    game.commander_ledger()
                        .history(CardId(0))
                        .unwrap()
                        .combat_damage
                        .is_empty()
                );
                assert_eq!(
                    game.events()
                        .iter()
                        .filter(|e| matches!(e,Event::Committed {
                    actual:EventOutcome::Untapped {object},..} if object.card==CardId(0)))
                        .count(),
                    1
                );
                verifier.verify(&game.snapshot().unwrap()).unwrap();
                assert_restore(&game, &prepared);
                assert_replay(&game, &prepared);
                return game;
            }
        }
        let response = match request {
            PlayerRequest::Mulligan { .. } => PlayerResponse::Mulligan { take: false },
            PlayerRequest::Priority { .. } => {
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == plains
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                let main = view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                if actor == PlayerId(1) && attacked && !swift_cast && view.stack.is_empty() {
                    assert_eq!(view.turn.step, TurnStep::DeclareAttackers);
                    assert_eq!(lands.iter().filter(|o| !o.tapped).count(), 2);
                    verifier.verify(&game.snapshot().unwrap()).unwrap();
                    swift_cast = true;
                    PlayerResponse::Cast {
                        card: view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == swift && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap()
                            .handle
                            .clone(),
                    }
                } else if actor == PlayerId(0) && swift_paid && !burst_cast && view.stack.len() == 1
                {
                    assert!(attacker.unwrap().tapped);
                    verifier.verify(&game.snapshot().unwrap()).unwrap();
                    burst_cast = true;
                    PlayerResponse::Cast {
                        card: view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == burst && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap()
                            .handle
                            .clone(),
                    }
                } else if main && actor.0 < 2 && !attacked {
                    if lands.len() < 2
                        && view.turn.lands_played == 0
                        && let Some(land) = view.objects.iter().find(|o| {
                            o.oracle_id == plains && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        }
                    } else if lands.iter().any(|o| !o.tapped)
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == commander && o.zone == (Zone::Command { owner: actor })
                        })
                    {
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
            PlayerRequest::Mana { cost, .. } => {
                let available = view
                    .players
                    .iter()
                    .find(|p| p.player == actor)
                    .unwrap()
                    .mana
                    .get(&Color::W)
                    .cloned()
                    .unwrap_or_default();
                let mut required = cost.generic;
                required.add(&Quantity::from(cost.colored.len() as i64));
                if available < required {
                    let land = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == plains
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .unwrap();
                    if burst_cast && !burst_paid {
                        assert_eq!(actor, PlayerId(0));
                        burst_mana += 1;
                    } else if swift_cast && !swift_paid {
                        assert_eq!(actor, PlayerId(1));
                        swift_mana += 1;
                    }
                    PlayerResponse::ActivateMana {
                        object: land.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => {
                if burst_cast && !burst_paid {
                    assert_eq!(actor, PlayerId(0));
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 0.into(),
                            colored: vec![Color::W]
                        }
                    );
                    burst_paid = true;
                } else if swift_cast && !swift_paid {
                    assert_eq!(actor, PlayerId(1));
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 1.into(),
                            colored: vec![Color::W]
                        }
                    );
                    swift_paid = true;
                }
                PlayerResponse::SpendMana {
                    spending: exact_spending(&cost, &available),
                }
            }
            PlayerRequest::Targets {
                token,
                candidates,
                occurrences,
                ..
            } => {
                assert_eq!(occurrences.len(), 1);
                let target = attacker.expect("same actual attacker remains on battlefield");
                assert!(target.tapped);
                assert!(candidates.contains(&target.handle));
                let filter = occurrences[0].filter.as_ref().unwrap();
                if actor == PlayerId(1) {
                    assert!(swift_cast && !swift_paid && filter.tapped);
                    let own = defender.unwrap();
                    assert!(!own.tapped && !candidates.contains(&own.handle));
                    reject_input(
                        &game,
                        actor,
                        token,
                        PlayerResponse::Targets {
                            objects: vec![own.handle.clone()],
                        },
                    );
                } else {
                    assert_eq!(actor, PlayerId(0));
                    assert!(burst_cast && !burst_paid && !filter.tapped);
                    assert!(filter.types.is_empty());
                }
                verifier.verify(&game.snapshot().unwrap()).unwrap();
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let ready = defender.is_some()
                    && view
                        .objects
                        .iter()
                        .filter(|o| {
                            o.oracle_id == plains
                                && o.controller == PlayerId(1)
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .count()
                        == 2
                    && view.objects.iter().any(|o| {
                        o.oracle_id == plains
                            && o.controller == PlayerId(0)
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && !attacked && ready {
                        let attacker = attacker.unwrap();
                        assert!(candidates.contains(&attacker.handle));
                        attacked = true;
                        vec![(attacker.handle.clone(), PlayerId(1))]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers { .. } => {
                assert!(!attacked, "the accepted prefix must finish before blockers");
                PlayerResponse::Blockers { blocks: vec![] }
            }
            PlayerRequest::Discard {
                count,
                mut candidates,
                ..
            } => {
                candidates.sort_by_key(|h| {
                    view.objects
                        .iter()
                        .find(|o| o.handle == *h)
                        .is_some_and(|o| o.oracle_id == burst || o.oracle_id == swift)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected accepted untap defense {other:?}"),
        };
        submit_match(&mut game, response);
        assert!(matches!(
            settle_match(&mut game, quantum),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("accepted untap defense exceeded predeclared1800 decisions");
}

#[test]
fn admitted_untap_defense_pays_both_spells_and_replays_before_blockers_across_quanta() {
    // First success of the declared0..1023 SETUP-only scan was index102. The
    // probe used the prior37-source catalog and Righteousness as P0's physical
    // slot1 proxy; it performed only4KeepMulligan inputs. This fresh witness
    // qualifies actual Burst/Swift source behavior in the current38-source pod.
    let hex = mtg_contracts::sha256(b"commander-blocking-targets-v1/opening-untap-defense/102");
    assert_eq!(
        hex,
        "8753ff7061bbe60b3e5adff059b6b35f70a6a58b492421651954e29c9d0f678f"
    );
    let seed = std::array::from_fn::<_, 32, _>(|i| {
        u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
    });
    let small = run_defense(seed, 1);
    let large = run_defense(seed, 4096);
    assert_eq!(small.state_digest().unwrap(), large.state_digest().unwrap());
    assert_eq!(small.replay_journal(), large.replay_journal());
}
