//! Source-backed legal 100-card pod prefix: pay U for Jump from the actual hand
//! after an ordinary paid commander attacks, before the actual blocker decision.
//! No gameplay state injection or outcome selection. Synthetic policy probes live
//! separately in the simulator.
use mtg_contracts::{Catalog, Color, KeywordAbility, ManaCost, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;
fn finish_query(g: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..100000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = g.resume_work(work, Budget::new(16)),
            QueryResult::Rejected(e) => panic!("query rejected {e:?}"),
        }
    }
    panic!("query work bound")
}
fn settle_match(g: &mut Game, budget: u64) -> Advance {
    for _ in 0..20000 {
        let a = g.advance(Budget::new(budget));
        if !matches!(a, Advance::Suspended { .. }) {
            return a;
        }
    }
    panic!("automatic work bound")
}
fn observation(g: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(o) = finish_query(g, g.observe(player, Budget::new(100000))) else {
        panic!("observation")
    };
    o
}
fn request(g: &Game) -> (PlayerId, PlayerRequest) {
    let p = g
        .live_players()
        .into_iter()
        .find(|p| g.decision_for(*p).unwrap().is_some())
        .expect("acting player");
    let QueryValue::Decision(Some(r)) =
        finish_query(g, g.player_decision_for(p, Budget::new(100000)))
    else {
        panic!("request")
    };
    (p, r)
}
fn submit_match(g: &mut Game, response: PlayerResponse) {
    let (p, r) = request(g);
    let QueryValue::Validated(receipt) = finish_query(
        g,
        g.check_player_response(p, r.token().clone(), response, Budget::new(100000)),
    ) else {
        panic!("receipt")
    };
    g.submit(receipt).unwrap();
}
fn assert_restore(g: &Game, p: &Prepared) {
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(restored.state_digest().unwrap(), g.state_digest().unwrap());
}
fn assert_replay(g: &Game, p: &Prepared) {
    let replay = Game::replay_with_trace(p.clone(), &g.replay_journal(), TraceMode::Off).unwrap();
    assert_eq!(replay.state_digest().unwrap(), g.state_digest().unwrap());
}
fn reject_input(g: &Game, p: PlayerId, token: DecisionToken, response: PlayerResponse) {
    let before = g.snapshot().unwrap();
    let mut r = g.check_player_response(p, token, response, Budget::new(0));
    for _ in 0..10000 {
        match r {
            QueryResult::Rejected(_) => {
                assert_eq!(before, g.snapshot().unwrap());
                return;
            }
            QueryResult::Incomplete { work, .. } => r = g.resume_work(work, Budget::new(16)),
            _ => panic!("illegal response accepted"),
        }
    }
    panic!("rejection work bound")
}
fn exact_spending(
    cost: &mtg_contracts::ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> BTreeMap<Color, Quantity> {
    let mut rest = available.clone();
    let mut spent = BTreeMap::<Color, Quantity>::new();
    let one = Quantity::from(1);
    for color in &cost.colored {
        rest.get_mut(color).expect("colored mana").subtract(&one);
        spent.entry(*color).or_default().add(&one);
    }
    let mut generic = cost.generic.clone();
    for (color, n) in &mut rest {
        while *n > Quantity::zero() && generic > Quantity::zero() {
            n.subtract(&one);
            generic.subtract(&one);
            spent.entry(*color).or_default().add(&one);
        }
    }
    assert_eq!(generic, Quantity::zero());
    spent
}
fn catalog() -> Catalog {
    match_profile("commander-blocking-targets-v1")
        .unwrap()
        .catalog()
        .clone()
}
fn id(c: &Catalog, name: &str) -> String {
    c.definitions
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .oracle_id
        .clone()
}
fn config(c: &Catalog, seed: [u8; 32]) -> MatchConfig {
    let mut m =
        MatchConfig::for_profile("commander-blocking-targets-v1", seed, PlayerId(0)).unwrap();
    m.decks = std::array::from_fn(|seat| {
        let names = if seat == 0 {
            vec![("Ertai, Wizard Adept", 1), ("Jump", 1), ("Island", 98)]
        } else {
            vec![("Isamaru, Hound of Konda", 1), ("Plains", 99)]
        };
        Decklist {
            commander: id(c, names[0].0),
            cards: names
                .into_iter()
                .map(|(name, count)| DeckEntry {
                    oracle_id: id(c, name),
                    count,
                })
                .collect(),
        }
    });
    assert_eq!(
        validate_pod(c, &m).unwrap().physical_config().cards.len(),
        400
    );
    m
}
fn seed(index: u32) -> [u8; 32] {
    let hex =
        mtg_contracts::sha256(format!("temporary-flight-admission/opening/{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
}
fn setup(c: &Catalog, seed: [u8; 32], q: u64) -> (Prepared, Game) {
    let p = Prepared::admit_pod(c.clone(), config(c, seed)).unwrap();
    let mut g = Game::with_trace(p.clone(), TraceMode::Detailed);
    assert!(matches!(
        settle_match(&mut g, q),
        Advance::AwaitingDecision(_)
    ));
    for _ in 0..4 {
        assert!(matches!(request(&g).1, PlayerRequest::Mulligan { .. }));
        submit_match(&mut g, PlayerResponse::Mulligan { take: false });
        assert!(matches!(
            settle_match(&mut g, q),
            Advance::AwaitingDecision(_)
        ));
    }
    assert!(matches!(request(&g).1, PlayerRequest::Priority { .. }));
    assert_eq!(
        observation(&g, PlayerId(0)).view.turn.step,
        TurnStep::Upkeep
    );
    (p, g)
}
fn opening(c: &Catalog, g: &Game) -> bool {
    observation(g, PlayerId(0))
        .view
        .objects
        .iter()
        .any(|o| o.oracle_id == id(c, "Jump") && o.zone == (Zone::Hand { owner: PlayerId(0) }))
}
#[test]
#[ignore = "bounded opening-only setup scan; stops before gameplay"]
fn temporary_flight_setup_only_probe() {
    let c = catalog();
    for index in 0..256 {
        let (_, g) = setup(&c, seed(index), 4096);
        if opening(&c, &g) {
            println!("opening-only index={index} seed={:?}", seed(index));
            return;
        }
    }
    panic!("no Jump opening in declared0..256");
}
fn recover(g: &Game, p: &Prepared, verifier: &mut SnapshotVerifier) {
    verifier.verify(&g.snapshot().unwrap()).unwrap();
    assert_restore(g, p);
    assert_replay(g, p);
}
// Deterministic diagnostic fork, not an opening/outcome search: passing this
// exact pre-flight priority must permit the same ordinary grounded block.
fn assert_grounded_block_control(g: &Game, q: u64) {
    let before = g.snapshot().unwrap();
    let mut control = g.fork();
    for _ in 0..16 {
        let (actor, r) = request(&control);
        if let PlayerRequest::Blockers {
            token,
            candidates,
            attackers,
            ..
        } = r
        {
            assert_eq!(actor, PlayerId(1));
            assert_eq!(attackers.len(), 1);
            let v = observation(&control, actor).view;
            let b = v
                .objects
                .iter()
                .find(|o| {
                    o.controller == actor
                        && o.zone == Zone::Battlefield
                        && o.characteristics
                            .as_ref()
                            .is_some_and(|c| c.has_type(mtg_contracts::CardKind::Creature))
                })
                .unwrap();
            assert!(candidates.contains(&b.handle));
            assert!(matches!(
                finish_query(
                    &control,
                    control.check_player_response(
                        actor,
                        token,
                        PlayerResponse::Blockers {
                            blocks: vec![(b.handle.clone(), attackers[0].clone())]
                        },
                        Budget::new(100000)
                    )
                ),
                QueryValue::Validated(_)
            ));
            assert_eq!(before, g.snapshot().unwrap());
            return;
        }
        assert!(matches!(r, PlayerRequest::Priority { .. }));
        submit_match(&mut control, PlayerResponse::Pass);
        assert!(matches!(
            settle_match(&mut control, q),
            Advance::AwaitingDecision(_)
        ));
    }
    panic!("grounded control exceeds declared16 decisions");
}
fn run(index: u32, q: u64) -> Game {
    let c = catalog();
    assert_eq!(c.schema_version, 1);
    assert_eq!(c.definitions.len(), 47);
    let jump = id(&c, "Jump");
    assert_eq!(jump, "f7518456-45ed-41d4-bd3c-5aacea28eb35");
    let source: serde_json::Value = serde_json::from_str(
        &c.definition_by_oracle_id(&jump)
            .unwrap()
            .provenance
            .source_json,
    )
    .unwrap();
    assert_eq!(
        source["oracle_text"],
        "Target creature gains flying until end of turn."
    );
    assert_eq!(source["mana_cost"], "{U}");
    assert_eq!(source["legalities"]["commander"], "legal");
    let ertai = id(&c, "Ertai, Wizard Adept");
    let isamaru = id(&c, "Isamaru, Hound of Konda");
    let island = id(&c, "Island");
    let plains = id(&c, "Plains");
    let (p, mut g) = setup(&c, seed(index), q);
    assert!(opening(&c, &g), "fixed setup must actually deal Jump");
    let source_card = p
        .config()
        .cards
        .iter()
        .find(|c| c.owner == PlayerId(0) && c.oracle_id == jump)
        .unwrap()
        .id;
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut attacked, mut cast, mut resolved, mut rejected_block) = (false, false, false, false);
    let mut pending_spell = String::new();
    let mut mana_activations = BTreeMap::<String, usize>::new();
    let mut payments = BTreeMap::<String, usize>::new();
    let mut stale_hand = None;
    let mut attack_turn = 0;
    for _ in 0..1600 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        let attacker = view.objects.iter().find(|o| {
            o.oracle_id == ertai && o.owner == PlayerId(0) && o.zone == Zone::Battlefield
        });
        let defender = view.objects.iter().find(|o| {
            o.oracle_id == isamaru && o.owner == PlayerId(1) && o.zone == Zone::Battlefield
        });
        if cast && !resolved && view.stack.is_empty() {
            assert_eq!(view.turn.step, TurnStep::DeclareAttackers);
            let a = attacker.unwrap();
            assert!(attacked && a.tapped && a.keywords.contains(&KeywordAbility::Flying));
            let link = view.combat.iter().find(|l| l.attacker == a.handle).unwrap();
            assert_eq!(link.defender, PlayerId(1));
            assert!(!link.blocked && link.blockers.is_empty());
            assert!(!defender.unwrap().tapped);
            assert!(view.effects.iter().any(|e| matches!(e, VisibleEffect::GrantKeywords { target, keywords, .. } if target.current.as_ref() == Some(&a.handle) && keywords == &[KeywordAbility::Flying])));
            assert!(
                view.objects
                    .iter()
                    .any(|o| o.oracle_id == jump
                        && o.zone == (Zone::Graveyard { owner: PlayerId(0) }))
            );
            assert_eq!(mana_activations.get(&jump), Some(&1));
            assert_eq!(payments.get(&jump), Some(&1));
            let spell = g
                .events()
                .iter()
                .find_map(|e| match e {
                    Event::SpellProposed { card, stack_id } if *card == source_card => {
                        Some(*stack_id)
                    }
                    _ => None,
                })
                .unwrap();
            assert!(g.events().iter().any(|e| matches!(e, Event::SpellCompleted { stack_id, resolved: true } if *stack_id == spell)));
            recover(&g, &p, &mut verifier);
            resolved = true;
        }
        if rejected_block
            && view
                .players
                .iter()
                .find(|p| p.player == PlayerId(1))
                .unwrap()
                .life
                == Quantity::from(39)
        {
            assert!(resolved && attacked);
            assert_eq!(view.turn.step, TurnStep::CombatDamage);
            assert_eq!(view.turn.number, attack_turn);
            assert_eq!(payments.get(&ertai), Some(&1));
            assert_eq!(payments.get(&isamaru), Some(&1));
            assert_eq!(mana_activations.get(&ertai), Some(&3));
            assert_eq!(mana_activations.get(&isamaru), Some(&1));
            let history = &view
                .commanders
                .iter()
                .find(|c| c.oracle_id == ertai)
                .unwrap()
                .history;
            assert_eq!(
                history.command_casts.get(&PlayerId(0)),
                Some(&Quantity::from(1))
            );
            assert_eq!(
                history.combat_damage.get(&PlayerId(1)),
                Some(&Quantity::from(1))
            );
            let link = view
                .combat
                .iter()
                .find(|l| l.attacker == attacker.unwrap().handle)
                .unwrap();
            assert!(!link.blocked && link.blockers.is_empty());
            assert_eq!(defender.unwrap().marked_damage, Quantity::zero());
            recover(&g, &p, &mut verifier);
            return g;
        }
        let response = match r {
            PlayerRequest::Priority { .. } => {
                let main = view.active == Some(actor)
                    && view.stack.is_empty()
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                let land_id = if actor == PlayerId(0) {
                    &island
                } else {
                    &plains
                };
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.oracle_id == *land_id
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                    })
                    .collect::<Vec<_>>();
                if actor == PlayerId(0)
                    && attacked
                    && !cast
                    && view.turn.step == TurnStep::DeclareAttackers
                    && view.stack.is_empty()
                {
                    let a = attacker.unwrap();
                    assert!(a.tapped && !a.keywords.contains(&KeywordAbility::Flying));
                    assert!(!defender.unwrap().tapped && defender.unwrap().attack_ready);
                    assert!(
                        !defender
                            .unwrap()
                            .keywords
                            .iter()
                            .any(|k| matches!(k, KeywordAbility::Flying | KeywordAbility::Reach))
                    );
                    let card = view
                        .objects
                        .iter()
                        .find(|o| o.oracle_id == jump && o.zone == (Zone::Hand { owner: actor }))
                        .unwrap();
                    assert_grounded_block_control(&g, q);
                    stale_hand = Some(card.handle.clone());
                    cast = true;
                    pending_spell = jump.clone();
                    recover(&g, &p, &mut verifier);
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else if main && actor.0 < 2 && !attacked {
                    let limit = if actor == PlayerId(0) { 3 } else { 1 };
                    if lands.len() < limit
                        && view.turn.lands_played == 0
                        && let Some(land) = view.objects.iter().find(|o| {
                            o.oracle_id == *land_id && o.zone == (Zone::Hand { owner: actor })
                        })
                    {
                        PlayerResponse::PlayLand {
                            object: land.handle.clone(),
                        }
                    } else if lands.iter().filter(|o| !o.tapped).count() >= limit
                        && let Some(commander) = view
                            .objects
                            .iter()
                            .find(|o| o.zone == (Zone::Command { owner: actor }))
                    {
                        pending_spell = commander.oracle_id.clone();
                        PlayerResponse::Cast {
                            card: commander.handle.clone(),
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let mana = &view
                    .players
                    .iter()
                    .find(|p| p.player == actor)
                    .unwrap()
                    .mana;
                let total = mana.values().fold(Quantity::zero(), |mut a, b| {
                    a.add(b);
                    a
                });
                let mut required = cost.generic.clone();
                required.add(&(cost.colored.len() as i64).into());
                if total >= required {
                    PlayerResponse::FinishMana { undo_mana: false }
                } else {
                    let land_id = if actor == PlayerId(0) {
                        &island
                    } else {
                        &plains
                    };
                    let land = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == *land_id
                                && o.zone == Zone::Battlefield
                                && o.controller == actor
                                && !o.tapped
                        })
                        .unwrap();
                    *mana_activations.entry(pending_spell.clone()).or_default() += 1;
                    PlayerResponse::ActivateMana {
                        object: land.handle.clone(),
                    }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => {
                let expected = if pending_spell == jump {
                    ManaCost {
                        generic: 0.into(),
                        colored: vec![Color::U],
                    }
                } else if pending_spell == ertai {
                    ManaCost {
                        generic: 2.into(),
                        colored: vec![Color::U],
                    }
                } else {
                    assert_eq!(pending_spell, isamaru);
                    ManaCost {
                        generic: 0.into(),
                        colored: vec![Color::W],
                    }
                };
                assert_eq!(cost, expected);
                *payments.entry(pending_spell.clone()).or_default() += 1;
                PlayerResponse::SpendMana {
                    spending: exact_spending(&cost, &available),
                }
            }
            PlayerRequest::Targets {
                token,
                candidates,
                occurrences,
                ..
            } => {
                assert!(cast && !resolved && actor == PlayerId(0));
                let a = attacker.unwrap();
                assert!(candidates.contains(&a.handle));
                assert_eq!(occurrences.len(), 1);
                assert_eq!(
                    occurrences[0].domain,
                    mtg_contracts::TargetDomain::BattlefieldCreature
                );
                for wrong in [
                    ObjectHandle("unknown".into()),
                    stale_hand.clone().unwrap(),
                    view.objects
                        .iter()
                        .find(|o| o.oracle_id == island && o.zone == Zone::Battlefield)
                        .unwrap()
                        .handle
                        .clone(),
                ] {
                    reject_input(
                        &g,
                        actor,
                        token.clone(),
                        PlayerResponse::Targets {
                            objects: vec![wrong],
                        },
                    );
                }
                reject_input(
                    &g,
                    PlayerId(2),
                    token,
                    PlayerResponse::Targets {
                        objects: vec![a.handle.clone()],
                    },
                );
                recover(&g, &p, &mut verifier);
                PlayerResponse::Targets {
                    objects: vec![a.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let ready = actor == PlayerId(0)
                    && !attacked
                    && attacker.is_some_and(|a| candidates.contains(&a.handle))
                    && defender.is_some_and(|d| !d.tapped && d.attack_ready)
                    && view.objects.iter().any(|o| {
                        o.oracle_id == island
                            && o.controller == actor
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    });
                PlayerResponse::Attackers {
                    attacks: if ready {
                        attacked = true;
                        attack_turn = view.turn.number;
                        vec![(attacker.unwrap().handle.clone(), PlayerId(1))]
                    } else {
                        vec![]
                    },
                }
            }
            PlayerRequest::Blockers {
                token,
                candidates,
                attackers,
                ..
            } => {
                if actor == PlayerId(1) && attacked {
                    assert!(resolved && !rejected_block);
                    let a = attacker.unwrap();
                    let b = defender.unwrap();
                    assert_eq!(attackers, vec![a.handle.clone()]);
                    assert!(candidates.contains(&b.handle) && !b.tapped);
                    for wrong in [
                        b.handle.clone(),
                        ObjectHandle("unknown".into()),
                        stale_hand.clone().unwrap(),
                        view.objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == plains
                                    && o.controller == actor
                                    && o.zone == Zone::Battlefield
                            })
                            .unwrap()
                            .handle
                            .clone(),
                    ] {
                        reject_input(
                            &g,
                            actor,
                            token.clone(),
                            PlayerResponse::Blockers {
                                blocks: vec![(wrong, a.handle.clone())],
                            },
                        );
                    }
                    recover(&g, &p, &mut verifier);
                    rejected_block = true;
                }
                PlayerResponse::Blockers { blocks: vec![] }
            }
            PlayerRequest::Discard {
                count,
                mut candidates,
                ..
            } => {
                candidates.sort_by_key(|h| {
                    view.objects
                        .iter()
                        .find(|o| &o.handle == h)
                        .is_some_and(|o| o.oracle_id == jump)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected paid flight pod request {other:?}"),
        };
        let action = format!("{actor:?} {response:?} turn={:?}", view.turn);
        submit_match(&mut g, response);
        let advance = settle_match(&mut g, q);
        assert!(
            matches!(advance, Advance::AwaitingDecision(_)),
            "{advance:?} after {action}"
        );
    }
    panic!("paid flight prefix exceeds declared1600 decisions");
}
#[test]
fn temporary_flight_paid_pod_evades_grounded_blocker_deals_commander_damage_and_replays() {
    // The fixed index is selected by the separate bounded opening-only probe.
    let a = run(14, 1);
    let b = run(14, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
