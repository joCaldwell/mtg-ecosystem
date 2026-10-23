//! Private diagnostic child of match_games::ordinary_diagnostic.
//! CR508.1, 601.2, 701.17, 800.4; pinned20260925 rules already used by core.
//! Policies consume only each chooser's current Observation/public Catalog.
use super::*;
use mtg_contracts::KeywordAbility;
use mtg_core::Request;
use std::collections::BTreeSet;
use std::io::{BufWriter, Seek, SeekFrom};

const CHOICE_SEED: &[u8] = b"foundations-shrinking-choices-v1/legal-BG-W-BG-W";
const SETUP_INPUT_LIMIT: usize = 30_000;
const CALL_LIMIT: usize = 100_000;
const CHOICE_REPETITIONS: usize = 6;
const P0: PlayerId = PlayerId(0);

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "choice", rename_all = "snake_case")]
enum Cell {
    Attack { digits: [usize; 3] },
    Edict { target: PlayerId },
}
#[derive(Clone, Default, Debug, Serialize, PartialEq, Eq)]
struct WorkCounts {
    fork_started: usize,
    forked: usize,
    initial_queries: usize,
    query_resumes: usize,
    validated_inputs: usize,
    submitted_inputs: usize,
    consumed_inputs: usize,
    advance_calls: usize,
    rule_steps: u64,
    fully_advanced: usize,
    verified: usize,
}
#[derive(Debug, Serialize)]
struct Segment {
    operation: &'static str,
    nanos: u128,
}
#[derive(Default, Serialize)]
struct Meter {
    counts: WorkCounts,
    segments: Vec<Segment>,
    /// Complete permitted request domains, rather than an invented domain size.
    requests: Vec<Value>,
    responses: Vec<Value>,
    processed: Vec<mtg_core::InputProcessingFact>,
}
impl Meter {
    fn timed<T>(&mut self, operation: &'static str, f: impl FnOnce() -> T) -> T {
        let clock = Instant::now();
        let out = f();
        self.segments.push(Segment {
            operation,
            nanos: clock.elapsed().as_nanos(),
        });
        out
    }
}
fn query(
    g: &Game,
    first: impl FnOnce() -> QueryResult,
    q: u64,
    m: &mut Meter,
    name: &'static str,
) -> Result<QueryValue, String> {
    m.counts.initial_queries += 1;
    let mut out = m.timed(name, first);
    for _ in 0..CALL_LIMIT {
        match out {
            QueryResult::Complete(v) => return Ok(v),
            QueryResult::Rejected(e) => return Err(format!("choice query:{e:?}")),
            QueryResult::Incomplete { work, .. } => {
                m.counts.query_resumes += 1;
                out = m.timed("query_resume", || g.resume_work(work, Budget::new(q)));
            }
        }
    }
    Err("choice query bound, no smaller domain substituted".into())
}
fn own_observation(g: &Game, p: PlayerId, q: u64, m: &mut Meter) -> Result<Observation, String> {
    match query(
        g,
        || g.observe(p, Budget::new(q)),
        q,
        m,
        "observation_query",
    )? {
        QueryValue::Observation(o) => Ok(o),
        _ => Err("choice observation type".into()),
    }
}
fn ready(g: &mut Game, q: u64, m: &mut Meter) -> Result<Request, String> {
    for _ in 0..CALL_LIMIT {
        m.counts.advance_calls += 1;
        let before = g.rule_boundary();
        let report = m.timed("advance_with_processing_facts", || {
            g.advance_with_report(Budget::new(q))
        });
        m.counts.rule_steps += g
            .rule_boundary()
            .checked_sub(before)
            .ok_or("boundary regressed")?;
        // A receipt is consumed only when a committed core step says so. This
        // does not claim that the action or spell resolved successfully.
        for fact in report.processed {
            if matches!(fact.identity, mtg_core::CoreInputIdentity::Decision { .. }) {
                if fact.cause != mtg_core::InputProcessingCause::RulesStep {
                    return Err("unexpected branch departure".into());
                }
                m.counts.consumed_inputs += 1;
            }
            m.processed.push(fact);
        }
        match report.advance {
            Advance::Suspended { .. } => {}
            Advance::AwaitingDecision(r) => return Ok(r),
            other => return Err(format!("choice branch stopped:{other:?}")),
        }
    }
    Err("choice advance bound".into())
}
fn accept(
    g: &mut Game,
    o: &Observation,
    r: PlayerResponse,
    q: u64,
    m: &mut Meter,
) -> Result<(), String> {
    let request = o.decision.as_ref().ok_or("missing own decision")?;
    m.requests
        .push(serde_json::to_value(request).map_err(|e| e.to_string())?);
    m.responses
        .push(serde_json::to_value(&r).map_err(|e| e.to_string())?);
    let QueryValue::Validated(receipt) = query(
        g,
        || g.check_player_response(o.view.viewer, request.token().clone(), r, Budget::new(q)),
        q,
        m,
        "check_response",
    )?
    else {
        return Err("receipt query type".into());
    };
    m.counts.validated_inputs += 1;
    m.timed("submit", || g.submit(receipt))?;
    m.counts.submitted_inputs += 1;
    Ok(())
}
fn profile_config() -> MatchConfig {
    let hex = sha256(CHOICE_SEED);
    let seed = std::array::from_fn(|i| {
        u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("SHA256 hex")
    });
    let mut config = MatchConfig::foundations(seed, P0);
    config.decks = [
        mtg_core::black_green_deck(),
        mtg_core::white_deck(),
        mtg_core::black_green_deck(),
        mtg_core::white_deck(),
    ];
    config
}
fn named<'a>(catalog: &'a Catalog, name: &str) -> Result<&'a mtg_contracts::Definition, String> {
    let mut found = catalog.definitions.iter().filter(|d| d.name == name);
    let d = found.next().ok_or("source definition missing")?;
    if found.next().is_some() {
        return Err("ambiguous archived name".into());
    }
    Ok(d)
}
fn edict_object(catalog: &Catalog, o: &Observation) -> Result<Option<ObjectHandle>, String> {
    let id = &named(catalog, "Cruel Edict")?.oracle_id;
    Ok(o.view
        .objects
        .iter()
        .find(|x| x.owner == P0 && x.zone == Zone::Hand { owner: P0 } && &x.oracle_id == id)
        .map(|x| x.handle.clone()))
}
fn ready_three(catalog: &Catalog, o: &Observation) -> Result<Vec<ObjectHandle>, String> {
    let mut rows = vec![];
    for x in &o.view.objects {
        if x.controller == P0 && x.zone == Zone::Battlefield && !x.tapped && x.attack_ready {
            let d = definition(catalog, &x.oracle_id)?;
            if x.characteristics
                .as_ref()
                .is_some_and(|c| c.kind == CardKind::Creature)
                && !d.keywords.contains(&KeywordAbility::Defender)
            {
                rows.push(x.handle.clone());
            }
        }
    }
    Ok(rows.into_iter().take(3).collect())
}
fn is_main(o: &Observation) -> bool {
    o.view.viewer == P0
        && o.view.active == Some(P0)
        && o.view.turn.step == TurnStep::PrecombatMain
        && o.view.stack.is_empty()
        && matches!(o.decision, Some(PlayerRequest::Priority { .. }))
}
fn condition(catalog: &Catalog, o: &Observation, policy: &Strategy<'_>) -> Result<bool, String> {
    if !is_main(o)
        || o.view.live_turn_order.len() != 4
        || ready_three(catalog, o)?.len() != 3
        || edict_object(catalog, o)?.is_none()
    {
        return Ok(false);
    }
    let d = named(catalog, "Cruel Edict")?;
    if spend(&d.characteristics.mana_cost, &policy.funds(&o.view)).is_none() {
        return Ok(false);
    }
    Ok(o.view
        .live_turn_order
        .iter()
        .filter(|p| **p != P0)
        .all(|p| {
            o.view.objects.iter().any(|x| {
                x.zone == Zone::Battlefield
                    && x.controller == *p
                    && x.characteristics
                        .as_ref()
                        .is_some_and(|c| c.kind == CardKind::Creature)
            })
        }))
}
/// Setup is a fixed public policy: lands, affordable creatures, no attacks and
/// no one-shot spells. Edict is retained through cleanup; no hidden deck peek.
fn development(
    catalog: &Catalog,
    o: &Observation,
    s: &mut Strategy<'_>,
) -> Result<PlayerResponse, String> {
    match o.decision.as_ref().ok_or("setup missing request")? {
        PlayerRequest::Attackers { .. } => Ok(PlayerResponse::Attackers { attacks: vec![] }),
        PlayerRequest::Priority { .. } => {
            let v = &o.view;
            if v.active != Some(v.viewer)
                || !v.stack.is_empty()
                || !matches!(
                    v.turn.step,
                    TurnStep::PrecombatMain | TurnStep::PostcombatMain
                )
            {
                return Ok(PlayerResponse::Pass);
            }
            if v.turn.lands_played == 0
                && let Some(x) = v.objects.iter().find(|x| {
                    x.zone == Zone::Hand { owner: v.viewer }
                        && definition(catalog, &x.oracle_id)
                            .is_ok_and(|d| d.characteristics.kind == CardKind::Land)
                })
            {
                return Ok(PlayerResponse::PlayLand {
                    object: x.handle.clone(),
                });
            }
            let funds = s.funds(v);
            for command in [true, false] {
                for x in &v.objects {
                    let wanted = if command {
                        Zone::Command { owner: v.viewer }
                    } else {
                        Zone::Hand { owner: v.viewer }
                    };
                    if x.owner != v.viewer || x.zone != wanted {
                        continue;
                    }
                    let d = definition(catalog, &x.oracle_id)?;
                    if d.characteristics.kind != CardKind::Creature {
                        continue;
                    }
                    let mut cost = d.characteristics.mana_cost.clone();
                    if command {
                        let history = v
                            .commanders
                            .iter()
                            .find(|c| c.history.owner == v.viewer && c.oracle_id == x.oracle_id)
                            .ok_or("public command history missing")?;
                        let n = history
                            .history
                            .command_casts
                            .get(&v.viewer)
                            .cloned()
                            .unwrap_or_default();
                        cost.generic.add(&n);
                        cost.generic.add(&n);
                    }
                    if spend(&cost, &funds).is_some() {
                        return Ok(PlayerResponse::Cast {
                            card: x.handle.clone(),
                        });
                    }
                }
            }
            Ok(PlayerResponse::Pass)
        }
        PlayerRequest::Discard {
            count, candidates, ..
        } => {
            let edict = &named(catalog, "Cruel Edict")?.oracle_id;
            let mut ordered = candidates
                .iter()
                .filter(|h| {
                    !o.view
                        .objects
                        .iter()
                        .any(|x| &x.handle == *h && &x.oracle_id == edict)
                })
                .cloned()
                .collect::<Vec<_>>();
            ordered.extend(
                candidates
                    .iter()
                    .filter(|h| {
                        o.view
                            .objects
                            .iter()
                            .any(|x| &x.handle == *h && &x.oracle_id == edict)
                    })
                    .cloned(),
            );
            Ok(PlayerResponse::Discard {
                objects: ordered.into_iter().take(*count).collect(),
            })
        }
        _ => s.choose(o),
    }
}
struct Root {
    config: MatchConfig,
    prepared: Prepared,
    game: Game,
    three: Vec<ObjectHandle>,
}
fn prepare_root(
    catalog: &Catalog,
    progress: &mut impl FnMut(Value) -> Result<(), String>,
) -> Result<Root, String> {
    let config = profile_config();
    let prepared =
        Prepared::admit_pod(catalog.clone(), config.clone()).map_err(|e| format!("admit:{e:?}"))?;
    let mut game = Game::with_trace(prepared.clone(), TraceMode::Off);
    let mut s = Strategy {
        catalog,
        policy: Policy::Develop,
        mulligans: [0; 4],
        beneficial: false,
    };
    let mut meter = Meter::default();
    let mut loop_ordinal = 0;
    let mut phase = "initial_advance";
    let result = (|| -> Result<Vec<ObjectHandle>, String> {
        for input in 0..SETUP_INPUT_LIMIT {
            loop_ordinal = input;
            phase = "advance";
            let request = ready(&mut game, 64, &mut meter)?;
            phase = "observation";
            let o = own_observation(&game, request.player(), 64, &mut meter)?;
            phase = "first_public_condition";
            if condition(catalog, &o, &s)? {
                let three = ready_three(catalog, &o)?;
                progress(
                    json!({"phase":"prepared_first_public_condition","setup_inputs":game.journal().len(),"rule_boundary":game.rule_boundary(),"public_root":o,"counts":meter.counts}),
                )?;
                return Ok(three);
            }
            phase = "public_policy";
            let response = development(catalog, &o, &mut s)?;
            phase = "check_and_submit";
            accept(&mut game, &o, response, 64, &mut meter)?;
            phase = "accepted_waiting_for_committed_processing";
            if input.is_multiple_of(128) {
                progress(
                    json!({"phase":"preparing","setup_inputs":game.journal().len(),"public_turn":o.view.turn,"public_battlefield_rows":o.view.objects.iter().filter(|x|x.zone==Zone::Battlefield).count(),"counts":meter.counts}),
                )?;
            }
        }
        Err("preparation input bound; fixed seed never retried, no synthetic fallback".into())
    })();
    match result {
        Ok(three) => Ok(Root {
            config,
            prepared,
            game,
            three,
        }),
        Err(error) => {
            // Caught failures have exact work, unlike a resource kill between
            // published checkpoints. Accepted journal length includes pending
            // inputs; committed processing is separately counted by facts.
            progress(
                json!({"phase":"preparation_failed","failed_stage":phase,"attempted_loop_ordinal":loop_ordinal,"actual_accepted_inputs":game.journal().len(),"last_accepted_journal_ordinal":game.journal().len().checked_sub(1),"rule_boundary":game.rule_boundary(),"error":error,"exact_caught_partial_work":meter}),
            )?;
            Err(error)
        }
    }
}
fn attack_matrix(defenders: &[PlayerId]) -> Result<Vec<[usize; 3]>, String> {
    if !(1..=3).contains(&defenders.len())
        || defenders.iter().any(|p| p.0 >= 4)
        || defenders.contains(&P0)
        || defenders.iter().copied().collect::<BTreeSet<_>>().len() != defenders.len()
    {
        return Err("exact distinct living opponent domain".into());
    }
    let base = defenders.len() + 1;
    Ok((0..base.pow(3))
        .map(|mut n| {
            std::array::from_fn(|_| {
                let d = n % base;
                n /= base;
                d
            })
        })
        .collect())
}
fn attacks(
    digits: [usize; 3],
    three: &[ObjectHandle],
    defenders: &[PlayerId],
) -> Result<Vec<(ObjectHandle, PlayerId)>, String> {
    if three.len() != 3
        || three
            .iter()
            .enumerate()
            .any(|(i, h)| three[..i].contains(h))
        || digits.iter().any(|d| *d > defenders.len())
    {
        return Err("invalid fixed three-attacker assignment".into());
    }
    Ok(digits
        .into_iter()
        .enumerate()
        .filter(|(_, d)| *d != 0)
        .map(|(i, d)| (three[i].clone(), defenders[d - 1]))
        .collect())
}
struct Branch {
    game: Game,
    meter: Meter,
    delta_journal: Vec<mtg_core::AcceptedInput>,
}
fn branch(
    root: &Root,
    catalog: &Catalog,
    cell: &Cell,
    q: u64,
) -> Result<Branch, Box<(String, Meter)>> {
    let mut m = Meter::default();
    m.counts.fork_started = 1;
    let mut g = m.timed("fork", || root.game.fork());
    m.counts.forked = 1;
    let initial = g.journal().len();
    let mut strategy = Strategy {
        catalog,
        policy: Policy::Develop,
        mulligans: [0; 4],
        beneficial: false,
    };
    let result = (|| -> Result<(), String> {
        let mut primary = false;
        let mut targeted = false;
        let mut sacrificed = false;
        let mut stopped_at_next_boundary = false;
        for _ in 0..CALL_LIMIT {
            let request = ready(&mut g, q, &mut m)?;
            let o = own_observation(&g, request.player(), q, &mut m)?;
            let r = o
                .decision
                .as_ref()
                .ok_or("branch missing permitted request")?;
            if matches!(cell, Cell::Attack { .. }) && primary {
                stopped_at_next_boundary = true;
                break;
            }
            if matches!(cell, Cell::Edict { .. }) && sacrificed && is_main(&o) {
                stopped_at_next_boundary = true;
                break;
            }
            let response = match (cell, r) {
                (
                    Cell::Attack { digits },
                    PlayerRequest::Attackers {
                        player,
                        candidates,
                        defenders,
                        ..
                    },
                ) => {
                    if *player != P0
                        || candidates.iter().take(3).cloned().collect::<Vec<_>>() != root.three
                    {
                        return Err("predeclared attackers no longer offered".into());
                    }
                    let expected = o
                        .view
                        .live_turn_order
                        .iter()
                        .copied()
                        .filter(|p| *p != P0)
                        .collect::<Vec<_>>();
                    if *defenders != expected {
                        return Err("actual defender domain differs from living seats".into());
                    }
                    primary = true;
                    PlayerResponse::Attackers {
                        attacks: attacks(*digits, &root.three, defenders)?,
                    }
                }
                (Cell::Edict { .. }, PlayerRequest::Priority { player, .. }) if !primary => {
                    if *player != P0 || !is_main(&o) {
                        return Err("Edict requires frozen P0 main root".into());
                    }
                    primary = true;
                    PlayerResponse::Cast {
                        card: edict_object(catalog, &o)?.ok_or("held Edict unavailable")?,
                    }
                }
                (
                    Cell::Edict { target },
                    PlayerRequest::PlayerTargets {
                        player,
                        occurrences,
                        candidates,
                        ..
                    },
                ) if !targeted => {
                    let expected = o
                        .view
                        .live_turn_order
                        .iter()
                        .copied()
                        .filter(|p| *p != P0)
                        .collect::<Vec<_>>();
                    if *player != P0
                        || occurrences.len() != 1
                        || *candidates != expected
                        || !candidates.contains(target)
                    {
                        return Err("complete actual Edict opponent domain differs".into());
                    }
                    targeted = true;
                    PlayerResponse::PlayerTargets {
                        players: vec![*target],
                    }
                }
                (
                    Cell::Edict { target },
                    PlayerRequest::Sacrifice {
                        player, candidates, ..
                    },
                ) => {
                    if sacrificed || !targeted || player != target || candidates.is_empty() {
                        return Err("Edict sacrifice chooser/domain mismatch".into());
                    }
                    if !candidates.iter().all(|h| {
                        o.view.objects.iter().any(|x| {
                            &x.handle == h
                                && x.controller == *target
                                && x.zone == Zone::Battlefield
                                && x.characteristics
                                    .as_ref()
                                    .is_some_and(|c| c.kind == CardKind::Creature)
                        })
                    }) {
                        return Err("sacrifice domain includes ineligible witness".into());
                    }
                    let actual = o
                        .view
                        .objects
                        .iter()
                        .filter(|x| {
                            x.controller == *target
                                && x.zone == Zone::Battlefield
                                && x.characteristics
                                    .as_ref()
                                    .is_some_and(|c| c.kind == CardKind::Creature)
                        })
                        .map(|x| x.handle.clone())
                        .collect::<Vec<_>>();
                    if *candidates != actual {
                        return Err("sacrifice candidate domain incomplete or reordered".into());
                    }
                    sacrificed = true;
                    PlayerResponse::Sacrifice {
                        object: candidates[0].clone(),
                    }
                }
                (_, PlayerRequest::Priority { .. }) => PlayerResponse::Pass,
                (_, PlayerRequest::Attackers { .. }) => {
                    return Err("unexpected combat boundary".into());
                }
                _ => strategy.choose(&o)?,
            };
            accept(&mut g, &o, response, q, &mut m)?;
        }
        if !stopped_at_next_boundary
            || !primary
            || matches!(cell, Cell::Edict { .. }) && (!targeted || !sacrificed)
        {
            return Err("required branch action incomplete".into());
        }
        let delta = &g.journal()[initial..];
        if delta.len() != m.counts.submitted_inputs
            || m.counts.validated_inputs != delta.len()
            || m.counts.consumed_inputs != delta.len()
        {
            return Err("accepted/committed processing counts differ".into());
        }
        for (i, input) in delta.iter().enumerate() {
            let expected = mtg_core::CoreInputIdentity::Decision {
                journal_ordinal: (initial + i) as u64,
                decision_id: input.decision_id,
                player: input.player,
            };
            if m.processed
                .iter()
                .filter(|f| f.identity == expected)
                .count()
                != 1
            {
                return Err("missing/duplicate actual processing fact".into());
            }
        }
        m.counts.fully_advanced = 1;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Branch {
            delta_journal: g.journal()[initial..].to_vec(),
            game: g,
            meter: m,
        }),
        Err(e) => {
            m.timed("failed_game_disposal", || drop(g));
            Err(Box::new((e, m)))
        }
    }
}
fn state_equal(a: &Game, b: &Game) -> Result<(), String> {
    let aa = generic_state_proof(a)?;
    let bb = generic_state_proof(b)?;
    equal_state_proofs(&aa, &bb)?;
    if aa.sha256 != a.state_digest()? || bb.sha256 != b.state_digest()? {
        return Err("generic/production State SHA differs".into());
    }
    aa.remove()?;
    bb.remove()?;
    Ok(())
}
/// Independent Detailed replay supplies actual typed effects; accepted target or
/// mana requests alone never count as successful sacrifice/payment/declaration.
fn effect_proof(
    root: &Root,
    catalog: &Catalog,
    cell: &Cell,
    run: &Branch,
) -> Result<Value, String> {
    let replay = Game::replay_with_trace(
        root.prepared.clone(),
        &run.game.replay_journal(),
        TraceMode::Detailed,
    )?;
    state_equal(&run.game, &replay)?;
    let mut outcomes = vec![];
    let mut spell_casts = 0;
    let mut completed = 0;
    let mut proposed = BTreeMap::new();
    let mut paid_stacks = vec![];
    let mut resolved_stacks = vec![];
    let mut actual_payment_costs = vec![];
    let mut sacrifice = vec![];
    let mut declared = vec![];
    for record in replay
        .trace()
        .records
        .iter()
        .filter(|r| r.decisions_seen > root.game.journal().len() as u64)
    {
        let mtg_core::EvidenceRecord::Event { event } = &record.record else {
            continue;
        };
        match event.as_ref() {
            mtg_core::Event::Committed { actual, .. } => {
                if let mtg_core::EventOutcome::ManaPaidSelected { player, cost, .. } = actual
                    && *player == P0
                {
                    actual_payment_costs.push(cost.clone());
                }
                if let mtg_core::EventOutcome::Sacrificed { player, movement } = actual {
                    sacrifice.push((*player, movement.clone()));
                }
                if let mtg_core::EventOutcome::AttackersDeclared { attacks } = actual {
                    declared.push(attacks.clone());
                }
                outcomes.push(serde_json::to_value(actual).map_err(|e| e.to_string())?);
            }
            mtg_core::Event::SpellProposed { card, stack_id } => {
                if proposed.insert(*stack_id, *card).is_some() {
                    return Err("duplicate actual proposal".into());
                }
            }
            mtg_core::Event::SpellCast { stack_id } => {
                spell_casts += 1;
                paid_stacks.push(*stack_id);
            }
            mtg_core::Event::SpellCompleted { stack_id, resolved } => {
                completed += 1;
                if *resolved {
                    resolved_stacks.push(*stack_id);
                }
            }
            _ => {}
        }
    }
    match cell {
        Cell::Attack { .. } => {
            let accepted = run
                .delta_journal
                .iter()
                .filter_map(|i| {
                    if let mtg_core::Response::Attackers { attacks } = &i.response {
                        Some(attacks.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if declared != accepted || declared.len() != 1 || !sacrifice.is_empty() {
                return Err("actual declaration differs from accepted complete assignment".into());
            }
        }
        Cell::Edict { target } => {
            let chosen = run
                .delta_journal
                .iter()
                .filter_map(|i| {
                    if let mtg_core::Response::Sacrifice { object } = &i.response {
                        Some(*object)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if spell_casts != 1
                || completed != 1
                || sacrifice.len() != 1
                || sacrifice[0].0 != *target
                || chosen != vec![sacrifice[0].1.previous]
                || sacrifice[0].1.from != Zone::Battlefield
                || !matches!(sacrifice[0].1.destination, Zone::Graveyard { .. })
            {
                return Err("actual paid Edict/selected chooser sacrifice/movement missing".into());
            }
            let edict = named(catalog, "Cruel Edict")?;
            let casts = run
                .delta_journal
                .iter()
                .filter_map(|i| {
                    if let mtg_core::Response::Cast { card } = &i.response {
                        Some(*card)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            if casts.len() != 1
                || proposed.len() != 1
                || paid_stacks.len() != 1
                || paid_stacks != resolved_stacks
                || proposed.get(&paid_stacks[0]) != casts.first()
                || !root.game.config().cards.iter().any(|row| {
                    row.id == casts[0] && row.owner == P0 && row.oracle_id == edict.oracle_id
                })
                || actual_payment_costs != vec![edict.characteristics.mana_cost.clone()]
            {
                return Err(
                    "actual Edict source/proposal/payment/resolved stack join differs".into(),
                );
            }
        }
    }
    Ok(
        json!({"typed_outcomes":outcomes,"spell_casts":spell_casts,"spell_completed":completed,"paid_stacks":paid_stacks,"resolved_stacks":resolved_stacks,"actual_payment_costs":actual_payment_costs,"sacrifices":sacrifice,"declarations":declared,"state_sha256":run.game.state_digest()?,"rule_boundary":run.game.rule_boundary(),"delta_journal":run.delta_journal,"requests":run.meter.requests,"responses":run.meter.responses}),
    )
}
/// Reuse the private spool directory guard for error/unwind cleanup. Close the
/// file before checked cleanup on either normal return path. A process kill
/// cannot run Rust destructors; the outer diagnostic host still owns that case.
fn with_root_snapshot<T>(
    operation: impl FnOnce(&std::path::Path, &mut fs::File) -> Result<T, String>,
) -> Result<T, String> {
    let owner = GenericStateProof::create()?;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&owner.path)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("root snapshot must be a regular file".into());
        }
        operation(&owner.path, &mut file)
    })();
    // Unlike Drop, this reports cleanup failures on both success and error.
    // The owner's Drop remains the fallback if cleanup fails or we unwind.
    let cleanup = fs::remove_dir_all(&owner.directory).map_err(|e| e.to_string());
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("root snapshot cleanup: {error}")),
        (Err(error), Err(cleanup)) => Err(format!("{error}; root snapshot cleanup: {cleanup}")),
    }
}

fn write_root_snapshot(
    game: &Game,
    file: &mut fs::File,
) -> Result<mtg_core::SnapshotWriteInfo, String> {
    let receipt = {
        let mut writer = BufWriter::with_capacity(64 * 1024, &mut *file);
        // The canonical exporter flushes the sink before returning its receipt.
        game.write_snapshot(&mut writer)?
    };
    if file.metadata().map_err(|e| e.to_string())?.len() != receipt.bytes {
        return Err("root snapshot export length differs from receipt".into());
    }
    Ok(receipt)
}

fn verify_root_snapshot(
    game: &Game,
    prepared: Prepared,
    progress: &mut impl FnMut(&str, Option<u64>) -> Result<(), String>,
) -> Result<(), String> {
    let restored = with_root_snapshot(|_, file| {
        progress("snapshot_export_started", None)?;
        let receipt = write_root_snapshot(game, file)?;
        progress("snapshot_export_complete", Some(receipt.bytes))?;
        if file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())? != 0 {
            return Err("root snapshot rewind differs".into());
        }
        progress("snapshot_restore_started", None)?;
        // This is standalone restore, including raw-byte/EOF binding, complete
        // identity/invariant checks and the unchanged cold accepted-input proof.
        let restored = Game::restore_from_reader(prepared, file)?;
        if file.metadata().map_err(|e| e.to_string())?.len() != receipt.bytes {
            return Err("root snapshot restore length differs from receipt".into());
        }
        Ok(restored)
    })?;
    // The completed standalone restore owns everything needed by equality.
    // Release the root file with checked cleanup before either generic proof.
    progress("snapshot_verification_started", None)?;
    state_equal(game, &restored)?;
    drop(restored);
    progress("snapshot_verified", None)
}

fn checked_root(
    mut root: Root,
    live: usize,
    progress: &mut impl FnMut(&str, Option<u64>) -> Result<(), String>,
) -> Result<Root, String> {
    let departures = match live {
        4 => vec![],
        3 => vec![PlayerId(2)],
        2 => vec![PlayerId(2), PlayerId(1)],
        _ => return Err("living matrix".into()),
    };
    for p in departures {
        root.game.concede(p)?;
        let mut m = Meter::default();
        let r = ready(&mut root.game, 64, &mut m)?;
        if r.player() != P0 {
            return Err("departure lost original active priority".into());
        }
    }
    let mut m = Meter::default();
    let o = own_observation(&root.game, P0, 64, &mut m)?;
    if !is_main(&o)
        || o.view.live_turn_order.len() != live
        || ready_three(&Catalog::read(CATALOG_JSON)?, &o)? != root.three
    {
        return Err("surviving root differs from frozen public attackers".into());
    }
    // All histories, RNG and command facts are preserved by ordinary recorded
    // core concessions; no player/card table is reconstructed or hand edited.
    verify_root_snapshot(&root.game, root.prepared.clone(), progress)?;
    Ok(root)
}
fn fork_root(root: &Root) -> Root {
    Root {
        config: root.config.clone(),
        prepared: root.prepared.clone(),
        game: root.game.fork(),
        three: root.three.clone(),
    }
}
fn run_cell(
    root: &Root,
    catalog: &Catalog,
    cell: &Cell,
    file: &mut fs::File,
    report: &mut Value,
) -> Result<(), String> {
    let mut row = json!({"living":root.game.live_players().len(),"root_state_sha256":root.game.state_digest()?,"cell":cell,"phase":"reference_started","reference":null,"budget_oracles":[],"repetitions":[]});
    report["in_progress_cell"] = row.clone();
    publish(file, report)?;
    let reference = match branch(root, catalog, cell, 64) {
        Ok(run) => run,
        Err(failure) => {
            let (error, meter) = *failure;
            row["failed_reference"] = json!({"error":error,"partial":meter});
            report["in_progress_cell"] = row;
            publish(file, report)?;
            return Err("reference branch failed".into());
        }
    };
    row["phase"] = json!("reference_oracle_started");
    row["reference_work"] = json!(reference.meter);
    report["in_progress_cell"] = row.clone();
    publish(file, report)?;
    row["reference"] = effect_proof(root, catalog, cell, &reference)?;
    // Unmeasured oracle executions are additional actual work inside the outer
    // deadline, not undeclared warmups or part of the component timing vectors.
    for q in [1, u64::MAX] {
        row["phase"] = json!({"budget_oracle_started":q});
        report["in_progress_cell"] = row.clone();
        publish(file, report)?;
        let alternate = match branch(root, catalog, cell, q) {
            Ok(run) => run,
            Err(failure) => {
                let (error, meter) = *failure;
                row["failed_budget"] = json!({"quantum":q,"error":error,"partial":meter});
                report["in_progress_cell"] = row;
                publish(file, report)?;
                return Err("budget oracle branch failed".into());
            }
        };
        row["budget_oracles"].as_array_mut().ok_or("oracles")?.push(
            json!({"quantum":q,"phase":"awaiting_state_oracle","actual_work":alternate.meter}),
        );
        report["in_progress_cell"] = row.clone();
        publish(file, report)?;
        state_equal(&reference.game, &alternate.game)?;
        if alternate.delta_journal != reference.delta_journal
            || alternate.meter.requests != reference.meter.requests
            || alternate.meter.responses != reference.meter.responses
            || alternate.game.rule_boundary() != reference.game.rule_boundary()
        {
            return Err("budget partition changed complete domains/input/State".into());
        }
        row["budget_oracles"]
            .as_array_mut()
            .ok_or("oracles")?
            .last_mut()
            .ok_or("oracle")?["phase"] = json!("verified");
    }
    for rep in 0..CHOICE_REPETITIONS {
        row["phase"] = json!({"repetition_started":rep,"warmup":rep==0,"midcall_counts_unknown_if_killed":true});
        report["in_progress_cell"] = row.clone();
        publish(file, report)?;
        let mut sample = match branch(root, catalog, cell, 64) {
            Ok(run) => run,
            Err(failure) => {
                let (error, meter) = *failure;
                row["repetitions"].as_array_mut().ok_or("samples")?.push(json!({"repetition":rep,"warmup":rep==0,"status":"failed","error":error,"partial":meter}));
                report["in_progress_cell"] = row;
                publish(file, report)?;
                return Err("declared branch sample failed; denominator unchanged".into());
            }
        };
        row["repetitions"].as_array_mut().ok_or("samples")?.push(json!({"repetition":rep,"warmup":rep==0,"status":"awaiting_oracle","work":sample.meter}));
        report["in_progress_cell"] = row.clone();
        publish(file, report)?;
        state_equal(&reference.game, &sample.game)?;
        if sample.delta_journal != reference.delta_journal
            || sample.meter.requests != reference.meter.requests
            || sample.meter.responses != reference.meter.responses
            || sample.meter.counts != reference.meter.counts
        {
            return Err("repeated complete cell semantics or work differs".into());
        }
        sample.meter.counts.verified = 1;
        let digest = sample.game.state_digest()?;
        sample.meter.timed("game_disposal", || drop(sample.game));
        *row["repetitions"]
            .as_array_mut()
            .ok_or("samples")?
            .last_mut()
            .ok_or("sample")? = json!({"repetition":rep,"warmup":rep==0,"status":"verified","state_sha256":digest,"work":sample.meter});
        report["in_progress_cell"] = row.clone();
        publish(file, report)?;
    }
    row["phase"] = json!("complete");
    report["cells"].as_array_mut().ok_or("cells")?.push(row);
    report["in_progress_cell"] = Value::Null;
    publish(file, report)
}
/// Negative validation is outside component clocks and cannot authorize a
/// response. The optional Defender witness is explicitly reported unavailable
/// rather than changing the first-public-condition root to force one.
fn ready_defender(catalog: &Catalog, row: &mtg_core::VisibleObject) -> bool {
    row.controller == P0
        && row.zone == Zone::Battlefield
        && !row.tapped
        && row.attack_ready
        && definition(catalog, &row.oracle_id)
            .is_ok_and(|d| d.keywords.contains(&KeywordAbility::Defender))
}
fn rejection_oracles(root: &Root, catalog: &Catalog) -> Result<Value, String> {
    let mut g = root.game.fork();
    let mut m = Meter::default();
    let o = loop {
        let r = ready(&mut g, 64, &mut m)?;
        let o = own_observation(&g, r.player(), 64, &mut m)?;
        if matches!(o.decision, Some(PlayerRequest::Attackers { .. })) {
            break o;
        }
        if !matches!(o.decision, Some(PlayerRequest::Priority { .. })) {
            return Err("unexpected pre-attack rejection probe".into());
        }
        accept(&mut g, &o, PlayerResponse::Pass, 64, &mut m)?;
    };
    let Some(PlayerRequest::Attackers {
        candidates,
        defenders,
        ..
    }) = o.decision.as_ref()
    else {
        return Err("attack request".into());
    };
    let a = root.three.first().ok_or("three missing")?;
    if !candidates.contains(a) {
        return Err("witness not offered".into());
    }
    let defender = *defenders.first().ok_or("no actual defender")?;
    let mut bad = vec![(
        "duplicate_attacker",
        PlayerResponse::Attackers {
            attacks: vec![(a.clone(), defender), (a.clone(), defender)],
        },
    )];
    if let Some(departed) = [PlayerId(2), PlayerId(1)]
        .into_iter()
        .find(|p| !o.view.live_turn_order.contains(p))
    {
        bad.push((
            "departed_defender",
            PlayerResponse::Attackers {
                attacks: vec![(a.clone(), departed)],
            },
        ));
    }
    // A Defender witness must satisfy the other ordinary attack eligibility
    // flags, so rejection cannot be explained by tapped/summoning-sick state.
    let wall = o.view.objects.iter().find(|x| ready_defender(catalog, x));
    if let Some(wall) = wall {
        if candidates.contains(&wall.handle) {
            return Err("ready Defender incorrectly offered as attacker".into());
        }
        bad.push((
            "defender_permanent",
            PlayerResponse::Attackers {
                attacks: vec![(wall.handle.clone(), defender)],
            },
        ));
    }
    let before = g.state_digest()?;
    let journal = g.replay_journal();
    let mut checked = vec![];
    for (name, response) in bad {
        let mut result = g.check_player_response(
            P0,
            o.decision.as_ref().ok_or("decision")?.token().clone(),
            response,
            Budget::new(64),
        );
        for _ in 0..CALL_LIMIT {
            match result {
                QueryResult::Rejected(_) => {
                    checked.push(name);
                    break;
                }
                QueryResult::Incomplete { work, .. } => {
                    result = g.resume_work(work, Budget::new(64))
                }
                QueryResult::Complete(_) => {
                    return Err("invalid declaration obtained a receipt".into());
                }
            }
        }
        if checked.last() != Some(&name) {
            return Err("negative query bound".into());
        }
        if g.state_digest()? != before || g.replay_journal() != journal {
            return Err("rejected query mutated State/journal".into());
        }
    }
    Ok(
        json!({"rejected_without_mutation":checked,"defender_witness_available":wall.is_some(),"defender_negative_claim":wall.is_some(),"departed_case_not_applicable_at4":o.view.live_turn_order.len()==4}),
    )
}
#[test]
fn choice_defender_probe_requires_other_attack_eligibility_without_changing_root() {
    let catalog = Catalog::read(CATALOG_JSON).unwrap();
    let defender = catalog
        .definitions
        .iter()
        .find(|d| d.keywords.contains(&KeywordAbility::Defender))
        .unwrap();
    assert_eq!(defender.characteristics.kind, CardKind::Creature);
    // Pure public-row selection domain, not a newly admitted/root position.
    let row = mtg_core::VisibleObject {
        characteristics: Some(defender.characteristics.clone()),
        marked_damage: Quantity::zero(),
        attack_ready: true,
        handle: ObjectHandle("selection-guard-only".into()),
        oracle_id: defender.oracle_id.clone(),
        owner: P0,
        controller: P0,
        zone: Zone::Battlefield,
        tapped: false,
    };
    assert!(ready_defender(&catalog, &row));
    let mut changed = row.clone();
    changed.tapped = true;
    assert!(!ready_defender(&catalog, &changed));
    changed = row.clone();
    changed.attack_ready = false;
    assert!(!ready_defender(&catalog, &changed));
    changed = row.clone();
    changed.controller = PlayerId(1);
    assert!(!ready_defender(&catalog, &changed));
    changed = row.clone();
    changed.zone = Zone::Hand { owner: P0 };
    assert!(!ready_defender(&catalog, &changed));
    changed = row;
    let ordinary = catalog
        .definitions
        .iter()
        .find(|d| {
            d.characteristics.kind == CardKind::Creature
                && !d.keywords.contains(&KeywordAbility::Defender)
        })
        .unwrap();
    changed.oracle_id = ordinary.oracle_id.clone();
    changed.characteristics = Some(ordinary.characteristics.clone());
    assert!(!ready_defender(&catalog, &changed));
}
#[test]
fn choice_matrix_exact64_27_8_with_home_and_repeated_defenders() {
    for (defenders, n) in [
        (vec![PlayerId(1), PlayerId(2), PlayerId(3)], 64),
        (vec![PlayerId(1), PlayerId(3)], 27),
        (vec![PlayerId(3)], 8),
    ] {
        let rows = attack_matrix(&defenders).unwrap();
        assert_eq!(rows.len(), n);
        assert_eq!(rows.iter().copied().collect::<BTreeSet<_>>().len(), n);
        assert_eq!(rows[0], [0, 0, 0]);
        assert!(rows.contains(&[1, 1, 1]));
        assert_eq!(rows.last(), Some(&[defenders.len(); 3]));
    }
    assert!(attack_matrix(&[P0]).is_err());
    assert!(attack_matrix(&[PlayerId(4)]).is_err());
    assert!(attack_matrix(&[PlayerId(1), PlayerId(1)]).is_err());
}
#[test]
fn choice_profile_is_one_fixed_admitted400_card_pod_and_archived_edict() {
    let catalog = Catalog::read(CATALOG_JSON).unwrap();
    let config = profile_config();
    let p = Prepared::admit_pod(catalog.clone(), config.clone()).unwrap();
    assert_eq!(p.config().cards.len(), 400);
    assert_eq!(p.config().commanders.len(), 4);
    assert_eq!(
        config.decks,
        [
            mtg_core::black_green_deck(),
            mtg_core::white_deck(),
            mtg_core::black_green_deck(),
            mtg_core::white_deck()
        ]
    );
    assert_eq!(config, profile_config());
    let d = named(&catalog, "Cruel Edict").unwrap();
    let program = d.spell.as_ref().unwrap();
    assert_eq!(program.targets.len(), 1);
    assert!(program.instructions.iter().any(|i| matches!(
        i,
        mtg_contracts::Instruction::TargetPlayerSacrificeCreature { .. }
    )));
}
#[test]
fn choice_setup_policy_initial_keep_is_public_and_does_not_mutate_game() {
    let catalog = Catalog::read(CATALOG_JSON).unwrap();
    let prepared = Prepared::admit_pod(catalog.clone(), profile_config()).unwrap();
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    let mut m = Meter::default();
    let request = ready(&mut game, 64, &mut m).unwrap();
    let o = own_observation(&game, request.player(), 64, &mut m).unwrap();
    let before = game.state_digest().unwrap();
    let journal = game.replay_journal();
    let mut strategy = Strategy {
        catalog: &catalog,
        policy: Policy::Develop,
        mulligans: [0; 4],
        beneficial: false,
    };
    assert!(matches!(o.decision, Some(PlayerRequest::Mulligan { .. })));
    assert!(matches!(
        development(&catalog, &o, &mut strategy).unwrap(),
        PlayerResponse::Mulligan { take: false }
    ));
    assert!(!condition(&catalog, &o, &strategy).unwrap());
    assert_eq!(before, game.state_digest().unwrap());
    assert_eq!(journal, game.replay_journal());
}

#[cfg(test)]
mod root_snapshot_tests {
    use super::*;
    use std::io::Read;

    fn prepared() -> Prepared {
        Prepared::admit(
            Catalog::read(include_str!(
                "../../../../../tests/fixtures/expected-ir.json"
            ))
            .unwrap(),
            mtg_core::ScenarioConfig::land_return(),
        )
        .unwrap()
    }

    fn pending_game(mode: TraceMode) -> Game {
        let mut game = Game::with_trace(prepared(), mode);
        // The tiny admitted source fixture reaches an actual pending target
        // with retained accepted inputs and visibility history, without the
        // ignored diagnostic's natural-root preparation or measurement work.
        for response in [
            mtg_core::Response::ActivateMana {
                object: game.object_id(mtg_core::CardId(0)).unwrap(),
            },
            mtg_core::Response::Cast {
                card: mtg_core::CardId(1),
            },
        ] {
            let Advance::AwaitingDecision(request) = game.advance(Budget::unlimited()) else {
                panic!("fixture request");
            };
            game.respond(request.player(), request.decision_id(), response)
                .unwrap();
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
        assert_eq!(game.journal().len(), 2);
        game
    }

    #[test]
    fn choice_root_snapshot_file_has_full_string_bytes_sha_and_standalone_restore() {
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            for game in [Game::with_trace(prepared(), mode), pending_game(mode)] {
                let expected = game.snapshot().unwrap();
                let mut removed = None;
                with_root_snapshot(|path, file| {
                    removed = Some(path.parent().unwrap().to_owned());
                    assert!(file.metadata().unwrap().is_file());
                    assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
                    assert_eq!(
                        fs::metadata(path.parent().unwrap())
                            .unwrap()
                            .permissions()
                            .mode()
                            & 0o777,
                        0o700
                    );
                    let receipt = write_root_snapshot(&game, file)?;
                    assert_eq!(receipt.bytes, expected.len() as u64);
                    assert_eq!(receipt.snapshot_sha256, sha256(expected.as_bytes()));
                    assert_eq!(receipt.state_sha256, game.state_digest()?);
                    file.rewind().unwrap();
                    let mut actual = Vec::new();
                    file.read_to_end(&mut actual).unwrap();
                    assert_eq!(actual, expected.as_bytes());
                    assert_eq!(sha256(&actual), receipt.snapshot_sha256);
                    file.rewind().unwrap();
                    let restored = Game::restore_from_reader(prepared(), file)?;
                    assert_eq!(restored.snapshot()?, expected);
                    assert_eq!(restored.replay_journal(), game.replay_journal());
                    state_equal(&game, &restored)
                })
                .unwrap();
                assert!(!removed.unwrap().exists());
            }
        }
    }

    #[test]
    fn choice_root_snapshot_restore_error_and_export_length_error_clean_up() {
        let game = pending_game(TraceMode::Off);
        for bad_export in [false, true] {
            let mut removed = None;
            let result = with_root_snapshot(|path, file| {
                removed = Some(path.parent().unwrap().to_owned());
                if bad_export {
                    // A stale longer file cannot acquire a successful receipt.
                    file.set_len(1024 * 1024).unwrap();
                    return write_root_snapshot(&game, file).map(|_| ());
                }
                write_root_snapshot(&game, file)?;
                file.set_len(1).unwrap();
                file.rewind().unwrap();
                Game::restore_from_reader(prepared(), file).map(|_| ())
            });
            let error = result.unwrap_err();
            if bad_export {
                assert!(
                    error.contains("export length differs from receipt"),
                    "{error}"
                );
            }
            assert!(!removed.unwrap().exists());
        }
    }

    #[test]
    fn choice_root_snapshot_exclusive_file_and_error_unwind_cleanup() {
        for unwind in [false, true] {
            let mut removed = None;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_root_snapshot(|path, file| -> Result<(), String> {
                    removed = Some(path.parent().unwrap().to_owned());
                    file.write_all(b"private partial snapshot").unwrap();
                    let error = fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .mode(0o600)
                        .open(path)
                        .unwrap_err();
                    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                    if unwind {
                        panic!("injected cancellation unwind");
                    }
                    Err("injected cancellation error".into())
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().unwrap_err(), "injected cancellation error");
            }
            assert!(!removed.unwrap().exists());
        }
    }

    #[test]
    fn choice_root_snapshot_phase_receipt_and_each_progress_cancellation() {
        let game = pending_game(TraceMode::Off);
        let expected_bytes = game.snapshot().unwrap().len() as u64;
        let mut phases = Vec::new();
        verify_root_snapshot(&game, prepared(), &mut |phase, bytes| {
            phases.push((phase.to_owned(), bytes));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            phases,
            [
                ("snapshot_export_started", None),
                ("snapshot_export_complete", Some(expected_bytes)),
                ("snapshot_restore_started", None),
                ("snapshot_verification_started", None),
                ("snapshot_verified", None),
            ]
            .map(|(phase, bytes)| (phase.to_owned(), bytes))
        );
        let before = game.snapshot().unwrap();
        for stop in 0..phases.len() {
            let mut seen = Vec::new();
            let error = verify_root_snapshot(&game, prepared(), &mut |phase, bytes| {
                seen.push((phase.to_owned(), bytes));
                if seen.len() == stop + 1 {
                    Err("injected progress publication failure".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error, "injected progress publication failure");
            assert_eq!(seen, phases[..=stop]);
            assert_eq!(game.snapshot().unwrap(), before);
        }
    }

    // Give each child its own temporary parent without changing this process's
    // environment. Other parallel proof tests cannot affect the file/FD oracle.
    fn isolated_snapshot_test(name: &str) -> bool {
        const CHILD: &str = "MTG_ROOT_SNAPSHOT_CLEANUP_TEST_CHILD";
        let full_name = format!("{}::{name}", module_path!().split_once("::").unwrap().1);
        if std::env::var(CHILD).as_deref() == Ok(full_name.as_str()) {
            return false;
        }
        let parent = GenericStateProof::create().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &full_name, "--nocapture"])
            .env(CHILD, &full_name)
            .env("TMPDIR", &parent.directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated snapshot test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "child must execute exactly the requested regression"
        );
        assert_eq!(fs::read_dir(&parent.directory).unwrap().count(), 0);
        true
    }

    fn owned_snapshot_directory() -> std::path::PathBuf {
        let entries = fs::read_dir(std::env::temp_dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "one owned root snapshot directory");
        assert!(entries[0].join("state.json").is_file());
        entries.into_iter().next().unwrap()
    }

    fn assert_snapshot_lifetime(directory: &std::path::Path, active: bool) {
        assert_eq!(directory.exists(), active, "snapshot directory lifetime");
        assert_eq!(
            directory.join("state.json").exists(),
            active,
            "snapshot file lifetime"
        );
        // Unlinking alone is insufficient on Linux: a descriptor can retain
        // the deleted file. read_link also sees the kernel's " (deleted)" suffix.
        let open = fs::read_dir("/proc/self/fd").unwrap().any(|entry| {
            fs::read_link(entry.unwrap().path()).is_ok_and(|target| target.starts_with(directory))
        });
        assert_eq!(open, active, "snapshot descriptor lifetime");
    }

    #[test]
    fn choice_root_snapshot_closed_and_removed_before_verification_callback() {
        if isolated_snapshot_test(
            "choice_root_snapshot_closed_and_removed_before_verification_callback",
        ) {
            return;
        }
        for mode in [TraceMode::Off, TraceMode::Compact, TraceMode::Detailed] {
            let game = pending_game(mode);
            let before = game.snapshot().unwrap();
            let mut directory = None;
            let mut verified = false;
            verify_root_snapshot(&game, prepared(), &mut |phase, _| {
                if phase == "snapshot_export_started" {
                    directory = Some(owned_snapshot_directory());
                }
                let active =
                    !matches!(phase, "snapshot_verification_started" | "snapshot_verified");
                assert_snapshot_lifetime(directory.as_ref().unwrap(), active);
                verified |= phase == "snapshot_verified";
                Ok(())
            })
            .unwrap();
            assert!(verified);
            assert_snapshot_lifetime(&directory.unwrap(), false);
            assert_eq!(game.snapshot().unwrap(), before);
        }
    }

    #[test]
    fn choice_root_snapshot_closed_and_removed_on_each_callback_cancel_or_unwind() {
        if isolated_snapshot_test(
            "choice_root_snapshot_closed_and_removed_on_each_callback_cancel_or_unwind",
        ) {
            return;
        }
        let game = pending_game(TraceMode::Detailed);
        let before = game.snapshot().unwrap();
        for stop in [
            "snapshot_export_started",
            "snapshot_export_complete",
            "snapshot_restore_started",
            "snapshot_verification_started",
            "snapshot_verified",
        ] {
            for unwind in [false, true] {
                let mut directory = None;
                let mut seen = Vec::new();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    verify_root_snapshot(&game, prepared(), &mut |phase, _| {
                        if phase == "snapshot_export_started" {
                            directory = Some(owned_snapshot_directory());
                        }
                        assert_snapshot_lifetime(
                            directory.as_ref().unwrap(),
                            !matches!(phase, "snapshot_verification_started" | "snapshot_verified"),
                        );
                        seen.push(phase.to_owned());
                        if phase == stop {
                            assert!(!unwind, "injected callback unwind");
                            return Err("injected callback cancellation".into());
                        }
                        Ok(())
                    })
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(
                        result.unwrap().unwrap_err(),
                        "injected callback cancellation"
                    );
                }
                assert_eq!(seen.last().map(String::as_str), Some(stop));
                assert_snapshot_lifetime(&directory.unwrap(), false);
                assert_eq!(game.snapshot().unwrap(), before);
            }
        }
    }

    #[test]
    fn choice_root_snapshot_restore_or_cleanup_failure_prevents_verification_callback() {
        if isolated_snapshot_test(
            "choice_root_snapshot_restore_or_cleanup_failure_prevents_verification_callback",
        ) {
            return;
        }
        let game = pending_game(TraceMode::Off);
        let before = game.snapshot().unwrap();
        for corrupt in [false, true] {
            let mut directory = None;
            let mut seen = Vec::new();
            let error = verify_root_snapshot(&game, prepared(), &mut |phase, _| {
                seen.push(phase.to_owned());
                if phase == "snapshot_export_started" {
                    directory = Some(owned_snapshot_directory());
                }
                if phase == "snapshot_restore_started" {
                    let directory = directory.as_ref().unwrap();
                    if corrupt {
                        fs::write(directory.join("state.json"), b"{").unwrap();
                    } else {
                        // Standalone restore can still read the open file, but
                        // checked directory removal must subsequently fail.
                        fs::remove_dir_all(directory).unwrap();
                    }
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(
                error.contains("root snapshot cleanup:"),
                !corrupt,
                "{error}"
            );
            assert_eq!(
                seen,
                [
                    "snapshot_export_started",
                    "snapshot_export_complete",
                    "snapshot_restore_started"
                ]
            );
            assert_snapshot_lifetime(&directory.unwrap(), false);
            assert_eq!(game.snapshot().unwrap(), before);
        }
    }
}

#[test]
#[ignore = "explicit fixed shrinking-pod choice diagnostic120s2GiB outerhost required"]
fn shrinking_pod_choice_diagnostic() {
    let output = private_output("MTG_SHRINKING_CHOICE_OUTPUT").unwrap();
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let mut report = json!({"schema":"legal-shrinking-choice-costs-v1","status":"preparing","build_id":BUILD_ID,"seed_domain":String::from_utf8_lossy(CHOICE_SEED),"config":profile_config(),"acceptance_claim":false,"planned_attack_cells":[64,27,8],"planned_edict_cells":[3,2,1],"planned_total_cells":105,"warmups_per_cell":1,"measured_repeats_per_cell":5,"planned_actual_repetitions":630,"oracle_work_scope":"extra reference64+budget1/MAX+Detailed replay percell; normal root restore and all generic State byte comparisons outside component clocks but inside host; no fit promise","cells":[],"in_progress_cell":null,"roots":[],"preparation_progress":null,"no_search_latency_or_quality_or_definition_diversity_claim":true});
    publish(&mut file, &report).unwrap();
    let result = (|| -> Result<(), String> {
        let (catalog, load) = load_catalog()?;
        report["catalog_load"] = load;
        let root = prepare_root(&catalog, &mut |progress| {
            report["preparation_progress"] = progress;
            publish(&mut file, &report)
        })?;
        report["root_journal"] = json!(root.game.replay_journal());
        report["frozen_root_state_sha256"] = json!(root.game.state_digest()?);
        report["selected_three"] = json!(root.three);
        publish(&mut file, &report)?;
        for live in [4, 3, 2] {
            let row_root = checked_root(fork_root(&root), live, &mut |phase, bytes| {
                // Keep the strict final preparation_progress shape and report
                // keys unchanged. Only this existing transient status changes;
                // each publish syncs it durably before the next expensive phase.
                report["status"] = json!(match bytes {
                    Some(bytes) => format!("checked_root_{live}_{phase}_bytes_{bytes}"),
                    None => format!("checked_root_{live}_{phase}"),
                });
                publish(&mut file, &report)
            })?;
            report["status"] = json!("preparing");
            let mut meter = Meter::default();
            let o = own_observation(&row_root.game, P0, 64, &mut meter)?;
            let defenders = o
                .view
                .live_turn_order
                .iter()
                .copied()
                .filter(|p| *p != P0)
                .collect::<Vec<_>>();
            report["roots"].as_array_mut().ok_or("roots")?.push(json!({"living":live,"journal":row_root.game.replay_journal(),"state_sha256":row_root.game.state_digest()?,"public_root":o}));
            publish(&mut file, &report)?;
            report["roots"]
                .as_array_mut()
                .ok_or("roots")?
                .last_mut()
                .ok_or("root")?["negative_oracles"] = rejection_oracles(&row_root, &catalog)?;
            publish(&mut file, &report)?;
            for digits in attack_matrix(&defenders)? {
                run_cell(
                    &row_root,
                    &catalog,
                    &Cell::Attack { digits },
                    &mut file,
                    &mut report,
                )?;
            }
            for target in defenders {
                run_cell(
                    &row_root,
                    &catalog,
                    &Cell::Edict { target },
                    &mut file,
                    &mut report,
                )?;
            }
        }
        if report["cells"].as_array().ok_or("cells")?.len() != 105 {
            return Err("incomplete fixed matrix".into());
        }
        Ok(())
    })();
    report["status"] = json!(if result.is_ok() {
        "complete105_cells630_repetitions_verified"
    } else {
        "failed_or_partial_no_denominator_reduction"
    });
    report["error"] = json!(result.as_ref().err());
    publish(&mut file, &report).unwrap();
    result.unwrap();
}
