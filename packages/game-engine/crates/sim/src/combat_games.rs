//! Small deterministic policies. Policy::choose receives only a player's view.
//! Trusted replay/recovery evidence is collected separately by the host below.
use super::*;
use mtg_contracts::Quantity;
use mtg_core::{Event, PlayerLossReason, PlayerRequest, PlayerResponse, SnapshotVerifier};

const BEARS: &str = "14c8f55d-d177-4c25-a931-ebeb9e6062a0";
const FOREST: &str = "b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6";
const UNSUMMON: &str = "837182db-1bf3-4a2c-bd01-1af9d9873561";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    Integrated,
    EmptyAttacks,
    SplitEvenly,
    SplitConcentrated,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RunResult {
    Won { winner: PlayerId },
    Draw,
    InteractionReached,
    HostBudgetExhausted,
    Fault { reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BoundaryEvidence {
    decisions: usize,
    boundary: u64,
    state_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CombatEvidence {
    pub config: ScenarioConfig,
    pub policy: Policy,
    pub result: RunResult,
    pub journal: ReplayJournal,
    pub checkpoints: Vec<BoundaryEvidence>,
    pub final_snapshot: String,
}

impl Policy {
    fn config(self) -> ScenarioConfig {
        match self {
            Self::Integrated => ScenarioConfig::combat_game(),
            _ => ScenarioConfig::combat_interactions(),
        }
    }
    /// No Game, raw ID, snapshot, library order or opponent private state enters
    /// this strategy function. Names are source-verified policy preferences;
    /// authoritative rules dispatch remains entirely in the core.
    fn choose(self, observation: &Observation) -> Result<PlayerResponse, String> {
        let v = &observation.view;
        let request = observation
            .decision
            .as_ref()
            .ok_or("policy lacks request")?;
        let own_hand = |id: &str| {
            v.objects
                .iter()
                .find(|o| o.zone == (Zone::Hand { owner: v.viewer }) && o.oracle_id == id)
        };
        Ok(match request {
            PlayerRequest::Priority { .. } => {
                if self == Self::Integrated {
                    let initial_cast_turn = (v.turn.number == 1 && v.viewer == PlayerId(0))
                        || (v.turn.number == 3 && v.viewer == PlayerId(2));
                    if initial_cast_turn
                        && v.turn.step == TurnStep::PrecombatMain
                        && v.active == Some(v.viewer)
                        && v.stack.is_empty()
                    {
                        if v.turn.lands_played == 0 {
                            if let Some(land) = own_hand(FOREST) {
                                return Ok(PlayerResponse::PlayLand {
                                    object: land.handle.clone(),
                                });
                            }
                        } else if let Some(creature) = own_hand(BEARS) {
                            return Ok(PlayerResponse::Cast {
                                card: creature.handle.clone(),
                            });
                        }
                    }
                    if v.turn.number == 1 && v.turn.step == TurnStep::DeclareBlockers {
                        let self_bounce = v.viewer == PlayerId(1) && v.stack.is_empty();
                        let response = v.viewer == PlayerId(3)
                            && v.stack.len() == 1
                            && v.stack[0].controller == PlayerId(1)
                            && v.stack[0].cast_complete;
                        if (self_bounce || response)
                            && let Some(spell) = own_hand(UNSUMMON)
                        {
                            return Ok(PlayerResponse::Cast {
                                card: spell.handle.clone(),
                            });
                        }
                    }
                }
                PlayerResponse::Pass
            }
            PlayerRequest::Mana { cost, .. } => {
                let pool = &v.players[v.viewer.0 as usize].mana;
                let mut required = cost.generic.clone();
                for _ in &cost.colored {
                    required.add(&Quantity::from(1));
                }
                let mut available = Quantity::zero();
                for amount in pool.values() {
                    available.add(amount);
                }
                if available < required
                    && let Some(land) = v.objects.iter().find(|o| {
                        o.zone == Zone::Battlefield
                            && o.controller == v.viewer
                            && !o.tapped
                            && o.oracle_id != BEARS
                    })
                {
                    PlayerResponse::ActivateMana {
                        object: land.handle.clone(),
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: true }
                }
            }
            PlayerRequest::Targets { candidates, .. } => {
                let creature = v
                    .objects
                    .iter()
                    .find(|o| {
                        o.controller == PlayerId(1)
                            && o.oracle_id == BEARS
                            && candidates.contains(&o.handle)
                    })
                    .ok_or("policy lost frozen bounce target")?;
                PlayerResponse::Targets {
                    objects: vec![creature.handle.clone()],
                }
            }
            PlayerRequest::Attackers { candidates, .. } => {
                let attacks = match self {
                    Self::EmptyAttacks => vec![],
                    Self::Integrated => {
                        let defender = match (v.turn.number, v.viewer.0) {
                            (1 | 5, 0) => Some(PlayerId(1)),
                            (3 | 6, 2) => Some(PlayerId(0)),
                            (8, 0) => Some(PlayerId(2)),
                            _ => None,
                        };
                        if let Some(defender) = defender {
                            vec![(
                                candidates.first().ok_or("missing frozen attacker")?.clone(),
                                defender,
                            )]
                        } else {
                            vec![]
                        }
                    }
                    Self::SplitEvenly | Self::SplitConcentrated => {
                        if candidates.len() != 3 {
                            return Err("expected three interaction attackers".into());
                        }
                        candidates
                            .iter()
                            .enumerate()
                            .map(|(n, h)| (h.clone(), PlayerId(if n < 2 { 1 } else { 2 })))
                            .collect()
                    }
                };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers {
                candidates,
                attackers,
                ..
            } => {
                let blocks = match self {
                    Self::Integrated
                        if (v.turn.number == 1 && v.viewer == PlayerId(1))
                            || (v.turn.number == 3 && v.viewer == PlayerId(0)) =>
                    {
                        vec![(
                            candidates.first().ok_or("missing frozen blocker")?.clone(),
                            attackers.first().ok_or("missing blocked attacker")?.clone(),
                        )]
                    }
                    Self::SplitEvenly | Self::SplitConcentrated if v.viewer == PlayerId(1) => {
                        let attacker = attackers.first().ok_or("missing double-block attacker")?;
                        candidates
                            .iter()
                            .map(|h| (h.clone(), attacker.clone()))
                            .collect()
                    }
                    _ => vec![],
                };
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage {
                recipients, power, ..
            } => {
                let assignments = recipients
                    .iter()
                    .enumerate()
                    .map(|(n, h)| {
                        let amount = if self == Self::SplitEvenly {
                            Quantity::from(1)
                        } else if n == 0 {
                            power.clone()
                        } else {
                            Quantity::zero()
                        };
                        (h.clone(), amount)
                    })
                    .collect();
                PlayerResponse::Damage { assignments }
            }
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().take(*count).cloned().collect(),
            },
        })
    }
}

fn complete_query(
    game: &Game,
    mut result: QueryResult,
    quantum: u64,
) -> Result<QueryValue, String> {
    loop {
        result = match result {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Rejected(reason) => {
                return Err(format!("policy query rejected: {reason:?}"));
            }
            QueryResult::Incomplete { work, .. } => game.resume_work(work, Budget::new(quantum)),
        };
    }
}
fn observe_budgeted(game: &Game, player: PlayerId, quantum: u64) -> Result<Observation, String> {
    match complete_query(game, game.observe(player, Budget::new(quantum)), quantum)? {
        QueryValue::Observation(observation) => Ok(observation),
        _ => Err("wrong observation query result".into()),
    }
}
fn send(
    game: &mut Game,
    observation: &Observation,
    response: PlayerResponse,
    quantum: u64,
) -> Result<(), String> {
    let request = observation
        .decision
        .as_ref()
        .ok_or("missing public decision")?;
    match complete_query(
        game,
        game.check_player_response(
            observation.view.viewer,
            request.token().clone(),
            response,
            Budget::new(quantum),
        ),
        quantum,
    )? {
        QueryValue::Validated(receipt) => game.submit(receipt),
        _ => Err("wrong response validation result".into()),
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Recovery {
    None,
    Capture,
    Verify,
}

fn retain_boundary(
    game: &Game,
    saved: &mut Vec<BoundaryEvidence>,
    verifier: Option<&mut SnapshotVerifier>,
) -> Result<(), String> {
    // Capture retains scalar witnesses only. Verification additionally exercises
    // JSON recovery at every boundary, with one independent incremental replay.
    let (snapshot, state_sha256) = if verifier.is_some() {
        let (snapshot, digest) = game.snapshot_with_digest()?;
        (Some(snapshot), digest)
    } else {
        (None, game.state_digest()?)
    };
    let evidence = BoundaryEvidence {
        decisions: game.journal().len(),
        boundary: game.rule_boundary(),
        state_sha256,
    };
    if saved.last() == Some(&evidence) {
        return Ok(());
    }
    if let Some(verifier) = verifier {
        let restored = verifier.verify(snapshot.as_deref().ok_or("missing recovery snapshot")?)?;
        if verifier.verified_digest() != Some(evidence.state_sha256.as_str())
            || restored.trace() != game.trace()
        {
            return Err("combat boundary recovery differs".into());
        }
    }
    saved.push(evidence);
    Ok(())
}

pub(super) fn run(
    catalog: Catalog,
    policy: Policy,
    mode: TraceMode,
    quantum: u64,
    host_steps: usize,
    recovery: Recovery,
) -> Result<(Game, RunResult, Vec<BoundaryEvidence>), String> {
    if quantum == 0 {
        return Err("runner quantum must permit progress".into());
    }
    let prepared = Prepared::admit(catalog, policy.config())?;
    let mut game = Game::with_trace(prepared.clone(), mode);
    let mut checkpoints = vec![];
    let mut verifier =
        (recovery == Recovery::Verify).then(|| SnapshotVerifier::new(prepared.clone(), mode));
    if recovery != Recovery::None {
        retain_boundary(&game, &mut checkpoints, verifier.as_mut())?;
    }
    for _ in 0..host_steps {
        let result = game.advance(Budget::new(quantum));
        if recovery != Recovery::None {
            retain_boundary(&game, &mut checkpoints, verifier.as_mut())?;
        }
        match result {
            Advance::Finished { winner } => {
                return Ok((game, RunResult::Won { winner }, checkpoints));
            }
            Advance::Draw => return Ok((game, RunResult::Draw, checkpoints)),
            Advance::Fault(reason) => return Ok((game, RunResult::Fault { reason }, checkpoints)),
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => {
                let observation = observe_budgeted(&game, request.player(), quantum)?;
                if matches!(policy, Policy::SplitEvenly | Policy::SplitConcentrated)
                    && observation.view.live_turn_order == vec![PlayerId(0), PlayerId(3)]
                {
                    return Ok((game, RunResult::InteractionReached, checkpoints));
                }
                let response = policy.choose(&observation)?;
                send(&mut game, &observation, response, quantum)?;
                if recovery != Recovery::None {
                    retain_boundary(&game, &mut checkpoints, verifier.as_mut())?;
                }
            }
        }
    }
    Ok((game, RunResult::HostBudgetExhausted, checkpoints))
}

fn assert_outcome(game: &Game, policy: Policy, result: &RunResult) -> Result<(), String> {
    let expected = match policy {
        Policy::Integrated | Policy::EmptyAttacks => RunResult::Won {
            winner: PlayerId(0),
        },
        _ => RunResult::InteractionReached,
    };
    if *result != expected {
        return Err(format!(
            "unexpected {policy:?} outcome: {result:?}; turn {:?}; last input {:?}",
            game.turn(),
            game.journal().last()
        ));
    }
    let losses = game
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::PlayersLost { losses } => Some(losses.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let expected_losses = match policy {
        Policy::Integrated => vec![
            (PlayerId(1), PlayerLossReason::Life),
            (PlayerId(3), PlayerLossReason::FailedDraw),
            (PlayerId(2), PlayerLossReason::Life),
        ],
        Policy::EmptyAttacks => (1..4)
            .map(|n| (PlayerId(n), PlayerLossReason::FailedDraw))
            .collect(),
        _ => vec![
            (PlayerId(1), PlayerLossReason::Life),
            (PlayerId(2), PlayerLossReason::Life),
        ],
    };
    if losses != expected_losses {
        return Err("scenario elimination causes/order differ".into());
    }
    let observation = observation(game, PlayerId(0))?;
    let view = &observation.view;
    if policy == Policy::Integrated {
        if view.turn.number != 8
            || view.players[0].life != Quantity::from(4)
            || view.players[0].library_count != 1
            || !game.replay_journal().concessions.is_empty()
        {
            return Err("integrated final turn/life/library/concession mismatch".into());
        }
        if !game.journal().iter().any(|input| matches!(&input.response,
            Response::Discard { objects } if input.player == PlayerId(3) && objects.len() == 1 && objects[0].card == CardId(19)))
        { return Err("frozen cleanup discard differs".into()); }
        for id in [4, 6] {
            if game.zone(game.object_id(CardId(id)).ok_or("missing drawn Forest")?)
                != Some(Zone::Hand { owner: PlayerId(0) })
            {
                return Err("ordered-library draw witness differs".into());
            }
        }
        if game.zone(
            game.object_id(CardId(5))
                .ok_or("missing remaining Forest")?,
        ) != Some(Zone::Library { owner: PlayerId(0) })
        {
            return Err("wrong remaining ordered-library card".into());
        }
    }
    if policy == Policy::EmptyAttacks && view.turn.number != 4 {
        return Err("empty-library game must finish on turn4".into());
    }
    if matches!(policy, Policy::SplitEvenly | Policy::SplitConcentrated)
        && (game.live_players() != vec![PlayerId(0), PlayerId(3)]
            || game.zone(game.object_id(CardId(0)).ok_or("missing dead attacker")?)
                != Some(Zone::Graveyard { owner: PlayerId(0) }))
    {
        return Err("interaction simultaneous SBA outcome mismatch".into());
    }
    Ok(())
}

pub(super) fn suite(
    catalog: &Catalog,
    recover: bool,
) -> Result<(Vec<CombatEvidence>, Vec<Game>, serde_json::Value), String> {
    let mut evidence = vec![];
    let mut games = vec![];
    let mut reports = vec![];
    for policy in [
        Policy::Integrated,
        Policy::EmptyAttacks,
        Policy::SplitEvenly,
        Policy::SplitConcentrated,
    ] {
        eprintln!("running {policy:?}: detailed witness and trace/budget equivalence");
        let (game, result, checkpoints) = run(
            catalog.clone(),
            policy,
            TraceMode::Detailed,
            1,
            4096,
            Recovery::Capture,
        )?;
        assert_outcome(&game, policy, &result)?;
        for (mode, quantum) in [(TraceMode::Off, u64::MAX), (TraceMode::Compact, 7)] {
            let (other, other_result, _) =
                run(catalog.clone(), policy, mode, quantum, 4096, Recovery::None)?;
            if other_result != result
                || other.state_digest()? != game.state_digest()?
                || other.replay_journal() != game.replay_journal()
                || (mode == TraceMode::Compact
                    && other.trace().verification != game.trace().verification)
            {
                return Err("combat trace/budget/query partition changed outcome".into());
            }
        }
        reports.push(serde_json::json!({"policy":policy,"result":result,"turn":game.turn(),
            "state_sha256":game.state_digest()?,"events":game.events(),"journal":game.replay_journal(),
            "checkpoints":checkpoints.len(),"final_observations":(0..4).map(|p|observation(&game,PlayerId(p))).collect::<Result<Vec<_>,_>>()?}));
        evidence.push(CombatEvidence {
            config: game.config().clone(),
            policy,
            result,
            journal: game.replay_journal(),
            checkpoints,
            final_snapshot: game.snapshot()?,
        });
        games.push(game);
    }
    if recover {
        verify(catalog, &evidence)?;
    }
    Ok((evidence, games, serde_json::Value::Array(reports)))
}

pub(super) fn verify(catalog: &Catalog, evidence: &[CombatEvidence]) -> Result<(), String> {
    let policies = [
        Policy::Integrated,
        Policy::EmptyAttacks,
        Policy::SplitEvenly,
        Policy::SplitConcentrated,
    ];
    if evidence.len() != policies.len() {
        return Err("missing combat suite evidence".into());
    }
    for (entry, policy) in evidence.iter().zip(policies) {
        if entry.policy != policy || entry.config != policy.config() {
            return Err("combat policy/config mismatch".into());
        }
        eprintln!(
            "verifying {policy:?}: {} semantic recovery boundaries and detailed replay",
            entry.checkpoints.len()
        );
        // Diagnostic histories are nonsemantic. Restore every exact continuation
        // without repeatedly encoding its growing Detailed trace; independently
        // restore and replay the entire Detailed final trace below. The saved
        // boundary sequence was captured from the Detailed execution.
        let (game, result, checkpoints) = run(
            catalog.clone(),
            policy,
            TraceMode::Off,
            1,
            4096,
            Recovery::Verify,
        )?;
        let restored = Game::restore(
            Prepared::admit(catalog.clone(), entry.config.clone())?,
            &entry.final_snapshot,
        )?;
        let replay = Game::replay(
            Prepared::admit(catalog.clone(), entry.config.clone())?,
            &entry.journal,
        )?;
        assert_outcome(&replay, policy, &result)?;
        if result != entry.result
            || checkpoints != entry.checkpoints
            || game.replay_journal() != entry.journal
            || game.state_digest()? != restored.state_digest()?
            || restored.trace().mode != TraceMode::Detailed
            || replay.trace() != restored.trace()
            || replay.state_digest()? != game.state_digest()?
        {
            return Err("combat retained checkpoint/journal/outcome mismatch".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_budget_exhaustion_is_not_a_rules_outcome() {
        let (game, result, checkpoints) = run(
            fixture_catalog().unwrap(),
            Policy::Integrated,
            TraceMode::Off,
            1,
            0,
            Recovery::None,
        )
        .unwrap();
        assert_eq!(result, RunResult::HostBudgetExhausted);
        assert_eq!(game.live_players().len(), 4);
        assert!(game.journal().is_empty());
        assert!(checkpoints.is_empty());
        assert!(
            run(
                fixture_catalog().unwrap(),
                Policy::Integrated,
                TraceMode::Off,
                0,
                1,
                Recovery::None
            )
            .is_err()
        );
    }

    #[test]
    fn frozen_policies_finish_real_games_and_distinct_interactions() {
        let (_, games, report) = suite(&fixture_catalog().unwrap(), false).unwrap();
        assert_eq!(games.len(), 4);
        assert_eq!(
            report[0]["result"],
            serde_json::json!({"result":"won","winner":0})
        );
        assert_eq!(report[0]["turn"]["number"], 8);
        assert_eq!(
            report[1]["result"],
            serde_json::json!({"result":"won","winner":0})
        );
        assert_eq!(report[1]["turn"]["number"], 4);
        assert_eq!(
            report[2]["result"],
            serde_json::json!({"result":"interaction_reached"})
        );
        assert_eq!(report[3]["result"], report[2]["result"]);
        assert_ne!(games[2].trace(), games[3].trace());
    }
}
