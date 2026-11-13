//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-removal-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    prepared_in(catalog(), rows)
}
fn prepared_in(c: Catalog, rows: &[(&str, u8, bool)]) -> Prepared {
    let mut cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, p, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*p),
            controller: PlayerId(*p),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*p),
                }
            } else {
                Zone::Battlefield
            },
        })
        .collect::<Vec<_>>();
    for p in 0..4 {
        for _ in 0..4 {
            cards.push(CardSetup {
                id: CardId(cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(p),
                controller: PlayerId(p),
                zone: Zone::Library { owner: PlayerId(p) },
            });
        }
    }
    let config = ScenarioConfig {
        initial: ScenarioInitial {
            turn_step: TurnStep::Untap,
            ..ScenarioInitial::default()
        },
        mode: Mode::CommanderMatch,
        players: 4,
        profile: "commander-removal-v1".into(),
        commanders: rows
            .iter()
            .enumerate()
            .filter(|(_, (name, _, _))| {
                matches!(
                    *name,
                    "Isamaru, Hound of Konda"
                        | "Yargle and Multani"
                        | "Caelorna, Coral Tyrant"
                        | "Rorix Bladewing"
                )
            })
            .map(|(i, _)| CardId(i as u32))
            .collect(),
        cards,
    };
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
fn request(g: &mut Game) -> Request {
    match g.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(r) => r,
        x => panic!("{x:?}"),
    }
}
fn answer(g: &mut Game, response: Response) {
    let r = request(g);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
fn reach(g: &mut Game, step: TurnStep) {
    for _ in 0..200 {
        let r = request(g);
        if g.turn().step == step && matches!(r, Request::Priority { .. }) {
            return;
        }
        let response = match r {
            Request::Priority { .. } => Response::Pass,
            Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: false },
            Request::DamageEffect { ref candidates, .. } => Response::DamageEffect {
                effect: candidates[0].effect.clone(),
                damage: candidates[0].damage,
            },
            x => panic!("unexpected {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("step not reached")
}
fn attackers(g: &mut Game) -> Vec<ObjectId> {
    for _ in 0..100 {
        let r = request(g);
        if let Request::Attackers { candidates, .. } = r {
            return candidates;
        }
        g.respond(r.player(), r.decision_id(), Response::Pass)
            .unwrap();
    }
    panic!("attackers not reached")
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    assert_eq!(restored.trace(), g.trace());
    let mut replayed = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    if matches!(g.state.rules.frame, Frame::Fault { .. }) {
        let _ = replayed.advance(Budget::new(1));
    }
    assert_eq!(replayed.state, g.state);
    assert_eq!(replayed.trace(), g.trace());
}
fn traces(p: Prepared, run: impl Fn(&mut Game)) {
    let mut digest0 = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(p.clone(), mode);
        run(&mut g);
        check(&g);
        let d = g.state_digest().unwrap();
        if let Some(old) = &digest0 {
            assert_eq!(&d, old)
        } else {
            digest0 = Some(d)
        }
    }
}
fn announce(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    if g.turn().step == TurnStep::Untap {
        reach(g, TurnStep::PrecombatMain);
    }
    let obj = object(g, id);
    answer(g, Response::Cast { card: obj.card });
    pay_proposal(g, targets, None);
}
fn pay_proposal(g: &mut Game, targets: Vec<TargetEntity>, stack_target: Option<StackId>) {
    loop {
        let r = request(g);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .filter(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .min_by_key(|o| {
                            usize::from(
                                !g.mana_abilities(o.id)
                                    .unwrap()
                                    .iter()
                                    .any(|c| Some(c) == cost.colored.first()),
                            )
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            Request::StackTargets { .. } => Response::StackTargets {
                entries: vec![stack_target.unwrap_or(g.state.rules.stack[0].id)],
            },
            Request::PlayerTargets { .. } => Response::PlayerTargets {
                players: vec![PlayerId(1)],
            },
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                order: candidates.clone(),
            },
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}

fn automatic(r: &Request, targets: &[TargetEntity]) -> Response {
    match r {
        Request::Priority { .. } => Response::Pass,
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::Targets { .. } => Response::Targets {
            objects: targets
                .iter()
                .map(|t| match t {
                    TargetEntity::Object { object } => *object,
                    _ => panic!("creature target"),
                })
                .collect(),
        },
        Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: false },
        Request::OptionalEffect { .. } => Response::OptionalEffect { perform: true },
        Request::OptionalRecursion { .. } => Response::OptionalRecursion { perform: true },
        Request::ResolveDiscard { candidates, .. } => Response::ResolveDiscard {
            objects: vec![candidates[0]],
        },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        Request::PlayerTargets { .. } => Response::PlayerTargets {
            players: vec![PlayerId(1)],
        },
        x => panic!("unexpected resolution request {x:?}"),
    }
}
fn resolve_with(g: &mut Game, targets: Vec<TargetEntity>, mut inspect: impl FnMut(&Game)) {
    for _ in 0..2000 {
        let advance = g.advance(Budget::new(1));
        inspect(g);
        match advance {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => {
                if matches!(r, Request::Priority { .. })
                    && g.state.rules.stack.is_empty()
                    && g.state.rules.pending_triggers.is_empty()
                {
                    return;
                }
                let response = automatic(&r, &targets);
                g.respond(r.player(), r.decision_id(), response).unwrap();
            }
            x => panic!("resolve {x:?}"),
        }
    }
    panic!("resolution budget")
}
fn resolve(g: &mut Game, targets: Vec<TargetEntity>) {
    resolve_with(g, targets, |_| {});
}
fn cast(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    announce(g, id, targets.clone());
    resolve(g, targets);
}
fn target(o: ObjectId) -> Vec<TargetEntity> {
    vec![TargetEntity::Object { object: o }]
}
fn token_objects(g: &Game) -> Vec<ObjectId> {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| g.state.rules.tokens.contains_key(&o.id.card))
        .map(|o| o.id)
        .collect()
}
fn observation(g: &Game, viewer: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(viewer), Budget::unlimited())
    else {
        panic!("observation")
    };
    o
}
fn lands(name: &str, n: usize) -> Vec<(&str, u8, bool)> {
    vec![(name, 0, false); n]
}

fn priority_for(g: &mut Game, player: u8) {
    for _ in 0..20 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && r.player() == PlayerId(player) {
            return;
        }
        answer(g, automatic(&r, &[]));
    }
    panic!("priority not reached");
}
fn counter(g: &mut Game, card: u32, target: StackId) {
    answer(g, Response::Cast { card: CardId(card) });
    pay_proposal(g, vec![], Some(target));
    check(g);
}
fn activated(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    answer(
        g,
        Response::ActivateAbility {
            object: object(g, id),
            ability: 0,
        },
    );
    pay_proposal(g, targets, None);
}
#[test]
fn removal_profile_decks_policy_and_reviewed_capacities() {
    let c = catalog();
    assert_eq!(c.schema_version, mtg_contracts::CURRENT_IR_VERSION);
    assert_eq!(
        digest(&c).unwrap(),
        match_profile("commander-removal-v1")
            .unwrap()
            .catalog_sha256
            .as_str()
    );
    let cfg = MatchConfig::for_profile("commander-removal-v1", [9; 32], PlayerId(0)).unwrap();
    assert_eq!(validate_pod(&c, &cfg).unwrap().config.cards.len(), 400);
    for (deck, n) in cfg.decks.iter().zip([58, 31, 71, 75]) {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
        assert_eq!(
            deck.cards
                .iter()
                .filter(|r| c
                    .definitions
                    .iter()
                    .any(|d| d.oracle_id == r.oracle_id && d.characteristics.basic))
                .map(|r| r.count)
                .sum::<u32>(),
            n
        );
    }
    let max_creatures = c
        .definitions
        .iter()
        .filter(|d| {
            d.characteristics.kind == CardKind::Creature
                && d.characteristics
                    .color_identity
                    .iter()
                    .all(|c| matches!(c, Color::B | Color::G))
        })
        .count();
    assert_eq!(max_creatures, 34);
    let g = Game::new(prepared(&[("Grizzly Bears", 0, false)]));
    assert_eq!(
        (
            g.token_capacity(),
            g.damage_limit(),
            g.stack_limit(),
            g.event_group_limit(),
            g.effect_limit()
        ),
        (
            capacities::TOKEN_CAPACITY,
            capacities::DAMAGE_LIMIT,
            capacities::STACK_LIMIT,
            capacities::EVENT_GROUP_LIMIT,
            capacities::EFFECT_LIMIT
        )
    );
    assert!(
        validate_pod(
            &c,
            &MatchConfig::for_profile("commander-attachments-v1", [9; 32], PlayerId(0)).unwrap()
        )
        .unwrap_err()
        .to_string()
        .contains("catalog")
    );
}
#[test]
fn swords_commander_equipment_lki_precedes_exile_return_and_public_exile_survives_restore() {
    let mut rows = vec![
        ("Isamaru, Hound of Konda", 0, false),
        ("Bonesplitter", 0, false),
        ("Swords to Plowshares", 1, true),
    ];
    rows.extend(lands("Plains", 3));
    rows.push(("Plains", 1, false));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 1, target(object(g, 0)));
        resolve(g, vec![]);
        assert_eq!(
            g.characteristics(object(g, 0)).unwrap().power,
            Some(4.into())
        );
        priority_for(g, 1);
        announce(g, 2, target(object(g, 0)));
        let mut choice = false;
        for _ in 0..100 {
            let r = request(g);
            if matches!(r, Request::CommanderReturn { .. }) {
                assert_eq!(g.state.rules.players[0].life, Quantity::from(44));
                assert_eq!(g.zone(object(g, 0)), Some(Zone::Exile));
                for seat in 0..4 {
                    let o = observation(g, seat);
                    assert_eq!(o.view.players[0].exile.len(), 1);
                    assert!(
                        o.view
                            .objects
                            .iter()
                            .any(|o| o.zone == Zone::Exile && o.oracle_id == ISAMARU)
                    );
                }
                check(g);
                answer(g, Response::CommanderReturn { to_command: true });
                choice = true;
                break;
            }
            answer(g, automatic(&r, &[]));
        }
        assert!(choice);
        resolve(g, vec![]);
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Command { owner: PlayerId(0) })
        );
        assert!(g.current(object(g, 1)).unwrap().attached_to.is_none());
        assert!(observation(g, 0).view.players[0].exile.is_empty());
    });
}
#[test]
fn swords_token_life_then_exile_cessation() {
    let mut rows = vec![
        ("Raise the Alarm", 0, true),
        ("Swords to Plowshares", 0, true),
    ];
    rows.extend(lands("Plains", 3));
    traces(prepared(&rows), |g| {
        cast(g, 0, vec![]);
        let t = token_objects(g)[0];
        cast(g, 1, target(t));
        assert!(g.current(t).is_err());
        assert_eq!(token_objects(g).len(), 1);
        assert_eq!(g.state.rules.players[0].life, Quantity::from(41));
        assert!(observation(g, 0).view.players[0].exile.is_empty());
    });
}
#[test]
fn disenchant_equipment_removes_bonus_keeps_creature() {
    let mut rows = vec![
        ("Grizzly Bears", 0, false),
        ("Bonesplitter", 0, false),
        ("Disenchant", 0, true),
    ];
    rows.extend(lands("Plains", 4));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 1, target(object(g, 0)));
        resolve(g, vec![]);
        assert_eq!(
            g.characteristics(object(g, 0)).unwrap().power,
            Some(4.into())
        );
        cast(g, 2, target(object(g, 1)));
        assert_eq!(g.zone(object(g, 0)), Some(Zone::Battlefield));
        assert_eq!(
            g.characteristics(object(g, 0)).unwrap().power,
            Some(2.into())
        );
    });
}
#[test]
fn disenchant_pacifism_restores_attack_and_naturalize_anthem_updates_pt() {
    for anthem in [false, true] {
        let mut rows = vec![
            ("Grizzly Bears", 0, false),
            (
                if anthem {
                    "Glorious Anthem"
                } else {
                    "Pacifism"
                },
                0,
                true,
            ),
            (if anthem { "Naturalize" } else { "Disenchant" }, 0, true),
        ];
        rows.extend(lands("Plains", 6));
        rows.push(("Forest", 0, false));
        traces(prepared(&rows), |g| {
            cast(g, 1, if anthem { vec![] } else { target(object(g, 0)) });
            if anthem {
                assert_eq!(
                    g.characteristics(object(g, 0)).unwrap().power,
                    Some(3.into())
                );
            } else {
                assert!(g.has_restriction(object(g, 0), CreatureRestriction::CantAttackOrBlock));
            }
            cast(g, 2, target(object(g, 1)));
            assert_eq!(
                g.characteristics(object(g, 0)).unwrap().power,
                Some(2.into())
            );
            assert!(!g.has_restriction(object(g, 0), CreatureRestriction::CantAttackOrBlock));
            assert!(attackers(g).contains(&object(g, 0)));
        });
    }
}
#[test]
fn beast_within_land_rock_and_own_permanent_controller_tokens_and_mana_windows() {
    for (name, owner) in [("Forest", 1), ("Sol Ring", 1), ("Sol Ring", 0)] {
        let mut rows = vec![(name, owner, false), ("Beast Within", 0, true)];
        rows.extend(lands("Forest", 3));
        traces(prepared(&rows), |g| {
            let old = object(g, 0);
            cast(g, 1, target(old));
            assert_eq!(
                g.zone(object(g, 0)),
                Some(Zone::Graveyard {
                    owner: PlayerId(owner)
                })
            );
            let t = token_objects(g);
            assert_eq!(t.len(), 1);
            assert_eq!(g.current(t[0]).unwrap().controller, PlayerId(owner));
            assert_eq!(g.characteristics(t[0]).unwrap().power, Some(3.into()));
            assert!(
                g.validate_response(
                    PlayerId(owner),
                    &Response::ActivateAbility {
                        object: old,
                        ability: 0
                    }
                )
                .is_err()
            );
            assert!(
                !g.state
                    .rules
                    .objects
                    .values()
                    .any(|o| o.id == old && o.zone == Zone::Battlefield)
            );
        });
    }
}
#[test]
fn generous_gift_unsummon_response_skips_entire_program() {
    let mut rows = vec![
        ("Generous Gift", 0, true),
        ("Unsummon", 1, true),
        ("Grizzly Bears", 2, false),
    ];
    rows.extend(lands("Plains", 3));
    rows.push(("Island", 1, false));
    traces(prepared(&rows), |g| {
        let t = object(g, 2);
        announce(g, 0, target(t));
        priority_for(g, 1);
        announce(g, 1, target(t));
        resolve(g, vec![]);
        assert!(token_objects(g).is_empty());
        assert_eq!(
            g.zone(object(g, 2)),
            Some(Zone::Hand { owner: PlayerId(2) })
        );
    });
}
#[test]
fn natures_claim_life_to_artifact_controller() {
    let mut rows = vec![("Sol Ring", 1, false), ("Nature's Claim", 0, true)];
    rows.extend(lands("Forest", 1));
    traces(prepared(&rows), |g| {
        cast(g, 1, target(object(g, 0)));
        assert_eq!(g.state.rules.players[1].life, Quantity::from(44));
        assert_eq!(g.state.rules.players[0].life, Quantity::from(40));
    });
}
#[test]
fn feed_swarm_token_zero_mv_commander_printed_mv_and_opponent_filter() {
    let mut rows = vec![
        ("Feed the Swarm", 0, true),
        ("Raise the Alarm", 1, true),
        ("Grizzly Bears", 0, false),
    ];
    rows.extend(lands("Swamp", 2));
    rows.extend([("Plains", 1, false); 2]);
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        priority_for(g, 1);
        cast(g, 1, vec![]);
        priority_for(g, 0);
        answer(g, Response::Cast { card: CardId(0) });
        let Request::Targets { candidates, .. } = request(g) else {
            panic!()
        };
        assert!(!candidates.contains(&object(g, 2)));
        let t = token_objects(g)[0];
        pay_proposal(g, target(t), None);
        resolve(g, vec![]);
        assert_eq!(g.state.rules.players[0].life, Quantity::from(40));
        assert!(g.current(t).is_err());
    });
    let mut rows = vec![
        ("Feed the Swarm", 0, true),
        ("Yargle and Multani", 1, false),
    ];
    rows.extend(lands("Swamp", 2));
    traces(prepared(&rows), |g| {
        cast(g, 0, target(object(g, 1)));
        assert_eq!(g.state.rules.players[0].life, Quantity::from(34));
        assert_eq!(
            g.state
                .rules
                .commanders
                .history(CardId(1))
                .unwrap()
                .command_casts
                .len(),
            0
        );
    });
}
#[test]
fn doom_blade_excludes_black_and_target_restrictions_apply_to_new_domains() {
    let mut rows = vec![
        ("Doom Blade", 0, true),
        ("Yargle and Multani", 1, false),
        ("Grizzly Bears", 1, false),
        ("Swiftfoot Boots", 1, false),
        ("Lightning Greaves", 0, false),
        ("Grizzly Bears", 0, false),
        ("Beast Within", 0, true),
    ];
    rows.extend(lands("Swamp", 2));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        answer(g, Response::Cast { card: CardId(0) });
        let Request::Targets { candidates, .. } = request(g) else {
            panic!()
        };
        assert!(!candidates.contains(&object(g, 1)));
        assert!(candidates.contains(&object(g, 2)));
        pay_proposal(g, target(object(g, 2)), None);
        resolve(g, vec![]);
    });
    for equipment in ["Swiftfoot Boots", "Lightning Greaves"] {
        let mut rows = vec![
            ("Grizzly Bears", 0, false),
            (equipment, 0, false),
            ("Swords to Plowshares", 1, true),
        ];
        rows.extend(lands("Plains", 1));
        rows.push(("Plains", 1, false));
        traces(prepared(&rows), |g| {
            reach(g, TurnStep::PrecombatMain);
            activated(g, 1, target(object(g, 0)));
            resolve(g, vec![]);
            priority_for(g, 1);
            let r = request(g);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Cast { card: CardId(2) }
                )
                .is_err()
            );
        });
    }
}
#[test]
fn chupacabra_no_legal_opposing_trigger_target_removed() {
    let mut rows = vec![
        ("Grizzly Bears", 0, false),
        ("Lightning Greaves", 0, false),
        ("Ravenous Chupacabra", 1, true),
    ];
    rows.extend(lands("Plains", 1));
    rows.extend([("Swamp", 1, false); 4]);
    // Equip is sorcery-only. Seat0 equips on its turn; seat1 casts Chupacabra
    // on its next main phase, with the only opposing creature now shrouded.
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 1, target(object(g, 0)));
        resolve(g, vec![]);
        for _ in 0..100 {
            let r = request(g);
            if g.state.rules.active == Some(PlayerId(1))
                && g.turn().step == TurnStep::PrecombatMain
                && matches!(r, Request::Priority { .. })
            {
                break;
            }
            let response = match r {
                Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                _ => automatic(&r, &[]),
            };
            answer(g, response);
        }
        cast(g, 2, vec![]);
        assert!(g.state.rules.stack.is_empty());
        if g.trace.mode == TraceMode::Detailed {
            assert!(g.events().iter().any(|e| matches!(
                e,
                Event::TriggeredAbilityNotPlaced {
                    reason: TriggerPlacementFailure::NoLegalTargets,
                    ..
                }
            )));
        }
        assert_eq!(g.zone(object(g, 0)), Some(Zone::Battlefield));
    });
}
#[test]
fn reclamation_sage_may_declined_after_target_selection() {
    let mut rows = vec![("Reclamation Sage", 0, true), ("Sol Ring", 1, false)];
    rows.extend(lands("Forest", 3));
    traces(prepared(&rows), |g| {
        announce(g, 0, vec![]);
        for _ in 0..100 {
            let r = request(g);
            if matches!(r, Request::OptionalEffect { .. }) {
                check(g);
                answer(g, Response::OptionalEffect { perform: false });
                break;
            }
            answer(g, automatic(&r, &target(object(g, 1))));
        }
        resolve(g, vec![]);
        assert_eq!(g.zone(object(g, 1)), Some(Zone::Battlefield));
    });
}
#[test]
fn zealot_targets_before_cost_artist_triggers_and_stifle_counters_detached_ability() {
    let mut rows = vec![
        ("Viridian Zealot", 0, false),
        ("Blood Artist", 0, false),
        ("Sol Ring", 2, false),
        ("Stifle", 1, true),
    ];
    rows.extend(lands("Forest", 2));
    rows.push(("Island", 1, false));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        answer(
            g,
            Response::ActivateAbility {
                object: object(g, 0),
                ability: 0,
            },
        );
        assert_eq!(g.zone(object(g, 0)), Some(Zone::Battlefield));
        assert!(matches!(request(g), Request::Targets { .. }));
        check(g);
        pay_proposal(g, target(object(g, 2)), None);
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert!(
            !g.state.rules.pending_triggers.is_empty()
                || g.state
                    .rules
                    .stack
                    .iter()
                    .any(|e| matches!(e.kind, StackKind::Triggered { .. }))
        );
        // Place Blood Artist's mandatory target before the next priority.
        for _ in 0..30 {
            let r = request(g);
            if matches!(r, Request::Priority { .. }) {
                break;
            }
            answer(g, automatic(&r, &[]));
        }
        let ability = g
            .state
            .rules
            .stack
            .iter()
            .find(|e| matches!(e.kind, StackKind::Activated { .. }))
            .unwrap()
            .id;
        priority_for(g, 1);
        counter(g, 3, ability);
        resolve(g, vec![]);
        assert_eq!(g.zone(object(g, 2)), Some(Zone::Battlefield));
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert_eq!(g.state.rules.players[1].life, Quantity::from(39));
    });
}
#[test]
fn smash_furnace_six_noncombat_damage_no_commander_history() {
    let mut rows = vec![
        ("Furnace of Rath", 0, false),
        ("Sol Ring", 1, false),
        ("Smash to Smithereens", 0, true),
    ];
    rows.extend(lands("Mountain", 2));
    traces(prepared(&rows), |g| {
        cast(g, 2, target(object(g, 1)));
        assert_eq!(g.state.rules.players[1].life, Quantity::from(34));
        assert!(g.state.rules.commanders.records.is_empty());
    });
}
#[test]
fn pongify_token_and_commander_compensation_before_return() {
    let mut rows = vec![("Pongify", 0, true), ("Isamaru, Hound of Konda", 1, false)];
    rows.extend(lands("Island", 1));
    traces(prepared(&rows), |g| {
        cast(g, 0, target(object(g, 1)));
        let t = token_objects(g);
        assert_eq!(t.len(), 1);
        assert_eq!(g.current(t[0]).unwrap().controller, PlayerId(1));
        assert_eq!(
            g.definition(t[0].card).unwrap().characteristics.subtypes,
            ["Ape"]
        );
    });
    let mut rows = vec![("Pongify", 0, true), ("Raise the Alarm", 0, true)];
    rows.extend(lands("Island", 1));
    rows.extend(lands("Plains", 2));
    traces(prepared(&rows), |g| {
        cast(g, 1, vec![]);
        let t = token_objects(g)[0];
        cast(g, 0, target(t));
        assert!(g.current(t).is_err());
        assert_eq!(token_objects(g).len(), 2);
    });
}
#[test]
fn swan_counters_pacifism_and_day_with_controller_bird_excludes_creature_equipment() {
    for name in [
        "Pacifism",
        "Day of Judgment",
        "Grizzly Bears",
        "Bonesplitter",
    ] {
        let mut rows = vec![
            (name, 0, true),
            ("Swan Song", 1, true),
            ("Grizzly Bears", 2, false),
        ];
        rows.extend(lands("Plains", 4));
        rows.extend(lands("Forest", 2));
        rows.push(("Island", 1, false));
        traces(prepared(&rows), |g| {
            announce(
                g,
                0,
                if name == "Pacifism" {
                    target(object(g, 2))
                } else {
                    vec![]
                },
            );
            let id = g.top().unwrap().id;
            priority_for(g, 1);
            if matches!(name, "Grizzly Bears" | "Bonesplitter") {
                let r = request(g);
                assert!(
                    g.respond(
                        r.player(),
                        r.decision_id(),
                        Response::Cast { card: CardId(1) }
                    )
                    .is_err()
                );
                resolve(g, vec![]);
                assert!(token_objects(g).is_empty());
            } else {
                counter(g, 1, id);
                resolve(g, vec![]);
                let t = token_objects(g);
                assert_eq!(t.len(), 1);
                assert_eq!(g.current(t[0]).unwrap().controller, PlayerId(0));
                assert!(g.has_keyword(t[0], KeywordAbility::Flying));
                assert_eq!(g.zone(object(g, 2)), Some(Zone::Battlefield));
            }
        });
    }
}
#[test]
fn destroy_attached_creature_aura_sba_equipment_detach_and_rancor_return() {
    let mut rows = vec![
        ("Grizzly Bears", 0, false),
        ("Bonesplitter", 0, false),
        ("Rancor", 0, true),
        ("Pongify", 0, true),
    ];
    rows.extend(lands("Forest", 2));
    rows.extend(lands("Island", 1));
    traces(prepared(&rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        activated(g, 1, target(object(g, 0)));
        resolve(g, vec![]);
        cast(g, 2, target(object(g, 0)));
        cast(g, 3, target(object(g, 0)));
        assert_eq!(
            g.zone(object(g, 2)),
            Some(Zone::Hand { owner: PlayerId(0) })
        );
        assert_eq!(g.zone(object(g, 1)), Some(Zone::Battlefield));
        assert!(g.current(object(g, 1)).unwrap().attached_to.is_none());
        if g.trace.mode == TraceMode::Detailed {
            assert!(g.trace().records.iter().any(|r|matches!(&r.record,EvidenceRecord::Batch {batch} if batch.cause==BatchCause::StateBased
                && batch.occurrences.iter().any(|o|matches!(o.proposed,ProposedEvent::Attach {object,target:None} if object.card==CardId(1)))
                && batch.occurrences.iter().any(|o|matches!(o.actual,EventOutcome::Moved {ref movement} if movement.previous.card==CardId(2) && matches!(movement.destination,Zone::Graveyard {..}))))));
        }
    });
}

#[test]
fn revoke_exiles_equipment_and_sage_chupacabra_destroy_legal_trigger_targets() {
    let mut rows = vec![
        ("Revoke Existence", 0, true),
        ("Bonesplitter", 1, false),
        ("Reclamation Sage", 0, true),
        ("Sol Ring", 1, false),
        ("Ravenous Chupacabra", 0, true),
        ("Grizzly Bears", 1, false),
    ];
    rows.extend(lands("Plains", 2));
    rows.extend(lands("Forest", 3));
    rows.extend(lands("Swamp", 4));
    traces(prepared(&rows), |g| {
        cast(g, 0, target(object(g, 1)));
        assert_eq!(g.zone(object(g, 1)), Some(Zone::Exile));
        check(g);
        assert_eq!(observation(g, 2).view.players[1].exile.len(), 1);
        cast(g, 2, target(object(g, 3)));
        assert_eq!(
            g.zone(object(g, 3)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
        cast(g, 4, target(object(g, 5)));
        assert_eq!(
            g.zone(object(g, 5)),
            Some(Zone::Graveyard { owner: PlayerId(1) })
        );
    });
}

fn resolve_to_command(g: &mut Game) {
    for _ in 0..200 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && g.state.rules.stack.is_empty() {
            return;
        }
        let response = if matches!(r, Request::CommanderReturn { .. }) {
            Response::CommanderReturn { to_command: true }
        } else {
            automatic(&r, &[])
        };
        answer(g, response);
    }
    panic!("commander resolution cap")
}
fn main_for(g: &mut Game, seat: u8) {
    for _ in 0..400 {
        let r = request(g);
        if g.active_player() == Some(PlayerId(seat))
            && g.turn().step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { .. })
        {
            return;
        }
        let response = match r {
            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
            _ => automatic(&r, &[]),
        };
        answer(g, response);
    }
    panic!("main phase cap")
}
#[test]
fn feed_swarm_uses_six_mana_value_after_eight_mana_taxed_commander_cast() {
    let mut rows = vec![
        ("Yargle and Multani", 0, false),
        ("Unsummon", 0, true),
        ("Unsummon", 1, true),
        ("Feed the Swarm", 1, true),
    ];
    rows.extend(lands("Swamp", 10));
    rows.extend(lands("Forest", 5));
    rows.extend(lands("Island", 1));
    rows.push(("Island", 1, false));
    rows.extend([("Swamp", 1, false); 2]);
    traces(prepared(&rows), |g| {
        announce(g, 1, target(object(g, 0)));
        resolve_to_command(g);
        announce(g, 0, vec![]);
        resolve(g, vec![]);
        priority_for(g, 1);
        announce(g, 2, target(object(g, 0)));
        resolve_to_command(g);
        priority_for(g, 0);
        announce(g, 0, vec![]);
        resolve(g, vec![]);
        assert_eq!(
            g.state
                .rules
                .commanders
                .history(CardId(0))
                .unwrap()
                .command_casts
                .get(&PlayerId(0)),
            Some(&2.into())
        );
        main_for(g, 1);
        cast(g, 3, target(object(g, 0)));
        assert_eq!(g.state.rules.players[1].life, Quantity::from(34));
    });
}
#[test]
fn removal_token_capacity_fault_is_typed_atomic_and_replayable_private_domain() {
    // Labelled compiler-domain fixture, not a claimed reachable accepted-pool
    // witness. It proves the overflow mechanism without manufacturing tokens.
    let mut p = prepared(&[
        ("Generous Gift", 0, true),
        ("Sol Ring", 1, false),
        ("Plains", 0, false),
        ("Plains", 0, false),
        ("Plains", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Generous Gift")
        .unwrap();
    let Instruction::CreateTokensFor { count, .. } = &mut d.spell.as_mut().unwrap().instructions[1]
    else {
        panic!()
    };
    *count = capacities::TOKEN_CAPACITY + 1;
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        announce(g, 0, target(object(g, 1)));
        for _ in 0..4 {
            answer(g, Response::Pass);
        }
        assert!(
            matches!(g.advance(Budget::unlimited()),Advance::Fault(ref reason) if reason=="token capacity exceeded")
        );
        assert_eq!(
            g.typed_fault(),
            Some(&RuleFault::TokenCapacityExceeded {
                capacity: capacities::TOKEN_CAPACITY,
                created: 0,
                requested: capacities::TOKEN_CAPACITY + 1
            })
        );
        assert!(g.state.rules.tokens.is_empty());
    });
}
#[test]
fn removal_effect_capacity_is_typed_and_atomic_in_private_state_domain() {
    // Private state-domain probe: it does not claim an accepted-input replay.
    let mut p = prepared(&[
        ("Giant Growth", 0, true),
        ("Grizzly Bears", 0, false),
        ("Forest", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Giant Growth")
        .unwrap();
    let program = d.spell.as_mut().unwrap();
    program.instructions = vec![program.instructions[0].clone(); 2];
    c.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::with_trace(p.clone(), mode);
        let victim = object(&g, 1);
        announce(&mut g, 0, target(victim));
        for _ in 0..4 {
            answer(&mut g, Response::Pass);
        }
        while g.state.rules.effects.is_empty() {
            assert!(matches!(
                g.advance(Budget::new(1)),
                Advance::Suspended { .. }
            ));
        }
        let effect = g.state.rules.effects[0].clone();
        g.state.rules.effects = vec![effect; capacities::EFFECT_LIMIT];
        let mut expected = g.state.clone();
        let trace = g.trace.clone();
        assert_eq!(
            g.advance(Budget::new(1)),
            Advance::Fault("effect capacity exceeded".into())
        );
        expected.rules.typed_fault = Some(RuleFault::EffectCapacityExceeded {
            capacity: capacities::EFFECT_LIMIT,
            requested: capacities::EFFECT_LIMIT + 1,
        });
        expected.rules.frame = Frame::Fault {
            reason: "effect capacity exceeded".into(),
        };
        assert_eq!(g.state, expected);
        assert_eq!(g.trace, trace);
    }
}
#[test]
fn swan_mixed_stack_rejects_wrong_typed_response_without_mutation() {
    let mut rows = vec![
        ("Grizzly Bears", 0, true),
        ("Unsummon", 1, true),
        ("Swan Song", 2, true),
        ("Grizzly Bears", 3, false),
    ];
    rows.extend(lands("Forest", 2));
    rows.push(("Island", 1, false));
    rows.push(("Island", 2, false));
    traces(prepared(&rows), |g| {
        announce(g, 0, vec![]);
        let creature = g.top().unwrap().id;
        priority_for(g, 1);
        announce(g, 1, target(object(g, 3)));
        let instant = g.top().unwrap().id;
        priority_for(g, 2);
        answer(g, Response::Cast { card: CardId(2) });
        let r = request(g);
        let Request::StackTargets { ref candidates, .. } = r else {
            panic!()
        };
        assert!(candidates.contains(&instant));
        assert!(!candidates.contains(&creature));
        let state = g.state_digest().unwrap();
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::StackTargets {
                    entries: vec![creature]
                }
            )
            .is_err()
        );
        assert_eq!(g.state_digest().unwrap(), state);
        check(g);
        pay_proposal(g, vec![], Some(instant));
        resolve(g, vec![]);
        assert_eq!(g.zone(object(g, 0)), Some(Zone::Battlefield));
        let t = token_objects(g);
        assert_eq!(t.len(), 1);
        assert_eq!(g.current(t[0]).unwrap().controller, PlayerId(1));
    });
}
// Reviewed P6.8 regression: CR 603.5 makes "may" the controller's resolution
// choice. A controller who concedes as the trigger begins resolving makes none,
// matching a concession at Request::OptionalEffect.
#[test]
fn sage_may_is_not_performed_for_a_departed_controller() {
    let mut rows = vec![("Reclamation Sage", 0, true), ("Sol Ring", 1, false)];
    rows.extend(lands("Forest", 3));
    traces(prepared(&rows), |g| {
        let ring = object(g, 1);
        announce(g, 0, vec![]);
        for _ in 0..4000 {
            if matches!(&g.state.rules.frame, Frame::Resolve { frame }
                if matches!(frame.spell.kind, StackKind::Triggered { .. }))
            {
                break;
            }
            match g.advance(Budget::new(1)) {
                Advance::Suspended { .. } => {}
                Advance::AwaitingDecision(r) => {
                    let response = automatic(&r, &target(ring));
                    g.respond(r.player(), r.decision_id(), response).unwrap();
                }
                x => panic!("{x:?}"),
            }
        }
        g.concede(PlayerId(0)).unwrap();
        while matches!(g.advance(Budget::new(1)), Advance::Suspended { .. }) {}
        assert_eq!(g.zone(object(g, 1)), Some(Zone::Battlefield));
    });
}

#[test]
fn combined_types_participate_in_every_filter_and_set() {
    let spec = |filter: &PermanentFilter| TargetSpec {
        slot: 0,
        selection_id: "combined-type-target".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldPermanent,
        filter: Some(filter.clone()),
    };
    let mut p = prepared(&[("Grizzly Bears", 0, false), ("Llanowar Elves", 0, false)]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    d.characteristics.additional_types = vec![CardKind::Enchantment, CardKind::Artifact];
    d.capabilities = d.requirements();
    d.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    let g = Game::new(p);
    let hybrid = object(&g, 0);
    let ordinary = object(&g, 1);
    for kind in [
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Creature,
    ] {
        let f = PermanentFilter {
            types: vec![kind],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        };
        assert!(g.permanent_target_legal(hybrid, PlayerId(1), &spec(&f)));
        assert_eq!(
            g.permanent_target_legal(ordinary, PlayerId(1), &spec(&f)),
            kind == CardKind::Creature
        );
        let excluded = PermanentFilter {
            types: vec![],
            non_types: vec![kind],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        };
        assert!(!g.permanent_target_legal(hybrid, PlayerId(1), &spec(&excluded)));
    }
    let f = SacrificeFilter {
        types: vec![CardKind::Artifact],
        subtypes: vec![],
    };
    assert_eq!(g.sacrifice_candidates(PlayerId(0), &f), vec![hybrid]);
    let union = SacrificeFilter {
        types: vec![CardKind::Artifact, CardKind::Creature],
        subtypes: vec![],
    };
    assert_eq!(
        g.sacrifice_candidates(PlayerId(0), &union),
        vec![hybrid, ordinary]
    );
    assert_eq!(
        g.objects_in_set(&ObjectSet::Enchantments, PlayerId(0)),
        vec![hybrid]
    );
    assert_eq!(
        g.objects_in_set(&ObjectSet::Creatures, PlayerId(0)),
        vec![hybrid, ordinary]
    );
    let c = g.characteristics(hybrid).unwrap();
    assert!(
        c.has_type(CardKind::Artifact)
            && c.has_type(CardKind::Enchantment)
            && c.has_type(CardKind::Creature)
    );
}

#[test]
fn artifact_creature_cast_is_both_an_artifact_spell_and_a_creature_permanent() {
    let mut p = prepared(&[
        ("Grizzly Bears", 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
    ]);
    let c = Arc::make_mut(&mut p.catalog);
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Grizzly Bears")
        .unwrap();
    d.characteristics.additional_types = vec![CardKind::Artifact];
    d.capabilities = d.requirements();
    d.validate().unwrap();
    p.catalog_digest = digest(c).unwrap();
    traces(p, |g| {
        announce(g, 0, vec![]);
        for kind in [CardKind::Artifact, CardKind::Creature] {
            let spec = TargetSpec {
                slot: 0,
                selection_id: "spell".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::StackSpell,
                filter: Some(PermanentFilter {
                    types: vec![kind],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                }),
            };
            assert_eq!(g.eligible_stack_targets_for(&spec, None).len(), 1);
        }
        resolve(g, vec![]);
        assert_eq!(g.current(object(g, 0)).unwrap().zone, Zone::Battlefield);
        assert!(
            !attackers(g).contains(&object(g, 0)),
            "a newly cast artifact creature is summoning sick"
        );
    });
}

#[test]
fn legend_rule_controller_keeps_each_permanent_type_through_public_choices_and_replay() {
    // CR 704.5j applies to every legendary permanent, independently of owner.
    for kind in [
        CardKind::Creature,
        CardKind::Artifact,
        CardKind::Enchantment,
        CardKind::Land,
    ] {
        let name = catalog()
            .definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == kind
                    && d.characteristics
                        .subtypes
                        .iter()
                        .all(|s| s != "Aura" && s != "Equipment")
            })
            .unwrap()
            .name
            .clone();
        for trace in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let mut p = prepared(&[(&name, 0, false), (&name, 0, false), (&name, 1, false)]);
            let d = Arc::make_mut(&mut p.catalog)
                .definitions
                .iter_mut()
                .find(|d| d.name == name)
                .unwrap();
            d.characteristics.legendary = true;
            d.capabilities = d.requirements();
            d.validate().unwrap();
            p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
            let mut g = Game::with_trace(p.clone(), trace);
            answer(&mut g, Response::Pass);
            let (first, keep, opponent) = (object(&g, 0), object(&g, 1), object(&g, 2));
            let Advance::AwaitingDecision(Request::LegendRule {
                player, candidates, ..
            }) = g.advance(Budget::unlimited())
            else {
                panic!("legend choice")
            };
            assert_eq!(player, PlayerId(0));
            assert_eq!(candidates, [first, keep]);
            assert!(
                g.respond(
                    PlayerId(1),
                    g.state.rules.revision,
                    Response::LegendRule { keep }
                )
                .is_err()
            );
            assert!(
                g.respond(
                    player,
                    g.state.rules.revision,
                    Response::LegendRule { keep: opponent }
                )
                .is_err()
            );
            let QueryResult::Complete(QueryValue::Observation(observation)) =
                g.observe(player, Budget::unlimited())
            else {
                panic!("observation")
            };
            let Some(PlayerRequest::LegendRule {
                token, candidates, ..
            }) = observation.decision
            else {
                panic!("public legend choice")
            };
            check(&g);
            let response = PlayerResponse::LegendRule {
                keep: candidates[1].clone(),
            };
            let QueryResult::Complete(QueryValue::Validated(receipt)) =
                g.check_player_response(player, token, response, Budget::unlimited())
            else {
                panic!("validated choice")
            };
            g.submit(receipt).unwrap();
            assert!(matches!(
                g.advance(Budget::unlimited()),
                Advance::AwaitingDecision(_)
            ));
            assert_eq!(g.zone(keep), Some(Zone::Battlefield));
            assert_eq!(g.zone(opponent), Some(Zone::Battlefield));
            assert!(g.current(first).is_err());
            assert_eq!(
                g.state.rules.objects[&first.card].zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            let replay = Game::replay(p.clone(), &g.replay_journal()).unwrap();
            assert_eq!(replay.state_digest().unwrap(), g.state_digest().unwrap());
            let restored = Game::restore(p, &g.snapshot().unwrap()).unwrap();
            assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
        }
    }
}

#[test]
fn legend_wave_finishes_before_fresh_commander_graveyard_return_choices() {
    // CR 704.5j / 903.9a: first put every unkept legend into its owner's
    // graveyard simultaneously, then offer the commanders' new incarnations.
    let mut p = prepared(&[
        ("Isamaru, Hound of Konda", 0, false),
        ("Isamaru, Hound of Konda", 0, false),
        ("Isamaru, Hound of Konda", 1, false),
        ("Isamaru, Hound of Konda", 1, false),
    ]);
    p.config.commanders = vec![CardId(0), CardId(2)];
    p.profile_digest = digest(&p.config).unwrap();
    traces(p, |g| {
        answer(g, Response::Pass);
        let first = object(g, 0);
        let second = object(g, 2);
        answer(g, Response::LegendRule { keep: object(g, 1) });
        assert!(matches!(
            request(g),
            Request::LegendRule {
                player: PlayerId(1),
                ..
            }
        ));
        assert_eq!(g.zone(first), Some(Zone::Battlefield));
        assert_eq!(g.zone(second), Some(Zone::Battlefield));
        answer(g, Response::LegendRule { keep: object(g, 3) });
        let r = request(g);
        for (old, owner) in [(first, PlayerId(0)), (second, PlayerId(1))] {
            assert!(g.current(old).is_err());
            assert_eq!(
                g.state.rules.objects[&old.card].zone,
                Zone::Graveyard { owner }
            );
        }
        assert!(
            matches!(r, Request::CommanderReturn { player: PlayerId(0), object, .. } if object == self::object(g, 0))
        );
        check(g);
        answer(g, Response::CommanderReturn { to_command: true });
        assert!(
            matches!(request(g), Request::CommanderReturn { player: PlayerId(1), object, .. } if object == self::object(g, 2))
        );
        check(g);
        answer(g, Response::CommanderReturn { to_command: false });
        request(g);
        assert_eq!(
            g.state.rules.objects[&first.card].zone,
            Zone::Command { owner: PlayerId(0) }
        );
        assert_eq!(
            g.state.rules.objects[&second.card].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert_eq!(g.zone(object(g, 1)), Some(Zone::Battlefield));
        assert_eq!(g.zone(object(g, 3)), Some(Zone::Battlefield));
    });
}

#[test]
fn artifact_land_survives_nonland_wipe_and_is_removed_by_artifact_removal() {
    let mut p = prepared(&[("Forest", 0, false)]);
    let d = Arc::make_mut(&mut p.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == FOREST)
        .unwrap();
    d.characteristics.additional_types = vec![CardKind::Artifact];
    d.capabilities = d.requirements();
    assert!(!d.capabilities.contains(&Capability::CastArtifactV1));
    d.validate().unwrap();
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    let mut g = Game::new(p);
    let land = object(&g, 0);
    assert!(
        !g.objects_in_set(&ObjectSet::NonlandPermanents, PlayerId(0))
            .contains(&land)
    );
    // The actual nonland wipe operator freezes this set before moving anything.
    for object in g.objects_in_set(&ObjectSet::NonlandPermanents, PlayerId(0)) {
        g.commit(
            ProposedEvent::DestroyPermanent { object },
            BatchCause::StateBased,
        )
        .unwrap();
    }
    assert_eq!(g.zone(land), Some(Zone::Battlefield));
    let target = TargetSpec {
        slot: 0,
        selection_id: "artifact".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldPermanent,
        filter: Some(PermanentFilter {
            types: vec![CardKind::Artifact],
            non_types: vec![],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        }),
    };
    assert!(g.permanent_target_legal(land, PlayerId(1), &target));
    g.commit(
        ProposedEvent::DestroyPermanent { object: land },
        BatchCause::StateBased,
    )
    .unwrap();
    assert_eq!(
        g.state.rules.objects[&land.card].zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
}

#[test]
fn legend_rule_choices_are_apnap_and_share_the_lethal_damage_wave() {
    let mut p = prepared(&[
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
        ("Grizzly Bears", 1, false),
    ]);
    let d = Arc::make_mut(&mut p.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == BEARS)
        .unwrap();
    d.characteristics.legendary = true;
    d.capabilities = d.requirements();
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    let mut g = Game::new(p);
    let (a, b, c, d) = (object(&g, 0), object(&g, 1), object(&g, 2), object(&g, 3));
    // A kept legend can still die from another simultaneous SBA (704.3).
    g.state
        .rules
        .objects
        .get_mut(&a.card)
        .unwrap()
        .marked_damage = Quantity::from(2);
    answer(&mut g, Response::Pass);
    let Advance::AwaitingDecision(r) = g.advance(Budget::unlimited()) else {
        panic!("legend")
    };
    assert_eq!(r.player(), PlayerId(0));
    g.respond(
        r.player(),
        r.decision_id(),
        Response::LegendRule { keep: a },
    )
    .unwrap();
    let Advance::AwaitingDecision(r) = g.advance(Budget::unlimited()) else {
        panic!("second legend")
    };
    assert!(matches!(&r, Request::LegendRule{previous_choices,..} if previous_choices == &[a]));
    assert_eq!(r.player(), PlayerId(1));
    assert!(
        [a, b, c, d]
            .iter()
            .all(|o| g.zone(*o) == Some(Zone::Battlefield))
    );
    g.respond(
        r.player(),
        r.decision_id(),
        Response::LegendRule { keep: d },
    )
    .unwrap();
    g.advance(Budget::unlimited());
    assert_eq!(g.zone(d), Some(Zone::Battlefield));
    assert!([a, b, c].iter().all(|o| g.current(*o).is_err()));
    assert!(g.trace().records.iter().any(|r| matches!(&r.record,
        EvidenceRecord::Batch {batch} if batch.cause == BatchCause::StateBased
        && batch.before.objects.iter().filter(|o|[a,b,c].contains(&o.object)).count()==3)));
}

#[test]
fn flash_casts_permanent_spells_on_opponents_turns_and_in_response_with_paired_rejections() {
    // CR 702.8a: use exactly the instant timing window, keeping normal payment
    // and stack resolution. Test all supported nonland permanent execution kinds.
    for kind in [
        CardKind::Creature,
        CardKind::Artifact,
        CardKind::Enchantment,
    ] {
        let name = catalog()
            .definitions
            .iter()
            .find(|d| {
                d.characteristics.kind == kind
                    && d.triggered.is_empty()
                    && d.characteristics
                        .subtypes
                        .iter()
                        .all(|s| s != "Aura" && s != "Equipment")
            })
            .unwrap()
            .name
            .clone();
        let mut p = prepared(&[(&name, 0, true), (&name, 1, true)]);
        Arc::make_mut(&mut p.catalog).schema_version = mtg_contracts::CURRENT_IR_VERSION;
        p.config.initial.turn_step = TurnStep::PrecombatMain;
        let d = Arc::make_mut(&mut p.catalog)
            .definitions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap();
        d.characteristics.mana_cost = ManaCost {
            generic: Quantity::zero(),
            colored: vec![],
        };
        let mut ordinary = p.clone();
        let d = Arc::make_mut(&mut p.catalog)
            .definitions
            .iter_mut()
            .find(|d| d.name == name)
            .unwrap();
        d.keywords.push(KeywordAbility::Flash);
        d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        d.capabilities = d.requirements();
        d.validate().unwrap();
        p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
        ordinary.catalog_digest = digest(ordinary.catalog.as_ref()).unwrap();
        for response_window in [false, true] {
            let mut g = Game::new(ordinary.clone());
            if response_window {
                announce(&mut g, 0, vec![]);
            }
            priority_for(&mut g, 1);
            let r = request(&mut g);
            let before = g.state_digest().unwrap();
            assert!(
                g.respond(
                    PlayerId(1),
                    r.decision_id(),
                    Response::Cast { card: CardId(1) }
                )
                .is_err()
            );
            assert_eq!(g.state_digest().unwrap(), before);
            traces(p.clone(), |g| {
                if response_window {
                    announce(g, 0, vec![]);
                }
                priority_for(g, 1);
                assert_eq!(g.state.rules.active, Some(PlayerId(0)));
                announce(g, 1, vec![]);
                assert_eq!(
                    g.state.rules.stack.len(),
                    if response_window { 2 } else { 1 }
                );
                resolve(g, vec![]);
                assert_eq!(g.state.rules.objects[&CardId(1)].zone, Zone::Battlefield);
            });
        }
    }
}

#[test]
fn enters_tapped_replaces_land_and_nonland_entry_before_any_priority_window() {
    let mut p = prepared(&[("Forest", 0, true), ("Grizzly Bears", 0, true)]);
    Arc::make_mut(&mut p.catalog).schema_version = mtg_contracts::CURRENT_IR_VERSION;
    for d in Arc::make_mut(&mut p.catalog)
        .definitions
        .iter_mut()
        .filter(|d| [FOREST, BEARS].contains(&d.oracle_id.as_str()))
    {
        d.static_abilities.push(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTapped,
        });
        if d.characteristics.kind == CardKind::Creature {
            d.characteristics.mana_cost = ManaCost {
                generic: Quantity::zero(),
                colored: vec![],
            };
        }
        d.capabilities = d.requirements();
        d.validate().unwrap();
    }
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    traces(p, |g| {
        reach(g, TurnStep::PrecombatMain);
        answer(g, Response::PlayLand { card: CardId(0) });
        request(g);
        let land = object(g, 0);
        assert!(g.current(land).unwrap().tapped);
        let r = request(g);
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateMana { object: land }
            )
            .is_err()
        );
        cast(g, 1, vec![]);
        assert!(g.current(object(g, 1)).unwrap().tapped);
        // Entry itself is the only event; there is no subsequent Tap outcome.
        assert!(!g.events().iter().any(|e| matches!(
            e,
            Event::Committed {
                actual: EventOutcome::Tapped { .. },
                ..
            }
        )));
    });
}

fn d2b_source_catalog() -> Catalog {
    let mut c = catalog();
    c.schema_version = mtg_contracts::CURRENT_IR_VERSION;
    c.definitions.extend(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/cards-data-d2b/expected-ir.json"
        ))
        .unwrap()
        .definitions,
    );
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    c
}

#[test]
fn archived_flash_sources_pay_and_resolve_in_response_on_an_opponents_turn() {
    let mut rows = vec![("Ambush Viper", 1, true), ("Daring Thunder-Thief", 1, true)];
    rows.extend(vec![("Forest", 1, false); 2]);
    rows.extend(vec![("Island", 1, false); 4]);
    traces(prepared_in(d2b_source_catalog(), &rows), |g| {
        reach(g, TurnStep::PrecombatMain);
        priority_for(g, 1);
        announce(g, 0, vec![]);
        announce(g, 1, vec![]);
        assert_eq!(g.state.rules.active, Some(PlayerId(0)));
        assert_eq!(g.state.rules.stack.len(), 2);
        resolve(g, vec![]);
        assert!(!g.current(object(g, 0)).unwrap().tapped);
        assert!(g.current(object(g, 1)).unwrap().tapped);
        assert!(
            g.current_keywords(object(g, 0))
                .contains(&KeywordAbility::Deathtouch)
        );
    });
}

#[test]
fn archived_bound_self_names_and_artifact_entry_replacements_execute_generically() {
    let mut rows = vec![
        ("Timeless Lotus", 0, true),
        ("Moss Diamond", 0, true),
        ("Winter Soldier, Bucky Barnes", 0, true),
    ];
    rows.extend(vec![("Forest", 0, false); 7]);
    rows.push(("Plains", 0, false));
    traces(prepared_in(d2b_source_catalog(), &rows), |g| {
        for id in 0..3 {
            cast(g, id, vec![]);
            let object = object(g, id);
            assert!(g.current(object).unwrap().tapped);
            let r = request(g);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::ActivateMana { object }
                )
                .is_err()
            );
        }
    });
}

#[test]
fn legend_rule_uses_controller_choices_owner_graveyards_and_revalidates_departures() {
    let mut p = prepared(&[
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
        ("Grizzly Bears", 1, false),
    ]);
    let d = Arc::make_mut(&mut p.catalog)
        .definitions
        .iter_mut()
        .find(|d| d.oracle_id == BEARS)
        .unwrap();
    d.characteristics.legendary = true;
    d.capabilities = d.requirements();
    p.catalog_digest = digest(p.catalog.as_ref()).unwrap();
    // Borrowed legend: controller chooses, but the other card's owner receives it.
    let mut borrowed = p.clone();
    borrowed.config.cards[1].owner = PlayerId(2);
    borrowed.profile_digest = digest(&borrowed.config).unwrap();
    traces(borrowed, |g| {
        answer(g, Response::Pass);
        answer(g, Response::LegendRule { keep: object(g, 0) });
        answer(g, Response::LegendRule { keep: object(g, 2) });
        request(g);
        assert_eq!(
            g.state.rules.objects[&CardId(1)].zone,
            Zone::Graveyard { owner: PlayerId(2) }
        );
    });
    traces(p, |g| {
        answer(g, Response::Pass);
        answer(g, Response::LegendRule { keep: object(g, 0) });
        let r = request(g);
        assert_eq!(r.player(), PlayerId(1));
        let stale_id = r.decision_id();
        g.concede(PlayerId(0)).unwrap();
        let r = request(g);
        assert!(
            matches!(&r,Request::LegendRule{player:PlayerId(1),previous_choices,..} if previous_choices.is_empty())
        );
        assert!(
            g.respond(
                PlayerId(1),
                stale_id,
                Response::LegendRule { keep: object(g, 2) }
            )
            .is_err()
        );
        answer(g, Response::LegendRule { keep: object(g, 3) });
        request(g);
        assert!(!g.state.rules.objects.contains_key(&CardId(0)));
        assert_eq!(
            g.state.rules.objects[&CardId(2)].zone,
            Zone::Graveyard { owner: PlayerId(1) }
        );
        assert_eq!(g.zone(object(g, 3)), Some(Zone::Battlefield));
    });
}
