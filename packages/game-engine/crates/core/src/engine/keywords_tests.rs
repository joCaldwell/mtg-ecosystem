//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-keywords-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)], haste_healer: bool) -> Prepared {
    let mut c = catalog();
    // No admitted source combines haste and a tap ability. This private domain
    // fixture tests CR702.10c composition without adding a card to the pool.
    if haste_healer {
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == HEALER)
            .unwrap();
        d.keywords.push(KeywordAbility::Haste);
        d.capabilities = d.requirements();
    }
    let mut cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, p, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*p),
            controller: PlayerId(*p),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*p),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect::<Vec<_>>();
    for p in 0..4 {
        for _ in 0..4 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(p),
                controller: PlayerId(p),
                zone: Zone::Library { owner: PlayerId(p) },
            });
        }
    }
    let config = ScenarioConfig {
        mode: Mode::CommanderMatch,
        players: 4,
        profile: KEYWORDS_PROFILE.into(),
        commanders: vec![],
        cards,
    };
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game) -> Request {
    match g.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(r) => r,
        x => panic!("{x:?}"),
    }
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn reach(g: &mut Game, step: TurnStep) {
    for _ in 0..200 {
        let r = request(g);
        if g.turn().step == step && matches!(r, Request::Priority { .. }) {
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: false },
            x => panic!("unexpected {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("step not reached")
}
fn attackers(g: &mut Game) -> Vec<ObjectId> {
    for _ in 0..100 {
        let r = request(g);
        if let Request::Attackers { candidates, .. } = r {
            return candidates;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("attackers not reached")
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let replayed = Game::replay_with_trace(p, &g.replay_journal(), g.trace.mode).unwrap();
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace(), g.trace());
    assert!(
        g.state
            .objects
            .values()
            .all(|o| o.zone != Zone::Battlefield || !o.deathtouch_damage)
    );
}
fn traces(p: Prepared, run: impl Fn(&mut Game)) {
    let mut digest0 = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(p.clone(), mode);
        run(&mut g);
        check(&g);
        let d = g.state_digest().unwrap();
        if let Some(old) = &digest0 {
            assert_eq!(&d, old)
        } else {
            digest0 = Some(d)
        }
    }
}
fn cast(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    let obj = object(g, id);
    answer(g, Response::Cast { card: obj.card });
    loop {
        let r = request(g);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    while !g.state.stack.is_empty() {
        answer(g, Response::Pass);
        request(g);
    }
}
#[test]
fn flying_reach_defender_and_vigilance_declarations() {
    // CR509.1b/702.9b/702.17b: legality per pair, defender may block.
    // CR702.20b/508.1f: the vigilance attacker remains untapped.
    for blocker in [
        "Grizzly Bears",
        "Giant Spider",
        "Angelic Wall",
        "Wall of Vines",
    ] {
        let legal = blocker != "Grizzly Bears";
        traces(
            prepared(&[("Serra Angel", 0, false), (blocker, 1, false)], false),
            |g| {
                assert_eq!(attackers(g), vec![object(g, 0)]);
                answer(
                    g,
                    Response::Attackers {
                        attacks: vec![(object(g, 0), PlayerId(1))],
                    },
                );
                loop {
                    let r = request(g);
                    if matches!(r, Request::Blockers { .. }) {
                        break;
                    }
                    g.respond(r.player(), r.decision_id(), Response::Pass)
                        .unwrap();
                }
                assert!(!g.current(object(g, 0)).unwrap().tapped);
                let a = object(g, 0);
                let b = object(g, 1);
                let QueryResult::Complete(QueryValue::Observation(obs)) =
                    g.observe(PlayerId(1), Budget::unlimited())
                else {
                    panic!("observe")
                };
                let Some(PlayerRequest::Blockers {
                    legal_attackers, ..
                }) = obs.decision
                else {
                    panic!("block hints")
                };
                assert_eq!(legal_attackers.len(), 1);
                assert_eq!(legal_attackers[0].attackers.len(), usize::from(legal));
                let r = request(g);
                let result = g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Blockers {
                        blocks: vec![(b, a)],
                    },
                );
                assert_eq!(result.is_ok(), legal);
                if !legal {
                    answer(g, Response::Blockers { blocks: vec![] });
                }
                reach(g, TurnStep::CombatDamage);
                assert_eq!(
                    g.state.players[1].life,
                    Quantity::from(if legal { 40 } else { 36 })
                );
            },
        );
    }
    for wall in ["Angelic Wall", "Wall of Vines"] {
        traces(prepared(&[(wall, 0, false)], false), |g| {
            assert!(attackers(g).is_empty());
            answer(g, Response::Attackers { attacks: vec![] });
            reach(g, TurnStep::PostcombatMain);
        });
    }
}
#[test]
fn freshly_cast_haste_attacks_but_other_creature_waits() {
    // CR302.6/702.10b: only haste bypasses continuous control.
    traces(
        prepared(
            &[
                ("Banehound", 0, true),
                ("Typhoid Rats", 0, true),
                ("Swamp", 0, false),
                ("Swamp", 0, false),
            ],
            false,
        ),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            cast(g, 0, vec![]);
            cast(g, 1, vec![]);
            let a = object(g, 0);
            let b = object(g, 1);
            assert!(!g.current(a).unwrap().attack_ready);
            assert!(!g.current(b).unwrap().attack_ready);
            assert_eq!(attackers(g), vec![a]);
            let r = request(g);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Attackers {
                        attacks: vec![(b, PlayerId(1))]
                    }
                )
                .is_err()
            );
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(a, PlayerId(1))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) {
                    answer(g, Response::Blockers { blocks: vec![] });
                    break;
                }
                answer(g, Response::Pass);
            }
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[0].life, 41.into());
            assert_eq!(g.state.players[1].life, 39.into());
        },
    );
}
#[test]
fn deathtouch_and_lifelink_use_actual_damage_after_fog_and_healer() {
    // CR702.2b/704.5h: two damage destroys a six-toughness Yargle.
    // CR120.3f/702.15b/510.2: gain only final positive damage, in same event.
    for (prevent, expected_gain, dead) in [(0, 2, true), (1, 1, true), (2, 0, false)] {
        traces(
            prepared(
                &[
                    ("Yargle and Multani", 0, false),
                    ("Vampire Nighthawk", 1, false),
                    ("Samite Healer", 0, false),
                    ("Fog", 0, true),
                    ("Forest", 0, false),
                ],
                false,
            ),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                if prevent == 2 {
                    cast(g, 3, vec![]);
                }
                if prevent == 1 {
                    answer(
                        g,
                        Response::ActivateAbility {
                            object: object(g, 2),
                            ability: 0,
                        },
                    );
                    answer(
                        g,
                        Response::EntityTargets {
                            entities: vec![TargetEntity::Object {
                                object: object(g, 0),
                            }],
                        },
                    );
                    while !g.state.stack.is_empty() {
                        answer(g, Response::Pass);
                        request(g);
                    }
                }
                let a = object(g, 0);
                let b = object(g, 1);
                attackers(g);
                answer(
                    g,
                    Response::Attackers {
                        attacks: vec![(a, PlayerId(1))],
                    },
                );
                loop {
                    let r = request(g);
                    if matches!(r, Request::Blockers { .. }) {
                        break;
                    }
                    answer(g, Response::Pass);
                }
                answer(
                    g,
                    Response::Blockers {
                        blocks: vec![(b, a)],
                    },
                );
                reach(g, TurnStep::CombatDamage);
                assert_eq!(g.state.players[1].life, Quantity::from(40 + expected_gain));
                assert_eq!(
                    g.zone(object(g, 0)),
                    Some(if dead {
                        Zone::Graveyard { owner: PlayerId(0) }
                    } else {
                        Zone::Battlefield
                    })
                );
                assert_eq!(
                    g.zone(object(g, 1)),
                    Some(if prevent == 2 {
                        Zone::Battlefield
                    } else {
                        Zone::Graveyard { owner: PlayerId(1) }
                    })
                );
                if g.trace.mode == TraceMode::Detailed {
                    let gains = g
                        .events()
                        .iter()
                        .filter_map(|e| match e {
                            Event::Committed {
                                actual: EventOutcome::DamageCommitted { life_gains, .. },
                                ..
                            } => Some(life_gains),
                            _ => None,
                        })
                        .flatten()
                        .collect::<Vec<_>>();
                    assert_eq!(gains.len(), usize::from(expected_gain > 0));
                    if expected_gain > 0 {
                        assert_eq!(gains[0].amount, Quantity::from(expected_gain));
                    }
                }
            },
        );
    }
}
#[test]
fn lifelink_sources_gain_separately_and_deathtouch_blocker_destroys_attacker() {
    traces(
        prepared(
            &[
                ("Healer's Hawk", 0, false),
                ("Ajani's Sunstriker", 0, false),
                ("Typhoid Rats", 1, false),
                ("Yoked Ox", 0, false),
            ],
            false,
        ),
        |g| {
            let a = object(g, 0);
            let b = object(g, 1);
            attackers(g);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(a, PlayerId(1)), (b, PlayerId(1))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) {
                    break;
                }
                answer(g, Response::Pass);
            }
            answer(
                g,
                Response::Blockers {
                    blocks: vec![(object(g, 2), b)],
                },
            );
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[0].life, 43.into());
            assert_eq!(g.state.players[1].life, 39.into());
            assert_eq!(
                g.zone(object(g, 1)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            if g.trace.mode == TraceMode::Detailed {
                let gains = g
                    .events()
                    .iter()
                    .find_map(|e| match e {
                        Event::Committed {
                            actual: EventOutcome::DamageCommitted { life_gains, .. },
                            ..
                        } => Some(life_gains),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(gains.len(), 2);
                assert_eq!(
                    gains
                        .iter()
                        .map(|g| g.amount.to_string())
                        .collect::<Vec<_>>(),
                    ["1", "2"]
                );
            }
        },
    );
}
#[test]
fn fresh_tap_cost_requires_continuous_control_or_haste() {
    // CR702.10c/302.6 also governs payment after targets are chosen.
    for haste in [false, true] {
        traces(
            prepared(
                &[
                    ("Samite Healer", 0, true),
                    ("Plains", 0, false),
                    ("Plains", 0, false),
                ],
                haste,
            ),
            |g| {
                reach(g, TurnStep::PrecombatMain);
                cast(g, 0, vec![]);
                let h = object(g, 0);
                assert!(!g.current(h).unwrap().attack_ready);
                let r = request(g);
                let result = g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::ActivateAbility {
                        object: h,
                        ability: 0,
                    },
                );
                assert_eq!(result.is_ok(), haste);
                if haste {
                    answer(
                        g,
                        Response::EntityTargets {
                            entities: vec![TargetEntity::Player {
                                player: PlayerId(0),
                            }],
                        },
                    );
                    request(g);
                    assert!(g.current(h).unwrap().tapped);
                    while !g.state.stack.is_empty() {
                        answer(g, Response::Pass);
                        request(g);
                    }
                    assert!(
                        g.state.effects.iter().any(|e| matches!(
                            e.kind,
                            ResolvedEffectKind::PreventNextDamage { .. }
                        ))
                    );
                }
            },
        );
    }
}
#[test]
fn lifelink_and_commander_damage_remain_distinct_in_both_combat_paths() {
    // CR120.3f/510.2/903.10a: Yargle deals18 Commander damage while the
    // defending Sunstriker deals2 to Bears. Life40 -18 +2 =24; history18.
    for effects in [false, true] {
        let mut p = prepared(
            &[
                ("Yargle and Multani", 0, false),
                ("Grizzly Bears", 0, false),
                ("Ajani's Sunstriker", 1, false),
            ],
            false,
        );
        p.config.commanders.push(CardId(0));
        if !effects {
            p.config.mode = Mode::Scenario;
            p.config.profile = "four-seat-commander-damage-v1".into();
        }
        p.profile_digest = digest(&p.config).unwrap();
        traces(p, |g| {
            attackers(g);
            let commander = object(g, 0);
            let bears = object(g, 1);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(commander, PlayerId(1)), (bears, PlayerId(1))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) {
                    break;
                }
                answer(g, Response::Pass);
            }
            answer(
                g,
                Response::Blockers {
                    blocks: vec![(object(g, 2), bears)],
                },
            );
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 24.into());
            assert_eq!(
                g.state
                    .commanders
                    .history(commander.card)
                    .unwrap()
                    .combat_damage[&PlayerId(1)],
                18.into()
            );
        });
    }
}
#[test]
fn healer_fully_prevented_deathtouch_never_marks_or_destroys() {
    // CR615.6/702.2b: the Rats deal no damage after a one-point shield.
    traces(
        prepared(
            &[
                ("Ajani's Sunstriker", 0, false),
                ("Typhoid Rats", 1, false),
                ("Samite Healer", 0, false),
            ],
            false,
        ),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(
                g,
                Response::ActivateAbility {
                    object: object(g, 2),
                    ability: 0,
                },
            );
            answer(
                g,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Object {
                        object: object(g, 0),
                    }],
                },
            );
            while !g.state.stack.is_empty() {
                answer(g, Response::Pass);
                request(g);
            }
            attackers(g);
            let a = object(g, 0);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(a, PlayerId(1))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) {
                    break;
                }
                answer(g, Response::Pass);
            }
            answer(
                g,
                Response::Blockers {
                    blocks: vec![(object(g, 1), a)],
                },
            );
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.zone(a), Some(Zone::Battlefield));
            assert_eq!(g.current(a).unwrap().marked_damage, 0.into());
            assert_eq!(g.state.players[0].life, 42.into());
        },
    );
}
#[test]
fn keyword_profile_admission_policy_and_historical_identity_are_separate() {
    let c = catalog();
    let config = MatchConfig::keywords([5; 32], PlayerId(2));
    let p = Prepared::admit_pod(c.clone(), config.clone()).unwrap();
    assert_eq!(p.config.cards.len(), 400);
    assert_eq!(p.catalog.schema_version, 12);
    for deck in &config.decks {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
    }
    for deck in [keywords_white_deck(), keywords_black_green_deck()] {
        let mut x = config.clone();
        x.decks = std::array::from_fn(|_| deck.clone());
        assert!(Prepared::admit_pod(c.clone(), x).is_ok());
    }
    let old = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    assert_eq!(digest(&old).unwrap(), MATCH_CATALOG_SHA256);
    assert_eq!(
        policy_digest(),
        "cfe83bd72e477756a7b4f778e22c6ec8e901489671d84d122574ecdc7b90caad"
    );
    assert!(Prepared::admit_pod(old, config).is_err());
    assert!(Prepared::admit_pod(c, MatchConfig::foundations([5; 32], PlayerId(2))).is_err());
    let profile = match_profile(KEYWORDS_PROFILE).unwrap();
    let policy: serde_json::Value = strict_read(profile.policy_json).unwrap();
    assert_eq!(policy["admitted_pool_review"].as_array().unwrap().len(), 50);
}
#[test]
fn two_life_defender_survives_simultaneous_three_damage_and_two_lifelink() {
    // CR510.2/120.3f/704.5a: after an earlier unblocked Bears hit, player1
    // is at2. Next combat deals3 to that player and2 from their Sunstriker:
    // life2 -3 +2 =1, before any SBA. Neither damage nor gain is a trigger.
    let mut p = prepared(
        &[
            ("Grizzly Bears", 0, false),
            ("Slaughterhouse Bouncer", 0, false),
            ("Ajani's Sunstriker", 1, false),
        ],
        false,
    );
    p.config.mode = Mode::Scenario;
    p.config.profile = "four-seat-effects-low-v1".into();
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        attackers(g);
        answer(
            g,
            Response::Attackers {
                attacks: vec![(object(g, 0), PlayerId(1))],
            },
        );
        loop {
            let r = request(g);
            if matches!(r, Request::Blockers { .. }) {
                break;
            }
            answer(g, Response::Pass);
        }
        answer(g, Response::Blockers { blocks: vec![] });
        reach(g, TurnStep::CombatDamage);
        assert_eq!(g.state.players[1].life, 2.into());
        for _ in 0..300 {
            let r = request(g);
            if matches!(r, Request::Attackers { .. }) && g.state.active == Some(PlayerId(0)) {
                break;
            }
            let response = match r {
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Priority { .. } => Response::Pass,
                x => panic!("{x:?}"),
            };
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        assert_eq!(g.state.active, Some(PlayerId(0)));
        answer(
            g,
            Response::Attackers {
                attacks: vec![(object(g, 0), PlayerId(1)), (object(g, 1), PlayerId(1))],
            },
        );
        loop {
            let r = request(g);
            if matches!(r, Request::Blockers { .. }) {
                break;
            }
            answer(g, Response::Pass);
        }
        answer(
            g,
            Response::Blockers {
                blocks: vec![(object(g, 2), object(g, 0))],
            },
        );
        reach(g, TurnStep::CombatDamage);
        assert_eq!(g.state.players[1].life, 1.into());
        assert!(g.is_live(PlayerId(1)));
    });
}
#[test]
fn assigned_departed_combat_source_uses_keyword_lki() {
    // CR702.2e/702.15c/510.2: a frozen source retains its keywords after
    // its owner leaves. The borrowed source's living controller still gains.
    let mut p = prepared(
        &[("Vampire Nighthawk", 0, false), ("Giant Spider", 1, false)],
        false,
    );
    p.config.cards[0].owner = PlayerId(2);
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        attackers(g);
        let a = object(g, 0);
        let b = object(g, 1);
        answer(
            g,
            Response::Attackers {
                attacks: vec![(a, PlayerId(1))],
            },
        );
        loop {
            let r = request(g);
            if matches!(r, Request::Blockers { .. }) {
                break;
            }
            answer(g, Response::Pass);
        }
        answer(
            g,
            Response::Blockers {
                blocks: vec![(b, a)],
            },
        );
        // Stop at the actual assignment boundary, not at a fabricated ledger.
        for _ in 0..100 {
            if matches!(g.state.frame, Frame::DamageEffects { .. }) {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::AwaitingDecision(r) => g
                    .respond(r.player(), r.decision_id(), Response::Pass)
                    .unwrap(),
                Advance::Suspended { .. } => {}
                x => panic!("{x:?}"),
            }
        }
        assert!(matches!(g.state.frame, Frame::DamageEffects { .. }));
        g.concede(PlayerId(2)).unwrap();
        reach(g, TurnStep::CombatDamage);
        assert!(g.object_id(CardId(0)).is_none());
        assert_eq!(g.state.players[0].life, 42.into());
        assert_eq!(
            g.zone(object(g, 1)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
    });
}
#[test]
fn noncombat_departed_source_lifelink_and_deathtouch_both_damage_paths() {
    // No source in this finite pool naturally combines these keywords and a
    // damage activation. A private Sorcerer domain checks CR702.2d-e/15c-d,
    // 113.7a/608.2h without broadening compiler or match admission.
    for effects in [false, true] {
        let mut p = prepared(
            &[("Prodigal Sorcerer", 0, false), ("Yoked Ox", 1, false)],
            false,
        );
        let c = Arc::make_mut(&mut p.catalog);
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Prodigal Sorcerer")
            .unwrap();
        d.keywords = vec![KeywordAbility::Lifelink, KeywordAbility::Deathtouch];
        d.capabilities = d.requirements();
        p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
        p.config.cards[0].owner = PlayerId(2);
        if !effects {
            p.config.mode = Mode::Scenario;
            p.config.profile = "four-seat-damage-life-v1".into();
        }
        p.profile_digest = digest(&p.config).unwrap();
        traces(p, |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(
                g,
                Response::ActivateAbility {
                    object: object(g, 0),
                    ability: 0,
                },
            );
            answer(
                g,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Object {
                        object: object(g, 1),
                    }],
                },
            );
            request(g);
            assert!(g.current(object(g, 0)).unwrap().tapped);
            g.concede(PlayerId(2)).unwrap();
            request(g);
            while !g.state.stack.is_empty() {
                answer(g, Response::Pass);
                request(g);
            }
            assert!(g.object_id(CardId(0)).is_none());
            assert_eq!(g.state.players[0].life, 41.into());
            assert_eq!(
                g.zone(object(g, 1)),
                Some(Zone::Graveyard { owner: PlayerId(1) })
            );
        });
    }
}
#[test]
fn lifelink_beneficiary_is_the_source_controller_not_invocation_controller() {
    // CR702.15b: "you" in a damage program and the source's lifelink
    // beneficiary are distinct identities. Isolate that private quantity rule.
    let mut g = Game::new(prepared(&[("Vampire Nighthawk", 0, false)], false));
    let source = g.object_before(object(&g, 0)).unwrap();
    let gains = g
        .damage_keywords(&[ActualDamage {
            id: DamageId(0),
            source,
            controller: PlayerId(1),
            recipient: DamageRecipient::Player {
                player: PlayerId(2),
            },
            amount: 2.into(),
        }])
        .unwrap();
    assert_eq!(gains.len(), 1);
    assert_eq!(gains[0].player, PlayerId(0));
    assert_eq!(g.state.players[0].life, 42.into());
    assert_eq!(g.state.players[1].life, 40.into());
}
