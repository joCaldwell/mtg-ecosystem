//! Matched four-rotation blocks. A worker owns both arms of an entire block.
use crate::pods::{Outcome, PodSource, paired_game};
use mtg_core::{BuildIdentity, DynamicPodTemplate, PlayerId};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
};
mod statistics;

#[derive(Serialize)]
struct Row {
    block: u32,
    rotation: usize,
    arm: &'static str,
    outcome: Outcome,
    detail: Option<String>,
    winner_deck_slot: Option<usize>,
    focal_placement: Option<usize>,
    turns: u64,
    decisions: usize,
    first_differing_decision: Option<usize>,
    focal_opening_hand_matched: Option<bool>,
}

#[derive(Default, Clone, Copy)]
struct Block {
    complete_pairs: usize,
    difference: i32,
    censored: [i32; 2],
    lower: i32,
    upper: i32,
}

fn play_block(
    sources: [PodSource<'_>; 2],
    b: u32,
    focal: usize,
    decision_limit: usize,
) -> Result<(Block, Vec<Row>), String> {
    let mut rows = Vec::with_capacity(8);
    let mut block = Block::default();
    for rotation in 0..4 {
        let [a, prime] = sources.map(|s| paired_game(s, b, rotation, focal, decision_limit));
        let (a, prime) = (a?, prime?);
        let first = a
            .journal
            .iter()
            .zip(&prime.journal)
            .position(|(a, b)| a != b)
            .or_else(|| {
                (a.journal.len() != prime.journal.len())
                    .then(|| a.journal.len().min(prime.journal.len()))
            });
        let matched = a
            .opening
            .as_ref()
            .zip(prime.opening.as_ref())
            .map(|(a, b)| a == b);
        let win = [
            i32::from(a.winner == Some(focal)),
            i32::from(prime.winner == Some(focal)),
        ];
        let missing = [
            i32::from(!a.outcome.terminal()),
            i32::from(!prime.outcome.terminal()),
        ];
        let delta = win[1] - win[0];
        if missing == [0, 0] {
            block.complete_pairs += 1;
            block.difference += delta;
        }
        block.censored[0] += missing[0];
        block.censored[1] += missing[1];
        block.lower += delta - missing[0];
        block.upper += delta + missing[1];
        for (arm, game) in [("A", a), ("A'", prime)] {
            rows.push(Row {
                block: b,
                rotation,
                arm,
                outcome: game.outcome,
                detail: game.detail,
                winner_deck_slot: game.winner,
                focal_placement: game.placement,
                turns: game.turns,
                decisions: game.journal.len(),
                first_differing_decision: first,
                focal_opening_hand_matched: matched,
            });
        }
    }
    Ok((block, rows))
}

pub fn run(
    build: BuildIdentity,
    base: &DynamicPodTemplate<'_>,
    focal: usize,
    swaps: &[(&str, &str)],
    blocks: usize,
    seed_start: u32,
    threads: usize,
    decision_limit: usize,
    out: &Path,
) -> Result<Value, String> {
    let last = blocks
        .checked_sub(1)
        .and_then(|n| u32::try_from(n).ok())
        .and_then(|n| seed_start.checked_add(n));
    if last.is_none()
        || !(1..=32).contains(&threads)
        || focal >= 4
        || !(1..=crate::play::DECISION_LIMIT).contains(&decision_limit)
    {
        return Err("invalid block range, thread count, focal slot or decision limit".into());
    }
    let prime = base
        .substitute_many(focal, swaps)
        .map_err(|e| e.to_string())?;
    let arms = [base, &prime];
    let sources = arms.map(PodSource::Dynamic);
    let admission = arms
        .map(|arm| arm.prepare(PlayerId(0), [0; 32], 0).map(|pod| pod.identity))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let identity = json!({"admission":admission,"swaps":swaps});
    for source in sources {
        crate::analytics::require_headless_qualified(&source.analytics_profile(build))?;
    }
    let run = json!({"schema":"bench-pair-v1", "build_id":build.id, "inputs":identity, "focal_slot":focal,
        "blocks":blocks,"seed_start":seed_start,"decision_limit":decision_limit, "policy":"develop"});
    let digest = mtg_contracts::digest(&run)?;
    fs::create_dir(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut file =
        BufWriter::new(fs::File::create_new(out.join("pairs.jsonl")).map_err(|e| e.to_string())?);
    let workers = blocks.min(threads);
    let mut results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for id in 0..workers {
            handles.push(
                std::thread::Builder::new()
                    .name(format!("pair-{id}"))
                    .spawn_scoped(scope, move || {
                        (id..blocks)
                            .step_by(workers)
                            .map(|i| {
                                Ok((
                                    i,
                                    play_block(
                                        sources,
                                        seed_start + i as u32,
                                        focal,
                                        decision_limit,
                                    )?,
                                ))
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                    .map_err(|e| e.to_string())?,
            );
        }
        let mut results = Vec::new();
        for handle in handles {
            results.extend(handle.join().map_err(|_| "paired worker panicked")??);
        }
        Ok::<_, String>(results)
    })?;
    results.sort_unstable_by_key(|r| r.0);
    let mut data = Vec::with_capacity(blocks);
    let mut table = [[0usize; 2]; 2];
    let mut invalid = false;
    for (_, (b, rows)) in results {
        for [a, prime] in rows.as_chunks::<2>().0 {
            table[usize::from(!a.outcome.terminal())][usize::from(!prime.outcome.terminal())] += 1;
        }
        for row in rows {
            invalid |= matches!(row.outcome, Outcome::RulesFault | Outcome::PolicyFailure);
            serde_json::to_writer(&mut file, &row).map_err(|e| e.to_string())?;
            file.write_all(b"\n").map_err(|e| e.to_string())?;
        }
        data.push(b);
    }
    file.flush().map_err(|e| e.to_string())?;
    let seed = std::array::from_fn(|i| {
        u8::from_str_radix(&digest[i * 2..i * 2 + 2], 16).expect("SHA256 hex")
    });
    let stats = statistics::report(&data, table, invalid, seed);
    Ok(json!({"run":run,"run_sha256":digest,"workers":workers,"statistics":stats}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_arms_have_identical_journals_and_exact_zero_effect() {
        let source =
            PodSource::Embedded(mtg_core::match_profile("commander-foundations-v1").unwrap());
        let (block, rows) = play_block([source; 2], 0, 0, crate::play::DECISION_LIMIT).unwrap();
        assert_eq!(rows.len(), 8);
        for row in rows {
            assert!(row.first_differing_decision.is_none());
            assert_eq!(row.focal_opening_hand_matched, Some(true));
            assert!(row.outcome.terminal());
        }
        let report = statistics::report(&[block], [[4, 0], [0, 0]], false, [0; 32]);
        assert_eq!(report["estimate"], 0.);
        assert_eq!(report["ci95"], json!([0., 0.]));
    }
}
