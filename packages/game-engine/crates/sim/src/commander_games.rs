//! Bounded Commander policies consume only permitted observations and requests.
//! The host separately retains exact continuations and verifies their replay.
use super::*;
use mtg_contracts::Quantity;
use mtg_core::{PlayerRequest, PlayerResponse};

const COMMANDER: &str = "cfb756dd-6fe1-430a-8f04-8b10cac62a86";
const PLAINS: &str = "bc71ebf6-2056-41f7-be35-b2e5c34afa99";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    TaxKeep,
    TaxUndo,
    HandRecast,
    GraveReturn,
    GraveDecline,
    ExileReturn,
    ExileDecline,
}
const POLICIES: [Policy; 7] = [
    Policy::TaxKeep,
    Policy::TaxUndo,
    Policy::HandRecast,
    Policy::GraveReturn,
    Policy::GraveDecline,
    Policy::ExileReturn,
    Policy::ExileDecline,
];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CommanderEvidence {
    pub policy: Policy,
    pub scenario: ScenarioEvidence,
}

struct Strategy {
    policy: Policy,
    attempts: u8,
    chose_return: bool,
}
impl Policy {
    fn config(self) -> ScenarioConfig {
        match self {
            Self::TaxKeep | Self::TaxUndo | Self::HandRecast => ScenarioConfig::command_casting(),
            Self::GraveReturn | Self::GraveDecline => ScenarioConfig::commander_lifecycle(),
            Self::ExileReturn | Self::ExileDecline => ScenarioConfig::commander_exile(),
        }
    }
    fn returns(self) -> bool {
        !matches!(
            self,
            Self::HandRecast | Self::GraveDecline | Self::ExileDecline
        )
    }
}
impl Strategy {
    fn done(&self, observation: &Observation) -> bool {
        let v = &observation.view;
        if !self.chose_return || !v.stack.is_empty() {
            return false;
        }
        let Some(commander) = v.objects.iter().find(|o| o.oracle_id == COMMANDER) else {
            return false;
        };
        let expected_attempts = match self.policy {
            Policy::TaxKeep | Policy::TaxUndo => 3,
            Policy::HandRecast => 2,
            Policy::GraveReturn | Policy::ExileReturn => 1,
            Policy::GraveDecline | Policy::ExileDecline => 0,
        };
        let expected_zone = match self.policy {
            Policy::GraveDecline => Zone::Graveyard { owner: PlayerId(0) },
            Policy::ExileDecline => Zone::Exile,
            _ => Zone::Battlefield,
        };
        self.attempts == expected_attempts
            && commander.zone == expected_zone
            && matches!(observation.decision, Some(PlayerRequest::Priority { .. }))
    }

    fn choose(&mut self, observation: &Observation) -> Result<PlayerResponse, String> {
        let v = &observation.view;
        let commander = v.objects.iter().find(|o| o.oracle_id == COMMANDER);
        let request = observation
            .decision
            .as_ref()
            .ok_or("missing commander request")?;
        Ok(match request {
            PlayerRequest::Priority { .. } => {
                if v.viewer == PlayerId(0)
                    && v.active == Some(v.viewer)
                    && v.stack.is_empty()
                    && matches!(
                        v.turn.step,
                        TurnStep::PrecombatMain | TurnStep::PostcombatMain
                    )
                    && let Some(commander) = commander
                    && matches!(
                        commander.zone,
                        Zone::Command { owner: PlayerId(0) } | Zone::Hand { owner: PlayerId(0) }
                    )
                {
                    self.attempts += 1;
                    PlayerResponse::Cast {
                        card: commander.handle.clone(),
                    }
                } else if v.viewer == PlayerId(1)
                    && v.stack.is_empty()
                    && !self.chose_return
                    && self.attempts == 1
                    && commander.is_some_and(|o| o.zone == Zone::Battlefield)
                    && let Some(spell) = v.objects.iter().find(|o| {
                        o.oracle_id == UNSUMMON && o.zone == (Zone::Hand { owner: v.viewer })
                    })
                {
                    PlayerResponse::Cast {
                        card: spell.handle.clone(),
                    }
                } else {
                    PlayerResponse::Pass
                }
            }
            PlayerRequest::Mana { cost, .. } => {
                let mut required = cost.generic.clone();
                for _ in &cost.colored {
                    required.add(&Quantity::from(1));
                }
                // Deliberately finish an underfunded taxed cast, then retry it.
                let failing = v.viewer == PlayerId(0)
                    && self.attempts == 2
                    && matches!(self.policy, Policy::TaxKeep | Policy::TaxUndo);
                if failing {
                    required = Quantity::from(1);
                }
                let mut available = Quantity::zero();
                for amount in v.players[v.viewer.0 as usize].mana.values() {
                    available.add(amount);
                }
                if available < required {
                    let land = v
                        .objects
                        .iter()
                        .find(|o| {
                            o.zone == Zone::Battlefield
                                && o.controller == v.viewer
                                && !o.tapped
                                && (o.oracle_id == PLAINS
                                    || o.oracle_id == "b2c6aa39-2d2a-459c-a555-fb48ba993373")
                        })
                        .ok_or("commander policy lacks required source")?;
                    PlayerResponse::ActivateMana {
                        object: land.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana {
                        undo_mana: self.policy == Policy::TaxUndo,
                    }
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let commander = commander.ok_or("missing bounce target")?;
                if !candidates.contains(&commander.handle) {
                    return Err("commander is not a legal bounce target".into());
                }
                PlayerResponse::Targets {
                    objects: vec![commander.handle.clone()],
                }
            }
            PlayerRequest::CommanderReturn { .. } => {
                if self.chose_return {
                    return Err("declined or consumed return choice repeated".into());
                }
                self.chose_return = true;
                PlayerResponse::CommanderReturn {
                    to_command: self.policy.returns(),
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let attacks = if matches!(self.policy, Policy::GraveReturn | Policy::GraveDecline)
                    && !self.chose_return
                    && v.viewer == PlayerId(0)
                {
                    let commander = commander.ok_or("missing lethal-combat commander")?;
                    if !candidates.contains(&commander.handle) {
                        return Err("commander is not attack-ready".into());
                    }
                    vec![(commander.handle.clone(), PlayerId(1))]
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
                let blocks = if v.viewer == PlayerId(1) {
                    vec![(
                        candidates.first().ok_or("missing lethal blocker")?.clone(),
                        attackers.first().ok_or("missing lethal attacker")?.clone(),
                    )]
                } else {
                    vec![]
                };
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage { .. } => {
                return Err("unexpected multi-blocker damage choice".into());
            }
            PlayerRequest::Discard { .. } => {
                return Err("unexpected cleanup in bounded witness".into());
            }
        })
    }
}

fn complete(game: &Game, mut query: QueryResult, quantum: u64) -> Result<QueryValue, String> {
    loop {
        match query {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("commander query rejected: {reason:?}"));
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
        _ => Err("commander observation has wrong query result".into()),
    }
}
fn capture(game: &Game, snapshots: &mut Vec<String>) -> Result<(), String> {
    let snapshot = game.snapshot()?;
    if snapshots.last() != Some(&snapshot) {
        snapshots.push(snapshot);
    }
    Ok(())
}
fn run(
    catalog: Catalog,
    policy: Policy,
    mode: TraceMode,
    quantum: u64,
    save: bool,
) -> Result<(Game, Vec<String>), String> {
    if quantum == 0 {
        return Err("commander runner requires positive budget".into());
    }
    let mut game = Game::with_trace(Prepared::admit(catalog, policy.config())?, mode);
    let mut strategy = Strategy {
        policy,
        attempts: 0,
        chose_return: false,
    };
    let mut checkpoints = vec![];
    if save {
        capture(&game, &mut checkpoints)?;
    }
    if matches!(policy, Policy::ExileReturn | Policy::ExileDecline) {
        game.concede(PlayerId(1))?;
        if save {
            capture(&game, &mut checkpoints)?;
        }
    }
    for _ in 0..1024 {
        let next = game.advance(Budget::new(quantum));
        if save {
            capture(&game, &mut checkpoints)?;
        }
        match next {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => {
                let observation = observe(&game, request.player(), quantum)?;
                if strategy.done(&observation) {
                    return Ok((game, checkpoints));
                }
                let response = strategy.choose(&observation)?;
                let token = observation
                    .decision
                    .as_ref()
                    .ok_or("missing public token")?
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
                    _ => return Err("commander response validation returned wrong result".into()),
                }
                if save {
                    capture(&game, &mut checkpoints)?;
                }
            }
            other => return Err(format!("commander witness stopped prematurely: {other:?}")),
        }
    }
    Err("commander host budget exhausted before witness completion".into())
}

fn assert_outcome(game: &Game, policy: Policy) -> Result<(), String> {
    let commander = game.object_id(CardId(0)).ok_or("lost physical commander")?;
    let expected_zone = match policy {
        Policy::GraveDecline => Zone::Graveyard { owner: PlayerId(0) },
        Policy::ExileDecline => Zone::Exile,
        _ => Zone::Battlefield,
    };
    let expected_casts = match policy {
        Policy::TaxKeep | Policy::TaxUndo => 2,
        Policy::GraveDecline | Policy::ExileDecline => 0,
        _ => 1,
    };
    let history = game
        .commander_ledger()
        .history(CardId(0))
        .ok_or("missing commander ledger")?;
    let actual = history
        .command_casts
        .get(&PlayerId(0))
        .cloned()
        .unwrap_or_default();
    if game.zone(commander) != Some(expected_zone)
        || actual != Quantity::from(expected_casts)
        || game.commander_ledger().tax(CardId(0), PlayerId(0))?
            != Quantity::from(expected_casts * 2)
        || !history.combat_damage.is_empty()
    {
        return Err(format!("unexpected {policy:?} commander zone/history/tax"));
    }
    if matches!(policy, Policy::ExileReturn | Policy::ExileDecline) {
        if game.live_players() != vec![PlayerId(0), PlayerId(2), PlayerId(3)]
            || game.replay_journal().concessions.len() != 1
        {
            return Err("exile witness lost departure provenance".into());
        }
    } else if !game.replay_journal().concessions.is_empty() {
        return Err("ordinary commander witness unexpectedly conceded".into());
    }
    Ok(())
}

pub(super) fn suite(
    catalog: &Catalog,
    recover: bool,
) -> Result<(Vec<CommanderEvidence>, Vec<Game>, serde_json::Value), String> {
    let mut evidence = vec![];
    let mut games = vec![];
    let mut reports = vec![];
    for policy in POLICIES {
        eprintln!("running Commander {policy:?}");
        let (game, checkpoints) = run(catalog.clone(), policy, TraceMode::Detailed, 1, true)?;
        assert_outcome(&game, policy)?;
        for (mode, quantum) in [(TraceMode::Off, u64::MAX), (TraceMode::Compact, 7)] {
            let (other, _) = run(catalog.clone(), policy, mode, quantum, false)?;
            if other.state_digest()? != game.state_digest()?
                || other.replay_journal() != game.replay_journal()
                || (mode == TraceMode::Compact
                    && other.trace().verification != game.trace().verification)
            {
                return Err("commander trace/budget partition changed behavior".into());
            }
        }
        reports.push(serde_json::json!({"policy":policy,"state_sha256":game.state_digest()?,
            "events":game.events(),"journal":game.replay_journal(),"checkpoints":checkpoints.len(),
            "final_observations":(0..4).map(|p|observation(&game,PlayerId(p))).collect::<Result<Vec<_>,_>>()?,
            "commander_ledger":game.commander_ledger()}));
        evidence.push(CommanderEvidence {
            policy,
            scenario: ScenarioEvidence {
                config: policy.config(),
                journal: game.replay_journal(),
                checkpoints,
                final_snapshot: game.snapshot()?,
            },
        });
        games.push(game);
    }
    if recover {
        verify(catalog, &evidence)?;
    }
    Ok((evidence, games, serde_json::Value::Array(reports)))
}

pub(super) fn verify(catalog: &Catalog, evidence: &[CommanderEvidence]) -> Result<(), String> {
    if evidence.len() != POLICIES.len() {
        return Err("incomplete commander evidence".into());
    }
    for (entry, policy) in evidence.iter().zip(POLICIES) {
        let scenario = &entry.scenario;
        if entry.policy != policy || scenario.config != policy.config() {
            return Err("commander evidence policy/config mismatch".into());
        }
        let prepared = Prepared::admit(catalog.clone(), policy.config())?;
        let (reference, checkpoints) = run(catalog.clone(), policy, TraceMode::Detailed, 1, true)?;
        if checkpoints != scenario.checkpoints
            || reference.replay_journal() != scenario.journal
            || reference.snapshot()? != scenario.final_snapshot
        {
            return Err("commander evidence differs from required witness".into());
        }
        let mut verifier = SnapshotVerifier::new(prepared.clone(), TraceMode::Detailed);
        for snapshot in &scenario.checkpoints {
            verifier.verify(snapshot)?;
        }
        let restored = Game::restore(prepared.clone(), &scenario.final_snapshot)?;
        let replay = Game::replay(prepared, &scenario.journal)?;
        if restored.state_digest()? != replay.state_digest()? || restored.trace() != replay.trace()
        {
            return Err("commander retained replay differs".into());
        }
        assert_outcome(&restored, policy)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_commander_evidence_rejects_missing_or_changed_witnesses() {
        let catalog = commander_fixture_catalog().unwrap();
        let (evidence, _, _) = suite(&catalog, false).unwrap();
        verify(&catalog, &evidence).unwrap();
        let original = serde_json::to_value(&evidence).unwrap();
        let mut changed = original.clone();
        changed[0]["policy"] = serde_json::json!("tax_undo");
        assert!(
            verify(
                &catalog,
                &serde_json::from_value::<Vec<CommanderEvidence>>(changed).unwrap()
            )
            .is_err()
        );
        let mut changed = original.clone();
        changed[0]["scenario"]["checkpoints"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(
            verify(
                &catalog,
                &serde_json::from_value::<Vec<CommanderEvidence>>(changed).unwrap()
            )
            .is_err()
        );
        let mut changed = original;
        changed.as_array_mut().unwrap().pop();
        assert!(
            verify(
                &catalog,
                &serde_json::from_value::<Vec<CommanderEvidence>>(changed).unwrap()
            )
            .is_err()
        );
    }
}
