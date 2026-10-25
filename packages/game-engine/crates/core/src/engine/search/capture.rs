use super::*;

pub enum CaptureOutcome {
    Complete(SearchCapture),
    Incomplete {
        work: CaptureWork,
        required_minimum: u64,
    },
    Rejected(SearchRejection),
}
pub struct CaptureWork {
    executor: Arc<()>,
    state_token: Arc<()>,
    viewer: PlayerId,
    query: Option<Box<Work>>,
    data: Option<Box<SearchCaptureData>>,
    action_cursor: usize,
    history_cursor: usize,
    object_cursor: usize,
    seen_public: BTreeSet<(PlayerId, String)>,
    history_sink: Option<Box<Sha256Sink>>,
}
impl Game {
    fn classify_search_root(&self, viewer: PlayerId) -> Result<(), SearchRejection> {
        if !(self.prepared.config.mode == Mode::CommanderMatch)
            || self.prepared.match_config.is_none()
        {
            return Err(unsupported(
                "requires an admitted search-enabled Commander profile",
            ));
        }
        if viewer.0 >= 4
            || !self.is_live(viewer)
            || self.state.active != Some(viewer)
            || self.state.priority != viewer
            || self.state.turn.step != TurnStep::PrecombatMain
            || !matches!(self.state.frame, Frame::Priority)
            || !self.state.passes.is_empty()
            || self.state.pending.is_some()
            || !self.state.pending_concessions.is_empty()
            || !self.state.stack.is_empty()
            || !self.state.pending_triggers.is_empty()
            || !self.state.effects.is_empty()
            || !self.state.combat.is_empty()
            || self.state.combat_declared
            || !self.state.damage_choices.is_empty()
            || !self.state.commander_returns.is_empty()
            || !self.state.control_effects.is_empty()
            || self.state.cleanup_repeat
            || self.state.simultaneous_losses
            || !self.state.losing_players.is_empty()
            || self
                .state
                .match_state
                .as_ref()
                .is_none_or(|m| m.setup.is_some())
            || !(2..=4).contains(&self.live_players().len())
        {
            return Err(unsupported(
                "not a fresh active precombat empty-window root",
            ));
        }
        // P5 constructors admit no control-changing, bounce, library rearranging,
        // recursion or copy mechanics. This exact catalog/profile gate is part
        // of the historical public-to-hidden information proof below.
        if self
            .prepared
            .match_config
            .as_ref()
            .and_then(|c| match_profile(&c.profile))
            .is_none_or(|p| !p.search || p.catalog_sha256 != self.prepared.catalog_digest)
            || self.state.objects.values().any(|o| {
                o.marked_damage != Quantity::zero()
                    || self
                        .state
                        .cards
                        .get(&o.id.card)
                        .is_none_or(|c| o.default_controller != c.owner || o.controller != c.owner)
            })
        {
            return Err(unsupported(
                "root needs zero marks and owner-default control",
            ));
        }
        Ok(())
    }
    pub fn capture_search(&self, viewer: PlayerId, budget: Budget) -> CaptureOutcome {
        if let Err(e) = self.classify_search_root(viewer) {
            return CaptureOutcome::Rejected(e);
        }
        let query = match self.search_context(viewer, "capture-v1".into(), Budget::new(0)) {
            QueryResult::Incomplete { work, .. } => work,
            _ => return CaptureOutcome::Rejected(invariant("capture query initialization")),
        };
        self.resume_search_capture(
            CaptureWork {
                executor: self.executor.clone(),
                state_token: self.state_token.clone(),
                viewer,
                query: Some(Box::new(query)),
                data: None,
                action_cursor: 0,
                history_cursor: 0,
                object_cursor: 0,
                seen_public: BTreeSet::new(),
                history_sink: Some(Box::new(Sha256Sink::new())),
            },
            budget,
        )
    }
    pub fn resume_search_capture(
        &self,
        mut work: CaptureWork,
        mut budget: Budget,
    ) -> CaptureOutcome {
        if !Arc::ptr_eq(&work.executor, &self.executor)
            || !Arc::ptr_eq(&work.state_token, &self.state_token)
        {
            return CaptureOutcome::Rejected(SearchRejection::StaleWork);
        }
        while budget.consume() {
            match self.capture_unit(&mut work) {
                Ok(Some(capture)) => return CaptureOutcome::Complete(capture),
                Ok(None) => {}
                Err(e) => return CaptureOutcome::Rejected(e),
            }
        }
        CaptureOutcome::Incomplete {
            work,
            required_minimum: 1,
        }
    }
    fn capture_unit(&self, w: &mut CaptureWork) -> Result<Option<SearchCapture>, SearchRejection> {
        if let Some(query) = w.query.take() {
            match self.resume_work(*query, Budget::new(1)) {
                QueryResult::Incomplete { work, .. } => w.query = Some(Box::new(work)),
                QueryResult::Complete(QueryValue::SearchContext(context)) => {
                    let config = self
                        .prepared
                        .match_config
                        .as_ref()
                        .ok_or_else(|| invariant("capture pod"))?;
                    w.data = Some(Box::new(SearchCaptureData {
                        capture_version: 1,
                        profile: PublicSearchProfile {
                            reconstruction_schema: SEARCH_RECONSTRUCTION.into(),
                            information_policy: SEARCH_INFORMATION.into(),
                            engine_build_id: BUILD_ID.into(),
                            catalog_digest: self.prepared.catalog_digest.clone(),
                            rules_digest: RULES_SHA256.into(),
                            commander_policy_digest: policy_digest(),
                            match_profile: config.profile.clone(),
                            decks: config.decks.clone(),
                            starting_player: config.starting_player,
                        },
                        context: CapturedSearchContext {
                            observation: context.observation,
                            permitted_history: context.permitted_history,
                        },
                        own_knowledge: vec![],
                        known_bottom: vec![],
                        known_hands: vec![],
                    }));
                }
                _ => return Err(invariant("capture observation query failed")),
            }
            return Ok(None);
        }
        let data = w
            .data
            .as_mut()
            .ok_or_else(|| invariant("capture payload"))?;
        if let Some(action) = self.state.visibility[w.viewer.0 as usize]
            .own_knowledge
            .get(w.action_cursor)
        {
            data.own_knowledge.push(action.clone());
            w.action_cursor += 1;
            return Ok(None);
        }
        if let Some(view) = data.context.permitted_history.stored(w.history_cursor) {
            if view.viewer != w.viewer {
                return Err(SearchRejection::UnsupportedHistory(
                    "foreign viewer history".into(),
                ));
            }
            if let Some(object) = view.objects.get(w.object_cursor) {
                w.object_cursor += 1;
                if object.owner != w.viewer
                    && !matches!(object.zone, Zone::Hand { .. } | Zone::Library { .. })
                {
                    w.seen_public
                        .insert((object.owner, object.oracle_id.clone()));
                }
            } else {
                let sink = w
                    .history_sink
                    .as_mut()
                    .ok_or_else(|| invariant("capture history hash state"))?;
                sink.update(if w.history_cursor == 0 { b"[" } else { b"," });
                view.write_canonical(sink).map_err(invariant)?;
                w.history_cursor += 1;
                w.object_cursor = 0;
            }
            return Ok(None);
        }
        let view = &data.context.observation.view;
        let own = view
            .players
            .iter()
            .find(|p| p.player == w.viewer)
            .ok_or_else(|| invariant("capture owner"))?;
        data.known_bottom = known_suffix(&data.own_knowledge, own.library_count as usize)?;
        for (player, oracle_id) in &w.seen_public {
            if view
                .players
                .iter()
                .any(|p| p.player == *player && p.status == PlayerStatus::Live)
                && !view
                    .objects
                    .iter()
                    .any(|o| o.owner == *player && o.oracle_id == *oracle_id)
            {
                data.known_hands.push(KnownHandMemory {
                    player: *player,
                    oracle_id: oracle_id.clone(),
                });
            }
        }
        // Sealed actual origin proves the history was reachable. This finite
        // constraint projection preserves its future-relevant identities and
        // positions; it does not claim a proposed root has a ChaCha setup seed.
        let data = w
            .data
            .take()
            .ok_or_else(|| invariant("capture completion"))?;
        // The identity covers the complete ordered history without allocating
        // an unbounded canonical Value/string for it. Each view was hashed at
        // its own budgeted cursor. The domain/version pins this composition.
        let mut sink = w
            .history_sink
            .take()
            .ok_or_else(|| invariant("capture history hash completion"))?;
        if w.history_cursor == 0 {
            sink.update(b"[");
        }
        sink.update(b"]");
        let history_digest = sink.finalize_hex();
        let id = capture_identity(&data, &history_digest, true).map_err(invariant)?;
        let sampler_identity =
            capture_identity(&data, &history_digest, false).map_err(invariant)?;
        Ok(Some(SearchCapture {
            data: Arc::new(*data),
            id,
            sampler_identity,
        }))
    }
}

// Build identity scopes permissions/restore, but is not strategy entropy. Otherwise
// even a byte-preserving optimization would change every sampled world, preventing
// identical-worklist comparisons. Every permitted semantic field stays bound.
pub(super) fn capture_identity(
    data: &SearchCaptureData,
    history_digest: &str,
    bind_build: bool,
) -> Result<String, String> {
    let mut profile = data.profile.clone();
    if !bind_build {
        profile.engine_build_id.clear();
    }
    digest(&(
        if bind_build {
            "capture-identity-v1"
        } else {
            "sampler-permitted-identity-v2"
        },
        data.capture_version,
        profile,
        &data.context.observation,
        &data.own_knowledge,
        &data.known_bottom,
        &data.known_hands,
        history_digest,
    ))
}
