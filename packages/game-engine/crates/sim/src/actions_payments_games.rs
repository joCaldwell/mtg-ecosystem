//! Finite source-backed policies. All decisions consume only player views;
//! physical-card assertions and snapshot verification belong to the host.
use super::*;
use mtg_contracts::{Color, ManaCost, Quantity};
use mtg_core::{Event, ObjectHandle, PlayerRequest, PlayerResponse, PlayerStatus};
use std::collections::BTreeMap;

const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const HAPLESS: &str = "240a349a-79f8-4e11-9a05-a78148e70a62";
const EDICT: &str = "10c585c4-bf5b-4d8f-94a9-e9a5036a688f";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const SWAMP: &str = "56719f6a-1a6c-4c0a-8d21-18f7d7350b68";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    MixedPreserveBlack,
    MixedSpendBlack,
    MixedTaxRecast,
    EdictCreature,
    EdictCommanderReturn,
    EdictCommanderDecline,
    EdictEmpty,
    EdictDepartureBefore,
    EdictDepartureChoice,
    AbilityOrdinary,
    AbilityBorrowedOwnerLeaves,
    AbilityEmptyDraw,
    AbilityCommanderReturn,
    AbilityCommanderDiscard,
}
const POLICIES: [Policy; 14] = [
    Policy::MixedPreserveBlack,
    Policy::MixedSpendBlack,
    Policy::MixedTaxRecast,
    Policy::EdictCreature,
    Policy::EdictCommanderReturn,
    Policy::EdictCommanderDecline,
    Policy::EdictEmpty,
    Policy::EdictDepartureBefore,
    Policy::EdictDepartureChoice,
    Policy::AbilityOrdinary,
    Policy::AbilityBorrowedOwnerLeaves,
    Policy::AbilityEmptyDraw,
    Policy::AbilityCommanderReturn,
    Policy::AbilityCommanderDiscard,
];
impl Policy {
    fn mixed(self) -> bool {
        matches!(
            self,
            Self::MixedPreserveBlack | Self::MixedSpendBlack | Self::MixedTaxRecast
        )
    }
    fn ability(self) -> bool {
        matches!(
            self,
            Self::AbilityOrdinary
                | Self::AbilityBorrowedOwnerLeaves
                | Self::AbilityEmptyDraw
                | Self::AbilityCommanderReturn
                | Self::AbilityCommanderDiscard
        )
    }
    fn tucked(self) -> bool {
        matches!(
            self,
            Self::AbilityCommanderReturn | Self::AbilityCommanderDiscard
        )
    }
    fn config(self) -> ScenarioConfig {
        match self {
            _ if self.mixed() => ScenarioConfig::mixed_mana(),
            Self::AbilityBorrowedOwnerLeaves => ScenarioConfig::actions_borrowed(),
            Self::AbilityEmptyDraw => ScenarioConfig::actions_empty_library(),
            _ => ScenarioConfig::actions(),
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
    casts: usize,
    bounce: bool,
    activated: bool,
    departed: bool,
    choices: Vec<Observation>,
}
fn handle(view: &mtg_core::PermittedView, oracle: &str, zone: Zone) -> Option<ObjectHandle> {
    view.objects
        .iter()
        .find(|o| o.oracle_id == oracle && o.zone == zone)
        .map(|o| o.handle.clone())
}
fn spending(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Result<BTreeMap<Color, Quantity>, String> {
    let mut free = available.clone();
    let mut selected = BTreeMap::<Color, Quantity>::new();
    for color in &cost.colored {
        let n = free.get_mut(color).ok_or("policy lacks required color")?;
        if *n < Quantity::from(1) {
            return Err("policy lacks colored mana".into());
        }
        n.subtract(&1.into());
        selected.entry(*color).or_default().add(&1.into());
    }
    let mut generic = cost.generic.clone();
    for (color, n) in free {
        let amount = std::cmp::min(n, generic.clone());
        if amount > Quantity::zero() {
            generic.subtract(&amount);
            selected.entry(color).or_default().add(&amount);
        }
    }
    if generic != Quantity::zero() {
        return Err("policy lacks generic mana".into());
    }
    Ok(selected)
}
impl Strategy {
    fn done(&self, o: &Observation) -> bool {
        let v = &o.view;
        if self.policy == Policy::AbilityEmptyDraw && v.players[1].status == PlayerStatus::Left {
            return true;
        }
        if !matches!(o.decision, Some(PlayerRequest::Priority { .. })) || !v.stack.is_empty() {
            return false;
        }
        if self.policy == Policy::MixedTaxRecast {
            return self.casts == 3 && handle(v, YARGLE, Zone::Battlefield).is_some();
        }
        if self.policy.mixed() {
            return self.casts == 2;
        }
        if self.policy.ability() {
            return self.activated
                && self
                    .choices
                    .iter()
                    .any(|o| matches!(o.decision, Some(PlayerRequest::ResolveDiscard { .. })));
        }
        self.casts == 1
    }
    fn departure(&mut self, o: &Observation) -> Option<PlayerId> {
        if self.departed {
            return None;
        }
        let depart = match self.policy {
            Policy::EdictDepartureBefore => {
                matches!(o.decision, Some(PlayerRequest::Priority { .. }))
                    && !o.view.stack.is_empty()
            }
            Policy::EdictDepartureChoice => {
                matches!(o.decision, Some(PlayerRequest::Sacrifice { .. }))
            }
            Policy::AbilityBorrowedOwnerLeaves => {
                self.activated
                    && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
                    && !o.view.stack.is_empty()
            }
            _ => false,
        };
        if depart {
            self.departed = true;
            Some(if self.policy == Policy::AbilityBorrowedOwnerLeaves {
                PlayerId(2)
            } else {
                PlayerId(1)
            })
        } else {
            None
        }
    }
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let r = o.decision.as_ref().ok_or("missing action decision")?;
        Ok(match r {
            PlayerRequest::Priority { .. } => {
                let main = v.stack.is_empty()
                    && v.active == Some(v.viewer)
                    && matches!(
                        v.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    );
                if self.policy.mixed()
                    && main
                    && v.viewer == PlayerId(0)
                    && (self.casts == 0
                        || self.policy == Policy::MixedTaxRecast
                            && self.bounce
                            && (self.casts == 1 || self.casts == 2 && v.turn.number >= 5))
                {
                    let card = handle(v, YARGLE, Zone::Command { owner: v.viewer })
                        .ok_or("missing command Yargle")?;
                    self.casts += 1;
                    PlayerResponse::Cast { card }
                } else if self.policy == Policy::MixedTaxRecast
                    && !self.bounce
                    && self.casts == 1
                    && v.stack.is_empty()
                    && v.viewer == PlayerId(1)
                    && handle(v, YARGLE, Zone::Battlefield).is_some()
                {
                    self.bounce = true;
                    PlayerResponse::Cast {
                        card: handle(v, UNSUMMON, Zone::Hand { owner: v.viewer })
                            .ok_or("missing bounce")?,
                    }
                } else if main
                    && v.viewer == PlayerId(0)
                    && !self.policy.ability()
                    && self.policy != Policy::MixedTaxRecast
                    && self.casts == usize::from(self.policy.mixed())
                {
                    self.casts += 1;
                    PlayerResponse::Cast {
                        card: handle(v, EDICT, Zone::Hand { owner: v.viewer })
                            .ok_or("missing Cruel Edict")?,
                    }
                } else if self.policy.tucked() && main && v.viewer == PlayerId(0) && self.casts == 0
                {
                    self.casts += 1;
                    PlayerResponse::Cast {
                        card: handle(v, TIME_EBB, Zone::Hand { owner: v.viewer })
                            .ok_or("missing Time Ebb")?,
                    }
                } else if self.policy.ability()
                    && !self.activated
                    && v.viewer == PlayerId(1)
                    && v.stack.is_empty()
                    && (!self.policy.tucked() || self.casts == 1)
                {
                    self.activated = true;
                    PlayerResponse::ActivateAbility {
                        object: handle(v, HAPLESS, Zone::Battlefield).ok_or("missing Hapless")?,
                        ability: 0,
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::StackTargets { .. } => {
                return Err("unexpected stack target in historical action profile".into());
            }
            PlayerRequest::PlayerTargets { .. } => PlayerResponse::PlayerTargets {
                players: vec![if self.policy == Policy::EdictEmpty {
                    PlayerId(3)
                } else {
                    PlayerId(1)
                }],
            },
            PlayerRequest::Targets { candidates, .. } => {
                let oracle = if self.policy.mixed() { YARGLE } else { ISAMARU };
                let card = handle(v, oracle, Zone::Battlefield).ok_or("missing creature target")?;
                if !candidates.contains(&card) {
                    return Err("target not offered".into());
                }
                PlayerResponse::Targets {
                    objects: vec![card],
                }
            }
            PlayerRequest::Mana { .. } => {
                if let Some(source) = v.objects.iter().find(|x| {
                    x.zone == Zone::Battlefield
                        && x.controller == v.viewer
                        && !x.tapped
                        && matches!(x.oracle_id.as_str(), ISLAND | FOREST | SWAMP | PLAINS)
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
                let selected = if self.policy.mixed() && v.viewer == PlayerId(0) && self.casts == 1
                {
                    let (b, g) = if self.policy == Policy::MixedSpendBlack {
                        (4, 2)
                    } else {
                        (2, 4)
                    };
                    [(Color::B, b.into()), (Color::G, g.into())].into()
                } else if self.policy == Policy::MixedPreserveBlack && v.viewer == PlayerId(0) {
                    [(Color::B, 1.into()), (Color::G, 1.into())].into()
                } else {
                    spending(cost, available)?
                };
                self.choices.push(o.clone());
                PlayerResponse::SpendMana { spending: selected }
            }
            PlayerRequest::Sacrifice { candidates, .. } => {
                let oracle = if matches!(
                    self.policy,
                    Policy::EdictCommanderReturn | Policy::EdictCommanderDecline
                ) {
                    ISAMARU
                } else {
                    BEARS
                };
                let object =
                    handle(v, oracle, Zone::Battlefield).ok_or("missing sacrifice choice")?;
                if !candidates.contains(&object) {
                    return Err("sacrifice not offered".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::Sacrifice { object }
            }
            PlayerRequest::ResolveDiscard { candidates, .. } => {
                let oracle = if self.policy == Policy::AbilityCommanderDiscard {
                    ISAMARU
                } else {
                    UNSUMMON
                };
                let object = handle(v, oracle, Zone::Hand { owner: v.viewer })
                    .ok_or("missing discard choice")?;
                if !candidates.contains(&object) {
                    return Err("discard not offered".into());
                }
                self.choices.push(o.clone());
                PlayerResponse::ResolveDiscard {
                    objects: vec![object],
                }
            }
            PlayerRequest::CommanderReturn { destination, .. } => {
                self.choices.push(o.clone());
                let to_command = if self.policy.tucked() {
                    match destination {
                        Zone::Library { .. } => false,
                        Zone::Hand { .. } => self.policy == Policy::AbilityCommanderReturn,
                        Zone::Graveyard { .. } => true,
                        _ => return Err("unexpected ability Commander destination".into()),
                    }
                } else {
                    self.policy != Policy::EdictCommanderDecline
                };
                PlayerResponse::CommanderReturn { to_command }
            }
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Damage { .. } => return Err("unexpected combat damage".into()),
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
        })
    }
}

fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("action query rejected: {reason:?}"));
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
        _ => Err("wrong action observation result".into()),
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
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing action snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("action boundary recovery differs".into());
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
        return Err("action runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut strategy = Strategy {
        policy,
        casts: 0,
        bounce: false,
        activated: false,
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
                    .ok_or("missing action token")?
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
                    _ => return Err("wrong action validation result".into()),
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "action witness {policy:?} terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err(format!("action witness {policy:?} exhausted host budget"))
}
fn zone(game: &Game, id: u32) -> Option<Zone> {
    game.object_id(CardId(id))
        .and_then(|object| game.zone(object))
}
fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let g = &run.game;
    let events = g.events();
    let has_resolution_discard = run
        .choices
        .iter()
        .any(|o| matches!(o.decision, Some(PlayerRequest::ResolveDiscard { .. })));
    let expected = match policy {
        Policy::MixedPreserveBlack => {
            zone(g, 0) == Some(Zone::Battlefield)
                && zone(g, 10) == Some(Zone::Graveyard { owner: PlayerId(0) })
                && zone(g, 12) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && g.mana(PlayerId(0), Color::B) == Some(1.into())
        }
        Policy::MixedSpendBlack => {
            zone(g, 0) == Some(Zone::Battlefield)
                && zone(g, 10) == Some(Zone::Hand { owner: PlayerId(0) })
                && g.mana(PlayerId(0), Color::G) == Some(3.into())
        }
        Policy::MixedTaxRecast => {
            zone(g, 0) == Some(Zone::Battlefield)
                && g.commander_ledger()
                    .history(CardId(0))
                    .is_some_and(|h| h.command_casts.get(&PlayerId(0)) == Some(&2.into()))
                && g.turn().number == 5
        }
        Policy::EdictCreature => {
            zone(g, 1) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 0) == Some(Zone::Battlefield)
        }
        Policy::EdictCommanderReturn => zone(g, 0) == Some(Zone::Command { owner: PlayerId(1) }),
        Policy::EdictCommanderDecline => zone(g, 0) == Some(Zone::Graveyard { owner: PlayerId(1) }),
        Policy::EdictEmpty => {
            zone(g, 0) == Some(Zone::Battlefield)
                && zone(g, 1) == Some(Zone::Battlefield)
                && !run
                    .choices
                    .iter()
                    .any(|o| matches!(o.decision, Some(PlayerRequest::Sacrifice { .. })))
        }
        Policy::EdictDepartureBefore | Policy::EdictDepartureChoice => {
            zone(g, 0).is_none() && g.replay_journal().concessions.len() == 1
        }
        Policy::AbilityOrdinary => {
            zone(g, 5) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 14) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 17) == Some(Zone::Hand { owner: PlayerId(1) })
                && has_resolution_discard
        }
        Policy::AbilityBorrowedOwnerLeaves => {
            zone(g, 5).is_none()
                && zone(g, 14) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && zone(g, 17) == Some(Zone::Hand { owner: PlayerId(1) })
                && g.replay_journal().concessions.len() == 1
                && has_resolution_discard
        }
        Policy::AbilityEmptyDraw => {
            zone(g, 5).is_none()
                && has_resolution_discard
                && observation(g, PlayerId(0))?.view.players[1].status == PlayerStatus::Left
        }
        Policy::AbilityCommanderReturn | Policy::AbilityCommanderDiscard => {
            zone(g, 0) == Some(Zone::Command { owner: PlayerId(1) })
                && zone(g, 5) == Some(Zone::Graveyard { owner: PlayerId(1) })
                && has_resolution_discard
        }
    };
    if !expected {
        return Err(format!("action witness {policy:?} physical result differs"));
    }
    if policy.ability()
        && g.trace().mode == TraceMode::Detailed
        && (events
            .iter()
            .filter(|e| matches!(e, Event::AbilityActivated { .. }))
            .count()
            != 1
            || events
                .iter()
                .filter(|e| matches!(e, Event::AbilityCompleted { .. }))
                .count()
                != 1)
    {
        return Err("ability lifetime evidence differs".into());
    }
    Ok(())
}
pub(super) fn suite(
    catalog: &Catalog,
    recover: bool,
) -> Result<(Vec<Evidence>, Vec<Game>, serde_json::Value), String> {
    let (mut evidence, mut games, mut reports) = (vec![], vec![], vec![]);
    for policy in POLICIES {
        eprintln!("running actions/payments {policy:?}");
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
                return Err("action trace/budget partition differs".into());
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
        return Err("missing action witness".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("action policy/config mismatch".into());
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
            return Err("action root recovery differs".into());
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
            return Err("action retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_action_coverage_binds_policies_configs_and_initial_roots() {
        let catalog = mtg_compiler::action_fixture_catalog().unwrap();
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
