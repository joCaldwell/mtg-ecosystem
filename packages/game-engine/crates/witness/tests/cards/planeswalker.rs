//! CR 306.5b, 606.3–4, 508.1b, 120.3c and 704.5i–j.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::evasion::{checked, game, until};
use mtg_core::testkit::*;

pub(super) const JACE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/phase1-r5/cc2b6e58-2d6f-495e-95f3-9496105c4cba.json"
));

fn view(g: &Game, player: u8) -> PermittedView {
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.observe(PlayerId(player), Budget::new(u64::MAX))
    else {
        panic!("expected permitted observation");
    };
    observation.view
}

fn resolve(g: &mut Game) -> Decision<Rules> {
    for _ in 0..12 {
        let decision = Driver::request(g, u64::MAX);
        if decision.reason != Reason::Priority || g.state.rules.stack.is_empty() {
            return decision;
        }
        Driver::pass(g);
    }
    panic!("stack did not resolve");
}

fn jace_loyalty(g: &Game, controller: u8) -> Quantity {
    view(g, controller)
        .objects
        .iter()
        .find(|o| {
            o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba" && o.zone == Zone::Battlefield
        })
        .unwrap()
        .loyalty_counters
        .clone()
}

#[test]
fn planeswalker_loyalty_timing_cost_limit_and_restore() {
    let mut g = game(
        &[JACE],
        &[
            ("Jace Beleren", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    let d = resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    let object = Driver::object(&g, 0);
    let action = |ability| Candidate::Action {
        action: Action::ActivateAbility { object, ability },
    };
    assert!(d.index(&action(0)).is_some());
    assert!(d.index(&action(1)).is_some());
    assert!(d.index(&action(2)).is_none());
    Driver::action(&mut g, Action::ActivateAbility { object, ability: 1 });
    let d = Driver::request(&mut g, u64::MAX);
    let answer = d
        .pick_one(&Candidate::Player {
            player: PlayerId(0),
        })
        .unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    let d = resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 2.into());
    assert!(d.index(&action(0)).is_none());
    assert!(d.index(&action(1)).is_none());
    checked(&g);
}

#[test]
fn planeswalker_defender_loses_loyalty_and_dies() {
    let mut g = game(
        &[JACE],
        &[
            ("Grizzly Bears", 0, Zone::Battlefield),
            ("Grizzly Bears", 0, Zone::Battlefield),
            ("Jace Beleren", 1, Zone::Battlefield),
        ],
    );
    let jace = Driver::object(&g, 2);
    let d = until(&mut g, &Reason::Attackers);
    let target = d
        .index(&Candidate::Object {
            object: Driver::object(&g, 2),
        })
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![target], vec![target]],
        },
    )
    .unwrap();
    let d = until(&mut g, &Reason::Blockers);
    g.respond(d.player, d.id, d.block(vec![])).unwrap();
    for _ in 0..16 {
        let d = Driver::request(&mut g, u64::MAX);
        let view = view(&g, 1);
        if view.objects.iter().any(|o| {
            o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba"
                && o.zone == (Zone::Graveyard { owner: PlayerId(1) })
        }) {
            let cause = g
                .trace()
                .records
                .iter()
                .find_map(|record| {
                    let EvidenceRecord::Batch { batch } = &record.record else {
                        return None;
                    };
                    batch.occurrences.iter().find_map(|occurrence| {
                        matches!(occurrence.actual, EventOutcome::DamageCommitted { .. })
                            .then(|| {
                                occurrence.counter_changes.iter().find(|change| {
                                    change.holder
                                        == (CounterHolder::Permanent {
                                            object: jace,
                                            controller: PlayerId(1),
                                        })
                                })
                            })
                            .flatten()
                    })
                })
                .expect("combat damage loyalty removal");
            assert_eq!(cause.counter, mtg_contracts::kir::CounterKind::Loyalty);
            assert_eq!(
                (cause.previous.clone(), cause.current.clone()),
                (3.into(), 0.into())
            );
            assert_eq!(view.players[1].life, 40.into());
            checked(&g);
            return;
        }
        g.respond(
            d.player,
            d.id,
            d.select(Candidate::Action {
                action: Action::Pass,
            }),
        )
        .unwrap();
    }
    panic!("combat did not put the planeswalker into its owner's graveyard");
}

#[test]
fn planeswalker_legend_choice_keeps_one_copy() {
    let mut g = game(
        &[JACE],
        &[
            ("Jace Beleren", 0, Zone::Battlefield),
            ("Jace Beleren", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3, 4]);
    let d = resolve(&mut g);
    assert!(matches!(d.reason, Reason::LegendRule { .. }));
    let keep = Driver::object(&g, 1);
    Driver::pick_object(&mut g, keep);
    Driver::request(&mut g, u64::MAX);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    checked(&g);
}

#[test]
fn planeswalker_is_a_player_or_planeswalker_damage_target() {
    let spike = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/2837888c-bfa8-4955-9334-0605ad409f7e.json"
    ));
    let mut g = game(
        &[JACE, spike],
        &[
            ("Jace Beleren", 1, Zone::Battlefield),
            ("Grizzly Bears", 1, Zone::Battlefield),
            ("Lava Spike", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Mountain", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(2));
    let d = Driver::request(&mut g, u64::MAX);
    let bears = Driver::object(&g, 1);
    assert!(d.index(&Candidate::Object { object: bears }).is_none());
    let jace = Driver::object(&g, 0);
    Driver::pick_object(&mut g, jace);
    Driver::pay(&mut g, &[3]);
    resolve(&mut g);
    assert_eq!(
        view(&g, 1)
            .objects
            .iter()
            .find(|o| o.oracle_id == "cc2b6e58-2d6f-495e-95f3-9496105c4cba")
            .unwrap()
            .zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    checked(&g);
}

#[test]
fn planeswalker_damage_preserves_the_printed_creature_and_color_filter() {
    let fry = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/3a747893-a514-4689-8486-ecccf654fecb.json"
    ));
    let mut g = game(
        &[JACE, fry],
        &[
            ("Jace Beleren", 1, Zone::Battlefield),
            ("Grizzly Bears", 1, Zone::Battlefield),
            ("Storm Crow", 1, Zone::Battlefield),
            ("Fry", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Mountain", 0, Zone::Battlefield),
            ("Mountain", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(3));
    let d = Driver::request(&mut g, u64::MAX);
    let object = |card| Candidate::Object {
        object: Driver::object(&g, card),
    };
    assert!(d.index(&object(0)).is_some());
    assert!(d.index(&object(1)).is_none());
    assert!(d.index(&object(2)).is_some());
    assert!(
        !d.candidates
            .iter()
            .any(|c| matches!(c, Candidate::Player { .. }))
    );
    let target = Driver::object(&g, 0);
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[4, 5]);
    resolve(&mut g);
    assert_eq!(
        g.state.rules.objects[&CardId(0)].zone,
        Zone::Graveyard { owner: PlayerId(1) }
    );
    checked(&g);
}

const NIGHTSTALKER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/phase1-r5/10733767-1c97-4e2e-b02b-54bf346f6583.json"
));
const RESCUE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/phase1-r5/f0ae687a-7222-4521-a514-ba10373fed7e.json"
));

fn attack_planeswalker(g: &mut Game, card: u32) {
    let d = until(g, &Reason::Attackers);
    let target = d
        .index(&Candidate::Object {
            object: Driver::object(g, card),
        })
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![target]],
        },
    )
    .unwrap();
}

fn priority_at(g: &mut Game, step: TurnStep) {
    for _ in 0..16 {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority);
        if g.state.rules.turn.step == step {
            return;
        }
        Driver::pass(g);
    }
    panic!("no priority at {step:?}");
}

fn rescue(g: &mut Game, spell: u32, target: u32, land: u32) {
    Driver::cast(g, CardId(spell));
    let target = Driver::object(g, target);
    Driver::pick_object(g, target);
    Driver::pay(g, &[land]);
    resolve(g);
}

// CR 509.3g, 506.4c: removal of the attacked permanent doesn't undo the
// attack or change the defending player who discards at resolution.
#[test]
fn planeswalker_unblocked_attack_triggers_for_its_defending_player() {
    for removed in [false, true] {
        let mut g = game(
            &[JACE, NIGHTSTALKER, RESCUE],
            &[
                ("Abyssal Nightstalker", 0, Zone::Battlefield),
                ("Jace Beleren", 1, Zone::Battlefield),
                ("Rescue", 1, Zone::Hand { owner: PlayerId(1) }),
                ("Island", 1, Zone::Battlefield),
                ("Island", 1, Zone::Hand { owner: PlayerId(1) }),
            ],
        );
        attack_planeswalker(&mut g, 1);
        if removed {
            Driver::pass(&mut g);
            rescue(&mut g, 2, 1, 3);
        }
        let d = until(&mut g, &Reason::Blockers);
        g.respond(d.player, d.id, d.block(vec![])).unwrap();
        let d = until(&mut g, &Reason::ResolveDiscard);
        assert_eq!(d.player, PlayerId(1));
        let discard = Driver::object(&g, 4);
        Driver::pick_object(&mut g, discard);
        priority_at(&mut g, TurnStep::CombatDamage);
        assert_eq!(view(&g, 1).players[1].life, 40.into());
        if removed {
            assert_eq!(
                g.state.rules.objects[&CardId(1)].zone,
                Zone::Hand { owner: PlayerId(1) }
            );
        } else {
            assert_eq!(jace_loyalty(&g, 1), 1.into());
        }
        checked(&g);
    }
}

// CR 702.19b, 506.4c: a legal blocker receives lethal before excess reaches the
// attacked planeswalker. Its departure gives no excess to the defending player.
#[test]
fn planeswalker_trample_excess_follows_the_attacked_permanent() {
    for removed in [false, true] {
        let mut g = game(
            &[JACE, RESCUE],
            &[
                ("Colossal Dreadmaw", 0, Zone::Battlefield),
                ("Jace Beleren", 1, Zone::Battlefield),
                ("Grizzly Bears", 1, Zone::Battlefield),
                ("Rescue", 1, Zone::Hand { owner: PlayerId(1) }),
                ("Island", 1, Zone::Battlefield),
            ],
        );
        attack_planeswalker(&mut g, 1);
        let d = until(&mut g, &Reason::Blockers);
        let attacker = Driver::object(&g, 0);
        let defender = Driver::object(&g, 1);
        let blocker = Driver::object(&g, 2);
        g.respond(d.player, d.id, d.block(vec![(blocker, attacker)]))
            .unwrap();
        if removed {
            Driver::pass(&mut g);
            rescue(&mut g, 3, 1, 4);
        } else {
            let d = until(
                &mut g,
                &Reason::CombatDamage {
                    source: attacker,
                    power: 6.into(),
                },
            );
            assert!(
                d.candidates
                    .iter()
                    .all(|c| matches!(c, Candidate::Object { .. }))
            );
            assert!(
                g.respond(
                    d.player,
                    d.id,
                    d.damage_to(vec![(blocker, 1.into()), (defender, 5.into())])
                )
                .is_err()
            );
            g.respond(
                d.player,
                d.id,
                d.damage_to(vec![(blocker, 4.into()), (defender, 2.into())]),
            )
            .unwrap();
        }
        priority_at(&mut g, TurnStep::CombatDamage);
        assert_eq!(view(&g, 1).players[1].life, 40.into());
        assert_eq!(
            g.state.rules.objects[&CardId(2)].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        if !removed {
            assert_eq!(jace_loyalty(&g, 1), 1.into());
        }
        checked(&g);
    }
}

#[test]
fn planeswalker_loyalty_use_is_public_and_resets_on_reentry() {
    let mut g = game(
        &[JACE, RESCUE],
        &[
            ("Jace Beleren", 0, Zone::Battlefield),
            ("Rescue", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    let original = Driver::object(&g, 0);
    let action = |object| Action::ActivateAbility { object, ability: 0 };
    Driver::action(&mut g, action(original));
    Driver::request(&mut g, u64::MAX);
    let loyalty_is_used = |g: &Game| {
        view(g, 1)
            .objects
            .iter()
            .any(|o| o.zone == Zone::Battlefield && o.loyalty_used_this_turn)
    };
    assert!(loyalty_is_used(&g));
    checked(&g);
    let snapshot: serde_json::Value = serde_json::from_str(&g.snapshot().unwrap()).unwrap();
    // On turn 1 the only earlier marker is turn 0, which no turn has had, so
    // the general loyalty invariant rejects it before the paid-cost check.
    let earlier = g.state.rules.turn.number.checked_sub(1).unwrap();
    for (marker, expected) in [
        (
            None,
            "snapshot paid loyalty activation lacks its turn marker",
        ),
        (
            Some(earlier),
            if earlier == 0 {
                "snapshot loyalty invariant"
            } else {
                "snapshot paid loyalty activation lacks its turn marker"
            },
        ),
    ] {
        let mut forged = snapshot.clone();
        let object = forged["state"]["objects"]["0"].as_object_mut().unwrap();
        if let Some(turn) = marker {
            object.insert("loyalty_used_turn".into(), turn.into());
        } else {
            object.remove("loyalty_used_turn");
        }
        forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
        let error = Game::restore(g.prepared.as_ref().clone(), &forged.to_string())
            .err()
            .expect("restored paid loyalty activation without its current turn marker");
        assert!(error.contains(expected), "{error}");
    }

    resolve(&mut g);
    rescue(&mut g, 1, 0, 2);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[3, 4, 5]);
    let d = resolve(&mut g);
    let returned = Driver::object(&g, 0);
    assert_ne!(original, returned);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    assert!(!loyalty_is_used(&g));
    assert!(
        d.index(&Candidate::Action {
            action: action(returned)
        })
        .is_some()
    );
    Driver::action(&mut g, action(returned));
    resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 5.into());
    assert!(loyalty_is_used(&g));
    checked(&g);
}

// CR 701.34a: loyalty is one of the counter kinds proliferate can add.
#[test]
fn planeswalker_loyalty_is_proliferated_once() {
    let plan = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/2fc6de19-9d04-422b-90bc-eee2f9f8c84c.json"
    ));
    let mut g = game(
        &[JACE, plan],
        &[
            ("Jace Beleren", 0, Zone::Battlefield),
            ("Contentious Plan", 0, Zone::Hand { owner: PlayerId(0) }),
            ("Island", 0, Zone::Battlefield),
            ("Island", 0, Zone::Battlefield),
        ],
    );
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3]);
    let d = until(&mut g, &Reason::Proliferate);
    let jace = Driver::object(&g, 0);
    let answer = d.pick_one(&Candidate::Object { object: jace }).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    resolve(&mut g);
    assert_eq!(jace_loyalty(&g, 0), 4.into());
    assert!(view(&g, 0).objects.iter().all(|o| {
        o.counters
            .iter()
            .all(|counter| counter.kind != mtg_contracts::kir::CounterKind::Loyalty)
    }));
    checked(&g);
}

// CR 702.105a: a planeswalker's controller is a defending player, but an
// attack on their planeswalker does not trigger dethrone.
#[test]
fn planeswalker_defender_does_not_trigger_dethrone() {
    let cutthroat = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/phase1-r5/9a133991-8a7a-4bfd-b0ae-d76433450c03.json"
    ));
    let mut g = game(
        &[JACE, cutthroat],
        &[
            ("Grenzo's Cutthroat", 0, Zone::Battlefield),
            ("Jace Beleren", 1, Zone::Battlefield),
        ],
    );
    attack_planeswalker(&mut g, 1);
    let d = resolve(&mut g);
    assert_eq!(d.reason, Reason::Priority);
    assert_eq!(
        g.state.rules.objects[&CardId(0)].plus_one_counters,
        0.into()
    );
    checked(&g);
}

/// CR 606.3–4: a loyalty cost of zero puts on and removes no loyalty
/// counter, yet it is still the permanent's loyalty activation this turn.
/// Jace Beleren's "+2" is given a synthetic `[0]` cost on this private
/// board: activating it leaves three counters, stamps the use, and the
/// other loyalty abilities are no longer offered; an unpayable "−10"
/// is rejected without any change.
#[test]
fn zero_loyalty_cost_stamps_the_use_without_a_counter_change() {
    let mut definition = super::evasion::lowered(JACE);
    let mtg_contracts::ActivationCost::Loyalty { change } = &mut definition.activated[0].cost
    else {
        panic!("Jace Beleren's first ability is a loyalty ability");
    };
    assert_eq!(*change, 2.into());
    *change = Quantity::zero();
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(definition);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut g = Game::with_trace(
        Board::new(catalog, "r5-zero-loyalty-witness")
            .step(TurnStep::PrecombatMain)
            .seats(2)
            .permanent_or_hand("Jace Beleren", 0, 0, false)
            .libraries("Island", 3)
            .prepare(),
        TraceMode::Detailed,
    );
    let jace = Driver::object(&g, 0);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    let unpayable = g.fork();
    assert!(g.validate_activation(PlayerId(0), jace, 2).is_err());
    assert_eq!(g.state, unpayable.state);
    let d = Driver::request(&mut g, u64::MAX);
    let action = |ability| Candidate::Action {
        action: Action::ActivateAbility {
            object: jace,
            ability,
        },
    };
    assert!(d.index(&action(0)).is_some() && d.index(&action(2)).is_none());
    let hands = |g: &Game| {
        g.state
            .rules
            .objects
            .values()
            .filter(|o| matches!(o.zone, Zone::Hand { .. }))
            .count()
    };
    let drawn = hands(&g);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: jace,
            ability: 0,
        },
    );
    Driver::request(&mut g, u64::MAX);
    assert_eq!(g.state.rules.stack.len(), 1);
    let row = g.current(jace).unwrap();
    assert_eq!(row.loyalty_counters, 3.into());
    assert_eq!(row.loyalty_used_turn, Some(g.state.rules.turn.number));
    assert!(g.state.rules.pending_triggers.is_empty());
    checked(&g);
    assert_recoverable((&g).into());
    let d = resolve(&mut g);
    // The ability still resolved: each player drew a card.
    assert_eq!(hands(&g), drawn + 2);
    assert_eq!(jace_loyalty(&g, 0), 3.into());
    assert!(
        view(&g, 1)
            .objects
            .iter()
            .any(|o| o.zone == Zone::Battlefield && o.loyalty_used_this_turn)
    );
    for ability in 0..3 {
        assert!(d.index(&action(ability)).is_none());
    }
    checked(&g);
    assert_recoverable((&g).into());
}
