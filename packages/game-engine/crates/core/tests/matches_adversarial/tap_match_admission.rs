//! Genuine seeded legal-pod prefix: paid Pressure Point, tap/draw, then combat.
//! No source, hand, shuffle, zone, mana or combat state is injected.
use super::*;

fn tap_catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 39);
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
fn tap_config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let commander = id(c, "Isamaru, Hound of Konda");
    let mut config =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|seat| Decklist {
        commander: commander.clone(),
        cards: if seat == 0 {
            vec![
                DeckEntry {
                    oracle_id: commander.clone(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Pressure Point"),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Plains"),
                    count: 98,
                },
            ]
        } else {
            vec![
                DeckEntry {
                    oracle_id: commander.clone(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: id(c, "Plains"),
                    count: 99,
                },
            ]
        },
    });
    validate_pod(c, &config).unwrap();
    config
}
fn tap_seed(index: u32) -> [u8; 32] {
    let hex = mtg_contracts::sha256(
        format!("commander-blocking-targets-v1/opening-pressure-point/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn tap_setup(c: &Catalog, seed: [u8; 32], quantum: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c.clone(), tap_config(c, seed)).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    assert!(matches!(
        settle_match(&mut g, quantum),
        Advance::AwaitingDecision(_)
    ));
    for _ in 0..4 {
        assert!(matches!(request(&g).1, PlayerRequest::Mulligan { .. }));
        submit_match(&mut g, PlayerResponse::Mulligan { take: false });
        assert!(matches!(
            settle_match(&mut g, quantum),
            Advance::AwaitingDecision(_)
        ));
    }
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_eq!(
        observation(&g, PlayerId(0)).view.turn.step,
        TurnStep::Upkeep
    );
    (p, g)
}
fn tap_opening(g: &Game, c: &Catalog) -> bool {
    observation(g, PlayerId(0)).view.objects.iter().any(|o| {
        o.oracle_id == id(c, "Pressure Point") && o.zone == (Zone::Hand { owner: PlayerId(0) })
    })
}
#[test]
#[ignore = "bounded setup-only opening probe; no gameplay or outcome selection"]
fn pressure_point_setup_only_seed_probe() {
    let c = tap_catalog();
    for index in 0..1024 {
        let (_, g) = tap_setup(&c, tap_seed(index), 4096);
        if tap_opening(&g, &c) {
            println!(
                "first opening-only success index={index} seed={:?}",
                tap_seed(index)
            );
            return;
        }
    }
    panic!("no Pressure opening in predeclared setup-only range0..1024");
}
fn run_tap(seed: [u8; 32], quantum: u64) -> Game {
    let c = tap_catalog();
    let commander = id(&c, "Isamaru, Hound of Konda");
    let plains = id(&c, "Plains");
    let pressure = id(&c, "Pressure Point");
    let source = c.definition_by_oracle_id(&pressure).unwrap();
    assert_eq!(
        source.characteristics.mana_cost,
        mtg_contracts::ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        }
    );
    assert!(
        matches!(source.spell.as_ref().unwrap().instructions.as_slice(), [mtg_contracts::Instruction::TapPermanent { target_slot: 0, .. }, mtg_contracts::Instruction::DrawCards { count, recipient: None, .. }] if *count == Quantity::from(1))
    );
    let (prepared, mut game) = tap_setup(&c, seed, quantum);
    assert!(
        tap_opening(&game, &c),
        "pinned setup seed must really deal Pressure Point"
    );
    let source_card = prepared
        .config()
        .cards
        .iter()
        .find(|row| row.oracle_id == pressure && row.owner == PlayerId(0))
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Detailed);
    let (mut cast, mut paid, mut targeted, mut draw_proved, mut attacked, mut bad_block_proved) =
        (false, false, false, false, false, false);
    let mut mana_activations = 0;
    let (mut hand_before, mut library_before) = (0, 0);
    for _ in 0..1400 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        if paid
            && !draw_proved
            && view.stack.is_empty()
            && view.objects.iter().any(|o| {
                o.oracle_id == pressure && o.zone == (Zone::Graveyard { owner: PlayerId(0) })
            })
        {
            let own = observation(&game, PlayerId(0)).view;
            let player = own
                .players
                .iter()
                .find(|p| p.player == PlayerId(0))
                .unwrap();
            assert_eq!(player.library_count, library_before - 1);
            assert_eq!(
                player.hand_count, hand_before,
                "cast consumes one card and actual draw restores it"
            );
            assert_eq!(mana_activations, 2);
            assert_eq!(
                player
                    .mana
                    .values()
                    .cloned()
                    .fold(Quantity::zero(), |mut a, b| {
                        a.add(&b);
                        a
                    }),
                Quantity::zero()
            );
            assert!(own.objects.iter().any(|o| o.oracle_id == commander
                && o.owner == PlayerId(1)
                && o.zone == Zone::Battlefield
                && o.tapped));
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
            assert_eq!(game.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if object.card == CardId(100))).count(),1);
            verifier.verify(&game.snapshot().unwrap()).unwrap();
            assert_restore(&game, &prepared);
            assert_replay(&game, &prepared);
            draw_proved = true;
        }
        if bad_block_proved
            && view
                .players
                .iter()
                .find(|p| p.player == PlayerId(1))
                .unwrap()
                .life
                == Quantity::from(38)
        {
            assert!(cast && paid && targeted && draw_proved && attacked);
            assert_eq!(view.turn.step, TurnStep::CombatDamage);
            verifier.verify(&game.snapshot().unwrap()).unwrap();
            assert_restore(&game, &prepared);
            assert_replay(&game, &prepared);
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
                        o.oracle_id == plains
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                if main && actor.0 < 2 && !cast {
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
                    } else if actor == PlayerId(0)
                        && lands.iter().filter(|o| !o.tapped).count() == 2
                        && view.objects.iter().any(|o| {
                            o.oracle_id == commander
                                && o.owner == PlayerId(1)
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == pressure && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        let player = view.players.iter().find(|p| p.player == actor).unwrap();
                        hand_before = player.hand_count;
                        library_before = player.library_count;
                        cast = true;
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
                    if cast && !paid {
                        assert_eq!(actor, PlayerId(0));
                        mana_activations += 1;
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
                if cast && !paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 1.into(),
                            colored: vec![Color::W]
                        }
                    );
                    paid = true;
                }
                PlayerResponse::SpendMana {
                    spending: exact_spending(&cost, &available),
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                assert!(cast && !targeted);
                let target = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == commander
                            && o.owner == PlayerId(1)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(!target.tapped && candidates.contains(&target.handle));
                targeted = true;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let attacker = view.objects.iter().find(|o| {
                    o.oracle_id == commander
                        && o.owner == PlayerId(0)
                        && o.zone == Zone::Battlefield
                        && candidates.contains(&o.handle)
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && draw_proved && !attacked {
                        attacked = true;
                        vec![(attacker.unwrap().handle.clone(), PlayerId(1))]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers {
                token,
                candidates,
                attackers,
                ..
            } => {
                if actor == PlayerId(1) && attacked {
                    assert_eq!(attackers.len(), 1);
                    let defender = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == commander
                                && o.owner == actor
                                && o.zone == Zone::Battlefield
                        })
                        .unwrap();
                    assert!(defender.tapped && !candidates.contains(&defender.handle));
                    reject_input(
                        &game,
                        actor,
                        token,
                        PlayerResponse::Blockers {
                            blocks: vec![(defender.handle.clone(), attackers[0].clone())],
                        },
                    );
                    bad_block_proved = true;
                }
                PlayerResponse::Blockers { blocks: vec![] }
            }
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
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
                        .is_some_and(|o| o.oracle_id == pressure)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected tap match request {other:?}"),
        };
        submit_match(&mut game, response);
        assert!(matches!(
            settle_match(&mut game, quantum),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("tap match predeclared1400 decision bound");
}
#[test]
fn admitted_tap_match_pays_pressure_draws_and_prevents_blocking_with_replay() {
    // First actual Pressure opening in the declared setup-only0..1024 scan;
    // gameplay and its outcome did not participate in seed selection.
    let seed = tap_seed(13);
    assert_eq!(
        seed,
        [
            14, 177, 32, 165, 137, 99, 200, 221, 186, 137, 82, 202, 203, 83, 250, 41, 225, 0, 87,
            63, 132, 102, 251, 93, 81, 7, 199, 96, 131, 64, 240, 198
        ]
    );
    let a = run_tap(seed, 1);
    let b = run_tap(seed, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
