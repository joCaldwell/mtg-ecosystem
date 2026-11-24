//! Track 3A wave W5 control: archived "you may", "unless [player] pays" and
//! "you may pay ... If you do" triggered abilities resolving in private
//! configurations, driven only by accepted decisions, with state validation
//! and an exact snapshot round trip at every choice.
//! Pinned CR 118.12, 118.12a, 603.3, 608.2c-d:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const LIFE: i64 = 20;

/// Cards are numbered in order: `(name, controller, in hand)`.
fn prepared(cards: &[(&str, u8, bool)]) -> Prepared {
    let mut board = Board::new(
        FixtureCatalog::Witness.get().clone(),
        "w5-control-private-witness",
    )
    .step(TurnStep::PrecombatMain)
    .seats(2);
    for seat in 0..2 {
        board = board.life(PlayerId(seat), LIFE.into());
    }
    for (name, seat, hand) in cards {
        board = board.permanent_or_hand(name, *seat, *seat, *hand);
    }
    board.libraries("Forest", 4).prepare()
}

/// State validation and an exact snapshot round trip at a boundary.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn hand(g: &Game, seat: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(seat) }))
        .count()
}

fn life(g: &Game, seat: u8) -> Quantity {
    g.state.rules.players[usize::from(seat)].life.clone()
}

/// Pass priority until the next decision that is not priority.
fn next_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..16 {
        let d = Driver::request(g, u64::MAX);
        if d.reason != Reason::Priority {
            return d;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no choice within 16 priority passes")
}

/// The pending decision is `player`'s optional effect; answer it.
fn choose(g: &mut Game, player: u8, accept: bool) {
    let d = next_choice(g);
    assert_eq!(
        (d.player, &d.reason),
        (PlayerId(player), &Reason::OptionalEffect),
        "{d:?}"
    );
    checked(g);
    let flag = if accept { Flag::Accept } else { Flag::Decline };
    g.respond(d.player, d.id, d.pick_one(&Candidate::Flag { flag }).unwrap())
        .unwrap();
}

/// Seat 0 casts Grizzly Bears (card 0) with its two Forests (cards 1-2).
fn cast_bears(g: &mut Game) {
    Driver::cast(g, CardId(0));
    Driver::pay(g, &[1, 2]);
}

/// Resolve the stack to an empty main phase, passing every priority.
fn settle(g: &mut Game) {
    while !g.state.rules.stack.is_empty() {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    checked(g);
}

/// Rhystic Study: "Whenever an opponent casts a spell, you may draw a card
/// unless that player pays {1}." The caster decides the payment first; only
/// unpaid does the controller choose to draw (CR 118.12a, 608.2d).
#[test]
fn rhystic_study_offers_the_caster_payment_then_the_controller_draw() {
    for draw in [true, false] {
        let mut g = Game::new(prepared(&[
            ("Grizzly Bears", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Rhystic Study", 1, false),
        ]));
        cast_bears(&mut g);
        let before = hand(&g, 1);
        choose(&mut g, 0, false);
        choose(&mut g, 1, draw);
        settle(&mut g);
        assert_eq!(hand(&g, 1), before + usize::from(draw));
    }
}

/// Soul's Attendant: "Whenever another creature enters, you may gain 1 life."
#[test]
fn souls_attendant_gains_life_only_when_accepted() {
    for accept in [true, false] {
        let mut g = Game::new(prepared(&[
            ("Grizzly Bears", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Soul's Attendant", 1, false),
        ]));
        cast_bears(&mut g);
        choose(&mut g, 1, accept);
        settle(&mut g);
        assert_eq!(life(&g, 1), Quantity::from(LIFE + i64::from(accept)));
    }
}

/// Wooden Sphere: "Whenever a player casts a green spell, you may pay {1}.
/// If you do, you gain 1 life." Declining pays nothing and gains nothing
/// (CR 118.12, 608.2c).
#[test]
fn wooden_sphere_gains_life_only_after_its_controller_pays() {
    let mut g = Game::new(prepared(&[
        ("Grizzly Bears", 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Wooden Sphere", 1, false),
        ("Forest", 1, false),
    ]));
    cast_bears(&mut g);
    choose(&mut g, 1, false);
    settle(&mut g);
    assert_eq!(life(&g, 1), Quantity::from(LIFE));
    assert!(!g.current(Driver::object(&g, 4)).unwrap().tapped);
}

/// Paying Wooden Sphere's {1} with a Forest, then gaining the life.
#[test]
fn wooden_sphere_paid_with_a_forest_gains_one_life() {
    let mut g = Game::new(prepared(&[
        ("Grizzly Bears", 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Wooden Sphere", 1, false),
        ("Forest", 1, false),
    ]));
    cast_bears(&mut g);
    choose(&mut g, 1, true);
    let forest = Driver::object(&g, 4);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.player, PlayerId(1));
    checked(&g);
    let source = d
        .index(&Candidate::Action {
            action: Action::ActivateMana { object: forest },
        })
        .unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pay {
            pay: PayAnswer::Activate { source },
        },
    )
    .unwrap();
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    g.complete_test_mana();
    settle(&mut g);
    assert!(g.current(forest).unwrap().tapped);
    assert_eq!(life(&g, 1), Quantity::from(LIFE + 1));
}
