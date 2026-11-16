//! Complete frozen sources; private legal 100-card configurations, no admission.
//! Midgame fixtures declare their initial zones/control BEFORE Game construction;
//! subsequent natural witnesses use accepted inputs and prove restore/replay.
use super::typed_land_search_tests::{observation_work, validate};
use super::*;
const CINDER: &str =
    include_str!("../../../../tests/fixtures/ramos/dfac0258-e148-4d7d-8ded-fc2466d9caa6.json");
const PRAIRIE: &str =
    include_str!("../../../../tests/fixtures/conditional-land/prairie-stream.json");
const RAMOS: &str =
    include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json");
const LORE: &str =
    include_str!("../../../../tests/fixtures/ramos/78826359-fe63-44ad-adc4-a17ffcd710e4.json");
const CLAIM: &str =
    include_str!("../../../../tests/fixtures/ramos/376c9d3f-21d3-4251-bb6a-026fa9e1b0e1.json");
const FARSEEK: &str =
    include_str!("../../../../tests/fixtures/ramos/495e52e6-4c2b-4574-9474-eadbdcc8b4ac.json");
fn lower(raw: &str) -> Definition {
    let r = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    );
    let mtg_compiler::CompileResult::Lowered { definition } = r else {
        panic!("{r:?}");
    };
    *definition
}
fn land_catalog() -> Catalog {
    let mut c = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-mana-ir.json"
    ))
    .unwrap();
    for d in [
        lower(CINDER),
        lower(PRAIRIE),
        lower(RAMOS),
        lower(LORE),
        lower(CLAIM),
        lower(FARSEEK),
    ] {
        c.definitions.retain(|old| old.oracle_id != d.oracle_id);
        c.definitions.push(d);
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
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
// zone: hand=true; otherwise battlefield. A declared private midgame fixture,
// not a reachable opening/pod admission claim. Every owned deck remains legal100.
fn midgame(rows: &[(&str, u8, u8, bool)]) -> Prepared {
    let c = land_catalog();
    let mut cards = vec![];
    let mut commanders = vec![];
    for seat in 0..4 {
        let owner = PlayerId(seat);
        commanders.push(CardId(cards.len() as u32));
        cards.push(CardSetup {
            id: CardId(cards.len() as u32),
            oracle_id: lower(RAMOS).oracle_id,
            owner,
            controller: owner,
            zone: if rows
                .iter()
                .any(|r| r.0 == "Ramos, Dragon Engine" && r.1 == seat)
            {
                Zone::Battlefield
            } else {
                Zone::Command { owner }
            },
        });
        for (name, own, controller, hand) in rows
            .iter()
            .filter(|r| r.1 == seat && r.0 != "Ramos, Dragon Engine")
        {
            let d = c.definitions.iter().find(|d| d.name == *name).unwrap();
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: d.oracle_id.clone(),
                owner: PlayerId(*own),
                controller: PlayerId(*controller),
                zone: if *hand {
                    Zone::Hand { owner }
                } else {
                    Zone::Battlefield
                },
            });
        }
        while cards.iter().filter(|r| r.owner == owner).count() < 100 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner,
                controller: owner,
                zone: Zone::Library { owner },
            });
        }
        let mut names = BTreeSet::new();
        for row in cards.iter().filter(|r| r.owner == owner) {
            let d = c.definition_by_oracle_id(&row.oracle_id).unwrap();
            assert!(d.characteristics.basic || names.insert(d.name.clone()));
            assert!(
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|color| lower(RAMOS).characteristics.color_identity.contains(color))
            );
        }
    }
    assert_eq!(cards.len(), 400);
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::PrecombatMain,
            ..Default::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "conditional-land-private-midgame".into(),
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
fn public(g: &mut Game, response: PlayerResponse) {
    let receipt = validate(g, response);
    g.submit(receipt).unwrap();
}
fn choose(g: &mut Game, color: Color) {
    let r = request(g);
    assert!(matches!(r, Request::ManaColor { .. }));
    check(g);
    observation_work(g, PlayerId(0));
    for seat in 1..4 {
        assert!(observation(g, seat).decision.is_none());
    }
    let PlayerRequest::ManaColor {
        colors,
        mana_ability_source: Some(_),
        ..
    } = observation(g, 0).decision.unwrap()
    else {
        panic!("public intrinsic choice")
    };
    assert!(colors.contains(&color));
    let before = g.state.clone();
    let bad = if colors.contains(&Color::U) {
        Color::C
    } else {
        Color::U
    };
    assert!(
        g.respond(
            PlayerId(0),
            r.decision_id(),
            Response::ManaColor { color: bad }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    public(g, PlayerResponse::ManaColor { color });
    check(g);
    let mut tiny = g.fork();
    for _ in 0..100 {
        if !matches!(tiny.advance(Budget::new(1)), Advance::Suspended { .. }) {
            break;
        }
    }
    let _ = request(g);
    assert_eq!(tiny.state, g.state);
    assert_eq!(tiny.trace, g.trace);
    check(g);
}
fn activate_public(g: &mut Game, source: ObjectId) {
    let handle = g.handle_for(PlayerId(0), source).unwrap();
    public(g, PlayerResponse::ActivateMana { object: handle });
    check(g);
    let _ = request(g);
}
#[test]
fn conditional_lands_landplay_thresholds_current_controller_and_nonbasic_counts() {
    for n in 0..=3 {
        let mut rows = vec![
            ("Cinder Glade", 0, 0, true),
            ("Prairie Stream", 0, 0, false),
            ("Mountain", 0, 1, false),
            ("Plains", 1, 1, false),
        ];
        for _ in 0..n {
            rows.push(("Forest", 1, 0, false));
        }
        traces(midgame(&rows), |g| {
            let old = named(g, "Cinder Glade");
            let r = request(g);
            let card = g.handle_for(PlayerId(0), old).unwrap();
            public(g, PlayerResponse::PlayLand { card });
            check(g);
            let _ = request(g);
            let entered = named(g, "Cinder Glade");
            assert_ne!(old, entered);
            assert_eq!(g.current(entered).unwrap().tapped, n < 2);
            assert_eq!(g.current(entered).unwrap().controller, PlayerId(0));
            assert!(g.validate_mana_activation(PlayerId(0), old).is_err());
            assert!(
                g.respond(
                    PlayerId(1),
                    r.decision_id(),
                    Response::ActivateMana { object: entered }
                )
                .is_err()
            );
            check(g);
        });
    }
}
#[test]
fn conditional_lands_intrinsic_both_colors_public_work_restore_priority_cast_nested_undo() {
    for color in [Color::R, Color::G] {
        for window in 0..3 {
            for undo in [false, true] {
                traces(
                    midgame(&[
                        ("Cinder Glade", 0, 0, false),
                        ("Colossal Dreadmaw", 0, 0, true),
                        ("Golgari Signet", 0, 0, false),
                    ]),
                    |g| {
                        let source = named(g, "Cinder Glade");
                        let _ = request(g);
                        if window > 0 {
                            let card = named(g, "Colossal Dreadmaw").card;
                            answer(g, Response::Cast { card });
                            let _ = request(g);
                        }
                        if window == 2 {
                            let signet = named(g, "Golgari Signet");
                            answer(g, Response::ActivateMana { object: signet });
                            let _ = request(g);
                        }
                        let mana = g.state.rules.players[0].mana.clone();
                        activate_public(g, source);
                        assert!(!g.current(source).unwrap().tapped);
                        assert_eq!(g.state.rules.players[0].mana, mana);
                        assert!(matches!(
                            g.state.rules.frame,
                            Frame::IntrinsicManaChoice { .. }
                        ));
                        check(g);
                        // Independent parameter authority cannot be forged into snapshots.
                        let mut forged = g.fork();
                        if let Frame::IntrinsicManaChoice { source, .. } =
                            &mut forged.state.rules.frame
                        {
                            source.incarnation += 1;
                        }
                        assert!(
                            Game::restore(g.prepared.as_ref().clone(), &forged.snapshot().unwrap())
                                .is_err()
                        );
                        choose(g, color);
                        assert!(g.current(source).unwrap().tapped);
                        assert_eq!(g.mana(PlayerId(0), color), Some(1.into()));
                        if window == 2 {
                            answer(g, Response::FinishMana { undo_mana: false });
                            let _ = request(g);
                            answer(
                                g,
                                Response::SpendMana {
                                    spending: BTreeMap::from([(color, 1.into())]),
                                },
                            );
                            let _ = request(g);
                            assert_eq!(g.mana(PlayerId(0), Color::B), Some(1.into()));
                            assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
                            check(g);
                        }
                        if window > 0 {
                            answer(g, Response::FinishMana { undo_mana: undo });
                            let _ = request(g);
                            assert_eq!(g.current(source).unwrap().tapped, !undo);
                            check(g);
                        }
                    },
                );
            }
        }
    }
}
#[test]
fn conditional_lands_unpaid_choice_departure_and_paid_intrinsic_retention() {
    for timing in 0..3 {
        for departing in [1, 2] {
            traces(midgame(&[("Cinder Glade", 1, 0, false)]), |g| {
                let source = named(g, "Cinder Glade");
                let _ = request(g);
                activate_public(g, source);
                check(g);
                if timing > 0 {
                    public(g, PlayerResponse::ManaColor { color: Color::R });
                    check(g);
                }
                if timing == 2 {
                    let _ = request(g);
                    assert!(g.current(source).unwrap().tapped);
                }
                g.concede(PlayerId(departing)).unwrap();
                let _ = request(g);
                check(g);
                if departing == 1 {
                    assert!(g.current(source).is_err());
                    assert!(!matches!(
                        g.state.rules.frame,
                        Frame::IntrinsicManaChoice { .. }
                    ));
                    assert_eq!(
                        g.mana(PlayerId(0), Color::R).unwrap_or_default(),
                        Quantity::from(i64::from(timing == 2))
                    );
                } else if timing == 0 {
                    assert!(matches!(
                        g.state.rules.frame,
                        Frame::IntrinsicManaChoice { .. }
                    ));
                    choose(g, Color::G);
                }
            });
        }
    }
}
// Seeded real opening, with exact source rows substituted BEFORE construction;
// no mutation of a running Game supplies mana, cards, control or search choices.
fn natural_prepared(raw: &str) -> Prepared {
    let mut p = super::typed_land_search_tests::typed_prepared(raw);
    let mut probe = Game::new(p.clone());
    opening(&mut probe);
    let opening_forests: Vec<_> = probe
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Hand { owner: PlayerId(0) }
                && probe.definition(o.id.card).unwrap().name == "Forest"
        })
        .map(|o| o.id.card)
        .collect();
    assert!(opening_forests.len() >= 2);
    let c = land_catalog();
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    let tower = id("Command Tower");
    // typed_prepared's existing Tower is outside the opening; replace it with
    // the selected exact Cinder witness, then give the opening one Tower/Ring.
    for card in &mut p.config.cards {
        if card.owner == PlayerId(0) && card.oracle_id == tower {
            card.oracle_id = id("Cinder Glade");
        }
        if card.id == opening_forests[0] {
            card.oracle_id = tower.clone();
        }
        if card.id == opening_forests[1] {
            card.oracle_id = id("Sol Ring");
        }
    }
    p.catalog = Arc::new(c);
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    let decks = &mut p.match_config.as_mut().unwrap().decks;
    for seat in 0..4 {
        let mut counts = BTreeMap::<String, u32>::new();
        let mut names = BTreeSet::new();
        for card in p.config.cards.iter().filter(|c| c.owner == PlayerId(seat)) {
            *counts.entry(card.oracle_id.clone()).or_default() += 1;
            let d = p.catalog.definition_by_oracle_id(&card.oracle_id).unwrap();
            assert!(d.characteristics.basic || names.insert(d.name.clone()));
        }
        assert_eq!(counts.values().sum::<u32>(), 100);
        decks[seat as usize].cards = counts
            .into_iter()
            .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
            .collect();
    }
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
    p
}
fn spend_for(cost: &ManaCost, available: &BTreeMap<Color, Quantity>) -> BTreeMap<Color, Quantity> {
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
    spending
}
fn paid_cast(g: &mut Game, name: &str) {
    let card = named(g, name).card;
    answer(g, Response::Cast { card });
    for _ in 0..30 {
        let r = request(g);
        let response = match &r {
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| g.validate_mana_activation(r.player(), o.id).is_ok())
                        .min_by_key(|o| {
                            usize::from(
                                !g.mana_abilities(o.id)
                                    .unwrap()
                                    .iter()
                                    .any(|c| cost.colored.contains(c)),
                            )
                        })
                        .expect("real available payment source");
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::ManaColor { colors, .. } => Response::ManaColor {
                color: if colors.contains(&Color::G) {
                    Color::G
                } else {
                    colors[0]
                },
            },
            Request::SpendMana {
                cost, available, ..
            } => Response::SpendMana {
                spending: spend_for(cost, available),
            },
            Request::Priority { .. } => return,
            x => panic!("paid cast {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("paid cast did not complete");
}
fn natural_search(g: &mut Game, name: &str, basics: usize) -> Request {
    opening(g);
    for _ in 0..1500 {
        let r = request(g);
        if r.player() == PlayerId(0)
            && g.active_player() == Some(PlayerId(0))
            && matches!(r, Request::Priority { .. })
            && matches!(
                g.turn().step,
                TurnStep::PrecombatMain | TurnStep::PostcombatMain
            )
        {
            let basic_count = g
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && o.controller == PlayerId(0)
                        && g.characteristics(o.id).unwrap().basic
                })
                .count();
            let ring = named(g, "Sol Ring");
            if g.zone(ring) == Some(Zone::Hand { owner: PlayerId(0) })
                && g.state
                    .rules
                    .objects
                    .values()
                    .any(|o| g.validate_mana_activation(PlayerId(0), o.id).is_ok())
            {
                paid_cast(g, "Sol Ring");
                for _ in 0..12 {
                    if g.zone(named(g, "Sol Ring")) == Some(Zone::Battlefield) {
                        break;
                    }
                    answer(g, Response::Pass);
                }
                continue;
            }
            let tower = named(g, "Command Tower");
            let ready = g.zone(ring) == Some(Zone::Battlefield)
                && g.zone(tower) == Some(Zone::Battlefield)
                && !g.current(ring).unwrap().tapped
                && !g.current(tower).unwrap().tapped;
            if basic_count == basics && ready && (name != "Skyshroud Claim" || basics >= 1) {
                paid_cast(g, name);
                let top = g.top().unwrap();
                assert_eq!(top.paid.as_ref(), Some(&top.total_cost));
                check(g);
                for _ in 0..20 {
                    let r = request(g);
                    if matches!(r, Request::SearchLands { .. }) {
                        return r;
                    }
                    g.respond(r.player(), r.decision_id(), Response::Pass)
                        .unwrap();
                }
                panic!("paid search missing");
            }
            if g.turn().lands_played == 0 {
                let desired = if basic_count < basics {
                    "Forest"
                } else {
                    "Command Tower"
                };
                if let Some(o) = g.state.rules.objects.values().find(|o| {
                    o.zone == Zone::Hand { owner: PlayerId(0) }
                        && g.definition(o.id.card).unwrap().name == desired
                }) {
                    let card = o.id.card;
                    g.respond(r.player(), r.decision_id(), Response::PlayLand { card })
                        .unwrap();
                    continue;
                }
            }
        }
        let response = match &r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                count, candidates, ..
            } => Response::Discard {
                objects: candidates
                    .iter()
                    .filter(|o| g.definition(o.card).unwrap().name == "Forest")
                    .take(*count as usize)
                    .copied()
                    .collect(),
            },
            x => panic!("natural development {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("natural setup did not reach search");
}
#[test]
fn conditional_lands_paid_lore_claim_farseek_common_prior_entry_and_shuffle() {
    for (raw, name, basics, second) in [
        (LORE, "Nature's Lore", 0, false),
        (LORE, "Nature's Lore", 1, false),
        (LORE, "Nature's Lore", 2, false),
        (FARSEEK, "Farseek", 2, false),
        (CLAIM, "Skyshroud Claim", 1, true),
        (CLAIM, "Skyshroud Claim", 2, true),
    ] {
        for reverse in if second {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            traces(natural_prepared(raw), |g| {
                let r = natural_search(g, name, basics);
                let Request::SearchLands { candidates, .. } = &r else {
                    panic!()
                };
                let cinder = named(g, "Cinder Glade");
                assert!(candidates.contains(&cinder));
                let mut selected = vec![cinder];
                if second {
                    selected.push(
                        *candidates
                            .iter()
                            .find(|o| g.definition(o.card).unwrap().name == "Forest")
                            .unwrap(),
                    );
                }
                if *reverse {
                    selected.reverse();
                }
                let own = observation(g, 0);
                let PlayerRequest::SearchLands {
                    candidates: visible,
                    ..
                } = own.decision.unwrap()
                else {
                    panic!()
                };
                let handles = selected
                    .iter()
                    .map(|id| {
                        visible
                            .iter()
                            .find(|c| {
                                g.resolve_search_handle(PlayerId(0), &c.handle).unwrap() == *id
                            })
                            .unwrap()
                            .handle
                            .clone()
                    })
                    .collect();
                for seat in 1..4 {
                    let view = observation(g, seat);
                    assert!(view.decision.is_none());
                    assert!(
                        view.view
                            .objects
                            .iter()
                            .all(|o| !matches!(o.zone, Zone::Library { .. }))
                    );
                }
                public(g, PlayerResponse::SearchLands { objects: handles });
                check(g);
                let _ = request(g);
                let entered = named(g, "Cinder Glade");
                assert_ne!(entered, cinder);
                assert_eq!(
                    g.current(entered).unwrap().tapped,
                    name == "Farseek" || basics < 2
                );
                assert_eq!(g.current(entered).unwrap().default_controller, PlayerId(0));
                assert_eq!(
                    g.state.rules.match_state.as_ref().unwrap().library_shuffles[0],
                    1
                );
                check(g);
            });
        }
    }
}
#[test]
fn conditional_lands_ramos_paid_counter_mana_stays_immediate_and_payable_cast_keeps_receipts() {
    let mut rows = vec![
        ("Ramos, Dragon Engine", 0, 0, false),
        ("Wall of Vines", 0, 0, true),
        ("Wall of Wood", 0, 0, true),
        ("Dragon Sniper", 0, 0, true),
        ("Grizzly Bears", 0, 0, true),
        ("Canopy Spider", 0, 0, true),
        ("Colossal Dreadmaw", 0, 0, true),
    ];
    rows.extend(vec![("Forest", 0, 0, false); 9]);
    rows.push(("Cinder Glade", 1, 0, false));
    traces(midgame(&rows), |g| {
        for name in [
            "Wall of Vines",
            "Wall of Wood",
            "Dragon Sniper",
            "Grizzly Bears",
            "Canopy Spider",
        ] {
            paid_cast(g, name);
            resolve(g, vec![]);
        }
        let ramos = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == PlayerId(0)
                    && o.zone == Zone::Battlefield
                    && g.definition(o.id.card).unwrap().name == "Ramos, Dragon Engine"
            })
            .unwrap()
            .id;
        assert_eq!(g.current(ramos).unwrap().plus_one_counters, 5.into());
        let card = named(g, "Colossal Dreadmaw").card;
        answer(g, Response::Cast { card });
        let _ = request(g);
        activate_public(g, ramos);
        assert!(matches!(request(g), Request::Mana { .. }));
        assert_eq!(g.current(ramos).unwrap().plus_one_counters, 0.into());
        assert!(!g.current(ramos).unwrap().tapped);
        let cinder = named(g, "Cinder Glade");
        activate_public(g, cinder);
        choose(g, Color::R);
        check(g);
        g.concede(PlayerId(1)).unwrap();
        let _ = request(g);
        assert!(g.current(cinder).is_err());
        // FinishMana cannot reverse a fully payable cast, including its legal
        // immediate Ramos/Cinder activations. Undo on insufficient payment is
        // witnessed by the separate genuinely insufficient nested Signet case.
        answer(g, Response::FinishMana { undo_mana: true });
        let Request::SpendMana {
            cost, available, ..
        } = request(g)
        else {
            panic!("payable cast spending")
        };
        answer(
            g,
            Response::SpendMana {
                spending: spend_for(&cost, &available),
            },
        );
        let _ = request(g);
        check(g);
        assert_eq!(g.current(ramos).unwrap().plus_one_counters, 0.into());
        assert!(g.mana_ability_used(ramos));
    });
}
#[test]
fn conditional_lands_nested_unpaid_departure_differs_from_paid_production_choice() {
    for accepted in [false, true] {
        traces(
            midgame(&[
                ("Cinder Glade", 0, 0, false),
                ("Golgari Signet", 1, 0, false),
                ("Colossal Dreadmaw", 0, 0, true),
            ]),
            |g| {
                let card = named(g, "Colossal Dreadmaw").card;
                answer(g, Response::Cast { card });
                let _ = request(g);
                let signet = named(g, "Golgari Signet");
                activate_public(g, signet);
                let source = named(g, "Cinder Glade");
                activate_public(g, source);
                if accepted {
                    public(g, PlayerResponse::ManaColor { color: Color::G });
                    check(g);
                }
                g.concede(PlayerId(1)).unwrap();
                let _ = request(g);
                check(g);
                assert!(matches!(g.state.rules.frame, Frame::Mana { .. }));
                assert!(!g.current(source).unwrap().tapped);
                assert_eq!(g.mana(PlayerId(0), Color::G), Some(0.into()));
            },
        );
    }
    traces(midgame(&[("Command Tower", 1, 0, false)]), |g| {
        let tower = named(g, "Command Tower");
        let _ = request(g);
        activate_public(g, tower);
        assert!(g.current(tower).unwrap().tapped);
        assert!(matches!(g.state.rules.frame, Frame::ManaAbility { .. }));
        g.concede(PlayerId(1)).unwrap();
        let _ = request(g);
        check(g);
        assert!(matches!(g.state.rules.frame, Frame::ManaAbility { .. }));
        choose(g, Color::G);
        assert_eq!(g.mana(PlayerId(0), Color::G), Some(1.into()));
    });
}
#[test]
fn conditional_lands_synthetic_dependency_and_wrapper_accounting_probes() {
    // Explicit synthetic aggregate and payload-boundary probes, not a claimed
    // source producer or reachable paid simultaneous-basic-removal program.
    let mut g = Game::new(midgame(&[
        ("Cinder Glade", 0, 0, true),
        ("Forest", 0, 0, false),
    ]));
    let _ = request(&mut g);
    let land = named(&g, "Cinder Glade");
    let basic = g
        .state
        .rules
        .objects
        .values()
        .find(|o| o.zone == Zone::Battlefield && g.characteristics(o.id).unwrap().basic)
        .unwrap()
        .id;
    let before = g.state.clone();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::Move {
                        object: land,
                        destination: Zone::Battlefield
                    },
                    ProposedEvent::Move {
                        object: basic,
                        destination: Zone::Graveyard { owner: PlayerId(0) }
                    }
                ]
            }
        )
        .is_err()
    );
    assert_eq!(g.state, before);
    let p = midgame(&[("Cinder Glade", 0, 0, false)]);
    let mut g = Game::new(p);
    let _ = request(&mut g);
    let source = named(&g, "Cinder Glade");
    activate_public(&mut g, source);
    let Frame::IntrinsicManaChoice { resume, .. } = &g.state.rules.frame else {
        panic!()
    };
    assert_eq!(
        g.intrinsic_choice_charge(),
        stored_history::payload_charge(&g.state.rules.frame)
            - stored_history::payload_charge(resume)
    );
    assert!(matches!(
        spell_cascade_grants::retained_effect_frame(&g.state.rules.frame),
        Frame::Priority
    ));
}
