use super::*;
use mtg_core::*;
use mtg_observe::action::{Composer, FINISH_KEEP, FINISH_UNDO, MAXIMUM, STOP, Step};
use mtg_sim::policy::{Policy, PolicyMemory};
const PROFILE: &str = "commander-foundations-v1";

#[test]
fn equal_permitted_history_in_distinct_sampled_worlds() {
    let source = Source::load(PROFILE, &[], None).unwrap();
    let catalog = source.catalog();
    let mut positions = 0;
    for seed in 0..256 {
        let mut root = Lane::new(&source, seed, 0, 0).unwrap();
        let viewer = loop {
            let Advance::AwaitingDecision(d) = root.game.advance(Budget::unlimited()) else {
                panic!("opening failed")
            };
            if root.game.turn().step == TurnStep::PrecombatMain {
                break d.player;
            }
            let mut work = 0;
            let mut calls = 0;
            let o =
                mtg_sim::play::observe(&root.game, d.player, 1024, &mut work, &mut calls).unwrap();
            let answer = PolicyMemory::default()
                .choose(catalog, Policy::Develop, &o)
                .unwrap();
            let QueryResult::Complete(QueryValue::Validated(receipt)) =
                root.game.check_player_response(
                    d.player,
                    o.decision.unwrap().id,
                    answer,
                    Budget::unlimited(),
                )
            else {
                panic!("opening answer")
            };
            root.game.submit(receipt).unwrap();
        };
        let CaptureOutcome::Complete(capture) =
            root.game.capture_search(viewer, Budget::unlimited())
        else {
            panic!("root capture")
        };
        let proposals = [0, 1]
            .map(|i| mtg_sim::search::propose(&capture, &[57; 32], i, 0, &mut || Ok(())).unwrap());
        assert_ne!(
            proposals[0].assignment_digest().unwrap(),
            proposals[1].assignment_digest().unwrap()
        );
        let mut worlds = proposals.map(|p| {
            let SampleBuildOutcome::Complete { game, .. } =
                SampledGame::build(catalog.clone(), &capture, p, Budget::unlimited())
            else {
                panic!("sample rejected")
            };
            game
        });
        // Follow identical public pass/empty-combat decisions through the turn.
        // Stop before the next player's draw can change the permitted history.
        loop {
            let [left, right] = worlds
                .each_mut()
                .map(|world| world.advance(Budget::unlimited()));
            let (Advance::AwaitingDecision(ld), Advance::AwaitingDecision(rd)) = (left, right)
            else {
                panic!("world stopped")
            };
            assert_eq!(ld.player, rd.player);
            let observations = worlds.each_ref().map(|w| {
                let QueryResult::Complete(QueryValue::Observation(o)) =
                    w.observe(ld.player, Budget::unlimited())
                else {
                    panic!("sample observation")
                };
                o
            });
            if observations[0].view.turn_owner != viewer {
                break;
            }
            let mut answers = vec![];
            for obs in &observations {
                let d = obs.decision.as_ref().unwrap();
                let answer = match &d.shape {
                    Shape::Pick { .. } if matches!(d.reason, Reason::Priority) => d
                        .pick_one(&Candidate::Action {
                            action: Action::Pass,
                        })
                        .unwrap(),
                    Shape::Pick { slots, .. } => Answer::Pick {
                        picks: slots
                            .iter()
                            .map(|s| s.allowed.iter().take(s.min as usize).copied().collect())
                            .collect(),
                    },
                    _ => PolicyMemory::default()
                        .choose(catalog, Policy::Develop, obs)
                        .unwrap(),
                };
                answers.push(answer);
            }
            if ld.player == viewer {
                assert_eq!(answers[0], answers[1]);
                let rows = observations.each_ref().map(|o| {
                    let Step::Choose(mask) = Composer::compose(
                        o.decision.as_ref().unwrap(),
                        &[],
                        mtg_observe::number_bound(catalog, &o.view),
                    )
                    .unwrap() else {
                        return None;
                    };
                    Some(mtg_observe::encode(catalog, o, 0, &mask, true).unwrap())
                });
                assert!(
                    rows[0] == rows[1],
                    "tensor mismatch at seed {seed}, position {positions}"
                );
                if rows[0].is_some() {
                    positions += 1;
                }
            }
            for ((world, obs), answer) in worlds.iter_mut().zip(observations).zip(answers) {
                let QueryResult::Complete(QueryValue::Validated(receipt)) = world
                    .check_player_response(
                        ld.player,
                        obs.decision.unwrap().id,
                        answer,
                        Budget::unlimited(),
                    )
                else {
                    panic!("sample rejected shared answer")
                };
                world.submit_validated(receipt).unwrap();
            }
            if positions == 256 {
                return;
            }
        }
    }
    panic!("insufficient viewer positions: {positions}")
}

#[test]
fn published_profile_vocabulary_pin() {
    let source = Source::load(PROFILE, &[], None).unwrap();
    let table = mtg_observe::catalog::CardTable::new(source.catalog()).unwrap();
    assert_eq!(
        table.vocabulary_sha256,
        "fc56a2f8cb8de44e19f9974cae6fecbd02bd0496fc2433a9e75b30853e465fda"
    );
    assert_eq!(
        table.card_tokens.len(),
        source.catalog().definitions.len() + 1
    );
}

fn baseline_game(seed: u64) -> Result<i32, String> {
    let source = Source::load(PROFILE, &[], None)?;
    let mut lane = Lane::new(&source, seed, 0, 0)?;
    let catalog = source.catalog();
    let mut policy = PolicyMemory::default();
    let mut work = 0;
    let mut queries = 0;
    let mut decisions = 0;
    loop {
        let next = lane.game.advance(Budget::new(mtg_sim::pods::QUANTUM));
        lane.game.drain_trace(&mut |_| {});
        let request = match next {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(d) => d,
            Advance::Finished { .. } | Advance::Draw => return Ok(decisions),
            Advance::Fault(e) => return Err(e),
        };
        let quantum = mtg_sim::pods::QUANTUM;
        let o =
            mtg_sim::play::observe(&lane.game, request.player, quantum, &mut work, &mut queries)?;
        let d = o.decision.as_ref().unwrap();
        let target = policy.choose(catalog, Policy::Develop, &o)?;
        let bound = mtg_observe::number_bound(catalog, &o.view);
        let mut prefix = vec![];
        let mut last_slot = None;
        let mut count = 0;
        loop {
            match Composer::compose(d, &prefix, bound)? {
                Step::Answer(a) => {
                    if a != target {
                        return Err(format!(
                            "seed {seed} decision {decisions}: unreachable {target:?}, got {a:?}"
                        ));
                    }
                    break;
                }
                Step::Choose(mask) => {
                    if last_slot != Some(mask.slot) {
                        last_slot = Some(mask.slot);
                        count = 0;
                    }
                    let number = |q: &mtg_contracts::Quantity| {
                        q.to_u32()
                            .filter(|n| *n < 63)
                            .map_or(MAXIMUM, |n| n as usize)
                    };
                    let a = match &target {
                        Answer::Pick { picks } => {
                            picks[mask.slot].get(count).map_or(STOP, |x| *x as usize)
                        }
                        Answer::Order { order } => order.get(count).map_or(STOP, |x| *x as usize),
                        Answer::Distribute { amounts } => number(&amounts[mask.slot]),
                        Answer::Number { value } => number(value),
                        Answer::Pay {
                            pay: PayAnswer::Activate { source },
                        } => *source as usize,
                        Answer::Pay {
                            pay: PayAnswer::Finish { undo_on_failure },
                        } => {
                            if *undo_on_failure {
                                FINISH_UNDO
                            } else {
                                FINISH_KEEP
                            }
                        }
                        _ => return Err("unexpected manual spend prefix".into()),
                    };
                    let row = mtg_observe::encode(catalog, &o, prefix.len(), &mask, true)?;
                    let allowed = if mask.numeric {
                        row.num_mask.get(a)
                    } else {
                        row.ptr_mask.get(a)
                    };
                    if allowed != Some(&1) {
                        return Err(format!(
                            "seed {seed} decision {decisions}: masked {a} in {target:?}, prefix {prefix:?}"
                        ));
                    }
                    prefix.push(a);
                    count += 1;
                }
            }
        }
        let QueryValue::Validated(receipt) = mtg_sim::play::complete(
            &lane.game,
            || {
                lane.game.check_player_response(
                    d.player,
                    d.id.clone(),
                    target,
                    Budget::new(quantum),
                )
            },
            quantum,
            &mut work,
            &mut queries,
        )?
        else {
            return Err("receipt type".into());
        };
        lane.game.submit(receipt)?;
        decisions += 1;
    }
}

#[test]
#[ignore = "full-game qualification; one libtest process per seed shard"]
fn baseline_mask_completeness() {
    let start = std::env::var("MTG_BASELINE_START")
        .unwrap_or_else(|_| "0".into())
        .parse::<u64>()
        .unwrap();
    let count = std::env::var("MTG_BASELINE_COUNT")
        .unwrap_or_else(|_| "1000".into())
        .parse::<u64>()
        .unwrap();
    for seed in start..start + count {
        let decisions = baseline_game(seed).unwrap();
        println!("baseline seed={seed} decisions={decisions}");
    }
}

#[test]
fn bounded_rollout_matches_across_worker_counts() {
    let make = |threads| {
        Gym::new(
            Source::load(PROFILE, &[], None).unwrap(),
            Config::default(),
            23,
            4,
            threads,
        )
        .unwrap()
    };
    let mut left = make(1);
    let mut right = make(4);
    left.advance(None).unwrap();
    right.advance(None).unwrap();
    for tick in 0..64 {
        assert!(left.storage == right.storage);
        let moves: Vec<_> = (0..4)
            .map(|i| {
                let mask = if left.storage.head[i] == 0 {
                    &left.storage.ptr_mask[i * 131..(i + 1) * 131]
                } else {
                    &left.storage.num_mask[i * 64..(i + 1) * 64]
                };
                let active: Vec<_> = mask
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v != 0)
                    .map(|(a, _)| a as i32)
                    .collect();
                active[(tick + 7 * i) % active.len()]
            })
            .collect();
        left.advance(Some(&moves)).unwrap();
        right.advance(Some(&moves)).unwrap();
    }
    assert!(left.storage == right.storage);
    for i in 0..4 {
        assert_eq!(
            left.lanes[0][i].game.state_digest().unwrap(),
            right.lanes[i][0].game.state_digest().unwrap()
        );
    }
}

// SplitMix64 supplies an independent deterministic policy stream. Rejection
// before reduction keeps every live category equiprobable.
fn uniform_action(mask: &[u8], state: &mut u64) -> Result<usize, String> {
    let count = mask.iter().filter(|&&enabled| enabled == 1).count() as u64;
    if count == 0 {
        return Err("decision has an empty mask".into());
    }
    let index = loop {
        *state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = *state;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^= x >> 31;
        if x >= count.wrapping_neg() % count {
            break (x % count) as usize;
        }
    };
    Ok(mask
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == 1)
        .nth(index)
        .unwrap()
        .0)
}
#[test]
#[ignore = "release full-game qualification, separately sharded by seed"]
fn random_mask_soundness() {
    let start = std::env::var("MTG_RANDOM_START")
        .unwrap_or_else(|_| "0".into())
        .parse::<u64>()
        .unwrap();
    let count = std::env::var("MTG_RANDOM_COUNT")
        .unwrap_or_else(|_| "10000".into())
        .parse::<u64>()
        .unwrap();
    let source = Source::load(PROFILE, &[], None).unwrap();
    for seed in start..start + count {
        let mut lane = Lane::new(&source, seed, 0, 0).unwrap();
        let mut rng = seed ^ 0x9c46bb824fecab71;
        let mut action = None;
        let mut steps = 0;
        let mut episode = 0;
        loop {
            let row = lane
                .run(&source, Config::default(), action)
                .unwrap_or_else(|e| panic!("seed {seed} step {steps}: {e}"));
            assert_eq!(row.info[0], 0);
            if row.done != 0 {
                if row.info[2] != game::CensorClass::None as i32 {
                    assert_eq!(
                        row.info[2],
                        game::CensorClass::TensorCapacity as i32,
                        "seed {seed} episode {episode}: unexpected censor"
                    );
                    assert_eq!(row.reward, [0; 4]);
                    println!(
                        "censored seed={seed} episode={episode} decisions={} microsteps={steps} class={} illegal=0",
                        row.info[3], row.info[2]
                    );
                    episode += 1;
                    steps = 0;
                } else {
                    println!(
                        "random seed={seed} episode={episode} decisions={} microsteps={steps} illegal=0",
                        row.info[3]
                    );
                    break;
                }
            }
            let mask = if row.head == 0 {
                row.ptr_mask.as_slice()
            } else {
                row.num_mask.as_slice()
            };
            action = Some(uniform_action(mask, &mut rng).unwrap());
            steps += 1;
        }
    }
}

#[test]
fn tensor_capacity_resets_with_zero_reward_and_no_rejected_answer() {
    let source = Source::load(PROFILE, &[], None).unwrap();
    let mut lane = Lane::new(&source, 57, 0, 0).unwrap();
    let initial = lane.run(&source, Config::default(), None).unwrap();
    assert_eq!(initial.done, 0);
    let view = &mut lane.observation.as_mut().unwrap().view;
    view.objects
        .resize(mtg_observe::MAX_ENTITIES + 1, view.objects[0].clone());
    let reset = lane.run(&source, Config::default(), None).unwrap();
    assert_eq!(
        (reset.info[2], reset.done, reset.reward, reset.info[0]),
        (3, 1, [0; 4], 0)
    );
    let mut fresh = Lane::new(&source, 57, 0, 1).unwrap();
    let next = fresh.run(&source, Config::default(), None).unwrap();
    assert_eq!(reset.globals, next.globals);
    assert_eq!(reset.ptr_mask, next.ptr_mask);
    assert_eq!(
        lane.game.state_digest().unwrap(),
        fresh.game.state_digest().unwrap()
    );
    let a = reset.ptr_mask.iter().position(|x| *x == 1).unwrap();
    assert_eq!(
        lane.run(&source, Config::default(), Some(a)).unwrap().info[0],
        0
    );
}

#[test]
fn unmodeled_mana_sources_leave_casts_unmasked() {
    let source = Source::load(PROFILE, &[], None).unwrap();
    let mut lane = Lane::new(&source, 57, 0, 0).unwrap();
    lane.run(&source, Config::default(), None).unwrap();
    let o = lane.observation.as_mut().unwrap();
    let commander = o
        .view
        .objects
        .iter()
        .find(|obj| matches!(obj.zone, Zone::Command { .. }) && obj.owner == o.view.viewer)
        .unwrap()
        .clone();
    let decision = o.decision.as_mut().unwrap();
    decision.candidates = vec![Candidate::Action {
        action: Action::Cast {
            card: commander.handle.clone(),
        },
    }];
    decision.shape = Shape::Pick {
        slots: vec![Slot {
            role: SlotRole::Choice,
            subject: None,
            allowed: vec![0],
            min: 1,
            max: 1,
            repeat: false,
        }],
        rules: vec![],
    };
    let Step::Choose(mask) = Composer::compose(decision, &[], 0).unwrap() else {
        panic!("missing cast pointer")
    };
    let encoded = mtg_observe::encode(source.catalog(), o, 0, &mask, true).unwrap();
    assert_eq!((encoded.cand[0][5], encoded.ptr_mask[0]), (0, 0));
    let mut token = commander;
    token.handle = ObjectHandle("new-token".into());
    token.zone = Zone::Battlefield;
    token.controller = o.view.viewer;
    token.oracle_id.clear();
    token.token = Some(VisibleToken {
        name: String::new(),
        keywords: vec![],
    });
    o.view.objects.push(token);
    let encoded = mtg_observe::encode(source.catalog(), o, 0, &mask, true).unwrap();
    assert_eq!((encoded.cand[0][5], encoded.ptr_mask[0]), (2, 1));
}
