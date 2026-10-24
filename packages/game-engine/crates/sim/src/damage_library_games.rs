//! Source-backed bounded policies; decisions use only the acting player's view.
//! Recovery and physical-card assertions belong to the trusted host below.
use super::*;
use mtg_contracts::Quantity;
use mtg_core::{Event, PlayerLossReason, PlayerRequest, PlayerResponse, PlayerStatus};

const YARGLE: &str = "980e1721-3717-4425-95a4-938dbf2ac661";
const ISAMARU: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const TIME_EBB: &str = "30cc8f7b-3c28-40f5-8f8f-157e8212280b";
const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const ISLAND: &str = "b2c6aa39-2d2a-459c-a555-fb48ba993373";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    CommanderDamage,
    BorrowedReturn,
    BorrowedDecline,
    TuckReturn,
    DrawReturn,
    DrawHandCast,
    OrdinaryTuck,
}
const POLICIES: [Policy; 7] = [
    Policy::CommanderDamage,
    Policy::BorrowedReturn,
    Policy::BorrowedDecline,
    Policy::TuckReturn,
    Policy::DrawReturn,
    Policy::DrawHandCast,
    Policy::OrdinaryTuck,
];

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
impl Policy {
    fn is_damage(self) -> bool {
        matches!(
            self,
            Self::CommanderDamage | Self::BorrowedReturn | Self::BorrowedDecline
        )
    }
    fn is_borrowed(self) -> bool {
        matches!(self, Self::BorrowedReturn | Self::BorrowedDecline)
    }
    fn config(self) -> ScenarioConfig {
        match self {
            Self::CommanderDamage => ScenarioConfig::commander_damage(),
            Self::BorrowedReturn | Self::BorrowedDecline => {
                ScenarioConfig::commander_damage_borrowed()
            }
            _ => ScenarioConfig::commander_library(),
        }
    }
    fn catalog<'a>(self, damage: &'a Catalog, library: &'a Catalog) -> &'a Catalog {
        if self.is_damage() { damage } else { library }
    }
}

struct Strategy {
    policy: Policy,
    spell_cast: bool,
    commander_cast: bool,
    departed: bool,
    choices: Vec<Observation>,
    first_hit: Option<Observation>,
}
impl Strategy {
    fn done(&self, observation: &Observation) -> bool {
        let view = &observation.view;
        if !matches!(observation.decision, Some(PlayerRequest::Priority { .. }))
            || !view.stack.is_empty()
        {
            return false;
        }
        if self.policy.is_damage() {
            if view.players[2].status != PlayerStatus::Left {
                return false;
            }
            if !self.policy.is_borrowed() {
                return true;
            }
            let expected_zone = if self.policy == Policy::BorrowedReturn {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Exile
            };
            return self.departed
                && self.choices.len() == 1
                && view
                    .objects
                    .iter()
                    .any(|o| o.oracle_id == YARGLE && o.zone == expected_zone);
        }
        if self.policy == Policy::OrdinaryTuck {
            return self.spell_cast
                && view.viewer == PlayerId(2)
                && view.objects.iter().any(|o| {
                    o.oracle_id == BEARS && o.zone == (Zone::Hand { owner: PlayerId(2) })
                });
        }
        self.commander_cast
            && view
                .objects
                .iter()
                .any(|o| o.oracle_id == ISAMARU && o.zone == Zone::Battlefield)
    }

    fn choose(&mut self, observation: &Observation) -> Result<PlayerResponse, String> {
        let view = &observation.view;
        let request = observation
            .decision
            .as_ref()
            .ok_or("missing lifecycle decision")?;
        Ok(match request {
            PlayerRequest::AnnounceX { .. } => {
                return Err("X request outside historical profile".into());
            }
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
            | PlayerRequest::DamageEffect { .. }
            | PlayerRequest::OrderTriggers { .. } => {
                return Err("ordinary trigger ordering outside historical profile".into());
            }
            PlayerRequest::EntityTargets { .. } => {
                return Err("mixed targets outside damage/life profile".into());
            }
            PlayerRequest::StackTargets { .. }
            | PlayerRequest::PlayerTargets { .. }
            | PlayerRequest::Sacrifice { .. }
            | PlayerRequest::ResolveDiscard { .. }
            | PlayerRequest::SpendMana { .. } => {
                return Err(
                    "action/payment request outside the frozen damage/library policy".into(),
                );
            }
            PlayerRequest::Priority { .. } => {
                if !self.policy.is_damage()
                    && view.stack.is_empty()
                    && view.active == Some(view.viewer)
                    && matches!(
                        view.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    )
                {
                    if !self.spell_cast && view.viewer == PlayerId(0) {
                        let spell = view
                            .objects
                            .iter()
                            .find(|o| {
                                o.oracle_id == TIME_EBB
                                    && o.zone == (Zone::Hand { owner: view.viewer })
                            })
                            .ok_or("missing source-backed sorcery")?;
                        self.spell_cast = true;
                        PlayerResponse::Cast {
                            card: spell.handle.clone(),
                        }
                    } else if self.policy != Policy::OrdinaryTuck
                        && !self.commander_cast
                        && view.viewer == PlayerId(1)
                        && let Some(commander) = view.objects.iter().find(|o| {
                            o.oracle_id == ISAMARU
                                && matches!(
                                    o.zone,
                                    Zone::Command { owner: PlayerId(1) }
                                        | Zone::Hand { owner: PlayerId(1) }
                                )
                        })
                    {
                        self.commander_cast = true;
                        PlayerResponse::Cast {
                            card: commander.handle.clone(),
                        }
                    } else {
                        PlayerResponse::Pass
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let oracle_id = if self.policy == Policy::OrdinaryTuck {
                    BEARS
                } else {
                    ISAMARU
                };
                let target = view
                    .objects
                    .iter()
                    .find(|o| o.oracle_id == oracle_id && candidates.contains(&o.handle))
                    .ok_or("missing legal library target")?;
                PlayerResponse::Targets {
                    objects: vec![target.handle.clone()],
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let mut required = cost.generic.clone();
                for _ in &cost.colored {
                    required.add(&Quantity::from(1));
                }
                let mut available = Quantity::zero();
                for amount in view.players[view.viewer.0 as usize].mana.values() {
                    available.add(amount);
                }
                if available < required {
                    let source = view
                        .objects
                        .iter()
                        .find(|o| {
                            o.zone == Zone::Battlefield
                                && o.controller == view.viewer
                                && !o.tapped
                                && matches!(o.oracle_id.as_str(), ISLAND | PLAINS)
                        })
                        .ok_or("lifecycle policy lacks payable mana source")?;
                    PlayerResponse::ActivateMana {
                        object: source.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: false }
                }
            }
            PlayerRequest::CommanderReturn {
                destination,
                object,
                designation,
                ..
            } => {
                if !view
                    .commanders
                    .iter()
                    .any(|c| &c.designation == designation)
                {
                    return Err("choice has no public commander designation".into());
                }
                let to_command = if self.policy.is_borrowed() {
                    if *destination != Zone::Exile || !self.choices.is_empty() {
                        return Err("unexpected borrowed commander choice".into());
                    }
                    self.policy == Policy::BorrowedReturn
                } else {
                    match destination {
                        Zone::Library { owner: PlayerId(1) } if self.choices.is_empty() => {
                            self.policy == Policy::TuckReturn
                        }
                        Zone::Hand { owner: PlayerId(1) }
                            if self.choices.len() == 1 && object.is_none() =>
                        {
                            self.policy == Policy::DrawReturn
                        }
                        _ => return Err("unexpected library/draw choice continuation".into()),
                    }
                };
                self.choices.push(observation.clone());
                PlayerResponse::CommanderReturn { to_command }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let attacks =
                    if self.policy.is_damage() && view.players[2].status == PlayerStatus::Live {
                        let source = view.objects.iter().find(|o| {
                            o.oracle_id == YARGLE
                                && o.controller == view.viewer
                                && candidates.contains(&o.handle)
                        });
                        source
                            .map(|o| vec![(o.handle.clone(), PlayerId(2))])
                            .unwrap_or_default()
                    } else {
                        vec![]
                    };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Damage { .. } => {
                return Err("unexpected damage split in lifecycle policy".into());
            }
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
                return Err(format!("lifecycle query rejected: {reason:?}"));
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
        _ => Err("wrong lifecycle observation result".into()),
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
    let (snapshot, state_sha256) = if verifier.is_some() {
        let (snapshot, digest) = game.snapshot_with_digest()?;
        (Some(snapshot), digest)
    } else {
        (None, game.state_digest()?)
    };
    let point = Boundary {
        decisions: game.journal().len(),
        concessions: game.replay_journal().concessions.len(),
        rule_boundary: game.rule_boundary(),
        state_sha256,
    };
    if checkpoints.last() == Some(&point) {
        return Ok(());
    }
    if let Some(verifier) = verifier {
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing lifecycle snapshot")?)?;
        if verifier.verified_digest() != Some(point.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("lifecycle boundary recovery differs".into());
        }
    }
    checkpoints.push(point);
    Ok(())
}
struct Run {
    game: Game,
    checkpoints: Vec<Boundary>,
    choices: Vec<Observation>,
    first_hit: Option<Observation>,
}
fn run(
    catalog: Catalog,
    policy: Policy,
    mode: TraceMode,
    quantum: u64,
    recovery: Recovery,
) -> Result<Run, String> {
    if quantum == 0 {
        return Err("lifecycle runner requires positive budget".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut strategy = Strategy {
        policy,
        spell_cast: false,
        commander_cast: false,
        departed: false,
        choices: vec![],
        first_hit: None,
    };
    let mut checkpoints = vec![];
    let mut verifier =
        (recovery == Recovery::Verify).then(|| SnapshotVerifier::new(prepared, mode));
    if recovery != Recovery::None {
        capture(&game, &mut checkpoints, verifier.as_mut())?;
    }
    for _ in 0..4096 {
        let next = game.advance(Budget::new(quantum));
        if recovery != Recovery::None {
            capture(&game, &mut checkpoints, verifier.as_mut())?;
        }
        match next {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => {
                let observation = observe(&game, request.player(), quantum)?;
                if policy.is_damage()
                    && strategy.first_hit.is_none()
                    && observation.view.commanders.iter().any(|c| {
                        c.history.combat_damage.get(&PlayerId(2)) == Some(&Quantity::from(18))
                    })
                {
                    strategy.first_hit = Some(observation.clone());
                }
                if policy.is_borrowed()
                    && !strategy.departed
                    && observation.view.players[2].status == PlayerStatus::Left
                {
                    game.concede(PlayerId(1))?;
                    strategy.departed = true;
                    if recovery != Recovery::None {
                        capture(&game, &mut checkpoints, verifier.as_mut())?;
                    }
                    continue;
                }
                if strategy.done(&observation) {
                    return Ok(Run {
                        game,
                        checkpoints,
                        choices: strategy.choices,
                        first_hit: strategy.first_hit,
                    });
                }
                let response = strategy.choose(&observation)?;
                let token = observation
                    .decision
                    .as_ref()
                    .ok_or("missing lifecycle token")?
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
                    _ => return Err("wrong lifecycle validation result".into()),
                }
                if recovery != Recovery::None {
                    capture(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
            other => {
                return Err(format!(
                    "lifecycle witness terminated prematurely: {other:?}"
                ));
            }
        }
    }
    Err("lifecycle host budget exhausted".into())
}

fn assert_outcome(run: &Run, policy: Policy) -> Result<(), String> {
    let game = &run.game;
    if policy.is_damage() {
        let first = run
            .first_hit
            .as_ref()
            .ok_or("missing nonlethal first-hit witness")?;
        if first.view.players[2].life != Quantity::from(22)
            || first.view.players[2].status != PlayerStatus::Live
        {
            return Err("18 commander damage incorrectly caused a loss".into());
        }
        let history = game
            .commander_ledger()
            .history(CardId(0))
            .ok_or("missing damage commander")?;
        let observation = observation(game, PlayerId(0))?;
        let losses = game
            .events()
            .iter()
            .filter_map(|e| {
                if let Event::PlayersLost { losses } = e {
                    Some(losses.as_slice())
                } else {
                    None
                }
            })
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if history.combat_damage
            != std::collections::BTreeMap::from([(PlayerId(2), Quantity::from(36))])
            || !history.command_casts.is_empty()
            || observation.view.players[2].life != Quantity::from(4)
            || observation.view.players[2].status != PlayerStatus::Left
            || (game.trace().mode == TraceMode::Detailed
                && losses != vec![(PlayerId(2), PlayerLossReason::CommanderDamage)])
        {
            return Err("Commander21 loss/history/life witness differs".into());
        }
        let expected_zone = match policy {
            Policy::CommanderDamage => Zone::Battlefield,
            Policy::BorrowedReturn => Zone::Command { owner: PlayerId(0) },
            _ => Zone::Exile,
        };
        if game.zone(
            game.object_id(CardId(0))
                .ok_or("missing surviving commander")?,
        ) != Some(expected_zone)
            || game.replay_journal().concessions.len() != usize::from(policy.is_borrowed())
        {
            return Err("damage/departure physical provenance differs".into());
        }
    } else {
        if game.zone(game.object_id(CardId(0)).ok_or("missing sorcery")?)
            != Some(Zone::Graveyard { owner: PlayerId(0) })
        {
            return Err("TimeEbb did not finish in caster graveyard".into());
        }
        if policy == Policy::OrdinaryTuck {
            if game.zone(
                game.object_id(CardId(16))
                    .ok_or("missing ordinary target")?,
            ) != Some(Zone::Hand { owner: PlayerId(2) })
                || !run.choices.is_empty()
            {
                return Err("ordinary creature top/draw witness differs".into());
            }
        } else {
            let history = game
                .commander_ledger()
                .history(CardId(4))
                .ok_or("missing library commander")?;
            let expected_casts = if policy == Policy::DrawHandCast {
                Quantity::zero()
            } else {
                Quantity::from(1)
            };
            let expected_choices = if policy == Policy::TuckReturn { 1 } else { 2 };
            if game.zone(
                game.object_id(CardId(4))
                    .ok_or("missing recast commander")?,
            ) != Some(Zone::Battlefield)
                || history
                    .command_casts
                    .get(&PlayerId(1))
                    .cloned()
                    .unwrap_or_default()
                    != expected_casts
                || run.choices.len() != expected_choices
            {
                return Err("library/draw replacement or recasting witness differs".into());
            }
        }
    }
    Ok(())
}

pub(super) fn suite(
    damage: &Catalog,
    library: &Catalog,
    recover: bool,
) -> Result<(Vec<Evidence>, Vec<Game>, serde_json::Value), String> {
    let mut evidence = vec![];
    let mut games = vec![];
    let mut reports = vec![];
    for policy in POLICIES {
        eprintln!("running damage/library {policy:?}");
        let catalog = policy.catalog(damage, library);
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
            if reference.game.state_digest()? != other.game.state_digest()?
                || reference.game.replay_journal() != other.game.replay_journal()
                || (mode == TraceMode::Compact
                    && reference.game.trace().verification != other.game.trace().verification)
            {
                return Err("damage/library trace or budget partition differs".into());
            }
        }
        let game = reference.game;
        reports.push(serde_json::json!({"policy":policy,"state_sha256":game.state_digest()?,"turn":game.turn(),
            "journal":game.replay_journal(),"events":game.events(),"checkpoints":reference.checkpoints.len(),
            "choices":reference.choices,"first_hit":reference.first_hit,
            "final_observations":(0..4).map(|p|observation(&game,PlayerId(p))).collect::<Result<Vec<_>,_>>()?,
            "commander_ledger":game.commander_ledger()}));
        evidence.push(Evidence {
            config: policy.config(),
            policy,
            journal: game.replay_journal(),
            checkpoints: reference.checkpoints,
            final_snapshot: game.snapshot()?,
        });
        games.push(game);
    }
    if recover {
        verify(damage, library, &evidence)?;
    }
    Ok((evidence, games, serde_json::Value::Array(reports)))
}

pub(super) fn verify(
    damage: &Catalog,
    library: &Catalog,
    evidence: &[Evidence],
) -> Result<(), String> {
    if evidence.len() != POLICIES.len() {
        return Err("missing damage/library witness".into());
    }
    // Reject missing/reordered coverage and forged roots before expensive replay.
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("damage/library policy/config mismatch".into());
        }
        let initial = Game::with_trace(
            Prepared::admit(policy.catalog(damage, library).clone(), policy.config())?,
            TraceMode::Off,
        );
        let expected = Boundary {
            decisions: 0,
            concessions: 0,
            rule_boundary: 0,
            state_sha256: initial.state_digest()?,
        };
        if entry.checkpoints.first() != Some(&expected) {
            return Err("damage/library root recovery witness differs".into());
        }
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("damage/library policy/config mismatch".into());
        }
        let catalog = policy.catalog(damage, library);
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
            return Err("damage/library retained recovery/replay differs".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_coverage_rejects_reordered_policies_missing_boundaries_and_forged_roots() {
        let damage = damage_fixture_catalog().unwrap();
        let library = library_fixture_catalog().unwrap();
        let (evidence, _, _) = suite(&damage, &library, false).unwrap();
        let original = serde_json::to_value(&evidence).unwrap();
        for mutation in 0..3 {
            let mut changed = original.clone();
            match mutation {
                0 => changed.as_array_mut().unwrap().swap(0, 1),
                1 => changed[0]["checkpoints"] = serde_json::json!([]),
                _ => changed[0]["checkpoints"][0]["concessions"] = serde_json::json!(1),
            }
            assert!(
                verify(
                    &damage,
                    &library,
                    &serde_json::from_value::<Vec<Evidence>>(changed).unwrap()
                )
                .is_err()
            );
        }
        assert!(verify(&damage, &library, &evidence[..evidence.len() - 1]).is_err());
    }
}
