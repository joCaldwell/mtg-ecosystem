//! Query predictions from the named fixture board, checked against derived and permitted facts.
use super::damage_controller_discard_tests::{answer, settle, to_step};
use super::*;
use mtg_contracts::{CreaturePowerFilter, ObjectSet, PermanentFilter, SacrificeFilter};
use mtg_core::testkit::query::{self, ObjectFacts, Tri};

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
    let mut catalog = FixtureCatalog::Witness.get().clone();
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
    {
        let selected_answer = game
            .next_test_decision()
            .attack(vec![(attacker, PlayerId(1))]);
        answer(&mut game, selected_answer)
    };
    loop {
        match settle(&mut game) {
            Decision {
                reason: Reason::Blockers,
                ..
            } => {
                let selected_answer = game.next_test_decision().block(vec![(blocker, attacker)]);
                answer(&mut game, selected_answer)
            }
            Decision {
                reason: Reason::Priority,
                ..
            } if game.state.rules.turn.step == TurnStep::DeclareBlockers => {
                break;
            }
            Decision {
                reason: Reason::Priority,
                ..
            } => {
                let selected_answer = Answer::Pick {
                    picks: vec![vec![0]],
                };
                answer(&mut game, selected_answer)
            }
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

// Each seat has twelve slots. The extra Anthem is 48 and the borrowed
// Dreadmaw is 49. Expected sets are fixed board positions, independent of
// either implementation's reads of characteristics, control, layers or zones.
type Slots = u64;
const fn slots(ids: &[u8]) -> Slots {
    let mut result = 0;
    let mut i = 0;
    while i < ids.len() {
        result |= 1 << ids[i];
        i += 1;
    }
    result
}
const CREATURES: Slots = slots(&[0, 1, 2, 5, 12, 13, 14, 17, 24, 25, 26, 29, 36, 38, 41, 49]);
const LANDS: Slots = slots(&[4, 5, 16, 17, 28, 29, 40, 41]);
const ARTIFACTS: Slots = slots(&[3, 15, 27, 39]);
const ENCHANTMENTS: Slots = 1 << 48;
const PERMANENTS: Slots = CREATURES | LANDS | ARTIFACTS | ENCHANTMENTS;
const FLYING: Slots = slots(&[1, 13, 25]); // seat 3's Crow is phased out
const GREEN: Slots = CREATURES & !FLYING;
const STRONG: Slots = slots(&[0, 2, 5, 14, 24, 26, 38, 49]); // Anthem and counters included
const TAPPED: Slots = slots(&[2, 27, 28]);
const GRAVEYARD: Slots = slots(&[6, 7, 18, 19, 30, 31, 42, 43]);
const BEAR_SUBTYPE: Slots = slots(&[0, 5, 12, 17, 24, 29, 36, 41]);
const CONTROLLED: [Slots; 4] = [
    slots(&[0, 1, 2, 3, 4, 5, 48]),
    slots(&[12, 13, 14, 15, 16, 17]),
    slots(&[24, 25, 26, 27, 28, 29]),
    slots(&[36, 38, 39, 40, 41, 49]),
];

// The domain inhabitants `object_target_legal_for` selects, without the
// hexproof, shroud, protection and own-source restrictions it adds.
fn core_domain(g: &Game, o: &Object, you: PlayerId, domain: TargetDomain) -> bool {
    match domain {
        TargetDomain::BattlefieldPermanent => o.present_on_battlefield(),
        TargetDomain::ControlledBattlefieldPermanent => {
            o.present_on_battlefield() && o.controller == you
        }
        TargetDomain::GraveyardCard { .. }
        | TargetDomain::OwnGraveyardCard
        | TargetDomain::OwnGraveyardCreatureCard => g.graveyard_target_legal(o, you, domain),
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

fn filters() -> Vec<(PermanentFilter, [Slots; 4])> {
    let mut filters = vec![];
    for (types, included) in [
        (vec![], PERMANENTS),
        (vec![CardKind::Creature], CREATURES),
        (vec![CardKind::Land], LANDS),
        (vec![CardKind::Artifact], ARTIFACTS),
        (vec![CardKind::Enchantment], ENCHANTMENTS),
        (vec![CardKind::Artifact, CardKind::Land], ARTIFACTS | LANDS),
    ] {
        for (non_types, excluded) in [
            (vec![], 0),
            (vec![CardKind::Land], LANDS),
            (vec![CardKind::Creature], CREATURES),
        ] {
            for (non_colors, colors) in [
                (vec![], 0),
                (vec![Color::G], GREEN),
                (vec![Color::U, Color::W], FLYING | ENCHANTMENTS),
            ] {
                for (opponent, tapped) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    for (power, powers) in [
                        (None, PERMANENTS),
                        (
                            Some(CreaturePowerFilter::AtLeast { value: 3.into() }),
                            STRONG,
                        ),
                        (
                            Some(CreaturePowerFilter::AtMost { value: 2.into() }),
                            CREATURES & !STRONG,
                        ),
                    ] {
                        let expected = CONTROLLED.map(|own| {
                            included
                                & !excluded
                                & !colors
                                & powers
                                & if tapped { TAPPED } else { PERMANENTS }
                                & if opponent { !own } else { PERMANENTS }
                        });
                        filters.push((
                            PermanentFilter {
                                types: types.clone(),
                                non_types: non_types.clone(),
                                non_colors: non_colors.clone(),
                                opponent,
                                tapped,
                                power,
                                predicate: None,
                            },
                            expected,
                        ));
                    }
                }
            }
        }
    }
    filters
}

fn sets(own: Slots) -> [(ObjectSet, Slots); 12] {
    [
        (ObjectSet::ControllerCreatures, CREATURES & own),
        (ObjectSet::ControllerPermanents, own),
        (ObjectSet::Creatures, CREATURES),
        (ObjectSet::NonlandPermanents, PERMANENTS & !LANDS),
        (
            ObjectSet::OpponentNonlandPermanents,
            PERMANENTS & !LANDS & !own,
        ),
        (ObjectSet::Enchantments, ENCHANTMENTS),
        (ObjectSet::Artifacts, ARTIFACTS),
        (ObjectSet::GraveyardCards, GRAVEYARD),
        (ObjectSet::AttackingCreatures, 1 << 2),
        (ObjectSet::OpponentCreatures, CREATURES & !own),
        (ObjectSet::FlyingCreatures, FLYING),
        (ObjectSet::NonflyingCreatures, CREATURES & !FLYING),
    ]
}
const DOMAINS: [TargetDomain; 15] = [
    TargetDomain::CreatureOrPlaneswalker,
    TargetDomain::BattlefieldPermanent,
    TargetDomain::ControlledBattlefieldPermanent,
    TargetDomain::ControlledBattlefieldCreature,
    TargetDomain::BattlefieldCreature,
    TargetDomain::BlockingBattlefieldCreature,
    TargetDomain::GraveyardCard {
        opponents_only: false,
    },
    TargetDomain::GraveyardCard {
        opponents_only: true,
    },
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
fn query_matches_expected_board_sets_on_derived_and_permitted_facts() {
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
            ..Default::default()
        },
        SacrificeFilter {
            types: vec![CardKind::Artifact, CardKind::Land],
            subtypes: vec![],
            ..Default::default()
        },
        SacrificeFilter {
            types: vec![],
            subtypes: vec!["Bear".into()],
            ..Default::default()
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
            let bit = 1u64 << o.id.card.0;
            let seat = usize::from(you.0);
            for (f, expected) in &filters {
                counted[0] += 1;
                assert_eq!(
                    g.matches_permanent_filter(o.id, you, f),
                    expected[seat] & bit != 0
                );
                check(
                    expected[seat] & bit != 0,
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
            for (set, expected) in &sets(CONTROLLED[seat]) {
                counted[1] += 1;
                assert_eq!(
                    g.objects_in_set(set, you).contains(&o.id),
                    expected & bit != 0
                );
                check(
                    expected & bit != 0,
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
            for (filter, expected) in
                sacrifices
                    .iter()
                    .zip([CREATURES, ARTIFACTS | LANDS, BEAR_SUBTYPE])
            {
                assert_eq!(
                    g.sacrifice_candidates(you, filter).contains(&o.id),
                    expected & CONTROLLED[seat] & bit != 0
                );
                check(
                    expected & CONTROLLED[seat] & bit != 0,
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
        predicate: None,
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
        entry_selection: None,
        faces: None,
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

/// Every closed legacy set and its kernel predicate set name the same
/// objects, to every reader: the core's derived facts and each seat's view.
#[test]
fn kernel_sets_answer_exactly_as_the_closed_sets_they_replace() {
    let Board { game: g, .. } = board();
    let views = views(&g);
    let sets = [
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
    let mut members = 0usize;
    for o in g.state.rules.objects.values() {
        let derived = g.object_facts(o);
        for you in (0..4).map(PlayerId) {
            for set in &sets {
                for context in [Context::Spell, Context::Activated, Context::Triggered] {
                    let SetExpr::Matching { zone, filter } = set.kernel(context) else {
                        panic!("closed sets are zone sets");
                    };
                    let legacy = query::matches_object_set(&derived, you, set);
                    let kernel = query::matches_zone_pred(&derived, you, &zone, &filter);
                    assert_eq!(kernel, legacy, "{set:?} for {:?} as {you:?}", o.id);
                    assert_ne!(kernel, Tri::Unknown, "the core knows every fact");
                    members += usize::from(kernel.is_yes());
                    for view in &views {
                        let Ok(handle) = g.handle_for(view.viewer, o.id) else {
                            continue;
                        };
                        let Some(visible) = view.objects.iter().find(|v| v.handle == handle) else {
                            continue;
                        };
                        let seen = query::in_view(view, visible);
                        assert_eq!(
                            query::matches_zone_pred(&seen, you, &zone, &filter),
                            query::matches_object_set(&seen, you, set),
                            "{set:?} in {:?}'s view of {:?}",
                            view.viewer,
                            o.id
                        );
                    }
                }
            }
        }
    }
    assert!(members > 100, "the board inhabits the sets: {members}");
}

/// General filter atoms read the same derived facts the rules read.
#[test]
fn kernel_filter_atoms_follow_derived_characteristics() {
    use mtg_contracts::kir::{Amount, Atom, Cmp, Pred, ZoneExpr};
    let Board { game: g, .. } = board();
    let you = PlayerId(0);
    let battlefield = ZoneExpr::Battlefield;
    let matches = |o: &Object, atoms: Vec<Atom>| {
        query::matches_zone_pred(&g.object_facts(o), you, &battlefield, &Pred(atoms)).is_yes()
    };
    let mut seen = [0usize; 8];
    for o in g.state.rules.objects.values() {
        let present = o.zone == Zone::Battlefield && o.phased_out.is_none();
        let c = g.characteristics(o.id).unwrap();
        let creature = c.has_type(CardKind::Creature);
        let power = |n: i64| c.power.as_ref().is_some_and(|p| *p >= Quantity::from(n));
        let cases: [(Vec<Atom>, bool); 8] = [
            // CR 208.1: only creatures have power.
            (
                vec![Atom::Power {
                    cmp: Cmp::AtLeast,
                    value: Amount::fixed(3),
                }],
                creature && power(3),
            ),
            (
                vec![Atom::Toughness {
                    cmp: Cmp::AtMost,
                    value: Amount::fixed(2),
                }],
                creature
                    && c.toughness
                        .as_ref()
                        .is_some_and(|t| *t <= Quantity::from(2)),
            ),
            (
                vec![Atom::ManaValue {
                    cmp: Cmp::Exactly,
                    value: Amount::fixed(1),
                }],
                c.printed_mana_value() == Quantity::from(1),
            ),
            (
                vec![Atom::Color { color: Color::G }],
                c.colors.contains(&Color::G),
            ),
            (
                vec![Atom::AnyType {
                    kinds: vec![CardKind::Artifact, CardKind::Land],
                }],
                c.has_type(CardKind::Artifact) || c.has_type(CardKind::Land),
            ),
            (
                vec![
                    Atom::Type {
                        kind: CardKind::Creature,
                    },
                    Atom::NotType {
                        kind: CardKind::Land,
                    },
                    Atom::Untapped,
                ],
                creature && !c.has_type(CardKind::Land) && !o.tapped,
            ),
            (
                vec![Atom::Subtype {
                    subtype: "Bear".into(),
                }],
                c.subtypes.iter().any(|s| s == "Bear"),
            ),
            (
                vec![
                    Atom::NotToken,
                    Atom::ControlledBy {
                        player: PlayerExpr::EachOpponent,
                    },
                ],
                o.controller != you,
            ),
        ];
        for (index, (atoms, expected)) in cases.into_iter().enumerate() {
            let expected = present && expected;
            assert_eq!(matches(o, atoms), expected, "case {index} for {:?}", o.id);
            seen[index] += usize::from(expected);
        }
    }
    assert!(
        seen.iter().all(|n| *n > 0),
        "every atom has a member: {seen:?}"
    );
}
