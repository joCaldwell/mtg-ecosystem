use super::*;

#[test]
fn composition_wave_b_counts_current_opponent_lands_only_at_resolution() {
    use mtg_witness::aura::prepared_with_sources;
    use mtg_witness::equipped::{answer, both, object, pay_to_priority, recover, request};
    const GEYSER: &str =
        include_str!("../../../../../tests/fixtures/a8dba58b-2956-492e-ae30-49db2ae68e53.json");
    both(
        prepared_with_sources(
            &[
                ("Mana Geyser", 0, 0, true),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Mountain", 0, 0, false),
                ("Forest", 0, 1, false),
                ("Forest", 2, 2, false),
            ],
            &[GEYSER],
        ),
        |g, q| {
            answer(g, q, Response::Cast { card: CardId(0) });
            pay_to_priority(g, q, vec![]);
            recover(g);
            let finish = |game: &mut Game| {
                while !game.state.rules.stack.is_empty() {
                    answer(game, q, Response::Pass);
                    request(game, q);
                }
            };
            let mut zero = g.fork();
            let prior = zero.state.rules.players[0].mana.clone();
            finish(&mut zero);
            assert_eq!(zero.state.rules.players[0].mana, prior);
            recover(&zero);
            answer(g, q, Response::Pass);
            let response = request(g, q);
            assert_eq!(response.player(), PlayerId(1));
            answer(
                g,
                q,
                Response::ActivateMana {
                    object: object(g, 6),
                },
            );
            request(g, q);
            recover(g);
            finish(g);
            assert_eq!(
                g.state.rules.players[0].mana.get(&Color::R),
                Some(&1.into())
            );
            assert_eq!(
                g.state.rules.players[1].mana.get(&Color::G),
                Some(&1.into())
            );
            assert!(!g.current(object(g, 7)).unwrap().tapped);
            recover(g);
        },
    );
}
