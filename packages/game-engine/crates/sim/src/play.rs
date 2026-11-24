//! Match execution and evidence verification. The caller owns timing and journal storage.
use crate::policy::{Policy, Strategy};
use mtg_contracts::{Catalog, sha256, strict_read};
use mtg_core::{
    Advance, Budget, BuildIdentity, Game, MatchConfig, Observation, PlayerId, Prepared,
    QueryResult, QueryValue, ReplayJournal, SnapshotVerifier, TraceMode,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
mod replay_reuse_tests;
pub(crate) const DECISION_LIMIT: usize = 100_000;
pub(crate) const TURN_LIMIT: u64 = 1_000;
pub(crate) const WORK_LIMIT: usize = 2_000_000;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub decisions: usize,
    pub rule_boundary: u64,
    pub snapshot: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
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
pub(crate) fn complete(
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
pub(crate) fn observe(
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
/// Run an admitted match and emit the live journal in order. A sink error
/// stops the run. Hosts that report wall time fill `Evidence::elapsed_millis`
/// after flushing their sink. Supply the identity installed in `mtg_core`.
pub fn run(
    build: BuildIdentity,
    catalog: &Catalog,
    config: MatchConfig,
    policy: Policy,
    mulligans: [u8; 4],
    quantum: u64,
    mut sink: Option<&mut dyn crate::events::EventSink>,
    mut record: impl FnMut(&serde_json::Value) -> Result<(), String>,
) -> Result<Evidence, String> {
    if quantum == 0 {
        return Err("positive match quantum required".into());
    }
    let prepared =
        Prepared::admit_pod(catalog.clone(), config.clone()).map_err(|e| format!("{e:?}"))?;
    let trace_mode = if sink.is_some() {
        TraceMode::Stream
    } else if config.seed == seed_for(&config.profile, 0) || policy == Policy::DrawOnly {
        TraceMode::Detailed
    } else {
        TraceMode::Off
    };
    let mut game = Game::with_trace(prepared, trace_mode);
    record(
        &serde_json::json!({"kind":"header","build_id":build.id,"config":config,"policy":policy,"mulligans":mulligans}),
    )?;
    let mut strategy = Strategy::new(catalog, policy, mulligans);
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
    let mut stream_losses = Vec::new();
    let mut placements = crate::pods::Placements::default();
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
        if let Some(sink) = sink.as_deref_mut() {
            let mut result = Ok(());
            game.drain_trace(&mut |record| {
                if let mtg_core::EvidenceRecord::Event { event } = &record.record
                    && let mtg_core::Event::Committed {
                        actual: mtg_core::EventOutcome::Lost { player, reason },
                        ..
                    } = event.as_ref()
                {
                    stream_losses.push((*player, *reason));
                }
                if result.is_ok() {
                    result = sink.record(record);
                }
            });
            result?;
            placements.observe(&game.live_players());
        }
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
                    && ((checkpoints.len() == 1 && matches!(r.reason, mtg_core::Reason::Priority))
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
                let response_result = (|| -> Result<Observation, String> {
                    let observation =
                        observe(&game, r.player, quantum, &mut work, &mut query_calls)?;
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
                        .id
                        .clone();
                    match complete(
                        &game,
                        || {
                            game.check_player_response(
                                r.player,
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
                    Ok(observation)
                })();
                match response_result {
                    Ok(o) => {
                        if let Some(sink) = sink.as_deref_mut() {
                            sink.decision(
                                &o,
                                &game
                                    .journal()
                                    .last()
                                    .ok_or("missing accepted answer")?
                                    .answer,
                            )?;
                        }
                    }
                    Err(e) => {
                        outcome = format!("policy_or_query_failure: {e}");
                        break;
                    }
                }
                record(
                    &serde_json::json!({"kind":"accepted","input":game.journal().last().ok_or("missing accepted journal entry")?}),
                )?;
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
    record(
        &serde_json::json!({"kind":"footer","outcome":outcome,"winner":winner,"state_sha256":final_state_sha256,"rule_boundary":game.rule_boundary()}),
    )?;
    if let Some(sink) = sink {
        if let Some(w) = winner {
            placements.ranks[usize::from(w.0)] = Some(1);
        }
        sink.finish(&crate::events::PodOutcome {
            index: 0,
            slots: [0, 1, 2, 3],
            starter: usize::from(config.starting_player.0),
            terminal: matches!(outcome.as_str(), "finished" | "draw"),
            decision_capped: decisions >= DECISION_LIMIT,
            budget_capped: matches!(outcome.as_str(), "advance_limit" | "host_limit"),
            winner,
            placements: placements.ranks,
            decisions,
            turns: game.turn().number,
        })?;
    }
    let survivors = game.live_players();
    Ok(Evidence {
        config,
        policy,
        mulligans,
        journal: game.replay_journal(),
        trace_mode,
        losses: if trace_mode == TraceMode::Stream {
            stream_losses
        } else {
            losses(&game)
        },
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
        elapsed_millis: 0,
    })
}
pub fn seed_for(profile: &str, index: u32) -> [u8; 32] {
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
pub fn verify(catalog: &Catalog, e: &Evidence) -> Result<(), String> {
    verify_replay(catalog, e).map(|_| ())
}
// Standalone verification constructs and checks its own independent replay.
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
    if matches!(e.trace_mode, TraceMode::Detailed | TraceMode::Stream) && loss_facts != e.losses {
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
fn decode_live(
    build: BuildIdentity,
    mut reader: impl std::io::BufRead,
) -> Result<DecodedLive, String> {
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
                if build_id != build.id {
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
pub fn replay_live(
    build: BuildIdentity,
    profile: &str,
    catalog: &Catalog,
    reader: impl std::io::BufRead,
) -> Result<serde_json::Value, String> {
    let decoded = decode_live(build, reader)?;
    if decoded.header.config.profile != profile {
        return Err("replay profile mismatch".into());
    }
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
        if request.player != input.player || request.id != input.decision_id {
            return Err("live accepted-input order or identity mismatch".into());
        }
        game.respond(input.player, input.decision_id, input.answer.clone())?;
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
#[cfg(test)]
mod admission_and_host_tests {
    use super::*;
    use crate::BUILD;
    use std::cell::Cell;

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
        serde_json::json!({"kind":"header","build_id":BUILD.id,"config":config,"policy":"develop","mulligans":[0,0,0,0]})
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
        assert!(matches!(request.reason, mtg_core::Reason::Mulligan { .. }));
        game.respond(
            request.player,
            request.id,
            request
                .pick_one(&mtg_core::Candidate::Flag {
                    flag: mtg_core::Flag::Keep,
                })
                .unwrap(),
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
        let replay = replay_live(
            BUILD,
            "commander-foundations-v1",
            &catalog,
            std::io::Cursor::new(&bytes),
        )
        .unwrap();
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
        let replay = replay_live(
            BUILD,
            "commander-foundations-v1",
            &catalog,
            std::io::Cursor::new(torn),
        )
        .unwrap();
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
        let replay = replay_live(
            BUILD,
            "commander-foundations-v1",
            &catalog,
            std::io::Cursor::new(live_bytes(&records)),
        )
        .unwrap();
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
        replay_live(
            BUILD,
            "commander-foundations-v1",
            &catalog,
            std::io::Cursor::new(live_bytes(&records)),
        )
        .unwrap();
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
                replay_live(
                    BUILD,
                    "commander-foundations-v1",
                    &catalog,
                    std::io::Cursor::new(live_bytes(&bad))
                )
                .is_err(),
                "accepted forged {field}"
            );
        }
        records[footer_index]
            .as_object_mut()
            .unwrap()
            .remove("winner");
        assert!(decode_live(BUILD, std::io::Cursor::new(live_bytes(&records))).is_err());
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
            assert!(decode_live(BUILD, std::io::Cursor::new(live_bytes(&invalid))).is_err());
        }
        let mut bad_header = header.clone();
        bad_header["build_id"] = serde_json::json!("other-build");
        assert!(decode_live(BUILD, std::io::Cursor::new(live_bytes(&[bad_header]))).is_err());
        let mut unknown = header.clone();
        unknown["future_field"] = serde_json::json!(true);
        assert!(decode_live(BUILD, std::io::Cursor::new(live_bytes(&[unknown]))).is_err());
        for tail in [
            b"{not-json}\n".as_slice(),
            b"{\n".as_slice(),
            br#"{"kind":"accepted","kind":"accepted","input":{}}"#.as_slice(),
        ] {
            let mut bytes = live_bytes(std::slice::from_ref(&header));
            bytes.extend_from_slice(tail);
            assert!(decode_live(BUILD, std::io::Cursor::new(bytes)).is_err());
        }
        assert!(
            replay_live(
                BUILD,
                "commander-foundations-v1",
                &catalog,
                std::io::Cursor::new(live_bytes(&[header, accepted.clone(), accepted]))
            )
            .is_err()
        );
        assert!(
            decode_live(
                BUILD,
                std::io::Cursor::new(vec![b' '; MAX_LIVE_RECORD_BYTES + 1])
            )
            .is_err()
        );
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
}
