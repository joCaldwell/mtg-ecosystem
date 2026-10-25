//! Every gameplay witness begins with admit_pod and seeded legal 100-card decks.
//! Host-only seed selection arranges no cards; all draws, land plays, casts,
//! costs, choices and concessions are recorded accepted inputs. No injected state.
use super::*;
use rand_core::{Rng, SeedableRng};
fn catalog() -> Catalog {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-wipes-ir.json"
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
        Request::AnnounceX { .. } => Response::AnnounceX { value: 3.into() },
        Request::Targets { candidates, .. } => Response::Targets {
            objects: vec![
                g.top()
                    .ok()
                    .map(|s| s.source)
                    .filter(|s| candidates.contains(s))
                    .unwrap_or(candidates[0]),
            ],
        },
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
        Request::DamageAllocation {
            candidates,
            required,
            ..
        } => {
            let mut left = required.clone();
            let mut allocations = vec![];
            for c in candidates {
                let n = std::cmp::min(left.clone(), c.amount.clone());
                left.subtract(&n);
                allocations.push((c.damage, n));
            }
            Response::DamageAllocation { allocations }
        }
        Request::DamageEffect { candidates, .. } => Response::DamageEffect {
            effect: candidates
                .iter()
                .find(|c| matches!(c.effect, EffectKey::Static { .. }))
                .unwrap_or(&candidates[0])
                .effect
                .clone(),
            damage: candidates
                .iter()
                .find(|c| matches!(c.effect, EffectKey::Static { .. }))
                .unwrap_or(&candidates[0])
                .damage,
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
            Request::AnnounceX { .. } => {
                check(g);
                let o = observation(g, r.player().0);
                let d = o.decision.unwrap();
                assert!(matches!(d, PlayerRequest::AnnounceX { .. }));
                let QueryResult::Incomplete { work, .. } = g.check_player_response(
                    r.player(),
                    d.token().clone(),
                    PlayerResponse::AnnounceX { value: 3.into() },
                    Budget::new(0),
                ) else {
                    panic!("X query work")
                };
                let bytes = g.save_work(&work).unwrap();
                let restored =
                    Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap();
                let work = restored.restore_work(&bytes).unwrap();
                assert!(matches!(
                    restored.resume_work(work, Budget::unlimited()),
                    QueryResult::Complete(QueryValue::Validated(_))
                ));
                Response::AnnounceX { value: 3.into() }
            }
            Request::Modes { .. } => Response::Modes { mode: 1 },
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
                        .unwrap_or_else(|| {
                            panic!(
                                "available land for {} at turn {} cost {:?}",
                                g.definition(g.top().unwrap().source.card).unwrap().name,
                                g.turn().number,
                                g.top().unwrap().total_cost
                            )
                        });
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
        profile: "commander-wipes-v1".into(),
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
    let seed = (0u32..2_000_000)
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
                if library
                    .iter()
                    .skip(if label == "wipes-anger-live-payoffs" {
                        30
                    } else {
                        20
                    })
                    .any(|id| wanted.contains(id))
                {
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
                        >= 3
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
fn observation(g: &Game, seat: u8) -> Observation {
    let QueryResult::Complete(QueryValue::Observation(o)) =
        g.observe(PlayerId(seat), Budget::unlimited())
    else {
        panic!("observation");
    };
    o
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
fn double_strike_in_wipes_profile() {
    let ace_id = catalog()
        .definitions
        .iter()
        .find(|d| d.name == "Fencing Ace")
        .unwrap()
        .oracle_id
        .clone();
    assert!(
        match_profile("commander-wipes-v1").unwrap().decks[0]
            .clone()
            .cards
            .iter()
            .any(|e| e.oracle_id == ace_id)
    );
    natural("wipes-ds", &[("Fencing Ace", 0)], [0, 1, 2, 3], |g| {
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
fn blocker_hints_and_first_strike_death_in_wipes_profile() {
    natural(
        "wipes-hints",
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

fn battlefield(g: &Game, name: &str, seat: u8) -> bool {
    g.current(named(g, name, seat)).unwrap().zone == Zone::Battlefield
}
fn life(g: &Game, p: u8) -> Quantity {
    g.state.players[p as usize].life.clone()
}
fn trigger_count(g: &Game, name: &str) -> usize {
    g.events().iter().filter(|e|matches!(e,Event::TriggerCollected {source,..} if g.definition(source.card).unwrap().name==name)).count()
}
#[test]
fn wipes_profile_catalog_decks_policy_bounds_and_prior_catalog_rejection() {
    let c = catalog();
    assert_eq!(
        digest(&c).unwrap(),
        match_profile("commander-wipes-v1")
            .unwrap()
            .catalog_sha256
            .as_str()
    );
    for p in MATCH_PROFILES.iter().take(10) {
        assert!(match_profile(&p.name).is_some());
    }
    let config = MatchConfig::for_profile("commander-wipes-v1", [7; 32], PlayerId(0)).unwrap();
    let g = Game::new(Prepared::admit_pod(c, config.clone()).unwrap());
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
    let old = Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-sacrifice-ir.json"
    ))
    .unwrap();
    let received = format!(
        "received {} at IR v{}",
        old.definitions.len(),
        old.schema_version
    );
    let MatchAdmissionError::InvalidCatalog { detail } =
        Prepared::admit_pod(old, config).err().unwrap()
    else {
        panic!("expected typed catalog rejection")
    };
    assert!(detail.contains(&format!(
            "expected {} definitions",
            super::match_admission::reviewed_catalog_size(
                match_profile("commander-wipes-v1")
                    .unwrap()
                    .catalog_sha256
                    .as_str()
            )
            .unwrap()
        )));
    assert!(detail.contains(&received));
}
#[test]
fn wipes_fumigate_counts_token_commander_and_simultaneous_payoffs() {
    natural(
        "wipes-fumigate",
        &[
            ("Fumigate", 0),
            ("Bastion of Remembrance", 1),
            ("Blood Artist", 1),
            ("Zulaport Cutthroat", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Yargle and Multani", 1, None, None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            cast(g, "Blood Artist", 1, None, None);
            cast(g, "Zulaport Cutthroat", 1, None, None);
            let count = g.objects_in_set(&ObjectSet::Creatures, PlayerId(0)).len();
            assert_eq!(count, 4);
            let before = life(g, 0);
            cast(g, "Fumigate", 0, None, None);
            assert!(!battlefield(g, "Yargle and Multani", 1));
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1)).unwrap().zone,
                Zone::Command { owner: PlayerId(1) }
            );
            assert!(tokens(g, 1, "Soldier").is_empty());
            let mut expected = before;
            expected.subtract(&8.into());
            assert_eq!(life(g, 0), expected);
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(trigger_count(g, "Blood Artist"), 4);
                assert_eq!(trigger_count(g, "Zulaport Cutthroat"), 4);
                assert_eq!(trigger_count(g, "Bastion of Remembrance"), 5); // enters plus four dies.
                assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::LifeGained {player:PlayerId(0),amount,..},..} if *amount==Quantity::from(4))));
            }
        },
    );
}
#[test]
fn wipes_planar_cleansing_nonlands_rancor_return() {
    natural(
        "wipes-planar",
        &[
            ("Planar Cleansing", 0),
            ("Sol Ring", 0),
            ("Glorious Anthem", 0),
            ("Bonesplitter", 0),
            ("Holy Strength", 0),
            ("Bastion of Remembrance", 1),
            ("Rancor", 1),
            ("Command Tower", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            let dog = named(g, "Isamaru, Hound of Konda", 0);
            for n in ["Sol Ring", "Glorious Anthem", "Bonesplitter"] {
                cast(g, n, 0, None, None);
            }
            cast(g, "Holy Strength", 0, obj(dog), None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            cast(g, "Yargle and Multani", 1, None, None);
            let y = named(g, "Yargle and Multani", 1);
            cast(g, "Rancor", 1, obj(y), None);
            main_for(g, 1);
            if g.state.turn.lands_played == 0 {
                answer(
                    g,
                    Response::PlayLand {
                        card: named(g, "Command Tower", 1).card,
                    },
                );
                let _ = request(g);
            }
            let lands = g
                .state
                .objects
                .values()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
                })
                .count();
            cast(g, "Planar Cleansing", 0, None, None);
            assert_eq!(
                g.state
                    .objects
                    .values()
                    .filter(|o| o.zone == Zone::Battlefield)
                    .count(),
                lands
            );
            assert_eq!(
                g.current(named(g, "Rancor", 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            for (n, p) in [
                ("Sol Ring", 0),
                ("Glorious Anthem", 0),
                ("Bonesplitter", 0),
                ("Holy Strength", 0),
                ("Bastion of Remembrance", 1),
            ] {
                assert!(matches!(
                    g.current(named(g, n, p)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
            }
        },
    );
}
#[test]
fn wipes_final_judgment_exiles_tokens_and_commander_without_dies() {
    natural(
        "wipes-final",
        &[
            ("Final Judgment", 0),
            ("Bastion of Remembrance", 1),
            ("Blood Artist", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Yargle and Multani", 1, None, None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            cast(g, "Blood Artist", 1, None, None);
            cast(g, "Final Judgment", 0, None, None);
            assert!(tokens(g, 1, "Soldier").is_empty());
            assert_eq!(
                g.current(named(g, "Blood Artist", 1)).unwrap().zone,
                Zone::Exile
            );
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1)).unwrap().zone,
                Zone::Command { owner: PlayerId(1) }
            );
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(trigger_count(g, "Blood Artist"), 0);
                assert_eq!(trigger_count(g, "Bastion of Remembrance"), 1);
            }
        },
    );
}
#[test]
fn wipes_infest_fixed_set_anthem_and_counter_survive_later_creature_unaffected() {
    natural(
        "wipes-infest",
        &[
            ("Infest", 1),
            ("Glorious Anthem", 0),
            ("Ironpaw Aspirant", 0),
            ("Alpine Watchdog", 0),
            ("Typhoid Rats", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Glorious Anthem", 0, None, None);
            cast(g, "Ironpaw Aspirant", 0, None, None);
            // Its ETB target is selected explicitly below, so cast it by hand.
            let aspirant = named(g, "Ironpaw Aspirant", 0);
            assert_eq!(
                g.characteristics(aspirant).unwrap().toughness,
                Some(4.into())
            );
            cast(g, "Alpine Watchdog", 0, None, None);
            cast(g, "Infest", 1, None, None);
            assert_eq!(
                g.characteristics(aspirant).unwrap().toughness,
                Some(2.into())
            );
            assert_eq!(
                g.characteristics(named(g, "Alpine Watchdog", 0))
                    .unwrap()
                    .toughness,
                Some(1.into())
            );
            cast(g, "Typhoid Rats", 1, None, None);
            assert_eq!(
                g.characteristics(named(g, "Typhoid Rats", 1))
                    .unwrap()
                    .toughness,
                Some(1.into())
            );
            assert!(!g.state.effects.is_empty());
        },
    );
}
#[test]
fn wipes_innocent_blood_includes_caster_apnap_simultaneous() {
    natural(
        "wipes-innocent",
        &[
            ("Innocent Blood", 1),
            ("Typhoid Rats", 1),
            ("Storm Crow", 2),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            cast(g, "Typhoid Rats", 1, None, None);
            cast(g, "Storm Crow", 2, None, None);
            main_for(g, 1);
            announce(g, "Innocent Blood", 1, None, None);
            let mut order = vec![];
            for _ in 0..100 {
                let r = request(g);
                if let Request::ForcedSacrifice {
                    player, candidates, ..
                } = &r
                {
                    if order.is_empty() {
                        check(g);
                    }
                    order.push(*player);
                    assert_eq!(
                        g.objects_in_set(&ObjectSet::Creatures, PlayerId(1)).len(),
                        3
                    );
                    answer(
                        g,
                        Response::ForcedSacrifice {
                            object: candidates[0],
                        },
                    );
                } else if matches!(r, Request::Priority { .. }) && g.state.stack.is_empty() {
                    break;
                } else {
                    let a = auto(g, &r);
                    answer(g, a);
                }
            }
            assert_eq!(order, [PlayerId(1), PlayerId(2), PlayerId(0)]);
            assert!(
                g.objects_in_set(&ObjectSet::Creatures, PlayerId(1))
                    .is_empty()
            );
        },
    );
}
#[test]
fn wipes_massacre_wurm_opponents_tokens_and_later_combat_death() {
    natural(
        "wipes-wurm",
        &[
            ("Massacre Wurm", 1),
            ("Raise the Alarm", 0),
            ("Alpine Watchdog", 0),
            ("Serra Angel", 0),
            ("Giant Spider", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Raise the Alarm", 0, None, None);
            cast(g, "Alpine Watchdog", 0, None, None);
            cast(g, "Serra Angel", 0, None, None);
            cast(g, "Giant Spider", 1, None, None);
            answer(g, Response::Pass);
            main_for(g, 1);
            let before = life(g, 0);
            cast(g, "Massacre Wurm", 1, None, None);
            let mut expected = before;
            expected.subtract(&6.into());
            assert_eq!(life(g, 0), expected);
            assert_eq!(
                g.characteristics(named(g, "Massacre Wurm", 1))
                    .unwrap()
                    .toughness,
                Some(5.into())
            );
            assert_eq!(
                g.characteristics(named(g, "Giant Spider", 1))
                    .unwrap()
                    .toughness,
                Some(4.into())
            );
            let spider = named(g, "Giant Spider", 1);
            let angel = named(g, "Serra Angel", 0);
            let turn = g.turn().number;
            to_attackers(g, 1, spider, turn - 1);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(spider, PlayerId(0))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) && r.player() == PlayerId(0) {
                    answer(
                        g,
                        Response::Blockers {
                            blocks: vec![(angel, spider)],
                        },
                    );
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            for _ in 0..100 {
                let r = request(g);
                if g.turn().step == TurnStep::PostcombatMain {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            assert_eq!(g.turn().number, turn);
            assert!(!battlefield(g, "Serra Angel", 0));
            expected.subtract(&2.into());
            assert_eq!(life(g, 0), expected);
        },
    );
}
#[test]
fn wipes_pyroclasm_furnace_and_individual_healer_shield() {
    natural(
        "wipes-pyro",
        &[
            ("Pyroclasm", 3),
            ("Furnace of Rath", 3),
            ("Samite Healer", 0),
            ("Serra Angel", 0),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Samite Healer", 0, None, None);
            cast(g, "Serra Angel", 0, None, None);
            cast(g, "Yargle and Multani", 1, None, None);
            cast(g, "Furnace of Rath", 3, None, None);
            main_for(g, 0);
            main_for(g, 3);
            let angel = named(g, "Serra Angel", 0);
            activate(g, "Samite Healer", 0, obj(angel), None);
            settle(g);
            announce(g, "Pyroclasm", 3, None, None);
            settle(g);
            assert_eq!(g.current(angel).unwrap().marked_damage, 3.into());
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1))
                    .unwrap()
                    .marked_damage,
                4.into()
            );
            assert!(!battlefield(g, "Samite Healer", 0));
        },
    );
}
#[test]
fn wipes_anger_damage_later_bolt_sacrifice_and_cleanup() {
    natural(
        "wipes-anger",
        &[
            ("Anger of the Gods", 3),
            ("Lightning Bolt", 3),
            ("Fling", 3),
            ("Blood Artist", 1),
            ("Zulaport Cutthroat", 1),
            ("Murder", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            cast(g, "Yargle and Multani", 1, None, None);
            cast(g, "Caelorna, Coral Tyrant", 2, None, None);
            cast(g, "Rorix Bladewing", 3, None, None);
            cast(g, "Blood Artist", 1, None, None);
            cast(g, "Zulaport Cutthroat", 1, None, None);
            main_for(g, 3);
            announce(g, "Anger of the Gods", 3, None, None);
            settle(g);
            check(g);
            assert_eq!(
                g.current(named(g, "Isamaru, Hound of Konda", 0))
                    .unwrap()
                    .zone,
                Zone::Command { owner: PlayerId(0) }
            );
            assert_eq!(
                g.current(named(g, "Blood Artist", 1)).unwrap().zone,
                Zone::Exile
            );
            assert_eq!(
                g.current(named(g, "Zulaport Cutthroat", 1)).unwrap().zone,
                Zone::Exile
            );
            let y = named(g, "Yargle and Multani", 1);
            announce(g, "Lightning Bolt", 3, obj(y), None);
            settle(g);
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1)).unwrap().zone,
                Zone::Command { owner: PlayerId(1) }
            );
            let r = named(g, "Rorix Bladewing", 3);
            announce(g, "Fling", 3, player(2), Some(r));
            settle(g);
            assert_eq!(
                g.current(named(g, "Rorix Bladewing", 3)).unwrap().zone,
                Zone::Command { owner: PlayerId(3) }
            );
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(trigger_count(g, "Blood Artist"), 0);
                assert_eq!(trigger_count(g, "Zulaport Cutthroat"), 0);
                let exiles=g.events().iter().filter(|e|matches!(e,Event::Committed {actual:EventOutcome::Moved {movement}|EventOutcome::StateBasedMoved {movement,..}|EventOutcome::Sacrificed {movement,..},..} if movement.from==Zone::Battlefield && movement.destination==Zone::Exile)).count();
                assert!(exiles >= 5);
            }
            main_for(g, 1);
            assert!(g.state.effects.is_empty());
            let c = named(g, "Caelorna, Coral Tyrant", 2);
            announce(g, "Murder", 1, obj(c), None);
            settle(g);
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::Moved {movement},..} if movement.previous==c && matches!(movement.destination,Zone::Graveyard {..}))));
            }
        },
    );
}
#[test]
fn wipes_anger_fully_prevented_creature_dies_normally() {
    natural(
        "wipes-anger-prevent",
        &[
            ("Anger of the Gods", 3),
            ("Lightning Bolt", 3),
            ("Healing Salve", 0),
            ("Blood Artist", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            cast(g, "Blood Artist", 1, None, None);
            main_for(g, 3);
            let dog = named(g, "Isamaru, Hound of Konda", 0);
            announce(g, "Healing Salve", 0, obj(dog), None);
            settle(g);
            announce(g, "Anger of the Gods", 3, None, None);
            settle(g);
            assert!(battlefield(g, "Isamaru, Hound of Konda", 0));
            announce(g, "Lightning Bolt", 3, obj(dog), None);
            settle(g);
            if g.trace.mode == TraceMode::Detailed {
                assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::StateBasedMoved {movement,..},..} if movement.previous==dog && matches!(movement.destination,Zone::Graveyard {..}))));
            }
        },
    );
}
#[test]
fn wipes_fallout_counterspell_and_swan_bird() {
    natural(
        "wipes-fallout",
        &[
            ("Volcanic Fallout", 3),
            ("Counterspell", 2),
            ("Swan Song", 2),
        ],
        [0, 1, 2, 3],
        |g| {
            main_for(g, 3);
            announce(g, "Volcanic Fallout", 3, None, None);
            let fallout = g.top().unwrap().id;
            announce(g, "Counterspell", 2, None, None);
            for _ in 0..50 {
                let r = request(g);
                if g.state.stack.len() == 1 {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            assert_eq!(g.top().unwrap().id, fallout);
            announce(g, "Swan Song", 2, None, None);
            for _ in 0..50 {
                let r = request(g);
                if g.state.stack.len() == 1 {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            assert_eq!(g.top().unwrap().id, fallout);
            assert_eq!(tokens(g, 3, "Bird").len(), 1);
            let before = life(g, 0);
            settle(g);
            let mut expected = before;
            expected.subtract(&2.into());
            assert_eq!(life(g, 0), expected);
            assert!(tokens(g, 3, "Bird").is_empty());
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(
                    g.events()
                        .iter()
                        .filter(|e| matches!(
                            e,
                            Event::Committed {
                                actual: EventOutcome::CounterPrevented { .. },
                                ..
                            }
                        ))
                        .count(),
                    2
                );
            }
        },
    );
}
#[test]
fn wipes_chain_reaction_counts_flying_and_tokens_at_resolution() {
    natural(
        "wipes-chain",
        &[
            ("Chain Reaction", 3),
            ("Raise the Alarm", 0),
            ("Storm Crow", 2),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Caelorna, Coral Tyrant", 2, None, None);
            cast(g, "Storm Crow", 2, None, None);
            main_for(g, 3);
            announce(g, "Chain Reaction", 3, None, None);
            announce(g, "Raise the Alarm", 0, None, None);
            settle(g);
            assert_eq!(
                g.current(named(g, "Caelorna, Coral Tyrant", 2))
                    .unwrap()
                    .marked_damage,
                4.into()
            );
            assert!(!battlefield(g, "Storm Crow", 2));
            assert!(tokens(g, 0, "Soldier").is_empty());
        },
    );
}
#[test]
fn wipes_earthquake_x_zero_announcement_restore_mv_flying_and_furnace() {
    natural(
        "wipes-earthquake",
        &[("Earthquake", 3), ("Furnace of Rath", 3), ("Storm Crow", 2)],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Storm Crow", 2, None, None);
            cast(g, "Rorix Bladewing", 3, None, None);
            cast(g, "Caelorna, Coral Tyrant", 2, None, None);
            cast(g, "Furnace of Rath", 3, None, None);
            main_for(g, 3);
            priority(g, 3);
            answer(
                g,
                Response::Cast {
                    card: named(g, "Earthquake", 3).card,
                },
            );
            assert!(matches!(request(g), Request::AnnounceX { .. }));
            check(g);
            let before = g.state_digest().unwrap();
            let req = request(g);
            assert!(
                g.respond(
                    PlayerId(2),
                    req.decision_id(),
                    Response::AnnounceX { value: 3.into() }
                )
                .is_err()
            );
            assert!(
                g.respond(
                    PlayerId(3),
                    req.decision_id(),
                    Response::AnnounceX { value: (-1).into() }
                )
                .is_err()
            );
            assert_eq!(before, g.state_digest().unwrap());
            // Exercise the public request, query receipt and sidecar path in pay.
            pay(g, None, None);
            let view = observation(g, 0).view;
            assert_eq!(view.stack.last().unwrap().x, Some(3.into()));
            assert_eq!(view.stack.last().unwrap().mana_value, Some(4.into()));
            check(g);
            let lives = g
                .state
                .players
                .iter()
                .map(|p| p.life.clone())
                .collect::<Vec<_>>();
            settle(g);
            for (i, old) in lives.iter().enumerate() {
                let mut expected = old.clone();
                expected.subtract(&6.into());
                assert_eq!(g.state.players[i].life, expected);
            }
            assert_eq!(
                g.current(named(g, "Storm Crow", 2)).unwrap().marked_damage,
                0.into()
            );
            assert_eq!(
                g.current(named(g, "Rorix Bladewing", 3))
                    .unwrap()
                    .marked_damage,
                0.into()
            );
            assert_eq!(
                g.current(named(g, "Caelorna, Coral Tyrant", 2))
                    .unwrap()
                    .marked_damage,
                6.into()
            );
            assert!(
                g.state
                    .commanders
                    .records
                    .values()
                    .all(|h| h.combat_damage.is_empty())
            );
        },
    );
    natural("wipes-xzero", &[("Earthquake", 3)], [0, 1, 2, 3], |g| {
        main_for(g, 3);
        answer(
            g,
            Response::Cast {
                card: named(g, "Earthquake", 3).card,
            },
        );
        answer(g, Response::AnnounceX { value: 0.into() });
        pay(g, None, None);
        assert_eq!(g.top().unwrap().total_cost.generic, 0.into());
        let before = life(g, 3);
        settle(g);
        assert_eq!(life(g, 3), before);
    });
}
#[test]
fn wipes_blasphemous_act_reduction_floor_eight_and_twelve_and_unpayable() {
    natural(
        "wipes-act",
        &[
            ("Blasphemous Act", 3),
            ("Bonesplitter", 3),
            ("Goblin Instigator", 3),
            ("Beetleback Chief", 3),
            ("Chimney Rabble", 3),
            ("Siege-Gang Commander", 3),
        ],
        [0, 1, 2, 3],
        |g| {
            main_for(g, 3);
            let act = named(g, "Blasphemous Act", 3).card;
            assert_eq!(
                g.casting_cost(act, PlayerId(3), Zone::Hand { owner: PlayerId(3) })
                    .unwrap()
                    .generic,
                8.into()
            );
            cast(g, "Bonesplitter", 3, None, None);
            cast(g, "Goblin Instigator", 3, None, None);
            let goblin = named(g, "Goblin Instigator", 3);
            while g
                .state
                .objects
                .values()
                .filter(|o| {
                    o.zone == Zone::Battlefield
                        && o.controller == PlayerId(3)
                        && !o.tapped
                        && g.definition(o.id.card).unwrap().characteristics.kind == CardKind::Land
                })
                .count()
                > 1
            {
                activate(g, "Bonesplitter", 3, obj(goblin), None);
                settle(g);
            }
            assert_eq!(
                g.casting_cost(act, PlayerId(3), Zone::Hand { owner: PlayerId(3) })
                    .unwrap()
                    .generic,
                6.into()
            );
            assert!(
                g.state.players[3]
                    .mana
                    .values()
                    .all(|q| *q == Quantity::zero())
            );
            answer(g, Response::Cast { card: act });
            answer(g, Response::FinishMana { undo_mana: true });
            let _ = request(g);
            assert!(g.state.stack.is_empty());
            assert_eq!(
                g.current(named(g, "Blasphemous Act", 3)).unwrap().zone,
                Zone::Hand { owner: PlayerId(3) }
            );
            for n in ["Beetleback Chief", "Chimney Rabble", "Rorix Bladewing"] {
                cast(g, n, 3, None, None);
            }
            assert_eq!(
                g.objects_in_set(&ObjectSet::Creatures, PlayerId(3)).len(),
                8
            );
            assert_eq!(
                g.casting_cost(act, PlayerId(3), Zone::Hand { owner: PlayerId(3) })
                    .unwrap(),
                ManaCost {
                    generic: 0.into(),
                    colored: vec![Color::R]
                }
            );
            cast(g, "Siege-Gang Commander", 3, None, None);
            assert_eq!(
                g.objects_in_set(&ObjectSet::Creatures, PlayerId(3)).len(),
                12
            );
            assert_eq!(
                g.casting_cost(act, PlayerId(3), Zone::Hand { owner: PlayerId(3) })
                    .unwrap()
                    .generic,
                0.into()
            );
            cast(g, "Blasphemous Act", 3, None, None);
            assert!(
                g.objects_in_set(&ObjectSet::Creatures, PlayerId(3))
                    .is_empty()
            );
        },
    );
}

#[test]
fn wipes_evacuation_combat_commander_choices_token_aura_and_equipment() {
    natural(
        "wipes-evac",
        &[
            ("Evacuation", 2),
            ("Bonesplitter", 0),
            ("Holy Strength", 0),
            ("Raise the Alarm", 0),
            ("Rancor", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            cast(g, "Bonesplitter", 0, None, None);
            let dog = named(g, "Isamaru, Hound of Konda", 0);
            activate(g, "Bonesplitter", 0, obj(dog), None);
            settle(g);
            cast(g, "Holy Strength", 0, obj(dog), None);
            cast(g, "Raise the Alarm", 0, None, None);
            cast(g, "Yargle and Multani", 1, None, None);
            let y = named(g, "Yargle and Multani", 1);
            cast(g, "Rancor", 1, obj(y), None);
            assert_eq!(g.characteristics(dog).unwrap().power, Some(5.into()));
            assert_eq!(g.characteristics(dog).unwrap().toughness, Some(4.into()));
            assert_eq!(g.characteristics(y).unwrap().power, Some(20.into()));
            assert!(g.has_keyword(y, KeywordAbility::Trample));
            let turn = g.turn().number;
            to_attackers(g, 0, dog, turn);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(dog, PlayerId(1))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) && r.player() == PlayerId(1) {
                    answer(
                        g,
                        Response::Blockers {
                            blocks: vec![(y, dog)],
                        },
                    );
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            announce(g, "Evacuation", 2, None, None);
            let mut choices = 0;
            for _ in 0..200 {
                let r = request(g);
                if let Request::CommanderReturn { player, .. } = r {
                    check(g);
                    choices += 1;
                    answer(
                        g,
                        Response::CommanderReturn {
                            to_command: player == PlayerId(0),
                        },
                    );
                } else if matches!(r, Request::Priority { .. }) && g.state.stack.is_empty() {
                    break;
                } else {
                    let a = auto(g, &r);
                    answer(g, a);
                }
            }
            assert_eq!(choices, 2);
            assert!(g.state.combat.is_empty());
            assert!(tokens(g, 0, "Soldier").is_empty());
            assert!(battlefield(g, "Bonesplitter", 0));
            assert_eq!(
                g.current(named(g, "Bonesplitter", 0)).unwrap().attached_to,
                None
            );
            assert_eq!(
                g.current(named(g, "Isamaru, Hound of Konda", 0))
                    .unwrap()
                    .zone,
                Zone::Command { owner: PlayerId(0) }
            );
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
            assert_eq!(
                g.current(named(g, "Holy Strength", 0)).unwrap().zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(
                g.current(named(g, "Rancor", 1)).unwrap().zone,
                Zone::Hand { owner: PlayerId(1) }
            );
        },
    );
}
#[test]
fn wipes_aetherize_only_attackers_and_tranquility_restores_attack() {
    natural(
        "wipes-aetherize",
        &[("Aetherize", 2), ("Fencing Ace", 0), ("Storm Crow", 2)],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Fencing Ace", 0, None, None);
            cast(g, "Storm Crow", 2, None, None);
            cast(g, "Rorix Bladewing", 3, None, None);
            let ace = named(g, "Fencing Ace", 0);
            let crow = named(g, "Storm Crow", 2);
            let turn = g.turn().number;
            to_attackers(g, 0, ace, turn);
            answer(
                g,
                Response::Attackers {
                    attacks: vec![(ace, PlayerId(2))],
                },
            );
            loop {
                let r = request(g);
                if matches!(r, Request::Blockers { .. }) && r.player() == PlayerId(2) {
                    answer(
                        g,
                        Response::Blockers {
                            blocks: vec![(crow, ace)],
                        },
                    );
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            announce(g, "Aetherize", 2, None, None);
            settle(g);
            assert_eq!(
                g.current(named(g, "Fencing Ace", 0)).unwrap().zone,
                Zone::Hand { owner: PlayerId(0) }
            );
            assert!(battlefield(g, "Storm Crow", 2));
            assert!(battlefield(g, "Rorix Bladewing", 3));
            assert!(g.state.combat.is_empty());
        },
    );
    natural(
        "wipes-tranquility",
        &[
            ("Tranquility", 1),
            ("Pacifism", 0),
            ("Spidersilk Armor", 1),
            ("Bastion of Remembrance", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Yargle and Multani", 1, None, None);
            let y = named(g, "Yargle and Multani", 1);
            cast(g, "Pacifism", 0, obj(y), None);
            cast(g, "Spidersilk Armor", 1, None, None);
            cast(g, "Bastion of Remembrance", 1, None, None);
            assert!(g.has_restriction(y, CreatureRestriction::CantAttackOrBlock));
            cast(g, "Tranquility", 1, None, None);
            assert!(!g.has_restriction(y, CreatureRestriction::CantAttackOrBlock));
            for (n, p) in [
                ("Pacifism", 0),
                ("Spidersilk Armor", 1),
                ("Bastion of Remembrance", 1),
            ] {
                assert!(matches!(
                    g.current(named(g, n, p)).unwrap().zone,
                    Zone::Graveyard { .. }
                ));
            }
            let turn = g.turn().number;
            to_attackers(g, 1, y, turn);
            let Request::Attackers { candidates, .. } = request(g) else {
                panic!("attackers")
            };
            assert!(candidates.contains(&y));
        },
    );
}
#[test]
fn wipes_fallout_last_players_lose_simultaneously_draw() {
    natural(
        "wipes-fallout-draw",
        &[
            ("Volcanic Fallout", 3),
            ("Raging Goblin", 3),
            ("Alpine Watchdog", 0),
        ],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Raging Goblin", 3, None, None);
            cast(g, "Alpine Watchdog", 0, None, None);
            g.concede(PlayerId(1)).unwrap();
            g.concede(PlayerId(2)).unwrap();
            let goblin = named(g, "Raging Goblin", 3);
            let lion = named(g, "Alpine Watchdog", 0);
            for _ in 0..12000 {
                let r = request(g);
                if life(g, 0) == 2.into()
                    && life(g, 3) == 2.into()
                    && r.player() == PlayerId(3)
                    && matches!(r, Request::Priority { .. })
                    && g.state.active == Some(PlayerId(3))
                    && g.turn().step == TurnStep::PrecombatMain
                {
                    break;
                }
                let a = if let Request::Attackers {
                    player, candidates, ..
                } = &r
                {
                    let (a, other) = if *player == PlayerId(0) {
                        (lion, 3)
                    } else {
                        (goblin, 0)
                    };
                    Response::Attackers {
                        attacks: if life(g, other) > 2.into() && candidates.contains(&a) {
                            vec![(a, PlayerId(other))]
                        } else {
                            vec![]
                        },
                    }
                } else {
                    auto(g, &r)
                };
                answer(g, a);
            }
            assert_eq!((life(g, 0), life(g, 3)), (2.into(), 2.into()));
            announce(g, "Volcanic Fallout", 3, None, None);
            for _ in 0..100 {
                match g.advance(Budget::unlimited()) {
                    Advance::Draw => {
                        assert!(g.live_players().is_empty());
                        return;
                    }
                    Advance::AwaitingDecision(r) => {
                        let a = auto(g, &r);
                        answer(g, a);
                    }
                    x => panic!("{x:?}"),
                }
            }
            panic!("draw not reached");
        },
    );
}

#[test]
fn wipes_x_has_no_fixed_width_ceiling_and_unpayable_cast_reverses() {
    natural(
        "wipes-xunpayable",
        &[("Earthquake", 3)],
        [0, 1, 2, 3],
        |g| {
            main_for(g, 3);
            answer(
                g,
                Response::Cast {
                    card: named(g, "Earthquake", 3).card,
                },
            );
            let huge: Quantity =
                "100000000000000000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap();
            answer(
                g,
                Response::AnnounceX {
                    value: huge.clone(),
                },
            );
            let _ = request(g);
            assert_eq!(g.top().unwrap().total_cost.generic, huge);
            check(g);
            answer(g, Response::FinishMana { undo_mana: true });
            let _ = request(g);
            assert!(g.state.stack.is_empty());
            assert_eq!(
                g.current(named(g, "Earthquake", 3)).unwrap().zone,
                Zone::Hand { owner: PlayerId(3) }
            );
        },
    );
}

#[test]
fn wipes_anger_suppresses_surviving_artist_and_zulaport_for_later_deaths() {
    natural(
        "wipes-anger-live-payoffs",
        &[
            ("Anger of the Gods", 3),
            ("Lightning Bolt", 3),
            ("Blood Artist", 1),
            ("Zulaport Cutthroat", 1),
            ("Spidersilk Armor", 1),
            ("Vulshok Morningstar", 1),
            ("Giant Growth", 1),
            ("Giant Spider", 1),
            ("Village Rites", 1),
        ],
        [0, 1, 2, 3],
        |g| {
            for n in [
                "Blood Artist",
                "Zulaport Cutthroat",
                "Spidersilk Armor",
                "Vulshok Morningstar",
                "Giant Spider",
                "Yargle and Multani",
            ] {
                cast(g, n, 1, None, None);
            }
            let artist = named(g, "Blood Artist", 1);
            let cutthroat = named(g, "Zulaport Cutthroat", 1);
            activate(g, "Vulshok Morningstar", 1, obj(artist), None);
            settle(g);
            cast(g, "Isamaru, Hound of Konda", 0, None, None);
            main_for(g, 1);
            main_for(g, 3);
            let turn = g.turn().number;
            announce(g, "Anger of the Gods", 3, None, None);
            announce(g, "Giant Growth", 1, obj(cutthroat), None);
            settle(g);
            assert!(battlefield(g, "Blood Artist", 1) && battlefield(g, "Zulaport Cutthroat", 1));
            let spider = named(g, "Giant Spider", 1);
            announce(g, "Lightning Bolt", 3, obj(spider), None);
            settle(g);
            assert_eq!(
                g.current(named(g, "Giant Spider", 1)).unwrap().zone,
                Zone::Exile
            );
            let y = named(g, "Yargle and Multani", 1);
            announce(g, "Village Rites", 1, None, Some(y));
            settle(g);
            assert_eq!(
                g.current(named(g, "Yargle and Multani", 1)).unwrap().zone,
                Zone::Command { owner: PlayerId(1) }
            );
            assert_eq!(g.turn().number, turn);
            assert!(battlefield(g, "Blood Artist", 1) && battlefield(g, "Zulaport Cutthroat", 1));
            if g.trace.mode == TraceMode::Detailed {
                assert_eq!(trigger_count(g, "Blood Artist"), 0);
                assert_eq!(trigger_count(g, "Zulaport Cutthroat"), 0);
            }
        },
    );
}
// Reviewed P6.10 regression: Massacre Wurm dying simultaneously with opposing
// creatures still triggers for each of them (CR 603.10a look-back), and the
// life loss goes to each dying creature's last-known controller.
#[test]
fn wurm_simultaneous_death_looks_back() {
    natural(
        "wurm-simultaneous-death",
        &[("Massacre Wurm", 1), ("Fumigate", 0), ("Serra Angel", 0)],
        [0, 1, 2, 3],
        |g| {
            cast(g, "Caelorna, Coral Tyrant", 2, None, None);
            cast(g, "Massacre Wurm", 1, None, None);
            cast(g, "Serra Angel", 0, None, None);
            main_for(g, 0);
            let n = g.objects_in_set(&ObjectSet::Creatures, PlayerId(0)).len();
            let (l0, l1, l2) = (life(g, 0), life(g, 1), life(g, 2));
            announce(g, "Fumigate", 0, None, None);
            for _ in 0..200 {
                let r = request(g);
                if g.state.stack.iter().any(|s| s.death_controller.is_some()) {
                    check(g);
                }
                if matches!(r, Request::Priority { .. }) && g.state.stack.is_empty() {
                    break;
                }
                let a = auto(g, &r);
                answer(g, a);
            }
            let mut e0 = l0.clone();
            e0.add(&Quantity::from(n as i64));
            e0.subtract(&2.into());
            let mut e2 = l2.clone();
            e2.subtract(&2.into());
            assert_eq!(life(g, 0), e0);
            assert_eq!(life(g, 2), e2);
            assert_eq!(life(g, 1), l1);
        },
    );
}
