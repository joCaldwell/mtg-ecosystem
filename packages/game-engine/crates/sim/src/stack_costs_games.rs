//! Source-backed policies use only the acting player's observation. Recovery,
//! physical identities and outcome assertions belong to the trusted host.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{Event, ObjectHandle, PlayerRequest, PlayerResponse, PlayerStatus};
use std::collections::BTreeMap;

const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const LOOTER: &str = "67362406-b1ca-49e2-800d-9050bfe8742a";
const MAGE: &str = "bbf9fd47-10e1-4749-b06f-4fe72200afc7";
const COUNTER: &str = "cc187110-1148-4090-bbb8-e205694a39f5";
const STIFLE: &str = "b3b00911-ece7-4484-bc36-f211ce72b6cc";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    CounterCreature,
    CounterCommanderReturn,
    CounterCommanderDecline,
    CounterWar,
    CounterOwnSpell,
    CounterDepartedTarget,
    StifleHapless,
    StiflePaidAbility,
    StifleDuplicateSource,
    ManaAbilityRepeat,
    ManaAbilityKeep,
    ManaAbilityUndo,
    BorrowedAnnouncementDeparture,
    TapAbilitySourceBounce,
    TapCommanderDrawDiscard,
    ManaCommanderDrawReturn,
    TapFailedDraw,
    CounterCommanderRecast,
}
const POLICIES: [Policy; 18] = [
    Policy::CounterCreature,
    Policy::CounterCommanderReturn,
    Policy::CounterCommanderDecline,
    Policy::CounterWar,
    Policy::CounterOwnSpell,
    Policy::CounterDepartedTarget,
    Policy::StifleHapless,
    Policy::StiflePaidAbility,
    Policy::StifleDuplicateSource,
    Policy::ManaAbilityRepeat,
    Policy::ManaAbilityKeep,
    Policy::ManaAbilityUndo,
    Policy::BorrowedAnnouncementDeparture,
    Policy::TapAbilitySourceBounce,
    Policy::TapCommanderDrawDiscard,
    Policy::ManaCommanderDrawReturn,
    Policy::TapFailedDraw,
    Policy::CounterCommanderRecast,
];
impl Policy {
    fn commander(self) -> bool {
        matches!(
            self,
            Self::CounterCommanderReturn
                | Self::CounterCommanderDecline
                | Self::CounterWar
                | Self::CounterCommanderRecast
        )
    }
    fn counter(self) -> bool {
        matches!(
            self,
            Self::CounterCreature
                | Self::CounterCommanderReturn
                | Self::CounterCommanderDecline
                | Self::CounterWar
                | Self::CounterOwnSpell
                | Self::CounterDepartedTarget
                | Self::CounterCommanderRecast
        )
    }
    fn stifle(self) -> bool {
        matches!(
            self,
            Self::StifleHapless | Self::StiflePaidAbility | Self::StifleDuplicateSource
        )
    }
    fn library(self) -> bool {
        matches!(
            self,
            Self::TapCommanderDrawDiscard | Self::ManaCommanderDrawReturn
        )
    }
    fn failed(self) -> bool {
        matches!(self, Self::ManaAbilityKeep | Self::ManaAbilityUndo)
    }
    fn repeated(self) -> bool {
        matches!(self, Self::StifleDuplicateSource | Self::ManaAbilityRepeat)
    }
    fn source(self) -> &'static str {
        match self {
            Self::StifleHapless => HAPLESS,
            Self::TapAbilitySourceBounce | Self::TapCommanderDrawDiscard | Self::TapFailedDraw => {
                LOOTER
            }
            _ => MAGE,
        }
    }
    fn config(self) -> ScenarioConfig {
        match self {
            Self::BorrowedAnnouncementDeparture => ScenarioConfig::stack_costs_borrowed(),
            Self::TapFailedDraw => ScenarioConfig::stack_costs_empty_library(),
            _ if self.library() => ScenarioConfig::stack_costs_library(),
            _ => ScenarioConfig::stack_costs(),
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
    counter1: bool,
    counter2: bool,
    stifle: bool,
    bounce: bool,
    activations: u8,
    departed: bool,
    choices: Vec<Observation>,
}
fn handle(v: &mtg_core::PermittedView, oracle: &str, zone: Zone) -> Option<ObjectHandle> {
    v.objects
        .iter()
        .find(|o| o.oracle_id == oracle && o.zone == zone)
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
    for (color, n) in free {
        let amount = std::cmp::min(n, generic.clone());
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
        let v = &o.view;
        if self.policy == Policy::TapFailedDraw && v.players[0].status == PlayerStatus::Left {
            return true;
        }
        if !matches!(o.decision, Some(PlayerRequest::Priority { .. })) || !v.stack.is_empty() {
            return false;
        }
        match self.policy {
            Policy::CounterWar => self.primary == 1 && self.counter1 && self.counter2,
            Policy::CounterOwnSpell => self.primary == 1 && self.counter1,
            Policy::CounterDepartedTarget => self.departed && self.counter1,
            Policy::CounterCommanderRecast => {
                self.primary == 3 && handle(v, COMMANDER, Zone::Battlefield).is_some()
            }
            _ if self.policy.counter() => self.primary == 1 && self.counter1,
            _ if self.policy.stifle() => {
                self.stifle && self.activations == if self.policy.repeated() { 2 } else { 1 }
            }
            Policy::ManaAbilityRepeat => self.activations == 2,
            Policy::BorrowedAnnouncementDeparture => self.activations == 1 && self.departed,
            Policy::TapAbilitySourceBounce => self.activations == 1 && self.bounce,
            _ => self.activations == 1,
        }
    }
    fn departure(&mut self, o: &Observation) -> Option<PlayerId> {
        if self.departed {
            return None;
        }
        let player = match self.policy {
            Policy::CounterDepartedTarget
                if self.counter1
                    && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
                    && o.view.stack.len() == 2 =>
            {
                Some(PlayerId(0))
            }
            Policy::BorrowedAnnouncementDeparture
                if self.activations == 1
                    && matches!(o.decision, Some(PlayerRequest::Mana { .. })) =>
            {
                Some(PlayerId(2))
            }
            _ => None,
        };
        if player.is_some() {
            self.departed = true;
        }
        player
    }
    fn cast(&self, v: &mtg_core::PermittedView, card: &str) -> Result<PlayerResponse, String> {
        Ok(PlayerResponse::Cast {
            card: handle(v, card, Zone::Hand { owner: v.viewer })
                .ok_or_else(|| format!("missing policy spell {card}"))?,
        })
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let r = o.decision.as_ref().ok_or("missing stack/cost decision")?;
        Ok(match r {
            PlayerRequest::ManaColor { .. }
            | PlayerRequest::SearchLands { .. }
            | PlayerRequest::OptionalRecursion { .. } => {
                return Err("staples request outside historical policy".into());
            }
            PlayerRequest::Mulligan { .. } | PlayerRequest::BottomOpening { .. } => {
                return Err("opening setup is not admitted by this historical scenario".into());
            }
            PlayerRequest::Priority { .. } => {
                let main = v.stack.is_empty()
                    && v.active == Some(v.viewer)
                    && matches!(
                        v.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                if self.policy.counter()
                    && self.policy != Policy::CounterOwnSpell
                    && v.viewer == PlayerId(0)
                    && main
                    && (self.primary == 0
                        || self.policy == Policy::CounterCommanderRecast
                            && self.counter1
                            && (self.primary == 1 || self.primary == 2 && v.turn.number >= 5))
                {
                    self.primary += 1;
                    if self.policy.commander() {
                        PlayerResponse::Cast {
                            card: handle(v, COMMANDER, Zone::Command { owner: v.viewer })
                                .ok_or("missing command caster")?,
                        }
                    } else {
                        self.cast(v, BEARS)?
                    }
                } else if self.policy == Policy::CounterOwnSpell
                    && v.viewer == PlayerId(1)
                    && v.stack.is_empty()
                    && self.primary == 0
                {
                    self.primary = 1;
                    self.cast(v, UNSUMMON)?
                } else if self.policy.counter()
                    && v.viewer == PlayerId(1)
                    && !self.counter1
                    && v.stack.len() == 1
                    && self.primary == 1
                {
                    self.counter1 = true;
                    self.cast(v, COUNTER)?
                } else if self.policy == Policy::CounterWar
                    && v.viewer == PlayerId(2)
                    && self.counter1
                    && !self.counter2
                    && v.stack.len() == 2
                {
                    self.counter2 = true;
                    self.cast(v, COUNTER)?
                } else if self.policy.stifle()
                    && v.viewer == PlayerId(1)
                    && !self.stifle
                    && self.activations == if self.policy.repeated() { 2 } else { 1 }
                    && !v.stack.is_empty()
                {
                    self.stifle = true;
                    self.cast(v, STIFLE)?
                } else if self.policy == Policy::TapAbilitySourceBounce
                    && v.viewer == PlayerId(1)
                    && self.activations == 1
                    && !self.bounce
                    && !v.stack.is_empty()
                {
                    self.bounce = true;
                    self.cast(v, UNSUMMON)?
                } else if self.policy.library()
                    && main
                    && v.viewer == PlayerId(1)
                    && self.primary == 0
                {
                    self.primary = 1;
                    self.cast(v, TIME_EBB)?
                } else if !self.policy.counter()
                    && v.viewer == PlayerId(0)
                    && self.activations < if self.policy.repeated() { 2 } else { 1 }
                    && (!self.policy.library() || self.primary == 1 && v.stack.is_empty())
                {
                    self.activations += 1;
                    PlayerResponse::ActivateAbility {
                        object: handle(v, self.policy.source(), Zone::Battlefield)
                            .ok_or("missing activated source")?,
                        ability: 0,
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Modes { .. }
            | PlayerRequest::DamageAllocation { .. }
            | PlayerRequest::DamageEffect { .. }
            | PlayerRequest::OrderTriggers { .. } => {
                return Err("ordinary trigger ordering outside historical profile".into());
            }
            PlayerRequest::EntityTargets { .. } => {
                return Err("mixed targets outside damage/life profile".into());
            }
            PlayerRequest::StackTargets { candidates, .. } => {
                let wanted = if self.policy.stifle() {
                    None
                } else if self.policy == Policy::CounterOwnSpell {
                    Some(UNSUMMON)
                } else if self.policy == Policy::CounterWar && v.viewer == PlayerId(2) {
                    Some(COUNTER)
                } else if self.policy.commander() {
                    Some(COMMANDER)
                } else {
                    Some(BEARS)
                };
                let target = v
                    .stack
                    .iter()
                    .find(|entry| match wanted {
                        None => entry.ability.is_some(),
                        Some(id) => {
                            entry.ability.is_none() && oracle(v, entry).is_ok_and(|x| x == id)
                        }
                    })
                    .ok_or("missing intended stack target")?;
                let entry = target.handle.clone().ok_or("missing public stack handle")?;
                if !candidates.contains(&entry) {
                    return Err("intended stack target is not eligible".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::StackTargets {
                    entries: vec![entry],
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let target = if self.policy.library() {
                    COMMANDER
                } else {
                    LOOTER
                };
                let object = handle(v, target, Zone::Battlefield)
                    .ok_or("missing ordinary response target")?;
                if !candidates.contains(&object) {
                    return Err("ordinary target not eligible".into());
                }
                PlayerResponse::Targets {
                    objects: vec![object],
                }
            }
            PlayerRequest::Mana { .. } => {
                let pool = &v.players[v.viewer.0 as usize].mana;
                if self.policy.failed()
                    && v.viewer == PlayerId(0)
                    && pool.values().any(|n| *n > Quantity::zero())
                {
                    PlayerResponse::FinishMana {
                        undo_mana: self.policy == Policy::ManaAbilityUndo,
                    }
                } else if let Some(source) = v.objects.iter().find(|x| {
                    x.zone == Zone::Battlefield
                        && x.controller == v.viewer
                        && !x.tapped
                        && matches!(x.oracle_id.as_str(), ISLAND | FOREST | PLAINS)
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
            PlayerRequest::ResolveDiscard { candidates, .. } => {
                let id = if self.policy == Policy::TapCommanderDrawDiscard {
                    COMMANDER
                } else {
                    BEARS
                };
                let object = handle(v, id, Zone::Hand { owner: v.viewer })
                    .ok_or("missing mandated discard")?;
                if !candidates.contains(&object) {
                    return Err("discard outside domain".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::ResolveDiscard {
                    objects: vec![object],
                }
            }
            PlayerRequest::CommanderReturn { destination, .. } => {
                self.choices.push(o.clone());
                let to_command = match destination {
                    Zone::Library { .. } => false,
                    Zone::Hand { .. } => self.policy == Policy::ManaCommanderDrawReturn,
                    Zone::Graveyard { .. } => self.policy != Policy::CounterCommanderDecline,
                    _ => return Err("unexpected counter/cost Commander destination".into()),
                };
                PlayerResponse::CommanderReturn { to_command }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
            PlayerRequest::PlayerTargets { .. }
            | PlayerRequest::Sacrifice { .. }
            | PlayerRequest::Damage { .. } => {
                return Err("unexpected stack/cost policy request".into());
            }
        })
    }
}

fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("stack/cost query rejected: {reason:?}"));
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
        _ => Err("wrong stack/cost observation result".into()),
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
            verifier.verify(snapshot.as_deref().ok_or("missing stack/cost snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("stack/cost boundary recovery differs".into());
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
        return Err("stack/cost runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut strategy = Strategy {
        policy,
        primary: 0,
        counter1: false,
        counter2: false,
        stifle: false,
        bounce: false,
        activations: 0,
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
                    .ok_or("missing stack/cost token")?
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
                    _ => return Err("wrong stack/cost validation result".into()),
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "stack/cost witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!(
        "stack/cost witness {policy:?} exhausted host budget"
    ))
}

fn zone(g: &Game, id: u32) -> Option<Zone> {
    g.object_id(CardId(id)).and_then(|o| g.zone(o))
}
fn mana_total(g: &Game, player: PlayerId) -> Result<Quantity, String> {
    let v = observation(g, player)?;
    let mut total = Quantity::zero();
    for n in v.view.players[player.0 as usize].mana.values() {
        total.add(n);
    }
    Ok(total)
}
fn command_casts(g: &Game) -> Quantity {
    g.commander_ledger()
        .history(CardId(0))
        .and_then(|h| h.command_casts.get(&PlayerId(0)))
        .cloned()
        .unwrap_or_default()
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let g = &run.game;
    let hand0 = Zone::Hand { owner: PlayerId(0) };
    let grave0 = Zone::Graveyard { owner: PlayerId(0) };
    let command0 = Zone::Command { owner: PlayerId(0) };
    let library0 = Zone::Library { owner: PlayerId(0) };
    let untapped = |id: u32| {
        let wanted = if id == 7 { LOOTER } else { MAGE };
        observation(g, PlayerId(0)).is_ok_and(|o| {
            g.object_id(CardId(id)).is_some()
                && o.view
                    .objects
                    .iter()
                    .any(|x| x.oracle_id == wanted && x.zone == Zone::Battlefield && !x.tapped)
        })
    };
    let ok = match policy {
        Policy::CounterCreature => {
            zone(g, 10) == Some(grave0) && mana_total(g, PlayerId(0))? == 6.into()
        }
        Policy::CounterCommanderReturn => {
            zone(g, 0) == Some(command0)
                && command_casts(g) == 1.into()
                && mana_total(g, PlayerId(0))? == 7.into()
        }
        Policy::CounterCommanderDecline => {
            zone(g, 0) == Some(grave0) && command_casts(g) == 1.into()
        }
        Policy::CounterWar => {
            zone(g, 0) == Some(Zone::Battlefield)
                && command_casts(g) == 1.into()
                && zone(g, 11) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 19) == Some(Zone::Graveyard { owner: PlayerId(2) })
                && mana_total(g, PlayerId(1))? == 2.into()
        }
        Policy::CounterOwnSpell => {
            zone(g, 13) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 11) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && untapped(7)
        }
        Policy::CounterDepartedTarget => {
            zone(g, 10).is_none()
                && observation(g, PlayerId(1))?.view.players[0].status == PlayerStatus::Left
                && g.replay_journal().concessions.len() == 1
                && zone(g, 11) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && mana_total(g, PlayerId(1))? == 2.into()
        }
        Policy::StifleHapless => {
            zone(g, 9) == Some(grave0)
                && zone(g, 23) == Some(library0)
                && zone(g, 24) == Some(library0)
        }
        Policy::StiflePaidAbility => {
            untapped(8)
                && zone(g, 23) == Some(library0)
                && zone(g, 24) == Some(library0)
                && mana_total(g, PlayerId(0))? == 4.into()
        }
        Policy::StifleDuplicateSource => {
            untapped(8)
                && zone(g, 23) == Some(hand0)
                && zone(g, 24) == Some(library0)
                && mana_total(g, PlayerId(0))? == Quantity::zero()
        }
        Policy::ManaAbilityRepeat => {
            untapped(8)
                && zone(g, 23) == Some(hand0)
                && zone(g, 24) == Some(hand0)
                && mana_total(g, PlayerId(0))? == Quantity::zero()
        }
        Policy::ManaAbilityKeep => {
            untapped(8)
                && g.mana(PlayerId(0), Color::U) == Some(1.into())
                && zone(g, 23) == Some(library0)
        }
        Policy::ManaAbilityUndo => {
            untapped(8)
                && mana_total(g, PlayerId(0))? == Quantity::zero()
                && zone(g, 23) == Some(library0)
        }
        Policy::BorrowedAnnouncementDeparture => {
            zone(g, 8).is_none()
                && zone(g, 23) == Some(hand0)
                && mana_total(g, PlayerId(0))? == 4.into()
                && g.replay_journal().concessions.len() == 1
        }
        Policy::TapAbilitySourceBounce => {
            zone(g, 7) == Some(hand0)
                && zone(g, 23) == Some(hand0)
                && zone(g, 10) == Some(grave0)
                && g.object_id(CardId(7)).is_some_and(|id| id.incarnation == 1)
        }
        Policy::TapCommanderDrawDiscard => {
            zone(g, 0) == Some(command0)
                && zone(g, 7) == Some(Zone::Battlefield)
                && !untapped(7)
                && zone(g, 23) == Some(library0)
                && zone(g, 24) == Some(library0)
        }
        Policy::ManaCommanderDrawReturn => {
            zone(g, 0) == Some(command0)
                && untapped(8)
                && zone(g, 23) == Some(library0)
                && zone(g, 24) == Some(library0)
                && !run
                    .choices
                    .iter()
                    .any(|o| matches!(o.decision, Some(PlayerRequest::ResolveDiscard { .. })))
        }
        Policy::TapFailedDraw => {
            zone(g, 7).is_none()
                && observation(g, PlayerId(1))?.view.players[0].status == PlayerStatus::Left
                && run
                    .choices
                    .iter()
                    .any(|o| matches!(o.decision, Some(PlayerRequest::ResolveDiscard { .. })))
        }
        Policy::CounterCommanderRecast => {
            zone(g, 0) == Some(Zone::Battlefield)
                && command_casts(g) == 2.into()
                && g.turn().number == 5
        }
    };
    if !ok {
        return Err(format!(
            "stack/cost witness {policy:?} physical/payment result differs"
        ));
    }
    if g.trace().mode == TraceMode::Detailed {
        let counters = g
            .events()
            .iter()
            .filter_map(|e| match e {
                Event::Committed {
                    actual: mtg_core::EventOutcome::StackObjectCountered { spell, source, .. },
                    ..
                } => Some((*spell, source.card)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let expected = match policy {
            Policy::CounterCreature => vec![(true, CardId(10))],
            Policy::CounterCommanderReturn
            | Policy::CounterCommanderDecline
            | Policy::CounterCommanderRecast => vec![(true, CardId(0))],
            Policy::CounterWar => vec![(true, CardId(11))],
            Policy::CounterOwnSpell => vec![(true, CardId(13))],
            Policy::StifleHapless => vec![(false, CardId(9))],
            Policy::StiflePaidAbility | Policy::StifleDuplicateSource => vec![(false, CardId(8))],
            _ => vec![],
        };
        if counters != expected {
            return Err(format!(
                "stack/cost witness {policy:?} counter provenance differs"
            ));
        }
        let completed = g
            .events()
            .iter()
            .filter(|e| matches!(e, Event::AbilityCompleted { .. }))
            .count();
        let activated = g
            .events()
            .iter()
            .filter(|e| matches!(e, Event::AbilityActivated { .. }))
            .count();
        let (a, c) = if policy.counter() || policy.failed() {
            (0, 0)
        } else if policy.stifle() {
            (
                if policy.repeated() { 2 } else { 1 },
                usize::from(policy.repeated()),
            )
        } else if policy.repeated() {
            (2, 2)
        } else {
            (1, 1)
        };
        if (activated, completed) != (a, c) {
            return Err(format!(
                "stack/cost witness {policy:?} activation lifetime differs"
            ));
        }
        if policy == Policy::StifleDuplicateSource {
            let activated_ids = g
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::AbilityActivated { stack_id, .. } => Some(*stack_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let completed_ids = g
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::AbilityCompleted { stack_id } => Some(*stack_id),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let countered_ids = g
                .events()
                .iter()
                .filter_map(|e| match e {
                    Event::Committed {
                        actual: mtg_core::EventOutcome::StackObjectCountered { target, .. },
                        ..
                    } => Some(*target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if activated_ids.len() != 2
                || countered_ids != vec![activated_ids[0]]
                || completed_ids != vec![activated_ids[1]]
            {
                return Err(
                    "identical-source abilities rebound the selected stack instance".into(),
                );
            }
        }
        if policy == Policy::CounterDepartedTarget
            && !g.events().iter().any(|e| {
                matches!(
                    e,
                    Event::SpellCompleted {
                        resolved: false,
                        ..
                    }
                )
            })
        {
            return Err("departed stack target did not fizzle".into());
        }
        if policy == Policy::TapFailedDraw {
            let finish = g
                .events()
                .iter()
                .position(|e| matches!(e, Event::AbilityCompleted { .. }))
                .ok_or("missing failed-draw completion")?;
            let loss = g
                .events()
                .iter()
                .position(|e| matches!(e, Event::PlayersLost { .. }))
                .ok_or("missing failed-draw loss")?;
            let discard=g.events().iter().position(|e|matches!(e,Event::Committed{actual:mtg_core::EventOutcome::CardDiscarded {player:PlayerId(0),movement},..} if movement.previous.card==CardId(10))).ok_or("missing mandatory discard after failed draw")?;
            if loss <= finish || discard >= finish {
                return Err("failed draw lost before whole ability completion".into());
            }
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
        eprintln!("running stack/costs {policy:?}");
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
                return Err("stack/cost trace/budget partition differs".into());
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
        return Err("missing stack/cost witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("stack/cost policy/config mismatch".into());
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
            return Err("stack/cost root recovery differs".into());
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
            return Err("stack/cost retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_stack_cost_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::stack_fixture_catalog().unwrap();
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
