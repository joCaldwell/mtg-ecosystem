//! Source-backed damage/life policies consume only the acting player's view.
//! Trusted host assertions and replay verification are separate from decisions.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{
    Event, ObjectHandle, PlayerRequest, PlayerResponse, PlayerStatus, PlayerTargetEntity,
};
use std::collections::BTreeMap;

const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const PINGER: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const SOUL: &str = "8186fd80-015f-470c-9e1c-cbf45764a057";
const HEAL: &str = "3cc48835-3ac0-4774-b380-f9b21d2dc974";
const NECTAR: &str = "30870ee5-6ad7-48a9-983e-d3b018f2344f";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    BoltCreature,
    BoltPlayer,
    BoltSelf,
    BoltCommanderReturn,
    BoltCommanderDecline,
    BoltTargetBounce,
    BoltTargetDeparture,
    BoltCountered,
    PingerCreature,
    PingerPlayer,
    PingerSelf,
    PingerSourceBounce,
    PingerSourceKilled,
    PingerBorrowedDeparture,
    PingerTargetDeparture,
    PingerStifled,
    SoulOpponent,
    SoulSelf,
    SoulDepartedTarget,
    SoulCountered,
    HealingOther,
    HealingSelf,
    HealingDepartedTarget,
    HealingFailedDraw,
    Nectar,
    DamageThenBolt,
    HealingCommanderReturn,
    HealingCommanderDecline,
}
const POLICIES: [Policy; 28] = [
    Policy::BoltCreature,
    Policy::BoltPlayer,
    Policy::BoltSelf,
    Policy::BoltCommanderReturn,
    Policy::BoltCommanderDecline,
    Policy::BoltTargetBounce,
    Policy::BoltTargetDeparture,
    Policy::BoltCountered,
    Policy::PingerCreature,
    Policy::PingerPlayer,
    Policy::PingerSelf,
    Policy::PingerSourceBounce,
    Policy::PingerSourceKilled,
    Policy::PingerBorrowedDeparture,
    Policy::PingerTargetDeparture,
    Policy::PingerStifled,
    Policy::SoulOpponent,
    Policy::SoulSelf,
    Policy::SoulDepartedTarget,
    Policy::SoulCountered,
    Policy::HealingOther,
    Policy::HealingSelf,
    Policy::HealingDepartedTarget,
    Policy::HealingFailedDraw,
    Policy::Nectar,
    Policy::DamageThenBolt,
    Policy::HealingCommanderReturn,
    Policy::HealingCommanderDecline,
];
impl Policy {
    fn pinger(self) -> bool {
        matches!(
            self,
            Self::PingerCreature
                | Self::PingerPlayer
                | Self::PingerSelf
                | Self::PingerSourceBounce
                | Self::PingerSourceKilled
                | Self::PingerBorrowedDeparture
                | Self::PingerTargetDeparture
                | Self::PingerStifled
                | Self::DamageThenBolt
        )
    }
    fn soul(self) -> bool {
        matches!(
            self,
            Self::SoulOpponent | Self::SoulSelf | Self::SoulDepartedTarget | Self::SoulCountered
        )
    }
    fn healing(self) -> bool {
        matches!(
            self,
            Self::HealingOther
                | Self::HealingSelf
                | Self::HealingDepartedTarget
                | Self::HealingFailedDraw
                | Self::HealingCommanderReturn
                | Self::HealingCommanderDecline
        )
    }
    fn library(self) -> bool {
        matches!(
            self,
            Self::HealingCommanderReturn | Self::HealingCommanderDecline
        )
    }
    fn target_departure(self) -> bool {
        matches!(
            self,
            Self::BoltTargetDeparture
                | Self::PingerTargetDeparture
                | Self::SoulDepartedTarget
                | Self::HealingDepartedTarget
        )
    }
    fn counter(self) -> bool {
        matches!(self, Self::BoltCountered | Self::SoulCountered)
    }
    fn bounce(self) -> bool {
        matches!(self, Self::BoltTargetBounce | Self::PingerSourceBounce)
    }
    fn config(self) -> ScenarioConfig {
        if self.library() {
            ScenarioConfig::damage_life_library()
        } else if matches!(
            self,
            Self::SoulOpponent | Self::SoulSelf | Self::DamageThenBolt | Self::Nectar
        ) {
            ScenarioConfig::damage_life_low()
        } else if self == Self::PingerBorrowedDeparture {
            ScenarioConfig::damage_life_borrowed()
        } else if self == Self::HealingFailedDraw {
            ScenarioConfig::damage_life_empty_library()
        } else {
            ScenarioConfig::damage_life()
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
    primary: u8,
    second: bool,
    response: bool,
    departed: bool,
    choices: Vec<Observation>,
}
fn handle(
    v: &mtg_core::PermittedView,
    oracle: &str,
    owner: PlayerId,
    zone: Zone,
) -> Option<ObjectHandle> {
    v.objects
        .iter()
        .find(|o| o.oracle_id == oracle && o.owner == owner && o.zone == zone)
        .map(|o| o.handle.clone())
}
fn oracle<'a>(
    v: &'a mtg_core::PermittedView,
    entry: &'a mtg_core::VisibleStackEntry,
) -> Result<&'a str, String> {
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
    // Preserve white for Healing Hands after Time Ebb in the connected witness.
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
        if self.policy == Policy::HealingFailedDraw
            && o.view.players[0].status == PlayerStatus::Left
        {
            return true;
        }
        self.primary == if self.policy.library() { 2 } else { 1 }
            && (self.policy != Policy::DamageThenBolt || self.second)
            && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
            && o.view.stack.is_empty()
    }
    fn departure(&mut self, o: &Observation) -> Option<PlayerId> {
        if self.departed
            || self.primary == 0
            || o.view.stack.len() != 1
            || !matches!(o.decision, Some(PlayerRequest::Priority { .. }))
        {
            return None;
        }
        let p = if self.policy.target_departure() {
            Some(PlayerId(1))
        } else if self.policy == Policy::PingerBorrowedDeparture {
            Some(PlayerId(2))
        } else {
            None
        };
        if p.is_some() {
            self.departed = true;
        }
        p
    }
    fn cast(&self, v: &mtg_core::PermittedView, id: &str) -> Result<PlayerResponse, String> {
        Ok(PlayerResponse::Cast {
            card: handle(v, id, v.viewer, Zone::Hand { owner: v.viewer })
                .ok_or_else(|| format!("missing policy spell {id}"))?,
        })
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let r = o.decision.as_ref().ok_or("missing damage/life request")?;
        Ok(match r {
            PlayerRequest::Priority { .. } => {
                if v.viewer == PlayerId(0)
                    && v.stack.is_empty()
                    && self.primary < if self.policy.library() { 2 } else { 1 }
                {
                    let id = if self.policy.library() && self.primary == 0 {
                        TIME_EBB
                    } else if self.policy.soul() {
                        SOUL
                    } else if self.policy.healing() {
                        HEAL
                    } else if self.policy == Policy::Nectar {
                        NECTAR
                    } else {
                        BOLT
                    };
                    self.primary += 1;
                    if self.policy.pinger() {
                        PlayerResponse::ActivateAbility {
                            object: v
                                .objects
                                .iter()
                                .find(|object| {
                                    object.oracle_id == PINGER
                                        && object.controller == v.viewer
                                        && object.zone == Zone::Battlefield
                                })
                                .map(|object| object.handle.clone())
                                .ok_or("missing P0 activated source")?,
                            ability: 0,
                        }
                    } else {
                        self.cast(v, id)?
                    }
                } else if v.viewer == PlayerId(0)
                    && v.stack.is_empty()
                    && self.policy == Policy::DamageThenBolt
                    && self.primary == 1
                    && !self.second
                {
                    self.second = true;
                    self.cast(v, BOLT)?
                } else if v.viewer == PlayerId(1)
                    && v.stack.len() == 1
                    && self.primary > 0
                    && !self.response
                    && (self.policy.counter()
                        || self.policy.bounce()
                        || matches!(
                            self.policy,
                            Policy::PingerStifled | Policy::PingerSourceKilled
                        ))
                {
                    self.response = true;
                    self.cast(
                        v,
                        if self.policy.counter() {
                            COUNTER
                        } else if self.policy.bounce() {
                            UNSUMMON
                        } else if self.policy == Policy::PingerStifled {
                            STIFLE
                        } else {
                            BOLT
                        },
                    )?
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::EntityTargets { candidates, .. } => {
                let target = if self.policy == Policy::PingerSourceKilled && v.viewer == PlayerId(1)
                {
                    PlayerTargetEntity::Object {
                        object: handle(v, PINGER, PlayerId(0), Zone::Battlefield)
                            .ok_or("missing damage response source")?,
                    }
                } else if matches!(self.policy, Policy::BoltCreature | Policy::BoltTargetBounce) {
                    PlayerTargetEntity::Object {
                        object: handle(v, BEARS, PlayerId(1), Zone::Battlefield)
                            .ok_or("missing Bolt creature")?,
                    }
                } else if matches!(
                    self.policy,
                    Policy::BoltCommanderReturn | Policy::BoltCommanderDecline
                ) {
                    PlayerTargetEntity::Object {
                        object: handle(v, COMMANDER, PlayerId(1), Zone::Battlefield)
                            .ok_or("missing Bolt commander")?,
                    }
                } else if self.policy == Policy::PingerCreature {
                    PlayerTargetEntity::Object {
                        object: handle(v, PINGER, PlayerId(2), Zone::Battlefield)
                            .ok_or("missing opposing pinger")?,
                    }
                } else if self.policy == Policy::PingerSelf {
                    PlayerTargetEntity::Object {
                        object: handle(v, PINGER, PlayerId(0), Zone::Battlefield)
                            .ok_or("missing self-target source")?,
                    }
                } else {
                    PlayerTargetEntity::Player {
                        player: if self.policy == Policy::BoltSelf {
                            PlayerId(0)
                        } else {
                            PlayerId(1)
                        },
                    }
                };
                if !candidates.contains(&target) {
                    return Err("intended entity target is ineligible".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::EntityTargets {
                    entities: vec![target],
                }
            }
            PlayerRequest::PlayerTargets { candidates, .. } => {
                let player = if matches!(
                    self.policy,
                    Policy::SoulSelf
                        | Policy::HealingSelf
                        | Policy::HealingFailedDraw
                        | Policy::HealingCommanderReturn
                        | Policy::HealingCommanderDecline
                ) {
                    PlayerId(0)
                } else {
                    PlayerId(1)
                };
                if !candidates.contains(&player) {
                    return Err("intended player target is ineligible".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::PlayerTargets {
                    players: vec![player],
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let (id, owner) = if self.policy.library() {
                    (COMMANDER, PlayerId(0))
                } else if self.policy == Policy::BoltTargetBounce {
                    (BEARS, PlayerId(1))
                } else {
                    (PINGER, PlayerId(0))
                };
                let object = handle(v, id, owner, Zone::Battlefield)
                    .ok_or("missing bounce/library target")?;
                if !candidates.contains(&object) {
                    return Err("ordinary target is ineligible".into());
                }
                PlayerResponse::Targets {
                    objects: vec![object],
                }
            }
            PlayerRequest::StackTargets { candidates, .. } => {
                let entry = v
                    .stack
                    .iter()
                    .find(|x| {
                        if self.policy == Policy::PingerStifled {
                            x.ability.is_some()
                        } else {
                            oracle(v, x).is_ok_and(|id| {
                                id == if self.policy == Policy::SoulCountered {
                                    SOUL
                                } else {
                                    BOLT
                                }
                            })
                        }
                    })
                    .ok_or("missing counter target")?;
                let h = entry.handle.clone().ok_or("missing stack target handle")?;
                if !candidates.contains(&h) {
                    return Err("stack target is ineligible".into());
                }
                PlayerResponse::StackTargets { entries: vec![h] }
            }
            PlayerRequest::Mana { .. } => {
                // Every admitted mana source is a basic land. Facts come from the catalog.
                if let Some(source) = v.objects.iter().find(|x| {
                    x.zone == Zone::Battlefield
                        && x.controller == v.viewer
                        && !x.tapped
                        && matches!(
                            x.oracle_id.as_str(),
                            "a3fb7228-e76b-4e96-a40e-20b5fed75685"
                                | "56719f6a-1a6c-4c0a-8d21-18f7d7350b68"
                                | "b2c6aa39-2d2a-459c-a555-fb48ba993373"
                                | "bc71ebf6-2056-41f7-be35-b2e5c34afa99"
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
            } => {
                self.choices.push(o.clone());
                PlayerResponse::SpendMana {
                    spending: selected(cost, available)?,
                }
            }
            PlayerRequest::CommanderReturn { destination, .. } => {
                self.choices.push(o.clone());
                PlayerResponse::CommanderReturn {
                    to_command: match destination {
                        Zone::Library { .. } => false,
                        Zone::Hand { .. } => self.policy == Policy::HealingCommanderReturn,
                        Zone::Graveyard { .. } => self.policy != Policy::BoltCommanderDecline,
                        _ => return Err("unexpected damage/life commander return".into()),
                    },
                }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            PlayerRequest::ResolveDiscard { .. }
            | PlayerRequest::Sacrifice { .. }
            | PlayerRequest::Damage { .. } => return Err("unexpected damage/life request".into()),
        })
    }
}
fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("damage/life query rejected: {reason:?}"));
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
        _ => Err("wrong damage/life observation result".into()),
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
        let restored =
            verifier.verify(snapshot.as_deref().ok_or("missing damage/life snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("damage/life boundary recovery differs".into());
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
        return Err("damage/life runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut strategy = Strategy {
        policy,
        primary: 0,
        second: false,
        response: false,
        departed: false,
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
                    .ok_or("missing damage/life token")?
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
                    _ => return Err("wrong damage/life validation result".into()),
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "damage/life witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!(
        "damage/life witness {policy:?} exhausted host budget"
    ))
}

fn zone(g: &Game, id: u32) -> Option<Zone> {
    g.object_id(CardId(id)).and_then(|o| g.zone(o))
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let g = &run.game;
    let v = observation(g, PlayerId(3))?.view;
    let life = |p: usize, n: i64| v.players[p].life == n.into();
    let left = |p: usize| v.players[p].status == PlayerStatus::Left;
    let grave = |owner: u8| Zone::Graveyard {
        owner: PlayerId(owner),
    };
    let hand = |owner: u8| Zone::Hand {
        owner: PlayerId(owner),
    };
    let tapped = g
        .object_id(CardId(0))
        .and_then(|_| {
            v.objects
                .iter()
                .find(|x| x.oracle_id == PINGER && x.controller == PlayerId(0))
        })
        .is_some_and(|x| x.tapped);
    let ok = match policy {
        Policy::BoltCreature => zone(g, 15) == Some(grave(1)) && life(1, 40),
        Policy::BoltPlayer => life(1, 37) && life(0, 40),
        Policy::BoltSelf => life(0, 37) && life(1, 40),
        Policy::BoltCommanderReturn => zone(g, 14) == Some(Zone::Command { owner: PlayerId(1) }),
        Policy::BoltCommanderDecline => zone(g, 14) == Some(grave(1)),
        Policy::BoltTargetBounce => zone(g, 15) == Some(hand(1)) && life(1, 40),
        Policy::BoltTargetDeparture => left(1) && zone(g, 11) == Some(grave(0)),
        Policy::BoltCountered => {
            life(1, 40) && zone(g, 11) == Some(grave(0)) && zone(g, 22) == Some(grave(1))
        }
        Policy::PingerCreature => zone(g, 26) == Some(grave(2)) && tapped,
        Policy::PingerPlayer => life(1, 39) && tapped,
        Policy::PingerSelf => zone(g, 0) == Some(grave(0)),
        Policy::PingerSourceBounce => zone(g, 0) == Some(hand(0)) && life(1, 39),
        Policy::PingerSourceKilled => zone(g, 0) == Some(grave(0)) && life(1, 39),
        Policy::PingerBorrowedDeparture => zone(g, 0).is_none() && left(2) && life(1, 39),
        Policy::PingerTargetDeparture => left(1) && tapped,
        Policy::PingerStifled => life(1, 40) && tapped && zone(g, 23) == Some(grave(1)),
        Policy::SoulOpponent => left(1) && life(0, 8),
        Policy::SoulSelf => !left(0) && life(0, 4),
        Policy::SoulDepartedTarget => left(1) && life(0, 40),
        Policy::SoulCountered => life(0, 40) && life(1, 40),
        Policy::HealingOther => life(1, 44) && life(0, 40) && zone(g, 12) == Some(hand(0)),
        Policy::HealingSelf => life(0, 44) && zone(g, 12) == Some(hand(0)),
        Policy::HealingDepartedTarget => {
            left(1) && life(0, 40) && zone(g, 12) == Some(Zone::Library { owner: PlayerId(0) })
        }
        Policy::HealingFailedDraw => left(0) && life(0, 44) && zone(g, 10).is_none(),
        Policy::Nectar => life(0, 8),
        Policy::DamageThenBolt => left(1) && life(0, 4) && tapped,
        Policy::HealingCommanderReturn => {
            life(0, 44)
                && zone(g, 14) == Some(Zone::Command { owner: PlayerId(0) })
                && zone(g, 12) == Some(Zone::Library { owner: PlayerId(0) })
        }
        Policy::HealingCommanderDecline => {
            life(0, 44)
                && zone(g, 14) == Some(hand(0))
                && zone(g, 12) == Some(Zone::Library { owner: PlayerId(0) })
        }
    };
    if !ok {
        return Err(format!(
            "damage/life witness {policy:?} physical/life result differs"
        ));
    }
    if g.commander_ledger()
        .history(CardId(14))
        .is_none_or(|history| !history.combat_damage.is_empty())
    {
        return Err("noncombat damage changed Commander combat history".into());
    }
    if g.trace().mode == TraceMode::Detailed {
        let damage = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::Committed {
                    actual: mtg_core::EventOutcome::NoncombatDamageDealt { damage, result },
                    ..
                } => Some((damage, result)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_damage = match policy {
            Policy::BoltCreature
            | Policy::BoltPlayer
            | Policy::BoltSelf
            | Policy::BoltCommanderReturn
            | Policy::BoltCommanderDecline
            | Policy::PingerCreature
            | Policy::PingerPlayer
            | Policy::PingerSelf
            | Policy::PingerSourceBounce
            | Policy::PingerBorrowedDeparture => 1,
            Policy::PingerSourceKilled | Policy::DamageThenBolt => 2,
            _ => 0,
        };
        if damage.len() != expected_damage {
            return Err(format!(
                "damage/life witness {policy:?} actual damage count differs"
            ));
        }
        for (hit, result) in &damage {
            let expected_amount = if hit.source.object.card == CardId(0) {
                1.into()
            } else {
                3.into()
            };
            let mut expected = result.previous.clone();
            match hit.recipient {
                mtg_core::DamageRecipient::Player { .. } => expected.subtract(&hit.amount),
                mtg_core::DamageRecipient::Creature { .. } => expected.add(&hit.amount),
            }
            if hit.amount != expected_amount
                || result.current != expected
                || result.recipient != hit.recipient
                || hit.source.object.incarnation
                    != if hit.source.object.card == CardId(0) {
                        0
                    } else {
                        1
                    }
            {
                return Err("noncombat typed damage facts/delta differ".into());
            }
        }
        if matches!(
            policy,
            Policy::PingerSourceBounce
                | Policy::PingerSourceKilled
                | Policy::PingerBorrowedDeparture
        ) {
            let (hit, _) = damage
                .iter()
                .find(|(hit, _)| hit.source.object.card == CardId(0))
                .ok_or("missing detached source damage")?;
            if hit.source.zone != Zone::Battlefield
                || !hit.source.tapped
                || hit.controller != PlayerId(0)
                || hit.source.controller != PlayerId(0)
                || hit.source.owner
                    != if policy == Policy::PingerBorrowedDeparture {
                        PlayerId(2)
                    } else {
                        PlayerId(0)
                    }
                || hit.recipient
                    != (mtg_core::DamageRecipient::Player {
                        player: PlayerId(1),
                    })
            {
                return Err("detached damage source/recipient facts differ".into());
            }
            if policy == Policy::PingerSourceKilled && hit.source.marked_damage != 3.into() {
                return Err("damage source did not retain lethal last-known marks".into());
            }
        }
        let life_events = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::Committed {
                    actual:
                        mtg_core::EventOutcome::LifeGained {
                            player,
                            amount,
                            previous,
                            current,
                        },
                    ..
                } => Some((true, *player, amount, previous, current)),
                Event::Committed {
                    actual:
                        mtg_core::EventOutcome::LifeLost {
                            player,
                            amount,
                            previous,
                            current,
                        },
                    ..
                } => Some((false, *player, amount, previous, current)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_life = match policy {
            Policy::SoulOpponent => vec![(false, PlayerId(1)), (true, PlayerId(0))],
            Policy::SoulSelf => vec![(false, PlayerId(0)), (true, PlayerId(0))],
            Policy::HealingOther => vec![(true, PlayerId(1))],
            Policy::HealingSelf
            | Policy::HealingFailedDraw
            | Policy::HealingCommanderReturn
            | Policy::HealingCommanderDecline
            | Policy::Nectar => vec![(true, PlayerId(0))],
            _ => vec![],
        };
        if life_events
            .iter()
            .map(|(gain, player, _, _, _)| (*gain, *player))
            .collect::<Vec<_>>()
            != expected_life
        {
            return Err(format!(
                "damage/life witness {policy:?} ordered life events differ"
            ));
        }
        for (gain, _, amount, previous, current) in &life_events {
            let mut expected = (*previous).clone();
            if *gain {
                expected.add(amount);
            } else {
                expected.subtract(amount);
            }
            if **amount != 4.into() || **current != expected {
                return Err("typed life amount/delta differs".into());
            }
        }
        if policy == Policy::SoulSelf
            && (life_events[0].4 != &Quantity::zero() || life_events[1].3 != &Quantity::zero())
        {
            return Err("Soul Feast self did not pass through exact zero".into());
        }
        if policy == Policy::HealingFailedDraw {
            let gain = g
                .events()
                .iter()
                .position(|e| {
                    matches!(
                        e,
                        Event::Committed {
                            actual: mtg_core::EventOutcome::LifeGained {
                                player: PlayerId(0),
                                ..
                            },
                            ..
                        }
                    )
                })
                .ok_or("missing Healing gain")?;
            let failed = g
                .events()
                .iter()
                .position(|e| {
                    matches!(
                        e,
                        Event::Committed {
                            actual: mtg_core::EventOutcome::DrawFailed {
                                player: PlayerId(0)
                            },
                            ..
                        }
                    )
                })
                .ok_or("missing Healing failed draw")?;
            if gain >= failed
                || !g.events().iter().any(|e| {
                    matches!(
                        e,
                        Event::Committed {
                            actual: mtg_core::EventOutcome::Lost {
                                player: PlayerId(0),
                                reason: mtg_core::PlayerLossReason::FailedDraw
                            },
                            ..
                        }
                    )
                })
            {
                return Err("Healing Hands loss lacked ordered gain/failed-draw provenance".into());
            }
        }
        let completed = g
            .events()
            .iter()
            .filter(|e| matches!(e, Event::AbilityCompleted { .. }))
            .count();
        if completed != usize::from(policy.pinger() && !matches!(policy, Policy::PingerStifled)) {
            return Err(format!(
                "damage/life witness {policy:?} ability completion differs"
            ));
        }
        if matches!(policy, Policy::SoulOpponent | Policy::HealingFailedDraw) {
            let completed = g
                .events()
                .iter()
                .position(|e| matches!(e, Event::SpellCompleted { .. }))
                .ok_or("missing life spell completion")?;
            let lost = g
                .events()
                .iter()
                .position(|e| matches!(e, Event::PlayersLost { .. }))
                .ok_or("missing delayed life loss")?;
            if lost <= completed {
                return Err("SBA loss interrupted a resolving life spell".into());
            }
        }
        if matches!(
            policy,
            Policy::BoltTargetBounce
                | Policy::BoltTargetDeparture
                | Policy::SoulDepartedTarget
                | Policy::HealingDepartedTarget
        ) && !g.events().iter().any(|e| {
            matches!(
                e,
                Event::SpellCompleted {
                    resolved: false,
                    ..
                }
            )
        }) {
            return Err("sole illegal target did not fizzle the full spell".into());
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
        eprintln!("running damage/life {policy:?}");
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
                return Err("damage/life trace/budget partition differs".into());
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
        return Err("missing damage/life witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("damage/life policy/config mismatch".into());
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
            return Err("damage/life root recovery differs".into());
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
            return Err("damage/life retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_damage_life_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::damage_life_fixture_catalog().unwrap();
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
