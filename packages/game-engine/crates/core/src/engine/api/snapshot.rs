//! Snapshot encoding, authenticated restore and journal replay.
use crate::engine::*;

impl RulesTxn {
    pub fn snapshot(&self) -> Result<String, String> {
        self.snapshot_with_digest().map(|(json, _)| json)
    }
    pub(in crate::engine) fn snapshot_ref<'a>(&'a self, state_sha256: &'a str) -> SnapshotRef<'a> {
        SnapshotRef {
            snapshot_version: crate::formats::SNAPSHOT_VERSION,
            build_id: build_id(),
            catalog_digest: &self.prepared.catalog_digest,
            profile_digest: &self.prepared.profile_digest,
            state_sha256,
            state: &self.state,
            trace: &self.trace,
        }
    }
    /// Snapshot JSON and its semantic digest, computed once at this boundary.
    /// String convenience still retains the complete output allocation.
    pub fn snapshot_with_digest(&self) -> Result<(String, String), String> {
        let state_sha256 = self.state_digest()?;
        let json = snapshot_codec::encode(&self.snapshot_ref(&state_sha256))?;
        Ok((json, state_sha256))
    }
    /// Write every canonical snapshot byte without a complete output String.
    /// The sink may contain a prefix on Err; the host owns its publication.
    pub fn write_snapshot<W: std::io::Write>(
        &self,
        sink: &mut W,
    ) -> Result<SnapshotWriteInfo, String> {
        let state_sha256 = self.state_digest()?;
        let raw = snapshot_codec::write_counted(&self.snapshot_ref(&state_sha256), sink)?;
        Ok(SnapshotWriteInfo {
            bytes: raw.bytes,
            snapshot_sha256: raw.sha256,
            state_sha256,
        })
    }
}
impl Game {
    pub(crate) fn read_snapshot(prepared: Prepared, json: &str) -> Result<(Self, String), String> {
        Self::admit_decoded_snapshot(prepared, strict_read(json)?)
    }
    pub(crate) fn read_snapshot_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<(Self, String), String> {
        Self::admit_decoded_snapshot(prepared, strict_read_seekable(source)?)
    }
    // Exactly one typed verification body for string and seekable input. No
    // RulesTxn is returned before unchanged identity/hash/invariant checks pass.
    pub(in crate::engine) fn admit_decoded_snapshot(
        prepared: Prepared,
        s: Snapshot,
    ) -> Result<(Self, String), String> {
        if s.snapshot_version != crate::formats::SNAPSHOT_VERSION
            || s.build_id != build_id()
            || s.catalog_digest != prepared.catalog_digest
            || s.profile_digest != prepared.profile_digest
            || state_digest::state_digest(&s.state)? != s.state_sha256
        {
            return Err("snapshot version/build/catalog/profile/hash mismatch".into());
        }
        let mut game = RulesTxn {
            opening_library_sizes: prepared.config.initial_library_sizes(),
            definition_index: RulesTxn::definition_index(&prepared.catalog, &s.state.rules.cards),
            prepared: Arc::new(prepared),
            state: s.state,
            trace: s.trace,
            cast_reversals: 0,
            stream_buffer: Vec::new(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
            priority_cache: Default::default(),
            in_rule_step: false,
            kernel_leaf: None,
            published: None,
            actual_journal: None,
        };
        game.validate_stack_links()?;
        game.rebuild_kernel_leaf()?;
        game.validate_state()?;
        Ok((Self::from_initial(game), s.state_sha256))
    }
    pub(in crate::engine) fn prove_decoded_snapshot(
        (game, digest): (Self, String),
    ) -> Result<Self, String> {
        SnapshotVerifier::new(game.prepared.as_ref().clone(), game.trace.mode)
            .verify_game(game, digest)
    }
    pub fn restore(prepared: Prepared, json: &str) -> Result<Self, String> {
        Self::prove_decoded_snapshot(Self::read_snapshot(prepared, json)?)
    }
    /// Strict duplicate scan and typed decode bind the same complete raw bytes
    /// from current offset through EOF. Reader position on Err is unspecified.
    /// All ordinary identity/hash/invariant/accepted-input replay proofs remain.
    pub fn restore_from_reader<R: std::io::Read + std::io::Seek>(
        prepared: Prepared,
        source: &mut R,
    ) -> Result<Self, String> {
        Self::prove_decoded_snapshot(Self::read_snapshot_from_reader(prepared, source)?)
    }
    pub fn replay(prepared: Prepared, journal: &ReplayJournal) -> Result<Self, String> {
        Self::replay_to(prepared, journal, None, TraceMode::Detailed)
    }
}
