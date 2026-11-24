//! CR 305.2, 508.1c–d, 509.1b, 514.1, 601.2f, 611.3, 613 and 700.5.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{
    Allowance, Atom, Change, Cmp, CombatRequirement, Cond, HandSizeChange, Pred, Scope,
    SpellCostChange, StaticRule,
};

fn board(abilities: Vec<(&str, StaticEffect)>) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for (name, effect) in abilities {
        let definition = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap();
        definition.source_labels = None;
        definition.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        });
        definition.capabilities = definition.requirements();
    }
    Board::new(catalog, "static-rules")
}

fn hand(owner: u8) -> Zone {
    Zone::Hand {
        owner: PlayerId(owner),
    }
}

fn creatures() -> Scope {
    Scope::Matching {
        filter: Pred::creatures(),
    }
}

fn characteristics(game: &Game, card: u32) -> Characteristics {
    game.characteristics(Driver::object(game, card)).unwrap()
}

fn pass_round(game: &mut Game) {
    for _ in 0..4 {
        Driver::pass(game);
    }
    let _ = Driver::request(game, u64::MAX);
}

#[test]
fn static_layers_reapply_in_order_and_later_timestamps_win() {
    let blue = Scope::Matching {
        filter: Pred(vec![
            Atom::Type {
                kind: CardKind::Creature,
            },
            Atom::Color { color: Color::U },
        ]),
    };
    let mut game = Game::new(
        board(vec![
            (
                "Darksteel Relic",
                StaticEffect::continuous(
                    creatures(),
                    vec![
                        Change::SetColors {
                            colors: vec![Color::R],
                        },
                        Change::SetPowerToughness {
                            power: Amount::fixed(3),
                            toughness: Amount::fixed(4),
                        },
                    ],
                ),
            ),
            (
                "Sol Ring",
                StaticEffect::continuous(
                    creatures(),
                    vec![
                        Change::SetColors {
                            colors: vec![Color::U],
                        },
                        Change::SetPowerToughness {
                            power: Amount::fixed(5),
                            toughness: Amount::fixed(6),
                        },
                    ],
                ),
            ),
            (
                "Icy Manipulator",
                StaticEffect::continuous(
                    blue,
                    vec![
                        Change::PowerToughness {
                            power: Amount::fixed(2),
                            toughness: Amount::fixed(-1),
                        },
                        Change::AddKeywords {
                            keywords: vec![KeywordAbility::Flying],
                        },
                    ],
                ),
            ),
        ])
        .card("Grizzly Bears", 0, 0, Zone::Battlefield)
        .card("Icy Manipulator", 0, 0, Zone::Battlefield)
        .card("Darksteel Relic", 0, 0, hand(0))
        .card("Sol Ring", 0, 0, hand(0))
        .card("Forest", 0, 0, Zone::Battlefield)
        .prepare(),
    );

    Driver::cast(&mut game, CardId(2));
    Driver::pay(&mut game, &[]);
    pass_round(&mut game);
    let red = characteristics(&game, 0);
    assert_eq!(
        (red.colors, red.power, red.toughness),
        (vec![Color::R], Some(3.into()), Some(4.into()))
    );

    Driver::cast(&mut game, CardId(3));
    Driver::pay(&mut game, &[4]);
    pass_round(&mut game);
    let blue = characteristics(&game, 0);
    assert_eq!(
        (blue.colors, blue.power, blue.toughness),
        (vec![Color::U], Some(7.into()), Some(5.into()))
    );
    assert!(
        game.current_keywords(Driver::object(&game, 0))
            .contains(&KeywordAbility::Flying)
    );
}

#[test]
fn static_conditional_devotion_and_active_turn_read_live_board() {
    let controller = PlayerExpr::static_controller();
    let comparison = |left, bound| Cond::Compare {
        left,
        cmp: Cmp::AtLeast,
        right: Amount::fixed(bound),
    };
    let mut game = Game::new(
        board(vec![(
            "Darksteel Relic",
            StaticEffect::Continuous {
                affects: creatures(),
                changes: vec![Change::PowerToughness {
                    power: Amount::fixed(1),
                    toughness: Amount::fixed(1),
                }],
                condition: Some(Cond::All {
                    conditions: vec![
                        Cond::ActivePlayer {
                            player: controller.clone(),
                        },
                        comparison(
                            Amount::Devotion {
                                player: controller.clone(),
                                colors: vec![Color::G],
                            },
                            2,
                        ),
                        comparison(
                            Amount::Count {
                                set: SetExpr::graveyard_of(controller),
                            },
                            1,
                        ),
                    ],
                }),
            },
        )])
        .card("Darksteel Relic", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 0, 0, hand(0))
        .card("Forest", 0, 0, Zone::Battlefield)
        .card("Forest", 0, 0, Zone::Battlefield)
        .card("Forest", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
        .libraries("Forest", 2)
        .prepare(),
    );
    assert_eq!(characteristics(&game, 1).power, Some(2.into()));
    Driver::cast(&mut game, CardId(2));
    Driver::pay(&mut game, &[3, 4]);
    pass_round(&mut game);
    assert_eq!(characteristics(&game, 1).power, Some(3.into()));

    for _ in 0..80 {
        let decision = Driver::request(&mut game, u64::MAX);
        if game.state.rules.turn.number > 1 {
            assert_eq!(characteristics(&game, 1).power, Some(2.into()));
            return;
        }
        if matches!(decision.reason, Reason::Attackers) {
            Driver::answer(&mut game, u64::MAX, decision.attack(vec![]));
        } else {
            Driver::pass(&mut game);
        }
    }
    panic!("next turn was not reached");
}

#[test]
fn static_cost_modification_freezes_total_and_preserves_printed_mana_value() {
    let caster = PlayerExpr::static_controller();
    let tax = StaticEffect::rule(StaticRule::SpellCost {
        affects: creatures(),
        caster: caster.clone(),
        change: SpellCostChange::Increase {
            mana: ManaCost {
                generic: 2.into(),
                colored: vec![Color::G],
            },
        },
        multiplier: None,
    });
    let reduction = StaticEffect::rule(StaticRule::SpellCost {
        affects: creatures(),
        caster,
        change: SpellCostChange::Reduce {
            mana: ManaCost {
                generic: 10.into(),
                colored: vec![Color::W],
            },
        },
        multiplier: None,
    });
    let mut game = Game::new(
        board(vec![("Darksteel Relic", tax), ("Sol Ring", reduction)])
            .card("Darksteel Relic", 0, 0, Zone::Battlefield)
            .card("Sol Ring", 0, 0, Zone::Battlefield)
            .card("Grizzly Bears", 0, 0, hand(0))
            .card("Forest", 0, 0, Zone::Battlefield)
            .card("Forest", 0, 0, Zone::Battlefield)
            .prepare(),
    );
    Driver::cast(&mut game, CardId(2));
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(matches!(decision.reason, Reason::Mana { .. }));
    let paid = ManaCost {
        generic: 0.into(),
        colored: vec![Color::G, Color::G],
    };
    assert_eq!(game.top().unwrap().total_cost, paid);
    assert_eq!(characteristics(&game, 2).printed_mana_value(), 2.into());
    Driver::pay(&mut game, &[3, 4]);
    assert_eq!(game.top().unwrap().paid, Some(paid));
    assert_recoverable(JournalCase::from(&game));
}

#[test]
fn static_extra_land_plays_use_the_live_allowance() {
    let mut game = Game::new(
        board(vec![(
            "Darksteel Relic",
            StaticEffect::rule(StaticRule::AdditionalLandPlays {
                player: PlayerExpr::static_controller(),
                allowance: Allowance::Additional { count: 1.into() },
            }),
        )])
        .card("Darksteel Relic", 0, 0, Zone::Battlefield)
        .card("Forest", 0, 0, hand(0))
        .card("Forest", 0, 0, hand(0))
        .card("Forest", 0, 0, hand(0))
        .prepare(),
    );
    for card in [1, 2] {
        let object = Driver::object(&game, card);
        Driver::action(&mut game, Action::PlayLand { object });
    }
    let choice = Driver::request(&mut game, u64::MAX);
    assert!(
        choice
            .index(&Candidate::Action {
                action: Action::PlayLand {
                    object: Driver::object(&game, 3)
                },
            })
            .is_none()
    );
    assert_eq!(game.state.rules.turn.lands_played, 2);
}

#[test]
fn static_hand_size_controls_cleanup_discard() {
    for (change, expected) in [
        (HandSizeChange::Unlimited, false),
        (HandSizeChange::Set { size: 6.into() }, true),
    ] {
        let mut board = board(vec![(
            "Darksteel Relic",
            StaticEffect::rule(StaticRule::MaximumHandSize {
                player: PlayerExpr::static_controller(),
                change,
            }),
        )])
        .step(TurnStep::End)
        .card("Darksteel Relic", 0, 0, Zone::Battlefield)
        .libraries("Forest", 2);
        for _ in 0..8 {
            board = board.card("Forest", 0, 0, hand(0));
        }
        let mut game = Game::new(board.prepare());
        pass_round(&mut game);
        let decision = Driver::request(&mut game, u64::MAX);
        assert_eq!(matches!(decision.reason, Reason::Discard), expected);
        if expected {
            assert!(matches!(
                game.state.rules.frame,
                Frame::Discard { count: 2, .. }
            ));
        }
    }
}

#[test]
fn static_combat_restrictions_override_attack_requirements() {
    let effect = |requirement| {
        StaticEffect::rule(StaticRule::Combat {
            affects: Scope::Source,
            requirement,
        })
    };
    let mut game = Game::new(
        board(vec![
            ("Grizzly Bears", effect(CombatRequirement::AttackEachCombat)),
            (
                "Isamaru, Hound of Konda",
                effect(CombatRequirement::AttackEachCombat),
            ),
            (
                "Isamaru, Hound of Konda",
                effect(CombatRequirement::CantAttack),
            ),
        ])
        .step(TurnStep::BeginningCombat)
        .card("Grizzly Bears", 0, 0, Zone::Battlefield)
        .card("Isamaru, Hound of Konda", 0, 0, Zone::Battlefield)
        .prepare(),
    );
    pass_round(&mut game);
    let choice = Driver::request(&mut game, u64::MAX);
    assert!(matches!(choice.reason, Reason::Attackers));
    assert!(
        game.respond(choice.player, choice.id, choice.attack(vec![]))
            .is_err()
    );
    let bear = Driver::object(&game, 0);
    Driver::answer(
        &mut game,
        u64::MAX,
        choice.attack(vec![(bear, PlayerId(1))]),
    );
    let _ = Driver::request(&mut game, u64::MAX);
    assert_eq!(game.state.rules.combat.len(), 1);
    assert_eq!(game.state.rules.combat[0].attacker, bear);
}

#[test]
fn static_type_changes_feed_layer_six_and_flash_permission_respects_caster() {
    let artifact_creatures = Scope::Matching {
        filter: Pred(vec![
            Atom::Type {
                kind: CardKind::Creature,
            },
            Atom::Type {
                kind: CardKind::Artifact,
            },
        ]),
    };
    let mut game = Game::new(
        board(vec![
            (
                "Darksteel Relic",
                StaticEffect::continuous(
                    Scope::Matching {
                        filter: Pred(vec![Atom::ControlledBy {
                            player: PlayerExpr::static_controller(),
                        }]),
                    },
                    vec![Change::AddTypes {
                        kinds: vec![CardKind::Artifact],
                    }],
                ),
            ),
            (
                "Darksteel Relic",
                StaticEffect::continuous(
                    artifact_creatures,
                    vec![Change::AddKeywords {
                        keywords: vec![KeywordAbility::Reach],
                    }],
                ),
            ),
            (
                "Darksteel Relic",
                StaticEffect::rule(StaticRule::CastAsThoughFlash {
                    affects: creatures(),
                    caster: PlayerExpr::static_controller(),
                }),
            ),
        ])
        .step(TurnStep::Upkeep)
        .card("Darksteel Relic", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 0, 0, hand(0))
        .card("Forest", 0, 0, Zone::Battlefield)
        .card("Forest", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 1, 1, hand(1))
        .prepare(),
    );
    assert!(!characteristics(&game, 1).has_type(CardKind::Artifact));
    Driver::cast(&mut game, CardId(1));
    Driver::pay(&mut game, &[2, 3]);
    Driver::pass(&mut game);
    let decision = Driver::request(&mut game, u64::MAX);
    assert_eq!(decision.player, PlayerId(1));
    assert!(
        decision
            .index(&Candidate::Action {
                action: Action::Cast {
                    card: Driver::object(&game, 4)
                },
            })
            .is_none()
    );
    for _ in 0..3 {
        Driver::pass(&mut game);
    }
    let _ = Driver::request(&mut game, u64::MAX);
    assert!(characteristics(&game, 1).has_type(CardKind::Artifact));
    assert!(
        game.current_keywords(Driver::object(&game, 1))
            .contains(&KeywordAbility::Reach)
    );
}

#[test]
fn static_combat_prohibition_applies_to_each_matching_attacker() {
    let controlled_creatures = Scope::Matching {
        filter: Pred(vec![
            Atom::Type {
                kind: CardKind::Creature,
            },
            Atom::ControlledBy {
                player: PlayerExpr::static_controller(),
            },
        ]),
    };
    let mut game = Game::new(
        board(vec![(
            "Darksteel Relic",
            StaticEffect::rule(StaticRule::Combat {
                affects: controlled_creatures,
                requirement: CombatRequirement::CantBeBlocked,
            }),
        )])
        .step(TurnStep::BeginningCombat)
        .card("Darksteel Relic", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 0, 0, Zone::Battlefield)
        .card("Grizzly Bears", 1, 1, Zone::Battlefield)
        .prepare(),
    );
    pass_round(&mut game);
    let decision = Driver::request(&mut game, u64::MAX);
    let attacker = Driver::object(&game, 1);
    Driver::answer(
        &mut game,
        u64::MAX,
        decision.attack(vec![(attacker, PlayerId(1))]),
    );
    pass_round(&mut game);
    let decision = Driver::request(&mut game, u64::MAX);
    assert!(matches!(decision.reason, Reason::Blockers));
    let Shape::Pick { slots, .. } = &decision.shape else {
        panic!("expected blocker slots");
    };
    assert_eq!(slots.len(), 1);
    assert!(slots[0].allowed.is_empty());
    Driver::answer(&mut game, u64::MAX, decision.block(vec![]));
}
