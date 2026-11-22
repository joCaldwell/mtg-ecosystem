//! The committed executor’s read-only public queries.
use super::*;

impl Game {
    /// Cheap trusted journal lengths; after a successful submit/concede, its
    /// zero-based ordinal is the corresponding count minus one.
    pub fn accepted_input_counts(&self) -> Result<AcceptedInputCounts, String> {
        self.committed.accepted_input_counts()
    }
    /// The decision `viewer` owes now, if any.
    pub fn decision_for(&self, viewer: PlayerId) -> Result<Option<Request>, String> {
        self.committed.decision_for(viewer)
    }
    pub fn check_response(
        &self,
        player: PlayerId,
        decision_id: u64,
        response: Response,
    ) -> Result<ValidatedResponse, String> {
        self.committed.check_response(player, decision_id, response)
    }
    /// Trusted host execution boundary; never a player-facing identifier.
    pub fn rule_boundary(&self) -> u64 {
        self.committed.rule_boundary()
    }
    pub fn state_digest(&self) -> Result<String, String> {
        self.committed.state_digest()
    }
    pub fn events(&self) -> &[Event] {
        self.committed.events()
    }
    pub fn config(&self) -> &ScenarioConfig {
        self.committed.config()
    }
    /// Borrow the persistent input log without materializing a flat copy.
    /// Use `iter()` for scans and explicitly collect when owned entries are needed.
    pub fn journal(&self) -> &History<AcceptedInput> {
        self.committed.journal()
    }
    pub fn object_id(&self, card: CardId) -> Option<ObjectId> {
        self.committed.object_id(card)
    }
    pub fn zone(&self, object: ObjectId) -> Option<Zone> {
        self.committed.zone(object)
    }
    pub fn mana(&self, player: PlayerId, color: Color) -> Option<Quantity> {
        self.committed.mana(player, color)
    }
    pub fn characteristics(&self, object: ObjectId) -> Result<Characteristics, String> {
        self.committed.characteristics(object)
    }
    /// Colors the object's mana ability can produce. In `commander-mana-v1`
    /// this describes production only, not current activatability: a tapped
    /// or summoning-sick source still reports its symbols. Use the mana
    /// window's offered sources to decide what can be activated now.
    pub fn mana_abilities(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        self.committed.mana_abilities(object)
    }
    pub fn snapshot(&self) -> Result<String, String> {
        self.committed.snapshot()
    }
    /// Snapshot JSON and its semantic digest, computed once at this boundary.
    /// String convenience still retains the complete output allocation.
    pub fn snapshot_with_digest(&self) -> Result<(String, String), String> {
        self.committed.snapshot_with_digest()
    }
    /// Write every canonical snapshot byte without a complete output String.
    /// The sink may contain a prefix on Err; the host owns its publication.
    pub fn write_snapshot<W: std::io::Write>(
        &self,
        sink: &mut W,
    ) -> Result<SnapshotWriteInfo, String> {
        self.committed.write_snapshot(sink)
    }
    /// Publicly known identities in hand. No targetable handle or hand position
    /// is disclosed. A subsequent zone change invalidates that incarnation.
    pub fn publicly_searched_hand(
        &self,
        viewer: PlayerId,
    ) -> Result<Vec<(PlayerId, String)>, String> {
        self.committed.publicly_searched_hand(viewer)
    }
    pub fn commander_ledger(&self) -> &CommanderLedger {
        self.committed.commander_ledger()
    }
    pub fn live_players(&self) -> Vec<PlayerId> {
        self.committed.live_players()
    }
    pub fn active_player(&self) -> Option<PlayerId> {
        self.committed.active_player()
    }
    pub fn turn_owner(&self) -> PlayerId {
        self.committed.turn_owner()
    }
    pub fn replay_journal(&self) -> ReplayJournal {
        self.committed.replay_journal()
    }
    /// Bind query access to a player already authorized by the host.
    /// Invalid player IDs are rejected by the budgeted query entry points.
    pub fn player_session(&self, viewer: PlayerId) -> PlayerSession<'_> {
        self.committed.player_session(viewer)
    }
    pub fn capture_search(&self, viewer: PlayerId, budget: Budget) -> CaptureOutcome {
        self.committed.capture_search(viewer, budget)
    }
    pub fn resume_search_capture(&self, work: CaptureWork, budget: Budget) -> CaptureOutcome {
        self.committed.resume_search_capture(work, budget)
    }
    pub fn typed_fault(&self) -> Option<&RuleFault> {
        self.committed.typed_fault()
    }
    pub fn trace(&self) -> &TraceEvidence {
        self.committed.trace()
    }
    pub fn turn(&self) -> &TurnState {
        self.committed.turn()
    }
    pub fn observe(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.committed.observe(viewer, budget)
    }
    pub fn player_decision_for(&self, viewer: PlayerId, budget: Budget) -> QueryResult {
        self.committed.player_decision_for(viewer, budget)
    }
    pub fn check_player_response(
        &self,
        viewer: PlayerId,
        token: DecisionToken,
        response: PlayerResponse,
        budget: Budget,
    ) -> QueryResult {
        self.committed
            .check_player_response(viewer, token, response, budget)
    }
    pub fn search_context(
        &self,
        viewer: PlayerId,
        sampling_seed: String,
        budget: Budget,
    ) -> QueryResult {
        self.committed.search_context(viewer, sampling_seed, budget)
    }
    pub fn resume_work(&self, work: Work, budget: Budget) -> QueryResult {
        self.committed.resume_work(work, budget)
    }
    pub fn save_work(&self, work: &Work) -> Result<String, String> {
        self.committed.save_work(work)
    }
    pub fn restore_work(&self, json: &str) -> Result<Work, String> {
        self.committed.restore_work(json)
    }
}
