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
        .filter(|o| {
            o.zone
                == (Zone::Hand {
                    owner: PlayerId(seat),
                })
        })
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
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag { flag }).unwrap(),
    )
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

/// Pass priority until the next decision that is not priority; `None` once
/// the stack is empty and the active player has priority again.
fn next_choice_or_empty(g: &mut Game) -> Option<Decision<Rules>> {
    for _ in 0..16 {
        let d = Driver::request(g, u64::MAX);
        if d.reason != Reason::Priority {
            return Some(d);
        }
        if g.state.rules.stack.is_empty() {
            return None;
        }
        let pass = d
            .pick_one(&Candidate::Action {
                action: Action::Pass,
            })
            .unwrap();
        g.respond(d.player, d.id, pass).unwrap();
    }
    panic!("no boundary within 16 priority passes")
}

/// Seat 0 casts card `id` with its untapped Plains, choosing `targets`.
fn cast_with(g: &mut Game, id: u32, targets: Vec<TargetEntity>) {
    Driver::answer(g, u64::MAX, g.next_test_decision().cast(CardId(id)));
    mtg_witness::equipped::pay_to_priority(g, u64::MAX, targets);
}

/// Leonin Snarecaster: "When this creature enters, you may tap target
/// creature." Tapping an already tapped permanent is impossible (CR
/// 701.26a), so no choice is offered (CR 608.2d); the target is still
/// chosen when the trigger is put on the stack (CR 603.3d).
#[test]
fn leonin_snarecaster_offers_no_choice_to_tap_a_tapped_creature() {
    for pre_tapped in [true, false] {
        let mut g = Game::new(prepared(&[
            ("Leonin Snarecaster", 0, true),
            ("Pressure Point", 0, true),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Grizzly Bears", 1, false),
        ]));
        let bears = Driver::object(&g, 6);
        if pre_tapped {
            // "Tap target creature. Draw a card."
            cast_with(&mut g, 1, vec![TargetEntity::Object { object: bears }]);
            assert!(next_choice_or_empty(&mut g).is_none());
            assert!(g.current(bears).unwrap().tapped);
        }
        cast_with(&mut g, 0, vec![]);
        let d = next_choice_or_empty(&mut g).expect("the entry trigger's target");
        assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
        g.respond(
            d.player,
            d.id,
            d.entity_targets(vec![TargetEntity::Object { object: bears }]),
        )
        .unwrap();
        checked(&g);
        match next_choice_or_empty(&mut g) {
            None => assert!(pre_tapped, "an untapped target offers the choice"),
            Some(d) => {
                assert!(!pre_tapped, "a tapped target offered {d:?}");
                assert_eq!(
                    (d.player, &d.reason),
                    (PlayerId(0), &Reason::OptionalEffect)
                );
                checked(&g);
                g.respond(
                    d.player,
                    d.id,
                    d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
                )
                .unwrap();
                assert!(next_choice_or_empty(&mut g).is_none());
            }
        }
        assert!(g.current(bears).unwrap().tapped);
        checked(&g);
    }
}

/// Deathcap Marionette: "When this creature enters, you may mill two
/// cards." A player can't choose to mill more cards than their library
/// holds (CR 701.17b), so one card offers no choice; two are offered and
/// milled.
#[test]
fn deathcap_marionette_offers_milling_only_a_whole_library_amount() {
    for library in [1usize, 2] {
        let mut board = Board::new(
            FixtureCatalog::Witness.get().clone(),
            "w5-control-private-witness",
        )
        .step(TurnStep::PrecombatMain)
        .seats(2)
        .permanent_or_hand("Deathcap Marionette", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false);
        for _ in 0..library {
            board = board.card("Forest", 0, 0, Zone::Library { owner: PlayerId(0) });
        }
        let mut g = Game::new(board.prepare());
        let in_zone = |g: &Game, zone: Zone| {
            g.state
                .rules
                .objects
                .values()
                .filter(|o| o.zone == zone)
                .count()
        };
        let graveyard = Zone::Graveyard { owner: PlayerId(0) };
        cast_with(&mut g, 0, vec![]);
        match next_choice_or_empty(&mut g) {
            None => assert_eq!(library, 1, "two cards must offer the mill"),
            Some(d) => {
                assert_eq!(library, 2, "one card offered {d:?}");
                assert_eq!(
                    (d.player, &d.reason),
                    (PlayerId(0), &Reason::OptionalEffect)
                );
                checked(&g);
                g.respond(
                    d.player,
                    d.id,
                    d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
                )
                .unwrap();
                assert!(next_choice_or_empty(&mut g).is_none());
            }
        }
        let milled = if library == 2 { 2 } else { 0 };
        assert_eq!(in_zone(&g, graveyard), milled);
        checked(&g);
    }
}

#[test]
fn bestiary_requires_colored_payment_before_drawing() {
    for (source, accept, fund) in [
        (Some("Forest"), false, false),
        (Some("Island"), true, false),
        (Some("Forest"), true, true),
        (None, false, false),
    ] {
        let mut cards = vec![
            ("Grizzly Bears", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Lifecrafter's Bestiary", 0, false),
        ];
        if let Some(land) = source {
            cards.push((land, 0, false));
        }
        let mut g = Game::new(prepared(&cards));
        cast_bears(&mut g);
        let before = hand(&g, 0);
        if source.is_some() {
            choose(&mut g, 0, accept);
        }
        if fund {
            Driver::pay(&mut g, &[4]);
        } else if accept {
            Driver::answer(
                &mut g,
                u64::MAX,
                Answer::Pay {
                    pay: PayAnswer::Finish {
                        undo_on_failure: true,
                    },
                },
            );
        }
        settle(&mut g);
        assert_eq!(hand(&g, 0), before + usize::from(fund));
    }
}

#[test]
fn thirst_for_knowledge_offers_artifact_or_two_unrestricted_discards() {
    for pay in [true, false] {
        let mut g = Game::new(prepared(&[
            ("Thirst for Knowledge", 0, true),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Darksteel Relic", 0, true),
        ]));
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3]);
        let offer = next_choice(&mut g);
        assert_eq!(
            (offer.player, &offer.reason),
            (PlayerId(0), &Reason::OptionalEffect)
        );
        checked(&g);
        let choice = if pay {
            Candidate::Object {
                object: Driver::object(&g, 4),
            }
        } else {
            Candidate::Flag {
                flag: Flag::Decline,
            }
        };
        g.respond(offer.player, offer.id, offer.pick_one(&choice).unwrap())
            .unwrap();
        if !pay {
            let discard = next_choice(&mut g);
            checked(&g);
            let picks = discard
                .candidates
                .iter()
                .filter_map(|candidate| {
                    if let Candidate::Object { object } = candidate
                        && object.card != CardId(4)
                    {
                        return discard.index(candidate);
                    }
                    None
                })
                .take(2)
                .collect();
            g.respond(
                discard.player,
                discard.id,
                Answer::Pick { picks: vec![picks] },
            )
            .unwrap();
        }
        settle(&mut g);
        assert_eq!(hand(&g, 0), if pay { 3 } else { 2 });
    }
}

#[test]
fn dega_sanctuary_replaces_the_original_life_gain() {
    for both_colors in [true, false] {
        let mut board = Board::new(
            FixtureCatalog::Witness.get().clone(),
            "dega-sanctuary-upkeep",
        )
        .seats(2)
        .life(PlayerId(0), LIFE.into())
        .step(TurnStep::Upkeep)
        .permanent_or_hand("Dega Sanctuary", 0, 0, false)
        .permanent_or_hand("Royal Assassin", 0, 0, false);
        if both_colors {
            board = board.permanent_or_hand("Kamahl, Pit Fighter", 0, 0, false);
        }
        let mut g = Game::new(board.libraries("Forest", 4).prepare());
        g.enter_turn_step().unwrap();
        for _ in 0..4 {
            assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
            if life(&g, 0) != LIFE.into() {
                break;
            }
            Driver::pass(&mut g);
        }
        assert_eq!(life(&g, 0), (LIFE + if both_colors { 4 } else { 2 }).into());
        checked(&g);
    }
}
