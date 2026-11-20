use super::*;
use mtg_contracts::ManaResolutionQuantity;

impl Game {
    pub(super) fn produce_resolution_mana(
        &mut self,
        frame: Resolving,
        color: Color,
        quantity: ManaResolutionQuantity,
        source_block: u32,
    ) -> Result<(), String> {
        let controller = frame.spell.controller;
        let amount = match quantity {
            ManaResolutionQuantity::Fixed { amount } => amount,
            ManaResolutionQuantity::PerMatchingPermanent { mut amount, filter } => {
                let count = self
                    .state
                    .rules
                    .objects
                    .values()
                    .filter(|object| self.matches_permanent_filter(object.id, controller, &filter))
                    .count();
                amount.multiply(
                    &i64::try_from(count)
                        .map_err(|_| "permanent count overflow")?
                        .into(),
                );
                amount
            }
        };
        let actual = if amount == Quantity::zero() || !self.is_live(controller) {
            None
        } else {
            Some(self.commit(
                ProposedEvent::AddMana {
                    player: controller,
                    color,
                    amount,
                    source: frame.spell.source,
                },
                BatchCause::Instruction {
                    stack_id: frame.spell.id,
                    player: controller,
                    source: frame.spell.source,
                    cursor: frame.cursor,
                    source_block,
                },
            )?)
        };
        self.finish_instruction(frame, InstructionOutcome::Effect { actual })
    }
}

#[cfg(test)]
#[test]
fn composition_wave_b_counts_current_opponent_lands_only_at_resolution() {
    use super::aura_control_tests::prepared_with_sources;
    use super::equipped_indestructible_tests::{
        answer, both, object, pay_to_priority, recover, request,
    };
    const GEYSER: &str =
        include_str!("../../../../tests/fixtures/a8dba58b-2956-492e-ae30-49db2ae68e53.json");
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
