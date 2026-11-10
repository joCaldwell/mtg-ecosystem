use super::*;

/// Read-only native query access for one host-authorized player.
///
/// Hosts authenticate the caller before constructing this facade. Policies can
/// receive a session without receiving the authoritative `Game`; queries remain
/// budgeted, pure, and scoped to the bound viewer, executor, and state epoch.
///
/// `Work` and `ValidatedResponse` remain opaque host objects. A host may resume
/// work or submit a completed receipt; neither is a client transport payload.
/// Only the existing player projections/requests cross that boundary.
///
/// The facade deliberately provides no snapshot or raw-state access:
///
/// ```compile_fail
/// use mtg_core::PlayerSession;
/// fn snapshot_is_not_player_data(session: &PlayerSession<'_>) {
///     session.snapshot();
/// }
/// ```
pub struct PlayerSession<'a> {
    game: &'a Game,
    viewer: PlayerId,
}

impl Game {
    /// Bind query access to a player already authorized by the host.
    /// Invalid player IDs are rejected by the budgeted query entry points.
    pub fn player_session(&self, viewer: PlayerId) -> PlayerSession<'_> {
        PlayerSession { game: self, viewer }
    }
}

impl PlayerSession<'_> {
    pub fn observe(&self, budget: Budget) -> QueryResult {
        self.game.observe(self.viewer, budget)
    }

    pub fn decision(&self, budget: Budget) -> QueryResult {
        self.game.player_decision_for(self.viewer, budget)
    }

    pub fn check_response(
        &self,
        token: DecisionToken,
        response: PlayerResponse,
        budget: Budget,
    ) -> QueryResult {
        self.game
            .check_player_response(self.viewer, token, response, budget)
    }

    pub fn resume(&self, work: Work, budget: Budget) -> QueryResult {
        if !work.belongs_to_viewer(self.viewer) {
            return QueryResult::Rejected(QueryRejection::StaleOrForeignWork);
        }
        self.game.resume_work(work, budget)
    }

    pub fn search_context(&self, sampling_seed: String, budget: Budget) -> QueryResult {
        self.game.search_context(self.viewer, sampling_seed, budget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(
            Prepared::admit(
                Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
                ScenarioConfig::land_return(),
            )
            .unwrap(),
        )
    }

    fn finish(session: &PlayerSession<'_>, mut result: QueryResult) -> QueryValue {
        for _ in 0..100 {
            result = match result {
                QueryResult::Complete(value) => return value,
                QueryResult::Incomplete { work, .. } => session.resume(work, Budget::new(1)),
                QueryResult::Rejected(reason) => panic!("unexpected query rejection: {reason:?}"),
            };
        }
        panic!("query made no progress")
    }

    fn decision(session: &PlayerSession<'_>) -> PlayerRequest {
        let QueryValue::Decision(Some(request)) = finish(session, session.decision(Budget::new(0)))
        else {
            panic!("expected the bound player's decision")
        };
        request
    }

    fn pass_receipt(session: &PlayerSession<'_>) -> ValidatedResponse {
        let request = decision(session);
        let QueryValue::Validated(receipt) = finish(
            session,
            session.check_response(
                request.token().clone(),
                PlayerResponse::Pass,
                Budget::new(0),
            ),
        ) else {
            panic!("expected host validation receipt")
        };
        receipt
    }

    #[test]
    fn sessions_keep_views_private_and_queries_pure() {
        let game = game();
        let before = game.snapshot().unwrap();
        for viewer in [PlayerId(0), PlayerId(1)] {
            let session = game.player_session(viewer);
            let QueryValue::Observation(observation) =
                finish(&session, session.observe(Budget::new(0)))
            else {
                panic!("expected observation")
            };
            assert_eq!(observation.view.viewer, viewer);
            assert_eq!(
                observation
                    .view
                    .objects
                    .iter()
                    .any(|o| o.oracle_id == SPELL),
                viewer == PlayerId(0)
            );
            let QueryValue::Decision(request) = finish(&session, session.decision(Budget::new(0)))
            else {
                panic!("expected decision result")
            };
            assert_eq!(request, observation.decision);
            let QueryValue::SearchContext(context) = finish(
                &session,
                session.search_context("session-seed".into(), Budget::new(0)),
            ) else {
                panic!("expected permitted search context")
            };
            let QueryResult::Complete(QueryValue::SearchContext(expected)) =
                game.search_context(viewer, "session-seed".into(), Budget::unlimited())
            else {
                panic!("expected direct permitted context")
            };
            assert_eq!(context, expected);
        }
        let _ = pass_receipt(&game.player_session(PlayerId(0)));
        assert_eq!(game.snapshot().unwrap(), before);
    }

    #[test]
    fn sessions_reject_invalid_viewers_and_another_players_response_token() {
        let game = game();
        let invalid = game.player_session(PlayerId(4));
        for result in [
            invalid.observe(Budget::new(0)),
            invalid.decision(Budget::new(0)),
            invalid.check_response(
                DecisionToken("invalid".into()),
                PlayerResponse::Pass,
                Budget::new(0),
            ),
            invalid.search_context("seed".into(), Budget::new(0)),
        ] {
            assert!(matches!(
                result,
                QueryResult::Rejected(QueryRejection::InvalidViewer)
            ));
        }
        let token = decision(&game.player_session(PlayerId(0))).token().clone();
        assert!(matches!(
            game.player_session(PlayerId(1)).check_response(
                token,
                PlayerResponse::Pass,
                Budget::unlimited()
            ),
            QueryResult::Rejected(QueryRejection::InvalidResponse)
        ));
    }

    #[test]
    fn sessions_cannot_resume_another_viewers_work_of_any_query_kind() {
        let game = game();
        let own = game.player_session(PlayerId(0));
        let foreign = game.player_session(PlayerId(1));
        let token = decision(&own).token().clone();
        for result in [
            own.observe(Budget::new(0)),
            own.decision(Budget::new(0)),
            own.check_response(token, PlayerResponse::Pass, Budget::new(0)),
            own.search_context("seed".into(), Budget::new(0)),
        ] {
            let QueryResult::Incomplete { work, .. } = result else {
                panic!("expected deferred work")
            };
            assert!(matches!(
                foreign.resume(work, Budget::unlimited()),
                QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
            ));
        }
    }

    #[test]
    fn sessions_preserve_executor_and_epoch_scoping_for_work_and_receipts() {
        let mut game = game();
        let fork = game.fork();
        let QueryResult::Incomplete { work, .. } =
            game.player_session(PlayerId(0)).observe(Budget::new(0))
        else {
            panic!("expected deferred work")
        };
        assert!(matches!(
            fork.player_session(PlayerId(0))
                .resume(work, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
        ));
        let foreign_receipt = pass_receipt(&fork.player_session(PlayerId(0)));
        assert!(game.submit(foreign_receipt).is_err());

        let stale = pass_receipt(&game.player_session(PlayerId(0)));
        let accepted = pass_receipt(&game.player_session(PlayerId(0)));
        let QueryResult::Incomplete { work, .. } =
            game.player_session(PlayerId(0)).observe(Budget::new(0))
        else {
            panic!("expected deferred work")
        };
        game.submit(accepted).unwrap();
        assert!(game.submit(stale).is_err());
        assert!(matches!(
            game.player_session(PlayerId(0))
                .resume(work, Budget::unlimited()),
            QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
        ));
    }
}
