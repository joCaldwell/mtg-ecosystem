//! Shared trusted-input primitives. Public-API witnesses have a separate permitted-input driver.
use super::*;

pub struct Driver;
impl Driver {
    pub fn settle(game: &mut Game, quantum: u64) -> Advance {
        assert!(quantum > 0);
        for _ in 0..2_000 {
            let result = game.advance(Budget::new(quantum));
            if !matches!(result, Advance::Suspended { .. }) {
                return result;
            }
        }
        panic!("no boundary within 2000 advances")
    }

    pub fn request(game: &mut Game, quantum: u64) -> Request {
        match Self::settle(game, quantum) {
            Advance::AwaitingDecision(request) => request,
            other => panic!("expected decision, got {other:?}"),
        }
    }

    pub fn answer(game: &mut Game, quantum: u64, response: Response) {
        let request = Self::request(game, quantum);
        game.respond(request.player(), request.decision_id(), response)
            .unwrap();
    }

    pub fn object(game: &Game, id: u32) -> ObjectId {
        game.object_id(CardId(id)).unwrap()
    }

    pub fn cast(game: &mut Game, card: CardId) {
        Self::answer(game, u64::MAX, Response::Cast { card });
    }

    pub fn pay(game: &mut Game, lands: &[u32]) {
        for id in lands {
            let object = Self::object(game, *id);
            Self::answer(game, u64::MAX, Response::ActivateMana { object });
        }
        Self::answer(game, u64::MAX, Response::FinishMana { undo_mana: true });
        game.complete_test_mana();
    }

    pub fn pass(game: &mut Game) {
        Self::answer(game, u64::MAX, Response::Pass);
    }
}

/// A replay journal alone cannot distinguish a pending response from its
/// consumption. Retain the exact boundary and independently reached state.
pub struct JournalCase {
    prepared: Prepared,
    inputs: ReplayJournal,
    boundary: u64,
    digest: String,
}
impl From<&Game> for JournalCase {
    fn from(game: &Game) -> Self {
        Self {
            prepared: game.prepared.as_ref().clone(),
            inputs: game.replay_journal(),
            boundary: game.rule_boundary(),
            digest: game.state_digest().unwrap(),
        }
    }
}

fn advance_to(game: &mut Game, boundary: u64, quantum: u64) {
    while game.rule_boundary() < boundary {
        let before = game.rule_boundary();
        game.advance(Budget::new(quantum.min(boundary - before)));
        assert!(game.rule_boundary() > before);
    }
    assert_eq!(game.rule_boundary(), boundary);
}

fn recover_journal(case: &JournalCase, mode: TraceMode, quantum: u64) -> Game {
    let mut game = Game::with_trace(case.prepared.clone(), mode);
    let mut concessions = case.inputs.concessions.iter().peekable();
    for ordinal in 0..=case.inputs.decisions.len() {
        while let Some(input) = concessions.next_if(|c| c.after_decisions == ordinal as u64) {
            advance_to(&mut game, input.boundary, quantum);
            game.concede(input.player).unwrap();
        }
        if let Some(input) = case.inputs.decisions.get(ordinal) {
            let request = Driver::request(&mut game, quantum);
            assert_eq!(
                (request.player(), request.decision_id()),
                (input.player, input.decision_id)
            );
            game.respond(input.player, input.decision_id, input.response.clone())
                .unwrap();
        }
    }
    assert!(concessions.next().is_none());
    advance_to(&mut game, case.boundary, quantum);
    game
}

pub fn assert_recoverable(case: JournalCase) {
    let mut verification = None;
    for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
        let replay = Game::replay_to(
            case.prepared.clone(),
            &case.inputs,
            Some(case.boundary),
            mode,
        )
        .unwrap();
        assert_eq!(replay.state_digest().unwrap(), case.digest);
        match mode {
            TraceMode::Off => assert!(replay.trace.verification.is_none()),
            TraceMode::Compact => {
                verification = replay.trace.verification.clone();
                assert!(verification.is_some());
            }
            TraceMode::Detailed => assert_eq!(verification, replay.trace.verification),
        }
        for quantum in [1, u64::MAX] {
            let game = recover_journal(&case, mode, quantum);
            assert_eq!(game.state, replay.state);
            assert_eq!(game.trace(), replay.trace());
            let restored = Game::restore(case.prepared.clone(), &game.snapshot().unwrap()).unwrap();
            assert_eq!(restored.state, game.state);
            assert_eq!(restored.trace(), game.trace());
            assert_eq!(restored.fork().state, game.state);
        }
    }
}
