//! Shared trusted-input primitives. Public-API witnesses have a separate permitted-input driver.
use super::*;

pub struct Driver;
impl Driver {
    pub fn settle(game: &mut Game, quantum: u64) -> Advance {
        assert!(quantum > 0);
        for _ in 0..2_000 {
            let result = game.advance(Budget::new(quantum));
            game.drain_trace(&mut |_| {});
            if !matches!(result, Advance::Suspended { .. }) {
                return result;
            }
        }
        panic!("no boundary within 2000 advances")
    }

    pub fn request(game: &mut Game, quantum: u64) -> Decision<Rules> {
        match Self::settle(game, quantum) {
            Advance::AwaitingDecision(request) => request,
            other => panic!("expected decision, got {other:?}"),
        }
    }

    pub fn answer(game: &mut Game, quantum: u64, answer: Answer) {
        let request = Self::request(game, quantum);
        game.respond(request.player, request.id, answer).unwrap();
    }

    pub fn object(game: &Game, id: u32) -> ObjectId {
        game.object_id(CardId(id)).unwrap()
    }

    pub fn action(game: &mut Game, action: Action<Rules>) {
        let decision = Self::request(game, u64::MAX);
        let answer = decision.pick_one(&Candidate::Action { action }).unwrap();
        game.respond(decision.player, decision.id, answer).unwrap();
    }
    pub fn pick_object(game: &mut Game, object: ObjectId) {
        let decision = Self::request(game, u64::MAX);
        let answer = decision.pick_one(&Candidate::Object { object }).unwrap();
        game.respond(decision.player, decision.id, answer).unwrap();
    }

    pub fn cast(game: &mut Game, card: CardId) {
        let object = game.object_id(card).unwrap();
        Self::action(game, Action::Cast { card: object });
    }

    pub fn pay(game: &mut Game, lands: &[u32]) {
        for id in lands {
            let object = Self::object(game, *id);
            let decision = Self::request(game, u64::MAX);
            let source = decision
                .index(&Candidate::Action {
                    action: Action::ActivateMana { object },
                })
                .unwrap();
            Self::answer(
                game,
                u64::MAX,
                Answer::Pay {
                    pay: PayAnswer::Activate { source },
                },
            );
        }
        Self::answer(
            game,
            u64::MAX,
            Answer::Pay {
                pay: PayAnswer::Finish {
                    undo_on_failure: true,
                },
            },
        );
        game.complete_test_mana();
    }

    pub fn pass(game: &mut Game) {
        Self::action(game, Action::Pass);
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
        game.drain_trace(&mut |_| {});
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
                (request.player, request.id),
                (input.player, input.decision_id)
            );
            game.respond(input.player, input.decision_id, input.answer.clone())
                .unwrap();
        }
    }
    assert!(concessions.next().is_none());
    advance_to(&mut game, case.boundary, quantum);
    game
}

pub fn assert_recoverable(case: JournalCase) {
    let mut verification = None;
    for mode in [
        TraceMode::Off,
        TraceMode::Compact,
        TraceMode::Stream,
        TraceMode::Detailed,
    ] {
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
            TraceMode::Stream | TraceMode::Detailed => {
                assert_eq!(verification, replay.trace.verification)
            }
        }
        for quantum in [1, u64::MAX] {
            let game = recover_journal(&case, mode, quantum);
            assert_eq!(game.state, replay.state);
            assert_eq!(game.trace(), replay.trace());
            let restored = Game::restore(case.prepared.clone(), &game.snapshot().unwrap()).unwrap();
            assert_eq!(restored.state, game.state);
            assert_eq!(restored.trace(), game.trace());
            let fork = restored.fork();
            assert_eq!(fork.state, game.state);
            assert_eq!(fork.trace(), game.trace());
        }
    }
}

impl Game {
    /// Select an intent for a driver which first settles the current step.
    pub fn next_test_decision(&self) -> Decision<Rules> {
        self.pending_decision()
            .unwrap()
            .unwrap_or_else(|| Driver::request(&mut self.fork(), u64::MAX))
    }
    pub fn next_test_viewer_decision(&self) -> Decision<Viewer> {
        let d = self.next_test_decision();
        let mut game = self.fork();
        Driver::request(&mut game, u64::MAX);
        let QueryResult::Complete(QueryValue::Decision(Some(d))) =
            game.player_decision_for(d.player, Budget::unlimited())
        else {
            panic!("missing viewer decision")
        };
        d
    }
}
