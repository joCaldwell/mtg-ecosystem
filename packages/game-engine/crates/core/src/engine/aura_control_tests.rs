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
    ] {
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

#[test]
fn paid_aura_control_complete_sources_enter_attached_before_observation() {
    for (name, cost) in [("Mind Control", 5), ("Control Magic", 4)] {
        let mut rows = vec![(name, 0, 0, true), ("Grizzly Bears", 2, 1, false)];
        lands(&mut rows, "Island", 0, cost);
        both(prepared(&rows), |g, q| {
            let creature = object(g, 1);
            announce(g, q, 0, target(g, 1));
            let stack = g.state.rules.stack.last().unwrap().clone();
            assert!(stack.paid.is_some());
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
            own_main(g, q, 1, g.turn().number);
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
        own_main(g, q, 1, g.turn().number);
        cast(g, q, 1, target(g, 2));
        recover(g);
        let wire = g.snapshot().unwrap();
        for variant in 0..4 {
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
                2 => {
                    let a = bad.state.rules.objects.get_mut(&CardId(1)).unwrap();
                    a.attached_to = None;
                    a.attachment_timestamp = None;
                    bad.state
                        .rules
                        .objects
                        .get_mut(&CardId(2))
                        .unwrap()
                        .controller = PlayerId(0);
                }
                _ => bad.state.rules.control_effects.clear(),
            }
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized control forgery {variant}"
            );
        }
    });
}
