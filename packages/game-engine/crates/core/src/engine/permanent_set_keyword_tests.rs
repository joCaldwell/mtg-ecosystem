//! Paid complete-source Heroic Intervention witnesses, private scenarios only.
//! Archive d705222b280a71cb, physical line 5491 / source index 5489;
//! raw record d3f0636f6881fb3a644f9a76bcf4c3cc03af1224d71cfe1e83b693fb4158369b.
//! CR20260925 109.5, 400.7, 608.2b/h, 611.2c, 702.11/12, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! New native tests UNRUN here; no pool/profile/policy/consumer admission.
use super::super::temporary_indestructible_tests::through_cleanup;
use super::*;

const HEROIC: &str =
    include_str!("../../../../tests/fixtures/24882fa2-3fe9-4c1b-aa3d-0e6488b9db27.json");
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
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == player
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
            })
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
