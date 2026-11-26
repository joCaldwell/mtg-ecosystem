//! Counted target occurrences and "target player or planeswalker" from
//! exact frozen R5 source. One printed occurrence announces exactly N, or up
//! to N, distinct objects in one pick; battlefield removal of the group is one
//! simultaneous event; a member that left before resolution is skipped and
//! the spell is countered only when every member is illegal.
//! Pinned CR 115.1, 115.1d, 115.3, 120.3, 601.2c, 608.2b, 903.9b:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

macro_rules! frozen {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

/// "Destroy two target nonblack creatures. You lose 5 life."
const WICKED_PACT: &str = frozen!("39e21a5a-b278-478a-854c-17695c0f6246");
/// "Return up to two target creatures to their owners' hands."
const INTO_THE_VOID: &str = frozen!("4d5a970b-dd36-4871-a2aa-36ba21917cdf");
/// "Two target creatures each get +2/+2 until end of turn."
const SYMBIOSIS: &str = frozen!("787b494b-731b-4e99-8e46-157bc86f6eea");
/// "Lava Spike deals 3 damage to target player or planeswalker."
const LAVA_SPIKE: &str = frozen!("2837888c-bfa8-4955-9334-0605ad409f7e");

/// The one group slot of a counted occurrence: its pick bounds.
fn group_slot(d: &Decision<Rules>) -> (u32, u32) {
    assert!(
        matches!(&d.reason, Reason::Targets { specs } if specs.len() == 1),
        "a counted occurrence is one decision slot: {d:?}"
    );
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("{d:?}")
    };
    let [slot] = slots.as_slice() else {
        panic!("{d:?}")
    };
    (slot.min, slot.max)
}

fn life(g: &Game, seat: u8) -> Quantity {
    g.state.rules.players[seat as usize].life.clone()
}

fn castable(g: &mut Game, card: u32) -> bool {
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority, "{d:?}");
    let object = Driver::object(g, card);
    d.candidates.contains(&Candidate::Action {
        action: Action::Cast { card: object },
    })
}

fn wicked_pact(seats: u8) -> Game {
    let prepared = board(&[(
        WICKED_PACT,
        &[Cell::RemoveTargetGroup, Cell::LifeController],
        Tiers::EXECUTES,
    )])
    .seats(seats)
    .permanent_or_hand("Wicked Pact", 0, 0, true)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .permanent_or_hand("Llanowar Elves", seats - 1, seats - 1, false)
    .permanent_or_hand("Typhoid Rats", 1, 1, false)
    .prepare();
    Game::with_trace(prepared, TraceMode::Detailed)
}

/// Exactly two distinct nonblack creatures are announced in one pick; the
/// same creature cannot fill both (CR 115.3, 601.2c). Both are destroyed
/// and the controller loses 5 life.
#[test]
fn wicked_pact_announces_two_distinct_targets_and_destroys_both() {
    let mut g = wicked_pact(2);
    let (bears, elves, rats) = (
        Driver::object(&g, 4),
        Driver::object(&g, 5),
        Driver::object(&g, 6),
    );
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(group_slot(&d), (2, 2));
    assert!(offers(&d, bears) && offers(&d, elves));
    assert!(!offers(&d, rats), "nonblack");
    let twice = d.index(&Candidate::Object { object: bears }).unwrap();
    assert!(
        g.respond(
            d.player,
            d.id,
            Answer::Pick {
                picks: vec![vec![twice, twice]]
            }
        )
        .is_err(),
        "one occurrence never names an object twice"
    );
    let single = Answer::Pick {
        picks: vec![vec![twice]],
    };
    assert!(g.respond(d.player, d.id, single).is_err(), "exactly two");
    answer_targets(&mut g, &d, &[bears, elves]);
    Driver::pay(&mut g, &[1, 2, 3]);
    checked(&g);
    settle(&mut g);
    assert_eq!(zone(&g, 4), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, 5), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, 6), Zone::Battlefield);
    let mut paid = before;
    paid.add(&Quantity::from(-5));
    assert_eq!(life(&g, 0), paid);
}

/// With one legal nonblack creature the spell cannot be cast: the
/// announcement needs two distinct legal objects (CR 601.2c).
#[test]
fn wicked_pact_is_not_castable_with_one_legal_target() {
    let prepared = board(&[(WICKED_PACT, &[Cell::RemoveTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("Wicked Pact", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Typhoid Rats", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    assert!(!castable(&mut g, 0));
}

/// One target leaves before resolution: the other is still destroyed and
/// the untargeted life loss happens. When both leave, the spell is
/// countered and nothing happens (CR 608.2b).
#[test]
fn wicked_pact_skips_a_departed_target_and_fizzles_when_both_depart() {
    for both in [false, true] {
        let mut g = wicked_pact(4);
        let (bears, elves) = (Driver::object(&g, 4), Driver::object(&g, 5));
        let before = life(&g, 0);
        Driver::cast(&mut g, CardId(0));
        let d = targets(&mut g);
        answer_targets(&mut g, &d, &[bears, elves]);
        Driver::pay(&mut g, &[1, 2, 3]);
        g.concede(PlayerId(3)).unwrap();
        if both {
            g.concede(PlayerId(1)).unwrap();
        }
        let d = Driver::request(&mut g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        assert!(g.current(elves).is_err());
        assert_eq!(g.current(bears).is_err(), both);
        checked(&g);
        settle(&mut g);
        assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
        let mut expected = before;
        if !both {
            assert_eq!(zone(&g, 4), Zone::Graveyard { owner: PlayerId(1) });
            expected.add(&Quantity::from(-5));
        }
        assert_eq!(life(&g, 0), expected, "both={both}");
    }
}

fn into_the_void(commander: bool) -> Game {
    let mut board = board(&[(INTO_THE_VOID, &[Cell::RemoveTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("Into the Void", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Isamaru, Hound of Konda", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false);
    if commander {
        board = board.commander(CardId(5));
    }
    Game::with_trace(board.prepare(), TraceMode::Detailed)
}

/// "Up to two": zero targets is a legal announcement and the spell
/// resolves without being countered (CR 115.1d, 601.2c, 608.2b).
#[test]
fn into_the_void_may_announce_zero_targets() {
    let mut g = into_the_void(false);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(group_slot(&d), (0, 2));
    answer_targets(&mut g, &d, &[]);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
    assert_eq!(zone(&g, 5), Zone::Battlefield);
    assert_eq!(zone(&g, 6), Zone::Battlefield);
}

/// Both creatures return in one simultaneous move; the commander's owner
/// may choose the command zone instead, and that pending choice validates
/// and recovers (CR 903.9b).
#[test]
fn into_the_void_returns_both_and_offers_the_commander_zone() {
    for to_command in [true, false] {
        let mut g = into_the_void(true);
        let (isamaru, bears) = (Driver::object(&g, 5), Driver::object(&g, 6));
        Driver::cast(&mut g, CardId(0));
        let d = targets(&mut g);
        answer_targets(&mut g, &d, &[isamaru, bears]);
        Driver::pay(&mut g, &[1, 2, 3, 4]);
        let d = pass_until_choice(&mut g);
        assert!(
            matches!(&d.reason, Reason::CommanderReturn { destination, .. }
                if *destination == Zone::Hand { owner: PlayerId(0) }),
            "{d:?}"
        );
        assert_eq!(zone(&g, 6), Zone::Battlefield, "one simultaneous move");
        checked(&g);
        assert_recoverable((&g).into());
        let flag = if to_command {
            Flag::CommandZone
        } else {
            Flag::Destination
        };
        g.respond(
            d.player,
            d.id,
            d.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        settle(&mut g);
        assert_eq!(
            zone(&g, 5),
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Hand { owner: PlayerId(0) }
            }
        );
        assert_eq!(zone(&g, 6), Zone::Hand { owner: PlayerId(1) });
    }
}

/// Each member gets its own pump; when one member leaves before
/// resolution, the other is still pumped (CR 608.2b).
#[test]
fn symbiosis_pumps_the_remaining_target_after_one_departs() {
    let prepared = board(&[(SYMBIOSIS, &[Cell::PumpTarget], Tiers::EXECUTES)])
        .seats(3)
        .permanent_or_hand("Symbiosis", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 2, 2, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (mine, theirs) = (Driver::object(&g, 3), Driver::object(&g, 4));
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(group_slot(&d), (2, 2));
    answer_targets(&mut g, &d, &[mine, theirs]);
    Driver::pay(&mut g, &[1, 2]);
    g.concede(PlayerId(2)).unwrap();
    assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
    assert!(g.current(theirs).is_err());
    checked(&g);
    settle(&mut g);
    let pumped = g.characteristics(mine).unwrap();
    assert_eq!(
        (pumped.power, pumped.toughness),
        (Some(4.into()), Some(4.into()))
    );
}

/// "Target player or planeswalker": no planeswalker is admitted, so only
/// players are offered, the caster included (CR 115.1, 120.3).
#[test]
fn lava_spike_offers_players_and_damages_the_chosen_one() {
    let prepared = board(&[(LAVA_SPIKE, &[Cell::DamageTarget], Tiers::EXECUTES)])
        .permanent_or_hand("Lava Spike", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let before = life(&g, 1);
    Driver::cast(&mut g, CardId(0));
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    checked(&g);
    assert!(
        d.candidates
            .iter()
            .all(|c| matches!(c, Candidate::Player { .. }))
    );
    assert!(d.candidates.contains(&Candidate::Player {
        player: PlayerId(0)
    }));
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap(),
    )
    .unwrap();
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    let mut hit = before;
    hit.add(&Quantity::from(-3));
    assert_eq!(life(&g, 1), hit);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
}

/// The simultaneous group destruction leaves an indestructible member on
/// the battlefield and destroys the other (CR 702.12b, 701.8); the
/// untargeted life loss still happens.
#[test]
fn wicked_pact_group_spares_an_indestructible_member() {
    let prepared = board(&[(WICKED_PACT, &[Cell::RemoveTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("Wicked Pact", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Darksteel Myr", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (bears, myr) = (Driver::object(&g, 4), Driver::object(&g, 5));
    let before = life(&g, 0);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    answer_targets(&mut g, &d, &[bears, myr]);
    Driver::pay(&mut g, &[1, 2, 3]);
    settle(&mut g);
    assert_eq!(zone(&g, 4), Zone::Graveyard { owner: PlayerId(1) });
    assert_eq!(zone(&g, 5), Zone::Battlefield);
    let mut paid = before;
    paid.add(&Quantity::from(-5));
    assert_eq!(life(&g, 0), paid);
}
