//! Complete finite Commander setup. CR103.5/103.5c,103.8c,903.6-.7,
//! pinned20260925: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! ChaCha20 stream0 is explicit host-seeded; never OS randomness. Word positions
//! are checked before rand_chacha's modulo2^68 cursor could wrap.
use super::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};
// Foundations construction bounds: <=28 creatures and4 Artists yield <=112 ordinary
// death triggers plus4 Bouncer self triggers per nonempty-stack episode. One
// entering permanent starts <=5 ETB triggers; <=13 ordinary entries remain
// below256 in foundations. Tokens use the separately proved tokens.rs bounds.
// <=56 positive combat edges fit64. Four BG decks create <=16 until-EOT
// effects; mixed/white shields remain below32. Group400 handles full-pod
// departures/untaps without changing historical group16 limits.
const RNG_END: u128 = 1u128 << 68;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RngPosition(pub u128);
impl Serialize for RngPosition {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for RngPosition {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let n = s.parse::<u128>().map_err(serde::de::Error::custom)?;
        if n > RNG_END || s != n.to_string() {
            return Err(serde::de::Error::custom(
                "invalid canonical ChaCha word position",
            ));
        }
        Ok(Self(n))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRandom {
    pub version: u32,
    pub seed: [u8; 32],
    pub stream: u64,
    pub word_position: RngPosition,
}
pub(super) fn sample_shuffle_word(word: u32, upper: u32) -> Option<u32> {
    if !(2..=99).contains(&upper) {
        return None;
    }
    let limit = (1u64 << 32) - ((1u64 << 32) % u64::from(upper));
    (u64::from(word) < limit).then_some(word % upper)
}
impl MatchRandom {
    pub(super) fn word(&mut self) -> Result<u32, String> {
        if self.version != 1 || self.stream != 0 || self.word_position.0 >= RNG_END {
            return Err("shuffle random stream exhausted or unsupported".into());
        }
        let mut rng = ChaCha20Rng::from_seed(self.seed);
        rng.set_stream(self.stream);
        rng.set_word_pos(self.word_position.0);
        let word = rng.next_u32();
        self.word_position.0 += 1;
        Ok(word)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "phase", deny_unknown_fields)]
pub enum SetupPhase {
    Shuffle {
        players: Vec<PlayerId>,
        index: u32,
        redraw: bool,
    },
    Draw {
        players: Vec<PlayerId>,
        remaining: u8,
        redraw: bool,
    },
    Declare {
        players: Vec<PlayerId>,
    },
    Return {
        players: Vec<PlayerId>,
    },
    Bottom {
        players: Vec<PlayerId>,
    },
    Finish,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningPlayer {
    pub kept: bool,
    pub mulligans_taken: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningSetup {
    pub round: u8,
    pub players: [OpeningPlayer; 4],
    pub takers: Vec<PlayerId>,
    pub phase: SetupPhase,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchState {
    #[serde(default, skip_serializing_if = "no_library_shuffles")]
    pub library_shuffles: [u64; 4],
    pub random: MatchRandom,
    pub setup: Option<OpeningSetup>,
}
pub(super) fn no_library_shuffles(counts: &[u64; 4]) -> bool {
    counts.iter().all(|n| *n == 0)
}
impl MatchState {
    pub(super) fn new(c: &MatchConfig) -> Self {
        Self {
            library_shuffles: [0; 4],
            random: MatchRandom {
                version: 1,
                seed: c.seed,
                stream: 0,
                word_position: RngPosition(0),
            },
            setup: Some(OpeningSetup {
                round: 0,
                players: std::array::from_fn(|_| OpeningPlayer {
                    kept: false,
                    mulligans_taken: 0,
                }),
                takers: vec![],
                phase: SetupPhase::Shuffle {
                    players: (0..4)
                        .map(|i| PlayerId((c.starting_player.0 + i) % 4))
                        .collect(),
                    index: 98,
                    redraw: false,
                },
            }),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisibleSetupPhase {
    Declare,
    Bottom,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSetupPlayer {
    pub player: PlayerId,
    pub kept: bool,
    /// Accepted declaration rule-steps, including the current round before redraw.
    pub mulligans_taken: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleSetup {
    pub round: u8,
    pub phase: VisibleSetupPhase,
    pub players: Vec<VisibleSetupPlayer>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupAction {
    ShuffleWord {
        player: PlayerId,
        index: u32,
    },
    Draw {
        player: PlayerId,
    },
    Return {
        player: PlayerId,
    },
    Declare {
        player: PlayerId,
        take: bool,
    },
    Bottom {
        player: PlayerId,
        objects: Vec<ObjectId>,
    },
    Finish,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "actual", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupOutcome {
    ShuffledWord {
        player: PlayerId,
        index: u32,
        word: u32,
        selected: Option<u32>,
        before: RngPosition,
        after: RngPosition,
    },
    Moved {
        player: PlayerId,
        moves: Vec<MoveOutcome>,
        bottom: Vec<CardId>,
    },
    Declared {
        player: PlayerId,
        take: bool,
        mulligans_taken: u8,
    },
    Complete {
        starting_player: PlayerId,
    },
}
impl ScenarioConfig {
    pub(super) fn match_profile(&self) -> bool {
        self.mode == Mode::CommanderMatch && match_profile(&self.profile).is_some()
    }
    pub(super) fn owned_limit(&self) -> usize {
        if self.match_profile() { 100 } else { 16 }
    }
}
impl Game {
    pub(super) fn damage_limit(&self) -> usize {
        if self.removal_enabled() {
            removal::DAMAGE_BOUND
        } else if self.four_colors_enabled() {
            four_colors::DAMAGE_BOUND
        } else if self.mana_enabled() {
            mana_permanents::DAMAGE_BOUND
        } else if self.staples_enabled() {
            staples::DAMAGE_BOUND
        } else if self.tokens_enabled() {
            tokens::DAMAGE_BOUND
        } else if self.prepared.config.match_profile() {
            64
        } else {
            24
        }
    }
    pub(super) fn effect_limit(&self) -> usize {
        if self.prepared.config.match_profile() {
            32
        } else {
            16
        }
    }
    fn opening(&self) -> Result<&OpeningSetup, String> {
        self.state
            .match_state
            .as_ref()
            .and_then(|s| s.setup.as_ref())
            .ok_or_else(|| "missing opening setup".into())
    }
    pub(super) fn setup_automatic(&self) -> bool {
        matches!(self.state.frame, Frame::Setup) && !matches!(self.setup_request(), Ok(Some(_)))
    }
    pub(super) fn visible_setup(&self) -> Option<VisibleSetup> {
        let s = self.state.match_state.as_ref()?.setup.as_ref()?;
        let phase = match s.phase {
            SetupPhase::Declare { .. } => VisibleSetupPhase::Declare,
            SetupPhase::Bottom { .. } => VisibleSetupPhase::Bottom,
            _ => return None,
        };
        Some(VisibleSetup {
            round: s.round,
            phase,
            players: s
                .players
                .iter()
                .enumerate()
                .map(|(p, s)| VisibleSetupPlayer {
                    player: PlayerId(p as u8),
                    kept: s.kept,
                    mulligans_taken: s.mulligans_taken,
                })
                .collect(),
        })
    }
    pub(super) fn setup_request(&self) -> Result<Option<Request>, String> {
        if !matches!(self.state.frame, Frame::Setup) {
            return Ok(None);
        }
        let s = self.opening()?;
        let req = match &s.phase {
            SetupPhase::Declare { players } => players
                .iter()
                .copied()
                .find(|p| self.is_live(*p) && !s.players[p.0 as usize].kept)
                .map(|player| {
                    let n = s.players[player.0 as usize].mulligans_taken;
                    Request::Mulligan {
                        decision_id: self.state.revision,
                        player,
                        mulligans_taken: n,
                        bottom_if_mulligan: n,
                        can_mulligan: n < 8,
                    }
                }),
            SetupPhase::Bottom { players } => players
                .iter()
                .copied()
                .find(|p| self.is_live(*p))
                .map(|player| Request::BottomOpening {
                    decision_id: self.state.revision,
                    player,
                    count: usize::from(
                        s.players[player.0 as usize]
                            .mulligans_taken
                            .saturating_sub(1),
                    ),
                    candidates: self
                        .state
                        .objects
                        .values()
                        .filter(|o| o.zone == (Zone::Hand { owner: player }))
                        .map(|o| o.id)
                        .collect(),
                }),
            _ => None,
        };
        Ok(req)
    }
    pub(super) fn validate_setup_response(
        &self,
        player: PlayerId,
        response: &Response,
    ) -> Result<(), String> {
        match (self.setup_request()?, response) {
            (
                Some(Request::Mulligan {
                    player: p,
                    can_mulligan,
                    ..
                }),
                Response::Mulligan { take },
            ) if p == player && (!take || can_mulligan) => Ok(()),
            (
                Some(Request::BottomOpening {
                    player: p,
                    count,
                    candidates,
                    ..
                }),
                Response::BottomOpening { objects },
            ) if p == player
                && objects.len() == count
                && objects.iter().collect::<BTreeSet<_>>().len() == count
                && objects.iter().all(|o| candidates.contains(o)) =>
            {
                Ok(())
            }
            _ => Err("invalid opening-hand decision".into()),
        }
    }
    fn setup_order(&self) -> Vec<PlayerId> {
        let anchor = self
            .prepared
            .match_config
            .as_ref()
            .expect("match config")
            .starting_player;
        (0..4)
            .map(|i| PlayerId((anchor.0 + i) % 4))
            .filter(|p| self.is_live(*p))
            .collect()
    }
    // Normalize exhausted/departed queues without inspecting hidden card identities.
    fn normalize_setup(&mut self) -> Result<(), String> {
        loop {
            let mut s = self.opening()?.clone();
            let old = s.clone();
            let live = self.live_players();
            match &mut s.phase {
                SetupPhase::Shuffle {
                    players,
                    index,
                    redraw,
                } => {
                    if players.first().is_some_and(|p| !live.contains(p)) {
                        players.remove(0);
                        *index = 98;
                    }
                    players.retain(|p| live.contains(p));
                    if players.is_empty() {
                        s.phase = SetupPhase::Draw {
                            players: if *redraw {
                                s.takers.clone()
                            } else {
                                self.setup_order()
                            },
                            remaining: 7,
                            redraw: *redraw,
                        };
                    }
                }
                SetupPhase::Draw {
                    players,
                    remaining,
                    redraw,
                } => {
                    if players.first().is_some_and(|p| !live.contains(p)) {
                        *remaining = 7;
                    }
                    players.retain(|p| live.contains(p));
                    if players.is_empty() {
                        if *redraw {
                            s.phase = SetupPhase::Bottom {
                                players: s
                                    .takers
                                    .iter()
                                    .copied()
                                    .filter(|p| {
                                        live.contains(p)
                                            && s.players[p.0 as usize].mulligans_taken > 1
                                    })
                                    .collect(),
                            };
                        } else {
                            s.phase = SetupPhase::Declare {
                                players: self.setup_order(),
                            };
                        }
                    }
                }
                SetupPhase::Declare { players } => {
                    players.retain(|p| live.contains(p) && !s.players[p.0 as usize].kept);
                    if players.is_empty() {
                        s.takers.retain(|p| live.contains(p));
                        s.phase = if s.takers.is_empty() {
                            SetupPhase::Finish
                        } else {
                            SetupPhase::Return {
                                players: s.takers.clone(),
                            }
                        };
                    }
                }
                SetupPhase::Return { players } => {
                    players.retain(|p| live.contains(p));
                    if players.is_empty() {
                        s.phase = SetupPhase::Shuffle {
                            players: s.takers.clone(),
                            index: 98,
                            redraw: true,
                        };
                    }
                }
                SetupPhase::Bottom { players } => {
                    players.retain(|p| live.contains(p));
                    if players.is_empty() {
                        s.round = s.round.checked_add(1).ok_or("opening round limit")?;
                        s.takers.clear();
                        s.phase = SetupPhase::Declare {
                            players: self
                                .setup_order()
                                .into_iter()
                                .filter(|p| !s.players[p.0 as usize].kept)
                                .collect(),
                        };
                    }
                }
                SetupPhase::Finish => {}
            }
            self.state
                .match_state
                .as_mut()
                .ok_or("missing match state")?
                .setup = Some(s.clone());
            if s == old {
                return Ok(());
            }
        }
    }
    pub(super) fn setup_step(&mut self) -> Result<(), String> {
        self.normalize_setup()?;
        let action = match self.opening()?.phase.clone() {
            SetupPhase::Shuffle { players, index, .. } => SetupAction::ShuffleWord {
                player: players[0],
                index,
            },
            SetupPhase::Draw { players, .. } => SetupAction::Draw { player: players[0] },
            SetupPhase::Return { players } => SetupAction::Return { player: players[0] },
            SetupPhase::Declare { .. } | SetupPhase::Bottom { .. } => {
                let input = self.state.pending.take().ok_or("missing opening input")?;
                self.validate_setup_response(input.player, &input.response)?;
                match input.response {
                    Response::Mulligan { take } => SetupAction::Declare {
                        player: input.player,
                        take,
                    },
                    Response::BottomOpening { objects } => SetupAction::Bottom {
                        player: input.player,
                        objects,
                    },
                    _ => return Err("wrong opening input".into()),
                }
            }
            SetupPhase::Finish => SetupAction::Finish,
        };
        self.commit_batch(
            BatchCause::Setup,
            EventGroup::Occurrence {
                proposed: ProposedEvent::Setup { action },
            },
        )?;
        if matches!(self.state.frame, Frame::Setup) {
            self.normalize_setup()?;
        }
        Ok(())
    }
    pub(super) fn apply_setup_action(
        &mut self,
        action: &SetupAction,
    ) -> Result<SetupOutcome, String> {
        if !matches!(self.state.frame, Frame::Setup) {
            return Err("setup event outside setup".into());
        }
        let mut s = self.opening()?.clone();
        let result = match (action, &mut s.phase) {
            (
                SetupAction::ShuffleWord { player, index },
                SetupPhase::Shuffle {
                    players, index: i, ..
                },
            ) if players.first() == Some(player) && i == index && *index > 0 => {
                let library = self
                    .state
                    .libraries
                    .get_mut(player)
                    .ok_or("missing shuffle library")?;
                if library.len() != 99 || *index as usize >= library.len() {
                    return Err("invalid shuffle range".into());
                }
                let random = &mut self
                    .state
                    .match_state
                    .as_mut()
                    .ok_or("missing match random")?
                    .random;
                let before = random.word_position;
                let word = random.word()?;
                let upper = *index + 1;
                let selected = sample_shuffle_word(word, upper);
                if let Some(selected) = selected {
                    library.swap(*index as usize, selected as usize);
                    if *i == 1 {
                        players.remove(0);
                        *i = 98;
                    } else {
                        *i -= 1;
                    }
                }
                SetupOutcome::ShuffledWord {
                    player: *player,
                    index: *index,
                    word,
                    selected,
                    before,
                    after: random.word_position,
                }
            }
            (
                SetupAction::Draw { player },
                SetupPhase::Draw {
                    players, remaining, ..
                },
            ) if players.first() == Some(player) && *remaining > 0 => {
                let card = *self
                    .state
                    .libraries
                    .get(player)
                    .and_then(|l| l.first())
                    .ok_or("opening draw missing card")?;
                let object = self
                    .state
                    .objects
                    .get(&card)
                    .ok_or("opening draw missing object")?
                    .id;
                let moved = self.apply_occurrence(&ProposedEvent::Move {
                    object,
                    destination: Zone::Hand { owner: *player },
                })?;
                let EventOutcome::Moved { movement } = moved else {
                    return Err("opening move result".into());
                };
                *remaining -= 1;
                if *remaining == 0 {
                    players.remove(0);
                    *remaining = 7;
                }
                SetupOutcome::Moved {
                    player: *player,
                    moves: vec![movement],
                    bottom: vec![],
                }
            }
            (SetupAction::Return { player }, SetupPhase::Return { players })
                if players.first() == Some(player) =>
            {
                let hand = self
                    .state
                    .objects
                    .values()
                    .filter(|o| o.zone == (Zone::Hand { owner: *player }))
                    .map(|o| o.id)
                    .collect::<Vec<_>>();
                if hand.len() > 7 {
                    return Err("opening hand range".into());
                }
                self.remember_opening_redraw(*player);
                let mut moves = vec![];
                for object in hand {
                    let EventOutcome::Moved { movement } =
                        self.apply_occurrence(&ProposedEvent::Move {
                            object,
                            destination: Zone::Library { owner: *player },
                        })?
                    else {
                        return Err("opening return result".into());
                    };
                    moves.push(movement);
                }
                players.remove(0);
                SetupOutcome::Moved {
                    player: *player,
                    moves,
                    bottom: vec![],
                }
            }
            (SetupAction::Declare { player, take }, SetupPhase::Declare { players })
                if players.first() == Some(player) =>
            {
                self.validate_setup_response(*player, &Response::Mulligan { take: *take })?;
                let p = &mut s.players[player.0 as usize];
                if *take {
                    p.mulligans_taken = p.mulligans_taken.checked_add(1).ok_or("mulligan count")?;
                    s.takers.push(*player);
                } else {
                    p.kept = true;
                }
                players.remove(0);
                SetupOutcome::Declared {
                    player: *player,
                    take: *take,
                    mulligans_taken: p.mulligans_taken,
                }
            }
            (SetupAction::Bottom { player, objects }, SetupPhase::Bottom { players })
                if players.first() == Some(player) =>
            {
                self.validate_setup_response(
                    *player,
                    &Response::BottomOpening {
                        objects: objects.clone(),
                    },
                )?;
                self.remember_opening_bottom(*player, objects)?;
                let mut moves = vec![];
                for object in objects {
                    let EventOutcome::Moved { movement } =
                        self.apply_occurrence(&ProposedEvent::Move {
                            object: *object,
                            destination: Zone::Library { owner: *player },
                        })?
                    else {
                        return Err("opening bottom result".into());
                    };
                    moves.push(movement);
                }
                players.remove(0);
                if s.players[player.0 as usize].mulligans_taken == 8 {
                    s.players[player.0 as usize].kept = true;
                }
                SetupOutcome::Moved {
                    player: *player,
                    moves,
                    bottom: objects.iter().map(|o| o.card).collect(),
                }
            }
            (SetupAction::Finish, SetupPhase::Finish) => {
                let player = *self
                    .setup_order()
                    .first()
                    .ok_or("no surviving opening player")?;
                self.state.active = Some(player);
                self.state.turn_owner = player;
                self.state.priority = player;
                self.state.turn.step = TurnStep::Untap;
                self.state.frame = Frame::EnterStep;
                self.state
                    .match_state
                    .as_mut()
                    .ok_or("missing match state")?
                    .setup = None;
                return Ok(SetupOutcome::Complete {
                    starting_player: player,
                });
            }
            _ => return Err("setup event does not match durable phase".into()),
        };
        self.state
            .match_state
            .as_mut()
            .ok_or("missing match state")?
            .setup = Some(s);
        Ok(result)
    }
    pub(super) fn revalidate_setup_departure(&mut self) -> Result<(), String> {
        self.normalize_setup()
    }
    pub(super) fn validate_match_state(&self) -> Result<(), String> {
        match (&self.prepared.match_config, &self.state.match_state) {
            (None, None) => Ok(()),
            (Some(config), Some(s)) => {
                if !no_library_shuffles(&s.library_shuffles) && !self.staples_enabled() {
                    return Err("snapshot public shuffle counts invariant".into());
                }
                if s.random.version != 1
                    || s.random.seed != config.seed
                    || s.random.stream != 0
                    || s.random.word_position.0 > RNG_END
                {
                    return Err("snapshot random identity".into());
                }
                if let Some(setup) = &s.setup {
                    if !matches!(
                        self.state.frame,
                        Frame::Setup | Frame::Fault { .. } | Frame::Finished { .. }
                    ) || setup.round > 8
                        || setup.players.iter().any(|p| p.mulligans_taken > 8)
                    {
                        return Err("snapshot opening phase".into());
                    }
                    let queues: Vec<&[PlayerId]> = match &setup.phase {
                        SetupPhase::Shuffle { players, index, .. } => {
                            if *index == 0 || *index > 98 {
                                return Err("snapshot shuffle index".into());
                            }
                            vec![players]
                        }
                        SetupPhase::Draw {
                            players, remaining, ..
                        } => {
                            if *remaining == 0 || *remaining > 7 {
                                return Err("snapshot opening draw range".into());
                            }
                            vec![players]
                        }
                        SetupPhase::Declare { players }
                        | SetupPhase::Return { players }
                        | SetupPhase::Bottom { players } => vec![players],
                        SetupPhase::Finish => vec![],
                    };
                    for queue in queues
                        .into_iter()
                        .chain(std::iter::once(setup.takers.as_slice()))
                    {
                        if queue.len() > 4
                            || queue.iter().any(|p| p.0 >= 4)
                            || queue.iter().collect::<BTreeSet<_>>().len() != queue.len()
                        {
                            return Err("snapshot opening seats".into());
                        }
                    }
                } else if matches!(self.state.frame, Frame::Setup) {
                    return Err("snapshot missing opening state".into());
                }
                Ok(())
            }
            _ => Err("snapshot match mode mismatch".into()),
        }
    }
}
