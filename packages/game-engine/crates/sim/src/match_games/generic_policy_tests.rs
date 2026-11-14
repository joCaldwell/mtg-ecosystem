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
fn tap_cantrip_policy_consumes_admitted_program_and_offered_candidates() {
    let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
    let baseline = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&baseline);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    v.turn.lands_played = 1;
    v.players
        .iter_mut()
        .find(|p| p.player == v.viewer)
        .unwrap()
        .mana
        .insert(Color::W, 2.into());
    let mut enemy = wipe_tests::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("enemy-isamaru".into());
    let mut own = wipe_tests::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
    own.handle = ObjectHandle("own-isamaru".into());
    v.objects = vec![
        wipe_tests::card(
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
        PlayerResponse::Cast {
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
fn sacrifice_shrapnel_source_uses_artifact_cost_and_fixed_damage_for_lethal() {
    let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let baseline = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&baseline);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Shrapnel Blast")
        .unwrap();
    let p = d.spell.as_ref().unwrap();
    assert_eq!(
        p.additional_sacrifice.as_ref().unwrap().types,
        [CardKind::Artifact]
    );
    let mut enemy = wipe_tests::card(&c, "Sol Ring", 0, Zone::Battlefield);
    enemy.handle = ObjectHandle("enemy-ring".into());
    v.objects = vec![
        wipe_tests::card(&c, "Sol Ring", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
        wipe_tests::card(&c, "Mind Stone", v.viewer.0, Zone::Hand { owner: v.viewer }),
        enemy,
    ];
    v.players
        .iter_mut()
        .find(|r| r.player == PlayerId(0))
        .unwrap()
        .life = 6.into();
    let s = strategy(&c);
    assert!(
        !s.definition_ready(&v, d),
        "a healthy Ring is not spent for nonlethal damage alone"
    );
    v.players
        .iter_mut()
        .find(|r| r.player == PlayerId(0))
        .unwrap()
        .life = 5.into();
    let before = canonical(&v).unwrap();
    assert!(s.definition_ready(&v, d));
    assert_eq!(s.damage_amount(&v, p), Quantity::from(5));
    assert_eq!(
        s.damage_target(
            &v,
            p,
            &[PlayerTargetEntity::Player {
                player: PlayerId(0)
            }]
        ),
        Some(PlayerTargetEntity::Player {
            player: PlayerId(0)
        })
    );
    assert_eq!(
        s.sacrifice_objects(&v, p.additional_sacrifice.as_ref().unwrap())
            .into_iter()
            .map(|o| o.handle.clone())
            .collect::<Vec<_>>(),
        vec![ObjectHandle("Sol Ring".into())]
    );
    assert_eq!(canonical(&v).unwrap(), before, "policy inspection is pure");
    v.objects.remove(0);
    assert!(
        !s.definition_ready(&v, d),
        "enemy/hand artifacts and an ordinary creature cannot pay"
    );
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

// Public payload policy probes, not accepted-input engine games or replay.
mod reactive_tapped_destroy_tests {
    use super::*;

    fn fixture() -> (Catalog, PermittedView) {
        let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let mut v = wipe_tests::view(&c);
        for raw in [
            include_str!("../../../../tests/fixtures/5820d760-8ed7-4abf-a959-c06ed86a3730.json"),
            include_str!("../../../../tests/fixtures/ce979f36-84f2-4419-b400-971263494cc6.json"),
        ] {
            let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
                include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
                raw,
            ) else {
                panic!("complete tapped-destroy source must lower")
            };
            c.definitions.push(*definition);
        }
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        v.active = Some(PlayerId(0));
        v.turn_owner = PlayerId(0);
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::W, 1.into()), (Color::C, 1.into())]);
        v.objects = vec![
            wipe_tests::card(
                &c,
                "Swift Response",
                v.viewer.0,
                Zone::Hand { owner: v.viewer },
            ),
            wipe_tests::card(&c, "Goliath Sphinx", 0, Zone::Battlefield),
            wipe_tests::card(&c, "Storm Crow", 0, Zone::Battlefield),
            wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
        ];
        v.objects[2].tapped = true;
        v.objects[3].tapped = true;
        (c, v)
    }

    #[test]
    fn tapped_destroy_instant_reacts_and_selects_current_offered_opponent() {
        let (c, mut v) = fixture();
        let before = canonical(&v).unwrap();
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Swift Response".into())
            }
        );
        assert_eq!(canonical(&v).unwrap(), before);
        let viewer = v.viewer.0;
        stack_spell(&mut v, &c, "Swift Response", viewer);
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Swift Response")
            .unwrap();
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("tapped-destroy-payload".into()),
            player: o.view.viewer,
            occurrences: d.spell.as_ref().unwrap().targets.clone(),
            candidates: vec![
                ObjectHandle("Goliath Sphinx".into()),
                ObjectHandle("Storm Crow".into()),
                ObjectHandle("Grizzly Bears".into()),
            ],
        });
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("Storm Crow".into())]
            }
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }

    #[test]
    fn tapped_destroy_reaction_requires_targetability_opposing_control_and_payment() {
        let (c, v) = fixture();
        for case in ["untapped", "own", "shroud", "hexproof", "unaffordable"] {
            let mut v = v.clone();
            match case {
                "untapped" => v.objects[2].tapped = false,
                "own" => v.objects[2].controller = v.viewer,
                "shroud" => v.objects[2].keywords.push(KeywordAbility::Shroud),
                "hexproof" => v.objects[2].keywords.push(KeywordAbility::Hexproof),
                "unaffordable" => {
                    v.players
                        .iter_mut()
                        .find(|p| p.player == v.viewer)
                        .unwrap()
                        .mana
                        .clear();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
        }
    }

    #[test]
    fn tapped_destroy_reaction_preserves_sorcery_and_nonmodal_mandatory_scope() {
        let (c, mut v) = fixture();
        v.objects[0] = wipe_tests::card(
            &c,
            "Assassinate",
            v.viewer.0,
            Zone::Hand { owner: v.viewer },
        );
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::B, 3.into())]);
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Pass
        );
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Assassinate".into())
            }
        );

        // Typed policy-shape controls do not claim additional source support.
        let (c, v) = fixture();
        for case in ["unrestricted", "optional", "modal"] {
            let mut c = c.clone();
            let p = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Swift Response")
                .unwrap()
                .spell
                .as_mut()
                .unwrap();
            match case {
                "unrestricted" => p.targets[0].filter.as_mut().unwrap().tapped = false,
                "optional" => {
                    let I::DestroyPermanent { optional, .. } = &mut p.instructions[0] else {
                        panic!("destroy")
                    };
                    *optional = true;
                }
                "modal" => {
                    let child = p.clone();
                    p.instructions.clear();
                    p.targets.clear();
                    p.modes.push(mtg_contracts::SpellMode { program: child });
                }
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
        }
    }
}

// Public IR/observation probes, not accepted-input games or replay evidence.
mod defensive_untap_policy_tests {
    use super::*;

    fn fixture() -> (Catalog, PermittedView) {
        let mut c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let mut v = wipe_tests::view(&c);
        for raw in [
            include_str!("../../../../tests/fixtures/b795ecfd-31ac-4a2a-9679-97b088932d93.json"),
            include_str!("../../../../tests/fixtures/5820d760-8ed7-4abf-a959-c06ed86a3730.json"),
            include_str!("../../../../tests/fixtures/ce979f36-84f2-4419-b400-971263494cc6.json"),
        ] {
            let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
                include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
                raw,
            ) else {
                panic!("complete untap/target-status source must lower")
            };
            c.definitions.push(*definition);
        }
        c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        c.validate().unwrap();
        v.active = Some(PlayerId(0));
        v.turn_owner = PlayerId(0);
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::DeclareAttackers;
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::W, 1.into())]);
        v.objects = vec![
            wipe_tests::card(
                &c,
                "Burst of Energy",
                v.viewer.0,
                Zone::Hand { owner: v.viewer },
            ),
            wipe_tests::card(&c, "Storm Crow", v.viewer.0, Zone::Battlefield),
            wipe_tests::card(&c, "Goliath Sphinx", v.viewer.0, Zone::Battlefield),
        ];
        v.objects[1].tapped = true;
        v.objects[1].owner = PlayerId(1); // Defense follows current control.
        v.objects[2].tapped = true;
        stack_spell(&mut v, &c, "Swift Response", 0);
        v.stack.last_mut().unwrap().targets = vec![mtg_core::VisibleTarget {
            announced: ObjectHandle("Storm Crow".into()),
            current: Some(ObjectHandle("Storm Crow".into())),
        }];
        (c, v)
    }

    #[test]
    fn defensive_untap_reacts_and_prefers_threatened_handle_over_more_valuable_permanent() {
        let (c, mut v) = fixture();
        let before = canonical(&v).unwrap();
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Burst of Energy".into())
            }
        );
        assert_eq!(canonical(&v).unwrap(), before);
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Burst of Energy")
            .unwrap();
        let p = d.spell.as_ref().unwrap();
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            Some(ObjectHandle("Storm Crow".into()))
        );

        v.objects.remove(0);
        let viewer = v.viewer.0;
        stack_spell(&mut v, &c, "Burst of Energy", viewer);
        v.stack.last_mut().unwrap().cast_complete = false;
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("defensive-untap-payload".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![
                ObjectHandle("Goliath Sphinx".into()),
                ObjectHandle("Storm Crow".into()),
            ],
        });
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("Storm Crow".into())]
            }
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }

    #[test]
    fn defensive_untap_requires_current_enemy_threat_targetability_and_payment() {
        let (c, v) = fixture();
        for case in [
            "empty",
            "own",
            "incomplete",
            "untapped",
            "enemy-controlled",
            "shroud",
            "hexproof",
            "missing",
            "no-current",
            "unaffordable",
            "unrelated-top",
        ] {
            let mut v = v.clone();
            match case {
                "empty" => v.stack.clear(),
                "own" => v.stack[0].controller = v.viewer,
                "incomplete" => v.stack[0].cast_complete = false,
                "untapped" => v.objects[1].tapped = false,
                "enemy-controlled" => v.objects[1].controller = PlayerId(0),
                "shroud" => v.objects[1].keywords.push(KeywordAbility::Shroud),
                "hexproof" => v.objects[1].keywords.push(KeywordAbility::Hexproof),
                "missing" => {
                    v.objects.remove(1);
                }
                "no-current" => v.stack[0].targets[0].current = None,
                "unaffordable" => v
                    .players
                    .iter_mut()
                    .find(|p| p.player == v.viewer)
                    .unwrap()
                    .mana
                    .clear(),
                "unrelated-top" => stack_spell(&mut v, &c, "Lightning Bolt", 0),
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
        }
    }

    #[test]
    fn defensive_untap_respects_incoming_and_outgoing_spell_scope() {
        let (c, v) = fixture();
        for case in [
            "unrestricted",
            "optional",
            "ability",
            "trigger",
            "sorcery-untap",
            "modal-untap",
        ] {
            let mut c = c.clone();
            let mut v = v.clone();
            match case {
                "unrestricted" | "optional" => {
                    let p = c
                        .definitions
                        .iter_mut()
                        .find(|d| d.name == "Swift Response")
                        .unwrap()
                        .spell
                        .as_mut()
                        .unwrap();
                    if case == "unrestricted" {
                        p.targets[0].filter.as_mut().unwrap().tapped = false;
                    } else {
                        let I::DestroyPermanent { optional, .. } = &mut p.instructions[0] else {
                            panic!("destroy")
                        };
                        *optional = true;
                    }
                }
                "ability" => {
                    let id = c
                        .definitions
                        .iter()
                        .find(|d| d.name == "Samite Healer")
                        .unwrap()
                        .oracle_id
                        .clone();
                    v.stack[0].ability = Some(mtg_core::VisibleAbility {
                        oracle_id: id,
                        ability: 0,
                        handle: StackHandle("ability".into()),
                        source: mtg_core::VisibleTarget {
                            announced: ObjectHandle("source".into()),
                            current: None,
                        },
                    });
                }
                "trigger" => {
                    let id = c
                        .definitions
                        .iter()
                        .find(|d| d.name == "Blood Artist")
                        .unwrap()
                        .oracle_id
                        .clone();
                    v.stack[0].triggered = Some(mtg_core::VisibleTriggered {
                        oracle_id: id,
                        ability: 0,
                        handle: StackHandle("trigger".into()),
                        source: mtg_core::VisibleTarget {
                            announced: ObjectHandle("source".into()),
                            current: None,
                        },
                    });
                }
                "sorcery-untap" => {
                    let d = c
                        .definitions
                        .iter_mut()
                        .find(|d| d.name == "Burst of Energy")
                        .unwrap();
                    d.characteristics.kind = CardKind::Sorcery;
                    d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
                }
                "modal-untap" => {
                    let p = c
                        .definitions
                        .iter_mut()
                        .find(|d| d.name == "Burst of Energy")
                        .unwrap()
                        .spell
                        .as_mut()
                        .unwrap();
                    let child = p.clone();
                    p.targets.clear();
                    p.instructions.clear();
                    p.modes = vec![
                        mtg_contracts::SpellMode {
                            program: child.clone(),
                        },
                        mtg_contracts::SpellMode { program: child },
                    ];
                }
                _ => unreachable!(),
            }
            assert_eq!(
                strategy(&c).priority(&observation(v)).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
        }

        // A sorcery removal already on the stack is also an ordinary threat;
        // only the responding untap spell must have instant timing.
        let mut v = v;
        v.stack.clear();
        v.objects.retain(|o| o.zone != Zone::Stack);
        stack_spell(&mut v, &c, "Assassinate", 0);
        v.stack[0].targets = vec![mtg_core::VisibleTarget {
            announced: ObjectHandle("Storm Crow".into()),
            current: Some(ObjectHandle("Storm Crow".into())),
        }];
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Burst of Energy".into())
            }
        );
    }

    #[test]
    fn defensive_untap_preference_never_invents_stale_or_unoffered_targets() {
        let (c, mut v) = fixture();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Burst of Energy")
            .unwrap();
        let p = d.spell.as_ref().unwrap();
        let offered = [ObjectHandle("Goliath Sphinx".into())];
        // Direct preference is absent; the existing mandatory Targets handler
        // retains its legal-candidate fallback after an initiated cast.
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&offered)),
            None
        );
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&[])),
            None
        );
        v.stack[0].targets[0].current = Some(ObjectHandle("stale-incarnation".into()));
        assert_eq!(
            strategy(&c).object_target(&v, d, p, &p.targets[0], None),
            None
        );
        v.stack.clear();
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.turn.step = TurnStep::PrecombatMain;
        assert_eq!(
            strategy(&c).priority(&observation(v)).unwrap(),
            PlayerResponse::Pass,
            "ordinary development must not spend untap on mana/ramp without a defensive threat"
        );
    }
}

#[path = "combined_type_policy_tests.rs"]
mod combined_type_policy_tests;

// Synthetic public payload probes over the admitted complete source catalog;
// these do not claim accepted-pod gameplay or replay coverage.
mod permanent_owner_hand_policy_tests {
    use super::*;

    fn fixture(name: &str, target: &str) -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
        assert_eq!(
            &c,
            mtg_core::match_profile("commander-sacrifice-v1")
                .unwrap()
                .catalog()
        );
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::PermanentOwnerHandMoveV1)
        );
        let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
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
            wipe_tests::card(&c, name, v.viewer.0, Zone::Hand { owner: v.viewer }),
            wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield),
            wipe_tests::card(&c, target, 0, Zone::Battlefield),
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
                PlayerResponse::Cast {
                    card: ObjectHandle(name.into())
                }
            );
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], None),
                Some(selected.clone())
            );
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&[selected.clone()])),
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
                    .push(wipe_tests::card(&c, "Island", 0, Zone::Battlefield));
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
            o.decision = Some(PlayerRequest::Targets {
                token: mtg_core::DecisionToken("permanent-bounce-payload".into()),
                player: o.view.viewer,
                occurrences: p.targets.clone(),
                candidates: vec![ObjectHandle("Grizzly Bears".into()), selected.clone()],
            });
            let before = canonical(&o).unwrap();
            assert_eq!(
                strategy(&c).choose(&o).unwrap(),
                PlayerResponse::Targets {
                    objects: vec![selected]
                }
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
                    PlayerResponse::Pass,
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
        let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
        assert_eq!(
            &c,
            mtg_core::match_profile("commander-blocking-targets-v1")
                .unwrap()
                .catalog()
        );
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
        let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
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
        let mut source = wipe_tests::card(&c, "Icy Manipulator", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("own-icy".into());
        source.owner = PlayerId(2);
        source.attack_ready = false; // An artifact has no creature tap restriction.
        let mut tapped = wipe_tests::card(&c, "Icy Manipulator", 0, Zone::Battlefield);
        tapped.handle = ObjectHandle("enemy-icy".into());
        tapped.tapped = true;
        let mut enemy = wipe_tests::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
        enemy.handle = ObjectHandle("enemy-defender".into());
        let mut own =
            wipe_tests::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-defender".into());
        v.objects = vec![
            source,
            wipe_tests::card(&c, "Plains", v.viewer.0, Zone::Battlefield),
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
            PlayerResponse::ActivateAbility {
                object: ObjectHandle("own-icy".into()),
                ability: 0
            }
        );
        assert_eq!(s.mana_plan.len(), 1);
        assert_eq!(
            s.mana_plan[0].source,
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
            x: None,
            mana_value: None,
            mode: None,
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
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("activated-tap-payload".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![
                ObjectHandle("enemy-icy".into()),
                ObjectHandle("own-defender".into()),
                ObjectHandle("enemy-defender".into()),
            ],
        });
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("enemy-defender".into())]
            }
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }

    #[test]
    fn activated_tap_rejects_noop_or_unready_choices_without_changing_cantrip_policy() {
        let (c, v) = fixture();
        for case in [
            "all-tapped",
            "source-tapped",
            "friendly",
            "shroud",
            "hexproof",
            "enchantment",
            "graveyard",
            "departed",
            "unaffordable",
            "opponent-priority",
        ] {
            let mut v = v.clone();
            match case {
                "all-tapped" => v.objects[3].tapped = true,
                "source-tapped" => v.objects[0].tapped = true,
                "friendly" => v.objects[3].controller = v.viewer,
                "shroud" => v.objects[3].keywords.push(KeywordAbility::Shroud),
                "hexproof" => v.objects[3].keywords.push(KeywordAbility::Hexproof),
                "enchantment" => {
                    v.objects[3] = wipe_tests::card(&c, "Furnace of Rath", 0, Zone::Battlefield)
                }
                "graveyard" => v.objects[3].zone = Zone::Graveyard { owner: PlayerId(0) },
                "departed" => {
                    v.objects.remove(3);
                }
                "unaffordable" => v.objects[1].tapped = true,
                "opponent-priority" => {
                    v.active = Some(PlayerId(0));
                    v.turn_owner = PlayerId(0);
                }
                _ => unreachable!(),
            }
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Icy Manipulator")
            .unwrap();
        let p = &d.activated[0].program;
        for offered in [
            vec![],
            vec![ObjectHandle("stale-incarnation".into())],
            vec![ObjectHandle("own-defender".into())],
        ] {
            assert_eq!(
                strategy(&c).object_target(&v, d, p, &p.targets[0], Some(&offered)),
                None,
                "direct preference cannot invent an unoffered opponent; mandatory-choice fallback is separate"
            );
        }
        // Unlike a pure tap activation, Pressure Point still draws from a
        // legal already-tapped creature. Preserve that existing spell policy.
        let mut cantrip = v;
        cantrip.objects = vec![
            wipe_tests::card(
                &c,
                "Pressure Point",
                cantrip.viewer.0,
                Zone::Hand {
                    owner: cantrip.viewer,
                },
            ),
            wipe_tests::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield),
        ];
        cantrip.objects[1].tapped = true;
        cantrip
            .players
            .iter_mut()
            .find(|p| p.player == cantrip.viewer)
            .unwrap()
            .mana = BTreeMap::from([(Color::W, 2.into())]);
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Pressure Point")
            .unwrap();
        let p = d.spell.as_ref().unwrap();
        assert!(
            strategy(&c)
                .object_target(&cantrip, d, p, &p.targets[0], None)
                .is_some()
        );
        assert_eq!(
            strategy(&c).priority(&observation(cantrip)).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Pressure Point".into())
            }
        );
    }
}

// Synthetic permitted-view probes over admitted source IR; paid game inputs
// and replay are qualified separately by core's accepted-pod witnesses.
mod activated_headless_policy_tests {
    use super::*;

    fn baseline() -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
        assert_eq!(
            &c,
            mtg_core::match_profile("commander-blocking-targets-v1")
                .unwrap()
                .catalog()
        );
        let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
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
            x: None,
            mana_value: None,
            mode: None,
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
        let mut source = wipe_tests::card(&c, "Royal Assassin", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("royal-source".into());
        let mut tapped = wipe_tests::card(&c, "Isamaru, Hound of Konda", 0, Zone::Battlefield);
        tapped.handle = ObjectHandle("enemy-tapped".into());
        tapped.tapped = true;
        let mut untapped = wipe_tests::card(&c, "Grizzly Bears", 0, Zone::Battlefield);
        untapped.handle = ObjectHandle("enemy-untapped".into());
        let mut own =
            wipe_tests::card(&c, "Isamaru, Hound of Konda", v.viewer.0, Zone::Battlefield);
        own.handle = ObjectHandle("own-tapped".into());
        own.tapped = true;
        v.objects = vec![source, tapped, untapped, own];
        let before = canonical(&v).unwrap();
        let mut s = strategy(&c);
        assert_eq!(
            s.priority(&observation(v.clone())).unwrap(),
            PlayerResponse::ActivateAbility {
                object: ObjectHandle("royal-source".into()),
                ability: 0
            }
        );
        assert!(s.mana_plan.is_empty(), "source-tap has no mana payment");
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
                PlayerResponse::Pass,
                "{case}"
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        proposed_activation(&mut v, d, "royal-source");
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("royal-payload".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![
                ObjectHandle("own-tapped".into()),
                ObjectHandle("enemy-tapped".into()),
            ],
        });
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![ObjectHandle("enemy-tapped".into())]
            }
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }

    #[test]
    fn ertai_policy_reacts_to_completed_enemy_spell_with_independent_four_island_plan() {
        let (c, mut v) = baseline();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Ertai, Wizard Adept")
            .unwrap();
        assert!(
            d.capabilities
                .contains(&mtg_contracts::Capability::ActivatedSpellCounterV1)
        );
        assert!(
            matches!(&d.activated[0].cost, ActivationCost::Composite { costs }
            if costs.tap && !costs.sacrifice && costs.mana.as_ref().is_some_and(|m| m.generic == Quantity::from(2) && m.colored == [Color::U, Color::U]))
        );
        let p = &d.activated[0].program;
        v.active = Some(PlayerId(0));
        v.turn_owner = PlayerId(0);
        let mut source = wipe_tests::card(&c, "Ertai, Wizard Adept", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("ertai-source".into());
        v.objects = vec![source];
        for n in 0..4 {
            let mut island = wipe_tests::card(&c, "Island", v.viewer.0, Zone::Battlefield);
            island.handle = ObjectHandle(format!("counter-island-{n}"));
            v.objects.push(island);
        }
        // cast_complete is the public completed-payment/cast boundary; this
        // synthetic payload does not itself claim a gameplay payment receipt.
        stack_spell(&mut v, &c, "Icy Manipulator", 0);
        assert!(v.stack[0].cast_complete);
        let enemy_spell = StackHandle("Icy Manipulator".into());
        let before = canonical(&v).unwrap();
        let mut s = strategy(&c);
        assert_eq!(
            s.priority(&observation(v.clone())).unwrap(),
            PlayerResponse::ActivateAbility {
                object: ObjectHandle("ertai-source".into()),
                ability: 0
            }
        );
        let mut plan = s
            .mana_plan
            .iter()
            .map(|p| p.source.0.clone())
            .collect::<Vec<_>>();
        plan.sort();
        assert_eq!(
            plan,
            (0..4)
                .map(|n| format!("counter-island-{n}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            s.stack_target(&v, &p.targets[0], None),
            Some(enemy_spell.clone())
        );
        assert_eq!(
            s.stack_target(&v, &p.targets[0], Some(&[StackHandle("foreign".into())])),
            None
        );
        assert_eq!(canonical(&v).unwrap(), before);
        for case in [
            "fresh",
            "source-tapped",
            "three-islands",
            "empty-stack",
            "own-spell",
            "incomplete",
            "ability",
        ] {
            let mut v = v.clone();
            match case {
                "fresh" => v.objects[0].attack_ready = false,
                "source-tapped" => v.objects[0].tapped = true,
                "three-islands" => v.objects[1].tapped = true,
                "empty-stack" => v.stack.clear(),
                "own-spell" => v.stack[0].controller = v.viewer,
                "incomplete" => v.stack[0].cast_complete = false,
                "ability" => {
                    v.stack[0].spell = None;
                    v.stack[0].ability = Some(mtg_core::VisibleAbility {
                        oracle_id: d.oracle_id.clone(),
                        ability: 0,
                        handle: enemy_spell.clone(),
                        source: mtg_core::VisibleTarget {
                            announced: ObjectHandle("departed-counter-source".into()),
                            current: None,
                        },
                    });
                }
                _ => unreachable!(),
            }
            let before = canonical(&v).unwrap();
            assert_eq!(
                strategy(&c).priority(&observation(v.clone())).unwrap(),
                PlayerResponse::Pass,
                "{case}"
            );
            assert_eq!(canonical(&v).unwrap(), before);
        }
        proposed_activation(&mut v, d, "ertai-source");
        assert_eq!(
            strategy(&c).stack_target(
                &v,
                &p.targets[0],
                Some(&[StackHandle("policy-activation".into())])
            ),
            None,
            "direct preference cannot turn an offered ability handle into a spell target; mandatory fallback is separate"
        );
        let mut o = observation(v);
        o.decision = Some(PlayerRequest::StackTargets {
            token: mtg_core::DecisionToken("ertai-payload".into()),
            player: o.view.viewer,
            occurrences: p.targets.clone(),
            candidates: vec![enemy_spell.clone()],
        });
        let before = canonical(&o).unwrap();
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::StackTargets {
                entries: vec![enemy_spell]
            }
        );
        assert_eq!(canonical(&o).unwrap(), before);
    }
}

// Synthetic public payload probes over admitted source IR. Core independently
// qualifies genuine paid pod inputs, source tapping and replay.
mod temporal_adept_policy_tests {
    use super::*;

    fn fixture(target: &str) -> (Catalog, PermittedView) {
        let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
        assert_eq!(
            &c,
            mtg_core::match_profile("commander-blocking-targets-v1")
                .unwrap()
                .catalog()
        );
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
        let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
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
        let mut source = wipe_tests::card(&c, "Temporal Adept", v.viewer.0, Zone::Battlefield);
        source.handle = ObjectHandle("return-source".into());
        source.owner = PlayerId(2);
        v.objects = vec![source];
        for n in 0..3 {
            let mut island = wipe_tests::card(&c, "Island", v.viewer.0, Zone::Battlefield);
            island.handle = ObjectHandle(format!("return-island-{n}"));
            v.objects.push(island);
        }
        let mut enemy = wipe_tests::card(&c, target, 0, Zone::Battlefield);
        enemy.handle = ObjectHandle("enemy-return-target".into());
        enemy.owner = v.viewer;
        enemy.tapped = true; // Preference follows current control, not owner/status.
        let mut own = wipe_tests::card(&c, "Grizzly Bears", v.viewer.0, Zone::Battlefield);
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
                PlayerResponse::ActivateAbility {
                    object: ObjectHandle("return-source".into()),
                    ability: 0
                }
            );
            let mut plan = s
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
                x: None,
                mana_value: None,
                mode: None,
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
            o.decision = Some(PlayerRequest::Targets {
                token: mtg_core::DecisionToken("temporal-payload".into()),
                player: o.view.viewer,
                occurrences: p.targets.clone(),
                candidates: vec![
                    ObjectHandle("own-bears".into()),
                    ObjectHandle("enemy-return-target".into()),
                ],
            });
            let before = canonical(&o).unwrap();
            assert_eq!(
                strategy(&c).choose(&o).unwrap(),
                PlayerResponse::Targets {
                    objects: vec![ObjectHandle("enemy-return-target".into())]
                }
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
                    v.objects[1] = wipe_tests::card(&c, "Plains", v.viewer.0, Zone::Battlefield)
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
                PlayerResponse::Pass,
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
        let c = mtg_compiler::catalog::embedded_catalog("blocking-matches").unwrap();
        let mut v = wipe_tests::view(&mtg_compiler::catalog::embedded_catalog("wipes").unwrap());
        v.active = Some(v.viewer);
        v.turn_owner = v.viewer;
        v.priority = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        v.turn.lands_played = 1;
        let source = wipe_tests::card(&c, "Seeker of Skybreak", v.viewer.0, Zone::Battlefield);
        let mut elf = wipe_tests::card(&c, "Llanowar Elves", v.viewer.0, Zone::Battlefield);
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
            PlayerResponse::ActivateAbility {
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
            PlayerResponse::Pass
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
                PlayerResponse::Pass,
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
                3 => v.objects[1] = wipe_tests::card(&c, "Forest", v.viewer.0, Zone::Battlefield),
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
                PlayerResponse::Pass,
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
            PlayerResponse::Pass
        );
        v.objects[0].tapped = true;
        v.objects[1].tapped = false;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::Pass
        );
        v.objects[0].tapped = false;
        let mut creature =
            wipe_tests::card(&c, "Yargle and Multani", v.viewer.0, Zone::Battlefield);
        creature.tapped = true;
        v.objects.push(creature);
        v.turn.step = TurnStep::PostcombatMain;
        assert_eq!(
            strategy(&c).priority(&observation(v.clone())).unwrap(),
            PlayerResponse::ActivateAbility {
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
    let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let baseline = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    for (spell, protected, ordinary) in [
        ("Murder", "Darksteel Myr", "Grizzly Bears"),
        ("Shatter", "Darksteel Relic", "Sol Ring"),
    ] {
        let mut v = wipe_tests::view(&baseline);
        v.active = Some(v.viewer);
        v.turn.step = TurnStep::PrecombatMain;
        let d = c.definitions.iter().find(|d| d.name == spell).unwrap();
        let p = d.spell.as_ref().unwrap();
        v.objects = vec![wipe_tests::card(&c, protected, 0, Zone::Battlefield)];
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
            view.objects.push(wipe_tests::card(
                &c,
                spell,
                view.viewer.0,
                Zone::Hand { owner: view.viewer },
            ));
            strategy(&c).priority(&observation(view)).unwrap()
        };
        assert_eq!(initiation(&v), PlayerResponse::Pass);
        // Existing mandatory choice fallback remains legal after announcement.
        let viewer = v.viewer;
        stack_spell(&mut v, &c, spell, viewer.0);
        let mut o = observation(v.clone());
        o.decision = Some(PlayerRequest::Targets {
            token: mtg_core::DecisionToken("public-announced".into()),
            player: viewer,
            occurrences: p.targets.clone(),
            candidates: vec![h.clone()],
        });
        assert_eq!(
            strategy(&c).choose(&o).unwrap(),
            PlayerResponse::Targets {
                objects: vec![h.clone()]
            }
        );
        v.stack.clear();
        v.objects.retain(|o| o.zone == Zone::Battlefield);
        v.objects
            .push(wipe_tests::card(&c, ordinary, 0, Zone::Battlefield));
        let ordinary_handle = v.objects[1].handle.clone();
        assert_eq!(
            initiation(&v),
            PlayerResponse::Cast {
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
            .push(mtg_contracts::Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: 0,
            });
        v.objects[0] = wipe_tests::card(&c, protected, 0, Zone::Battlefield);
        assert!(strategy(&c).definition_ready(&v, &compound));
    }
}

#[test]
fn indestructible_policy_exile_bounce_and_real_artifact_sacrifice_keep_preferences() {
    let c = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let baseline = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
    let mut v = wipe_tests::view(&baseline);
    v.active = Some(v.viewer);
    v.turn.step = TurnStep::PrecombatMain;
    for (spell, target) in [
        ("Swords to Plowshares", "Darksteel Myr"),
        ("Unsummon", "Darksteel Myr"),
        ("Revoke Existence", "Darksteel Relic"),
        ("Regress", "Darksteel Relic"),
    ] {
        v.objects = vec![wipe_tests::card(&c, target, 0, Zone::Battlefield)];
        let d = c.definitions.iter().find(|d| d.name == spell).unwrap();
        assert!(strategy(&c).definition_ready(&v, d), "{spell}");
    }
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Shrapnel Blast")
        .unwrap();
    let p = d.spell.as_ref().unwrap();
    v.players
        .iter_mut()
        .find(|p| p.player == PlayerId(0))
        .unwrap()
        .life = 5.into();
    v.objects = vec![wipe_tests::card(
        &c,
        "Darksteel Relic",
        v.viewer.0,
        Zone::Battlefield,
    )];
    assert!(strategy(&c).definition_ready(&v, d));
    let h = v.objects[0].handle.clone();
    assert_eq!(
        strategy(&c)
            .sacrifice_objects(&v, p.additional_sacrifice.as_ref().unwrap())
            .iter()
            .map(|o| o.handle.clone())
            .collect::<Vec<_>>(),
        vec![h.clone()]
    );
    let viewer = v.viewer;
    stack_spell(&mut v, &c, "Shrapnel Blast", viewer.0);
    assert_eq!(
        strategy(&c)
            .sacrifice_choice(&v, std::slice::from_ref(&h))
            .unwrap(),
        h
    );
    v.objects[0].controller = PlayerId(0);
    assert!(!strategy(&c).definition_ready(&v, d));
}

#[path = "plate_policy_tests.rs"]
mod plate_policy_tests;

#[path = "temporary_protection_policy_tests.rs"]
mod temporary_protection_policy_tests;

#[path = "temporary_flight_policy_tests.rs"]
mod temporary_flight_policy_tests;
