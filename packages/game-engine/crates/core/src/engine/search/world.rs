use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HypotheticalCard {
    pub symbol: String,
    pub oracle_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatHypothesis {
    pub hidden_hand: Vec<HypotheticalCard>,
    pub ordered_library: Vec<HypotheticalCard>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldProposal {
    pub proposal_version: u32,
    pub context_id: String,
    pub sampler_version: String,
    pub world_index: u64,
    pub seats: [SeatHypothesis; 4],
    pub future_seed: [u8; 32],
}
impl WorldProposal {
    pub(super) fn validate_shape(&self) -> Result<(), String> {
        if self.context_id.len() != 64 || self.sampler_version.len() > 64 {
            return Err("proposal identity length bound".into());
        }
        for seat in &self.seats {
            if seat
                .hidden_hand
                .len()
                .saturating_add(seat.ordered_library.len())
                > 100
            {
                return Err("hypothetical seat exceeds hundred-card bound".into());
            }
            for card in seat.hidden_hand.iter().chain(&seat.ordered_library) {
                if card.symbol.is_empty() || card.symbol.len() > 128 || card.oracle_id.len() != 36 {
                    return Err("hypothetical identity length bound".into());
                }
            }
        }
        Ok(())
    }
    pub fn assignment_digest(&self) -> Result<String, String> {
        self.validate_shape()?;
        let seats = self
            .seats
            .iter()
            .map(|seat| {
                let mut hand = seat
                    .hidden_hand
                    .iter()
                    .map(|c| c.oracle_id.as_str())
                    .collect::<Vec<_>>();
                hand.sort_unstable();
                (
                    hand,
                    seat.ordered_library
                        .iter()
                        .map(|c| c.oracle_id.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        digest(&(&self.context_id, seats))
    }
}
pub enum SampleBuildOutcome {
    Complete {
        game: SampledGame,
        root_map: RootHandleMap,
    },
    Incomplete {
        work: SampleBuildWork,
        required_minimum: u64,
    },
    Rejected(SearchRejection),
}
struct Assignment {
    owner: PlayerId,
    oracle_id: String,
    zone: Zone,
    visible: Option<VisibleObject>,
    symbol: Option<String>,
}
pub struct SampleBuildWork {
    capture: SearchCapture,
    proposal: Box<WorldProposal>,
    game: Box<RulesTxn>,
    rows: Vec<Assignment>,
    cursor: usize,
    unused: BTreeMap<(PlayerId, String), BTreeSet<CardId>>,
    symbols: BTreeSet<String>,
    mapping: Vec<(ObjectHandle, ObjectId)>,
    origin_id: String,
}
impl SampledGame {
    pub fn build(
        catalog: Catalog,
        capture: &SearchCapture,
        proposal: WorldProposal,
        budget: Budget,
    ) -> SampleBuildOutcome {
        match SampleBuildWork::new(catalog, capture, proposal) {
            Ok(work) => work.resume(budget),
            Err(e) => SampleBuildOutcome::Rejected(e),
        }
    }
}
impl SampleBuildWork {
    pub(super) fn start_for_restore(
        catalog: Catalog,
        capture: &SearchCapture,
        proposal: WorldProposal,
    ) -> Result<Self, SearchRejection> {
        Self::new(catalog, capture, proposal)
    }
    fn new(
        catalog: Catalog,
        capture: &SearchCapture,
        proposal: WorldProposal,
    ) -> Result<Self, SearchRejection> {
        // Defensive input validation precedes every clone, sort and digest.
        // The fixed P5 table bounds admission/construction initialization by
        // 400 cards; per-assignment interpreter construction remains resumable.
        proposal.validate_shape().map_err(invalid)?;
        let data = capture.data();
        let profile = &data.profile;
        if proposal.proposal_version != crate::formats::PROPOSAL_VERSION
            || proposal.context_id != capture.context_id()
            || proposal.sampler_version != SEARCH_SAMPLER
        {
            return Err(invalid("proposal version, capture identity or sampler"));
        }
        if match_profile(&profile.match_profile).is_none_or(|p| {
            p.require_feature(&catalog, Feature::Search).is_err()
                || p.catalog_sha256 != profile.catalog_digest
        }) || profile.reconstruction_schema != SEARCH_RECONSTRUCTION
            || profile.information_policy != SEARCH_INFORMATION
            || profile.engine_build_id != build_id()
            || profile.rules_digest != RULES_SHA256
            || profile.commander_policy_digest
                != policy_digest(&profile.match_profile).map_err(invariant)?
            || profile.catalog_digest != digest(&catalog).map_err(invariant)?
        {
            return Err(invalid("catalog or public profile identity"));
        }
        let config = MatchConfig {
            profile: profile.match_profile.clone(),
            decks: profile.decks.clone(),
            starting_player: profile.starting_player,
            seed: proposal.future_seed,
        };
        let prepared = Prepared::admit_pod(catalog, config).map_err(|e| invalid(e.to_string()))?;
        let mut game = RulesTxn::with_trace(prepared, TraceMode::Off);
        let origin_id = digest(&(capture.context_id(), &proposal)).map_err(invariant)?;
        let view = &data.context.observation.view;
        // All scalar future-relevant facts are public. All inert fields remain
        // the explicitly reviewed fresh-origin values from RulesTxn::new: empty
        // stack/passes/effects/combat/queues/journal, zero counters, no failed
        // draws or pending fresh Commander opportunities. Setup is NOT rerun.
        game.rules_mut().objects.clear();
        game.rules_mut().libraries.clear();
        game.rules_mut().active = view.active;
        game.rules_mut().turn_owner = view.turn_owner;
        game.rules_mut().turn = view.turn.clone();
        game.rules_mut().priority = view.viewer;
        game.rules_mut().frame = Frame::Priority;
        game.rules_mut().match_state = Some(MatchState {
            library_shuffles: [0; 4],
            random: MatchRandom::new(proposal.future_seed),
            setup: None,
        });
        for knowledge in &mut game.state.knowledge.visibility {
            *knowledge = visibility::Knowledge::default().into();
            // A permission namespace is public metadata, never a hash of the
            // hidden proposal/capture. Otherwise every policy view would carry
            // a hidden-world fingerprint. Executor/state brands isolate worlds.
            knowledge.namespace = Some("hypothesis-v1".into());
        }
        game.state.knowledge.visibility[view.viewer.0 as usize].own_knowledge =
            data.own_knowledge.clone();
        for row in &view.players {
            game.rules_mut()
                .match_state
                .as_mut()
                .expect("sample match")
                .library_shuffles[usize::from(row.player.0)] = row.library_shuffles;
            let p = game
                .rules_mut()
                .players
                .get_mut(row.player.0 as usize)
                .ok_or_else(|| invariant("public player"))?;
            p.life_frozen_until_next_turn = row.life_frozen_until_next_turn;
            p.life = row.life.clone();
            p.mana = row.mana.clone();
            // CR 106.4a-b, 106.6: the public pool, restricted buckets included.
            p.restricted_mana = row.restricted_mana.clone();
            p.status = row.status;
            p.last_known = (row.status == PlayerStatus::Left).then(|| PlayerLastKnown {
                life: row.life.clone(),
                mana: row.mana.clone(),
                restricted_mana: row.restricted_mana.clone(),
            });
        }
        for row in &view.commanders {
            let owner = row.history.owner;
            let card = CardId(u32::from(owner.0) * 100);
            let expected = game
                .state
                .rules
                .cards
                .get(&card)
                .ok_or_else(|| invariant("public commander owner"))?;
            if expected.oracle_id != row.oracle_id {
                return Err(invariant("public commander identity"));
            }
            game.rules_mut()
                .commanders
                .records
                .insert(card, row.history.clone());
        }
        let mut unused = BTreeMap::<_, BTreeSet<_>>::new();
        for (id, card) in game.state.rules.cards.iter() {
            unused
                .entry((card.owner, card.oracle_id.clone()))
                .or_default()
                .insert(*id);
        }
        let mut rows = vec![];
        for object in &view.objects {
            if object.phased_out {
                return Err(invariant(
                    "world reconstruction requires private phasing return authority",
                ));
            }
            if matches!(object.zone, Zone::Library { .. } | Zone::Stack)
                || matches!(object.zone,Zone::Hand{owner} if owner!=view.viewer)
            {
                return Err(invariant("capture contains hidden or stack object"));
            }
            rows.push(Assignment {
                owner: object.owner,
                oracle_id: object.oracle_id.clone(),
                zone: object.zone,
                visible: Some(object.clone()),
                symbol: None,
            });
        }
        for (index, seat) in proposal.seats.iter().enumerate() {
            let owner = PlayerId(index as u8);
            let public = view
                .players
                .iter()
                .find(|p| p.player == owner)
                .ok_or_else(|| invariant("seat public counts"))?;
            let expected_hand = if owner == view.viewer {
                0
            } else {
                public.hand_count as usize
            };
            if seat.hidden_hand.len() != expected_hand
                || seat.ordered_library.len() != public.library_count as usize
                || (public.status == PlayerStatus::Left
                    && (!seat.hidden_hand.is_empty() || !seat.ordered_library.is_empty()))
            {
                return Err(invalid("complete hidden hand/library counts required"));
            }
            // Hand order is not a world distinction or physical identity input.
            let mut hand = seat.hidden_hand.iter().collect::<Vec<_>>();
            hand.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id).then(a.symbol.cmp(&b.symbol)));
            for card in hand {
                rows.push(Assignment {
                    owner,
                    oracle_id: card.oracle_id.clone(),
                    zone: Zone::Hand { owner },
                    visible: None,
                    symbol: Some(card.symbol.clone()),
                });
            }
            for card in &seat.ordered_library {
                rows.push(Assignment {
                    owner,
                    oracle_id: card.oracle_id.clone(),
                    zone: Zone::Library { owner },
                    visible: None,
                    symbol: Some(card.symbol.clone()),
                });
            }
        }
        let own_library = &proposal.seats[view.viewer.0 as usize].ordered_library;
        if own_library.len() < data.known_bottom.len()
            || own_library[own_library.len() - data.known_bottom.len()..]
                .iter()
                .zip(&data.known_bottom)
                .any(|(a, b)| a.oracle_id != b.oracle_id)
        {
            return Err(invalid("known bottom suffix/order"));
        }
        for known in &data.known_hands {
            if !proposal.seats[known.player.0 as usize]
                .hidden_hand
                .iter()
                .any(|c| c.oracle_id == known.oracle_id)
            {
                return Err(invalid("remembered opponent hand identity missing"));
            }
        }
        Ok(Self {
            capture: capture.clone(),
            proposal: Box::new(proposal),
            game: Box::new(game),
            rows,
            cursor: 0,
            unused,
            symbols: BTreeSet::new(),
            mapping: vec![],
            origin_id,
        })
    }
    pub fn resume(mut self, mut budget: Budget) -> SampleBuildOutcome {
        while budget.consume() {
            match self.unit() {
                Ok(Some(map)) => {
                    return SampleBuildOutcome::Complete {
                        game: SampledGame::from_origin(
                            Game::from_initial(*self.game),
                            self.capture,
                            *self.proposal,
                            self.origin_id,
                        ),
                        root_map: map,
                    };
                }
                Ok(None) => {}
                Err(e) => return SampleBuildOutcome::Rejected(e),
            }
        }
        SampleBuildOutcome::Incomplete {
            work: self,
            required_minimum: 1,
        }
    }
    fn unit(&mut self) -> Result<Option<RootHandleMap>, SearchRejection> {
        if let Some(row) = self.rows.get(self.cursor) {
            if let Some(symbol) = &row.symbol
                && (symbol.is_empty() || symbol.len() > 128 || !self.symbols.insert(symbol.clone()))
            {
                return Err(invalid("duplicate/empty/oversized local occurrence symbol"));
            }
            let copies = self
                .unused
                .get_mut(&(row.owner, row.oracle_id.clone()))
                .ok_or_else(|| invalid("card outside original owner deck"))?;
            let id = copies
                .pop_first()
                .ok_or_else(|| invalid("definition count exceeds original deck"))?;
            let object = ObjectId {
                face: Default::default(),
                card: id,
                incarnation: 0,
            };
            let (controller, tapped, ready, marked) = match &row.visible {
                Some(v) => (
                    v.controller,
                    v.tapped,
                    v.attack_ready,
                    v.marked_damage.clone(),
                ),
                None => (row.owner, false, false, Quantity::zero()),
            };
            let turn = self.game.state.rules.turn.number;
            self.game.rules_mut().objects.insert(
                id,
                Object {
                    hideaway: None,
                    phased_out: None,
                    attached_to: None,
                    attachment_timestamp: None,
                    battlefield_entry_turn: row
                        .visible
                        .as_ref()
                        .filter(|v| v.entered_this_turn)
                        .map(|_| turn),
                    // CR 702.30a: echo history is not in the permitted
                    // view, and echo is outside the reviewed search tier.
                    control_upkeeps: (row.zone == Zone::Battlefield).then_some(0),
                    keyword_timestamp: None,
                    keyword_rank: 0,
                    plus_one_counters: Quantity::zero(),
                    loyalty_counters: Quantity::zero(),
                    loyalty_used_turn: None,
                    counters: vec![],
                    transforms: 0,
                    adventure_player: None,
                    // Rebuilds even the viewer's own face-down permanents face
                    // up; face-down cards are outside the reviewed search tier,
                    // and this must change before they are promoted to it.
                    face_down: false,
                    deathtouch_damage: false,
                    unearthed: false,
                    dashed: false,
                    id: object,
                    zone: row.zone,
                    controller,
                    default_controller: row.owner,
                    tapped,
                    attack_ready: ready,
                    marked_damage: marked,
                },
            );
            if matches!(row.zone, Zone::Library { .. }) {
                self.game
                    .rules_mut()
                    .libraries
                    .entry(row.owner)
                    .or_default()
                    .push(id);
            }
            if let Some(v) = &row.visible {
                self.mapping.push((v.handle.clone(), object));
            }
            self.cursor += 1;
            return Ok(None);
        }
        for player in &self.capture.data().context.observation.view.players {
            if player.status == PlayerStatus::Live
                && self
                    .unused
                    .iter()
                    .any(|((p, _), ids)| *p == player.player && !ids.is_empty())
            {
                return Err(invalid("proposal omits live owner's physical cards"));
            }
            self.game
                .rules_mut()
                .libraries
                .entry(player.player)
                .or_default();
        }
        self.game.refresh_visibility().map_err(invariant)?;
        self.game.validate_state().map_err(invariant)?;
        let viewer = self.capture.data().context.observation.view.viewer;
        let pairs = self
            .mapping
            .iter()
            .map(|(h, id)| {
                Ok((
                    h.clone(),
                    self.game.handle_for(viewer, *id).map_err(invariant)?,
                ))
            })
            .collect::<Result<Vec<_>, SearchRejection>>()?;
        let map = RootHandleMap::new(&self.game, &self.capture, pairs)?;
        Ok(Some(map))
    }
}

pub struct RootHandleMap {
    objects: Vec<(ObjectHandle, ObjectHandle)>,
    captured: Box<Observation>,
    sampled: Box<Observation>,
    indices: Vec<u32>,
}
impl RootHandleMap {
    fn new(
        game: &RulesTxn,
        capture: &SearchCapture,
        objects: Vec<(ObjectHandle, ObjectHandle)>,
    ) -> Result<Self, SearchRejection> {
        let viewer = capture.data().context.observation.view.viewer;
        let sampled = match game.observe(viewer, Budget::unlimited()) {
            QueryResult::Complete(QueryValue::Observation(o)) => o,
            _ => return Err(invariant("sample root observation")),
        };
        let mut map = Self {
            indices: vec![],
            objects,
            captured: Box::new(capture.data().context.observation.clone()),
            sampled: Box::new(sampled),
        };
        let mut renamed = map.sampled.clone();
        for object in &mut renamed.view.objects {
            object.handle = map
                .objects
                .iter()
                .find(|(_, h)| *h == object.handle)
                .map(|(h, _)| h.clone())
                .ok_or_else(|| invariant("root handle bijection"))?;
        }
        renamed
            .view
            .objects
            .sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
        let captured = map
            .captured
            .decision
            .as_ref()
            .ok_or_else(|| invariant("missing root decision"))?;
        let sampled = renamed
            .decision
            .as_mut()
            .ok_or_else(|| invariant("missing sample decision"))?;
        if !matches!(
            (&captured.reason, &sampled.reason),
            (Reason::Priority, Reason::Priority)
        ) || captured.player != sampled.player
            || captured.shape != sampled.shape
            || captured.candidates.len() != sampled.candidates.len()
        {
            return Err(invariant("sample root priority decision differs"));
        }
        for candidate in &captured.candidates {
            let Candidate::Action { action } = candidate else {
                return Err(invariant("non-action in priority window"));
            };
            let object = |handle| map.object(handle).map_err(invariant);
            let action = match action {
                Action::Pass => Action::Pass,
                Action::Cast { card } => Action::Cast {
                    card: object(card)?,
                },
                Action::PlayLand { object: id } => Action::PlayLand {
                    object: object(id)?,
                },
                Action::CastBackFace { card } => Action::CastBackFace {
                    card: object(card)?,
                },
                Action::CastFaceDown { card } => Action::CastFaceDown {
                    card: object(card)?,
                },
                Action::TurnFaceUp { object: id } => Action::TurnFaceUp {
                    object: object(id)?,
                },
                Action::PlayLandBackFace { object: id } => Action::PlayLandBackFace {
                    object: object(id)?,
                },
                Action::ActivateMana { object: id } => Action::ActivateMana {
                    object: object(id)?,
                },
                Action::ActivateAbility {
                    object: id,
                    ability,
                } => Action::ActivateAbility {
                    object: object(id)?,
                    ability: *ability,
                },
            };
            let index = sampled
                .index(&Candidate::Action { action })
                .ok_or_else(|| invariant("sample omits captured action"))?;
            if map.indices.contains(&index) {
                return Err(invariant("sample action map is not one-to-one"));
            }
            map.indices.push(index);
        }
        sampled.id = captured.id.clone();
        sampled.candidates.clone_from(&captured.candidates);
        if let Some(context) = &mut renamed.decision_context {
            context.candidates = map
                .indices
                .iter()
                .map(|i| {
                    let mut label = context
                        .candidates
                        .get(*i as usize)
                        .cloned()
                        .ok_or_else(|| invariant("missing sampled label"))?;
                    if let Some(handle) = label.source.as_mut().and_then(|s| s.object.as_mut()) {
                        *handle = map
                            .objects
                            .iter()
                            .find(|(_, h)| h == handle)
                            .map(|(h, _)| h.clone())
                            .ok_or_else(|| invariant("sample label handle"))?;
                    }
                    Ok(label)
                })
                .collect::<Result<_, SearchRejection>>()?;
        }
        if renamed != map.captured || map.objects.len() != map.captured.view.objects.len() {
            return Err(invariant(
                "sample root projection differs from permitted capture",
            ));
        }
        Ok(map)
    }
    pub fn object(&self, handle: &ObjectHandle) -> Result<ObjectHandle, String> {
        self.objects
            .iter()
            .find(|(h, _)| h == handle)
            .map(|(_, h)| h.clone())
            .ok_or_else(|| "not a captured current object permission".into())
    }
    pub fn token(&self, token: &DecisionToken) -> Result<DecisionToken, String> {
        match (&self.captured.decision, &self.sampled.decision) {
            (Some(a), Some(b)) if &a.id == token => Ok(b.id.clone()),
            _ => Err("not the captured root decision token".into()),
        }
    }
    pub fn captured_decision(&self) -> Result<&Decision<Viewer>, String> {
        self.captured
            .decision
            .as_ref()
            .ok_or_else(|| "missing root decision".into())
    }
    pub fn answer(&self, answer: &Answer) -> Result<Answer, String> {
        let decision = self
            .captured
            .decision
            .as_ref()
            .ok_or("missing root decision")?;
        decision.shape.check(answer)?;
        let Answer::Pick { picks } = answer else {
            return Err("expected priority pick".into());
        };
        let mapped = picks
            .iter()
            .map(|slot| {
                slot.iter()
                    .map(|i| {
                        self.indices
                            .get(*i as usize)
                            .copied()
                            .ok_or("invalid root index")
                    })
                    .collect()
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Answer::Pick { picks: mapped })
    }
    pub fn mapped_observation(&self, observation: &Observation) -> Result<Observation, String> {
        if observation != self.sampled.as_ref() {
            return Err("observation is not this world's unchanged root".into());
        }
        Ok((*self.captured).clone())
    }
}
