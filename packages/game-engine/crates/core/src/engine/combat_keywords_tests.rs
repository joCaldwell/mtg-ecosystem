//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-combat-keywords-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let c = catalog();
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
        profile: COMBAT_KEYWORDS_PROFILE.into(),
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

fn declare(g: &mut Game, attacks: &[(u32, u8)], blocks: &[(u32, u32)]) {
    attackers(g);
    answer(
        g,
        Response::Attackers {
            attacks: attacks
                .iter()
                .map(|(o, p)| (object(g, *o), PlayerId(*p)))
                .collect(),
        },
    );
    loop {
        if matches!(request(g), Request::Blockers { .. }) {
            break;
        }
        answer(g, Response::Pass);
    }
    answer(
        g,
        Response::Blockers {
            blocks: blocks
                .iter()
                .map(|(b, a)| (object(g, *b), object(g, *a)))
                .collect(),
        },
    );
}
fn damage_request(g: &mut Game) -> Request {
    loop {
        let r = request(g);
        if matches!(r, Request::Damage { .. }) {
            return r;
        }
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        answer(g, Response::Pass);
    }
}

#[test]
fn trample_excess_lethal_and_public_request() {
    // CR702.19b/510.1e: Dreadmaw6 vs Bears2, lethal2 and excess4.
    traces(
        prepared(&[("Colossal Dreadmaw", 0, false), ("Grizzly Bears", 1, false)]),
        |g| {
            declare(g, &[(0, 1)], &[(1, 0)]);
            let r = damage_request(g);
            let (a, b) = (object(g, 0), object(g, 1));
            assert!(
                matches!(r,Request::Damage { player_recipient:Some(PlayerId(1)), ref lethal_requirements,.. } if lethal_requirements==&vec![(b,2.into())])
            );
            let view = g.observe(PlayerId(0), Budget::unlimited());
            let QueryResult::Complete(QueryValue::Observation(observation)) = view else {
                panic!("observation")
            };
            assert!(
                matches!(&observation.decision,Some(PlayerRequest::Damage {player_recipient:Some(PlayerId(1)),lethal_requirements,..}) if lethal_requirements.len()==1)
            );
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::TrampleDamage {
                        assignments: vec![(b, 1.into())],
                        player: PlayerId(1),
                        amount: 5.into()
                    }
                )
                .is_err()
            );
            let Some(PlayerRequest::Damage {
                token, recipients, ..
            }) = observation.decision
            else {
                panic!("damage request")
            };
            let QueryResult::Complete(QueryValue::Validated(receipt)) = g.check_player_response(
                PlayerId(0),
                token,
                PlayerResponse::TrampleDamage {
                    assignments: vec![(recipients[0].clone(), 2.into())],
                    player: PlayerId(1),
                    amount: 4.into(),
                },
                Budget::unlimited(),
            ) else {
                panic!("public response validation")
            };
            g.submit(receipt).unwrap();
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 36.into());
            assert_eq!(g.current(a).unwrap().marked_damage, 2.into());
            assert!(matches!(
                g.state.objects[&b.card].zone,
                Zone::Graveyard { .. }
            ));
            assert!(g.state.first_strike_participants.is_empty());
        },
    );
}
#[test]
fn trample_deathtouch_private_domain_lethal_one() {
    // Private fixture: no admitted creature combines these keywords.
    // CR702.2c/702.19b: one assigned damage is lethal, before prevention.
    let mut p = prepared(&[("Colossal Dreadmaw", 0, false), ("Yoked Ox", 1, false)]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Colossal Dreadmaw")
        .unwrap();
    d.keywords.push(KeywordAbility::Deathtouch);
    d.capabilities = d.requirements();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        declare(g, &[(0, 1)], &[(1, 0)]);
        damage_request(g);
        let b = object(g, 1);
        assert_eq!(g.lethal_requirement(object(g, 0), b).unwrap(), 1.into());
        answer(
            g,
            Response::TrampleDamage {
                assignments: vec![(b, 1.into())],
                player: PlayerId(1),
                amount: 5.into(),
            },
        );
        reach(g, TurnStep::CombatDamage);
        assert_eq!(g.state.players[1].life, 35.into());
        assert!(matches!(
            g.state.objects[&b.card].zone,
            Zone::Graveyard { .. }
        ));
    });
}
#[test]
fn trample_after_blocker_removed() {
    // CR702.19d: blocked status survives, all6 now assigned to the player.
    traces(
        prepared(&[
            ("Colossal Dreadmaw", 0, false),
            ("Grizzly Bears", 1, false),
            ("Disfigure", 0, true),
            ("Swamp", 0, false),
        ]),
        |g| {
            declare(g, &[(0, 1)], &[(1, 0)]);
            reach(g, TurnStep::DeclareBlockers);
            cast(
                g,
                2,
                vec![TargetEntity::Object {
                    object: object(g, 1),
                }],
            );
            assert!(g.state.combat[0].blocked);
            assert!(g.state.combat[0].blockers.is_empty());
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 34.into());
        },
    );
}
#[test]
fn first_strike_kills_before_regular_damage_and_restore_between_steps() {
    // CR510.4/702.7b/704.5g: Bears dies before it can hit Knight.
    traces(
        prepared(&[("Youthful Knight", 0, false), ("Grizzly Bears", 1, false)]),
        |g| {
            declare(g, &[(0, 1)], &[(1, 0)]);
            reach(g, TurnStep::FirstStrikeDamage);
            assert_eq!(g.current(object(g, 0)).unwrap().marked_damage, 0.into());
            assert!(matches!(
                g.state.objects[&CardId(1)].zone,
                Zone::Graveyard { .. }
            ));
            check(g);
            let mut restored =
                Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
            reach(&mut restored, TurnStep::CombatDamage);
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state, restored.state);
            assert_eq!(g.state.players[1].life, 40.into());
        },
    );
}
#[test]
fn double_strike_and_regular_creature_use_separate_steps() {
    // Ace1 twice, unblocked Bears2 only in the second step: 40->39->36.
    traces(
        prepared(&[("Fencing Ace", 0, false), ("Grizzly Bears", 0, false)]),
        |g| {
            declare(g, &[(0, 1), (1, 1)], &[]);
            reach(g, TurnStep::FirstStrikeDamage);
            assert_eq!(g.state.players[1].life, 39.into());
            check(g);
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 36.into());
        },
    );
}
#[test]
fn double_strike_lifelink_and_commander_history_private_fixture() {
    // Private keyword composition on an eligible, source-backed commander.
    // CR702.4b/120.3f/903.10a: life40->42->44, physical history0->2->4.
    let mut p = prepared(&[("Isamaru, Hound of Konda", 0, false)]);
    p.config.commanders.push(CardId(0));
    p.profile_digest = digest(&p.config).unwrap();
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Isamaru, Hound of Konda")
        .unwrap();
    d.keywords
        .extend([KeywordAbility::DoubleStrike, KeywordAbility::Lifelink]);
    d.capabilities = d.requirements();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        declare(g, &[(0, 1)], &[]);
        for (step, n) in [
            (TurnStep::FirstStrikeDamage, 1),
            (TurnStep::CombatDamage, 2),
        ] {
            reach(g, step);
            assert_eq!(g.state.players[0].life, Quantity::from(40 + 2 * n));
            assert_eq!(
                g.state.commanders.history(CardId(0)).unwrap().combat_damage[&PlayerId(1)],
                Quantity::from(2 * n)
            );
            check(g);
        }
    });
}
#[test]
fn fog_prevents_both_steps() {
    // CR615.1/702.4b: the turn-scoped prevention remains for the second step.
    traces(
        prepared(&[
            ("Fencing Ace", 0, false),
            ("Fog", 0, true),
            ("Forest", 0, false),
        ]),
        |g| {
            declare(g, &[(0, 1)], &[]);
            reach(g, TurnStep::DeclareBlockers);
            cast(g, 1, vec![]);
            reach(g, TurnStep::FirstStrikeDamage);
            assert_eq!(g.state.players[1].life, 40.into());
            check(g);
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 40.into());
        },
    );
}
#[test]
fn menace_complete_declaration_rejects_one_accepts_two() {
    // CR509.1b/702.111b, no damage assignment order (510.1c).
    traces(
        prepared(&[
            ("Cursed Minotaur", 0, false),
            ("Grizzly Bears", 1, false),
            ("Grizzly Bears", 1, false),
        ]),
        |g| {
            attackers(g);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(object(g, 0), PlayerId(1))],
                },
            );
            loop {
                if matches!(request(g), Request::Blockers { .. }) {
                    break;
                }
                answer(g, Response::Pass);
            }
            let QueryResult::Complete(QueryValue::Observation(obs)) =
                g.observe(PlayerId(1), Budget::unlimited())
            else {
                panic!("block observation")
            };
            assert!(
                matches!(obs.decision,Some(PlayerRequest::Blockers {ref menace_attackers,..}) if menace_attackers.len()==1)
            );
            let r = request(g);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Blockers {
                        blocks: vec![(object(g, 1), object(g, 0))]
                    }
                )
                .is_err()
            );
            answer(
                g,
                Response::Blockers {
                    blocks: vec![(object(g, 1), object(g, 0)), (object(g, 2), object(g, 0))],
                },
            );
            damage_request(g);
            answer(
                g,
                Response::Damage {
                    assignments: vec![(object(g, 1), 2.into()), (object(g, 2), 1.into())],
                },
            );
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 40.into());
            assert!(matches!(
                g.state.objects[&CardId(0)].zone,
                Zone::Graveyard { .. }
            ));
            assert!(matches!(
                g.state.objects[&CardId(1)].zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 1.into());
        },
    );
}
#[test]
fn no_extra_step_without_first_or_double_strike() {
    traces(prepared(&[("Grizzly Bears", 0, false)]), |g| {
        declare(g, &[(0, 1)], &[]);
        reach(g, TurnStep::CombatDamage);
        assert!(g.state.first_strike_participants.is_empty());
        assert_eq!(g.state.players[1].life, 38.into());
        reach(g, TurnStep::EndCombat);
    });
}
#[test]
fn combat_keywords_admission_is_separate() {
    let c = catalog();
    let cfg = MatchConfig::combat_keywords([5; 32], PlayerId(0));
    let p = Prepared::admit_pod(c.clone(), cfg.clone()).unwrap();
    assert_eq!(p.config.cards.len(), 400);
    assert_eq!(digest(&c).unwrap(), COMBAT_KEYWORDS_CATALOG_SHA256);
    for deck in &cfg.decks {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
    }
    for deck in [
        combat_keywords_white_deck(),
        combat_keywords_black_green_deck(),
    ] {
        let mut x = cfg.clone();
        x.decks = std::array::from_fn(|_| deck.clone());
        Prepared::admit_pod(c.clone(), x).unwrap();
    }
    assert!(Prepared::admit_pod(c.clone(), MatchConfig::keywords([5; 32], PlayerId(0))).is_err());
    assert!(Prepared::admit_pod(c, MatchConfig::foundations([5; 32], PlayerId(0))).is_err());
}
#[test]
fn blood_artist_sbas_triggers_and_priority_between_steps() {
    // CR510.3/117.5/704.3/603.3: first-strike kills Bears; put Artist's
    // trigger on stack and resolve it before the ordinary Bears deals2.
    traces(
        prepared(&[
            ("Youthful Knight", 0, false),
            ("Grizzly Bears", 1, false),
            ("Blood Artist", 0, false),
            ("Grizzly Bears", 0, false),
        ]),
        |g| {
            declare(g, &[(0, 1), (3, 1)], &[(1, 0)]);
            let mut saw_target = false;
            loop {
                let r = request(g);
                match r {
                    Request::PlayerTargets { .. } => {
                        assert_eq!(g.turn().step, TurnStep::FirstStrikeDamage);
                        assert!(matches!(
                            g.state.objects[&CardId(1)].zone,
                            Zone::Graveyard { .. }
                        ));
                        saw_target = true;
                        answer(
                            g,
                            Response::PlayerTargets {
                                players: vec![PlayerId(1)],
                            },
                        );
                    }
                    Request::Priority { .. } if g.turn().step == TurnStep::FirstStrikeDamage => {
                        break;
                    }
                    Request::Priority { .. } => answer(g, Response::Pass),
                    x => panic!("{x:?}"),
                }
            }
            assert!(saw_target);
            assert_eq!(g.state.stack.len(), 1);
            assert_eq!(g.state.priority, PlayerId(0));
            check(g);
            while !g.state.stack.is_empty() {
                answer(g, Response::Pass);
                request(g);
            }
            assert_eq!(g.turn().step, TurnStep::FirstStrikeDamage);
            assert_eq!(g.state.players[0].life, 41.into());
            assert_eq!(g.state.players[1].life, 39.into());
            check(g);
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 37.into());
        },
    );
}
#[test]
fn removal_between_steps_and_first_strike_blocker() {
    // CR702.7b: a first-strike blocker can cause the extra step itself.
    traces(
        prepared(&[("Grizzly Bears", 0, false), ("Youthful Knight", 1, false)]),
        |g| {
            declare(g, &[(0, 1)], &[(1, 0)]);
            reach(g, TurnStep::FirstStrikeDamage);
            check(g);
            assert!(matches!(
                g.state.objects[&CardId(0)].zone,
                Zone::Graveyard { .. }
            ));
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 0.into());
        },
    );
    // CR506.4/702.4b: removed double striker never assigns in the second step.
    traces(
        prepared(&[
            ("Fencing Ace", 0, false),
            ("Disfigure", 0, true),
            ("Swamp", 0, false),
        ]),
        |g| {
            declare(g, &[(0, 1)], &[]);
            reach(g, TurnStep::FirstStrikeDamage);
            cast(
                g,
                1,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            check(g);
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 39.into());
        },
    );
}
#[test]
fn trample_marked_damage_and_other_step_assignments_private_domain() {
    // Private state domain to exercise 702.19b's whole-step arithmetic. The
    // public pool cannot have one creature blocking multiple attackers.
    let p = prepared(&[
        ("Colossal Dreadmaw", 0, false),
        ("Yoked Ox", 1, false),
        ("Grizzly Bears", 0, false),
    ]);
    let mut g = Game::new(p);
    let (a, b, c) = (object(&g, 0), object(&g, 1), object(&g, 2));
    g.state.turn.step = TurnStep::CombatDamage;
    g.state.combat = vec![
        Attacker {
            attacker: a,
            defender: PlayerId(1),
            blocked: true,
            blockers: vec![b],
        },
        Attacker {
            attacker: c,
            defender: PlayerId(1),
            blocked: true,
            blockers: vec![b],
        },
    ];
    g.state.objects.get_mut(&b.card).unwrap().marked_damage = 1.into();
    // Ox toughness4 - marked1 - other Bears assignment2 = lethal1.
    assert_eq!(g.lethal_requirement(a, b).unwrap(), 1.into());
    g.state.frame = Frame::DamageChoice { source: a };
    assert!(
        g.validate_combat_response(
            PlayerId(0),
            &Response::TrampleDamage {
                assignments: vec![(b, 1.into())],
                player: PlayerId(1),
                amount: 5.into()
            }
        )
        .is_ok()
    );
    assert!(
        g.validate_combat_response(
            PlayerId(0),
            &Response::TrampleDamage {
                assignments: vec![(b, 0.into())],
                player: PlayerId(1),
                amount: 6.into()
            }
        )
        .is_err()
    );
    assert!(
        g.validate_combat_response(
            PlayerId(0),
            &Response::TrampleDamage {
                assignments: vec![(b, 1.into())],
                player: PlayerId(1),
                amount: 4.into()
            }
        )
        .is_err()
    );
}
#[test]
fn first_strike_blocker_of_departed_attacker_still_starts_first_step() {
    // CR506.4/509.1h: the Knight remains a blocking creature after its
    // attacker leaves; CR510.4 then requires a first-strike damage step even
    // though it assigns no damage (510.1d). Unblocked Bears deal damage only
    // in the second step.
    traces(
        prepared(&[
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 0, false),
            ("Youthful Knight", 1, false),
            ("Disfigure", 0, true),
            ("Swamp", 0, false),
        ]),
        |g| {
            declare(g, &[(0, 1), (1, 1)], &[(2, 0)]);
            cast(
                g,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            reach(g, TurnStep::FirstStrikeDamage);
            check(g);
            assert!(matches!(
                g.state.objects[&CardId(0)].zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(g.state.players[1].life, 40.into());
            reach(g, TurnStep::CombatDamage);
            assert_eq!(g.state.players[1].life, 38.into());
            assert_eq!(g.current(object(g, 2)).unwrap().marked_damage, 0.into());
            reach(g, TurnStep::PostcombatMain);
            assert!(g.state.orphan_first_strike_blockers.is_empty());
        },
    );
}
