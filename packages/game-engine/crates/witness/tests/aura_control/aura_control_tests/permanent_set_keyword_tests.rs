//! Paid complete-source Heroic Intervention witnesses, private scenarios only.
//! Archive d705222b280a71cb, physical line 5491 / source index 5489;
//! raw record d3f0636f6881fb3a644f9a76bcf4c3cc03af1224d71cfe1e83b693fb4158369b.
//! Rules SHA256 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca.
//! CR20260925 109.5, 400.7, 608.2b/h, 611.2c, 702.11/12, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! New native tests UNRUN here; no pool/profile/policy/consumer admission.
use super::*;
use mtg_witness::temporary::through_cleanup;

const HEROIC: &str =
    include_str!("../../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json");
const PAIR: &[KeywordAbility] = &[KeywordAbility::Hexproof, KeywordAbility::Indestructible];

fn prepared(rows: &[Row]) -> Prepared {
    let p = prepared_with_sources(rows, &[HEROIC]);
    let d = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Heroic Intervention")
        .unwrap();
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(d.characteristics.mana_cost.generic, 1.into());
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G]);
    assert_eq!(d.provenance.source_index, 5489);
    assert_eq!(
        d.provenance.source_sha256,
        "c3e5efdcfd9bba5274c2ee23ce69db79540c43e4a6dd690b8d5e26010ed55377"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "0bde69f5f551f4e7de9b657198715bd62da91040fe9970c0afe74e8afa5ab7f0"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "2f6d3ffaf9781c4e848bf6273a92ee6fc0124f35100804d8898da68cc8831e64"
    );
    let program = d.spell.as_ref().unwrap();
    let grant = mtg_contracts::temporary_permanent_set_keyword_program(program).unwrap();
    assert_eq!(grant.set, &ObjectSet::ControllerPermanents);
    assert!(grant.modifier.is_none());
    assert_eq!(grant.keywords, PAIR);
    assert!(mtg_contracts::temporary_set_keyword_program(program).is_some());
    assert!(mtg_contracts::temporary_creature_set_keyword_program(program).is_none());
    let expected: BTreeSet<_> = [
        Capability::NormalLayoutV1,
        Capability::CastInstantV1,
        Capability::TemporaryPermanentSetKeywordProgramV1,
        Capability::AttachmentsV1,
        Capability::IndestructibleV1,
        Capability::BoardWipesV1,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        d.capabilities.iter().copied().collect::<BTreeSet<_>>(),
        expected
    );
    p
}

// Prefund the printed colored symbols before the generic payment, then use the
// ordinary announcement/SpendMana lifecycle. Names select basic mana witnesses
// only; neither the compiler nor runtime dispatches on any witness identity.
fn prefund(g: &mut Game, q: u64, id: u32) {
    let cost = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    for color in &cost.colored {
        fund(
            g,
            q,
            match color {
                Color::W => "Plains",
                Color::U => "Island",
                Color::B => "Swamp",
                Color::R => "Mountain",
                Color::G => "Forest",
                _ => panic!("basic color"),
            },
            1,
        );
    }
    loop {
        let player = request(g, q).player();
        if g.payment_pool(player, &cost).is_ok() {
            break;
        }
        let preferred = cost.colored.last().map(|color| match color {
            Color::W => "Plains",
            Color::U => "Island",
            Color::B => "Swamp",
            Color::R => "Mountain",
            Color::G => "Forest",
            _ => panic!("basic color"),
        });
        let sources = || {
            g.state.rules.objects.values().filter(|o| {
                o.controller == player
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
            })
        };
        let source = sources()
            .find(|o| preferred.is_some_and(|name| g.definition(o.id.card).unwrap().name == name))
            .or_else(|| sources().next())
            .unwrap()
            .id;
        answer(g, q, Response::ActivateMana { object: source });
    }
}

fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    prefund(g, q, id);
    announce(g, q, id, targets);
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}

fn cast_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce_paid(g, q, id, targets);
    resolve(g, q, |_| {});
}

fn announce_heroic(g: &mut Game, q: u64, id: u32) {
    prefund(g, q, id);
    answer(g, q, Response::Cast { card: CardId(id) });
    let r = request(g, q);
    assert!(matches!(&r, Request::Mana { cost, .. }
        if cost.generic == 1.into() && cost.colored == [Color::G]));
    let before = g.snapshot().unwrap();
    for response in [
        Response::Targets { objects: vec![] },
        Response::EntityTargets { entities: vec![] },
    ] {
        assert!(g.respond(r.player(), r.decision_id(), response).is_err());
        assert_eq!(g.snapshot().unwrap(), before);
    }
    answer(g, q, Response::FinishMana { undo_mana: false });
    assert!(matches!(request(g, q), Request::SpendMana { .. }));
    // All these witnesses provide exactly two Forest mana for the printed 1G.
    answer(
        g,
        q,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::G, 2.into())]),
        },
    );
    assert!(matches!(request(g, q), Request::Priority { .. }));
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
}

fn grant(g: &Game) -> (&ResolvedEffect, &[ObjectId]) {
    assert_eq!(g.state.rules.effects.len(), 1);
    let e = &g.state.rules.effects[0];
    let ResolvedEffectKind::GrantKeywordsSet {
        targets,
        keywords,
        modifier,
    } = &e.kind
    else {
        panic!("one permanent set grant")
    };
    assert_eq!(keywords, PAIR);
    assert!(modifier.is_none());
    assert_eq!(e.origin.source_block, 0);
    assert_eq!(e.origin.cursor, 0);
    (e, targets)
}

fn protected(g: &Game, id: u32, expected: bool) {
    for keyword in PAIR {
        assert_eq!(
            g.has_keyword(object(g, id), *keyword),
            expected,
            "card {id} / {keyword:?}"
        );
    }
}

fn zone(g: &Game, id: u32, expected: Zone) {
    assert_eq!(g.current(object(g, id)).unwrap().zone, expected);
}

fn resolve_top(g: &mut Game, q: u64, remaining: usize) {
    for _ in 0..1000 {
        let r = request(g, q);
        if g.state.rules.stack.len() == remaining {
            assert!(matches!(r, Request::Priority { .. }));
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("bounded top-only resolution")
}

fn queries(g: &Game) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let (e, captured) = grant(g);
    for viewer in 0..4 {
        let viewer = PlayerId(viewer);
        if !g.is_live(viewer) {
            continue;
        }
        let QueryValue::Observation(o) = finish_query(g, g.observe(viewer, Budget::new(0))) else {
            panic!("resumable observation")
        };
        assert!(
            !o.view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Library { .. }))
        );
        let effects = g.visible_effects(viewer).unwrap();
        let VisibleEffect::GrantKeywordsSet {
            handle,
            origin,
            targets,
            keywords,
            modifier,
            expires_in_cleanup_of_turn,
        } = effects.last().unwrap()
        else {
            panic!("visible permanent grant")
        };
        assert_eq!(
            handle,
            &g.effect_handle(viewer, &EffectKey::Resolved { id: e.id })
                .unwrap()
        );
        assert_eq!(origin.controller, e.origin.controller);
        assert_eq!(
            origin.source.announced,
            e.origin.source_handles[viewer.0 as usize]
        );
        assert_eq!(
            origin.source.current,
            g.handle_for(viewer, e.origin.source).ok()
        );
        assert_eq!(*expires_in_cleanup_of_turn, e.expires_in_cleanup_of_turn);
        assert_eq!(targets.len(), captured.len());
        assert_eq!(keywords, PAIR);
        assert!(modifier.is_none());
        for (i, target) in targets.iter().enumerate() {
            assert_eq!(
                target.announced,
                e.target_handles[i * 4 + viewer.0 as usize]
            );
            assert_eq!(target.current, g.handle_for(viewer, captured[i]).ok());
            if let Some(handle) = &target.current {
                let row = o.view.objects.iter().find(|o| &o.handle == handle).unwrap();
                for keyword in PAIR {
                    assert!(row.keywords.contains(keyword));
                }
            }
        }
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

#[test]
fn permanent_set_keyword_paid_mixed_capture_atomic_queries_and_normalized_forgery() {
    let rows = [
        ("Heroic Intervention", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Bonesplitter", 0, 0, false),
        ("Goblin Bombardment", 0, 0, false),
        ("Forest", 0, 0, false),
        ("Grizzly Bears", 0, 1, false),
        ("Bonesplitter", 1, 1, false),
        ("Grizzly Bears", 0, 0, true),
        ("Forest", 0, 0, false),
        ("Forest", 0, 0, false),
    ];
    both(prepared(&rows), |g, q| {
        let expected: Vec<_> = [1, 2, 3, 4, 8, 9].map(|id| object(g, id)).into();
        let before = g.state.clone();
        let trace = g.trace.clone();
        let QueryResult::Incomplete { work: stale, .. } = g.observe(PlayerId(0), Budget::new(0))
        else {
            panic!("query work")
        };
        finish_query(g, g.observe(PlayerId(0), Budget::new(0)));
        assert_eq!(g.state, before);
        assert_eq!(g.trace, trace);
        announce_heroic(g, q, 0);
        assert!(matches!(
            g.resume_work(stale, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
        ));
        // Use the identical first creation boundary in both outer budget runs.
        let mut complete = false;
        for _ in 0..2000 {
            let a = g.advance(Budget::new(1));
            if !g.state.rules.effects.is_empty() {
                assert_eq!(grant(g).1, expected);
                for id in [1, 2, 3, 4, 8, 9] {
                    protected(g, id, true);
                }
                queries(g);
                recover(g);
                complete = true;
                break;
            }
            for id in [1, 2, 3, 4, 8, 9] {
                protected(g, id, false);
            }
            match a {
                Advance::Suspended { .. } => (),
                Advance::AwaitingDecision(r) => {
                    g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
                }
                x => panic!("atomic permanent creation {x:?}"),
            }
        }
        assert!(complete);
        resolve(g, q, |_| {});
        for id in [5, 6, 7] {
            protected(g, id, false);
        }
        queries(g);
        let good = g.snapshot().unwrap();
        // Replace the creature recipient with an opposing artifact and normalize
        // every historical handle and the state hash. Local structure remains
        // valid; authenticated replay must reject the fabricated capture.
        let mut bad: Snapshot = strict_read(&good).unwrap();
        let mut forged = expected.clone();
        forged[0] = object(g, 6);
        forged.sort();
        let mut handles = Vec::new();
        for target in &forged {
            for viewer in 0..4 {
                handles.push(g.handle_for(PlayerId(viewer), *target).unwrap());
            }
        }
        let e = bad.state.rules.effects.last_mut().unwrap();
        let ResolvedEffectKind::GrantKeywordsSet { targets, .. } = &mut e.kind else {
            unreachable!()
        };
        *targets = forged;
        e.target_handles = handles;
        let mut structural = g.fork();
        structural.state = bad.state.clone();
        structural.validate_effect_state().unwrap();
        bad.state_sha256 = digest(&bad.state).unwrap();
        assert!(Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err());
        recover(g);
    });
}

#[test]
fn permanent_set_keyword_opposing_targets_candidates_new_cast_and_resolution_revalidation() {
    for other_target in [false, true] {
        let mut rows = vec![
            ("Heroic Intervention", 0, 0, true),
            ("Bonesplitter", 0, 0, false),
            ("Goblin Bombardment", 0, 0, false),
            ("Grizzly Bears", 0, 0, false),
            ("Naturalize", 1, 1, true),
            ("Naturalize", 1, 1, true),
            ("Disperse", 1, 1, true),
        ];
        if other_target {
            rows.push(("Bonesplitter", 1, 1, false));
        }
        lands(&mut rows, "Forest", 0, 2);
        lands(&mut rows, "Forest", 1, 4);
        lands(&mut rows, "Island", 1, 2);
        both(prepared(&rows), |g, q| {
            until_player(g, q, PlayerId(1));
            // The target is actually announced and paid before protection.
            announce_paid(g, q, 4, target(g, 1));
            assert_eq!(g.state.rules.stack.len(), 1);
            until_player(g, q, PlayerId(0));
            announce_heroic(g, q, 0);
            resolve_top(g, q, 1);
            for id in [1, 2, 3] {
                protected(g, id, true);
            }
            queries(g);
            recover(g);
            until_player(g, q, PlayerId(1));
            if other_target {
                // Query a real target-stage request on an independent fork;
                // opposing candidates must omit EVERY protected nonland kind.
                let mut probe = g.fork();
                answer(&mut probe, q, Response::Cast { card: CardId(6) });
                let r = request(&mut probe, q);
                let Request::Targets { candidates, .. } = &r else {
                    panic!("Disperse candidates")
                };
                assert_eq!(candidates, &[object(&probe, 7)]);
                let before = probe.snapshot().unwrap();
                let trace = probe.trace.clone();
                let QueryValue::Decision(Some(PlayerRequest::Targets { candidates, .. })) =
                    finish_query(
                        &probe,
                        probe.player_decision_for(PlayerId(1), Budget::new(0)),
                    )
                else {
                    panic!("public opposing target candidates")
                };
                assert_eq!(
                    candidates,
                    [probe.handle_for(PlayerId(1), object(&probe, 7)).unwrap()]
                );
                assert_eq!(probe.snapshot().unwrap(), before);
                assert_eq!(probe.trace, trace);
                for id in [1, 2, 3] {
                    let response = Response::Targets {
                        objects: vec![object(&probe, id)],
                    };
                    assert!(
                        probe
                            .respond(r.player(), r.decision_id(), response)
                            .is_err()
                    );
                    assert_eq!(probe.snapshot().unwrap(), before);
                    assert_eq!(probe.trace, trace);
                }
                recover(&probe);
            } else {
                let r = request(g, q);
                let before = g.snapshot().unwrap();
                let trace = g.trace.clone();
                for id in [5, 6] {
                    assert!(
                        g.respond(
                            r.player(),
                            r.decision_id(),
                            Response::Cast { card: CardId(id) }
                        )
                        .is_err()
                    );
                    assert_eq!(g.snapshot().unwrap(), before);
                    assert_eq!(g.trace, trace);
                }
            }
            // Resolution must revalidate the earlier legal opposing target.
            resolve(g, q, |_| {});
            zone(g, 1, Zone::Battlefield);
            zone(g, 4, Zone::Graveyard { owner: PlayerId(1) });
            assert!(g.state.rules.stack.is_empty());
            recover(g);
        });
    }
}

#[test]
fn permanent_set_keyword_own_naturalize_planar_cleansing_cleanup_fresh_destruction() {
    let mut rows = vec![
        ("Heroic Intervention", 0, 0, true),
        ("Bonesplitter", 0, 0, false),
        ("Goblin Bombardment", 0, 0, false),
        ("Grizzly Bears", 0, 0, false),
        ("Bonesplitter", 1, 1, false),
        ("Goblin Bombardment", 1, 1, false),
        ("Grizzly Bears", 1, 1, false),
        ("Naturalize", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Planar Cleansing", 0, 0, true),
        ("Naturalize", 0, 0, true),
    ];
    lands(&mut rows, "Forest", 0, 8);
    lands(&mut rows, "Plains", 0, 6);
    both(prepared(&rows), |g, q| {
        announce_heroic(g, q, 0);
        resolve(g, q, |_| {});
        let captured = grant(g).1.to_vec();
        // Own Hexproof never bars either of these genuine paid target choices.
        cast_paid(g, q, 7, target(g, 1));
        cast_paid(g, q, 8, target(g, 2));
        for id in [1, 2] {
            zone(g, id, Zone::Battlefield);
        }
        cast_paid(g, q, 9, vec![]);
        for id in [1, 2, 3] {
            zone(g, id, Zone::Battlefield);
        }
        for id in [4, 5, 6] {
            zone(g, id, Zone::Graveyard { owner: PlayerId(1) });
        }
        for object in &captured {
            assert_eq!(g.current(*object).unwrap().zone, Zone::Battlefield);
        }
        if g.trace.mode == TraceMode::Detailed {
            for id in [1, 2, 3] {
                assert!(batches(g).iter().flat_map(|b| &b.occurrences).any(|o| matches!(&o.actual,
                    EventOutcome::DestructionPrevented { object: prevented, .. } if *prevented == object(g, id))));
            }
        }
        queries(g);
        recover(g);
        through_cleanup(g, q);
        assert!(g.state.rules.effects.is_empty());
        for id in [1, 2, 3] {
            protected(g, id, false);
        }
        until_player(g, q, PlayerId(0));
        cast_paid(g, q, 10, target(g, 2));
        zone(g, 2, Zone::Graveyard { owner: PlayerId(0) });
        for id in [1, 3] {
            zone(g, id, Zone::Battlefield);
        }
    });
}

#[test]
fn permanent_set_keyword_late_artifact_entry_and_actual_bounce_reentry_never_reselect() {
    let mut rows = vec![
        ("Heroic Intervention", 0, 0, true),
        ("Bonesplitter", 0, 0, false),
        ("Goblin Bombardment", 0, 0, false),
        ("Grizzly Bears", 0, 0, false),
        ("Bonesplitter", 0, 0, true),
        ("Disperse", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Bonesplitter", 1, 1, false),
    ];
    lands(&mut rows, "Forest", 0, 10);
    lands(&mut rows, "Island", 0, 2);
    both(prepared(&rows), |g, q| {
        let old = object(g, 1);
        announce_heroic(g, q, 0);
        resolve(g, q, |_| {});
        let captured = grant(g).1.to_vec();
        cast_paid(g, q, 4, vec![]);
        protected(g, 4, false);
        protected(g, 8, false);
        cast_paid(g, q, 5, target(g, 1));
        zone(g, 1, Zone::Hand { owner: PlayerId(0) });
        assert!(g.current(old).is_err());
        queries(g);
        cast_paid(g, q, 1, vec![]);
        assert_ne!(object(g, 1), old);
        protected(g, 1, false);
        assert_eq!(grant(g).1, captured);
        // The fixed capture retains the old historical target, never the new
        // battlefield incarnation, although its physical card is the same.
        for viewer in 0..4 {
            let visible = g.visible_effects(PlayerId(viewer)).unwrap();
            let VisibleEffect::GrantKeywordsSet { targets, .. } = visible.last().unwrap() else {
                unreachable!()
            };
            let index = captured.iter().position(|t| *t == old).unwrap();
            assert!(targets[index].current.is_none());
        }
        cast_paid(g, q, 6, target(g, 1));
        cast_paid(g, q, 7, target(g, 4));
        for id in [1, 4] {
            zone(g, id, Zone::Graveyard { owner: PlayerId(0) });
        }
        for id in [2, 3] {
            protected(g, id, true);
        }
        queries(g);
        recover(g);
    });
}

#[test]
fn permanent_set_keyword_paid_control_magic_frozen_recipients_and_departure_authority() {
    for depart_caster in [false, true] {
        let mut rows = vec![
            ("Heroic Intervention", 0, 0, true),
            ("Grizzly Bears", 2, 2, false),
            ("Grizzly Bears", 1, 1, false),
            ("Control Magic", 0, 0, true),
            ("Control Magic", 0, 0, true),
            ("Disperse", 0, 0, true),
        ];
        lands(&mut rows, "Forest", 0, 2);
        lands(&mut rows, "Island", 0, 10);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 3, target(g, 1));
            control(g, 1, 0, 2, 2);
            let borrowed = object(g, 1);
            announce_heroic(g, q, 0);
            resolve(g, q, |_| {});
            let captured = grant(g).1.to_vec();
            assert!(captured.contains(&borrowed));
            assert!(captured.contains(&object(g, 3))); // Actual enchantment too.
            assert!(!captured.contains(&object(g, 2)));
            protected(g, 1, true);
            if depart_caster {
                g.concede(PlayerId(0)).unwrap();
                request(g, q);
                assert!(!g.is_live(PlayerId(0)));
                assert!(g.object_id(CardId(0)).is_none());
                assert!(g.object_id(CardId(3)).is_none());
                control(g, 1, 2, 2, 2);
                protected(g, 1, true);
                assert_eq!(grant(g).0.origin.controller, PlayerId(0));
                queries(g);
                recover(g);
                g.concede(PlayerId(2)).unwrap();
                request(g, q);
                assert!(g.object_id(CardId(1)).is_none());
            } else {
                // Bounce our own protected Aura through a real legal target;
                // the creature's current controller changes, its grant does not.
                cast_paid(g, q, 5, target(g, 3));
                control(g, 1, 2, 2, 2);
                protected(g, 1, true);
                cast_paid(g, q, 4, target(g, 2));
                control(g, 2, 0, 1, 1);
                protected(g, 2, false); // Becoming controlled later is too late.
                protected(g, 4, false); // The late Aura also stays outside.
                assert_eq!(grant(g).1, captured);
                queries(g);
                recover(g);
                g.concede(PlayerId(2)).unwrap();
                request(g, q);
                assert!(g.object_id(CardId(1)).is_none());
            }
            assert_eq!(grant(g).1, captured);
            queries(g);
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
        });
    }
}

#[test]
fn permanent_set_keyword_paid_bombardment_current_and_own_disperse_departed_source_lki() {
    for bounce_source in [false, true] {
        let mut rows = vec![
            ("Heroic Intervention", 0, 0, true),
            ("Goblin Bombardment", 0, 0, false),
            ("Grizzly Bears", 0, 0, false),
            ("Yoked Ox", 1, 1, false),
            ("Disperse", 0, 0, true),
        ];
        lands(&mut rows, "Forest", 0, 2);
        lands(&mut rows, "Island", 0, 2);
        both(prepared(&rows), |g, q| {
            announce_heroic(g, q, 0);
            resolve(g, q, |_| {});
            let source = object(g, 1);
            let sacrificed = object(g, 2);
            protected(g, 1, true);
            answer(
                g,
                q,
                Response::ActivateAbility {
                    object: source,
                    ability: 0,
                },
            );
            answer(
                g,
                q,
                Response::EntityTargets {
                    entities: target(g, 3),
                },
            );
            let r = request(g, q);
            assert!(
                matches!(&r, Request::SacrificeCost { candidates, .. } if candidates == &[sacrificed])
            );
            answer(g, q, Response::SacrificeCost { object: sacrificed });
            assert!(matches!(request(g, q), Request::Priority { .. }));
            let ping = g.state.rules.stack.last().unwrap();
            assert!(ping.paid.is_some());
            assert_eq!(ping.source, source); // The enchantment, not the creature.
            assert_eq!(ping.controller, PlayerId(0));
            zone(g, 2, Zone::Graveyard { owner: PlayerId(0) });
            assert!(g.current(sacrificed).is_err());
            recover(g);
            if bounce_source {
                // Own Hexproof permits the genuine paid Disperse. Stop before
                // the lower ability, preserving the departed noncreature LKI.
                announce_paid(g, q, 4, target(g, 1));
                resolve_top(g, q, 1);
                assert!(g.current(source).is_err());
                zone(g, 1, Zone::Hand { owner: PlayerId(0) });
                let saved = g
                    .state
                    .rules
                    .stack
                    .last()
                    .unwrap()
                    .source_facts
                    .as_ref()
                    .unwrap();
                assert_eq!(saved.object, source);
                assert_eq!(saved.characteristics.kind, CardKind::Enchantment);
                assert_eq!(saved.zone, Zone::Battlefield);
                for keyword in PAIR {
                    assert!(saved.keywords.contains(keyword));
                }
                queries(g);
                recover(g);
            }
            resolve(g, q, |_| {});
            assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 1.into());
            assert_eq!(g.state.rules.players[0].life, 40.into());
            assert_eq!(g.state.rules.players[1].life, 40.into());
            if g.trace.mode == TraceMode::Detailed {
                // Ordinary paid damage goes through the shared transaction,
                // not the legacy direct NoncombatDamageDealt kernel outcome.
                let actual: Vec<_> = batches(g)
                    .into_iter()
                    .flat_map(|b| &b.occurrences)
                    .filter_map(|o| match &o.actual {
                        EventOutcome::DamageCommitted {
                            actual,
                            life_gains,
                            cancelled,
                            ..
                        } => {
                            assert!(life_gains.is_empty());
                            assert!(cancelled.is_empty());
                            Some(actual.as_slice())
                        }
                        _ => None,
                    })
                    .flatten()
                    .collect();
                assert_eq!(actual.len(), 1, "one actual paid damage transaction entry");
                let actual = actual[0];
                assert_eq!(actual.source.object, source);
                assert_eq!(actual.source.characteristics.kind, CardKind::Enchantment);
                assert_eq!(actual.source.zone, Zone::Battlefield);
                assert_eq!(actual.controller, PlayerId(0));
                assert_eq!(
                    actual.recipient,
                    DamageRecipient::Creature {
                        object: object(g, 3)
                    }
                );
                assert_eq!(actual.amount, 1.into());
                for keyword in PAIR {
                    assert!(actual.source.keywords.contains(keyword));
                }
            }
            queries(g);
            recover(g);
        });
    }
}
