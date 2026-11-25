use super::*;
use mtg_contracts::canonical;
use mtg_core::{Budget, Game, MatchConfig, Prepared, QueryResult, QueryValue};
#[cfg(test)]
mod reactive_tests {}

#[cfg(test)]
mod board {
    use super::*;
    pub(super) fn view() -> PermittedView {
        let p = Prepared::admit_pod(
            mtg_compiler::catalog::embedded_catalog("match").unwrap(),
            MatchConfig::for_profile("commander-foundations-v1", [3; 32], PlayerId(3)).unwrap(),
        )
        .unwrap();
        let mut g = Game::new(p);
        g.advance(Budget::unlimited());
        let QueryResult::Complete(QueryValue::Observation(o)) =
            g.observe(PlayerId(3), Budget::unlimited())
        else {
            panic!("view")
        };
        let mut v = o.view;
        v.objects.clear();
        v.stack.clear();
        v.commanders.clear();
        v
    }
    pub(super) fn card(c: &Catalog, name: &str, owner: u8, zone: Zone) -> VisibleObject {
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        VisibleObject {
            modal_faces: None,
            face_down: false,
            exiled_by: None,
            phased_out: false,
            entered_this_turn: false,
            mana_ability_used: None,
            attached_to: None,
            attachments: vec![],
            keywords: d.keywords.clone(),
            granted_mana: vec![],
            restrictions: vec![],
            token: None,
            plus_one_counters: 0.into(),
            characteristics: Some(d.characteristics.clone()),
            marked_damage: 0.into(),
            attack_ready: true,
            handle: ObjectHandle(name.into()),
            oracle_id: d.oracle_id.clone(),
            owner: PlayerId(owner),
            controller: PlayerId(owner),
            zone,
            tapped: false,
        }
    }
}

fn strategy(c: &Catalog) -> Strategy<'_> {
    Strategy::new(c, Policy::Develop, [0; 4])
}
fn pick_fixture(
    player: PlayerId,
    reason: Reason<Viewer>,
    candidates: Vec<Candidate<Viewer>>,
    roles: Vec<mtg_core::SlotRole>,
    rules: Vec<JointRule>,
) -> Decision<Viewer> {
    let allowed: Vec<_> = (0..candidates.len() as u32).collect();
    let slots = roles
        .into_iter()
        .map(|role| mtg_core::Slot {
            role,
            allowed: allowed.clone(),
            subject: None,
            min: 1,
            max: 1,
            repeat: false,
        })
        .collect();
    Decision {
        id: mtg_core::DecisionToken("policy-fixture".into()),
        player,
        reason,
        candidates,
        shape: Shape::Pick { slots, rules },
    }
}
fn targets(
    player: PlayerId,
    specs: &[mtg_contracts::TargetSpec],
    objects: Vec<ObjectHandle>,
) -> Decision<Viewer> {
    pick_fixture(
        player,
        Reason::Targets {
            specs: specs.to_vec(),
        },
        objects
            .into_iter()
            .map(|object| Candidate::Object { object })
            .collect(),
        (0..specs.len() as u32)
            .map(|spec| mtg_core::SlotRole::Target { spec })
            .collect(),
        vec![],
    )
}
fn observation(view: PermittedView) -> Observation {
    Observation {
        observation_version: mtg_core::formats::OBSERVATION_VERSION,
        decision_context: None,
        decision: None,
        view,
    }
}
fn stack_spell(v: &mut PermittedView, c: &Catalog, name: &str, controller: u8) {
    let o = board::card(c, name, controller, Zone::Stack);
    v.stack.push(VisibleStackEntry {
        mode_mask: None,
        spell_copy: false,
        flashback: false,
        hybrid_colors: vec![],
        selected_additional_costs: None,
        x: None,
        mana_value: None,
        triggered: None,
        entity_targets: vec![],
        handle: Some(StackHandle(name.into())),
        stack_targets: vec![],
        spell: Some(o.handle.clone()),
        ability: None,
        player_targets: vec![],
        controller: PlayerId(controller),
        cast_complete: true,
        targets: vec![],
    });
    v.objects.push(o);
}

#[test]
fn selective_set_damage_uses_the_stack_controller_and_scores_lethal_recipients() {
    for (players, hits_controller, score) in [
        (kir::PlayerExpr::EachOpponent, false, i64::MAX),
        (kir::PlayerExpr::you(Context::Spell), true, i64::MIN),
    ] {
        let mut catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let card = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Lightning Bolt")
            .unwrap();
        let program = card.spell.as_mut().unwrap();
        program.targets.clear();
        program.instructions = vec![I::damage(
            kir::DamageTo::Players { players },
            kir::Amount::fixed(2),
            0,
        )];
        let mut view = board::view();
        for player in &mut view.players {
            player.life = 2.into();
        }
        let controller = PlayerId(0);
        stack_spell(&mut view, &catalog, "Lightning Bolt", controller.0);
        let policy = strategy(&catalog);
        let target = |player| PlayerTargetEntity::Player { player };
        assert_eq!(
            policy.incoming_damage(&view, &target(controller)),
            Quantity::from(if hits_controller { 2 } else { 0 })
        );
        assert_eq!(
            policy.incoming_damage(&view, &target(view.viewer)),
            Quantity::from(if hits_controller { 0 } else { 2 })
        );
        let (_, program) = policy.top_program(&view).unwrap();
        assert_eq!(
            policy.wipe_score(&view, program, &0.into()),
            Some(score.into())
        );
    }
}

#[test]
fn tap_cantrip_policy_consumes_admitted_program_and_offered_candidates() {
    let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    let mut v = board::view();
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::W, 2.into());
    let mut enemy = board::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("enemy-isamaru".into());
    let mut own = board::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
    own.handle = ObjectHandle("own-isamaru".into());
    v.objects = vec![
        board::card(
            &c,
            "Pressure Point",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
        enemy,
        own,
    ];
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Pressure Point")
        .unwrap();
    let p = d.spell.as_ref().unwrap();
    let before = canonical(&v).unwrap();
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        Action::Cast {
            card: ObjectHandle("Pressure Point".into())
        }
    );
    assert_eq!(
        strategy(&c).object_target(&v, d, p, &p.targets[0], None),
        Some(ObjectHandle("enemy-isamaru".into()))
    );
    assert_eq!(
        strategy(&c).object_target(
            &v,
            d,
            p,
            &p.targets[0],
            Some(&[ObjectHandle("own-isamaru".into())])
        ),
        None
    );
    assert_eq!(canonical(&v).unwrap(), before);
    v.objects[1].tapped = true;
    assert!(
        strategy(&c).definition_ready(&v, d),
        "the cantrip remains legal on an already-tapped creature"
    );
    v.objects[1].zone = Zone::Graveyard { owner: PlayerId(0) };
    assert!(!strategy(&c).definition_ready(&v, d));
}

#[test]
fn mana_planner_preserves_flexible_source_for_the_second_color() {
    let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    let mut v = board::view();
    v.objects = vec![
        board::card(&c, "Birds of Paradise", v.viewer.0, Zone::Battlefield),
        board::card(&c, "Plains", v.viewer.0, Zone::Battlefield),
    ];
    let plan = mana_policy::plan_for(
        &c,
        &v,
        &ManaCost {
            generic: 0.into(),
            colored: vec![Color::W, Color::U],
        },
        None,
    )
    .unwrap();
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].source, ObjectHandle("Plains".into()));
    assert_eq!(plan[1].source, ObjectHandle("Birds of Paradise".into()));
    assert_eq!(plan[1].color, Some(Color::U));
}

#[test]
fn optional_effect_uses_the_bound_target_slot_and_entity_targets_spread() {
    let mut c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    let mut v = board::view();
    v.objects = vec![board::card(&c, "Storm Crow", 0, Zone::Battlefield)];
    stack_spell(&mut v, &c, "Lightning Bolt", 3);
    let bolt = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Lightning Bolt")
        .unwrap();
    let p = bolt.spell.as_mut().unwrap();
    p.instructions = vec![
        I::pump(kir::SetExpr::target(0), 1.into(), 1.into(), 0),
        I::optional_destroy_target(kir::Context::Spell, 1, 0),
    ];
    v.stack.last_mut().unwrap().targets = vec![
        mtg_core::VisibleTarget {
            announced: ObjectHandle("gone".into()),
            current: None,
        },
        mtg_core::VisibleTarget {
            announced: ObjectHandle("Storm Crow".into()),
            current: Some(ObjectHandle("Storm Crow".into())),
        },
    ];
    assert!(strategy(&c).optional_effect(&v, None));
    let bolt = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Lightning Bolt")
        .unwrap();
    let p = bolt.spell.as_mut().unwrap();
    p.instructions = vec![
        I::damage_target(0, kir::Amount::fixed(3), 0),
        I::damage_target(1, kir::Amount::fixed(3), 0),
    ];
    let mut second = p.targets[0].clone();
    second.slot = 1;
    second.selection_id = "second".into();
    p.targets.push(second);
    let mut o = observation(v);
    o.decision = Some(pick_fixture(
        o.view.viewer,
        Reason::Targets {
            specs: p.targets.clone(),
        },
        vec![
            Candidate::Player {
                player: PlayerId(0),
            },
            Candidate::Player {
                player: PlayerId(1),
            },
        ],
        vec![
            mtg_core::SlotRole::Target { spec: 0 },
            mtg_core::SlotRole::Target { spec: 1 },
        ],
        vec![JointRule::Distinct { slots: vec![0, 1] }],
    ));
    let Answer::Pick { picks } = strategy(&c).choose(&o).unwrap() else {
        panic!("expected indexed targets")
    };
    assert_eq!(picks.len(), 2);
    assert!(picks.iter().all(|pick| pick.len() == 1 && pick[0] < 2));
    assert_ne!(picks[0], picks[1]);
}

#[test]
fn generic_policy_develops_flash_at_opponents_end_and_chooses_legends() {
    let mut c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    let mut v = board::view();
    let viewer = v.viewer;
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    d.keywords.push(mtg_contracts::KeywordAbility::Flash);
    d.spell.as_mut().unwrap().timing = mtg_contracts::SpellTiming::Instant;
    d.characteristics.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![],
    };
    d.capabilities = d.requirements();
    d.validate().unwrap();
    v.active = Some(PlayerId((viewer.0 + 1) % 4));
    v.turn.step = TurnStep::End;
    v.objects = vec![board::card(
        &c,
        "Grizzly Bears",
        viewer.0,
        Zone::Hand { owner: viewer },
    )];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        Action::Cast {
            card: ObjectHandle("Grizzly Bears".into())
        }
    );
    c.definitions
        .iter_mut()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap()
        .keywords
        .clear();
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        Action::Pass
    );
    let mut o = observation(board::view());
    o.decision = Some(pick_fixture(
        viewer,
        Reason::LegendRule { previous: vec![] },
        ["one", "two"]
            .map(|name| Candidate::Object {
                object: ObjectHandle(name.into()),
            })
            .into(),
        vec![mtg_core::SlotRole::Choice],
        vec![],
    ));
    assert_eq!(
        strategy(&c).choose(&o).unwrap(),
        Answer::Pick {
            picks: vec![vec![1]]
        }
    );
}

// Public payload policy probes, not accepted-input engine games or replay.

// Public IR/observation probes, not accepted-input games or replay evidence.

// Synthetic public payload probes over the admitted complete source catalog;
// these do not claim accepted-pod gameplay or replay coverage.
mod permanent_owner_hand_policy_tests {
    use super::*;

    fn fixture(name: &str, target: &str) -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::PermanentOwnerHandMoveV1)
        );
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::U, 1.into()), (Color::C, 2.into())]);
        v.objects = vec![
            board::card(&c, name, v.viewer.0, Zone::Hand { owner: v.viewer }),
            board::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
            board::card(&c, target, 0, Zone::Battlefield),
        ];
        v.objects[2].owner = v.viewer; // Harmful preference follows control, not ownership.
        (c, v)
    }

    #[test]
    fn permanent_bounce_develops_against_opposing_noncreatures_and_honors_offered_targets() {
        for (name, target) in [("Regress", "Island"), ("Disperse", "Sol Ring")] {
            let (c, mut v) = fixture(name, target);
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            let p = d.spell.as_ref().unwrap();
            let selected = ObjectHandle(target.into());
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                Action::Cast {
                    card: ObjectHandle(name.into())
                }
            );
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], None),
                Some(selected.clone())
            );
            assert_eq!(
                strategy(&c).object_target(
                    &v,
                    d,
                    p,
                    &p.targets[0],
                    Some(std::slice::from_ref(&selected))
                ),
                Some(selected.clone())
            );
            assert_eq!(
                strategy(&c).object_target(
                    &v,
                    d,
                    p,
                    &p.targets[0],
                    Some(&[ObjectHandle("Grizzly Bears".into())])
                ),
                None
            );
            assert_eq!(canonical(&v).unwrap(), before);

            let unsummon = c.definitions.iter().find(|d| d.name == "Unsummon").unwrap();
            let old = unsummon.spell.as_ref().unwrap();
            assert_eq!(
                strategy(&c).object_target(&v, unsummon, old, &old.targets[0], None),
                None,
                "the old creature-only domain must not acquire permanent targets"
            );
            if name == "Disperse" {
                v.objects
                    .push(board::card(&c, "Island", 0, Zone::Battlefield));
                assert_eq!(
                    strategy(&c).object_target(
                        &v,
                        d,
                        p,
                        &p.targets[0],
                        Some(&[ObjectHandle("Island".into())])
                    ),
                    None,
                    "an offered handle does not erase the nonland filter"
                );
            }
            v.objects.remove(0);
            let viewer = v.viewer.0;
            stack_spell(&mut v, &c, name, viewer);
            v.stack.last_mut().unwrap().cast_complete = false;
            let mut o = observation(v);
            o.decision = Some(targets(
                o.view.viewer,
                &p.targets,
                vec![ObjectHandle("Grizzly Bears".into()), selected.clone()],
            ));
            let before = canonical(&o).unwrap();
            assert_eq!(
                strategy(&c).choose(&o).unwrap(),
                o.decision
                    .as_ref()
                    .unwrap()
                    .pick_one(&Candidate::Object { object: selected })
                    .unwrap()
            );
            assert_eq!(canonical(&o).unwrap(), before);
        }
    }

    #[test]
    fn permanent_bounce_requires_current_enemy_targetability_payment_and_development_timing() {
        for (name, target) in [("Regress", "Island"), ("Disperse", "Sol Ring")] {
            let (c, v) = fixture(name, target);
            for case in [
                "own",
                "shroud",
                "hexproof",
                "departed",
                "graveyard",
                "unaffordable",
                "opponent-priority",
            ] {
                let mut v = v.clone();
                match case {
                    "own" => v.objects[2].controller = v.viewer,
                    "shroud" => v.objects[2].keywords.push(KeywordAbility::Shroud),
                    "hexproof" => v.objects[2].keywords.push(KeywordAbility::Hexproof),
                    "departed" => {
                        v.objects.remove(2);
                    }
                    "graveyard" => v.objects[2].zone = Zone::Graveyard { owner: v.viewer },
                    "unaffordable" => v
                        .players
                        .iter_mut()
                        .find(|p| p.player == v.viewer)
                        .unwrap()
                        .mana
                        .clear(),
                    "opponent-priority" => {
                        v.active = Some(PlayerId(0));
                        v.turn_owner = PlayerId(0);
                    }
                    _ => unreachable!(),
                }
                let before = canonical(&v).unwrap();
                assert_eq!(
                    strategy(&c).priority(&observation(v.clone())).unwrap(),
                    Action::Pass,
                    "{name}: {case}"
                );
                assert_eq!(canonical(&v).unwrap(), before);
            }
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            let p = d.spell.as_ref().unwrap();
            for candidates in [vec![], vec![ObjectHandle("stale-incarnation".into())]] {
                assert_eq!(
                    strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&candidates)),
                    None
                );
            }
            let mut departed = v;
            departed.objects.remove(2);
            assert_eq!(
                strategy(&c).object_target(
                    &departed,
                    d,
                    p,
                    &p.targets[0],
                    Some(&[ObjectHandle(target.into())])
                ),
                None,
                "direct preference never invents a disappeared object; existing mandatory-choice fallback is separate"
            );
        }
    }
}

// Synthetic public payload probes over admitted complete source IR, not
// accepted-pod/payment/replay evidence.
mod activated_permanent_tap_policy_tests {
    use super::*;

    fn fixture() -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Icy Manipulator")
            .unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::ActivatedTapPermanentV1)
        );
        assert!(
            matches!(&d.activated[0].cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m| m.generic == Quantity::from(1) && m.colored.is_empty()))
        );
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana
            .clear();
        let mut source = board::card(&c, "Icy Manipulator", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("own-icy".into());
        source.owner = PlayerId(2);
        source.attack_ready = false; // An artifact has no creature tap restriction.
        let mut tapped = board::card(&c, "Icy Manipulator", 0, Zone::Battlefield);
        tapped.handle = ObjectHandle("enemy-icy".into());
        tapped.tapped = true;
        let mut enemy = board::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
        enemy.handle = ObjectHandle("enemy-defender".into());
        let mut own = board::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-defender".into());
        v.objects = vec![
            source,
            board::card(&c, "Plains", v.viewer.0, Zone::Battlefield),
            tapped,
            enemy,
            own,
        ];
        (c, v)
    }

    #[test]
    fn activated_tap_prefers_untapped_enemy_and_uses_independent_printed_cost_plan() {
        let (c, mut v) = fixture();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Icy Manipulator")
            .unwrap();
        let p = &d.activated[0].program;
        assert!(strategy(&c).value(&v.objects[2]) > strategy(&c).value(&v.objects[3]));
        let before = canonical(&v).unwrap();
        let mut s = strategy(&c);
        assert_eq!(
            s.priority(&observation(v.clone())).unwrap(),
            Action::ActivateAbility {
                object: ObjectHandle("own-icy".into()),
                ability: 0
            }
        );
        assert_eq!(s.memory.mana_plan.len(), 1);
        assert_eq!(
            s.memory.mana_plan[0].source,
            ObjectHandle("Plains".into()),
            "the printed generic1 plan excludes the tapped-as-cost source"
        );
        assert_eq!(
            s.object_target(&v, d, p, &p.targets[0], None),
            Some(ObjectHandle("enemy-defender".into()))
        );
        assert_eq!(
            s.object_target(
                &v,
                d,
                p,
                &p.targets[0],
                Some(&[ObjectHandle("enemy-icy".into())])
            ),
            None,
            "a legal already-tapped target is not a useful activation preference"
        );
        assert_eq!(canonical(&v).unwrap(), before);
        v.stack.push(VisibleStackEntry {
            mode_mask: None,
            spell_copy: false,
            flashback: false,
            hybrid_colors: vec![],
            selected_additional_costs: None,
            x: None,
            mana_value: None,
            triggered: None,
            entity_targets: vec![],
            handle: Some(StackHandle("icy-activation".into())),
            stack_targets: vec![],
            spell: None,
            ability: Some(mtg_core::VisibleAbility {
                oracle_id: d.oracle_id.clone(),
                ability: 0,
                handle: StackHandle("icy-activation".into()),
                source: mtg_core::VisibleTarget {
                    announced: ObjectHandle("own-icy".into()),
                    current: Some(ObjectHandle("own-icy".into())),
                },
            }),
            player_targets: vec![],
            controller: v.viewer,
            cast_complete: false,
            targets: vec![],
        });
        let mut o = observation(v);
        o.decision = Some(targets(
            o.view.viewer,
            &p.targets,
            vec![
                ObjectHandle("enemy-icy".into()),
                ObjectHandle("own-defender".into()),
                ObjectHandle("enemy-defender".into()),
            ],
        ));
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            o.decision
                .as_ref()
                .unwrap()
                .pick_one(&Candidate::Object {
                    object: ObjectHandle("enemy-defender".into())
                })
                .unwrap()
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }
}

// Synthetic permitted-view probes over admitted source IR; paid game inputs
// and replay are qualified separately by core's accepted-pod witnesses.
mod activated_headless_policy_tests {
    use super::*;

    fn baseline() -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana
            .clear();
        (c, v)
    }
    fn proposed_activation(v: &mut PermittedView, d: &Definition, source: &str) {
        v.stack.push(VisibleStackEntry {
            mode_mask: None,
            spell_copy: false,
            flashback: false,
            hybrid_colors: vec![],
            selected_additional_costs: None,
            x: None,
            mana_value: None,
            triggered: None,
            entity_targets: vec![],
            handle: Some(StackHandle("policy-activation".into())),
            stack_targets: vec![],
            spell: None,
            ability: Some(mtg_core::VisibleAbility {
                oracle_id: d.oracle_id.clone(),
                ability: 0,
                handle: StackHandle("policy-activation".into()),
                source: mtg_core::VisibleTarget {
                    announced: ObjectHandle(source.into()),
                    current: Some(ObjectHandle(source.into())),
                },
            }),
            player_targets: vec![],
            controller: v.viewer,
            cast_complete: false,
            targets: vec![],
        });
    }

    #[test]
    fn royal_policy_uses_mature_zero_mana_source_and_current_enemy_tapped_creature() {
        let (c, mut v) = baseline();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Royal Assassin")
            .unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::TapSourceDestroyV1)
        );
        assert_eq!(d.activated[0].cost, ActivationCost::TapSource);
        let p = &d.activated[0].program;
        assert!(p.targets[0].filter.as_ref().unwrap().tapped);
        let mut source = board::card(&c, "Royal Assassin", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("royal-source".into());
        let mut tapped = board::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
        tapped.handle = ObjectHandle("enemy-tapped".into());
        tapped.tapped = true;
        let mut untapped = board::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
        untapped.handle = ObjectHandle("enemy-untapped".into());
        let mut own = board::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-tapped".into());
        own.tapped = true;
        v.objects = vec![source, tapped, untapped, own];
        let before = canonical(&v).unwrap();
        let mut s = strategy(&c);
        assert_eq!(
            s.priority(&observation(v.clone())).unwrap(),
            Action::ActivateAbility {
                object: ObjectHandle("royal-source".into()),
                ability: 0
            }
        );
        assert!(
            s.memory.mana_plan.is_empty(),
            "source-tap has no mana payment"
        );
        assert_eq!(
            s.object_target(&v, d, p, &p.targets[0], None),
            Some(ObjectHandle("enemy-tapped".into()))
        );
        for handle in ["enemy-untapped", "own-tapped", "stale"] {
            assert_eq!(
                s.object_target(
                    &v,
                    d,
                    p,
                    &p.targets[0],
                    Some(&[ObjectHandle(handle.into())])
                ),
                None
            );
        }
        assert_eq!(canonical(&v).unwrap(), before);
        for case in [
            "fresh",
            "source-tapped",
            "enemy-untapped",
            "own",
            "opponent-priority",
            "combat-priority",
        ] {
            let mut v = v.clone();
            match case {
                "fresh" => v.objects[0].attack_ready = false,
                "source-tapped" => v.objects[0].tapped = true,
                "enemy-untapped" => v.objects[1].tapped = false,
                "own" => v.objects[1].controller = v.viewer,
                "opponent-priority" => {
                    v.active = Some(PlayerId(0));
                    v.turn_owner = PlayerId(0);
                }
                "combat-priority" => v.turn.step = TurnStep::DeclareAttackers,
                _ => unreachable!(),
            }
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                Action::Pass,
                "{case}"
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        proposed_activation(&mut v, d, "royal-source");
        let mut o = observation(v);
        o.decision = Some(targets(
            o.view.viewer,
            &p.targets,
            vec![
                ObjectHandle("own-tapped".into()),
                ObjectHandle("enemy-tapped".into()),
            ],
        ));
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            o.decision
                .as_ref()
                .unwrap()
                .pick_one(&Candidate::Object {
                    object: ObjectHandle("enemy-tapped".into())
                })
                .unwrap()
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }
}

// Synthetic public payload probes over admitted source IR. Core independently
// qualifies genuine paid pod inputs, source tapping and replay.
mod temporal_adept_policy_tests {
    use super::*;

    fn fixture(target: &str) -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Temporal Adept")
            .unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::ActivatedPermanentOwnerHandV1)
        );
        assert!(
            matches!(&d.activated[0].cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m| m.generic == Quantity::zero() && m.colored == [Color::U, Color::U, Color::U]))
        );
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana
            .clear();
        let mut source = board::card(&c, "Temporal Adept", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("return-source".into());
        source.owner = PlayerId(2);
        v.objects = vec![source];
        for n in 0..3 {
            let mut island = board::card(&c, "Island", v.viewer.0, Zone::Battlefield);
            island.handle = ObjectHandle(format!("return-island-{n}"));
            v.objects.push(island);
        }
        let mut enemy = board::card(&c, target, 0, Zone::Battlefield);
        enemy.handle = ObjectHandle("enemy-return-target".into());
        enemy.owner = v.viewer;
        enemy.tapped = true; // Preference follows current control, not owner/status.
        let mut own = board::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-bears".into());
        v.objects.extend([enemy, own]);
        (c, v)
    }

    #[test]
    fn temporal_policy_activates_in_own_main_and_binds_current_offered_permanent() {
        for target in ["Icy Manipulator", "Island"] {
            let (c, mut v) = fixture(target);
            let d = c
                .definitions
                .iter()
                .find(|d| d.name == "Temporal Adept")
                .unwrap();
            let p = &d.activated[0].program;
            let before = canonical(&v).unwrap();
            let mut s = strategy(&c);
            assert_eq!(
                s.priority(&observation(v.clone())).unwrap(),
                Action::ActivateAbility {
                    object: ObjectHandle("return-source".into()),
                    ability: 0
                }
            );
            let mut plan = s
                .memory
                .mana_plan
                .iter()
                .map(|p| p.source.0.clone())
                .collect::<Vec<_>>();
            plan.sort();
            assert_eq!(
                plan,
                (0..3)
                    .map(|n| format!("return-island-{n}"))
                    .collect::<Vec<_>>(),
                "the UUU plan uses three independent Islands, never the tap-cost source"
            );
            assert_eq!(
                s.object_target(&v, d, p, &p.targets[0], None),
                Some(ObjectHandle("enemy-return-target".into()))
            );
            assert_eq!(
                s.object_target(
                    &v,
                    d,
                    p,
                    &p.targets[0],
                    Some(&[ObjectHandle("own-bears".into())])
                ),
                None
            );
            assert_eq!(canonical(&v).unwrap(), before);
            v.stack.push(VisibleStackEntry {
                mode_mask: None,
                spell_copy: false,
                flashback: false,
                hybrid_colors: vec![],
                selected_additional_costs: None,
                x: None,
                mana_value: None,
                triggered: None,
                entity_targets: vec![],
                handle: Some(StackHandle("temporal-activation".into())),
                stack_targets: vec![],
                spell: None,
                ability: Some(mtg_core::VisibleAbility {
                    oracle_id: d.oracle_id.clone(),
                    ability: 0,
                    handle: StackHandle("temporal-activation".into()),
                    source: mtg_core::VisibleTarget {
                        announced: ObjectHandle("return-source".into()),
                        current: Some(ObjectHandle("return-source".into())),
                    },
                }),
                player_targets: vec![],
                controller: v.viewer,
                cast_complete: false,
                targets: vec![],
            });
            let mut o = observation(v);
            o.decision = Some(targets(
                o.view.viewer,
                &p.targets,
                vec![
                    ObjectHandle("own-bears".into()),
                    ObjectHandle("enemy-return-target".into()),
                ],
            ));
            let before = canonical(&o).unwrap();
            assert_eq!(
                strategy(&c).choose(&o).unwrap(),
                o.decision
                    .as_ref()
                    .unwrap()
                    .pick_one(&Candidate::Object {
                        object: ObjectHandle("enemy-return-target".into())
                    })
                    .unwrap()
            );
            assert_eq!(canonical(&o).unwrap(), before);
        }
    }

    #[test]
    fn temporal_policy_rejects_unready_unfunded_or_ineligible_public_choices() {
        let (c, v) = fixture("Icy Manipulator");
        for case in [
            "fresh",
            "source-tapped",
            "two-islands",
            "wrong-color",
            "friendly",
            "shroud",
            "hexproof",
            "graveyard",
            "departed",
            "foreign-source",
            "opponent-priority",
            "combat-priority",
        ] {
            let mut v = v.clone();
            match case {
                "fresh" => v.objects[0].attack_ready = false,
                "source-tapped" => v.objects[0].tapped = true,
                "two-islands" => v.objects[1].tapped = true,
                "wrong-color" => {
                    v.objects[1] = board::card(&c, "Plains", v.viewer.0, Zone::Battlefield)
                }
                "friendly" => v.objects[4].controller = v.viewer,
                "shroud" => v.objects[4].keywords.push(KeywordAbility::Shroud),
                "hexproof" => v.objects[4].keywords.push(KeywordAbility::Hexproof),
                "graveyard" => v.objects[4].zone = Zone::Graveyard { owner: v.viewer },
                "departed" => {
                    v.objects.remove(4);
                }
                "foreign-source" => v.objects[0].controller = PlayerId(0),
                "opponent-priority" => {
                    v.active = Some(PlayerId(0));
                    v.turn_owner = PlayerId(0);
                }
                "combat-priority" => v.turn.step = TurnStep::DeclareAttackers,
                _ => unreachable!(),
            }
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                Action::Pass,
                "{case}"
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Temporal Adept")
            .unwrap();
        let p = &d.activated[0].program;
        for candidates in [
            vec![],
            vec![ObjectHandle("stale-incarnation".into())],
            vec![ObjectHandle("own-bears".into())],
        ] {
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&candidates)),
                None,
                "direct preference cannot invent a current offered opponent; mandatory-choice fallback is separate"
            );
        }
        let mut stack = v;
        stack_spell(&mut stack, &c, "Icy Manipulator", 0);
        let before = canonical(&stack).unwrap();
        assert_eq!(
            strategy(&c).object_target(
                &stack,
                d,
                p,
                &p.targets[0],
                Some(&[ObjectHandle("Icy Manipulator".into())])
            ),
            None,
            "a permanent card spell on the stack is not a battlefield permanent"
        );
        assert_eq!(canonical(&stack).unwrap(), before);
    }
}

// Synthetic public observations: preference/readiness only, separate from pod evidence.
mod untap_activation_policy_tests {
    use super::*;
    fn fixture() -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        let source = board::card(&c, "Seeker of Skybreak", v.viewer.0, Zone::Battlefield);
        let mut elf = board::card(&c, "Llanowar Elves", v.viewer.0, Zone::Battlefield);
        elf.tapped = true;
        v.objects = vec![source, elf];
        (c, v)
    }
    #[test]
    fn strict_untap_activation_uses_current_control_and_offered_useful_creature() {
        let (c, mut v) = fixture();
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        let p = &d.activated[0].program;
        assert!(tap_source_creature_untap(d, p));
        let expected = Some(v.objects[1].handle.clone());
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            expected
        );
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            Action::ActivateAbility {
                object: v.objects[0].handle.clone(),
                ability: 0
            }
        );
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&[])),
            None
        );
        assert_eq!(
            strategy(&c).object_target(
                &v,
                d,
                p,
                &p.targets[0],
                Some(&[ObjectHandle("stale".into())])
            ),
            None
        );
        assert_eq!(
            strategy(&c).object_target(
                &v,
                d,
                p,
                &p.targets[0],
                Some(&[v.objects[1].handle.clone()])
            ),
            expected
        );
        v.objects[1].owner = PlayerId(0); // Current control, not ownership.
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            expected
        );
        v.objects[1].controller = PlayerId(0);
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            Action::Pass
        );
    }
    #[test]
    fn strict_untap_activation_declines_fresh_tapped_foreign_and_wrong_timing_sources() {
        let (c, v) = fixture();
        for n in 0..6 {
            let mut v = v.clone();
            match n {
                0 => v.objects[0].attack_ready = false,
                1 => v.objects[0].tapped = true,
                2 => v.objects[0].controller = PlayerId(0),
                3 => v.active = Some(PlayerId(0)),
                4 => v.turn.step = TurnStep::End,
                5 => v.objects[0].zone = Zone::Graveyard { owner: v.viewer },
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                Action::Pass,
                "case{n}"
            );
        }
    }
    #[test]
    fn strict_untap_activation_declines_untapped_ineligible_missing_and_stale_targets() {
        let (c, v) = fixture();
        let d = definition(&c, &v.objects[0].oracle_id).unwrap();
        let p = &d.activated[0].program;
        for n in 0..7 {
            let mut v = v.clone();
            match n {
                0 => v.objects[1].tapped = false,
                1 => v.objects[1].keywords.push(KeywordAbility::Shroud),
                2 => v.objects[1].zone = Zone::Hand { owner: v.viewer },
                3 => v.objects[1] = board::card(&c, "Forest", v.viewer.0, Zone::Battlefield),
                4 => {
                    v.objects.pop();
                }
                5 => v.objects[1].oracle_id = "unknown-definition".into(),
                6 => v.objects[1].characteristics = None,
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], None),
                None,
                "case{n}"
            );
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                Action::Pass,
                "case{n}"
            );
        }
    }
    #[test]
    fn two_untappers_never_autonomously_alternate_and_postcombat_creatures_are_useful() {
        let (c, mut v) = fixture();
        let mut second = v.objects[0].clone();
        second.handle = ObjectHandle("second-untapper".into());
        second.tapped = true;
        v.objects[1] = second;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            Action::Pass
        );
        v.objects[0].tapped = true;
        v.objects[1].tapped = false;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            Action::Pass
        );
        v.objects[0].tapped = false;
        let mut creature = board::card(&c, "Yargle and Multani", v.viewer.0, Zone::Battlefield);
        creature.tapped = true;
        v.objects.push(creature);
        v.turn.step = TurnStep::PostcombatMain;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            Action::ActivateAbility {
                object: v.objects[0].handle.clone(),
                ability: 0
            }
        );
    }
}

// Synthetic permitted observations using actual newly admitted source IR.
#[test]
fn indestructible_policy_sole_destruction_declines_and_announced_choice_falls_back() {
    use mtg_contracts::KeywordAbility;
    let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    for (spell, protected, ordinary) in [
        ("Murder", "Darksteel Myr", "Grizzly Bears"),
        ("Shatter", "Darksteel Relic", "Sol Ring"),
    ] {
        let mut v = board::view();
        v.active = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        let d = c.definitions.iter().find(|d| d.name == spell).unwrap();
        let p = d.spell.as_ref().unwrap();
        v.objects = vec![board::card(&c, protected, 0, Zone::Battlefield)];
        let h = v.objects[0].handle.clone();
        assert!(
            v.objects[0]
                .keywords
                .contains(&KeywordAbility::Indestructible)
        );
        assert!(!strategy(&c).definition_ready(&v, d));
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(std::slice::from_ref(&h))),
            None
        );
        let initiation = |view: &PermittedView| {
            let mut view = view.clone();
            view.turn.lands_played = 1;
            let mut mana =
                BTreeMap::from([(Color::C, d.characteristics.mana_cost.generic.clone())]);
            for color in &d.characteristics.mana_cost.colored {
                mana.entry(*color).or_default().add(&1.into());
            }
            view.players
                .iter_mut()
                .find(|p| p.player == view.viewer)
                .unwrap()
                .mana = mana;
            view.objects.push(board::card(
                &c,
                spell,
                view.viewer.0,
                Zone::Hand { owner: view.viewer },
            ));
            strategy(&c).priority(&observation(view)).unwrap()
        };
        assert_eq!(initiation(&v), Action::Pass);
        // Existing mandatory choice fallback remains legal after announcement.
        let viewer = v.viewer;
        stack_spell(&mut v, &c, spell, viewer.0);
        let mut o = observation(v.clone());
        o.decision = Some(targets(viewer, &p.targets, vec![h.clone()]));
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            o.decision
                .as_ref()
                .unwrap()
                .pick_one(&Candidate::Object { object: h.clone() })
                .unwrap()
        );
        v.stack.clear();
        v.objects.retain(|o| o.zone == Zone::Battlefield);
        v.objects
            .push(board::card(&c, ordinary, 0, Zone::Battlefield));
        let ordinary_handle = v.objects[1].handle.clone();
        assert_eq!(
            initiation(&v),
            Action::Cast {
                card: ObjectHandle(spell.into())
            }
        );
        assert!(strategy(&c).definition_ready(&v, d));
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            Some(ordinary_handle.clone())
        );
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&[])),
            None
        );
        assert_eq!(
            strategy(&c).object_target(
                &v,
                d,
                p,
                &p.targets[0],
                Some(&[ObjectHandle("stale".into())])
            ),
            None
        );
        v.objects[1].controller = viewer;
        assert!(!strategy(&c).definition_ready(&v, d));
        v.objects[1].controller = PlayerId(0);
        v.objects[1].zone = Zone::Graveyard { owner: PlayerId(0) };
        assert!(!strategy(&c).definition_ready(&v, d));
        v.objects[0].keywords.clear();
        // Current public keywords, rather than catalog identity, govern preference.
        assert!(strategy(&c).definition_ready(&v, d));
        v.objects[0].keywords = vec![KeywordAbility::Indestructible];
        v.objects[0].characteristics = Some(
            c.definitions
                .iter()
                .find(|d| d.name == "Mountain")
                .unwrap()
                .characteristics
                .clone(),
        );
        assert!(!strategy(&c).definition_ready(&v, d));
        // Other mandatory benefits keep their previous policy.
        let mut compound = d.clone();
        compound
            .spell
            .as_mut()
            .unwrap()
            .instructions
            .push(mtg_contracts::Instruction::draw(
                mtg_contracts::kir::PlayerExpr::you(mtg_contracts::kir::Context::Spell),
                1.into(),
                0,
            ));
        v.objects[0] = board::card(&c, protected, 0, Zone::Battlefield);
        assert!(strategy(&c).definition_ready(&v, &compound));
    }
}

#[cfg(test)]
mod shared_query_tests {
    use super::board::{card, view};
    use super::*;
    use mtg_contracts::{Chooser, CreaturePowerFilter, EvaluationStage, PermanentFilter};

    // The policy ranks candidates through `mtg_core::query`. Its retired
    // copies ignored a power bound, counted a creature land as a nonland
    // permanent and counted phased-out permanents.
    #[test]
    fn policy_filters_and_sets_follow_the_shared_core_predicates() {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let mut v = view();
        let me = v.viewer;
        let opponent = (0..4).find(|p| PlayerId(*p) != me).unwrap();
        let named = |name: &str, handle: &str| {
            let mut o = card(&c, name, opponent, Zone::Battlefield);
            o.handle = ObjectHandle(handle.into());
            o
        };
        let small = named("Storm Crow", "small");
        let large = named("Colossal Dreadmaw", "large");
        let mut creature_land = named("Storm Crow", "creature land");
        creature_land
            .characteristics
            .as_mut()
            .unwrap()
            .additional_types = vec![CardKind::Land];
        let mut phased = named("Colossal Dreadmaw", "phased");
        phased.phased_out = true;
        v.objects = vec![
            small.clone(),
            large.clone(),
            creature_land.clone(),
            phased.clone(),
        ];
        let s = Strategy::new(&c, Policy::Develop, [0; 4]);
        let strong = TargetSpec {
            slot: 0,
            selection_id: "line:0:0".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::BattlefieldPermanent,
            filter: Some(PermanentFilter {
                types: vec![CardKind::Creature],
                non_types: vec![],
                non_colors: vec![],
                opponent: true,
                tapped: false,
                power: Some(CreaturePowerFilter::AtLeast { value: 4.into() }),
                predicate: None,
            }),
        };
        assert!(s.object_matches(&v, &large, &strong, false));
        assert!(!s.object_matches(&v, &small, &strong, false));
        assert!(!s.object_matches(&v, &phased, &strong, true));
        for set in [
            ObjectSet::NonlandPermanents,
            ObjectSet::OpponentNonlandPermanents,
        ] {
            assert!(s.in_set(&v, &small, &set));
            assert!(!s.in_set(&v, &creature_land, &set));
            assert!(!s.in_set(&v, &phased, &set));
        }
        assert!(s.in_set(&v, &creature_land, &ObjectSet::Creatures));
        assert!(!s.in_set(&v, &phased, &ObjectSet::Creatures));
    }
}

// Public payload policy probes, not accepted-input engine games or replay.

/// CR 119.4, 119.8: a player whose life total cannot change cannot pay life,
/// so core offers no life-paying activation and Develop never selects one;
/// Develop also never selects an activation the decision does not offer.
#[test]
fn life_payment_activation_respects_frozen_life_and_offered_candidates() {
    let mtg_compiler::CompileResult::Lowered { definition: greed } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json"),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/1ff62220-be95-4901-b8d8-812b9a1a1b0a.json"
        ),
    ) else {
        panic!("Greed lowers");
    };
    let mut c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
    c.definitions.push(*greed);
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let mut v = board::view();
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    let me = v.players.iter_mut().find(|p| p.player == v.viewer).unwrap();
    me.mana.insert(Color::B, 1.into());
    me.library_count = 10;
    v.objects = vec![board::card(&c, "Greed", v.viewer.0, Zone::Battlefield)];
    let activate = Action::ActivateAbility {
        object: ObjectHandle("Greed".into()),
        ability: 0,
    };
    let offered = |v: &PermittedView, actions: Vec<Action<Viewer>>| {
        let mut o = observation(v.clone());
        o.decision = Some(pick_fixture(
            v.viewer,
            Reason::Priority,
            actions
                .into_iter()
                .map(|action| Candidate::Action { action })
                .collect(),
            vec![mtg_core::SlotRole::Choice],
            vec![],
        ));
        o
    };
    assert_eq!(
        strategy(&c)
            .priority(&offered(&v, vec![Action::Pass, activate.clone()]))
            .unwrap(),
        activate
    );
    // Not offered: Develop passes instead of selecting it.
    assert_eq!(
        strategy(&c)
            .priority(&offered(&v, vec![Action::Pass]))
            .unwrap(),
        Action::Pass
    );
    // Frozen life: no life payment even if a stale decision listed it.
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .life_frozen_until_next_turn = true;
    assert_eq!(
        strategy(&c)
            .priority(&offered(&v, vec![Action::Pass, activate.clone()]))
            .unwrap(),
        Action::Pass
    );
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        Action::Pass
    );
}
