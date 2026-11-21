//! Bounded private complete-source M3 exits, outside frozen revision 17.
//! Nested under the existing cascade test harness; no old expectations changed.
use super::*;
const BLOW: &str = include_str!("../../../../../tests/fixtures/dismantling-blow.json");
const RECLAMATION: &str = include_str!("../../../../../tests/fixtures/natural-reclamation.json");
fn source_prepared(original: &str, library: &[&str]) -> Prepared {
    let mut p = prepared_with_records(
        original,
        library,
        &[
            BLOW,
            RECLAMATION,
            include_str!("../../../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json"),
            include_str!("../../../../../tests/fixtures/2bda1fb1-528c-4e66-89f1-bb1a85e9181e.json"),
        ],
    );
    for name in [
        "Plains", "Plains", "Plains", "Plains", "Plains", "Plains", "Island", "Island", "Island",
        "Island", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest", "Forest",
        "Mountain", "Mountain",
    ] {
        replace_spare(&mut p, PlayerId(0), name, Zone::Battlefield);
    }
    replace_spare(&mut p, PlayerId(0), "Arcane Signet", Zone::Battlefield);
    finish_prepared(p)
}
fn replace_spare(p: &mut Prepared, owner: PlayerId, name: &str, zone: Zone) -> CardId {
    let oracle = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap_or_else(|| panic!("missing source {name}"))
        .oracle_id
        .clone();
    let forest = p
        .catalog
        .definitions
        .iter()
        .find(|d| d.name == "Forest")
        .unwrap()
        .oracle_id
        .clone();
    let c = p
        .config
        .cards
        .iter_mut()
        .filter(|c| {
            c.owner == owner
                && c.oracle_id == forest
                && (c.zone == Zone::Hand { owner } || c.zone == Zone::Library { owner })
        })
        .min_by_key(|c| usize::from(matches!(c.zone, Zone::Library { .. })))
        .expect("physical spare");
    c.oracle_id = oracle;
    c.zone = zone;
    c.controller = owner;
    c.id
}
fn finish_prepared(mut p: Prepared) -> Prepared {
    for seat in 0..4 {
        assert_eq!(
            p.config
                .cards
                .iter()
                .filter(|c| c.owner == PlayerId(seat))
                .count(),
            100
        );
    }
    p.profile_digest = digest(&p.config).unwrap();
    p
}
fn named(g: &Game, name: &str, owner: PlayerId, zone: Zone) -> ObjectId {
    g.state
        .rules
        .objects
        .values()
        .find(|o| {
            o.zone == zone
                && g.state.rules.cards[&o.id.card].owner == owner
                && g.definition(o.id.card).unwrap().name == name
        })
        .unwrap_or_else(|| panic!("missing {name} {owner:?} {zone:?}"))
        .id
}
fn at_priority(g: &mut Game, player: PlayerId) {
    for _ in 0..100 {
        let r = request(g);
        if matches!(r,Request::Priority {player:p,..} if p==player) {
            return;
        }
        assert!(matches!(r, Request::Priority { .. }));
        answer(g, Response::Pass);
    }
    panic!("priority owner");
}
pub(super) fn fund_response(g: &Game, r: &Request) -> Response {
    let Request::Mana { player, cost, .. } = r else {
        panic!()
    };
    if g.can_pay(*player, cost).unwrap() {
        return Response::FinishMana { undo_mana: false };
    }
    let need = cost
        .colored
        .iter()
        .find(|c| {
            g.mana(*player, **c).unwrap_or_default()
                < Quantity::from(cost.colored.iter().filter(|s| *s == *c).count() as i64)
        })
        .copied();
    let source = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == *player
                && !o.tapped
                && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
                && g.validate_mana_activation(*player, o.id).is_ok()
                && g.mana_abilities(o.id)
                    .is_ok_and(|colors| need.is_none_or(|c| colors.contains(&c)))
        })
        .min_by_key(|o| {
            usize::from(need.is_none() && !g.mana_abilities(o.id).unwrap().contains(&Color::G))
        })
        .unwrap()
        .id;
    Response::ActivateMana { object: source }
}
pub(super) fn spending(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> BTreeMap<Color, Quantity> {
    let mut pool = available.clone();
    let mut out = BTreeMap::<Color, Quantity>::new();
    for c in &cost.colored {
        pool.get_mut(c).unwrap().subtract(&1.into());
        out.entry(*c).or_default().add(&1.into());
    }
    let mut generic = cost.generic.clone();
    for (c, n) in pool {
        let used = n.min(generic.clone());
        generic.subtract(&used);
        if used > Quantity::zero() {
            out.entry(c).or_default().add(&used);
        }
    }
    assert_eq!(generic, 0.into());
    out
}
pub(super) fn finish_announcement(
    g: &mut Game,
    kick: bool,
    target: Option<ObjectId>,
    stack: Option<StackId>,
) {
    for _ in 0..120 {
        let r = request(g);
        let response = match &r {
            Request::AdditionalCosts { candidates, .. } => Response::AdditionalCosts {
                selected: if kick {
                    candidates.iter().map(|c| c.identity.clone()).collect()
                } else {
                    vec![]
                },
            },
            Request::Targets { candidates, .. } => Response::Targets {
                objects: vec![target.unwrap_or(candidates[0])],
            },
            Request::StackTargets { .. } => Response::StackTargets {
                entries: vec![stack.unwrap()],
            },
            Request::Modes { .. } => Response::Modes { mode: 0 },
            Request::PlayerTargets { .. } => Response::PlayerTargets {
                players: vec![r.player()],
            },
            Request::Mana { .. } => fund_response(g, &r),
            Request::SpendMana {
                cost, available, ..
            } => Response::SpendMana {
                spending: spending(cost, available),
            },
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                order: candidates.iter().rev().copied().collect(),
            },
            Request::Priority { .. } => return,
            other => panic!("announcement {other:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
        prove(g);
    }
    panic!("announcement did not complete");
}
fn cast(
    g: &mut Game,
    player: PlayerId,
    name: &str,
    kick: bool,
    target: Option<ObjectId>,
    stack: Option<StackId>,
) -> StackId {
    at_priority(g, player);
    let source = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            g.definition(o.id.card).unwrap().name == name
                && g.state.rules.cards[&o.id.card].owner == player
                && matches!(o.zone, Zone::Hand { .. } | Zone::Command { .. })
        })
        .unwrap()
        .id;
    answer(g, Response::Cast { card: source.card });
    finish_announcement(g, kick, target, stack);
    g.state
        .rules
        .stack
        .iter()
        .find(|s| s.kind.is_spell() && s.source.card == source.card)
        .unwrap()
        .id
}
fn resolve_to_empty(g: &mut Game) {
    for _ in 0..160 {
        let r = request(g);
        if g.state.rules.stack.is_empty() {
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::OptionalEffect { .. } => Response::OptionalEffect { perform: false },
            Request::OrderTriggers { candidates, .. } => {
                Response::OrderTriggers { order: candidates }
            }
            other => panic!("resolve {other:?}"),
        };
        answer(g, response);
        prove(g);
    }
    panic!("resolution loop");
}
fn to_offer(g: &mut Game) {
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::OptionalEffect { .. }) {
            return;
        }
        assert!(matches!(r, Request::Priority { .. }));
        answer(g, Response::Pass);
        prove(g);
    }
    panic!("offer");
}
fn observe(g: &Game, viewer: PlayerId) -> Observation {
    match g.observe(viewer, Budget::unlimited()) {
        QueryResult::Complete(QueryValue::Observation(o)) => o,
        _ => panic!("observation query did not complete"),
    }
}
#[test]
fn m3c_bloodbraid_free_kicker_choose_decline_exact_spending_draw_privacy_and_mv() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for kick in [false, true] {
            let mut g = Game::with_trace(
                source_prepared("Bloodbraid Elf", &["Dismantling Blow", "Forest", "Island"]),
                mode,
            );
            let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
            let original = cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
            to_offer(&mut g);
            let parent = g.top().unwrap().id;
            answer(&mut g, Response::OptionalEffect { perform: true });
            assert!(matches!(request(&mut g), Request::AdditionalCosts { .. }));
            prove(&g);
            let selected = match request(&mut g) {
                Request::AdditionalCosts { candidates, .. } => {
                    if kick {
                        vec![candidates[0].identity.clone()]
                    } else {
                        vec![]
                    }
                }
                _ => unreachable!(),
            };
            answer(&mut g, Response::AdditionalCosts { selected });
            answer(
                &mut g,
                Response::Targets {
                    objects: vec![target],
                },
            );
            let r = request(&mut g);
            if kick {
                assert!(
                    matches!(&r,Request::Mana {cost,..} if *cost==ManaCost {generic:2.into(),colored:vec![Color::U]})
                );
            } else {
                assert!(
                    matches!(r, Request::Priority { .. }),
                    "free unselected cost has no mana payment"
                );
            }
            finish_announcement(&mut g, kick, Some(target), None);
            let child = g
                .state
                .rules
                .stack
                .iter()
                .find(|s| {
                    s.kind.is_spell()
                        && g.definition(s.source.card).unwrap().name == "Dismantling Blow"
                })
                .unwrap()
                .clone();
            assert_eq!(g.stack_mana_value(&child).unwrap(), 3.into());
            assert_eq!(
                child.paid.as_ref(),
                Some(&if kick {
                    ManaCost {
                        generic: 2.into(),
                        colored: vec![Color::U],
                    }
                } else {
                    zero_mana_cost()
                })
            );
            assert!(!g.state.rules.stack.iter().any(|s| s.id == parent));
            assert!(g.state.rules.stack.iter().any(|s| s.id == original));
            let own_before = observe(&g, PlayerId(0));
            let other_before = observe(&g, PlayerId(1));
            let hands = g
                .state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                .count();
            resolve_to_empty(&mut g);
            assert_eq!(
                g.current(g.object_id(target.card).unwrap()).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(
                g.state
                    .rules
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                    .count(),
                hands + if kick { 2 } else { 0 }
            );
            if kick {
                let own = observe(&g, PlayerId(0));
                let other = observe(&g, PlayerId(1));
                assert_eq!(
                    own.view
                        .objects
                        .iter()
                        .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                        .count(),
                    own_before
                        .view
                        .objects
                        .iter()
                        .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                        .count()
                        + 2
                );
                assert_eq!(
                    other
                        .view
                        .objects
                        .iter()
                        .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                        .count(),
                    other_before
                        .view
                        .objects
                        .iter()
                        .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                        .count()
                );
            }
            prove(&g);
        }
    }
}
#[test]
fn m3c_ordinary_kicker_total_countered_and_all_targets_illegal_suppress_draw() {
    for path in 0..3 {
        let mut p = source_prepared("Dismantling Blow", &["Forest", "Island"]);
        replace_spare(
            &mut p,
            PlayerId(0),
            "Counterspell",
            Zone::Hand { owner: PlayerId(0) },
        );
        if path == 1 {
            replace_spare(
                &mut p,
                PlayerId(0),
                "Dismantling Blow",
                Zone::Hand { owner: PlayerId(0) },
            );
        }
        p = finish_prepared(p);
        let mut g = Game::new(p);
        let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
        let spell = cast(
            &mut g,
            PlayerId(0),
            "Dismantling Blow",
            true,
            Some(target),
            None,
        );
        assert_eq!(
            g.state
                .rules
                .stack
                .iter()
                .find(|s| s.id == spell)
                .unwrap()
                .total_cost,
            ManaCost {
                generic: 4.into(),
                colored: vec![Color::W, Color::U]
            }
        );
        let before = g.state.rules.libraries[&PlayerId(0)].len();
        if path == 0 {
            cast(
                &mut g,
                PlayerId(0),
                "Counterspell",
                false,
                None,
                Some(spell),
            );
        }
        if path == 1 {
            cast(
                &mut g,
                PlayerId(0),
                "Dismantling Blow",
                false,
                Some(target),
                None,
            );
        }
        resolve_to_empty(&mut g);
        assert_eq!(
            g.state.rules.libraries[&PlayerId(0)].len(),
            before - if path == 2 { 2 } else { 0 }
        );
        prove(&g);
    }
}
#[test]
fn m3c_indestructible_target_does_not_suppress_linked_kicked_draw() {
    let mut p = source_prepared("Dismantling Blow", &["Forest", "Island"]);
    let target = replace_spare(&mut p, PlayerId(0), "Darksteel Relic", Zone::Battlefield);
    let mut g = Game::new(finish_prepared(p));
    let target = g.object_id(target).unwrap();
    cast(
        &mut g,
        PlayerId(0),
        "Dismantling Blow",
        true,
        Some(target),
        None,
    );
    resolve_to_empty(&mut g);
    assert_eq!(g.current(target).unwrap().zone, Zone::Battlefield);
    assert!(g.state.rules.libraries[&PlayerId(0)].is_empty());
    prove(&g);
}
#[test]
fn m3c_selected_zero_mana_declaration_retains_window_synthetic_quantity_control() {
    let mut p = source_prepared("Bloodbraid Elf", &["Dismantling Blow"]);
    let mut catalog = (*p.catalog).clone();
    let d = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Dismantling Blow")
        .unwrap();
    d.spell.as_mut().unwrap().optional_additional_costs[0].mana = zero_mana_cost();
    d.capabilities = d.requirements();
    catalog.validate().unwrap();
    p.catalog_digest = digest(&catalog).unwrap();
    p.catalog = Arc::new(catalog);
    let mut g = Game::new(p);
    let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
    cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
    to_offer(&mut g);
    answer(&mut g, Response::OptionalEffect { perform: true });
    let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
        panic!()
    };
    answer(
        &mut g,
        Response::AdditionalCosts {
            selected: vec![candidates[0].identity.clone()],
        },
    );
    answer(
        &mut g,
        Response::Targets {
            objects: vec![target],
        },
    );
    assert!(matches!(request(&mut g),Request::Mana {cost,..} if cost==zero_mana_cost()));
    // Internal zero-quantity control; the exact archived {2}{U} witness is above.
}
#[test]
fn m3c_genuine_selected_kicker_nested_signet_ramos_failure_keep_undo_replays() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for undo in [false, true] {
            let mut p = source_prepared("Bloodbraid Elf", &["Dismantling Blow"]);
            replace_spare(
                &mut p,
                PlayerId(0),
                "Bloodbraid Elf",
                Zone::Hand { owner: PlayerId(0) },
            );
            replace_spare(
                &mut p,
                PlayerId(0),
                "Healing Salve",
                Zone::Hand { owner: PlayerId(0) },
            );
            for _ in 0..2 {
                replace_spare(&mut p, PlayerId(0), "Golgari Signet", Zone::Battlefield);
            }
            let mut g = Game::with_trace(finish_prepared(p), mode);
            cast(
                &mut g,
                PlayerId(0),
                "Ramos, Dragon Engine",
                false,
                None,
                None,
            );
            resolve_to_empty(&mut g);
            cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
            to_offer(&mut g);
            answer(&mut g, Response::OptionalEffect { perform: false });
            resolve_to_empty(&mut g);
            cast(&mut g, PlayerId(0), "Healing Salve", false, None, None);
            resolve_to_empty(&mut g);
            cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
            to_offer(&mut g);
            let ramos = named(&g, "Ramos, Dragon Engine", PlayerId(0), Zone::Battlefield);
            assert_eq!(g.current(ramos).unwrap().plus_one_counters, 5.into());
            let before = g.state.rules.frame.clone();
            let resources = g.state.rules.players[0].mana.clone();
            let occurrence = g
                .events()
                .iter()
                .filter(|e| matches!(e, Event::SpellCast { .. }))
                .count(); // no failed proposal is a cast occurrence
            let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
            answer(&mut g, Response::OptionalEffect { perform: true });
            let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
                panic!()
            };
            let Some(PlayerRequest::AdditionalCosts {
                candidates: old_public_costs,
                ..
            }) = observe(&g, PlayerId(0)).decision
            else {
                panic!()
            };
            answer(
                &mut g,
                Response::AdditionalCosts {
                    selected: vec![candidates[0].identity.clone()],
                },
            );
            answer(
                &mut g,
                Response::Targets {
                    objects: vec![target],
                },
            );
            assert!(matches!(request(&mut g), Request::Mana { .. }));
            let signets = g
                .state
                .rules
                .objects
                .values()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && g.definition(o.id.card).unwrap().name == "Golgari Signet"
                })
                .map(|o| o.id)
                .collect::<Vec<_>>();
            answer(&mut g, Response::ActivateMana { object: signets[0] });
            answer(&mut g, Response::ActivateMana { object: ramos });
            answer(&mut g, Response::FinishMana { undo_mana: false });
            answer(
                &mut g,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::U, 1.into())]),
                },
            );
            answer(&mut g, Response::ActivateMana { object: signets[1] });
            answer(&mut g, Response::FinishMana { undo_mana: false });
            answer(
                &mut g,
                Response::SpendMana {
                    spending: BTreeMap::from([(Color::U, 1.into())]),
                },
            );
            assert!(matches!(request(&mut g), Request::Mana { .. }));
            assert_eq!(g.mana(PlayerId(0), Color::U).unwrap_or_default(), 0.into());
            assert!(
                !g.can_pay(PlayerId(0), &g.top().unwrap().total_cost)
                    .unwrap()
            );
            prove(&g);
            answer(&mut g, Response::FinishMana { undo_mana: undo });
            assert!(matches!(request(&mut g), Request::OptionalEffect { .. }));
            assert_eq!(g.state.rules.frame, before);
            assert_eq!(
                g.events()
                    .iter()
                    .filter(|e| matches!(e, Event::SpellCast { .. }))
                    .count(),
                occurrence
            );
            assert!(g.state.rules.pending_triggers.is_empty());
            assert_eq!(
                g.current(ramos).unwrap().plus_one_counters,
                if undo { 5.into() } else { 0.into() }
            );
            assert_eq!(g.mana_ability_used(ramos), !undo);
            assert!(
                signets
                    .iter()
                    .all(|s| g.current(*s).unwrap().tapped == !undo)
            );
            if undo {
                assert_eq!(g.state.rules.players[0].mana, resources);
            }
            prove(&g);
            answer(&mut g, Response::OptionalEffect { perform: true });
            request(&mut g);
            let Some(PlayerRequest::AdditionalCosts {
                token,
                candidates: new_public_costs,
                ..
            }) = observe(&g, PlayerId(0)).decision
            else {
                panic!()
            };
            assert_ne!(old_public_costs[0].id, new_public_costs[0].id);
            assert!(matches!(
                g.check_player_response(
                    PlayerId(0),
                    token,
                    PlayerResponse::AdditionalCosts {
                        selected: vec![old_public_costs[0].id.clone()]
                    },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(_)
            ));
            finish_announcement(&mut g, false, Some(target), None);
            assert!(g.state.rules.stack.iter().any(|s| s.kind.is_spell()
                && g.definition(s.source.card).unwrap().name == "Dismantling Blow"
                && s.selected_additional_costs == Some(vec![])
                && s.paid == Some(zero_mana_cost())));
            prove(&g);
        }
    }
}
fn advance_to_turn(g: &mut Game, player: PlayerId, minimum_turn: u64) {
    for _ in 0..300 {
        let r = request(g);
        if g.state.rules.turn_owner == player
            && g.state.rules.turn.number >= minimum_turn
            && g.state.rules.turn.step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { .. })
        {
            at_priority(g, player);
            prove(g); // Replays the complete natural-turn accepted-input prefix.
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            Request::Discard {
                candidates, count, ..
            } => Response::Discard {
                objects: candidates.into_iter().take(count as usize).collect(),
            },
            other => panic!("natural turn {other:?}"),
        };
        answer(g, response);
    }
    panic!("natural turn advance");
}
#[test]
fn m3c_paid_instant_cascade_prohibition_returns_current_controller_and_paired_allow() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for remove_aura in [false, true] {
            let mut p = source_prepared("Kutzil, Malamet Exemplar", &["Forest", "Forest"]);
            replace_spare(
                &mut p,
                PlayerId(0),
                "Dismantling Blow",
                Zone::Hand { owner: PlayerId(0) },
            );
            for name in [
                "Island", "Island", "Island", "Island", "Forest", "Forest", "Forest", "Forest",
                "Forest", "Forest", "Forest", "Forest",
            ] {
                replace_spare(&mut p, PlayerId(1), name, Zone::Battlefield);
            }
            replace_spare(
                &mut p,
                PlayerId(1),
                "Control Magic",
                Zone::Hand { owner: PlayerId(1) },
            );
            replace_spare(
                &mut p,
                PlayerId(1),
                "Natural Reclamation",
                Zone::Hand { owner: PlayerId(1) },
            );
            // B's real next-turn draw consumes one Forest. Cascade later scans
            // the sole eligible Bear; at most one remainder
            // remains. No arbitrary midgame random seed is introduced.
            let bear = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Grizzly Bears")
                .unwrap()
                .oracle_id
                .clone();
            let third = p
                .config
                .cards
                .iter_mut()
                .filter(|c| {
                    c.owner == PlayerId(1) && c.zone == Zone::Library { owner: PlayerId(1) }
                })
                .nth(1)
                .unwrap();
            third.oracle_id = bear;
            // Move A's unused physical spare basics into its library before play,
            // preserving all 100 owners and avoiding an invented cleanup hand.
            let forest = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Forest")
                .unwrap()
                .oracle_id
                .clone();
            for c in &mut p.config.cards {
                if c.owner == PlayerId(0)
                    && c.oracle_id == forest
                    && c.zone == (Zone::Hand { owner: PlayerId(0) })
                {
                    c.zone = Zone::Library { owner: PlayerId(0) };
                }
            }
            let mut g = Game::with_trace(finish_prepared(p), mode);
            let _kutzil_cast = cast(
                &mut g,
                PlayerId(0),
                "Kutzil, Malamet Exemplar",
                false,
                None,
                None,
            );
            resolve_to_empty(&mut g);
            let kutzil = named(
                &g,
                "Kutzil, Malamet Exemplar",
                PlayerId(0),
                Zone::Battlefield,
            );
            assert!(g.cast_is_prohibited(PlayerId(1)).unwrap());
            let initial_turn = g.state.rules.turn.number;
            advance_to_turn(&mut g, PlayerId(1), initial_turn + 1);
            cast(
                &mut g,
                PlayerId(1),
                "Control Magic",
                false,
                Some(kutzil),
                None,
            );
            resolve_to_empty(&mut g);
            assert_eq!(g.current(kutzil).unwrap().controller, PlayerId(1));
            advance_to_turn(&mut g, PlayerId(0), initial_turn + 4);
            assert!(!g.cast_is_prohibited(PlayerId(1)).unwrap());
            let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
            let original = cast(
                &mut g,
                PlayerId(1),
                "Natural Reclamation",
                false,
                Some(target),
                None,
            );
            let cascade = g.top().unwrap().id;
            assert!(matches!(g.top().unwrap().kind, StackKind::Triggered { .. }));
            if remove_aura {
                let aura = named(&g, "Control Magic", PlayerId(1), Zone::Battlefield);
                let blow = cast(
                    &mut g,
                    PlayerId(0),
                    "Dismantling Blow",
                    false,
                    Some(aura),
                    None,
                );
                for _ in 0..12 {
                    request(&mut g);
                    if !g.state.rules.stack.iter().any(|s| s.id == blow) {
                        break;
                    }
                    answer(&mut g, Response::Pass);
                    prove(&g);
                }
                assert_eq!(g.current(kutzil).unwrap().controller, PlayerId(0));
                assert!(g.cast_is_prohibited(PlayerId(1)).unwrap());
                assert_eq!(
                    g.state
                        .rules
                        .stack
                        .iter()
                        .find(|s| s.id == cascade)
                        .unwrap()
                        .controller,
                    PlayerId(1)
                );
                assert_eq!(g.state.rules.cards[&kutzil.card].owner, PlayerId(0));
                // The trigger runs and exiles the actual hit, but cannot offer an
                // illegal child. The original remains cast and resolves normally.
                resolve_to_empty(&mut g);
                assert!(
                    !g.state
                        .rules
                        .objects
                        .values()
                        .any(|o| o.zone == Zone::Battlefield
                            && g.definition(o.id.card).unwrap().name == "Grizzly Bears")
                );
            } else {
                to_offer(&mut g);
                let r = request(&mut g);
                assert_eq!(r.player(), PlayerId(1));
                answer(&mut g, Response::OptionalEffect { perform: true });
                finish_announcement(&mut g, false, None, None);
                assert!(g.state.rules.stack.iter().any(|s| s.kind.is_spell()
                    && s.controller == PlayerId(1)
                    && g.definition(s.source.card).unwrap().name == "Grizzly Bears"));
                resolve_to_empty(&mut g);
            }
            assert!(!g.state.rules.stack.iter().any(|s| s.id == original));
            assert_eq!(
                g.zone(g.object_id(target.card).unwrap()),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            prove(&g);
        }
    }
}
#[test]
fn m3c_selection_public_ids_work_budget_prefixes_stale_foreign_and_rehashed_forgery() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(
            source_prepared("Bloodbraid Elf", &["Dismantling Blow", "Forest", "Island"]),
            mode,
        );
        cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
        to_offer(&mut g);
        answer(&mut g, Response::OptionalEffect { perform: true });
        request(&mut g);
        prove(&g);
        let before = g.state.clone();
        let own = observe(&g, PlayerId(0));
        let Some(PlayerRequest::AdditionalCosts {
            token, candidates, ..
        }) = own.decision
        else {
            panic!()
        };
        assert_eq!(candidates.len(), 1);
        let response = PlayerResponse::AdditionalCosts {
            selected: vec![candidates[0].id.clone()],
        };
        for viewer in [PlayerId(1), PlayerId(2), PlayerId(3)] {
            assert!(observe(&g, viewer).decision.is_none());
            assert!(matches!(
                g.check_player_response(
                    viewer,
                    token.clone(),
                    response.clone(),
                    Budget::unlimited()
                ),
                QueryResult::Rejected(_)
            ));
        }
        for selected in [
            vec!["foreign-cost".into()],
            vec![candidates[0].id.clone(), candidates[0].id.clone()],
        ] {
            assert!(matches!(
                g.check_player_response(
                    PlayerId(0),
                    token.clone(),
                    PlayerResponse::AdditionalCosts { selected },
                    Budget::unlimited()
                ),
                QueryResult::Rejected(_)
            ));
        }
        let QueryResult::Incomplete { work, .. } =
            g.check_player_response(PlayerId(0), token.clone(), response.clone(), Budget::new(1))
        else {
            panic!()
        };
        let saved = g.save_work(&work).unwrap();
        let work = g.restore_work(&saved).unwrap();
        let QueryResult::Complete(QueryValue::Validated(validated)) =
            g.resume_work(work, Budget::unlimited())
        else {
            panic!()
        };
        assert_eq!(g.state, before);
        let mut zero = g.fork();
        zero.advance(Budget::new(0));
        assert_eq!(zero.state, g.state);
        g.submit(validated).unwrap();
        prove(&g);
        let mut forged_pending = g.fork();
        forged_pending
            .state
            .rules
            .pending
            .as_mut()
            .unwrap()
            .response = Response::AdditionalCosts {
            selected: vec!["foreign-cost".into()],
        };
        assert!(
            Game::restore(
                forged_pending.prepared.as_ref().clone(),
                &forged_pending.snapshot().unwrap()
            )
            .is_err()
        );
        assert!(matches!(
            g.check_player_response(PlayerId(0), token, response, Budget::unlimited()),
            QueryResult::Rejected(_)
        ));
        let mut unlimited = g.fork();
        let expected = match unlimited.advance(Budget::unlimited()) {
            Advance::AwaitingDecision(r) => r,
            other => panic!("{other:?}"),
        };
        assert_eq!(request(&mut g), expected);
        assert_eq!(g.state, unlimited.state);
        prove(&g);
        let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
        answer(
            &mut g,
            Response::Targets {
                objects: vec![target],
            },
        );
        request(&mut g);
        prove(&g);
        for mutation in 0..6 {
            let mut forged = g.fork();
            let top = forged.state.rules.stack.last_mut().unwrap();
            match mutation {
                0 => top.selected_additional_costs = None,
                1 => top.selected_additional_costs = Some(vec!["foreign".into()]),
                2 => {
                    let id = top.selected_additional_costs.as_ref().unwrap()[0].clone();
                    top.selected_additional_costs = Some(vec![id.clone(), id]);
                }
                3 => top.selected_additional_costs = Some(vec![]),
                4 => top.total_cost.generic = 19.into(),
                5 => top.alternative_cost = None,
                _ => unreachable!(),
            }
            assert!(
                Game::restore(
                    forged.prepared.as_ref().clone(),
                    &forged.snapshot().unwrap()
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
        finish_announcement(&mut g, true, Some(target), None);
        prove(&g);
        let mut forged = g.fork();
        let top = forged.state.rules.stack.last_mut().unwrap();
        top.total_cost.generic = 7.into();
        top.paid = Some(top.total_cost.clone());
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
fn m3c_selected_kicker_foreign_target_owner_departure_and_caster_departure_recover() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        for caster_departs in [false, true] {
            let mut p =
                source_prepared("Bloodbraid Elf", &["Dismantling Blow", "Forest", "Island"]);
            let signet = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == "Arcane Signet")
                .unwrap()
                .oracle_id
                .clone();
            let target = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.oracle_id == signet && c.zone == Zone::Battlefield)
                .unwrap();
            let target_id = target.id;
            target.owner = PlayerId(1);
            target.controller = PlayerId(1);
            let swap = p
                .config
                .cards
                .iter_mut()
                .find(|c| c.owner == PlayerId(1) && matches!(c.zone, Zone::Library { .. }))
                .unwrap();
            swap.owner = PlayerId(0);
            swap.controller = PlayerId(0);
            swap.zone = Zone::Hand { owner: PlayerId(0) };
            let mut g = Game::with_trace(finish_prepared(p), mode);
            cast(&mut g, PlayerId(0), "Bloodbraid Elf", false, None, None);
            to_offer(&mut g);
            answer(&mut g, Response::OptionalEffect { perform: true });
            let Request::AdditionalCosts { candidates, .. } = request(&mut g) else {
                panic!()
            };
            if caster_departs {
                g.concede(PlayerId(0)).unwrap();
                request(&mut g);
                prove(&g);
                assert!(
                    !g.state
                        .rules
                        .stack
                        .iter()
                        .any(|s| s.controller == PlayerId(0))
                );
                continue;
            }
            answer(
                &mut g,
                Response::AdditionalCosts {
                    selected: vec![candidates[0].identity.clone()],
                },
            );
            let target_object = g.object_id(target_id).unwrap();
            answer(
                &mut g,
                Response::Targets {
                    objects: vec![target_object],
                },
            );
            request(&mut g);
            let before = g.state.rules.libraries[&PlayerId(0)].len();
            g.concede(PlayerId(1)).unwrap();
            request(&mut g);
            prove(&g);
            finish_announcement(&mut g, true, None, None);
            resolve_to_empty(&mut g);
            assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), before);
            prove(&g);
        }
    }
}
#[test]
fn m3c_linked_quantity_draw_suspended_prefix_and_commander_replacement_recover() {
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut p = source_prepared("Dismantling Blow", &["Forest", "Island"]);
        let commander = p.config.commanders[0];
        p.config
            .cards
            .iter_mut()
            .find(|c| c.id == commander)
            .unwrap()
            .zone = Zone::Library { owner: PlayerId(0) };
        let mut g = Game::with_trace(finish_prepared(p), mode);
        let target = named(&g, "Arcane Signet", PlayerId(0), Zone::Battlefield);
        cast(
            &mut g,
            PlayerId(0),
            "Dismantling Blow",
            true,
            Some(target),
            None,
        );
        let mut replacement = false;
        let mut progress = false;
        let mut forged_progress_checked = false;
        for _ in 0..80 {
            let mut zero = g.fork();
            zero.advance(Budget::new(0));
            assert_eq!(zero.state, g.state);
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => {
                    progress |= matches!(&g.state.rules.frame,Frame::Resolve {frame} if frame.draw_progress.is_some());
                    prove(&g);
                    if !forged_progress_checked {
                        let mut forged = g.fork();
                        if let Frame::Resolve { frame } = &mut forged.state.rules.frame
                            && let Some(draw) = &mut frame.draw_progress
                            && draw.remaining > Quantity::zero()
                        {
                            // Keep the declared total unchanged: an invented
                            // successful attempt still needs replay evidence.
                            draw.completed.add(&1.into());
                            draw.remaining.subtract(&1.into());
                            assert!(
                                Game::restore(
                                    forged.prepared.as_ref().clone(),
                                    &forged.snapshot().unwrap()
                                )
                                .is_err()
                            );
                            forged_progress_checked = true;
                        }
                    }
                }
                Advance::AwaitingDecision(Request::Priority { .. }) => {
                    if g.state.rules.stack.is_empty() {
                        break;
                    }
                    answer(&mut g, Response::Pass);
                    prove(&g);
                }
                Advance::AwaitingDecision(Request::CommanderReturn { .. }) => {
                    replacement = true;
                    answer(&mut g, Response::CommanderReturn { to_command: true });
                    prove(&g);
                }
                other => panic!("quantity draw {other:?}"),
            }
        }
        assert!(replacement && progress && forged_progress_checked);
        assert!(g.state.rules.stack.is_empty());
        assert_eq!(
            g.zone(g.object_id(commander).unwrap()),
            Some(Zone::Command { owner: PlayerId(0) })
        );
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(), 1);
        prove(&g);
    }
}
