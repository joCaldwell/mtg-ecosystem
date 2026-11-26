//! Alternative-cost and graveyard keywords from exact frozen R5 source on
//! private boards: evoke (CR 702.74a, 118.9, 601.2b/f-h) and unearth (CR
//! 702.84a, 603.7, 614.1a). Every choice is checked for state validity and
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
