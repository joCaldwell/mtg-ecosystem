//! Legal pods: real Temporal casting/aging/payment and Commander owner choices.
//! The hand branch recasts on the owner's own main phase, for printed cost.
//! No opening, mana, readiness/tap, stack or movement state is injected.
use super::royal_match_admission::{oracle, recover, setup};
use super::*;

fn catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 47);
    c
}
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let mut m =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    m.decks = std::array::from_fn(|seat| {
        let cmd = oracle(
            c,
            if seat == 0 {
                "Ertai, Wizard Adept"
            } else {
                "Isamaru, Hound of Konda"
            },
        );
        Decklist {
            commander: cmd.clone(),
            cards: if seat == 0 {
                vec![
                    DeckEntry {
                        oracle_id: cmd,
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: oracle(c, "Temporal Adept"),
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: oracle(c, "Island"),
                        count: 98,
                    },
                ]
            } else {
                vec![
                    DeckEntry {
                        oracle_id: cmd,
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: oracle(c, "Plains"),
                        count: 99,
                    },
                ]
            },
        }
    });
    validate_pod(c, &m).unwrap();
    m
}
fn seed(index: u32) -> [u8; 32] {
    let hex = mtg_contracts::sha256(
        format!("commander-blocking-targets-v1/opening-temporal-adept/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn opening(g: &Game, c: &Catalog) -> bool {
    observation(g, PlayerId(0)).view.objects.iter().any(|o| {
        o.oracle_id == oracle(c, "Temporal Adept") && o.zone == (Zone::Hand { owner: PlayerId(0) })
    })
}
#[test]
#[ignore = "bounded setup-only Temporal opening scan; never gameplay/outcome search"]
fn permanent_return_setup_only_seed_probe() {
    let c = catalog();
    for index in 0..1024 {
        let (_, g) = setup(c.clone(), config(&c, seed(index)), 4096);
        if opening(&g, &c) {
            println!(
                "first opening-only success index={index} seed={:?}",
                seed(index)
            );
            return;
        }
    }
    panic!("no Temporal opening in predeclared setup-only0..1024");
}
fn run(seed: [u8; 32], to_command: bool, q: u64) -> Game {
    let c = catalog();
    let temporal = oracle(&c, "Temporal Adept");
    let ertai = oracle(&c, "Ertai, Wizard Adept");
    let isamaru = oracle(&c, "Isamaru, Hound of Konda");
    let island = oracle(&c, "Island");
    let plains = oracle(&c, "Plains");
    let definition = c.definition_by_oracle_id(&temporal).unwrap();
    assert_eq!(
        definition.characteristics.mana_cost,
        mtg_contracts::ManaCost {
            generic: 1.into(),
            colored: vec![Color::U, Color::U]
        }
    );
    assert!(
        matches!(&definition.activated[0].cost,mtg_contracts::ActivationCost::Composite{costs}if costs.mana==Some(mtg_contracts::ManaCost{generic:0.into(),colored:vec![Color::U,Color::U,Color::U]})&&costs.tap&&!costs.sacrifice)
    );
    let (p, mut g) = setup(c.clone(), config(&c, seed), q);
    assert!(opening(&g, &c));
    let source_card = p
        .config()
        .cards
        .iter()
        .find(|row| row.oracle_id == temporal)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (
        mut cast,
        mut paid,
        mut sick,
        mut target_cast,
        mut target_paid,
        mut activated,
        mut ability_paid,
        mut targeted,
        mut choice_made,
        mut returned,
        mut recast,
        mut recast_paid,
    ) = (
        false, false, false, false, false, false, false, false, false, false, false, false,
    );
    let (mut cast_mana, mut ability_mana, mut command_mana, mut recast_mana, mut cast_turn) =
        (0, 0, 0, 0, 0);
    for _ in 0..3400 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        if paid
            && !sick
            && view.stack.is_empty()
            && view
                .objects
                .iter()
                .any(|o| o.oracle_id == temporal && o.zone == Zone::Battlefield)
        {
            assert_eq!(actor, PlayerId(0));
            assert_eq!(
                view.objects
                    .iter()
                    .filter(|o| o.oracle_id == island
                        && o.controller == actor
                        && o.zone == Zone::Battlefield
                        && !o.tapped)
                    .count(),
                3
            );
            assert!(view.objects.iter().any(|o| o.oracle_id == isamaru
                && o.owner == PlayerId(1)
                && o.zone == Zone::Battlefield));
            let raw = g.decision_for(actor).unwrap().unwrap();
            let source = g.object_id(source_card).unwrap();
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    actor,
                    raw.decision_id(),
                    Response::ActivateAbility {
                        object: source,
                        ability: 0
                    }
                )
                .unwrap_err()
                .contains("continuously controlled")
            );
            assert_eq!(g.snapshot().unwrap(), before);
            sick = true;
            recover(&g, &p, &mut verifier);
        }
        if choice_made && !returned && view.stack.is_empty() {
            assert!(
                cast && paid
                    && sick
                    && target_cast
                    && target_paid
                    && activated
                    && ability_paid
                    && targeted
            );
            assert_eq!((cast_mana, ability_mana, command_mana), (3, 3, 1));
            let expected = if to_command {
                Zone::Command { owner: PlayerId(1) }
            } else {
                Zone::Hand { owner: PlayerId(1) }
            };
            let own = observation(&g, PlayerId(1)).view;
            assert!(
                own.objects.iter().any(|o| o.oracle_id == isamaru
                    && o.owner == PlayerId(1)
                    && o.zone == expected)
            );
            assert!(view.objects.iter().any(|o| o.oracle_id == temporal
                && o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.tapped));
            assert_eq!(
                g.commander_ledger()
                    .history(CardId(100))
                    .unwrap()
                    .command_casts
                    .get(&PlayerId(1)),
                Some(&Quantity::from(1))
            );
            assert_eq!(
                g.commander_ledger().tax(CardId(100), PlayerId(1)).unwrap(),
                2.into()
            );
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed{actual:EventOutcome::Tapped{object},..}if object.card==source_card)).count(),1);
            let ability = g
                .events()
                .iter()
                .find_map(|event| match event {
                    Event::AbilityActivated {
                        stack_id,
                        source,
                        ability: 0,
                        ..
                    } if source.card == source_card => Some(*stack_id),
                    _ => None,
                })
                .unwrap();
            assert!(g.events().iter().any(|event| matches!(event, Event::AbilityCompleted { stack_id } if *stack_id == ability)));
            assert_eq!(g.events().iter().filter(|event| matches!(event, Event::Committed { actual: EventOutcome::Moved { movement }, .. } if movement.previous.card == CardId(100) && movement.from == Zone::Battlefield && movement.destination == expected)).count(), 1);
            assert!(
                view.players
                    .iter()
                    .find(|player| player.player == PlayerId(0))
                    .unwrap()
                    .mana
                    .values()
                    .all(|n| *n == Quantity::zero())
            );
            recover(&g, &p, &mut verifier);
            returned = true;
            if to_command {
                return g;
            }
        }
        if recast_paid
            && view.stack.is_empty()
            && view.objects.iter().any(|o| {
                o.oracle_id == isamaru && o.owner == PlayerId(1) && o.zone == Zone::Battlefield
            })
        {
            assert!(returned && !to_command && recast);
            assert_eq!(recast_mana, 1);
            assert!(
                view.objects
                    .iter()
                    .any(|object| object.oracle_id == temporal
                        && object.owner == PlayerId(0)
                        && object.zone == Zone::Battlefield
                        && object.tapped)
            );
            assert_eq!(
                g.commander_ledger()
                    .history(CardId(100))
                    .unwrap()
                    .command_casts
                    .get(&PlayerId(1)),
                Some(&Quantity::from(1))
            );
            assert_eq!(
                g.commander_ledger().tax(CardId(100), PlayerId(1)).unwrap(),
                2.into()
            );
            assert_eq!(
                g.events()
                    .iter()
                    .filter(|e| matches!(e,Event::SpellProposed{card,..}if*card==CardId(100)))
                    .count(),
                2
            );
            recover(&g, &p, &mut verifier);
            return g;
        }
        let response = match r {
            PlayerRequest::Priority { .. } => {
                let main = view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                let land = if actor == PlayerId(0) {
                    &island
                } else {
                    &plains
                };
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == *land && o.controller == actor && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                if main && actor.0 < 2 {
                    if actor == PlayerId(1) && returned && !to_command && !recast {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == isamaru && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        recast = true;
                        recover(&g, &p, &mut verifier);
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if !activated
                        && lands.len() < if actor == PlayerId(0) { 6 } else { 1 }
                        && view.turn.lands_played == 0
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == *land && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: card.handle.clone(),
                        }
                    } else if actor == PlayerId(1)
                        && !target_cast
                        && lands.iter().any(|o| !o.tapped)
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == isamaru && o.zone == (Zone::Command { owner: actor })
                            })
                            .unwrap();
                        target_cast = true;
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && !cast
                        && lands.iter().filter(|o| !o.tapped).count() == 6
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == temporal && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        cast = true;
                        cast_turn = view.turn.number;
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && sick
                        && !activated
                        && view.turn.number > cast_turn
                        && let Some(source) = view.objects.iter().find(|o| {
                            o.oracle_id == temporal && o.zone == Zone::Battlefield && !o.tapped
                        })
                    {
                        assert_eq!(lands.iter().filter(|o| !o.tapped).count(), 6);
                        activated = true;
                        recover(&g, &p, &mut verifier);
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
                let color = if actor == PlayerId(0) {
                    Color::U
                } else {
                    Color::W
                };
                let available = view
                    .players
                    .iter()
                    .find(|p| p.player == actor)
                    .unwrap()
                    .mana
                    .get(&color)
                    .cloned()
                    .unwrap_or_default();
                let mut required = cost.generic;
                required.add(&Quantity::from(cost.colored.len() as i64));
                if available < required {
                    let land = if actor == PlayerId(0) {
                        &island
                    } else {
                        &plains
                    };
                    let source = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == *land
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .unwrap();
                    if actor == PlayerId(0) && activated && !ability_paid {
                        ability_mana += 1;
                    } else if actor == PlayerId(0) && cast && !paid {
                        cast_mana += 1;
                    } else if actor == PlayerId(1) && recast && !recast_paid {
                        recast_mana += 1;
                    } else if actor == PlayerId(1) && target_cast && !target_paid {
                        command_mana += 1;
                    }
                    PlayerResponse::ActivateMana {
                        object: source.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => {
                if actor == PlayerId(0) && activated && !ability_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 0.into(),
                            colored: vec![Color::U, Color::U, Color::U]
                        }
                    );
                    ability_paid = true;
                } else if actor == PlayerId(0) && cast && !paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 1.into(),
                            colored: vec![Color::U, Color::U]
                        }
                    );
                    paid = true;
                } else if actor == PlayerId(1) && recast && !recast_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 0.into(),
                            colored: vec![Color::W]
                        }
                    );
                    recast_paid = true;
                } else if actor == PlayerId(1) && target_cast && !target_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 0.into(),
                            colored: vec![Color::W]
                        }
                    );
                    target_paid = true;
                }
                PlayerResponse::SpendMana {
                    spending: exact_spending(&cost, &available),
                }
            }
            PlayerRequest::Targets {
                token, candidates, ..
            } => {
                assert!(activated && !targeted);
                let target = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == isamaru
                            && o.owner == PlayerId(1)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(candidates.contains(&target.handle));
                let command = view
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == ertai && o.zone == (Zone::Command { owner: PlayerId(0) })
                    })
                    .unwrap();
                assert!(!candidates.contains(&command.handle));
                reject_input(
                    &g,
                    actor,
                    token.clone(),
                    PlayerResponse::Targets {
                        objects: vec![command.handle.clone()],
                    },
                );
                reject_input(
                    &g,
                    actor,
                    token,
                    PlayerResponse::Targets {
                        objects: vec![ObjectHandle("unknown".into())],
                    },
                );
                targeted = true;
                recover(&g, &p, &mut verifier);
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert!(ability_paid && targeted);
                assert_eq!(actor, PlayerId(1));
                assert!(!choice_made);
                choice_made = true;
                recover(&g, &p, &mut verifier);
                PlayerResponse::CommanderReturn { to_command }
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
                        .is_some_and(|o| o.oracle_id == temporal)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected permanent return pod decision {other:?}"),
        };
        submit_match(&mut g, response);
        assert!(matches!(
            settle_match(&mut g, q),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("permanent return pod predeclared3400 decisions");
}
#[test]
fn admitted_permanent_return_match_owner_hand_recasts_without_tax_or_chooses_command() {
    // First setup-only opening success; no gameplay/outcome search.
    let seed = seed(36);
    for to_command in [false, true] {
        let a = run(seed, to_command, 1);
        let b = run(seed, to_command, 4096);
        assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
        assert_eq!(a.replay_journal(), b.replay_journal());
    }
}
