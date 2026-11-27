//! The game executor, its construction and its read-only accessors.
use crate::engine::*;

/// Full raw snapshot output receipt, returned only after the caller sink flushes.
/// Host metadata only; not a snapshot field, schema or semantic State input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotWriteInfo {
    pub bytes: u64,
    pub snapshot_sha256: String,
    pub state_sha256: String,
}
testkit_fields! {
    pub struct RulesTxn {
        pub(in crate::engine) opening_library_sizes: [u32; 4],
        // Admitted metadata is immutable for this executor and all staged forks.
        pub(in crate::engine) prepared: Arc<Prepared>,
        // Derived accelerator only, never state: catalog row for each physical
        // card when this executor was built. Every use rechecks the Oracle ID.
        pub(in crate::engine) definition_index: Arc<[u32]>,
        pub(in crate::engine) state: State,
        pub(in crate::engine) trace: TraceEvidence,
        pub(in crate::engine) cast_reversals: u32,
        pub(in crate::engine) stream_buffer: Vec<TraceRecord>,
        pub(in crate::engine) executor: Arc<()>,
        pub(in crate::engine) state_token: Arc<()>,
        // Derived from this committed state's handles and rules. A stage starts cold.
        pub(in crate::engine) priority_cache: std::sync::OnceLock<Decision<Rules>>,
        // Private staging scope: nested event batches share the outer rollback.
        pub(in crate::engine) in_rule_step: bool,
        // Reconstructed from the admitted program and a saved node address.
        pub(in crate::engine) kernel_leaf: Option<Arc<kir::LeafProgram>>,
        // Derived accelerator only, never state: what the last publication read.
        pub(in crate::engine) published: Option<Arc<publish::Published>>,
    }
}
testkit_fields! {
    pub struct ValidatedResponse {
        pub(in crate::engine) executor: Arc<()>,
        pub(in crate::engine) revision: u64,
        pub(in crate::engine) state_token: Arc<()>,
        pub(in crate::engine) input: PendingInput,
    }
}
testkit_fields! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Snapshot {
        pub(in crate::engine) snapshot_version: u32,
        pub(in crate::engine) build_id: String,
        pub(in crate::engine) catalog_digest: String,
        pub(in crate::engine) profile_digest: String,
        pub(in crate::engine) state_sha256: String,
        pub(in crate::engine) state: State,
        pub(in crate::engine) trace: TraceEvidence,
    }
}
#[derive(Serialize)]
pub struct SnapshotRef<'a> {
    pub snapshot_version: u32,
    pub build_id: &'a str,
    pub catalog_digest: &'a str,
    pub profile_digest: &'a str,
    pub state_sha256: &'a str,
    pub state: &'a State,
    pub trace: &'a TraceEvidence,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Advance {
    Draw,
    Finished { winner: PlayerId },
    AwaitingDecision(Decision<Rules>),
    Fault(String),
    Suspended { required_minimum: u64 },
}
impl RulesTxn {
    pub(in crate::engine) fn new(prepared: Prepared) -> Self {
        let cards: BTreeMap<_, _> = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Card {
                        owner: c.owner,
                        oracle_id: c.oracle_id.clone(),
                    },
                )
            })
            .collect();
        let objects = prepared
            .config
            .cards
            .iter()
            .map(|c| {
                (
                    c.id,
                    Object {
                        hideaway: None,
                        phased_out: None,
                        attached_to: None,
                        attachment_timestamp: None,
                        battlefield_entry_turn: None,
                        // CR 702.30a: setup permanents came under control
                        // before any upkeep.
                        control_upkeeps: (c.zone == Zone::Battlefield).then_some(0),
                        keyword_timestamp: None,
                        // CR 613.7m: setup permanents share one timestamp,
                        // ordered by APNAP from the first active player.
                        keyword_rank: c.controller.0,
                        plus_one_counters: Quantity::zero(),
                        loyalty_counters: if c.zone == Zone::Battlefield {
                            prepared
                                .catalog
                                .definition_by_oracle_id(&c.oracle_id)
                                .and_then(|d| d.characteristics.loyalty.clone())
                                .unwrap_or_default()
                        } else {
                            Quantity::zero()
                        },
                        loyalty_used_turn: None,
                        transforms: 0,
                        adventure_player: None,
                        face_down: false,
                        marked_damage: Quantity::zero(),
                        deathtouch_damage: false,
                        unearthed: false,
                        dashed: false,
                        attack_ready: c.zone == Zone::Battlefield,
                        id: ObjectId {
                            face: Default::default(),
                            card: c.id,
                            incarnation: 0,
                        },
                        controller: c.controller,
                        default_controller: c.controller,
                        zone: c.zone,
                        tapped: false,
                    },
                )
            })
            .collect();
        let opening_library_sizes = prepared.config.initial_library_sizes();
        let state = State {
            rules: RulesState::new(RulesTables {
                mana_ability_uses: BTreeSet::new(),
                activation_uses: BTreeSet::new(),
                trigger_uses: BTreeSet::new(),
                tokens: Arc::default(),
                retained_auxiliary_charge: 0,
                typed_fault: None,
                match_state: prepared
                    .match_config
                    .as_ref()
                    .map(|config| {
                        MatchState::new(
                            config,
                            opening_library_sizes[config.starting_player.0 as usize],
                        )
                    })
                    .or_else(|| prepared.config.initial.seed.map(MatchState::scenario)),
                effects: vec![].into(),
                next_effect_id: 0,
                next_damage_transaction_id: 0,
                pending_triggers: vec![],
                resolution_triggers: Shared::default(),
                resolution_trigger_clock: Default::default(),
                next_trigger_id: 0,
                libraries: prepared.config.initial_libraries().into(),
                combat: vec![],
                combat_declared: false,
                damage_choices: BTreeMap::new(),
                defender_damage_choices: BTreeMap::new(),
                first_strike_participants: BTreeSet::new(),
                blocking_creatures: BTreeSet::new(),
                cleanup_repeat: false,
                simultaneous_losses: false,
                losing_players: vec![],
                revision: 0,
                rule_steps: 0,
                players: (0..4)
                    .map(|p| Player {
                        life_frozen_until_next_turn: false,
                        poison_counters: Quantity::zero(),
                        energy_counters: Quantity::zero(),
                        upkeeps: 0,
                        failed_draw: false,
                        life: prepared.config.initial_life(PlayerId(p)),
                        mana: BTreeMap::new(),
                        restricted_mana: vec![],
                        status: PlayerStatus::Live,
                        last_known: None,
                    })
                    .collect(),
                cards: Arc::new(cards),
                objects,
                active: Some(PlayerId(0)),
                turn_owner: PlayerId(0),
                turn: TurnState {
                    number: 1,
                    step: prepared.config.initial_turn_step(),
                    lands_played: 0,
                },
                original_seating: (0..4).map(PlayerId).collect(),
                cast_history: std::array::from_fn(|_| triggers::TurnCastHistory::default()),
                commanders: CommanderLedger::new(),
                commander_returns: BTreeSet::new(),
                control_effects: vec![],
                pending_concessions: vec![],
                priority: PlayerId(0),
                passes: vec![],
                next_attachment_timestamp: 1,
                next_stack_id: 0,
                stack: vec![].into(),
                detached: BTreeMap::new(),
                frames: vec![],
                frame: Frame::Priority,
                pending: None,
                next_batch_id: 0,
                next_event_sequence: 0,
            }),
            knowledge: KnowledgeState {
                visibility: (0..4)
                    .map(|_| visibility::Knowledge::default().into())
                    .collect(),
            },
            audit: AuditState {
                external_inputs: History::default(),
                journal: History::default(),
            },
        };
        let mut game = Self {
            opening_library_sizes,
            definition_index: Self::definition_index(&prepared.catalog, &state.rules.cards),
            prepared: Arc::new(prepared),
            state,
            trace: Self::initial_trace(),
            cast_reversals: 0,
            stream_buffer: Vec::new(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
            priority_cache: Default::default(),
            in_rule_step: false,
            kernel_leaf: None,
            published: None,
        };
        if let Some(starting) = game
            .prepared
            .match_config
            .as_ref()
            .map(|config| config.starting_player)
        {
            let rules = game.rules_mut();
            rules.active = Some(starting);
            rules.turn_owner = starting;
            rules.priority = starting;
            rules.frame = Frame::Setup;
        }
        game.initialize_commanders()
            .expect("admitted commander configuration");
        game.refresh_visibility()
            .expect("admitted initial visibility");
        game
    }
    pub(in crate::engine) fn fork(&self) -> Self {
        Self {
            opening_library_sizes: self.opening_library_sizes,
            prepared: self.prepared.clone(),
            definition_index: self.definition_index.clone(),
            state: self.state.clone(),
            trace: self.trace.clone(),
            cast_reversals: self.cast_reversals,
            stream_buffer: Vec::new(),
            executor: Arc::new(()),
            state_token: Arc::new(()),
            priority_cache: Default::default(),
            in_rule_step: false,
            kernel_leaf: self.kernel_leaf.clone(),
            published: self.published.clone(),
        }
    }
    /// Trusted host execution boundary; never a player-facing identifier.
    pub fn rule_boundary(&self) -> u64 {
        self.state.rules.rule_steps
    }
    pub fn state_digest(&self) -> Result<String, String> {
        state_digest::state_digest(&self.state)
    }
    pub fn events(&self) -> &[Event] {
        self.trace.events.as_slice()
    }
    pub fn config(&self) -> &ScenarioConfig {
        self.prepared.config()
    }
    /// Borrow the persistent input log without materializing a flat copy.
    /// Use `iter()` for scans and explicitly collect when owned entries are needed.
    pub fn journal(&self) -> &History<AcceptedInput> {
        &self.state.audit.journal
    }
    pub fn object_id(&self, card: CardId) -> Option<ObjectId> {
        self.state.rules.objects.get(&card).map(|o| o.id)
    }
    pub fn zone(&self, object: ObjectId) -> Option<Zone> {
        self.current(object).ok().map(|o| o.zone)
    }
    pub fn mana(&self, player: PlayerId, color: Color) -> Option<Quantity> {
        self.state
            .rules
            .players
            .get(player.0 as usize)
            .map(|p| p.mana.get(&color).cloned().unwrap_or_default())
    }
    pub fn characteristics(&self, object: ObjectId) -> Result<Characteristics, String> {
        self.characteristics_with_sources(object, None)
    }
    pub(in crate::engine) fn characteristics_with_sources(
        &self,
        object: ObjectId,
        sources: Option<&[CardId]>,
    ) -> Result<Characteristics, String> {
        if self.current(object)?.hideaway.is_some() {
            return Err("face-down exiled card has no characteristics".into());
        }
        self.derived_characteristics_with_sources(object, sources)
    }
    // Characteristics supply intrinsic behavior; reminder text never supplies
    // runtime code. CR 305.6 / 113.6 / 605.3b, pinned official rules:
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    /// Colors the object's mana ability can produce. In `commander-mana-v1`
    /// this describes production only, not current activatability: a tapped
    /// or summoning-sick source still reports its symbols. Use the mana
    /// window's offered sources to decide what can be activated now.
    pub fn mana_abilities(&self, object: ObjectId) -> Result<Vec<Color>, String> {
        let o = self.current(object)?;
        if !o.present_on_battlefield() {
            return Ok(vec![]);
        }
        let mut colors = self.intrinsic_mana_colors(object)?;
        let mixed = !colors.is_empty();
        if o.zone == Zone::Battlefield {
            for (_, ability) in self.current_mana_abilities(object)? {
                for color in self.mana_colors(o.controller, &ability.production)? {
                    if !mixed || !colors.contains(&color) {
                        colors.push(color);
                    }
                }
            }
        }
        Ok(colors)
    }
}
#[cfg(any(test, feature = "testkit"))]
impl Game {
    /// Historical scenario drivers now answer the common explicit spending request.
    pub(crate) fn complete_test_mana(&mut self) {
        if let Advance::AwaitingDecision(Decision {
            player,
            id,
            shape:
                Shape::Pay {
                    cost,
                    pool,
                    step: PayStep::Spend,
                },
            ..
        }) = self.advance(Budget::unlimited())
        {
            let remaining = self.payment_pool(player, &cost).unwrap();
            let unrestricted = pool
                .unrestricted
                .into_iter()
                .filter_map(|(color, mut amount)| {
                    amount.subtract(&remaining.get(&color).cloned().unwrap_or_default());
                    (amount > Quantity::zero()).then_some((color, amount))
                })
                .collect();
            self.respond(
                player,
                id,
                Answer::Pay {
                    pay: PayAnswer::Spend { unrestricted },
                },
            )
            .unwrap();
            self.advance(Budget::unlimited());
        }
    }
}
