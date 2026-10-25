//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-tokens-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let mut c = catalog();
    if rows.iter().any(|(name, _, _)| *name == "Unsummon") {
        let old =
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap();
        c.definitions.push(
            old.definitions
                .into_iter()
                .find(|d| d.name == "Unsummon")
                .unwrap(),
        );
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
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
        initial: ScenarioInitial {
            turn_step: TurnStep::Untap,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: TOKENS_PROFILE.into(),
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

#[test]
fn simultaneous_spell_tokens_have_physical_identity_visibility_and_sickness() {
    // CR111.1–4/603.6a: one instruction, two distinct simultaneous creatures.
    let mut rows = vec![("Raise the Alarm", 0, true), ("Essence Warden", 0, false)];
    rows.extend(lands("Plains", 2));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        let objects = token_objects(g);
        assert_eq!(objects.len(), 2);
        assert_ne!(objects[0].card, objects[1].card);
        assert_eq!(g.state.players[0].life, 42.into());
        for p in 0..4 {
            let view = observation(g, p);
            let tokens = view
                .view
                .objects
                .iter()
                .filter(|o| o.token.is_some())
                .collect::<Vec<_>>();
            assert_eq!(tokens.len(), 2);
            for t in tokens {
                assert_eq!(t.token.as_ref().unwrap().name, "Soldier Token");
                assert!(t.token.as_ref().unwrap().keywords.is_empty());
                assert_eq!(
                    (t.owner, t.controller, t.zone),
                    (PlayerId(0), PlayerId(0), Zone::Battlefield)
                );
                let c = t.characteristics.as_ref().unwrap();
                assert_eq!(
                    (c.power.clone(), c.toughness.clone()),
                    (Some(1.into()), Some(1.into()))
                );
                assert_eq!(c.colors, vec![Color::W]);
                assert_eq!(c.subtypes, vec!["Soldier"]);
                assert!(!t.attack_ready);
                assert!(!t.tapped);
            }
        }
        if g.trace.mode == TraceMode::Detailed {
            let creations = g
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::Committed {
                        actual: EventOutcome::TokensCreated { objects, .. },
                        ..
                    } => Some(objects),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(creations, vec![&objects]);
        }
        // CR302.6: they cannot attack on their creation turn.
        let candidates = attackers(g);
        assert!(objects.iter().all(|o| !candidates.contains(o)));
        let r = request(g);
        assert!(
            g.check_response(
                r.player(),
                r.decision_id(),
                Response::Attackers {
                    attacks: vec![(objects[0], PlayerId(1))]
                }
            )
            .is_err()
        );
    });
}

#[test]
fn source_backed_token_spells_and_enters_triggers_define_exact_characteristics() {
    // Hand-reviewed CR111.3/111.4 expectations, independent of lowering.
    for (name, land, cost, count, subtype, color, pt, keywords) in [
        (
            "Midnight Haunting",
            "Plains",
            3,
            2,
            "Spirit",
            Color::W,
            1,
            vec![KeywordAbility::Flying],
        ),
        (
            "Queen's Commission",
            "Plains",
            3,
            2,
            "Vampire",
            Color::W,
            1,
            vec![KeywordAbility::Lifelink],
        ),
        (
            "Attended Knight",
            "Plains",
            3,
            1,
            "Soldier",
            Color::W,
            1,
            vec![],
        ),
        (
            "Gallant Cavalry",
            "Plains",
            4,
            1,
            "Knight",
            Color::W,
            2,
            vec![KeywordAbility::Vigilance],
        ),
        (
            "Seller of Songbirds",
            "Plains",
            3,
            1,
            "Bird",
            Color::W,
            1,
            vec![KeywordAbility::Flying],
        ),
        (
            "Hornet Queen",
            "Forest",
            7,
            4,
            "Insect",
            Color::G,
            1,
            vec![KeywordAbility::Flying, KeywordAbility::Deathtouch],
        ),
        (
            "Spore Swarm",
            "Forest",
            4,
            3,
            "Saproling",
            Color::G,
            1,
            vec![],
        ),
        ("Ferocious Pup", "Forest", 3, 1, "Wolf", Color::G, 2, vec![]),
        (
            "Crested Herdcaller",
            "Forest",
            5,
            1,
            "Dinosaur",
            Color::G,
            3,
            vec![KeywordAbility::Trample],
        ),
    ] {
        let mut rows = vec![(name, 0, true)];
        rows.extend(lands(land, cost));
        traces(prepared(&rows), |g| {
            cast(g, 0, vec![]);
            let objects = token_objects(g);
            assert_eq!(objects.len(), count, "{name}");
            for o in objects {
                let d = g.definition(o.card).unwrap();
                let c = g.characteristics(o).unwrap();
                assert_eq!(d.name, format!("{subtype} Token"));
                assert_eq!(d.keywords, keywords);
                assert_eq!(c.subtypes, vec![subtype]);
                assert_eq!(c.colors, vec![color]);
                assert_eq!((c.power, c.toughness), (Some(pt.into()), Some(pt.into())));
                assert_eq!(g.state.cards[&o.card].owner, PlayerId(0));
                assert_eq!(g.current(o).unwrap().controller, PlayerId(0));
                assert!(!g.current(o).unwrap().attack_ready);
                assert!(g.state.commanders.history(o.card).is_none());
            }
        });
    }
}

#[test]
fn token_dies_triggers_artist_before_cessation_and_bounce_never_returns_a_playable_card() {
    for (spell, land, dies) in [("Disfigure", "Swamp", true), ("Unsummon", "Island", false)] {
        let mut rows = vec![
            ("Raise the Alarm", 0, true),
            ("Blood Artist", 0, false),
            (spell, 0, true),
            (land, 0, false),
        ];
        rows.extend(lands("Plains", 2));
        traces(prepared(&rows), |g| {
            cast(g, 0, vec![]);
            let old = token_objects(g)[0];
            let old_handle = g.handle_for(PlayerId(0), old).unwrap();
            announce(g, 2, target(old));
            let mut destination_seen = false;
            resolve_with(g, target(old), |g| {
                if let Some(id) = g.object_id(old.card)
                    && id.incarnation == 1
                    && !destination_seen
                {
                    destination_seen = true;
                    assert_eq!(
                        g.zone(id),
                        Some(if dies {
                            Zone::Graveyard { owner: PlayerId(0) }
                        } else {
                            Zone::Hand { owner: PlayerId(0) }
                        })
                    );
                    assert_eq!(g.state.pending_triggers.len(), usize::from(dies));
                    // CR111.8: a token outside the battlefield cannot move again.
                    let mut branch = g.fork();
                    assert!(branch.move_object(id, Zone::Battlefield).is_err());
                    check(g);
                }
            });
            assert!(destination_seen);
            assert!(g.object_id(old.card).is_none());
            assert!(g.zone(old).is_none());
            assert!(g.state.tokens.contains_key(&old.card));
            let r = request(g);
            assert!(
                g.check_response(
                    r.player(),
                    r.decision_id(),
                    Response::Cast { card: old.card }
                )
                .is_err()
            );
            assert_eq!(
                g.state.players[0].life,
                Quantity::from(if dies { 41 } else { 40 })
            );
            assert_eq!(
                g.state.players[1].life,
                Quantity::from(if dies { 39 } else { 40 })
            );
            assert!(
                observation(g, 0)
                    .view
                    .objects
                    .iter()
                    .all(|o| o.handle != old_handle)
            );
        });
    }
}

#[test]
fn counter_combines_in_layer7c_and_is_lost_on_zone_change() {
    // CR122.1a/613.4c: Bears 2/2 +counter1 +Growth3 -Disfigure2 =4/4.
    let mut rows = vec![
        ("Grizzly Bears", 0, false),
        ("Battlegrowth", 0, true),
        ("Giant Growth", 0, true),
        ("Disfigure", 0, true),
        ("Unsummon", 0, true),
        ("Swamp", 0, false),
        ("Island", 0, false),
    ];
    rows.extend(lands("Forest", 2));
    traces(prepared(&rows), |g| {
        let bears = object(g, 0);
        cast(g, 1, target(bears));
        cast(g, 2, target(bears));
        cast(g, 3, target(bears));
        let c = g.characteristics(bears).unwrap();
        assert_eq!((c.power, c.toughness), (Some(4.into()), Some(4.into())));
        assert_eq!(g.current(bears).unwrap().plus_one_counters, 1.into());
        let view = observation(g, 0);
        assert_eq!(
            view.view
                .objects
                .iter()
                .find(|o| o.handle == g.handle_for(PlayerId(0), bears).unwrap())
                .unwrap()
                .plus_one_counters,
            1.into()
        );
        cast(g, 4, target(bears));
        let returned = object(g, 0);
        assert_ne!(bears, returned);
        assert_eq!(
            g.current(returned).unwrap().plus_one_counters,
            Quantity::zero()
        );
        assert_eq!(
            g.characteristics(returned).unwrap().toughness,
            Some(2.into())
        );
    });
}

#[test]
fn counter_modified_toughness_survives_then_lethal_damage_destroys() {
    // CR704.5g: a +1/+1 counter makes a 2/2 survive two marked damage;
    // Disfigure then leaves toughness1, so the same two marks become lethal.
    let mut rows = vec![
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
        ("Battlegrowth", 0, true),
        ("Disfigure", 0, true),
        ("Forest", 0, false),
        ("Swamp", 0, false),
    ];
    rows.extend(lands("Forest", 1));
    traces(prepared(&rows), |g| {
        let bears = object(g, 0);
        cast(g, 2, target(bears));
        attackers(g);
        answer(
            g,
            Response::Attackers {
                attacks: vec![(bears, PlayerId(1))],
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
                blocks: vec![(object(g, 1), bears)],
            },
        );
        reach(g, TurnStep::EndCombat);
        assert_eq!(g.current(bears).unwrap().marked_damage, 2.into());
        assert_eq!(g.characteristics(bears).unwrap().toughness, Some(3.into()));
        cast(g, 3, target(bears));
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert_eq!(
            g.current(object(g, 0)).unwrap().plus_one_counters,
            Quantity::zero()
        );
    });
}

#[test]
fn enters_counter_targets_and_source_bound_counter() {
    for name in ["Ironpaw Aspirant", "Satyr Grovedancer"] {
        let land = if name == "Ironpaw Aspirant" {
            "Plains"
        } else {
            "Forest"
        };
        let mut rows = vec![(name, 0, true), ("Grizzly Bears", 0, false)];
        rows.extend(lands(land, 2));
        traces(prepared(&rows), |g| {
            let bears = object(g, 1);
            cast(g, 0, target(bears));
            assert_eq!(g.current(bears).unwrap().plus_one_counters, 1.into());
            assert_eq!(g.characteristics(bears).unwrap().toughness, Some(3.into()));
        });
    }
    let mut p = prepared(&[
        ("Satyr Grovedancer", 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Satyr Grovedancer")
        .unwrap();
    d.triggered[0].program.targets.clear();
    let Instruction::PutPlusOneCounters { on, .. } = &mut d.triggered[0].program.instructions[0]
    else {
        panic!()
    };
    *on = mtg_contracts::CounterRecipient::Source;
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        cast(g, 0, vec![]);
        let source = object(g, 0);
        assert_eq!(g.current(source).unwrap().plus_one_counters, 1.into());
        assert_eq!(g.characteristics(source).unwrap().power, Some(2.into()));
    });
}

#[test]
fn token_capacity_is_a_typed_atomic_replayable_fault() {
    let mut p = prepared(&[
        ("Raise the Alarm", 0, true),
        ("Plains", 0, false),
        ("Plains", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Raise the Alarm")
        .unwrap();
    let Instruction::CreateCreatureTokens { count, .. } =
        &mut d.spell.as_mut().unwrap().instructions[0]
    else {
        panic!()
    };
    *count = TOKEN_CAPACITY + 1;
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        announce(g, 0, vec![]);
        for _ in 0..4 {
            answer(g, Response::Pass);
        }
        assert!(
            matches!(g.advance(Budget::unlimited()),Advance::Fault(ref s) if s=="token capacity exceeded")
        );
        assert_eq!(
            g.typed_fault(),
            Some(&RuleFault::TokenCapacityExceeded {
                capacity: TOKEN_CAPACITY,
                created: 0,
                requested: TOKEN_CAPACITY + 1
            })
        );
        assert!(g.state.tokens.is_empty());
    });
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

fn next_own_main(g: &mut Game) {
    let turn = g.turn().number;
    for _ in 0..1000 {
        let r = request(g);
        if g.turn().number > turn
            && g.state.active == Some(PlayerId(0))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { .. })
        {
            return;
        }
        let response = match &r {
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates[..*count].to_vec(),
            },
            _ => automatic(&r, &[]),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("next own turn")
}
fn attack_token(g: &mut Game, token: ObjectId) {
    let candidates = attackers(g);
    assert!(candidates.contains(&token));
    answer(
        g,
        Response::Attackers {
            attacks: vec![(token, PlayerId(1))],
        },
    );
    loop {
        if matches!(request(g), Request::Blockers { .. }) {
            break;
        }
        answer(g, Response::Pass);
    }
}
#[test]
fn token_flying_deathtouch_and_trample_use_existing_combat_rules() {
    // CR702.9b/702.17b: Insect can be blocked by reach, not by ground.
    // CR702.2b/704.5h: one dealt by that 1/1 kills a 2/4 Giant Spider.
    let mut rows = vec![
        ("Hornet Queen", 0, true),
        ("Grizzly Bears", 1, false),
        ("Giant Spider", 1, false),
    ];
    rows.extend(lands("Forest", 7));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        let token = token_objects(g)[0];
        next_own_main(g);
        attack_token(g, token);
        // Policies need the same public legality hints as the inherited profiles.
        let view = observation(g, 1);
        let Some(PlayerRequest::Blockers {
            legal_attackers, ..
        }) = view.decision
        else {
            panic!("public blockers")
        };
        let ground = g.handle_for(PlayerId(1), object(g, 1)).unwrap();
        let reach_handle = g.handle_for(PlayerId(1), object(g, 2)).unwrap();
        let attacker = g.handle_for(PlayerId(1), token).unwrap();
        assert!(
            legal_attackers
                .iter()
                .find(|h| h.blocker == ground)
                .unwrap()
                .attackers
                .is_empty()
        );
        assert!(
            legal_attackers
                .iter()
                .find(|h| h.blocker == reach_handle)
                .unwrap()
                .attackers
                .contains(&attacker)
        );
        let r = request(g);
        assert!(
            g.check_response(
                r.player(),
                r.decision_id(),
                Response::Blockers {
                    blocks: vec![(object(g, 1), token)]
                }
            )
            .is_err()
        );
        answer(
            g,
            Response::Blockers {
                blocks: vec![(object(g, 2), token)],
            },
        );
        reach(g, TurnStep::EndCombat);
        assert_eq!(
            g.zone(object(g, 2)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
        assert!(g.object_id(token.card).is_none());
        assert_eq!(g.state.players[1].life, 40.into());
    });
    // CR702.19b: Dinosaur3 assigns Bears2 lethal and player1 excess.
    let mut rows = vec![("Crested Herdcaller", 0, true), ("Grizzly Bears", 1, false)];
    rows.extend(lands("Forest", 5));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        let token = token_objects(g)[0];
        next_own_main(g);
        attack_token(g, token);
        let blocker = object(g, 1);
        answer(
            g,
            Response::Blockers {
                blocks: vec![(blocker, token)],
            },
        );
        let r = damage_request(g);
        assert!(matches!(
            r,
            Request::Damage {
                player_recipient: Some(PlayerId(1)),
                ..
            }
        ));
        answer(
            g,
            Response::TrampleDamage {
                assignments: vec![(blocker, 2.into())],
                player: PlayerId(1),
                amount: 1.into(),
            },
        );
        reach(g, TurnStep::EndCombat);
        assert_eq!(g.state.players[1].life, 39.into());
        assert_eq!(g.current(token).unwrap().marked_damage, 2.into());
    });
}
#[test]
fn token_lifelink_vigilance_and_no_commander_damage() {
    for (name, cost, lifelink) in [
        ("Queen's Commission", 3, true),
        ("Gallant Cavalry", 4, false),
    ] {
        let mut rows = vec![(name, 0, true)];
        rows.extend(lands("Plains", cost));
        traces(prepared(&rows), |g| {
            cast(g, 0, vec![]);
            let token = token_objects(g)[0];
            next_own_main(g);
            attack_token(g, token);
            assert_eq!(g.current(token).unwrap().tapped, lifelink);
            answer(g, Response::Blockers { blocks: vec![] });
            reach(g, TurnStep::EndCombat);
            assert_eq!(
                g.state.players[0].life,
                Quantity::from(if lifelink { 41 } else { 40 })
            );
            assert_eq!(
                g.state.players[1].life,
                Quantity::from(if lifelink { 39 } else { 38 })
            );
            assert!(g.state.commanders.history(token.card).is_none());
            assert!(g.state.commanders.combat_loss_candidates().is_empty());
        });
    }
}
#[test]
fn maximal_pool_capacity_derivation_and_earlier_limits() {
    let c = catalog();
    let mut most_creatures = 0;
    let mut most_tokens = 0;
    for colors in [vec![Color::W], vec![Color::B, Color::G]] {
        let admitted = c
            .definitions
            .iter()
            .filter(|d| {
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|k| colors.contains(k))
            })
            .collect::<Vec<_>>();
        most_creatures = most_creatures.max(
            admitted
                .iter()
                .filter(|d| d.characteristics.kind == CardKind::Creature)
                .count(),
        );
        let tokens = admitted
            .iter()
            .flat_map(|d| d.spell.iter().chain(d.triggered.iter().map(|t| &t.program)))
            .flat_map(|p| p.instructions.iter())
            .map(|i| match i {
                Instruction::CreateCreatureTokens { count, .. } => *count as usize,
                _ => 0,
            })
            .sum::<usize>();
        most_tokens = most_tokens.max(tokens);
        assert!(admitted.iter().filter(|d| d.name == "Blood Artist").count() <= 1);
        assert!(
            admitted
                .iter()
                .filter(|d| d.name == "Essence Warden")
                .count()
                <= 1
        );
        assert!(
            admitted
                .iter()
                .filter(|d| !d.activated.is_empty())
                .all(|d| d.name == "Samite Healer")
        );
    }
    assert_eq!(most_creatures, 27);
    assert_eq!(most_tokens, 9);
    assert_eq!(tokens::CREATURE_CARD_BOUND, 4 * most_creatures);
    assert!(4 * most_tokens < 64usize);
    let creatures = 4 * most_creatures + 64usize;
    assert_eq!(creatures, 172);
    assert_eq!(tokens::DAMAGE_BOUND, 2 * creatures);
    let maximal_stack = 400 + 4 + creatures * 4 + creatures * 4 + 4 * most_creatures + 4;
    assert_eq!(maximal_stack, 1892);
    assert_eq!(tokens::STACK_BOUND, maximal_stack);
    assert_eq!(tokens::EVENT_GROUP_BOUND, 400 + 64usize + 4);
    let g = Game::new(prepared(&[]));
    assert_eq!(g.damage_limit(), capacities::DAMAGE_LIMIT);
    assert_eq!(g.stack_limit(), capacities::STACK_LIMIT);
    let config = MatchConfig::tokens([1; 32], PlayerId(0));
    let pod = Prepared::admit_pod(c.clone(), config.clone()).unwrap();
    assert_eq!(pod.config.cards.len(), 400);
    let white = &config.decks[0];
    let bg = &config.decks[1];
    assert_eq!(
        white
            .cards
            .iter()
            .find(|r| r.oracle_id == PLAINS)
            .unwrap()
            .count,
        78
    );
    assert_eq!(
        bg.cards
            .iter()
            .find(|r| r.oracle_id == FOREST)
            .unwrap()
            .count,
        31
    );
    assert_eq!(
        bg.cards
            .iter()
            .find(|r| r.oracle_id == SWAMP)
            .unwrap()
            .count,
        35
    );
    assert!(
        config
            .decks
            .iter()
            .all(|d| d.cards.iter().map(|r| r.count).sum::<u32>() == 100)
    );
    let foundations = Game::new(
        Prepared::admit_pod(
            Catalog::read(include_str!(
                "../../../../tests/fixtures/expected-match-ir.json"
            ))
            .unwrap(),
            MatchConfig::foundations([0; 32], PlayerId(0)),
        )
        .unwrap(),
    );
    assert_eq!(foundations.damage_limit(), capacities::DAMAGE_LIMIT);
    assert_eq!(foundations.stack_limit(), capacities::STACK_LIMIT);
}

#[test]
fn tokens_profile_inherits_first_and_double_strike_with_between_step_restore() {
    // CR702.7b/702.4b/510.4: P6.3 inherits the complete P6.2 step machine.
    // Attended Knight kills Bears before return damage. Unblocked Fencing Ace
    // deals one in each step, with priority and a restore checkpoint between.
    for (name, blocked) in [("Attended Knight", true), ("Fencing Ace", false)] {
        traces(
            prepared(&[(name, 0, false), ("Grizzly Bears", 1, false)]),
            |g| {
                let source = object(g, 0);
                attack_token(g, source);
                let blocker = object(g, 1);
                answer(
                    g,
                    Response::Blockers {
                        blocks: if blocked {
                            vec![(blocker, source)]
                        } else {
                            vec![]
                        },
                    },
                );
                reach(g, TurnStep::FirstStrikeDamage);
                assert_eq!(g.state.first_strike_participants, BTreeSet::from([source]));
                assert_eq!(g.current(source).unwrap().marked_damage, Quantity::zero());
                if blocked {
                    assert_eq!(
                        g.zone(object(g, 1)),
                        Some(Zone::Graveyard { owner: PlayerId(1) })
                    );
                    assert_eq!(g.state.players[1].life, 40.into());
                } else {
                    assert_eq!(g.state.players[1].life, 39.into());
                }
                check(g);
                reach(g, TurnStep::CombatDamage);
                assert_eq!(
                    g.state.players[1].life,
                    Quantity::from(if blocked { 40 } else { 38 })
                );
            },
        );
    }
}

#[test]
fn detached_enters_token_program_survives_source_destruction() {
    // CR113.7a/608.2m: destroying the Knight in response to its enters
    // ability does not remove that ability or its fixed token definition.
    let p = prepared(&[
        ("Attended Knight", 0, true),
        ("Disfigure", 1, true),
        ("Plains", 0, false),
        ("Plains", 0, false),
        ("Plains", 0, false),
        ("Swamp", 1, false),
    ]);
    traces(p, |g| {
        announce(g, 0, vec![]);
        let mut placed = false;
        for _ in 0..100 {
            let r = request(g);
            if matches!(r, Request::Priority { .. })
                && g.state
                    .stack
                    .iter()
                    .any(|s| matches!(s.kind, StackKind::Triggered { .. }))
            {
                assert_eq!(r.player(), PlayerId(0));
                placed = true;
                break;
            }
            let response = automatic(&r, &[]);
            g.respond(r.player(), r.decision_id(), response).unwrap();
        }
        assert!(placed);
        let source = object(g, 0);
        assert_eq!(g.zone(source), Some(Zone::Battlefield));
        answer(g, Response::Pass); // player1 may respond with Disfigure.
        cast(g, 1, target(source));
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        let tokens = token_objects(g);
        assert_eq!(tokens.len(), 1);
        let token = tokens[0];
        assert_eq!(g.state.tokens[&token.card].source, source);
        assert_eq!(g.definition(token.card).unwrap().name, "Soldier Token");
        assert_eq!(g.characteristics(token).unwrap().power, Some(1.into()));
        assert_eq!(g.state.cards[&token.card].owner, PlayerId(0));
        assert_eq!(g.current(token).unwrap().controller, PlayerId(0));
    });
}
