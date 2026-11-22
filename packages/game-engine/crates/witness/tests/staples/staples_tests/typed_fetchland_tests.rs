//! Frozen source witnesses through accepted public inputs, in private legal100
//! seeded setups. Synthetic cost/control/fault probes are identified separately.
//! CR 118.3b, 119.4, 602.2, 113.7a, 701.23b, 701.24a (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::typed_land_search_tests::{observation_work, typed_prepared, validate};
use super::*;
const DELTA: &str =
    include_str!("../../../../../tests/fixtures/ramos/ef86989d-ce80-4e55-aece-7d11710eeffa.json");
const HEATH: &str =
    include_str!("../../../../../tests/fixtures/ramos/29737a60-3ebd-40d9-b935-c4f54b90d45d.json");
const POOL: &str =
    include_str!("../../../../../tests/fixtures/ramos/20283c4a-f1f0-42f0-bc08-6da87474426b.json");
const CINDER: &str =
    include_str!("../../../../../tests/fixtures/ramos/dfac0258-e148-4d7d-8ded-fc2466d9caa6.json");
fn lower(raw: &str) -> Definition {
    match mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/ramos/archive-manifest-current.json"),
        raw,
    ) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    }
}
fn reseal(p: &mut Prepared) {
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    for seat in 0..4 {
        let mut counts = BTreeMap::<String, u32>::new();
        let mut names = BTreeSet::new();
        for row in p.config.cards.iter().filter(|r| r.owner == PlayerId(seat)) {
            let d = p.catalog.definition_by_oracle_id(&row.oracle_id).unwrap();
            assert!(d.characteristics.basic || names.insert(d.name.clone()));
            *counts.entry(row.oracle_id.clone()).or_default() += 1;
        }
        assert_eq!(counts.values().sum::<u32>(), 100);
        p.match_config.as_mut().unwrap().decks[seat as usize].cards = counts
            .into_iter()
            .map(|(oracle_id, count)| DeckEntry { oracle_id, count })
            .collect();
    }
    p.profile_digest = digest(&(&p.config, &p.match_config)).unwrap();
}
fn prepared(raw: &str) -> Prepared {
    let mut p = typed_prepared(raw);
    let mut probe = Game::new(p.clone());
    opening(&mut probe);
    let replace: Vec<_> = probe.state.rules.libraries[&PlayerId(0)]
        .iter()
        .rev()
        .filter(|id| probe.definition(**id).unwrap().name == "Forest")
        .take(2)
        .copied()
        .collect();
    assert_eq!(replace.len(), 2);
    for (id, source) in replace.into_iter().zip([POOL, CINDER]) {
        let d = lower(source);
        p.config
            .cards
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .oracle_id = d.oracle_id.clone();
        Arc::make_mut(&mut p.catalog).definitions.push(d);
    }
    reseal(&mut p);
    p
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
fn public(g: &mut Game, response: PlayerResponse) {
    let receipt = validate(g, response);
    g.submit(receipt).unwrap();
}
fn public_for(g: &mut Game, player: PlayerId, response: PlayerResponse) {
    let token = observation(g, player.0).decision.unwrap().token().clone();
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        g.check_player_response(player, token, response, Budget::unlimited())
    else {
        panic!("validated actor response")
    };
    g.submit(receipt).unwrap();
}
fn main(g: &mut Game) {
    opening(g);
    for _ in 0..40 {
        let r = request(g);
        if g.turn().step == TurnStep::PrecombatMain && r.player() == PlayerId(0) {
            return;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("bounded main priority");
}
fn play(g: &mut Game, raw: &str) -> ObjectId {
    main(g);
    let old = named(g, &lower(raw).name);
    let handle = g.handle_for(PlayerId(0), old).unwrap();
    public(g, PlayerResponse::PlayLand { object: handle });
    let _ = request(g);
    let source = g.object_id(old.card).unwrap();
    assert_ne!(source, old);
    assert_eq!(g.zone(source), Some(Zone::Battlefield));
    source
}
fn activate(g: &mut Game, source: ObjectId) -> StackInstance {
    activate_for(g, PlayerId(0), source)
}
fn activate_for(g: &mut Game, player: PlayerId, source: ObjectId) -> StackInstance {
    let handle = g.handle_for(player, source).unwrap();
    let before = g.state.clone();
    let trace = g.trace.clone();
    let token = observation(g, player.0).decision.unwrap().token().clone();
    let (outcome, continuation) = g
        .player_session(player)
        .action_beginnings(
            token,
            ActionScope::Object {
                object: handle.clone(),
            },
            64,
            Budget::unlimited(),
        )
        .into_parts();
    let ActionQueryOutcome::Complete { page } = outcome else {
        panic!("complete action query");
    };
    assert!(continuation.is_none());
    assert!(page.beginnings.iter().any(|b| matches!(
        b.initiation,
        ActionInitiation::ActivateAbility { ability: 0, .. }
    )));
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
    public_for(
        g,
        player,
        PlayerResponse::ActivateAbility {
            object: handle,
            ability: 0,
        },
    );
    // The accepted command still owns the old pre-cost state. A single staged
    // rule step must commit all costs before exposing any priority window.
    assert_eq!(g.zone(source), Some(Zone::Battlefield));
    let mut life = g.state.rules.players[player.0 as usize].life.clone();
    life.subtract(&1.into());
    let _ = g.advance(Budget::new(1));
    assert_eq!(g.state.rules.players[player.0 as usize].life, life);
    assert!(g.current(source).is_err());
    assert_eq!(
        g.zone(g.object_id(source.card).unwrap()),
        Some(Zone::Graveyard {
            owner: g.state.rules.cards[&source.card].owner
        })
    );
    let invocation = g.state.rules.stack.last().unwrap().clone();
    assert_eq!(invocation.source, source);
    assert_eq!(invocation.source_facts.as_ref().unwrap().object, source);
    assert_eq!(
        invocation.source_facts.as_ref().unwrap().zone,
        Zone::Battlefield
    );
    assert!(
        invocation.source_facts.as_ref().unwrap().tapped,
        "sacrifice retains post-tap LKI"
    );
    assert_eq!(invocation.paid.as_ref(), Some(&invocation.total_cost));
    let _ = request(g);
    invocation
}
fn search(g: &mut Game) -> Request {
    for _ in 0..8 {
        let r = request(g);
        if matches!(r, Request::SearchLands { .. }) {
            return r;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("bounded search");
}
#[test]
fn typed_fetchland_paid_public_zero_find_and_typed_nonbasic_entry_recover() {
    // Three traces only for these two bounded paths; no whole-game matrix.
    for (raw, target) in [(DELTA, None), (HEATH, Some("Cinder Glade"))] {
        traces(prepared(raw), |g| {
            let source = play(g, raw);
            let invocation = activate(g, source);
            let r = search(g);
            check(g);
            let Request::SearchLands {
                candidates,
                maximum: 1,
                ..
            } = &r
            else {
                panic!("typed search");
            };
            let selected = target.map(|name| named(g, name));
            if let Some(object) = selected {
                assert!(candidates.contains(&object));
            }
            // Delta accepts typed Island nonbasics, but excludes Forest/Mountain.
            assert!(candidates.contains(&named(g, "Breeding Pool")));
            if raw == DELTA {
                assert!(!candidates.contains(&named(g, "Cinder Glade")));
            }
            for seat in 0..4 {
                observation_work(g, PlayerId(seat));
                let own = observation(g, seat);
                assert!(
                    own.view
                        .objects
                        .iter()
                        .all(|o| !matches!(o.zone, Zone::Library { .. }))
                );
                if seat != 0 {
                    assert!(own.decision.is_none());
                }
            }
            let before = g.state.clone();
            for objects in [
                vec![source],
                vec![candidates[0], candidates[0]],
                vec![ObjectId {
                    card: candidates[0].card,
                    incarnation: candidates[0].incarnation + 1,
                }],
            ] {
                assert!(
                    g.respond(
                        PlayerId(0),
                        r.decision_id(),
                        Response::SearchLands { objects }
                    )
                    .is_err()
                );
                assert_eq!(g.state, before);
            }
            let PlayerRequest::SearchLands {
                candidates: visible,
                ..
            } = observation(g, 0).decision.unwrap()
            else {
                panic!();
            };
            for seat in 1..4 {
                assert!(
                    g.resolve_search_handle(PlayerId(seat), &visible[0].handle)
                        .is_err()
                );
            }
            let handles = selected
                .into_iter()
                .map(|o| {
                    visible
                        .iter()
                        .find(|v| g.resolve_search_handle(PlayerId(0), &v.handle).unwrap() == o)
                        .unwrap()
                        .handle
                        .clone()
                })
                .collect();
            let shuffle = observation(g, 0).view.players[0].library_shuffles;
            public(g, PlayerResponse::SearchLands { objects: handles });
            let _ = g.advance(Budget::new(1));
            if target.is_some() {
                assert_eq!(
                    g.state
                        .rules
                        .frame
                        .resolving_invocation(&g.state.rules.frames)
                        .map(|id| g.entry(id).unwrap()),
                    Some(&invocation),
                    "plain entry retains the activation through its shuffle continuation"
                );
                check(g);
            }
            resolve(g, vec![]);
            assert_eq!(
                observation(g, 0).view.players[0].library_shuffles,
                shuffle + 1
            );
            if let Some(old) = selected {
                let new = g.object_id(old.card).unwrap();
                assert_ne!(new, old);
                assert_eq!(g.zone(new), Some(Zone::Battlefield));
                assert!(
                    g.current(new).unwrap().tapped,
                    "Cinder counts prior basic lands, not printed type or fetch source"
                );
            }
            if g.trace.mode == TraceMode::Detailed {
                let batch = g.trace.records.iter().find_map(|r| match &r.record {
                    EvidenceRecord::Batch { batch } if matches!(batch.cause, BatchCause::AbilityActivation { source: o, .. } if o == source) => Some(batch),
                    _ => None,
                }).unwrap();
                assert!(
                    matches!(batch.occurrences[0].proposed, ProposedEvent::Tap { object } if object == source)
                );
                assert!(
                    matches!(&batch.occurrences[1].proposed, ProposedEvent::LoseLife { player: PlayerId(0), amount } if *amount == 1.into())
                );
                assert!(
                    matches!(batch.occurrences[2].proposed, ProposedEvent::Sacrifice { object, player: PlayerId(0) } if object == source)
                );
                assert_eq!(batch.occurrences.len(), 3);
            }
        });
    }
}
#[test]
fn typed_fetchland_shock_entry_retains_sacrificed_activation_root() {
    for pay in [false, true] {
        let mut g = Game::with_trace(prepared(DELTA), TraceMode::Detailed);
        let source = play(&mut g, DELTA);
        let invocation = activate(&mut g, source);
        let _ = search(&mut g);
        let pool = named(&g, "Breeding Pool");
        let PlayerRequest::SearchLands { candidates, .. } = observation(&g, 0).decision.unwrap()
        else {
            panic!();
        };
        let handle = candidates
            .iter()
            .find(|v| g.resolve_search_handle(PlayerId(0), &v.handle).unwrap() == pool)
            .unwrap()
            .handle
            .clone();
        public(
            &mut g,
            PlayerResponse::SearchLands {
                objects: vec![handle],
            },
        );
        assert!(matches!(request(&mut g), Request::OptionalEffect { .. }));
        let Frame::LandEntry {
            origin: EntryOrigin::Search { frame },
            ..
        } = &g.state.rules.frame
        else {
            panic!();
        };
        assert_eq!(*g.entry(frame.stack_id).unwrap(), invocation);
        assert!(g.current(source).is_err());
        check(&g);
        public(&mut g, PlayerResponse::OptionalEffect { perform: pay });
        resolve(&mut g, vec![]);
        assert_eq!(
            g.state.rules.players[0].life,
            Quantity::from(if pay { 37 } else { 39 })
        );
        assert_eq!(
            g.current(g.object_id(pool.card).unwrap()).unwrap().tapped,
            !pay
        );
        check(&g);
    }
}
#[test]
fn typed_fetchland_synthetic_exact_life_affordability_and_atomic_cost_failure() {
    for (cost, life, payable) in [
        ("0", "1", true),
        ("1", "1", true),
        ("2", "1", false),
        ("3", "4", true),
        (
            "999999999999999999999999999999999999999",
            "999999999999999999999999999999999999999",
            true,
        ),
        ("999999999999999999999999999999999999999", "40", false),
    ] {
        // Explicit synthetic IR quantities on the complete frozen source.
        let mut p = prepared(DELTA);
        p.config.initial.life[0] = life.parse().unwrap();
        let d = Arc::make_mut(&mut p.catalog)
            .definitions
            .iter_mut()
            .find(|d| d.name == "Polluted Delta")
            .unwrap();
        d.source_labels = None;
        d.activated[0].cost = ActivationCost::TapPayLifeSacrificeSource {
            life: cost.parse().unwrap(),
        };
        reseal(&mut p);
        let mut g = Game::with_trace(p, TraceMode::Detailed);
        let source = play(&mut g, DELTA);
        let r = request(&mut g);
        let before = g.state.clone();
        let trace = g.trace.clone();
        let result = g.respond(
            PlayerId(0),
            r.decision_id(),
            Response::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        if !payable {
            assert!(result.is_err());
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            continue;
        }
        result.unwrap();
        let _ = g.advance(Budget::new(1));
        let mut expected: Quantity = life.parse().unwrap();
        expected.subtract(&cost.parse().unwrap());
        assert_eq!(g.state.rules.players[0].life, expected);
        assert!(g.current(source).is_err());
        let events = g
            .events()
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Committed {
                        proposed: ProposedEvent::LoseLife { .. },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(events, usize::from(cost != "0"));
        check(&g);
    }
    let mut g = Game::with_trace(prepared(DELTA), TraceMode::Detailed);
    let source = play(&mut g, DELTA);
    // Synthetic validation controls: zero is payable even at negative life;
    // wrong controllers, tapped and stale incarnations cannot pay this source.
    let mut probe = g.fork();
    probe.state.rules.players[0].life = (-1).into();
    assert!(
        probe
            .validate_tap_life_sacrifice_source(PlayerId(0), source, &0.into())
            .is_ok()
    );
    assert!(
        probe
            .validate_tap_life_sacrifice_source(PlayerId(0), source, &1.into())
            .is_err()
    );
    assert!(
        g.validate_tap_life_sacrifice_source(PlayerId(1), source, &1.into())
            .is_err()
    );
    probe
        .state
        .rules
        .objects
        .get_mut(&source.card)
        .unwrap()
        .tapped = true;
    assert!(
        probe
            .validate_tap_life_sacrifice_source(PlayerId(0), source, &0.into())
            .is_err()
    );
    let stale = ObjectId {
        card: source.card,
        incarnation: source.incarnation + 1,
    };
    assert!(
        g.validate_tap_life_sacrifice_source(PlayerId(0), stale, &1.into())
            .is_err()
    );
    // Force failure in the LAST cost occurrence, after tap and life would apply.
    // The real staged executor preserves all pre-step semantics, retaining fault.
    let exhausted = ObjectId {
        card: source.card,
        incarnation: u32::MAX,
    };
    g.state.rules.objects.get_mut(&source.card).unwrap().id = exhausted;
    g.refresh_visibility().unwrap();
    let r = request(&mut g);
    g.respond(
        PlayerId(0),
        r.decision_id(),
        Response::ActivateAbility {
            object: exhausted,
            ability: 0,
        },
    )
    .unwrap();
    let before = g.state.clone();
    let trace = g.trace.clone();
    let Advance::Fault(reason) = g.advance(Budget::new(1)) else {
        panic!("preserved synthetic payment failure");
    };
    assert!(reason.contains("incarnation"), "{reason}");
    assert_eq!(g.state.rules.players, before.rules.players);
    assert_eq!(g.state.rules.objects, before.rules.objects);
    assert_eq!(g.state.rules.stack, before.rules.stack);
    assert_eq!(g.trace, trace);
}
// Develop and pay for a real Annex before testing borrowed activation. Every
// original library still has 99 cards during setup; no raw battlefield injection
// or invented shuffle authority is used.
fn borrowed_fetch() -> Game {
    let mut p = prepared(DELTA);
    let mut probe = Game::new(p.clone());
    opening(&mut probe);
    let chosen = probe
        .state
        .rules
        .objects
        .values()
        .find(|o| o.zone == (Zone::Hand { owner: PlayerId(1) }))
        .unwrap()
        .id
        .card;
    let annex = match mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../../tests/fixtures/5f1f49dd-b7ce-494a-9b4b-ac702d3664c5.json"),
    ) {
        mtg_compiler::CompileResult::Lowered { definition } => *definition,
        other => panic!("{other:?}"),
    };
    let island = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Island")
        .unwrap()
        .oracle_id
        .clone();
    let ramos = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Ramos, Dragon Engine")
        .unwrap()
        .oracle_id
        .clone();
    for row in &mut p.config.cards {
        if row.owner == PlayerId(1) {
            row.oracle_id = if matches!(row.zone, Zone::Command { .. }) {
                ramos.clone()
            } else if row.id == chosen {
                annex.oracle_id.clone()
            } else {
                island.clone()
            };
        }
    }
    p.match_config.as_mut().unwrap().decks[1].commander = ramos;
    Arc::make_mut(&mut p.catalog).definitions.push(annex);
    reseal(&mut p);
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let source = play(&mut g, DELTA);
    for _ in 0..2000 {
        let r = request(&mut g);
        if g.active_player() == Some(PlayerId(1))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(
                r,
                Request::Priority {
                    player: PlayerId(1),
                    ..
                }
            )
        {
            let islands = g
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.controller == PlayerId(1)
                        && o.zone == Zone::Battlefield
                        && g.definition(o.id.card).unwrap().name == "Island"
                })
                .count();
            if islands == 4 {
                let aura = named(&g, "Annex");
                announce(
                    &mut g,
                    aura.card.0,
                    vec![TargetEntity::Object { object: source }],
                );
                resolve(&mut g, vec![]);
                assert_eq!(g.current(source).unwrap().controller, PlayerId(1));
                assert_eq!(g.state.rules.cards[&source.card].owner, PlayerId(0));
                check(&g);
                return g;
            }
            if g.turn().lands_played == 0 {
                let card = g
                    .state
                    .rules
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == (Zone::Hand { owner: PlayerId(1) })
                            && g.definition(o.id.card).unwrap().name == "Island"
                    })
                    .unwrap()
                    .id
                    .card;
                g.respond(r.player(), r.decision_id(), Response::PlayLand { card })
                    .unwrap();
                continue;
            }
        }
        let response = match &r {
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Discard {
                candidates, count, ..
            } => Response::Discard {
                objects: candidates
                    .iter()
                    .filter(|o| g.definition(o.card).unwrap().name != "Annex")
                    .take(*count)
                    .copied()
                    .collect(),
            },
            Request::Priority { .. } => Response::Pass,
            other => panic!("borrowed fetch development {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("bounded paid Annex development")
}
#[test]
fn typed_fetchland_paid_root_survives_owner_departure_and_controller_departure_skips() {
    let borrowed = borrowed_fetch();
    let snapshot = borrowed.snapshot().unwrap();
    let prepared = borrowed.prepared.as_ref().clone();
    let source = named(&borrowed, "Polluted Delta");
    // An owner's departure after accepted input but before payment invalidates
    // the borrowed source without any partial life payment or activation.
    let mut before_cost = Game::restore(prepared.clone(), &snapshot).unwrap();
    let handle = before_cost.handle_for(PlayerId(1), source).unwrap();
    public_for(
        &mut before_cost,
        PlayerId(1),
        PlayerResponse::ActivateAbility {
            object: handle,
            ability: 0,
        },
    );
    before_cost.concede(PlayerId(0)).unwrap();
    let _ = request(&mut before_cost);
    assert_eq!(before_cost.state.rules.players[1].life, 40.into());
    assert!(before_cost.state.rules.stack.is_empty());
    assert!(
        !before_cost
            .events()
            .iter()
            .any(|e| matches!(e, Event::AbilityActivated { .. }))
    );
    check(&before_cost);
    for departing in [PlayerId(0), PlayerId(1)] {
        let mut g = Game::restore(prepared.clone(), &snapshot).unwrap();
        let invocation = activate_for(&mut g, PlayerId(1), source);
        assert_eq!(invocation.controller, PlayerId(1));
        assert_eq!(invocation.source_facts.as_ref().unwrap().owner, PlayerId(0));
        let _ = search(&mut g);
        g.concede(departing).unwrap();
        let next = request(&mut g);
        if departing == PlayerId(0) {
            assert!(matches!(
                next,
                Request::SearchLands {
                    player: PlayerId(1),
                    ..
                }
            ));
            assert!(!g.state.rules.objects.contains_key(&source.card));
            assert_eq!(
                g.state
                    .rules
                    .frame
                    .resolving_invocation(&g.state.rules.frames)
                    .map(|id| g.entry(id).unwrap()),
                Some(&invocation)
            );
            let shuffles = observation(&g, 1).view.players[1].library_shuffles;
            public_for(
                &mut g,
                PlayerId(1),
                PlayerResponse::SearchLands { objects: vec![] },
            );
            resolve(&mut g, vec![]);
            assert_eq!(
                observation(&g, 1).view.players[1].library_shuffles,
                shuffles + 1
            );
        } else {
            assert!(!matches!(
                next,
                Request::SearchLands {
                    player: PlayerId(1),
                    ..
                }
            ));
            assert!(
                !g.state
                    .rules
                    .objects
                    .values()
                    .any(|o| g.state.rules.cards[&o.id.card].owner == PlayerId(1))
            );
        }
        check(&g);
    }
}
