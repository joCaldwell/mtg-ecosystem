//! Accepted legal pods, public checked decisions and actual paid tapped-target
//! destruction. No source, zone, mana, shuffle or combat state is injected.
use super::*;

fn tapped_catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 40);
    c
}
fn tapped_config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let commander = id("Isamaru, Hound of Konda");
    let deck = Decklist {
        commander: commander.clone(),
        cards: vec![
            DeckEntry {
                oracle_id: commander,
                count: 1,
            },
            DeckEntry {
                oracle_id: id("Swift Response"),
                count: 1,
            },
            DeckEntry {
                oracle_id: id("Plains"),
                count: 98,
            },
        ],
    };
    let mut config =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    config.decks = std::array::from_fn(|_| deck.clone());
    validate_pod(c, &config).unwrap();
    config
}
fn run_tapped_pod(seed: [u8; 32], quantum: u64) -> Game {
    let catalog = tapped_catalog();
    let id = |name: &str| {
        catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let commander = id("Isamaru, Hound of Konda");
    let plains = id("Plains");
    let swift = id("Swift Response");
    let source = catalog.definition_by_oracle_id(&swift).unwrap();
    assert_eq!(
        source.characteristics.mana_cost,
        mtg_contracts::ManaCost {
            generic: 1.into(),
            colored: vec![Color::W],
        }
    );
    assert!(
        source.spell.as_ref().unwrap().targets[0]
            .filter
            .as_ref()
            .unwrap()
            .tapped
    );
    assert!(matches!(
        source.spell.as_ref().unwrap().instructions.as_slice(),
        [mtg_contracts::Instruction::DestroyPermanent {
            optional: false,
            ..
        }]
    ));
    let prepared = Prepared::admit_pod(catalog.clone(), tapped_config(&catalog, seed)).unwrap();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    assert!(matches!(
        settle_match(&mut game, quantum),
        Advance::AwaitingDecision(_)
    ));
    // The retained setup seed places the sole nonbasic library card in P1's
    // real opening hand. Assert the new source rather than assuming its name.
    assert!(
        observation(&game, PlayerId(1))
            .view
            .objects
            .iter()
            .any(|o| o.oracle_id == swift && o.zone == (Zone::Hand { owner: PlayerId(1) }))
    );
    let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Off);
    let (mut attacked, mut cast, mut targeted, mut paid) = (false, false, false, false);
    let mut mana_activations = 0;
    for _ in 0..1600 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        if paid
            && view.objects.iter().any(|o| {
                o.oracle_id == swift
                    && o.owner == PlayerId(1)
                    && o.zone == (Zone::Graveyard { owner: PlayerId(1) })
            })
            && view.objects.iter().any(|o| {
                o.oracle_id == commander
                    && o.owner == PlayerId(0)
                    && o.zone == (Zone::Command { owner: PlayerId(0) })
            })
        {
            assert!(attacked && cast && targeted);
            assert_eq!(
                mana_activations, 2,
                "printed1W paid by two actual Plains activations"
            );
            assert_eq!(
                view.players
                    .iter()
                    .find(|p| p.player == PlayerId(1))
                    .unwrap()
                    .life,
                40.into()
            );
            assert!(view.objects.iter().any(|o| o.oracle_id == commander
                && o.owner == PlayerId(1)
                && o.zone == Zone::Battlefield
                && !o.tapped));
            assert_eq!(
                view.objects
                    .iter()
                    .filter(|o| o.oracle_id == plains
                        && o.controller == PlayerId(1)
                        && o.zone == Zone::Battlefield
                        && o.tapped)
                    .count(),
                2
            );
            assert!(
                game.commander_ledger()
                    .history(CardId(0))
                    .unwrap()
                    .combat_damage
                    .is_empty()
            );
            verifier.verify(&game.snapshot().unwrap()).unwrap();
            assert_restore(&game, &prepared);
            assert_replay(&game, &prepared);
            return game;
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
                if actor == PlayerId(1)
                    && attacked
                    && !cast
                    && view.stack.is_empty()
                    && view.turn.step == TurnStep::DeclareAttackers
                {
                    assert_eq!(lands.iter().filter(|o| !o.tapped).count(), 2);
                    let card = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == swift && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    verifier.verify(&game.snapshot().unwrap()).unwrap();
                    cast = true;
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
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
                    if cast && !paid {
                        assert_eq!(actor, PlayerId(1));
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
            PlayerRequest::Targets {
                token,
                candidates,
                occurrences,
                ..
            } => {
                assert!(attacked && cast && !targeted);
                assert_eq!(actor, PlayerId(1));
                assert_eq!(occurrences.len(), 1);
                let filter = occurrences[0].filter.as_ref().unwrap();
                assert!(filter.tapped);
                assert_eq!(filter.types, vec![CardKind::Creature]);
                let target = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == commander
                            && o.owner == PlayerId(0)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                let own = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == commander
                            && o.owner == PlayerId(1)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(target.tapped);
                assert!(!own.tapped);
                assert!(candidates.contains(&target.handle));
                assert!(!candidates.contains(&own.handle));
                reject_input(
                    &game,
                    actor,
                    token,
                    PlayerResponse::Targets {
                        objects: vec![own.handle.clone()],
                    },
                );
                verifier.verify(&game.snapshot().unwrap()).unwrap();
                targeted = true;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let ready = view.objects.iter().any(|o| {
                    o.oracle_id == commander
                        && o.controller == PlayerId(1)
                        && o.zone == Zone::Battlefield
                }) && view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == plains
                            && o.controller == PlayerId(1)
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    })
                    .count()
                    == 2;
                let target = view.objects.iter().find(|o| {
                    o.oracle_id == commander
                        && o.controller == PlayerId(0)
                        && o.zone == Zone::Battlefield
                        && candidates.contains(&o.handle)
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && !attacked && ready {
                        attacked = true;
                        vec![(
                            target.expect("ready real attacker").handle.clone(),
                            PlayerId(1),
                        )]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers { .. } => {
                assert!(!attacked, "Swift must remove the attacker before blockers");
                PlayerResponse::Blockers { blocks: vec![] }
            }
            PlayerRequest::CommanderReturn { .. } => {
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Discard {
                count,
                mut candidates,
                ..
            } => {
                candidates.sort_by_key(|handle| {
                    view.objects
                        .iter()
                        .find(|o| o.handle == *handle)
                        .is_some_and(|o| o.oracle_id == swift)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected accepted tapped-target request {other:?}"),
        };
        submit_match(&mut game, response);
        assert!(matches!(
            settle_match(&mut game, quantum),
            Advance::AwaitingDecision(_)
        ));
        if paid {
            verifier.verify(&game.snapshot().unwrap()).unwrap();
        }
    }
    panic!("accepted tapped-target witness exceeded predeclared1600 decisions");
}

#[test]
fn admitted_tapped_match_casts_paid_swift_response_and_replays_across_quanta() {
    // Reuse the earlier setup-qualified seed, not a new gameplay/outcome search.
    // Swift and Righteousness both sort before Plains, so the sole nonbasic
    // library card occupies the same physical slot; actual opening is checked.
    let hex = mtg_contracts::sha256(b"commander-blocking-targets-v1/opening-righteousness/3");
    let seed = std::array::from_fn::<_, 32, _>(|i| {
        u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
    });
    let small = run_tapped_pod(seed, 1);
    let large = run_tapped_pod(seed, 4096);
    assert_eq!(small.state_digest().unwrap(), large.state_digest().unwrap());
    assert_eq!(small.replay_journal(), large.replay_journal());
}
