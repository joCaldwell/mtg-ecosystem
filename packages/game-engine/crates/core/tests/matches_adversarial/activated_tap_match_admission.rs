//! Legal-pod prefix: cast Icy for four, later pay one plus tap, then attack.
//! No hand, mana, untap, zone or combat state is injected.
use super::*;

fn icy_catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 46);
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
fn icy_config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
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
                    oracle_id: id(c, "Icy Manipulator"),
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
fn icy_seed(index: u32) -> [u8; 32] {
    let hex = mtg_contracts::sha256(
        format!("commander-blocking-targets-v1/opening-icy-manipulator/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn icy_setup(c: &Catalog, seed: [u8; 32], quantum: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c.clone(), icy_config(c, seed)).unwrap();
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
fn icy_opening(g: &Game, c: &Catalog) -> bool {
    observation(g, PlayerId(0)).view.objects.iter().any(|o| {
        o.oracle_id == id(c, "Icy Manipulator") && o.zone == (Zone::Hand { owner: PlayerId(0) })
    })
}
#[test]
#[ignore = "bounded setup-only opening scan, never gameplay/outcome search"]
fn activated_tap_setup_only_seed_probe() {
    let c = icy_catalog();
    for index in 0..1024 {
        let (_, g) = icy_setup(&c, icy_seed(index), 4096);
        if icy_opening(&g, &c) {
            println!(
                "first opening-only success index={index} seed={:?}",
                icy_seed(index)
            );
            return;
        }
    }
    panic!("no Icy opening in predeclared setup-only range0..1024");
}
fn recover(g: &Game, p: &Prepared, verifier: &mut SnapshotVerifier) {
    verifier.verify(&g.snapshot().unwrap()).unwrap();
    assert_restore(g, p);
    assert_replay(g, p);
}
fn run_icy(seed: [u8; 32], quantum: u64) -> Game {
    let c = icy_catalog();
    let icy = id(&c, "Icy Manipulator");
    let plains = id(&c, "Plains");
    let commander = id(&c, "Isamaru, Hound of Konda");
    let definition = c.definition_by_oracle_id(&icy).unwrap();
    assert_eq!(
        definition.characteristics.mana_cost,
        mtg_contracts::ManaCost {
            generic: 4.into(),
            colored: vec![]
        }
    );
    assert!(
        matches!(&definition.activated[0].cost,mtg_contracts::ActivationCost::Composite { costs } if costs.mana == Some(mtg_contracts::ManaCost { generic:1.into(),colored:vec![] }) && costs.tap && !costs.sacrifice)
    );
    let (p, mut game) = icy_setup(&c, seed, quantum);
    assert!(
        icy_opening(&game, &c),
        "pinned setup must actually deal Icy"
    );
    let icy_card = p
        .config()
        .cards
        .iter()
        .find(|row| row.owner == PlayerId(0) && row.oracle_id == icy)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (
        mut cast,
        mut cast_paid,
        mut activated,
        mut ability_paid,
        mut targeted,
        mut tap_proved,
        mut attacked,
        mut block_rejected,
    ) = (false, false, false, false, false, false, false, false);
    let (mut cast_mana, mut ability_mana, mut cast_turn) = (0, 0, 0);
    for _ in 0..2400 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        if ability_paid && !tap_proved && view.stack.is_empty() {
            assert!(cast && cast_paid && activated && targeted);
            assert_eq!((cast_mana, ability_mana), (4, 1));
            assert!(view.objects.iter().any(|o| o.oracle_id == icy
                && o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.tapped));
            assert!(view.objects.iter().any(|o| o.oracle_id == commander
                && o.owner == PlayerId(1)
                && o.zone == Zone::Battlefield
                && o.tapped));
            let player = view
                .players
                .iter()
                .find(|p| p.player == PlayerId(0))
                .unwrap();
            assert!(player.mana.values().all(|n| *n == Quantity::zero()));
            let spell = game
                .events()
                .iter()
                .find_map(|e| match e {
                    Event::SpellProposed { card, stack_id } if *card == icy_card => Some(*stack_id),
                    _ => None,
                })
                .unwrap();
            assert!(game.events().iter().any(|e|matches!(e,Event::SpellCompleted { stack_id,resolved:true } if *stack_id == spell)));
            let ability = game
                .events()
                .iter()
                .find_map(|e| match e {
                    Event::AbilityActivated {
                        stack_id,
                        source,
                        ability: 0,
                        ..
                    } if source.card == icy_card => Some(*stack_id),
                    _ => None,
                })
                .unwrap();
            assert!(
                game.events().iter().any(
                    |e| matches!(e,Event::AbilityCompleted { stack_id } if *stack_id == ability)
                )
            );
            for card in [icy_card, CardId(100)] {
                assert_eq!(game.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if object.card == card)).count(),1);
            }
            recover(&game, &p, &mut verifier);
            tap_proved = true;
        }
        if block_rejected
            && view
                .players
                .iter()
                .find(|p| p.player == PlayerId(1))
                .unwrap()
                .life
                == Quantity::from(38)
        {
            assert!(tap_proved && attacked);
            assert_eq!(view.turn.step, TurnStep::CombatDamage);
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
                        o.oracle_id == plains
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                if main && actor.0 < 2 && !activated {
                    if lands.len() < if actor == PlayerId(0) { 4 } else { 1 }
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
                        && !cast
                        && lands.iter().filter(|o| !o.tapped).count() == 4
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| o.oracle_id == icy && o.zone == (Zone::Hand { owner: actor }))
                            .unwrap();
                        cast = true;
                        cast_turn = view.turn.number;
                        recover(&game, &p, &mut verifier);
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && cast_paid
                        && lands.iter().any(|o| !o.tapped)
                        && view.objects.iter().any(|o| {
                            o.oracle_id == commander
                                && o.owner == PlayerId(1)
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        && let Some(source) = view.objects.iter().find(|o| {
                            o.oracle_id == icy && o.zone == Zone::Battlefield && !o.tapped
                        })
                    {
                        assert!(
                            view.turn.number > cast_turn,
                            "activation waits for a later real untap turn"
                        );
                        activated = true;
                        recover(&game, &p, &mut verifier);
                        PlayerResponse::ActivateAbility {
                            object: source.handle.clone(),
                            ability: 0,
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
                    if activated && !ability_paid {
                        ability_mana += 1;
                    } else if cast && !cast_paid {
                        cast_mana += 1;
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
                if activated && !ability_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 1.into(),
                            colored: vec![]
                        }
                    );
                    ability_paid = true;
                } else if cast && !cast_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 4.into(),
                            colored: vec![]
                        }
                    );
                    cast_paid = true;
                }
                PlayerResponse::SpendMana {
                    spending: exact_spending(&cost, &available),
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                assert!(activated && !targeted);
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
                    attacks: if actor == PlayerId(0) && tap_proved && !attacked {
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
                    block_rejected = true;
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
                        .is_some_and(|o| o.oracle_id == icy)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected activated tap pod request {other:?}"),
        };
        submit_match(&mut game, response);
        assert!(matches!(
            settle_match(&mut game, quantum),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("activated tap pod predeclared2400 decision bound");
}
#[test]
fn admitted_activated_tap_match_pays_cast_and_activation_then_prevents_blocking() {
    // First actual Icy opening in the declared setup-only scan.
    let seed = icy_seed(0);
    assert_eq!(
        seed,
        [
            63, 235, 83, 41, 199, 95, 30, 94, 172, 186, 50, 116, 248, 161, 5, 20, 70, 68, 222, 127,
            27, 171, 172, 47, 101, 167, 178, 0, 154, 55, 85, 242
        ]
    );
    let a = run_icy(seed, 1);
    let b = run_icy(seed, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
