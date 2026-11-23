//! Headless throughput for admitted four-seat pods. Worker clocks measure elapsed
//! time in each phase and are summed across workers. Proof runs are outside the
//! measured window and go through the ordinary snapshot verifier.
use crate::play::{DECISION_LIMIT, TURN_LIMIT, WORK_LIMIT, complete, observe, seed_for};
use crate::policy::{Policy, Strategy};
use mtg_contracts::Catalog;
use mtg_core::{
    Advance, Budget, BuildIdentity, Game, MatchConfig, PlayerId, Prepared, QueryValue,
    SnapshotVerifier, TraceMode,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::{fs, time::Instant};

pub mod bulk_input;
use crate::analytics::{self, AnalyticsDeck, AnalyticsProfile, PodOutcome};
use mtg_core::{DYNAMIC_PROFILE, DynamicDeckEntry, DynamicPodTemplate, MatchProfile};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const QUANTUM: u64 = 1024;
pub const USAGE: &str = "usage: mtg-sim bench-pods PROFILE [OPTIONS]\n\
       mtg-sim bench-pods --bulk-artifact DIR --manifest-sha SHA --pod-decks JSON [OPTIONS]\n\
OPTIONS: [--threads 1..32] [--games N] \
[--seed-start N] [--rotate-seats] [--decision-limit N] [--analytics-out NEW_DIR]\n\
--games is the total number of pods (default 32). Rotation requires a multiple of four.\n\
Each rotated group reuses one seed and starting seat while cycling the four deck slots.\n\
The first and last seed groups are independently rerun and replayed.\n\
Pod JSON is an object with four decks of {oracle_id, quantity, role} rows.\n\
--analytics-out writes aggregate analytics.json and summary.md from independent terminal replays.";

pub struct Options {
    threads: usize,
    games: usize,
    seed_start: u32,
    rotate: bool,
    decision_limit: usize,
    analytics_out: Option<PathBuf>,
}
impl Options {
    pub fn ensure_new_analytics_out(&self) -> Result<(), String> {
        let Some(path) = &self.analytics_out else {
            return Ok(());
        };
        match fs::symlink_metadata(path) {
            Ok(_) => Err(format!(
                "analytics output already exists: {}",
                path.display()
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self {
            threads: std::thread::available_parallelism().map_or(1, |n| n.get().min(32)),
            games: 32,
            seed_start: 0,
            rotate: false,
            decision_limit: DECISION_LIMIT,
            analytics_out: None,
        };
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            if flag == "--rotate-seats" {
                options.rotate = true;
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            let invalid = |_| format!("invalid value for {flag}: {value}");
            match flag.as_str() {
                "--threads" => options.threads = value.parse().map_err(invalid)?,
                "--games" => options.games = value.parse().map_err(invalid)?,
                "--seed-start" => options.seed_start = value.parse().map_err(invalid)?,
                "--decision-limit" => options.decision_limit = value.parse().map_err(invalid)?,
                "--analytics-out" if options.analytics_out.is_none() => {
                    options.analytics_out = Some(value.into())
                }
                _ => return Err(format!("unknown bench-pods flag {flag}")),
            }
        }
        if !(1..=32).contains(&options.threads)
            || !(1..=DECISION_LIMIT).contains(&options.decision_limit)
            || options.games == 0
            || !options.games.is_multiple_of(options.rotations())
        {
            return Err(USAGE.into());
        }
        let last = u32::try_from(options.games / options.rotations() - 1)
            .map_err(|_| "seed range exceeds u32")?;
        options
            .seed_start
            .checked_add(last)
            .ok_or("seed range exceeds u32")?;
        Ok(options)
    }

    fn rotations(&self) -> usize {
        if self.rotate { 4 } else { 1 }
    }

    fn case(&self, index: usize) -> Case {
        Case {
            index,
            seed_index: self.seed_start + (index / self.rotations()) as u32,
            rotation: index % self.rotations(),
        }
    }

    fn sample(&self, index: usize) -> bool {
        index < self.rotations() || index >= self.games - self.rotations()
    }
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
struct Case {
    index: usize,
    seed_index: u32,
    rotation: usize,
}
impl Case {
    fn config(self, profile: &str) -> Result<MatchConfig, String> {
        let mut config = MatchConfig::for_profile(
            profile,
            seed_for("bench-pods-v1", self.seed_index),
            PlayerId((self.seed_index % 4) as u8),
        )
        .map_err(|error| error.to_string())?;
        config.decks.rotate_right(self.rotation);
        Ok(config)
    }

    fn deck_slot(self, seat: usize) -> usize {
        (seat + 4 - self.rotation) % 4
    }
}

#[derive(Clone, Copy)]
pub enum PodSource<'a> {
    Embedded(&'a MatchProfile),
    Dynamic {
        template: &'a DynamicPodTemplate,
        decks: &'a [Vec<DynamicDeckEntry>; 4],
    },
}
impl<'a> PodSource<'a> {
    fn prepare(
        self,
        case: Case,
        admission_ns: &mut u128,
    ) -> Result<(Prepared, u8, Option<String>), String> {
        match self {
            Self::Embedded(profile) => {
                let config = case.config(&profile.name)?;
                let starting = config.starting_player.0;
                let prepared = timed(admission_ns, || {
                    Prepared::admit_pod(profile.catalog().clone(), config)
                })
                .map_err(|error| error.to_string())?;
                Ok((prepared, starting, None))
            }
            Self::Dynamic { template, .. } => {
                let starting = PlayerId((case.seed_index % 4) as u8);
                let seed = seed_for("bench-pods-v1", case.seed_index);
                let pod = timed(admission_ns, || {
                    template.prepare(starting, seed, case.rotation)
                })
                .map_err(|error| error.to_string())?;
                Ok((
                    pod.prepared,
                    starting.0,
                    Some(pod.identity.admission_digest),
                ))
            }
        }
    }
    fn catalog(self) -> &'a Catalog {
        match self {
            Self::Embedded(profile) => profile.catalog(),
            Self::Dynamic { template, .. } => template.catalog().catalog(),
        }
    }
    fn timing_scope(self) -> &'static str {
        match self {
            Self::Embedded(_) => {
                "wall includes worker startup, pod admission, play and unsampled disposal; excludes catalog load, sampled disposal/proofs, aggregation and output. Phase clocks sum concurrent worker elapsed times."
            }
            Self::Dynamic { .. } => {
                "wall includes worker startup, pod setup, play and unsampled disposal. Bulk load, projection, sampled disposal/proofs, aggregation and output are outside worker wall time. Phase clocks sum concurrent worker elapsed times."
            }
        }
    }
    fn analytics_profile(self, build: BuildIdentity) -> AnalyticsProfile<'a> {
        let (_, name, catalog_sha256) = self.report_profile();
        let decks = match self {
            Self::Embedded(profile) => profile.decks.each_ref().map(AnalyticsDeck::embedded),
            Self::Dynamic { decks, .. } => {
                decks.each_ref().map(|rows| AnalyticsDeck::dynamic(rows))
            }
        };
        AnalyticsProfile {
            build,
            name,
            catalog: self.catalog(),
            catalog_sha256,
            decks,
        }
    }
    fn report_profile(self) -> (&'static str, &'a str, &'a str) {
        match self {
            Self::Embedded(profile) => ("bench-pods-v1", &profile.name, &profile.catalog_sha256),
            Self::Dynamic { template, .. } => (
                "bench-pods-bulk-v1",
                DYNAMIC_PROFILE,
                template.catalog().digest(),
            ),
        }
    }
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Win,
    Draw,
    DecisionLimit,
    TurnLimit,
    WorkLimit,
    ResourceTruncation,
    RulesFault,
    PolicyFailure,
    QueryFailure,
}
impl Outcome {
    fn terminal(self) -> bool {
        matches!(self, Self::Win | Self::Draw)
    }
    fn capped(self) -> bool {
        matches!(
            self,
            Self::DecisionLimit | Self::TurnLimit | Self::WorkLimit
        )
    }
}

// Running sums avoid retaining timing samples for every decision.
macro_rules! phase_clocks {
    ($($phase:ident),+) => {
        #[derive(Default, Serialize)]
        struct Clocks { $($phase: u128),+ }
        impl Clocks {
            fn add(&mut self, other: &Self) {
                $(self.$phase += other.$phase;)+
            }
        }
    };
}
phase_clocks!(
    admission,
    rules,
    observation,
    policy,
    validation,
    submit,
    disposal
);

fn timed<T>(sum: &mut u128, f: impl FnOnce() -> T) -> T {
    let clock = Instant::now();
    let result = f();
    *sum += clock.elapsed().as_nanos();
    result
}

#[derive(Serialize, PartialEq, Eq)]
struct ResultRow {
    case: Case,
    starting_seat: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    admission_sha256: Option<String>,
    outcome: Outcome,
    detail: Option<String>,
    winner: Option<PlayerId>,
    placements: [Option<usize>; 4],
    turns: u64,
    decisions: usize,
    rule_steps: u64,
    query_calls: usize,
}
struct Pod {
    row: ResultRow,
    clocks: Clocks,
    proof: Option<(Prepared, Game)>,
    analytics: Option<Box<analytics::ReplayInput>>,
}

fn play(
    source: PodSource<'_>,
    case: Case,
    decision_limit: usize,
    sample: bool,
    analytics: bool,
) -> Result<Pod, String> {
    let mut clocks = Clocks::default();
    let (prepared, starting_seat, admission_sha256) =
        source.prepare(case, &mut clocks.admission)?;
    let replay_start = (sample || analytics).then(|| prepared.clone());
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    let mut strategy = Strategy::new(source.catalog(), Policy::Develop, [0; 4]);
    let mut placements = Placements::default();
    let mut winner = None;
    let mut detail = None;
    let mut work = 0;
    let mut decisions = 0;
    let mut query_calls = 0;
    let outcome = loop {
        if work == WORK_LIMIT {
            break Outcome::WorkLimit;
        }
        work += 1;
        let next = timed(&mut clocks.rules, || game.advance(Budget::new(QUANTUM)));
        if !matches!(&next, Advance::AwaitingDecision(_)) {
            placements.observe(&game.live_players());
        }
        let request = match next {
            Advance::Suspended { .. } => continue,
            Advance::Finished { winner: player } => {
                winner = Some(player);
                placements.ranks[player.0 as usize] = Some(1);
                break Outcome::Win;
            }
            Advance::Draw => break Outcome::Draw,
            Advance::Fault(error) => {
                detail = Some(error);
                break if game.typed_fault().is_some() {
                    Outcome::ResourceTruncation
                } else {
                    Outcome::RulesFault
                };
            }
            Advance::AwaitingDecision(request) => request,
        };
        if decisions >= decision_limit || game.turn().number > TURN_LIMIT {
            placements.observe(&game.live_players());
            break if decisions >= decision_limit {
                Outcome::DecisionLimit
            } else {
                Outcome::TurnLimit
            };
        }
        let player = request.player();
        let observed = timed(&mut clocks.observation, || {
            observe(&game, player, QUANTUM, &mut work, &mut query_calls)
        });
        let observation = match observed {
            Ok(observation) => observation,
            Err(error) => {
                placements.observe(&game.live_players());
                detail = Some(error);
                break query_stop(detail.as_deref());
            }
        };
        placements.observe(&observation.view.live_turn_order);
        let response = match timed(&mut clocks.policy, || strategy.choose(&observation)) {
            Ok(response) => response,
            Err(error) => {
                detail = Some(error);
                break Outcome::PolicyFailure;
            }
        };
        let token = observation
            .decision
            .as_ref()
            .ok_or("missing decision token")?
            .token()
            .clone();
        let checked = timed(&mut clocks.validation, || {
            complete(
                &game,
                || game.check_player_response(player, token, response, Budget::new(QUANTUM)),
                QUANTUM,
                &mut work,
                &mut query_calls,
            )
        });
        match checked {
            Ok(QueryValue::Validated(receipt)) => {
                if let Err(error) = timed(&mut clocks.submit, || game.submit(receipt)) {
                    detail = Some(error);
                    break Outcome::QueryFailure;
                }
            }
            Ok(_) => return Err("wrong match receipt type".into()),
            Err(error) => {
                detail = Some(error);
                break query_stop(detail.as_deref());
            }
        }
        decisions += 1;
    };
    let row = ResultRow {
        case,
        starting_seat,
        admission_sha256,
        outcome,
        detail,
        winner,
        placements: placements.ranks,
        turns: game.turn().number,
        decisions,
        rule_steps: game.rule_boundary(),
        query_calls,
    };
    let analytics = if analytics && outcome.terminal() {
        Some(Box::new(analytics::ReplayInput::capture(
            replay_start
                .as_ref()
                .ok_or("missing analytics admission")?
                .clone(),
            &game,
        )?))
    } else {
        None
    };
    let proof = if sample {
        Some((replay_start.ok_or("missing sampled admission")?, game))
    } else {
        timed(&mut clocks.disposal, || drop(game));
        None
    };
    Ok(Pod {
        row,
        clocks,
        proof,
        analytics,
    })
}

struct Placements {
    living: u8,
    ranks: [Option<usize>; 4],
}
impl Default for Placements {
    fn default() -> Self {
        Self {
            living: 0b1111,
            ranks: [None; 4],
        }
    }
}
impl Placements {
    fn observe(&mut self, live: &[PlayerId]) {
        let mask = live.iter().fold(0u8, |mask, player| mask | (1 << player.0));
        if mask != self.living {
            let rank = live.len() + 1;
            for (seat, placement) in self.ranks.iter_mut().enumerate() {
                if (self.living & !mask) & (1 << seat) != 0 {
                    *placement = Some(rank);
                }
            }
            self.living = mask;
        }
    }
}

fn query_stop(detail: Option<&str>) -> Outcome {
    if detail == Some("shared host work limit") {
        Outcome::WorkLimit
    } else {
        Outcome::QueryFailure
    }
}

#[derive(Serialize)]
struct Proof {
    case: Case,
    state_sha256: Option<String>,
    journal_sha256: Option<String>,
    error: Option<String>,
}

fn check_sample(source: PodSource<'_>, options: &Options, measured: &mut Pod) -> Proof {
    let case = measured.row.case;
    let checked = (|| {
        let (_, original) = measured.proof.take().ok_or("missing proof game")?;
        let rerun = play(source, case, options.decision_limit, true, false)?;
        let (prepared, rerun_game) = rerun.proof.ok_or("missing rerun proof game")?;
        let journal = original.replay_journal();
        let state_sha256 = original.state_digest()?;
        if measured.row != rerun.row
            || journal != rerun_game.replay_journal()
            || state_sha256 != rerun_game.state_digest()?
        {
            return Err("independent policy rerun diverged".into());
        }
        let replay = if measured.row.outcome.terminal() {
            Game::replay_with_trace(prepared, &journal, TraceMode::Off)?
        } else {
            let snapshot = original.snapshot()?;
            SnapshotVerifier::new(prepared, TraceMode::Off).verify(&snapshot)?
        };
        if replay.rule_boundary() != measured.row.rule_steps
            || replay.replay_journal() != journal
            || replay.state_digest()? != state_sha256
        {
            return Err("snapshot replay diverged".into());
        }
        if measured.row.outcome.terminal() {
            match (replay.fork().advance(Budget::new(1)), measured.row.winner) {
                (Advance::Finished { winner }, Some(expected)) if winner == expected => {}
                (Advance::Draw, None) => {}
                _ => return Err("replay terminal outcome diverged".into()),
            }
        }
        Ok((state_sha256, mtg_contracts::digest(&journal)?))
    })();
    match checked {
        Ok((state, journal)) => Proof {
            case,
            state_sha256: Some(state),
            journal_sha256: Some(journal),
            error: None,
        },
        Err(error) => Proof {
            case,
            state_sha256: None,
            journal_sha256: None,
            error: Some(error),
        },
    }
}

pub fn run(
    build: BuildIdentity,
    source: PodSource<'_>,
    options: &Options,
) -> Result<(serde_json::Value, bool), String> {
    let analytics = options.analytics_out.is_some();
    let workers = options.threads.min(options.games);
    let next = AtomicUsize::new(0);
    let clock = Instant::now();
    let mut pods = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        let mut error = None;
        for id in 0..workers {
            let next = &next;
            match std::thread::Builder::new()
                .name(format!("pod-{id}"))
                .spawn_scoped(scope, move || {
                    let mut pods = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= options.games {
                            break;
                        }
                        let case = options.case(index);
                        pods.push(
                            play(
                                source,
                                case,
                                options.decision_limit,
                                options.sample(index),
                                analytics,
                            )
                            .map_err(|error| format!("pod {index}: {error}"))?,
                        );
                    }
                    Ok::<_, String>(pods)
                }) {
                Ok(handle) => handles.push(handle),
                Err(failure) => {
                    next.store(options.games, Ordering::Relaxed);
                    error = Some(format!("cannot start pod worker: {failure}"));
                    break;
                }
            }
        }
        let mut pods = Vec::new();
        for worker in handles {
            match worker.join() {
                Ok(Ok(rows)) => pods.extend(rows),
                Ok(Err(failure)) => {
                    error.get_or_insert(failure);
                }
                Err(_) => {
                    error.get_or_insert_with(|| "pod worker panicked".into());
                }
            }
        }
        error.map_or(Ok(pods), Err)
    })?;
    let wall = clock.elapsed();
    pods.sort_unstable_by_key(|pod| pod.row.case.index);
    if pods.len() != options.games {
        return Err("incomplete benchmark workload".into());
    }

    let clock = Instant::now();
    let proofs: Vec<_> = pods
        .iter_mut()
        .filter(|pod| pod.proof.is_some())
        .map(|pod| check_sample(source, options, pod))
        .collect();
    let proof_wall = clock.elapsed();
    let verified = proofs.iter().all(|p| p.error.is_none());
    let analytics_wall = if let Some(path) = &options.analytics_out
        && verified
    {
        let clock = Instant::now();
        analytics::write(
            path,
            &source.analytics_profile(build),
            workers,
            options.seed_start,
            options.rotate,
            options.decision_limit,
            pods.iter_mut().map(|pod| {
                let row = &pod.row;
                (
                    PodOutcome {
                        index: row.case.index,
                        slots: std::array::from_fn(|seat| row.case.deck_slot(seat)),
                        starter: row.starting_seat as usize,
                        terminal: row.outcome.terminal(),
                        decision_capped: row.outcome == Outcome::DecisionLimit,
                        budget_capped: row.outcome.capped(),
                        winner: row.winner,
                        placements: row.placements,
                        turns: row.turns,
                        decisions: row.decisions,
                    },
                    pod.analytics.take().map(|input| *input),
                )
            }),
        )?;
        Some(clock.elapsed().as_nanos())
    } else {
        None
    };
    let mut clocks = Clocks::default();
    let mut decisions = 0;
    let mut unfinished = 0;
    let mut capped = 0;
    let mut outcomes = BTreeMap::new();
    let mut wins_by_seat = [0; 4];
    let mut wins_by_turn_order = [0; 4];
    let mut wins_by_deck = [0; 4];
    let mut ranks_by_seat = [[0; 5]; 4];
    let mut ranks_by_deck = [[0; 5]; 4];
    for pod in &pods {
        let row = &pod.row;
        clocks.add(&pod.clocks);
        *outcomes.entry(row.outcome).or_insert(0usize) += 1;
        decisions += row.decisions as u64;
        unfinished += usize::from(!row.outcome.terminal());
        capped += usize::from(row.outcome.capped());
        if let Some(winner) = row.winner {
            let seat = winner.0 as usize;
            wins_by_seat[seat] += 1;
            wins_by_turn_order[(seat + 4 - row.starting_seat as usize) % 4] += 1;
            wins_by_deck[row.case.deck_slot(seat)] += 1;
        }
        for (seat, rank) in row.placements.iter().enumerate() {
            ranks_by_seat[seat][rank.unwrap_or(0)] += 1;
            ranks_by_deck[row.case.deck_slot(seat)][rank.unwrap_or(0)] += 1;
        }
    }
    let games = options.games as f64;
    let seconds = wall.as_secs_f64();
    let rows: Vec<_> = pods.into_iter().map(|pod| pod.row).collect();
    let (schema, profile, catalog_sha256) = source.report_profile();
    let mut report = serde_json::json!({
        "schema": schema,
        "build": {"id": build.id, "profile": build.profile, "opt_level": build.opt_level,
                  "debug": build.debug, "debug_assertions": build.debug_assertions,
                  "rustc": build.rustc, "target": build.target, "rustflags": build.rustflags},
        "profile": profile,
        "catalog_sha256": catalog_sha256,
        "policy": Policy::Develop,
        "trace_mode": TraceMode::Off,
        "threads": workers, "threads_requested": options.threads,
        "games": options.games, "seed_start": options.seed_start, "rotate_seats": options.rotate,
        "limits": {"quantum": QUANTUM, "decisions": options.decision_limit, "turns": TURN_LIMIT, "host_calls": WORK_LIMIT},
        "wall_ns": wall.as_nanos(), "proof_wall_ns": proof_wall.as_nanos(),
        "games_per_second": games / seconds,
        "completed_games_per_second": (options.games - unfinished) as f64 / seconds,
        "decisions": decisions, "decisions_per_second": decisions as f64 / seconds,
        "unfinished": unfinished, "unfinished_percent": 100.0 * unfinished as f64 / games,
        "budget_capped": capped, "budget_capped_percent": 100.0 * capped as f64 / games,
        "outcomes": outcomes,
        "wins_by_seat": wins_by_seat,
        "wins_by_deck_slot": wins_by_deck,
        "wins_by_turn_order": wins_by_turn_order,
        "win_rate_by_turn_order": wins_by_turn_order.map(|wins| wins as f64 / games),
        "win_rate_by_seat": wins_by_seat.map(|wins| wins as f64 / games),
        "placements_by_seat": ranks_by_seat,
        "placements_by_deck_slot": ranks_by_deck,
        "placement_method": "columns 0=unplaced, 1..4=rank; seats eliminated in the same advance call share a rank",
        "worker_elapsed_ns": clocks,
        "timing_scope": source.timing_scope(),
        "proofs_verified": verified,
        "proofs": proofs, "rows": rows,
    });
    if let Some(elapsed) = analytics_wall {
        report["analytics_wall_ns"] = serde_json::json!(elapsed);
        report["analytics_out"] = serde_json::json!(options.analytics_out);
        report["timing_scope"] = serde_json::json!(format!(
            "{} Wall also includes optional terminal journal/digest capture. Detailed analytics replays and file output are measured separately.",
            source.timing_scope()
        ));
    }
    Ok((report, verified))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_rotation_and_capped_replay_are_schedule_independent() {
        let profile = mtg_core::match_profile("commander-foundations-v1").unwrap();
        let mut options = Options::parse(
            &[
                "--games",
                "4",
                "--rotate-seats",
                "--decision-limit",
                "8",
                "--threads",
                "1",
            ]
            .map(String::from),
        )
        .unwrap();
        for index in 0..4 {
            let case = options.case(index);
            let config = case.config(&profile.name).unwrap();
            assert_eq!(
                config.seed,
                options.case(0).config(&profile.name).unwrap().seed
            );
            assert_eq!(config.starting_player, PlayerId(0));
            for seat in 0..4 {
                assert_eq!(config.decks[seat], profile.decks[case.deck_slot(seat)]);
            }
        }
        let (serial, verified) = run(crate::BUILD, PodSource::Embedded(profile), &options).unwrap();
        assert!(verified, "{}", serial["proofs"]);
        options.threads = 4;
        let (parallel, verified) =
            run(crate::BUILD, PodSource::Embedded(profile), &options).unwrap();
        assert!(verified, "{}", parallel["proofs"]);
        assert_eq!(parallel["budget_capped"], 4);
        assert_eq!(parallel["rows"], serial["rows"]);
        assert_eq!(parallel["proofs"], serial["proofs"]);

        let directory = test_output("capped");
        options.analytics_out = Some(directory.clone());
        let (instrumented, verified) =
            run(crate::BUILD, PodSource::Embedded(profile), &options).unwrap();
        let analytics: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("analytics.json")).unwrap()).unwrap();
        fs::remove_dir_all(&directory).unwrap();
        assert!(verified);
        assert_eq!(instrumented["rows"], parallel["rows"]);
        assert_eq!(instrumented["proofs"], parallel["proofs"]);
        assert!(instrumented["analytics_wall_ns"].is_number());
        assert!(parallel.get("analytics_wall_ns").is_none());
        assert_eq!(analytics["verified_terminal_replays"], 0);
        let decks = analytics["decks"].as_array().unwrap();
        assert_eq!(decks.len(), 2);
        for deck in decks {
            assert_eq!(deck["attempts"], 8);
            assert_eq!(deck["decision_cap_rate"]["count"], 8);
            assert!(deck["win_rate"]["rate"].is_null());
        }
    }
    #[test]
    fn terminal_analytics_replay_preserves_state_and_journal() {
        let profile = mtg_core::match_profile("commander-foundations-v1").unwrap();
        let mut options =
            Options::parse(&["--games", "1", "--threads", "1"].map(String::from)).unwrap();
        let (plain, verified) = run(crate::BUILD, PodSource::Embedded(profile), &options).unwrap();
        assert!(verified);
        assert_eq!(plain["unfinished"], 0);

        let directory = test_output("terminal");
        options.analytics_out = Some(directory.clone());
        let (enabled, verified) =
            run(crate::BUILD, PodSource::Embedded(profile), &options).unwrap();
        let analytics: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("analytics.json")).unwrap()).unwrap();
        let markdown = fs::read_to_string(directory.join("summary.md")).unwrap();
        fs::remove_dir_all(&directory).unwrap();
        assert!(verified);
        assert_eq!(enabled["rows"], plain["rows"]);
        assert_eq!(enabled["proofs"], plain["proofs"]);
        assert_eq!(analytics["verified_terminal_replays"], 1);
        assert!(markdown.contains("OH WR") && markdown.contains("GIH WR"));

        let decks = analytics["decks"].as_array().unwrap();
        let seats: u64 = decks
            .iter()
            .map(|deck| deck["completed"].as_u64().unwrap())
            .sum();
        let wins: u64 = decks
            .iter()
            .map(|deck| deck["win_rate"]["count"].as_u64().unwrap())
            .sum();
        assert_eq!((seats, wins), (4, 1));
        assert!(
            decks
                .iter()
                .all(|deck| deck["unclassified_library_to_hand_moves"] == 0)
        );
        assert!(decks.iter().any(|deck| {
            deck["cards_by_oracle_id"]
                .as_object()
                .unwrap()
                .values()
                .any(|card| {
                    card["games_drawn"].as_u64().unwrap() > 0
                        && card["games_land_played"].as_u64().unwrap() > 0
                        && card["cast_rate"]["rate"].is_null()
                })
        }));
    }

    fn test_output(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mtg-bench-analytics-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
