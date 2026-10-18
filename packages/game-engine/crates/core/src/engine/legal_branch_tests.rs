//! Finite legal Commander branch and first-write diagnostics.
//! Intended as a private engine #[cfg(test)] module, never admission bypass.
//! CR903.5 (100/deck, basics), 103.5 (mulligans), 305 (land), 704/800.4
//! (departures): https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Private trusted-host tests; clocks and filesystem I/O remain outside production core.
use super::*;
use std::{hint::black_box, time::Instant};

const LEGAL_BRANCH_CONTRACT: &str = r#"{
 "schema":"legal-branch-diagnostic-v1",
 "admission":"Prepared::admit_pod commander-foundations-v1 only, unchanged34-definition catalog; never injectedScenarioConfig",
 "initial_physical_cards":400,"initial_physical_commanders":4,
 "seed_domains":{"ordinary":"legal-branches-v1/ordinary","dense":"legal-branches-v1/all-plains"},
 "ordinary":{"decks":"default W/BG/W/BG","mulligans":[0,1,2,3],"captures_P0_main_visits":[3,5,7],"captures_after_natural_land_attempt":true,"departure_P2_before_visit":5,"departure_P1_before_visit":7,"fixed_battlefield_count_claim":false},
 "dense":{"decks":"each Isamaru1 Plains99, commander included100","mulligans":[0,0,0,0],"play_land_until_P0_visit_exclusive":65,"captures_P0_main_visits":[65,69,73],"expected_battlefield_rows":[256,192,128],"departure_P2_before_visit":69,"departure_P1_before_visit":73},
 "root_policy":"own active PrecombatMain first visible basicland while enabled; otherwisePass; emptycombat, reverseoffered bottom/discard; unsupportedrequestfails; no hiddenstate root/seed selection",
 "retained_counts":[64,256,1024,10000],"warmup_repetitions":1,"measured_repetitions":5,
 "sweeps":["selected_detail","all_first_write"],"selected_indices":"unique0,n/2,n-1 of actualretained",
 "all_first_write":"EVERYcreatedbranch freshownObservation/ownexecutorreceipt/submitlegalresponse/advance/verify; retainmutatedbranches until disposal; handles alone notusefulbranches",
 "stage_counts":["creation_started","created","query_started","queried","validated","submitted","fully_advanced","verified"],
 "quantum":64,"maximum_query_resumes":100000,"maximum_advance_calls":100000,"maximum_setup_inputs":50000,
 "memory_bytes":2147483648,"rss_reserve_bytes":67108864,"outer_guard":"120s-2gib-v1 independentOSproof required, environment alone isnotproof",
 "oracles":"onegenericfullStatefile/streamingproductionbytecomparison+SHA; fullstructuralStateequality percompletedbranch against separatelyprovedreference; selectedfullgenericoracles; normalrestore/replay and budget1/MAX parity outside operationstopwatches",
 "history":"complete actual setup/draw/discard/concession histories, ownerbottomknowledge, RNG andphysicalcommanderfacts retained",
 "selected_hash_probes":"only selected completedindices0,n/2,n-1: independentgenericfile typedState decode outside hashclock, genuinelyempty perentrymemos; exactfullState equality then timedcold/warmfullSHA; nevereagerfullsnapshot everybranch",
 "measurement":"raw productionsegment timings separate from inclusive lifecycle INCLUDING proof scans; generic/originalquery/restore/replay proof work outside segmentclocks but inside wholeprocessbudget; no tailquantileclaim fromfive sweeps",
 "scope":"legalordinary ANDlegal256BF naturalhistory; notnewmechanics, arbitraryCommanderorstrategicstrength; partialshortfalls retained, no10000usefulclaim fromhandles+3writes"
}"#;
const MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const RSS_RESERVE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Family {
    Ordinary,
    Dense,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Sweep {
    SelectedDetail,
    AllFirstWrite,
}
#[derive(Clone, Copy, Debug, Serialize)]
struct RootCase {
    family: Family,
    living: usize,
}
impl RootCase {
    fn named(name: &str) -> Result<Self, String> {
        match name {
            "ordinary_4" => Ok(Self {
                family: Family::Ordinary,
                living: 4,
            }),
            "ordinary_3" => Ok(Self {
                family: Family::Ordinary,
                living: 3,
            }),
            "ordinary_2" => Ok(Self {
                family: Family::Ordinary,
                living: 2,
            }),
            "dense_4" => Ok(Self {
                family: Family::Dense,
                living: 4,
            }),
            "dense_3" => Ok(Self {
                family: Family::Dense,
                living: 3,
            }),
            "dense_2" => Ok(Self {
                family: Family::Dense,
                living: 2,
            }),
            _ => Err("undeclared legal branch root".into()),
        }
    }
    fn visit(self) -> u64 {
        match (self.family, self.living) {
            (Family::Ordinary, 4) => 3,
            (Family::Ordinary, 3) => 5,
            (Family::Ordinary, 2) => 7,
            (Family::Dense, 4) => 65,
            (Family::Dense, 3) => 69,
            (Family::Dense, 2) => 73,
            _ => unreachable!("RootCase constructed only by named case"),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
struct Counts {
    initial_queries: u64,
    query_resumptions: u64,
    submits: u64,
    advance_calls: u64,
    rule_steps: u64,
}
#[derive(Default, Serialize)]
struct Stages {
    creation_started: usize,
    created: usize,
    query_started: usize,
    queried: usize,
    validated: usize,
    submitted: usize,
    fully_advanced: usize,
    verified: usize,
}
#[derive(Serialize)]
struct Segment {
    operation: &'static str,
    branch: Option<usize>,
    nanos: u128,
}
#[derive(Serialize)]
struct Memory {
    rss_bytes: u64,
    high_water_bytes: u64,
}
fn memory() -> Result<Memory, String> {
    let raw = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let parse = |key: &str| -> Result<u64, String> {
        raw.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<u64>().ok())
            .and_then(|n| n.checked_mul(1024))
            .ok_or_else(|| "missingRSScounter".into())
    };
    Ok(Memory {
        rss_bytes: parse("VmRSS:")?,
        high_water_bytes: parse("VmHWM:")?,
    })
}
fn reserve() -> Result<bool, String> {
    Ok(memory()?.rss_bytes < MEMORY_LIMIT - RSS_RESERVE)
}
fn timed<T>(
    samples: &mut Vec<Segment>,
    operation: &'static str,
    branch: Option<usize>,
    f: impl FnOnce() -> T,
) -> T {
    let clock = Instant::now();
    let value = black_box(f());
    samples.push(Segment {
        operation,
        branch,
        nanos: clock.elapsed().as_nanos(),
    });
    value
}
fn query(g: &Game, mut result: QueryResult, counts: &mut Counts) -> Result<QueryValue, String> {
    counts.initial_queries += 1;
    for _ in 0..100_000 {
        match result {
            QueryResult::Complete(value) => return Ok(value),
            QueryResult::Incomplete { work, .. } => {
                counts.query_resumptions += 1;
                result = g.resume_work(work, Budget::new(64));
            }
            QueryResult::Rejected(e) => return Err(format!("legal branch query:{e:?}")),
        }
    }
    Err("legal branch query call bound".into())
}
fn observe(g: &Game, viewer: PlayerId, counts: &mut Counts) -> Result<Observation, String> {
    match query(g, g.observe(viewer, Budget::new(64)), counts)? {
        QueryValue::Observation(o) => Ok(o),
        _ => Err("observationresult".into()),
    }
}
fn ready(g: &mut Game, counts: &mut Counts, quantum: u64) -> Result<Request, String> {
    for _ in 0..100_000 {
        counts.advance_calls += 1;
        let before = g.rule_boundary();
        let outcome = g.advance(Budget::new(quantum));
        // Even an Err return may follow committed work. Retain its actual
        // boundary delta before classifying success, fault or truncation.
        counts.rule_steps += g.rule_boundary() - before;
        match outcome {
            Advance::AwaitingDecision(r) => return Ok(r),
            Advance::Suspended { .. } => {}
            other => return Err(format!("legal branch root stopped:{other:?}")),
        }
    }
    Err("legal branch advance call bound".into())
}
fn accept(
    g: &mut Game,
    o: &Observation,
    response: PlayerResponse,
    counts: &mut Counts,
) -> Result<(), String> {
    let request = o.decision.as_ref().ok_or("missingownrequest")?;
    let QueryValue::Validated(receipt) = query(
        g,
        g.check_player_response(
            o.view.viewer,
            request.token().clone(),
            response,
            Budget::new(64),
        ),
        counts,
    )?
    else {
        return Err("validationresult".into());
    };
    g.submit(receipt)?;
    counts.submits += 1;
    Ok(())
}
fn basic_land(catalog: &Catalog, o: &Observation) -> Option<ObjectHandle> {
    if o.view.active != Some(o.view.viewer)
        || o.view.turn.step != TurnStep::PrecombatMain
        || o.view.turn.lands_played != 0
    {
        return None;
    }
    o.view
        .objects
        .iter()
        .find(|row| {
            row.owner == o.view.viewer
                && row.zone
                    == (Zone::Hand {
                        owner: o.view.viewer,
                    })
                && catalog.definitions.iter().any(|d| {
                    d.oracle_id == row.oracle_id
                        && d.characteristics.kind == CardKind::Land
                        && d.characteristics.basic
                })
        })
        .map(|row| row.handle.clone())
}
fn choose(
    catalog: &Catalog,
    o: &Observation,
    mulls: [u8; 4],
    lands: bool,
) -> Result<PlayerResponse, String> {
    Ok(
        match o.decision.as_ref().ok_or("no public chooser request")? {
            PlayerRequest::Mulligan {
                player,
                mulligans_taken,
                can_mulligan,
                ..
            } => PlayerResponse::Mulligan {
                take: *can_mulligan && *mulligans_taken < mulls[player.0 as usize],
            },
            PlayerRequest::BottomOpening {
                count, candidates, ..
            } => PlayerResponse::BottomOpening {
                objects: candidates.iter().rev().take(*count).cloned().collect(),
            },
            PlayerRequest::Discard {
                count, candidates, ..
            } => PlayerResponse::Discard {
                objects: candidates.iter().rev().take(*count).cloned().collect(),
            },
            PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
            PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
            PlayerRequest::Priority { .. } => {
                if lands {
                    basic_land(catalog, o).map_or(PlayerResponse::Pass, |object| {
                        PlayerResponse::PlayLand { object }
                    })
                } else {
                    PlayerResponse::Pass
                }
            }
            other => return Err(format!("outside declared legal branch policy:{other:?}")),
        },
    )
}
fn catalog() -> Result<Catalog, String> {
    Catalog::read(include_str!(
        "../../../../tests/fixtures/expected-match-ir.json"
    ))
}
fn config(family: Family) -> MatchConfig {
    let domain = match family {
        Family::Ordinary => "legal-branches-v1/ordinary",
        Family::Dense => "legal-branches-v1/all-plains",
    };
    let hex = sha256(domain.as_bytes());
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
    }
    let mut c = MatchConfig::foundations(seed, PlayerId(0));
    if family == Family::Dense {
        c.decks = std::array::from_fn(|_| Decklist {
            commander: ISAMARU.into(),
            cards: vec![
                DeckEntry {
                    oracle_id: ISAMARU.into(),
                    count: 1,
                },
                DeckEntry {
                    oracle_id: PLAINS.into(),
                    count: 99,
                },
            ],
        })
    }
    c
}
struct Root {
    game: Game,
    case: RootCase,
    setup: Counts,
    visits: u64,
    accepted: usize,
}
fn prepare(case: RootCase) -> Result<Root, String> {
    let catalog = catalog()?;
    let prepared = Prepared::admit_pod(catalog.clone(), config(case.family))
        .map_err(|e| format!("legal admission:{e:?}"))?;
    assert!(prepared.match_config.is_some());
    assert_eq!(prepared.config.cards.len(), 400);
    assert_eq!(prepared.config.commanders.len(), 4);
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    let mut counts = Counts::default();
    let mut last = None;
    let mut visits = 0;
    let mut departed = [false; 2];
    let (p2, p1) = if case.family == Family::Ordinary {
        (5, 7)
    } else {
        (69, 73)
    };
    let mulligans = if case.family == Family::Ordinary {
        [0, 1, 2, 3]
    } else {
        [0; 4]
    };
    for _ in 0..50_000 {
        let r = ready(&mut game, &mut counts, 64)?;
        let o = observe(&game, r.player(), &mut counts)?;
        let p0main = o.view.viewer == PlayerId(0)
            && o.view.active == Some(PlayerId(0))
            && o.view.turn.step == TurnStep::PrecombatMain
            && matches!(o.decision, Some(PlayerRequest::Priority { .. }));
        if p0main && last != Some(o.view.turn.number) {
            last = Some(o.view.turn.number);
            visits += 1
        }
        if p0main && case.living <= 3 && visits == p2 && !departed[0] {
            game.concede(PlayerId(2))?;
            departed[0] = true;
            continue;
        }
        if p0main && case.living <= 2 && visits == p1 && !departed[1] {
            game.concede(PlayerId(1))?;
            departed[1] = true;
            continue;
        }
        let after_land = o.view.turn.lands_played != 0 || basic_land(&catalog, &o).is_none();
        if p0main && visits == case.visit() && (case.family == Family::Dense || after_land) {
            assert_eq!(game.live_players().len(), case.living);
            game.validate_state()?;
            assert_eq!(
                game.state.cards.len(),
                400,
                "departed physical facts retained"
            );
            if case.family == Family::Dense {
                assert_eq!(
                    game.state
                        .objects
                        .values()
                        .filter(|o| o.zone == Zone::Battlefield)
                        .count(),
                    case.living * 64
                )
            }
            let accepted = game.journal().len();
            assert_eq!(accepted as u64, counts.submits);
            return Ok(Root {
                game,
                case,
                setup: counts,
                visits,
                accepted,
            });
        }
        let lands = case.family == Family::Ordinary || visits < 65;
        let response = choose(&catalog, &o, mulligans, lands)?;
        accept(&mut game, &o, response, &mut counts)?;
        if counts.submits.is_multiple_of(64) {
            println!("LEGAL_BRANCH_ROOT_PROGRESS={}",canonical(&serde_json::json!({"case":case,"accepted_inputs":counts.submits,"work":counts,"P0_main_visits":visits,"living":game.live_players().len(),"current_battlefield":game.state.objects.values().filter(|o|o.zone==Zone::Battlefield).count(),"history_lengths":game.state.visibility.iter().map(|k|k.history.len()).collect::<Vec<_>>(),"pending_input_may_not_yet_be_consumed":game.state.pending.is_some()})).unwrap());
        }
    }
    Err("legal root fixed input bound exceeded; no replacementseed".into())
}

#[derive(Clone)]
struct Action {
    response: PlayerResponse,
    label: &'static str,
}
fn action(catalog: &Catalog, o: &Observation) -> Result<Action, String> {
    if o.view.viewer != PlayerId(0) || !matches!(o.decision, Some(PlayerRequest::Priority { .. })) {
        return Err("rootaction requires ownP0Priority".into());
    }
    Ok(match basic_land(catalog, o) {
        Some(object) => Action {
            response: PlayerResponse::PlayLand { object },
            label: "play_land",
        },
        None => Action {
            response: PlayerResponse::Pass,
            label: "pass",
        },
    })
}
fn progress(
    rep: usize,
    target: usize,
    sweep: Sweep,
    phase: &str,
    branch: Option<usize>,
    counts: &Stages,
) {
    println!("LEGAL_BRANCH_PROGRESS={}",canonical(&serde_json::json!({"repetition":rep,"warmup":rep==0,"requested":target,"sweep":sweep,"phase":phase,"branch":branch,"stages":counts})).unwrap());
}
#[derive(Clone, Copy)]
struct Trial {
    rep: usize,
    target: usize,
    sweep: Sweep,
}
struct BranchMetrics<'a> {
    stages: &'a mut Stages,
    samples: &'a mut Vec<Segment>,
    work: &'a mut Counts,
}
fn write_branch(
    g: &mut Game,
    catalog: &Catalog,
    expected_action: &Action,
    index: usize,
    trial: Trial,
    metrics: BranchMetrics<'_>,
) -> Result<(), String> {
    let Trial { rep, target, sweep } = trial;
    let BranchMetrics {
        stages,
        samples,
        work,
    } = metrics;
    stages.query_started += 1;
    progress(rep, target, sweep, "query_started", Some(index), stages);
    let o = timed(samples, "own_observation", Some(index), || {
        observe(g, PlayerId(0), work)
    })?;
    stages.queried += 1;
    progress(rep, target, sweep, "queried", Some(index), stages);
    let own = action(catalog, &o)?;
    if own.label != expected_action.label
        || canonical(&own.response)? != canonical(&expected_action.response)?
    {
        return Err("branchaction diverged fromownvisible reference".into());
    }
    let token = o
        .decision
        .as_ref()
        .ok_or("missingbranchrequest")?
        .token()
        .clone();
    let validation = timed(samples, "check_first_write", Some(index), || {
        query(
            g,
            g.check_player_response(PlayerId(0), token, own.response, Budget::new(64)),
            work,
        )
    })?;
    let QueryValue::Validated(receipt) = validation else {
        return Err("branchvalidationresult".into());
    };
    stages.validated += 1;
    progress(rep, target, sweep, "validated", Some(index), stages);
    timed(samples, "submit_first_write", Some(index), || {
        g.submit(receipt)
    })?;
    work.submits += 1;
    stages.submitted += 1;
    progress(
        rep,
        target,
        sweep,
        "submitted_not_yet_consumed",
        Some(index),
        stages,
    );
    let _ = timed(samples, "advance_first_write", Some(index), || {
        ready(g, work, 64)
    })?;
    if g.state.pending.is_some() {
        return Err("advance returned withunconsumed acceptedinput".into());
    }
    stages.fully_advanced += 1;
    progress(rep, target, sweep, "fully_advanced", Some(index), stages);
    Ok(())
}
struct Expected {
    game: Game,
    action: Action,
    root_sha: String,
    after_sha: String,
}
fn prove_root_and_reference(root: &Root) -> Result<Expected, String> {
    let game = &root.game;
    let before = oracle::state_sha(game);
    let journal = game.replay_journal();
    assert_eq!(journal.decisions.len(), game.journal().len());
    work::optimization_tests::compare_queries(game);
    let catalog = catalog()?;
    let mut counts = Counts::default();
    let o = observe(game, PlayerId(0), &mut counts)?;
    let chosen = action(&catalog, &o)?;
    let mut outcomes = vec![];
    for quantum in [1, u64::MAX] {
        let mut copy = game.fork();
        let before_inputs = copy.journal().len();
        let mut work = Counts::default();
        let obs = observe(&copy, PlayerId(0), &mut work)?;
        let same = action(&catalog, &obs)?;
        assert_eq!(canonical(&same.response)?, canonical(&chosen.response)?);
        accept(&mut copy, &obs, same.response, &mut work)?;
        let _ = ready(&mut copy, &mut work, quantum)?;
        assert_eq!(copy.journal().len(), before_inputs + 1);
        assert!(copy.state.pending.is_none());
        assert_ne!(
            copy.state, game.state,
            "firstwrite mustchange actualState evenwhenPass"
        );
        copy.validate_state()?;
        outcomes.push(copy);
    }
    let reference = outcomes.pop().unwrap();
    let other = outcomes.pop().unwrap();
    assert_eq!(reference.state, other.state);
    drop(other);
    let after = oracle::state_sha(&reference);
    for current in [game, &reference] {
        let snapshot = current.snapshot()?;
        let restored = Game::restore(current.prepared.as_ref().clone(), &snapshot)?;
        assert_eq!(restored.state, current.state);
        drop(restored);
        drop(snapshot);
        let replayed = Game::replay(current.prepared.as_ref().clone(), &current.replay_journal())?;
        assert_eq!(replayed.state, current.state);
        assert_eq!(oracle::state_sha(&replayed), current.state_digest()?);
        drop(replayed);
    }
    assert_eq!(game.state_digest()?, before);
    Ok(Expected {
        game: reference,
        action: chosen,
        root_sha: before,
        after_sha: after,
    })
}
fn selected(n: usize) -> BTreeSet<usize> {
    [0, n / 2, n.saturating_sub(1)]
        .into_iter()
        .filter(|i| *i < n)
        .collect()
}
fn sweep(
    root: &Root,
    expected: &Expected,
    kind: Sweep,
    rep: usize,
    target: usize,
) -> Result<serde_json::Value, String> {
    let catalog = catalog()?;
    let baseline = root.game.state.clone();
    let mut stages = Stages::default();
    let mut samples = vec![];
    let mut branches = vec![];
    let mut stop: Option<String> = None;
    println!("LEGAL_BRANCH_SWEEP_START={}",canonical(&serde_json::json!({"root":root.case,"repetition":rep,"warmup":rep==0,"requested":target,"sweep":kind,"baseline_state_sha256":expected.root_sha})).unwrap());
    let before = memory()?;
    let lifecycle = Instant::now();
    for index in 0..target {
        if !reserve()? {
            stop = Some("rss_reserve_before_next_fork".into());
            break;
        }
        stages.creation_started += 1;
        progress(rep, target, kind, "creation_started", Some(index), &stages);
        branches.push(timed(&mut samples, "fork", Some(index), || {
            root.game.fork()
        }));
        stages.created += 1;
        progress(rep, target, kind, "created_unmutated", Some(index), &stages);
        if memory()?.rss_bytes >= MEMORY_LIMIT {
            stop = Some("rss_hard_after_fork".into());
            break;
        }
    }
    let targets = match kind {
        Sweep::SelectedDetail => selected(branches.len()).into_iter().collect::<Vec<_>>(),
        Sweep::AllFirstWrite => (0..branches.len()).collect(),
    };
    let selected_indexes = selected(branches.len());
    let mut branch_counts = vec![];
    let mut completed = BTreeSet::new();
    let mut failed_branch = None;
    if stop.as_deref() != Some("rss_hard_after_fork") {
        for index in targets.iter().copied() {
            if !reserve()? {
                stop.get_or_insert("rss_reserve_before_next_firstwrite".into());
                break;
            }
            assert!(
                branches[index].state == root.game.state,
                "notyetwritten branch equals originalbaseline"
            );
            let mut work = Counts::default();
            let outcome = write_branch(
                &mut branches[index],
                &catalog,
                &expected.action,
                index,
                Trial {
                    rep,
                    target,
                    sweep: kind,
                },
                BranchMetrics {
                    stages: &mut stages,
                    samples: &mut samples,
                    work: &mut work,
                },
            );
            // Persist partial actual query/submit/advance counts even on error.
            branch_counts.push(serde_json::json!({"index":index,"work":work,"completed":outcome.is_ok(),"error":outcome.as_ref().err()}));
            match outcome {
                Ok(()) => {
                    assert!(
                        branches[index].state == expected.game.state,
                        "completed firstwrite fullState reference"
                    );
                    assert_eq!(
                        branches[index].journal().len(),
                        root.game.journal().len() + 1
                    );
                    assert!(branches[index].state.pending.is_none());
                    stages.verified += 1;
                    progress(
                        rep,
                        target,
                        kind,
                        "verified_full_State",
                        Some(index),
                        &stages,
                    );
                    completed.insert(index);
                }
                Err(error) => {
                    failed_branch = Some(index);
                    stop = Some(format!("firstwrite_failed:{error}"));
                    break;
                }
            }
            assert!(
                root.game.state == baseline,
                "parent remainssemanticallyunchanged"
            );
            if memory()?.rss_bytes >= MEMORY_LIMIT {
                stop = Some("rss_hard_after_firstwrite".into());
                break;
            }
        }
    }
    let retained = branches.len();
    let useful = stages.verified;
    let at_retention = memory()?;
    // References/sibling/hash checks lie outside production operation clocks.
    // Mutated branches remain retained until ALL declared firstwrites finish.
    for (index, branch) in branches.iter().enumerate() {
        let was_written = completed.contains(&index);
        if was_written {
            assert!(
                branch.state == expected.game.state,
                "completed branch reference"
            )
        } else if failed_branch != Some(index) {
            assert!(branch.state == baseline, "untouchedsibling")
        }
    }
    for index in selected_indexes {
        if completed.contains(&index) {
            assert_eq!(oracle::state_sha(&branches[index]), expected.after_sha);
            // A newly deserialized standalone State has empty derived memos.
            // It is never installed in a Game or used to bypass admission.
            let cold = oracle::cold_state(&branches[index]);
            let cold_sha = timed(
                &mut samples,
                "selected_full_state_hash_cold",
                Some(index),
                || state_digest::state_digest(&cold),
            )?;
            assert_eq!(cold_sha, expected.after_sha);
            let warm_sha = timed(
                &mut samples,
                "selected_full_state_hash_warm",
                Some(index),
                || state_digest::state_digest(&cold),
            )?;
            assert_eq!(warm_sha, expected.after_sha);
            timed(
                &mut samples,
                "dispose_selected_cold_state",
                Some(index),
                || drop(cold),
            );
        }
    }
    timed(&mut samples, "dispose_all_retained", None, || {
        drop(branches)
    });
    let elapsed = lifecycle.elapsed().as_nanos();
    assert_eq!(root.game.state_digest()?, expected.root_sha);
    let full_handles = retained == target && stop.is_none();
    let all_useful = kind == Sweep::AllFirstWrite && useful == target && full_handles;
    let selected_complete =
        kind == Sweep::SelectedDetail && useful == targets.len() && full_handles;
    let row = serde_json::json!({"root":root.case,"repetition":rep,"warmup":rep==0,"sweep":kind,"requested":target,"retained":retained,"stages":stages,"branch_counts":branch_counts,"resource_or_fault_shortfall":stop,"failed_branch_index":failed_branch,"requested_handles_created_without_stop":full_handles,"all_requested_firstwrites_verified":all_useful,"selected_detail_complete":selected_complete,"useful_firstwrite_branches":useful,"untouched_or_incomplete_retained_branches":retained-useful,"created_handles_are_not_useful_branches":true,"baseline_state_sha256":expected.root_sha,"firstwrite_state_sha256":expected.after_sha,"actual_action":expected.action.label,"actual_response":expected.action.response,"memory_before":before,"memory_at_retention":at_retention,"memory_after_disposal":memory()?,"samples":samples,"inclusive_lifecycle_ns":elapsed,"five_sweeps_not_tail_precision":true,"legal_match_admission":true});
    println!("LEGAL_BRANCH_SWEEP_RESULT={}", canonical(&row)?);
    Ok(row)
}

#[test]
fn legal_branch_contract_preserves_real_admission_and_all_firstwrite_denominators() {
    let value: serde_json::Value = serde_json::from_str(LEGAL_BRANCH_CONTRACT).unwrap();
    assert_eq!(
        value["retained_counts"],
        serde_json::json!([64, 256, 1024, 10000])
    );
    assert_eq!(value["warmup_repetitions"], 1);
    assert_eq!(value["measured_repetitions"], 5);
    let c = catalog().unwrap();
    for family in [Family::Ordinary, Family::Dense] {
        let prepared = Prepared::admit_pod(c.clone(), config(family)).unwrap();
        assert_eq!(prepared.config.cards.len(), 400);
        assert_eq!(prepared.config.commanders.len(), 4);
        assert!(prepared.match_config.is_some());
    }
    assert_eq!(selected(0), BTreeSet::new());
    assert_eq!(selected(1), BTreeSet::from([0]));
    assert_eq!(selected(64), BTreeSet::from([0, 32, 63]));
    assert!(RootCase::named("seed-filtered-fallback").is_err());
}

#[test]
fn legal_branch_small_public_prefix_proves_fresh_receipts_fullbytes_and_sibling_isolation() {
    let c = catalog().unwrap();
    let prepared = Prepared::admit_pod(c.clone(), config(Family::Dense)).unwrap();
    let mut g = Game::new(prepared);
    let mut work = Counts::default();
    for _ in 0..2000 {
        let r = ready(&mut g, &mut work, 64).unwrap();
        let o = observe(&g, r.player(), &mut work).unwrap();
        if o.view.viewer == PlayerId(0)
            && o.view.active == Some(PlayerId(0))
            && o.view.turn.step == TurnStep::PrecombatMain
            && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
        {
            break;
        }
        let response = choose(&c, &o, [0; 4], false).unwrap();
        accept(&mut g, &o, response, &mut work).unwrap();
    }
    let root = Root {
        game: g,
        case: RootCase {
            family: Family::Dense,
            living: 4,
        },
        setup: work,
        visits: 1,
        accepted: 0,
    };
    let expected = prove_root_and_reference(&root).unwrap();
    assert_eq!(expected.action.label, "play_land");
    let whole = canonical(&root.game.state).unwrap();
    let mut generic = vec![];
    oracle::generic_bytes(&root.game.state, &mut generic);
    assert_eq!(whole.as_bytes(), generic);
    let o = observe(&root.game, PlayerId(0), &mut Counts::default()).unwrap();
    let token = o.decision.as_ref().unwrap().token().clone();
    let QueryValue::Validated(parent_receipt) = query(
        &root.game,
        root.game.check_player_response(
            PlayerId(0),
            token,
            expected.action.response.clone(),
            Budget::new(64),
        ),
        &mut Counts::default(),
    )
    .unwrap() else {
        panic!("parentreceipt")
    };
    let mut fork = root.game.fork();
    assert!(fork.submit(parent_receipt).is_err());
    assert_eq!(fork.state, root.game.state);
    // Correctness-only small worklist, not the ignored64..10000 measurements.
    let mut stages = Stages::default();
    let mut samples = vec![];
    for index in 0..4 {
        let mut branch = root.game.fork();
        let mut work = Counts::default();
        write_branch(
            &mut branch,
            &c,
            &expected.action,
            index,
            Trial {
                rep: 0,
                target: 4,
                sweep: Sweep::AllFirstWrite,
            },
            BranchMetrics {
                stages: &mut stages,
                samples: &mut samples,
                work: &mut work,
            },
        )
        .unwrap();
        assert_eq!(branch.state, expected.game.state);
        assert_eq!(root.game.state_digest().unwrap(), expected.root_sha);
    }
    assert_eq!(stages.queried, 4);
    assert_eq!(stages.submitted, 4);
    assert_eq!(stages.fully_advanced, 4);
}

#[test]
#[ignore = "explicit legal branch host worklist; independent120s/2GiB outerguard required"]
fn legal_branch_host_diagnostic() {
    assert_eq!(
        std::env::var("LEGAL_BRANCH_OUTER_GUARD").unwrap(),
        "120s-2gib-v1"
    );
    let case = RootCase::named(&std::env::var("LEGAL_BRANCH_CASE").unwrap()).unwrap();
    let kind = match std::env::var("LEGAL_BRANCH_SWEEP").unwrap().as_str() {
        "selected_detail" => Sweep::SelectedDetail,
        "all_first_write" => Sweep::AllFirstWrite,
        _ => panic!("undeclared legalsweep"),
    };
    println!("LEGAL_BRANCH_ROOT_START={}",canonical(&serde_json::json!({"case":case,"build_id":BUILD_ID,"contract_sha256":sha256(LEGAL_BRANCH_CONTRACT.as_bytes()),"contract":serde_json::from_str::<serde_json::Value>(LEGAL_BRANCH_CONTRACT).unwrap(),"sweep":kind})).unwrap());
    let root = prepare(case).unwrap();
    let expected = prove_root_and_reference(&root).unwrap();
    println!("LEGAL_BRANCH_ROOT_PROOF={}",canonical(&serde_json::json!({"case":case,"visits":root.visits,"setup_accepted":root.accepted,"setup_work":root.setup,"physical_config_cards":root.game.prepared.config.cards.len(),"physical_card_facts":root.game.state.cards.len(),"current_objects":root.game.state.objects.len(),"battlefield":root.game.state.objects.values().filter(|o|o.zone==Zone::Battlefield).count(),"history_entries":root.game.state.visibility.iter().map(|k|k.history.len()).collect::<Vec<_>>(),"own_knowledge_action_counts":root.game.state.visibility.iter().map(|k|k.own_knowledge.len()).collect::<Vec<_>>(),"living_players":root.game.live_players(),"profile_digest":root.game.prepared.profile_digest,"raw_journal":root.game.replay_journal(),"root_state_sha256":expected.root_sha,"firstwrite_state_sha256":expected.after_sha,"actual_response":expected.action.response,"all_fullbytes_query_partition_restore_replay_checks":true})).unwrap());
    let mut rows = vec![];
    for repetition in 0..6 {
        for requested in [64, 256, 1024, 10000] {
            rows.push(sweep(&root, &expected, kind, repetition, requested).unwrap());
        }
    }
    println!("LEGAL_BRANCH_RESULT={}",canonical(&serde_json::json!({"schema":"legal-branch-diagnostic-result-v1","build_id":BUILD_ID,"case":case,"sweep":kind,"contract_sha256":sha256(LEGAL_BRANCH_CONTRACT.as_bytes()),"contract":serde_json::from_str::<serde_json::Value>(LEGAL_BRANCH_CONTRACT).unwrap(),"warmup_sweeps":4,"measured_sweeps":20,"rows":rows,"outer_guard_declared_not_proven_here":true,"latency_or_strategic_strength_claim":false,"initial400_card_legal_admission":true})).unwrap());
}

// Same independently reviewed whole-View generic oracle/comparator as the
// source-only dense proof candidate1091616c; private to this diagnostic.
mod oracle {
    use super::*;
    fn generic_state_to<W: std::io::Write>(state: &State, out: &mut W) {
        use std::io::Write;
        fn json<W: Write, T: Serialize>(out: &mut W, value: &T) {
            serde_json::to_writer(out, value).unwrap();
        }
        let mut skeleton = state.clone();
        for k in &mut skeleton.visibility {
            k.history.clear();
        }
        let value = serde_json::to_value(&skeleton).unwrap();
        drop(skeleton);
        out.write_all(b"{").unwrap();
        let fields = value.as_object().unwrap();
        let mut visibility_seen = false;
        for (index, (key, value)) in fields.iter().enumerate() {
            if index != 0 {
                out.write_all(b",").unwrap();
            }
            json(out, key);
            out.write_all(b":").unwrap();
            if key != "visibility" {
                json(out, value);
                continue;
            }
            visibility_seen = true;
            let rows = value.as_array().unwrap();
            assert_eq!(rows.len(), state.visibility.len());
            out.write_all(b"[").unwrap();
            for (index, (row, knowledge)) in rows.iter().zip(&state.visibility).enumerate() {
                if index != 0 {
                    out.write_all(b",").unwrap();
                }
                out.write_all(b"{").unwrap();
                let mut history_seen = false;
                for (index, (key, value)) in row.as_object().unwrap().iter().enumerate() {
                    if index != 0 {
                        out.write_all(b",").unwrap();
                    }
                    json(out, key);
                    out.write_all(b":").unwrap();
                    if key != "history" {
                        json(out, value);
                        continue;
                    }
                    history_seen = true;
                    assert_eq!(value, &serde_json::json!([]));
                    out.write_all(b"[").unwrap();
                    for (index, view) in knowledge.history.iter().enumerate() {
                        if index != 0 {
                            out.write_all(b",").unwrap();
                        }
                        let generic = serde_json::to_value(view).unwrap();
                        json(out, &generic);
                    }
                    out.write_all(b"]").unwrap();
                }
                assert!(history_seen);
                out.write_all(b"}").unwrap();
            }
            out.write_all(b"]").unwrap();
        }
        assert!(visibility_seen);
        out.write_all(b"}").unwrap();
    }
    /// Test-oracle-only sink. Every production-emitted byte must match the generic
    /// reader, with bounded scratch storage, exact length/EOF, and a complete SHA.
    /// An I/O/mismatch error permanently poisons the sink; no digest is returned.
    struct CanonicalByteComparator<R> {
        expected: R,
        expected_len: u64,
        count: u64,
        hash: Sha256Sink,
        failed: bool,
        scratch: [u8; 65536],
    }
    impl<R: std::io::Read> CanonicalByteComparator<R> {
        fn new(expected: R, expected_len: u64) -> Self {
            Self {
                expected,
                expected_len,
                count: 0,
                hash: Sha256Sink::new(),
                failed: false,
                scratch: [0; 65536],
            }
        }
        fn fail(&mut self, kind: std::io::ErrorKind, message: &'static str) -> std::io::Error {
            self.failed = true;
            std::io::Error::new(kind, message)
        }
        fn finish(mut self) -> std::io::Result<(String, u64)> {
            std::io::Write::flush(&mut self)?;
            if self.count != self.expected_len {
                return Err(self.fail(
                    std::io::ErrorKind::UnexpectedEof,
                    "production stream truncated",
                ));
            }
            let mut extra = [0u8; 1];
            match self.expected.read(&mut extra) {
                Ok(0) => Ok((self.hash.finalize_hex(), self.count)),
                Ok(_) => Err(self.fail(std::io::ErrorKind::InvalidData, "generic stream appended")),
                Err(error) => {
                    self.failed = true;
                    Err(error)
                }
            }
        }
    }
    impl<R: std::io::Read> std::io::Write for CanonicalByteComparator<R> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.failed {
                return Err(self.fail(
                    std::io::ErrorKind::InvalidData,
                    "canonical comparator poisoned",
                ));
            }
            let Ok(length) = u64::try_from(bytes.len()) else {
                return Err(self.fail(
                    std::io::ErrorKind::InvalidData,
                    "canonical byte count overflow",
                ));
            };
            let Some(next) = self.count.checked_add(length) else {
                return Err(self.fail(
                    std::io::ErrorKind::InvalidData,
                    "canonical byte count overflow",
                ));
            };
            if next > self.expected_len {
                return Err(self.fail(
                    std::io::ErrorKind::InvalidData,
                    "production stream appended",
                ));
            }
            for chunk in bytes.chunks(self.scratch.len()) {
                if let Err(error) = self.expected.read_exact(&mut self.scratch[..chunk.len()]) {
                    self.failed = true;
                    return Err(error);
                }
                if &self.scratch[..chunk.len()] != chunk {
                    return Err(
                        self.fail(std::io::ErrorKind::InvalidData, "canonical byte mismatch")
                    );
                }
                self.hash.update(chunk);
            }
            self.count = next;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            // This sink owns no output buffer/file: each write synchronously compares
            // and hashes its bytes. The generic file was flushed before it was opened.
            if self.failed {
                Err(self.fail(
                    std::io::ErrorKind::InvalidData,
                    "canonical comparator poisoned",
                ))
            } else {
                Ok(())
            }
        }
    }

    fn state_oracle_raw_keep(state: &State, retain: Option<&std::path::Path>) -> String {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../generated/performance-verification/legal-branch-oracle-work");
        std::fs::create_dir_all(&base).unwrap();
        let directory = base.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let generic = directory.join("generic.json");
        let open = |p: &std::path::Path| {
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(p)
                .unwrap()
        };
        {
            let mut out = std::io::BufWriter::new(open(&generic));
            generic_state_to(state, &mut out);
            out.flush().unwrap();
        }
        let generic_len = std::fs::metadata(&generic).unwrap().len();
        let reader = std::io::BufReader::new(std::fs::File::open(&generic).unwrap());
        let mut compared = CanonicalByteComparator::new(reader, generic_len);
        {
            // Coalesce tiny serializer fragments before comparing/hashing. This is
            // bounded I/O buffering only: the complete byte stream still must match.
            let mut out = std::io::BufWriter::with_capacity(65536, &mut compared);
            state_digest::write_state(state, &mut out).unwrap();
            out.flush().unwrap();
        }
        compared.flush().unwrap();
        let (expected, actual_len) = compared.finish().unwrap();
        assert_eq!(actual_len, generic_len);
        assert_eq!(state_digest::state_digest(state).unwrap(), expected);
        if let Some(path) = retain {
            std::fs::hard_link(&generic, path).unwrap();
        }
        expected
    }

    pub(super) fn cold_state(game: &Game) -> State {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../generated/performance-verification/legal-branch-oracle-work");
        std::fs::create_dir_all(&base).unwrap();
        let directory = base.join(format!(
            "cold-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let file = directory.join("generic-state.json");
        {
            let raw = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&file)
                .unwrap();
            let mut out = std::io::BufWriter::new(raw);
            generic_state_to(&game.state, &mut out);
            out.flush().unwrap();
        }
        let state: State =
            serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(&file).unwrap()))
                .unwrap();
        assert!(
            state == game.state,
            "cold typedState is the complete original semanticState"
        );
        state
    }
    pub(super) fn state_sha(game: &Game) -> String {
        state_oracle_raw_keep(&game.state, None)
    }
    pub(super) fn generic_bytes(state: &State, out: &mut impl std::io::Write) {
        generic_state_to(state, out)
    }
}

// Correctness-only accounting fixture: the old finite combat scenario supplies
// a real accepted public Pass. The structural overflow below is explicitly a
// private failure domain, never an admitted legal-match position.
fn accounting_pending_pass() -> Game {
    let prepared = Prepared::admit(
        Catalog::read(include_str!("../../../../tests/fixtures/expected-ir.json")).unwrap(),
        ScenarioConfig::combat_game(),
    )
    .unwrap();
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    let mut setup = Counts::default();
    let request = ready(&mut game, &mut setup, u64::MAX).unwrap();
    let observation = observe(&game, request.player(), &mut setup).unwrap();
    accept(&mut game, &observation, PlayerResponse::Pass, &mut setup).unwrap();
    assert!(game.state.pending.is_some());
    game
}
#[test]
fn legal_ready_counts_all_committed_steps_across_successful_budget_partitions() {
    let root = accounting_pending_pass();
    let mut expected = None;
    for quantum in [1, 64, u64::MAX] {
        let mut game = root.fork();
        let before = game.rule_boundary();
        let mut work = Counts::default();
        ready(&mut game, &mut work, quantum).unwrap();
        assert_eq!(work.rule_steps, game.rule_boundary() - before);
        assert!(work.rule_steps > 1);
        assert!(work.advance_calls >= 1);
        if quantum == 1 {
            assert!(work.advance_calls > 1);
        }
        if let Some(state) = &expected {
            assert_eq!(&game.state, state);
        } else {
            expected = Some(game.state.clone());
        }
    }
}
#[test]
fn legal_ready_counts_committed_prefix_when_later_step_faults() {
    let mut game = accounting_pending_pass();
    game.state.rule_steps = u64::MAX - 1;
    let before = game.rule_boundary();
    let mut work = Counts::default();
    assert!(ready(&mut game, &mut work, u64::MAX).is_err());
    assert_eq!(work.advance_calls, 1);
    assert_eq!(work.rule_steps, 1);
    assert_eq!(work.rule_steps, game.rule_boundary() - before);
    assert!(game.state.pending.is_none());
}
