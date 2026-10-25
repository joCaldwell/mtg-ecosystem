//! Public-request spending policy for historical player-facing test drivers.
use mtg_contracts::Quantity;
use mtg_core::{Advance, Budget, Game, PlayerRequest, PlayerResponse, QueryResult, QueryValue};
use std::collections::BTreeMap;

pub fn complete_mana(game: &mut Game) {
    let Advance::AwaitingDecision(r) = game.advance(Budget::unlimited()) else {
        return;
    };
    let QueryResult::Complete(QueryValue::Decision(Some(PlayerRequest::SpendMana {
        token,
        cost,
        available,
        ..
    }))) = game.player_decision_for(r.player(), Budget::unlimited())
    else {
        return;
    };
    let mut pool = available;
    let mut spending = BTreeMap::<_, Quantity>::new();
    for color in cost.colored {
        pool.get_mut(&color).unwrap().subtract(&1.into());
        spending.entry(color).or_default().add(&1.into());
    }
    let mut generic = cost.generic;
    for (color, n) in pool {
        let amount = std::cmp::min(n, generic.clone());
        if amount > Quantity::zero() {
            spending.entry(color).or_default().add(&amount);
            generic.subtract(&amount);
        }
    }
    assert_eq!(generic, Quantity::zero());
    let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
        r.player(),
        token,
        PlayerResponse::SpendMana { spending },
        Budget::unlimited(),
    ) else {
        panic!("explicit spending rejected")
    };
    game.submit(receipt).unwrap();
}
