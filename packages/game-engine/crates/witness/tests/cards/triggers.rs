//! Track 3A wave W7: trigger events as pure-KIR event patterns, from exact
//! frozen R5 source on private boards. Each witness checks state validity
//! and an exact snapshot round trip; the ones driven only by accepted
//! inputs also replay from their journal.
//! Pinned CR 507.1, 508.3a, 603.2, 603.3b, 603.4, 603.6a, 603.10a, 700.4,
//! 704.5m: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{EventPattern, Tiers, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

mod discard_events;
mod event_alternatives;
mod intervening_history;
mod keyword_triggers;
mod object_events;
mod observed_events;
mod resolution_triggers;
mod turn_history;
mod upkeep_keywords;

const BOGWATER_LUMARET: &str = record!("0109432d-5a2a-456f-ad39-b75cb4c73420");
const UTVARA_HELLKITE: &str = record!("05a6a571-643e-429e-8e5c-1c3f8b0dc746");
const LAGONNA_BAND_ELDER: &str = record!("60e3ccf2-63f5-4435-a752-1d82fefb55d7");
const REACH_FOR_THE_SKY: &str = record!("68930fb6-d833-40cf-8917-1a1bf60edbe3");
const FEMEREF_ENCHANTRESS: &str = record!("8b983a46-580f-4a94-8ef3-5253e503148e");
const SINISTER_MONOLITH: &str = record!("ae8003eb-9f19-4f23-be3b-0ac3ddc03a72");
const UNDEAD_AUGUR: &str = record!("c26887e1-27f4-4550-921b-53460e43c079");
const HOWLING_MINE: &str = record!("d26b27db-a567-4631-b4b6-7294222fbdd1");
const ELEMENTAL_BOND: &str = record!("d9a7e5a6-3e41-4fc6-987a-18fe1b9d67dd");
const SHATTERED_ANGEL: &str = record!("e339c6cb-c75d-4dd4-8c1a-78f844983242");
const IMPACT_TREMORS: &str = record!("9242cd3e-1a71-4700-8182-9c1005616033");
const GRISTLE_GRINNER: &str = record!("419f4648-2ace-4bd4-9ae4-8d456688c089");
const CONVALESCENT_CARE: &str = record!("162407d9-3e3c-440f-a7a9-22936d99fc8a");
const GIXIAN_SKULLFLAYER: &str = record!("0b2a59a3-cf87-48fe-99e2-4c2063119b55");
const GRAVE_TITAN: &str = record!("f3abd4d1-a975-4e85-8684-aa0fce029670");
const ICHOR_WELLSPRING: &str = record!("5b5ef43b-13fd-4461-8d2d-18be65e9a790");
const IVORY_CRANE_NETSUKE: &str = record!("ab151429-0890-4231-a3a7-34e8455dac70");

/// A two-seat board at `step` whose catalog adds `cards`, each compiled
/// from frozen source to `Executes` only.
fn board(cards: &[&str], step: TurnStep) -> Board {
    board_with(compiled(cards, true), step)
}

/// The frozen `cards`, each `Executes` only when `executes_only`.
fn compiled(cards: &[&str], executes_only: bool) -> Vec<Definition> {
    cards
        .iter()
        .map(|raw| {
            let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
            let mtg_compiler::CompileResult::Lowered { definition } = result else {
                panic!("frozen trigger witness failed: {result:?}");
            };
            let tiers = definition_tiers(&definition).unwrap();
            assert!(!executes_only || tiers == Tiers::EXECUTES);
            *definition
        })
        .collect()
}

/// A two-seat board at `step` whose catalog adds `definitions`.
fn board_with(definitions: Vec<Definition>, step: TurnStep) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.extend(definitions);
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
            Reason::OptionalEffect => d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
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

/// Shattered Angel: "Whenever a land an opponent controls enters, you may
/// gain 3 life." The entering land's controller concedes with the trigger
/// on the stack. The frozen entry facts are those of the event, when that
/// player was in the game (CR 800.4a), so state validation, restore and
/// replay keep the trigger, and it resolves for its surviving controller.
#[test]
fn shattered_angel_keeps_its_trigger_when_the_lands_controller_concedes() {
    let prepared = prepare(
        board(&[SHATTERED_ANGEL], TurnStep::PostcombatMain)
            .seats(3)
            .permanent_or_hand("Shattered Angel", 0, 0, false)
            .permanent_or_hand("Forest", 1, 1, true),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let forest = Driver::object(&g, 1);
    let start = life(&g, 0);
    drive(&mut g, &[], |g| {
        g.state.rules.turn_owner == PlayerId(1)
            && g.state.rules.turn.step == TurnStep::PrecombatMain
            && matches!(g.state.rules.frame, Frame::Priority)
    });
    Driver::action(&mut g, Action::PlayLand { object: forest });
    drive(&mut g, &[], triggered);
    assert!(matches!(
        &g.state.rules.stack[0].kind,
        StackKind::Triggered {
            witness: TriggerEventWitness::Entered { .. },
            ..
        }
    ));
    checked(&g);
    g.concede(PlayerId(1)).unwrap();
    checked(&g);
    // The departed player is still a seat of this game; a seat that never
    // existed is not.
    let mut forged = g.fork();
    let StackKind::Triggered {
        witness: TriggerEventWitness::Entered { entrant, .. },
        ..
    } = &mut forged.state.rules.stack[0].kind
    else {
        panic!("expected an entry witness");
    };
    entrant.controller = PlayerId(255);
    assert!(forged.validate_state().is_err());
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&start, 3));
    assert_recoverable((&g).into());
}

/// A private observer that draws a card whenever an enchantment deals
/// damage to an opponent. Its source predicate never names a creature.
fn enchantment_damage_observer(base: &Definition) -> Definition {
    let mut d = base.clone();
    d.oracle_id = "ffffffff-ffff-4fff-8fff-0000000000b4".into();
    d.name = "Private Tremor Observer".into();
    let you = PlayerExpr::you(Context::Triggered);
    d.triggered[0].condition = TriggerCondition::on(kir::EventPattern::DealsDamage {
        source: kir::Pred(vec![kir::Atom::Type {
            kind: CardKind::Enchantment,
        }]),
        combat: false,
        recipient: PlayerExpr::EachOpponent,
    });
    d.triggered[0].program.instructions = vec![Instruction::Kernel {
        node: kir::Node::Do(Verb::Draw {
            player: you,
            amount: Amount::fixed(1),
        }),
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d.validate().unwrap();
    assert_eq!(definition_tiers(&d).unwrap(), Tiers::EXECUTES);
    d
}

/// CR 510.1: only creatures deal combat damage, so only a combat damage
/// witness requires a creature source. Impact Tremors ("Whenever a creature
/// you control enters, this enchantment deals 1 damage to each opponent")
/// deals noncombat damage as an enchantment; an admitted observer of
/// enchantment damage triggers, validates, restores and replays.
#[test]
fn noncreature_damage_sources_validate_for_an_admitted_observer() {
    let tremors = compiled(&[IMPACT_TREMORS], false).remove(0);
    let observer = enchantment_damage_observer(&tremors);
    let prepared = prepare(
        board_with(vec![tremors, observer], TurnStep::PrecombatMain)
            .permanent_or_hand("Impact Tremors", 0, 0, false)
            .permanent_or_hand("Private Tremor Observer", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = hand(&g, 0);
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[3, 4]);
    drive(&mut g, &[], |g| {
        g.state
            .rules
            .stack
            .iter()
            .any(|s| s.source.card == CardId(1))
    });
    checked(&g);
    let observed = g
        .state
        .rules
        .stack
        .iter()
        .filter(|s| s.source.card == CardId(1))
        .count();
    assert!(observed > 0);
    settle(&mut g);
    assert!(hand(&g, 0) >= before - 1 + observed);
    assert_recoverable((&g).into());
}

/// An observer of damage from "a source other than" itself (CR 609.7) reads
/// only the source's identity, so damage from a spell on the stack or from
/// an ability whose source has left the battlefield still triggers it.
fn other_source_damage_observer(base: &Definition) -> Definition {
    let mut d = enchantment_damage_observer(base);
    d.triggered[0].condition = TriggerCondition::on(kir::EventPattern::DealsDamage {
        source: kir::Pred(vec![kir::Atom::IsNot {
            object: kir::Ref::Source,
        }]),
        combat: false,
        recipient: PlayerExpr::EachOpponent,
    });
    d.capabilities = d.requirements();
    d.validate().unwrap();
    d
}

/// Lightning Bolt: "Lightning Bolt deals 3 damage to any target." Its
/// damage comes from the spell on the stack.
#[test]
fn another_sources_spell_damage_triggers_an_identity_observer() {
    let tremors = compiled(&[IMPACT_TREMORS], false).remove(0);
    let prepared = prepare(
        board_with(
            vec![other_source_damage_observer(&tremors)],
            TurnStep::PrecombatMain,
        )
        .permanent_or_hand("Private Tremor Observer", 0, 0, false)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = hand(&g, 0);
    let opponent = life(&g, 1);
    Driver::cast(&mut g, CardId(1));
    let d = Driver::request(&mut g, u64::MAX);
    let a = d
        .pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap();
    g.respond(d.player, d.id, a).unwrap();
    Driver::pay(&mut g, &[2]);
    drive(&mut g, &[], triggered);
    checked(&g);
    settle(&mut g);
    assert_eq!(plus(&life(&g, 1), 3), opponent);
    // Bolt left the hand; the observer drew one card.
    assert_eq!(hand(&g, 0), before);
    assert_recoverable((&g).into());
}

/// Impact Tremors is returned by Disperse ("Return target nonland permanent
/// to its owner's hand.") before its trigger resolves; the damage it still
/// deals (CR 113.7a) triggers the observer.
#[test]
fn a_departed_sources_damage_triggers_an_identity_observer() {
    let tremors = compiled(&[IMPACT_TREMORS], false).remove(0);
    let observer = other_source_damage_observer(&tremors);
    let prepared = prepare(
        board_with(vec![tremors, observer], TurnStep::PrecombatMain)
            .permanent_or_hand("Impact Tremors", 0, 0, false)
            .permanent_or_hand("Private Tremor Observer", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Disperse", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let opponent = life(&g, 1);
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[4, 5]);
    drive(&mut g, &[], triggered);
    let tremors = Driver::object(&g, 0);
    Driver::cast(&mut g, CardId(3));
    Driver::pick_object(&mut g, tremors);
    Driver::pay(&mut g, &[6, 7]);
    // Stop at the observer's trigger, or at an empty stack if it never
    // triggers.
    drive(&mut g, &[], |g| {
        g.state
            .rules
            .stack
            .iter()
            .any(|s| s.source.card == CardId(1))
            || (g.state.rules.stack.is_empty()
                && g.state.rules.pending_triggers.is_empty()
                && matches!(g.state.rules.frame, Frame::Priority))
    });
    assert!(
        g.state
            .rules
            .stack
            .iter()
            .any(|s| s.source.card == CardId(1)),
        "the observer did not trigger"
    );
    checked(&g);
    // That Tremors never occupied the stack: a frozen Stack source for its
    // damage is impossible (CR 608.3).
    let mut forged = g.fork();
    let StackKind::Triggered {
        witness: TriggerEventWitness::DamageDealt { damage, .. },
        ..
    } = &mut forged
        .state
        .rules
        .stack
        .iter_mut()
        .find(|s| s.source.card == CardId(1))
        .unwrap()
        .kind
    else {
        panic!("expected a damage witness");
    };
    damage.source.zone = Zone::Stack;
    assert!(forged.validate_state().is_err());
    settle(&mut g);
    assert_eq!(plus(&life(&g, 1), 1), opponent);
    // The returned Impact Tremors and one draw per damaged opponent.
    assert_eq!(hand(&g, 0), g.live_players().len());
    assert_recoverable((&g).into());
}

/// CR 603.10a: only the reviewed opponent-death row freezes a dying
/// creature's controller. Gristle Grinner ("Whenever a creature dies, Gristle
/// Grinner gets +2/+2 until end of turn") does not, so a snapshot whose
/// pending Grinner trigger names one is rejected, as it is on the stack.
#[test]
fn a_forged_death_controller_on_a_pending_trigger_is_rejected() {
    let prepared = prepare(
        board_with(compiled(&[GRISTLE_GRINNER], false), TurnStep::PrecombatMain)
            .permanent_or_hand("Gristle Grinner", 0, 0, false)
            .permanent_or_hand("Gristle Grinner", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Murder", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(3));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[4, 5, 6]);
    for _ in 0..16 {
        let d = Driver::request(&mut g, u64::MAX);
        if d.reason == Reason::OrderTriggers {
            break;
        }
        let pass = mtg_witness::temporary::pass(&d);
        g.respond(d.player, d.id, pass).unwrap();
    }
    assert_eq!(g.state.rules.pending_triggers.len(), 2);
    checked(&g);
    let raw = g.snapshot().unwrap();
    let mut forged: Snapshot = strict_read(&raw).unwrap();
    let mut pending = serde_json::to_value(&forged.state.rules.pending_triggers[0]).unwrap();
    pending["death_controller"] = serde_json::json!(PlayerId(1));
    forged.state.rules.pending_triggers[0] = serde_json::from_value(pending).unwrap();
    forged.state_sha256 = digest(&forged.state).unwrap();
    let error = Game::restore(g.prepared.as_ref().clone(), &canonical(&forged).unwrap())
        .err()
        .unwrap();
    assert!(error.contains("death LKI binding"), "{error}");
}

/// CR 603.2c, 603.6a, 603.10a: entry and death use the same printed
/// ability, each on the source's own incarnation.
#[test]
fn lane_d_wellspring_uses_one_ability_for_entry_and_death() {
    let prepared = prepare(
        board(&[ICHOR_WELLSPRING], TurnStep::PrecombatMain)
            .permanent_or_hand("Ichor Wellspring", 0, 0, true)
            .permanent_or_hand("Shatter", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let definition = g.definition(CardId(0)).unwrap();
    let [ability] = definition.triggered.as_slice() else {
        panic!("one printed trigger");
    };
    assert!(matches!(
        ability.condition.pattern(),
        EventPattern::AnyOf { .. }
    ));

    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3]);
    drive(&mut g, &[], triggered);
    let source = Driver::object(&g, 0);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(matches!(
        &g.state.rules.stack[0].kind,
        StackKind::Triggered {
            ability: EffectiveTriggerAbility::Intrinsic { index: 0 },
            witness: TriggerEventWitness::Moved { movement },
            ..
        } if movement.successor == source && movement.destination == Zone::Battlefield
    ));
    checked(&g);
    assert_eq!(hand(&g, 0), 1);

    // The death and entry triggers for this incarnation can coexist.
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, source);
    Driver::pay(&mut g, &[4, 5]);
    drive(&mut g, &[], |g| {
        triggered(g) && g.state.rules.stack.len() == 2
    });
    let StackKind::Triggered {
        ability: EffectiveTriggerAbility::Intrinsic { index: 0 },
        witness: TriggerEventWitness::Moved { movement },
        ..
    } = &g.state.rules.stack[1].kind
    else {
        panic!("same printed trigger on death");
    };
    assert_eq!(movement.previous, source);
    assert_eq!(movement.destination, Zone::Graveyard { owner: PlayerId(0) });
    checked(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    let StackKind::Triggered { witness, .. } = &mut forged.state.rules.stack[1].kind else {
        unreachable!()
    };
    *witness = TriggerEventWitness::Tapped {
        object: source,
        controller: PlayerId(0),
    };
    assert_forgery_rejected(&g, forged, "no matching alternative");

    for (slot, different_path, error) in [
        (1, false, "duplicate trigger occurrence"),
        (1, true, "duplicate trigger movement"),
        (0, true, "duplicate trigger movement"),
    ] {
        let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
        let mut duplicate = forged.state.rules.stack[slot].clone();
        let StackKind::Triggered {
            trigger_id,
            event: TriggerOccurrenceId::Batch(occurrence),
            ..
        } = &mut duplicate.kind
        else {
            unreachable!()
        };
        trigger_id.0 = forged.state.rules.next_trigger_id;
        duplicate.id.0 = forged.state.rules.next_stack_id;
        forged.state.rules.next_trigger_id += 1;
        forged.state.rules.next_stack_id += 1;
        if different_path {
            occurrence.path.push(0);
        }
        forged.state.rules.stack.push(duplicate);
        assert_forgery_rejected(&g, forged, error);
    }

    settle(&mut g);
    assert_eq!(hand(&g, 0), 2);
    assert_recoverable((&g).into());
}

/// CR 508.3a, 603.2c: the attack branch observes just the declared source,
/// even when multiple creatures are declared in the same occurrence.
#[test]
fn lane_d_grave_titan_matches_its_own_attack_once() {
    let prepared = prepare(
        board(&[GRAVE_TITAN], TurnStep::BeginningCombat)
            .permanent_or_hand("Grave Titan", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
    );
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let attackers = [Driver::object(&g, 0), Driver::object(&g, 1)];
    drive(&mut g, &attackers, triggered);
    assert_eq!(g.state.rules.stack.len(), 1);
    assert!(matches!(
        &g.state.rules.stack[0].kind,
        StackKind::Triggered {
            ability: EffectiveTriggerAbility::Intrinsic { index: 0 },
            witness: TriggerEventWitness::CreatureAttacked { attacker, .. },
            ..
        } if attacker.object == attackers[0]
    ));
    checked(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    forged.state.rules.stack[0].controller = PlayerId(1);
    assert_forgery_rejected(&g, forged, "invalid self-attack trigger controller");
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 2);
    assert_recoverable((&g).into());
}

/// CR 603.4, 608.2a: playing a spell in response changes the hand size and
/// makes a formerly true condition fail at resolution.
#[test]
fn lane_d_netsuke_rechecks_hand_size_on_resolution() {
    let mut setup = board(&[IVORY_CRANE_NETSUKE], TurnStep::Untap)
        .permanent_or_hand("Ivory Crane Netsuke", 0, 0, false)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false);
    for _ in 0..6 {
        setup = setup.permanent_or_hand("Island", 0, 0, true);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    let netsuke = Driver::object(&g, 0);
    let before = life(&g, 0);
    drive(&mut g, &[], triggered);
    assert_eq!(hand(&g, 0), 7);
    checked(&g);
    Driver::cast(&mut g, CardId(1));
    let target = g.next_test_decision().select(Candidate::Player {
        player: PlayerId(1),
    });
    Driver::answer(&mut g, u64::MAX, target);
    Driver::pay(&mut g, &[2]);
    settle(&mut g);
    assert_eq!(hand(&g, 0), 6);
    assert_eq!(life(&g, 0), before);
    assert_eq!(intervening(&g, netsuke), [(true, true), (false, false)]);
    assert_recoverable((&g).into());
}

/// The exact upper life threshold includes 5 and excludes 6 (CR 603.4).
#[test]
fn lane_d_convalescent_care_checks_its_controllers_life() {
    for total in [5, 6] {
        let prepared = prepare(
            board(&[CONVALESCENT_CARE], TurnStep::Untap)
                .permanent_or_hand("Convalescent Care", 0, 0, false)
                .life(PlayerId(0), total.into()),
        );
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let care = Driver::object(&g, 0);
        drive(&mut g, &[], |g| g.state.rules.turn.step == TurnStep::Upkeep);
        if total == 5 {
            checked(&g);
            settle(&mut g);
            assert_eq!(intervening(&g, care), [(true, true), (false, true)]);
            assert_eq!(life(&g, 0), 8.into());
            assert_eq!(hand(&g, 0), 1);
        } else {
            assert_eq!(intervening(&g, care), [(true, false)]);
            assert!(g.state.rules.stack.is_empty());
            assert_eq!(life(&g, 0), 6.into());
            assert_eq!(hand(&g, 0), 0);
        }
        assert_recoverable((&g).into());
    }
}

/// CR 404, 603.4: other graveyards and other card types cannot satisfy the
/// threshold. Snapshots must carry the right kind and a possible count.
#[test]
fn lane_d_skullflayer_checks_only_creature_cards_in_its_controllers_graveyard() {
    for creatures in [2, 3] {
        let mut setup = board(&[GIXIAN_SKULLFLAYER], TurnStep::Untap)
            .permanent_or_hand("Gixian Skullflayer", 0, 0, false)
            .card(
                "Grizzly Bears",
                1,
                1,
                Zone::Graveyard { owner: PlayerId(1) },
            )
            .card("Island", 0, 0, Zone::Graveyard { owner: PlayerId(0) });
        for _ in 0..creatures {
            setup = setup.card(
                "Grizzly Bears",
                0,
                0,
                Zone::Graveyard { owner: PlayerId(0) },
            );
        }
        let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
        let source = Driver::object(&g, 0);
        drive(&mut g, &[], |g| g.state.rules.turn.step == TurnStep::Upkeep);
        assert_eq!(triggered(&g), creatures == 3);
        if creatures == 3 {
            checked(&g);
            for (observed, passed) in [
                (ControllerMeasurement::GraveyardCount(2), true),
                (ControllerMeasurement::GraveyardCount(5), true),
                (ControllerMeasurement::GraveyardCount(100), true),
                (ControllerMeasurement::HandCount(3), true),
                (ControllerMeasurement::GraveyardCount(2), false),
            ] {
                let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
                let StackKind::Triggered {
                    match_condition, ..
                } = &mut forged.state.rules.stack[0].kind
                else {
                    unreachable!();
                };
                *match_condition = Some(InterveningCheck::ControllerComparison {
                    controller: PlayerId(0),
                    observed,
                    passed,
                });
                assert_forgery_rejected(&g, forged, "invalid trigger match condition proof");
            }
            settle(&mut g);
        }
        assert_eq!(
            g.current(source).unwrap().plus_one_counters,
            i64::from(creatures - 2).into()
        );
        assert_recoverable((&g).into());
    }
}

fn assert_forgery_rejected(g: &Game, mut forged: Snapshot, expected: &str) {
    forged.state_sha256 = digest(&forged.state).unwrap();
    let error = Game::read_snapshot(g.prepared.as_ref().clone(), &canonical(&forged).unwrap())
        .err()
        .expect("forged snapshot rejected");
    assert!(error.contains(expected), "{error}");
}
