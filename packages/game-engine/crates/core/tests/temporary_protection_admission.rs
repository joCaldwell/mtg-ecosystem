//! Genuine 100-card pod prefix: paid protection above opposing Royal destruction,
//! cleanup expiry and a fresh destruction. No hand/mana/board state injection.
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
        let names = match seat {
            0 => vec![
                ("Yargle and Multani", 1),
                ("Withstand Death", 1),
                ("Forest", 49),
                ("Swamp", 49),
            ],
            1 => vec![
                ("Yargle and Multani", 1),
                ("Royal Assassin", 1),
                ("Swamp", 98),
            ],
            _ => vec![("Isamaru, Hound of Konda", 1), ("Plains", 99)],
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
        mtg_contracts::sha256(format!("temporary-protection-admission/opening/{index}").as_bytes());
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
    let a = observation(g, PlayerId(0)).view;
    let b = observation(g, PlayerId(1)).view;
    let hand_count = |v: &PermittedView, name: &str| {
        v.objects
            .iter()
            .filter(|o| o.oracle_id == id(c, name) && o.zone == (Zone::Hand { owner: v.viewer }))
            .count()
    };
    hand_count(&a, "Withstand Death") == 1
        && hand_count(&a, "Forest") >= 2
        && hand_count(&a, "Swamp") >= 2
        && hand_count(&b, "Royal Assassin") == 1
}
#[test]
#[ignore = "bounded opening-only setup scan; stops before gameplay"]
fn temporary_protection_setup_only_probe() {
    let c = catalog();
    for index in 0..4096 {
        let (_, g) = setup(&c, seed(index), 4096);
        if opening(&c, &g) {
            println!("opening-only index={index} seed={:?}", seed(index));
            return;
        }
    }
    panic!("no acceptable opening in declared0..4096");
}
fn recover(g: &Game, p: &Prepared, verifier: &mut SnapshotVerifier) {
    verifier.verify(&g.snapshot().unwrap()).unwrap();
    assert_restore(g, p);
    assert_replay(g, p);
}
fn run(index: u32, q: u64) -> Game {
    let c = catalog();
    assert_eq!(c.definitions.len(), 47);
    let (p, mut g) = setup(&c, seed(index), q);
    assert!(opening(&c, &g));
    let protection = id(&c, "Withstand Death");
    let royal = id(&c, "Royal Assassin");
    let yargle = id(&c, "Yargle and Multani");
    let forest = id(&c, "Forest");
    let swamp = id(&c, "Swamp");
    let source_card = p
        .config()
        .cards
        .iter()
        .find(|c| c.owner == PlayerId(0) && c.oracle_id == protection)
        .unwrap()
        .id;
    let target_card = CardId(0);
    let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
    let (mut commander_cast, mut royal_cast, mut protection_cast) = (false, false, false);
    let (mut attacked, mut first, mut second, mut survived, mut expired, mut returned) =
        (false, false, false, false, false, false);
    let mut attack_turn = 0;
    let mut payments = BTreeMap::<String, usize>::new();
    let mut mana_activations = BTreeMap::<String, usize>::new();
    let mut pending_spell = String::new();
    let mut stale_hand = None;
    let mut initial_target = None;
    for _ in 0..3000 {
        let (actor, r) = request(&g);
        let view = observation(&g, actor).view;
        let target = view.objects.iter().find(|o| {
            o.owner == PlayerId(0) && o.oracle_id == yargle && o.zone == Zone::Battlefield
        });
        let source = view.objects.iter().find(|o| {
            o.owner == PlayerId(1) && o.oracle_id == royal && o.zone == Zone::Battlefield
        });
        if first && !survived && view.stack.is_empty() {
            let target = target.expect("target survived actual destruction");
            assert!(target.tapped && target.keywords.contains(&KeywordAbility::Indestructible));
            assert!(protection_cast);
            assert!(
                view.objects.iter().any(|o| o.oracle_id == protection
                    && o.zone == (Zone::Graveyard { owner: PlayerId(0) }))
            );
            assert!(view.effects.iter().any(|e|matches!(e,VisibleEffect::GrantKeywords {target:t,keywords,..}
                if t.current.as_ref()==Some(&target.handle)&&keywords==&[KeywordAbility::Indestructible])));
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed {actual:EventOutcome::DestructionPrevented {object},..} if object.card==target_card)).count(),1);
            assert_eq!(payments.get(&protection), Some(&1));
            assert_eq!(mana_activations.get(&protection), Some(&1));
            assert!(
                g.events()
                    .iter()
                    .any(|e| matches!(e,Event::SpellProposed {card,..} if *card==source_card))
            );
            recover(&g, &p, &mut verifier);
            survived = true;
        }
        if survived && !expired && view.turn.number > attack_turn {
            let target = target.expect("same creature survives real cleanup");
            assert!(!target.keywords.contains(&KeywordAbility::Indestructible));
            assert_eq!(target.marked_damage, Quantity::zero());
            assert!(
                !view
                    .effects
                    .iter()
                    .any(|e| matches!(e, VisibleEffect::GrantKeywords { .. }))
            );
            assert_eq!(Some(g.object_id(target_card).unwrap()), initial_target);
            recover(&g, &p, &mut verifier);
            expired = true;
        }
        if returned && view.stack.is_empty() {
            assert!(survived && expired && second);
            assert!(view.objects.iter().any(|o| o.owner == PlayerId(0)
                && o.oracle_id == yargle
                && o.zone == (Zone::Command { owner: PlayerId(0) })));
            assert!(g.events().iter().any(|e|matches!(e,Event::Committed {actual:EventOutcome::Moved {movement},..}
                if movement.previous.card==target_card&&movement.destination==(Zone::Graveyard {owner:PlayerId(0)}))));
            assert_eq!(payments.get(&yargle), Some(&1));
            assert_eq!(payments.get(&royal), Some(&1));
            assert_eq!(mana_activations.get(&yargle), Some(&6));
            assert_eq!(mana_activations.get(&royal), Some(&3));
            assert_eq!(g.events().iter().filter(|e|matches!(e,Event::Committed {actual:EventOutcome::DestructionPrevented {object},..} if object.card==target_card)).count(),1);
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
                let lands = view
                    .objects
                    .iter()
                    .filter(|o| {
                        o.zone == Zone::Battlefield
                            && o.controller == actor
                            && (o.oracle_id == forest || o.oracle_id == swamp)
                    })
                    .collect::<Vec<_>>();
                if actor == PlayerId(0) && first && !protection_cast && view.stack.len() == 1 {
                    assert!(view.stack[0].cast_complete && view.stack[0].controller == PlayerId(1));
                    let target = target.unwrap();
                    assert!(target.tapped);
                    assert!(source.unwrap().tapped);
                    assert_eq!(
                        view.stack[0].targets[0].current.as_ref(),
                        Some(&target.handle)
                    );
                    let card = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.oracle_id == protection && o.zone == (Zone::Hand { owner: actor })
                        })
                        .unwrap();
                    stale_hand = Some(card.handle.clone());
                    pending_spell = protection.clone();
                    protection_cast = true;
                    recover(&g, &p, &mut verifier);
                    PlayerResponse::Cast {
                        card: card.handle.clone(),
                    }
                } else if actor == PlayerId(1)
                    && ((attacked && !first) || (expired && !second))
                    && view.stack.is_empty()
                    && source.is_some_and(|o| !o.tapped && o.attack_ready)
                {
                    assert!(target.unwrap().tapped);
                    if first {
                        second = true;
                    } else {
                        first = true;
                    }
                    recover(&g, &p, &mut verifier);
                    PlayerResponse::ActivateAbility {
                        object: source.unwrap().handle.clone(),
                        ability: 0,
                    }
                } else if main && actor.0 < 2 && !attacked {
                    let swamps = lands.iter().filter(|o| o.oracle_id == swamp).count();
                    let wanted = if actor == PlayerId(1) || swamps < 2 {
                        &swamp
                    } else {
                        &forest
                    };
                    let limit = if actor == PlayerId(0) { 7 } else { 3 };
                    if lands.len() < limit && view.turn.lands_played == 0 {
                        let card = view.objects.iter().find(|o| {
                            &o.oracle_id == wanted && o.zone == (Zone::Hand { owner: actor })
                        });
                        if let Some(card) = card {
                            PlayerResponse::PlayLand {
                                object: card.handle.clone(),
                            }
                        } else {
                            PlayerResponse::Pass
                        }
                    } else if actor == PlayerId(0)
                        && !commander_cast
                        && lands.iter().filter(|o| !o.tapped).count() >= 6
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == yargle && o.zone == (Zone::Command { owner: actor })
                            })
                            .unwrap();
                        commander_cast = true;
                        pending_spell = yargle.clone();
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
                        }
                    } else if actor == PlayerId(1)
                        && !royal_cast
                        && lands.iter().filter(|o| !o.tapped).count() >= 3
                    {
                        let card = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == royal && o.zone == (Zone::Hand { owner: actor })
                            })
                            .unwrap();
                        royal_cast = true;
                        pending_spell = royal.clone();
                        PlayerResponse::Cast {
                            card: card.handle.clone(),
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
                let mut required = cost.generic.clone();
                required.add(&(cost.colored.len() as i64).into());
                let total = mana.values().fold(Quantity::zero(), |mut a, b| {
                    a.add(b);
                    a
                });
                if total >= required {
                    PlayerResponse::FinishMana { undo_mana: false }
                } else {
                    // Supply every colored requirement first; generic mana then
                    // comes from a real unused basic, never the activation source.
                    let needed = cost.colored.iter().find(|color| {
                        let n = cost.colored.iter().filter(|c| *c == *color).count() as i64;
                        mana.get(*color).cloned().unwrap_or_default() < n.into()
                    });
                    let preferred = needed.map(|c| if *c == Color::G { &forest } else { &swamp });
                    let land = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.controller == actor
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && (o.oracle_id == forest || o.oracle_id == swamp)
                                && preferred.is_none_or(|id| &o.oracle_id == id)
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
                let expected = if pending_spell == protection {
                    ManaCost {
                        generic: 0.into(),
                        colored: vec![Color::G],
                    }
                } else if pending_spell == royal {
                    ManaCost {
                        generic: 1.into(),
                        colored: vec![Color::B, Color::B],
                    }
                } else {
                    ManaCost {
                        generic: 3.into(),
                        colored: vec![Color::B, Color::B, Color::G],
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
                let target = target.unwrap();
                assert!(target.tapped && candidates.contains(&target.handle));
                assert_eq!(occurrences.len(), 1);
                if actor == PlayerId(0) {
                    assert!(protection_cast && !payments.contains_key(&protection));
                    assert_eq!(
                        occurrences[0].domain,
                        mtg_contracts::TargetDomain::BattlefieldCreature
                    );
                    for wrong in [
                        ObjectHandle("unknown".into()),
                        stale_hand.clone().unwrap(),
                        view.objects
                            .iter()
                            .find(|o| {
                                o.controller == actor
                                    && o.oracle_id == forest
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
                            PlayerResponse::Targets {
                                objects: vec![wrong],
                            },
                        );
                    }
                    reject_input(
                        &g,
                        PlayerId(2),
                        token.clone(),
                        PlayerResponse::Targets {
                            objects: vec![target.handle.clone()],
                        },
                    );
                }
                initial_target = Some(g.object_id(target_card).unwrap());
                recover(&g, &p, &mut verifier);
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let ready = target.is_some_and(|o| candidates.contains(&o.handle))
                    && source.is_some_and(|o| !o.tapped && o.attack_ready)
                    && view.objects.iter().any(|o| {
                        o.oracle_id == forest
                            && o.controller == PlayerId(0)
                            && o.zone == Zone::Battlefield
                            && !o.tapped
                    });
                let attacks = if actor == PlayerId(0) && !attacked && ready {
                    attacked = true;
                    attack_turn = view.turn.number;
                    vec![(target.unwrap().handle.clone(), PlayerId(1))]
                } else {
                    vec![]
                };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                assert!(survived && recipients.len() == 1);
                PlayerResponse::Damage {
                    assignments: vec![(recipients[0].clone(), power)],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                assert!(second && expired);
                assert_eq!(actor, PlayerId(0));
                assert!(initial_target.is_some());
                returned = true;
                PlayerResponse::CommanderReturn { to_command: true }
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
                        .is_some_and(|o| o.oracle_id == protection || o.oracle_id == royal)
                });
                PlayerResponse::Discard {
                    objects: candidates.into_iter().take(count).collect(),
                }
            }
            other => panic!("unexpected paid protection pod request {other:?}"),
        };
        let action = format!("{actor:?} {response:?} turn={:?}", view.turn);
        submit_match(&mut g, response);
        let advance = settle_match(&mut g, q);
        assert!(
            matches!(advance, Advance::AwaitingDecision(_)),
            "{advance:?} after {action}"
        );
    }
    panic!("paid protection prefix exceeds declared3000 decisions");
}
#[test]
fn temporary_protection_paid_pod_survives_destruction_expires_and_fresh_destruction_replays() {
    // First qualifying opening in the separate declared0..4096 setup-only probe.
    assert_eq!(
        seed(126),
        [
            234, 97, 219, 101, 207, 210, 156, 214, 231, 138, 251, 62, 241, 179, 240, 82, 221, 52,
            108, 155, 15, 78, 146, 14, 43, 172, 204, 150, 9, 141, 153, 219
        ]
    );
    let a = run(126, 1);
    let b = run(126, 4096);
    assert_eq!(a.state_digest().unwrap(), b.state_digest().unwrap());
    assert_eq!(a.replay_journal(), b.replay_journal());
}
