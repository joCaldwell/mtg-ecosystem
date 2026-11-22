//! Actual accepted-input 100-card pod. No injected mana, state or outcomes.
//! Policy observation probes are separate simulator tests.
use mtg_contracts::{Catalog, Color, KeywordAbility, digest, sha256};
use mtg_core::*;
use std::collections::BTreeMap;
const PROFILE: &str = "commander-sacrifice-v1";
const COLLAR: &str = "f5f4dd28-f4ae-4d39-b9b8-6ebfd63c93fe";
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
    let blue = oracle(c, "Caelorna, Coral Tyrant");
    let land = oracle(c, "Plains");
    m.decks = std::array::from_fn(|seat| {
        let mut cards = vec![
            DeckEntry {
                oracle_id: if seat == 1 { blue.clone() } else { cmd.clone() },
                count: 1,
            },
            DeckEntry {
                oracle_id: if seat == 1 {
                    oracle(c, "Island")
                } else {
                    land.clone()
                },
                count: if seat == 0 { 97 } else { 99 },
            },
        ];
        if seat == 0 {
            cards.push(DeckEntry {
                oracle_id: oracle(c, "Disenchant"),
                count: 1,
            });
            cards.push(DeckEntry {
                oracle_id: COLLAR.into(),
                count: 1,
            });
        }
        Decklist {
            commander: if seat == 1 { blue.clone() } else { cmd.clone() },
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
    let h = sha256(format!("commander-sacrifice-v1/opening-collar/{i}").as_bytes());
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
    let v = view(g, PlayerId(0));
    [COLLAR, "a7e97fa9-4b72-4548-b854-5be5f18a6f1a"]
        .iter()
        .all(|id| {
            v.objects
                .iter()
                .any(|o| o.oracle_id == *id && o.zone == (Zone::Hand { owner: PlayerId(0) }))
        })
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
fn collar_admission_exact_catalog_and_default_delta() {
    let profile = match_profile(PROFILE).unwrap();
    let mut c = profile.catalog().clone();
    assert_eq!(c.definitions.len(), 160);
    assert_eq!(c.schema_version, 1);
    let d = c.definition_by_oracle_id(COLLAR).unwrap();
    assert_eq!(d.name, "Basilisk Collar");
    assert!(d.keywords.is_empty());
    c.definitions.retain(|d| d.oracle_id != COLLAR);
    assert_eq!(
        digest(&c).unwrap(),
        "195d409b7fd7e6d364c47073a9262dcfcee24f1c14ad0582aad12dc66d1ee9bc"
    );
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/profiles/commander-sacrifice-v1.json"
    ))
    .unwrap();
    let mut before: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/profiles/commander-sacrifice-v1.json"
    ))
    .unwrap();
    let rows = raw["decks"][0]["cards"].as_array_mut().unwrap();
    rows.retain(|r| r["oracle_id"] != COLLAR);
    rows.iter_mut()
        .find(|r| r["oracle_id"] == oracle(&c, "Plains"))
        .unwrap()["count"] = 56.into();
    // Exact previous profile hash binds all other decks, Plate, Relic and flags.
    assert_eq!(
        digest(&raw).unwrap(),
        "e972431a9ab20a50f416cb462943aedf60a6860b9f3bb1ae26cefc5f2a78d062"
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
        }
        assert!(matches!(
            forced.validate_requested_features(profile.catalog()),
            Err(ProfileError::UnreviewedCapabilities { .. })
        ));
    }
    let m = MatchConfig::for_profile(PROFILE, [2; 32], PlayerId(0)).unwrap();
    validate_pod(profile.catalog(), &m).unwrap();
    for (seat, deck) in m.decks.iter().enumerate() {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|r| r.oracle_id == COLLAR)
                .map(|r| r.count)
                .sum::<u32>(),
            u32::from(seat == 0)
        );
    }
    assert_eq!(
        before["decks"][0]["cards"]
            .as_array_mut()
            .unwrap()
            .iter()
            .find(|r| r["oracle_id"] == oracle(&c, "Plains"))
            .unwrap()["count"],
        55
    );
}
#[test]
#[ignore = "bounded setup-only scan: keeps four hands, stops at Upkeep; no outcome search"]
fn collar_setup_only_opening_probe() {
    let c = match_profile(PROFILE).unwrap().catalog().clone();
    for i in 0..2048 {
        let (_, g) = setup(&c, i, 4096);
        if opening(&g) {
            println!("first opening-only Collar index={i}, seed={:?}", seed(i));
            return;
        }
    }
    panic!("opening bound")
}
fn paid(q: u64) -> Game {
    let c = match_profile(PROFILE).unwrap().catalog().clone();
    let cmd = oracle(&c, "Isamaru, Hound of Konda");
    let plains = oracle(&c, "Plains");
    let blue = oracle(&c, "Caelorna, Coral Tyrant");
    let removal = oracle(&c, "Disenchant");
    // First setup-only opening index; retained probe stops before gameplay.
    let (p, mut g) = setup(&c, 73, q);
    assert!(opening(&g));
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut cast, mut equipped, mut blocked, mut returned) = (false, false, false, false);
    let stale_hand = view(&g, PlayerId(0))
        .objects
        .iter()
        .find(|o| o.oracle_id == COLLAR && o.zone == (Zone::Hand { owner: PlayerId(0) }))
        .unwrap()
        .handle
        .clone();
    let mut target_receipt = None;
    let mut paid_costs = vec![];
    let mut removed = false;
    let mut combat_checked = false;
    for _ in 0..2400 {
        let (actor, r) = request(&g);
        let v = view(&g, actor);
        if returned && v.stack.is_empty() {
            let own = view(&g, PlayerId(0));
            let source = own.objects.iter().find(|o| o.oracle_id == COLLAR).unwrap();
            let survivor = own
                .objects
                .iter()
                .find(|o| {
                    o.oracle_id == cmd && o.controller == PlayerId(0) && o.zone == Zone::Battlefield
                })
                .unwrap();
            if !removed && !combat_checked {
                assert_eq!(source.attached_to, Some(survivor.handle.clone()));
                assert!(survivor.attachments.contains(&source.handle));
                assert!(survivor.keywords.contains(&KeywordAbility::Deathtouch));
                assert!(survivor.keywords.contains(&KeywordAbility::Lifelink));
                assert_eq!(
                    survivor.characteristics.as_ref().unwrap().power,
                    Some(2.into())
                );
                assert!(own.objects.iter().any(
                    |o| o.oracle_id == blue && o.zone == (Zone::Command { owner: PlayerId(1) })
                ));
                assert_eq!(
                    own.players
                        .iter()
                        .find(|p| p.player == PlayerId(0))
                        .unwrap()
                        .life,
                    42.into()
                );
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
                    .filter(|d| d.amount > 0.into())
                    .collect::<Vec<_>>();
                assert_eq!(damage.len(), 1);
                assert_eq!(damage[0].amount, 2.into());
                assert!(
                    damage[0].amount < 8.into(),
                    "deathtouch kills the exact 0/8 blocker"
                );
                combat_checked = true;
                recover(&g, &p, &mut verifier);
            } else if source.zone == (Zone::Graveyard { owner: PlayerId(0) }) {
                assert!(combat_checked);
                assert!(source.attached_to.is_none());
                assert!(survivor.attachments.is_empty());
                assert!(!survivor.keywords.contains(&KeywordAbility::Deathtouch));
                assert!(!survivor.keywords.contains(&KeywordAbility::Lifelink));
                for generic in [1, 2] {
                    assert!(paid_costs.contains(&mtg_contracts::ManaCost {
                        generic: generic.into(),
                        colored: vec![]
                    }));
                }
                assert!(paid_costs.contains(&mtg_contracts::ManaCost {
                    generic: 1.into(),
                    colored: vec![Color::W]
                }));
                recover(&g, &p, &mut verifier);
                let snapshot = g.snapshot().unwrap();
                let mut forged: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
                forged["state"]["players"][0]["life"] = "43".into();
                forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
                let error = match Game::restore(p.clone(), &forged.to_string()) {
                    Ok(_) => panic!("forged life accepted"),
                    Err(e) => e,
                };
                assert_eq!(
                    error,
                    "snapshot is not reachable from this scenario's accepted inputs"
                );
                // Preserve the unmodified positive control after the normalized negative.
                assert_eq!(
                    Game::restore(p.clone(), &snapshot)
                        .unwrap()
                        .state_digest()
                        .unwrap(),
                    g.state_digest().unwrap()
                );
                if let Ok(dir) = std::env::var("COLLAR_EVIDENCE_DIR") {
                    let dir = std::path::Path::new(&dir);
                    std::fs::create_dir_all(dir).unwrap();
                    for (name, bytes) in [
                        (format!("q{q}-snapshot.json"), snapshot),
                        (
                            format!("q{q}-journal.json"),
                            serde_json::to_string_pretty(&g.replay_journal()).unwrap(),
                        ),
                        (format!("q{q}-forged-life.json"), forged.to_string()),
                        (
                            format!("q{q}-events.json"),
                            serde_json::to_string_pretty(g.events()).unwrap(),
                        ),
                    ] {
                        use std::io::Write;
                        std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(dir.join(name))
                            .unwrap()
                            .write_all(bytes.as_bytes())
                            .unwrap();
                    }
                }
                return g;
            }
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
                        o.oracle_id
                            == if actor == PlayerId(1) {
                                oracle(&c, "Island")
                            } else {
                                plains.clone()
                            }
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    })
                    .count();
                if v.turn.lands_played == 0
                    && let Some(o) = v.objects.iter().find(|o| {
                        o.oracle_id
                            == if actor == PlayerId(1) {
                                oracle(&c, "Island")
                            } else {
                                plains.clone()
                            }
                            && o.zone == (Zone::Hand { owner: actor })
                    })
                {
                    PlayerResponse::PlayLand {
                        object: o.handle.clone(),
                    }
                } else if (actor != PlayerId(1) || !returned)
                    && lands >= if actor == PlayerId(1) { 2 } else { 1 }
                    && let Some(o) = v.objects.iter().find(|o| {
                        o.oracle_id
                            == if actor == PlayerId(1) {
                                blue.clone()
                            } else {
                                cmd.clone()
                            }
                            && o.owner == actor
                            && o.zone == (Zone::Command { owner: actor })
                    })
                {
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }
                } else if actor == PlayerId(0) && !cast && lands >= 1 {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == COLLAR && o.zone == (Zone::Hand { owner: actor }))
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
                        .find(|o| o.oracle_id == COLLAR && o.zone == Zone::Battlefield)
                {
                    equipped = true;
                    PlayerResponse::ActivateAbility {
                        object: o.handle.clone(),
                        ability: 0,
                    }
                } else if actor == PlayerId(0) && combat_checked && !removed && lands >= 2 {
                    removed = true;
                    let o = v
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == removal && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    PlayerResponse::Cast {
                        card: o.handle.clone(),
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
                if mana
                    .get(&if actor == PlayerId(1) {
                        Color::U
                    } else {
                        Color::W
                    })
                    .cloned()
                    .unwrap_or_default()
                    < need
                {
                    let o = v
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id
                                == if actor == PlayerId(1) {
                                    oracle(&c, "Island")
                                } else {
                                    plains.clone()
                                }
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
                    spending: BTreeMap::from([(
                        if actor == PlayerId(1) {
                            Color::U
                        } else {
                            Color::W
                        },
                        n,
                    )]),
                }
            }
            PlayerRequest::Targets { candidates, .. } if removed => {
                let o = v
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == COLLAR && o.zone == Zone::Battlefield)
                    .unwrap();
                assert!(candidates.contains(&o.handle));
                PlayerResponse::Targets {
                    objects: vec![o.handle.clone()],
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
                        o.oracle_id == blue
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
                    o.oracle_id == blue
                        && o.controller == PlayerId(1)
                        && o.zone == Zone::Battlefield
                        && !o.tapped
                });
                let own = v.objects.iter().find(|o| {
                    o.oracle_id == cmd
                        && o.controller == actor
                        && o.zone == Zone::Battlefield
                        && o.keywords.contains(&KeywordAbility::Deathtouch)
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
                            o.oracle_id == blue
                                && o.controller == actor
                                && candidates.contains(&o.handle)
                        })
                        .unwrap();
                    assert_eq!(o.characteristics.as_ref().unwrap().power, Some(0.into()));
                    assert_eq!(
                        o.characteristics.as_ref().unwrap().toughness,
                        Some(8.into())
                    );
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
                assert!(power == 2.into() || power == 0.into());
                assert_eq!(recipients.len(), 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert_eq!(actor, PlayerId(1));
                assert!(blocked);
                assert!(
                    v.objects.iter().any(
                        |o| o.oracle_id == blue && o.zone == (Zone::Graveyard { owner: actor })
                    )
                );
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
fn collar_paid_pod_real_mana_combat_snapshot_replay_quanta() {
    let a = paid(1);
    let b = paid(4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
