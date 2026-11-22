//! Private paid complete-source witnesses; no catalog/profile/consumer admission.
//! Tamiyo's Safekeeping: archive d705222b280a71cb, physical 28168/index 28166;
//! record 914e134284083b1540c71d66f115424264560de04ee18674b39f0270daa836cd.
//! Rules SHA256 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca.
//! CR20260925 109.5, 205.2b, 400.7, 608.2b/h, 611.2c, 702.11/12, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! All new Rust tests/compilation UNRUN here; parent owns native qualification.
use super::*;
use mtg_witness::temporary::through_cleanup;

const SAFEKEEPING: &str =
    include_str!("../../../../../tests/fixtures/bb2b324c-970a-4920-884e-c92ba49669f0.json");
const CONFISCATE: &str =
    include_str!("../../../../../tests/fixtures/d599a38c-7719-443e-bb59-55bd43a8fee6.json");
const SILVER_MYR: &str =
    include_str!("../../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json");
const SEAT: &str =
    include_str!("../../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json");
const PAIR: &[KeywordAbility] = &[KeywordAbility::Hexproof, KeywordAbility::Indestructible];

fn prepared(rows: &[Row]) -> Prepared {
    // The inherited helper already adds Mind Control, Control Magic, Disperse,
    // Dive Down and Masterful Flourish. Include each new definition exactly once.
    let p = prepared_with_sources(rows, &[SAFEKEEPING, CONFISCATE, SILVER_MYR, SEAT]);
    let d = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Tamiyo's Safekeeping")
        .unwrap();
    assert_eq!(d.characteristics.kind, CardKind::Instant);
    assert_eq!(d.characteristics.mana_cost.generic, 0.into());
    assert_eq!(d.characteristics.mana_cost.colored, [Color::G]);
    assert_eq!(d.provenance.source_index, 28166);
    assert_eq!(
        d.provenance.source_sha256,
        "48e40697b4ba35ab71ae951b900f756eeeb6b0ab5d3a9e4ca6434326474e0a78"
    );
    assert_eq!(
        d.provenance.ast_sha256,
        "050500699479157305337569bca1c1d753142907cc8ba254403d7ecc75053b86"
    );
    assert_eq!(
        d.provenance.binding_sha256,
        "2ecd1544ded73bedd155b56966a2e3d3886b3c83288e58f5081a5c0ae85a5ec6"
    );
    let program = d.spell.as_ref().unwrap();
    let descriptor = mtg_contracts::temporary_permanent_keyword_program(program).unwrap();
    assert!(descriptor.modifier.is_none());
    assert_eq!(descriptor.keywords, PAIR);
    assert!(descriptor.controller_life.is_some());
    assert!(mtg_contracts::temporary_targeted_keyword_program(program).is_some());
    assert!(mtg_contracts::temporary_creature_keyword_program(program).is_none());
    assert_eq!(program.targets.len(), 1);
    assert_eq!(
        program.targets[0].domain,
        TargetDomain::ControlledBattlefieldPermanent
    );
    assert_eq!(program.targets[0].slot, 0);
    assert_eq!(program.targets[0].selection_id, "line:0:0");
    assert_eq!(
        program.targets[0].filter,
        Some(PermanentFilter {
            types: vec![],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
            power: None,
        })
    );
    assert_eq!(program.instructions.len(), 2, "reminder adds no semantics");
    assert!(program.instructions.iter().all(|i| i.source_block() == 0));
    assert!(
        d.capabilities
            .contains(&Capability::TemporaryPermanentKeywordProgramV1)
    );
    p
}

// Float the whole printed cost through actual mana activations. Prefer the
// printed color for generic funding too, keeping later colored spells payable.
fn prefund(g: &mut Game, q: u64, id: u32) {
    let caster = g.state.rules.cards[&CardId(id)].owner;
    until_player(g, q, caster);
    let cost = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    let basic = |c| match c {
        Color::W => "Plains",
        Color::U => "Island",
        Color::B => "Swamp",
        Color::R => "Mountain",
        Color::G => "Forest",
        _ => panic!("basic witness"),
    };
    for color in &cost.colored {
        fund(g, q, basic(*color), 1);
    }
    loop {
        // Apply the last accepted mana activation before inspecting the pool
        // or choosing another untapped source. Responses enqueue an action.
        let r = request(g, q);
        assert_eq!(r.player(), caster);
        if g.payment_pool(caster, &cost).is_ok() {
            break;
        }
        let sources = || {
            g.state.rules.objects.values().filter(|o| {
                o.controller == caster
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card)
                        .unwrap()
                        .characteristics
                        .has_type(CardKind::Land)
            })
        };
        let source = sources()
            .find(|o| {
                cost.colored
                    .last()
                    .is_some_and(|c| g.definition(o.id.card).unwrap().name == basic(*c))
            })
            .or_else(|| sources().next())
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

fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    prefund(g, q, id);
    announce(g, q, id, targets);
    let stack = g.state.rules.stack.last().unwrap();
    assert_eq!(stack.controller, g.state.rules.cards[&CardId(id)].owner);
    assert_eq!(stack.source.card, CardId(id));
    assert_eq!(
        stack.paid.as_ref().unwrap(),
        &g.definition(CardId(id)).unwrap().characteristics.mana_cost
    );
}

fn cast_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    announce_paid(g, q, id, targets);
    resolve(g, q, |_| {});
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
    panic!("bounded top-only paid resolution")
}

fn grant(g: &Game, recipient: ObjectId) -> &ResolvedEffect {
    assert_eq!(g.state.rules.effects.len(), 1);
    let e = &g.state.rules.effects[0];
    assert!(
        matches!(&e.kind, ResolvedEffectKind::GrantKeywords { target, modifier: None, keywords }
        if *target == recipient && keywords == PAIR)
    );
    assert_eq!(e.origin.controller, PlayerId(0));
    assert_eq!(e.origin.source_block, 0);
    assert_eq!(e.origin.cursor, 0);
    assert_eq!(e.expires_in_cleanup_of_turn, g.turn().number);
    e
}

fn protected(g: &Game, recipient: ObjectId, expected: bool) {
    for k in PAIR {
        assert_eq!(g.has_keyword(recipient, *k), expected);
    }
}

fn zone(g: &Game, id: u32, expected: Zone) {
    assert_eq!(g.current(object(g, id)).unwrap().zone, expected);
}

fn queries(g: &Game, recipient: ObjectId) {
    let state = g.state.clone();
    let trace = g.trace.clone();
    let e = grant(g, recipient);
    for seat in 0..4 {
        let viewer = PlayerId(seat);
        if !g.is_live(viewer) {
            continue;
        }
        let QueryValue::Observation(view) = finish_query(g, g.observe(viewer, Budget::new(0)))
        else {
            panic!("resumable public observation")
        };
        assert!(
            !view
                .view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Library { .. }))
        );
        assert!(
            !view
                .view
                .objects
                .iter()
                .any(|o| matches!(o.zone, Zone::Hand { owner } if owner != viewer))
        );
        let effects = g.visible_effects(viewer).unwrap();
        let [
            VisibleEffect::GrantKeywords {
                origin,
                target,
                keywords,
                modifier,
                expires_in_cleanup_of_turn,
                ..
            },
        ] = effects.as_slice()
        else {
            panic!("one visible scalar grant")
        };
        assert_eq!(origin.controller, PlayerId(0));
        assert_eq!(
            origin.oracle_id,
            g.definition(e.origin.source.card).unwrap().oracle_id
        );
        assert_eq!(
            origin.source.announced,
            e.origin.source_handles[seat as usize]
        );
        assert_eq!(
            origin.source.current,
            g.handle_for(viewer, e.origin.source).ok()
        );
        assert_eq!(target.announced, e.target_handles[seat as usize]);
        assert_eq!(target.current, g.handle_for(viewer, recipient).ok());
        assert_eq!(keywords, PAIR);
        assert!(modifier.is_none());
        assert_eq!(*expires_in_cleanup_of_turn, e.expires_in_cleanup_of_turn);
        if let Some(handle) = &target.current {
            let row = view
                .view
                .objects
                .iter()
                .find(|o| &o.handle == handle)
                .unwrap();
            assert_eq!(
                row.characteristics,
                Some(g.characteristics(recipient).unwrap())
            );
            for k in PAIR {
                assert!(row.keywords.contains(k));
            }
        }
    }
    assert_eq!(g.state, state);
    assert_eq!(g.trace, trace);
}

// Force only the advance quantum, never the frame/history. Both outer runs
// observe the same genuine atomic grant-before-life boundary and recover it.
fn first_grant(g: &mut Game, recipient: ObjectId) {
    let characteristics = g.characteristics(recipient).unwrap();
    for _ in 0..2000 {
        let a = g.advance(Budget::new(1));
        if !g.state.rules.effects.is_empty() {
            grant(g, recipient);
            protected(g, recipient, true);
            assert_eq!(g.characteristics(recipient).unwrap(), characteristics);
            assert_eq!(g.state.rules.players[0].life, 40.into());
            assert!(matches!(&g.state.rules.frame, Frame::Resolve { frame } if frame.cursor == 1));
            recover(g);
            return;
        }
        protected(g, recipient, false);
        match a {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => {
                g.respond(r.player(), r.decision_id(), pass(&r)).unwrap();
            }
            x => panic!("first paid grant {x:?}"),
        }
    }
    panic!("actual grant-before-life suspension")
}

#[test]
fn targeted_permanent_keyword_paid_full_types_single_capture_and_separate_caster_life() {
    for (name, types) in [
        ("Grizzly Bears", vec![CardKind::Creature]),
        ("Bonesplitter", vec![CardKind::Artifact]),
        ("Forest", vec![CardKind::Land]),
        ("Goblin Bombardment", vec![CardKind::Enchantment]),
        ("Silver Myr", vec![CardKind::Artifact, CardKind::Creature]),
        (
            "Seat of the Synod",
            vec![CardKind::Artifact, CardKind::Land],
        ),
    ] {
        let mut rows = vec![
            ("Tamiyo's Safekeeping", 0, 0, true),
            (name, 0, 0, false),
            (name, 1, 1, false),
        ];
        lands(&mut rows, "Forest", 0, 1);
        both(prepared(&rows), |g, q| {
            let recipient = object(g, 1);
            let base = g.characteristics(recipient).unwrap();
            for kind in &types {
                assert!(base.has_type(*kind));
            }
            let payer = g
                .state
                .rules
                .objects
                .values()
                .find(|o| {
                    o.controller == PlayerId(0)
                        && o.zone == Zone::Battlefield
                        && g.definition(o.id.card).unwrap().name == "Forest"
                })
                .unwrap()
                .id;
            announce_paid(g, q, 0, target(g, 1));
            let paid = g.state.rules.stack.last().unwrap().clone();
            assert_eq!(paid.targets, [recipient]);
            assert_eq!(paid.paid.as_ref().unwrap().colored, [Color::G]);
            assert_eq!(paid.paid.as_ref().unwrap().generic, 0.into());
            first_grant(g, recipient);
            assert_eq!(grant(g, recipient).origin.stack_id, paid.id);
            assert_eq!(grant(g, recipient).origin.source, paid.source);
            queries(g, recipient);
            resolve(g, q, |_| {});
            assert_eq!(g.characteristics(recipient).unwrap(), base);
            protected(g, object(g, 2), false);
            assert_eq!(g.state.rules.players[0].life, 42.into());
            for seat in 1..4 {
                assert_eq!(g.state.rules.players[seat].life, 40.into());
            }
            assert!(g.current(payer).unwrap().tapped, "actual green land paid");
            zone(g, 0, Zone::Graveyard { owner: PlayerId(0) });
            if g.trace.mode == TraceMode::Detailed {
                let gains: Vec<_> = batches(g)
                    .into_iter()
                    .flat_map(|b| &b.occurrences)
                    .filter(|o| {
                        matches!(&o.proposed, ProposedEvent::GainLife { player, amount }
                        if *player == PlayerId(0) && *amount == 2.into())
                    })
                    .collect();
                assert_eq!(gains.len(), 1);
            }
            queries(g, recipient);
        });
    }
}

#[test]
fn targeted_permanent_keyword_paid_borrowed_control_recheck_and_postgrant_hexproof_authority() {
    for name in [
        "Grizzly Bears",
        "Bonesplitter",
        "Forest",
        "Goblin Bombardment",
    ] {
        for grant_first in [false, true] {
            let mut rows = vec![
                ("Confiscate", 0, 0, true),
                (name, 2, 2, false),
                ("Tamiyo's Safekeeping", 0, 0, true),
                ("Disperse", 1, 1, true),
                ("Disperse", 0, 0, true),
                (
                    if name == "Forest" {
                        "Tamiyo's Safekeeping"
                    } else {
                        "Disperse"
                    },
                    2,
                    2,
                    true,
                ),
                ("Bonesplitter", 1, 1, false), // honest unprotected decoy
            ];
            lands(&mut rows, "Island", 0, 8);
            lands(&mut rows, "Forest", 0, 1);
            lands(&mut rows, "Island", 1, 2);
            lands(&mut rows, "Island", 2, 2);
            lands(&mut rows, "Forest", 2, 1);
            both(prepared(&rows), |g, q| {
                cast_paid(g, q, 0, target(g, 1));
                control(g, 1, 0, 2, 2);
                let recipient = object(g, 1);
                announce_paid(g, q, 2, target(g, 1));
                if grant_first {
                    resolve(g, q, |_| {});
                }
                // Ordinary opposing Auras cannot be cast in response. A paid
                // opponent bounce of our real control Aura removes control legally.
                cast_paid(g, q, 3, target(g, 0));
                control(g, 1, 2, 2, 2);
                if !grant_first {
                    assert!(
                        g.state.rules.effects.is_empty(),
                        "all targets illegal skips whole spell"
                    );
                    assert_eq!(g.state.rules.players[0].life, 40.into());
                    protected(g, recipient, false);
                    return;
                }
                grant(g, recipient);
                protected(g, recipient, true);
                assert_eq!(g.state.rules.players[0].life, 42.into());
                assert!(!g.targetable_by(recipient, PlayerId(0)));
                assert!(g.targetable_by(recipient, PlayerId(2)));
                // Current controller, not owner of the paid grant, governs new
                // target choices. Decoy makes Cast preflight independently legal.
                let mut probe = g.fork();
                until_player(&mut probe, q, PlayerId(0));
                answer(&mut probe, q, Response::Cast { card: CardId(4) });
                let r = request(&mut probe, q);
                assert!(matches!(&r, Request::Targets { candidates, .. }
                    if candidates.contains(&object(&probe, 6)) && !candidates.contains(&recipient)));
                let state = probe.state.clone();
                let trace = probe.trace.clone();
                assert!(
                    probe
                        .respond(
                            r.player(),
                            r.decision_id(),
                            Response::Targets {
                                objects: vec![recipient]
                            }
                        )
                        .is_err()
                );
                assert_eq!(probe.state, state);
                assert_eq!(probe.trace, trace);
                queries(g, recipient);
                // Disperse's complete source excludes lands. For the land,
                // the current controller instead pays for a second Safekeeping
                // while the original Hexproof still applies.
                cast_paid(g, q, 5, target(g, 1));
                if name == "Forest" {
                    assert!(g.current(recipient).is_ok());
                    assert_eq!(g.state.rules.effects.len(), 2);
                    assert_eq!(g.state.rules.players[2].life, 42.into());
                    protected(g, recipient, true);
                } else {
                    assert!(g.current(recipient).is_err());
                    zone(g, 1, Zone::Hand { owner: PlayerId(2) });
                    queries(g, recipient);
                }
                assert_eq!(g.state.rules.players[0].life, 42.into());
            });
        }
    }
}

#[test]
fn targeted_permanent_keyword_paid_target_departure_and_reentry_never_rebind() {
    for timing in ["before_grant", "after_grant", "owner_departure"] {
        let mut rows = vec![
            ("Tamiyo's Safekeeping", 0, 0, true),
            // Explicit initial foreign controller solely for owner-departure
            // case; genuine paid borrowing is covered in the adjacent witness.
            (
                "Grizzly Bears",
                if timing == "owner_departure" { 2 } else { 0 },
                0,
                false,
            ),
            ("Unsummon", 0, 0, true),
        ];
        lands(&mut rows, "Forest", 0, 4);
        lands(&mut rows, "Island", 0, 1);
        both(prepared(&rows), |g, q| {
            let recipient = object(g, 1);
            announce_paid(g, q, 0, target(g, 1));
            if timing == "after_grant" {
                resolve(g, q, |_| {});
            }
            if timing == "owner_departure" {
                g.concede(PlayerId(2)).unwrap();
                request(g, q);
                resolve(g, q, |_| {});
                assert!(g.object_id(CardId(1)).is_none());
            } else {
                cast_paid(g, q, 2, target(g, 1));
                zone(g, 1, Zone::Hand { owner: PlayerId(0) });
                assert!(g.current(recipient).is_err());
                // Stack is now empty; only a genuine active own main may recast.
                assert!(g.state.rules.stack.is_empty());
                assert_eq!(g.active_player(), Some(PlayerId(0)));
                assert_eq!(g.turn().step, TurnStep::PrecombatMain);
                cast_paid(g, q, 1, vec![]);
                assert_ne!(object(g, 1), recipient);
                protected(g, object(g, 1), false);
            }
            if timing == "after_grant" {
                grant(g, recipient);
                assert_eq!(g.state.rules.players[0].life, 42.into());
                queries(g, recipient);
            } else {
                assert!(g.state.rules.effects.is_empty());
                assert_eq!(g.state.rules.players[0].life, 40.into());
            }
            recover(g);
        });
    }
}

#[test]
fn targeted_permanent_keyword_paid_atomic_prefix_target_owner_and_caster_concessions() {
    for departing in [None, Some(PlayerId(2)), Some(PlayerId(0))] {
        let mut rows = vec![
            ("Control Magic", 0, 0, true),
            ("Grizzly Bears", 2, 2, false),
            ("Tamiyo's Safekeeping", 0, 0, true),
        ];
        lands(&mut rows, "Island", 0, 4);
        lands(&mut rows, "Forest", 0, 1);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 0, target(g, 1));
            control(g, 1, 0, 2, 2);
            let recipient = object(g, 1);
            announce_paid(g, q, 2, target(g, 1));
            first_grant(g, recipient);
            queries(g, recipient);
            if let Some(player) = departing {
                g.concede(player).unwrap();
            }
            resolve(g, q, |_| {});
            grant(g, recipient);
            if departing == Some(PlayerId(2)) {
                assert!(g.object_id(CardId(1)).is_none());
            } else {
                protected(g, recipient, true);
                if departing == Some(PlayerId(0)) {
                    control(g, 1, 2, 2, 2);
                }
            }
            if departing != Some(PlayerId(0)) {
                assert_eq!(
                    g.state.rules.players[0].life,
                    42.into(),
                    "already-started life tail stays with caster"
                );
            } else {
                assert_eq!(g.state.rules.players[0].life, 40.into());
            }
            for seat in 1..4 {
                assert_eq!(g.state.rules.players[seat].life, 40.into());
            }
            queries(g, recipient);
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
        });
    }
}

#[test]
fn targeted_permanent_keyword_public_targets_privacy_invalid_responses_and_stale_work() {
    let mut rows = vec![
        ("Tamiyo's Safekeeping", 0, 0, true),
        ("Silver Myr", 0, 0, false),
        ("Seat of the Synod", 0, 0, false),
        ("Goblin Bombardment", 0, 0, false),
        ("Grizzly Bears", 1, 1, false),
        ("Tamiyo's Safekeeping", 1, 1, true),
    ];
    lands(&mut rows, "Forest", 0, 1);
    both(prepared(&rows), |g, q| {
        prefund(g, q, 0);
        answer(g, q, Response::Cast { card: CardId(0) });
        let r = request(g, q);
        let expected: Vec<_> = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && o.controller == PlayerId(0)
                    && g.targetable_by(o.id, PlayerId(0))
            })
            .map(|o| o.id)
            .collect();
        assert!(matches!(&r, Request::Targets { candidates, .. } if candidates == &expected));
        for id in [1, 2, 3] {
            assert!(expected.contains(&object(g, id)));
        }
        let state = g.state.clone();
        let trace = g.trace.clone();
        let QueryValue::Decision(Some(PlayerRequest::Targets {
            token,
            candidates,
            occurrences,
            ..
        })) = finish_query(g, g.player_decision_for(PlayerId(0), Budget::new(0)))
        else {
            panic!("public controlled permanent targets")
        };
        assert_eq!(
            occurrences,
            g.definition(CardId(0))
                .unwrap()
                .spell
                .as_ref()
                .unwrap()
                .targets
        );
        assert_eq!(
            candidates,
            expected
                .iter()
                .map(|o| g.handle_for(PlayerId(0), *o).unwrap())
                .collect::<Vec<_>>()
        );
        for seat in 1..4 {
            assert!(matches!(
                finish_query(g, g.player_decision_for(PlayerId(seat), Budget::new(0))),
                QueryValue::Decision(None)
            ));
        }
        let recipient = object(g, 1);
        assert!(
            g.respond(
                PlayerId(1),
                r.decision_id(),
                Response::Targets {
                    objects: vec![recipient],
                }
            )
            .is_err()
        );
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        for objects in [
            vec![],
            vec![object(g, 4)],
            vec![object(g, 0)],
            vec![ObjectId {
                incarnation: recipient.incarnation + 1,
                ..recipient
            }],
            vec![recipient, object(g, 2)],
        ] {
            assert!(
                g.respond(r.player(), r.decision_id(), Response::Targets { objects })
                    .is_err()
            );
            assert_eq!(g.state, state);
            assert_eq!(g.trace, trace);
        }
        let QueryResult::Incomplete { work: stale, .. } = g.observe(PlayerId(0), Budget::new(0))
        else {
            panic!("pending query work")
        };
        let response = PlayerResponse::Targets {
            objects: vec![g.handle_for(PlayerId(0), recipient).unwrap()],
        };
        let QueryValue::Validated(receipt) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token.clone(), response.clone(), Budget::new(0)),
        ) else {
            panic!("receipt")
        };
        let QueryValue::Validated(duplicate) = finish_query(
            g,
            g.check_player_response(PlayerId(0), token, response, Budget::new(0)),
        ) else {
            panic!("duplicate receipt")
        };
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        g.submit(receipt).unwrap();
        let state = g.state.clone();
        let trace = g.trace.clone();
        assert!(g.submit(duplicate).is_err());
        assert!(matches!(
            g.resume_work(stale, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        assert_eq!(g.state, state);
        assert_eq!(g.trace, trace);
        assert!(matches!(request(g, q), Request::Mana { .. }));
        answer(g, q, Response::FinishMana { undo_mana: false });
        if matches!(request(g, q), Request::SpendMana { .. }) {
            answer(
                g,
                q,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::G, 1.into())]),
                },
            );
        }
        assert!(matches!(request(g, q), Request::Priority { .. }));
        assert!(g.state.rules.stack.last().unwrap().paid.is_some());
        resolve(g, q, |_| {});
        queries(g, recipient);
        assert_eq!(g.state.rules.players[0].life, 42.into());
    });
}

#[test]
fn targeted_permanent_keyword_paid_opposing_removal_wipe_cleanup_and_fresh_destroy() {
    for (name, removal, fresh) in [
        ("Grizzly Bears", "Doom Blade", "Doom Blade"),
        ("Grizzly Bears", "Lightning Bolt", "Doom Blade"),
        ("Bonesplitter", "Naturalize", "Naturalize"),
        ("Goblin Bombardment", "Disperse", "Naturalize"),
    ] {
        let mut rows = vec![
            ("Tamiyo's Safekeeping", 0, 0, true),
            (name, 0, 0, false),
            (removal, 1, 1, true),
            (fresh, 1, 1, true),
            ("Planar Cleansing", 0, 0, true),
            ("Grizzly Bears", 1, 1, false),
            ("Lightning Bolt", 0, 0, true),
            ("Naturalize", 0, 0, true),
            ("Prodigal Sorcerer", 1, 1, false),
        ];
        lands(&mut rows, "Forest", 0, 5);
        lands(&mut rows, "Plains", 0, 6);
        lands(&mut rows, "Mountain", 0, 1);
        lands(&mut rows, "Swamp", 1, 4);
        lands(&mut rows, "Forest", 1, 4);
        lands(&mut rows, "Island", 1, 2);
        lands(&mut rows, "Mountain", 1, 1);
        both(prepared(&rows), |g, q| {
            let recipient = object(g, 1);
            announce_paid(g, q, 2, target(g, 1)); // legal before protection
            announce_paid(g, q, 0, target(g, 1));
            resolve(g, q, |_| {}); // old opposing target rechecked, whole removal skipped
            zone(g, 1, Zone::Battlefield);
            zone(g, 2, Zone::Graveyard { owner: PlayerId(1) });
            protected(g, recipient, true);
            if name == "Grizzly Bears" {
                let mut probe = g.fork();
                until_player(&mut probe, q, PlayerId(1));
                let pinger = object(&probe, 8);
                answer(
                    &mut probe,
                    q,
                    Response::ActivateAbility {
                        object: pinger,
                        ability: 0,
                    },
                );
                let r = request(&mut probe, q);
                assert!(matches!(&r, Request::EntityTargets { candidates, .. }
                    if !candidates.contains(&TargetEntity::Object { object: recipient })
                        && candidates.contains(&TargetEntity::Object { object: object(&probe, 5) })));
                let before = probe.state.clone();
                let trace = probe.trace.clone();
                assert!(
                    probe
                        .respond(
                            r.player(),
                            r.decision_id(),
                            Response::EntityTargets {
                                entities: vec![TargetEntity::Object { object: recipient }],
                            }
                        )
                        .is_err()
                );
                assert_eq!(probe.state, before);
                assert_eq!(probe.trace, trace);
                let decoy = target(&probe, 5);
                answer(&mut probe, q, Response::EntityTargets { entities: decoy });
                assert!(matches!(request(&mut probe, q), Request::Priority { .. }));
                assert!(probe.state.rules.stack.last().unwrap().paid.is_some());
                assert!(probe.current(pinger).unwrap().tapped);
                resolve(&mut probe, q, |_| {});
                assert_eq!(
                    probe.current(object(&probe, 5)).unwrap().marked_damage,
                    1.into()
                );
                assert_eq!(probe.current(recipient).unwrap().marked_damage, 0.into());
                recover(&probe);
            }
            // Own targeting remains legal; actual lethal damage/destruction is
            // prevented by Indestructible, independently of opposing Hexproof.
            cast_paid(
                g,
                q,
                if name == "Grizzly Bears" { 6 } else { 7 },
                target(g, 1),
            );
            if name == "Grizzly Bears" {
                assert_eq!(g.current(recipient).unwrap().marked_damage, 3.into());
            }
            cast_paid(g, q, 4, vec![]);
            zone(g, 1, Zone::Battlefield);
            zone(g, 5, Zone::Graveyard { owner: PlayerId(1) });
            assert_eq!(g.state.rules.players[0].life, 42.into());
            queries(g, recipient);
            recover(g);
            through_cleanup(g, q);
            assert!(g.state.rules.effects.is_empty());
            protected(g, recipient, false);
            assert_eq!(g.current(recipient).unwrap().marked_damage, 0.into());
            cast_paid(g, q, 3, target(g, 1));
            zone(g, 1, Zone::Graveyard { owner: PlayerId(0) });
        });
    }
}

#[test]
fn targeted_permanent_keyword_paid_bombardment_own_bounce_lki_and_one_damage_no_damage_lifegain() {
    for bounce in [false, true] {
        let mut rows = vec![
            ("Tamiyo's Safekeeping", 0, 0, true),
            ("Goblin Bombardment", 0, 0, false),
            ("Grizzly Bears", 0, 0, false),
            ("Yoked Ox", 1, 1, false),
            ("Disperse", 0, 0, true),
        ];
        lands(&mut rows, "Forest", 0, 1);
        lands(&mut rows, "Island", 0, 2);
        both(prepared(&rows), |g, q| {
            cast_paid(g, q, 0, target(g, 1));
            let source = object(g, 1);
            let sacrificed = object(g, 2);
            assert_eq!(g.state.rules.players[0].life, 42.into());
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
            assert!(matches!(request(g, q), Request::SacrificeCost { .. }));
            answer(g, q, Response::SacrificeCost { object: sacrificed });
            assert!(matches!(request(g, q), Request::Priority { .. }));
            let ping = g.state.rules.stack.last().unwrap().clone();
            assert!(ping.paid.is_some());
            assert_eq!(ping.source, source);
            zone(g, 2, Zone::Graveyard { owner: PlayerId(0) });
            if bounce {
                announce_paid(g, q, 4, target(g, 1));
                resolve_top(g, q, 1);
                assert!(g.current(source).is_err());
                let pending = g.state.rules.stack.last().unwrap();
                assert_eq!(pending.id, ping.id);
                let lki = pending.source_facts.as_ref().unwrap();
                assert_eq!(lki.object, source);
                assert_eq!(lki.controller, PlayerId(0));
                assert_eq!(lki.characteristics.kind, CardKind::Enchantment);
                assert_eq!(lki.zone, Zone::Battlefield);
                for k in PAIR {
                    assert!(lki.keywords.contains(k));
                }
                assert!(!lki.keywords.contains(&KeywordAbility::Lifelink));
                assert!(!lki.keywords.contains(&KeywordAbility::Deathtouch));
                queries(g, source);
                recover(g);
            }
            resolve(g, q, |_| {});
            assert_eq!(g.current(object(g, 3)).unwrap().marked_damage, 1.into());
            zone(g, 3, Zone::Battlefield);
            assert_eq!(
                g.state.rules.players[0].life,
                42.into(),
                "2 life was separate; damage gains none"
            );
            if g.trace.mode == TraceMode::Detailed {
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
                assert_eq!(actual.len(), 1);
                assert_eq!(actual[0].amount, 1.into());
                assert_eq!(actual[0].source.object, source);
                assert_eq!(actual[0].source.characteristics.kind, CardKind::Enchantment);
                assert_eq!(actual[0].controller, PlayerId(0));
                for k in PAIR {
                    assert!(actual[0].source.keywords.contains(k));
                }
            }
            queries(g, source);
        });
    }
}

#[test]
fn targeted_permanent_keyword_authenticated_restore_rejects_normalized_grant_and_life_tail_forgery()
{
    let rows = [
        ("Tamiyo's Safekeeping", 0, 0, true),
        ("Grizzly Bears", 0, 0, false),
        ("Bonesplitter", 0, 0, false),
        ("Forest", 0, 0, false),
    ];
    both(prepared(&rows), |g, q| {
        let recipient = object(g, 1);
        announce_paid(g, q, 0, target(g, 1));
        first_grant(g, recipient);
        let wire = g.snapshot().unwrap();
        for variant in 0..10 {
            let mut bad: Snapshot = strict_read(&wire).unwrap();
            let e = &mut bad.state.rules.effects[0];
            match variant {
                0 => {
                    let ResolvedEffectKind::GrantKeywords { keywords, .. } = &mut e.kind else {
                        unreachable!()
                    };
                    keywords.pop();
                }
                1 => {
                    let ResolvedEffectKind::GrantKeywords { target, .. } = &mut e.kind else {
                        unreachable!()
                    };
                    *target = object(g, 2);
                    e.target_handles = (0..4)
                        .map(|s| g.handle_for(PlayerId(s), *target).unwrap())
                        .collect();
                }
                2 => e.origin.controller = PlayerId(1),
                3 => e.expires_in_cleanup_of_turn += 1,
                4 => e.origin.cursor = 1,
                5 => e.origin.source_block = 1,
                6 => e.origin.source = recipient,
                7 => {
                    bad.state.rules.stack.last_mut().unwrap().controller = PlayerId(1);
                }
                8 => {
                    bad.state.rules.stack.last_mut().unwrap().source = object(g, 2);
                }
                _ => bad.state.rules.players[0].life.add(&2.into()),
            }
            assert_ne!(bad.state, g.state, "meaningful forgery {variant}");
            bad.state_sha256 = digest(&bad.state).unwrap();
            assert!(
                Game::restore(g.prepared.as_ref().clone(), &canonical(&bad).unwrap()).is_err(),
                "normalized forgery {variant}"
            );
        }
        // Identity/capability forgery must fail authenticated restore as well:
        // a different Prepared cannot authenticate the genuine paid prefix.
        let mut changed = g.prepared.as_ref().clone();
        let mut catalog = (*changed.catalog).clone();
        let d = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Tamiyo's Safekeeping")
            .unwrap();
        d.capabilities
            .retain(|c| *c != Capability::TemporaryPermanentKeywordProgramV1);
        changed.catalog_digest = digest(&catalog).unwrap();
        changed.catalog = Arc::new(catalog);
        assert!(Game::restore(changed, &wire).is_err());
        resolve(g, q, |_| {});
        assert_eq!(g.state.rules.players[0].life, 42.into());
        queries(g, recipient);
    });
}
