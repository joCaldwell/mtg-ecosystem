//! M1a private source witness, not a reviewed/admitted pod. The initial midgame
//! setup has legal 100-card singleton Commander decks and ordinary zones; all
//! progression after Game construction uses accepted inputs. Synthetic edge
//! states below are explicitly separate from that paid source witness.
//! CR 105.2, 113.7a, 201.5, 602.5a-b, 605.3, 608.2h, 733.1-2:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const RAMOS: &str = "3ed41d2d-211b-4013-8562-8c64d54cc43a";
const NAMES: [&str; 6] = [
    "Wall of Vines",
    "Wall of Wood",
    "Dragon Sniper",
    "Grizzly Bears",
    "Canopy Spider",
    "Colossal Dreadmaw",
];
fn source() -> Definition {
    let result = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    *definition
}
fn setup(nested: bool, unpayable: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    c.definitions.push(source());
    c.definitions.retain(|d| {
        NAMES.contains(&d.name.as_str())
            || [RAMOS, YARGLE, FOREST].contains(&d.oracle_id.as_str())
            || (nested && d.name == "Golgari Signet")
    });
    assert_eq!(c.definitions.len(), if nested { 10 } else { 9 });
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    if unpayable {
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == NAMES[5])
            .unwrap();
        d.characteristics.mana_cost.generic = 30.into(); // explicitly synthetic unpayable cost
    }
    c.validate().unwrap();
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let mut cards = vec![];
    let mut commanders = vec![];
    for seat in 0..4 {
        let player = PlayerId(seat);
        commanders.push(CardId(cards.len() as u32));
        cards.push(CardSetup {
            id: CardId(cards.len() as u32),
            oracle_id: if seat == 0 {
                RAMOS.into()
            } else {
                YARGLE.into()
            },
            owner: player,
            controller: player,
            zone: Zone::Command { owner: player },
        });
        if seat == 0 {
            for name in NAMES {
                cards.push(CardSetup {
                    id: CardId(cards.len() as u32),
                    oracle_id: id(name),
                    owner: player,
                    controller: player,
                    zone: Zone::Hand { owner: player },
                });
            }
            if nested {
                cards.push(CardSetup {
                    id: CardId(cards.len() as u32),
                    oracle_id: id("Golgari Signet"),
                    owner: player,
                    controller: player,
                    zone: Zone::Battlefield,
                });
            }
        }
        let n = if seat == 0 {
            93 - usize::from(nested)
        } else {
            99
        };
        for i in 0..n {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: player,
                controller: player,
                zone: if seat == 0 && i < 14 {
                    Zone::Battlefield
                } else {
                    Zone::Library { owner: player }
                },
            });
        }
    }
    assert_eq!(cards.len(), 400);
    for seat in 0..4 {
        let rows = cards
            .iter()
            .filter(|x| x.owner == PlayerId(seat))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 100);
        let commander_id = if seat == 0 { RAMOS } else { YARGLE };
        let commander = &c
            .definitions
            .iter()
            .find(|d| d.oracle_id == commander_id)
            .unwrap()
            .characteristics;
        assert!(commander.legendary && commander.kind == CardKind::Creature);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r.zone, Zone::Command { .. }))
                .count(),
            1
        );
        let mut names = BTreeSet::new();
        for row in rows {
            let d = c
                .definitions
                .iter()
                .find(|d| d.oracle_id == row.oracle_id)
                .unwrap();
            assert!(
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|color| commander.color_identity.contains(color))
            );
            assert!(d.characteristics.basic || names.insert(&d.name));
        }
    }
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::PrecombatMain,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "ramos-m1a-private-source-witness".into(),
        commanders,
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
fn named(g: &Game, name: &str) -> ObjectId {
    g.state
        .rules
        .objects
        .values()
        .find(|o| g.definition(o.id.card).unwrap().name == name)
        .unwrap()
        .id
}
fn paid_counters(g: &mut Game) -> ObjectId {
    let cmd = named(g, "Ramos, Dragon Engine");
    announce(g, cmd.card.0, vec![]);
    check(g);
    assert_eq!(
        g.state
            .rules
            .commanders
            .history(cmd.card)
            .unwrap()
            .command_casts[&PlayerId(0)],
        Quantity::from(1)
    );
    resolve(g, vec![]);
    let r = named(g, "Ramos, Dragon Engine");
    assert!(!g.current(r).unwrap().attack_ready);
    assert_eq!(g.current(r).unwrap().plus_one_counters, 0.into());
    for (i, name) in NAMES[..5].iter().enumerate() {
        let card = named(g, name).card;
        announce(g, card.0, vec![]);
        // Capture genuine paid casting/trigger placement with ordinary restore/replay.
        check(g);
        assert!(
            g.state
                .rules
                .stack
                .iter()
                .find(|s| s.source.card == card)
                .unwrap()
                .paid
                .is_some()
        );
        resolve(g, vec![]);
        assert_eq!(
            g.current(r).unwrap().plus_one_counters,
            Quantity::from((i + 1) as i64)
        );
    }
    r
}
fn spend(g: &mut Game) {
    let r = request(g);
    let Request::SpendMana { cost, .. } = &r else {
        panic!("{r:?}")
    };
    let remaining = g.payment_pool(r.player(), cost).unwrap();
    let map = g.state.rules.players[r.player().0 as usize]
        .mana
        .iter()
        .filter_map(|(c, n)| {
            let mut spent = n.clone();
            spent.subtract(&remaining.get(c).cloned().unwrap_or_default());
            (spent > Quantity::zero()).then_some((*c, spent))
        })
        .collect();
    g.respond(
        r.player(),
        r.decision_id(),
        Response::SpendMana { spending: map },
    )
    .unwrap();
}
fn privacy_query(g: &Game, source: ObjectId) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        let QueryResult::Incomplete { work, .. } = g.observe(PlayerId(seat), Budget::new(1)) else {
            panic!("query should suspend")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        let QueryResult::Complete(QueryValue::Observation(obs)) =
            g.resume_work(work, Budget::unlimited())
        else {
            panic!("query recover")
        };
        assert!(
            obs.view
                .objects
                .iter()
                .all(|o| !matches!(o.zone, Zone::Library { .. })
                    && (!matches!(o.zone,Zone::Hand {owner} if owner!=PlayerId(seat))))
        );
        let visible = obs
            .view
            .objects
            .iter()
            .find(|o| o.oracle_id == RAMOS && o.zone == Zone::Battlefield)
            .unwrap();
        assert_eq!(visible.mana_ability_used, Some(g.mana_ability_used(source)));
        let serialized = canonical(&obs).unwrap();
        assert!(
            !serialized.contains("mana_ability_uses")
                && !serialized.contains("\"ability_use\"")
                && !serialized.contains("receipts")
        );
        if seat != 0 {
            assert!(obs.decision.is_none());
        }
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}
#[test]
fn ramos_m1_paid_command_cast_five_counters_payment_snapshot_replay_privacy() {
    traces(setup(false, false), |g| {
        let r = paid_counters(g);
        eprintln!(
            "M1a build={BUILD_ID} catalog={} profile={} trace={:?}",
            g.prepared.catalog_digest, g.prepared.profile_digest, g.trace.mode
        );
        let spell = named(g, NAMES[5]).card;
        answer(g, Response::Cast { card: spell });
        assert!(matches!(request(g), Request::Mana { .. }));
        privacy_query(g, r);
        check(g);
        let own = observation(g, 0);
        let token = own.decision.unwrap().token().clone();
        let handle = g.handle_for(PlayerId(0), r).unwrap();
        let before = g.state.clone();
        let QueryResult::Incomplete { work, .. } = g.check_player_response(
            PlayerId(0),
            token.clone(),
            PlayerResponse::ActivateMana {
                object: handle.clone(),
            },
            Budget::new(1),
        ) else {
            panic!("validation budget")
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        let QueryResult::Complete(QueryValue::Validated(receipt)) =
            g.resume_work(work, Budget::unlimited())
        else {
            panic!("validation recovery")
        };
        assert_eq!(g.state, before);
        assert!(matches!(
            g.check_player_response(
                PlayerId(1),
                token,
                PlayerResponse::ActivateMana { object: handle },
                Budget::unlimited()
            ),
            QueryResult::Rejected(_)
        ));
        g.submit(receipt).unwrap();
        assert!(matches!(request(g), Request::Mana { .. })); // immediate, no stack/color request
        assert_eq!(g.current(r).unwrap().plus_one_counters, 0.into());
        for color in [Color::W, Color::U, Color::B, Color::R, Color::G] {
            assert_eq!(g.state.rules.players[0].mana[&color], 2.into());
        }
        assert!(g.mana_ability_used(r));
        privacy_query(g, r);
        check(g);
        let before = g.state.clone();
        let trace = g.trace.clone();
        let req = request(g);
        assert!(
            g.respond(
                PlayerId(0),
                req.decision_id(),
                Response::ActivateMana { object: r }
            )
            .is_err()
        );
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        for change in 0..4 {
            let mut forged = g.fork();
            let Frame::Mana { cast } = &mut forged.state.rules.frame else {
                panic!("cast mana frame")
            };
            let receipt = cast
                .mana_activations
                .iter_mut()
                .find(|x| x.object == r)
                .unwrap()
                .permanent
                .as_mut()
                .unwrap();
            match change {
                0 => receipt.counters = 4.into(),
                1 => receipt.ability_use.as_mut().unwrap().index = 1,
                2 => receipt.previously_used = true,
                _ => {
                    receipt.produced.insert(Color::W, 3.into());
                }
            }
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &forged.snapshot().unwrap()).is_err()
            );
        }
        let mut forged = g.fork();
        forged.state.rules.mana_ability_uses.clear();
        assert!(Game::restore(g.prepared.as_ref().clone(), &forged.snapshot().unwrap()).is_err());
        answer(g, Response::FinishMana { undo_mana: false });
        spend(g);
        let _ = request(g);
        check(g);
        assert!(
            g.state
                .rules
                .stack
                .iter()
                .find(|s| s.source.card == spell)
                .unwrap()
                .paid
                .is_some()
        );
        resolve(g, vec![]);
        assert_eq!(g.current(r).unwrap().plus_one_counters, 1.into());
        assert!(g.mana_ability_used(r));
        check(g);
        // Independently execute every accepted input with one rules unit at a
        // time, then compare with ordinary-budget announcement and replay.
        let mut one = Game::with_trace(g.prepared.as_ref().clone(), g.trace.mode);
        for input in g.replay_journal().decisions {
            let req = loop {
                match one.advance(Budget::new(1)) {
                    Advance::Suspended { .. } => (),
                    Advance::AwaitingDecision(r) => break r,
                    x => panic!("{x:?}"),
                }
            };
            assert_eq!(
                (req.player(), req.decision_id()),
                (input.player, input.decision_id)
            );
            one.respond(input.player, input.decision_id, input.response)
                .unwrap();
        }
        while one.rule_boundary() < g.rule_boundary() {
            let _ = one.advance(Budget::new(1));
        }
        assert_eq!(one.state, g.state);
        assert_eq!(one.trace, g.trace);
        let mut profile = match_profile("commander-mana-v1").unwrap().clone();
        profile.hosted = true;
        profile.search = true;
        for feature in [Feature::Hosting, Feature::Search] {
            let Err(ProfileError::UnreviewedCapabilities { capabilities, .. }) =
                profile.require_feature(&g.prepared.catalog, feature)
            else {
                panic!("consumer fence")
            };
            assert!(capabilities.contains(&Capability::CastSpellColorCountersV1));
            assert!(capabilities.contains(&Capability::CounterRemovalManaV1));
        }
        assert!(
            Prepared::admit(
                g.prepared.catalog.as_ref().clone(),
                g.prepared.config.clone()
            )
            .is_err()
        );
    });
}
#[test]
fn ramos_m1_nested_retain_and_reverse_exact_cost_use_pools() {
    for undo in [false, true] {
        traces(setup(true, true), |g| {
            let r = paid_counters(g);
            let forest = g
                .state
                .rules
                .objects
                .values()
                .find(|o| {
                    o.zone == Zone::Battlefield
                        && !o.tapped
                        && g.definition(o.id.card).unwrap().name == "Forest"
                })
                .unwrap()
                .id;
            answer(g, Response::ActivateMana { object: forest });
            let _ = request(g);
            let before = g.state.rules.players[0].mana.clone();
            answer(
                g,
                Response::Cast {
                    card: named(g, NAMES[5]).card,
                },
            );
            let _ = request(g);
            answer(
                g,
                Response::ActivateMana {
                    object: named(g, "Golgari Signet"),
                },
            );
            let _ = request(g);
            answer(g, Response::ActivateMana { object: r });
            let _ = request(g);
            check(g);
            answer(g, Response::FinishMana { undo_mana: false });
            spend(g);
            let _ = request(g);
            check(g);
            assert_eq!(g.current(r).unwrap().plus_one_counters, 0.into());
            // Synthetic source-departure branches supplement accepted-input
            // reversal. Neither choice can restore/rebind the old incarnation.
            for reverse in [false, true] {
                let mut departed = g.fork();
                departed.move_object(r, Zone::Exile).unwrap();
                let exile = departed.object_id(r.card).unwrap();
                departed.move_object(exile, Zone::Battlefield).unwrap();
                let new = departed.object_id(r.card).unwrap();
                answer(&mut departed, Response::FinishMana { undo_mana: reverse });
                let _ = request(&mut departed);
                assert_eq!(departed.current(new).unwrap().plus_one_counters, 0.into());
                assert!(!departed.mana_ability_used(new));
                if reverse {
                    assert_eq!(departed.state.rules.players[0].mana, before);
                }
            }
            let placements = g
                .events()
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        Event::Committed {
                            actual: EventOutcome::PlusOneCountersPlaced { .. },
                            ..
                        }
                    )
                })
                .count();
            answer(g, Response::FinishMana { undo_mana: undo });
            let _ = request(g);
            assert!(g.state.rules.stack.is_empty());
            assert!(g.state.rules.pending_triggers.is_empty());
            assert_eq!(
                placements,
                g.events()
                    .iter()
                    .filter(|e| matches!(
                        e,
                        Event::Committed {
                            actual: EventOutcome::PlusOneCountersPlaced { .. },
                            ..
                        }
                    ))
                    .count()
            );
            assert_eq!(
                g.current(r).unwrap().plus_one_counters,
                Quantity::from(if undo { 5 } else { 0 })
            );
            assert_eq!(g.mana_ability_used(r), !undo);
            if undo {
                assert_eq!(g.state.rules.players[0].mana, before);
                assert!(!g.current(named(g, "Golgari Signet")).unwrap().tapped);
            } else {
                assert!(g.current(named(g, "Golgari Signet")).unwrap().tapped);
            }
            check(g);
            // Genuine next opponent turn clears this source's restriction.
            for _ in 0..100 {
                let req = request(g);
                if g.turn().number > 1 {
                    break;
                }
                let (actor, decision_id) = (req.player(), req.decision_id());
                let response = match req {
                    Request::Priority { .. } => Response::Pass,
                    Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                    Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                    Request::Discard {
                        count, candidates, ..
                    } => Response::Discard {
                        objects: candidates.into_iter().take(count).collect(),
                    },
                    x => panic!("{x:?}"),
                };
                g.respond(actor, decision_id, response).unwrap();
            }
            assert!(g.turn().number > 1);
            assert_eq!(g.active_player(), Some(PlayerId(1)));
            assert!(!g.mana_ability_used(r));
            check(g);
            if undo {
                for _ in 0..4 {
                    let req = request(g);
                    if req.player() == PlayerId(0) {
                        break;
                    }
                    g.respond(req.player(), req.decision_id(), Response::Pass)
                        .unwrap();
                }
                assert_eq!(request(g).player(), PlayerId(0));
                answer(g, Response::ActivateMana { object: r });
                let _ = request(g);
                assert!(g.mana_ability_used(r));
                assert_eq!(g.current(r).unwrap().plus_one_counters, 0.into());
                check(g);
            }
        });
    }
}
#[test]
fn ramos_m1_synthetic_colors_departure_countered_spell_and_copy_boundary() {
    for colors in [
        vec![],
        vec![Color::G],
        vec![Color::G, Color::U],
        vec![Color::W, Color::U, Color::B, Color::R, Color::G],
    ] {
        let mut p = setup(false, false);
        let mut c = p.catalog.as_ref().clone();
        c.definitions
            .iter_mut()
            .find(|d| d.name == NAMES[0])
            .unwrap()
            .characteristics
            .colors = colors.clone();
        p.catalog_digest = digest(&c).unwrap();
        p.catalog = Arc::new(c);
        let mut g = Game::new(p);
        let old = named(&g, "Ramos, Dragon Engine");
        announce(&mut g, old.card.0, vec![]);
        resolve(&mut g, vec![]);
        let r = named(&g, "Ramos, Dragon Engine");
        let card = named(&g, NAMES[0]).card;
        announce(&mut g, card.0, vec![]);
        let _ = request(&mut g);
        let original = g
            .state
            .rules
            .stack
            .iter()
            .find(|s| s.source.card == card)
            .unwrap()
            .clone();
        let trigger = g.state.rules.stack.last().unwrap().id;
        // Synthetic typed counter event supplements the real paid-cast witness.
        g.commit(
            ProposedEvent::CounterStackObject {
                target: original.id,
                countering: trigger,
                domain: TargetDomain::StackSpell,
            },
            BatchCause::Fixture,
        )
        .unwrap();
        assert!(g.current(original.source).is_err());
        resolve(&mut g, vec![]);
        assert_eq!(
            g.current(r).unwrap().plus_one_counters,
            Quantity::from(colors.len() as i64)
        );
        // Unpaid stack insertion cannot invoke the sole paid-cast event boundary.
        let mut unpaid = original.clone();
        unpaid.paid = None;
        g.state.rules.stack.push(unpaid.clone());
        let pending = g.state.rules.pending_triggers.clone();
        assert!(g.collect_cast_triggers(&unpaid).is_err());
        assert_eq!(g.state.rules.pending_triggers, pending);
        g.state.rules.stack.pop();
        // Queue another trigger and replace Ramos's incarnation before resolution.
        let card = named(&g, NAMES[1]).card;
        announce(&mut g, card.0, vec![]);
        g.move_object(r, Zone::Exile).unwrap();
        let ex = g.object_id(r.card).unwrap();
        g.move_object(ex, Zone::Battlefield).unwrap();
        let new = g.object_id(r.card).unwrap();
        resolve(&mut g, vec![]);
        assert_ne!(new, r);
        assert_eq!(g.current(new).unwrap().plus_one_counters, 0.into());
    }
}
#[test]
fn ramos_m1_synthetic_eligibility_control_and_incarnation_use() {
    let mut g = Game::new(setup(false, false));
    let old = named(&g, "Ramos, Dragon Engine");
    g.move_object(old, Zone::Battlefield).unwrap();
    let r = g.object_id(old.card).unwrap();
    let _ = request(&mut g);
    for n in [4, 5, 6] {
        let mut branch = g.fork();
        let o = branch.state.rules.objects.get_mut(&r.card).unwrap();
        o.plus_one_counters = n.into();
        o.tapped = true;
        o.attack_ready = false;
        let before = branch.state.clone();
        let trace = branch.trace.clone();
        let req = request(&mut branch);
        let result = branch.respond(
            PlayerId(0),
            req.decision_id(),
            Response::ActivateMana { object: r },
        );
        if n == 4 {
            assert!(result.is_err());
            assert_eq!(branch.state, before);
            assert_eq!(branch.trace, trace);
            continue;
        }
        result.unwrap();
        let _ = request(&mut branch);
        assert_eq!(
            branch.current(r).unwrap().plus_one_counters,
            Quantity::from(n - 5)
        );
        assert!(branch.mana_ability_used(r));
        branch
            .state
            .rules
            .objects
            .get_mut(&r.card)
            .unwrap()
            .plus_one_counters = 10.into();
        assert!(
            branch
                .validate_permanent_mana_activation(PlayerId(0), r)
                .is_err()
        );
        branch
            .state
            .rules
            .objects
            .get_mut(&r.card)
            .unwrap()
            .controller = PlayerId(1);
        assert!(
            branch
                .validate_permanent_mana_activation(PlayerId(1), r)
                .is_err()
        );
        branch.move_object(r, Zone::Exile).unwrap();
        let ex = branch.object_id(r.card).unwrap();
        branch.move_object(ex, Zone::Battlefield).unwrap();
        let new = branch.object_id(r.card).unwrap();
        assert!(!branch.mana_ability_used(new));
        branch
            .state
            .rules
            .objects
            .get_mut(&new.card)
            .unwrap()
            .plus_one_counters = 5.into();
        let player = branch.current(new).unwrap().controller;
        assert!(
            branch
                .validate_permanent_mana_activation(player, new)
                .is_ok()
        );
        assert!(
            branch
                .validate_permanent_mana_activation(player, r)
                .is_err()
        );
    }
    assert!(
        g.validate_permanent_mana_activation(PlayerId(1), r)
            .is_err()
    );
    // Opponent paid cast is a distinct event with no controller Ramos trigger.
    g.state.rules.objects.get_mut(&r.card).unwrap().controller = PlayerId(1);
    g.state
        .rules
        .objects
        .get_mut(&r.card)
        .unwrap()
        .default_controller = PlayerId(1);
    let card = named(&g, NAMES[0]).card;
    announce(&mut g, card.0, vec![]);
    resolve(&mut g, vec![]);
    assert_eq!(g.current(r).unwrap().plus_one_counters, 0.into());
}

#[test]
fn ramos_m1_synthetic_parameterized_counter_cost_and_fixed_output_family() {
    let mut p = setup(false, false);
    let mut c = p.catalog.as_ref().clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == RAMOS)
        .unwrap();
    d.mana_abilities[0].costs.remove_source_plus_one_counters = Some(3.into());
    d.mana_abilities[0].production = ManaProduction::Fixed {
        symbols: vec![Color::R, Color::G, Color::G],
    };
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    let mut g = Game::new(p);
    let cmd = named(&g, "Ramos, Dragon Engine");
    g.move_object(cmd, Zone::Battlefield).unwrap();
    let source = g.object_id(cmd.card).unwrap();
    let o = g.state.rules.objects.get_mut(&source.card).unwrap();
    o.plus_one_counters = 4.into();
    o.tapped = true;
    o.attack_ready = false;
    let _ = request(&mut g);
    answer(&mut g, Response::ActivateMana { object: source });
    let _ = request(&mut g);
    assert_eq!(g.current(source).unwrap().plus_one_counters, 1.into());
    assert_eq!(
        g.state.rules.players[0].mana,
        BTreeMap::from([(Color::R, 1.into()), (Color::G, 2.into())])
    );
    assert!(g.mana_ability_used(source));
}
