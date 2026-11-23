//! Drive adversarial witnesses through public player projections and validated receipts.
use mtg_contracts::Quantity;
use mtg_core::{
    Advance, Budget, Game, Observation, PlayerId, PlayerRequest, PlayerResponse, QueryResult,
    QueryValue, ValidatedResponse,
};
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

pub fn complete(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..2000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => result = game.resume_work(work, Budget::new(1)),
            QueryResult::Rejected(reason) => panic!("query rejected {reason:?}"),
        }
    }
    panic!("query did not complete")
}
pub fn request(game: &Game) -> (PlayerId, PlayerRequest) {
    game.live_players()
        .into_iter()
        .find_map(
            |player| match complete(game, game.player_decision_for(player, Budget::new(0))) {
                QueryValue::Decision(Some(req)) => Some((player, req)),
                QueryValue::Decision(None) => None,
                _ => panic!("decision"),
            },
        )
        .expect("missing decision")
}
pub fn observe(game: &Game, player: PlayerId) -> Observation {
    let QueryValue::Observation(view) = complete(game, game.observe(player, Budget::new(0))) else {
        panic!("observation")
    };
    view
}
pub fn receipt(game: &Game, response: PlayerResponse) -> ValidatedResponse {
    let (actor, req) = request(game);
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(actor, req.token().clone(), response, Budget::new(0)),
    ) else {
        panic!("validated")
    };
    receipt
}

pub fn submit(game: &mut Game, response: PlayerResponse) {
    let checked = receipt(game, response);
    game.submit(checked).unwrap();
}

pub fn settle(game: &mut Game) -> Advance {
    game.advance(Budget::unlimited())
}

pub fn input(game: &mut Game, response: PlayerResponse) {
    let finish = matches!(response, PlayerResponse::FinishMana { .. });
    submit(game, response);
    if finish {
        complete_mana(game);
    }
    assert!(matches!(settle(game), Advance::AwaitingDecision(_)));
}
