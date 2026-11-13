//! Accepted-pod witness: ordinary setup, public decisions and a paid blocking pump.
use super::*;
use mtg_contracts::TargetDomain;

fn blocking_catalog() -> Catalog {
    let catalog = match_profile("commander-blocking-targets-v1")
        .expect("embedded blocking development profile")
        .catalog()
        .clone();
    assert_eq!(catalog.definitions.len(), 37);
    catalog
}
fn blocking_oracle(name: &str) -> String {
    blocking_catalog()
        .definitions
        .into_iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
}
fn blocking_config(seed: [u8; 32]) -> MatchConfig {
    let commander = blocking_oracle("Isamaru, Hound of Konda");
    let deck = Decklist {
        commander: commander.clone(),
        cards: vec![
            DeckEntry {
                oracle_id: commander,
                count: 1,
            },
            DeckEntry {
                oracle_id: blocking_oracle("Righteousness"),
                count: 1,
            },
            DeckEntry {
                oracle_id: blocking_oracle("Plains"),
                count: 98,
            },
        ],
    };
    let mut config =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|_| deck.clone());
    validate_pod(&blocking_catalog(), &config).unwrap();
    config
}
fn blocking_input(game: &mut Game, response: PlayerResponse, quantum: u64) {
    submit_match(game, response);
    assert!(matches!(
        settle_match(game, quantum),
        Advance::AwaitingDecision(_)
    ));
}
fn blocking_run(seed: [u8; 32], quantum: u64) -> Game {
    let cat = blocking_catalog();
    let commander = blocking_oracle("Isamaru, Hound of Konda");
    let plains = blocking_oracle("Plains");
    let righteousness = blocking_oracle("Righteousness");
    let source = cat.definition_by_oracle_id(&commander).unwrap();
    assert_eq!(source.characteristics.power, Some(Quantity::from(2)));
    assert_eq!(source.characteristics.toughness, Some(Quantity::from(2)));
    let prepared = Prepared::admit_pod(cat, blocking_config(seed)).unwrap();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    assert!(matches!(
        settle_match(&mut game, quantum),
        Advance::AwaitingDecision(_)
    ));
    assert!(
        observation(&game, PlayerId(1))
            .view
            .objects
            .iter()
            .any(|object| {
                object.oracle_id == righteousness
                    && object.zone == (Zone::Hand { owner: PlayerId(1) })
            })
    );
    let mut blocked = false;
    let mut cast = false;
    let mut targeted = false;
    let mut paid = false;
    let mut mana_activations = 0;
    let mut pumped = false;
    let mut combat = false;
    let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Off);
    for _ in 0..1800 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        let defender = view.objects.iter().find(|object| {
            object.oracle_id == commander
                && object.owner == PlayerId(1)
                && object.zone == Zone::Battlefield
        });
        if let Some(defender) = defender {
            let characteristics = defender.characteristics.as_ref().unwrap();
            if cast && characteristics.power == Some(Quantity::from(9)) {
                assert!(blocked && targeted && paid);
                assert_eq!(
                    mana_activations, 1,
                    "Righteousness received exactly one paid W"
                );
                assert_eq!(characteristics.toughness, Some(Quantity::from(9)));
                if !pumped {
                    assert!(
                        view.combat
                            .iter()
                            .any(|attack| attack.blocked
                                && attack.blockers.contains(&defender.handle))
                    );
                    verifier.verify(&game.snapshot().unwrap()).unwrap();
                    pumped = true;
                }
            }
            if pumped
                && defender.marked_damage == Quantity::from(2)
                && view.objects.iter().any(|object| {
                    object.oracle_id == commander
                        && object.owner == PlayerId(0)
                        && object.zone == (Zone::Command { owner: PlayerId(0) })
                })
            {
                assert_eq!(characteristics.power, Some(Quantity::from(9)));
                assert_eq!(characteristics.toughness, Some(Quantity::from(9)));
                assert_eq!(
                    view.players
                        .iter()
                        .find(|p| p.player == PlayerId(1))
                        .unwrap()
                        .life,
                    Quantity::from(40)
                );
                assert!(
                    game.commander_ledger()
                        .history(CardId(0))
                        .unwrap()
                        .combat_damage
                        .is_empty()
                );
                combat = true;
            }
            if combat && characteristics.power == Some(Quantity::from(2)) {
                assert_eq!(characteristics.toughness, Some(Quantity::from(2)));
                assert_eq!(defender.marked_damage, Quantity::zero());
                assert!(
                    view.effects.is_empty(),
                    "the pump expired during ordinary cleanup"
                );
                assert!(
                    view.objects
                        .iter()
                        .any(|object| object.oracle_id == righteousness
                            && object.owner == PlayerId(1)
                            && object.zone == (Zone::Graveyard { owner: PlayerId(1) }))
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
                let main = view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                let lands = view
                    .objects
                    .iter()
                    .filter(|object| {
                        object.controller == actor
                            && object.zone == Zone::Battlefield
                            && object.oracle_id == plains
                    })
                    .collect::<Vec<_>>();
                if actor == PlayerId(1)
                    && blocked
                    && !cast
                    && view.stack.is_empty()
                    && view.turn.step == TurnStep::DeclareBlockers
                    && lands.iter().any(|land| !land.tapped)
                {
                    let card = view
                        .objects
                        .iter()
                        .find(|object| {
                            object.oracle_id == righteousness
                                && object.zone == (Zone::Hand { owner: actor })
                        })
                        .unwrap();
                    verifier.verify(&game.snapshot().unwrap()).unwrap();
                    cast = true;
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else if main && actor.0 < 2 && !combat {
                    if lands.len() < 2
                        && view.turn.lands_played == 0
                        && let Some(land) = view.objects.iter().find(|object| {
                            object.oracle_id == plains
                                && object.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        }
                    } else if lands.iter().any(|land| !land.tapped)
                        && let Some(card) = view.objects.iter().find(|object| {
                            object.oracle_id == commander
                                && object.zone == (Zone::Command { owner: actor })
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
            PlayerRequest::Mana { .. } => {
                let available = view
                    .players
                    .iter()
                    .find(|player| player.player == actor)
                    .unwrap()
                    .mana
                    .get(&Color::W)
                    .cloned()
                    .unwrap_or_default();
                if available == Quantity::zero() {
                    let land = view
                        .objects
                        .iter()
                        .find(|object| {
                            object.controller == actor
                                && object.zone == Zone::Battlefield
                                && object.oracle_id == plains
                                && !object.tapped
                        })
                        .unwrap();
                    if cast && !paid {
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
                    assert_eq!(actor, PlayerId(1));
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: Quantity::zero(),
                            colored: vec![Color::W]
                        }
                    );
                    paid = true;
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
                assert!(cast && !targeted);
                assert_eq!(actor, PlayerId(1));
                assert_eq!(occurrences.len(), 1);
                assert_eq!(
                    occurrences[0].domain,
                    TargetDomain::BlockingBattlefieldCreature
                );
                let target = defender.expect("actual declared blocker");
                let attacker = view
                    .objects
                    .iter()
                    .find(|object| {
                        object.oracle_id == commander
                            && object.owner == PlayerId(0)
                            && object.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(candidates.contains(&target.handle));
                assert!(!candidates.contains(&attacker.handle));
                reject_input(
                    &game,
                    actor,
                    token,
                    PlayerResponse::Targets {
                        objects: vec![attacker.handle.clone()],
                    },
                );
                targeted = true;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let defending_ready = view.objects.iter().any(|object| {
                    object.oracle_id == commander
                        && object.controller == PlayerId(1)
                        && object.zone == Zone::Battlefield
                }) && view.objects.iter().any(|object| {
                    object.oracle_id == plains
                        && object.controller == PlayerId(1)
                        && object.zone == Zone::Battlefield
                        && !object.tapped
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && !blocked && defending_ready {
                        let attacker = view
                            .objects
                            .iter()
                            .find(|object| {
                                object.oracle_id == commander
                                    && object.controller == actor
                                    && object.zone == Zone::Battlefield
                                    && candidates.contains(&object.handle)
                            })
                            .unwrap();
                        vec![(attacker.handle.clone(), PlayerId(1))]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => PlayerResponse::Blockers {
                blocks: if actor == PlayerId(1) && !attackers.is_empty() {
                    assert_eq!(attackers.len(), 1);
                    let blocker = defender.unwrap();
                    assert!(candidates.contains(&blocker.handle));
                    blocked = true;
                    vec![(blocker.handle.clone(), attackers[0].clone())]
                } else {
                    vec![]
                },
            },
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert!(pumped);
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => {
                let mut candidates = candidates;
                candidates.sort_by_key(|handle| {
                    view.objects
                        .iter()
                        .find(|object| object.handle == *handle)
                        .is_some_and(|object| object.oracle_id == righteousness)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected accepted blocking-pump decision {other:?}"),
        };
        blocking_input(&mut game, response, quantum);
    }
    panic!("accepted blocking pump exceeded declared development bound1800")
}
#[test]
fn admitted_blocking_match_casts_paid_righteousness_replays_and_cleans_up_across_quanta() {
    // Separate focused witness, not a completion-batch seed. A bounded
    // SETUP-only search over indices0..255 first found index3 with P1 opening
    // Righteousness. No gameplay outcome was used to select this seed.
    let seed = [
        83, 119, 48, 174, 200, 72, 242, 215, 30, 28, 42, 248, 254, 219, 225, 30, 175, 198, 88, 231,
        103, 208, 222, 15, 57, 125, 66, 59, 124, 78, 119, 151,
    ];
    let hex = mtg_contracts::sha256(b"commander-blocking-targets-v1/opening-righteousness/3");
    assert_eq!(
        seed,
        std::array::from_fn::<_, 32, _>(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
    );
    let small = blocking_run(seed, 1);
    let large = blocking_run(seed, 4096);
    assert_eq!(small.state_digest().unwrap(), large.state_digest().unwrap());
    assert_eq!(small.replay_journal(), large.replay_journal());
}
