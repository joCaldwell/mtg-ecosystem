//! Genuine legal-pod prefix: cast Royal, age it, then destroy an actual attacker.
//! No opening, mana, tap/readiness, stack or combat state is injected.
use super::*;

pub(super) fn source_catalog() -> Catalog {
    let c = match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone();
    assert_eq!(c.definitions.len(), 47);
    c
}
pub(super) fn oracle(c: &Catalog, name: &str) -> String {
    c.definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
pub(super) fn setup(c: Catalog, config: MatchConfig, q: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c, config).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    assert!(matches!(
        settle_match(&mut g, q),
        Advance::AwaitingDecision(_)
    ));
    for _ in 0..4 {
        assert!(matches!(request(&g).1, PlayerRequest::Mulligan { .. }));
        submit_match(&mut g, PlayerResponse::Mulligan { take: false });
        assert!(matches!(
            settle_match(&mut g, q),
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
pub(super) fn recover(g: &Game, p: &Prepared, v: &mut SnapshotVerifier) {
    v.verify(&g.snapshot().unwrap()).unwrap();
    assert_restore(g, p);
    assert_replay(g, p);
}
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let mut m =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    m.decks = std::array::from_fn(|seat| {
        let cmd = oracle(
            c,
            if seat == 0 {
                "Yargle and Multani"
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
                        oracle_id: oracle(c, "Royal Assassin"),
                        count: 1,
                    },
                    DeckEntry {
                        oracle_id: oracle(c, "Swamp"),
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
        format!("commander-blocking-targets-v1/opening-royal-assassin/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn opening(g: &Game, c: &Catalog) -> bool {
    observation(g, PlayerId(0)).view.objects.iter().any(|o| {
        o.oracle_id == oracle(c, "Royal Assassin") && o.zone == (Zone::Hand { owner: PlayerId(0) })
    })
}
#[test]
#[ignore = "bounded setup-only opening scan; never gameplay/outcome search"]
fn royal_setup_only_seed_probe() {
    let c = source_catalog();
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
    panic!("no Royal opening in predeclared setup-only0..1024");
}
fn run(seed: [u8; 32], q: u64) -> Game {
    let c = source_catalog();
    let royal = oracle(&c, "Royal Assassin");
    let swamp = oracle(&c, "Swamp");
    let plains = oracle(&c, "Plains");
    let isamaru = oracle(&c, "Isamaru, Hound of Konda");
    let (p, mut g) = setup(c.clone(), config(&c, seed), q);
    assert!(opening(&g, &c));
    let royal_card = p
        .config()
        .cards
        .iter()
        .find(|r| r.oracle_id == royal)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (
        mut cast,
        mut paid,
        mut sick_proved,
        mut attacked,
        mut activated,
        mut targeted,
        mut returned,
    ) = (false, false, false, false, false, false, false);
    let (mut cast_mana, mut cast_turn) = (0, 0);
    for _ in 0..2200 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        if paid
            && !sick_proved
            && view.stack.is_empty()
            && view
                .objects
                .iter()
                .any(|o| o.oracle_id == royal && o.zone == Zone::Battlefield)
        {
            assert_eq!(actor, PlayerId(0));
            let raw = g.decision_for(actor).unwrap().unwrap();
            let source = g.object_id(royal_card).unwrap();
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
            assert!(cast && paid && sick_proved && attacked && activated && targeted);
            assert_eq!(cast_mana, 3);
            assert_eq!(
                view.players
                    .iter()
                    .find(|p| p.player == PlayerId(0))
                    .unwrap()
                    .life,
                38.into()
            );
            assert!(view.objects.iter().any(|o| o.oracle_id == royal
                && o.owner == PlayerId(0)
                && o.zone == Zone::Battlefield
                && o.tapped));
            assert!(view.objects.iter().any(|o| o.oracle_id == isamaru
                && o.owner == PlayerId(1)
                && o.zone == (Zone::Command { owner: PlayerId(1) })));
            assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::Moved {movement},..} if movement.previous.card==CardId(100)&&movement.destination==(Zone::Graveyard {owner:PlayerId(1)}))));
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed {actual:EventOutcome::Tapped {object},..} if object.card==royal_card)).count(),1);
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
                    &swamp
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
                if main && actor.0 < 2 && !activated {
                    if lands.len() < if actor == PlayerId(0) { 4 } else { 1 }
                        && view.turn.lands_played == 0
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == *land && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: card.handle.clone(),
                        }
                    } else if actor == PlayerId(1)
                        && lands.iter().any(|o| !o.tapped)
                        && let Some(card) = view.objects.iter().find(|o| {
                            o.oracle_id == isamaru && o.zone == (Zone::Command { owner: actor })
                        })
                    {
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && !cast
                        && lands.iter().filter(|o| !o.tapped).count() >= 3
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == royal && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        cast = true;
                        cast_turn = view.turn.number;
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(0)
                        && sick_proved
                        && attacked
                        && view.turn.number > cast_turn
                        && let Some(source) = view.objects.iter().find(|o| {
                            o.oracle_id == royal && o.zone == Zone::Battlefield && !o.tapped
                        })
                    {
                        assert!(view.objects.iter().any(|o| o.oracle_id == isamaru
                            && o.owner == PlayerId(1)
                            && o.zone == Zone::Battlefield
                            && o.tapped));
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
                assert!(
                    !activated,
                    "tap-only activation must not offer a mana window"
                );
                let color = if actor == PlayerId(0) {
                    Color::B
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
                        &swamp
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
                    if cast && !paid {
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
                if cast && !paid {
                    assert_eq!(
                        cost,
                        mtg_contracts::ManaCost {
                            generic: 1.into(),
                            colored: vec![Color::B, Color::B]
                        }
                    );
                    paid = true;
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
                        o.oracle_id == isamaru
                            && o.owner == PlayerId(1)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert!(target.tapped && candidates.contains(&target.handle));
                targeted = true;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let target = view.objects.iter().find(|o| {
                    o.oracle_id == isamaru
                        && o.owner == PlayerId(1)
                        && o.zone == Zone::Battlefield
                        && candidates.contains(&o.handle)
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(1) && sick_proved && !attacked {
                        attacked = true;
                        vec![(target.unwrap().handle.clone(), PlayerId(0))]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert!(activated && targeted);
                assert_eq!(actor, PlayerId(1));
                returned = true;
                PlayerResponse::CommanderReturn { to_command: true }
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
                        .is_some_and(|o| o.oracle_id == royal)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected Royal pod decision {other:?}"),
        };
        submit_match(&mut g, response);
        assert!(matches!(
            settle_match(&mut g, q),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("Royal pod predeclared2200 decisions");
}
#[test]
fn admitted_royal_match_casts_ages_and_destroys_real_attacking_commander() {
    // First Royal opening in the declared setup-only scan.
    let seed = seed(2);
    assert_eq!(
        seed,
        [
            65, 170, 235, 17, 202, 133, 75, 214, 31, 81, 230, 124, 140, 196, 4, 13, 36, 157, 22,
            192, 20, 111, 186, 95, 42, 114, 249, 75, 174, 194, 101, 50
        ]
    );
    let a = run(seed, 1);
    let b = run(seed, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
