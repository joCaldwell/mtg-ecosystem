//! Independent public-API oracle for immutable history-byte memoization.
//! The oracle serializes the entire State, without using the digest writer.
use mtg_contracts::{Catalog, canonical, digest, sha256};
use mtg_core::*;
use serde_json::Value;

fn prepared() -> Prepared {
    let catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-match-ir.json"
    ))
    .unwrap();
    let mut config = MatchConfig::foundations([201; 32], PlayerId(0));
    config.decks = std::array::from_fn(|_| white_deck());
    Prepared::admit_pod(catalog, config).unwrap()
}

fn complete(game: &Game, mut result: QueryResult) -> QueryValue {
    for _ in 0..100_000 {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => {
                result = game.resume_work(work, Budget::new(64));
            }
            QueryResult::Rejected(error) => panic!("unexpected query rejection: {error:?}"),
        }
    }
    panic!("query work bound")
}

fn settle(game: &mut Game, budget: u64) {
    for _ in 0..20_000 {
        match game.advance(Budget::new(budget)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(_) => return,
            other => panic!("unexpected finite-prefix boundary: {other:?}"),
        }
    }
    panic!("automatic work bound")
}

fn pass_draw(game: &mut Game, budget: u64) {
    let (player, request) = game
        .live_players()
        .into_iter()
        .find_map(|player| {
            match complete(game, game.player_decision_for(player, Budget::new(64))) {
                QueryValue::Decision(Some(request)) => Some((player, request)),
                QueryValue::Decision(None) => None,
                _ => panic!("decision result"),
            }
        })
        .expect("offered decision");
    let response = match &request {
        PlayerRequest::Mulligan { .. } => PlayerResponse::Mulligan { take: false },
        PlayerRequest::Priority { .. } => PlayerResponse::Pass,
        PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
        PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
        PlayerRequest::Discard {
            count, candidates, ..
        } => PlayerResponse::Discard {
            objects: candidates[..*count].to_vec(),
        },
        _ => panic!("request outside finite pass/draw prefix"),
    };
    let QueryValue::Validated(receipt) = complete(
        game,
        game.check_player_response(player, request.token().clone(), response, Budget::new(64)),
    ) else {
        panic!("response validation")
    };
    game.submit(receipt).unwrap();
    settle(game, budget);
}

fn prefix(trace: TraceMode, budget: u64) -> (Prepared, Game) {
    let prepared = prepared();
    let mut game = Game::with_trace(prepared.clone(), trace);
    settle(&mut game, budget);
    for _ in 0..68 {
        pass_draw(&mut game, budget);
    }
    (prepared, game)
}

fn oracle(game: &Game) -> Value {
    let raw = game.snapshot().unwrap();
    let envelope: Value = serde_json::from_str(&raw).unwrap();
    let expected = sha256(canonical(&envelope["state"]).unwrap().as_bytes());
    assert_eq!(envelope["state_sha256"], expected);
    for _ in 0..3 {
        assert_eq!(game.state_digest().unwrap(), expected);
    }
    assert_eq!(
        game.snapshot().unwrap(),
        raw,
        "warming must not alter wire bytes"
    );
    envelope
}

#[test]
fn warmed_multi_chunk_history_preserves_whole_state_bytes_queries_and_replay() {
    let (prepared, game) = prefix(TraceMode::Off, 64);
    let envelope = oracle(&game);
    for knowledge in envelope["state"]["visibility"].as_array().unwrap() {
        assert!(knowledge["history"].as_array().unwrap().len() > 64);
    }
    let before = game.snapshot().unwrap();
    for player in game.live_players() {
        assert!(matches!(
            complete(&game, game.observe(player, Budget::new(0))),
            QueryValue::Observation(_)
        ));
        complete(&game, game.player_decision_for(player, Budget::new(0)));
    }
    assert_eq!(game.snapshot().unwrap(), before);
    let restored = Game::restore(prepared.clone(), &before).unwrap();
    let replayed =
        Game::replay_with_trace(prepared, &game.replay_journal(), TraceMode::Off).unwrap();
    assert_eq!(oracle(&restored)["state"], envelope["state"]);
    assert_eq!(oracle(&replayed)["state"], envelope["state"]);
}

#[test]
fn warmed_forks_share_no_semantic_mutation_and_match_different_work_partitions() {
    let (_, game) = prefix(TraceMode::Off, 64);
    let before = game.snapshot().unwrap();
    oracle(&game);
    let mut left = game.fork();
    let mut right = game.fork();
    for _ in 0..12 {
        pass_draw(&mut left, 1);
        pass_draw(&mut right, 1024);
    }
    assert_eq!(oracle(&left)["state"], oracle(&right)["state"]);
    right.concede(PlayerId(3)).unwrap();
    settle(&mut right, 64);
    assert_ne!(oracle(&left)["state"], oracle(&right)["state"]);
    assert_eq!(game.snapshot().unwrap(), before);
}

#[test]
fn rehashed_old_chunk_and_tail_memories_still_require_actual_replay() {
    let (prepared, game) = prefix(TraceMode::Off, 64);
    let baseline = oracle(&game);
    let length = baseline["state"]["visibility"][0]["history"]
        .as_array()
        .unwrap()
        .len();
    assert!(length > 64);
    for index in [0, 31, 32, length - 1] {
        let mut forged = baseline.clone();
        let turn = &mut forged["state"]["visibility"][0]["history"][index]["turn"]["number"];
        *turn = (turn.as_u64().unwrap() + 1).into();
        forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
        assert!(Game::restore(prepared.clone(), &canonical(&forged).unwrap()).is_err());
    }
    assert_eq!(oracle(&game), baseline);
}
