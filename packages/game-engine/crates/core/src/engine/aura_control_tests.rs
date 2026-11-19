//! Private complete-source paid scenarios; no pool or consumer admission.
//! Frozen archive d705222b280a71cb; CR20260925 109.5, 302.6, 303.4,
//! 400.7, 506.4, 608.2b, 608.3c, 613.7e, 702.10/11, 800.4a-c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::equipped_indestructible_tests::{answer, both, equip, object, recover, request};
use super::temporary_indestructible_tests::{announce, pass, target, until_player};
use super::*;

const MIND: &str =
    include_str!("../../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json");
const MAGIC: &str =
    include_str!("../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json");
type Row = (&'static str, u8, u8, bool);

fn prepared(rows: &[Row]) -> Prepared {
    prepared_with_sources(rows, &[])
}

fn prepared_with_sources(rows: &[Row], extras: &[&str]) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    // Every added definition is compiled in its matching accepted archive
    // context. Never compare provenance from distinct parser/manifest identities.
    for raw in [
        MIND,
        MAGIC,
        include_str!("../../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"),
        include_str!("../../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        include_str!("../../../../tests/fixtures/1cb419cc-d8e4-4676-987f-089cebccc374.json"),
    ]
    .into_iter()
    .chain(extras.iter().copied())
    {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
            raw,
        ) else {
            panic!("complete frozen source must lower")
        };
        if raw == MIND || raw == MAGIC {
            assert!(mtg_contracts::aura_control_source(&definition));
            assert!(definition.capabilities.contains(&Capability::AuraControlV1));
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "aura-control-private-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.commanders.clear();
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, owner, controller, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*owner),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..12 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}

fn lands(rows: &mut Vec<Row>, name: &'static str, seat: u8, count: usize) {
    rows.extend(vec![(name, seat, seat, false); count]);
}

fn resolve(g: &mut Game, q: u64, mut inspect: impl FnMut(&Game)) {
    for _ in 0..3000 {
        let a = g.advance(Budget::new(q));
        inspect(g);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.rules.stack.is_empty()
                    && g.state.rules.pending_triggers.is_empty()
                {
                    return;
                }
                let response = match &r {
                    Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                        order: candidates.clone(),
                    },
                    Request::ResolveDiscard {
                        candidates, count, ..
                    } => Response::ResolveDiscard {
                        objects: candidates.iter().take(*count).copied().collect(),
                    },
                    _ => pass(&r),
                };
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("paid Aura control resolution {x:?}"),
        }
    }
    panic!("bounded paid resolution")
}

fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce(g, q, id, targets);
    assert!(g.state.rules.stack.last().unwrap().paid.is_some());
    resolve(g, q, |_| {});
}

fn own_main(g: &mut Game, q: u64, seat: u8, after: u64) {
    for _ in 0..2000 {
        let r = request(g, q);
        if g.turn().number > after
            && g.active_player() == Some(PlayerId(seat))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r,Request::Priority {player,..} if player==PlayerId(seat))
        {
            return;
        }
        answer(g, q, pass(&r));
    }
    panic!("actual own main/untap")
}

fn batches(g: &Game) -> Vec<&CommittedBatch> {
    g.trace
        .records
        .iter()
        .filter_map(|r| match &r.record {
            EvidenceRecord::Batch { batch } => Some(batch.as_ref()),
            _ => None,
        })
        .collect()
}

fn control(g: &Game, id: u32, controller: u8, owner: u8, default: u8) {
    let o = g.current(object(g, id)).unwrap();
    assert_eq!(o.controller, PlayerId(controller));
    assert_eq!(o.default_controller, PlayerId(default));
    assert_eq!(g.state.rules.cards[&CardId(id)].owner, PlayerId(owner));
}

fn visible(g: &Game, aura: u32, creature: u32, controller: u8) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    for seat in 0..4 {
        if !g.is_live(PlayerId(seat)) {
            continue;
        }
        let QueryResult::Complete(QueryValue::Observation(view)) =
            g.observe(PlayerId(seat), Budget::unlimited())
        else {
            panic!("public observation")
        };
        let ah = g.handle_for(PlayerId(seat), object(g, aura)).unwrap();
        let th = g.handle_for(PlayerId(seat), object(g, creature)).unwrap();
        let a = view.view.objects.iter().find(|o| o.handle == ah).unwrap();
        let t = view.view.objects.iter().find(|o| o.handle == th).unwrap();
        assert_eq!(a.attached_to, Some(th));
        assert!(t.attachments.contains(&ah));
        assert_eq!(t.controller, PlayerId(controller));
        assert_eq!(t.owner, g.state.rules.cards[&CardId(creature)].owner);
    }
    assert_eq!(g.state, before);
    assert_eq!(g.trace, trace);
}

fn fund(g: &mut Game, q: u64, name: &str, count: usize) {
    for _ in 0..count {
        let r = request(g, q);
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == r.player()
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).unwrap().name == name
            })
            .unwrap()
            .id;
        g.respond(
            r.player(),
            r.decision_id(),
            Response::ActivateMana { object: source },
        )
        .unwrap();
    }
}

fn finish_query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..10000 {
        result = match result {
            QueryResult::Complete(v) => return v,
            QueryResult::Incomplete { work, .. } => g.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(r) => panic!("query rejected {r:?}"),
        };
    }
    panic!("bounded query")
}

#[test]
fn paid_aura_control_same_controller_preserves_combat_real_change_preserves_other_facts() {
    let mut rows = vec![
        ("Mind Control", 0, 0, true),
        ("Control Magic", 0, 0, true),
        ("Charging Monstrosaur", 2, 1, false),
        ("Bonesplitter", 0, 0, false),
        ("Battlegrowth", 0, 0, true),
        ("Lightning Bolt", 0, 0, true),
        ("Naturalize", 0, 0, true),
        ("Naturalize", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 10);
    lands(&mut rows, "Forest", 0, 5);
    lands(&mut rows, "Mountain", 0, 1);
    both(prepared(&rows), |g, q| {
        cast(g, q, 0, target(g, 2));
        assert!(!g.current(object(g, 2)).unwrap().attack_ready);
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        let creature = object(g, 2);
        assert!(g.current(creature).unwrap().attack_ready);
        equip(g, q, 3, 2);
        fund(g, q, "Forest", 1);
        cast(g, q, 4, target(g, 2));
        fund(g, q, "Mountain", 1);
        cast(g, q, 5, target(g, 2));
        let before = g.current(creature).unwrap().clone();
        cast(g, q, 1, target(g, 2));
        assert_eq!(
            g.current(creature).unwrap(),
            &before,
            "same controller preserves readiness and marks"
        );
        for _ in 0..100 {
            let r = request(g, q);
            if let Request::Attackers { ref candidates, .. } = r {
                assert!(candidates.contains(&creature));
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(creature, PlayerId(1))],
                    },
                );
                break;
            }
            answer(g, q, pass(&r));
        }
        until_player(g, q, PlayerId(0));
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == creature));
        let before = g.current(creature).unwrap().clone();
        fund(g, q, "Forest", 2);
        cast(g, q, 6, target(g, 1));
        assert_eq!(g.current(creature).unwrap(), &before);
        assert!(g.state.rules.combat.iter().any(|a| a.attacker == creature));
        recover(g);
        fund(g, q, "Forest", 2);
        cast(g, q, 7, target(g, 0));
        control(g, 2, 1, 2, 1);
        let after = g.current(creature).unwrap();
        assert!(!after.attack_ready);
        assert!(after.tapped);
        assert_eq!(after.marked_damage, 3.into());
        assert_eq!(after.plus_one_counters, 1.into());
        assert!(
            !g.state
                .rules
                .combat
                .iter()
                .any(|a| a.attacker == creature || a.blockers.contains(&creature))
        );
        assert_eq!(g.current(object(g, 3)).unwrap().attached_to, Some(creature));
        assert_eq!(g.current(object(g, 3)).unwrap().controller, PlayerId(0));
    });
}

#[test]
fn paid_aura_control_source_and_target_bounce_never_follow_successor() {
    let mut rows = vec![
        ("Mind Control", 0, 0, true),
        ("Grizzly Bears", 0, 1, false),
        ("Disperse", 0, 0, true),
        ("Unsummon", 0, 0, true),
    ];
    lands(&mut rows, "Island", 0, 15);
    lands(&mut rows, "Forest", 0, 2);
    both(prepared(&rows), |g, q| {
        cast(g, q, 0, target(g, 1));
        let aura = object(g, 0);
        let creature = object(g, 1);
        cast(g, q, 2, target(g, 0));
        control(g, 1, 1, 0, 1);
        assert!(g.current(aura).is_err());
        assert_eq!(
            g.current(object(g, 0)).unwrap().zone,
            Zone::Hand { owner: PlayerId(0) }
        );
        assert_eq!(g.current(object(g, 0)).unwrap().attachment_timestamp, None);
        cast(g, q, 0, target(g, 1));
        assert_eq!(
            g.current(object(g, 0)).unwrap().attachment_timestamp,
            Some(2)
        );
        let second_aura = object(g, 0);
        assert_ne!(second_aura, aura);
        announce(g, q, 3, target(g, 1));
        let mut paused = false;
        resolve(g, q, |g| {
            if !paused
                && g.current(creature).is_err()
                && g.current(second_aura)
                    .is_ok_and(|o| o.zone == Zone::Battlefield)
            {
                paused = true;
                recover(g);
                assert_eq!(g.current(second_aura).unwrap().attached_to, Some(creature));
                assert_ne!(
                    g.current(second_aura).unwrap().attached_to,
                    Some(object(g, 1))
                );
            }
        });
        if q == 1 {
            assert!(paused);
        }
        assert_eq!(
            g.current(object(g, 0)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        let successor = g.current(object(g, 1)).unwrap();
        assert_eq!(successor.zone, Zone::Hand { owner: PlayerId(0) });
        fund(g, q, "Forest", 2);
        cast(g, q, 1, vec![]);
        assert_ne!(object(g, 1), creature);
        control(g, 1, 0, 0, 0);
        assert_eq!(g.current(object(g, 0)).unwrap().attached_to, None);
        assert_eq!(g.state.rules.next_attachment_timestamp, 3);
    });
}

#[test]
fn paid_aura_control_dynamic_controlled_targets_hexproof_and_stale_query_receipts() {
    // Both branches remove a real paid control source in response. Masterful
    // Flourish rechecks "you control"; Dive Down resolves first and its existing
    // Hexproof follows current control when the paid source subsequently leaves.
    for protection in ["Masterful Flourish", "Dive Down"] {
        let mut rows = vec![
            ("Mind Control", 0, 0, true),
            ("Grizzly Bears", 2, 1, false),
            (protection, 0, 0, true),
            ("Disperse", 1, 1, true),
            ("Unsummon", 0, 0, true),
            ("Unsummon", 1, 1, true),
        ];
        lands(&mut rows, "Island", 0, 7);
        lands(&mut rows, "Swamp", 0, 1);
        lands(&mut rows, "Island", 1, 4);
        both(prepared(&rows), |g, q| {
            cast(g, q, 0, target(g, 1));
            control(g, 1, 0, 2, 1);
            let creature = object(g, 1);
            answer(g, q, Response::Cast { card: CardId(2) });
            let r = request(g, q);
            assert!(matches!(&r,Request::Targets {candidates,..} if candidates==&vec![creature]));
            let before = g.state.clone();
            let trace = g.trace.clone();
            let QueryValue::Decision(Some(PlayerRequest::Targets {
                token, candidates, ..
            })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
            else {
                panic!("current-controller target query")
            };
            assert_eq!(candidates.len(), 1);
            for seat in 1..4 {
                assert!(matches!(
                    finish_query(g, g.player_decision_for(PlayerId(seat), Budget::new(0))),
                    QueryValue::Decision(None)
                ));
            }
            let QueryResult::Incomplete { work: stale, .. } =
                g.observe(PlayerId(0), Budget::new(0))
            else {
                panic!("suspended observation")
            };
            let QueryValue::Validated(receipt) = finish_query(
                g,
                g.check_player_response(
                    PlayerId(0),
                    token.clone(),
                    PlayerResponse::Targets {
                        objects: candidates.clone(),
                    },
                    Budget::new(0),
                ),
            ) else {
                panic!("target validation receipt")
            };
            let QueryValue::Validated(duplicate) = finish_query(
                g,
                g.check_player_response(
                    PlayerId(0),
                    token,
                    PlayerResponse::Targets {
                        objects: candidates,
                    },
                    Budget::new(0),
                ),
            ) else {
                panic!("second receipt before change")
            };
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
            g.submit(receipt).unwrap();
            if protection == "Masterful Flourish" {
                // The spell is now in its actual mana window.
                let r = request(g, q);
                assert!(matches!(r, Request::Mana { .. }));
                let land = g
                    .state
                    .rules
                    .objects
                    .values()
                    .find(|o| {
                        o.zone == Zone::Battlefield
                            && !o.tapped
                            && g.definition(o.id.card).unwrap().name == "Swamp"
                    })
                    .unwrap()
                    .id;
                answer(g, q, Response::ActivateMana { object: land });
            }
            // The existing payment helper completes the accepted target binding.
            // Stop at priority rather than resolving the pending spell here.
            loop {
                let r = request(g, q);
                match r {
                    Request::Mana { .. } => {
                        if g.payment_pool(
                            r.player(),
                            match &r {
                                Request::Mana { cost, .. } => cost,
                                _ => unreachable!(),
                            },
                        )
                        .is_ok()
                        {
                            answer(g, q, Response::FinishMana { undo_mana: false });
                        } else {
                            fund(g, q, "Island", 1);
                        }
                    }
                    Request::SpendMana { cost, .. } => {
                        assert_eq!(cost.generic, Quantity::zero());
                        assert_eq!(cost.colored.len(), 1);
                        answer(
                            g,
                            q,
                            Response::SpendMana {
                                spending: BTreeMap::from([(cost.colored[0], 1.into())]),
                            },
                        );
                    }
                    Request::Priority { .. } => break,
                    x => panic!("one-symbol paid protection {x:?}"),
                }
            }
            let protection_stack = g.state.rules.stack.last().unwrap().id;
            assert!(g.state.rules.stack.last().unwrap().paid.is_some());
            if protection == "Dive Down" {
                resolve(g, q, |_| {});
                assert!(g.has_keyword(creature, KeywordAbility::Hexproof));
                assert!(g.targetable_by(creature, PlayerId(0)));
                assert!(!g.targetable_by(creature, PlayerId(1)));
                let mut opposing = g.fork();
                until_player(&mut opposing, q, PlayerId(1));
                let r = request(&mut opposing, q);
                let before = opposing.state.clone();
                let trace = opposing.trace.clone();
                // There is no other creature to target: the ordinary casting
                // guard must reject this opponent's actual source proposal.
                assert!(
                    opposing
                        .respond(
                            r.player(),
                            r.decision_id(),
                            Response::Cast { card: CardId(5) }
                        )
                        .is_err()
                );
                assert_eq!(opposing.state, before);
                assert_eq!(opposing.trace, trace);
                announce(g, q, 4, target(g, 1));
                let bounce = g.state.rules.stack.last().unwrap().id;
                until_player(g, q, PlayerId(1));
                cast(g, q, 3, target(g, 0));
                control(g, 1, 1, 2, 1);
                assert!(g.targetable_by(creature, PlayerId(1)));
                assert!(!g.targetable_by(creature, PlayerId(0)));
                assert_eq!(object(g, 1), creature);
                if g.trace.mode == TraceMode::Detailed {
                    assert!(g.events().iter().any(|e| matches!(e,Event::SpellCompleted {stack_id,resolved:false} if *stack_id==bounce)));
                }
                until_player(g, q, PlayerId(1));
                cast(g, q, 5, target(g, 1)); // Actual current controller can target it.
                assert!(g.current(creature).is_err());
            } else {
                until_player(g, q, PlayerId(1));
                cast(g, q, 3, target(g, 0));
                control(g, 1, 1, 2, 1);
                assert_eq!(object(g, 1), creature);
                assert!(!g.has_keyword(creature, KeywordAbility::Indestructible));
                assert_eq!(g.characteristics(creature).unwrap().power, Some(2.into()));
                if g.trace.mode == TraceMode::Detailed {
                    assert!(g.events().iter().any(|e| matches!(e,Event::SpellCompleted {stack_id,resolved:false} if *stack_id==protection_stack)));
                }
            }
            let before = g.state.clone();
            let trace = g.trace.clone();
            assert!(g.submit(duplicate).is_err());
            assert!(matches!(
                g.resume_work(stale, Budget::unlimited()),
                QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
            ));
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        });
    }
}

#[test]
fn paid_aura_control_sickness_haste_tap_authority_and_frozen_invocation_controller() {
    let mut rows = vec![
        ("Mind Control", 0, 0, true),
        ("Control Magic", 0, 0, true),
        ("Control Magic", 0, 0, true),
        ("Merfolk Looter", 2, 1, false),
        ("Llanowar Elves", 2, 1, false),
        ("Raging Goblin", 2, 1, false),
        ("Disperse", 1, 1, true),
    ];
    lands(&mut rows, "Island", 0, 13);
    lands(&mut rows, "Island", 1, 2);
    both(prepared(&rows), |g, q| {
        for (a, t) in [(0, 3), (1, 4), (2, 5)] {
            cast(g, q, a, target(g, t));
            control(g, t, 0, 2, 1);
        }
        for id in [3, 4, 5] {
            assert!(!g.current(object(g, id)).unwrap().attack_ready);
        }
        let r = request(g, q);
        let before = g.state.clone();
        let trace = g.trace.clone();
        for response in [
            Response::ActivateAbility {
                object: object(g, 3),
                ability: 0,
            },
            Response::ActivateMana {
                object: object(g, 4),
            },
        ] {
            assert!(g.respond(r.player(), r.decision_id(), response).is_err());
            assert_eq!(g.state, before);
            assert_eq!(g.trace, trace);
        }
        assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 3)));
        assert!(!g.creatures(PlayerId(0), true).contains(&object(g, 4)));
        assert!(
            g.creatures(PlayerId(0), true).contains(&object(g, 5)),
            "exact intrinsic Haste overrides new control sickness"
        );
        assert!(
            g.validate_permanent_mana_activation(PlayerId(1), object(g, 4))
                .is_err()
        );
        // Exercise the real declaration boundary too: Haste permits this
        // stolen creature immediately, while the other two remain ineligible.
        let goblin = object(g, 5);
        let mut declared = false;
        for _ in 0..100 {
            let r = request(g, q);
            if let Request::Attackers { candidates, .. } = &r {
                assert!(candidates.contains(&goblin));
                assert!(!candidates.contains(&object(g, 3)));
                assert!(!candidates.contains(&object(g, 4)));
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(goblin, PlayerId(1))],
                    },
                );
                declared = true;
                break;
            }
            answer(g, q, pass(&r));
        }
        assert!(declared);
        let turn = g.turn().number;
        own_main(g, q, 0, turn);
        assert_eq!(g.state.rules.players[1].life, 39.into());
        let looter = object(g, 3);
        let elf = object(g, 4);
        assert!(g.current(looter).unwrap().attack_ready);
        assert!(g.current(elf).unwrap().attack_ready);
        assert!(
            g.validate_permanent_mana_activation(PlayerId(1), elf)
                .is_err()
        );
        answer(g, q, Response::ActivateMana { object: elf });
        request(g, q);
        assert!(g.current(elf).unwrap().tapped);
        assert_eq!(
            g.state.rules.players[0].mana.get(&Color::G),
            Some(&1.into())
        );
        answer(
            g,
            q,
            Response::ActivateAbility {
                object: looter,
                ability: 0,
            },
        );
        loop {
            let r = request(g, q);
            match r {
                Request::Mana { .. } => answer(g, q, Response::FinishMana { undo_mana: false }),
                Request::Priority { .. } => break,
                x => panic!("paid zero-mana tap ability {x:?}"),
            }
        }
        let invocation = g.state.rules.stack.last().unwrap().clone();
        assert!(invocation.paid.is_some());
        assert_eq!(invocation.controller, PlayerId(0));
        assert!(g.current(looter).unwrap().tapped);
        recover(g);
        let library = |g: &Game, seat: u8| {
            g.state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.zone
                        == (Zone::Library {
                            owner: PlayerId(seat),
                        })
                })
                .count()
        };
        let old0 = library(g, 0);
        let old1 = library(g, 1);
        until_player(g, q, PlayerId(1));
        announce(g, q, 6, target(g, 0));
        let mut changed_before_resolution = false;
        resolve(g, q, |g| {
            if !changed_before_resolution
                && g.current(looter).unwrap().controller == PlayerId(1)
                && g.state.rules.stack.iter().any(|s| s.id == invocation.id)
            {
                changed_before_resolution = true;
                let frozen = g
                    .state
                    .rules
                    .stack
                    .iter()
                    .find(|s| s.id == invocation.id)
                    .unwrap();
                assert_eq!(frozen.controller, PlayerId(0));
                assert_eq!(frozen.source, looter);
                recover(g);
            }
        });
        if q == 1 {
            assert!(changed_before_resolution);
        }
        control(g, 3, 1, 2, 1);
        assert_eq!(library(g, 0), old0 - 1);
        assert_eq!(library(g, 1), old1);
        assert!(g.current(looter).unwrap().tapped);
        assert!(!g.current(looter).unwrap().attack_ready);
    });
}

#[test]
fn paid_aura_control_departures_simultaneous_losses_and_common_prior_wipe() {
    for scenario in [
        "owner",
        "aura_owner",
        "dead_default",
        "losses",
        "wipe_source_first",
        "wipe_target_first",
    ] {
        let mut rows = vec![
            ("Mind Control", 0, 0, true),
            ("Control Magic", 1, 1, true),
            ("Grizzly Bears", 3, 2, false),
            ("Disperse", 0, 0, true),
            ("Planar Cleansing", 0, 0, true),
            ("Earthquake", 0, 0, true),
        ];
        let (older, creature) = if scenario == "wipe_target_first" {
            rows.swap(0, 2);
            (2, 0)
        } else {
            (0, 2)
        };
        lands(&mut rows, "Island", 0, 8);
        lands(&mut rows, "Island", 1, 4);
        lands(&mut rows, "Plains", 0, 6);
        lands(&mut rows, "Mountain", 0, 2);
        let mut p = prepared(&rows);
        if scenario == "losses" {
            p.config.initial.life = [40.into(), 1.into(), 1.into(), 40.into()];
            p.profile_digest = digest(&p.config).unwrap();
        }
        both(p, |g, q| {
            cast(g, q, older, target(g, creature));
            let turn = g.turn().number;
            own_main(g, q, 1, turn);
            cast(g, q, 1, target(g, creature));
            let target_object = object(g, creature);
            control(g, creature, 1, 3, 2);
            recover(g);
            match scenario {
                "owner" => {
                    g.concede(PlayerId(3)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(creature)).is_none());
                    for aura in [older, 1] {
                        assert_eq!(
                            g.current(object(g, aura)).unwrap().zone,
                            Zone::Graveyard {
                                owner: if aura == 1 { PlayerId(1) } else { PlayerId(0) }
                            }
                        );
                    }
                }
                "aura_owner" => {
                    g.concede(PlayerId(1)).unwrap();
                    request(g, q);
                    assert!(g.object_id(CardId(1)).is_none());
                    control(g, creature, 0, 3, 2);
                    g.concede(PlayerId(0)).unwrap();
                    request(g, q);
                    control(g, creature, 2, 3, 2);
                }
                "dead_default" => {
                    g.concede(PlayerId(2)).unwrap();
                    request(g, q);
                    control(g, creature, 1, 3, 2);
                    g.concede(PlayerId(1)).unwrap();
                    request(g, q);
                    control(g, creature, 0, 3, 2);
                    until_player(g, q, PlayerId(0));
                    cast(g, q, 3, target(g, older));
                    assert!(g.current(target_object).is_err());
                    assert_eq!(g.current(object(g, creature)).unwrap().zone, Zone::Exile);
                }
                "losses" => {
                    let turn = g.turn().number;
                    own_main(g, q, 0, turn);
                    fund(g, q, "Mountain", 2);
                    answer(g, q, Response::Cast { card: CardId(5) });
                    answer(g, q, Response::AnnounceX { value: 1.into() });
                    super::equipped_indestructible_tests::pay_and_resolve(g, q, vec![]);
                    assert!(!g.is_live(PlayerId(1)));
                    assert!(!g.is_live(PlayerId(2)));
                    control(g, creature, 0, 3, 2);
                    assert_eq!(g.current(target_object).unwrap().marked_damage, 1.into());
                    if g.trace.mode == TraceMode::Detailed {
                        assert!(g.events().iter().any(|e| matches!(e,Event::PlayersLost {losses} if losses.iter().map(|(p,_)|*p).collect::<Vec<_>>()==vec![PlayerId(1),PlayerId(2)])));
                    }
                    cast(g, q, 3, target(g, older));
                    assert_eq!(g.current(object(g, creature)).unwrap().zone, Zone::Exile);
                }
                _ => {
                    let turn = g.turn().number;
                    own_main(g, q, 0, turn);
                    fund(g, q, "Plains", 6);
                    let count = g.events().len();
                    cast(g, q, 4, vec![]);
                    assert!(g.current(target_object).is_err());
                    for (id, owner) in [(older, 0), (1, 1), (creature, 3)] {
                        assert_eq!(
                            g.current(object(g, id)).unwrap().zone,
                            Zone::Graveyard {
                                owner: PlayerId(owner)
                            }
                        );
                    }
                    if g.trace.mode == TraceMode::Detailed {
                        assert!(!g.events().iter().skip(count).any(|e| matches!(e,Event::ControlChanged {object,..} if *object==target_object)),"no transient survivor control during one wipe");
                        let wipe=batches(g).into_iter().find(|b|b.occurrences.iter().any(|o|matches!(&o.actual,EventOutcome::Moved {movement} if movement.previous==target_object))).unwrap();
                        assert!(wipe.occurrences.len() >= 3);
                        for occurrence in &wipe.occurrences {
                            let authority = occurrence
                                .before
                                .control
                                .as_ref()
                                .expect("common-prior Aura authority");
                            let target_fact = authority
                                .objects
                                .iter()
                                .find(|o| o.object == target_object)
                                .unwrap();
                            assert_eq!(target_fact.controller, PlayerId(1));
                            if let Some(fact) = occurrence
                                .before
                                .objects
                                .iter()
                                .find(|o| o.object == target_object)
                            {
                                assert_eq!(fact.controller, PlayerId(1));
                            }
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn paid_aura_control_complete_sources_enter_attached_before_observation() {
    for (name, cost) in [("Mind Control", 5), ("Control Magic", 4)] {
        let mut rows = vec![(name, 0, 0, true), ("Grizzly Bears", 2, 1, false)];
        lands(&mut rows, "Island", 0, cost);
        both(prepared(&rows), |g, q| {
            let creature = object(g, 1);
            let mut proposal = g.fork();
            answer(&mut proposal, q, Response::Cast { card: CardId(0) });
            let r = request(&mut proposal, q);
            assert!(matches!(r, Request::Targets { .. }));
            let before = proposal.state.clone();
            let trace = proposal.trace.clone();
            for objects in [
                vec![],
                vec![object(g, 2)],
                vec![ObjectId {
                    incarnation: creature.incarnation + 1,
                    ..creature
                }],
            ] {
                assert!(
                    proposal
                        .respond(r.player(), r.decision_id(), Response::Targets { objects })
                        .is_err()
                );
                assert_eq!(proposal.state, before);
                assert_eq!(proposal.trace, trace);
            }
            announce(g, q, 0, target(g, 1));
            let stack = g.state.rules.stack.last().unwrap().clone();
            assert!(stack.paid.is_some());
            assert_eq!(
                stack.paid.as_ref().unwrap().colored,
                vec![Color::U, Color::U]
            );
            assert_eq!(
                stack.paid.as_ref().unwrap().generic,
                Quantity::from((cost - 2) as i64)
            );
            recover(g);
            let mut entered = false;
            resolve(g, q, |g| {
                if g.current(object(g, 0)).unwrap().zone == Zone::Battlefield {
                    entered = true;
                    assert_eq!(g.current(object(g, 0)).unwrap().attached_to, Some(creature));
                    control(g, 1, 0, 2, 1);
                    visible(g, 0, 1, 0);
                }
            });
            assert!(entered);
            assert_eq!(g.state.rules.next_attachment_timestamp, 2);
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                cost
            );
            assert!(!g.current(creature).unwrap().attack_ready);
            if g.trace.mode == TraceMode::Detailed {
                let entries:Vec<_>=batches(g).into_iter().filter(|b| matches!(b.cause,BatchCause::SpellCompletion {stack_id,..} if stack_id==stack.id)).collect();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].occurrences.len(), 1);
                let occurrence = &entries[0].occurrences[0];
                let EventOutcome::EnteredAttached {
                    movement,
                    target,
                    attachment_timestamp,
                } = &occurrence.actual
                else {
                    panic!("one exact attached-entry successor")
                };
                assert_eq!(movement.previous, stack.source);
                assert_eq!(movement.successor, object(g, 0));
                assert_eq!(*target, creature);
                assert_eq!(*attachment_timestamp, 1);
                assert_eq!(movement.from, Zone::Stack);
                assert_eq!(movement.destination, Zone::Battlefield);
                assert!(g.events().iter().any(|e| matches!(e,Event::ControlChanged {object,previous,controller} if *object==creature && *previous==PlayerId(1) && *controller==PlayerId(0))));
            }
        });
    }
}

#[test]
fn paid_aura_control_chronology_beats_physical_order_and_reveals_older() {
    for first in [0, 1] {
        let second = 1 - first;
        let mut rows = vec![
            ("Mind Control", 0, 0, true),
            ("Control Magic", 1, 1, true),
            ("Grizzly Bears", 3, 2, false),
            ("Disperse", 1, 1, true),
            ("Disperse", 1, 1, true),
        ];
        // Whichever physical ID enters first belongs to the first player.
        rows[first as usize].1 = 0;
        rows[first as usize].2 = 0;
        rows[second as usize].1 = 1;
        rows[second as usize].2 = 1;
        lands(&mut rows, "Island", 0, 6);
        lands(&mut rows, "Island", 1, 10);
        both(prepared(&rows), |g, q| {
            cast(g, q, first, target(g, 2));
            control(g, 2, 0, 3, 2);
            let turn = g.turn().number;
            own_main(g, q, 1, turn);
            cast(g, q, second, target(g, 2));
            control(g, 2, 1, 3, 2);
            assert_eq!(
                g.current(object(g, first)).unwrap().attachment_timestamp,
                Some(1)
            );
            assert_eq!(
                g.current(object(g, second)).unwrap().attachment_timestamp,
                Some(2)
            );
            recover(g);
            cast(g, q, 3, target(g, second));
            control(g, 2, 0, 3, 2);
            assert_eq!(
                g.current(object(g, first)).unwrap().attached_to,
                Some(object(g, 2))
            );
            cast(g, q, 4, target(g, first));
            control(g, 2, 2, 3, 2);
            assert_eq!(g.state.rules.next_attachment_timestamp, 3);
        });
    }
}

#[test]
fn paid_aura_control_snapshot_rejects_coherent_timestamp_controller_and_provenance_forgery() {
    let mut rows = vec![
        ("Mind Control", 0, 0, true),
        ("Control Magic", 1, 1, true),
        ("Grizzly Bears", 3, 2, false),
    ];
    lands(&mut rows, "Island", 0, 5);
    lands(&mut rows, "Island", 1, 4);
    both(prepared(&rows), |g, q| {
        cast(g, q, 0, target(g, 2));
        recover(g);
        let turn = g.turn().number;
        own_main(g, q, 1, turn);
        cast(g, q, 1, target(g, 2));
        recover(g);
        let wire = g.snapshot().unwrap();
        for variant in 0..3 {
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            match variant {
                0 => {
                    // Keep local derived controller coherent with the forged order.
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(0))
                        .unwrap()
                        .attachment_timestamp = Some(2);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(1))
                        .unwrap()
                        .attachment_timestamp = Some(1);
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(0);
                }
                1 => {
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(2)
                }
                _ => {
                    let a = bad.state.rules.objects.get_mut(&CardId(1)).unwrap();
                    a.attached_to = None;
                    a.attachment_timestamp = None;
                    a.zone = Zone::Graveyard { owner: PlayerId(1) };
                    a.id.incarnation += 1;
                    a.attack_ready = false;
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(0);
                }
            }
            if variant == 0 {
                let mut structural = g.fork();
                structural.state = bad.state.clone();
                structural.validate_state().unwrap(); // Locally coherent, historically false.
            }
            if variant == 1 {
                let mut structural = g.fork();
                structural.state = bad.state.clone();
                assert!(structural.validate_control_state().is_err());
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized control forgery {variant}"
            );
        }
    });
}

#[cfg(test)]
#[path = "composed_aura_control_tests.rs"]
mod composed_tests;

#[cfg(test)]
#[path = "atomic_targeted_keyword_tests.rs"]
mod atomic_targeted_keyword_tests;

#[cfg(test)]
#[path = "permanent_set_keyword_tests.rs"]
mod permanent_set_keyword_tests;

#[cfg(test)]
#[path = "typed_aura_control_tests.rs"]
mod typed_aura_control_tests;

#[cfg(test)]
#[path = "general_aura_control_tests.rs"]
mod general_aura_control_tests;

#[cfg(test)]
#[path = "targeted_permanent_keyword_tests.rs"]
mod targeted_permanent_keyword_tests;

#[cfg(test)]
#[path = "controller_scry_paid_tests.rs"]
mod controller_scry_paid_tests;

#[path = "controller_surveil_tests.rs"]
mod controller_surveil_tests;

#[cfg(test)]
#[path = "fixed_mill_tests.rs"]
mod fixed_mill_tests;

#[cfg(test)]
#[path = "library_selection_tests.rs"]
mod library_selection_tests;

#[cfg(test)]
#[path = "library_order_tests.rs"]
mod library_order_tests;

#[cfg(test)]
#[path = "hand_return_tests.rs"]
mod hand_return_tests;

#[cfg(test)]
#[path = "draw_discard_tests.rs"]
mod draw_discard_tests;

#[path = "controller_counted_draw_tests.rs"]
mod controller_counted_draw_tests;

#[path = "announced_x_draw_tests.rs"]
mod announced_x_draw_tests;
#[path = "targeted_counted_draw_tests.rs"]
mod targeted_counted_draw_tests;

#[cfg(test)]
#[path = "temporary_reach_tests.rs"]
mod temporary_reach_tests;

#[path = "temporary_vigilance_tests.rs"]
mod temporary_vigilance_tests;

#[path = "temporary_double_strike_tests.rs"]
mod temporary_double_strike_tests;

#[path = "activated_artifact_draw_tests.rs"]
mod activated_artifact_draw_tests;
#[path = "targeted_discard_tests.rs"]
mod targeted_discard_tests;
