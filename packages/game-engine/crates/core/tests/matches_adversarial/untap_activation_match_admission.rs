//! Legal 100-card pods, ordinary shuffle/keeps, paid casts and natural aging.
use super::royal_match_admission::{oracle, recover, setup, source_catalog};
use super::*;

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
        let mut cards = vec![DeckEntry {
            oracle_id: cmd.clone(),
            count: 1,
        }];
        if seat == 0 {
            for name in ["Seeker of Skybreak", "Llanowar Elves"] {
                cards.push(DeckEntry {
                    oracle_id: oracle(c, name),
                    count: 1,
                });
            }
        }
        cards.push(DeckEntry {
            oracle_id: oracle(c, if seat == 0 { "Forest" } else { "Plains" }),
            count: if seat == 0 { 97 } else { 99 },
        });
        Decklist {
            commander: cmd,
            cards,
        }
    });
    validate_pod(c, &m).unwrap();
    m
}
fn seed(index: u32) -> [u8; 32] {
    let hex = mtg_contracts::sha256(
        format!("commander-blocking-targets-v1/opening-seeker-elf/{index}").as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn opening(g: &Game, c: &Catalog) -> bool {
    ["Seeker of Skybreak", "Llanowar Elves"].iter().all(|name| {
        observation(g, PlayerId(0)).view.objects.iter().any(|o| {
            o.oracle_id == oracle(c, name) && o.zone == (Zone::Hand { owner: PlayerId(0) })
        })
    })
}
#[test]
#[ignore = "bounded opening keep/shuffle scan only; no gameplay or outcome search"]
fn untap_activation_setup_only_seed_probe() {
    let c = source_catalog();
    for index in 0..2048 {
        let (_, g) = setup(c.clone(), config(&c, seed(index)), 4096);
        if opening(&g, &c) {
            println!("first opening-only success index={index}");
            return;
        }
    }
    panic!("no opening in predeclared 0..2048");
}
fn run(index: u32, q: u64) -> Game {
    let c = source_catalog();
    let seeker = oracle(&c, "Seeker of Skybreak");
    let elf = oracle(&c, "Llanowar Elves");
    let forest = oracle(&c, "Forest");
    let (p, mut g) = setup(c.clone(), config(&c, seed(index)), q);
    assert!(opening(&g, &c));
    let source_card = p
        .config()
        .cards
        .iter()
        .find(|r| r.oracle_id == seeker)
        .unwrap()
        .id;
    let elf_card = p
        .config()
        .cards
        .iter()
        .find(|r| r.oracle_id == elf)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut elf_cast, mut seeker_cast, mut fresh, mut activated, mut targeted) =
        (false, false, false, false, false);
    let (mut elf_mana, mut seeker_mana, mut elf_taps, mut cast_turn) = (0, 0, 0, 0);
    let mut old_hand = None;
    for _ in 0..1800 {
        let (actor, r) = request(&g);
        let v = observation(&g, actor).view;
        let bf = |id: &str| {
            v.objects
                .iter()
                .find(|o| o.oracle_id == id && o.zone == Zone::Battlefield)
        };
        if actor == PlayerId(0) && v.stack.is_empty() && !fresh && bf(&seeker).is_some() {
            assert!(
                bf(&elf).is_some(),
                "independent legal creature target exists"
            );
            let raw = g.decision_for(actor).unwrap().unwrap();
            let before = g.snapshot().unwrap();
            assert!(
                g.respond(
                    actor,
                    raw.decision_id(),
                    Response::ActivateAbility {
                        object: g.object_id(source_card).unwrap(),
                        ability: 0
                    }
                )
                .unwrap_err()
                .contains("continuously controlled")
            );
            assert_eq!(g.snapshot().unwrap(), before);
            fresh = true;
            recover(&g, &p, &mut verifier);
        }
        if actor == PlayerId(0) && elf_taps == 2 && v.stack.is_empty() {
            assert!(fresh && activated && targeted);
            assert_eq!((elf_mana, seeker_mana), (1, 2));
            assert_eq!(
                v.players
                    .iter()
                    .find(|r| r.player == actor)
                    .unwrap()
                    .mana
                    .get(&Color::G),
                Some(&Quantity::from(2))
            );
            assert!(bf(&seeker).unwrap().tapped && bf(&elf).unwrap().tapped);
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Untapped { object },.. } if object.card==elf_card)).count(),1);
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed { actual:EventOutcome::Tapped { object },.. } if object.card==source_card)).count(),1);
            assert!(
                g.events()
                    .iter()
                    .any(|e| matches!(e, Event::AbilityCompleted { .. }))
            );
            recover(&g, &p, &mut verifier);
            return g;
        }
        let response = match r {
            PlayerRequest::Priority { .. } => {
                let main = actor == PlayerId(0)
                    && v.active == Some(actor)
                    && v.stack.is_empty()
                    && matches!(
                        v.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                let lands = v
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == forest
                            && o.zone == Zone::Battlefield
                            && o.controller == actor
                    })
                    .collect::<Vec<_>>();
                if main
                    && v.turn.lands_played == 0
                    && lands.len() < 3
                    && let Some(o) = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == forest && o.zone == (Zone::Hand { owner: actor }))
                {
                    PlayerResponse::PlayLand {
                        object: o.handle.clone(),
                    }
                } else if main && !elf_cast && lands.iter().filter(|o| !o.tapped).count() >= 1 {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == elf && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    old_hand = Some(o.handle.clone());
                    elf_cast = true;
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }
                } else if main
                    && elf_cast
                    && !seeker_cast
                    && lands.iter().filter(|o| !o.tapped).count() >= 2
                {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == seeker && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    seeker_cast = true;
                    cast_turn = v.turn.number;
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }
                } else if main && fresh && v.turn.number > cast_turn && elf_taps == 0 {
                    assert!(bf(&seeker).unwrap().attack_ready && bf(&elf).unwrap().attack_ready);
                    elf_taps += 1;
                    PlayerResponse::ActivateMana {
                        object: bf(&elf).unwrap().handle.clone(),
                    }
                } else if main && elf_taps == 1 && !activated {
                    activated = true;
                    recover(&g, &p, &mut verifier);
                    PlayerResponse::ActivateAbility {
                        object: bf(&seeker).unwrap().handle.clone(),
                        ability: 0,
                    }
                } else if main && activated && targeted && !bf(&elf).unwrap().tapped {
                    elf_taps += 1;
                    PlayerResponse::ActivateMana {
                        object: bf(&elf).unwrap().handle.clone(),
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let available = v
                    .players
                    .iter()
                    .find(|r| r.player == actor)
                    .unwrap()
                    .mana
                    .get(&Color::G)
                    .cloned()
                    .unwrap_or_default();
                let mut required = cost.generic;
                required.add(&Quantity::from(cost.colored.len() as i64));
                if available < required {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == forest
                                && o.controller == actor
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                        })
                        .unwrap();
                    if seeker_cast {
                        seeker_mana += 1
                    } else {
                        elf_mana += 1
                    }
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: exact_spending(&cost, &available),
            },
            PlayerRequest::Targets {
                token, candidates, ..
            } => {
                assert!(activated && !targeted);
                let target = bf(&elf).unwrap();
                assert!(target.tapped && candidates.contains(&target.handle));
                let land = bf(&forest).unwrap();
                assert!(!candidates.contains(&land.handle));
                for h in [
                    land.handle.clone(),
                    old_hand.clone().unwrap(),
                    ObjectHandle("unoffered-unknown".into()),
                ] {
                    reject_input(
                        &g,
                        actor,
                        token.clone(),
                        PlayerResponse::Targets { objects: vec![h] },
                    );
                }
                recover(&g, &p, &mut verifier);
                targeted = true;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            other => panic!("unexpected untap pod decision {other:?}"),
        };
        submit_match(&mut g, response);
        assert!(matches!(
            settle_match(&mut g, q),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("untap pod exceeded predeclared1800 decisions");
}
#[test]
fn admitted_untap_activation_pays_casts_ages_and_reuses_real_elf_green_mana() {
    let a = run(169, 1);
    let b = run(169, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
