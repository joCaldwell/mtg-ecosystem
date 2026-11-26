//! Alternative-cost and graveyard keywords from exact frozen R5 source on
//! private boards: evoke (CR 702.74a, 118.9, 601.2b/f-h), unearth (CR
//! 702.84a, 603.7, 614.1a), encore (CR 702.141a, 508.1d), retrace (CR
//! 702.81a, 601.2h), dash (CR 702.109a) and escape (CR 702.138a). Every choice is checked for state validity and
//! an exact snapshot round trip; finished resolutions replay from their
//! journal.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const MULLDRIFTER: &str = record!("24d0f5e7-0d9e-4b76-900e-a7274e80312d");
const SHRIEKMAW: &str = record!("717bbb9d-0a5b-4979-a24b-9d54aac7657a");
const DREGSCAPE_ZOMBIE: &str = record!("be9d1346-4416-4ade-ae84-7a4121e0bd12");
const GRIPTIDE: &str = record!("5a6bbc45-6cb8-40ff-9f7c-7b2c5713dd78");
const GOD_PHARAOHS_STATUE: &str = record!("183954d2-7b40-4c7e-adfa-d80191ab5ad6");
const IMPULSIVE_PILFERER: &str = record!("7d9fc9e7-d80b-49c3-871c-ed25b3059ae8");
const FLAME_JAB: &str = record!("05eccdee-84f8-42d5-b79c-36d081656915");
const MARDU_SCOUT: &str = record!("c18ab011-c5e0-4174-b623-d89e4d745ce6");
const GLIMPSE_OF_FREEDOM: &str = record!("02d78e76-8151-4851-bb4e-e0a1fa98756f");

fn evoke_candidate(d: &Decision<Rules>) -> bool {
    matches!(
        d.candidates.as_slice(),
        [Candidate::AdditionalCost { id, mana }]
            if id == mtg_contracts::EVOKE_COST && mana.generic == Quantity::from(2)
    )
}

/// Pass priority and order triggers as offered until the stack is empty,
/// then check state and prove replay from the journal.
fn resolve_all(g: &mut Game) {
    for _ in 0..48 {
        let d = Driver::request(g, u64::MAX);
        if d.reason == Reason::Priority && g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => d
                .pick_one(&Candidate::Action {
                    action: Action::Pass,
                })
                .unwrap(),
            Reason::OrderTriggers => {
                checked(g);
                Answer::Order {
                    order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
                }
            }
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the stack never emptied");
}

/// Mulldrifter: "When this creature enters, draw two cards. Evoke {2}{U}."
/// Announcing evoke replaces the mana cost (CR 118.9) but not the mana
/// value; on entry both the draw and the evoke sacrifice trigger, and the
/// creature ends in its owner's graveyard after its controller draws two.
/// Declining pays {4}{U}: only the draw triggers, the evoke check fails at
/// match (CR 603.4), and the creature stays. Declining without the mana
/// for the full cost reverses the cast (CR 601.2h).
#[test]
fn mulldrifter_evoke_replaces_its_cost_and_sacrifices_it_on_entry() {
    for (evoke, lands) in [(true, 3), (false, 5), (false, 3)] {
        let mut fixture = board(&[(MULLDRIFTER, &[Cell::Evoke, Cell::SacrificeSource])])
            .permanent_or_hand("Mulldrifter", 0, 0, true);
        for _ in 0..lands {
            fixture = fixture.permanent_or_hand("Island", 0, 0, false);
        }
        let mut g = Game::with_trace(
            fixture.libraries("Island", 4).prepare(),
            TraceMode::Detailed,
        );
        let definition = g.definition(CardId(0)).unwrap();
        assert_eq!(
            definition.characteristics.mana_cost.generic,
            Quantity::from(4)
        );
        Driver::cast(&mut g, CardId(0));
        let d = expect(&mut g, 0, Reason::AdditionalCosts);
        assert!(evoke_candidate(&d), "{d:?}");
        let picks = if evoke { vec![0] } else { vec![] };
        g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
            .unwrap();
        Driver::request(&mut g, u64::MAX);
        checked(&g);
        let cost = g.state.rules.stack.last().unwrap().total_cost.clone();
        assert_eq!(
            cost.generic,
            Quantity::from(if evoke { 2 } else { 4 }),
            "evoke replaces the mana cost"
        );
        let paid: Vec<u32> = (1..=lands).collect();
        Driver::pay(&mut g, &paid);
        if !evoke && lands < 5 {
            expect(&mut g, 0, Reason::Priority);
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(zone(&g, 0), Some(Zone::Hand { owner: PlayerId(0) }));
            continue;
        }
        let drifter = g.state.rules.stack.last().unwrap().source;
        resolve_all(&mut g);
        let owner = PlayerId(0);
        assert_eq!(hand_size(&g, 0), 2, "the draw resolves either way");
        let card = g.state.rules.objects[&CardId(0)].clone();
        assert_eq!(
            card.zone,
            if evoke {
                Zone::Graveyard { owner }
            } else {
                Zone::Battlefield
            }
        );
        // CR 603.4: the evoke clause is checked at match and, when it
        // passed, again on resolution.
        let checks: Vec<(bool, bool)> = g
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::TriggerConditionChecked {
                    source,
                    stage,
                    check,
                    ..
                } if source.card == drifter.card => Some((
                    matches!(stage, InterveningStage::Match { .. }),
                    check.passed(),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            checks,
            if evoke {
                vec![(true, true), (false, true)]
            } else {
                vec![(true, false)]
            }
        );
    }
}

/// Shriekmaw: "When this creature enters, destroy target nonartifact,
/// nonblack creature. Evoke {1}{B}." The two entry triggers are ordered by
/// their controller: with the sacrifice resolving first, the destroy
/// trigger still resolves from the stack after its source has left (CR
/// 113.7a, 603.4) and its target dies.
#[test]
fn shriekmaw_evoked_destroys_its_target_after_being_sacrificed() {
    let mut g = Game::with_trace(
        board(&[(SHRIEKMAW, &[Cell::Evoke, Cell::SacrificeSource])])
            .permanent_or_hand("Shriekmaw", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .libraries("Island", 2)
            .prepare(),
        TraceMode::Detailed,
    );
    Driver::cast(&mut g, CardId(0));
    let d = expect(&mut g, 0, Reason::AdditionalCosts);
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![0]],
        },
    )
    .unwrap();
    Driver::pay(&mut g, &[1, 2]);
    // The spell resolves; both entry triggers wait for ordering and the
    // destroy trigger's target.
    let mut ordered = false;
    for _ in 0..32 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::Priority && g.state.rules.stack.is_empty() && ordered {
            break;
        }
        let answer = match &d.reason {
            Reason::Priority => d
                .pick_one(&Candidate::Action {
                    action: Action::Pass,
                })
                .unwrap(),
            Reason::OrderTriggers => {
                checked(&g);
                ordered = true;
                Answer::Order {
                    order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
                }
            }
            _ => {
                let bears = Driver::object(&g, 3);
                d.pick_one(&Candidate::Object { object: bears })
                    .unwrap_or_else(|_| panic!("unexpected decision {d:?}"))
            }
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    assert!(ordered, "both entry triggers were ordered");
    checked(&g);
    assert_recoverable((&g).into());
    assert_eq!(zone(&g, 0), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(1) }));
}

/// Pass priority (attacking with `attackers`) until `stop`, ordering
/// triggers as offered; other decisions fail the test.
fn advance(
    g: &mut Game,
    attackers: &[ObjectId],
    stop: impl Fn(&Game, &Decision<Rules>) -> bool,
) -> Decision<Rules> {
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if stop(g, &d) {
            checked(g);
            return d;
        }
        let answer = match &d.reason {
            Reason::Priority => d
                .pick_one(&Candidate::Action {
                    action: Action::Pass,
                })
                .unwrap(),
            Reason::Attackers => d.attack(attackers.iter().map(|a| (*a, PlayerId(1))).collect()),
            Reason::Blockers => d.block(vec![]),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the board never reached its stop");
}

/// Dregscape Zombie: "Unearth {B}." The graveyard card returns at sorcery
/// speed with haste, so it can attack the turn it returns, and the delayed
/// trigger exiles it at the next end step (CR 702.84a, 603.7). A card in
/// hand offers no unearth. With `bolted`, Lightning Bolt destroys the
/// returned creature: the replacement exiles it instead of putting it into
/// the graveyard (CR 614.1a), and the end-step trigger then finds nothing.
#[test]
fn dregscape_zombie_unearths_with_haste_and_is_exiled() {
    for bolted in [false, true] {
        let mut fixture = board(&[(DREGSCAPE_ZOMBIE, &[Cell::Unearth])])
            .card(
                "Dregscape Zombie",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Dregscape Zombie", 0, 0, true);
        if bolted {
            fixture = fixture
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false);
        }
        let mut g = Game::with_trace(
            fixture.libraries("Island", 4).prepare(),
            TraceMode::Detailed,
        );
        let unearth = |g: &Game, card| Action::ActivateAbility {
            object: Driver::object(g, card),
            ability: 0,
        };
        let (from_graveyard, from_hand) = (unearth(&g, 0), unearth(&g, 2));
        let offered = actions(&mut g);
        assert!(offered.contains(&from_graveyard));
        assert!(
            !offered.contains(&from_hand),
            "a hand card offers no unearth"
        );
        Driver::action(&mut g, from_graveyard);
        checked(&g);
        Driver::pay(&mut g, &[1]);
        assert_eq!(
            zone(&g, 0),
            Some(Zone::Graveyard { owner: PlayerId(0) }),
            "the card stays until the ability resolves"
        );
        resolve_all(&mut g);
        let zombie = Driver::object(&g, 0);
        assert_eq!(g.zone(zombie), Some(Zone::Battlefield));
        let exile = Zone::Exile;
        if bolted {
            Driver::cast(&mut g, CardId(3));
            Driver::pick_object(&mut g, zombie);
            Driver::pay(&mut g, &[4]);
            resolve_all(&mut g);
            assert_eq!(zone(&g, 0), Some(exile), "exiled instead of dying");
        } else {
            // CR 702.10b: haste lets it attack the turn it returned.
            let d = advance(&mut g, &[], |_, d| d.reason == Reason::Attackers);
            assert!(
                d.candidates
                    .iter()
                    .any(|c| matches!(c, Candidate::Object { object } if *object == zombie)),
                "{d:?}"
            );
            g.respond(d.player, d.id, d.attack(vec![(zombie, PlayerId(1))]))
                .unwrap();
        }
        // The delayed trigger resolves at the next end step.
        advance(&mut g, &[], |g, d| {
            d.reason == Reason::Priority
                && g.state.rules.turn.step == TurnStep::End
                && g.state.rules.stack.is_empty()
                && g.state.rules.turn.number == 1
                && zone(g, 0) == Some(Zone::Exile)
        });
        checked(&g);
        assert_recoverable((&g).into());
        assert_eq!(zone(&g, 0), Some(exile));
        if !bolted {
            assert_eq!(life(&g, 1), plus(&Quantity::from(40), -2), "it attacked");
        }
    }
}

/// Griptide: "Put target creature on top of its owner's library." An
/// opponent's Griptide on an unearthed creature is replaced by unearth's
/// exile (CR 614.1a, 702.84a): the card ends only in exile, never also on
/// top of the library, and the state restores exactly.
#[test]
fn unearthed_creature_put_on_top_of_its_library_is_exiled_instead() {
    let mut g = Game::with_trace(
        board(&[(DREGSCAPE_ZOMBIE, &[Cell::Unearth]), (GRIPTIDE, &[])])
            .card(
                "Dregscape Zombie",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Griptide", 1, 1, true)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 1, 1, false)
            .permanent_or_hand("Island", 1, 1, false)
            .libraries("Island", 4)
            .prepare(),
        TraceMode::Detailed,
    );
    let unearth = Action::ActivateAbility {
        object: Driver::object(&g, 0),
        ability: 0,
    };
    Driver::action(&mut g, unearth);
    Driver::pay(&mut g, &[1]);
    resolve_all(&mut g);
    let zombie = Driver::object(&g, 0);
    assert_eq!(g.zone(zombie), Some(Zone::Battlefield));
    let library_before = g.state.rules.libraries[&PlayerId(0)].clone();
    // Seat 0 passes; seat 1 casts Griptide at the unearthed Zombie.
    let d = expect(&mut g, 0, Reason::Priority);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Action {
            action: Action::Pass,
        })
        .unwrap(),
    )
    .unwrap();
    Driver::cast(&mut g, CardId(2));
    Driver::pick_object(&mut g, zombie);
    Driver::pay(&mut g, &[3, 4, 5, 6]);
    resolve_all(&mut g);
    assert_eq!(zone(&g, 0), Some(Zone::Exile));
    assert_eq!(
        g.state.rules.libraries[&PlayerId(0)],
        library_before,
        "the exiled card is not also on top of the library"
    );
    checked(&g);
    assert_recoverable((&g).into());
}

/// God-Pharaoh's Statue: "Spells your opponents cast cost {2} more to
/// cast." The engine quote of Mulldrifter's evoke includes the increase
/// (CR 601.2f), so with three Islands the simulator evokes without the
/// Statue and does not cast with it; an observation alone plans no evoke.
#[test]
fn simulator_plans_evoke_only_from_the_engine_quote() {
    use mtg_sim::policy::{Policy, Strategy};
    for statue in [false, true] {
        let mut fixture = board(&[(MULLDRIFTER, &[]), (GOD_PHARAOHS_STATUE, &[])])
            .permanent_or_hand("Mulldrifter", 0, 0, true);
        for _ in 0..3 {
            fixture = fixture.permanent_or_hand("Island", 0, 0, false);
        }
        if statue {
            fixture = fixture.permanent_or_hand("God-Pharaoh's Statue", 1, 1, false);
        }
        let mut g = Game::new(fixture.libraries("Island", 4).prepare());
        Driver::request(&mut g, u64::MAX);
        let o = observation(&g, 0);
        let d = o.decision.clone().unwrap();
        let cast = d
            .candidates
            .iter()
            .find(|c| {
                matches!(
                    c,
                    Candidate::Action {
                        action: Action::Cast { .. }
                    }
                )
            })
            .unwrap()
            .clone();
        let Candidate::Action {
            action: Action::Cast { card },
        } = &cast
        else {
            unreachable!()
        };
        let (identity, quote) = g
            .quote_alternative_cast_cost(o.view.viewer, card)
            .unwrap()
            .unwrap();
        assert_eq!(identity, mtg_contracts::EVOKE_COST);
        assert_eq!(quote.generic, Quantity::from(if statue { 4 } else { 2 }));
        assert_eq!(quote.colored, [Color::U]);
        let pass = Candidate::Action {
            action: Action::Pass,
        };
        let catalog = g.prepared.catalog.clone();
        // Without the engine there is no quote, so no evoke is planned.
        let mut blind = Strategy::new(&catalog, Policy::Develop, [0; 4]);
        assert_eq!(blind.choose(&o).unwrap(), d.pick_one(&pass).unwrap());
        let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
        let answer = strategy.choose_with_game(&o, &g).unwrap();
        if statue {
            assert_eq!(answer, d.pick_one(&pass).unwrap());
            continue;
        }
        assert_eq!(answer, d.pick_one(&cast).unwrap());
        Driver::cast(&mut g, CardId(0));
        Driver::request(&mut g, u64::MAX);
        let o = observation(&g, 0);
        let d = o.decision.clone().unwrap();
        assert_eq!(d.reason, Reason::AdditionalCosts);
        assert_eq!(
            strategy.choose_with_game(&o, &g).unwrap(),
            Answer::Pick {
                picks: vec![vec![0]]
            },
            "the planned cast announces evoke"
        );
    }
}

/// A synthetic combination on the archived evoke witness verifies both the
/// simulator's alternative quote and the engine's payment against that lock.
#[test]
fn evoke_combines_with_locked_keyword_payments() {
    use mtg_sim::policy::{Policy, Strategy};

    for (delve, convoke, improvise, resource) in [
        (true, false, false, "Forest"),
        (false, true, false, "Grizzly Bears"),
        (false, false, true, "Darksteel Relic"),
    ] {
        let mtg_compiler::CompileResult::Lowered { mut definition } =
            mtg_witness::compile(mtg_witness::R5_MANIFEST, MULLDRIFTER)
        else {
            panic!("invalid archived Mulldrifter");
        };
        definition.delve = delve;
        definition.convoke = convoke;
        definition.improvise = improvise;
        let mut catalog = FixtureCatalog::Witness.get().clone();
        catalog.definitions.push(*definition);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let zone = if delve {
            Zone::Graveyard { owner: PlayerId(0) }
        } else {
            Zone::Battlefield
        };
        let mut game = Game::new(
            Board::new(catalog, "evoke-keyword-payment")
                .step(TurnStep::PrecombatMain)
                .permanent_or_hand("Mulldrifter", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .card(resource, 0, 0, zone)
                .card(resource, 0, 0, zone)
                .libraries("Island", 4)
                .prepare(),
        );
        let catalog = game.prepared.catalog.clone();
        let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
        for _ in 0..8 {
            let decision = Driver::request(&mut game, u64::MAX);
            if decision.reason == Reason::Priority && !game.state.rules.stack.is_empty() {
                break;
            }
            let answer = strategy
                .choose_with_game(&observation(&game, 0), &game)
                .unwrap();
            game.respond(decision.player, decision.id, answer).unwrap();
        }
        let spell = game
            .state
            .rules
            .stack
            .last()
            .expect("simulator cast Mulldrifter");
        assert_eq!(spell.total_cost.generic, 2.into());
        assert_eq!(spell.total_cost.colored, [Color::U]);
        assert_eq!(spell.paid.as_ref(), Some(&spell.total_cost));
        for id in [2, 3] {
            let object = &game.state.rules.objects[&CardId(id)];
            if delve {
                assert_eq!(object.zone, Zone::Exile);
            } else {
                assert!(object.tapped);
            }
        }
        checked(&game);
    }
}

/// Impulsive Pilferer: "When this creature dies, create a Treasure token.
/// Encore {3}{R}." With three opponents, encore exiles the graveyard card
/// as its cost and creates three hasty token copies, each of which must
/// attack its own opponent this turn (CR 702.141a, 508.1d): a declaration
/// sending a token at another opponent, or omitting one, is illegal. A hand card
/// offers no encore. With `bolted`, Lightning Bolt kills one token first;
/// at the next end step the delayed trigger sacrifices the survivors only,
/// and every copy's own dies trigger makes a Treasure (CR 603.7, 707.2).
#[test]
fn impulsive_pilferer_encore_copies_attack_each_opponent_and_are_sacrificed() {
    for bolted in [false, true] {
        let mut fixture = board(&[(IMPULSIVE_PILFERER, &[Cell::Encore])])
            .card(
                "Impulsive Pilferer",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            )
            .permanent_or_hand("Impulsive Pilferer", 0, 0, true);
        for _ in 0..4 {
            fixture = fixture.permanent_or_hand("Mountain", 0, 0, false);
        }
        if bolted {
            fixture = fixture
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false);
        }
        let mut g = Game::with_trace(
            fixture.libraries("Island", 4).prepare(),
            TraceMode::Detailed,
        );
        let encore = |g: &Game, card| Action::ActivateAbility {
            object: Driver::object(g, card),
            ability: 0,
        };
        let (from_graveyard, from_hand) = (encore(&g, 0), encore(&g, 1));
        let offered = actions(&mut g);
        assert!(offered.contains(&from_graveyard));
        assert!(
            !offered.contains(&from_hand),
            "a hand card offers no encore"
        );
        Driver::action(&mut g, from_graveyard);
        Driver::pay(&mut g, &[2, 3, 4, 5]);
        assert_eq!(zone(&g, 0), Some(Zone::Exile), "the cost exiles the card");
        resolve_all(&mut g);
        let mut tokens: Vec<_> = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| o.id.card.0 >= 1 << 31 && o.zone == Zone::Battlefield)
            .map(|o| o.id)
            .collect();
        tokens.sort();
        assert_eq!(tokens.len(), 3, "one copy for each opponent");
        for token in &tokens {
            assert_eq!(g.state.rules.objects[&token.card].controller, PlayerId(0));
            assert_eq!(
                g.state.rules.tokens[&token.card].definition.name,
                "Impulsive Pilferer"
            );
        }
        if bolted {
            Driver::cast(&mut g, CardId(6));
            Driver::pick_object(&mut g, tokens[1]);
            Driver::pay(&mut g, &[7]);
            resolve_all(&mut g);
            assert!(g.zone(tokens[1]).is_none(), "the bolted token ceased");
        }
        let living: Vec<_> = tokens
            .iter()
            .copied()
            .filter(|t| g.zone(*t) == Some(Zone::Battlefield))
            .collect();
        let d = advance(&mut g, &[], |_, d| d.reason == Reason::Attackers);
        // Each surviving token may attack only its own opponent, and must.
        let seat = |token: &ObjectId| tokens.iter().position(|t| t == token).unwrap() as u8;
        let defender = |token: &ObjectId| PlayerId(seat(token) + 1);
        let other = |token: &ObjectId| PlayerId((seat(token) + 1) % 3 + 1);
        let wrong = d.attack(living.iter().map(|t| (*t, other(t))).collect());
        assert!(g.respond(d.player, d.id, wrong).is_err());
        assert!(g.respond(d.player, d.id, d.attack(vec![])).is_err());
        g.respond(
            d.player,
            d.id,
            d.attack(living.iter().map(|t| (*t, defender(t))).collect()),
        )
        .unwrap();
        checked(&g);
        advance(&mut g, &[], |g, d| {
            d.reason == Reason::Priority
                && g.state.rules.turn.step == TurnStep::End
                && g.state.rules.stack.is_empty()
                && tokens.iter().all(|t| g.zone(*t).is_none())
        });
        checked(&g);
        assert_recoverable((&g).into());
        for seat in [1, 3] {
            assert_eq!(life(&g, seat), plus(&Quantity::from(40), -1), "seat {seat}");
        }
        assert_eq!(
            life(&g, 2),
            plus(&Quantity::from(40), if bolted { 0 } else { -1 }),
        );
        let treasures = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && g.state
                        .rules
                        .tokens
                        .get(&o.id.card)
                        .is_some_and(|t| t.definition.characteristics.subtypes == ["Treasure"])
            })
            .count();
        assert_eq!(treasures, 3, "each copy that died made a Treasure");
    }
}

/// Flame Jab: "Flame Jab deals 1 damage to any target. Retrace." Without a
/// land card in hand the graveyard card offers no cast. With one, the cast
/// pays {R} and then discards a land card (a nonland card is not offered),
/// and the resolved spell returns to its owner's graveyard rather than
/// exile, so it can be retraced again (CR 702.81a, 601.2h).
#[test]
fn flame_jab_retraces_from_the_graveyard_by_discarding_a_land() {
    let game = |land: bool| {
        Game::with_trace(
            board(&[(FLAME_JAB, &[Cell::Retrace])])
                .card("Flame Jab", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand(if land { "Forest" } else { "Lightning Bolt" }, 0, 0, true)
                .libraries("Island", 4)
                .prepare(),
            TraceMode::Detailed,
        )
    };
    let retrace = |g: &Game| Action::Cast {
        card: Driver::object(g, 0),
    };
    let mut g = game(false);
    let offered = actions(&mut g);
    assert!(!offered.contains(&retrace(&g)), "no land card to discard");
    let mut g = game(true);
    let offered = actions(&mut g);
    assert!(offered.contains(&retrace(&g)));
    let forest = Driver::object(&g, 3);
    let cast = retrace(&g);
    Driver::action(&mut g, cast);
    let d = Driver::request(&mut g, u64::MAX);
    let target = d
        .pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap();
    g.respond(d.player, d.id, target).unwrap();
    Driver::pay(&mut g, &[1]);
    let d = expect(
        &mut g,
        0,
        Reason::ChosenObjectCost {
            action: mtg_contracts::ChosenCostAction::Discard,
        },
    );
    assert_eq!(
        d.candidates,
        vec![Candidate::Object { object: forest }],
        "only the land card can pay retrace"
    );
    checked(&g);
    g.respond(d.player, d.id, d.pick_one(&d.candidates[0]).unwrap())
        .unwrap();
    resolve_all(&mut g);
    assert_eq!(zone(&g, 3), Some(Zone::Graveyard { owner: PlayerId(0) }));
    assert_eq!(life(&g, 1), plus(&Quantity::from(40), -1));
    assert_eq!(
        zone(&g, 0),
        Some(Zone::Graveyard { owner: PlayerId(0) }),
        "a retraced spell is not exiled"
    );
    assert_eq!(zone(&g, 2), Some(Zone::Hand { owner: PlayerId(0) }));
}

/// Mardu Scout: "Dash {1}{R}." Announcing dash replaces {R}{R} with {1}{R}
/// (CR 118.9); the permanent it becomes has haste, attacks the turn it
/// arrives, and the delayed trigger returns it to its owner's hand at the
/// next end step (CR 702.109a, 603.7). Declining pays {R}{R}: the Scout has
/// no haste, is not offered as an attacker and stays on the battlefield.
#[test]
fn mardu_scout_dash_gives_haste_and_returns_it_at_end_step() {
    for dash in [true, false] {
        let mut g = Game::with_trace(
            board(&[(MARDU_SCOUT, &[Cell::Dash])])
                .permanent_or_hand("Mardu Scout", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand(if dash { "Forest" } else { "Mountain" }, 0, 0, false)
                .libraries("Island", 4)
                .prepare(),
            TraceMode::Detailed,
        );
        Driver::cast(&mut g, CardId(0));
        let d = expect(&mut g, 0, Reason::AdditionalCosts);
        assert!(
            matches!(d.candidates.as_slice(), [Candidate::AdditionalCost { id, mana }]
                if id == mtg_contracts::DASH_COST && mana.generic == Quantity::from(1)),
            "{d:?}"
        );
        let picks = if dash { vec![0] } else { vec![] };
        g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
            .unwrap();
        Driver::request(&mut g, u64::MAX);
        checked(&g);
        Driver::pay(&mut g, &[1, 2]);
        resolve_all(&mut g);
        let scout = Driver::object(&g, 0);
        assert_eq!(g.zone(scout), Some(Zone::Battlefield));
        let d = advance(&mut g, &[], |_, d| d.reason == Reason::Attackers);
        let offered = d
            .candidates
            .iter()
            .any(|c| matches!(c, Candidate::Object { object } if *object == scout));
        assert_eq!(offered, dash, "only a dashed Scout has haste");
        let attack = if dash {
            vec![(scout, PlayerId(1))]
        } else {
            vec![]
        };
        g.respond(d.player, d.id, d.attack(attack)).unwrap();
        advance(&mut g, &[], |g, d| {
            d.reason == Reason::Priority
                && g.state.rules.turn.step == TurnStep::End
                && g.state.rules.stack.is_empty()
                && (!dash || zone(g, 0) == Some(Zone::Hand { owner: PlayerId(0) }))
        });
        checked(&g);
        assert_recoverable((&g).into());
        if dash {
            assert_eq!(
                life(&g, 1),
                plus(&Quantity::from(40), -3),
                "the 3/1 attacked"
            );
            assert_eq!(zone(&g, 0), Some(Zone::Hand { owner: PlayerId(0) }));
        } else {
            assert_eq!(life(&g, 1), Quantity::from(40));
            assert_eq!(g.zone(scout), Some(Zone::Battlefield), "it stays");
        }
    }
}

/// Glimpse of Freedom: "Draw a card. Escape—{2}{U}, Exile five other cards
/// from your graveyard." With four other graveyard cards it offers no cast.
/// With six, the graveyard cast pays {2}{U} instead of {1}{U} (CR 118.9),
/// then exiles exactly five other cards, chosen one at a time; the
/// resolved spell returns to the graveyard, not exile (CR 702.138a).
#[test]
fn glimpse_of_freedom_escapes_by_exiling_five_other_cards() {
    for others in [4u32, 6] {
        let mut fixture = board(&[(GLIMPSE_OF_FREEDOM, &[Cell::Escape])]).card(
            "Glimpse of Freedom",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        );
        for _ in 0..3 {
            fixture = fixture.permanent_or_hand("Island", 0, 0, false);
        }
        for _ in 0..others {
            fixture = fixture.card("Forest", 0, 0, Zone::Graveyard { owner: PlayerId(0) });
        }
        let mut g = Game::with_trace(
            fixture.libraries("Island", 4).prepare(),
            TraceMode::Detailed,
        );
        let escape = Action::Cast {
            card: Driver::object(&g, 0),
        };
        let offered = actions(&mut g);
        if others < 5 {
            assert!(!offered.contains(&escape), "too few other cards to exile");
            continue;
        }
        assert!(offered.contains(&escape));
        Driver::action(&mut g, escape);
        Driver::request(&mut g, u64::MAX);
        assert_eq!(
            g.state.rules.stack.last().unwrap().total_cost.generic,
            Quantity::from(2),
            "the escape cost replaces the mana cost"
        );
        Driver::pay(&mut g, &[1, 2, 3]);
        for _ in 0..5 {
            let d = expect(
                &mut g,
                0,
                Reason::ChosenObjectCost {
                    action: mtg_contracts::ChosenCostAction::ExileFromGraveyard,
                },
            );
            checked(&g);
            g.respond(d.player, d.id, d.pick_one(&d.candidates[0]).unwrap())
                .unwrap();
        }
        resolve_all(&mut g);
        assert_eq!(hand_size(&g, 0), 1, "it drew a card");
        assert_eq!(
            zone(&g, 0),
            Some(Zone::Graveyard { owner: PlayerId(0) }),
            "an escaped instant is not exiled"
        );
        let exiled = (4..4 + others)
            .filter(|card| zone(&g, *card) == Some(Zone::Exile))
            .count();
        assert_eq!(exiled, 5);
    }
}

/// The Develop policy plans retrace, escape and dash casts only from the
/// engine quote, which includes God-Pharaoh's Statue's increase for its
/// controller's opponents (CR 601.2f). With the Statue none is affordable
/// and the policy passes; without it each is cast (and dash announced). An
/// observation alone plans no graveyard cast and no dash.
#[test]
fn simulator_plans_graveyard_casts_and_dash_only_from_the_engine_quote() {
    use mtg_sim::policy::{Policy, Strategy};
    // (record, card, zone, lands on the battlefield, quoted generic without/with the Statue)
    let cases: [(&str, &str, bool, &[&str], i64); 3] = [
        (FLAME_JAB, "Flame Jab", true, &["Mountain"], 0),
        (
            GLIMPSE_OF_FREEDOM,
            "Glimpse of Freedom",
            true,
            &["Island", "Island", "Island"],
            2,
        ),
        (
            MARDU_SCOUT,
            "Mardu Scout",
            false,
            &["Mountain", "Forest"],
            1,
        ),
    ];
    for (record, name, graveyard, lands, generic) in cases {
        for statue in [false, true] {
            let zone = if graveyard {
                Zone::Graveyard { owner: PlayerId(0) }
            } else {
                Zone::Hand { owner: PlayerId(0) }
            };
            let mut fixture = board(&[(record, &[]), (GOD_PHARAOHS_STATUE, &[])])
                .card(name, 0, 0, zone)
                // A land card to discard; the policy may play another first.
                .permanent_or_hand("Plains", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, true);
            for land in lands {
                fixture = fixture.permanent_or_hand(land, 0, 0, false);
            }
            for _ in 0..5 {
                fixture = fixture.card("Swamp", 0, 0, Zone::Graveyard { owner: PlayerId(0) });
            }
            if statue {
                fixture = fixture.permanent_or_hand("God-Pharaoh's Statue", 1, 1, false);
            }
            let mut g = Game::new(fixture.libraries("Island", 4).prepare());
            let catalog = g.prepared.catalog.clone();
            let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
            let pass = Candidate::Action {
                action: Action::Pass,
            };
            let (answer, d) = loop {
                let request = Driver::request(&mut g, u64::MAX);
                let o = observation(&g, 0);
                let d = o.decision.clone().unwrap();
                assert_eq!(d.reason, Reason::Priority);
                let answer = strategy.choose_with_game(&o, &g).unwrap();
                let land = |a: &Answer| {
                    matches!(a, Answer::Pick { picks } if matches!(
                        d.candidate(picks[0][0]),
                        Ok(Candidate::Action { action: Action::PlayLand { .. } })
                    ))
                };
                if land(&answer) {
                    g.respond(request.player, request.id, answer).unwrap();
                    continue;
                }
                // An observation alone plans none of these casts.
                let mut blind = Strategy::new(&catalog, Policy::Develop, [0; 4]);
                let guess = blind.choose(&o).unwrap();
                assert!(
                    land(&guess) || guess == d.pick_one(&pass).unwrap(),
                    "{name}: {guess:?}"
                );
                break (answer, d);
            };
            let cast = d
                .candidates
                .iter()
                .find(|c| {
                    matches!(
                        c,
                        Candidate::Action {
                            action: Action::Cast { .. }
                        }
                    )
                })
                .unwrap()
                .clone();
            let Candidate::Action {
                action: Action::Cast { card },
            } = &cast
            else {
                unreachable!()
            };
            let quote = if graveyard {
                g.quote_cast_cost(PlayerId(0), card, None).unwrap()
            } else {
                g.quote_alternative_cast_cost(PlayerId(0), card)
                    .unwrap()
                    .unwrap()
                    .1
            };
            let increase = if statue { 2 } else { 0 };
            assert_eq!(quote.generic, Quantity::from(generic + increase), "{name}");
            if statue {
                assert_eq!(answer, d.pick_one(&pass).unwrap(), "{name} with the Statue");
                continue;
            }
            assert_eq!(answer, d.pick_one(&cast).unwrap(), "{name}");
        }
    }
}
