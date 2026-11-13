//! Source-qualified development admission and a genuine seeded 100-card pod.
//! No source, opening hand, zone, mana or runtime state is injected.
use mtg_contracts::{Capability, CardKind, Catalog, Color, ManaCost, sha256};
use mtg_core::*;
use std::collections::BTreeMap;

const PROFILE: &str = "commander-sacrifice-v1";
const SHRAPNEL: &str = "d3c623f3-c7c9-47c0-8096-6dee3267baff";

fn catalog() -> Catalog {
    match_profile(PROFILE).unwrap().catalog().clone()
}
fn oracle(c: &Catalog, name: &str) -> String {
    c.definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let commander = oracle(c, "Rorix Bladewing");
    let mountain = oracle(c, "Mountain");
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
                    oracle_id: oracle(c, "Sol Ring"),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: SHRAPNEL.into(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: mountain.clone(),
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
                    oracle_id: mountain.clone(),
                    count: 99,
                },
            ]
        },
    });
    validate_pod(c, &config).unwrap();
    config
}
fn settle(g: &mut Game, quantum: u64) {
    for _ in 0..20000 {
        match g.advance(Budget::new(quantum)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => return,
            other => panic!("artifact sacrifice pod {other:?}"),
        }
    }
    panic!("automatic work bound");
}
fn query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(64)),
            QueryResult::Rejected(error) => panic!("query rejected {error:?}"),
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
fn setup(c: &Catalog, seed: [u8; 32], quantum: u64) -> (Prepared, Game) {
    let prepared = Prepared::admit_pod(c.clone(), config(c, seed)).unwrap();
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
    settle(&mut game, quantum);
    for _ in 0..4 {
        assert!(matches!(request(&game).1, PlayerRequest::Mulligan { .. }));
        submit(&mut game, PlayerResponse::Mulligan { take: false }, quantum);
    }
    assert!(matches!(request(&game).1, PlayerRequest::Priority { .. }));
    assert_eq!(
        observation(&game, PlayerId(0)).view.turn.step,
        TurnStep::Upkeep
    );
    (prepared, game)
}
fn opening_has_sources(g: &Game, c: &Catalog) -> bool {
    let view = observation(g, PlayerId(0)).view;
    [oracle(c, "Sol Ring"), SHRAPNEL.into()].iter().all(|id| {
        view.objects
            .iter()
            .any(|o| o.oracle_id == *id && o.zone == (Zone::Hand { owner: PlayerId(0) }))
    })
}
fn seed(index: u32) -> [u8; 32] {
    let hash = sha256(format!("commander-sacrifice-v1/opening-shrapnel/{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).unwrap())
}

#[test]
#[ignore = "bounded setup-only seed selection; never executes gameplay"]
fn artifact_sacrifice_setup_only_seed_probe() {
    let c = catalog();
    for index in 0..2048 {
        let (_, g) = setup(&c, seed(index), 4096);
        if opening_has_sources(&g, &c) {
            println!(
                "first opening-only success index={index} seed={:?}",
                seed(index)
            );
            return;
        }
    }
    panic!("no source-pair opening in declared 0..2048 setup-only range");
}

#[test]
fn artifact_sacrifice_default_decks_and_feature_boundaries_remain_legal() {
    let c = catalog();
    assert_eq!(c.definitions.len(), 154);
    let profile = match_profile(PROFILE).unwrap();
    assert!(!profile.hosted && !profile.search);
    let config = MatchConfig::for_profile(PROFILE, [29; 32], PlayerId(0)).unwrap();
    for (seat, deck) in config.decks.iter().enumerate() {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|r| r.oracle_id == SHRAPNEL)
                .map(|r| r.count)
                .sum::<u32>(),
            u32::from(seat == 3)
        );
        for row in &deck.cards {
            let d = c.definition_by_oracle_id(&row.oracle_id).unwrap();
            if !d.characteristics.basic {
                assert_eq!(row.count, 1);
            }
        }
    }
    assert_eq!(
        validate_pod(&c, &config)
            .unwrap()
            .physical_config()
            .cards
            .len(),
        400
    );
    Prepared::admit_pod(c.clone(), config.clone()).unwrap();
    let mountain = oracle(&c, "Mountain");
    let mut wrong_size = config.clone();
    wrong_size.decks[3]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == mountain)
        .unwrap()
        .count += 1;
    assert!(matches!(
        validate_pod(&c, &wrong_size),
        Err(MatchAdmissionError::InvalidDeckSize {
            player: PlayerId(3),
            total: 101
        })
    ));
    let mut duplicate = config.clone();
    duplicate.decks[3]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == SHRAPNEL)
        .unwrap()
        .count += 1;
    duplicate.decks[3]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == mountain)
        .unwrap()
        .count -= 1;
    assert!(matches!(
        validate_pod(&c, &duplicate),
        Err(MatchAdmissionError::SingletonName {
            player: PlayerId(3),
            ..
        })
    ));
    let mut off_color = config;
    let basic = off_color.decks[0]
        .cards
        .iter_mut()
        .find(|r| {
            c.definition_by_oracle_id(&r.oracle_id)
                .unwrap()
                .characteristics
                .basic
        })
        .unwrap();
    basic.count -= 1;
    off_color.decks[0].cards.push(DeckEntry {
        oracle_id: SHRAPNEL.into(),
        count: 1,
    });
    assert!(
        matches!(validate_pod(&c, &off_color), Err(MatchAdmissionError::ColorIdentity { player: PlayerId(0), oracle_id }) if oracle_id == SHRAPNEL)
    );
    for feature in [Feature::Hosting, Feature::Search] {
        assert!(
            matches!(profile.require_feature(&c, feature), Err(ProfileError::FeatureNotRequested { feature: actual }) if actual == feature)
        );
        let mut enabled = profile.clone();
        match feature {
            Feature::Hosting => enabled.hosted = true,
            Feature::Search => enabled.search = true,
        }
        assert!(
            matches!(enabled.validate_requested_features(&c), Err(ProfileError::UnreviewedCapabilities { feature: actual, capabilities }) if actual == feature && capabilities.contains(&Capability::SacrificeV1))
        );
    }
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
        Game::replay_with_trace(p.clone(), &g.replay_journal(), TraceMode::Detailed)
            .unwrap()
            .state_digest()
            .unwrap(),
        g.state_digest().unwrap()
    );
}
fn run(seed: [u8; 32], quantum: u64) -> Game {
    let c = catalog();
    let ring = oracle(&c, "Sol Ring");
    let mountain = oracle(&c, "Mountain");
    let source = c.definition_by_oracle_id(SHRAPNEL).unwrap();
    assert_eq!(
        source.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::R]
        }
    );
    assert_eq!(
        source
            .spell
            .as_ref()
            .unwrap()
            .additional_sacrifice
            .as_ref()
            .unwrap()
            .types,
        vec![CardKind::Artifact]
    );
    let (prepared, mut game) = setup(&c, seed, quantum);
    let blast_card = prepared
        .config()
        .cards
        .iter()
        .find(|row| row.owner == PlayerId(0) && row.oracle_id == SHRAPNEL)
        .unwrap()
        .id;
    assert!(
        opening_has_sources(&game, &c),
        "pinned seed must actually deal both complete sources"
    );
    let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Detailed);
    let (mut ring_cast, mut blast_cast, mut sacrificed) = (false, false, false);
    let (mut ring_mana, mut red_mana) = (0, 0);
    for _ in 0..1200 {
        let (actor, request) = request(&game);
        let view = observation(&game, actor).view;
        if sacrificed && view.stack.is_empty() {
            let own = observation(&game, PlayerId(0)).view;
            assert_eq!(
                own.players
                    .iter()
                    .find(|p| p.player == PlayerId(1))
                    .unwrap()
                    .life,
                35.into()
            );
            for id in [ring.as_str(), SHRAPNEL] {
                assert!(own.objects.iter().any(|o| o.oracle_id == *id
                    && o.owner == PlayerId(0)
                    && o.zone == (Zone::Graveyard { owner: PlayerId(0) })));
            }
            assert_eq!((ring_mana, red_mana), (1, 1));
            let stack = game
                .events()
                .iter()
                .find_map(|event| match event {
                    Event::SpellProposed { card, stack_id } if *card == blast_card => {
                        Some(*stack_id)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(game.events().iter().any(|event| matches!(event, Event::SpellCompleted { stack_id, resolved: true } if *stack_id == stack)));
            let damage = game
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::Committed {
                        actual: EventOutcome::DamageCommitted { actual, .. },
                        ..
                    } => Some(actual),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(damage.len(), 1);
            assert_eq!(damage[0].source.object.card, blast_card);
            assert_eq!(damage[0].amount, 5.into());
            let mana = &own
                .players
                .iter()
                .find(|p| p.player == PlayerId(0))
                .unwrap()
                .mana;
            assert_eq!(mana.get(&Color::C).cloned().unwrap_or_default(), 1.into());
            assert_eq!(mana.get(&Color::R).cloned().unwrap_or_default(), 0.into());
            recover(&game, &prepared, &mut verifier);
            return game;
        }
        let response = match request {
            PlayerRequest::Priority { .. }
                if actor == PlayerId(0)
                    && view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    ) =>
            {
                if view.turn.lands_played == 0
                    && let Some(land) = view.objects.iter().find(|o| {
                        o.oracle_id == mountain && o.zone == (Zone::Hand { owner: actor })
                    })
                {
                    PlayerResponse::PlayLand {
                        object: land.handle.clone(),
                    }
                } else if !ring_cast
                    && let Some(card) = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == ring && o.zone == (Zone::Hand { owner: actor }))
                {
                    ring_cast = true;
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else if !blast_cast
                    && view
                        .objects
                        .iter()
                        .any(|o| o.oracle_id == ring && o.zone == Zone::Battlefield && !o.tapped)
                    && view.objects.iter().any(|o| {
                        o.oracle_id == mountain && o.zone == Zone::Battlefield && !o.tapped
                    })
                {
                    let card = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == SHRAPNEL && o.zone == (Zone::Hand { owner: actor })
                        })
                        .unwrap();
                    blast_cast = true;
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Mana { cost, .. } => {
                let mana = &view
                    .players
                    .iter()
                    .find(|p| p.player == actor)
                    .unwrap()
                    .mana;
                let amount = |c| mana.get(&c).cloned().unwrap_or_default();
                if blast_cast && amount(Color::C) < 1.into() {
                    let object = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == ring && o.zone == Zone::Battlefield && !o.tapped)
                        .unwrap();
                    ring_mana += 1;
                    PlayerResponse::ActivateMana {
                        object: object.handle.clone(),
                    }
                } else if amount(Color::R) < if blast_cast { 1.into() } else { cost.generic } {
                    let object = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == mountain && o.zone == Zone::Battlefield && !o.tapped
                        })
                        .unwrap();
                    if blast_cast {
                        red_mana += 1;
                    }
                    PlayerResponse::ActivateMana {
                        object: object.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana { cost, .. } => {
                if blast_cast {
                    assert_eq!(
                        cost,
                        ManaCost {
                            generic: 1.into(),
                            colored: vec![Color::R]
                        }
                    );
                    PlayerResponse::SpendMana {
                        spending: BTreeMap::from([(Color::C, 1.into()), (Color::R, 1.into())]),
                    }
                } else {
                    PlayerResponse::SpendMana {
                        spending: BTreeMap::from([(Color::R, 1.into())]),
                    }
                }
            }
            PlayerRequest::EntityTargets { candidates, .. } => {
                let target = PlayerTargetEntity::Player {
                    player: PlayerId(1),
                };
                assert!(candidates.contains(&target));
                PlayerResponse::EntityTargets {
                    entities: vec![target],
                }
            }
            PlayerRequest::SacrificeCost { candidates, .. } => {
                let object = view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == ring && o.zone == Zone::Battlefield)
                    .unwrap();
                assert!(object.tapped && candidates.contains(&object.handle));
                recover(&game, &prepared, &mut verifier);
                sacrificed = true;
                PlayerResponse::SacrificeCost {
                    object: object.handle.clone(),
                }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count,
                mut candidates,
                ..
            } => {
                // Passing opponents still draw and discard at ordinary cleanup.
                // Prefer actual offered Mountains, preserving the source pair.
                candidates.sort_by_key(|handle| {
                    !view
                        .objects
                        .iter()
                        .any(|o| o.handle == *handle && o.oracle_id == mountain)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected pod decision {other:?}"),
        };
        submit(&mut game, response, quantum);
        if sacrificed && !observation(&game, PlayerId(0)).view.stack.is_empty() {
            recover(&game, &prepared, &mut verifier);
        }
    }
    panic!("paid artifact sacrifice witness bound");
}

#[test]
fn artifact_sacrifice_accepted_pod_pays_ring_then_shrapnel_and_replays_across_quanta() {
    // First actual source-pair opening in the declared setup-only 0..2048 scan.
    // The probe keeps four hands and stops at Upkeep, before gameplay choices.
    let seed = seed(40);
    assert_eq!(
        seed,
        [
            72, 94, 226, 193, 250, 173, 67, 36, 190, 71, 242, 179, 63, 197, 10, 200, 247, 215, 107,
            55, 28, 27, 141, 130, 201, 104, 254, 54, 63, 84, 49, 232
        ]
    );
    let a = run(seed, 1);
    let b = run(seed, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
