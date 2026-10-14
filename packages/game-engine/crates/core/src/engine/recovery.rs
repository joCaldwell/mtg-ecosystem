use super::*;

/// Trusted host verifier for an ordered sequence of snapshots of one run.
/// Each snapshot undergoes the same decoding, hash, invariant, state and trace
/// checks as standalone restore. A private independent replay proves its entire
/// input prefix; previously proved prefixes are never executed a second time.
pub struct SnapshotVerifier {
    replay: Game,
    digest: Option<String>,
}
impl SnapshotVerifier {
    pub fn new(prepared: Prepared, mode: TraceMode) -> Self {
        Self {
            replay: Game::with_trace(prepared, mode),
            digest: None,
        }
    }
    pub fn verified_digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }
    pub fn verify(&mut self, json: &str) -> Result<Game, String> {
        let (game, digest) = Game::read_snapshot(self.replay.prepared.clone(), json)?;
        self.verify_game(game, digest)
    }
    pub(super) fn verify_game(&mut self, game: Game, digest: String) -> Result<Game, String> {
        if game.trace.mode != self.replay.trace.mode {
            return Err("snapshot sequence changes trace mode".into());
        }
        // Failure must not change the proved cursor or last verified digest.
        let mut replayed = self.replay.fork();
        replayed.extend_replay(&game.replay_journal(), Some(game.state.rule_steps))?;
        if matches!(game.state.frame, Frame::Fault { .. })
            && !matches!(replayed.state.frame, Frame::Fault { .. })
        {
            let _ = replayed.advance(Budget::new(1));
        }
        if replayed.state != game.state || replayed.trace != game.trace {
            return Err("snapshot is not reachable from this scenario's accepted inputs".into());
        }
        self.replay = replayed;
        self.digest = Some(digest);
        Ok(game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepared(config: ScenarioConfig) -> Prepared {
        Prepared::admit(
            Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
            config,
        )
        .unwrap()
    }
    #[test]
    fn sequence_verification_matches_standalone_restore_through_pending_and_automatic_work() {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let p = prepared(ScenarioConfig::combat_interactions());
            let mut game = Game::with_trace(p.clone(), mode);
            let mut verifier = SnapshotVerifier::new(p.clone(), mode);
            let mut finished = false;
            for index in 0..400 {
                let json = game.snapshot().unwrap();
                let recovered = verifier.verify(&json).unwrap();
                assert_eq!(recovered.state, game.state);
                assert_eq!(recovered.trace, game.trace);
                assert_eq!(
                    verifier.verified_digest(),
                    Some(game.state_digest().unwrap().as_str())
                );
                if index % 23 == 0 {
                    let standalone = Game::restore(p.clone(), &json).unwrap();
                    assert_eq!(recovered.state, standalone.state);
                    assert_eq!(recovered.trace, standalone.trace);
                }
                match game.advance(Budget::new(1)) {
                    Advance::AwaitingDecision(request) => {
                        let response = match request {
                            Request::Priority { .. } => Response::Pass,
                            Request::Attackers { .. } => Response::Attackers { attacks: vec![] },
                            _ => panic!("unexpected passive request"),
                        };
                        game.respond(request.player(), request.decision_id(), response)
                            .unwrap();
                    }
                    Advance::Suspended { .. } => {}
                    Advance::Finished { winner } => {
                        assert_eq!(winner, PlayerId(0));
                        verifier.verify(&game.snapshot().unwrap()).unwrap();
                        finished = true;
                        break;
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(finished);
        }
    }
    #[test]
    fn queued_departures_at_same_boundary_extend_and_recover_without_repeating_inputs() {
        let p = prepared(ScenarioConfig::land_return());
        let mut game = Game::new(p.clone());
        let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
        for player in [PlayerId(1), PlayerId(2)] {
            game.concede(player).unwrap();
            let json = game.snapshot().unwrap();
            assert_eq!(
                verifier.verify(&json).unwrap().state,
                Game::restore(p.clone(), &json).unwrap().state
            );
        }
        for _ in 0..2 {
            game.advance(Budget::new(1));
            verifier.verify(&game.snapshot().unwrap()).unwrap();
        }
        game.concede(PlayerId(3)).unwrap();
        verifier.verify(&game.snapshot().unwrap()).unwrap();
        assert_eq!(
            game.advance(Budget::new(1)),
            Advance::Finished {
                winner: PlayerId(0)
            }
        );
        verifier.verify(&game.snapshot().unwrap()).unwrap();
    }
    #[test]
    fn fault_reachability_is_verified_even_without_a_new_rule_boundary() {
        let p = prepared(ScenarioConfig::land_return());
        let mut game = Game::new(p.clone());
        let mut verifier = SnapshotVerifier::new(p, TraceMode::Detailed);
        for _ in 0..4 {
            let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
                panic!("expected priority before the unsupported transition");
            };
            game.respond(request.player(), request.decision_id(), Response::Pass)
                .unwrap();
            verifier.verify(&game.snapshot().unwrap()).unwrap();
        }
        let before_fault = game.snapshot().unwrap();
        let boundary = game.rule_boundary();
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::Fault(_)
        ));
        assert_eq!(game.rule_boundary(), boundary);
        let fault = game.snapshot().unwrap();
        verifier.verify(&fault).unwrap();
        verifier.verify(&fault).unwrap();
        assert!(verifier.verify(&before_fault).is_err());
        assert_eq!(verifier.verify(&fault).unwrap().state, game.state);
    }
    #[test]
    fn forged_backward_and_changed_prefix_snapshots_cannot_poison_the_verified_cursor() {
        let p = prepared(ScenarioConfig::combat_game());
        let mut game = Game::new(p.clone());
        let mut verifier = SnapshotVerifier::new(p.clone(), TraceMode::Detailed);
        let initial = game.snapshot().unwrap();
        verifier.verify(&initial).unwrap();
        let mut recovered = verifier.verify(&initial).unwrap();
        let receipt = game.check_response(PlayerId(0), 0, Response::Pass).unwrap();
        assert!(
            recovered.submit(receipt).is_err(),
            "restored games need a fresh receipt scope"
        );
        let mut forged: Snapshot = strict_read(&initial).unwrap();
        forged.state.turn.lands_played = 1;
        forged.state_sha256 = digest(&forged.state).unwrap();
        assert!(verifier.verify(&canonical(&forged).unwrap()).is_err());
        assert_eq!(verifier.verify(&initial).unwrap().state, game.state);
        game.respond(PlayerId(0), 0, Response::Pass).unwrap();
        let accepted = game.snapshot().unwrap();
        verifier.verify(&accepted).unwrap();
        let request = match game.advance(Budget::unlimited()) {
            Advance::AwaitingDecision(request) => request,
            other => panic!("unexpected {other:?}"),
        };
        game.respond(request.player(), request.decision_id(), Response::Pass)
            .unwrap();
        let valid = game.snapshot().unwrap();
        let mut later_forgery: Snapshot = strict_read(&valid).unwrap();
        later_forgery.state.turn.lands_played = 1;
        later_forgery.state_sha256 = digest(&later_forgery.state).unwrap();
        assert!(
            verifier
                .verify(&canonical(&later_forgery).unwrap())
                .is_err()
        );
        // Even a failure after consuming additional inputs must leave the
        // original pending-input boundary available to the cursor.
        assert_eq!(verifier.verify(&accepted).unwrap().state.journal.len(), 1);
        let mut forged: Snapshot = strict_read(&valid).unwrap();
        forged.state.journal[0].response = Response::ActivateMana {
            object: ObjectId {
                card: CardId(0),
                incarnation: 0,
            },
        };
        forged.state_sha256 = digest(&forged.state).unwrap();
        assert!(verifier.verify(&canonical(&forged).unwrap()).is_err());
        assert!(verifier.verify(&initial).is_err());
        let mut changed: Snapshot = strict_read(&valid).unwrap();
        changed.trace.mode = TraceMode::Off;
        assert!(verifier.verify(&canonical(&changed).unwrap()).is_err());
        let recovered = verifier.verify(&valid).unwrap();
        assert_eq!(recovered.state, game.state);
        assert_eq!(recovered.trace, game.trace);
        let receipt = game.check_response(request.player(), request.decision_id(), Response::Pass);
        assert!(
            receipt.is_err(),
            "accepted pending input still suppresses another response"
        );
        assert_eq!(recovered.snapshot().unwrap(), valid);
    }
}
