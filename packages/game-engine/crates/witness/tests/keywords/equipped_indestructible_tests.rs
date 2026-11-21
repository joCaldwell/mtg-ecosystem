//! Private source-backed configurations. No pool or consumer admission.
//! CR 702.12a-c,701.8a-b,704.5f-h,701.21a,120.3,514.2,614.17c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_witness::equipped::*;
fn protected(g: &Game, id: u32) -> bool {
    g.has_keyword(object(g, id), KeywordAbility::Indestructible)
}
#[test]
fn equipped_indestructible_paid_plate_reequip_lethal_loss_public_and_replay() {
    both(
        prepared(
            &[
                ("Darksteel Plate", true),
                ("Grizzly Bears", false),
                ("Grizzly Bears", false),
                ("Lightning Bolt", true),
                ("Shatter", true),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
            ],
            false,
        ),
        |g, q| {
            cast(g, q, 0, vec![]);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                3
            );
            equip(g, q, 0, 1);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                5
            );
            assert!(protected(g, 1));
            assert!(!protected(g, 2));
            let QueryResult::Complete(QueryValue::Observation(view)) =
                g.observe(PlayerId(0), Budget::unlimited())
            else {
                panic!("public observation")
            };
            let bear_id = &g.definition(CardId(1)).unwrap().oracle_id;
            assert_eq!(
                view.view
                    .objects
                    .iter()
                    .filter(|o| &o.oracle_id == bear_id
                        && o.keywords.contains(&KeywordAbility::Indestructible))
                    .count(),
                1
            );

            assert!(
                g.visible_object(PlayerId(0), g.current(object(g, 1)).unwrap())
                    .unwrap()
                    .unwrap()
                    .keywords
                    .contains(&KeywordAbility::Indestructible)
            );
            cast(
                g,
                q,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 1),
                }],
            );
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            recover(g);
            equip(g, q, 0, 2);
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert!(protected(g, 2));
            cast(
                g,
                q,
                4,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
            assert!(protected(g, 2));
        },
    );
}
#[test]
fn equipped_indestructible_paid_cleansing_common_prior_in_both_physical_orders() {
    for source_first in [false, true] {
        let (source, target, rows) = if source_first {
            (
                0,
                1,
                vec![
                    ("Typed nonintrinsic equipment grant control", false),
                    ("Grizzly Bears", false),
                ],
            )
        } else {
            (
                1,
                0,
                vec![
                    ("Grizzly Bears", false),
                    ("Typed nonintrinsic equipment grant control", false),
                ],
            )
        };
        let mut rows = rows;
        rows.push(("Planar Cleansing", true));
        rows.extend(vec![("Plains", false); 8]);
        both(prepared(&rows, true), |g, q| {
            equip(g, q, source, target);
            assert!(protected(g, target));
            cast(g, q, 2, vec![]);
            assert!(matches!(
                g.current(object(g, source)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
            assert_eq!(
                g.current(object(g, target)).unwrap().zone,
                Zone::Battlefield,
                "source_first={source_first}"
            );
            assert!(!protected(g, target));
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::DestructionPrevented {object:o},..} if *o==object(g,target))));
                for missing in [false, true] {
                    let mut snapshot: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
                    let index = snapshot.trace.records.iter().position(|r| matches!(&r.record,
                        EvidenceRecord::Batch { batch } if batch.occurrences.iter().any(|o| matches!(o.actual, EventOutcome::DestructionPrevented {..})))).unwrap();
                    let EvidenceRecord::Batch { batch } = &mut snapshot.trace.records[index].record
                    else {
                        unreachable!()
                    };
                    let occurrence = batch
                        .occurrences
                        .iter_mut()
                        .find(|o| matches!(o.actual, EventOutcome::DestructionPrevented { .. }))
                        .unwrap();
                    if missing {
                        occurrence.before.objects.clear();
                    } else {
                        occurrence.before.objects[0].keywords.clear();
                    }
                    assert!(
                        Game::restore(g.prepared.as_ref().clone(), &canonical(&snapshot).unwrap())
                            .is_err()
                    );
                }
            }
        });
    }
}
#[test]
fn equipped_indestructible_equip_timing_foreign_and_stale_responses_are_atomic() {
    let mut p = prepared(
        &[
            ("Darksteel Plate", false),
            ("Grizzly Bears", false),
            ("Grizzly Bears", false),
            ("Disperse", true),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
            ("Island", false),
        ],
        false,
    );
    p.config.cards[2].controller = PlayerId(1);
    p.profile_digest = digest(&p.config).unwrap();
    both(p, |g, q| {
        let stale = object(g, 0);
        cast(g, q, 3, vec![TargetEntity::Object { object: stale }]);
        assert!(matches!(
            g.current(object(g, 0)).unwrap().zone,
            Zone::Hand { .. }
        ));
        cast(g, q, 0, vec![]);
        let source = object(g, 0);
        assert_ne!(stale, source);
        assert_eq!(g.current(source).unwrap().zone, Zone::Battlefield);
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: stale,
                    ability: 0
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
        answer(
            g,
            q,
            Response::ActivateAbility {
                object: source,
                ability: 0,
            },
        );
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(matches!(r, Request::Targets { .. }));
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::Targets {
                    objects: vec![object(g, 2)]
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
        pay_and_resolve(
            g,
            q,
            vec![TargetEntity::Object {
                object: object(g, 1),
            }],
        );
        for _ in 0..30 {
            if g.turn().step != TurnStep::PrecombatMain {
                break;
            }
            answer(g, q, Response::Pass);
        }
        let r = request(g, q);
        let snapshot = g.snapshot().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: source,
                    ability: 0
                }
            )
            .is_err()
        );
        assert_eq!(g.snapshot().unwrap(), snapshot);
    });
}
#[test]
fn equipped_indestructible_internal_ordered_destruction_and_atomic_failure() {
    // Direct internal event invariant probe, not an accepted-input replay witness.
    let p = prepared(
        &[
            ("Typed nonintrinsic equipment grant control", false),
            ("Grizzly Bears", false),
            ("Mountain", false),
            ("Mountain", false),
        ],
        true,
    );
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    equip(&mut g, 1, 0, 1);
    let source = object(&g, 0);
    let target = object(&g, 1);
    let before = g.snapshot().unwrap();
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyPermanent { object: source }
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature {
                            object: object(&g, 2)
                        }
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(before, g.snapshot().unwrap());
    assert!(
        g.commit_batch(
            BatchCause::Fixture,
            EventGroup::Simultaneous {
                occurrences: vec![
                    ProposedEvent::DestroyPermanent { object: source },
                    ProposedEvent::DestroyCreature { object: target },
                    ProposedEvent::DestroyCreature {
                        object: object(&g, 2)
                    },
                ]
            }
        )
        .is_err()
    );
    assert_eq!(before, g.snapshot().unwrap());
    let mut probe = g.fork();
    let receipt = probe
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::DestroyCreature { object: target },
            },
        )
        .unwrap();
    let frozen = receipt.occurrences[0].before.clone();
    for variant in 0..3 {
        let mut bad = frozen.clone();
        match variant {
            0 => bad.objects.clear(),
            1 => bad.objects[0].object.incarnation += 1,
            _ => bad.objects[0].zone = Zone::Exile,
        }
        assert!(
            g.apply_occurrence_with_prior(
                &ProposedEvent::DestroyCreature { object: target },
                Some(&bad)
            )
            .is_err()
        );
        assert_eq!(before, g.snapshot().unwrap());
    }
    // Internal authority probe: a grant gained after the frozen view cannot
    // retroactively prevent that destruction; direct calls use current facts.
    let mut no_grant = g.fork();
    no_grant
        .apply_occurrence(&ProposedEvent::Attach {
            object: source,
            target: None,
        })
        .unwrap();
    let mut capture = no_grant.fork();
    let prior = capture
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Occurrence {
                proposed: ProposedEvent::DestroyCreature { object: target },
            },
        )
        .unwrap()
        .occurrences[0]
        .before
        .clone();
    let mut gained = g.fork();
    assert!(matches!(
        gained
            .apply_occurrence_with_prior(
                &ProposedEvent::DestroyCreature { object: target },
                Some(&prior)
            )
            .unwrap(),
        EventOutcome::Moved { .. }
    ));
    assert!(matches!(
        g.apply_occurrence(&ProposedEvent::DestroyCreature { object: target })
            .unwrap(),
        EventOutcome::DestructionPrevented { .. }
    ));
    let receipt = g
        .commit_batch(
            BatchCause::Fixture,
            EventGroup::Ordered {
                children: vec![
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyPermanent { object: source },
                    },
                    EventGroup::Occurrence {
                        proposed: ProposedEvent::DestroyCreature { object: target },
                    },
                ],
            },
        )
        .unwrap();
    assert!(
        !receipt.occurrences[1].before.objects[0]
            .keywords
            .contains(&KeywordAbility::Indestructible)
    );
    assert!(matches!(
        g.current(object(&g, 1)).unwrap().zone,
        Zone::Graveyard { .. }
    ));
}

#[test]
fn equipped_indestructible_paid_exile_removes_grant_before_lethal_sba() {
    both(
        prepared(
            &[
                ("Darksteel Plate", false),
                ("Grizzly Bears", false),
                ("Lightning Bolt", true),
                ("Revoke Existence", true),
                ("Mountain", false),
                ("Mountain", false),
                ("Mountain", false),
                ("Plains", false),
                ("Plains", false),
            ],
            false,
        ),
        |g, q| {
            equip(g, q, 0, 1);
            cast(
                g,
                q,
                2,
                vec![TargetEntity::Object {
                    object: object(g, 1),
                }],
            );
            assert_eq!(g.current(object(g, 1)).unwrap().marked_damage, 3.into());
            cast(
                g,
                q,
                3,
                vec![TargetEntity::Object {
                    object: object(g, 0),
                }],
            );
            assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Exile);
            assert!(matches!(
                g.current(object(g, 1)).unwrap().zone,
                Zone::Graveyard { .. }
            ));
        },
    );
}

#[test]
fn paid_hybrid_colors_are_bound_per_symbol_and_replay() {
    hybrid_payment_witness(
        include_str!("../../../../tests/fixtures/synthetic-glaze-lattice.json"),
        false,
    );
}
#[test]
fn paid_hybrid_controller_grants_begin_only_after_resolution() {
    hybrid_payment_witness(
        include_str!("../../../../tests/fixtures/synthetic-glasshouse-field.json"),
        true,
    );
}
#[test]
fn paid_phyrexian_symbol_debits_life_on_cast() {
    hybrid_payment_witness(
        include_str!("../../../../tests/fixtures/synthetic-ashen-lattice.json"),
        false,
    );
}

#[test]
fn paid_phyrexian_creature_retains_intrinsic_keywords() {
    let mut p = prepared(&[("Darksteel Plate", true), ("Forest", false)], false);
    let compiled = mtg_compiler::compile(
        mtg_compiler::fixtures::MANIFEST,
        include_str!("../../../../tests/fixtures/phyrexian-keywords-vault-skirge.json"),
    );
    let mtg_compiler::CompileResult::Lowered { definition } = compiled else {
        panic!("{compiled:?}");
    };
    assert_eq!(
        definition.keywords,
        [KeywordAbility::Flying, KeywordAbility::Lifelink]
    );
    let mut missing = (*definition).clone();
    missing.keywords.pop();
    missing.capabilities = missing.requirements();
    assert!(missing.validate().is_err());
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    let catalog = Arc::make_mut(&mut p.catalog);
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();

    let mut g = Game::with_trace(p, TraceMode::Detailed);
    answer(&mut g, 1, Response::Cast { card: CardId(0) });
    assert!(matches!(request(&mut g, 1), Request::HybridCost { .. }));
    answer(
        &mut g,
        1,
        Response::HybridCost {
            colors: vec![HybridPayment::PayTwoLife],
        },
    );
    pay_and_resolve(&mut g, 1, vec![]);

    let creature = object(&g, 0);
    assert_eq!(g.zone(creature), Some(Zone::Battlefield));
    assert_eq!(g.state.rules.players[0].life, 38.into());
    assert!(
        g.characteristics(creature)
            .unwrap()
            .has_type(CardKind::Artifact)
    );
    assert!(g.has_keyword(creature, KeywordAbility::Flying));
    assert!(g.has_keyword(creature, KeywordAbility::Lifelink));
    recover(&g);
}

fn hybrid_payment_witness(raw: &str, grants: bool) {
    let mut p = prepared(
        &[
            ("Darksteel Plate", true),
            ("Forest", false),
            ("Plains", false),
        ],
        false,
    );
    let result = mtg_compiler::compile(mtg_compiler::fixtures::MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    assert_eq!(definition.characteristics.printed_mana_value(), 2.into());
    let mut stripped = (*definition).clone();
    stripped.characteristics.hybrid_mana.clear();
    stripped.capabilities = stripped.requirements();
    assert!(stripped.validate().is_err());
    let catalog = Arc::make_mut(&mut p.catalog);
    p.config.cards[0].oracle_id = definition.oracle_id.clone();
    catalog.definitions.push(*definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();
    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 1);
    answer(&mut g, 1, Response::Cast { card: CardId(0) });
    let Request::HybridCost {
        player,
        decision_id,
        symbols,
    } = request(&mut g, 1)
    else {
        panic!("missing hybrid announcement")
    };
    assert_eq!(symbols.len(), 2);
    let phyrexian = symbols[1].colors.contains(&HybridPayment::PayTwoLife);
    let choices = vec![
        HybridPayment::Mana(Color::G),
        if phyrexian {
            HybridPayment::PayTwoLife
        } else {
            HybridPayment::Mana(Color::W)
        },
    ];
    let mut expected_life = g.state.rules.players[player.0 as usize].life.clone();
    let source = g.top().unwrap().source;
    for (actor, colors) in [
        (PlayerId(1), vec![Color::G, Color::W]),
        (player, vec![Color::W]),
        (player, vec![Color::U, Color::G]),
    ] {
        assert!(
            g.respond(
                actor,
                decision_id,
                Response::HybridCost {
                    colors: colors.into_iter().map(HybridPayment::Mana).collect()
                }
            )
            .is_err()
        );
    }
    recover(&g);
    g.respond(
        player,
        decision_id,
        Response::HybridCost {
            colors: choices.clone(),
        },
    )
    .unwrap();
    let Request::Mana { cost, .. } = request(&mut g, 1) else {
        panic!("missing locked payment")
    };
    assert_eq!(
        cost.colored,
        choices.iter().filter_map(|c| c.color()).collect::<Vec<_>>()
    );
    assert_eq!(g.top().unwrap().hybrid_colors, choices);
    assert_eq!(g.state.rules.players[player.0 as usize].life, expected_life);
    recover(&g);
    pay_and_resolve(&mut g, 1, vec![]);
    if phyrexian {
        expected_life.subtract(&2.into());
    }
    assert_eq!(g.state.rules.players[player.0 as usize].life, expected_life);
    let permanent = object(&g, 0);
    assert_ne!(permanent, source);
    if grants {
        assert_eq!(
            g.characteristics(permanent).unwrap().colors,
            [Color::W, Color::U, Color::B, Color::R, Color::G]
        );
        assert_eq!(g.characteristics(object(&g, 1)).unwrap().subtypes.len(), 5);
    }
    assert_eq!(g.zone(permanent), Some(Zone::Battlefield));
    recover(&g);
}

#[test]
fn paid_omnath_first_main_looks_reveals_and_puts_titanic_ultimatum_in_hand() {
    let owner = PlayerId(0);
    let mut p = prepared(
        &[
            ("Darksteel Plate", true),
            ("Plains", false),
            ("Island", false),
            ("Mountain", false),
            ("Forest", false),
        ],
        false,
    );
    // The ordinary draw step takes the first Forest. Omnath looks at the next card.
    let top = p.config.initial_libraries()[&owner][1];
    let catalog = Arc::make_mut(&mut p.catalog);
    for (card, name, manifest, raw) in [
        (
            CardId(0),
            "Omnath, Locus of All",
            include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            include_str!(
                "../../../../tests/fixtures/ramos/26445dc7-8363-4205-aee8-1cafeb4ba4c0.json"
            ),
        ),
        (
            top,
            "Titanic Ultimatum",
            mtg_compiler::fixtures::MANIFEST,
            include_str!("../../../../tests/fixtures/0cd28580-d718-4d0b-96e3-a836c2911c27.json"),
        ),
    ] {
        let result = mtg_compiler::compile(manifest, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert_eq!(definition.name, name);
        p.config.cards[card.0 as usize].oracle_id = definition.oracle_id.clone();
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    p.catalog_digest = digest(catalog).unwrap();
    p.profile_digest = digest(&p.config).unwrap();

    let mut g = Game::with_trace(p, TraceMode::Detailed);
    request(&mut g, 4096);
    let mut life = g.state.rules.players[0].life.clone();
    answer(&mut g, 4096, Response::Cast { card: CardId(0) });
    answer(
        &mut g,
        4096,
        Response::HybridCost {
            colors: vec![HybridPayment::PayTwoLife],
        },
    );
    let Request::Mana { cost, .. } = request(&mut g, 4096) else {
        panic!("locked cast cost")
    };
    assert_eq!(cost.colored, [Color::W, Color::U, Color::R, Color::G]);
    pay_and_resolve(&mut g, 4096, vec![]);
    life.subtract(&2.into());
    assert_eq!(g.state.rules.players[0].life, life);
    assert!((1..=4).all(|id| g.current(object(&g, id)).unwrap().tapped));
    assert_eq!(g.zone(object(&g, 0)), Some(Zone::Battlefield));

    let next_turn = g.state.rules.turn.number + 4;
    loop {
        let r = request(&mut g, 4096);
        assert!(g.state.rules.turn.number <= next_turn);
        if matches!(r, Request::OptionalEffect {player,..} if player == owner) {
            break;
        }
        let response = mtg_witness::temporary::pass(&r);
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    assert_eq!(g.state.rules.turn.number, next_turn);
    assert_eq!(g.state.rules.active, Some(owner));
    assert_eq!(g.state.rules.turn.step, TurnStep::PrecombatMain);
    let looked = g.library_prefix(owner, &1.into()).unwrap()[0];
    assert_eq!(looked.card, top);
    let oracle = g.definition(top).unwrap().oracle_id.clone();
    let sees_card = |game: &Game, viewer| {
        let QueryResult::Complete(QueryValue::Observation(o)) =
            game.observe(viewer, Budget::unlimited())
        else {
            panic!("observation")
        };
        o.view.objects.iter().any(|o| o.oracle_id == oracle)
            || o.view
                .known_searched_hand
                .iter()
                .any(|(player, id)| *player == owner && id == &oracle)
            || o.view
                .known_library_positions
                .iter()
                .any(|p| p.oracle_id == oracle && p.position == 1 && p.library_owner == owner)
    };
    assert!(sees_card(&g, owner));
    assert!(!sees_card(&g, PlayerId(1)));
    recover(&g);
    // Synthetic prior knowledge only. The ordinary privately looked and
    // publicly revealed branches below retain their accepted-input proof.
    let mut already_known = g.fork();
    already_known.state.knowledge.visibility[1]
        .public_tucked_cards
        .insert(looked);
    assert!(sees_card(&already_known, PlayerId(1)));
    answer(
        &mut already_known,
        4096,
        Response::OptionalEffect { perform: false },
    );
    request(&mut already_known, 4096);
    assert_eq!(
        already_known.zone(already_known.object_id(top).unwrap()),
        Some(Zone::Hand { owner })
    );
    assert!(sees_card(&already_known, PlayerId(1)));
    assert!(!sees_card(&already_known, PlayerId(2)));
    let start = g.trace.records.len();
    for reveal in [false, true] {
        let mut branch = g.fork();
        answer(
            &mut branch,
            4096,
            Response::OptionalEffect { perform: reveal },
        );
        if reveal {
            let Request::ManaCombination {
                player,
                total,
                colors,
                ..
            } = request(&mut branch, 4096)
            else {
                panic!("reveal allocation")
            };
            assert_eq!(player, owner);
            assert_eq!(total, 3.into());
            assert_eq!(colors, [Color::W, Color::R, Color::G]);
            assert!(sees_card(&branch, PlayerId(1)));
            recover(&branch);
            answer(
                &mut branch,
                4096,
                Response::ManaCombination {
                    amounts: [2_i64, 0, 0, 1, 0].map(Quantity::from),
                },
            );
        }
        assert!(
            matches!(request(&mut branch,4096), Request::Priority {player,..} if player==owner)
        );
        assert_eq!(
            branch.zone(branch.object_id(top).unwrap()),
            Some(Zone::Hand { owner })
        );
        if !reveal {
            assert!(!sees_card(&branch, PlayerId(1)));
        }
        for (color, n) in [Color::W, Color::U, Color::B, Color::R, Color::G]
            .into_iter()
            .zip([2_i64, 0, 0, 1, 0])
        {
            assert_eq!(
                branch.mana(owner, color),
                Some((if reveal { n } else { 0 }).into())
            );
        }
        let mut reveals = 0;
        for record in branch.trace.records.iter().skip(start) {
            if let EvidenceRecord::Batch { batch } = &record.record {
                assert!(!matches!(batch.cause, BatchCause::DrawStep { .. }));
                for event in &batch.occurrences {
                    assert!(!matches!(event.proposed, ProposedEvent::FailedDraw { .. }));
                    if let EventOutcome::LibraryCardRevealed { player, object } = &event.actual {
                        assert_eq!((*player, *object), (owner, looked));
                        reveals += 1;
                    }
                }
            }
        }
        assert_eq!(reveals, usize::from(reveal));
        recover(&branch);
    }
}
