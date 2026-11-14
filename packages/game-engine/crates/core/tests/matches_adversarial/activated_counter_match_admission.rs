//! Legal-pod prefix: cast Ertai from command, age, then counter a real commander.
//! No stack, mana, tap/readiness or zone state is injected.
use super::royal_match_admission::{oracle, recover, setup, source_catalog};
use super::*;

fn config(c: &Catalog) -> MatchConfig {
    let mut m =
        MatchConfig::for_profile("commander-blocking-targets-v1", [73; 32], PlayerId(0)).unwrap();
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
            cards: vec![
                DeckEntry {
                    oracle_id: cmd,
                    count: 1,
                },
                DeckEntry {
                    oracle_id: oracle(c, if seat == 0 { "Island" } else { "Plains" }),
                    count: 99,
                },
            ],
        }
    });
    validate_pod(c, &m).unwrap();
    m
}
fn run(q: u64) -> Game {
    let c = source_catalog();
    let ertai = oracle(&c, "Ertai, Wizard Adept");
    let isamaru = oracle(&c, "Isamaru, Hound of Konda");
    let island = oracle(&c, "Island");
    let plains = oracle(&c, "Plains");
    let (p, mut g) = setup(c.clone(), config(&c), q);
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (
        mut cast,
        mut cast_paid,
        mut sick_proved,
        mut target_cast,
        mut target_paid,
        mut activated,
        mut ability_paid,
        mut targeted,
        mut returned,
    ) = (
        false, false, false, false, false, false, false, false, false,
    );
    let (mut cast_mana, mut ability_mana, mut cast_turn) = (0, 0, 0);
    for _ in 0..2200 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        if cast_paid
            && !sick_proved
            && view.stack.is_empty()
            && view
                .objects
                .iter()
                .any(|o| o.oracle_id == ertai && o.zone == Zone::Battlefield)
        {
            assert_eq!(actor, PlayerId(0));
            let raw = g.decision_for(actor).unwrap().unwrap();
            let source = g.object_id(CardId(0)).unwrap();
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
            sick_proved = true;
            recover(&g, &p, &mut verifier);
        }
        if returned && view.stack.is_empty() {
            assert!(
                cast && cast_paid
                    && sick_proved
                    && target_cast
                    && target_paid
                    && activated
                    && ability_paid
                    && targeted
            );
            assert_eq!((cast_mana, ability_mana), (3, 4));
            assert!(
                view.players
                    .iter()
                    .find(|p| p.player == PlayerId(0))
                    .unwrap()
                    .mana
                    .values()
                    .all(|n| *n == Quantity::zero())
            );
            assert_eq!(
                view.objects
                    .iter()
                    .filter(|o| o.oracle_id == island
                        && o.controller == PlayerId(0)
                        && o.zone == Zone::Battlefield
                        && o.tapped)
                    .count(),
                4
            );
            assert!(view.objects.iter().any(|o| o.oracle_id == ertai
                && o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.tapped));
            assert!(view.objects.iter().any(|o| o.oracle_id == isamaru
                && o.owner == PlayerId(1)
                && o.zone == (Zone::Command { owner: PlayerId(1) })));
            let history = g.commander_ledger().history(CardId(0)).unwrap();
            assert_eq!(
                history.command_casts.get(&PlayerId(0)),
                Some(&Quantity::from(1))
            );
            assert_eq!(
                g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
                2.into()
            );
            assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::StackObjectCountered {source,spell:true,..},..} if source.card==CardId(100))));
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed {actual:EventOutcome::Tapped {object},..} if object.card==CardId(0))).count(),1);
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
                let ready = view.objects.iter().any(|o| {
                    o.oracle_id == ertai
                        && o.owner == PlayerId(0)
                        && o.zone == Zone::Battlefield
                        && !o.tapped
                }) && view.turn.number > cast_turn
                    && view
                        .objects
                        .iter()
                        .filter(|o| {
                            o.oracle_id == island
                                && o.controller == PlayerId(0)
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .count()
                        == 4;
                if actor == PlayerId(0) && target_paid && !activated {
                    assert!(ready);
                    let source = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == ertai && o.zone == Zone::Battlefield)
                        .unwrap();
                    activated = true;
                    recover(&g, &p, &mut verifier);
                    PlayerResponse::ActivateAbility {
                        object: source.handle.clone(),
                        ability: 0,
                    }
                } else if main && actor.0 < 2 && !target_cast {
                    if lands.len() < if actor == PlayerId(0) { 4 } else { 1 }
                        && view.turn.lands_played == 0
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == *land && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && !cast
                        && lands.iter().filter(|o| !o.tapped).count() >= 3
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == ertai && o.zone == (Zone::Command { owner: actor })
                            })
                            .unwrap();
                        cast = true;
                        cast_turn = view.turn.number;
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(1) && sick_proved && ready {
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
                    if activated && !ability_paid {
                        ability_mana += 1;
                    } else if cast && !cast_paid {
                        cast_mana += 1;
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
                if activated && !ability_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 2.into(),
                            colored: vec![Color::U, Color::U]
                        }
                    );
                    assert_eq!(
                        g.commander_ledger().tax(CardId(0), PlayerId(0)).unwrap(),
                        2.into()
                    );
                    ability_paid = true;
                } else if actor == PlayerId(0) && cast && !cast_paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 2.into(),
                            colored: vec![Color::U]
                        }
                    );
                    cast_paid = true;
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
            PlayerRequest::StackTargets {
                token,
                candidates,
                occurrences,
                ..
            } => {
                assert!(activated && !targeted);
                assert_eq!(candidates.len(), 1);
                assert_eq!(
                    occurrences[0].domain,
                    mtg_contracts::TargetDomain::StackSpell
                );
                assert!(view.stack.iter().any(|e| e.controller == PlayerId(1)
                    && e.cast_complete
                    && e.spell.is_some()
                    && e.handle.as_ref() == Some(&candidates[0])));
                let object = view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == ertai && o.zone == Zone::Battlefield)
                    .unwrap();
                reject_input(
                    &g,
                    actor,
                    token,
                    PlayerResponse::StackTargets {
                        entries: vec![StackHandle(object.handle.0.clone())],
                    },
                );
                targeted = true;
                recover(&g, &p, &mut verifier);
                PlayerResponse::StackTargets {
                    entries: vec![candidates[0].clone()],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert!(ability_paid && targeted);
                assert_eq!(actor, PlayerId(1));
                returned = true;
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            other => panic!("unexpected counter pod decision {other:?}"),
        };
        submit_match(&mut g, response);
        assert!(matches!(
            settle_match(&mut g, q),
            Advance::AwaitingDecision(_)
        ));
        if ability_paid {
            recover(&g, &p, &mut verifier);
        }
    }
    panic!("counter pod predeclared2200 decisions");
}
#[test]
fn admitted_activated_counter_commander_pays_activation_without_tax_and_counters_commander() {
    let a = run(1);
    let b = run(4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
