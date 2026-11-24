//! Frozen admitted-prefix producer: only committed permitted observations and
//! core action outcomes leave the worker. Tokens/handles are local capture IDs.
use mtg_core::{Action, Answer, Candidate, Decision, Flag, MatchConfig, PlayerId, Reason};
use mtg_server::{
    protocol::*,
    worker::{Worker, WorkerPhase},
};
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};
fn settle(w: &mut Worker) -> Result<(), String> {
    for _ in 0..2000 {
        if !matches!(w.host_state().phase, WorkerPhase::Running) {
            return Ok(());
        }
        w.advance(Instant::now())?;
    }
    Err("prefix progress limit".into())
}
fn offered(w: &Worker) -> Result<(u8, mtg_core::Observation), String> {
    for seat in 0..4 {
        if let ResumeState::Available { observation } = w.resume(seat)?.state
            && observation.decision.is_some()
        {
            return Ok((seat, *observation));
        }
    }
    Err("no offered decision".into())
}
fn submit(w: &mut Worker, n: &mut u64, answer: Answer) -> Result<(), String> {
    let (seat, o) = offered(w)?;
    *n += 1;
    let r = w.submit(
        seat,
        CommandEnvelope {
            protocol_version: 2,
            command_id: format!("{n:032x}"),
            expected_revision: Counter::new(w.host_state().viewer_cursors[usize::from(seat)]),
            command: CommandBody::Decision {
                token: o.decision.unwrap().id.clone(),
                answer,
            },
        },
        Instant::now(),
    )?;
    if !matches!(r, SubmissionResult::Accepted { .. }) {
        return Err("prefix response rejected".into());
    }
    settle(w)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    mtg_core::install_build_identity(mtg_server::ENGINE_BUILD)?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: action_fixture NEW_SCRATCH NEW_PUBLIC_JSON".into());
    }
    let root = Path::new(&args[0]);
    if root.exists() {
        return Err("scratch must be new".into());
    }
    std::fs::create_dir(root)?;
    let mut captures = vec![];
    for (name, fixture, bytes) in [(
        "commander-foundations-v1",
        "packages/game-engine/tests/fixtures/expected-match-ir.json",
        include_str!("../../game-engine/tests/fixtures/expected-match-ir.json"),
    )] {
        let catalog = mtg_contracts::Catalog::read(bytes)?;
        let catalog_hash = mtg_contracts::digest(&catalog)?;
        let config = MatchConfig::for_profile(name, [3; 32], PlayerId(0))?;
        let mut w = Worker::create(&root.join(name), catalog, config, "a1", Instant::now())?;
        settle(&mut w)?;
        let mut n = 0;
        let mut stages = vec![];
        let (_, opening) = offered(&w)?;
        stages.push(serde_json::json!({"stage":"opening","observation":opening}));
        for _ in 0..4 {
            let (_, o) = offered(&w)?;
            if !matches!(
                o.decision,
                Some(Decision {
                    reason: Reason::Mulligan { .. },
                    ..
                })
            ) {
                return Err("unexpected opening request".into());
            }
            submit(
                &mut w,
                &mut n,
                o.decision
                    .unwrap()
                    .pick_one(&Candidate::Flag { flag: Flag::Keep })?,
            )?;
        }
        let mut land = false;
        let mut cast = false;
        for _ in 0..128 {
            let (_, o) = offered(&w)?;
            let d = o.decision.as_ref().ok_or("missing decision")?;
            let response = d
                .candidates
                .iter()
                .find(|c| {
                    matches!(
                        (land, c),
                        (
                            false,
                            Candidate::Action {
                                action: Action::PlayLand { .. }
                            }
                        ) | (
                            true,
                            Candidate::Action {
                                action: Action::Cast { .. }
                            }
                        )
                    )
                })
                .map(|c| d.pick_one(c))
                .transpose()?;
            if let Some(response) = response {
                let stage = if !land {
                    "main-priority-before-land"
                } else {
                    "main-priority-before-cast"
                };
                stages.push(serde_json::json!({"stage":stage,"observation":o}));
                if !land {
                    land = true
                } else {
                    cast = true
                }
                submit(&mut w, &mut n, response)?;
                if cast {
                    let (_, o) = offered(&w)?;
                    if !matches!(
                        o.decision,
                        Some(Decision {
                            reason: Reason::Mana { .. },
                            ..
                        })
                    ) {
                        return Err("cast did not reach mana".into());
                    }
                    stages.push(serde_json::json!({"stage":"cast-mana","observation":o}));
                    break;
                }
            } else {
                submit(
                    &mut w,
                    &mut n,
                    d.pick_one(&Candidate::Action {
                        action: Action::Pass,
                    })?,
                )?;
            }
        }
        if !cast {
            return Err("no cast prefix within limit".into());
        }
        captures.push(serde_json::json!({"profile":name,"catalog_fixture":fixture,"fixture_sha256":mtg_contracts::sha256(bytes.as_bytes()),"catalog_sha256":catalog_hash,"match_id":"a1","capture_namespace":format!("{name}/seed3/offline"),"stages":stages}));
    }
    let output = serde_json::json!({"fixture_version":1,"engine_build_id":mtg_server::ENGINE_BUILD.id,"server_build_id":*mtg_server::SERVER_BUILD_ID,"scope":"isolated frozen admitted prefixes; tokens and handles valid only inside each capture namespace; no credentials or trusted state; labels/context remain core output", "captures":captures});
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    f.write_all(mtg_contracts::canonical(&output)?.as_bytes())?;
    f.sync_all()?;
    Ok(())
}
