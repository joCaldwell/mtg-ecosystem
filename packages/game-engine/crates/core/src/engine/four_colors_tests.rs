//! Private source-backed witness configurations exercise rules without broadening
//! public admission. Every natural witness proves restore/replay in all traces.
//! CR citations refer to the pinned 2026-09-25 rules:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-four-colors-ir.json"
    ))
    .unwrap()
}
fn prepared(rows: &[(&str, u8, bool)]) -> Prepared {
    let c = catalog();
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
        mode: Mode::CommanderMatch,
        players: 4,
        profile: FOUR_COLORS_PROFILE.into(),
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
    if matches!(g.state.frame, Frame::Fault { .. }) {
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
                entries: vec![stack_target.unwrap_or(g.state.stack[0].id)],
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
                    && g.state.stack.is_empty()
                    && g.state.pending_triggers.is_empty()
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
        .objects
        .values()
        .filter(|o| g.state.tokens.contains_key(&o.id.card))
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
fn edit_setup(mut p: Prepared, id: u32, zone: Zone) -> Prepared {
    p.config.cards[id as usize].zone = zone;
    p.profile_digest = digest(&p.config).unwrap();
    p
}
#[test]
fn profile_decks_admission_and_compiled_pool_resource_proof() {
    let c = catalog();
    let cfg = MatchConfig::for_profile(FOUR_COLORS_PROFILE, [9; 32], PlayerId(0)).unwrap();
    validate_pod(&c, &cfg).unwrap();
    assert_eq!(
        cfg.decks,
        [
            mana_white_deck(),
            mana_black_green_deck(),
            four_colors_blue_deck(),
            four_colors_red_deck()
        ]
    );
    for deck in &cfg.decks {
        assert_eq!(deck.cards.iter().map(|r| r.count).sum::<u32>(), 100);
    }
    assert_eq!(
        cfg.decks[2]
            .cards
            .iter()
            .find(|r| r.oracle_id == ISLAND)
            .unwrap()
            .count,
        80
    );
    assert_eq!(
        cfg.decks[3]
            .cards
            .iter()
            .find(|r| r.oracle_id == MOUNTAIN)
            .unwrap()
            .count,
        84
    );
    let mut wrong = cfg.clone();
    wrong.decks[3]
        .cards
        .iter_mut()
        .find(|r| r.oracle_id == BOLT)
        .unwrap()
        .oracle_id = COUNTERSPELL.into();
    assert!(matches!(
        validate_pod(&c, &wrong),
        Err(MatchAdmissionError::ColorIdentity { .. })
    ));
    let mut maxima = (0, 0, 0);
    for colors in [
        vec![Color::W],
        vec![Color::B, Color::G],
        vec![Color::U],
        vec![Color::R],
    ] {
        let legal = c
            .definitions
            .iter()
            .filter(|d| {
                d.characteristics
                    .color_identity
                    .iter()
                    .all(|c| colors.contains(c))
            })
            .collect::<Vec<_>>();
        maxima.0 = maxima.0.max(
            legal
                .iter()
                .filter(|d| d.characteristics.kind == CardKind::Creature)
                .count(),
        );
        let programs = legal
            .iter()
            .flat_map(|d| d.spell.iter().chain(d.triggered.iter().map(|t| &t.program)));
        let mut tokens = 0;
        let mut largest = 0;
        for p in programs {
            for i in &p.instructions {
                if let Instruction::CreateCreatureTokens { count, .. } = i {
                    tokens += *count as usize;
                    largest = largest.max(*count as usize);
                }
            }
        }
        maxima.1 = maxima.1.max(tokens);
        maxima.2 = maxima.2.max(largest);
        for d in legal {
            if d.characteristics.kind == CardKind::Creature {
                assert_eq!(d.spell.as_ref().unwrap().timing, SpellTiming::MainPhase);
            }
            for a in &d.mana_abilities {
                assert!(a.costs.tap);
                if let ManaProduction::Fixed { symbols } = &a.production {
                    assert!(symbols.len() <= 2);
                }
            }
        }
    }
    assert_eq!(maxima, (31, 9, 4));
    // <=8 producer recursions plus <=8 bounce/tuck uses, each adds <=4.
    assert_eq!(4 * maxima.1 + (8 + 8) * maxima.2, 100);
    let g = Game::new(prepared(&[]));
    assert_eq!(
        (
            g.damage_limit(),
            g.stack_limit(),
            g.event_group_limit(),
            g.token_capacity(),
            g.effect_limit()
        ),
        (760, 4080, 660, 256, 32)
    );
}
#[test]
fn counterspell_commander_grave_return_and_tax_survive_countering() {
    let p = edit_setup(
        prepared(&[
            ("Isamaru, Hound of Konda", 0, true),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Counterspell", 1, true),
            ("Island", 1, false),
            ("Island", 1, false),
        ]),
        0,
        Zone::Command { owner: PlayerId(0) },
    );
    traces(p, |g| {
        announce(g, 0, vec![]);
        let original = g.state.stack[0].id;
        priority_for(g, 1);
        counter(g, 5, original);
        loop {
            let r = request(g);
            if matches!(r, Request::CommanderReturn { .. }) {
                break;
            }
            answer(g, automatic(&r, &[]));
        }
        assert_eq!(
            g.zone(object(g, 0)),
            Some(Zone::Graveyard { owner: PlayerId(0) })
        );
        assert_eq!(
            g.state.commanders.history(CardId(0)).unwrap().command_casts[&PlayerId(0)],
            1.into()
        );
        check(g);
        answer(g, Response::CommanderReturn { to_command: true });
        resolve(g, vec![]);
        priority_for(g, 0);
        announce(g, 0, vec![]);
        assert_eq!(g.state.stack[0].total_cost.generic, 2.into());
        assert_eq!(
            g.state.commanders.history(CardId(0)).unwrap().command_casts[&PlayerId(0)],
            2.into()
        );
    });
}
#[test]
fn counterspell_token_spell_produces_no_tokens() {
    traces(
        prepared(&[
            ("Raise the Alarm", 0, true),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Counterspell", 1, true),
            ("Island", 1, false),
            ("Island", 1, false),
        ]),
        |g| {
            announce(g, 0, vec![]);
            let id = g.state.stack[0].id;
            priority_for(g, 1);
            counter(g, 3, id);
            resolve(g, vec![]);
            assert!(token_objects(g).is_empty());
            assert!(g.state.tokens.is_empty());
            assert_eq!(
                g.zone(object(g, 0)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
        },
    );
}
#[test]
fn stifle_new_enters_triggers_and_sacrificed_artifact_abilities() {
    for name in [
        "Hornet Queen",
        "Gravedigger",
        "Commander's Sphere",
        "Mind Stone",
    ] {
        let trigger = matches!(name, "Hornet Queen" | "Gravedigger");
        let mut rows = vec![
            (name, 0, trigger),
            ("Hapless Researcher", 0, false),
            ("Stifle", 1, true),
            ("Island", 1, false),
        ];
        for _ in 0..7 {
            rows.push((
                if name == "Hornet Queen" {
                    "Forest"
                } else {
                    "Swamp"
                },
                0,
                false,
            ));
        }
        let p = edit_setup(prepared(&rows), 1, Zone::Graveyard { owner: PlayerId(0) });
        traces(p, |g| {
            reach(g, TurnStep::PrecombatMain);
            if trigger {
                announce(g, 0, vec![]);
                // Resolve only the permanent, then place its enters invocation.
                for _ in 0..100 {
                    let r = request(g);
                    if matches!(r, Request::Priority { .. })
                        && g.state
                            .stack
                            .last()
                            .is_some_and(|s| matches!(s.kind, StackKind::Triggered { .. }))
                    {
                        break;
                    }
                    answer(g, automatic(&r, &target(object(g, 1))));
                }
            } else {
                activated(g, 0, vec![]);
            }
            let invocation = g.state.stack.last().unwrap().id;
            assert!(!matches!(
                g.state.stack.last().unwrap().kind,
                StackKind::Spell
            ));
            priority_for(g, 1);
            counter(g, 2, invocation);
            resolve(g, vec![]);
            assert!(g.state.stack.is_empty());
            assert!(token_objects(g).is_empty());
            assert_eq!(
                g.zone(object(g, 1)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            if !trigger {
                assert_eq!(
                    g.zone(object(g, 0)),
                    Some(Zone::Graveyard { owner: PlayerId(0) })
                );
            }
        });
    }
}
#[test]
fn hapless_sacrifice_triggers_artist_and_stifle_counters_only_the_trigger() {
    traces(
        prepared(&[
            ("Hapless Researcher", 0, false),
            ("Blood Artist", 0, false),
            ("Stifle", 1, true),
            ("Island", 1, false),
        ]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(
                g,
                Response::ActivateAbility {
                    object: object(g, 0),
                    ability: 0,
                },
            );
            for _ in 0..50 {
                let r = request(g);
                if matches!(r, Request::Priority { .. }) && g.state.stack.len() == 2 {
                    break;
                }
                answer(g, automatic(&r, &[]));
            }
            let id = g.state.stack.last().unwrap().id;
            assert!(matches!(
                g.state.stack.last().unwrap().kind,
                StackKind::Triggered { .. }
            ));
            check(g);
            priority_for(g, 1);
            counter(g, 2, id);
            resolve(g, vec![]);
            assert_eq!(g.state.players[0].life, 40.into());
            assert_eq!(g.state.players[1].life, 40.into());
            assert_eq!(
                g.zone(object(g, 0)),
                Some(Zone::Graveyard { owner: PlayerId(0) })
            );
            assert_eq!(g.state.libraries[&PlayerId(0)].len(), 2); // turn draw + Researcher draw
        },
    );
}
#[test]
fn mana_abilities_never_supply_stifle_stack_targets() {
    traces(
        prepared(&[
            ("Sol Ring", 0, false),
            ("Stifle", 0, true),
            ("Island", 0, false),
        ]),
        |g| {
            reach(g, TurnStep::PrecombatMain);
            answer(
                g,
                Response::ActivateMana {
                    object: object(g, 0),
                },
            );
            let _ = request(g);
            assert!(g.state.stack.is_empty());
            let r = request(g);
            let before = g.state_digest().unwrap();
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Cast { card: CardId(1) }
                )
                .is_err()
            );
            assert_eq!(g.state_digest().unwrap(), before);
        },
    );
}

#[test]
fn bounce_and_tuck_commanders_offer_both_destinations_and_forget_counters() {
    // CR 903.9b replaces either move; 122.2 resets counters, 611.3b ends
    // battlefield-only anthem application to a card now in hand/library.
    for spell in ["Unsummon", "Time Ebb"] {
        for to_command in [false, true] {
            let mut rows = vec![
                (spell, 0, true),
                ("Isamaru, Hound of Konda", 1, false),
                ("Glorious Anthem", 1, false),
                ("Battlegrowth", 0, true),
                ("Forest", 0, false),
            ];
            rows.extend(lands("Island", 3));
            traces(prepared(&rows), |g| {
                cast(g, 3, target(object(g, 1)));
                assert_eq!(
                    g.derived_characteristics(object(g, 1)).unwrap().power,
                    Some(4.into())
                );
                announce(g, 0, target(object(g, 1)));
                loop {
                    let r = request(g);
                    if matches!(r, Request::CommanderReturn { .. }) {
                        break;
                    }
                    answer(g, automatic(&r, &[]));
                }
                check(g);
                answer(g, Response::CommanderReturn { to_command });
                resolve(g, vec![]);
                let expected = if to_command {
                    Zone::Command { owner: PlayerId(1) }
                } else if spell == "Unsummon" {
                    Zone::Hand { owner: PlayerId(1) }
                } else {
                    Zone::Library { owner: PlayerId(1) }
                };
                assert_eq!(g.zone(object(g, 1)), Some(expected));
                assert_eq!(g.current(object(g, 1)).unwrap().plus_one_counters, 0.into());
                assert_eq!(
                    g.derived_characteristics(object(g, 1)).unwrap().power,
                    Some(2.into())
                );
                if !to_command && spell == "Time Ebb" {
                    // The next draw supplies another 903.9b choice without a
                    // hidden-library object handle in the owner's projection.
                    for _ in 0..100 {
                        let r = request(g);
                        if matches!(r, Request::CommanderReturn { .. }) {
                            assert!(matches!(
                                observation(g, 1).decision,
                                Some(PlayerRequest::CommanderReturn { object: None, .. })
                            ));
                            check(g);
                            answer(g, Response::CommanderReturn { to_command: true });
                            break;
                        }
                        let response = match r {
                            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                            Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
                            _ => automatic(&r, &[]),
                        };
                        answer(g, response);
                    }
                    let _ = request(g);
                    assert_eq!(
                        g.zone(object(g, 1)),
                        Some(Zone::Command { owner: PlayerId(1) })
                    );
                }
            });
        }
    }
}
#[test]
fn bounced_and_tucked_tokens_reach_destination_then_cease() {
    for spell in ["Unsummon", "Time Ebb"] {
        let mut rows = vec![
            ("Goblin Instigator", 0, true),
            (spell, 0, true),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
        ];
        rows.extend(lands("Island", 3));
        traces(prepared(&rows), |g| {
            cast(g, 0, vec![]);
            let token = token_objects(g)[0];
            assert_eq!(
                g.definition(token.card).unwrap().characteristics.subtypes,
                ["Goblin"]
            );
            cast(g, 1, target(token));
            assert!(g.object_id(token.card).is_none());
            assert!(token_objects(g).is_empty());
            // Retained token identity cannot become a playable card in hand.
            assert!(g.state.tokens.contains_key(&token.card));
            assert!(
                !observation(g, 0)
                    .view
                    .objects
                    .iter()
                    .any(|o| o.token.is_some())
            );
            assert!(
                !g.state
                    .libraries
                    .values()
                    .any(|ids| ids.contains(&token.card))
            );
        });
    }
}
#[test]
fn lightning_bolt_complete_any_target_and_furnace_doubling() {
    for furnace in [false, true] {
        for victim in ["player", "Wall of Air", "Caelorna, Coral Tyrant", "token"] {
            let mut rows = vec![
                ("Lightning Bolt", 0, true),
                ("Mountain", 0, false),
                ("Furnace of Rath", 2, false),
                ("Wall of Air", 1, false),
                ("Caelorna, Coral Tyrant", 1, false),
                ("Goblin Instigator", 0, true),
                ("Mountain", 0, false),
                ("Mountain", 0, false),
            ];
            if !furnace {
                rows[2] = ("Mountain", 2, false);
            }
            traces(prepared(&rows), |g| {
                reach(g, TurnStep::PrecombatMain);
                let target_entity = match victim {
                    "player" => TargetEntity::Player {
                        player: PlayerId(1),
                    },
                    "Wall of Air" => TargetEntity::Object {
                        object: object(g, 3),
                    },
                    "Caelorna, Coral Tyrant" => TargetEntity::Object {
                        object: object(g, 4),
                    },
                    _ => {
                        cast(g, 5, vec![]);
                        TargetEntity::Object {
                            object: token_objects(g)[0],
                        }
                    }
                };
                cast(g, 0, vec![target_entity]);
                let amount = if furnace { 6 } else { 3 };
                match target_entity {
                    TargetEntity::Player { player } => assert_eq!(
                        g.state.players[player.0 as usize].life,
                        Quantity::from(40 - amount)
                    ),
                    TargetEntity::Object { object: o } => {
                        if victim == "token" {
                            assert!(g.object_id(o.card).is_none());
                        } else if victim == "Wall of Air" && furnace {
                            assert!(matches!(g.zone(object(g, 3)), Some(Zone::Graveyard { .. })));
                        } else {
                            assert_eq!(g.current(o).unwrap().marked_damage, amount.into());
                        }
                    }
                }
                // Noncombat damage never adds physical Commander history.
                assert!(
                    g.state
                        .commanders
                        .history(CardId(4))
                        .unwrap()
                        .combat_damage
                        .is_empty()
                );
            });
        }
    }
}
fn next_own_turn(g: &mut Game) {
    let turn = g.turn().number;
    for _ in 0..400 {
        let r = request(g);
        if g.turn().number > turn
            && g.state.turn_owner == PlayerId(0)
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
    panic!("next own turn not reached");
}
#[test]
fn sorcerer_sickness_then_token_ping_and_non_deathtouch_damage() {
    let mut rows = vec![
        ("Prodigal Sorcerer", 0, true),
        ("Goblin Instigator", 0, true),
        ("Mountain", 0, false),
        ("Mountain", 0, false),
        ("Caelorna, Coral Tyrant", 1, false),
    ];
    rows.extend(lands("Island", 3));
    traces(prepared(&rows), |g| {
        cast(g, 1, vec![]);
        let token = token_objects(g)[0];
        cast(g, 0, vec![]);
        let r = request(g);
        assert!(
            g.respond(
                r.player(),
                r.decision_id(),
                Response::ActivateAbility {
                    object: object(g, 0),
                    ability: 0
                }
            )
            .is_err()
        );
        next_own_turn(g);
        activated(g, 0, target(token));
        resolve(g, vec![]);
        assert!(g.object_id(token.card).is_none());
        next_own_turn(g);
        activated(g, 0, target(object(g, 4)));
        resolve(g, vec![]);
        assert_eq!(g.current(object(g, 4)).unwrap().marked_damage, 1.into());
        assert!(!g.current(object(g, 4)).unwrap().deathtouch_damage);
    });
}
#[test]
fn looter_and_azure_draw_abilities_use_the_match_stack() {
    for name in ["Merfolk Looter", "Azure Mage"] {
        let mut rows = vec![(name, 0, false)];
        rows.extend(lands("Island", 4));
        traces(prepared(&rows), |g| {
            reach(g, TurnStep::PrecombatMain);
            let hand = g
                .state
                .objects
                .values()
                .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                .count();
            activated(g, 0, vec![]);
            assert_eq!(g.state.stack.len(), 1);
            check(g);
            resolve(g, vec![]);
            assert_eq!(
                g.state
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
                    .count(),
                hand + usize::from(name == "Azure Mage")
            );
            assert_eq!(g.state.libraries[&PlayerId(0)].len(), 2);
        });
    }
}

fn declare(g: &mut Game, attacks: &[(u32, u8)], blocks: &[(u32, u32)]) {
    attackers(g);
    answer(
        g,
        Response::Attackers {
            attacks: attacks
                .iter()
                .map(|(o, p)| (object(g, *o), PlayerId(*p)))
                .collect(),
        },
    );
    loop {
        if matches!(request(g), Request::Blockers { .. }) {
            break;
        }
        answer(g, Response::Pass);
    }
    answer(
        g,
        Response::Blockers {
            blocks: blocks
                .iter()
                .map(|(b, a)| (object(g, *b), object(g, *a)))
                .collect(),
        },
    );
}
fn damage_request(g: &mut Game) -> Request {
    loop {
        let r = request(g);
        if matches!(r, Request::Damage { .. }) {
            return r;
        }
        assert!(matches!(r, Request::Priority { .. }), "{r:?}");
        answer(g, Response::Pass);
    }
}

#[test]
fn furnace_first_double_strike_lifelink_and_commander_final_damage() {
    for (name, first, life, damage) in [
        ("Knight of Meadowgrain", true, 44, 4),
        ("Fencing Ace", true, 40, 4),
        ("Rorix Bladewing", false, 40, 12),
    ] {
        traces(
            prepared(&[(name, 0, false), ("Furnace of Rath", 2, false)]),
            |g| {
                declare(g, &[(0, 1)], &[]);
                // Pass combat-step priorities, explicitly choosing damage effects.
                for _ in 0..100 {
                    let r = request(g);
                    if g.turn().step == TurnStep::EndCombat {
                        break;
                    }
                    if matches!(r, Request::DamageEffect { .. }) {
                        check(g);
                    }
                    answer(g, automatic(&r, &[]));
                }
                assert_eq!(!g.state.first_strike_participants.is_empty(), first);
                assert_eq!(g.state.players[0].life, Quantity::from(life));
                assert_eq!(g.state.players[1].life, Quantity::from(40 - damage));
                if name == "Rorix Bladewing" {
                    assert_eq!(
                        g.state.commanders.history(CardId(0)).unwrap().combat_damage[&PlayerId(1)],
                        12.into()
                    );
                }
            },
        );
    }
}
#[test]
fn furnace_trample_assigns_lethal_before_doubling_and_deathtouch_still_kills() {
    for name in ["Colossal Dreadmaw", "Deadly Recluse"] {
        traces(
            prepared(&[
                (name, 0, false),
                ("Yoked Ox", 1, false),
                ("Furnace of Rath", 2, false),
            ]),
            |g| {
                declare(g, &[(0, 1)], &[(1, 0)]);
                if name == "Colossal Dreadmaw" {
                    let r = damage_request(g);
                    assert!(
                        matches!(r,Request::Damage{ref lethal_requirements,..} if lethal_requirements==&vec![(object(g,1),4.into())])
                    );
                    answer(
                        g,
                        Response::TrampleDamage {
                            assignments: vec![(object(g, 1), 4.into())],
                            player: PlayerId(1),
                            amount: 2.into(),
                        },
                    );
                }
                for _ in 0..100 {
                    let r = request(g);
                    if g.turn().step == TurnStep::EndCombat {
                        break;
                    }
                    answer(g, automatic(&r, &[]));
                }
                assert!(matches!(g.zone(object(g, 1)), Some(Zone::Graveyard { .. })));
                assert_eq!(
                    g.state.players[1].life,
                    Quantity::from(if name == "Colossal Dreadmaw" { 36 } else { 40 })
                );
                assert_eq!(g.current(object(g, 0)).unwrap().marked_damage, 0.into());
            },
        );
    }
}
