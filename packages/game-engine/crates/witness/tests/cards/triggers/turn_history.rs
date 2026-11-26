//! Frozen R5 witnesses for trigger ordinals, the once-each-turn limit and
//! the life-gain, draw and sacrifice events counted in the turn ledger
//! (CR 119.9, 121.2, 400.7, 601.2i, 603.2, 603.2c, 603.10a, 701.21a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MONK: &str = record!("2d3998fc-a68a-4923-a456-b3e43ab6decb");
const MORBID_OPPORTUNIST: &str = record!("322f44f0-e6da-4ee0-b474-e7d5e9a461c5");
const AJANIS_PRIDEMATE: &str = record!("95e94dea-5ac0-4d6f-adec-ca147aee861f");
const MAZIREK: &str = record!("e0420f2c-d578-421e-ae75-e7dc5f70661a");
const FAERIE_MASTERMIND: &str = record!("a984db23-40ea-428d-829f-e944267280f8");

fn counters(g: &Game, object: ObjectId) -> Quantity {
    g.current(object).unwrap().plus_one_counters.clone()
}

/// Cast Giant Growth `card` on `target`, paid by Forest `land`, and resolve
/// it and anything it triggered.
fn grow(g: &mut Game, card: u32, target: ObjectId, land: u32) {
    Driver::cast(g, CardId(card));
    Driver::pick_object(g, target);
    Driver::pay(g, &[land]);
    settle(g);
}

fn spells_this_turn(g: &Game, seat: usize) -> usize {
    g.state.rules.cast_history[seat].spells.len()
}

/// Monk of the Open Hand: "Whenever you cast your second spell each turn,
/// put a +1/+1 counter on this creature." Only the second spell triggers;
/// the ledger survives a mid-turn snapshot and must agree with the cast
/// flags it sits beside.
#[test]
fn turn_history_second_spell_triggers_once_and_survives_restore() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MONK], TurnStep::PrecombatMain)
                .permanent_or_hand("Monk of the Open Hand", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let monk = Driver::object(&g, 0);
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::Cast { nth: Some(ordinal), .. } if ordinal.nth == 2 && !ordinal.any_player
    ));
    grow(&mut g, 1, monk, 4);
    assert_eq!(counters(&g, monk), 0.into());
    assert_eq!(spells_this_turn(&g, 0), 1);

    // A mid-turn snapshot keeps the ledger; one that drops it is rejected.
    checked(&g);
    let mut forged: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    forged.state.rules.cast_history[0].spells.clear();
    assert_forgery_rejected(&g, forged, "seating/table invariant");
    let snapshot = g.snapshot().unwrap();
    let mut g = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();

    grow(&mut g, 2, monk, 5);
    assert_eq!(counters(&g, monk), 1.into());
    grow(&mut g, 3, monk, 6);
    assert_eq!(counters(&g, monk), 1.into());
    assert_eq!(spells_this_turn(&g, 0), 3);
}

/// A spell cast before the Monk entered still counts (CR 603.2): the Monk
/// is the second spell, so the spell after it is the third and the Monk,
/// which was not on the battlefield as the second spell was cast, never
/// triggers.
#[test]
fn turn_history_counts_spells_cast_before_the_source_entered() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MONK], TurnStep::PrecombatMain)
                .permanent_or_hand("Monk of the Open Hand", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let bears = Driver::object(&g, 1);
    grow(&mut g, 2, bears, 5);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[4]);
    settle(&mut g);
    let monk = Driver::object(&g, 0);
    assert_eq!(g.current(monk).unwrap().zone, Zone::Battlefield);
    grow(&mut g, 3, monk, 6);
    assert_eq!(spells_this_turn(&g, 0), 3);
    assert_eq!(counters(&g, monk), 0.into());
    assert_recoverable((&g).into());
}

/// The ledger resets as the next turn begins: on the opponent's turn the
/// Monk's controller casts a first and then a second spell, and only the
/// second triggers.
#[test]
fn turn_history_resets_at_the_next_turn() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MONK], TurnStep::PrecombatMain)
                .permanent_or_hand("Monk of the Open Hand", 0, 0, false)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Giant Growth", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let monk = Driver::object(&g, 0);
    let turn = g.state.rules.turn.number;
    grow(&mut g, 1, monk, 5);
    grow(&mut g, 2, monk, 6);
    assert_eq!(counters(&g, monk), 1.into());
    drive(&mut g, &[], |g| {
        g.state.rules.turn.number == turn + 1
            && g.state.rules.turn.step == TurnStep::Upkeep
            && g.state.rules.priority == PlayerId(0)
            && g.state.rules.stack.is_empty()
    });
    assert_eq!(spells_this_turn(&g, 0), 0);
    checked(&g);
    grow(&mut g, 3, monk, 7);
    assert_eq!(counters(&g, monk), 1.into());
    // Priority returns to the active opponent, who passes it back.
    drive(&mut g, &[], |g| g.state.rules.priority == PlayerId(0));
    grow(&mut g, 4, monk, 8);
    assert_eq!(counters(&g, monk), 2.into());
    assert_recoverable((&g).into());
}

/// Morbid Opportunist: "Whenever one or more other creatures die, draw a
/// card. This ability triggers only once each turn." Infest kills two
/// creatures at once and the ability triggers once (CR 603.2c); a later
/// death the same turn does not trigger it again.
#[test]
fn turn_history_once_each_turn_limits_simultaneous_and_later_deaths() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MORBID_OPPORTUNIST], TurnStep::PrecombatMain)
                .permanent_or_hand("Morbid Opportunist", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Vampire Nighthawk", 1, 1, false)
                .permanent_or_hand("Infest", 0, 0, true)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    assert!(
        g.definition(CardId(0)).unwrap().triggered[0]
            .condition
            .once_each_turn()
    );
    let nighthawk = Driver::object(&g, 3);
    let before = hand(&g, 0);
    Driver::cast(&mut g, CardId(4));
    Driver::pay(&mut g, &[6, 7, 8]);
    drive(&mut g, &[], |g| triggered(g));
    assert_eq!(g.state.rules.stack.len(), 1);
    assert_eq!(g.state.rules.trigger_uses.len(), 1);
    checked(&g);
    settle(&mut g);
    // Infest left the hand and the trigger drew one card.
    assert_eq!(hand(&g, 0), before);
    assert_eq!(g.current(nighthawk).unwrap().zone, Zone::Battlefield);

    Driver::cast(&mut g, CardId(5));
    Driver::pick_object(&mut g, nighthawk);
    Driver::pay(&mut g, &[9]);
    settle(&mut g);
    assert!(g.current(nighthawk).is_err());
    assert_eq!(hand(&g, 0), before - 1);
    assert_recoverable((&g).into());
}

/// Ajani's Pridemate: "Whenever you gain life, put a +1/+1 counter on this
/// creature." Soul's Attendant's gain is one life-gain event (CR 119.9).
#[test]
fn turn_history_life_gain_event_triggers() {
    let mut g = Game::with_trace(
        prepare(
            board(&[AJANIS_PRIDEMATE], TurnStep::PrecombatMain)
                .permanent_or_hand("Ajani's Pridemate", 0, 0, false)
                .permanent_or_hand("Soul's Attendant", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let pridemate = Driver::object(&g, 0);
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[3, 4]);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 1));
    assert_eq!(counters(&g, pridemate), 1.into());
    assert_eq!(g.state.rules.cast_history[0].life_gains, 1);
    assert_recoverable((&g).into());
}

/// Faerie Mastermind: "Whenever an opponent draws their second card each
/// turn, you draw a card." Sign in Blood makes the opponent draw two; only
/// the second draw triggers, and the controller's own draw counts in their
/// own row.
#[test]
fn turn_history_opponent_second_draw_triggers() {
    let mut g = Game::with_trace(
        prepare(
            board(&[FAERIE_MASTERMIND], TurnStep::PrecombatMain)
                .permanent_or_hand("Faerie Mastermind", 0, 0, false)
                .permanent_or_hand("Sign in Blood", 0, 0, true)
                .permanent_or_hand("Swamp", 0, 0, false)
                .permanent_or_hand("Swamp", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (mine, theirs) = (hand(&g, 0), hand(&g, 1));
    Driver::cast(&mut g, CardId(1));
    let d = Driver::request(&mut g, u64::MAX);
    let answer = d
        .pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    Driver::pay(&mut g, &[2, 3]);
    settle(&mut g);
    assert_eq!(hand(&g, 1), theirs + 2);
    assert_eq!(hand(&g, 0), mine);
    assert_eq!(g.state.rules.cast_history[1].draws, 2);
    assert_eq!(g.state.rules.cast_history[0].draws, 1);
    assert_recoverable((&g).into());
}

/// Mazirek: "Whenever a player sacrifices another permanent, put a +1/+1
/// counter on each creature you control." Innocent Blood's two sacrifices
/// are two events, each matched as the permanent last existed on the
/// battlefield (CR 603.10a).
#[test]
fn turn_history_each_sacrifice_triggers() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MAZIREK], TurnStep::PrecombatMain)
                .permanent_or_hand("Mazirek, Kraul Death Priest", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false)
                .permanent_or_hand("Innocent Blood", 0, 0, true)
                .permanent_or_hand("Swamp", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let mazirek = Driver::object(&g, 0);
    let bears = Driver::object(&g, 1);
    Driver::cast(&mut g, CardId(3));
    Driver::pay(&mut g, &[4]);
    for _ in 0..64 {
        let d = Driver::request(&mut g, u64::MAX);
        if g.state.rules.stack.len() == 2 && triggered(&g) {
            break;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            // Each player's forced choice: the controller keeps Mazirek.
            _ => {
                let bears = Candidate::Object { object: bears };
                let pick = if d.candidates.contains(&bears) {
                    bears
                } else {
                    d.candidates[0].clone()
                };
                d.pick_one(&pick).unwrap()
            }
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    settle(&mut g);
    assert_eq!(counters(&g, mazirek), 2.into());
    assert_recoverable((&g).into());
}
