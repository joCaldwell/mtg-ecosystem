//! Exact frozen sources in private seedless paid roots; no pod admission.
use super::*;
const SPHINX: &str =
    include_str!("../../../../tests/fixtures/ramos/71366469-04ea-4526-bf24-7abc02fdc5ab.json");
fn root(original: &str, library: &[&str], artifacts: usize, imoti: bool) -> Prepared {
    let mut p = m4c_tests::root("Bloodbraid Elf", &vec!["Forest"; library.len()], false);
    let c = Arc::make_mut(&mut p.catalog);
    c.definitions.push(lowered(SPHINX));
    c.definitions.push(lowered(include_str!(
        "../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json"
    )));
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let id = |name: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .oracle_id
            .clone()
    };
    p.config.cards[1].oracle_id = id(original);
    let hydra = id("Managorger Hydra");
    let forest = id("Forest");
    for setup in &mut p.config.cards {
        if setup.oracle_id == hydra {
            setup.oracle_id = forest.clone();
        }
    }
    for (s, name) in p
        .config
        .cards
        .iter_mut()
        .filter(|s| s.owner == PlayerId(0) && matches!(s.zone, Zone::Library { .. }))
        .zip(library)
    {
        s.oracle_id = id(name);
    }
    // Exact artifact creatures, each contributes one, including dual types.
    for _ in 0..artifacts {
        m4a_tests::replace(&mut p, PlayerId(0), "Ethersworn Sphinx", Zone::Battlefield);
    }
    if imoti {
        m4a_tests::replace(
            &mut p,
            PlayerId(0),
            "Imoti, Celebrant of Bounty",
            Zone::Battlefield,
        );
    }
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    p
}
#[test]
fn m4e_paid_zero_one_many_floor_printed_mv_grants_ramos_history_and_replay() {
    for (n, generic) in [(0, 7), (1, 6), (3, 4), (9, 0)] {
        let mut g = Game::with_trace(
            root("Ethersworn Sphinx", &["Forest"], n, true),
            TraceMode::Off,
        );
        let id = paid(&mut g, "Ethersworn Sphinx");
        let spell = g.state.rules.stack.iter().find(|s| s.id == id).unwrap();
        assert_eq!(
            spell.paid,
            Some(ManaCost {
                generic: generic.into(),
                colored: vec![Color::W, Color::U]
            })
        );
        assert_eq!(g.stack_mana_value(spell).unwrap(), 9.into());
        assert_eq!(m4c_tests::grants(&g, id).len(), 1);
        assert_eq!(m4d_tests::flags(&g, 0), (true, false));
        let counts=g.state.rules.stack.iter().filter(|s|matches!(&s.kind,StackKind::Triggered{witness:TriggerEventWitness::SpellCast{cast},..} if cast.stack_id==id)).count();
        assert_eq!(counts, 2); // own cascade and Imoti
        prove(&g);
        if n == 3 {
            let mut lie = g.fork();
            let s = lie
                .state
                .rules
                .stack
                .iter_mut()
                .find(|s| s.id == id)
                .unwrap();
            let mut lock = serde_json::to_value(&s.cost_lock).unwrap();
            lock["reducers"][0]["count"] = 4.into();
            s.cost_lock = serde_json::from_value(lock).unwrap();
            s.total_cost.generic = 3.into();
            s.paid = Some(s.total_cost.clone());
            lie.validate_cost_lock(lie.state.rules.stack.iter().find(|s| s.id == id).unwrap())
                .unwrap();
            assert!(
                Game::restore(lie.prepared.as_ref().clone(), &lie.snapshot().unwrap()).is_err()
            );
        }
        let mut forged = g.fork();
        let s = forged
            .state
            .rules
            .stack
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap();
        s.total_cost.generic.add(&1.into());
        s.paid = Some(s.total_cost.clone());
        assert!(
            Game::restore(
                forged.prepared.as_ref().clone(),
                &forged.snapshot().unwrap()
            )
            .is_err()
        );
    }
}
#[test]
fn m4e_genuine_free_affinity_child_still_mv9_and_cascade() {
    let mut g = Game::with_trace(
        root(
            "Apex Devastator",
            &["Ethersworn Sphinx", "Grizzly Bears"],
            3,
            false,
        ),
        TraceMode::Off,
    );
    paid(&mut g, "Apex Devastator");
    offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    m3c_tests::finish_announcement(&mut g, false, None, None);
    let child = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.cast_from == Zone::Exile)
        .unwrap();
    assert_eq!(
        g.definition(child.source.card).unwrap().name,
        "Ethersworn Sphinx"
    );
    assert_eq!(child.paid, Some(zero_mana_cost()));
    assert_eq!(g.stack_mana_value(child).unwrap(), 9.into());
    assert_eq!(m4d_tests::flags(&g, 0), (true, true));
    prove(&g);
}
#[test]
fn m4e_current_control_before_lock_and_owner_concession_during_mana() {
    let mut p = root("Control Magic", &["Forest"], 0, false);
    m4a_tests::replace(
        &mut p,
        PlayerId(0),
        "Ethersworn Sphinx",
        Zone::Hand { owner: PlayerId(0) },
    );
    m4a_tests::replace(&mut p, PlayerId(1), "Ethersworn Sphinx", Zone::Battlefield);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Off);
    let target = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            o.zone == Zone::Battlefield
                && g.definition(o.id.card).unwrap().name == "Ethersworn Sphinx"
        })
        .unwrap()
        .id;
    let card = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            matches!(o.zone, Zone::Hand { owner: PlayerId(0) })
                && g.definition(o.id.card).unwrap().name == "Control Magic"
        })
        .unwrap()
        .id
        .card;
    // Opponent artifact is excluded. A genuine paid Aura then borrows it.
    let sphinx = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            matches!(o.zone, Zone::Hand { owner: PlayerId(0) })
                && g.definition(o.id.card).unwrap().name == "Ethersworn Sphinx"
        })
        .unwrap()
        .id
        .card;
    assert_eq!(
        g.casting_cost(sphinx, PlayerId(0), Zone::Hand { owner: PlayerId(0) })
            .unwrap()
            .generic,
        7.into()
    );
    answer(&mut g, Response::Cast { card });
    m3c_tests::finish_announcement(&mut g, false, Some(target), None);
    let aura = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell())
        .unwrap()
        .id;
    m4a_tests::resolve_entry(&mut g, aura);
    assert_eq!(g.current(target).unwrap().controller, PlayerId(0));
    answer(&mut g, Response::Cast { card: sphinx });
    let Request::Mana { cost, .. } = request(&mut g) else {
        panic!()
    };
    assert_eq!(cost.generic, 6.into());
    prove(&g);
    g.concede(PlayerId(1)).unwrap();
    let Request::Mana { cost, .. } = request(&mut g) else {
        panic!()
    };
    assert_eq!(cost.generic, 6.into());
    assert!(g.current(target).is_err());
    m3c_tests::finish_announcement(&mut g, false, None, None);
    let s = g
        .state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell())
        .unwrap();
    assert_eq!(s.paid.as_ref().unwrap().generic, 6.into());
    prove(&g);
}
#[test]
fn m4e_actual_artifact_land_and_ramos_wu_payment_printed_color_counters() {
    let mut p = root("Ethersworn Sphinx", &["Forest"], 5, true);
    m4a_tests::replace(&mut p, PlayerId(0), "Seat of the Synod", Zone::Battlefield);
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Off);
    let ramos = paid(&mut g, "Ramos, Dragon Engine");
    m4a_tests::resolve_entry(&mut g, ramos);
    let id = paid(&mut g, "Ethersworn Sphinx");
    let spell = g.state.rules.stack.iter().find(|s| s.id == id).unwrap();
    assert_eq!(
        spell.paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![Color::W, Color::U]
        })
    );
    assert_eq!(g.stack_mana_value(spell).unwrap(), 9.into());
    assert_eq!(m4c_tests::grants(&g, id).len(), 1);
    let occurrence = m4d_tests::witness(&g, id);
    assert_eq!(
        occurrence.spell.characteristics.colors,
        lowered(SPHINX).characteristics.colors
    );
    prove(&g);
    m4c_tests::settle(&mut g);
    let ramos = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            o.zone == Zone::Battlefield
                && g.definition(o.id.card).unwrap().name == "Ramos, Dragon Engine"
        })
        .unwrap()
        .id;
    assert_eq!(g.current(ramos).unwrap().plus_one_counters, 2.into());
    prove(&g);
}
#[test]
fn m4e_producer_identity() {
    eprintln!(
        "M4E_PRODUCER {}",
        serde_json::json!({"build_id":crate::BUILD_ID,"profile":crate::BUILD_PROFILE,"opt_level":crate::BUILD_OPT_LEVEL,"debug_assertions":crate::BUILD_DEBUG_ASSERTIONS,"rustc":crate::BUILD_RUSTC,"target":crate::BUILD_TARGET})
    );
    assert_eq!(crate::BUILD_OPT_LEVEL, "2");
    assert!(cfg!(debug_assertions));
}
#[test]
fn m4e_synthetic_typed_multiple_slots_and_ordinary_zero_keep_mana_opportunity() {
    for zero in [false, true] {
        let mut p = root("Ethersworn Sphinx", &["Forest"], 1, false);
        let catalog = Arc::make_mut(&mut p.catalog);
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Ethersworn Sphinx")
            .unwrap();
        if zero {
            d.characteristics.mana_cost = zero_mana_cost();
        } else {
            let mut reducer = d.static_abilities[0].clone();
            let StaticEffect::ReduceSelfGenericCost { per_object, .. } = &mut reducer.effect else {
                panic!()
            };
            *per_object = 2.into();
            d.static_abilities = vec![reducer.clone(), reducer];
        }
        d.validate().unwrap();
        p.catalog_digest = digest(catalog).unwrap();
        let mut g = Game::with_trace(p, TraceMode::Off);
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                matches!(o.zone, Zone::Hand { owner: PlayerId(0) })
                    && g.definition(o.id.card).unwrap().name == "Ethersworn Sphinx"
            })
            .unwrap()
            .id
            .card;
        answer(&mut g, Response::Cast { card: source });
        let Request::Mana { cost, .. } = request(&mut g) else {
            panic!("ordinary zero must retain mana opportunity")
        };
        assert_eq!(cost.generic, if zero { 0.into() } else { 3.into() });
        m3c_tests::finish_announcement(&mut g, false, None, None);
        prove(&g);
    }
}
