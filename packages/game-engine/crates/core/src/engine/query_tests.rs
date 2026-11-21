//! Differential witness for `crate::query`: the retired core evaluators, the
//! shared predicates over derived state, and the same predicates over every
//! seat's permitted view answer alike on one board. Archived cards are
//! witnesses only; the creature land is a synthetic multitype definition.
use super::damage_controller_discard_tests::{answer, settle, to_step};
use super::*;
use crate::query::{self, ObjectFacts, Tri};
use mtg_contracts::{CreaturePowerFilter, ObjectSet, PermanentFilter, SacrificeFilter};

const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const CROW: &str = "000d5588-5a4c-434e-988d-396632ade42c";
const DREADMAW: &str = "08c7db90-c0cf-4482-b7ee-bb033e5996d2";
const SOL_RING: &str = "6ad8011d-3471-4369-9d68-b264cc027487";
const ANTHEM: &str = "e3886fe8-9b76-4613-8891-4ec74657c087";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const CREATURE_LAND: &str = "ffffffff-0000-4000-8000-0000000000b0";

struct Board {
    game: Game,
    creature_land: ObjectId,
    grown: ObjectId,
    phased: ObjectId,
    attacker: ObjectId,
    blocker: ObjectId,
}

fn board() -> Board {
    let mut catalog = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    // CR 205.2b: a creature that is also a land. Its execution type is
    // creature; the land type is an additional card type.
    let mut arbor = catalog.definition_by_oracle_id(BEARS).unwrap().clone();
    arbor.oracle_id = CREATURE_LAND.into();
    arbor.name = "Synthetic Creature Land".into();
    arbor.characteristics.additional_types = vec![CardKind::Land];
    catalog.definitions.push(arbor);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "query-differential-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.cards.clear();
    let mut place = |oracle: &str, owner: u8, controller: u8, zone: Zone| {
        let id = CardId(config.cards.len() as u32);
        config.cards.push(CardSetup {
            id,
            oracle_id: oracle.into(),
            owner: PlayerId(owner),
            controller: PlayerId(controller),
            zone,
        });
        id
    };
    let mut first = vec![];
    for seat in 0..4u8 {
        for oracle in [BEARS, CROW, DREADMAW, SOL_RING, FOREST, CREATURE_LAND] {
            first.push((seat, oracle, place(oracle, seat, seat, Zone::Battlefield)));
        }
        place(
            BEARS,
            seat,
            seat,
            Zone::Graveyard {
                owner: PlayerId(seat),
            },
        );
        place(
            SOL_RING,
            seat,
            seat,
            Zone::Graveyard {
                owner: PlayerId(seat),
            },
        );
        place(
            BOLT,
            seat,
            seat,
            Zone::Hand {
                owner: PlayerId(seat),
            },
        );
        for _ in 0..3 {
            place(
                FOREST,
                seat,
                seat,
                Zone::Library {
                    owner: PlayerId(seat),
                },
            );
        }
    }
    // Derived power differs from printed power, and control from ownership.
    place(ANTHEM, 0, 0, Zone::Battlefield);
    place(DREADMAW, 2, 3, Zone::Battlefield);
    let card = |seat: u8, oracle: &str| {
        first
            .iter()
            .find(|(s, o, _)| *s == seat && *o == oracle)
            .unwrap()
            .2
    };
    let prepared = Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    };
    let mut game = Game::new(prepared);
    to_step(&mut game, 1, TurnStep::DeclareAttackers);
    let attacker = game.object_id(card(0, DREADMAW)).unwrap();
    let blocker = game.object_id(card(1, BEARS)).unwrap();
    answer(
        &mut game,
        Response::Attackers {
            attacks: vec![(attacker, PlayerId(1))],
        },
    );
    loop {
        match settle(&mut game) {
            Request::Blockers { .. } => answer(
                &mut game,
                Response::Blockers {
                    blocks: vec![(blocker, attacker)],
                },
            ),
            Request::Priority { .. } if game.state.rules.turn.step == TurnStep::DeclareBlockers => {
                break;
            }
            Request::Priority { .. } => answer(&mut game, Response::Pass),
            other => panic!("unexpected combat request {other:?}"),
        }
    }
    assert!(game.is_blocking_creature(blocker));
    // Status facts no archived card here supplies: tapped noncreatures, three
    // +1/+1 counters and a phased-out permanent.
    let grown = game.object_id(card(2, BEARS)).unwrap();
    let phased = game.object_id(card(3, CROW)).unwrap();
    let objects = &mut game.state.rules.objects;
    objects.get_mut(&card(2, SOL_RING)).unwrap().tapped = true;
    objects.get_mut(&card(2, FOREST)).unwrap().tapped = true;
    objects.get_mut(&grown.card).unwrap().plus_one_counters = Quantity::from(3);
    objects.get_mut(&phased.card).unwrap().phased_out = Some(phasing::PhaseReturn::Direct {
        controller_at_phase_out: PlayerId(3),
    });
    game.refresh_visibility().unwrap();
    Board {
        creature_land: game.object_id(card(1, CREATURE_LAND)).unwrap(),
        grown,
        phased,
        attacker,
        blocker,
        game,
    }
}

// The evaluators as they stood before `crate::query`, kept as the oracle.
fn retired_filter(g: &Game, object: ObjectId, controller: PlayerId, f: &PermanentFilter) -> bool {
    g.current(object).is_ok_and(|o| {
        o.present_on_battlefield()
            && (!f.opponent || o.controller != controller)
            && (!f.tapped || o.tapped)
    }) && g.color_layer(object).is_ok_and(|c| {
        !matches!(c.kind, CardKind::Instant | CardKind::Sorcery)
            && (f.types.is_empty() || f.types.iter().any(|k| c.has_type(*k)))
            && !f.non_types.iter().any(|k| c.has_type(*k))
            && !c.colors.iter().any(|c| f.non_colors.contains(c))
    }) && f.power.as_ref().is_none_or(|bound| {
        g.characteristics(object).is_ok_and(|c| {
            c.has_type(CardKind::Creature)
                && c.power.as_ref().is_some_and(|power| match bound {
                    CreaturePowerFilter::AtLeast { value } => power >= value,
                    CreaturePowerFilter::AtMost { value } => power <= value,
                })
        })
    })
}

fn retired_set(g: &Game, o: &Object, set: &ObjectSet, controller: PlayerId) -> bool {
    if *set == ObjectSet::GraveyardCards {
        return matches!(o.zone, Zone::Graveyard { .. })
            && !g.state.rules.tokens.contains_key(&o.id.card);
    }
    if !o.present_on_battlefield() {
        return false;
    }
    let Ok(d) = g.copiable_definition(o.id) else {
        return false;
    };
    let creature = g.target_is_creature(o.id);
    match set {
        ObjectSet::GraveyardCards => false,
        ObjectSet::Artifacts => d.characteristics.has_type(CardKind::Artifact),
        ObjectSet::Creatures => creature,
        ObjectSet::NonlandPermanents => !d.characteristics.has_type(CardKind::Land),
        ObjectSet::OpponentNonlandPermanents => {
            o.controller != controller
                && g.is_live(o.controller)
                && !d.characteristics.has_type(CardKind::Land)
        }
        ObjectSet::Enchantments => d.characteristics.has_type(CardKind::Enchantment),
        ObjectSet::AttackingCreatures => {
            creature && g.state.rules.combat.iter().any(|a| a.attacker == o.id)
        }
        ObjectSet::ControllerCreatures => creature && o.controller == controller,
        ObjectSet::ControllerPermanents => o.controller == controller,
        ObjectSet::OpponentCreatures => creature && o.controller != controller,
        ObjectSet::FlyingCreatures => creature && g.has_keyword(o.id, KeywordAbility::Flying),
        ObjectSet::NonflyingCreatures => creature && !g.has_keyword(o.id, KeywordAbility::Flying),
    }
}

fn retired_sacrifice(g: &Game, o: &Object, player: PlayerId, filter: &SacrificeFilter) -> bool {
    o.present_on_battlefield()
        && o.controller == player
        && g.characteristics(o.id).is_ok_and(|c| {
            (filter.types.is_empty() || filter.types.iter().any(|k| c.has_type(*k)))
                && filter.subtypes.iter().all(|s| c.subtypes.contains(s))
        })
}

// The domain inhabitants `object_target_legal_for` selects, without the
// hexproof, shroud, protection and own-source restrictions it adds.
fn core_domain(g: &Game, o: &Object, you: PlayerId, domain: TargetDomain) -> bool {
    match domain {
        TargetDomain::BattlefieldPermanent => o.present_on_battlefield(),
        TargetDomain::ControlledBattlefieldPermanent => {
            o.present_on_battlefield() && o.controller == you
        }
        TargetDomain::OwnGraveyardCard | TargetDomain::OwnGraveyardCreatureCard => {
            g.graveyard_target_legal(o, you, domain)
        }
        TargetDomain::CreatureOrPlaneswalker => g.creature_or_planeswalker(o.id),
        TargetDomain::ControlledBattlefieldCreature => {
            g.is_battlefield_creature(o.id) && o.controller == you
        }
        TargetDomain::BlockingBattlefieldCreature => g.is_blocking_creature(o.id),
        TargetDomain::BattlefieldCreature | TargetDomain::AnyDamageTarget => {
            g.is_battlefield_creature(o.id)
        }
        _ => false,
    }
}

fn filters() -> Vec<PermanentFilter> {
    let mut filters = vec![];
    for types in [
        vec![],
        vec![CardKind::Creature],
        vec![CardKind::Land],
        vec![CardKind::Artifact],
        vec![CardKind::Enchantment],
        vec![CardKind::Artifact, CardKind::Land],
    ] {
        for non_types in [vec![], vec![CardKind::Land], vec![CardKind::Creature]] {
            for non_colors in [vec![], vec![Color::G], vec![Color::U, Color::W]] {
                for (opponent, tapped) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    for power in [
                        None,
                        Some(CreaturePowerFilter::AtLeast { value: 3.into() }),
                        Some(CreaturePowerFilter::AtMost { value: 2.into() }),
                    ] {
                        filters.push(PermanentFilter {
                            types: types.clone(),
                            non_types: non_types.clone(),
                            non_colors: non_colors.clone(),
                            opponent,
                            tapped,
                            power,
                        });
                    }
                }
            }
        }
    }
    filters
}

const SETS: [ObjectSet; 12] = [
    ObjectSet::ControllerCreatures,
    ObjectSet::ControllerPermanents,
    ObjectSet::Creatures,
    ObjectSet::NonlandPermanents,
    ObjectSet::OpponentNonlandPermanents,
    ObjectSet::Enchantments,
    ObjectSet::Artifacts,
    ObjectSet::GraveyardCards,
    ObjectSet::AttackingCreatures,
    ObjectSet::OpponentCreatures,
    ObjectSet::FlyingCreatures,
    ObjectSet::NonflyingCreatures,
];

const DOMAINS: [TargetDomain; 13] = [
    TargetDomain::CreatureOrPlaneswalker,
    TargetDomain::BattlefieldPermanent,
    TargetDomain::ControlledBattlefieldPermanent,
    TargetDomain::ControlledBattlefieldCreature,
    TargetDomain::BattlefieldCreature,
    TargetDomain::BlockingBattlefieldCreature,
    TargetDomain::OwnGraveyardCreatureCard,
    TargetDomain::OwnGraveyardCard,
    TargetDomain::AnyDamageTarget,
    TargetDomain::OpponentPlayer,
    TargetDomain::StackSpell,
    TargetDomain::PlayerOrPlaneswalker,
    TargetDomain::AnyPlayer,
];

fn views(g: &Game) -> Vec<PermittedView> {
    (0..4)
        .map(|seat| {
            let QueryResult::Complete(QueryValue::Observation(o)) =
                g.observe(PlayerId(seat), Budget::unlimited())
            else {
                panic!("observation")
            };
            o.view
        })
        .collect()
}

#[test]
fn query_matches_the_retired_core_evaluators_on_derived_and_permitted_facts() {
    let Board {
        game: g,
        creature_land,
        grown,
        phased,
        attacker,
        blocker,
    } = board();
    let views = views(&g);
    let filters = filters();
    let sacrifices = [
        SacrificeFilter {
            types: vec![CardKind::Creature],
            subtypes: vec![],
        },
        SacrificeFilter {
            types: vec![CardKind::Artifact, CardKind::Land],
            subtypes: vec![],
        },
        SacrificeFilter {
            types: vec![],
            subtypes: vec!["Bear".into()],
        },
    ];
    let keywords = [
        KeywordAbility::Flying,
        KeywordAbility::Trample,
        KeywordAbility::Hexproof,
    ];
    let mut counted = [0usize; 3];
    let mut unproven_blockers = 0usize;
    for o in g.state.rules.objects.values() {
        let derived = g.object_facts(o);
        // Each seat's record of this object, where that seat can see it.
        let seen: Vec<(&PermittedView, &VisibleObject)> = views
            .iter()
            .filter_map(|view| {
                let handle = g.handle_for(view.viewer, o.id).ok()?;
                let object = view.objects.iter().find(|v| v.handle == handle)?;
                Some((view, object))
            })
            .collect();
        if o.zone == Zone::Battlefield {
            assert_eq!(seen.len(), 4, "a permanent is public");
        }
        // A view publishes keywords for some objects only; elsewhere the
        // permitted answer is unknown, never a contradiction.
        for keyword in keywords {
            let core = Tri::from(g.has_keyword(o.id, keyword));
            assert_eq!(derived.has_keyword(keyword), core);
            for (view, object) in &seen {
                for answer in [
                    query::in_view(view, object).has_keyword(keyword),
                    object.has_keyword(keyword),
                ] {
                    assert!(
                        answer == Tri::Unknown || answer == core,
                        "{keyword:?} {answer:?} contradicts {core:?} for {:?} in {:?}",
                        o.id,
                        o.zone
                    );
                }
            }
        }
        for you in (0..4).map(PlayerId) {
            let mut check = |expected: bool,
                             core: Tri,
                             permitted: Vec<(Tri, Tri)>,
                             complete: bool,
                             what: &dyn Fn() -> String| {
                assert_eq!(core, Tri::from(expected), "core {} for {:?}", what(), o.id);
                for (in_view, alone) in permitted {
                    for answer in [in_view, alone] {
                        assert!(
                            answer == Tri::Unknown || answer == core,
                            "permitted {answer:?} contradicts {core:?}: {} for {:?}",
                            what(),
                            o.id
                        );
                    }
                    if complete {
                        assert_eq!(in_view, core, "incomplete {} for {:?}", what(), o.id);
                    } else if in_view == Tri::Unknown {
                        unproven_blockers += 1;
                    }
                }
            };
            for f in &filters {
                counted[0] += 1;
                assert_eq!(
                    g.matches_permanent_filter(o.id, you, f),
                    retired_filter(&g, o.id, you, f)
                );
                check(
                    retired_filter(&g, o.id, you, f),
                    query::matches_permanent_filter(&derived, you, f),
                    seen.iter()
                        .map(|(view, object)| {
                            (
                                query::matches_permanent_filter(
                                    &query::in_view(view, object),
                                    you,
                                    f,
                                ),
                                query::matches_permanent_filter(*object, you, f),
                            )
                        })
                        .collect(),
                    true,
                    &|| format!("filter {f:?} for {you:?}"),
                );
            }
            for set in &SETS {
                counted[1] += 1;
                assert_eq!(
                    g.objects_in_set(set, you).contains(&o.id),
                    retired_set(&g, o, set, you)
                );
                check(
                    retired_set(&g, o, set, you),
                    query::matches_object_set(&derived, you, set),
                    seen.iter()
                        .map(|(view, object)| {
                            (
                                query::matches_object_set(&query::in_view(view, object), you, set),
                                query::matches_object_set(*object, you, set),
                            )
                        })
                        .collect(),
                    true,
                    &|| format!("set {set:?} for {you:?}"),
                );
            }
            for filter in &sacrifices {
                assert_eq!(
                    g.sacrifice_candidates(you, filter).contains(&o.id),
                    retired_sacrifice(&g, o, you, filter)
                );
                check(
                    retired_sacrifice(&g, o, you, filter),
                    query::matches_sacrifice_filter(&derived, you, filter),
                    seen.iter()
                        .map(|(view, object)| {
                            (
                                query::matches_sacrifice_filter(
                                    &query::in_view(view, object),
                                    you,
                                    filter,
                                ),
                                query::matches_sacrifice_filter(*object, you, filter),
                            )
                        })
                        .collect(),
                    true,
                    &|| format!("sacrifice {filter:?} for {you:?}"),
                );
            }
            for domain in DOMAINS {
                counted[2] += 1;
                // Only attacker links prove a blocker to a permitted reader.
                let complete = domain != TargetDomain::BlockingBattlefieldCreature;
                check(
                    core_domain(&g, o, you, domain),
                    query::matches_target_domain(&derived, you, domain),
                    seen.iter()
                        .map(|(view, object)| {
                            (
                                query::matches_target_domain(
                                    &query::in_view(view, object),
                                    you,
                                    domain,
                                ),
                                query::matches_target_domain(*object, you, domain),
                            )
                        })
                        .collect(),
                    complete,
                    &|| format!("domain {domain:?} for {you:?}"),
                );
            }
        }
    }
    assert!(counted.iter().all(|n| *n > 1_000), "{counted:?}");
    assert!(
        unproven_blockers > 0,
        "an unlinked creature's blocking status is unknown"
    );

    // The cases the two retired simulator copies answered differently.
    let object = |id: ObjectId| g.current(id).unwrap();
    let view = &views[0];
    let seen = |id: ObjectId| {
        let handle = g.handle_for(PlayerId(0), id).unwrap();
        query::in_view(
            view,
            view.objects.iter().find(|v| v.handle == handle).unwrap(),
        )
    };
    let you = PlayerId(0);
    // A creature land is a land: it is no nonland permanent (CR 205.2b).
    for set in [
        ObjectSet::NonlandPermanents,
        ObjectSet::OpponentNonlandPermanents,
    ] {
        assert!(!g.objects_in_set(&set, you).contains(&creature_land));
        assert_eq!(
            query::matches_object_set(&seen(creature_land), you, &set),
            Tri::No
        );
    }
    assert_eq!(
        query::matches_object_set(&seen(creature_land), you, &ObjectSet::Creatures),
        Tri::Yes
    );
    // A power bound reads derived power: three counters make a 2/2 a 5/5.
    let strong = PermanentFilter {
        types: vec![CardKind::Creature],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
        power: Some(CreaturePowerFilter::AtLeast { value: 5.into() }),
    };
    assert_eq!(
        query::matches_permanent_filter(&seen(grown), you, &strong),
        Tri::Yes
    );
    assert_eq!(
        query::matches_permanent_filter(&seen(creature_land), you, &strong),
        Tri::No
    );
    assert!(g.matches_permanent_filter(grown, you, &strong));
    // A phased-out permanent is treated as though it does not exist (CR 702.26b).
    assert!(object(phased).phased_out.is_some());
    let any = PermanentFilter {
        power: None,
        types: vec![],
        ..strong.clone()
    };
    assert_eq!(
        query::matches_permanent_filter(&seen(phased), you, &any),
        Tri::No
    );
    assert_eq!(
        query::matches_object_set(&seen(phased), you, &ObjectSet::Creatures),
        Tri::No
    );
    // Combat membership: the attacker is public, and so is its linked blocker.
    assert_eq!(seen(attacker).attacking(), Tri::Yes);
    assert_eq!(seen(blocker).blocking(), Tri::Yes);
    assert_eq!(seen(grown).blocking(), Tri::Unknown);
    assert_eq!(
        query::matches_target_domain(&seen(grown), you, TargetDomain::BlockingBattlefieldCreature),
        Tri::Unknown
    );
    // A hidden object's unpublished characteristics are unknown, never guessed.
    let hidden = VisibleObject {
        characteristics: None,
        ..view
            .objects
            .iter()
            .find(|v| v.zone == Zone::Battlefield)
            .unwrap()
            .clone()
    };
    assert_eq!(
        query::matches_permanent_filter(&hidden, you, &any),
        Tri::Unknown
    );
    assert_eq!(
        query::matches_object_set(&hidden, you, &ObjectSet::Creatures),
        Tri::Unknown
    );
    assert_eq!(hidden.has_keyword(KeywordAbility::Flying), Tri::Unknown);
    // A graveyard card's keywords are not published: unknown, not absent.
    let buried = view
        .objects
        .iter()
        .find(|v| matches!(v.zone, Zone::Graveyard { .. }))
        .unwrap();
    assert_eq!(buried.has_keyword(KeywordAbility::Flying), Tri::Unknown);
    assert_eq!(
        seen(attacker).has_keyword(KeywordAbility::Trample),
        Tri::Yes
    );
    assert_eq!(seen(attacker).has_keyword(KeywordAbility::Flying), Tri::No);
    // A graveyard-group card depends on the player bound to another slot.
    let group = TargetDomain::PlayerGraveyardCards {
        owner_slot: 0,
        maximum: 2,
    };
    assert_eq!(
        query::matches_target_domain(buried, you, group),
        Tri::Unknown
    );
    assert_eq!(
        query::matches_target_domain(&seen(attacker), you, group),
        Tri::No
    );
}
