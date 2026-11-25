//! Track 3A wave W7: trigger events as pure-KIR event patterns, from exact
//! frozen R4 source on private boards. Each witness checks state validity
//! and an exact snapshot round trip; the ones driven only by accepted
//! inputs also replay from their journal.
//! Pinned CR 507.1, 508.3a, 603.2, 603.3b, 603.4, 603.6a, 603.10a, 700.4,
//! 704.5m: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{EventPattern, Tiers, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const BOGWATER_LUMARET: &str = record!("0109432d-5a2a-456f-ad39-b75cb4c73420");
const UTVARA_HELLKITE: &str = record!("05a6a571-643e-429e-8e5c-1c3f8b0dc746");
const LAGONNA_BAND_ELDER: &str = record!("60e3ccf2-63f5-4435-a752-1d82fefb55d7");
const REACH_FOR_THE_SKY: &str = record!("68930fb6-d833-40cf-8917-1a1bf60edbe3");
const FEMEREF_ENCHANTRESS: &str = record!("8b983a46-580f-4a94-8ef3-5253e503148e");
const SINISTER_MONOLITH: &str = record!("ae8003eb-9f19-4f23-be3b-0ac3ddc03a72");
const UNDEAD_AUGUR: &str = record!("c26887e1-27f4-4550-921b-53460e43c079");
const HOWLING_MINE: &str = record!("d26b27db-a567-4631-b4b6-7294222fbdd1");
const ELEMENTAL_BOND: &str = record!("d9a7e5a6-3e41-4fc6-987a-18fe1b9d67dd");

/// A two-seat board at `step` whose catalog adds `cards`, each compiled
/// from frozen source to `Executes` only.
fn board(cards: &[&str], step: TurnStep) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in cards {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen trigger witness failed: {result:?}");
        };
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-trigger-witness")
        .step(step)
        .seats(2)
}

/// Prepare a board after its permanents and hands, so their card IDs come
/// first, with six Islands in each library.
fn prepare(board: Board) -> Prepared {
    board.libraries("Island", 6).prepare()
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Answer every decision with the default (pass, no attack unless listed,
/// no blocks, triggers in offered order) until `stop` holds, checking state
/// at each trigger-ordering decision.
fn drive(g: &mut Game, attack: &[ObjectId], stop: impl Fn(&Game) -> bool) {
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if stop(g) {
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(attack.iter().map(|a| (*a, PlayerId(1))).collect()),
            Reason::Blockers => d.block(vec![]),
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
    panic!("the board never reached its stop");
}

/// A triggered ability is on top of the stack.
fn triggered(g: &Game) -> bool {
    g.state
        .rules
        .stack
        .last()
        .is_some_and(|s| matches!(s.kind, StackKind::Triggered { .. }))
}

/// Resolve to an empty stack with priority.
fn settle(g: &mut Game) {
    drive(g, &[], |g| {
        g.state.rules.stack.is_empty()
            && g.state.rules.pending_triggers.is_empty()
            && matches!(g.state.rules.frame, Frame::Priority)
    });
    checked(g);
}

fn hand(g: &Game, seat: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == Zone::Hand {
                    owner: PlayerId(seat),
                }
        })
        .count()
}

fn life(g: &Game, seat: usize) -> Quantity {
    g.state.rules.players[seat].life.clone()
}

/// `value` changed by `delta`.
fn plus(value: &Quantity, delta: i64) -> Quantity {
    let mut value = value.clone();
    value.add(&Quantity::from(delta));
    value
}

/// The intervening checks recorded for `source`, in order, as passed flags.
fn intervening(g: &Game, source: ObjectId) -> Vec<(bool, bool)> {
    g.events()
        .iter()
        .filter_map(|e| match e {
            Event::TriggerConditionChecked {
                source: s,
                stage,
                check,
                ..
            } if *s == source => Some((
                matches!(stage, InterveningStage::Match { .. }),
                check.passed(),
            )),
            _ => None,
        })
        .collect()
}

/// Howling Mine: "At the beginning of each player's draw step, if this
/// artifact is untapped, that player draws an additional card." Two Mines
/// under different controllers trigger at the active player's draw step;
/// CR 603.3b puts the active player's trigger on the stack first, so the
/// nonactive player's resolves first. Both draws go to the active player.
#[test]
fn howling_mines_trigger_in_apnap_order_for_the_active_player() {
    let prepared = prepare(
        board(&[HOWLING_MINE], TurnStep::Upkeep)
            .permanent_or_hand("Howling Mine", 0, 0, false)
            .permanent_or_hand("Howling Mine", 1, 1, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let mines = [Driver::object(&g, 0), Driver::object(&g, 1)];
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0]
            .condition
            .pattern(),
        EventPattern::StepBegins { .. }
    ));
    drive(&mut g, &[], |g| g.state.rules.stack.len() == 2);
    assert_eq!(g.state.rules.turn.step, TurnStep::Draw);
    let controllers: Vec<_> = g.state.rules.stack.iter().map(|s| s.controller).collect();
    assert_eq!(controllers, [PlayerId(0), PlayerId(1)]);
    assert_eq!(
        g.state
            .rules
            .stack
            .iter()
            .map(|s| s.source)
            .collect::<Vec<_>>(),
        mines
    );
    checked(&g);
    assert_eq!(hand(&g, 0), 1, "the draw step's own draw");
    settle(&mut g);
    assert_eq!((hand(&g, 0), hand(&g, 1)), (3, 0));
    for mine in mines {
        assert_eq!(intervening(&g, mine), [(true, true), (false, true)]);
    }
    assert_recoverable((&g).into());
}

/// Activate Icy Manipulator ("{1}, {T}: Tap target artifact, creature, or
/// land.") on `target`, paying with `land`.
fn icy(g: &mut Game, icy: ObjectId, target: ObjectId, land: u32) {
    let activate = g.next_test_decision().select(Candidate::Action {
        action: Action::ActivateAbility {
            object: icy,
            ability: 0,
        },
    });
    Driver::answer(g, u64::MAX, activate);
    let targets = g.next_test_decision().targets(vec![target]);
    Driver::answer(g, u64::MAX, targets);
    Driver::pay(g, &[land]);
}

/// CR 603.4: the Mine's "if this artifact is untapped" is checked when the
/// draw step begins and again as the ability resolves. Tapped by Icy
/// Manipulator during the upkeep, it does not trigger; tapped in response
/// to its trigger, it does nothing.
#[test]
fn howling_mine_rechecks_its_intervening_if_on_resolution() {
    for tap_before in [true, false] {
        let prepared = prepare(
            board(&[HOWLING_MINE], TurnStep::Upkeep)
                .permanent_or_hand("Howling Mine", 0, 0, false)
                .permanent_or_hand("Icy Manipulator", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false),
        );
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let (mine, manipulator) = (Driver::object(&g, 0), Driver::object(&g, 1));
        Driver::request(&mut g, u64::MAX);
        if tap_before {
            icy(&mut g, manipulator, mine, 2);
            settle(&mut g);
            assert!(g.current(mine).unwrap().tapped);
        }
        drive(&mut g, &[], |g| {
            g.state.rules.turn.step == TurnStep::Draw
                && matches!(g.state.rules.frame, Frame::Priority)
        });
        if tap_before {
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(intervening(&g, mine), [(true, false)]);
        } else {
            assert_eq!(g.state.rules.stack.len(), 1);
            checked(&g);
            icy(&mut g, manipulator, mine, 2);
            settle(&mut g);
            assert_eq!(intervening(&g, mine), [(true, true), (false, false)]);
        }
        assert_eq!(hand(&g, 0), 1, "only the draw step's own draw");
        assert_recoverable((&g).into());
    }
}

/// Bogwater Lumaret: "Whenever this creature or another creature you
/// control enters, you gain 1 life." One alternative is the source itself
/// (CR 603.6a, 201.5); casting it and then a Bears gains 2 life.
#[test]
fn bogwater_lumaret_sees_itself_and_another_creature_enter() {
    let prepared = prepare(
        board(&[BOGWATER_LUMARET], TurnStep::PrecombatMain)
            .permanent_or_hand("Bogwater Lumaret", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let start = life(&g, 0);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3]);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&start, 1));
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[4, 5]);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&start, 2));
    assert_recoverable((&g).into());
}

/// Elemental Bond: "Whenever a creature you control with power 3 or
/// greater enters, draw a card." The pattern reads the entering creature on
/// the battlefield and retains those facts as `Entered` evidence; a 2/2 or
/// an opponent's 6/6 does not trigger it.
#[test]
fn elemental_bond_reads_the_entering_creature_on_the_battlefield() {
    let mut board = board(&[ELEMENTAL_BOND], TurnStep::PrecombatMain)
        .permanent_or_hand("Elemental Bond", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, true)
        .permanent_or_hand("Colossal Dreadmaw", 0, 0, true)
        .permanent_or_hand("Colossal Dreadmaw", 1, 1, true);
    for _ in 0..8 {
        board = board.permanent_or_hand("Forest", 0, 0, false);
    }
    let mut g = Game::with_trace(prepare(board), TraceMode::Detailed);
    let before = hand(&g, 0);
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[4, 5]);
    settle(&mut g);
    assert_eq!(hand(&g, 0), before - 1, "a 2/2 draws nothing");
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[6, 7, 8, 9, 10, 11]);
    drive(&mut g, &[], triggered);
    let [entry] = g.state.rules.stack.as_slice() else {
        panic!("one entry trigger");
    };
    let StackKind::Triggered { witness, .. } = &entry.kind else {
        panic!("{entry:?}");
    };
    let dreadmaw = g.object_id(CardId(2)).unwrap();
    assert!(
        matches!(witness, TriggerEventWitness::Entered { entrant, .. }
        if entrant.object == dreadmaw && entrant.characteristics.power == Some(6.into()))
    );
    checked(&g);
    settle(&mut g);
    assert_eq!(
        hand(&g, 0),
        before - 1,
        "the Dreadmaw left the hand, its trigger drew"
    );
    assert_recoverable((&g).into());
    // CR 109.5: an opponent's 6/6 is not a creature "you control".
    let entry = ProposedEvent::Move {
        object: Driver::object(&g, 3),
        destination: Zone::Battlefield,
    };
    g.commit(entry, BatchCause::Fixture).unwrap();
    assert!(g.state.rules.pending_triggers.is_empty());
    g.validate_state().unwrap();
}

/// Femeref Enchantress and Reach for the Sky: "put into a graveyard from
/// the battlefield" is the dies event for any permanent (CR 700.4). When the
/// enchanted Bears is destroyed, the Aura follows as a state-based action
/// (CR 704.5m): its own trigger and the Enchantress's both see it, and
/// their controller orders them (CR 603.3b).
#[test]
fn enchantments_put_into_a_graveyard_trigger_their_observers() {
    let prepared = prepare(
        board(
            &[FEMEREF_ENCHANTRESS, REACH_FOR_THE_SKY],
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Femeref Enchantress", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Reach for the Sky", 0, 0, true)
        .permanent_or_hand("Murder", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 1);
    Driver::cast(&mut g, CardId(2));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[4, 5, 6, 7]);
    settle(&mut g);
    let aura = g.object_id(CardId(2)).unwrap();
    assert_eq!(g.current(aura).unwrap().attached_to, Some(bears));
    let before = hand(&g, 0);
    Driver::cast(&mut g, CardId(3));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[8, 9, 10]);
    drive(&mut g, &[], |g| g.state.rules.stack.len() == 2);
    let sources: Vec<_> = g.state.rules.stack.iter().map(|s| s.source.card).collect();
    assert!(sources.contains(&CardId(0)) && sources.contains(&CardId(2)));
    checked(&g);
    settle(&mut g);
    assert_eq!(hand(&g, 0), before - 1 + 2);
    assert_recoverable((&g).into());
}

/// Undead Augur: "Whenever this creature or another Zombie you control
/// dies, you draw a card and lose 1 life." Infest kills two Augurs and a
/// Bears together; each Augur looks back at the battlefield before the
/// deaths (CR 603.10a) and sees itself and the other: four abilities
/// trigger, none for the Bears.
#[test]
fn undead_augurs_dying_together_see_each_other() {
    let prepared = prepare(
        board(&[UNDEAD_AUGUR], TurnStep::PrecombatMain)
            .permanent_or_hand("Undead Augur", 0, 0, false)
            .permanent_or_hand("Undead Augur", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Infest", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(3));
    Driver::pay(&mut g, &[4, 5, 6]);
    drive(&mut g, &[], |g| g.state.rules.pending_triggers.len() == 4);
    checked(&g);
    let start = life(&g, 0);
    let before = hand(&g, 0);
    settle(&mut g);
    assert_eq!((hand(&g, 0), life(&g, 0)), (before + 4, plus(&start, -4)));
    assert_recoverable((&g).into());
}

/// Utvara Hellkite: "Whenever a Dragon you control attacks, create a 6/6
/// red Dragon creature token with flying." Each declared Dragon is matched
/// as it attacks (CR 508.3a); a Bears attacking beside it is not.
#[test]
fn utvara_hellkite_triggers_once_per_attacking_dragon() {
    let prepared = prepare(
        board(&[UTVARA_HELLKITE], TurnStep::BeginningCombat)
            .permanent_or_hand("Utvara Hellkite", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let attackers = [Driver::object(&g, 0), Driver::object(&g, 1)];
    drive(&mut g, &attackers, |g| !g.state.rules.stack.is_empty());
    assert_eq!(g.state.rules.stack.len(), 1);
    checked(&g);
    let tokens = g.state.rules.tokens.len();
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), tokens + 1);
    assert_recoverable((&g).into());
}

/// Sinister Monolith: "At the beginning of combat on your turn, each
/// opponent loses 1 life and you gain 1 life." Only its controller's
/// beginning of combat step triggers it (CR 507.1).
#[test]
fn sinister_monolith_drains_at_its_controllers_beginning_of_combat() {
    let prepared = prepare(
        board(&[SINISTER_MONOLITH], TurnStep::PrecombatMain)
            .permanent_or_hand("Sinister Monolith", 0, 0, false)
            .permanent_or_hand("Sinister Monolith", 1, 1, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let start = [life(&g, 0), life(&g, 1)];
    drive(&mut g, &[], |g| !g.state.rules.stack.is_empty());
    assert_eq!(g.state.rules.turn.step, TurnStep::BeginningCombat);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert_eq!(g.state.rules.stack[0].controller, PlayerId(0));
    settle(&mut g);
    assert_eq!(
        [life(&g, 0), life(&g, 1)],
        [plus(&start[0], 1), plus(&start[1], -1)]
    );
    assert_recoverable((&g).into());
}

/// Lagonna-Band Elder: "When this creature enters, if you control an
/// enchantment, you gain 3 life." CR 603.4: with only an opponent's
/// enchantment it does not trigger; with its controller's Anthem bounced in
/// response, it does nothing; with the Anthem staying, it gains 3 life.
#[test]
fn lagonna_band_elder_counts_enchantments_twice() {
    for (own, bounce) in [(false, false), (true, true), (true, false)] {
        let prepared = prepare(
            board(&[LAGONNA_BAND_ELDER], TurnStep::PrecombatMain)
                .permanent_or_hand("Lagonna-Band Elder", 0, 0, true)
                .permanent_or_hand("Glorious Anthem", u8::from(!own), u8::from(!own), false)
                .permanent_or_hand("Disperse", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false),
        );
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let start = life(&g, 0);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[3, 4, 5]);
        drive(&mut g, &[], |g| {
            g.state.rules.stack.is_empty() || triggered(g)
        });
        let source = g.object_id(CardId(0)).unwrap();
        if !own {
            settle(&mut g);
            assert_eq!(intervening(&g, source), [(true, false)]);
            assert_eq!(life(&g, 0), start);
            assert_recoverable((&g).into());
            continue;
        }
        assert_eq!(g.state.rules.stack.len(), 1);
        if bounce {
            let anthem = Driver::object(&g, 1);
            Driver::cast(&mut g, CardId(2));
            Driver::pick_object(&mut g, anthem);
            Driver::pay(&mut g, &[6, 7]);
        }
        settle(&mut g);
        assert_eq!(life(&g, 0), plus(&start, if bounce { 0 } else { 3 }));
        assert_eq!(intervening(&g, source), [(true, true), (false, !bounce)]);
        assert_recoverable((&g).into());
    }
}
