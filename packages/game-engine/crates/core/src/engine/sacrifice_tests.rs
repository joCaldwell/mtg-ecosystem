//! Every gameplay witness begins with admit_pod and seeded legal 100-card decks.
//! Host-only seed selection arranges no cards; all draws, land plays, casts,
//! costs, choices and concessions are recorded accepted inputs. No injected state.
use super::*;
use rand_core::{Rng, SeedableRng};
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-sacrifice-ir.json"
    ))
    .unwrap()
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
fn named(g: &Game, name: &str, seat: u8) -> ObjectId {
    g.state
        .objects
        .values()
        .find(|o| {
            g.state.cards[&o.id.card].owner == PlayerId(seat)
                && g.definition(o.id.card).unwrap().name == name
        })
        .unwrap_or_else(|| panic!("{name} seat {seat}"))
        .id
}
fn auto(g: &Game, r: &Request) -> Response {
    match r {
        Request::Mulligan { .. } => Response::Mulligan { take: false },
        Request::BottomOpening {
            count, candidates, ..
        } => Response::BottomOpening {
            objects: candidates[..*count].to_vec(),
        },
        Request::Priority { .. } => Response::Pass,
        Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
        Request::Blockers { .. } => Response::Blockers { blocks: vec![] },
        Request::Discard {
            count, candidates, ..
        } => Response::Discard {
            objects: candidates
                .iter()
                .copied()
                .filter(|o| g.definition(o.card).unwrap().characteristics.basic)
                .take(*count)
                .collect(),
        },
        Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
            order: candidates.clone(),
        },
        Request::PlayerTargets { candidates, .. } => Response::PlayerTargets {
            players: vec![
                *candidates
                    .iter()
                    .find(|p| **p != r.player())
                    .unwrap_or(&candidates[0]),
            ],
        },
        Request::EntityTargets { candidates, .. } => Response::EntityTargets {
            entities: vec![
                *candidates
                    .iter()
                    .find(|t| matches!(t,TargetEntity::Player {player} if *player!=r.player()))
                    .unwrap(),
            ],
        },
        Request::CommanderReturn { .. } => Response::CommanderReturn { to_command: true },
        Request::OptionalEffect { .. } => Response::OptionalEffect { perform: true },
        Request::OptionalRecursion { .. } => Response::OptionalRecursion { perform: false },
        Request::ForcedSacrifice { candidates, .. } => Response::ForcedSacrifice {
            object: candidates[0],
        },
        Request::SacrificeCost { candidates, .. } => Response::SacrificeCost {
            object: candidates[0],
        },
        Request::SearchLands {
            maximum,
            candidates,
            ..
        } => Response::SearchLands {
            objects: candidates.iter().copied().take(*maximum as usize).collect(),
        },
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates[0].effect.clone(),
            damage: candidates[0].damage,
        },
        x => panic!("unhandled {x:?}"),
    }
}
fn main_for(g: &mut Game, seat: u8) {
    for _ in 0..1000 {
        let r = request(g);
        if matches!(r, Request::Priority { .. })
            && r.player() == PlayerId(seat)
            && g.state.active == Some(PlayerId(seat))
            && g.turn().step == TurnStep::PrecombatMain
        {
            return;
        }
        let a = auto(g, &r);
        answer(g, a);
    }
    panic!("main cap")
}
fn priority(g: &mut Game, seat: u8) {
    for _ in 0..100 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && r.player() == PlayerId(seat) {
            return;
        }
        let a = auto(g, &r);
        answer(g, a);
    }
    panic!("priority cap")
}
fn settle(g: &mut Game) {
    for _ in 0..2000 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && g.state.stack.is_empty() {
            return;
        }
        let a = auto(g, &r);
        answer(g, a);
    }
    panic!("settle cap")
}
fn pay(g: &mut Game, target: Option<TargetEntity>, sacrifice: Option<ObjectId>) {
    for _ in 0..100 {
        let r = request(g);
        if matches!(
            r,
            Request::Priority { .. }
                | Request::OrderTriggers { .. }
                | Request::CommanderReturn { .. }
        ) {
            return;
        }
        let a = match &r {
            Request::Targets { .. } => Response::Targets {
                objects: vec![match target.unwrap() {
                    TargetEntity::Object { object } => object,
                    _ => panic!("object target"),
                }],
            },
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: vec![target.unwrap()],
            },
            Request::StackTargets { candidates, .. } => Response::StackTargets {
                entries: vec![candidates[0]],
            },
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let o = g
                        .state
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && g.validate_mana_activation(r.player(), o.id).is_ok()
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                                && cost.colored.iter().any(|c| {
                                    g.mana_abilities(o.id).unwrap().contains(c)
                                        && g.state.players[r.player().0 as usize]
                                            .mana
                                            .get(c)
                                            .cloned()
                                            .unwrap_or_default()
                                            < Quantity::from(
                                                cost.colored.iter().filter(|x| *x == c).count()
                                                    as i64,
                                            )
                                })
                        })
                        .or_else(|| {
                            g.state.objects.values().find(|o| {
                                o.controller == r.player()
                                    && g.validate_mana_activation(r.player(), o.id).is_ok()
                                    && g.definition(o.id.card).unwrap().characteristics.kind
                                        == CardKind::Land
                            })
                        })
                        .expect("available land");
                    Response::ActivateMana { object: o.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut free = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    free.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut rest = cost.generic.clone();
                for (c, n) in free {
                    let used = std::cmp::min(rest.clone(), n);
                    if used > Quantity::zero() {
                        rest.subtract(&used);
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(rest, 0.into());
                Response::SpendMana { spending }
            }
            Request::SacrificeCost { candidates, .. } => {
                if g.definition(g.top().unwrap().source.card).unwrap().name == "Village Rites" {
                    check(g);
                    let o = observation(g, r.player().0);
                    let d = o.decision.unwrap();
                    let PlayerRequest::SacrificeCost {
                        candidates: handles,
                        ..
                    } = &d
                    else {
                        panic!("cost projection");
                    };
                    for other in 0..4 {
                        if other != r.player().0 {
                            assert!(observation(g, other).decision.is_none());
                        }
                    }
                    let before = g.state_digest().unwrap();
                    let q = g.check_player_response(
                        r.player(),
                        d.token().clone(),
                        PlayerResponse::SacrificeCost {
                            object: handles[0].clone(),
                        },
                        Budget::new(0),
                    );
                    let QueryResult::Incomplete { work, .. } = q else {
                        panic!("cost work suspended");
                    };
                    let saved = g.save_work(&work).unwrap();
                    let restored =
                        Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
                    let work = restored.restore_work(&saved).unwrap();
                    assert!(matches!(
                        restored.resume_work(work, Budget::unlimited()),
                        QueryResult::Complete(QueryValue::Validated(_))
                    ));
                    assert_eq!(before, g.state_digest().unwrap());
                }
                if g.definition(g.top().unwrap().source.card).unwrap().name
                    == "Siege-Gang Commander"
                {
                    assert!(candidates.iter().all(|o| {
                        g.definition(o.card)
                            .unwrap()
                            .characteristics
                            .subtypes
                            .contains(&"Goblin".into())
                    }));
                }
                Response::SacrificeCost {
                    object: sacrifice.unwrap_or(candidates[0]),
                }
            }
            _ => auto(g, &r),
        };
        answer(g, a);
    }
    panic!("pay cap")
}
fn announce(
    g: &mut Game,
    name: &str,
    seat: u8,
    target: Option<TargetEntity>,
    sac: Option<ObjectId>,
) {
    priority(g, seat);
    let card = named(g, name, seat).card;
    answer(g, Response::Cast { card });
    pay(g, target, sac);
}
fn cast(g: &mut Game, name: &str, seat: u8, target: Option<TargetEntity>, sac: Option<ObjectId>) {
    main_for(g, seat);
    // Only issue a payable proposal. Earlier setup casts may have used this
    // turn's lands; advancing through the next normal untap is accepted play.
    loop {
        let o = named(g, name, seat);
        let cost = g
            .casting_cost(o.card, PlayerId(seat), g.current(o).unwrap().zone)
            .unwrap();
        let mut funds = g.state.players[seat as usize].mana.clone();
        for land in
            g.state.objects.values().filter(|o| {
                o.controller == PlayerId(seat) && !o.tapped && o.zone == Zone::Battlefield
            })
        {
            if let Ok(colors) = g.mana_abilities(land.id)
                && colors.len() == 1
            {
                funds.entry(colors[0]).or_default().add(&1.into());
            }
        }
        let mut payable = true;
        for c in &cost.colored {
            let n = funds.entry(*c).or_default();
            if *n < 1.into() {
                payable = false;
                break;
            }
            n.subtract(&1.into());
        }
        let mut total = Quantity::zero();
        for n in funds.values() {
            total.add(n);
        }
        if payable && total >= cost.generic {
            break;
        }
        answer(g, Response::Pass);
        main_for(g, seat);
    }
    announce(g, name, seat, target, sac);
    settle(g);
}
fn activate(
    g: &mut Game,
    name: &str,
    seat: u8,
    target: Option<TargetEntity>,
    sac: Option<ObjectId>,
) {
    priority(g, seat);
    let object = named(g, name, seat);
    answer(g, Response::ActivateAbility { object, ability: 0 });
    pay(g, target, sac);
}
fn obj(o: ObjectId) -> Option<TargetEntity> {
    Some(TargetEntity::Object { object: o })
}
fn player(p: u8) -> Option<TargetEntity> {
    Some(TargetEntity::Player {
        player: PlayerId(p),
    })
}
fn tokens(g: &Game, seat: u8, subtype: &str) -> Vec<ObjectId> {
    g.state
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == PlayerId(seat)
                && g.state.tokens.contains_key(&o.id.card)
                && g.definition(o.id.card)
                    .unwrap()
                    .characteristics
                    .subtypes
                    .contains(&subtype.into())
        })
        .map(|o| o.id)
        .collect()
}
fn check(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state, g.state);
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(replay.state, g.state);
    assert_eq!(replay.trace(), g.trace());
}
fn natural(label: &str, rows: &[(&str, u8)], colors: [u8; 4], run: impl Fn(&mut Game)) {
    let c = catalog();
    let id = |n: &str| {
        c.definitions
            .iter()
            .find(|d| d.name == n)
            .unwrap()
            .oracle_id
            .clone()
    };
    let commanders = [
        ISAMARU,
        YARGLE,
        "4bbc5df9-60d7-46c1-8856-91c9c74abf18",
        "48fe0f52-1676-49d5-acf6-5476081e8e1b",
    ];
    let decks = std::array::from_fn(|seat| {
        let color = colors[seat] as usize;
        let commander = commanders[color].to_string();
        let mut cards = vec![DeckEntry {
            oracle_id: commander.clone(),
            count: 1,
        }];
        for (n, p) in rows {
            if *p as usize == seat && id(n) != commander {
                cards.push(DeckEntry {
                    oracle_id: id(n),
                    count: 1,
                });
            }
        }
        let remaining = 100 - cards.len() as u32;
        let basics = match color {
            0 => vec![(PLAINS, remaining)],
            1 => vec![(FOREST, remaining / 2), (SWAMP, remaining - remaining / 2)],
            2 => vec![(ISLAND, remaining)],
            _ => vec![("a3fb7228-e76b-4e96-a40e-20b5fed75685", remaining)],
        };
        cards.extend(basics.into_iter().map(|(n, count)| DeckEntry {
            oracle_id: n.into(),
            count,
        }));
        Decklist { commander, cards }
    });
    let mut config = MatchConfig {
        profile: "commander-sacrifice-v1".into(),
        decks,
        starting_player: PlayerId(0),
        seed: [0; 32],
    };
    let initial = Prepared::admit_pod(c.clone(), config.clone()).unwrap();
    let wanted = rows
        .iter()
        .filter_map(|(n, p)| {
            initial
                .config
                .cards
                .iter()
                .find(|v| v.owner == PlayerId(*p) && v.oracle_id == id(n))
                .map(|v| v.id)
        })
        .filter(|v| !initial.config.commanders.contains(v))
        .collect::<BTreeSet<_>>();
    // Trusted evidence adapter selects a host seed using the exact documented
    // Fisher–Yates algorithm, then the real setup independently executes it.
    let seed = (0u32..200_000)
        .find_map(|n| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&n.to_le_bytes());
            let mut rng = rand_chacha::ChaCha20Rng::from_seed(seed);
            let mut good = true;
            for (_, mut library) in initial.config.initial_libraries() {
                for i in (1..library.len()).rev() {
                    loop {
                        let w = rng.next_u32();
                        if let Some(j) = setup::sample_shuffle_word(w, (i + 1) as u32) {
                            library.swap(i, j as usize);
                            break;
                        }
                    }
                }
                if library.iter().skip(20).any(|id| wanted.contains(id)) {
                    good = false;
                }
            }
            good.then_some(seed)
        })
        .expect("reachable witness seed");
    config.seed = seed;
    eprintln!(
        "{label}: seed {}",
        u32::from_le_bytes(seed[..4].try_into().unwrap())
    );
    let p = Prepared::admit_pod(c, config).unwrap();
    let mut base = Game::with_trace(p.clone(), TraceMode::Off);
    for _ in 0..20000 {
        let r = request(&mut base);
        let ready = wanted.iter().all(|id| {
            base.state.objects[id].zone
                == Zone::Hand {
                    owner: base.state.cards[id].owner,
                }
        });
        let mana_ready = (0..4).all(|seat| {
            colors[seat] != 1
                || [FOREST, SWAMP].iter().all(|id| {
                    base.state
                        .objects
                        .values()
                        .filter(|o| {
                            o.zone == Zone::Battlefield
                                && o.controller == PlayerId(seat as u8)
                                && base.state.cards[&o.id.card].oracle_id == *id
                        })
                        .count()
                        >= 2
                })
        });
        if ready
            && mana_ready
            && base.turn().number >= 33
            && base.state.active == Some(PlayerId(0))
            && base.turn().step == TurnStep::PrecombatMain
            && matches!(r, Request::Priority { .. })
        {
            break;
        }
        let response = if matches!(r, Request::Priority { .. })
            && base.state.active == Some(r.player())
            && base.turn().step == TurnStep::PrecombatMain
            && base.state.turn.lands_played == 0
        {
            base.state
                .objects
                .values()
                .find(|o| {
                    o.zone == Zone::Hand { owner: r.player() }
                        && base.definition(o.id.card).unwrap().characteristics.basic
                })
                .map(|o| Response::PlayLand { card: o.id.card })
                .unwrap_or(Response::Pass)
        } else {
            auto(&base, &r)
        };
        answer(&mut base, response);
    }
    assert!(
        wanted
            .iter()
            .all(|id| matches!(base.state.objects[id].zone, Zone::Hand { .. })),
        "witness cards drawn"
    );
    let journal = base.replay_journal();
    let boundary = base.rule_boundary();
    let mut digest0 = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let mut g = Game::replay_to(p.clone(), &journal, Some(boundary), mode).unwrap();
        run(&mut g);
        check(&g);
        let d = g.state_digest().unwrap();
        if let Some(old) = &digest0 {
            assert_eq!(&d, old);
        } else {
            digest0 = Some(d);
        }
    }
}
#[test]
fn sacrifice_profile_catalog_decks_policy_and_bounds() {
    let c = catalog();
    assert_eq!(
        digest(&c).unwrap(),
        match_profile("commander-sacrifice-v1")
            .unwrap()
            .catalog_sha256
            .as_str()
    );
    for (colors, expected) in [
        (vec![Color::W], 17),
        (vec![Color::B, Color::G], 42),
        (vec![Color::U], 11),
        (vec![Color::R], 12),
    ] {
        assert_eq!(
            c.definitions
                .iter()
                .filter(|d| d.characteristics.kind == CardKind::Creature
                    && d.characteristics
                        .color_identity
                        .iter()
                        .all(|k| colors.contains(k)))
                .count(),
            expected
        );
    }
    for d in &c.definitions {
        for a in &d.mana_abilities {
            assert!(match &a.production {
                ManaProduction::Fixed { symbols } => symbols.len() <= 2,
                _ => true,
            });
            assert!(!a.costs.sacrifice);
        }
        for p in d
            .spell
            .iter()
            .chain(d.activated.iter().map(|a| &a.program))
            .chain(d.triggered.iter().map(|a| &a.program))
        {
            for i in &p.instructions {
                if let Instruction::CreateCreatureTokens { count, .. }
                | Instruction::CreateTokensFor { count, .. } = i
                {
                    assert!(*count <= 4);
                }
            }
        }
    }
    let angel = c
        .definitions
        .iter()
        .find(|d| d.name == "Requiem Angel")
        .unwrap();
    let f = angel.triggered[0].death_filter.as_ref().unwrap();
    assert!(f.controlled && f.other);
    assert_eq!(f.non_subtypes, ["Spirit"]);
    assert!(
        matches!(&angel.triggered[0].program.instructions[0],Instruction::CreateCreatureTokens {count:1,token,..} if token.subtypes==["Spirit"])
    );
    const {
        assert!(4898 < sacrifice::LIVE_TOKEN_CAPACITY);
        assert!(6468 + 32 < sacrifice::EFFECT_BOUND);
    };
    let p = Prepared::admit_pod(
        c,
        MatchConfig::for_profile("commander-sacrifice-v1", [7; 32], PlayerId(0)).unwrap(),
    )
    .unwrap();
    let g = Game::new(p);
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
}
#[test]
fn sacrifice_rites_cost_token_two_draws_two_payoffs_and_uncastable_without_creature() {
    natural(
        "rites",
        &[
            ("Village Rites", 1),
            ("Zulaport Cutthroat", 1),
            ("Bastion of Remembrance", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            main_for(g, 1);
            let before = g.state_digest().unwrap();
            let r = request(g);
            assert!(
                g.respond(
                    r.player(),
                    r.decision_id(),
                    Response::Cast {
                        card: named(g, "Village Rites", 1).card
                    }
                )
                .is_err()
            );
            assert_eq!(before, g.state_digest().unwrap());
            cast(g, "Zulaport Cutthroat", 1, None, None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            let token = tokens(g, 1, "Soldier")[0];
            let life = g
                .state
                .players
                .iter()
                .map(|p| p.life.clone())
                .collect::<Vec<_>>();
            let library = g.state.libraries[&PlayerId(1)].len();
            main_for(g, 1);
            announce(g, "Village Rites", 1, None, Some(token));
            assert!(g.current(token).is_err() || g.zone(token) != Some(Zone::Battlefield));
            settle(g);
            assert_eq!(g.state.libraries[&PlayerId(1)].len(), library - 2);
            for (p, l) in life.iter().enumerate() {
                let mut e = l.clone();
                if p == 1 {
                    e.add(&2.into());
                } else {
                    e.subtract(&2.into());
                }
                assert_eq!(g.state.players[p].life, e);
            }
        },
    );
}
#[test]
fn sacrifice_feeder_self_no_counter_cannot_block_and_bloodthrone_cleanup() {
    natural(
        "outlets",
        &[
            ("Carrion Feeder", 1),
            ("Bloodthrone Vampire", 1),
            ("Bastion of Remembrance", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Carrion Feeder", 1, None, None);
            let feeder = named(g, "Carrion Feeder", 1);
            assert!(!g.creatures(PlayerId(1), false).contains(&feeder));
            activate(g, "Carrion Feeder", 1, None, Some(feeder));
            settle(g);
            assert!(matches!(
                g.zone(named(g, "Carrion Feeder", 1)),
                Some(Zone::Graveyard { .. })
            ));
            cast(g, "Bloodthrone Vampire", 1, None, None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            // Wait through a normal untap so Bloodthrone may attack this turn.
            answer(g, Response::Pass);
            main_for(g, 1);
            let token = tokens(g, 1, "Soldier")[0];
            activate(g, "Bloodthrone Vampire", 1, None, Some(token));
            settle(g);
            let vampire = named(g, "Bloodthrone Vampire", 1);
            assert_eq!(g.characteristics(vampire).unwrap().power, Some(3.into()));
            let before = g.state.players[0].life.clone();
            loop {
                let r = request(g);
                if matches!(r, Request::Attackers { .. }) && r.player() == PlayerId(1) {
                    answer(
                        g,
                        Response::Attackers {
                            attacks: vec![(vampire, PlayerId(0))],
                        },
                    );
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            loop {
                let r = request(g);
                if g.turn().step == TurnStep::PostcombatMain
                    && matches!(r, Request::Priority { .. })
                {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            let mut expected = before;
            expected.subtract(&3.into());
            assert_eq!(g.state.players[0].life, expected);
            main_for(g, 2);
            assert_eq!(g.characteristics(vampire).unwrap().power, Some(1.into()));
        },
    );
}
#[test]
fn sacrifice_costly_plunder_ring_and_bone_splinters_illegal_target_keeps_cost() {
    natural(
        "plunder",
        &[("Costly Plunder", 1), ("Sol Ring", 1)],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Sol Ring", 1, None, None);
            let ring = named(g, "Sol Ring", 1);
            let n = g.state.libraries[&PlayerId(1)].len();
            cast(g, "Costly Plunder", 1, None, Some(ring));
            assert!(matches!(
                g.zone(named(g, "Sol Ring", 1)),
                Some(Zone::Graveyard { .. })
            ));
            assert_eq!(g.state.libraries[&PlayerId(1)].len(), n - 2);
        },
    );
    natural(
        "splinters",
        &[
            ("Bone Splinters", 1),
            ("Carrion Feeder", 1),
            ("Typhoid Rats", 2),
            ("Murder", 2),
        ],
        [0, 1, 1, 3],
        |g| {
            cast(g, "Carrion Feeder", 1, None, None);
            cast(g, "Typhoid Rats", 2, None, None);
            main_for(g, 1);
            let own = named(g, "Carrion Feeder", 1);
            let enemy = named(g, "Typhoid Rats", 2);
            announce(g, "Bone Splinters", 1, obj(enemy), Some(own));
            announce(g, "Murder", 2, obj(enemy), None);
            settle(g);
            assert!(matches!(
                g.zone(named(g, "Carrion Feeder", 1)),
                Some(Zone::Graveyard { .. })
            ));
            assert!(matches!(
                g.zone(named(g, "Typhoid Rats", 2)),
                Some(Zone::Graveyard { .. })
            ));
        },
    );
}
#[test]
fn sacrifice_fling_frozen_equipment_power_furnace_and_paid_cost_snapshot() {
    natural(
        "fling",
        &[
            ("Fling", 3),
            ("Raging Goblin", 3),
            ("Bonesplitter", 3),
            ("Furnace of Rath", 3),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Raging Goblin", 3, None, None);
            cast(g, "Bonesplitter", 3, None, None);
            main_for(g, 3);
            let goblin = named(g, "Raging Goblin", 3);
            activate(g, "Bonesplitter", 3, obj(goblin), None);
            settle(g);
            cast(g, "Furnace of Rath", 3, None, None);
            assert_eq!(g.characteristics(goblin).unwrap().power, Some(3.into()));
            let life = g.state.players[0].life.clone();
            announce(g, "Fling", 3, player(0), Some(goblin));
            check(g);
            assert_eq!(
                g.top()
                    .unwrap()
                    .sacrificed
                    .as_ref()
                    .unwrap()
                    .characteristics
                    .power,
                Some(3.into())
            );
            settle(g);
            let mut expected = life;
            expected.subtract(&6.into());
            assert_eq!(g.state.players[0].life, expected);
            assert!(
                g.state
                    .commanders
                    .records
                    .values()
                    .all(|r| r.combat_damage.is_empty())
            );
        },
    );
}
fn settle_forced(g: &mut Game, seat: u8, victim: ObjectId) {
    for _ in 0..2000 {
        let r = request(g);
        if matches!(r, Request::Priority { .. }) && g.state.stack.is_empty() {
            return;
        }
        let a = if matches!(r, Request::ForcedSacrifice { .. }) && r.player() == PlayerId(seat) {
            Response::ForcedSacrifice { object: victim }
        } else {
            auto(g, &r)
        };
        answer(g, a);
    }
    panic!("forced settle cap")
}
#[test]
fn sacrifice_requiem_nonspirit_card_token_and_spirit_filter() {
    natural(
        "requiem",
        &[
            ("Requiem Angel", 0),
            ("Seller of Songbirds", 0),
            ("Butcher of Malakir", 1),
            ("Carrion Feeder", 1),
            ("Doomed Dissenter", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Requiem Angel", 0, None, None);
            cast(g, "Seller of Songbirds", 0, None, None);
            cast(g, "Carrion Feeder", 1, None, None);
            cast(g, "Butcher of Malakir", 1, None, None);
            cast(g, "Doomed Dissenter", 1, None, None);
            let dissenter = named(g, "Doomed Dissenter", 1);
            activate(g, "Carrion Feeder", 1, None, Some(dissenter));
            let seller = named(g, "Seller of Songbirds", 0);
            settle_forced(g, 0, seller);
            assert_eq!(tokens(g, 0, "Spirit").len(), 1);
            assert_eq!(tokens(g, 1, "Zombie").len(), 1);
            let zombie = tokens(g, 1, "Zombie")[0];
            let bird = tokens(g, 0, "Bird")[0];
            activate(g, "Carrion Feeder", 1, None, Some(zombie));
            settle_forced(g, 0, bird);
            assert_eq!(tokens(g, 0, "Spirit").len(), 2);
            let spirit = tokens(g, 0, "Spirit")[0];
            let feeder = named(g, "Carrion Feeder", 1);
            activate(g, "Carrion Feeder", 1, None, Some(feeder));
            settle_forced(g, 0, spirit);
            assert_eq!(tokens(g, 0, "Spirit").len(), 1);
        },
    );
}
fn observation(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("observation");
    };
    o
}
#[test]
fn sacrifice_butcher_apnap_public_previous_choices_simultaneous_and_finite_cascade() {
    natural(
        "butchers",
        &[
            ("Butcher of Malakir", 0),
            ("Carrion Feeder", 0),
            ("Butcher of Malakir", 1),
            ("Typhoid Rats", 2),
        ],
        [1, 1, 1, 1],
        |g| {
            cast(g, "Butcher of Malakir", 0, None, None);
            cast(g, "Carrion Feeder", 0, None, None);
            cast(g, "Butcher of Malakir", 1, None, None);
            cast(g, "Typhoid Rats", 2, None, None);
            main_for(g, 0);
            let feeder = named(g, "Carrion Feeder", 0);
            activate(g, "Carrion Feeder", 0, None, Some(feeder));
            loop {
                let r = request(g);
                if matches!(r, Request::ForcedSacrifice { .. }) {
                    assert_eq!(r.player(), PlayerId(1));
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            check(g);
            let b1 = named(g, "Butcher of Malakir", 1);
            answer(g, Response::ForcedSacrifice { object: b1 });
            let r = request(g);
            assert!(
                matches!(r,Request::ForcedSacrifice {ref previous_choices,..} if previous_choices==&vec![(PlayerId(1),b1)])
            );
            assert_eq!(r.player(), PlayerId(2));
            assert_eq!(g.zone(b1), Some(Zone::Battlefield));
            for seat in 0..4 {
                let o = observation(g, seat);
                assert_eq!(o.view.sacrifice_choices.len(), 1);
                assert_eq!(o.decision.is_some(), seat == 2);
            }
            check(g);
            answer(
                g,
                Response::ForcedSacrifice {
                    object: named(g, "Typhoid Rats", 2),
                },
            );
            settle(g);
            assert!(matches!(
                g.zone(named(g, "Butcher of Malakir", 0)),
                Some(Zone::Graveyard { .. })
            ));
            assert!(matches!(
                g.zone(named(g, "Butcher of Malakir", 1)),
                Some(Zone::Graveyard { .. })
            ));
            if g.trace.mode == TraceMode::Detailed {
                let simultaneous=g.trace().records.iter().filter_map(|r|if let EvidenceRecord::Batch {batch}=&r.record{Some(batch)}else{None}).find(|b|matches!(&b.group,EventGroup::Simultaneous {occurrences} if occurrences.len()==2 && occurrences.iter().all(|e|matches!(e,ProposedEvent::Sacrifice {..}))));
                assert!(simultaneous.is_some());
            }
        },
    );
}
#[test]
fn sacrifice_midnight_nontoken_own_death_and_conceded_opponent_payoffs() {
    natural(
        "midnight",
        &[
            ("Midnight Reaper", 1),
            ("Zulaport Cutthroat", 1),
            ("Bastion of Remembrance", 1),
            ("Vindictive Vampire", 1),
            ("Carrion Feeder", 1),
            ("Furnace of Rath", 3),
        ],
        [0, 1, 2, 3],
        |g| {
            for n in [
                "Midnight Reaper",
                "Zulaport Cutthroat",
                "Bastion of Remembrance",
                "Vindictive Vampire",
                "Carrion Feeder",
            ] {
                cast(g, n, 1, None, None);
            }
            cast(g, "Furnace of Rath", 3, None, None);
            g.concede(PlayerId(2)).unwrap();
            priority(g, 1);
            let before = g
                .state
                .players
                .iter()
                .map(|p| p.life.clone())
                .collect::<Vec<_>>();
            let lib = g.state.libraries[&PlayerId(1)].len();
            let token = tokens(g, 1, "Soldier")[0];
            activate(g, "Carrion Feeder", 1, None, Some(token));
            settle(g);
            assert_eq!(g.state.libraries[&PlayerId(1)].len(), lib);
            let mut expected = before[0].clone();
            expected.subtract(&4.into());
            assert_eq!(g.state.players[0].life, expected);
            assert_eq!(g.state.players[2].life, before[2]);
            let ownlife = g.state.players[1].life.clone();
            let reaper = named(g, "Midnight Reaper", 1);
            activate(g, "Carrion Feeder", 1, None, Some(reaper));
            settle(g);
            assert_eq!(g.state.libraries[&PlayerId(1)].len(), lib - 1);
            let mut expected = ownlife;
            expected.add(&1.into());
            assert_eq!(g.state.players[1].life, expected); // three gains minus doubled Reaper damage
        },
    );
}
#[test]
fn sacrifice_day_simultaneous_deaths_artist_zulaport_bastion_butcher_ordering() {
    natural(
        "day",
        &[
            ("Day of Judgment", 0),
            ("Blood Artist", 1),
            ("Zulaport Cutthroat", 1),
            ("Bastion of Remembrance", 1),
            ("Butcher of Malakir", 1),
            ("Doomed Dissenter", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            for n in [
                "Blood Artist",
                "Zulaport Cutthroat",
                "Bastion of Remembrance",
                "Butcher of Malakir",
                "Doomed Dissenter",
            ] {
                cast(g, n, 1, None, None);
            }
            cast(g, "Day of Judgment", 0, None, None);
            if g.trace.mode == TraceMode::Detailed {
                for name in ["Blood Artist", "Zulaport Cutthroat", "Butcher of Malakir"] {
                    let source = named(g, name, 1).card;
                    let n = g
                        .events()
                        .iter()
                        .filter(
                            |e| matches!(e,Event::TriggerCollected {source:s,..} if s.card==source),
                        )
                        .count();
                    assert_eq!(n, 5, "{name}");
                }
                assert!(
                    g.events()
                        .iter()
                        .any(|e| matches!(e, Event::TriggersOrdered { .. }))
                );
            }
            assert_eq!(tokens(g, 1, "Zombie").len(), 1);
        },
    );
}
#[test]
fn sacrifice_elder_response_to_murder_search_and_stifle_keep_paid_source_gone() {
    for stifle in [false, true] {
        natural(
            if stifle {
                "elder-stifle"
            } else {
                "elder-search"
            },
            &[("Sakura-Tribe Elder", 1), ("Murder", 3), ("Stifle", 2)],
            [0, 1, 2, 1],
            |g| {
                cast(g, "Sakura-Tribe Elder", 1, None, None);
                let elder = named(g, "Sakura-Tribe Elder", 1);
                let lands = g
                    .state
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(1))
                    .count();
                announce(g, "Murder", 3, obj(elder), None);
                activate(g, "Sakura-Tribe Elder", 1, None, None);
                if stifle {
                    announce(g, "Stifle", 2, None, None);
                }
                settle(g);
                assert!(matches!(
                    g.zone(named(g, "Sakura-Tribe Elder", 1)),
                    Some(Zone::Graveyard { .. })
                ));
                let after = g
                    .state
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield && o.controller == PlayerId(1))
                    .count();
                assert_eq!(after, lands - 1 + usize::from(!stifle));
                if !stifle {
                    assert_eq!(g.state.match_state.as_ref().unwrap().library_shuffles[1], 1);
                }
            },
        );
    }
}
#[test]
fn sacrifice_mogg_arsonist_optional_and_siege_goblin_filter() {
    natural(
        "goblins",
        &[
            ("Goblin Bombardment", 3),
            ("Mogg Fanatic", 3),
            ("Goblin Arsonist", 3),
            ("Siege-Gang Commander", 3),
            ("Raging Goblin", 3),
            ("Blood Artist", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Blood Artist", 1, None, None);
            for n in [
                "Goblin Bombardment",
                "Mogg Fanatic",
                "Goblin Arsonist",
                "Siege-Gang Commander",
                "Raging Goblin",
            ] {
                cast(g, n, 3, None, None);
            }
            let life = g.state.players[0].life.clone();
            activate(g, "Mogg Fanatic", 3, player(0), None);
            settle(g);
            let mut e = life;
            e.subtract(&2.into());
            assert_eq!(g.state.players[0].life, e); // one damage and one Artist trigger
            let arsonist = named(g, "Goblin Arsonist", 3);
            activate(g, "Goblin Bombardment", 3, player(0), Some(arsonist));
            loop {
                let r = request(g);
                if matches!(r, Request::OptionalEffect { .. }) {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            check(g);
            let mut declined = g.fork();
            answer(&mut declined, Response::OptionalEffect { perform: false });
            settle(&mut declined);
            check(&declined);
            answer(g, Response::OptionalEffect { perform: true });
            settle(g);
            let mut expected = declined.state.players[0].life.clone();
            expected.subtract(&1.into());
            assert_eq!(g.state.players[0].life, expected);
            let token = tokens(g, 3, "Goblin")[0];
            activate(g, "Siege-Gang Commander", 3, player(0), Some(token));
            settle(g);
            let raging = named(g, "Raging Goblin", 3);
            activate(g, "Siege-Gang Commander", 3, player(0), Some(raging));
            settle(g);
            cast(g, "Rorix Bladewing", 3, None, None);
            priority(g, 3);
            answer(
                g,
                Response::ActivateAbility {
                    object: named(g, "Siege-Gang Commander", 3),
                    ability: 0,
                },
            );
            answer(
                g,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player {
                        player: PlayerId(0),
                    }],
                },
            );
            loop {
                let r = request(g);
                if let Request::SacrificeCost { candidates, .. } = r {
                    assert!(!candidates.contains(&named(g, "Rorix Bladewing", 3)));
                    answer(
                        g,
                        Response::SacrificeCost {
                            object: candidates[0],
                        },
                    );
                    break;
                }
                let a = match &r {
                    Request::Mana { .. } | Request::SpendMana { .. } => {
                        pay(g, player(0), None);
                        break;
                    }
                    _ => auto(g, &r),
                };
                answer(g, a);
            }
            settle(g);
        },
    );
}
#[test]
fn sacrifice_commander_graveyard_death_then_command_return_and_exile_is_not_death() {
    natural(
        "commander-artist",
        &[
            ("Blood Artist", 1),
            ("Carrion Feeder", 1),
            ("Swords to Plowshares", 0),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Blood Artist", 1, None, None);
            cast(g, "Carrion Feeder", 1, None, None);
            cast(g, "Yargle and Multani", 1, None, None);
            let commander = named(g, "Yargle and Multani", 1);
            activate(g, "Carrion Feeder", 1, None, Some(commander));
            settle(g);
            assert!(matches!(
                g.zone(named(g, "Yargle and Multani", 1)),
                Some(Zone::Command { .. })
            ));
            let before = g.state.players[0].life.clone();
            cast(
                g,
                "Swords to Plowshares",
                0,
                obj(named(g, "Carrion Feeder", 1)),
                None,
            );
            assert_eq!(g.state.players[0].life, before);
            if g.trace.mode == TraceMode::Detailed {
                let source = named(g, "Blood Artist", 1).card;
                assert_eq!(
                    g.events()
                        .iter()
                        .filter(
                            |e| matches!(e,Event::TriggerCollected {source:s,..} if s.card==source)
                        )
                        .count(),
                    1
                );
            }
        },
    );
}
#[test]
fn sacrifice_tucked_token_ceases_with_rancor_sba_and_return_in_same_wave() {
    natural(
        "token-aura-tuck",
        &[
            ("Bastion of Remembrance", 1),
            ("Rancor", 1),
            ("Time Ebb", 2),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Bastion of Remembrance", 1, None, None);
            let token = tokens(g, 1, "Soldier")[0];
            cast(g, "Rancor", 1, obj(token), None);
            cast(g, "Time Ebb", 2, obj(token), None);
            assert!(g.current(token).is_err());
            assert!(matches!(
                g.zone(named(g, "Rancor", 1)),
                Some(Zone::Hand { .. })
            ));
            assert!(tokens(g, 1, "Soldier").is_empty());
        },
    );
}

fn to_attackers(g: &mut Game, seat: u8, attacker: ObjectId, after_turn: u64) {
    for _ in 0..2000 {
        let r = request(g);
        if let Request::Attackers { candidates, .. } = &r
            && r.player() == PlayerId(seat)
            && g.turn().number > after_turn
            && candidates.contains(&attacker)
        {
            return;
        }
        let a = auto(g, &r);
        answer(g, a);
    }
    panic!("attackers cap")
}

#[test]
fn double_strike_in_sacrifice_profile() {
    let ace_id = catalog()
        .definitions
        .iter()
        .find(|d| d.name == "Fencing Ace")
        .unwrap()
        .oracle_id
        .clone();
    assert!(
        match_profile("commander-sacrifice-v1").unwrap().decks[0]
            .clone()
            .cards
            .iter()
            .any(|e| e.oracle_id == ace_id)
    );
    natural("probe-ds", &[("Fencing Ace", 0)], [0, 1, 2, 3], |g| {
        cast(g, "Fencing Ace", 0, None, None);
        let ace = named(g, "Fencing Ace", 0);
        let turn = g.turn().number;
        to_attackers(g, 0, ace, turn);
        let life = g.state.players[1].life.clone();
        answer(
            g,
            Response::Attackers {
                attacks: vec![(ace, PlayerId(1))],
            },
        );
        let mut saw_first_strike_step = false;
        for _ in 0..200 {
            if g.turn().step == TurnStep::FirstStrikeDamage && !saw_first_strike_step {
                check(g);
                saw_first_strike_step = true;
            }
            if matches!(
                g.turn().step,
                TurnStep::EndCombat | TurnStep::PostcombatMain
            ) {
                break;
            }
            let r = request(g);
            let a = auto(g, &r);
            answer(g, a);
        }
        let mut expected = life;
        expected.subtract(&2.into());
        assert_eq!(
            g.state.players[1].life, expected,
            "double strike dealt damage once"
        );
        assert!(saw_first_strike_step, "no first-strike damage step");
    });
}

#[test]
fn blocker_hints_and_first_strike_death_in_sacrifice_profile() {
    natural(
        "probe-hints",
        &[("Typhoid Rats", 1), ("Fencing Ace", 0)],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Typhoid Rats", 1, None, None);
            cast(g, "Fencing Ace", 0, None, None);
            let ace = named(g, "Fencing Ace", 0);
            let rats = named(g, "Typhoid Rats", 1);
            let turn = g.turn().number;
            to_attackers(g, 0, ace, turn);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(ace, PlayerId(1))],
                },
            );
            for _ in 0..200 {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) && r.player() == PlayerId(1) {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            let Some(PlayerRequest::Blockers {
                legal_attackers,
                candidates,
                ..
            }) = observation(g, 1).decision
            else {
                panic!("blockers projection")
            };
            let rat_handle = g.handle_for(PlayerId(1), rats).unwrap();
            let ace_handle = g.handle_for(PlayerId(1), ace).unwrap();
            assert!(candidates.contains(&rat_handle));
            assert!(
                legal_attackers
                    .iter()
                    .any(|row| row.blocker == rat_handle && row.attackers.contains(&ace_handle))
            );
            assert!(
                !legal_attackers.is_empty(),
                "per-blocker legality hints are empty in the sacrifice profile"
            );
            answer(
                g,
                Response::Blockers {
                    blocks: vec![(rats, ace)],
                },
            );
            for _ in 0..200 {
                if matches!(
                    g.turn().step,
                    TurnStep::EndCombat | TurnStep::PostcombatMain
                ) {
                    break;
                }
                let r = request(g);
                let a = auto(g, &r);
                answer(g, a);
            }
            // CR 510.4, 702.4b/7b: the Rats die before normal damage and
            // cannot deal their deathtouch damage to the first-step participant.
            assert_eq!(g.current(ace).unwrap().zone, Zone::Battlefield);
            assert_eq!(
                named(g, "Typhoid Rats", 1).incarnation,
                rats.incarnation + 1
            );
            assert_eq!(
                g.current(named(g, "Typhoid Rats", 1)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(1) }
            );
        },
    );
}

#[test]
fn forced_previous_choice_after_chooser_concedes() {
    natural(
        "probe-forced-concede",
        &[
            ("Butcher of Malakir", 0),
            ("Carrion Feeder", 0),
            ("Butcher of Malakir", 1),
            ("Typhoid Rats", 2),
        ],
        [1, 1, 1, 1],
        |g| {
            cast(g, "Butcher of Malakir", 0, None, None);
            cast(g, "Carrion Feeder", 0, None, None);
            cast(g, "Butcher of Malakir", 1, None, None);
            cast(g, "Typhoid Rats", 2, None, None);
            main_for(g, 0);
            let feeder = named(g, "Carrion Feeder", 0);
            activate(g, "Carrion Feeder", 0, None, Some(feeder));
            for _ in 0..200 {
                let r = request(g);
                if matches!(r, Request::ForcedSacrifice { .. }) {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            let b1 = named(g, "Butcher of Malakir", 1);
            answer(g, Response::ForcedSacrifice { object: b1 });
            assert_eq!(request(g).player(), PlayerId(2));
            g.concede(PlayerId(1)).unwrap();
            let r = request(g);
            assert_eq!(r.player(), PlayerId(2), "{r:?}");
            let o = observation(g, 2);
            let Some(PlayerRequest::ForcedSacrifice {
                previous_choices, ..
            }) = o.decision
            else {
                panic!("forced projection")
            };
            assert!(previous_choices.is_empty());
            assert!(o.view.sacrifice_choices.is_empty());
            check(g);
            answer(
                g,
                Response::ForcedSacrifice {
                    object: named(g, "Typhoid Rats", 2),
                },
            );
            settle(g);
        },
    );
}
