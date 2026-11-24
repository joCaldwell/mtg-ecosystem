//! Complete finite Commander setup. CR103.5/103.5c,103.8c,903.6-.7,
//! pinned20260925: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Each shuffle or seat-local random effect consumes its own ChaCha20 stream.
use super::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};
pub(super) const LIBRARY_SHUFFLE: u8 = 1;
pub(super) const SEAT_RANDOM: u8 = 2;
const TABLE: u8 = 3;
const ORDINAL_LIMIT: u64 = 1 << 48;
pub(super) fn stream_id(domain: u8, seat: PlayerId, ordinal: u64) -> Result<u64, String> {
    if ordinal >= ORDINAL_LIMIT
        || seat.0 >= 4
        || !matches!(
            (domain, seat.0),
            (LIBRARY_SHUFFLE | SEAT_RANDOM, _) | (TABLE, 0)
        )
    {
        return Err("invalid random stream domain, seat or ordinal".into());
    }
    Ok(u64::from(domain) << 56 | u64::from(seat.0) << 48 | ordinal)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenStream {
    pub stream: u64,
    pub word: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchRandom {
    pub version: u32,
    pub seed: [u8; 32],
    pub seat_random: [u64; 4],
    pub table: u64,
    pub open: Option<OpenStream>,
}
// Generic physical-card bound includes a tucked commander. Setup still checks
// its own starting library; callers never infer that as a sampler limit.
pub fn sample_shuffle_word(word: u32, upper: u32) -> Option<u32> {
    if !(2..=capacities::OWNED_LIMIT as u32).contains(&upper) {
        return None;
    }
    sample_bounded_word(word, upper)
}
/// A single trial for a nontrivial range. Reject the incomplete final interval
/// so every accepted index has the same number of preimages.
pub(super) fn sample_bounded_word(word: u32, upper: u32) -> Option<u32> {
    if upper < 2 {
        return None;
    }
    let limit = (1u64 << 32) - ((1u64 << 32) % u64::from(upper));
    (u64::from(word) < limit).then_some(word % upper)
}
impl MatchRandom {
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            version: 2,
            seed,
            seat_random: [0; 4],
            table: 0,
            open: None,
        }
    }

    pub(crate) fn word(&mut self, stream: u64) -> Result<u32, String> {
        let ordinal = stream & (ORDINAL_LIMIT - 1);
        stream_id(
            (stream >> 56) as u8,
            PlayerId((stream >> 48) as u8),
            ordinal,
        )?;
        if self.version != 2 {
            return Err("unsupported random version".into());
        }
        let word = self
            .open
            .filter(|p| p.stream == stream)
            .map_or(0, |p| p.word);
        let next = word.checked_add(1).ok_or("random event exhausted")?;
        let mut rng = ChaCha20Rng::from_seed(self.seed);
        rng.set_stream(stream);
        rng.set_word_pos(u128::from(word));
        self.open = Some(OpenStream { stream, word: next });
        Ok(rng.next_u32())
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
fn first_shuffle_index(sizes: &[u32; 4], players: &[PlayerId]) -> u32 {
    // Retain the existing empty-queue sentinel until normalization
    // switches to the next phase. The queue selects the actual bound.
    players
        .first()
        .and_then(|p| sizes.get(p.0 as usize))
        .map_or(98, |size| size.saturating_sub(1))
}
impl MatchState {
    fn valid_open_stream(&self, open: OpenStream) -> bool {
        let domain = (open.stream >> 56) as u8;
        let seat = PlayerId((open.stream >> 48) as u8);
        let index = usize::from(seat.0);
        // Shuffles increment on completion; seat effects reserve on entry.
        // The table domain has no producer that can leave an open cursor.
        let ordinal = match domain {
            LIBRARY_SHUFFLE => self.library_shuffles.get(index).copied(),
            SEAT_RANDOM => self
                .random
                .seat_random
                .get(index)
                .and_then(|n| n.checked_sub(1)),
            _ => None,
        };
        open.word > 0 && ordinal.and_then(|n| stream_id(domain, seat, n).ok()) == Some(open.stream)
    }

    pub(super) fn new(c: &MatchConfig, initial_library_size: u32) -> Self {
        Self {
            library_shuffles: [0; 4],
            random: MatchRandom::new(c.seed),
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
                    index: initial_library_size.saturating_sub(1),
                    redraw: false,
                },
            }),
        }
    }
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
        position: OpenStream,
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
    pub(super) fn owned_limit(&self) -> usize {
        capacities::OWNED_LIMIT
    }
}
impl RulesTxn {
    pub(crate) fn damage_limit(&self) -> usize {
        capacities::DAMAGE_LIMIT
    }
    pub(crate) fn effect_limit(&self) -> usize {
        capacities::EFFECT_LIMIT
    }
    fn opening(&self) -> Result<&OpeningSetup, String> {
        self.state
            .rules
            .match_state
            .as_ref()
            .and_then(|s| s.setup.as_ref())
            .ok_or_else(|| "missing opening setup".into())
    }
    pub(super) fn setup_automatic(&self) -> bool {
        if !matches!(self.state.rules.frame, Frame::Setup) {
            return false;
        }
        let Ok(setup) = self.opening() else {
            return true;
        };
        match &setup.phase {
            SetupPhase::Declare { players } => !players
                .iter()
                .any(|p| self.is_live(*p) && !setup.players[p.0 as usize].kept),
            SetupPhase::Bottom { players } => !players.iter().any(|p| self.is_live(*p)),
            _ => true,
        }
    }
    pub(super) fn visible_setup(&self) -> Option<VisibleSetup> {
        let s = self.state.rules.match_state.as_ref()?.setup.as_ref()?;
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

    pub(super) fn setup_request(&self) -> Result<Option<Decision<Rules>>, String> {
        if !matches!(self.state.rules.frame, Frame::Setup) {
            return Ok(None);
        }
        let s = self.opening()?;
        Ok(match &s.phase {
            SetupPhase::Declare { players } => players
                .iter()
                .copied()
                .find(|p| self.is_live(*p) && !s.players[p.0 as usize].kept)
                .map(|player| {
                    let n = s.players[player.0 as usize].mulligans_taken;
                    let flags = if n < 8 {
                        &[Flag::Keep, Flag::Mulligan][..]
                    } else {
                        &[Flag::Keep][..]
                    };
                    self.pick_flags(
                        player,
                        Reason::Mulligan {
                            mulligans_taken: n,
                            bottom_if_mulligan: n,
                        },
                        flags,
                    )
                }),
            SetupPhase::Bottom { players } => players
                .iter()
                .copied()
                .find(|p| self.is_live(*p))
                .map(|player| {
                    let count = u32::from(
                        s.players[player.0 as usize]
                            .mulligans_taken
                            .saturating_sub(1),
                    );
                    let cards = self
                        .action_hand(player)
                        .into_iter()
                        .map(|object| Candidate::Object { object })
                        .collect();
                    self.order(player, Reason::BottomOpening, cards, count)
                }),
            _ => None,
        })
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
                    let front_departed = players.first().is_some_and(|p| !live.contains(p));
                    players.retain(|p| live.contains(p));
                    if front_departed {
                        *index = first_shuffle_index(&self.opening_library_sizes, players);
                    }
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
                            index: first_shuffle_index(&self.opening_library_sizes, &s.takers),
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
            self.rules_mut()
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
                let input = self
                    .rules_mut()
                    .pending
                    .take()
                    .ok_or("missing opening input")?;

                match self.opening()?.phase {
                    SetupPhase::Declare { .. } => SetupAction::Declare {
                        player: input.player,
                        take: input.flag(Flag::Mulligan)?,
                    },
                    SetupPhase::Bottom { .. } => SetupAction::Bottom {
                        player: input.player,
                        objects: input.objects()?,
                    },
                    _ => return Err("unexpected opening phase".into()),
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
        if matches!(self.state.rules.frame, Frame::Setup) {
            self.normalize_setup()?;
        }
        Ok(())
    }
    pub(super) fn apply_setup_action(
        &mut self,
        action: &SetupAction,
    ) -> Result<SetupOutcome, String> {
        if !matches!(self.state.rules.frame, Frame::Setup) {
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
                let sizes = self.opening_library_sizes;
                let rules = self.rules_mut();
                let library = rules
                    .libraries
                    .get_mut(player)
                    .ok_or("missing shuffle library")?;
                if library.len() != sizes[player.0 as usize] as usize
                    || *index as usize >= library.len()
                {
                    return Err("invalid shuffle range".into());
                }
                let state = rules.match_state.as_mut().ok_or("missing match random")?;
                let ordinal = &mut state.library_shuffles[player.0 as usize];
                let stream = stream_id(LIBRARY_SHUFFLE, *player, *ordinal)?;
                let word = state.random.word(stream)?;
                let position = OpenStream {
                    stream,
                    word: state.random.open.unwrap().word - 1,
                };
                let upper = *index + 1;
                let selected = sample_shuffle_word(word, upper);
                if let Some(selected) = selected {
                    library.swap(*index as usize, selected as usize);
                    if *i == 1 {
                        *ordinal += 1;
                        state.random.open = None;
                        players.remove(0);
                        *i = first_shuffle_index(&sizes, players);
                    } else {
                        *i -= 1;
                    }
                }
                SetupOutcome::ShuffledWord {
                    player: *player,
                    index: *index,
                    word,
                    selected,
                    position,
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
                    .rules
                    .libraries
                    .get(player)
                    .and_then(|l| l.first())
                    .ok_or("opening draw missing card")?;
                let object = self
                    .state
                    .rules
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
                    .rules
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
                if !self.is_live(*player)
                    || s.players[player.0 as usize].kept
                    || (*take && s.players[player.0 as usize].mulligans_taken >= 8)
                {
                    return Err("invalid mulligan declaration".into());
                }
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
                let count = usize::from(
                    s.players[player.0 as usize]
                        .mulligans_taken
                        .saturating_sub(1),
                );
                if !self.is_live(*player)
                    || count != objects.len()
                    || objects.iter().collect::<BTreeSet<_>>().len() != count
                    || objects.iter().any(|o| {
                        !self
                            .current(*o)
                            .is_ok_and(|o| o.zone == (Zone::Hand { owner: *player }))
                    })
                {
                    return Err("invalid opening bottom".into());
                }
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
                self.rules_mut().active = Some(player);
                self.rules_mut().turn_owner = player;
                self.rules_mut().priority = player;
                self.rules_mut().turn.step = TurnStep::Untap;
                self.rules_mut().frame = Frame::EnterStep;
                self.rules_mut()
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
        self.rules_mut()
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
        match (&self.prepared.match_config, &self.state.rules.match_state) {
            (None, None) => Ok(()),
            (Some(config), Some(s)) => {
                if s.random.version != 2
                    || s.random.seed != config.seed
                    || s.random.table > ORDINAL_LIMIT
                    || s.library_shuffles
                        .iter()
                        .chain(&s.random.seat_random)
                        .any(|n| *n > ORDINAL_LIMIT)
                    || s.random.open.is_some_and(|p| !s.valid_open_stream(p))
                {
                    return Err("snapshot random identity".into());
                }
                if let Some(setup) = &s.setup {
                    if !matches!(
                        self.state.rules.frame,
                        Frame::Setup | Frame::Fault { .. } | Frame::Finished { .. }
                    ) || setup.round > 8
                        || setup.players.iter().any(|p| p.mulligans_taken > 8)
                    {
                        return Err("snapshot opening phase".into());
                    }
                    let queues: Vec<&[PlayerId]> = match &setup.phase {
                        SetupPhase::Shuffle { players, index, .. } => {
                            if *index == 0
                                || *index
                                    > first_shuffle_index(&self.opening_library_sizes, players)
                            {
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
                } else if matches!(self.state.rules.frame, Frame::Setup) {
                    return Err("snapshot missing opening state".into());
                }
                Ok(())
            }
            _ => Err("snapshot match mode mismatch".into()),
        }
    }
}
