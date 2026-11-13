use super::*;
#[cfg(test)]
mod reactive_tests {
    use super::*;
    #[test]
    fn reactive_policy_uses_opposing_cost_and_power_and_respects_payment() {
        let c = mtg_compiler::catalog::embedded_catalog("four-colors").unwrap();
        let player = PlayerId(0);
        let card = |name: &str, owner: u8, zone: Zone| {
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            VisibleObject {
                attached_to: None,
                attachments: vec![],
                keywords: vec![],
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
        };
        let mut v = PermittedView {
            sacrifice_choices: vec![],
            setup: None,
            effects: vec![],
            combat: vec![],
            viewer: player,
            active: Some(PlayerId(1)),
            turn_owner: PlayerId(1),
            turn: mtg_core::TurnState {
                number: 10,
                step: TurnStep::PrecombatMain,
                lands_played: 1,
            },
            live_turn_order: (0..4).map(PlayerId).collect(),
            commanders: vec![],
            priority: Some(player),
            players: (0..4)
                .map(|i| mtg_core::VisiblePlayer {
                    exile: vec![],
                    library_shuffles: 0,
                    player: PlayerId(i),
                    status: mtg_core::PlayerStatus::Live,
                    life: Quantity::from(40 - i64::from(i)),
                    mana: BTreeMap::new(),
                    hand_count: 1,
                    library_count: 20,
                })
                .collect(),
            objects: vec![
                card("Counterspell", 0, Zone::Hand { owner: player }),
                card("Island", 0, Zone::Battlefield),
                card("Sol Ring", 0, Zone::Stack),
                card("Rorix Bladewing", 1, Zone::Stack),
                card("Goliath Sphinx", 2, Zone::Stack),
                card("Storm Crow", 1, Zone::Battlefield),
                card("Caelorna, Coral Tyrant", 2, Zone::Battlefield),
            ],
            stack: vec![],
        };
        for (name, owner) in [
            ("Sol Ring", 0),
            ("Rorix Bladewing", 1),
            ("Goliath Sphinx", 2),
        ] {
            v.stack.push(mtg_core::VisibleStackEntry {
                x: None,
                mana_value: None,
                mode: None,
                triggered: None,
                entity_targets: vec![],
                handle: Some(StackHandle(name.into())),
                stack_targets: vec![],
                spell: Some(ObjectHandle(name.into())),
                ability: None,
                player_targets: vec![],
                controller: PlayerId(owner),
                cast_complete: true,
                targets: vec![],
            });
        }
        let mut s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            mana_plan: Default::default(),
            mana_color: None,
        };
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(StackHandle("Goliath Sphinx".into()))
        );
        assert!(s.reactive_spells(&v).unwrap().is_none()); // Only one Island.
        let mut second = card("Island", 0, Zone::Battlefield);
        second.handle = ObjectHandle("second Island".into());
        v.objects.push(second);
        assert_eq!(
            s.reactive_spells(&v).unwrap(),
            Some(PlayerResponse::Cast {
                card: ObjectHandle("Counterspell".into())
            })
        );
        let entities = vec![
            PlayerTargetEntity::Player {
                player: PlayerId(1),
            },
            PlayerTargetEntity::Player {
                player: PlayerId(3),
            },
            PlayerTargetEntity::Object {
                object: ObjectHandle("Storm Crow".into()),
            },
        ];
        assert_eq!(
            s.damage_target(
                &v,
                &mtg_contracts::SpellProgram {
                    additional_sacrifice: None,
                    timing: mtg_contracts::SpellTiming::Instant,
                    resolution: mtg_contracts::SpellResolution::Instructions,
                    functioning_zone: mtg_contracts::FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![mtg_contracts::Instruction::DealDamage {
                        target_slot: 0,
                        amount: 2.into(),
                        source_block: 0
                    }],
                    modes: vec![]
                },
                &entities
            )
            .unwrap(),
            entities[2]
        );
        v.objects.retain(|o| o.handle.0 != "Storm Crow");
        assert_eq!(
            s.damage_target(
                &v,
                &mtg_contracts::SpellProgram {
                    additional_sacrifice: None,
                    timing: mtg_contracts::SpellTiming::Instant,
                    resolution: mtg_contracts::SpellResolution::Instructions,
                    functioning_zone: mtg_contracts::FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![mtg_contracts::Instruction::DealDamage {
                        target_slot: 0,
                        amount: 2.into(),
                        source_block: 0
                    }],
                    modes: vec![]
                },
                &entities
            )
            .unwrap(),
            entities[1]
        );
        v.stack[0].spell = None;
        v.stack[0].controller = PlayerId(1);
        v.stack[0].ability = Some(mtg_core::VisibleAbility {
            oracle_id: definition(&c, &v.objects[0].oracle_id)
                .unwrap()
                .oracle_id
                .clone(),
            ability: 0,
            handle: StackHandle("Sol Ring".into()),
            source: mtg_core::VisibleTarget {
                announced: ObjectHandle("source".into()),
                current: None,
            },
        });
        assert_eq!(
            s.opposing_stack(&v, true, None),
            Some(StackHandle("Sol Ring".into()))
        );
    }
}

#[cfg(test)]
mod wipe_tests {
    use super::*;
    pub(super) fn view(c: &Catalog) -> PermittedView {
        let p = Prepared::admit_pod(
            c.clone(),
            MatchConfig::for_profile("commander-wipes-v1", [3; 32], PlayerId(3)).unwrap(),
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
            attached_to: None,
            attachments: vec![],
            keywords: d.keywords.clone(),
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
    #[test]
    fn wipes_counter_policy_orders_announced_x_mana_value() {
        let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let mut v = view(&c);
        for (name, x, mv) in [
            ("Earthquake", Some(3.into()), Some(4.into())),
            ("Pyroclasm", None, None),
        ] {
            v.objects.push(card(&c, name, 1, Zone::Stack));
            v.stack.push(mtg_core::VisibleStackEntry {
                x,
                mana_value: mv,
                mode: None,
                triggered: None,
                entity_targets: vec![],
                handle: Some(mtg_core::StackHandle(name.into())),
                stack_targets: vec![],
                spell: Some(ObjectHandle(name.into())),
                ability: None,
                player_targets: vec![],
                controller: PlayerId(1),
                cast_complete: true,
                targets: vec![],
            });
        }
        let s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            mana_plan: Default::default(),
            mana_color: None,
        };
        assert!(!c.definitions.is_empty());
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(mtg_core::StackHandle("Earthquake".into()))
        );
        v.stack[0].x = Some(0.into());
        v.stack[0].mana_value = Some(1.into());
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(mtg_core::StackHandle("Pyroclasm".into()))
        );
    }
    #[test]
    fn wipes_policy_thresholds_and_smallest_safe_earthquake_x() {
        let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let mut v = view(&c);
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana
            .insert(Color::R, 8.into());
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .life = 6.into();
        v.objects = vec![
            card(&c, "Furnace of Rath", 3, Zone::Battlefield),
            card(&c, "Alpine Watchdog", 0, Zone::Battlefield),
            card(&c, "Serra Angel", 0, Zone::Battlefield),
            card(&c, "Giant Spider", 1, Zone::Battlefield),
            card(&c, "Storm Crow", 2, Zone::Battlefield),
        ];
        let s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            mana_plan: Default::default(),
            mana_color: None,
        };
        let d = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
        assert_eq!(
            s.choose_x(&v, d("Earthquake"), d("Earthquake").spell.as_ref().unwrap()),
            Some(2.into())
        ); // 4 damage, two ground kills; flyers excluded.
        let d = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(s.definition_ready(&v, d("Pyroclasm")));
        assert!(s.definition_ready(&v, d("Innocent Blood")));
        assert!(s.definition_ready(&v, d("Massacre Wurm")));
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .life = 2.into();
        assert_eq!(
            s.choose_x(&v, d("Earthquake"), d("Earthquake").spell.as_ref().unwrap()),
            None
        );
        assert!(!s.definition_ready(&v, d("Volcanic Fallout")));
        v.objects = vec![
            card(&c, "Bastion of Remembrance", 3, Zone::Battlefield),
            card(&c, "Glorious Anthem", 0, Zone::Battlefield),
        ];
        assert!(!s.definition_ready(&v, d("Tranquility")));
        v.objects
            .push(card(&c, "Spidersilk Armor", 1, Zone::Battlefield));
        assert!(s.definition_ready(&v, d("Tranquility")));
        v.viewer = PlayerId(1);
        v.objects = vec![
            card(&c, "Giant Spider", 1, Zone::Battlefield),
            card(&c, "Typhoid Rats", 2, Zone::Battlefield),
            card(&c, "Storm Crow", 3, Zone::Battlefield),
        ];
        v.objects[0].marked_damage = 3.into();
        assert!(
            !s.definition_ready(&v, d("Infest")),
            "the marked friendly Spider also dies, so the removal margin is only one"
        );
    }
}

#[test]
fn names_and_catalog_size_do_not_select_policy_or_wipe_amounts() {
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.objects = vec![wipe_tests::card(
        &c,
        "Alpine Watchdog",
        0,
        Zone::Battlefield,
    )];
    let id = c
        .definitions
        .iter()
        .find(|d| d.name == "Pyroclasm")
        .unwrap()
        .oracle_id
        .clone();
    for (n, d) in c.definitions.iter_mut().enumerate() {
        d.name = format!("anonymous-{n}");
    }
    let mut extra = c.definitions[0].clone();
    extra.oracle_id = "unrelated".into();
    c.definitions.push(extra);
    c.definitions.reverse();
    let s = Strategy {
        catalog: &c,
        policy: Policy::Develop,
        mulligans: [0; 4],
        mana_plan: Default::default(),
        mana_color: None,
    };
    let d = definition(&c, &id).unwrap();
    assert!(s.definition_ready(&v, d));
    let mut changed = d.clone();
    for i in &mut changed.spell.as_mut().unwrap().instructions {
        if let I::DamageSet { amount, .. } = i {
            *amount = WipeAmount::Fixed { value: 1.into() };
        }
    }
    assert!(
        !s.definition_ready(&v, &changed),
        "amount comes from IR even with an unchanged identity"
    );
}

#[test]
fn creature_target_readiness_and_prevention_follow_programs() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let v = wipe_tests::view(&c);
    let s = Strategy {
        catalog: &c,
        policy: Policy::Develop,
        mulligans: [0; 4],
        mana_plan: Default::default(),
        mana_color: None,
    };
    for name in ["Murder", "Disfigure", "Giant Growth", "Pacifism"] {
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(
            !s.definition_ready(&v, d),
            "{name} needs a suitable creature"
        );
    }
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Samite Healer")
        .unwrap();
    let cs = vec![
        PlayerTargetEntity::Player {
            player: PlayerId(0),
        },
        PlayerTargetEntity::Player { player: v.viewer },
    ];
    assert_eq!(
        s.damage_target(&v, &d.activated[0].program, &cs),
        Some(cs[1].clone())
    );
}

#[test]
fn tap_loot_is_used_before_combat_can_tap_the_source() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![wipe_tests::card(
        &c,
        "Merfolk Looter",
        v.viewer.0,
        Zone::Battlefield,
    )];
    let expected = PlayerResponse::ActivateAbility {
        object: v.objects[0].handle.clone(),
        ability: 0,
    };
    let mut s = Strategy {
        catalog: &c,
        policy: Policy::Develop,
        mulligans: [0; 4],
        mana_plan: Default::default(),
        mana_color: None,
    };
    assert_eq!(
        s.priority(&Observation {
            observation_version: 16,
            decision: None,
            view: v
        })
        .unwrap(),
        expected
    );
}

fn strategy(c: &Catalog) -> Strategy<'_> {
    Strategy {
        catalog: c,
        policy: Policy::Develop,
        mulligans: [0; 4],
        mana_plan: Default::default(),
        mana_color: None,
    }
}
fn observation(view: PermittedView) -> Observation {
    Observation {
        observation_version: 16,
        decision: None,
        view,
    }
}
fn stack_spell(v: &mut PermittedView, c: &Catalog, name: &str, controller: u8) {
    let o = wipe_tests::card(c, name, controller, Zone::Stack);
    v.stack.push(VisibleStackEntry {
        x: None,
        mana_value: None,
        mode: None,
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
fn prevention_ability_responds_to_attacks_and_announced_damage() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    let own = v.viewer.0;
    v.active = Some(PlayerId(0));
    v.turn.step = TurnStep::DeclareBlockers;
    v.objects = vec![
        wipe_tests::card(&c, "Samite Healer", own, Zone::Battlefield),
        wipe_tests::card(&c, "Serra Angel", 0, Zone::Battlefield),
    ];
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: ObjectHandle("Serra Angel".into()),
        defender: v.viewer,
        blocked: false,
        blockers: vec![],
    }];
    let expected = PlayerResponse::ActivateAbility {
        object: ObjectHandle("Samite Healer".into()),
        ability: 0,
    };
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    // Ordinary damage has already happened at this priority boundary.
    v.turn.step = TurnStep::CombatDamage;
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    v.combat.clear();
    v.turn.step = TurnStep::PrecombatMain;
    stack_spell(&mut v, &c, "Lightning Bolt", 0);
    v.stack.last_mut().unwrap().entity_targets = vec![VisibleEntityTarget::Object {
        object: mtg_core::VisibleTarget {
            announced: ObjectHandle("Samite Healer".into()),
            current: Some(ObjectHandle("Samite Healer".into())),
        },
    }];
    let s = strategy(&c);
    assert_eq!(
        s.damage_target(
            &v,
            &c.definitions
                .iter()
                .find(|d| d.name == "Samite Healer")
                .unwrap()
                .activated[0]
                .program,
            &s.friendly_entities(&v)
        ),
        Some(PlayerTargetEntity::Object {
            object: ObjectHandle("Samite Healer".into())
        })
    );
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    v.stack.last_mut().unwrap().entity_targets =
        vec![VisibleEntityTarget::Player { player: v.viewer }];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    v.objects[0].tapped = true;
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        PlayerResponse::Pass
    );
}

#[test]
fn reactive_modal_spell_selects_prevention_instead_of_ready_life_mode() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(PlayerId(0));
    v.turn.step = TurnStep::DeclareBlockers;
    v.objects = vec![
        wipe_tests::card(
            &c,
            "Healing Salve",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
        wipe_tests::card(&c, "Serra Angel", 0, Zone::Battlefield),
    ];
    v.players
        .iter_mut()
        .find(|r| r.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::W, 1.into());
    assert!(strategy(&c).reactive_spells(&v).unwrap().is_none());
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: ObjectHandle("Serra Angel".into()),
        defender: v.viewer,
        blocked: false,
        blockers: vec![],
    }];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Healing Salve".into())
        }
    );
    stack_spell(&mut v, &c, "Healing Salve", 3);
    // The caster has not selected a mode yet; both modes are otherwise usable.
    v.stack.last_mut().unwrap().cast_complete = false;
    assert_eq!(strategy(&c).choose_mode(&v, &[0, 1]), Some(1));
    v.combat.clear();
    v.stack.pop();
    stack_spell(&mut v, &c, "Lightning Bolt", 0);
    v.stack.last_mut().unwrap().entity_targets =
        vec![VisibleEntityTarget::Player { player: v.viewer }];
    assert!(strategy(&c).reactive_spells(&v).unwrap().is_some());
    stack_spell(&mut v, &c, "Healing Salve", 3);
    v.stack.last_mut().unwrap().cast_complete = false;
    assert_eq!(strategy(&c).choose_mode(&v, &[0, 1]), Some(1));
}

#[test]
fn sacrifice_costs_use_tokens_doomed_creatures_and_lethal_payoffs() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(PlayerId(0));
    v.turn.step = TurnStep::PrecombatMain;
    v.objects = vec![
        wipe_tests::card(&c, "Goblin Bombardment", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Serra Angel", v.viewer.0, Zone::Battlefield),
    ];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    v.objects[1].token = Some(mtg_core::VisibleToken {
        name: "anonymous".into(),
        keywords: vec![],
    });
    let expected = PlayerResponse::ActivateAbility {
        object: ObjectHandle("Goblin Bombardment".into()),
        ability: 0,
    };
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    v.objects[1].token = None;
    stack_spell(&mut v, &c, "Day of Judgment", 0);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    v.stack.clear();
    stack_spell(&mut v, &c, "Lightning Bolt", 0);
    // Two damage already marked makes the announced Bolt lethal to the Angel.
    v.objects[1].marked_damage = 2.into();
    v.stack.last_mut().unwrap().entity_targets = vec![VisibleEntityTarget::Object {
        object: mtg_core::VisibleTarget {
            announced: v.objects[1].handle.clone(),
            current: Some(v.objects[1].handle.clone()),
        },
    }];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        expected
    );
    v.stack.clear();
    v.objects[1].marked_damage = 0.into();
    v.objects.push(wipe_tests::card(
        &c,
        "Fling",
        v.viewer.0,
        Zone::Hand { owner: v.viewer },
    ));
    let fling = c.definitions.iter().find(|d| d.name == "Fling").unwrap();
    assert!(!strategy(&c).definition_ready(&v, fling));
    v.players
        .iter_mut()
        .find(|r| r.player == PlayerId(0))
        .unwrap()
        .life = 4.into();
    assert!(strategy(&c).definition_ready(&v, fling));
    v.players
        .iter_mut()
        .find(|r| r.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::R, 2.into());
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Fling".into())
        }
    );
}

#[test]
fn death_payoffs_and_ramp_develop_before_commander_recasts() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(
            &c,
            "Yargle and Multani",
            v.viewer.0,
            Zone::Command { owner: v.viewer },
        ),
        wipe_tests::card(
            &c,
            "Butcher of Malakir",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
    ];
    let row = v.players.iter_mut().find(|r| r.player == v.viewer).unwrap();
    row.mana.insert(Color::B, 7.into());
    row.mana.insert(Color::G, 1.into());
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Butcher of Malakir".into())
        }
    );
}

#[test]
fn mana_planner_preserves_flexible_source_for_the_second_color() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.objects = vec![
        wipe_tests::card(&c, "Birds of Paradise", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Plains", v.viewer.0, Zone::Battlefield),
    ];
    let plan = mana_policy::plan(
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
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.objects = vec![wipe_tests::card(&c, "Storm Crow", 0, Zone::Battlefield)];
    stack_spell(&mut v, &c, "Lightning Bolt", 3);
    let bolt = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Lightning Bolt")
        .unwrap();
    let p = bolt.spell.as_mut().unwrap();
    p.instructions = vec![
        I::ModifyPowerToughness {
            target_slot: 0,
            power: 1.into(),
            toughness: 1.into(),
            duration: mtg_contracts::Duration::UntilEndOfTurn,
            source_block: 0,
        },
        I::DestroyPermanent {
            target_slot: 1,
            optional: true,
            regeneration_forbidden: false,
            source_block: 0,
        },
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
    assert!(strategy(&c).optional_effect(&v));
    let bolt = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Lightning Bolt")
        .unwrap();
    let p = bolt.spell.as_mut().unwrap();
    p.instructions = vec![
        I::DealDamage {
            target_slot: 0,
            amount: 3.into(),
            source_block: 0,
        },
        I::DealDamage {
            target_slot: 1,
            amount: 3.into(),
            source_block: 0,
        },
    ];
    let mut second = p.targets[0].clone();
    second.slot = 1;
    second.selection_id = "second".into();
    p.targets.push(second);
    let candidates = vec![
        PlayerTargetEntity::Player {
            player: PlayerId(0),
        },
        PlayerTargetEntity::Player {
            player: PlayerId(1),
        },
    ];
    let request = PlayerRequest::EntityTargets {
        token: mtg_core::DecisionToken("policy-fixture".into()),
        player: v.viewer,
        occurrences: p.targets.clone(),
        candidates: candidates.clone(),
    };
    let mut o = observation(v);
    o.decision = Some(request);
    let PlayerResponse::EntityTargets { entities } = strategy(&c).choose(&o).unwrap() else {
        panic!("entity response")
    };
    assert_eq!(entities.len(), 2);
    assert_ne!(entities[0], entities[1]);
    assert!(entities.iter().all(|e| candidates.contains(e)));
}

#[test]
fn sacrifice_pump_is_used_before_combat_and_damage_trades_for_value() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(&c, "Bloodthrone Vampire", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
    ];
    v.objects[1].token = Some(mtg_core::VisibleToken {
        name: "anonymous".into(),
        keywords: vec![],
    });
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::ActivateAbility {
            object: ObjectHandle("Bloodthrone Vampire".into()),
            ability: 0
        }
    );
    v.objects = vec![
        wipe_tests::card(&c, "Serra Angel", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Goliath Sphinx", 0, Zone::Battlefield),
    ];
    v.objects[1].marked_damage = 3.into();
    let fling = c.definitions.iter().find(|d| d.name == "Fling").unwrap();
    assert!(
        strategy(&c).definition_ready(&v, fling),
        "four damage trades the smaller creature for the marked Sphinx"
    );
    v.objects[1].marked_damage = 0.into();
    assert!(!strategy(&c).definition_ready(&v, fling));
}

#[test]
fn sacrifice_estimate_matches_the_profitable_candidate_and_spells_use_precombat_mana() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(&c, "Serra Angel", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Goliath Sphinx", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Giant Spider", 0, Zone::Battlefield),
    ];
    // Synthetic public characteristics: only the larger sacrifice can kill this
    // valuable target. The chooser and prepayment damage estimate must agree.
    let target = v.objects[2].characteristics.as_mut().unwrap();
    target.toughness = Some(6.into());
    target.mana_cost.generic = 30.into();
    let fling = c
        .definitions
        .iter()
        .find(|d| d.name == "Fling")
        .unwrap()
        .spell
        .as_ref()
        .unwrap();
    let s = strategy(&c);
    assert!(s.program_ready(
        &v,
        c.definitions.iter().find(|d| d.name == "Fling").unwrap(),
        fling
    ));
    assert_eq!(
        s.damage_amount(&v, fling),
        v.objects[1]
            .characteristics
            .as_ref()
            .unwrap()
            .power
            .clone()
            .unwrap()
    );
    assert_eq!(
        s.damage_target(
            &v,
            fling,
            &[
                PlayerTargetEntity::Player {
                    player: PlayerId(0)
                },
                PlayerTargetEntity::Object {
                    object: v.objects[2].handle.clone()
                }
            ]
        ),
        Some(PlayerTargetEntity::Object {
            object: v.objects[2].handle.clone()
        })
    );
    v.objects.clear();
    v.objects.push(wipe_tests::card(
        &c,
        "Night's Whisper",
        v.viewer.0,
        Zone::Hand { owner: v.viewer },
    ));
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::B, 2.into());
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Night's Whisper".into())
        }
    );
}

#[test]
fn defensive_modal_cards_and_tap_sources_are_reserved_for_reactive_priority() {
    let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.objects = vec![
        wipe_tests::card(
            &c,
            "Healing Salve",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        ),
        wipe_tests::card(&c, "Samite Healer", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Serra Angel", v.viewer.0, Zone::Battlefield),
    ];
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::W, 1.into());
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass,
        "do not spend the modal prevention card on unconditional life gain"
    );
    let mut attack = observation(v.clone());
    attack.decision = Some(PlayerRequest::Attackers {
        token: mtg_core::DecisionToken("policy-fixture".into()),
        player: v.viewer,
        candidates: vec![
            ObjectHandle("Samite Healer".into()),
            ObjectHandle("Serra Angel".into()),
        ],
        defenders: vec![PlayerId(0)],
    });
    assert_eq!(
        strategy(&c).choose(&attack).unwrap(),
        PlayerResponse::Attackers {
            attacks: vec![(ObjectHandle("Serra Angel".into()), PlayerId(0))]
        }
    );
    v.active = Some(PlayerId(0));
    v.turn.step = TurnStep::DeclareBlockers;
    v.objects[2].controller = PlayerId(0);
    v.combat = vec![mtg_core::VisibleAttacker {
        attacker: ObjectHandle("Serra Angel".into()),
        defender: v.viewer,
        blocked: false,
        blockers: vec![],
    }];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
            card: ObjectHandle("Healing Salve".into())
        }
    );
    v.objects.remove(0);
    assert_eq!(
        strategy(&c).priority(&observation(v)).unwrap(),
        PlayerResponse::ActivateAbility {
            object: ObjectHandle("Samite Healer".into()),
            ability: 0
        }
    );
}

#[test]
fn generic_policy_develops_flash_at_opponents_end_and_chooses_legends() {
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
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
    v.objects = vec![wipe_tests::card(
        &c,
        "Grizzly Bears",
        viewer.0,
        Zone::Hand { owner: viewer },
    )];
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Cast {
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
        PlayerResponse::Pass
    );
    let candidates = vec![ObjectHandle("one".into()), ObjectHandle("two".into())];
    assert_eq!(
        strategy(&c).legend_choice(&candidates).unwrap(),
        PlayerResponse::LegendRule {
            keep: candidates[1].clone()
        }
    );
}

#[test]
fn generic_policy_respects_public_opponent_cast_prohibition_before_flash_planning() {
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    let header =
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json");
    for raw in [
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/3233af43-7826-4895-b67e-03c6102c2cd5.json"
        ),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
        ),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            mtg_compiler::compile(header, raw)
        else {
            panic!("complete frozen Kutzil/Research source must lower");
        };
        c.definitions.push(*definition);
    }
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let viewer = v.viewer;
    v.active = Some(PlayerId(1));
    v.turn_owner = PlayerId(1);
    v.turn.step = TurnStep::End;
    v.players
        .iter_mut()
        .find(|p| p.player == viewer)
        .unwrap()
        .mana = [(Color::U, 5.into())].into();
    v.objects = vec![wipe_tests::card(
        &c,
        "Research Thief",
        viewer.0,
        Zone::Hand { owner: viewer },
    )];
    let cast = PlayerResponse::Cast {
        card: ObjectHandle("Research Thief".into()),
    };
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        cast
    );

    // These are public-view policy probes, not accepted-input engine replay.
    // Ownership differs from control; only the source's current controller and
    // the retained turn owner define the prohibition.
    let mut lock = wipe_tests::card(&c, "Kutzil, Malamet Exemplar", 2, Zone::Battlefield);
    lock.controller = PlayerId(1);
    v.objects.push(lock);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    v.active = None;
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        PlayerResponse::Pass
    );
    v.active = Some(PlayerId(1));
    v.objects[1].controller = PlayerId(0);
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        cast
    );
    v.objects[1].controller = PlayerId(1);
    v.turn_owner = PlayerId(0);
    v.active = Some(PlayerId(0));
    assert_eq!(
        strategy(&c).priority(&observation(v.clone())).unwrap(),
        cast
    );
    v.turn_owner = PlayerId(1);
    v.active = Some(PlayerId(1));
    v.objects[1].zone = Zone::Graveyard { owner: PlayerId(2) };
    assert_eq!(strategy(&c).priority(&observation(v)).unwrap(), cast);
}

// Public payload policy probes, not accepted-input engine games or replay.
mod blocking_target_policy_tests {
    use super::*;

    fn fixture() -> (Catalog, PermittedView) {
        let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let mut v = wipe_tests::view(&c);
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            mtg_compiler::fixtures::MANIFEST,
            include_str!("../../../../tests/fixtures/righteousness.json"),
        ) else {
            panic!("complete retained Righteousness must lower");
        };
        c.definitions.push(*definition);
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        v.active = Some(PlayerId(0));
        v.turn_owner = PlayerId(0);
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::DeclareBlockers;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::W, 1.into())]);
        let mut own = wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-blocker".into());
        own.owner = PlayerId(2); // Benefit follows control, not ownership.
        let mut other = wipe_tests::card(&c, "Grizzly Bears", 1, Zone::Battlefield);
        other.handle = ObjectHandle("opposing-blocker".into());
        let mut attacker = wipe_tests::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
        attacker.handle = ObjectHandle("attacker".into());
        v.objects = vec![
            wipe_tests::card(
                &c,
                "Righteousness",
                v.viewer.0,
                Zone::Hand { owner: v.viewer },
            ),
            own,
            other,
            attacker,
        ];
        v.combat = vec![mtg_core::VisibleAttacker {
            attacker: ObjectHandle("attacker".into()),
            defender: v.viewer,
            blocked: true,
            blockers: vec![ObjectHandle("own-blocker".into())],
        }];
        (c, v)
    }

    #[test]
    fn blocking_pump_policy_reacts_only_before_remaining_combat_damage() {
        let (c, mut v) = fixture();
        let cast = PlayerResponse::Cast {
            card: ObjectHandle("Righteousness".into()),
        };
        for step in [TurnStep::DeclareBlockers, TurnStep::FirstStrikeDamage] {
            v.turn.step = step;
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                cast
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        for step in [
            TurnStep::DeclareAttackers,
            TurnStep::CombatDamage,
            TurnStep::EndCombat,
            TurnStep::PostcombatMain,
        ] {
            v.turn.step = step;
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                PlayerResponse::Pass,
                "{step:?}"
            );
        }
        // The new recognition is specific to blocking-target pumps; ordinary
        // Giant Growth does not become a general reactive combat policy.
        v.turn.step = TurnStep::DeclareBlockers;
        v.objects[0] = wipe_tests::card(
            &c,
            "Giant Growth",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        );
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::G, 1.into())]);
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Pass
        );
    }

    #[test]
    fn blocking_pump_policy_requires_linked_friendly_targetability_and_payment() {
        let (c, baseline) = fixture();
        for mutate in [
            (|v: &mut PermittedView| v.combat.clear()) as fn(&mut PermittedView),
            |v| v.combat[0].blockers = vec![ObjectHandle("opposing-blocker".into())],
            |v| v.objects[1].controller = PlayerId(1),
            |v| v.objects[1].zone = Zone::Graveyard { owner: PlayerId(2) },
            |v| {
                v.objects[1]
                    .keywords
                    .push(mtg_contracts::KeywordAbility::Shroud)
            },
            |v| v.objects[1].characteristics = None,
            |v| {
                v.players
                    .iter_mut()
                    .find(|p| p.player == v.viewer)
                    .unwrap()
                    .mana
                    .clear()
            },
        ] {
            let mut v = baseline.clone();
            mutate(&mut v);
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                PlayerResponse::Pass
            );
        }
        let mut v = baseline;
        v.objects[1]
            .keywords
            .push(mtg_contracts::KeywordAbility::Hexproof);
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Righteousness".into())
            }
        );
    }

    #[test]
    fn blocking_pump_policy_trusts_offered_orphans_and_keeps_target_choice_stable() {
        let (c, mut v) = fixture();
        v.combat.clear(); // The attacker left; this view cannot prove blocking.
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Righteousness")
            .unwrap();
        let p = d.spell.as_ref().unwrap();
        let candidates = vec![
            ObjectHandle("opposing-blocker".into()),
            ObjectHandle("own-blocker".into()),
        ];
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            None
        );
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&[])),
            None
        );
        v.objects.remove(0);
        let viewer = v.viewer;
        stack_spell(&mut v, &c, "Righteousness", viewer.0);
        for reverse in [false, true] {
            let mut view = v.clone();
            let mut offered = candidates.clone();
            if reverse {
                view.objects.reverse();
                offered.reverse();
            }
            let mut o = observation(view);
            o.decision = Some(PlayerRequest::Targets {
                token: mtg_core::DecisionToken("blocking-payload".into()),
                player: viewer,
                occurrences: p.targets.clone(),
                candidates: offered,
            });
            let before = canonical(&o).unwrap();
            assert_eq!(
                strategy(&c).choose(&o).unwrap(),
                PlayerResponse::Targets {
                    objects: vec![ObjectHandle("own-blocker".into())]
                }
            );
            assert_eq!(canonical(&o).unwrap(), before);
        }
    }
}

#[test]
fn tapped_target_policy_uses_current_public_status_and_offered_candidates() {
    let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&c);
    let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
        include_str!("../../../../tests/fixtures/5820d760-8ed7-4abf-a959-c06ed86a3730.json"),
    ) else {
        panic!("complete Swift Response source must lower")
    };
    c.definitions.push(*definition);
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Swift Response")
        .unwrap();
    let p = d.spell.as_ref().unwrap();
    let t = &p.targets[0];
    v.objects = vec![
        wipe_tests::card(&c, "Goliath Sphinx", 0, Zone::Battlefield),
        wipe_tests::card(&c, "Storm Crow", 0, Zone::Battlefield),
        wipe_tests::card(&c, "Sol Ring", 0, Zone::Battlefield),
        wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
    ];
    v.objects[1].tapped = true;
    v.objects[2].tapped = true;
    v.objects[3].tapped = true;
    let before = canonical(&v).unwrap();
    let selected = ObjectHandle("Storm Crow".into());
    assert_eq!(
        strategy(&c).object_target(&v, d, p, t, None),
        Some(selected.clone())
    );
    assert_eq!(
        strategy(&c).object_target(&v, d, p, t, Some(&[selected.clone()])),
        Some(selected)
    );
    assert_eq!(
        strategy(&c).object_target(&v, d, p, t, Some(&[ObjectHandle("Goliath Sphinx".into())])),
        None
    );
    assert_eq!(canonical(&v).unwrap(), before);
    v.objects[1].tapped = false;
    assert_eq!(strategy(&c).object_target(&v, d, p, t, None), None);
}
