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
