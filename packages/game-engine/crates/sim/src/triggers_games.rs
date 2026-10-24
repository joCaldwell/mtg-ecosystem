//! Ordinary trigger policies use only player-facing observations and decisions.
//! Physical identities and retained recovery assertions belong to the host.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{
    Event, EventOutcome, EvidenceRecord, ObjectHandle, PlayerRequest, PlayerResponse, PlayerStatus,
    PlayerTargetEntity,
};
use std::collections::BTreeMap;
const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const WARDEN: &str = "6ca2a89e-7032-4864-b4e9-66f3178f90ab";
const VISIONARY: &str = "c6a3a882-a127-4590-93d7-679ef4313efe";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const PINGER: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    VisionaryEtb,
    VisionaryStifled,
    VisionaryBounced,
    WardenBounced,
    WardenStifled,
    VisionaryCountered,
    EmptyDrawGainFirst,
    EmptyDrawDrawFirst,
    ArtistSelfDeath,
    ArtistSacrifice,
    ArtistStifled,
    ArtistBounced,
    ArtistBorrowedOwnerLeaves,
    ArtistControllerLeaves,
    ArtistRetargetAccepted,
    ArtistTargetLeaves,
    SimultaneousForward,
    SimultaneousReverse,
    CommanderReturn,
    CommanderDecline,
    NonactiveActivation,
    ResearcherEmptyDraw,
    TriggerEliminatesActive,
}
const POLICIES: [Policy; 23] = [
    Policy::VisionaryEtb,
    Policy::VisionaryStifled,
    Policy::VisionaryBounced,
    Policy::WardenBounced,
    Policy::WardenStifled,
    Policy::VisionaryCountered,
    Policy::EmptyDrawGainFirst,
    Policy::EmptyDrawDrawFirst,
    Policy::ArtistSelfDeath,
    Policy::ArtistSacrifice,
    Policy::ArtistStifled,
    Policy::ArtistBounced,
    Policy::ArtistBorrowedOwnerLeaves,
    Policy::ArtistControllerLeaves,
    Policy::ArtistRetargetAccepted,
    Policy::ArtistTargetLeaves,
    Policy::SimultaneousForward,
    Policy::SimultaneousReverse,
    Policy::CommanderReturn,
    Policy::CommanderDecline,
    Policy::NonactiveActivation,
    Policy::ResearcherEmptyDraw,
    Policy::TriggerEliminatesActive,
];
impl Policy {
    fn etb(self) -> bool {
        matches!(
            self,
            Self::VisionaryEtb
                | Self::VisionaryStifled
                | Self::VisionaryBounced
                | Self::WardenBounced
                | Self::WardenStifled
                | Self::VisionaryCountered
                | Self::EmptyDrawGainFirst
                | Self::EmptyDrawDrawFirst
        )
    }
    fn combat(self) -> bool {
        matches!(
            self,
            Self::SimultaneousForward | Self::SimultaneousReverse | Self::TriggerEliminatesActive
        )
    }
    fn commander(self) -> bool {
        matches!(self, Self::CommanderReturn | Self::CommanderDecline)
    }
    fn stifle(self) -> bool {
        matches!(
            self,
            Self::VisionaryStifled | Self::WardenStifled | Self::ArtistStifled
        )
    }
    fn bounce(self) -> bool {
        matches!(
            self,
            Self::VisionaryBounced | Self::WardenBounced | Self::ArtistBounced
        )
    }
    fn empty(self) -> bool {
        matches!(
            self,
            Self::EmptyDrawGainFirst | Self::EmptyDrawDrawFirst | Self::ResearcherEmptyDraw
        )
    }
    fn config(self) -> ScenarioConfig {
        match self {
            Self::ArtistBorrowedOwnerLeaves => ScenarioConfig::triggers_borrowed(),
            Self::NonactiveActivation => ScenarioConfig::triggers_nonactive(),
            Self::TriggerEliminatesActive => ScenarioConfig::triggers_low(),
            _ if self.empty() => ScenarioConfig::triggers_empty_library(),
            _ => ScenarioConfig::triggers(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Boundary {
    decisions: usize,
    concessions: usize,
    rule_boundary: u64,
    state_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Evidence {
    pub config: ScenarioConfig,
    pub policy: Policy,
    pub journal: ReplayJournal,
    pub checkpoints: Vec<Boundary>,
    pub final_snapshot: String,
}
struct Strategy {
    policy: Policy,
    primary: bool,
    response: bool,
    departed: bool,
    attacked: bool,
    trigger_seen: bool,
    priority_recorded: bool,
    choices: Vec<Observation>,
}
fn handle(
    v: &mtg_core::PermittedView,
    id: &str,
    owner: PlayerId,
    zone: Zone,
) -> Option<ObjectHandle> {
    v.objects
        .iter()
        .find(|o| o.oracle_id == id && o.owner == owner && o.zone == zone)
        .map(|o| o.handle.clone())
}
fn oracle<'a>(
    v: &'a mtg_core::PermittedView,
    entry: &'a mtg_core::VisibleStackEntry,
) -> Result<&'a str, String> {
    if let Some(t) = &entry.triggered {
        return Ok(&t.oracle_id);
    }
    if let Some(a) = &entry.ability {
        return Ok(&a.oracle_id);
    }
    let h = entry
        .spell
        .as_ref()
        .ok_or("stack spell has no source reference")?;
    v.objects
        .iter()
        .find(|o| &o.handle == h)
        .map(|o| o.oracle_id.as_str())
        .ok_or_else(|| "visible stack source is absent".into())
}
fn selected(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Result<BTreeMap<Color, Quantity>, String> {
    let mut free = available.clone();
    let mut result = BTreeMap::<Color, Quantity>::new();
    for color in &cost.colored {
        let n = free.get_mut(color).ok_or("policy lacks colored mana")?;
        if *n < 1.into() {
            return Err("policy lacks colored mana".into());
        }
        n.subtract(&1.into());
        result.entry(*color).or_default().add(&1.into());
    }
    let mut generic = cost.generic.clone();
    // The policy chooses and submits explicit deterministic color spending.
    for color in [Color::B, Color::R, Color::G, Color::U, Color::W, Color::C] {
        let amount = std::cmp::min(
            free.get(&color).cloned().unwrap_or_default(),
            generic.clone(),
        );
        if amount > Quantity::zero() {
            generic.subtract(&amount);
            result.entry(color).or_default().add(&amount);
        }
    }
    if generic != Quantity::zero() {
        return Err("policy lacks generic mana".into());
    }
    Ok(result)
}
impl Strategy {
    fn done(&self, o: &Observation) -> bool {
        self.primary
            && (!self.policy.combat() || self.trigger_seen)
            && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
            && o.view.stack.is_empty()
    }
    fn departure(&mut self, o: &Observation) -> Option<PlayerId> {
        if self.departed {
            return None;
        }
        let p = match self.policy {
            Policy::ArtistBorrowedOwnerLeaves
                if matches!(o.decision, Some(PlayerRequest::OrderTriggers { .. })) =>
            {
                Some(PlayerId(2))
            }
            Policy::ArtistControllerLeaves
                if matches!(o.decision, Some(PlayerRequest::OrderTriggers { .. })) =>
            {
                Some(PlayerId(0))
            }
            Policy::ArtistTargetLeaves
                if matches!(o.decision, Some(PlayerRequest::Priority { .. }))
                    && o.view
                        .stack
                        .iter()
                        .filter(|s| s.triggered.is_some())
                        .count()
                        == 5 =>
            {
                Some(PlayerId(3))
            }
            _ => None,
        };
        if p.is_some() {
            self.departed = true;
        }
        p
    }
    fn cast(&self, v: &mtg_core::PermittedView, id: &str) -> Result<PlayerResponse, String> {
        Ok(PlayerResponse::Cast {
            card: handle(v, id, v.viewer, Zone::Hand { owner: v.viewer })
                .ok_or("missing trigger policy spell")?,
        })
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let request = o
            .decision
            .as_ref()
            .ok_or("missing trigger policy request")?;
        Ok(match request {
            PlayerRequest::SacrificeCost { .. } | PlayerRequest::ForcedSacrifice { .. } => {
                return Err("sacrifice request outside historical profile".into());
            }
            PlayerRequest::ManaColor { .. }
            | PlayerRequest::SearchLands { .. }
            | PlayerRequest::OptionalEffect { .. }
            | PlayerRequest::OptionalRecursion { .. } => {
                return Err("staples request outside historical policy".into());
            }
            PlayerRequest::Mulligan { .. } | PlayerRequest::BottomOpening { .. } => {
                return Err("opening setup is not admitted by this historical scenario".into());
            }
            PlayerRequest::Modes { .. }
            | PlayerRequest::DamageAllocation { .. }
            | PlayerRequest::DamageEffect { .. } => {
                return Err("damage effects outside historical trigger profile".into());
            }
            PlayerRequest::Priority { .. } => {
                let has_triggers = v.stack.iter().any(|s| s.triggered.is_some());
                if has_triggers {
                    self.trigger_seen = true;
                    if !self.priority_recorded {
                        self.priority_recorded = true;
                        self.choices.push(o.clone());
                    }
                }
                if !self.primary
                    && !self.policy.combat()
                    && v.stack.is_empty()
                    && v.viewer
                        == if self.policy.commander() || self.policy == Policy::NonactiveActivation
                        {
                            PlayerId(1)
                        } else {
                            PlayerId(0)
                        }
                {
                    self.primary = true;
                    if self.policy.etb() {
                        self.cast(v, VISIONARY)?
                    } else if self.policy.commander() {
                        self.cast(v, BOLT)?
                    } else {
                        let id = if self.policy == Policy::ArtistSelfDeath {
                            PINGER
                        } else {
                            HAPLESS
                        };
                        PlayerResponse::ActivateAbility {
                            object: v
                                .objects
                                .iter()
                                .find(|x| {
                                    x.oracle_id == id
                                        && x.controller == v.viewer
                                        && x.zone == Zone::Battlefield
                                })
                                .ok_or("missing policy activated source")?
                                .handle
                                .clone(),
                            ability: 0,
                        }
                    }
                } else if v.viewer == PlayerId(1)
                    && !self.response
                    && ((self.policy.stifle() || self.policy.bounce()) && has_triggers
                        || self.policy == Policy::VisionaryCountered
                            && v.stack.iter().any(|s| {
                                s.spell.is_some() && oracle(v, s).is_ok_and(|x| x == VISIONARY)
                            }))
                {
                    self.response = true;
                    self.cast(
                        v,
                        if self.policy.stifle() {
                            STIFLE
                        } else if self.policy.bounce() {
                            UNSUMMON
                        } else {
                            COUNTER
                        },
                    )?
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::OrderTriggers { candidates, .. } => {
                self.trigger_seen = true;
                self.choices.push(o.clone());
                let mut order = candidates
                    .iter()
                    .map(|c| c.handle.clone())
                    .collect::<Vec<_>>();
                if self.policy.etb() {
                    // Last placed resolves first. Choose the relative order of own draw/gain.
                    order.sort_by_key(|h| {
                        let c = candidates
                            .iter()
                            .find(|c| &c.handle == h)
                            .expect("candidate");
                        if self.policy == Policy::EmptyDrawDrawFirst {
                            usize::from(c.oracle_id == VISIONARY)
                        } else {
                            usize::from(c.oracle_id == WARDEN)
                        }
                    });
                } else if self.policy == Policy::SimultaneousReverse {
                    order.reverse();
                }
                PlayerResponse::OrderTriggers { order }
            }
            PlayerRequest::PlayerTargets { candidates, .. } => {
                self.trigger_seen = true;
                self.choices.push(o.clone());
                let player = if self.policy == Policy::TriggerEliminatesActive {
                    PlayerId(0)
                } else if candidates.contains(&PlayerId(3)) {
                    PlayerId(3)
                } else {
                    PlayerId(2)
                };
                if !candidates.contains(&player) {
                    return Err("missing intended trigger target".into());
                }
                PlayerResponse::PlayerTargets {
                    players: vec![player],
                }
            }
            PlayerRequest::EntityTargets { candidates, .. } => {
                let (id, owner) = if self.policy.commander() {
                    (COMMANDER, PlayerId(0))
                } else {
                    (ARTIST, PlayerId(0))
                };
                let target = PlayerTargetEntity::Object {
                    object: handle(v, id, owner, Zone::Battlefield)
                        .ok_or("missing death target")?,
                };
                if !candidates.contains(&target) {
                    return Err("death target is ineligible".into());
                }
                PlayerResponse::EntityTargets {
                    entities: vec![target],
                }
            }
            PlayerRequest::StackTargets { candidates, .. } => {
                let wanted = if self.policy == Policy::VisionaryCountered {
                    VISIONARY
                } else if self.policy == Policy::WardenStifled {
                    WARDEN
                } else if self.policy == Policy::ArtistStifled {
                    ARTIST
                } else {
                    VISIONARY
                };
                let entry = v
                    .stack
                    .iter()
                    .find(|s| {
                        oracle(v, s).is_ok_and(|id| id == wanted)
                            && if self.policy == Policy::VisionaryCountered {
                                s.spell.is_some()
                            } else {
                                s.triggered.is_some()
                                    && s.controller
                                        == if self.policy == Policy::ArtistStifled {
                                            PlayerId(3)
                                        } else {
                                            PlayerId(0)
                                        }
                            }
                    })
                    .ok_or("missing intended counter target")?;
                let target = entry.handle.clone().ok_or("missing public stack handle")?;
                if !candidates.contains(&target) {
                    return Err("trigger counter target is ineligible".into());
                }
                PlayerResponse::StackTargets {
                    entries: vec![target],
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let wanted = match self.policy {
                    Policy::VisionaryBounced => VISIONARY,
                    Policy::WardenBounced => WARDEN,
                    _ => ARTIST,
                };
                let target = handle(v, wanted, PlayerId(0), Zone::Battlefield)
                    .ok_or("missing bounced observer")?;
                if !candidates.contains(&target) {
                    return Err("bounce target is ineligible".into());
                }
                PlayerResponse::Targets {
                    objects: vec![target],
                }
            }
            PlayerRequest::Mana { .. } => {
                if let Some(source) = v.objects.iter().find(|x| {
                    x.zone == Zone::Battlefield
                        && x.controller == v.viewer
                        && !x.tapped
                        && matches!(
                            x.oracle_id.as_str(),
                            "56719f6a-1a6c-4c0a-8d21-18f7d7350b68"
                                | "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6"
                                | "bc71ebf6-2056-41f7-be35-b2e5c34afa99"
                                | "b2c6aa39-2d2a-459c-a555-fb48ba993373"
                                | "a3fb7228-e76b-4e96-a40e-20b5fed75685"
                        )
                }) {
                    PlayerResponse::ActivateMana {
                        object: source.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: true }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: selected(cost, available)?,
            },
            PlayerRequest::ResolveDiscard { candidates, .. } => {
                self.choices.push(o.clone());
                let id = if self.policy == Policy::NonactiveActivation {
                    BOLT
                } else {
                    VISIONARY
                };
                let object = handle(v, id, v.viewer, Zone::Hand { owner: v.viewer })
                    .ok_or("missing trigger-policy discard")?;
                if !candidates.contains(&object) {
                    return Err("discard is ineligible".into());
                }
                PlayerResponse::ResolveDiscard {
                    objects: vec![object],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                self.choices.push(o.clone());
                PlayerResponse::CommanderReturn {
                    to_command: self.policy == Policy::CommanderReturn,
                }
            }
            PlayerRequest::Attackers {
                candidates,
                defenders,
                ..
            } => {
                let attacks = if self.policy.combat() && v.viewer == PlayerId(0) && !self.attacked {
                    self.attacked = true;
                    self.primary = true;
                    let artists = v
                        .objects
                        .iter()
                        .filter(|x| {
                            x.oracle_id == ARTIST
                                && x.controller == v.viewer
                                && candidates.contains(&x.handle)
                        })
                        .take(2)
                        .collect::<Vec<_>>();
                    if artists.len() != 2
                        || !defenders.contains(&PlayerId(1))
                        || !defenders.contains(&PlayerId(2))
                    {
                        return Err("missing simultaneous attacker domain".into());
                    }
                    artists
                        .into_iter()
                        .enumerate()
                        .map(|(n, x)| (x.handle.clone(), PlayerId(n as u8 + 1)))
                        .collect()
                } else {
                    vec![]
                };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => {
                let blocks =
                    if self.policy.combat() && matches!(v.viewer, PlayerId(1) | PlayerId(2)) {
                        let blocker = v
                            .objects
                            .iter()
                            .find(|x| {
                                x.oracle_id == BEARS
                                    && x.controller == v.viewer
                                    && candidates.contains(&x.handle)
                            })
                            .ok_or("missing simultaneous blocker")?;
                        vec![(
                            blocker.handle.clone(),
                            attackers.first().ok_or("missing blocked Artist")?.clone(),
                        )]
                    } else {
                        vec![]
                    };
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage {
                power, recipients, ..
            } => PlayerResponse::Damage {
                assignments: recipients
                    .iter()
                    .enumerate()
                    .map(|(n, x)| {
                        (
                            x.clone(),
                            if n == 0 {
                                power.clone()
                            } else {
                                Quantity::zero()
                            },
                        )
                    })
                    .collect(),
            },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            PlayerRequest::Sacrifice { .. } => {
                return Err("unexpected trigger sacrifice choice".into());
            }
        })
    }
}
fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("trigger query rejected: {reason:?}"));
            }
            QueryResult::Incomplete { work, .. } => {
                query = game.resume_work(work, Budget::new(quantum))
            }
        }
    }
}
fn observe(game: &Game, player: PlayerId, quantum: u64) -> Result<Observation, String> {
    match complete(game, game.observe(player, Budget::new(quantum)), quantum)? {
        QueryValue::Observation(value) => Ok(value),
        _ => Err("wrong trigger observation result".into()),
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recovery {
    None,
    Capture,
    Verify,
}
fn capture(
    game: &Game,
    checkpoints: &mut Vec<Boundary>,
    verifier: Option<&mut SnapshotVerifier>,
) -> Result<(), String> {
    let (snapshot, digest) = if verifier.is_some() {
        let (s, d) = game.snapshot_with_digest()?;
        (Some(s), d)
    } else {
        (None, game.state_digest()?)
    };
    let point = Boundary {
        decisions: game.journal().len(),
        concessions: game.replay_journal().concessions.len(),
        rule_boundary: game.rule_boundary(),
        state_sha256: digest,
    };
    if checkpoints.last() == Some(&point) {
        return Ok(());
    }
    if let Some(verifier) = verifier {
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing trigger snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("trigger boundary recovery differs".into());
        }
    }
    checkpoints.push(point);
    Ok(())
}
struct Run {
    game: Game,
    checkpoints: Vec<Boundary>,
    choices: Vec<Observation>,
}
fn run(
    catalog: Catalog,
    policy: Policy,
    mode: TraceMode,
    quantum: u64,
    recovery: Recovery,
) -> Result<Run, String> {
    if quantum == 0 {
        return Err("trigger runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut strategy = Strategy {
        policy,
        primary: false,
        response: false,
        departed: false,
        attacked: false,
        trigger_seen: false,
        priority_recorded: false,
        choices: vec![],
    };
    let mut checkpoints = vec![];
    let mut verifier =
        (recovery == Recovery::Verify).then(|| SnapshotVerifier::new(prepared, mode));
    if recovery != Recovery::None {
        capture(&game, &mut checkpoints, verifier.as_mut())?;
    }
    for _ in 0..8192 {
        let next = game.advance(Budget::new(quantum));
        if recovery != Recovery::None {
            capture(&game, &mut checkpoints, verifier.as_mut())?;
        }
        match next {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => {
                let o = observe(&game, request.player(), quantum)?;
                if let Some(player) = strategy.departure(&o) {
                    game.concede(player)?;
                    if recovery != Recovery::None {
                        capture(&game, &mut checkpoints, verifier.as_mut())?;
                    }
                    continue;
                }
                if strategy.done(&o) {
                    return Ok(Run {
                        game,
                        checkpoints,
                        choices: strategy.choices,
                    });
                }
                let response = strategy.choose(&o)?;
                let token = o
                    .decision
                    .as_ref()
                    .ok_or("missing trigger token")?
                    .token()
                    .clone();
                match complete(
                    &game,
                    game.check_player_response(
                        request.player(),
                        token,
                        response,
                        Budget::new(quantum),
                    ),
                    quantum,
                )? {
                    QueryValue::Validated(receipt) => game.submit(receipt)?,
                    _ => return Err("wrong trigger validation result".into()),
                }
                if policy == Policy::ArtistRetargetAccepted
                    && !strategy.departed
                    && matches!(o.decision, Some(PlayerRequest::PlayerTargets { .. }))
                {
                    strategy.departed = true;
                    game.concede(PlayerId(3))?;
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "trigger witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!("trigger witness {policy:?} exhausted host budget"))
}

fn zone(g: &Game, id: u32) -> Option<Zone> {
    g.object_id(CardId(id)).and_then(|id| g.zone(id))
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let g = &run.game;
    let view = observation(g, PlayerId(3))?.view;
    let lives = view
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    let expected: [i64; 4] = match policy {
        Policy::VisionaryEtb
        | Policy::VisionaryStifled
        | Policy::VisionaryBounced
        | Policy::WardenBounced
        | Policy::EmptyDrawGainFirst => [41, 41, 41, 40],
        Policy::WardenStifled | Policy::EmptyDrawDrawFirst => [40, 41, 41, 40],
        Policy::VisionaryCountered | Policy::ArtistTargetLeaves => [40, 40, 40, 40],
        Policy::ArtistBorrowedOwnerLeaves => [42, 41, 40, 37],
        Policy::ArtistControllerLeaves => [40, 41, 41, 38],
        Policy::ArtistRetargetAccepted => [42, 41, 37, 40],
        Policy::SimultaneousForward | Policy::SimultaneousReverse => [44, 42, 42, 32],
        Policy::TriggerEliminatesActive => [0, 2, 2, 4],
        _ => [42, 41, 41, 36],
    };
    if lives != expected.map(Quantity::from) {
        return Err(format!(
            "trigger witness {policy:?} life result differs: {lives:?}"
        ));
    }
    let grave = |p| Zone::Graveyard { owner: PlayerId(p) };
    let hand = |p| Zone::Hand { owner: PlayerId(p) };
    let library = |p| Zone::Library { owner: PlayerId(p) };
    let ok = match policy {
        Policy::VisionaryEtb | Policy::WardenBounced | Policy::WardenStifled => {
            zone(g, 4) == Some(Zone::Battlefield) && zone(g, 14) == Some(hand(0))
        }
        Policy::VisionaryStifled => {
            zone(g, 4) == Some(Zone::Battlefield) && zone(g, 14) == Some(library(0))
        }
        Policy::VisionaryBounced => zone(g, 4) == Some(hand(0)) && zone(g, 14) == Some(hand(0)),
        Policy::VisionaryCountered => {
            zone(g, 4) == Some(grave(0)) && zone(g, 14) == Some(library(0))
        }
        Policy::EmptyDrawGainFirst
        | Policy::EmptyDrawDrawFirst
        | Policy::ResearcherEmptyDraw
        | Policy::TriggerEliminatesActive
        | Policy::ArtistControllerLeaves => view.players[0].status == PlayerStatus::Left,
        Policy::ArtistSelfDeath => zone(g, 1) == Some(grave(0)),
        Policy::ArtistBorrowedOwnerLeaves => {
            zone(g, 1).is_none() && view.players[2].status == PlayerStatus::Left
        }
        Policy::ArtistRetargetAccepted | Policy::ArtistTargetLeaves => {
            view.players[3].status == PlayerStatus::Left
        }
        Policy::SimultaneousForward | Policy::SimultaneousReverse => {
            zone(g, 1) == Some(grave(0)) && zone(g, 2) == Some(grave(0))
        }
        Policy::CommanderReturn => zone(g, 0) == Some(Zone::Command { owner: PlayerId(0) }),
        Policy::CommanderDecline => zone(g, 0) == Some(grave(0)),
        Policy::NonactiveActivation => {
            zone(g, 5) == Some(grave(2))
                && zone(g, 26) == Some(hand(1))
                && zone(g, 19) == Some(grave(1))
                && zone(g, 14) == Some(library(0))
        }
        Policy::ArtistBounced => zone(g, 1) == Some(hand(0)) && zone(g, 14) == Some(hand(0)),
        _ => zone(g, 5) == Some(grave(0)) && zone(g, 14) == Some(hand(0)),
    };
    if !ok {
        return Err(format!(
            "trigger witness {policy:?} physical result differs"
        ));
    }
    if g.commander_ledger()
        .history(CardId(0))
        .is_none_or(|h| !h.combat_damage.is_empty())
    {
        return Err("trigger fixture changed Commander combat damage".into());
    }
    if g.trace().mode == TraceMode::Detailed {
        let collected = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::TriggerCollected {
                    trigger_id,
                    source,
                    player,
                    subject,
                    ..
                } => Some((*trigger_id, *source, *player, *subject)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_count = if policy == Policy::VisionaryCountered {
            0
        } else if policy.etb() {
            4
        } else if policy.combat() {
            10
        } else {
            5
        };
        if collected.len() != expected_count {
            return Err(format!(
                "trigger witness {policy:?} collection differs: {}",
                collected.len()
            ));
        }
        let placed = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::TriggerPlaced {
                    trigger_id,
                    stack_id,
                    player,
                } => Some((*trigger_id, *stack_id, *player)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_players: Vec<PlayerId> = if policy == Policy::VisionaryCountered {
            vec![]
        } else if policy.etb() {
            vec![PlayerId(0), PlayerId(0), PlayerId(1), PlayerId(2)]
        } else if policy.combat() {
            [0, 0, 0, 0, 1, 1, 2, 2, 3, 3]
                .into_iter()
                .map(PlayerId)
                .collect()
        } else if policy == Policy::ArtistControllerLeaves {
            [1, 2, 3].into_iter().map(PlayerId).collect()
        } else if policy == Policy::ArtistBorrowedOwnerLeaves {
            [0, 0, 1, 3].into_iter().map(PlayerId).collect()
        } else if policy == Policy::ArtistRetargetAccepted {
            [0, 0, 1, 2].into_iter().map(PlayerId).collect()
        } else {
            [0, 0, 1, 2, 3].into_iter().map(PlayerId).collect()
        };
        if placed.iter().map(|(_, _, p)| *p).collect::<Vec<_>>() != expected_players {
            return Err(format!(
                "trigger witness {policy:?} APNAP placement differs"
            ));
        }
        for (id, _, _) in &placed {
            if collected.iter().filter(|(c, _, _, _)| c == id).count() != 1 {
                return Err("placed trigger lacks unique actual occurrence".into());
            }
        }
        // Bind each collection to the committed typed occurrence, including the
        // exact zone incarnation and the frozen controller/source facts.
        let mut pairs = std::collections::BTreeSet::new();
        for event in g.events() {
            if let Event::TriggerCollected {
                trigger_id,
                source,
                player,
                ability,
                occurrence,
                subject,
                source_facts,
            } = event
            {
                if source_facts.object != *source
                    || source_facts.controller != *player
                    || source_facts.zone != Zone::Battlefield
                    || *ability != 0
                    || !pairs.insert((source.card, subject.card))
                {
                    return Err("trigger source facts or unique source/subject pair differs".into());
                }
                let actual = g
                    .trace()
                    .records
                    .iter()
                    .find_map(|record| {
                        let EvidenceRecord::Batch { batch } = &record.record else {
                            return None;
                        };
                        batch
                            .occurrences
                            .iter()
                            .find(|entry| entry.id == *occurrence)
                    })
                    .ok_or("trigger occurrence missing from committed evidence")?;
                let movement = match &actual.actual {
                    EventOutcome::Moved { movement }
                    | EventOutcome::StateBasedMoved { movement, .. }
                    | EventOutcome::Sacrificed { movement, .. } => movement,
                    _ => return Err("trigger bound to a nonmovement occurrence".into()),
                };
                let expected = if policy.etb() {
                    movement.successor
                } else {
                    movement.previous
                };
                if expected != *subject
                    || (policy.etb() && movement.destination != Zone::Battlefield)
                    || (!policy.etb()
                        && (movement.from != Zone::Battlefield
                            || !matches!(movement.destination, Zone::Graveyard { .. })))
                    || collected
                        .iter()
                        .filter(|(id, _, _, _)| id == trigger_id)
                        .count()
                        != 1
                {
                    return Err("trigger subject is not the actual triggering movement".into());
                }
            }
        }
        // Same-controller choices are bottom-to-top; completion is LIFO after
        // explicit countering/departure removal. Counts distinguish a countered
        // net-zero Artist invocation from executing its loss and gain anyway.
        let mut pending_order = BTreeMap::new();
        let mut stack = Vec::new();
        let (mut completed, mut resolved, mut lost, mut gained, mut counters) = (0, 0, 0, 0, 0);
        for event in g.events() {
            match event {
                Event::TriggersOrdered { player, order } => {
                    pending_order.insert(*player, order.clone());
                }
                Event::TriggerPlaced {
                    trigger_id,
                    stack_id,
                    player,
                } => {
                    if let Some(order) = pending_order.get_mut(player) {
                        if order.first() != Some(trigger_id) {
                            return Err(
                                "trigger placement ignored chosen bottom-to-top order".into()
                            );
                        }
                        order.remove(0);
                    }
                    stack.push(*stack_id);
                }
                Event::Committed {
                    actual:
                        EventOutcome::StackObjectCountered {
                            target,
                            spell,
                            movement,
                            ..
                        },
                    ..
                } if stack.contains(target) => {
                    if *spell || movement.is_some() {
                        return Err("Stifle moved a source card".into());
                    }
                    counters += 1;
                    stack.retain(|id| id != target);
                }
                Event::StackRemovedForDeparture { stack_id, .. } => {
                    stack.retain(|id| id != stack_id)
                }
                Event::TriggeredAbilityCompleted {
                    stack_id,
                    resolved: did_resolve,
                } => {
                    if stack.pop() != Some(*stack_id) {
                        return Err("trigger resolution was not LIFO".into());
                    }
                    completed += 1;
                    resolved += usize::from(*did_resolve);
                }
                Event::Committed {
                    actual: EventOutcome::LifeLost { amount, .. },
                    ..
                } => {
                    if *amount != Quantity::from(1) {
                        return Err("Artist life loss differs".into());
                    }
                    lost += 1;
                }
                Event::Committed {
                    actual: EventOutcome::LifeGained { amount, .. },
                    ..
                } => {
                    if *amount != Quantity::from(1) {
                        return Err("trigger life gain differs".into());
                    }
                    gained += 1;
                }
                _ => {}
            }
        }
        let expected_counts = match policy {
            Policy::VisionaryCountered => (0, 0, 0, 0),
            Policy::VisionaryStifled => (3, 3, 0, 3),
            Policy::WardenStifled | Policy::EmptyDrawDrawFirst => (3, 3, 0, 2),
            _ if policy.etb() => (4, 4, 0, 3),
            Policy::ArtistStifled
            | Policy::ArtistBorrowedOwnerLeaves
            | Policy::ArtistRetargetAccepted => (4, 4, 4, 4),
            Policy::ArtistControllerLeaves => (3, 3, 3, 3),
            Policy::ArtistTargetLeaves => (4, 0, 0, 0),
            Policy::TriggerEliminatesActive => (6, 2, 2, 2),
            _ if policy.combat() => (10, 10, 10, 10),
            _ => (5, 5, 5, 5),
        };
        if !stack.is_empty()
            || (completed, resolved, lost, gained) != expected_counts
            || counters != usize::from(policy.stifle())
        {
            return Err(format!(
                "trigger completion/life/counter evidence differs for {policy:?}"
            ));
        }
        if policy.combat() {
            for subject in [CardId(1), CardId(2)] {
                if collected
                    .iter()
                    .filter(|(_, _, _, s)| s.card == subject)
                    .count()
                    != 5
                {
                    return Err("simultaneous deaths lost a pre-event observer".into());
                }
            }
            for source in [CardId(1), CardId(2)] {
                if collected
                    .iter()
                    .filter(|(_, s, _, _)| s.card == source)
                    .count()
                    != 2
                {
                    return Err("dying Artist did not observe both deaths".into());
                }
            }
        }
        if policy.commander() {
            let _first = g
                .events()
                .iter()
                .position(|e| matches!(e, Event::TriggerPlaced { .. }))
                .ok_or("missing Commander death trigger placement")?;
            if !run.choices.iter().any(|o| {
                matches!(o.decision, Some(PlayerRequest::CommanderReturn { .. }))
                    && o.view.stack.iter().all(|s| s.triggered.is_none())
            }) {
                return Err("Commander choice did not precede trigger placement".into());
            }
        }
        if policy == Policy::NonactiveActivation
            && !run.choices.iter().any(|o| {
                matches!(
                    o.decision,
                    Some(PlayerRequest::Priority {
                        player: PlayerId(1),
                        ..
                    })
                ) && o
                    .view
                    .stack
                    .iter()
                    .filter(|s| s.triggered.is_some())
                    .count()
                    == 5
            })
        {
            return Err("APNAP placement changed nonactive priority recipient".into());
        }
    }
    Ok(())
}
pub(super) fn suite(
    catalog: &Catalog,
    recover: bool,
) -> Result<(Vec<Evidence>, Vec<Game>, serde_json::Value), String> {
    let (mut evidence, mut games, mut reports) = (vec![], vec![], vec![]);
    for policy in POLICIES {
        eprintln!("running trigger {policy:?}");
        let reference = run(
            catalog.clone(),
            policy,
            TraceMode::Detailed,
            1,
            Recovery::Capture,
        )?;
        assert_outcome(&reference, policy)?;
        for (mode, quantum) in [(TraceMode::Off, u64::MAX), (TraceMode::Compact, 7)] {
            let other = run(catalog.clone(), policy, mode, quantum, Recovery::None)?;
            assert_outcome(&other, policy)?;
            if reference.game.state_digest()? != other.game.state_digest()?
                || reference.game.replay_journal() != other.game.replay_journal()
                || (mode == TraceMode::Compact
                    && reference.game.trace().verification != other.game.trace().verification)
            {
                return Err("trigger trace/budget partition differs".into());
            }
        }
        let g = reference.game;
        reports.push(serde_json::json!({"policy":policy,"state_sha256":g.state_digest()?,"turn":g.turn(),"journal":g.replay_journal(),"events":g.events(),"checkpoints":reference.checkpoints.len(),"choices":reference.choices,"final_observations":(0..4).map(|p|observation(&g,PlayerId(p))).collect::<Result<Vec<_>,_>>()?,"commander_ledger":g.commander_ledger()}));
        evidence.push(Evidence {
            config: policy.config(),
            policy,
            journal: g.replay_journal(),
            checkpoints: reference.checkpoints,
            final_snapshot: g.snapshot()?,
        });
        games.push(g);
    }
    if recover {
        verify(catalog, &evidence)?;
    }
    Ok((evidence, games, serde_json::Value::Array(reports)))
}
pub(super) fn verify(catalog: &Catalog, evidence: &[Evidence]) -> Result<(), String> {
    if evidence.len() != POLICIES.len() {
        return Err("missing trigger witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("trigger policy/config mismatch".into());
        }
        let initial = Game::with_trace(
            Prepared::admit(catalog.clone(), policy.config())?,
            TraceMode::Off,
        );
        let expected = Boundary {
            decisions: 0,
            concessions: 0,
            rule_boundary: 0,
            state_sha256: initial.state_digest()?,
        };
        if entry.checkpoints.first() != Some(&expected) {
            return Err("trigger root recovery differs".into());
        }
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        eprintln!(
            "verifying {policy:?}: {} recovery boundaries",
            entry.checkpoints.len()
        );
        let reference = run(catalog.clone(), policy, TraceMode::Off, 1, Recovery::Verify)?;
        assert_outcome(&reference, policy)?;
        let prepared = Prepared::admit(catalog.clone(), policy.config())?;
        let restored = Game::restore(prepared.clone(), &entry.final_snapshot)?;
        let replay = Game::replay(prepared, &entry.journal)?;
        if reference.checkpoints != entry.checkpoints
            || reference.game.replay_journal() != entry.journal
            || reference.game.state_digest()? != restored.state_digest()?
            || restored.trace().mode != TraceMode::Detailed
            || restored.trace() != replay.trace()
            || replay.state_digest()? != restored.state_digest()?
        {
            return Err("trigger retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_trigger_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::trigger_fixture_catalog().unwrap();
        let (evidence, _, _) = suite(&catalog, false).unwrap();
        let original = serde_json::to_value(&evidence).unwrap();
        for mutation in 0..4 {
            let mut changed = original.clone();
            match mutation {
                0 => changed.as_array_mut().unwrap().swap(0, 1),
                1 => changed[0]["checkpoints"] = serde_json::json!([]),
                2 => changed[0]["checkpoints"][0]["concessions"] = serde_json::json!(1),
                _ => changed[0]["checkpoints"][0]["state_sha256"] = serde_json::json!("forged"),
            };
            assert!(
                verify(
                    &catalog,
                    &serde_json::from_value::<Vec<Evidence>>(changed).unwrap()
                )
                .is_err()
            );
        }
        assert!(verify(&catalog, &evidence[..evidence.len() - 1]).is_err());
    }
}
