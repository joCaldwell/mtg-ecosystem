//! Paid CR510.4/702.4b/611.2c/514.2; shared combat damage-step machinery.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const STROBE: &str =
    include_str!("../../../../tests/fixtures/9bf4ee0f-6e28-4793-b251-f81ee444a119.json");
const FURY: &str =
    include_str!("../../../../tests/fixtures/7aac51ef-ae03-4a9e-9a48-46223787f291.json");
const RIOT: &str =
    include_str!("../../../../tests/fixtures/49f42896-90cd-4a05-bcca-12b19f780446.json");
fn prepared(rows: &[Row]) -> Prepared {
    prepared_with_sources(rows, &[STROBE, FURY, RIOT])
}
fn seek(g: &mut Game, q: u64, mut yes: impl FnMut(&Game, &Request) -> bool) -> Request {
    for _ in 0..500 {
        let r = request(g, q);
        if yes(g, &r) {
            return r;
        }
        answer(g, q, pass(&r));
    }
    panic!("bounded combat boundary")
}
fn damage_step(g: &mut Game, q: u64, step: TurnStep) {
    seek(g, q, |g, r| {
        g.turn().step == step
            && matches!(
                r,
                Request::Priority {
                    player: PlayerId(0),
                    ..
                }
            )
    });
}
fn cleanup(g: &mut Game, q: u64) {
    let turn = g.turn().number;
    seek(g, q, |g, _| g.turn().number > turn);
}
#[test]
fn temporary_double_strike_paid_sources_two_damage_steps_fixed_capture_and_cleanup() {
    for (name, creature, set, power) in [
        ("Assault Strobe", "Grizzly Bears", false, 2i64),
        ("Uncaged Fury", "Grizzly Bears", false, 3),
        ("Cleaver Riot", "Grizzly Bears", true, 2),
        ("Assault Strobe", "Fencing Ace", false, 1),
    ] {
        let mut rows = vec![
            (name, 0, 0, true),
            (creature, 0, 0, false),
            ("Grizzly Bears", 0, 0, true),
            ("Grizzly Bears", 1, 1, false),
        ];
        rows.extend(std::iter::repeat_n(("Mountain", 0, 0, false), 5));
        rows.extend(std::iter::repeat_n(("Forest", 0, 0, false), 2));
        both(prepared(&rows), |g, q| {
            cast(g, q, 0, if set { vec![] } else { target(g, 1) });
            assert_eq!(
                g.current_keywords(object(g, 1))
                    .iter()
                    .filter(|k| **k == KeywordAbility::DoubleStrike)
                    .count(),
                1
            );
            assert_eq!(g.state.rules.effects.len(), 1);
            assert!(!g.has_keyword(object(g, 3), KeywordAbility::DoubleStrike));
            until_player(g, q, PlayerId(0));
            cast(g, q, 2, vec![]);
            assert!(!g.has_keyword(object(g, 2), KeywordAbility::DoubleStrike));
            seek(g, q, |_, r| matches!(r, Request::Attackers { .. }));
            answer(
                g,
                q,
                Response::Attackers {
                    attacks: vec![(object(g, 1), PlayerId(1))],
                },
            );
            damage_step(g, q, TurnStep::FirstStrikeDamage);
            assert_eq!(g.state.rules.players[1].life, Quantity::from(40 - power));
            assert_eq!(
                g.state.rules.first_strike_participants,
                BTreeSet::from([object(g, 1)])
            );
            recover(g);
            damage_step(g, q, TurnStep::CombatDamage);
            assert_eq!(
                g.state.rules.players[1].life,
                Quantity::from(40 - 2 * power)
            );
            recover(g);
            cleanup(g, q);
            assert_eq!(
                g.characteristics(object(g, 1)).unwrap().power,
                Some(Quantity::from(if creature == "Fencing Ace" {
                    1
                } else {
                    2
                }))
            );
            assert_eq!(
                g.has_keyword(object(g, 1), KeywordAbility::DoubleStrike),
                creature == "Fencing Ace"
            );
            assert!(g.state.rules.effects.is_empty());
            recover(g);
        });
    }
}
#[test]
fn temporary_double_strike_late_gain_keeps_first_step_history_and_enables_normal_step() {
    for recipient in [1u32, 2] {
        both(
            prepared(&[
                ("Uncaged Fury", 0, 0, true),
                ("Grizzly Bears", 0, 0, false),
                ("Youthful Knight", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
            ]),
            |g, q| {
                seek(g, q, |_, r| matches!(r, Request::Attackers { .. }));
                answer(
                    g,
                    q,
                    Response::Attackers {
                        attacks: vec![(object(g, 1), PlayerId(1)), (object(g, 2), PlayerId(1))],
                    },
                );
                damage_step(g, q, TurnStep::FirstStrikeDamage);
                assert_eq!(g.state.rules.players[1].life, 38.into());
                let frozen = BTreeSet::from([object(g, 2)]);
                assert_eq!(g.state.rules.first_strike_participants, frozen);
                recover(g);
                cast(g, q, 0, target(g, recipient));
                assert_eq!(g.turn().step, TurnStep::FirstStrikeDamage);
                assert_eq!(g.state.rules.first_strike_participants, frozen);
                assert_eq!(g.state.rules.players[1].life, 38.into());
                recover(g);
                damage_step(g, q, TurnStep::CombatDamage);
                assert_eq!(
                    g.state.rules.players[1].life,
                    Quantity::from(if recipient == 1 { 35 } else { 33 })
                );
                recover(g);
                cleanup(g, q);
                assert!(!g.has_keyword(object(g, recipient), KeywordAbility::DoubleStrike));
                recover(g);
            },
        );
    }
}
