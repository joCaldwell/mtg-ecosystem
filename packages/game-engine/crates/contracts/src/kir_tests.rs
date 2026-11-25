//! KIR checker cells: admitted and rejected verbs, operands, scopes and tags.
use super::*;

#[test]
fn lane_d_mixed_events_admit_only_two_disjoint_source_events() {
    let source = EventPattern::self_enters();
    let pair = |first, second| EventPattern::AnyOf {
        first: Box::new(first),
        second: Box::new(second),
    };
    let attack = EventPattern::Attacks {
        attacker: Pred::source(),
    };
    let valid = pair(source.clone(), attack.clone());
    check_event(&valid, false).unwrap();
    assert!(event_player_role(&TriggerCondition::on(valid.clone())).is_none());
    for invalid in [
        pair(source.clone(), source.clone()),
        pair(attack, source.clone()),
        pair(
            source.clone(),
            EventPattern::Dies {
                object: Pred::creatures(),
            },
        ),
        pair(source.clone(), valid),
    ] {
        assert!(check_event(&invalid, false).is_err(), "{invalid:?}");
    }
}
use crate::{BeginningStep, PermanentFilter};

fn spell<'a>(targets: &'a [TargetSpec]) -> Env<'a> {
    Env {
        context: Context::Spell,
        targets,
        event: None,
        announced_x: false,
    }
}
fn gain(player: PlayerExpr, amount: Amount) -> Node {
    Node::Do(Verb::GainLife { player, amount })
}
fn player_target() -> Vec<TargetSpec> {
    vec![TargetSpec::new(0, "line:0:1", TargetDomain::AnyPlayer)]
}
fn creature_target() -> Vec<TargetSpec> {
    vec![TargetSpec::new(
        0,
        "line:0:1",
        TargetDomain::BattlefieldCreature,
    )]
}

#[test]
fn controller_role_must_be_the_one_the_context_defines() {
    // CR 109.5: "you" in a spell is its controller, never an ability's.
    let you = |context| gain(PlayerExpr::you(context), Amount::fixed(3));
    assert!(check(&you(Context::Spell), &spell(&[])).is_ok());
    for wrong in [Context::Activated, Context::Triggered] {
        assert!(matches!(
            check(&you(wrong), &spell(&[])),
            Err(KirError::Scope(_))
        ));
    }
    let mut ability = spell(&[]);
    ability.context = Context::Activated;
    assert!(check(&you(Context::Activated), &ability).is_ok());
    assert!(matches!(
        check(&you(Context::Spell), &ability),
        Err(KirError::Scope(_))
    ));
    for role in [
        ControllerRole::StaticController,
        ControllerRole::EnteringController,
        ControllerRole::ProspectiveCaster,
        ControllerRole::SpecialActionPlayer,
        ControllerRole::CardOwner,
        ControllerRole::AbilityHolder,
    ] {
        let node = gain(
            PlayerExpr::Is {
                who: Ref::Controller { role },
            },
            Amount::fixed(1),
        );
        assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
    }
}

#[test]
fn target_references_need_an_announced_slot_of_the_right_kind() {
    let node = gain(PlayerExpr::target(0), Amount::fixed(2));
    // CR 601.2c: no announced occurrence.
    assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
    let requirements = check(&node, &spell(&player_target())).unwrap();
    assert_eq!(requirements.target_slots, BTreeSet::from([0]));
    // A creature target is not a player.
    assert!(matches!(
        check(&node, &spell(&creature_target())),
        Err(KirError::Type(_))
    ));
    assert_eq!(target_slots(&node), BTreeSet::from([0]));
}

#[test]
fn event_players_exist_only_for_events_that_supply_the_role() {
    let caster = gain(PlayerExpr::event(EventPlayerRole::Caster), Amount::fixed(1));
    let cast = TriggerCondition::on(EventPattern::Cast {
        caster: PlayerExpr::EachPlayer,
        spell: Pred::default(),
    });
    let step = TriggerCondition::on(EventPattern::StepBegins {
        step: BeginningStep::Upkeep,
        active: PlayerExpr::EachPlayer,
    });
    let self_dies = TriggerCondition::on(EventPattern::self_dies());
    let triggered = |event| Env {
        context: Context::Triggered,
        targets: &[],
        event,
        announced_x: false,
    };
    let requirements = check(&caster, &triggered(Some(&cast))).unwrap();
    assert!(requirements.event_player);
    assert!(
        requirements
            .capabilities
            .contains(&Capability::EventPlayerRecipientV1)
    );
    // CR 603.2: a beginning-of-step event has an active player, no caster.
    assert!(matches!(
        check(&caster, &triggered(Some(&step))),
        Err(KirError::Scope(_))
    ));
    let active = gain(
        PlayerExpr::event(EventPlayerRole::ActivePlayer),
        Amount::fixed(1),
    );
    assert!(check(&active, &triggered(Some(&step))).is_ok());
    assert!(matches!(
        check(&caster, &triggered(Some(&self_dies))),
        Err(KirError::Scope(_))
    ));
    assert!(matches!(
        check(&caster, &triggered(None)),
        Err(KirError::Scope(_))
    ));
    assert!(matches!(
        check(&caster, &spell(&[])),
        Err(KirError::Scope(_))
    ));
}

#[test]
fn announced_x_is_in_scope_only_when_the_object_has_x() {
    let node = gain(
        PlayerExpr::you(Context::Spell),
        Amount::Var {
            of: Ref::AnnouncedX,
        },
    );
    assert!(matches!(check(&node, &spell(&[])), Err(KirError::Scope(_))));
    let mut with_x = spell(&[]);
    with_x.announced_x = true;
    let requirements = check(&node, &with_x).unwrap();
    assert!(requirements.announced_x);
    assert_eq!(requirements.cells, BTreeSet::from([Cell::LifeControllerX]));
    assert_eq!(cell_tiers(Cell::LifeControllerX), Tiers::EXECUTES);
    // CR 107.3: X is the amount for the controller or one announced
    // player; a group or event player has no audited X cell.
    let targets = player_target();
    let mut targeted = spell(&targets);
    targeted.announced_x = true;
    let x = |player| {
        Node::Do(Verb::LoseLife {
            player,
            amount: Amount::announced_x(),
        })
    };
    assert_eq!(
        check(&x(PlayerExpr::you(Context::Spell)), &with_x)
            .unwrap()
            .cells,
        requirements.cells
    );
    check(&x(PlayerExpr::target(0)), &targeted).unwrap();
    assert!(matches!(
        check(&x(PlayerExpr::EachOpponent), &targeted),
        Err(KirError::Unaudited(_))
    ));
    // A battlefield count does not consume the announced X.
    let counted = gain(
        PlayerExpr::you(Context::Spell),
        Amount::Count {
            set: SetExpr::battlefield(Pred::default()),
        },
    );
    let requirements = check(&counted, &with_x).unwrap();
    assert!(!requirements.announced_x);
    assert_eq!(requirements.cells, BTreeSet::from([Cell::LifeCounted]));
}

#[test]
fn the_drain_pair_uses_the_top_level_executor() {
    let pair = |amount| Node::drain(Context::Spell, amount);
    let requirements = check(&pair(Amount::fixed(2)), &spell(&[])).unwrap();
    assert_eq!(
        requirements.capabilities,
        BTreeSet::from([
            Capability::OpponentLifeDrainV1,
            Capability::LifeLossV1,
            Capability::LifeGainV1
        ])
    );
    assert_eq!(
        node_capabilities(&pair(Amount::fixed(2))),
        requirements.capabilities
    );
    assert_eq!(drain(&pair(Amount::fixed(2))), Some(&Amount::fixed(2)));
    // CR 119.3: a fixed drain is positive; X needs an announced X.
    assert!(matches!(
        check(&pair(Amount::fixed(0)), &spell(&[])),
        Err(KirError::Type(_))
    ));
    assert!(matches!(
        check(&pair(Amount::announced_x()), &spell(&[])),
        Err(KirError::Scope(_))
    ));
    let mut with_x = spell(&[]);
    with_x.announced_x = true;
    assert!(
        check(&pair(Amount::announced_x()), &with_x)
            .unwrap()
            .announced_x
    );
    // "You" must be the role of the resolving context (CR 109.5).
    assert!(matches!(
        check(
            &Node::drain(Context::Triggered, Amount::fixed(1)),
            &spell(&[])
        ),
        Err(KirError::Scope(_))
    ));
    let Node::Seq(steps) = pair(Amount::fixed(2)) else {
        unreachable!()
    };
    for ordinary in [
        Node::Seq(vec![steps[0].clone()]),
        Node::Seq(vec![steps[0].clone(), steps[0].clone()]),
    ] {
        assert_eq!(drain(&ordinary), None);
        let required = check(&ordinary, &spell(&[])).unwrap();
        assert_eq!(required.capabilities, node_capabilities(&ordinary));
        assert!(
            !required
                .capabilities
                .contains(&Capability::OpponentLifeDrainV1)
        );
    }
    for unaudited in [
        Node::Seq(vec![steps[0].clone(), steps[1].clone(), steps[0].clone()]),
        Node::Seq(vec![pair(Amount::fixed(2))]),
    ] {
        assert!(matches!(
            check(&unaudited, &spell(&[])),
            Err(KirError::Unaudited(_))
        ));
    }
}

#[test]
fn bound_variables_and_step_results_follow_lexical_scope() {
    let you = PlayerExpr::you(Context::Spell);
    let each = |body| Node::ForEach {
        var: 7,
        over: Domain::Players {
            players: PlayerExpr::EachOpponent,
        },
        body: Box::new(body),
    };
    let bound = gain(
        PlayerExpr::Is {
            who: Ref::BoundVar { id: 7 },
        },
        Amount::fixed(1),
    );
    // Unbound outside the loop, bound inside it.
    assert!(matches!(
        check(&bound, &spell(&[])),
        Err(KirError::Scope(_))
    ));
    assert!(check(&each(bound), &spell(&[])).is_ok());
    let bound_draw = Node::Do(Verb::Draw {
        player: PlayerExpr::Is {
            who: Ref::BoundVar { id: 7 },
        },
        amount: Amount::fixed(1),
    });
    assert!(matches!(
        check(&each(bound_draw), &spell(&[])),
        Err(KirError::Unaudited(_))
    ));
    let other = gain(
        PlayerExpr::Is {
            who: Ref::BoundVar { id: 8 },
        },
        Amount::fixed(1),
    );
    assert!(matches!(
        check(&each(other), &spell(&[])),
        Err(KirError::Scope(_))
    ));
    // A step may read only steps performed before it (CR 608.2c).
    let result = |step| {
        gain(
            you.clone(),
            Amount::Var {
                of: Ref::StepResult {
                    step,
                    role: StepRole::Amount,
                },
            },
        )
    };
    let first = gain(you.clone(), Amount::fixed(1));
    assert!(matches!(
        check(&Node::Seq(vec![first.clone(), result(0)]), &spell(&[])),
        Err(KirError::Unaudited(_))
    ));
    let step_draw = Node::Do(Verb::Draw {
        player: you.clone(),
        amount: Amount::Var {
            of: Ref::StepResult {
                step: 0,
                role: StepRole::Amount,
            },
        },
    });
    assert!(matches!(
        check(&Node::Seq(vec![first.clone(), step_draw]), &spell(&[])),
        Err(KirError::Unaudited(_))
    ));
    for forward in [
        Node::Seq(vec![result(0), first.clone()]),
        Node::Seq(vec![first.clone(), result(1)]),
        result(0),
    ] {
        assert!(matches!(
            check(&forward, &spell(&[])),
            Err(KirError::Scope(_))
        ));
    }
}

#[test]
fn control_checks_branches_bindings_payments_and_capabilities() {
    let you = PlayerExpr::you(Context::Spell);
    let set = SetExpr::Matching {
        zone: ZoneExpr::Graveyards,
        filter: Pred::default(),
    };
    let size = Amount::Count { set: set.clone() };
    let body = Node::ForEach {
        var: 2,
        over: Domain::Objects {
            set: SetExpr::Pool {
                pool: Ref::BoundVar { id: 1 },
            },
        },
        body: Box::new(gain(
            PlayerExpr::OwnerOf {
                object: Ref::BoundVar { id: 2 },
            },
            Amount::Count {
                set: SetExpr::One {
                    object: Ref::BoundVar { id: 2 },
                },
            },
        )),
    };
    let choose = Node::Choose {
        var: 1,
        chooser: you.clone(),
        from: set.clone(),
        minimum: size.clone(),
        maximum: size,
        body: Box::new(body),
    };
    let pay = |payment| Node::UnlessPays {
        payer: you.clone(),
        payment,
        node: Box::new(Node::Do(Verb::LoseLife {
            player: you.clone(),
            amount: Amount::fixed(1),
        })),
    };
    let control = Node::If {
        condition: Cond::All {
            conditions: vec![
                Cond::Exists { set },
                Cond::Not {
                    condition: Box::new(Cond::CostPaid {
                        identity: "kicker".into(),
                    }),
                },
                Cond::Compare {
                    left: Amount::announced_x(),
                    cmp: Cmp::AtLeast,
                    right: Amount::fixed(1),
                },
            ],
        },
        then: Box::new(Node::Optional {
            chooser: you.clone(),
            node: Box::new(choose),
        }),
        otherwise: Some(Box::new(Node::Seq(vec![
            pay(Payment::Life {
                amount: Amount::fixed(1),
            }),
            pay(Payment::Generic {
                amount: Amount::announced_x(),
            }),
        ]))),
    };
    let env = Env {
        announced_x: true,
        ..spell(&[])
    };
    let requirements = check(&control, &env).unwrap();
    assert!(requirements.announced_x);
    assert_eq!(
        requirements.capabilities,
        BTreeSet::from([
            Capability::KernelControlV1,
            Capability::LifeGainV1,
            Capability::LifeLossV1,
        ])
    );
    assert_eq!(requirements.capabilities, node_capabilities(&control));
}

#[test]
fn control_rejects_unsupported_operands_and_children() {
    let you = PlayerExpr::you(Context::Spell);
    let child = || Box::new(gain(you.clone(), Amount::fixed(1)));
    let choice = |minimum, maximum| Node::Choose {
        var: 0,
        chooser: you.clone(),
        from: SetExpr::battlefield(Pred::creatures()),
        minimum: Amount::fixed(minimum),
        maximum: Amount::fixed(maximum),
        body: child(),
    };
    assert!(check(&choice(0, 0), &spell(&[])).is_ok());
    assert!(matches!(
        check(&choice(2, 1), &spell(&[])),
        Err(KirError::Type(_))
    ));
    assert!(matches!(
        check(
            &Node::UnlessPays {
                payer: you.clone(),
                payment: Payment::Life {
                    amount: Amount::fixed(-1)
                },
                node: child(),
            },
            &spell(&[])
        ),
        Err(KirError::Type(_))
    ));
    for node in [
        Node::Delayed {
            event: DelayedEvent::NextEndStep,
            program: Box::new(crate::SpellProgram::instructions(crate::SpellTiming::Instant)),
        },
        Node::Reflexive {
            payment: crate::ManaCost { generic: 1.into(), colored: vec![] },
            program: Box::new(crate::SpellProgram::instructions(crate::SpellTiming::Instant)),
        },
        Node::ForEach {
            var: 0,
            over: Domain::Objects {
                set: SetExpr::Matching {
                    zone: ZoneExpr::Exile,
                    filter: Pred::default(),
                },
            },
            body: child(),
        },
        Node::Optional {
            chooser: PlayerExpr::EachOpponent,
            node: child(),
        },
        Node::If {
            condition: Cond::Exists {
                set: SetExpr::source(),
            },
            then: child(),
            otherwise: Some(Box::new(Node::Do(Verb::Shuffle {
                player: you.clone(),
            }))),
        },
        Node::Seq(vec![gain(
            you.clone(),
            Amount::LifeOf {
                player: you.clone(),
            },
        )]),
        Node::Seq(vec![gain(
            PlayerExpr::ControllerOf {
                object: Ref::Source,
            },
            Amount::fixed(1),
        )]),
        Node::Seq(vec![Node::Do(Verb::ModifyUntil {
            what: SetExpr::source(),
            layer: Layer::PowerToughnessModify,
            change: Change::PowerToughness {
                power: Amount::fixed(1),
                toughness: Amount::fixed(1),
            },
            until: Until::EndOfTurn,
        })]),
        choice(1, 1),
    ] {
        assert!(
            matches!(check(&node, &spell(&[])), Err(KirError::Unaudited(_))),
            "{node:?}"
        );
    }
    let optional_destroy = Node::Optional {
        chooser: you,
        node: Box::new(Node::Do(Verb::Destroy {
            what: SetExpr::target(0),
            regeneration_forbidden: false,
        })),
    };
    let targets = [TargetSpec::new(
        0,
        "line:0:0",
        TargetDomain::BattlefieldPermanent,
    )];
    let env = spell(&targets);
    assert!(
        !check(&optional_destroy, &env)
            .unwrap()
            .capabilities
            .contains(&Capability::KernelControlV1)
    );
    assert!(matches!(
        check(&Node::Seq(vec![optional_destroy]), &env),
        Err(KirError::Unaudited(_))
    ));
}

#[test]
fn permanent_source_modifications_require_an_executable_context() {
    let pump = Node::Do(Verb::ModifyUntil {
        what: SetExpr::source(),
        layer: Layer::PowerToughnessModify,
        change: Change::PowerToughness {
            power: Amount::fixed(1),
            toughness: Amount::fixed(1),
        },
        until: Until::EndOfTurn,
    });
    let grant = |keyword| {
        Node::Do(Verb::ModifyUntil {
            what: SetExpr::source(),
            layer: Layer::Ability,
            change: Change::AddKeywords {
                keywords: vec![keyword],
            },
            until: Until::EndOfTurn,
        })
    };
    let activated = Env {
        context: Context::Activated,
        ..spell(&[])
    };
    let triggered = Env {
        context: Context::Triggered,
        ..spell(&[])
    };
    assert!(check(&pump, &spell(&[])).is_err());
    assert!(check(&pump, &activated).is_ok());
    assert!(check(&grant(KeywordAbility::Flying), &activated).is_ok());
    assert!(check(&grant(KeywordAbility::Flying), &triggered).is_err());
    assert!(check(&grant(KeywordAbility::Flying), &spell(&[])).is_err());
    for keyword in [
        KeywordAbility::Menace,
        KeywordAbility::Shroud,
        KeywordAbility::Defender,
        KeywordAbility::ProtectionFromEachColor,
    ] {
        assert!(check(&grant(keyword), &activated).is_err());
    }
}

#[test]
fn control_rejects_counted_draw_leaves() {
    let targets = [TargetSpec::new(0, "line:0:0", TargetDomain::AnyPlayer)];
    let env = Env {
        announced_x: true,
        ..spell(&targets)
    };
    let you = PlayerExpr::you(Context::Spell);
    let target = PlayerExpr::Is {
        who: Ref::Target { slot: 0 },
    };
    for (player, amount) in [
        (you.clone(), Amount::fixed(0)),
        (you.clone(), Amount::fixed(2)),
        (you.clone(), Amount::announced_x()),
        (PlayerExpr::EachPlayer, Amount::fixed(1)),
    ] {
        let leaf = Node::Do(Verb::Draw { player, amount });
        assert!(check(&leaf, &env).is_ok());
        assert!(matches!(
            check(&Node::Seq(vec![leaf]), &env),
            Err(KirError::Unaudited(reason)) if reason.contains("nested counted draw")
        ));
    }
    for amount in [Amount::fixed(0), Amount::fixed(2), Amount::announced_x()] {
        let leaf = Node::Do(Verb::Draw {
            player: target.clone(),
            amount,
        });
        assert!(matches!(check(&leaf, &env), Err(KirError::Unaudited(_))));
    }
    for player in [you, target] {
        let leaf = Node::Do(Verb::Draw {
            player,
            amount: Amount::fixed(1),
        });
        assert!(check(&Node::Seq(vec![leaf]), &env).is_ok());
    }
}

#[test]
fn graveyard_control_sets_reject_battlefield_predicates() {
    let you = PlayerExpr::you(Context::Spell);
    let child = || Box::new(gain(you.clone(), Amount::fixed(1)));
    for atom in [
        Atom::Tapped,
        Atom::Untapped,
        Atom::Attacking,
        Atom::Blocking,
        Atom::ControlledBy {
            player: you.clone(),
        },
    ] {
        let set = SetExpr::Matching {
            zone: ZoneExpr::Graveyards,
            filter: Pred(vec![Atom::AnyOf {
                alternatives: vec![Pred::creatures(), Pred(vec![atom.clone()])],
            }]),
        };
        let amount = Amount::Count { set: set.clone() };
        for node in [
            Node::If {
                condition: Cond::Exists { set: set.clone() },
                then: child(),
                otherwise: None,
            },
            Node::ForEach {
                var: 0,
                over: Domain::Objects { set: set.clone() },
                body: child(),
            },
            Node::Seq(vec![gain(you.clone(), amount.clone())]),
            Node::Choose {
                var: 0,
                chooser: you.clone(),
                from: set,
                minimum: amount.clone(),
                maximum: amount,
                body: child(),
            },
        ] {
            assert!(
                matches!(check(&node, &spell(&[])), Err(KirError::Unaudited(reason))
                    if reason == "battlefield predicate outside the battlefield"),
                "{atom:?} in {node:?}"
            );
        }
    }
}

#[test]
fn a_filtered_count_of_a_hidden_zone_is_rejected() {
    let you = PlayerExpr::you(Context::Spell);
    let count = |zone, filter| {
        gain(
            you.clone(),
            Amount::Count {
                set: SetExpr::Matching { zone, filter },
            },
        )
    };
    let creatures = || {
        Pred(vec![Atom::Type {
            kind: CardKind::Creature,
        }])
    };
    // CR 400.2: which cards a hand or library holds is hidden.
    for hidden in [
        ZoneExpr::Hand { of: you.clone() },
        ZoneExpr::Library { of: you.clone() },
    ] {
        assert_eq!(
            check(&count(hidden.clone(), creatures()), &spell(&[])),
            Err(KirError::HiddenCount)
        );
    }
    // CR 402.3, 401.2: zone sizes are public. The life cell reads hand size.
    let hand_size = count(ZoneExpr::Hand { of: you.clone() }, Pred::default());
    assert_eq!(
        check(&hand_size, &spell(&[])).unwrap().cells,
        BTreeSet::from([Cell::LifeCounted])
    );
    let library_size = count(ZoneExpr::Library { of: you.clone() }, Pred::default());
    assert!(matches!(
        check(&library_size, &spell(&[])),
        Err(KirError::Unaudited(_))
    ));
    for public in [
        ZoneExpr::Battlefield,
        ZoneExpr::Graveyard { of: you.clone() },
        ZoneExpr::Graveyards,
    ] {
        assert_eq!(
            check(&count(public, creatures()), &spell(&[]))
                .unwrap()
                .cells,
            BTreeSet::from([Cell::LifeCounted])
        );
    }
    assert!(matches!(
        check(&count(ZoneExpr::Exile, creatures()), &spell(&[])),
        Err(KirError::Unaudited(_))
    ));
}

#[test]
fn a_continuous_change_carries_its_own_layer() {
    let pump = |layer| {
        Node::Do(Verb::ModifyUntil {
            what: SetExpr::target(0),
            layer,
            change: Change::PowerToughness {
                power: Amount::fixed(2),
                toughness: Amount::fixed(2),
            },
            until: Until::EndOfTurn,
        })
    };
    // CR 613.4c: a modification is not a layer-6 ability change.
    assert!(matches!(
        check(&pump(Layer::Ability), &spell(&creature_target())),
        Err(KirError::Type(_))
    ));
    assert!(!matches!(
        check(
            &pump(Layer::PowerToughnessModify),
            &spell(&creature_target())
        ),
        Err(KirError::Type(_) | KirError::Scope(_))
    ));
    for (change, layer) in [
        (
            Change::AddKeywords {
                keywords: vec![KeywordAbility::Flying],
            },
            Layer::Ability,
        ),
        (Change::SetColors { colors: vec![] }, Layer::Color),
        (
            Change::AddTypes {
                kinds: vec![CardKind::Artifact],
            },
            Layer::Type,
        ),
        (
            Change::Controller {
                player: PlayerExpr::EachOpponent,
            },
            Layer::Control,
        ),
    ] {
        assert_eq!(change.layer(), layer);
    }
}

#[test]
fn life_cells_admit_exactly_the_audited_recipients_and_amounts() {
    let targets = player_target();
    let env = spell(&targets);
    for player in [
        PlayerExpr::you(Context::Spell),
        PlayerExpr::EachOpponent,
        PlayerExpr::target(0),
    ] {
        for node in [
            gain(player.clone(), Amount::fixed(0)),
            Node::Do(Verb::LoseLife {
                player: player.clone(),
                amount: Amount::fixed(4),
            }),
        ] {
            check(&node, &env).unwrap();
        }
        assert!(matches!(
            check(&gain(player, Amount::fixed(-1)), &env),
            Err(KirError::Type(_))
        ));
    }
    // CR 119.3: an opponent-only target domain has no audited life cell.
    let opponent = [TargetSpec::new(0, "line:0:1", TargetDomain::OpponentPlayer)];
    assert!(matches!(
        check(
            &gain(PlayerExpr::target(0), Amount::fixed(1)),
            &spell(&opponent)
        ),
        Err(KirError::Unaudited(_))
    ));
    assert!(matches!(
        check(&gain(PlayerExpr::EachPlayer, Amount::fixed(1)), &env),
        Err(KirError::Unaudited(_))
    ));
}

#[test]
fn board_wide_cells_audit_the_zone_destination_and_every_filter_atom() {
    let creatures = Pred(vec![Atom::Type {
        kind: CardKind::Creature,
    }]);
    let destroy = |filter: Pred, regeneration_forbidden| {
        Node::Do(Verb::Destroy {
            what: SetExpr::battlefield(filter),
            regeneration_forbidden,
        })
    };
    let moved = |what, to| Node::Do(Verb::Move { what, to });
    let env = spell(&[]);
    // CR 701.8: destroying every creature names the creature
    // destruction surface; every other board-wide set, the wipe surface.
    let requirements = check(&destroy(creatures.clone(), false), &env).unwrap();
    assert_eq!(
        requirements.capabilities,
        BTreeSet::from([Capability::DestroyCreatureV1])
    );
    let artifacts = Pred(vec![Atom::Type {
        kind: CardKind::Artifact,
    }]);
    assert_eq!(
        check(&destroy(artifacts, false), &env)
            .unwrap()
            .capabilities,
        BTreeSet::from([Capability::BoardWipesV1])
    );
    for to in [Destination::Exile, Destination::OwnerHand] {
        check(&moved(SetExpr::battlefield(creatures.clone()), to), &env).unwrap();
    }
    check(&moved(SetExpr::graveyards(), Destination::Exile), &env).unwrap();
    // CR 701.19c: the prohibition is audited for the nonland wave only.
    check(&destroy(Pred::nonland_permanents(), true), &env).unwrap();
    for node in [
        destroy(creatures.clone(), true),
        moved(
            SetExpr::battlefield(creatures.clone()),
            Destination::OwnerLibraryTop,
        ),
        moved(SetExpr::graveyards(), Destination::OwnerHand),
        moved(
            SetExpr::Matching {
                zone: ZoneExpr::Graveyards,
                filter: creatures.clone(),
            },
            Destination::Exile,
        ),
        moved(
            SetExpr::Matching {
                zone: ZoneExpr::Exile,
                filter: Pred::default(),
            },
            Destination::OwnerHand,
        ),
        // Atoms the board evaluator does not read.
        destroy(Pred(vec![Atom::Blocking]), false),
        destroy(
            Pred(vec![Atom::IsNot {
                object: Ref::Source,
            }]),
            false,
        ),
        destroy(
            Pred(vec![Atom::ControlledBy {
                player: PlayerExpr::EachPlayer,
            }]),
            false,
        ),
        destroy(
            Pred(vec![Atom::AnyOf {
                alternatives: vec![Pred(vec![Atom::Blocking])],
            }]),
            false,
        ),
    ] {
        assert!(
            matches!(check(&node, &env), Err(KirError::Unaudited(_))),
            "{node:?}"
        );
    }
    // A computed comparison is in scope only with X, and unaudited then.
    let by_x = destroy(
        Pred(vec![Atom::ManaValue {
            cmp: Cmp::AtMost,
            value: Amount::Var {
                of: Ref::AnnouncedX,
            },
        }]),
        false,
    );
    assert!(matches!(check(&by_x, &env), Err(KirError::Scope(_))));
    let mut with_x = spell(&[]);
    with_x.announced_x = true;
    assert!(matches!(check(&by_x, &with_x), Err(KirError::Unaudited(_))));
    // "You control" in a filter obeys the same controller-role rule.
    let yours = |context| {
        destroy(
            Pred(vec![Atom::ControlledBy {
                player: PlayerExpr::you(context),
            }]),
            false,
        )
    };
    check(&yours(Context::Spell), &env).unwrap();
    assert!(matches!(
        check(&yours(Context::Triggered), &env),
        Err(KirError::Scope(_))
    ));
    // Creature pump sets admit creature predicates regardless of conjunct order.
    let reordered_pump = Node::Do(Verb::ModifyUntil {
        what: SetExpr::battlefield(Pred(vec![
            Atom::Color { color: Color::B },
            Atom::Type {
                kind: CardKind::Creature,
            },
        ])),
        layer: Layer::PowerToughnessModify,
        change: Change::PowerToughness {
            power: Amount::fixed(1),
            toughness: Amount::fixed(1),
        },
        until: Until::EndOfTurn,
    });
    check(&reordered_pump, &env).unwrap();
    assert!(
        Pred(vec![Atom::ControlledBy {
            player: PlayerExpr::you(Context::Spell)
        }])
        .names_you()
    );
    assert!(!creatures.names_you());
}

#[test]
fn capabilities_include_the_continuation_and_its_leaves() {
    let caps = |node: &Node| node_capabilities(node);
    assert_eq!(
        caps(&gain(PlayerExpr::you(Context::Spell), Amount::fixed(1))),
        BTreeSet::from([Capability::LifeGainV1])
    );
    assert_eq!(
        caps(&Node::Do(Verb::LoseLife {
            player: PlayerExpr::EachOpponent,
            amount: Amount::fixed(1)
        })),
        BTreeSet::from([Capability::LifeLossV1, Capability::SacrificeV1])
    );
    // The walker reaches verbs under every control node.
    let nested = Node::Optional {
        chooser: PlayerExpr::you(Context::Spell),
        node: Box::new(Node::Seq(vec![
            gain(PlayerExpr::target(0), Amount::fixed(1)),
            Node::If {
                condition: Cond::CostPaid {
                    identity: "kicker".into(),
                },
                then: Box::new(Node::Do(Verb::LoseLife {
                    player: PlayerExpr::target(0),
                    amount: Amount::fixed(1),
                })),
                otherwise: None,
            },
        ])),
    };
    assert_eq!(
        caps(&nested),
        BTreeSet::from([
            Capability::KernelControlV1,
            Capability::LifeGainV1,
            Capability::LifeLossV1,
        ])
    );
}

#[test]
fn unit_operands_reject_unknown_fields() {
    // A unit operand's only field is its tag.
    assert!(serde_json::from_value::<Destination>(serde_json::json!({"to":"exile"})).is_ok());
    assert!(
        serde_json::from_value::<Destination>(serde_json::json!({"to":"exile","x":1})).is_err()
    );
    assert!(serde_json::from_value::<Ref>(serde_json::json!({"ref":"source","x":1})).is_err());
    assert!(
        serde_json::from_value::<PlayerExpr>(serde_json::json!({"player":"each_opponent","x":1}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<ZoneExpr>(serde_json::json!({"zone":"battlefield","x":1}))
            .is_err()
    );
    assert!(serde_json::from_value::<Atom>(serde_json::json!({"atom":"tapped","x":1})).is_err());
    assert!(
        serde_json::from_value::<CounterKind>(
            serde_json::json!({"counter":"plus_one_plus_one","x":1})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<DelayedEvent>(serde_json::json!({"at":"next_end_step","x":1}))
            .is_err()
    );
    assert_eq!(
        serde_json::from_value::<Ref>(serde_json::json!({"ref":"announced_x"})).unwrap(),
        Ref::AnnouncedX
    );
}

#[test]
fn nodes_round_trip_and_reject_unknown_fields() {
    let node = Node::Seq(vec![
        gain(PlayerExpr::target(0), Amount::fixed(3)),
        Node::Do(Verb::Destroy {
            what: SetExpr::battlefield(Pred(vec![
                Atom::Type {
                    kind: CardKind::Creature,
                },
                Atom::Power {
                    cmp: Cmp::AtLeast,
                    value: Amount::fixed(4),
                },
                Atom::ControlledBy {
                    player: PlayerExpr::EachOpponent,
                },
            ])),
            regeneration_forbidden: false,
        }),
    ]);
    let json = serde_json::to_value(&node).unwrap();
    assert_eq!(
        json["seq"][0],
        serde_json::json!({"do":{"verb":"gain_life",
            "player":{"player":"is","who":{"ref":"target","slot":0}},
            "amount":{"amount":"fixed","value":"3"}}})
    );
    assert_eq!(serde_json::from_value::<Node>(json.clone()).unwrap(), node);
    let mut unknown = json;
    unknown["seq"][0]["do"]["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Node>(unknown).is_err());
    assert!(serde_json::from_value::<Node>(serde_json::json!({"do":{"verb":"levitate"}})).is_err());
}

#[test]
fn every_audited_verb_names_the_cell_it_occupies() {
    let cells = |node: &Node, env: &Env<'_>| check(node, env).unwrap().cells;
    let you = PlayerExpr::you(Context::Spell);
    assert_eq!(
        cells(&gain(you.clone(), Amount::fixed(2)), &spell(&[])),
        BTreeSet::from([Cell::LifeController])
    );
    assert_eq!(
        cells(
            &gain(PlayerExpr::EachOpponent, Amount::fixed(2)),
            &spell(&[])
        ),
        BTreeSet::from([Cell::LifeEachOpponent])
    );
    let targets = creature_target();
    let damage = |amount| {
        Node::Do(Verb::Damage {
            source: Ref::Source,
            to: DamageTo::Ref {
                who: Ref::Target { slot: 0 },
            },
            amount,
        })
    };
    assert_eq!(
        cells(&damage(Amount::fixed(3)), &spell(&targets)),
        BTreeSet::from([Cell::DamageTarget])
    );
    let x = Env {
        announced_x: true,
        ..spell(&targets)
    };
    assert_eq!(
        cells(&damage(Amount::announced_x()), &x),
        BTreeSet::from([Cell::DamageTargetX])
    );
    // A control node occupies the continuation cell and its leaves' cells.
    let sequence = Node::Seq(vec![
        gain(you.clone(), Amount::fixed(1)),
        Node::Do(Verb::Draw {
            player: you,
            amount: Amount::fixed(1),
        }),
    ]);
    assert_eq!(
        cells(&sequence, &spell(&[])),
        BTreeSet::from([Cell::Control, Cell::DrawController])
    );
}

#[test]
fn tiers_are_independent_reviews_above_executes() {
    use ExecutionTier as T;
    for tiers in [
        Tiers::EXECUTES,
        Tiers::HOSTING,
        Tiers::SEARCH_HOSTING,
        Tiers::ALL,
    ] {
        assert!(tiers.admits(T::Executes));
        assert_eq!(tiers.meet(Tiers::ALL), tiers);
        assert_eq!(tiers.meet(Tiers::EXECUTES), Tiers::EXECUTES);
    }
    assert!(Tiers::HOSTING.admits(T::Hosting) && !Tiers::HOSTING.admits(T::Search));
    assert!(!Tiers::SEARCH_HOSTING.admits(T::HeadlessQualified));
    assert_eq!(Tiers::HOSTING.meet(Tiers::SEARCH_HOSTING), Tiers::HOSTING);
}

#[test]
fn a_definition_earns_the_meet_over_its_surfaces_and_cells() {
    use ExecutionTier as T;
    let mut bears = crate::Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json"))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    assert_eq!(definition_tiers(&bears).unwrap(), Tiers::ALL);
    assert_eq!(unqualified(&bears, T::Hosting).unwrap(), (vec![], vec![]));
    // A keyword reviewed for hosting only lowers the definition to hosting.
    bears.keywords.push(crate::KeywordAbility::Flying);
    assert_eq!(definition_tiers(&bears).unwrap(), Tiers::HOSTING);
    assert_eq!(
        unqualified(&bears, T::Search).unwrap().0,
        vec![Capability::FlyingV1]
    );
    // A kernel cell that executes but has passed no review keeps the
    // definition at executes, whatever its surfaces earned.
    bears.keywords.clear();
    bears.triggered.push(crate::TriggeredAbility {
        functioning_zone: crate::FunctioningZone::Battlefield,
        condition: TriggerCondition::on(EventPattern::self_enters()),
        program: crate::SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: crate::SpellTiming::Instant,
            resolution: crate::SpellResolution::Instructions,
            functioning_zone: crate::FunctioningZone::Stack,
            optional_target: false,
            targets: vec![],
            instructions: vec![crate::Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::EachOpponent,
                    amount: Amount::fixed(1),
                },
                0,
            )],
        },
    });
    assert_eq!(
        definition_cells(&bears).unwrap(),
        BTreeSet::from([Cell::LifeEachOpponent])
    );
    assert!(
        !definition_tiers(&bears)
            .unwrap()
            .admits(T::HeadlessQualified)
    );
    assert!(definition_tiers(&bears).unwrap().admits(T::Executes));
    assert!(
        unqualified(&bears, T::HeadlessQualified)
            .unwrap()
            .1
            .contains(&Cell::LifeEachOpponent)
    );
}

#[test]
fn tier_helpers_never_qualify_a_node_that_does_not_check() {
    use ExecutionTier as T;
    // CR 109.5: a spell's "you" is its controller, never an ability's; this
    // unvalidated spell's node does not check, so it earns no tier at all.
    let mut spell = crate::Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json"))
        .unwrap()
        .definitions
        .into_iter()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    spell.spell.as_mut().unwrap().instructions = vec![crate::Instruction::act(
        Verb::GainLife {
            player: PlayerExpr::you(Context::Activated),
            amount: Amount::fixed(2),
        },
        0,
    )];
    assert!(matches!(definition_cells(&spell), Err(KirError::Scope(_))));
    assert!(definition_tiers(&spell).is_err());
    for tier in [T::Executes, T::HeadlessQualified, T::Search, T::Hosting] {
        assert!(unqualified(&spell, tier).is_err());
    }
}

/// One filtered "target creature" whose controller the frame captures.
fn filtered_creature_target() -> Vec<TargetSpec> {
    let mut target = TargetSpec::new(0, "line:0:1", TargetDomain::BattlefieldPermanent);
    target.filter = Some(PermanentFilter {
        types: vec![CardKind::Creature],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        tapped: false,
        power: None,
    });
    vec![target]
}

fn search(player: PlayerExpr, filter: Pred, maximum: i64, to: Destination) -> Node {
    Node::Do(Verb::Search {
        player,
        search: SearchSpec {
            filter,
            maximum: Amount::fixed(maximum),
            to,
            reveal: false,
        },
    })
}

#[test]
fn library_cells_admit_own_library_searches_shuffles_and_looks() {
    let cells = |node: &Node, env: &Env<'_>| check(node, env).map(|d| d.cells);
    let you = PlayerExpr::you(Context::Spell);
    let basic_land = Pred(vec![
        Atom::Type {
            kind: CardKind::Land,
        },
        Atom::Basic,
    ]);
    let battlefield = |controller: &PlayerExpr| Destination::Battlefield {
        controller: controller.clone(),
        tapped: true,
    };
    // CR 701.23a: your own library, into a public zone or your hand.
    for to in [
        battlefield(&you),
        Destination::OwnerHand,
        Destination::OwnerGraveyard,
    ] {
        assert_eq!(
            cells(&search(you.clone(), basic_land.clone(), 1, to), &spell(&[])),
            Ok(BTreeSet::from([Cell::SearchController]))
        );
    }
    // CR 701.24b: a singleton library-top search includes the shuffle.
    assert_eq!(
        cells(
            &search(
                you.clone(),
                basic_land.clone(),
                1,
                Destination::OwnerLibraryTop
            ),
            &spell(&[])
        ),
        Ok(BTreeSet::from([Cell::SearchLibraryTop]))
    );
    // CR 608.2h: a filtered target's last-known controller searches theirs.
    let targets = filtered_creature_target();
    let theirs = PlayerExpr::ControllerOf {
        object: Ref::Target { slot: 0 },
    };
    assert_eq!(
        cells(
            &search(theirs.clone(), basic_land.clone(), 1, battlefield(&theirs)),
            &spell(&targets)
        ),
        Ok(BTreeSet::from([Cell::SearchTargetController]))
    );
    assert_eq!(
        cells(
            &Node::Do(Verb::Shuffle {
                player: theirs.clone()
            }),
            &spell(&targets)
        ),
        Ok(BTreeSet::from([Cell::ShuffleTargetController]))
    );
    // "Its controller may search": the chooser is that same player.
    let optional = Node::Optional {
        chooser: theirs.clone(),
        node: Box::new(Node::Seq(vec![
            search(theirs.clone(), basic_land.clone(), 1, battlefield(&theirs)),
            Node::Do(Verb::Shuffle {
                player: theirs.clone(),
            }),
        ])),
    };
    assert_eq!(
        cells(&optional, &spell(&targets)),
        Ok(BTreeSet::from([
            Cell::Control,
            Cell::SearchTargetController,
            Cell::ShuffleTargetController
        ]))
    );
    for (amount, surveil, cell) in [
        (1, false, Cell::ScryController),
        (2, true, Cell::SurveilController),
    ] {
        let node = Node::Do(if surveil {
            Verb::Surveil {
                player: you.clone(),
                amount: Amount::fixed(amount),
            }
        } else {
            Verb::Scry {
                player: you.clone(),
                amount: Amount::fixed(amount),
            }
        });
        assert_eq!(cells(&node, &spell(&[])), Ok(BTreeSet::from([cell])));
    }
}

#[test]
fn library_cells_reject_other_libraries_destinations_and_filters() {
    let you = PlayerExpr::you(Context::Spell);
    let unaudited = |node: &Node, targets: &[TargetSpec]| {
        matches!(check(node, &spell(targets)), Err(KirError::Unaudited(_)))
    };
    // An unfiltered target captures no controller facts.
    let unfiltered = creature_target();
    let theirs = PlayerExpr::ControllerOf {
        object: Ref::Target { slot: 0 },
    };
    assert!(unaudited(
        &Node::Do(Verb::Shuffle {
            player: theirs.clone()
        }),
        &unfiltered
    ));
    assert!(unaudited(
        &Node::Optional {
            chooser: theirs,
            node: Box::new(Node::Do(Verb::Draw {
                player: you.clone(),
                amount: Amount::fixed(1),
            })),
        },
        &unfiltered
    ));
    // CR 800.4g: a filtered target's controller may only choose to search
    // or shuffle their own library. Skipping any other choice after they
    // leave would deprive a live player of its effect.
    let filtered = filtered_creature_target();
    let theirs = PlayerExpr::ControllerOf {
        object: Ref::Target { slot: 0 },
    };
    for node in [
        Node::Do(Verb::Draw {
            player: you.clone(),
            amount: Amount::fixed(1),
        }),
        Node::Seq(vec![
            search(you.clone(), Pred(vec![]), 1, Destination::OwnerHand),
            Node::Do(Verb::Shuffle {
                player: theirs.clone(),
            }),
        ]),
    ] {
        assert!(unaudited(
            &Node::Optional {
                chooser: theirs.clone(),
                node: Box::new(node),
            },
            &filtered
        ));
    }
    // That player chooses nothing outside an optional library action.
    assert!(unaudited(
        &Node::ForEach {
            var: 0,
            over: Domain::Players {
                players: theirs.clone(),
            },
            body: Box::new(Node::Do(Verb::Draw {
                player: you.clone(),
                amount: Amount::fixed(1),
            })),
        },
        &filtered
    ));
    // Each player and an announced player have no library cell.
    for player in [PlayerExpr::EachPlayer, PlayerExpr::target(0)] {
        assert!(unaudited(
            &Node::Do(Verb::Shuffle { player }),
            &player_target()
        ));
    }
    assert!(unaudited(
        &Node::Do(Verb::Scry {
            player: PlayerExpr::target(0),
            amount: Amount::fixed(1),
        }),
        &player_target()
    ));
    // Status, control and keyword atoms are not library-card qualities.
    for atom in [
        Atom::Tapped,
        Atom::Token,
        Atom::ControlledBy {
            player: you.clone(),
        },
        Atom::Keyword {
            keyword: KeywordAbility::Flying,
        },
    ] {
        assert!(unaudited(
            &search(you.clone(), Pred(vec![atom]), 1, Destination::OwnerHand),
            &[]
        ));
    }
    // Only the controller's singleton search may put its find on top.
    assert!(unaudited(
        &search(you.clone(), Pred(vec![]), 2, Destination::OwnerLibraryTop),
        &[]
    ));
    assert!(unaudited(
        &search(
            theirs.clone(),
            Pred(vec![]),
            1,
            Destination::OwnerLibraryTop
        ),
        &filtered
    ));
    for to in [
        Destination::Exile,
        Destination::Battlefield {
            controller: PlayerExpr::EachOpponent,
            tapped: false,
        },
    ] {
        assert!(unaudited(&search(you.clone(), Pred(vec![]), 1, to), &[]));
    }
    // CR 701.22b: no zero or negative look; a search finds at least one.
    for node in [
        search(you.clone(), Pred(vec![]), 0, Destination::OwnerHand),
        Node::Do(Verb::Scry {
            player: you.clone(),
            amount: Amount::fixed(0),
        }),
        Node::Do(Verb::Surveil {
            player: you,
            amount: Amount::fixed(-1),
        }),
    ] {
        assert!(matches!(check(&node, &spell(&[])), Err(KirError::Type(_))));
    }
}

/// Every admitted event pattern is one the collector and snapshot recovery
/// evaluate identically; the rest reject (CR 110.5d, 508.1f, 603.2, 701.26a).
#[test]
fn event_patterns_admit_only_what_collection_and_recovery_agree_on() {
    let creature = Atom::Type {
        kind: CardKind::Creature,
    };
    let unaudited = |pattern: EventPattern, attachment: bool| {
        assert!(
            matches!(
                check_event(&pattern, attachment),
                Err(KirError::Unaudited(_))
            ),
            "{pattern:?}"
        );
    };
    // Only the source's own tap is observed.
    check_event(&EventPattern::self_becomes_tapped(), false).unwrap();
    unaudited(
        EventPattern::BecomesTapped {
            object: Pred(vec![creature.clone()]),
        },
        false,
    );
    // A spell has no tapped status; an attacker's is read before its tap.
    for status in [Atom::Tapped, Atom::Untapped] {
        unaudited(
            EventPattern::Cast {
                caster: PlayerExpr::EachPlayer,
                spell: Pred(vec![status.clone()]),
            },
            false,
        );
        unaudited(
            EventPattern::Attacks {
                attacker: Pred(vec![Atom::AnyOf {
                    alternatives: vec![Pred(vec![status.clone()])],
                }]),
            },
            false,
        );
    }
    // Recovery cannot prove an object was not the host.
    unaudited(
        EventPattern::Dies {
            object: Pred(vec![
                creature.clone(),
                Atom::IsNot {
                    object: Ref::AttachedTo,
                },
            ]),
        },
        true,
    );
    check_event(
        &EventPattern::Dies {
            object: Pred::attached_creature(),
        },
        true,
    )
    .unwrap();
}

/// CR 603.4: a battlefield-count clause never names the source itself, at
/// any depth of alternatives.
#[test]
fn intervening_counts_reject_nested_identity() {
    let you = Atom::ControlledBy {
        player: PlayerExpr::you(Context::Triggered),
    };
    let count = |inner: Atom| Cond::Exists {
        set: SetExpr::battlefield(Pred(vec![inner, you.clone()])),
    };
    let nested = Atom::AnyOf {
        alternatives: vec![Pred(vec![Atom::AnyOf {
            alternatives: vec![Pred(vec![Atom::Is {
                object: Ref::Source,
            }])],
        }])],
    };
    assert!(matches!(
        Intervening::of(&count(nested)),
        Err(KirError::Unaudited(_))
    ));
    let other = Atom::IsNot {
        object: Ref::Source,
    };
    assert!(matches!(
        Intervening::of(&count(other)),
        Ok(Intervening::BattlefieldCount { .. })
    ));
}
