//! Released-profile policies read only the acting player's permitted observation
//! and the public compiled catalog. Trusted host evidence is collected separately.
use super::*;
use mtg_contracts::{CardKind, Color, ManaCost, Quantity};
use mtg_core::{MatchConfig, ObjectHandle, PlayerRequest, PlayerResponse, PlayerTargetEntity};
use std::collections::BTreeMap;

#[cfg(test)]
mod current_pod_throughput;
mod generic_policy;
mod interactions;
mod mana_policy;
#[cfg(test)]
mod ordinary_diagnostic;

// Evidence belongs outside native crate sources regardless of Cargo's test cwd.
fn artifact_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("sim crate workspace layout")
        .join("generated")
}

const DECISION_LIMIT: usize = 100_000;
const TURN_LIMIT: u64 = 1_000;
const WORK_LIMIT: usize = 2_000_000;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Policy {
    Develop,
    DrawOnly,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    pub decisions: usize,
    pub rule_boundary: u64,
    pub snapshot: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Evidence {
    pub config: MatchConfig,
    pub policy: Policy,
    pub mulligans: [u8; 4],
    pub journal: ReplayJournal,
    pub trace_mode: TraceMode,
    pub losses: Vec<(PlayerId, mtg_core::PlayerLossReason)>,
    pub checkpoints: Vec<Checkpoint>,
    pub outcome: String,
    pub winner: Option<PlayerId>,
    pub decisions: usize,
    pub advances: usize,
    pub rule_steps: u64,
    pub query_calls: usize,
    pub work_limit: usize,
    pub quantum: u64,
    pub turns: u64,
    pub final_state_sha256: String,
    pub surviving_seats: Vec<PlayerId>,
    pub survivor_counts: Vec<usize>,
    pub elapsed_millis: u128,
}
pub(crate) fn spend(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<BTreeMap<Color, Quantity>> {
    let mut free = available.clone();
    let mut used = BTreeMap::<Color, Quantity>::new();
    for c in &cost.colored {
        let n = free.get_mut(c)?;
        if *n < 1.into() {
            return None;
        }
        n.subtract(&1.into());
        used.entry(*c).or_default().add(&1.into());
    }
    let mut rest = cost.generic.clone();
    for c in [Color::B, Color::G, Color::W, Color::U, Color::R, Color::C] {
        let n = std::cmp::min(free.get(&c).cloned().unwrap_or_default(), rest.clone());
        if n > Quantity::zero() {
            rest.subtract(&n);
            used.entry(c).or_default().add(&n);
        }
    }
    (rest == Quantity::zero()).then_some(used)
}
fn definition<'a>(catalog: &'a Catalog, id: &str) -> Result<&'a mtg_contracts::Definition, String> {
    catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == id)
        .ok_or_else(|| format!("public definition missing {id}"))
}
struct Strategy<'a> {
    catalog: &'a Catalog,
    policy: Policy,
    mulligans: [u8; 4],
    mana_plan: std::collections::VecDeque<mana_policy::PlannedMana>,
    mana_color: Option<Color>,
}
impl Strategy<'_> {
    fn choose(&mut self, o: &Observation) -> Result<PlayerResponse, String> {
        let v = &o.view;
        let request = o.decision.as_ref().ok_or("missing match request")?;
        Ok(match request {
            PlayerRequest::AnnounceX { .. } => PlayerResponse::AnnounceX {
                value: self
                    .top_program(v)
                    .and_then(|(d, p)| self.choose_x(v, d, p))
                    .unwrap_or_default(),
            },
            PlayerRequest::ManaColor { colors, .. } => PlayerResponse::ManaColor {
                color: self
                    .mana_color
                    .take()
                    .filter(|c| colors.contains(c))
                    .or_else(|| colors.first().copied())
                    .ok_or("empty mana colors")?,
            },
            PlayerRequest::SearchLands {
                maximum,
                candidates,
                ..
            } => PlayerResponse::SearchLands {
                objects: candidates
                    .iter()
                    .take(*maximum as usize)
                    .map(|c| c.handle.clone())
                    .collect(),
            },
            PlayerRequest::SacrificeCost { candidates, .. } => PlayerResponse::SacrificeCost {
                object: self.sacrifice_choice(v, candidates)?,
            },
            PlayerRequest::ForcedSacrifice { candidates, .. } => PlayerResponse::ForcedSacrifice {
                object: self.sacrifice_choice(v, candidates)?,
            },
            PlayerRequest::OptionalEffect { .. } => PlayerResponse::OptionalEffect {
                perform: self.optional_effect(v),
            },
            PlayerRequest::OptionalRecursion { .. } => {
                PlayerResponse::OptionalRecursion { perform: true }
            }
            PlayerRequest::Mulligan {
                mulligans_taken,
                can_mulligan,
                ..
            } => PlayerResponse::Mulligan {
                take: *can_mulligan && *mulligans_taken < self.mulligans[usize::from(v.viewer.0)],
            },
            PlayerRequest::BottomOpening {
                count, candidates, ..
            } => PlayerResponse::BottomOpening {
                objects: candidates.iter().rev().take(*count).cloned().collect(),
            },
            PlayerRequest::Priority { .. } => return self.priority(o),
            PlayerRequest::Modes { candidates, .. } => PlayerResponse::Modes {
                mode: self.choose_mode(v, candidates).ok_or("empty modes")?,
            },
            PlayerRequest::Targets {
                occurrences,
                candidates,
                ..
            } => {
                let (d, p) = self.top_program(v).ok_or("missing target program")?;
                let objects = occurrences
                    .iter()
                    .map(|t| {
                        self.object_target(v, d, p, t, Some(candidates))
                            .or_else(|| candidates.first().cloned())
                            .ok_or("empty object targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::Targets { objects }
            }
            PlayerRequest::PlayerTargets {
                occurrences,
                candidates,
                ..
            } => {
                let (d, p) = self.top_program(v).ok_or("missing player program")?;
                let players = occurrences
                    .iter()
                    .map(|t| {
                        self.player_target(v, candidates, self.target_beneficial(d, p, t.slot))
                            .ok_or("empty player targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::PlayerTargets { players }
            }
            PlayerRequest::EntityTargets {
                occurrences,
                candidates,
                ..
            } => {
                let (_, p) = self.top_program(v).ok_or("missing damage program")?;
                let mut entities: Vec<PlayerTargetEntity> = vec![];
                for _ in occurrences {
                    let distinct: Vec<_> = candidates
                        .iter()
                        .filter(|c| !entities.contains(*c))
                        .cloned()
                        .collect();
                    entities.push(
                        self.damage_target(v, p, &distinct)
                            .or_else(|| self.damage_target(v, p, candidates))
                            .ok_or("empty entity targets")?,
                    );
                }
                PlayerResponse::EntityTargets { entities }
            }
            PlayerRequest::StackTargets {
                occurrences,
                candidates,
                ..
            } => {
                let entries = occurrences
                    .iter()
                    .map(|t| {
                        self.stack_target(v, t, Some(candidates))
                            .or_else(|| candidates.first().cloned())
                            .ok_or("empty stack targets".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                PlayerResponse::StackTargets { entries }
            }

            PlayerRequest::OrderTriggers { candidates, .. } => PlayerResponse::OrderTriggers {
                order: candidates.iter().map(|c| c.handle.clone()).collect(),
            },
            PlayerRequest::Mana { cost, .. } => {
                let available = &v
                    .players
                    .iter()
                    .find(|p| p.player == v.viewer)
                    .ok_or("missing mana payer")?
                    .mana;
                if spend(cost, available).is_some() {
                    PlayerResponse::FinishMana { undo_mana: true }
                } else if let Some(next) = self.mana_plan.pop_front() {
                    self.mana_color = next.color;
                    PlayerResponse::ActivateMana {
                        object: next.source,
                    }
                } else {
                    PlayerResponse::FinishMana { undo_mana: true }
                }
            }

            PlayerRequest::SpendMana {
                cost, available, ..
            } => PlayerResponse::SpendMana {
                spending: spend(cost, available).ok_or("cannot spend announced cost")?,
            },
            PlayerRequest::CommanderReturn { .. } => {
                PlayerResponse::CommanderReturn { to_command: true }
            }
            PlayerRequest::Attackers {
                candidates,
                defenders,
                ..
            } => {
                let defender = defenders
                    .iter()
                    .copied()
                    .min_by_key(|p| (p.0 + 4 - v.viewer.0) % 4);
                let attacks = if self.policy == Policy::DrawOnly {
                    vec![]
                } else {
                    candidates
                        .iter()
                        .filter(|h| {
                            v.objects.iter().any(|x| {
                                &x.handle == *h
                                    && !self.reserve_reactive_tap(x)
                                    && x.characteristics
                                        .as_ref()
                                        .and_then(|c| c.power.as_ref())
                                        .is_some_and(|p| p > &Quantity::zero())
                            })
                        })
                        .filter_map(|h| defender.map(|p| (h.clone(), p)))
                        .collect()
                };
                PlayerResponse::Attackers { attacks }
            }
            PlayerRequest::Blockers {
                legal_attackers,
                menace_attackers,
                ..
            } => {
                let mut blocks = vec![];
                let mut blocked = Vec::new();
                if self.policy == Policy::Develop {
                    for hint in legal_attackers {
                        let blocker = v
                            .objects
                            .iter()
                            .find(|o| o.handle == hint.blocker)
                            .ok_or("missing public blocker")?;
                        let bd = definition(self.catalog, &blocker.oracle_id).ok();
                        let bd_keywords = { Some(blocker.keywords.as_slice()) }
                            .ok_or("missing public blocker keywords")?;
                        let toughness = blocker
                            .characteristics
                            .as_ref()
                            .or_else(|| bd.map(|d| &d.characteristics))
                            .ok_or("missing public blocker characteristics")?
                            .toughness
                            .as_ref()
                            .ok_or("missing blocker toughness")?;
                        for attacker in &hint.attackers {
                            if blocked.contains(attacker) {
                                continue;
                            }
                            let a = v
                                .objects
                                .iter()
                                .find(|o| &o.handle == attacker)
                                .ok_or("missing public attacker")?;
                            let ad = definition(self.catalog, &a.oracle_id).ok();
                            let ad_keywords = { Some(a.keywords.as_slice()) }
                                .ok_or("missing public attacker keywords")?;
                            let power = a
                                .characteristics
                                .as_ref()
                                .or_else(|| ad.map(|d| &d.characteristics))
                                .ok_or("missing public attacker characteristics")?
                                .power
                                .clone()
                                .unwrap_or_default()
                                .max(Quantity::zero());
                            let mut marked = blocker.marked_damage.clone();
                            marked.add(&power);
                            let survives = marked < *toughness
                                && (power == Quantity::zero()
                                    || !ad_keywords
                                        .contains(&mtg_contracts::KeywordAbility::Deathtouch));
                            if survives
                                || bd_keywords.contains(&mtg_contracts::KeywordAbility::Deathtouch)
                            {
                                blocks.push((hint.blocker.clone(), attacker.clone()));
                                blocked.push(attacker.clone());
                                break;
                            }
                        }
                    }
                }
                // CR702.111b: complete a menace pair with a remaining legal
                // blocker, or withdraw the singleton. Observation/catalog only.
                for attacker in menace_attackers {
                    if blocks.iter().filter(|(_, a)| a == attacker).count() == 1 {
                        if let Some(hint) = legal_attackers.iter().find(|h| {
                            h.attackers.contains(attacker)
                                && !blocks.iter().any(|(b, _)| b == &h.blocker)
                        }) {
                            blocks.push((hint.blocker.clone(), attacker.clone()));
                        } else {
                            blocks.retain(|(_, a)| a != attacker);
                        }
                    }
                }
                PlayerResponse::Blockers { blocks }
            }
            PlayerRequest::Damage {
                power,
                recipients,
                player_recipient: Some(player),
                lethal_requirements,
                ..
            } => {
                let mut remaining = power.clone();
                let mut assignments = vec![];
                for h in recipients {
                    let lethal = lethal_requirements
                        .iter()
                        .find(|(o, _)| o == h)
                        .ok_or("missing public lethal requirement")?
                        .1
                        .clone();
                    let amount = lethal.min(remaining.clone());
                    remaining.subtract(&amount);
                    assignments.push((h.clone(), amount));
                }
                PlayerResponse::TrampleDamage {
                    assignments,
                    player: *player,
                    amount: remaining,
                }
            }
            PlayerRequest::Damage {
                power, recipients, ..
            } => PlayerResponse::Damage {
                assignments: recipients
                    .iter()
                    .enumerate()
                    .map(|(i, h)| {
                        (
                            h.clone(),
                            if i == 0 {
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
            }
            | PlayerRequest::ResolveDiscard {
                count, candidates, ..
            } => {
                let objects: Vec<ObjectHandle> = candidates.iter().take(*count).cloned().collect();
                if matches!(request, PlayerRequest::Discard { .. }) {
                    PlayerResponse::Discard { objects }
                } else {
                    PlayerResponse::ResolveDiscard { objects }
                }
            }
            PlayerRequest::Sacrifice { candidates, .. } => PlayerResponse::Sacrifice {
                object: { self.sacrifice_choice(v, candidates)? },
            },
            PlayerRequest::DamageEffect { candidates, .. } => {
                let c = candidates.first().ok_or("empty damage effects")?;
                PlayerResponse::DamageEffect {
                    effect: c.effect.clone(),
                    damage: c.damage.clone(),
                }
            }
            PlayerRequest::DamageAllocation {
                required,
                candidates,
                ..
            } => {
                let mut rest = required.clone();
                let allocations = candidates
                    .iter()
                    .map(|c| {
                        let n = std::cmp::min(c.amount.clone(), rest.clone());
                        rest.subtract(&n);
                        (c.damage.clone(), n)
                    })
                    .collect();
                if rest != Quantity::zero() {
                    return Err("insufficient allocation candidates".into());
                }
                PlayerResponse::DamageAllocation { allocations }
            }
        })
    }
}
fn query_call(
    call: impl FnOnce() -> QueryResult,
    work: &mut usize,
    queries: &mut usize,
) -> Result<QueryResult, String> {
    if *work >= WORK_LIMIT {
        return Err("shared host work limit".into());
    }
    *work += 1;
    *queries += 1;
    Ok(call())
}
fn complete(
    game: &Game,
    initial: impl FnOnce() -> QueryResult,
    quantum: u64,
    work: &mut usize,
    queries: &mut usize,
) -> Result<QueryValue, String> {
    let mut q = query_call(initial, work, queries)?;
    loop {
        match q {
            QueryResult::Complete(v) => return Ok(v),
            QueryResult::Rejected(r) => return Err(format!("match query rejected: {r:?}")),
            QueryResult::Incomplete { work: pending, .. } => {
                q = query_call(
                    || game.resume_work(pending, Budget::new(quantum)),
                    work,
                    queries,
                )?
            }
        }
    }
}
fn observe(
    game: &Game,
    player: PlayerId,
    quantum: u64,
    work: &mut usize,
    queries: &mut usize,
) -> Result<Observation, String> {
    match complete(
        game,
        || game.observe(player, Budget::new(quantum)),
        quantum,
        work,
        queries,
    )? {
        QueryValue::Observation(o) => Ok(o),
        _ => Err("wrong match observation type".into()),
    }
}
struct LiveJournal {
    file: fs::File,
}
impl LiveJournal {
    fn new(config: &MatchConfig, policy: Policy, mulligans: [u8; 4]) -> Result<Self, String> {
        let parent = artifact_root().join("p5-live-journals");
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        for index in 0..1000 {
            let path = parent.join(format!("run-{}-{index}.jsonl", std::process::id()));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    let mut out = Self { file };
                    out.append(&serde_json::json!({"kind":"header","build_id":BUILD_ID,"config":config,"policy":policy,"mulligans":mulligans}))?;
                    eprintln!("streaming accepted inputs to {}", path.display());
                    return Ok(out);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("no fresh live journal path".into())
    }
    fn append(&mut self, value: &impl Serialize) -> Result<(), String> {
        use std::io::Write;
        let mut line = canonical(value)?.into_bytes();
        line.push(b'\n');
        self.file.write_all(&line).map_err(|e| e.to_string())
    }
}
pub(super) fn run(
    catalog: &Catalog,
    config: MatchConfig,
    policy: Policy,
    mulligans: [u8; 4],
    quantum: u64,
) -> Result<Evidence, String> {
    let start = Instant::now();
    if quantum == 0 {
        return Err("positive match quantum required".into());
    }
    let prepared =
        Prepared::admit_pod(catalog.clone(), config.clone()).map_err(|e| format!("{e:?}"))?;
    let trace_mode = if config.seed == seed_for(&config.profile, 0) || policy == Policy::DrawOnly {
        TraceMode::Detailed
    } else {
        TraceMode::Off
    };
    let mut game = Game::with_trace(prepared, trace_mode);
    let mut live_journal = LiveJournal::new(&config, policy, mulligans)?;
    let mut strategy = Strategy {
        catalog,
        policy,
        mulligans,
        mana_plan: Default::default(),
        mana_color: None,
    };
    let selected_checkpoints =
        config.seed == seed_for(&config.profile, 0) && policy == Policy::Develop;
    let mut checkpoints = vec![];
    if selected_checkpoints {
        checkpoints.push(Checkpoint {
            decisions: 0,
            rule_boundary: game.rule_boundary(),
            snapshot: game.snapshot()?,
        });
    }
    let mut survivor_counts = vec![4];

    let mut decisions = 0;
    let mut advances = 0;
    let mut work = 0;
    let mut query_calls = 0;
    let mut winner = None;
    let mut outcome = "advance_limit".to_string();
    while work < WORK_LIMIT {
        advances += 1;
        work += 1;
        let next = game.advance(Budget::new(quantum));
        let living = game.live_players().len();
        if survivor_counts.last() != Some(&living) {
            survivor_counts.push(living)
        }
        match next {
            Advance::Suspended { .. } => continue,
            Advance::Finished { winner: p } => {
                winner = Some(p);
                outcome = "finished".into();
                break;
            }
            Advance::Draw => {
                outcome = "draw".into();
                break;
            }
            Advance::Fault(e) => {
                outcome = if game.typed_fault().is_some() {
                    format!("resource_truncation: {e}")
                } else {
                    format!("fault: {e}")
                };
                break;
            }
            Advance::AwaitingDecision(r) => {
                if selected_checkpoints
                    && ((checkpoints.len() == 1 && matches!(r, mtg_core::Request::Priority { .. }))
                        || (checkpoints.len() == 2 && game.turn().number >= 16))
                {
                    checkpoints.push(Checkpoint {
                        decisions,
                        rule_boundary: game.rule_boundary(),
                        snapshot: game.snapshot()?,
                    });
                }
                if decisions >= DECISION_LIMIT || game.turn().number > TURN_LIMIT {
                    outcome = "host_limit".into();
                    break;
                }
                let response_result = (|| -> Result<(), String> {
                    let observation =
                        observe(&game, r.player(), quantum, &mut work, &mut query_calls)?;
                    let response = strategy.choose(&observation).map_err(|e| {
                        format!(
                            "turn{} decision{decisions} {:?}: {e}",
                            game.turn().number,
                            observation.decision
                        )
                    })?;
                    let token = observation
                        .decision
                        .as_ref()
                        .ok_or("missing decision token")?
                        .token()
                        .clone();
                    match complete(
                        &game,
                        || {
                            game.check_player_response(
                                r.player(),
                                token,
                                response,
                                Budget::new(quantum),
                            )
                        },
                        quantum,
                        &mut work,
                        &mut query_calls,
                    )? {
                        QueryValue::Validated(receipt) => game.submit(receipt)?,
                        _ => return Err("wrong match receipt type".into()),
                    }
                    Ok(())
                })();
                if let Err(e) = response_result {
                    outcome = format!("policy_or_query_failure: {e}");
                    break;
                }
                live_journal.append(&serde_json::json!({"kind":"accepted","input":game.journal().last().ok_or("missing accepted journal entry")?}))?;
                decisions += 1;
            }
        }
    }
    let final_state_sha256 = game.state_digest()?;
    if selected_checkpoints {
        checkpoints.push(Checkpoint {
            decisions,
            rule_boundary: game.rule_boundary(),
            snapshot: game.snapshot()?,
        });
    }
    live_journal.append(&serde_json::json!({"kind":"footer","outcome":outcome,"winner":winner,"state_sha256":final_state_sha256,"rule_boundary":game.rule_boundary()}))?;
    live_journal.file.sync_data().map_err(|e| e.to_string())?;
    let survivors = game.live_players();
    Ok(Evidence {
        config,
        policy,
        mulligans,
        journal: game.replay_journal(),
        trace_mode,
        losses: losses(&game),
        checkpoints,
        outcome,
        winner,
        decisions,
        advances,
        rule_steps: game.rule_boundary(),
        query_calls,
        work_limit: WORK_LIMIT,
        quantum,
        turns: game.turn().number,
        final_state_sha256,
        surviving_seats: survivors,
        survivor_counts,
        elapsed_millis: start.elapsed().as_millis(),
    })
}
pub(super) fn seed(index: u32) -> [u8; 32] {
    // Domain and index are fixed before running the acceptance batch.
    seed_for("commander-foundations-v1", index)
}
pub(super) fn seed_for(profile: &str, index: u32) -> [u8; 32] {
    let hex = sha256(format!("{profile}/completion-batch/{index}").as_bytes());
    std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("SHA256 hex"))
}

// Host counters are retained diagnostics. These guards enforce the published
// ceilings without claiming independent reconstruction of exact query costs.
fn verify_host_metrics(e: &Evidence) -> Result<(), String> {
    if e.trace_mode == TraceMode::Compact {
        return Err("match driver never emits compact trace evidence".into());
    }
    if e.decisions != e.journal.decisions.len() {
        return Err("match decision count differs from accepted journal".into());
    }
    if e.turns > TURN_LIMIT
        || e.decisions > DECISION_LIMIT
        || e.advances
            .checked_add(e.query_calls)
            .is_none_or(|total| total > WORK_LIMIT)
    {
        return Err("match evidence exceeds published host ceilings".into());
    }
    Ok(())
}
pub(super) fn verify(catalog: &Catalog, e: &Evidence) -> Result<(), String> {
    verify_replay(catalog, e).map(|_| ())
}
fn verify_replay(
    catalog: &Catalog,
    e: &Evidence,
) -> Result<Vec<(PlayerId, mtg_core::PlayerLossReason)>, String> {
    verify_host_metrics(e)?;
    let p = Prepared::admit_pod(catalog.clone(), e.config.clone()).map_err(|e| format!("{e:?}"))?;
    let mut verifier = SnapshotVerifier::new(p.clone(), e.trace_mode);
    for checkpoint in &e.checkpoints {
        let restored = verifier.verify(&checkpoint.snapshot)?;
        let prefix = restored.replay_journal();
        if restored.rule_boundary() != checkpoint.rule_boundary
            || prefix.decisions.len() != checkpoint.decisions
            || !e.journal.decisions.starts_with(&prefix.decisions)
            || !e.journal.concessions.starts_with(&prefix.concessions)
        {
            return Err("match checkpoint prefix/boundary mismatch".into());
        }
    }
    if !e.checkpoints.is_empty()
        && verifier.verified_digest() != Some(e.final_state_sha256.as_str())
    {
        return Err("final match checkpoint digest mismatch".into());
    }
    let g = Game::replay_with_trace(p, &e.journal, TraceMode::Detailed)?;
    let loss_facts = losses(&g);
    let unique = loss_facts
        .iter()
        .map(|(player, _)| *player)
        .collect::<std::collections::BTreeSet<_>>();
    if unique.len() != loss_facts.len()
        || (e.journal.concessions.is_empty() && loss_facts.len() != 4 - g.live_players().len())
    {
        return Err("duplicate or incomplete actual player-loss proof".into());
    }
    let mut remaining = 4usize;
    let mut survivor_counts = vec![4];
    for event in g.events() {
        if let mtg_core::Event::PlayersLost { losses } = event {
            remaining = remaining
                .checked_sub(losses.len())
                .ok_or("invalid loss wave count")?;
            survivor_counts.push(remaining);
        }
    }
    if survivor_counts != e.survivor_counts {
        return Err("replayed survivor sequence mismatch".into());
    }
    if e.trace_mode == TraceMode::Detailed && loss_facts != e.losses {
        return Err("match typed loss evidence mismatch".into());
    }
    if e.trace_mode == TraceMode::Off && !e.losses.is_empty() {
        return Err("off-mode run cannot claim retained typed losses".into());
    }
    if g.state_digest()? != e.final_state_sha256 {
        return Err("match replay digest mismatch".into());
    }
    if g.turn().number != e.turns
        || g.rule_boundary() != e.rule_steps
        || g.live_players() != e.surviving_seats
    {
        return Err("match replay outcome mismatch".into());
    }
    verify_terminal_result(&g, e)?;
    Ok(loss_facts)
}
fn verify_terminal_result(game: &Game, e: &Evidence) -> Result<(), String> {
    let mut probe = game.fork();
    match (e.outcome.as_str(), probe.advance(Budget::new(1)), e.winner) {
        ("finished", Advance::Finished { winner }, Some(reported)) if winner == reported => {}
        ("draw", Advance::Draw, None) => {}
        _ => return Err("match terminal result or reported winner mismatch".into()),
    }
    Ok(())
}
pub(super) fn batch(
    catalog: &Catalog,
    profile_name: &str,
    directory: &Path,
) -> Result<serde_json::Value, String> {
    let catalog_sha256 = mtg_contracts::digest(catalog)?;
    let profile_spec = mtg_core::match_profile(profile_name).ok_or("unknown batch profile")?;
    if profile_spec.catalog_sha256 != catalog_sha256
        || profile_spec.ir_version != catalog.schema_version
    {
        return Err("batch catalog mismatch".into());
    }
    let profile = profile_spec.name.as_str();
    fs::create_dir(directory).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({"profile":profile,"policy":"ir-develop-v2","seed_domain":format!("{profile}/completion-batch/"),"seed_indices":(0..16).collect::<Vec<u32>>(),"starting_seat":"index modulo4","mulligans":"(index+seat) modulo3","additional":{"homogeneous_seats":profile_spec.completion.homogeneous_seats},"decision_limit":DECISION_LIMIT,"turn_limit":TURN_LIMIT,"shared_work_limit":WORK_LIMIT,"quantum":1024,"required_complete":18,"allowed_faults":0,"allowed_truncations":0,"build_id":BUILD_ID,"catalog_sha256":mtg_contracts::digest(catalog)?});
    let mut manifest = manifest;
    // The optional rotation layout and extra homogeneous decks come from data.
    if profile_spec.completion.rotate {
        manifest["seat_rotation"] =
            serde_json::json!("default decks rotate left by floor(index/4), indices0..15");
    }
    fs::write(directory.join("manifest.json"), canonical(&manifest)?).map_err(|e| e.to_string())?;
    let mut reports = vec![];
    let mut failures = vec![];
    for index in 0..18u32 {
        let mut config = MatchConfig::for_profile(
            profile,
            seed_for(profile, index),
            PlayerId((index % 4) as u8),
        )
        .map_err(|e| e.to_string())?;
        apply_completion_layout(profile_spec, index as usize, &mut config);
        eprintln!("running legal Commander batch seed{index}");
        let e = match run(
            catalog,
            config,
            Policy::Develop,
            std::array::from_fn(|p| ((index as usize + p) % 3) as u8),
            1024,
        ) {
            Ok(e) => e,
            Err(error) => {
                let failure = serde_json::json!({"index":index,"error":error,"phase":"before evidence construction"});
                fs::write(
                    directory.join(format!("failure-{index:02}.json")),
                    canonical(&failure)?,
                )
                .map_err(|e| e.to_string())?;
                failures.push(failure);
                continue;
            }
        };
        // Persist the journal even if completion or independent replay fails.
        fs::write(
            directory.join(format!("game-{index:02}.json")),
            canonical(&e)?,
        )
        .map_err(|e| e.to_string())?;
        let replay =
            verify_suite_entry(index as usize, &e).and_then(|()| verify_replay(catalog, &e));
        if !matches!(e.outcome.as_str(), "finished" | "draw") || replay.is_err() {
            failures.push(
                serde_json::json!({"index":index,"outcome":e.outcome,"replay_error":replay.as_ref().err()}),
            );
        }
        let mut report = serde_json::json!({"index":index,"outcome":e.outcome,"winner":e.winner,"survivor_counts":e.survivor_counts,"turns":e.turns,"decisions":e.decisions,"advances":e.advances,"rule_steps":e.rule_steps,"query_calls":e.query_calls,"verified_losses":replay.as_ref().ok(),"elapsed_millis":e.elapsed_millis,"final_state_sha256":e.final_state_sha256});
        {
            report["interactions"] = interactions::game(catalog, &e)?;
        }
        reports.push(report);
    }
    let interaction_summary = interactions::summarize(catalog, &reports)?;
    let report = serde_json::json!({"manifest":manifest,"games":reports,"interactions":interaction_summary,"failures":failures,"passed":failures.is_empty()});
    fs::write(directory.join("report.json"), canonical(&report)?).map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        return Err(format!(
            "legal Commander batch failed; see {}",
            directory.display()
        ));
    }
    Ok(report)
}

pub(super) fn suite(catalog: &Catalog) -> Result<(Vec<Evidence>, serde_json::Value), String> {
    let parent = artifact_root().join("p5-batches");
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let directory = (0..1000)
        .map(|i| parent.join(format!("capture-{}-{i}", std::process::id())))
        .find(|p| !p.exists())
        .ok_or("no fresh batch directory")?;
    let report = batch(catalog, "commander-foundations-v1", &directory)?;
    let evidence = (0..18)
        .map(|i| {
            fs::read_to_string(directory.join(format!("game-{i:02}.json")))
                .map_err(|e| e.to_string())
                .and_then(|text| strict_read(&text))
        })
        .collect::<Result<Vec<Evidence>, String>>()?;
    verify_suite(catalog, &evidence)?;
    Ok((evidence, report))
}
pub(super) fn verify_suite(catalog: &Catalog, evidence: &[Evidence]) -> Result<(), String> {
    if evidence.len() != 18 {
        return Err("fixed completion batch must contain18 games".into());
    }
    let mut reasons = vec![];
    for (index, e) in evidence.iter().enumerate() {
        verify_suite_entry(index, e)?;
        reasons.extend(
            verify_replay(catalog, e)?
                .into_iter()
                .map(|(_, reason)| reason),
        );
    }
    if !reasons.contains(&mtg_core::PlayerLossReason::CommanderDamage)
        || !reasons.contains(&mtg_core::PlayerLossReason::Life)
    {
        return Err(
            "fixed completion evidence lacks Commander-damage or ordinary-life loss witness".into(),
        );
    }
    Ok(())
}
fn apply_completion_layout(
    profile: &mtg_core::MatchProfile,
    index: usize,
    config: &mut MatchConfig,
) {
    if profile.completion.rotate {
        config.decks.rotate_left((index / 4) % 4);
    }
    if (16..18).contains(&index) {
        let deck = &profile.decks[profile.completion.homogeneous_seats[index - 16]];
        config.decks = std::array::from_fn(|_| deck.clone());
    }
}
fn verify_suite_entry(index: usize, e: &Evidence) -> Result<(), String> {
    let profile = mtg_core::match_profile(&e.config.profile).ok_or("unknown completion profile")?;
    let mut config = MatchConfig::for_profile(
        &profile.name,
        seed_for(&profile.name, index as u32),
        PlayerId((index % 4) as u8),
    )
    .map_err(|e| e.to_string())?;
    apply_completion_layout(profile, index, &mut config);
    let trace_mode = if index == 0 {
        TraceMode::Detailed
    } else {
        TraceMode::Off
    };
    if !e.journal.concessions.is_empty()
        || e.config != config
        || e.policy != Policy::Develop
        || e.mulligans != std::array::from_fn(|p| ((index + p) % 3) as u8)
        || e.quantum != 1024
        || e.work_limit != WORK_LIMIT
        || e.trace_mode != trace_mode
    {
        return Err("fixed completion batch manifest mismatch".into());
    }
    if index == 0 && e.checkpoints.len() != 4 || index != 0 && !e.checkpoints.is_empty() {
        return Err("selected checkpoint closure mismatch".into());
    }
    verify_host_metrics(e)
}

fn losses(game: &Game) -> Vec<(PlayerId, mtg_core::PlayerLossReason)> {
    // Detailed events contain each actual loss once. Batch trace records repeat
    // those outcomes as transaction proofs and must not be counted again.
    game.events()
        .iter()
        .filter_map(|event| match event {
            mtg_core::Event::Committed {
                actual: mtg_core::EventOutcome::Lost { player, reason },
                ..
            } => Some((*player, *reason)),
            _ => None,
        })
        .collect()
}
pub(super) fn draw_failure(catalog: &Catalog) -> Result<Evidence, String> {
    let mut config =
        MatchConfig::for_profile("commander-foundations-v1", seed(18), PlayerId(2)).unwrap();
    config.decks = std::array::from_fn(|_| {
        mtg_core::match_profile("commander-foundations-v1")
            .unwrap()
            .decks[0]
            .clone()
    });
    let e = run(catalog, config, Policy::DrawOnly, [0; 4], 1024)?;
    verify_draw_failure(catalog, &e)?;
    Ok(e)
}
pub(super) fn verify_draw_failure(catalog: &Catalog, e: &Evidence) -> Result<(), String> {
    let mut config =
        MatchConfig::for_profile("commander-foundations-v1", seed(18), PlayerId(2)).unwrap();
    config.decks = std::array::from_fn(|_| {
        mtg_core::match_profile("commander-foundations-v1")
            .unwrap()
            .decks[0]
            .clone()
    });
    if !e.journal.concessions.is_empty()
        || e.config != config
        || e.policy != Policy::DrawOnly
        || e.mulligans != [0; 4]
        || e.trace_mode != TraceMode::Detailed
        || e.losses
            != vec![
                (PlayerId(2), mtg_core::PlayerLossReason::FailedDraw),
                (PlayerId(3), mtg_core::PlayerLossReason::FailedDraw),
                (PlayerId(0), mtg_core::PlayerLossReason::FailedDraw),
            ]
        || e.winner != Some(PlayerId(1))
        || e.turns != 371
        || e.survivor_counts != vec![4, 3, 2, 1]
    {
        return Err("natural draw-failure witness differs".into());
    }
    verify(catalog, e)
}

// Only newline-delimited, typed host records are executable. This is a recovery
// format, never a player-facing observation or a general state import.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LiveRecord {
    Header {
        build_id: String,
        config: Box<MatchConfig>,
        policy: Policy,
        mulligans: [u8; 4],
    },
    Accepted {
        input: mtg_core::AcceptedInput,
    },
    Footer {
        outcome: String,
        #[serde(deserialize_with = "required_winner")]
        winner: Option<PlayerId>,
        state_sha256: String,
        rule_boundary: u64,
    },
}
fn required_winner<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PlayerId>, D::Error> {
    Option::<PlayerId>::deserialize(deserializer)
}
struct LiveHeader {
    config: MatchConfig,
    policy: Policy,
    mulligans: [u8; 4],
}
struct LiveFooter {
    outcome: String,
    winner: Option<PlayerId>,
    state_sha256: String,
    rule_boundary: u64,
}
struct DecodedLive {
    header: LiveHeader,
    inputs: Vec<mtg_core::AcceptedInput>,
    footer: Option<LiveFooter>,
    incomplete_tail: bool,
}
const MAX_LIVE_RECORD_BYTES: usize = 1_048_576;
fn decode_live(mut reader: impl std::io::BufRead) -> Result<DecodedLive, String> {
    use std::io::{BufRead, Read};
    let (mut header, mut footer, mut inputs, mut incomplete_tail) = (None, None, vec![], false);
    let mut line_number = 0;
    loop {
        let mut bytes = vec![];
        let size = (&mut reader)
            .take((MAX_LIVE_RECORD_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        line_number += 1;
        if size > MAX_LIVE_RECORD_BYTES {
            return Err("live journal record exceeds reviewed size bound".into());
        }
        if footer.is_some() {
            return Err("live journal data follows its footer".into());
        }
        let terminated = bytes.last() == Some(&b'\n');
        // Accept a recoverable *incomplete* final JSON value, never a malformed
        // complete record. IgnoredAny checks syntax; strict_read below also
        // rejects duplicate keys and unknown typed fields.
        if let Err(error) = serde_json::from_slice::<serde::de::IgnoredAny>(&bytes) {
            if !terminated && error.is_eof() {
                incomplete_tail = true;
                break;
            }
            return Err(format!(
                "invalid live journal JSON at line{line_number}: {error}"
            ));
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let record: LiveRecord = strict_read(text)
            .map_err(|e| format!("invalid live journal record at line{line_number}: {e}"))?;
        match record {
            LiveRecord::Header {
                build_id,
                config,
                policy,
                mulligans,
            } => {
                if header.is_some() || !inputs.is_empty() {
                    return Err("live journal header is not first and unique".into());
                }
                if build_id != BUILD_ID {
                    return Err("live journal belongs to another build".into());
                }
                header = Some(LiveHeader {
                    config: *config,
                    policy,
                    mulligans,
                });
            }
            LiveRecord::Accepted { input } => {
                if header.is_none() {
                    return Err("live accepted input precedes header".into());
                }
                if inputs.len() >= DECISION_LIMIT {
                    return Err("live journal exceeds declared decision bound".into());
                }
                inputs.push(input);
            }
            LiveRecord::Footer {
                outcome,
                winner,
                state_sha256,
                rule_boundary,
            } => {
                if header.is_none() {
                    return Err("live footer precedes header".into());
                }
                footer = Some(LiveFooter {
                    outcome,
                    winner,
                    state_sha256,
                    rule_boundary,
                });
            }
        }
    }
    Ok(DecodedLive {
        header: header.ok_or("live journal has no complete header")?,
        inputs,
        footer,
        incomplete_tail,
    })
}
fn replay_live_reader(
    catalog: &Catalog,
    reader: impl std::io::BufRead,
) -> Result<serde_json::Value, String> {
    let decoded = decode_live(reader)?;
    let prepared = Prepared::admit_pod(catalog.clone(), decoded.header.config)
        .map_err(|e| format!("{e:?}"))?;
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    for input in &decoded.inputs {
        let budget = if let Some(footer) = &decoded.footer {
            Budget::new(
                footer
                    .rule_boundary
                    .checked_sub(game.rule_boundary())
                    .ok_or("footer boundary precedes accepted input")?,
            )
        } else {
            Budget::new(u64::MAX)
        };
        let Advance::AwaitingDecision(request) = game.advance(budget) else {
            return Err("live accepted input is unreachable".into());
        };
        if request.player() != input.player || request.decision_id() != input.decision_id {
            return Err("live accepted-input order or identity mismatch".into());
        }
        game.respond(input.player, input.decision_id, input.response.clone())?;
    }
    if let Some(footer) = decoded.footer {
        let remaining = footer
            .rule_boundary
            .checked_sub(game.rule_boundary())
            .ok_or("live footer boundary precedes replay")?;
        let mut actual = game.advance(Budget::new(remaining));
        if game.rule_boundary() != footer.rule_boundary {
            return Err("live footer boundary is unreachable".into());
        }
        // A failed rules step may fault without advancing the successful-step
        // boundary. Reproduce that attempt, but never cross the recorded bound.
        if footer.outcome.starts_with("fault: ") && !matches!(actual, Advance::Fault(_)) {
            let mut probe = game.fork();
            let next = probe.advance(Budget::new(1));
            if matches!(next, Advance::Fault(_)) && probe.rule_boundary() == footer.rule_boundary {
                game = probe;
                actual = next;
            }
        }
        if game.state_digest()? != footer.state_sha256 {
            return Err("live footer state hash mismatch".into());
        }
        let verification = match (footer.outcome.as_str(), actual, footer.winner) {
            ("finished", Advance::Finished { winner }, Some(reported)) if winner == reported => {
                "rules_terminal"
            }
            ("draw", Advance::Draw, None) => "rules_terminal",
            (claimed, Advance::Fault(actual), None)
                if claimed.strip_prefix("fault: ") == Some(actual.as_str()) =>
            {
                "rules_fault"
            }
            (claimed, Advance::AwaitingDecision(_) | Advance::Suspended { .. }, None)
                if matches!(claimed, "host_limit" | "advance_limit")
                    || claimed
                        .strip_prefix("policy_or_query_failure: ")
                        .is_some_and(|s| !s.is_empty()) =>
            {
                "nonterminal_stop_only"
            }
            _ => return Err("live footer outcome or winner mismatch".into()),
        };
        Ok(
            serde_json::json!({"status":"live_journal_footer_verified","accepted_inputs":decoded.inputs.len(),"state_sha256":footer.state_sha256,"rule_boundary":footer.rule_boundary,"outcome":footer.outcome,"winner":footer.winner,"outcome_verification":verification,"diagnostic_stop_reason_verified":verification!="nonterminal_stop_only","exact_interruption_boundary":true,"incomplete_tail_ignored":false,"policy":decoded.header.policy,"mulligans":decoded.header.mulligans}),
        )
    } else {
        let continuation = match game.advance(Budget::new(u64::MAX)) {
            Advance::Finished { .. } => "finished",
            Advance::Draw => "draw",
            Advance::Fault(_) => "fault",
            Advance::AwaitingDecision(_) => "awaiting_decision",
            Advance::Suspended { .. } => "suspended",
        };
        Ok(
            serde_json::json!({"status":"live_journal_prefix_replayed","accepted_inputs":decoded.inputs.len(),"replayed_state_sha256":game.state_digest()?,"replayed_rule_boundary":game.rule_boundary(),"continuation":continuation,"exact_interruption_boundary":false,"incomplete_tail_ignored":decoded.incomplete_tail,"policy":decoded.header.policy,"mulligans":decoded.header.mulligans}),
        )
    }
}
pub(super) fn replay_live(
    catalog: &Catalog,
    profile: &str,
    path: &Path,
) -> Result<serde_json::Value, String> {
    let decoded = decode_live(std::io::BufReader::new(
        fs::File::open(path).map_err(|e| e.to_string())?,
    ))?;
    if decoded.header.config.profile != profile {
        return Err("replay profile mismatch".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    replay_live_reader(catalog, std::io::BufReader::new(file))
}

#[cfg(test)]
mod admission_and_host_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn optional_effect_policy_uses_only_the_top_program() {
        let catalog = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
        let (_, _, _, mut game) = initial();
        game.advance(Budget::unlimited());
        let mut o = observe(&game, PlayerId(0), u64::MAX, &mut 0, &mut 0).unwrap();
        o.decision = Some(PlayerRequest::OptionalEffect {
            token: mtg_core::DecisionToken("policy-fixture".into()),
            player: PlayerId(0),
        });
        let mut strategy = Strategy {
            catalog: &catalog,
            policy: Policy::Develop,
            mulligans: [0; 4],
            mana_plan: Default::default(),
            mana_color: None,
        };
        let id = |name: &str| {
            catalog
                .definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .oracle_id
                .clone()
        };
        let target = mtg_core::VisibleTarget {
            announced: ObjectHandle("own-permanent".into()),
            current: Some(ObjectHandle("own-permanent".into())),
        };
        let entry = |name: &str| mtg_core::VisibleStackEntry {
            x: None,
            mana_value: None,
            mode: None,
            triggered: Some(mtg_core::VisibleTriggered {
                handle: mtg_core::StackHandle(name.into()),
                source: target.clone(),
                oracle_id: id(name),
                ability: 0,
            }),
            entity_targets: vec![],
            handle: Some(mtg_core::StackHandle(name.into())),
            stack_targets: vec![],
            spell: None,
            ability: None,
            player_targets: vec![],
            controller: PlayerId(0),
            cast_complete: true,
            targets: vec![target.clone()],
        };
        for name in ["Bastion of Remembrance", "Sol Ring"] {
            let d = definition(&catalog, &id(name)).unwrap();
            let mut permanent = o.view.objects[0].clone();
            permanent.handle = target.announced.clone();
            permanent.oracle_id = d.oracle_id.clone();
            permanent.characteristics = Some(d.characteristics.clone());
            permanent.controller = PlayerId(0);
            permanent.zone = Zone::Battlefield;
            o.view.objects = vec![permanent];
            // A lower Arsonist must not make Sage's own-permanent target desirable.
            o.view.stack = vec![entry("Goblin Arsonist"), entry("Reclamation Sage")];
            assert_eq!(
                strategy.choose(&o).unwrap(),
                PlayerResponse::OptionalEffect { perform: false }
            );
            o.view.objects[0].controller = PlayerId(1);
            assert_eq!(
                strategy.choose(&o).unwrap(),
                PlayerResponse::OptionalEffect { perform: true }
            );
            o.view.objects[0].controller = PlayerId(0);
            o.view.stack = vec![entry("Goblin Arsonist")];
            assert_eq!(
                strategy.choose(&o).unwrap(),
                PlayerResponse::OptionalEffect { perform: true }
            );
        }
        o.view.stack.clear();
        assert_eq!(
            strategy.choose(&o).unwrap(),
            PlayerResponse::OptionalEffect { perform: false }
        );
    }

    #[test]
    fn frozen_completion_suite_cannot_substitute_host_concessions() {
        let (_, _, _, game) = initial();
        let mut e = evidence(
            &game,
            MatchConfig::for_profile("commander-foundations-v1", seed(1), PlayerId(1)).unwrap(),
        );
        e.policy = Policy::Develop;
        e.mulligans = [1, 2, 0, 1];
        e.quantum = 1024;
        e.trace_mode = TraceMode::Off;
        e.survivor_counts = vec![4];
        verify_suite_entry(1, &e).unwrap();
        e.journal.concessions.push(mtg_core::ExternalInput {
            after_decisions: 0,
            boundary: 0,
            player: PlayerId(0),
        });
        assert!(
            verify_suite_entry(1, &e)
                .unwrap_err()
                .contains("manifest mismatch")
        );
    }

    fn initial() -> (Catalog, MatchConfig, Prepared, Game) {
        let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let config =
            MatchConfig::for_profile("commander-foundations-v1", [31; 32], PlayerId(0)).unwrap();
        let prepared = Prepared::admit_pod(catalog.clone(), config.clone()).unwrap();
        let game = Game::with_trace(prepared.clone(), TraceMode::Off);
        (catalog, config, prepared, game)
    }
    fn evidence(game: &Game, config: MatchConfig) -> Evidence {
        Evidence {
            config,
            policy: Policy::Develop,
            mulligans: [0; 4],
            journal: game.replay_journal(),
            checkpoints: vec![],
            trace_mode: TraceMode::Off,
            losses: vec![],
            outcome: "finished".into(),
            winner: Some(PlayerId(0)),
            decisions: game.journal().len(),
            advances: 0,
            rule_steps: game.rule_boundary(),
            query_calls: 0,
            work_limit: WORK_LIMIT,
            quantum: 1,
            turns: game.turn().number,
            final_state_sha256: game.state_digest().unwrap(),
            surviving_seats: game.live_players(),
            survivor_counts: vec![4],
            elapsed_millis: 0,
        }
    }
    #[test]
    fn exhausted_host_cap_never_invokes_initial_or_extra_query() {
        let mut work = WORK_LIMIT;
        let mut queries = 13;
        assert!(
            query_call(
                || panic!("query executed after exhausted host cap"),
                &mut work,
                &mut queries
            )
            .is_err()
        );
        assert_eq!((work, queries), (WORK_LIMIT, 13));
        let (_, _, _, game) = initial();
        let calls = Cell::new(0);
        work = WORK_LIMIT - 1;
        assert!(
            query_call(
                || {
                    calls.set(calls.get() + 1);
                    game.observe(PlayerId(0), Budget::new(1))
                },
                &mut work,
                &mut queries
            )
            .is_ok()
        );
        assert_eq!((work, queries, calls.get()), (WORK_LIMIT, 14, 1));
        assert!(
            query_call(
                || panic!("one unrecorded query escaped the cap"),
                &mut work,
                &mut queries
            )
            .is_err()
        );
        assert_eq!((work, queries, calls.get()), (WORK_LIMIT, 14, 1));
    }
    #[test]
    fn incomplete_initial_query_cannot_resume_after_using_last_host_call() {
        let (_, _, _, game) = initial();
        let before = game.state_digest().unwrap();
        let mut work = WORK_LIMIT - 1;
        let mut queries = 0;
        let result = complete(
            &game,
            || game.observe(PlayerId(0), Budget::new(1)),
            1,
            &mut work,
            &mut queries,
        );
        assert!(result.is_err());
        assert_eq!((work, queries), (WORK_LIMIT, 1));
        assert_eq!(game.state_digest().unwrap(), before);
    }
    #[test]
    fn matching_nonterminal_replay_cannot_forge_a_completed_result() {
        let (catalog, config, prepared, mut game) = initial();
        // Replay stops at a real decision boundary. Reach the first setup
        // decision without accepting any input, rather than using an unadvanced
        // constructor state as though it were a retained replay boundary.
        loop {
            match game.advance(Budget::new(u64::MAX)) {
                Advance::AwaitingDecision(_) => break,
                Advance::Suspended { .. } => {}
                other => panic!("nonterminal setup proof ended unexpectedly: {other:?}"),
            }
        }
        let mut retained = evidence(&game, config);
        // Establish that the negative is about terminal metadata, not a bad
        // journal, state hash, turn number, or surviving-seat declaration.
        let replay = Game::replay_with_trace(prepared, &retained.journal, TraceMode::Off).unwrap();
        assert_eq!(replay.state_digest().unwrap(), retained.final_state_sha256);
        assert_eq!(replay.live_players(), retained.surviving_seats);
        assert!(verify(&catalog, &retained).is_err());
        retained.outcome = "draw".into();
        retained.winner = None;
        assert!(verify(&catalog, &retained).is_err());
    }
    #[test]
    fn completed_replay_binds_reported_winner_and_rejects_draw_substitution() {
        let (_, config, _, mut game) = initial();
        for player in [PlayerId(1), PlayerId(2), PlayerId(3)] {
            game.concede(player).unwrap();
        }
        let mut finished = false;
        for _ in 0..1024 {
            match game.advance(Budget::new(1)) {
                Advance::Finished {
                    winner: PlayerId(0),
                } => {
                    finished = true;
                    break;
                }
                Advance::Suspended { .. } => {}
                other => panic!("setup concession proof stopped unexpectedly: {other:?}"),
            }
        }
        assert!(finished);
        let mut retained = evidence(&game, config);
        verify_terminal_result(&game, &retained).unwrap();
        retained.winner = Some(PlayerId(1));
        assert!(verify_terminal_result(&game, &retained).is_err());
        retained.outcome = "draw".into();
        retained.winner = None;
        assert!(verify_terminal_result(&game, &retained).is_err());
    }
    fn live_header(config: &MatchConfig) -> serde_json::Value {
        serde_json::json!({"kind":"header","build_id":BUILD_ID,"config":config,"policy":"develop","mulligans":[0,0,0,0]})
    }
    fn live_bytes(records: &[serde_json::Value]) -> Vec<u8> {
        records
            .iter()
            .flat_map(|record| {
                let mut bytes = serde_json::to_vec(record).unwrap();
                bytes.push(b'\n');
                bytes
            })
            .collect()
    }
    fn accepted_setup_prefix() -> (Catalog, Vec<serde_json::Value>, Game) {
        let (catalog, config, _, mut game) = initial();
        let Advance::AwaitingDecision(request) = game.advance(Budget::new(u64::MAX)) else {
            panic!("setup should request a mulligan")
        };
        assert!(matches!(request, mtg_core::Request::Mulligan { .. }));
        game.respond(
            request.player(),
            request.decision_id(),
            mtg_core::Response::Mulligan { take: false },
        )
        .unwrap();
        let records = vec![
            live_header(&config),
            serde_json::json!({"kind":"accepted","input":game.journal().last().unwrap()}),
        ];
        (catalog, records, game)
    }
    fn live_footer(game: &Game) -> serde_json::Value {
        serde_json::json!({"kind":"footer","outcome":"advance_limit","winner":null,"state_sha256":game.state_digest().unwrap(),"rule_boundary":game.rule_boundary()})
    }
    #[test]
    fn live_prefix_replays_accepted_inputs_without_claiming_interruption_boundary() {
        let (catalog, records, mut game) = accepted_setup_prefix();
        assert!(matches!(
            game.advance(Budget::new(u64::MAX)),
            Advance::AwaitingDecision(_)
        ));
        let bytes = live_bytes(&records);
        let replay = replay_live_reader(&catalog, std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(replay["status"], "live_journal_prefix_replayed");
        assert_eq!(replay["accepted_inputs"], 1);
        assert_eq!(
            replay["replayed_state_sha256"],
            game.state_digest().unwrap()
        );
        assert_eq!(replay["replayed_rule_boundary"], game.rule_boundary());
        assert_eq!(replay["exact_interruption_boundary"], false);
        assert_eq!(replay["incomplete_tail_ignored"], false);
        let mut torn = bytes;
        torn.extend_from_slice(br#"{"kind":"footer","outcome":"#);
        let replay = replay_live_reader(&catalog, std::io::Cursor::new(torn)).unwrap();
        assert_eq!(replay["incomplete_tail_ignored"], true);
        assert_eq!(
            replay["replayed_state_sha256"],
            game.state_digest().unwrap()
        );
    }
    #[test]
    fn live_footer_binds_exact_boundary_and_hash_but_not_diagnostic_stop_reason() {
        let (catalog, mut records, mut game) = accepted_setup_prefix();
        // Capture immediately after an accepted input, before its pending rule
        // work. This must not be silently advanced to the next decision.
        records.push(live_footer(&game));
        let replay =
            replay_live_reader(&catalog, std::io::Cursor::new(live_bytes(&records))).unwrap();
        assert_eq!(replay["status"], "live_journal_footer_verified");
        assert_eq!(replay["state_sha256"], game.state_digest().unwrap());
        assert_eq!(replay["rule_boundary"], game.rule_boundary());
        assert_eq!(replay["exact_interruption_boundary"], true);
        assert_eq!(replay["outcome_verification"], "nonterminal_stop_only");
        assert_eq!(replay["diagnostic_stop_reason_verified"], false);
        assert!(matches!(
            game.advance(Budget::new(u64::MAX)),
            Advance::AwaitingDecision(_)
        ));
        *records.last_mut().unwrap() = live_footer(&game);
        replay_live_reader(&catalog, std::io::Cursor::new(live_bytes(&records))).unwrap();
        let footer_index = records.len() - 1;
        for (field, value) in [
            ("state_sha256", serde_json::json!("00")),
            ("rule_boundary", serde_json::json!(0)),
            ("outcome", serde_json::json!("finished")),
            ("winner", serde_json::json!(0)),
            ("outcome", serde_json::json!("fault: invented")),
        ] {
            let mut bad = records.clone();
            bad[footer_index][field] = value;
            assert!(
                replay_live_reader(&catalog, std::io::Cursor::new(live_bytes(&bad))).is_err(),
                "accepted forged {field}"
            );
        }
        records[footer_index]
            .as_object_mut()
            .unwrap()
            .remove("winner");
        assert!(decode_live(std::io::Cursor::new(live_bytes(&records))).is_err());
    }
    #[test]
    fn live_decoder_rejects_complete_malformed_unknown_duplicate_and_misordered_records() {
        let (catalog, records, _) = accepted_setup_prefix();
        let header = records[0].clone();
        let accepted = records[1].clone();
        let footer = serde_json::json!({"kind":"footer","outcome":"host_limit","winner":null,"state_sha256":"00","rule_boundary":0});
        for invalid in [
            vec![accepted.clone()],
            vec![footer.clone()],
            vec![header.clone(), header.clone()],
            vec![header.clone(), footer.clone(), accepted.clone()],
            vec![header.clone(), footer.clone(), footer.clone()],
        ] {
            assert!(decode_live(std::io::Cursor::new(live_bytes(&invalid))).is_err());
        }
        let mut bad_header = header.clone();
        bad_header["build_id"] = serde_json::json!("other-build");
        assert!(decode_live(std::io::Cursor::new(live_bytes(&[bad_header]))).is_err());
        let mut unknown = header.clone();
        unknown["future_field"] = serde_json::json!(true);
        assert!(decode_live(std::io::Cursor::new(live_bytes(&[unknown]))).is_err());
        for tail in [
            b"{not-json}\n".as_slice(),
            b"{\n".as_slice(),
            br#"{"kind":"accepted","kind":"accepted","input":{}}"#.as_slice(),
        ] {
            let mut bytes = live_bytes(std::slice::from_ref(&header));
            bytes.extend_from_slice(tail);
            assert!(decode_live(std::io::Cursor::new(bytes)).is_err());
        }
        assert!(
            replay_live_reader(
                &catalog,
                std::io::Cursor::new(live_bytes(&[header, accepted.clone(), accepted]))
            )
            .is_err()
        );
        assert!(decode_live(std::io::Cursor::new(vec![b' '; MAX_LIVE_RECORD_BYTES + 1])).is_err());
    }
    #[test]
    fn match_metadata_rejects_trace_bypass_journal_count_and_host_cap_forgery() {
        let (_, config, _, game) = initial();
        let mut retained = evidence(&game, config);
        verify_host_metrics(&retained).unwrap();
        retained.trace_mode = TraceMode::Compact;
        assert!(verify_host_metrics(&retained).is_err());
        retained.trace_mode = TraceMode::Off;
        retained.decisions = 1;
        assert!(verify_host_metrics(&retained).is_err());
        retained.decisions = 0;
        retained.turns = TURN_LIMIT + 1;
        assert!(verify_host_metrics(&retained).is_err());
        retained.turns = TURN_LIMIT;
        retained.advances = WORK_LIMIT;
        retained.query_calls = 1;
        assert!(verify_host_metrics(&retained).is_err());
        retained.advances = usize::MAX;
        assert!(verify_host_metrics(&retained).is_err());
        retained.advances = WORK_LIMIT - 1;
        verify_host_metrics(&retained).unwrap();
    }
    #[test]
    fn fixed_batch_manifest_cannot_drop_seed_zero_trace_or_enable_unselected_traces() {
        let (_, _, _, game) = initial();
        for index in [0, 1, 16, 17] {
            let mut config = MatchConfig::for_profile(
                "commander-foundations-v1",
                seed(index as u32),
                PlayerId((index % 4) as u8),
            )
            .unwrap();
            if index == 16 {
                config.decks = std::array::from_fn(|_| {
                    mtg_core::match_profile("commander-foundations-v1")
                        .unwrap()
                        .decks[0]
                        .clone()
                });
            }
            if index == 17 {
                config.decks = std::array::from_fn(|_| {
                    mtg_core::match_profile("commander-foundations-v1")
                        .unwrap()
                        .decks[1]
                        .clone()
                });
            }
            let mut retained = evidence(&game, config);
            retained.quantum = 1024;
            retained.mulligans = std::array::from_fn(|p| ((index + p) % 3) as u8);
            retained.trace_mode = if index == 0 {
                TraceMode::Detailed
            } else {
                TraceMode::Off
            };
            if index == 0 {
                retained.checkpoints = (0..4)
                    .map(|_| Checkpoint {
                        decisions: 0,
                        rule_boundary: 0,
                        snapshot: String::new(),
                    })
                    .collect();
            }
            verify_suite_entry(index, &retained).unwrap();
            retained.trace_mode = if index == 0 {
                TraceMode::Off
            } else {
                TraceMode::Detailed
            };
            assert!(verify_suite_entry(index, &retained).is_err());
        }
    }
}
