//! Choices policies consume only the acting player's public observation.
//! Trusted departure fixtures, physical assertions and recovery stay in the host.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{
    DamageEffectKind, Event, EventOutcome, EvidenceRecord, ObjectHandle, PlayerRequest,
    PlayerResponse, PlayerTargetEntity,
};
use std::collections::BTreeMap;
const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
const BOUNCER: &str = "82076865-7917-4d3b-8fe8-c21bfcd64d29";
const MAGE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const SEEDS: &str = "dd3d4d96-1e88-49e1-8276-7f62c22ad91a";
const SALVE: &str = "8da8644c-75a1-4fe9-8e94-900d948d631c";
const ARTIST: &str = "310f141c-7f37-4729-aed6-dd9c09db448d";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    SeedsSame,
    SeedsDistinct,
    SeedsOneIllegal,
    SeedsTwoIllegal,
    SeedsAllIllegal,
    SeedsReincarnation,
    SeedsCountered,
    SeedsCasterLeaves,
    SeedsCleanup,
    SalveGainSelf,
    SalveGainOpponent,
    SalveShieldPlayer,
    SalveShieldCreature,
    SalveResidualBudget,
    SalveSplitFirst,
    SalveSplitLast,
    SalveCountered,
    SalveFizzle,
    SalveDoubleFirst,
    SalvePreventFirst,
    BouncerEmpty,
    BouncerNonempty,
    BouncerDrawFalse,
    BouncerDrawCastTrue,
    BouncerStifled,
    BouncerTargetGone,
    BouncerBorrowedOwnerLeaves,
    BouncerControllerLeaves,
    BouncerNoTargets,
    BouncerCast,
}
pub(super) const POLICIES: [Policy; 30] = [
    Policy::SeedsSame,
    Policy::SeedsDistinct,
    Policy::SeedsOneIllegal,
    Policy::SeedsTwoIllegal,
    Policy::SeedsAllIllegal,
    Policy::SeedsReincarnation,
    Policy::SeedsCountered,
    Policy::SeedsCasterLeaves,
    Policy::SeedsCleanup,
    Policy::SalveGainSelf,
    Policy::SalveGainOpponent,
    Policy::SalveShieldPlayer,
    Policy::SalveShieldCreature,
    Policy::SalveResidualBudget,
    Policy::SalveSplitFirst,
    Policy::SalveSplitLast,
    Policy::SalveCountered,
    Policy::SalveFizzle,
    Policy::SalveDoubleFirst,
    Policy::SalvePreventFirst,
    Policy::BouncerEmpty,
    Policy::BouncerNonempty,
    Policy::BouncerDrawFalse,
    Policy::BouncerDrawCastTrue,
    Policy::BouncerStifled,
    Policy::BouncerTargetGone,
    Policy::BouncerBorrowedOwnerLeaves,
    Policy::BouncerControllerLeaves,
    Policy::BouncerNoTargets,
    Policy::BouncerCast,
];
impl Policy {
    fn config(self) -> ScenarioConfig {
        match self {
            Self::SalveDoubleFirst | Self::SalvePreventFirst => ScenarioConfig::choices_furnace(),
            Self::BouncerNonempty => ScenarioConfig::conditions_nonempty(),
            Self::BouncerBorrowedOwnerLeaves => ScenarioConfig::conditions_borrowed(),
            Self::BouncerNoTargets => ScenarioConfig::conditions_no_targets(),
            Self::BouncerEmpty
            | Self::BouncerDrawFalse
            | Self::BouncerDrawCastTrue
            | Self::BouncerStifled
            | Self::BouncerTargetGone
            | Self::BouncerControllerLeaves
            | Self::BouncerCast => ScenarioConfig::conditions(),
            _ => ScenarioConfig::choices(),
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
#[derive(Clone, Copy)]
enum Aim {
    Object(&'static str, u8, usize),
    Player(u8),
    Stack(&'static str),
}
#[derive(Clone, Copy)]
enum Op {
    Cast(&'static str, Option<Aim>),
    Activate(&'static str, Option<Aim>),
    Attack(&'static str, u8),
    Block(&'static str, usize),
}
#[derive(Clone, Copy)]
enum When {
    Empty,
    Stack(&'static str),
    Step(TurnStep),
}
#[derive(Clone, Copy)]
struct Action {
    player: u8,
    op: Op,
    when: When,
}
#[derive(Clone, Copy)]
enum End {
    Empty,
    Combat,
    NextTurn,
}
struct Strategy {
    policy: Policy,
    actions: Vec<Action>,
    cursor: usize,
    current: Option<Op>,
    end: End,
    departed: bool,
    choices: Vec<Observation>,
}
fn obj(id: &'static str, owner: u8) -> Aim {
    Aim::Object(id, owner, 0)
}
fn c(player: u8, id: &'static str, target: Option<Aim>) -> Action {
    Action {
        player,
        op: Op::Cast(id, target),
        when: When::Empty,
    }
}
fn response(mut action: Action, id: &'static str) -> Action {
    action.when = When::Stack(id);
    action
}
fn plan(policy: Policy) -> (Vec<Action>, End) {
    use Policy::*;
    let seeds = c(0, SEEDS, Some(obj(OX, 0)));
    let salve = |aim| c(0, SALVE, Some(aim));
    let kill = c(1, BOLT, Some(obj(BOUNCER, 0)));
    let draw = Action {
        player: 0,
        op: Op::Activate(MAGE, None),
        when: When::Stack(BOUNCER),
    };
    match policy {
        SeedsSame | SeedsDistinct | SeedsCasterLeaves => (vec![seeds], End::Empty),
        SeedsCleanup => (vec![seeds], End::NextTurn),
        SeedsOneIllegal | SeedsTwoIllegal | SeedsAllIllegal | SeedsReincarnation => {
            let aim = if matches!(policy, SeedsAllIllegal | SeedsReincarnation) {
                obj(OX, 0)
            } else {
                obj(BEARS, 1)
            };
            let mut actions = vec![seeds, response(c(1, UNSUMMON, Some(aim)), SEEDS)];
            if policy == SeedsReincarnation {
                actions.push(c(0, OX, None));
            }
            (actions, End::Empty)
        }
        SeedsCountered => (
            vec![
                seeds,
                response(c(1, COUNTER, Some(Aim::Stack(SEEDS))), SEEDS),
            ],
            End::Empty,
        ),
        SalveGainSelf => (vec![salve(Aim::Player(0))], End::Empty),
        SalveGainOpponent => (vec![salve(Aim::Player(1))], End::Empty),
        SalveShieldPlayer | SalveDoubleFirst | SalvePreventFirst => (
            vec![salve(Aim::Player(1)), c(1, BOLT, Some(Aim::Player(1)))],
            End::Empty,
        ),
        SalveShieldCreature => (
            vec![salve(obj(OX, 0)), c(1, BOLT, Some(obj(OX, 0)))],
            End::Empty,
        ),
        SalveResidualBudget => (
            vec![
                salve(Aim::Player(1)),
                Action {
                    player: 0,
                    op: Op::Attack(COMMANDER, 1),
                    when: When::Empty,
                },
                Action {
                    player: 1,
                    op: Op::Cast(BOLT, Some(Aim::Player(1))),
                    when: When::Step(TurnStep::EndCombat),
                },
            ],
            End::Combat,
        ),
        SalveSplitFirst | SalveSplitLast => (
            vec![
                seeds,
                salve(obj(OX, 0)),
                Action {
                    player: 0,
                    op: Op::Attack(OX, 1),
                    when: When::Empty,
                },
                Action {
                    player: 1,
                    op: Op::Block(BEARS, 2),
                    when: When::Empty,
                },
            ],
            End::Combat,
        ),
        SalveCountered => (
            vec![
                salve(Aim::Player(0)),
                response(c(1, COUNTER, Some(Aim::Stack(SALVE))), SALVE),
            ],
            End::Empty,
        ),
        SalveFizzle => (
            vec![
                salve(obj(OX, 0)),
                response(c(1, UNSUMMON, Some(obj(OX, 0))), SALVE),
            ],
            End::Empty,
        ),
        BouncerEmpty | BouncerControllerLeaves | BouncerNoTargets => (vec![kill], End::Empty),
        BouncerNonempty => (
            vec![kill, response(salve(Aim::Player(0)), ARTIST)],
            End::Empty,
        ),
        BouncerDrawFalse => (vec![kill, draw], End::Empty),
        BouncerDrawCastTrue => (
            vec![kill, draw, response(salve(Aim::Player(0)), BOUNCER)],
            End::Empty,
        ),
        BouncerStifled => (
            vec![
                kill,
                response(c(1, STIFLE, Some(Aim::Stack(BOUNCER))), BOUNCER),
            ],
            End::Empty,
        ),
        BouncerTargetGone => (
            vec![kill, response(c(1, UNSUMMON, Some(obj(BEARS, 1))), BOUNCER)],
            End::Empty,
        ),
        BouncerBorrowedOwnerLeaves => (vec![c(1, BOLT, Some(obj(BOUNCER, 2)))], End::Empty),
        BouncerCast => (
            vec![
                c(1, UNSUMMON, Some(obj(BOUNCER, 0))),
                c(0, BOUNCER, None),
                kill,
            ],
            End::Empty,
        ),
    }
}
fn seed_targets(policy: Policy) -> Vec<Aim> {
    match policy {
        Policy::SeedsDistinct | Policy::SeedsOneIllegal => {
            vec![obj(OX, 0), obj(BEARS, 1), obj(COMMANDER, 0)]
        }
        Policy::SeedsTwoIllegal => vec![obj(OX, 0), obj(BEARS, 1), obj(BEARS, 1)],
        Policy::SeedsCasterLeaves => vec![obj(BEARS, 1); 3],
        Policy::SalveSplitFirst | Policy::SalveSplitLast => vec![
            obj(OX, 0),
            Aim::Object(BEARS, 1, 0),
            Aim::Object(BEARS, 1, 1),
        ],
        _ => vec![obj(OX, 0); 3],
    }
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
fn aim_handle(v: &mtg_core::PermittedView, aim: Aim) -> Result<ObjectHandle, String> {
    let Aim::Object(id, owner, nth) = aim else {
        return Err("effect aim is not an object".into());
    };
    v.objects
        .iter()
        .filter(|o| o.oracle_id == id && o.owner == PlayerId(owner) && o.zone == Zone::Battlefield)
        .nth(nth)
        .map(|o| o.handle.clone())
        .ok_or_else(|| format!("effect target {id} owner{owner} is absent"))
}
impl Strategy {
    fn done(&self, o: &Observation) -> bool {
        self.cursor == self.actions.len()
            && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
            && o.view.stack.is_empty()
            && match self.end {
                End::Empty => true,
                End::Combat => o.view.turn.step == TurnStep::EndCombat,
                End::NextTurn => {
                    o.view.turn.number > 1 && o.view.turn.step == TurnStep::PrecombatMain
                }
            }
    }
    fn departure(&mut self, o: &Observation) -> Option<PlayerId> {
        if self.departed || !matches!(o.decision, Some(PlayerRequest::Priority { .. })) {
            return None;
        }
        let has = |id| {
            o.view
                .stack
                .iter()
                .any(|s| oracle(&o.view, s).is_ok_and(|x| x == id))
        };
        let player = match self.policy {
            Policy::SeedsCasterLeaves if self.cursor == 1 && o.view.stack.is_empty() => {
                Some(PlayerId(0))
            }
            Policy::BouncerBorrowedOwnerLeaves if has(BOUNCER) => Some(PlayerId(2)),
            Policy::BouncerControllerLeaves if has(BOUNCER) => Some(PlayerId(0)),
            _ => None,
        };
        if player.is_some() {
            self.departed = true;
            self.choices.push(o.clone());
        }
        player
    }
    fn target(&self) -> Result<Aim, String> {
        match self.current {
            Some(Op::Cast(_, Some(target)) | Op::Activate(_, Some(target))) => Ok(target),
            _ => Err("effect request lacks chosen action target".into()),
        }
    }
    fn execute(&mut self, o: &Observation, action: Action) -> Result<PlayerResponse, String> {
        self.cursor += 1;
        self.current = Some(action.op);
        self.choices.push(o.clone());
        match action.op {
            Op::Cast(id, _) => Ok(PlayerResponse::Cast {
                card: handle(
                    &o.view,
                    id,
                    PlayerId(action.player),
                    Zone::Hand {
                        owner: PlayerId(action.player),
                    },
                )
                .ok_or("missing choices spell")?,
            }),
            Op::Activate(id, _) => Ok(PlayerResponse::ActivateAbility {
                object: o
                    .view
                    .objects
                    .iter()
                    .find(|x| {
                        x.oracle_id == id
                            && x.controller == o.view.viewer
                            && x.zone == Zone::Battlefield
                    })
                    .ok_or("missing choices activated source")?
                    .handle
                    .clone(),
                ability: 0,
            }),
            _ => Err("combat action at priority".into()),
        }
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let request = o.decision.as_ref().ok_or("missing choices request")?;
        if !matches!(request, PlayerRequest::Priority { .. }) {
            self.choices.push(o.clone());
        }
        Ok(match request {
            PlayerRequest::Priority { .. } => {
                if let Some(action) = self.actions.get(self.cursor).copied() {
                    let ready = match action.when {
                        When::Empty => v.stack.is_empty(),
                        When::Stack(id) => {
                            v.stack.iter().any(|s| oracle(v, s).is_ok_and(|x| x == id))
                        }
                        When::Step(step) => v.stack.is_empty() && v.turn.step == step,
                    };
                    // A scripted response to a lower trigger waits for the
                    // preceding draw ability to finish. Availability is read
                    // only from this viewer's own hand observation.
                    let available = match action.op {
                        Op::Cast(id, _) => handle(
                            v,
                            id,
                            PlayerId(action.player),
                            Zone::Hand {
                                owner: PlayerId(action.player),
                            },
                        )
                        .is_some(),
                        _ => true,
                    };
                    if ready
                        && available
                        && v.viewer == PlayerId(action.player)
                        && matches!(action.op, Op::Cast(..) | Op::Activate(..))
                    {
                        return self.execute(o, action);
                    }
                }
                PlayerResponse::Pass
            }
            PlayerRequest::Modes { candidates, .. } => {
                let mode = if matches!(
                    self.policy,
                    Policy::SalveGainSelf
                        | Policy::SalveGainOpponent
                        | Policy::SalveCountered
                        | Policy::BouncerNonempty
                        | Policy::BouncerDrawCastTrue
                ) {
                    0
                } else {
                    1
                };
                if !candidates.contains(&mode) {
                    return Err("chosen source mode unavailable".into());
                }
                PlayerResponse::Modes { mode }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let aims =
                    if v.stack.last().is_some_and(|s| {
                        s.triggered.as_ref().is_some_and(|t| t.oracle_id == BOUNCER)
                    }) {
                        vec![obj(BEARS, 1)]
                    } else if matches!(self.current, Some(Op::Cast(SEEDS, _))) {
                        seed_targets(self.policy)
                    } else {
                        vec![self.target()?]
                    };
                let objects = aims
                    .into_iter()
                    .map(|aim| aim_handle(v, aim))
                    .collect::<Result<Vec<_>, _>>()?;
                if objects.iter().any(|object| !candidates.contains(object)) {
                    return Err("chosen creature target ineligible".into());
                }
                PlayerResponse::Targets { objects }
            }
            PlayerRequest::EntityTargets { candidates, .. } => {
                let target = match self.target()? {
                    Aim::Player(p) => PlayerTargetEntity::Player {
                        player: PlayerId(p),
                    },
                    t => PlayerTargetEntity::Object {
                        object: aim_handle(v, t)?,
                    },
                };
                if !candidates.contains(&target) {
                    return Err("choices entity target ineligible".into());
                }
                PlayerResponse::EntityTargets {
                    entities: vec![target],
                }
            }
            PlayerRequest::StackTargets { candidates, .. } => {
                let Aim::Stack(id) = self.target()? else {
                    return Err("missing choices stack aim".into());
                };
                let entry = v
                    .stack
                    .iter()
                    .find(|s| oracle(v, s).is_ok_and(|x| x == id))
                    .ok_or("missing effect counter target")?;
                let target = entry.handle.clone().ok_or("missing choices stack handle")?;
                if !candidates.contains(&target) {
                    return Err("choices stack target ineligible".into());
                }
                PlayerResponse::StackTargets {
                    entries: vec![target],
                }
            }
            PlayerRequest::PlayerTargets { candidates, .. } => {
                let player = if v.stack.last().is_some_and(|s| s.triggered.is_some()) {
                    PlayerId(1)
                } else if let Aim::Player(p) = self.target()? {
                    PlayerId(p)
                } else {
                    return Err("missing chosen player target".into());
                };
                if !candidates.contains(&player) {
                    return Err("chosen player unavailable".into());
                }
                PlayerResponse::PlayerTargets {
                    players: vec![player],
                }
            }
            PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
                order: candidates.iter().map(|c| c.handle.clone()).collect(),
            },
            PlayerRequest::DamageEffect { candidates, .. } => {
                let kind = if self.policy == Policy::SalveDoubleFirst {
                    DamageEffectKind::DoubleDamage
                } else {
                    DamageEffectKind::PreventNextDamage
                };
                let chosen = candidates
                    .iter()
                    .find(|c| c.kind == kind)
                    .or_else(|| candidates.first())
                    .ok_or("no effect choice")?;
                PlayerResponse::DamageEffect {
                    effect: chosen.effect.clone(),
                    damage: chosen.damage.clone(),
                }
            }
            PlayerRequest::DamageAllocation {
                candidates,
                required,
                ..
            } => {
                if candidates.len() != 2
                    || *required != 3.into()
                    || candidates.iter().any(|c| c.amount != 3.into())
                {
                    return Err("split policy expected two current three-damage sources".into());
                }
                let amounts = if self.policy == Policy::SalveSplitFirst {
                    [1, 2]
                } else {
                    [2, 1]
                };
                PlayerResponse::DamageAllocation {
                    allocations: candidates
                        .iter()
                        .zip(amounts)
                        .map(|(c, n)| (c.damage.clone(), Quantity::from(n)))
                        .collect(),
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let available = &v
                    .players
                    .iter()
                    .find(|p| p.player == v.viewer)
                    .ok_or("missing choices player")?
                    .mana;
                if selected(cost, available).is_ok() {
                    PlayerResponse::FinishMana { undo_mana: true }
                } else {
                    let mut needed = BTreeMap::<Color, Quantity>::new();
                    for color in &cost.colored {
                        needed.entry(*color).or_default().add(&1.into());
                    }
                    let deficient = needed
                        .iter()
                        .find(|(c, n)| available.get(c).cloned().unwrap_or_default() < **n)
                        .map(|(c, _)| *c);
                    let landcolor = |id: &str| match id {
                        FOREST => Some(Color::G),
                        PLAINS => Some(Color::W),
                        SWAMP => Some(Color::B),
                        MOUNTAIN => Some(Color::R),
                        ISLAND => Some(Color::U),
                        _ => None,
                    };
                    let source = v
                        .objects
                        .iter()
                        .find(|x| {
                            x.zone == Zone::Battlefield
                                && x.controller == v.viewer
                                && !x.tapped
                                && landcolor(&x.oracle_id)
                                    .is_some_and(|c| deficient.is_none_or(|n| n == c))
                        })
                        .ok_or("choices policy cannot fund cost")?;
                    PlayerResponse::ActivateMana {
                        object: source.handle.clone(),
                    }
                }
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: selected(cost, available)?,
            },
            PlayerRequest::CommanderReturn { .. } => {
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Attackers {
                candidates,
                defenders,
                ..
            } => {
                let mut attacks = vec![];
                if let Some(action) = self.actions.get(self.cursor).copied()
                    && action.player == v.viewer.0
                    && let Op::Attack(id, defender) = action.op
                {
                    let attacker = v
                        .objects
                        .iter()
                        .find(|x| {
                            x.oracle_id == id
                                && x.controller == v.viewer
                                && candidates.contains(&x.handle)
                        })
                        .ok_or("missing choices attacker")?;
                    if !defenders.contains(&PlayerId(defender)) {
                        return Err("missing choices defender".into());
                    }
                    attacks.push((attacker.handle.clone(), PlayerId(defender)));
                    self.cursor += 1;
                }
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => {
                let mut blocks = vec![];
                if let Some(action) = self.actions.get(self.cursor).copied()
                    && action.player == v.viewer.0
                    && let Op::Block(id, count) = action.op
                {
                    for source in v
                        .objects
                        .iter()
                        .filter(|x| {
                            x.oracle_id == id
                                && x.controller == v.viewer
                                && candidates.contains(&x.handle)
                        })
                        .take(count)
                    {
                        blocks.push((
                            source.handle.clone(),
                            attackers
                                .first()
                                .ok_or("missing choices blocked attacker")?
                                .clone(),
                        ));
                    }
                    if blocks.len() != count {
                        return Err("missing simultaneous shield blockers".into());
                    }
                    self.cursor += 1;
                }
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage {
                power, recipients, ..
            } => {
                let mut remaining = power.clone();
                let mut assignments = vec![];
                for (n, target) in recipients.iter().enumerate() {
                    let amount = if n + 1 == recipients.len() {
                        remaining.clone()
                    } else {
                        std::cmp::min(remaining.clone(), 1.into())
                    };
                    remaining.subtract(&amount);
                    assignments.push((target.clone(), amount));
                }
                PlayerResponse::Damage { assignments }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            _ => return Err("unexpected choices scenario decision".into()),
        })
    }
}
fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("choices query rejected: {reason:?}"));
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
        _ => Err("wrong choices observation result".into()),
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
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing choices snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("choices boundary recovery differs".into());
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
        return Err("choices runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let (actions, end) = plan(policy);
    let mut strategy = Strategy {
        policy,
        actions,
        cursor: 0,
        current: None,
        end,
        departed: false,
        choices: vec![],
    };
    let mut checkpoints = vec![];
    let mut verifier =
        (recovery == Recovery::Verify).then(|| SnapshotVerifier::new(prepared, mode));
    if recovery != Recovery::None {
        capture(&game, &mut checkpoints, verifier.as_mut())?;
    }
    for _ in 0..16384 {
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
                    strategy.choices.push(o.clone());
                    return Ok(Run {
                        game,
                        checkpoints,
                        choices: strategy.choices,
                    });
                }
                let response = strategy.choose(&o).map_err(|error| {
                    format!("choices {policy:?} action{}: {error}", strategy.cursor)
                })?;
                let token = o
                    .decision
                    .as_ref()
                    .ok_or("missing choices token")?
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
                    _ => return Err("wrong choices validation result".into()),
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "choices witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!("choices witness {policy:?} exhausted host budget"))
}

fn zone(g: &Game, id: u32) -> Option<Zone> {
    g.object_id(CardId(id)).and_then(|x| g.zone(x))
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    use Policy::*;
    let g = &run.game;
    let view = observation(g, PlayerId(3))?.view;
    let expected = match policy {
        SalveGainSelf => [43, 40, 40, 40],
        SalveGainOpponent => [40, 43, 40, 40],
        SalveResidualBudget => [40, 38, 40, 40],
        SalveDoubleFirst => [40, 37, 40, 40],
        BouncerEmpty | BouncerCast => [40, 36, 42, 42],
        BouncerDrawCastTrue => [43, 36, 42, 42],
        BouncerNonempty => [43, 38, 41, 41],
        BouncerDrawFalse | BouncerStifled | BouncerTargetGone | BouncerControllerLeaves => {
            [40, 38, 41, 41]
        }
        BouncerBorrowedOwnerLeaves => [40, 38, 40, 42],
        _ => [40, 40, 40, 40],
    };
    let lives = view
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    if lives != expected.map(Quantity::from) {
        return Err(format!("choices {policy:?} life differs: {lives:?}"));
    }
    let created = g
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Committed {
                actual: EventOutcome::EffectCreated { effect },
                ..
            } => Some(effect.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let creations = match policy {
        SeedsSame | SeedsDistinct | SeedsCasterLeaves | SeedsCleanup => 3,
        SeedsOneIllegal => 2,
        SeedsTwoIllegal => 1,
        SalveShieldPlayer | SalveShieldCreature | SalveResidualBudget | SalveDoubleFirst
        | SalvePreventFirst => 1,
        SalveSplitFirst | SalveSplitLast => 4,
        BouncerEmpty | BouncerDrawCastTrue | BouncerBorrowedOwnerLeaves | BouncerCast => 1,
        _ => 0,
    };
    if g.trace().mode == TraceMode::Detailed && created.len() != creations {
        return Err(format!(
            "choices {policy:?} effect creation count {} expected {creations}",
            created.len()
        ));
    }
    match policy {
        SeedsAllIllegal | SalveFizzle => {
            if zone(g, 1) != Some(Zone::Hand { owner: PlayerId(0) }) {
                return Err("bounced sole target did not stay in hand".into());
            }
        }
        SeedsOneIllegal | SeedsTwoIllegal | BouncerTargetGone => {
            if zone(g, 16) != Some(Zone::Hand { owner: PlayerId(1) }) {
                return Err("bounced creature target did not stay in hand".into());
            }
        }
        SeedsReincarnation => {
            if zone(g, 1) != Some(Zone::Battlefield)
                || g.object_id(CardId(1)).is_none_or(|o| o.incarnation < 2)
            {
                return Err("target did not return as a fresh incarnation".into());
            }
        }
        BouncerEmpty | BouncerDrawCastTrue | BouncerBorrowedOwnerLeaves | BouncerCast => {
            if zone(g, 16) != Some(Zone::Graveyard { owner: PlayerId(1) }) {
                return Err("intervening-if modifier did not kill its legal target".into());
            }
        }
        BouncerNonempty | BouncerDrawFalse | BouncerStifled | BouncerControllerLeaves
            if zone(g, 16) != Some(Zone::Battlefield) =>
        {
            return Err("failed/countered/removed trigger affected its target".into());
        }
        _ => {}
    }
    let stats = |card| -> Result<_, String> {
        let c = g.characteristics(
            g.object_id(CardId(card))
                .ok_or("missing characteristic object")?,
        )?;
        Ok((c.power, c.toughness))
    };
    let wanted: Vec<(u32, i64, i64)> = match policy {
        SeedsSame => vec![(1, 3, 7)],
        SeedsCasterLeaves => vec![(16, 5, 5)],
        SeedsDistinct | SeedsOneIllegal => vec![(1, 1, 5), (0, 3, 3)],
        SeedsTwoIllegal => vec![(1, 1, 5)],
        SeedsCountered | SeedsReincarnation | SeedsCleanup => vec![(1, 0, 4)],
        SalveSplitFirst | SalveSplitLast => vec![(1, 1, 5), (16, 3, 3), (17, 3, 3)],
        BouncerNonempty | BouncerDrawFalse | BouncerStifled | BouncerControllerLeaves => {
            vec![(16, 2, 2)]
        }
        _ => vec![],
    };
    for (id, p, t) in wanted {
        if stats(id)? != (Some(p.into()), Some(t.into())) {
            return Err(format!("choices {policy:?} derived stats differ for{id}"));
        }
    }
    if matches!(
        policy,
        SalveShieldCreature | SalveSplitFirst | SalveSplitLast
    ) {
        let snap: serde_json::Value =
            serde_json::from_str(&g.snapshot()?).map_err(|e| e.to_string())?;
        let marks: Quantity =
            serde_json::from_value(snap["state"]["objects"]["1"]["marked_damage"].clone())
                .map_err(|e| e.to_string())?;
        let expected = if policy == SalveShieldCreature { 0 } else { 3 };
        if marks != expected.into() {
            return Err("shared shield/source allocation marked wrong damage".into());
        }
    }
    if g.trace().mode == TraceMode::Detailed
        && matches!(
            policy,
            SeedsSame
                | SeedsDistinct
                | SeedsOneIllegal
                | SeedsTwoIllegal
                | SeedsCasterLeaves
                | SeedsCleanup
        )
    {
        let cursors = created
            .iter()
            .map(|e| (e.origin.cursor, e.origin.source_block))
            .collect::<Vec<_>>();
        let expected = match policy {
            SeedsOneIllegal => vec![(0, 0), (2, 2)],
            SeedsTwoIllegal => vec![(0, 0)],
            _ => vec![(0, 0), (1, 1), (2, 2)],
        };
        if cursors != expected {
            return Err(
                "separate source blocks/target occurrences were collapsed or reordered".into(),
            );
        }
    }
    if g.trace().mode == TraceMode::Detailed {
        let checks = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::TriggerConditionChecked {
                    source,
                    controller,
                    stage,
                    check,
                    ..
                } if source.card == CardId(2) => Some((*controller, stage, check)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected_checks: Vec<(bool, u32, bool)> = match policy {
            BouncerEmpty
            | BouncerCast
            | BouncerBorrowedOwnerLeaves
            | BouncerTargetGone
            | BouncerDrawCastTrue => vec![(true, 0, true), (false, 0, true)],
            BouncerDrawFalse => vec![(true, 0, true), (false, 1, false)],
            BouncerNonempty => vec![(true, 1, false)],
            BouncerStifled | BouncerControllerLeaves | BouncerNoTargets => vec![(true, 0, true)],
            _ => vec![],
        };
        if checks.len() != expected_checks.len() {
            return Err(format!(
                "choices {policy:?} condition checks {} expected {}",
                checks.len(),
                expected_checks.len()
            ));
        }
        for ((controller, stage, check), (first, hand, passed)) in
            checks.iter().zip(expected_checks)
        {
            if *controller != PlayerId(0)
                || !matches!(check, mtg_core::InterveningCheck::ControllerHandEmpty { controller: PlayerId(0), hand_count, passed: p } if *hand_count == hand && *p == passed)
                || matches!(stage, mtg_core::InterveningStage::Match { .. }) != first
            {
                return Err(
                    "intervening condition checked wrong player/hand/time or extra placement gate"
                        .into(),
                );
            }
        }
        let modes = g
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::ModeSelected { mode, .. } => Some(*mode),
                _ => None,
            })
            .collect::<Vec<_>>();
        let selected_mode = match policy {
            SalveGainSelf | SalveGainOpponent | SalveCountered | BouncerNonempty
            | BouncerDrawCastTrue => Some(0),
            SalveShieldPlayer | SalveShieldCreature | SalveResidualBudget | SalveDoubleFirst
            | SalvePreventFirst | SalveFizzle | SalveSplitFirst | SalveSplitLast => Some(1),
            _ => None,
        };
        if modes != selected_mode.into_iter().collect::<Vec<_>>() {
            return Err("mode was omitted, changed or unselected program executed".into());
        }
        if matches!(policy, SalveSplitFirst | SalveSplitLast) {
            let allocations = g
                .events()
                .iter()
                .filter_map(|event| match event {
                    Event::Committed {
                        actual: EventOutcome::DamageEffectAllocated { applications, .. },
                        ..
                    } => Some(applications),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = if policy == SalveSplitFirst {
                [1, 2]
            } else {
                [2, 1]
            };
            if allocations.len() != 1
                || allocations[0].len() != 2
                || allocations[0].iter().zip(expected).any(|(a, n)| {
                    a.before != 3.into()
                        || a.prevented != n.into()
                        || a.after != Quantity::from(3 - n)
                })
            {
                return Err(
                    "full shared-budget allocation did not preserve legal nongreedy split".into(),
                );
            }
        }
    }
    let ledger = g.commander_ledger();
    if let Some(history) = ledger.history(CardId(0))
        && !history.combat_damage.is_empty()
    {
        return Err("fully prevented/ordinary damage advanced Commander history".into());
    }
    Ok(())
}
fn choices_proofs(g: &Game) -> Vec<&mtg_core::OccurrenceResult> {
    g.trace()
        .records
        .iter()
        .filter_map(|record| match &record.record {
            EvidenceRecord::Batch { batch } => Some(batch.occurrences.iter()),
            _ => None,
        })
        .flatten()
        .filter(|occurrence| {
            matches!(
                occurrence.actual,
                EventOutcome::EffectCreated { .. }
                    | EventOutcome::DamageEffectAllocated { .. }
                    | EventOutcome::DamageEffectApplied { .. }
                    | EventOutcome::DamageCommitted { .. }
                    | EventOutcome::EffectsCleaned { .. }
            )
        })
        .collect()
}
pub(super) fn suite(
    catalog: &Catalog,
    recover: bool,
) -> Result<(Vec<Evidence>, Vec<Game>, serde_json::Value), String> {
    let (mut evidence, mut games, mut reports) = (vec![], vec![], vec![]);
    for policy in POLICIES {
        eprintln!("running choices {policy:?}");
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
                return Err("choices trace/budget partition differs".into());
            }
        }
        let g = reference.game;
        reports.push(serde_json::json!({"policy":policy,"state_sha256":g.state_digest()?,"turn":g.turn(),"journal":g.replay_journal(),"events":g.events(),"choices_proofs":choices_proofs(&g),"checkpoints":reference.checkpoints.len(),"choices":reference.choices,"final_observations":(0..4).map(|p|observation(&g,PlayerId(p))).collect::<Result<Vec<_>,_>>()?,"commander_ledger":g.commander_ledger()}));
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
        return Err("missing choices witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("choices policy/config mismatch".into());
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
            return Err("choices root recovery differs".into());
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
            return Err("choices retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_choices_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::catalog::embedded_catalog("choices").unwrap();
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
