//! Effects policies consume only the acting player's public observation.
//! Trusted departure fixtures, physical assertions and recovery stay in the host.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{
    DamageEffectKind, DamageRecipient, EffectKey, Event, EventOutcome, EvidenceRecord,
    ObjectHandle, PlayerRequest, PlayerResponse, PlayerStatus, PlayerTargetEntity,
};
use std::collections::BTreeMap;
const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const OX: &str = "19b4cf0b-e1e4-4e50-80bc-673bc6ef8bb8";
const HEALER: &str = "95a0ca48-d924-47f4-86ed-42c673ee778c";
const PINGER: &str = "5e961d15-5972-4e4b-9385-1cd7cd7c6bbe";
const BOLT: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const GROWTH: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";
const DISFIGURE: &str = "77eafe49-b9c5-461d-89c5-cec217dd2974";
const FOG: &str = "27e9db49-7af7-4bef-ad4c-bf5dfb92030d";
const FURNACE: &str = "68715465-6cf9-4006-87e9-31f227fe9ed3";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const MOUNTAIN: &str = "a3fb7228-e76b-4e96-a40e-20b5fed75685";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    Growth,
    GrowthCountered,
    AdditiveModifiers,
    MultiCasterModifiers,
    ZeroToughnessReturn,
    ZeroToughnessDecline,
    NegativePower,
    GrowthReincarnation,
    GrowthCasterLeaves,
    GrowthCleanup,
    GrowthActiveLeavesCleanup,
    PumpedPinger,
    PumpedPingerBounced,
    PumpedPingerBorrowedOwnerLeaves,
    ShieldPlayer,
    ShieldCreature,
    ShieldNextPing,
    ShieldStifled,
    ShieldSourceBounced,
    ShieldBorrowedOwnerLeaves,
    ShieldTargetReincarnation,
    ShieldFizzle,
    FurnaceCast,
    FurnaceBolt,
    TwoFurnacesBolt,
    FurnaceSourceLeaves,
    DoubleThenPrevent,
    PreventThenDouble,
    FogCombat,
    FogNoncombat,
    FogThenShield,
    ShieldThenFog,
    ShieldSimultaneousFirst,
    ShieldSimultaneousLast,
    PreventedDeath,
    AmplifiedDeath,
    CommanderAmplified,
    FogCasterLeaves,
    AcceptedSourceLeaves,
    AppliedPrefixSourceLeaves,
    BoltEliminatesPlayer,
}
pub(super) const POLICIES: [Policy; 41] = [
    Policy::Growth,
    Policy::GrowthCountered,
    Policy::AdditiveModifiers,
    Policy::MultiCasterModifiers,
    Policy::ZeroToughnessReturn,
    Policy::ZeroToughnessDecline,
    Policy::NegativePower,
    Policy::GrowthReincarnation,
    Policy::GrowthCasterLeaves,
    Policy::GrowthCleanup,
    Policy::GrowthActiveLeavesCleanup,
    Policy::PumpedPinger,
    Policy::PumpedPingerBounced,
    Policy::PumpedPingerBorrowedOwnerLeaves,
    Policy::ShieldPlayer,
    Policy::ShieldCreature,
    Policy::ShieldNextPing,
    Policy::ShieldStifled,
    Policy::ShieldSourceBounced,
    Policy::ShieldBorrowedOwnerLeaves,
    Policy::ShieldTargetReincarnation,
    Policy::ShieldFizzle,
    Policy::FurnaceCast,
    Policy::FurnaceBolt,
    Policy::TwoFurnacesBolt,
    Policy::FurnaceSourceLeaves,
    Policy::DoubleThenPrevent,
    Policy::PreventThenDouble,
    Policy::FogCombat,
    Policy::FogNoncombat,
    Policy::FogThenShield,
    Policy::ShieldThenFog,
    Policy::ShieldSimultaneousFirst,
    Policy::ShieldSimultaneousLast,
    Policy::PreventedDeath,
    Policy::AmplifiedDeath,
    Policy::CommanderAmplified,
    Policy::FogCasterLeaves,
    Policy::AcceptedSourceLeaves,
    Policy::AppliedPrefixSourceLeaves,
    Policy::BoltEliminatesPlayer,
];
impl Policy {
    fn config(self) -> ScenarioConfig {
        match self {
            Self::TwoFurnacesBolt | Self::AppliedPrefixSourceLeaves => {
                ScenarioConfig::effects_two_furnaces()
            }
            Self::PumpedPingerBorrowedOwnerLeaves | Self::ShieldBorrowedOwnerLeaves => {
                ScenarioConfig::effects_borrowed()
            }
            Self::BoltEliminatesPlayer => ScenarioConfig::effects_low(),
            Self::FurnaceBolt
            | Self::FurnaceSourceLeaves
            | Self::DoubleThenPrevent
            | Self::PreventThenDouble
            | Self::AmplifiedDeath
            | Self::CommanderAmplified
            | Self::AcceptedSourceLeaves => ScenarioConfig::effects_furnace(),
            _ => ScenarioConfig::effects(),
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
    Activate(&'static str, Aim),
    Attack(&'static str, u8),
    Block(&'static str, usize),
}
#[derive(Clone, Copy)]
enum When {
    Empty,
    Stack(&'static str),
    Step(TurnStep),
    Turn(u64, TurnStep),
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
fn a(player: u8, id: &'static str, target: Aim) -> Action {
    Action {
        player,
        op: Op::Activate(id, target),
        when: When::Empty,
    }
}
fn attack(id: &'static str, player: u8, defender: u8) -> Action {
    Action {
        player,
        op: Op::Attack(id, defender),
        when: When::Empty,
    }
}
fn block(id: &'static str, player: u8, count: usize) -> Action {
    Action {
        player,
        op: Op::Block(id, count),
        when: When::Empty,
    }
}
fn response(mut action: Action, id: &'static str) -> Action {
    action.when = When::Stack(id);
    action
}
fn phase(mut action: Action, step: TurnStep) -> Action {
    action.when = When::Step(step);
    action
}
fn turn(mut action: Action, n: u64, step: TurnStep) -> Action {
    action.when = When::Turn(n, step);
    action
}
fn plan(policy: Policy) -> (Vec<Action>, End) {
    use Policy::*;
    let ox = obj(OX, 0);
    let commander = obj(COMMANDER, 0);
    let healer = obj(HEALER, 0);
    let growth = |target| c(0, GROWTH, Some(target));
    let bolt = |player, target| c(player, BOLT, Some(target));
    let shield = |target| a(0, HEALER, target);
    let pinger_owner = if policy == PumpedPingerBorrowedOwnerLeaves {
        2
    } else {
        0
    };
    let (actions, end) = match policy {
        Growth => (vec![growth(ox)], End::Empty),
        GrowthCountered => (
            vec![
                growth(ox),
                response(c(1, COUNTER, Some(Aim::Stack(GROWTH))), GROWTH),
            ],
            End::Empty,
        ),
        AdditiveModifiers => (vec![growth(ox), c(0, DISFIGURE, Some(ox))], End::Empty),
        MultiCasterModifiers => (
            vec![
                growth(ox),
                c(1, GROWTH, Some(ox)),
                c(0, DISFIGURE, Some(ox)),
            ],
            End::Empty,
        ),
        ZeroToughnessReturn | ZeroToughnessDecline => {
            (vec![c(0, DISFIGURE, Some(commander))], End::Empty)
        }
        NegativePower => (
            vec![c(0, DISFIGURE, Some(ox)), attack(OX, 0, 1)],
            End::Combat,
        ),
        GrowthReincarnation => (
            vec![growth(ox), c(1, UNSUMMON, Some(ox)), c(0, OX, None)],
            End::Empty,
        ),
        GrowthCasterLeaves => (vec![growth(obj(BEARS, 1))], End::Empty),
        GrowthCleanup => (vec![growth(healer), bolt(1, healer)], End::NextTurn),
        GrowthActiveLeavesCleanup => (
            vec![growth(obj(BEARS, 1)), bolt(1, obj(BEARS, 1))],
            End::NextTurn,
        ),
        PumpedPinger | PumpedPingerBorrowedOwnerLeaves => (
            vec![
                growth(obj(PINGER, pinger_owner)),
                a(0, PINGER, Aim::Player(1)),
            ],
            End::Empty,
        ),
        PumpedPingerBounced => (
            vec![
                growth(obj(PINGER, 0)),
                a(0, PINGER, Aim::Player(1)),
                response(c(1, UNSUMMON, Some(obj(PINGER, 0))), PINGER),
            ],
            End::Empty,
        ),
        ShieldPlayer => (
            vec![shield(Aim::Player(1)), bolt(0, Aim::Player(1))],
            End::Empty,
        ),
        ShieldCreature => (vec![shield(ox), bolt(1, ox)], End::Empty),
        ShieldNextPing => (
            vec![
                shield(Aim::Player(1)),
                a(0, PINGER, Aim::Player(1)),
                bolt(1, Aim::Player(1)),
            ],
            End::Empty,
        ),
        ShieldStifled => (
            vec![
                shield(Aim::Player(1)),
                response(c(1, STIFLE, Some(Aim::Stack(HEALER))), HEALER),
                bolt(0, Aim::Player(1)),
            ],
            End::Empty,
        ),
        ShieldSourceBounced => (
            vec![
                shield(Aim::Player(1)),
                response(c(1, UNSUMMON, Some(healer)), HEALER),
                bolt(0, Aim::Player(1)),
            ],
            End::Empty,
        ),
        ShieldBorrowedOwnerLeaves => (
            vec![shield(Aim::Player(1)), bolt(0, Aim::Player(1))],
            End::Empty,
        ),
        ShieldTargetReincarnation => (
            vec![
                shield(ox),
                c(1, UNSUMMON, Some(ox)),
                c(0, OX, None),
                bolt(0, ox),
            ],
            End::Empty,
        ),
        ShieldFizzle => (
            vec![
                shield(ox),
                response(c(1, UNSUMMON, Some(ox)), HEALER),
                c(0, OX, None),
                bolt(0, ox),
            ],
            End::Empty,
        ),
        FurnaceCast => (
            vec![turn(c(2, FURNACE, None), 3, TurnStep::PrecombatMain)],
            End::Empty,
        ),
        FurnaceBolt | TwoFurnacesBolt | FurnaceSourceLeaves | BoltEliminatesPlayer => {
            (vec![bolt(0, Aim::Player(1))], End::Empty)
        }
        DoubleThenPrevent
        | PreventThenDouble
        | AcceptedSourceLeaves
        | AppliedPrefixSourceLeaves => (
            vec![shield(Aim::Player(1)), bolt(0, Aim::Player(1))],
            End::Empty,
        ),
        FogCombat => (vec![c(0, FOG, None), attack(COMMANDER, 0, 1)], End::Combat),
        FogNoncombat => (vec![c(0, FOG, None), bolt(0, Aim::Player(1))], End::Empty),
        FogThenShield | ShieldThenFog => (
            vec![
                shield(Aim::Player(1)),
                c(0, FOG, None),
                attack(COMMANDER, 0, 1),
                phase(a(0, PINGER, Aim::Player(1)), TurnStep::EndCombat),
            ],
            End::Combat,
        ),
        ShieldSimultaneousFirst | ShieldSimultaneousLast => (
            vec![
                shield(commander),
                attack(COMMANDER, 0, 1),
                block(BEARS, 1, 2),
            ],
            End::Combat,
        ),
        PreventedDeath => (vec![shield(healer), a(0, PINGER, healer)], End::Empty),
        AmplifiedDeath => (vec![a(0, PINGER, obj(BEARS, 1))], End::Empty),
        CommanderAmplified => (vec![attack(COMMANDER, 0, 1)], End::Combat),
        FogCasterLeaves => (
            vec![
                attack(BEARS, 1, 2),
                phase(c(0, FOG, None), TurnStep::DeclareAttackers),
            ],
            End::Combat,
        ),
    };
    (actions, end)
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
        if !self.departed
            && self.policy == Policy::AppliedPrefixSourceLeaves
            && matches!(&o.decision,Some(PlayerRequest::DamageEffect {candidates,..}) if candidates.iter().any(|c|c.amount_before==Quantity::from(6)))
        {
            self.departed = true;
            self.choices.push(o.clone());
            return Some(PlayerId(2));
        }
        if self.departed || !matches!(o.decision, Some(PlayerRequest::Priority { .. })) {
            return None;
        }
        let stack_has = |id| {
            o.view
                .stack
                .iter()
                .any(|s| oracle(&o.view, s).is_ok_and(|x| x == id))
        };
        let player = match self.policy {
            Policy::GrowthCasterLeaves if self.cursor == 1 && o.view.stack.is_empty() => {
                Some(PlayerId(0))
            }
            Policy::GrowthActiveLeavesCleanup if self.cursor == 2 && o.view.stack.is_empty() => {
                Some(PlayerId(0))
            }
            Policy::FogCasterLeaves if self.cursor == 2 && o.view.stack.is_empty() => {
                Some(PlayerId(0))
            }
            Policy::PumpedPingerBorrowedOwnerLeaves if self.cursor >= 2 && stack_has(PINGER) => {
                Some(PlayerId(2))
            }
            Policy::ShieldBorrowedOwnerLeaves if self.cursor >= 1 && stack_has(HEALER) => {
                Some(PlayerId(2))
            }
            Policy::FurnaceSourceLeaves if self.cursor >= 1 && stack_has(BOLT) => Some(PlayerId(2)),
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
            Some(Op::Cast(_, Some(target)) | Op::Activate(_, target)) => Ok(target),
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
                .ok_or("missing effects spell")?,
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
                    .ok_or("missing effects activated source")?
                    .handle
                    .clone(),
                ability: 0,
            }),
            _ => Err("combat action at priority".into()),
        }
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let request = o.decision.as_ref().ok_or("missing effects request")?;
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
                        When::Turn(n, step) => {
                            v.stack.is_empty() && v.turn.number == n && v.turn.step == step
                        }
                    };
                    if ready
                        && v.viewer == PlayerId(action.player)
                        && matches!(action.op, Op::Cast(..) | Op::Activate(..))
                    {
                        return self.execute(o, action);
                    }
                }
                PlayerResponse::Pass
            }
            PlayerRequest::Targets { candidates, .. } => {
                let object = aim_handle(v, self.target()?)?;
                if !candidates.contains(&object) {
                    return Err("effects creature target ineligible".into());
                }
                PlayerResponse::Targets {
                    objects: vec![object],
                }
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
                    return Err("effects entity target ineligible".into());
                }
                PlayerResponse::EntityTargets {
                    entities: vec![target],
                }
            }
            PlayerRequest::StackTargets { candidates, .. } => {
                let Aim::Stack(id) = self.target()? else {
                    return Err("missing effects stack aim".into());
                };
                let entry = v
                    .stack
                    .iter()
                    .find(|s| oracle(v, s).is_ok_and(|x| x == id))
                    .ok_or("missing effect counter target")?;
                let target = entry.handle.clone().ok_or("missing effects stack handle")?;
                if !candidates.contains(&target) {
                    return Err("effects stack target ineligible".into());
                }
                PlayerResponse::StackTargets {
                    entries: vec![target],
                }
            }
            PlayerRequest::PlayerTargets { candidates, .. } => {
                let player = if candidates.contains(&PlayerId(3)) {
                    PlayerId(3)
                } else {
                    *candidates.first().ok_or("no effects player target")?
                };
                PlayerResponse::PlayerTargets {
                    players: vec![player],
                }
            }
            PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
                order: candidates.iter().map(|c| c.handle.clone()).collect(),
            },
            PlayerRequest::DamageEffect { candidates, .. } => {
                let kind = match self.policy {
                    Policy::DoubleThenPrevent
                    | Policy::AcceptedSourceLeaves
                    | Policy::AppliedPrefixSourceLeaves => Some(DamageEffectKind::DoubleDamage),
                    Policy::PreventThenDouble | Policy::ShieldThenFog => {
                        Some(DamageEffectKind::PreventNextDamage)
                    }
                    Policy::FogThenShield => Some(DamageEffectKind::PreventCombatDamage),
                    _ => None,
                };
                let chosen = if self.policy == Policy::ShieldSimultaneousLast {
                    candidates.last()
                } else {
                    candidates
                        .iter()
                        .find(|c| kind.is_some_and(|k| c.kind == k))
                        .or_else(|| candidates.first())
                }
                .ok_or("empty effects pair choice")?;
                PlayerResponse::DamageEffect {
                    effect: chosen.effect.clone(),
                    damage: chosen.damage.clone(),
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let available = &v
                    .players
                    .iter()
                    .find(|p| p.player == v.viewer)
                    .ok_or("missing effects player")?
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
                        .ok_or("effects policy cannot fund cost")?;
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
            PlayerRequest::CommanderReturn { .. } => PlayerResponse::CommanderReturn {
                to_command: self.policy != Policy::ZeroToughnessDecline,
            },
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
                        .ok_or("missing effects attacker")?;
                    if !defenders.contains(&PlayerId(defender)) {
                        return Err("missing effects defender".into());
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
                                .ok_or("missing effects blocked attacker")?
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
            _ => return Err("unexpected effects scenario decision".into()),
        })
    }
}
fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("effects query rejected: {reason:?}"));
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
        _ => Err("wrong effects observation result".into()),
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
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing effects snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("effects boundary recovery differs".into());
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
        return Err("effects runner requires positive budget".into());
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
                let response = strategy.choose(&o)?;
                let token = o
                    .decision
                    .as_ref()
                    .ok_or("missing effects token")?
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
                    _ => return Err("wrong effects validation result".into()),
                }
                if policy == Policy::AcceptedSourceLeaves
                    && !strategy.departed
                    && matches!(o.decision, Some(PlayerRequest::DamageEffect { .. }))
                {
                    strategy.departed = true;
                    game.concede(PlayerId(2))?;
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "effects witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!("effects witness {policy:?} exhausted host budget"))
}

fn zone(g: &Game, id: u32) -> Option<Zone> {
    g.object_id(CardId(id)).and_then(|x| g.zone(x))
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let g = &run.game;
    let view = observation(g, PlayerId(3))?.view;
    let mut expected = [40_i64; 4];
    use Policy::*;
    match policy {
        PumpedPinger | PumpedPingerBounced | PumpedPingerBorrowedOwnerLeaves | ShieldThenFog => {
            expected[1] = 39
        }
        ShieldPlayer | ShieldSourceBounced | ShieldBorrowedOwnerLeaves | AcceptedSourceLeaves => {
            expected[1] = 38
        }
        ShieldNextPing | ShieldStifled | FurnaceSourceLeaves | FogNoncombat => expected[1] = 37,
        FurnaceBolt => expected[1] = 34,
        TwoFurnacesBolt => expected[1] = 28,
        DoubleThenPrevent | AppliedPrefixSourceLeaves => expected[1] = 35,
        PreventThenDouble | CommanderAmplified => expected[1] = 36,
        ZeroToughnessReturn
        | ZeroToughnessDecline
        | ShieldSimultaneousFirst
        | ShieldSimultaneousLast
        | AmplifiedDeath => expected = [41, 41, 41, 37],
        BoltEliminatesPlayer => expected = [4, -2, 4, 4],
        _ => {}
    }
    let lives = view
        .players
        .iter()
        .map(|p| p.life.clone())
        .collect::<Vec<_>>();
    if lives != expected.map(Quantity::from) {
        return Err(format!("effects {policy:?} life differs: {lives:?}"));
    }
    let physical = match policy {
        ZeroToughnessReturn | ShieldSimultaneousFirst | ShieldSimultaneousLast => {
            zone(g, 0) == Some(Zone::Command { owner: PlayerId(0) })
        }
        ZeroToughnessDecline => zone(g, 0) == Some(Zone::Graveyard { owner: PlayerId(0) }),
        PumpedPingerBounced => zone(g, 3) == Some(Zone::Hand { owner: PlayerId(0) }),
        PumpedPingerBorrowedOwnerLeaves => {
            zone(g, 3).is_none() && view.players[2].status == PlayerStatus::Left
        }
        ShieldBorrowedOwnerLeaves => {
            zone(g, 2).is_none() && view.players[2].status == PlayerStatus::Left
        }
        ShieldSourceBounced => zone(g, 2) == Some(Zone::Hand { owner: PlayerId(0) }),
        GrowthCasterLeaves | GrowthActiveLeavesCleanup | FogCasterLeaves => {
            view.players[0].status == PlayerStatus::Left
        }
        FurnaceSourceLeaves | AcceptedSourceLeaves | AppliedPrefixSourceLeaves => {
            zone(g, 32).is_none() && view.players[2].status == PlayerStatus::Left
        }
        FurnaceCast => zone(g, 31) == Some(Zone::Battlefield),
        AmplifiedDeath => zone(g, 16) == Some(Zone::Graveyard { owner: PlayerId(1) }),
        BoltEliminatesPlayer => view.players[1].status == PlayerStatus::Left,
        _ => true,
    };
    if !physical {
        return Err(format!("effects {policy:?} physical outcome differs"));
    }
    let stats = |id| -> Result<(Option<Quantity>, Option<Quantity>), String> {
        let c = g.characteristics(g.object_id(CardId(id)).ok_or("missing stat object")?)?;
        Ok((c.power, c.toughness))
    };
    let wanted = match policy {
        Growth => Some((1, 3, 7)),
        AdditiveModifiers => Some((1, 1, 5)),
        MultiCasterModifiers => Some((1, 4, 8)),
        NegativePower => Some((1, -2, 2)),
        GrowthReincarnation | GrowthCountered => Some((1, 0, 4)),
        GrowthCleanup => Some((2, 1, 1)),
        GrowthActiveLeavesCleanup => Some((16, 2, 2)),
        GrowthCasterLeaves => Some((16, 5, 5)),
        _ => None,
    };
    if let Some((id, p, t)) = wanted
        && stats(id)? != (Some(p.into()), Some(t.into()))
    {
        return Err(format!("effects {policy:?} derived stats differ"));
    }
    let marked = match policy {
        ShieldCreature => Some((1, 2)),
        ShieldTargetReincarnation | ShieldFizzle => Some((1, 3)),
        GrowthCleanup => Some((2, 0)),
        GrowthActiveLeavesCleanup => Some((16, 0)),
        PreventedDeath => Some((2, 0)),
        _ => None,
    };
    if let Some((id, n)) = marked {
        let snap: serde_json::Value =
            serde_json::from_str(&g.snapshot()?).map_err(|e| e.to_string())?;
        let actual: Quantity = serde_json::from_value(
            snap["state"]["objects"][id.to_string()]["marked_damage"].clone(),
        )
        .map_err(|e| e.to_string())?;
        if actual != Quantity::from(n) {
            return Err(format!(
                "effects {policy:?} exact target marked damage differs: {actual}"
            ));
        }
    }
    if g.trace().mode == TraceMode::Detailed {
        let mut created = 0;
        let mut collected = 0;
        let mut losses = 0;
        let mut gains = 0;
        let mut counters = 0;
        let mut death_reasons = vec![];
        let mut applications = vec![];
        let mut actual_damage = vec![];
        for event in g.events() {
            match event {
                Event::TriggerCollected { .. } => collected += 1,
                Event::Committed { actual, .. } => match actual {
                    EventOutcome::EffectCreated { .. } => created += 1,
                    EventOutcome::DamageEffectApplied { application } => {
                        applications.push(application.as_ref())
                    }
                    EventOutcome::DamageCommitted { actual, .. } => {
                        actual_damage.extend(actual.iter())
                    }
                    EventOutcome::LifeLost { amount, .. } => {
                        if *amount != 1.into() {
                            return Err("damage modifier changed direct Artist life loss".into());
                        }
                        losses += 1;
                    }
                    EventOutcome::LifeGained { amount, .. } => {
                        if *amount != 1.into() {
                            return Err("damage modifier changed direct Artist gain".into());
                        }
                        gains += 1;
                    }
                    EventOutcome::StackObjectCountered { .. } => counters += 1,
                    EventOutcome::StateBasedMoved { reason, .. } => death_reasons.push(*reason),
                    _ => {}
                },
                _ => {}
            }
        }
        let expected_created = match policy {
            GrowthCountered | ShieldStifled | ShieldFizzle | FurnaceCast | FurnaceBolt
            | TwoFurnacesBolt | FurnaceSourceLeaves | AmplifiedDeath | CommanderAmplified
            | BoltEliminatesPlayer => 0,
            MultiCasterModifiers => 3,
            AdditiveModifiers | FogThenShield | ShieldThenFog => 2,
            _ => 1,
        };
        let death = matches!(
            policy,
            ZeroToughnessReturn
                | ZeroToughnessDecline
                | ShieldSimultaneousFirst
                | ShieldSimultaneousLast
                | AmplifiedDeath
        );
        if created != expected_created
            || collected != if death { 4 } else { 0 }
            || losses != if death { 4 } else { 0 }
            || gains != if death { 4 } else { 0 }
            || counters != usize::from(matches!(policy, GrowthCountered | ShieldStifled))
        {
            return Err(format!(
                "effects {policy:?} invocation/creation/life counts differ: {created}/{collected}/{losses}/{gains}/{counters}"
            ));
        }
        if death_reasons
            != if death {
                vec![
                    if matches!(policy, ZeroToughnessReturn | ZeroToughnessDecline) {
                        mtg_core::CreatureLossReason::ZeroToughness
                    } else {
                        mtg_core::CreatureLossReason::LethalDamage
                    },
                ]
            } else {
                vec![]
            }
        {
            return Err("effects zero-toughness and lethal-damage causes differ".into());
        }
        for a in &applications {
            if a.before <= Quantity::zero() || a.after.is_negative() || a.prevented.is_negative() {
                return Err("invalid typed damage application quantities".into());
            }
            match a.effect {
                EffectKey::Static { .. } => {
                    let mut doubled = a.before.clone();
                    doubled.add(&a.before);
                    if a.after != doubled
                        || a.prevented != Quantity::zero()
                        || a.budget_before.is_some()
                        || a.budget_after.is_some()
                    {
                        return Err("damage doubling facts differ".into());
                    }
                }
                EffectKey::Resolved { .. } => {
                    let mut remaining = a.before.clone();
                    remaining.subtract(&a.prevented);
                    if remaining != a.after {
                        return Err("prevention amount conservation differs".into());
                    }
                    if let Some(before) = &a.budget_before {
                        let mut after = before.clone();
                        after.subtract(&a.prevented);
                        if a.budget_after.as_ref() != Some(&after) {
                            return Err("shield budget conservation differs".into());
                        }
                    }
                }
            }
        }
        if matches!(
            policy,
            DoubleThenPrevent | PreventThenDouble | AppliedPrefixSourceLeaves
        ) {
            let expected = if policy != PreventThenDouble {
                [(3, 6), (6, 5)]
            } else {
                [(3, 2), (2, 4)]
            };
            if applications.len() != 2
                || applications
                    .iter()
                    .zip(expected)
                    .any(|(a, (before, after))| {
                        a.before != before.into() || a.after != after.into()
                    })
            {
                return Err(
                    "chosen replacement order differs from typed application prefix".into(),
                );
            }
        }
        if policy == AppliedPrefixSourceLeaves {
            let before=g.events().iter().position(|e|matches!(e,Event::Committed{actual:EventOutcome::DamageEffectApplied{application},..} if matches!(application.effect,EffectKey::Static{..}))).ok_or("missing applied static prefix")?;
            let departure = g
                .events()
                .iter()
                .position(|e| {
                    matches!(
                        e,
                        Event::PlayerDeparted {
                            player: PlayerId(2)
                        }
                    )
                })
                .ok_or("missing provider departure")?;
            let shield=g.events().iter().position(|e|matches!(e,Event::Committed{actual:EventOutcome::DamageEffectApplied{application},..} if matches!(application.effect,EffectKey::Resolved{..}))).ok_or("missing remaining shield application")?;
            if !(before < departure && departure < shield) {
                return Err(
                    "provider departure did not split applied prefix and remaining effect".into(),
                );
            }
        }
        if policy == TwoFurnacesBolt
            && (applications.len() != 2
                || applications[0].effect == applications[1].effect
                || !applications
                    .iter()
                    .all(|a| matches!(a.effect, EffectKey::Static { .. })))
        {
            return Err("distinct Furnace identities did not apply exactly once".into());
        }
        if matches!(
            policy,
            PumpedPinger | PumpedPingerBounced | PumpedPingerBorrowedOwnerLeaves
        ) && (actual_damage.len() != 1
            || actual_damage[0].source.object.card != CardId(3)
            || actual_damage[0].source.characteristics.power != Some(4.into())
            || actual_damage[0].source.characteristics.toughness != Some(4.into())
            || actual_damage[0].amount != 1.into()
            || actual_damage[0].controller != PlayerId(0))
        {
            return Err("pumped source facts changed fixed-one damage or lost LKI".into());
        }
        if matches!(
            policy,
            FogCombat | NegativePower | PreventedDeath | FogCasterLeaves
        ) && !actual_damage.is_empty()
        {
            return Err("prevented or negative-power occurrence dealt actual damage".into());
        }
        if matches!(policy, ShieldSimultaneousFirst | ShieldSimultaneousLast) {
            let source = if policy == ShieldSimultaneousFirst {
                CardId(16)
            } else {
                CardId(17)
            };
            let reduced=actual_damage.iter().filter(|d|d.source.object.card==source&&matches!(d.recipient,DamageRecipient::Creature{object}if object.card==CardId(0))).collect::<Vec<_>>();
            if applications.len() != 1 || reduced.len() != 1 || reduced[0].amount != 1.into() {
                return Err("shared shield ignored selected simultaneous source allocation".into());
            }
        }
        for proof in effects_proofs(g) {
            if let EventOutcome::DamageCommitted {
                actual, results, ..
            } = &proof.actual
            {
                let tx = proof
                    .before
                    .damage_transaction
                    .as_ref()
                    .ok_or("actual damage lacks consumed transaction proof")?;
                let mut seen = std::collections::BTreeSet::new();
                for d in actual {
                    let original = tx
                        .entries
                        .iter()
                        .find(|entry| entry.id == d.id)
                        .ok_or("actual damage lost original occurrence")?;
                    if d.amount <= Quantity::zero()
                        || !seen.insert(d.id)
                        || d.amount != original.amount
                        || d.source != original.source
                        || d.recipient != original.recipient
                        || d.controller != original.controller
                    {
                        return Err(
                            "actual damage not bound to transformed source/recipient occurrence"
                                .into(),
                        );
                    }
                }
                for result in results {
                    let mut sum = Quantity::zero();
                    for d in actual.iter().filter(|d| d.recipient == result.recipient) {
                        sum.add(&d.amount);
                    }
                    let mut current = result.previous.clone();
                    match result.recipient {
                        DamageRecipient::Player { .. } => current.subtract(&sum),
                        DamageRecipient::Creature { .. } => current.add(&sum),
                    }
                    if sum <= Quantity::zero() || current != result.current {
                        return Err(
                            "simultaneous recipient aggregate differs from actual damage".into(),
                        );
                    }
                }
            }
        }
    }
    let history = g
        .commander_ledger()
        .history(CardId(0))
        .ok_or("missing effects Commander history")?;
    if policy == CommanderAmplified {
        if history.combat_damage.get(&PlayerId(1)) != Some(&Quantity::from(4)) {
            return Err("amplified Commander damage differs".into());
        }
    } else if !history.combat_damage.is_empty() {
        return Err("prevented/ordinary effects changed Commander history".into());
    }
    Ok(())
}
fn effects_proofs(g: &Game) -> Vec<&mtg_core::OccurrenceResult> {
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
        eprintln!("running effects {policy:?}");
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
                return Err("effects trace/budget partition differs".into());
            }
        }
        let g = reference.game;
        reports.push(serde_json::json!({"policy":policy,"state_sha256":g.state_digest()?,"turn":g.turn(),"journal":g.replay_journal(),"events":g.events(),"effects_proofs":effects_proofs(&g),"checkpoints":reference.checkpoints.len(),"choices":reference.choices,"final_observations":(0..4).map(|p|observation(&g,PlayerId(p))).collect::<Result<Vec<_>,_>>()?,"commander_ledger":g.commander_ledger()}));
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
        return Err("missing effects witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("effects policy/config mismatch".into());
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
            return Err("effects root recovery differs".into());
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
            return Err("effects retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_effects_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::catalog::embedded_catalog("effects").unwrap();
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
