//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-attachments-ir.json"
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
        profile: ATTACHMENTS_PROFILE.into(),
        commanders: rows
            .iter()
            .enumerate()
            .filter(|(_, (name, _, _))| {
                matches!(
                    *name,
                    "Isamaru, Hound of Konda"
                        | "Yargle and Multani"
                        | "Caelorna, Coral Tyrant"
                        | "Rorix Bladewing"
                )
            })
            .map(|(i, _)| CardId(i as u32))
            .collect(),
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
            Request::DamageEffect { ref candidates, .. } => Response::DamageEffect {
                effect: candidates[0].effect.clone(),
                damage: candidates[0].damage,
            },
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
    let mut replayed = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    if matches!(g.state.frame, Frame::Fault { .. }) {
        let _ = replayed.advance(Budget::new(1));
    }
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace(), g.trace());
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
fn announce(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    if g.turn().step == TurnStep::Untap {
        reach(g, TurnStep::PrecombatMain);
    }
    let obj = object(g, id);
    answer(g, Response::Cast { card: obj.card });
    pay_proposal(g, targets, None);
}
fn pay_proposal(g: &mut Game, targets: Vec<TargetEntity>, stack_target: Option<StackId>) {
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
                        .filter(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .min_by_key(|o| {
                            usize::from(
                                !g.mana_abilities(o.id)
                                    .unwrap()
                                    .iter()
                                    .any(|c| Some(c) == cost.colored.first()),
                            )
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
            Request::StackTargets { .. } => Response::StackTargets {
                entries: vec![stack_target.unwrap_or(g.state.stack[0].id)],
            },
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}

fn automatic(r: &Request, targets: &[TargetEntity]) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::Targets { .. } => Response::Targets {
            objects: targets
                .iter()
                .map(|t| match t {
                    TargetEntity::Object { object } => *object,
                    _ => panic!("creature target"),
                })
                .collect(),
        },
        Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: false },
        Request::OptionalRecursion { .. } => Response::OptionalRecursion { perform: true },
        Request::ResolveDiscard { candidates, .. } => Response::ResolveDiscard {
            objects: vec![candidates[0]],
        },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        Request::PlayerTargets { .. } => Response::PlayerTargets {
            players: vec![PlayerId(1)],
        },
        x => panic!("unexpected resolution request {x:?}"),
    }
}
fn resolve_with(g: &mut Game, targets: Vec<TargetEntity>, mut inspect: impl FnMut(&Game)) {
    for _ in 0..2000 {
        let advance = g.advance(Budget::new(1));
        inspect(g);
        match advance {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.stack.is_empty()
                    && g.state.pending_triggers.is_empty()
                {
                    return;
                }
                let response = automatic(&r, &targets);
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("resolve {x:?}"),
        }
    }
    panic!("resolution budget")
}
fn resolve(g: &mut Game, targets: Vec<TargetEntity>) {
    resolve_with(g, targets, |_| {});
}
fn cast(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    announce(g, id, targets.clone());
    resolve(g, targets);
}
fn target(o: ObjectId) -> Vec<TargetEntity> {
    vec![TargetEntity::Object { object: o }]
}
fn token_objects(g: &Game) -> Vec<ObjectId> {
    g.state
        .objects
        .values()
        .filter(|o| g.state.tokens.contains_key(&o.id.card))
        .map(|o| o.id)
        .collect()
}
fn observation(g: &Game, viewer: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(viewer), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}
fn lands(name: &str, n: usize) -> Vec<(&str, u8, bool)> {
    vec![(name, 0, false); n]
}

fn priority_for(g: &mut Game, player: u8) {
    for _ in 0..20 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && r.player() == PlayerId(player) {
            return;
        }
        answer(g, automatic(&r, &[]));
    }
    panic!("priority not reached");
}
fn counter(g: &mut Game, card: u32, target: StackId) {
    answer(g, Response::Cast { card: CardId(card) });
    pay_proposal(g, vec![], Some(target));
    check(g);
}
fn activated(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    answer(
        g,
        Response::ActivateAbility {
            object: object(g, id),
            ability: 0,
        },
    );
    pay_proposal(g, targets, None);
}
#[test]
fn profile_decks_policy_and_bounds_are_separate() {
    let c = catalog();
    let cfg = MatchConfig::for_profile(ATTACHMENTS_PROFILE, [9; 32], PlayerId(0)).unwrap();
    let pod = validate_pod(&c, &cfg).unwrap();
    assert_eq!(pod.config.cards.len(), 400);
    for (deck, basics) in cfg.decks.iter().zip([62, 40, 73, 77]) {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|r| c
                    .definitions
                    .iter()
                    .find(|d| d.oracle_id == r.oracle_id)
                    .unwrap()
                    .characteristics
                    .basic)
                .map(|r| r.count)
                .sum::<u32>(),
            basics
        );
    }
    assert_eq!(digest(&c).unwrap(), ATTACHMENTS_CATALOG_SHA256);
    let g = Game::new(prepared(&[("Grizzly Bears", 0, false)]));
    assert_eq!(g.stack_limit(), 4093);
    assert_eq!(g.damage_limit(), 760);
    assert_eq!(g.effect_limit(), 32);
    assert!(
        validate_pod(
            &c,
            &MatchConfig::for_profile(FOUR_COLORS_PROFILE, [9; 32], PlayerId(0)).unwrap()
        )
        .is_err()
    );
}
#[test]
fn equip_sorcery_timing_re_equip_and_equipment_survives_creature_bounce() {
    let mut rows = vec![
        ("Bonesplitter", 0, false),
        ("Grizzly Bears", 0, false),
        ("Greenwood Sentinel", 0, false),
        ("Unsummon", 0, true),
    ];
    rows.extend(lands("Island", 3));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::Upkeep);
        let r = request(g);
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: object(g, 0),
                    ability: 0
                }
            )
            .is_err()
        );
        reach(g, TurnStep::PrecombatMain);
        activated(g, 0, target(object(g, 1)));
        check(g);
        resolve(g, vec![]);
        assert_eq!(
            g.characteristics(object(g, 1)).unwrap().power,
            Some(4.into())
        );
        activated(g, 0, target(object(g, 2)));
        resolve(g, vec![]);
        assert_eq!(
            g.characteristics(object(g, 1)).unwrap().power,
            Some(2.into())
        );
        assert_eq!(
            g.characteristics(object(g, 2)).unwrap().power,
            Some(4.into())
        );
        let o = observation(g, 0);
        let equipment = o
            .view
            .objects
            .iter()
            .find(|o| o.oracle_id == g.definition(CardId(0)).unwrap().oracle_id)
            .unwrap();
        assert!(equipment.attached_to.is_some());
        cast(g, 3, target(object(g, 2)));
        assert_eq!(g.zone(object(g, 0)), Some(Zone::Battlefield));
        assert_eq!(g.state.objects[&CardId(0)].attached_to, None);
    });
}
#[test]
fn auras_attach_and_sba_on_bounce_tuck_destruction_and_mass_destruction() {
    for (spell, land, cost) in [
        ("Unsummon", "Island", 1),
        ("Time Ebb", "Island", 3),
        ("Murder", "Swamp", 3),
        ("Day of Judgment", "Plains", 4),
    ] {
        let mut rows = vec![
            ("Holy Strength", 0, true),
            ("Grizzly Bears", 0, false),
            (spell, 0, true),
            ("Short Sword", 0, false),
        ];
        rows.extend(lands("Plains", 1));
        rows.extend(lands(land, cost + 1));
        traces(prepared(&rows), |g| {
            cast(g, 0, target(object(g, 1)));
            assert_eq!(
                g.characteristics(object(g, 1)).unwrap().toughness,
                Some(4.into())
            );
            activated(g, 3, target(object(g, 1)));
            resolve(g, vec![]);
            cast(
                g,
                2,
                if spell == "Day of Judgment" {
                    vec![]
                } else {
                    target(object(g, 1))
                },
            );
            assert_eq!(
                g.zone(object(g, 0)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            assert_eq!(g.zone(object(g, 3)), Some(Zone::Battlefield));
            assert_eq!(g.state.objects[&CardId(3)].attached_to, None);
        });
    }
}
#[test]
fn hexproof_and_shroud_candidates_and_resolution_use_invocation_controller() {
    for (equipment, own_legal) in [("Swiftfoot Boots", true), ("Lightning Greaves", false)] {
        let mut rows = vec![
            (equipment, 0, false),
            ("Grizzly Bears", 0, false),
            ("Giant Growth", 0, true),
            ("Murder", 1, true),
            ("Bonesplitter", 0, false),
            ("Greenwood Sentinel", 1, false),
        ];
        rows.extend(lands("Forest", 3));
        rows.extend(vec![("Swamp", 1, false); 3]);
        traces(prepared(&rows), |g| {
            reach(g, TurnStep::PrecombatMain);
            activated(g, 0, target(object(g, 1)));
            resolve(g, vec![]);
            assert_eq!(g.targetable_by(object(g, 1), PlayerId(0)), own_legal);
            assert!(!g.targetable_by(object(g, 1), PlayerId(1)));
            if own_legal {
                cast(g, 2, target(object(g, 1)));
                assert_eq!(
                    g.characteristics(object(g, 1)).unwrap().power,
                    Some(5.into())
                );
            } else {
                let r = request(g);
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::ActivateAbility {
                            object: object(g, 4),
                            ability: 0
                        }
                    )
                    .is_err()
                );
            }
            priority_for(g, 1);
            answer(g, Response::Cast { card: CardId(3) });
            let r = request(g);
            let Request::Targets { candidates, .. } = &r else {
                panic!("Murder targets")
            };
            assert!(!candidates.contains(&object(g, 1)));
            assert!(candidates.contains(&object(g, 5)));
            pay_proposal(g, target(object(g, 5)), None);
            resolve(g, vec![]);
        });
    }
}
#[test]
fn aura_target_leaving_before_resolution_does_not_trigger_rancor() {
    let mut rows = vec![
        ("Rancor", 0, true),
        ("Grizzly Bears", 0, false),
        ("Unsummon", 1, true),
    ];
    rows.extend(lands("Forest", 1));
    rows.extend(vec![("Island", 1, false)]);
    traces(prepared(&rows), |g| {
        announce(g, 0, target(object(g, 1)));
        priority_for(g, 1);
        announce(g, 2, target(object(g, 1)));
        resolve(g, vec![]);
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert!(g.state.stack.is_empty());
    });
}
#[test]
fn rancor_returns_after_bounce_and_its_return_can_be_stifled() {
    for stifle in [false, true] {
        let mut rows = vec![
            ("Rancor", 0, true),
            ("Grizzly Bears", 0, false),
            ("Unsummon", 0, true),
            ("Stifle", 1, true),
        ];
        rows.extend(lands("Forest", 1));
        rows.extend(lands("Island", 1));
        rows.extend(vec![("Island", 1, false)]);
        traces(prepared(&rows), |g| {
            cast(g, 0, target(object(g, 1)));
            assert!(g.has_keyword(object(g, 1), KeywordAbility::Trample));
            announce(g, 2, target(object(g, 1)));
            if stifle {
                for _ in 0..100 {
                    let r = request(g);
                    if g.state
                        .stack
                        .last()
                        .is_some_and(|e| matches!(e.kind, StackKind::Triggered { .. }))
                    {
                        break;
                    }
                    answer(g, automatic(&r, &[]));
                }
                priority_for(g, 1);
                let trigger = g.top().unwrap().id;
                counter(g, 3, trigger);
            }
            resolve(g, vec![]);
            assert_eq!(
                g.zone(object(g, 0)),
                Some(if stifle {
                    Zone::Graveyard { owner: PlayerId(0) }
                } else {
                    Zone::Hand { owner: PlayerId(0) }
                })
            );
        });
    }
}
#[test]
fn cloak_unblockable_pacifism_and_spidersilk_late_entry_are_continuous() {
    let mut rows = vec![
        ("Whispersilk Cloak", 0, false),
        ("Grizzly Bears", 0, false),
        ("Yoked Ox", 1, false),
        ("Pacifism", 0, true),
        ("Spidersilk Armor", 0, true),
        ("Llanowar Elves", 0, true),
    ];
    rows.extend(lands("Island", 2));
    rows.extend(lands("Plains", 2));
    rows.extend(lands("Forest", 4));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 0, target(object(g, 1)));
        resolve(g, vec![]);
        assert!(!g.can_block(object(g, 2), object(g, 1)));
        cast(g, 3, target(object(g, 2)));
        assert!(!g.creatures(PlayerId(1), true).contains(&object(g, 2)));
        assert!(!g.creatures(PlayerId(1), false).contains(&object(g, 2)));
        cast(g, 4, vec![]);
        cast(g, 5, vec![]);
        assert!(g.has_keyword(object(g, 5), KeywordAbility::Reach));
        assert_eq!(
            g.characteristics(object(g, 5)).unwrap().toughness,
            Some(2.into())
        );
        assert_eq!(token_objects(g), vec![]);
        let view = observation(g, 0).view;
        assert!(
            view.objects
                .iter()
                .any(|o| o.restrictions.contains(&CreatureRestriction::CantBeBlocked))
        );
        assert!(view.objects.iter().any(|o| {
            o.restrictions
                .contains(&CreatureRestriction::CantAttackOrBlock)
        }));
    });
}
#[test]
fn granted_lifelink_lki_survives_the_damage_source_leaving() {
    let mut rows = vec![
        ("Loxodon Warhammer", 0, false),
        ("Prodigal Sorcerer", 0, false),
        ("Murder", 1, true),
        ("Furnace of Rath", 2, false),
    ];
    rows.extend(lands("Island", 3));
    rows.extend(vec![("Swamp", 1, false); 3]);
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 0, target(object(g, 1)));
        resolve(g, vec![]);
        activated(
            g,
            1,
            vec![TargetEntity::Player {
                player: PlayerId(2),
            }],
        );
        priority_for(g, 1);
        announce(g, 2, target(object(g, 1)));
        resolve(g, vec![]);
        assert_eq!(g.state.players[0].life, Quantity::from(42));
        assert_eq!(g.state.players[2].life, Quantity::from(38));
        assert_eq!(g.state.objects[&CardId(0)].attached_to, None);
    });
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
fn warhammer_lifelink_trample_furnace_and_first_strike() {
    let mut rows = vec![
        ("Youthful Knight", 0, false),
        ("Yoked Ox", 1, false),
        ("Loxodon Warhammer", 0, false),
        ("Furnace of Rath", 2, false),
    ];
    rows.extend(lands("Plains", 3));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 2, target(object(g, 0)));
        resolve(g, vec![]);
        declare(g, &[(0, 1)], &[(1, 0)]);
        let r = damage_request(g);
        assert_eq!(g.turn().step, TurnStep::FirstStrikeDamage);
        assert!(
            matches!(r,Request::Damage {ref lethal_requirements,..} if lethal_requirements == &vec![(object(g,1),4.into())])
        );
        answer(
            g,
            Response::TrampleDamage {
                assignments: vec![(object(g, 1), 4.into())],
                player: PlayerId(1),
                amount: 1.into(),
            },
        );
        for _ in 0..100 {
            let r = request(g);
            if g.turn().step == TurnStep::EndCombat {
                break;
            }
            if matches!(r, Request::DamageEffect { .. }) {
                check(g);
            }
            answer(g, automatic(&r, &[]));
        }
        assert_eq!(g.state.players[0].life, Quantity::from(50));
        assert_eq!(g.state.players[1].life, Quantity::from(38));
        assert_eq!(
            g.zone(object(g, 1)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
        assert_eq!(g.state.objects[&CardId(0)].marked_damage, Quantity::zero());
    });
}
#[test]
fn losing_granted_reach_after_declaration_keeps_the_legal_block() {
    let mut p = prepared(&[
        ("Serra Angel", 0, false),
        ("Grizzly Bears", 2, false),
        ("Spidersilk Armor", 2, false),
    ]);
    p.config.cards[2].owner = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        assert!(g.can_block(object(g, 1), object(g, 0)));
        declare(g, &[(0, 2)], &[(1, 0)]);
        request(g);
        g.concede(PlayerId(1)).unwrap();
        request(g);
        assert!(!g.has_keyword(object(g, 1), KeywordAbility::Reach));
        assert!(!g.can_block(object(g, 1), object(g, 0)));
        assert_eq!(g.state.combat[0].blockers, vec![object(g, 1)]);
        reach(g, TurnStep::EndCombat);
        assert_eq!(g.state.players[2].life, Quantity::from(40));
    });
}
#[test]
fn every_admitted_keyword_grant_is_continuous_and_captured_in_lki() {
    for keyword in [
        KeywordAbility::Defender,
        KeywordAbility::Flying,
        KeywordAbility::Reach,
        KeywordAbility::Vigilance,
        KeywordAbility::Haste,
        KeywordAbility::Lifelink,
        KeywordAbility::Deathtouch,
        KeywordAbility::Trample,
        KeywordAbility::FirstStrike,
        KeywordAbility::DoubleStrike,
        KeywordAbility::Menace,
        KeywordAbility::Hexproof,
        KeywordAbility::Shroud,
    ] {
        let mut rows = vec![
            ("Loxodon Warhammer", 0, false),
            ("Grizzly Bears", 0, false),
            ("Unsummon", 1, true),
        ];
        rows.extend(lands("Plains", 3));
        rows.extend(vec![("Island", 1, false)]);
        let mut p = prepared(&rows);
        let d = Arc::make_mut(&mut p.catalog)
            .definitions
            .iter_mut()
            .find(|d| d.name == "Loxodon Warhammer")
            .unwrap();
        let StaticEffect::ModifyCreatures { keywords, .. } = &mut d.static_abilities[0].effect
        else {
            panic!("static grant")
        };
        *keywords = vec![keyword];
        d.capabilities = d.requirements();
        p.catalog.validate().unwrap();
        p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
        traces(p, |g| {
            reach(g, TurnStep::PrecombatMain);
            activated(g, 0, target(object(g, 1)));
            resolve(g, vec![]);
            assert!(g.has_keyword(object(g, 1), keyword));
            assert!(
                g.object_before(object(g, 1))
                    .unwrap()
                    .keywords
                    .contains(&keyword)
            );
            // A completed equip to the already-equipped creature has no attach
            // occurrence; shroud instead prohibits choosing that target at all.
            if keyword != KeywordAbility::Shroud {
                assert!(g.targetable_by(object(g, 1), PlayerId(0)));
            }
        });
    }
}
#[test]
fn simultaneous_departure_preserves_granted_damage_source_lki() {
    let mut p = prepared(&[
        ("Loxodon Warhammer", 0, false),
        ("Prodigal Sorcerer", 0, false),
        ("Grizzly Bears", 1, false),
        ("Plains", 0, false),
        ("Plains", 0, false),
        ("Plains", 0, false),
    ]);
    p.config.cards[0].owner = PlayerId(2);
    p.config.cards[1].owner = PlayerId(2);
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 0, target(object(g, 1)));
        resolve(g, vec![]);
        activated(
            g,
            1,
            vec![TargetEntity::Player {
                player: PlayerId(1),
            }],
        );
        request(g);
        g.concede(PlayerId(2)).unwrap();
        resolve(g, vec![]);
        assert_eq!(g.state.players[0].life, Quantity::from(41));
        assert_eq!(g.state.players[1].life, Quantity::from(39));
    });
}
// PRIVATE INJECTED DOMAIN, deliberately outside natural match reachability:
// sorcery-only Pacifism cannot enter after declaration through this pool's
// public actions. CR 506.4a still constrains the kernel's combat semantics.
#[test]
fn private_pacifism_after_attack_or_block_preserves_combat_and_damage() {
    for on_attacker in [true, false] {
        let p = prepared(&[
            ("Grizzly Bears", 0, false),
            ("Greenwood Sentinel", 1, false),
            ("Pacifism", 2, true),
        ]);
        let mut baseline = None;
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut g = Game::with_trace(p.clone(), mode);
            declare(&mut g, &[(0, 1)], &[(1, 0)]);
            request(&mut g);
            let aura = object(&g, 2);
            g.move_object_for(aura, Zone::Battlefield, BatchCause::Setup)
                .unwrap();
            let aura = object(&g, 2);
            let enchanted = object(&g, if on_attacker { 0 } else { 1 });
            g.commit(
                ProposedEvent::Attach {
                    object: aura,
                    target: Some(enchanted),
                },
                BatchCause::Setup,
            )
            .unwrap();
            assert!(g.has_restriction(enchanted, CreatureRestriction::CantAttackOrBlock));
            assert_eq!(g.state.combat[0].attacker, object(&g, 0));
            assert_eq!(g.state.combat[0].blockers, vec![object(&g, 1)]);
            let snapshot = g.snapshot().unwrap();
            let prefix = g.journal().len();
            // This test-only fixture is outside accepted-input reachability.
            // Preserve the public proof and round-trip through the same strict
            // identity/hash/invariant decoder before replaying its continuation.
            assert!(
                matches!(Game::restore(p.clone(), &snapshot),Err(e) if e.contains("not reachable"))
            );
            let (mut replay, _) = Game::read_snapshot(p.clone(), &snapshot).unwrap();
            assert_eq!(replay.state, g.state);
            reach(&mut g, TurnStep::EndCombat);
            for input in &g.journal()[prefix..] {
                assert!(matches!(
                    replay.advance(Budget::unlimited()),
                    Advance::AwaitingDecision(_)
                ));
                replay
                    .respond(input.player, input.decision_id, input.response.clone())
                    .unwrap();
            }
            request(&mut replay);
            assert_eq!(replay.state, g.state);
            assert_eq!(replay.trace(), g.trace());
            assert_eq!(
                g.zone(object(&g, 0)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            assert_eq!(
                g.zone(object(&g, 1)),
                Some(Zone::Graveyard { owner: PlayerId(1) })
            );
            assert_eq!(g.state.players[1].life, Quantity::from(40));
            let d = g.state_digest().unwrap();
            if let Some(old) = &baseline {
                assert_eq!(&d, old);
            } else {
                baseline = Some(d);
            }
        }
    }
}
#[test]
fn pacifism_owner_concession_restores_next_declaration_eligibility() {
    let mut rows = vec![("Grizzly Bears", 0, false), ("Pacifism", 1, true)];
    rows.extend(vec![("Plains", 1, false); 2]);
    traces(prepared(&rows), |g| {
        for _ in 0..300 {
            let r = request(g);
            if g.state.active == Some(PlayerId(1))
                && g.turn().step == TurnStep::PrecombatMain
                && matches!(r, Request::Priority { .. })
            {
                break;
            }
            let response = match &r {
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                _ => automatic(&r, &[]),
            };
            answer(g, response);
        }
        cast(g, 1, target(object(g, 0)));
        assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 0)));
        assert!(!g.creatures(PlayerId(0), false).contains(&object(g, 0)));
        g.concede(PlayerId(1)).unwrap();
        request(g);
        for _ in 0..500 {
            let r = request(g);
            if let Request::Attackers {
                player: PlayerId(0),
                candidates,
                ..
            } = &r
            {
                assert!(candidates.contains(&object(g, 0)));
                break;
            }
            let response = match &r {
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                _ => automatic(&r, &[]),
            };
            answer(g, response);
        }
        assert!(matches!(
            request(g),
            Request::Attackers {
                player: PlayerId(0),
                ..
            }
        ));
        assert!(g.creatures(PlayerId(0), false).contains(&object(g, 0)));
        assert!(g.object_id(CardId(1)).is_none());
    });
}
// Reviewed P6.7 regression: an opponent's Aura modifies a damage source and its
// owner concedes while the prepared damage awaits a replacement-order choice
// (Furnace of Rath plus a Samite Healer shield). CR 104.3a/800.4a remove the
// Aura, but the frozen damage facts stay authoritative (510.2, 608.2h).
fn advance_after_concession(g: &mut Game) {
    for _ in 0..200 {
        match g.advance(Budget::unlimited()) {
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.stack.is_empty()
                    && g.state.pending_triggers.is_empty()
                    && !matches!(g.state.frame, Frame::DamageEffects { .. })
                {
                    return;
                }
                let resp = automatic(&r, &[]);
                g.respond(r.player(), r.decision_id(), resp).unwrap();
            }
            other => panic!("after concession: {other:?}"),
        }
    }
}
#[test]
fn aura_owner_concession_during_combat_damage_effect_choice_keeps_frozen_facts() {
    let rows = vec![
        ("Unholy Strength", 0, true),
        ("Swamp", 0, false),
        ("Grizzly Bears", 1, false),
        ("Furnace of Rath", 3, false),
        ("Samite Healer", 2, false),
    ];
    let mut g = Game::with_trace(prepared(&rows), TraceMode::Detailed);
    let g = &mut g;
    cast(g, 0, target(object(g, 2)));
    let mut shielded = false;
    loop {
        let r = request(g);
        match &r {
            Request::Attackers {
                player: PlayerId(1),
                ..
            } => answer(
                g,
                Response::Attackers {
                    attacks: vec![(object(g, 2), PlayerId(2))],
                },
            ),
            Request::Attackers { .. } => answer(g, Response::Attackers { attacks: vec![] }),
            Request::Blockers { .. } => answer(g, Response::Blockers { blocks: vec![] }),
            Request::DamageEffect { .. } => break,
            Request::Priority {
                player: PlayerId(2),
                ..
            } if !shielded
                && g.turn().number == 2
                && g.turn().step == TurnStep::DeclareAttackers =>
            {
                shielded = true;
                activated(
                    g,
                    4,
                    vec![TargetEntity::Player {
                        player: PlayerId(2),
                    }],
                );
            }
            _ => answer(g, automatic(&r, &[])),
        }
        assert!(g.turn().number <= 2, "damage effect choice not reached");
    }
    check(g);
    g.concede(PlayerId(0)).unwrap();
    advance_after_concession(g);
    check(g);
}
#[test]
fn aura_owner_concession_during_noncombat_damage_effect_choice_keeps_frozen_facts() {
    let rows = vec![
        ("Holy Strength", 0, true),
        ("Plains", 0, false),
        ("Prodigal Sorcerer", 1, false),
        ("Furnace of Rath", 3, false),
        ("Samite Healer", 2, false),
    ];
    let mut g = Game::with_trace(prepared(&rows), TraceMode::Detailed);
    let g = &mut g;
    cast(g, 0, target(object(g, 2)));
    priority_for(g, 2);
    activated(
        g,
        4,
        vec![TargetEntity::Player {
            player: PlayerId(2),
        }],
    );
    resolve(g, vec![]);
    priority_for(g, 1);
    activated(
        g,
        2,
        vec![TargetEntity::Player {
            player: PlayerId(2),
        }],
    );
    loop {
        let r = request(g);
        if matches!(r, Request::DamageEffect { .. }) {
            break;
        }
        answer(g, automatic(&r, &[]));
        assert!(
            !g.state.stack.is_empty(),
            "damage effect choice not reached"
        );
    }
    check(g);
    g.concede(PlayerId(0)).unwrap();
    advance_after_concession(g);
    check(g);
}
