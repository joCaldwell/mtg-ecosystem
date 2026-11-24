//! Generate answers from the permitted shape and feed them through the real
//! viewer receipt and rules executor. Candidate descriptions never enter play.
use mtg_contracts::{Color, Quantity};
use mtg_core::*;
use std::collections::BTreeMap;

fn next(seed: &mut u64, count: usize) -> usize {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*seed >> 32) as usize) % count
}

fn shuffle<T>(items: &mut [T], seed: &mut u64) {
    for end in (1..items.len()).rev() {
        items.swap(end, next(seed, end + 1));
    }
}

fn sample(shape: &Shape, seed: &mut u64) -> Answer {
    match shape {
        Shape::Pick { slots, rules } => loop {
            let picks: Vec<Vec<u32>> = slots
                .iter()
                .map(|slot| {
                    let count = slot.min as usize + next(seed, (slot.max - slot.min + 1) as usize);
                    let mut options = slot.allowed.clone();
                    shuffle(&mut options, seed);
                    if slot.repeat {
                        (0..count)
                            .map(|_| options[next(seed, options.len())])
                            .collect()
                    } else {
                        options.into_iter().take(count).collect()
                    }
                })
                .collect();
            let legal = rules.iter().all(|rule| match rule {
                JointRule::Distinct { slots } => {
                    let mut seen = std::collections::BTreeSet::new();
                    slots
                        .iter()
                        .all(|s| picks[*s as usize].iter().all(|i| seen.insert(i)))
                }
                JointRule::Total { min, max } => {
                    (*min as usize..=*max as usize).contains(&picks.iter().map(Vec::len).sum())
                }
                JointRule::PickedAtLeast { candidates, count } => candidates.iter().all(|c| {
                    let n = picks.iter().flatten().filter(|i| *i == c).count();
                    n == 0 || n >= *count as usize
                }),
            });
            if legal {
                break Answer::Pick { picks };
            }
        },
        Shape::Order { items, min, max } => {
            let mut order = items.clone();
            shuffle(&mut order, seed);
            order.truncate(*min as usize + next(seed, (max - min + 1) as usize));
            Answer::Order { order }
        }
        Shape::Number { min, max } => Answer::Number {
            value: max
                .as_ref()
                .filter(|_| next(seed, 2) == 0)
                .unwrap_or(min)
                .clone(),
        },
        Shape::Distribute {
            total,
            buckets,
            gates,
        } => {
            let mut amounts: Vec<_> = buckets.iter().map(|b| b.min.clone()).collect();
            let mut remaining = total.clone();
            for n in &amounts {
                remaining.subtract(n);
            }
            let mut order: Vec<_> = (0..buckets.len()).collect();
            shuffle(&mut order, seed);
            for i in order {
                if gates.iter().any(|g| {
                    g.bucket as usize == i
                        && g.requires
                            .iter()
                            .any(|(j, min)| amounts[*j as usize] < *min)
                }) {
                    continue;
                }
                let mut room = buckets[i].max.clone();
                room.subtract(&amounts[i]);
                let delta = room.min(remaining.clone());
                amounts[i].add(&delta);
                remaining.subtract(&delta);
            }
            assert_eq!(remaining, Quantity::zero());
            Answer::Distribute { amounts }
        }
        Shape::Pay { cost, pool, step } => Answer::Pay {
            pay: match step {
                PayStep::Produce { sources } => match sources.get(next(seed, sources.len() + 1)) {
                    Some(source) => PayAnswer::Activate { source: *source },
                    None => PayAnswer::Finish {
                        undo_on_failure: next(seed, 2) == 0,
                    },
                },
                PayStep::Spend => {
                    let mut unrestricted = BTreeMap::<Color, Quantity>::new();
                    let mut available = pool.unrestricted.clone();
                    for color in &cost.colored {
                        available.get_mut(color).unwrap().subtract(&1.into());
                        unrestricted.entry(*color).or_default().add(&1.into());
                    }
                    let mut colors: Vec<_> = available.into_iter().collect();
                    shuffle(&mut colors, seed);
                    let mut generic = cost.generic.clone();
                    for (color, n) in colors {
                        let spend = n.min(generic.clone());
                        if spend > Quantity::zero() {
                            generic.subtract(&spend);
                            unrestricted.entry(color).or_default().add(&spend);
                        }
                    }
                    assert_eq!(generic, Quantity::zero());
                    PayAnswer::Spend { unrestricted }
                }
            },
        },
    }
}

#[test]
fn random_shape_answers_are_accepted_and_consumed() {
    let name = "commander-foundations-v1";
    let profile = match_profile(name).unwrap();
    for seed in [3, 7] {
        let config = MatchConfig::for_profile(name, [seed; 32], PlayerId(0)).unwrap();
        let mut game = Game::new(Prepared::admit_pod(profile.catalog().clone(), config).unwrap());
        let mut random = u64::from(seed);
        for _ in 0..48 {
            let player = match game.advance(Budget::unlimited()) {
                Advance::AwaitingDecision(d) => d.player,
                other => panic!("unexpected rules boundary {other:?}"),
            };
            let QueryResult::Complete(QueryValue::Observation(o)) =
                game.observe(player, Budget::unlimited())
            else {
                panic!("missing permitted observation")
            };
            let decision = o.decision.unwrap();
            let answer = sample(&decision.shape, &mut random);
            let QueryResult::Complete(QueryValue::Validated(receipt)) = game.check_player_response(
                player,
                decision.id,
                answer.clone(),
                Budget::unlimited(),
            ) else {
                panic!("rejected answer {answer:?} for {:?}", decision.shape)
            };
            game.submit(receipt).unwrap();
        }
        assert!(matches!(
            game.advance(Budget::unlimited()),
            Advance::AwaitingDecision(_)
        ));
    }
}
