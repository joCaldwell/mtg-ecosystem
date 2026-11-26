//! Upkeep keywords lowered to their defining triggers, from exact frozen R5
//! source on private boards: CR 702.30a (echo), 603.4, 118.12.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MOGG_WAR_MARSHAL: &str = record!("7b8f4879-578e-40e4-863a-41d0c32c6bdd");

/// Pass priority until `stop`, ordering triggers as offered and never
/// attacking or blocking; other decisions fail the test.
fn advance(g: &mut Game, stop: impl Fn(&Game, &Decision<Rules>) -> bool) -> Decision<Rules> {
    for _ in 0..512 {
        let d = Driver::request(g, u64::MAX);
        if stop(g, &d) {
            checked(g);
            return d;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(vec![]),
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

fn upkeep_of(g: &Game, seat: u8, turn: u64) -> bool {
    g.state.rules.turn.step == TurnStep::Upkeep
        && g.state.rules.active == Some(PlayerId(seat))
        && g.state.rules.turn.number == turn
}

/// Mogg War Marshal: "Echo {1}{R}. When this creature enters or dies,
/// create a 1/1 red Goblin creature token." It came under its controller's
/// control before that player's first upkeep, so echo triggers there (CR
/// 702.30a, 603.4) and offers its payment (CR 118.12). Paying keeps it, and
/// at the following upkeep the intervening clause fails at match. Declining
/// sacrifices it, so its dies trigger creates a Goblin.
#[test]
fn mogg_war_marshal_echo_is_paid_once_or_sacrificed() {
    for pay in [true, false] {
        let mut g = Game::with_trace(
            prepare(
                board(&[MOGG_WAR_MARSHAL], TurnStep::PrecombatMain)
                    .permanent_or_hand("Mogg War Marshal", 0, 0, false)
                    .permanent_or_hand("Mountain", 0, 0, false)
                    .permanent_or_hand("Mountain", 0, 0, false),
            ),
            TraceMode::Detailed,
        );
        let marshal = Driver::object(&g, 0);
        let d = advance(&mut g, |_, d| d.reason == Reason::OptionalEffect);
        assert!(upkeep_of(&g, 0, 5), "{:?}", g.state.rules.turn);
        assert_eq!(intervening(&g, marshal), vec![(true, true), (false, true)]);
        let flag = if pay { Flag::Accept } else { Flag::Decline };
        g.respond(
            d.player,
            d.id,
            d.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        if pay {
            Driver::pay(&mut g, &[1, 2]);
        }
        settle(&mut g);
        assert_recoverable((&g).into());
        let goblins = g
            .state
            .rules
            .objects
            .values()
            .filter(|o| {
                o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card)
            })
            .count();
        if !pay {
            assert_eq!(
                g.zone(marshal),
                None,
                "the sacrificed Marshal is a new object in the graveyard"
            );
            assert_eq!(
                g.state.rules.objects[&CardId(0)].zone,
                Zone::Graveyard { owner: PlayerId(0) }
            );
            assert_eq!(goblins, 1, "its dies trigger created a Goblin");
            continue;
        }
        assert_eq!(g.zone(marshal), Some(Zone::Battlefield));
        assert_eq!(goblins, 0);
        // The next upkeep's check fails at match: it has been under its
        // controller's control since before the last upkeep began.
        advance(&mut g, |g, d| {
            d.reason == Reason::Priority
                && g.state.rules.turn.number == 7
                && g.state.rules.turn.step == TurnStep::Draw
        });
        assert_eq!(
            intervening(&g, marshal),
            vec![(true, true), (false, true), (true, false)]
        );
        assert_eq!(g.zone(marshal), Some(Zone::Battlefield));
    }
}
