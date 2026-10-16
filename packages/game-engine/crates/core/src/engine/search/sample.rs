use super::*;

pub struct SampledGame {
    game: Box<Game>,
    capture: SearchCapture,
    proposal: Arc<WorldProposal>,
    origin_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleSnapshot {
    sample_snapshot_version: u32,
    origin_kind: String,
    build_id: String,
    capture_id: String,
    proposal: WorldProposal,
    origin_id: String,
    boundary: u64,
    state_sha256: String,
    journal: ReplayJournal,
}
pub enum SampleRestoreOutcome {
    Complete(SampledGame),
    Incomplete {
        work: SampleRestoreWork,
        required_minimum: u64,
    },
    Rejected(SearchRejection),
}
enum RestoreStage {
    Build(SampleBuildWork),
    Replay(SampledGame),
}
pub struct SampleRestoreWork {
    stage: Option<RestoreStage>,
    snapshot: Box<SampleSnapshot>,
}
impl SampledGame {
    pub(super) fn from_origin(
        game: Game,
        capture: SearchCapture,
        proposal: WorldProposal,
        origin_id: String,
    ) -> Self {
        Self {
            game: Box::new(game),
            capture,
            proposal: Arc::new(proposal),
            origin_id,
        }
    }
    pub fn observe(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.game.observe(viewer, budget)
    }
    pub fn player_decision_for(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.game.player_decision_for(viewer, budget)
    }
    pub fn check_player_response(
        &self,
        viewer: PlayerId,
        token: DecisionToken,
        response: PlayerResponse,
        budget: Budget,
    ) -> QueryResult {
        self.game
            .check_player_response(viewer, token, response, budget)
    }
    pub fn resume_work(&self, work: Work, budget: Budget) -> QueryResult {
        self.game.resume_work(work, budget)
    }
    pub fn save_work(&self, work: &Work) -> Result<String, String> {
        self.game.save_work(work)
    }
    pub fn restore_work(&self, json: &str) -> Result<Work, String> {
        self.game.restore_work(json)
    }
    pub fn submit_validated(&mut self, receipt: ValidatedResponse) -> Result<(), String> {
        self.game.submit(receipt)
    }
    pub fn advance(&mut self, budget: Budget) -> Advance {
        self.game.advance(budget)
    }
    pub fn concede(&mut self, player: PlayerId) -> Result<(), String> {
        self.game.concede(player)
    }
    pub fn state_digest(&self) -> Result<String, String> {
        self.game.state_digest()
    }
    pub fn rule_boundary(&self) -> u64 {
        self.game.rule_boundary()
    }
    pub fn fork(&self) -> Self {
        Self {
            game: Box::new(self.game.fork()),
            capture: self.capture.clone(),
            proposal: self.proposal.clone(),
            origin_id: self.origin_id.clone(),
        }
    }
    pub fn snapshot(&self) -> Result<String, String> {
        canonical(&SampleSnapshot {
            sample_snapshot_version: 1,
            origin_kind: SEARCH_RECONSTRUCTION.into(),
            build_id: BUILD_ID.into(),
            capture_id: self.capture.context_id().into(),
            proposal: (*self.proposal).clone(),
            origin_id: self.origin_id.clone(),
            boundary: self.game.rule_boundary(),
            state_sha256: self.game.state_digest()?,
            journal: self.game.replay_journal(),
        })
    }
    pub fn restore(
        catalog: Catalog,
        capture: &SearchCapture,
        json: &str,
        budget: Budget,
    ) -> SampleRestoreOutcome {
        let snapshot = match strict_read::<SampleSnapshot>(json) {
            Ok(s) => s,
            Err(e) => return SampleRestoreOutcome::Rejected(invalid(e)),
        };
        // Validate proposal bounds before either the origin digest or cloning
        // into resumable construction, just as the direct builder does.
        if let Err(e) = snapshot.proposal.validate_shape() {
            return SampleRestoreOutcome::Rejected(invalid(e));
        }
        if snapshot.sample_snapshot_version != 1
            || snapshot.origin_kind != SEARCH_RECONSTRUCTION
            || snapshot.build_id != BUILD_ID
            || snapshot.capture_id != capture.context_id()
            || snapshot.origin_id
                != digest(&(capture.context_id(), &snapshot.proposal)).unwrap_or_default()
        {
            return SampleRestoreOutcome::Rejected(invalid("sample origin envelope"));
        }
        let work =
            match SampleBuildWork::start_for_restore(catalog, capture, snapshot.proposal.clone()) {
                Ok(work) => work,
                Err(e) => return SampleRestoreOutcome::Rejected(e),
            };
        SampleRestoreWork {
            stage: Some(RestoreStage::Build(work)),
            snapshot: Box::new(snapshot),
        }
        .resume(budget)
    }
}
impl SampleRestoreWork {
    pub fn resume(mut self, mut budget: Budget) -> SampleRestoreOutcome {
        while budget.consume() {
            match self.unit() {
                Ok(Some(game)) => return SampleRestoreOutcome::Complete(game),
                Ok(None) => {}
                Err(e) => return SampleRestoreOutcome::Rejected(e),
            }
        }
        SampleRestoreOutcome::Incomplete {
            work: self,
            required_minimum: 1,
        }
    }
    fn unit(&mut self) -> Result<Option<SampledGame>, SearchRejection> {
        let stage = self
            .stage
            .take()
            .ok_or_else(|| invariant("restore stage"))?;
        match stage {
            RestoreStage::Build(work) => match work.resume(Budget::new(1)) {
                SampleBuildOutcome::Complete { game, .. } => {
                    self.stage = Some(RestoreStage::Replay(game))
                }
                SampleBuildOutcome::Incomplete { work, .. } => {
                    self.stage = Some(RestoreStage::Build(work))
                }
                SampleBuildOutcome::Rejected(e) => return Err(e),
            },
            RestoreStage::Replay(mut sampled) => {
                let game = &mut sampled.game;
                let n = game.state.journal.len();
                let m = game.state.external_inputs.len();
                let snapshot = &self.snapshot;
                if game.state.rule_steps > snapshot.boundary {
                    return Err(invalid("sample replay passed boundary"));
                }
                if n == snapshot.journal.decisions.len()
                    && m == snapshot.journal.concessions.len()
                    && game.state.rule_steps == snapshot.boundary
                {
                    if game.state_digest().map_err(invariant)? != snapshot.state_sha256 {
                        return Err(invalid(
                            "sample state is not reachable from validated origin and future inputs",
                        ));
                    }
                    return Ok(Some(sampled));
                }
                let next_external = snapshot.journal.concessions.get(m);
                if let Some(input) = next_external {
                    if input.after_decisions < n as u64
                        || input.boundary < game.state.rule_steps
                        || input.boundary > snapshot.boundary
                    {
                        return Err(invalid("sample concession ordering/boundary"));
                    }
                    if input.after_decisions == n as u64 && input.boundary == game.state.rule_steps
                    {
                        game.concede(input.player).map_err(invalid)?;
                        if game.state.external_inputs.last() != Some(input) {
                            return Err(invalid("sample concession mismatch"));
                        }
                        self.stage = Some(RestoreStage::Replay(sampled));
                        return Ok(None);
                    }
                }
                if next_external.is_none_or(|input| input.after_decisions > n as u64)
                    && let Some(input) = snapshot.journal.decisions.get(n)
                {
                    match game.advance(Budget::new(u64::from(
                        game.state.rule_steps < snapshot.boundary,
                    ))) {
                        Advance::AwaitingDecision(request) => {
                            if request.player() != input.player
                                || request.decision_id() != input.decision_id
                            {
                                return Err(invalid("sample replay decision identity"));
                            }
                            game.respond(input.player, input.decision_id, input.response.clone())
                                .map_err(invalid)?;
                        }
                        Advance::Suspended { .. } => {}
                        _ => return Err(invalid("sample future decision is unreachable")),
                    }
                } else if game.state.rule_steps < snapshot.boundary {
                    let before = game.state.rule_steps;
                    let _ = game.advance(Budget::new(1));
                    if before == game.state.rule_steps {
                        return Err(invalid("sample replay boundary is unreachable"));
                    }
                } else {
                    return Err(invalid("sample journal exceeds claimed boundary"));
                }
                self.stage = Some(RestoreStage::Replay(sampled));
            }
        }
        Ok(None)
    }
}
