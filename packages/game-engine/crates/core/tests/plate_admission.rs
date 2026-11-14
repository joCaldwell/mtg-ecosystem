//! Actual accepted-input 100-card pod. No injected mana, state or outcomes.
//! Policy observation probes are separate simulator tests.
use mtg_contracts::{Catalog, Color, KeywordAbility, digest, sha256};
use mtg_core::*;
use std::collections::BTreeMap;
const PROFILE: &str = "commander-sacrifice-v1";
const PLATE: &str = "b5b4cf54-ed5e-42d0-9d98-5fec76b0b0b8";
fn oracle(c: &Catalog, name: &str) -> String {
    c.definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let mut m = MatchConfig::for_profile(PROFILE, seed, PlayerId(0)).unwrap();
    let cmd = oracle(c, "Isamaru, Hound of Konda");
    let land = oracle(c, "Plains");
    m.decks = std::array::from_fn(|seat| {
        let mut cards = vec![
            DeckEntry {
                oracle_id: cmd.clone(),
                count: 1,
            },
            DeckEntry {
                oracle_id: land.clone(),
                count: if seat == 0 { 98 } else { 99 },
            },
        ];
        if seat == 0 {
            cards.push(DeckEntry {
                oracle_id: PLATE.into(),
                count: 1,
            });
        }
        Decklist {
            commander: cmd.clone(),
            cards,
        }
    });
    validate_pod(c, &m).unwrap();
    m
}
fn settle(g: &mut Game, q: u64) {
    for _ in 0..20000 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => return,
            x => panic!("advance {x:?}"),
        }
    }
    panic!("advance bound")
}
fn query(g: &Game, mut r: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        match r {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => r = g.resume_work(work, Budget::new(64)),
            QueryResult::Rejected(error) => panic!("query rejected {error:?}"),
        }
    }
    panic!("query bound")
}
fn view(g: &Game, p: PlayerId) -> PermittedView {
    let QueryValue::Observation(o) = query(g, g.observe(p, Budget::new(10000))) else {
        panic!("view")
    };
    o.view
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    let p = g
        .live_players()
        .into_iter()
        .find(|p| g.decision_for(*p).unwrap().is_some())
        .unwrap();
    let QueryValue::Decision(Some(r)) = query(g, g.player_decision_for(p, Budget::new(10000)))
    else {
        panic!("request")
    };
    (p, r)
}
fn submit(g: &mut Game, r: PlayerResponse, q: u64) {
    let (p, d) = request(g);
    let QueryValue::Validated(receipt) = query(
        g,
        g.check_player_response(p, d.token().clone(), r, Budget::new(10000)),
    ) else {
        panic!("validation")
    };
    g.submit(receipt).unwrap();
    settle(g, q);
}
fn seed(i: u32) -> [u8; 32] {
    let h = sha256(format!("commander-sacrifice-v1/opening-plate/{i}").as_bytes());
    std::array::from_fn(|n| u8::from_str_radix(&h[n * 2..n * 2 + 2], 16).unwrap())
}
fn setup(c: &Catalog, i: u32, q: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c.clone(), config(c, seed(i))).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    settle(&mut g, q);
    for _ in 0..4 {
        assert!(matches!(request(&g).1, PlayerRequest::Mulligan { .. }));
        submit(&mut g, PlayerResponse::Mulligan { take: false }, q);
    }
    assert_eq!(view(&g, PlayerId(0)).turn.step, TurnStep::Upkeep);
    (p, g)
}
fn opening(g: &Game) -> bool {
    view(g, PlayerId(0))
        .objects
        .iter()
        .any(|o| o.oracle_id == PLATE && o.zone == (Zone::Hand { owner: PlayerId(0) }))
}
fn recover(g: &Game, p: &Prepared, v: &mut SnapshotVerifier) {
    let s = g.snapshot().unwrap();
    v.verify(&s).unwrap();
    assert_eq!(
        Game::restore(p.clone(), &s)
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
#[test]
fn plate_admission_preserves_catalog_default_delta_and_consumer_guards() {
    let profile = match_profile(PROFILE).unwrap();
    let mut c = profile.catalog().clone();
    assert_eq!(c.definitions.len(), 159);
    assert_eq!(c.schema_version, 1);
    let d = c.definition_by_oracle_id(PLATE).unwrap();
    assert_eq!(d.name, "Darksteel Plate");
    assert_eq!(d.keywords, [KeywordAbility::Indestructible]);
    c.definitions.retain(|d| d.oracle_id != PLATE);
    assert_eq!(
        digest(&c).unwrap(),
        "ea382a4cd52a6ce30747e422fa4c3023ae886a7bf962aa96f05b3afe5ba8a820",
        "all 158 original definition bodies and envelope unchanged"
    );
    let m = MatchConfig::for_profile(PROFILE, [2; 32], PlayerId(0)).unwrap();
    validate_pod(profile.catalog(), &m).unwrap();
    for (seat, d) in m.decks.iter().enumerate() {
        assert_eq!(d.cards.iter().map(|r| r.count).sum::<u32>(), 100);
        assert_eq!(
            d.cards
                .iter()
                .filter(|r| r.oracle_id == PLATE)
                .map(|r| r.count)
                .sum::<u32>(),
            u32::from(seat == 0)
        );
    }
    assert_eq!(
        m.decks[0]
            .cards
            .iter()
            .find(|r| r.oracle_id == oracle(&c, "Plains"))
            .unwrap()
            .count,
        56
    );
    for f in [Feature::Hosting, Feature::Search] {
        assert!(matches!(
            profile.require_feature(profile.catalog(), f),
            Err(ProfileError::FeatureNotRequested { .. })
        ));
        let mut forced = profile.clone();
        match f {
            Feature::Hosting => forced.hosted = true,
            Feature::Search => forced.search = true,
        };
        assert!(matches!(
            forced.validate_requested_features(profile.catalog()),
            Err(ProfileError::UnreviewedCapabilities { .. })
        ));
    }
    // Reversing exactly the white Plate-for-Plains delta restores the base profile bytes semantically.
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/profiles/commander-sacrifice-v1.json"
    ))
    .unwrap();
    let rows = raw["decks"][0]["cards"].as_array_mut().unwrap();
    rows.retain(|r| r["oracle_id"] != PLATE);
    let plains = rows
        .iter_mut()
        .find(|r| r["oracle_id"] == oracle(&c, "Plains"))
        .unwrap();
    plains["count"] = serde_json::json!(57);
    assert_eq!(
        digest(&raw).unwrap(),
        "50592fea90b14b808fd1f334dc1c1596223b32655bd12e6f8360d120249db228"
    );
}
#[test]
#[ignore = "bounded setup-only scan: keeps four hands, stops at Upkeep; no outcome search"]
fn plate_setup_only_opening_probe() {
    let c = match_profile(PROFILE).unwrap().catalog().clone();
    for i in 0..2048 {
        let (_, g) = setup(&c, i, 4096);
        if opening(&g) {
            println!("first opening-only Plate index={i}, seed={:?}", seed(i));
            return;
        }
    }
    panic!("opening bound")
}
fn paid(q: u64) -> Game {
    let c = match_profile(PROFILE).unwrap().catalog().clone();
    let cmd = oracle(&c, "Isamaru, Hound of Konda");
    let plains = oracle(&c, "Plains");
    // First setup-only opening index; retained probe stops before gameplay.
    let (p, mut g) = setup(&c, 0, q);
    assert!(opening(&g));
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut cast, mut equipped, mut blocked, mut returned) = (false, false, false, false);
    let stale_hand = view(&g, PlayerId(0))
        .objects
        .iter()
        .find(|o| o.oracle_id == PLATE && o.zone == (Zone::Hand { owner: PlayerId(0) }))
        .unwrap()
        .handle
        .clone();
    let mut target_receipt = None;
    let mut paid_costs = vec![];
    for _ in 0..2400 {
        let (actor, r) = request(&g);
        let v = view(&g, actor);
        if returned && v.stack.is_empty() {
            let own = view(&g, PlayerId(0));
            let source = own
                .objects
                .iter()
                .find(|o| o.oracle_id == PLATE && o.zone == Zone::Battlefield)
                .unwrap();
            let survivor = own
                .objects
                .iter()
                .find(|o| {
                    o.oracle_id == cmd && o.controller == PlayerId(0) && o.zone == Zone::Battlefield
                })
                .unwrap();
            assert_eq!(source.attached_to, Some(survivor.handle.clone()));
            assert!(survivor.attachments.contains(&source.handle));
            assert!(source.keywords.contains(&KeywordAbility::Indestructible));
            assert!(survivor.keywords.contains(&KeywordAbility::Indestructible));
            assert_eq!(survivor.marked_damage, 2.into());
            assert!(own.objects.iter().any(|o| o.oracle_id == cmd
                && o.owner == PlayerId(1)
                && o.zone == (Zone::Command { owner: PlayerId(1) })));
            assert!(paid_costs.contains(&mtg_contracts::ManaCost {
                generic: 3.into(),
                colored: vec![]
            }));
            assert!(paid_costs.contains(&mtg_contracts::ManaCost {
                generic: 2.into(),
                colored: vec![]
            }));
            assert!(paid_costs.contains(&mtg_contracts::ManaCost {
                generic: 0.into(),
                colored: vec![Color::W]
            }));
            let damage = g
                .events()
                .iter()
                .filter_map(|e| {
                    if let Event::Committed {
                        actual: EventOutcome::DamageCommitted { actual, .. },
                        ..
                    } = e
                    {
                        Some(actual)
                    } else {
                        None
                    }
                })
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(damage.len(), 2);
            assert!(damage.iter().all(|d| d.amount == 2.into()));
            recover(&g, &p, &mut verifier);
            return g;
        }
        let response = match r {
            PlayerRequest::Priority { .. }
                if actor.0 <= 1
                    && v.active == Some(actor)
                    && v.stack.is_empty()
                    && matches!(
                        v.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    ) =>
            {
                let lands = v
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == plains
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    })
                    .count();
                if v.turn.lands_played == 0
                    && let Some(o) = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == plains && o.zone == (Zone::Hand { owner: actor }))
                {
                    PlayerResponse::PlayLand {
                        object: o.handle.clone(),
                    }
                } else if lands >= 1
                    && let Some(o) = v.objects.iter().find(|o| {
                        o.oracle_id == cmd
                            && o.owner == actor
                            && o.zone == (Zone::Command { owner: actor })
                    })
                {
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }
                } else if actor == PlayerId(0) && !cast && lands >= 3 {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == PLATE && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    cast = true;
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }
                } else if actor == PlayerId(0)
                    && cast
                    && !equipped
                    && lands >= 2
                    && let Some(o) = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == PLATE && o.zone == Zone::Battlefield)
                {
                    equipped = true;
                    PlayerResponse::ActivateAbility {
                        object: o.handle.clone(),
                        ability: 0,
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Priority { .. } => PlayerResponse::Pass,
            PlayerRequest::Mana { cost, .. } => {
                let mana = &v.players.iter().find(|p| p.player == actor).unwrap().mana;
                let mut need = cost.generic.clone();
                need.add(&mtg_contracts::Quantity::from(cost.colored.len() as i64));
                if mana.get(&Color::W).cloned().unwrap_or_default() < need {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == plains
                                && o.zone == Zone::Battlefield
                                && o.controller == actor
                                && !o.tapped
                        })
                        .unwrap();
                    PlayerResponse::ActivateMana {
                        object: o.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::SpendMana { cost, .. } => {
                let mut n = cost.generic.clone();
                n.add(&mtg_contracts::Quantity::from(cost.colored.len() as i64));
                paid_costs.push(cost);
                PlayerResponse::SpendMana {
                    spending: BTreeMap::from([(Color::W, n)]),
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let own = v
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == cmd && o.controller == actor && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                assert_eq!(candidates, vec![own.handle.clone()]);
                let foreign = v
                    .objects
                    .iter()
                    .find(|o| {
                        o.oracle_id == cmd
                            && o.controller == PlayerId(1)
                            && o.zone == Zone::Battlefield
                    })
                    .unwrap();
                let before = g.snapshot().unwrap();
                for h in [
                    foreign.handle.clone(),
                    ObjectHandle("unoffered".into()),
                    stale_hand.clone(),
                ] {
                    assert!(matches!(
                        g.check_player_response(
                            actor,
                            request(&g).1.token().clone(),
                            PlayerResponse::Targets { objects: vec![h] },
                            Budget::new(10000)
                        ),
                        QueryResult::Rejected(_)
                    ));
                    assert_eq!(before, g.snapshot().unwrap());
                }
                let response = PlayerResponse::Targets {
                    objects: vec![own.handle.clone()],
                };
                let QueryValue::Validated(receipt) = query(
                    &g,
                    g.check_player_response(
                        actor,
                        request(&g).1.token().clone(),
                        response.clone(),
                        Budget::new(10000),
                    ),
                ) else {
                    panic!("target receipt")
                };
                target_receipt = Some(receipt);
                recover(&g, &p, &mut verifier);
                response
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let defender = v.objects.iter().any(|o| {
                    o.oracle_id == cmd
                        && o.controller == PlayerId(1)
                        && o.zone == Zone::Battlefield
                        && !o.tapped
                });
                let own = v.objects.iter().find(|o| {
                    o.oracle_id == cmd
                        && o.controller == actor
                        && o.zone == Zone::Battlefield
                        && o.keywords.contains(&KeywordAbility::Indestructible)
                        && candidates.contains(&o.handle)
                });
                PlayerResponse::Attackers {
                    attacks: if actor == PlayerId(0) && equipped && !blocked && defender {
                        vec![(own.unwrap().handle.clone(), PlayerId(1))]
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
                    let o = v
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == cmd
                                && o.controller == actor
                                && candidates.contains(&o.handle)
                        })
                        .unwrap();
                    blocked = true;
                    vec![(o.handle.clone(), attackers[0].clone())]
                } else {
                    vec![]
                },
            },
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert!(blocked);
                assert_eq!(power, 2.into());
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert_eq!(actor, PlayerId(1));
                assert!(blocked);
                returned = true;
                recover(&g, &p, &mut verifier);
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.into_iter().take(count).collect(),
            },
            x => panic!("unexpected {x:?}"),
        };
        submit(&mut g, response, q);
        if let Some(receipt) = target_receipt.take() {
            let before = g.snapshot().unwrap();
            assert!(g.submit(receipt).is_err());
            assert_eq!(before, g.snapshot().unwrap());
        }
    }
    panic!("paid pod bound")
}
#[test]
fn plate_paid_pod_real_mana_combat_snapshot_replay_quanta() {
    assert_eq!(
        seed(0),
        [
            74, 87, 152, 17, 162, 171, 145, 163, 164, 69, 137, 234, 206, 14, 240, 65, 87, 204, 235,
            102, 136, 85, 138, 145, 174, 118, 152, 10, 132, 153, 215, 244
        ]
    );
    let a = paid(1);
    let b = paid(4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
