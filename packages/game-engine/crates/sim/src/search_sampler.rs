//! Strategy-side hypotheses use only a sealed permitted capture. No actual Game,
//! match seed, physical identity, or private RNG is available to this module.
use mtg_contracts::{canonical, sha256};
use mtg_core::{
    HypotheticalCard, PlayerStatus, SEARCH_SAMPLER, SearchCapture, SeatHypothesis, WorldProposal,
};
use std::collections::BTreeMap;

pub(super) const ASSIGNMENT_RNG: &str = "sha256-counter-le32-rejection-fisher-yates-v1";
pub(super) const MAX_DISTINCT_ATTEMPTS: u64 = 256;

fn seed_bytes(parts: &impl serde::Serialize) -> Result<[u8; 32], String> {
    let hex = sha256(canonical(parts)?.as_bytes());
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
struct Words {
    seed: [u8; 32],
    block: u64,
    words: [u32; 8],
    cursor: usize,
}
impl Words {
    fn new(seed: [u8; 32]) -> Self {
        Self {
            seed,
            block: 0,
            words: [0; 8],
            cursor: 8,
        }
    }
    fn next(&mut self, check: &mut dyn FnMut() -> Result<(), String>) -> Result<u32, String> {
        check()?;
        if self.cursor == 8 {
            let bytes = seed_bytes(&(ASSIGNMENT_RNG, self.seed, self.block))?;
            self.block = self
                .block
                .checked_add(1)
                .ok_or("sampler random counter exhausted")?;
            for (index, word) in self.words.iter_mut().enumerate() {
                *word = u32::from_le_bytes(
                    bytes[index * 4..index * 4 + 4]
                        .try_into()
                        .map_err(|_| "random block width")?,
                );
            }
            self.cursor = 0;
        }
        let word = self.words[self.cursor];
        self.cursor += 1;
        Ok(word)
    }
    fn below(
        &mut self,
        bound: usize,
        check: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<usize, String> {
        let bound = u64::try_from(bound).map_err(|_| "sampler bound overflow")?;
        if bound == 0 || bound > 400 {
            return Err("sampler bound outside finite card pool".into());
        }
        let range = 1u64 << 32;
        let limit = range - range % bound;
        loop {
            let word = u64::from(self.next(check)?);
            if word < limit {
                return usize::try_from(word % bound).map_err(|_| "sampler index overflow".into());
            }
        }
    }
}
fn remove(counts: &mut BTreeMap<String, u32>, oracle: &str) -> Result<(), String> {
    let count = counts
        .get_mut(oracle)
        .ok_or("observed/remembered card absent from open deck")?;
    *count = count
        .checked_sub(1)
        .ok_or("observed/remembered cards exceed open deck multiplicity")?;
    Ok(())
}

/// Uniform remaining physical-copy permutations, conditioned on permitted facts;
/// output hand order and interchangeable copy labels are subsequently quotiented.
/// This sampling assumption is not a certified posterior or opponent policy.
pub(super) fn propose(
    capture: &SearchCapture,
    public_job_seed: &[u8; 32],
    world_index: u64,
    attempt: u64,
    check: &mut dyn FnMut() -> Result<(), String>,
) -> Result<WorldProposal, String> {
    let data = capture.data();
    let view = &data.context.observation.view;
    let mut words = Words::new(seed_bytes(&(
        SEARCH_SAMPLER,
        ASSIGNMENT_RNG,
        capture.sampler_identity(),
        public_job_seed,
        world_index,
        attempt,
    ))?);
    let mut seats = std::array::from_fn(|_| SeatHypothesis {
        hidden_hand: vec![],
        ordered_library: vec![],
    });
    for (seat, hypothesis) in seats.iter_mut().enumerate() {
        check()?;
        let player = view
            .players
            .iter()
            .find(|p| usize::from(p.player.0) == seat)
            .ok_or("capture omits a seat")?;
        if player.status != PlayerStatus::Live {
            continue;
        }
        let mut remaining = BTreeMap::<String, u32>::new();
        for entry in &data.profile.decks[seat].cards {
            let count = remaining.entry(entry.oracle_id.clone()).or_default();
            *count = count
                .checked_add(entry.count)
                .ok_or("deck count overflow")?;
        }
        for object in view.objects.iter().filter(|o| o.owner == player.player) {
            check()?;
            remove(&mut remaining, &object.oracle_id)?;
        }
        let mut hand = vec![];
        if player.player != view.viewer {
            for known in data
                .known_hands
                .iter()
                .filter(|k| k.player == player.player)
            {
                check()?;
                remove(&mut remaining, &known.oracle_id)?;
                hand.push(known.oracle_id.clone());
            }
        }
        let known_bottom = if player.player == view.viewer {
            data.known_bottom
                .iter()
                .map(|k| k.oracle_id.clone())
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        for oracle in &known_bottom {
            check()?;
            remove(&mut remaining, oracle)?;
        }
        let mut unknown = vec![];
        for (oracle, count) in remaining {
            for _ in 0..count {
                check()?;
                unknown.push(oracle.clone());
            }
        }
        if unknown.len() > 100 {
            return Err("sample exceeds per-seat physical card bound".into());
        }
        for index in (1..unknown.len()).rev() {
            let other = words.below(index + 1, check)?;
            unknown.swap(index, other);
        }
        let hand_slots = if player.player == view.viewer {
            0
        } else {
            usize::try_from(player.hand_count)
                .map_err(|_| "hand count overflow")?
                .checked_sub(hand.len())
                .ok_or("known hand exceeds public count")?
        };
        if hand_slots > unknown.len() {
            return Err("not enough unknown cards for public hand count".into());
        }
        hand.extend(unknown.drain(..hand_slots));
        hand.sort();
        unknown.extend(known_bottom);
        if unknown.len()
            != usize::try_from(player.library_count).map_err(|_| "library count overflow")?
        {
            return Err("unknown cards do not account for the whole library".into());
        }
        hypothesis.hidden_hand = hand
            .into_iter()
            .enumerate()
            .map(|(index, oracle_id)| HypotheticalCard {
                symbol: format!("w{world_index}-a{attempt}-p{seat}-h{index}"),
                oracle_id,
            })
            .collect();
        hypothesis.ordered_library = unknown
            .into_iter()
            .enumerate()
            .map(|(index, oracle_id)| HypotheticalCard {
                symbol: format!("w{world_index}-a{attempt}-p{seat}-l{index}"),
                oracle_id,
            })
            .collect();
    }
    Ok(WorldProposal {
        proposal_version: 1,
        context_id: capture.context_id().into(),
        sampler_version: SEARCH_SAMPLER.into(),
        world_index,
        seats,
        // Independent of assignment retries, candidate IDs, worker scheduling,
        // actual MatchConfig.seed and actual match RNG position.
        future_seed: seed_bytes(&(
            "foundations-future-rng-v1",
            capture.sampler_identity(),
            public_job_seed,
            world_index,
        ))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampler_word_stream_is_partition_independent_and_domain_separated() {
        let seed = seed_bytes(&("job", 7u64)).unwrap();
        let mut a = Words::new(seed);
        let mut b = Words::new(seed);
        let expected = (0..64)
            .map(|_| a.next(&mut || Ok(())).unwrap())
            .collect::<Vec<_>>();
        let mut actual = vec![];
        for width in [1, 7, 13, 43] {
            for _ in 0..width {
                actual.push(b.next(&mut || Ok(())).unwrap());
            }
        }
        assert_eq!(actual, expected);
        assert_ne!(seed, seed_bytes(&("future", 7u64)).unwrap());
        assert!(b.next(&mut || Err("cancelled".into())).is_err());
    }

    fn natural_root(index: u16) -> (mtg_core::Game, SearchCapture) {
        use mtg_core::*;
        let hex = sha256(format!("p7-independent-capture/{index}").as_bytes());
        let seed = std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap());
        let catalog = mtg_compiler::match_fixture_catalog().unwrap();
        let prepared = Prepared::admit_pod(
            catalog,
            MatchConfig::for_profile("commander-foundations-v1", seed, PlayerId(0)).unwrap(),
        )
        .unwrap();
        let mut game = Game::with_trace(prepared, TraceMode::Off);
        for _ in 0..100_000 {
            match game.advance(Budget::new(1024)) {
                Advance::Suspended { .. } => continue,
                Advance::AwaitingDecision(request) => {
                    let player = request.player();
                    let QueryResult::Complete(QueryValue::Observation(observation)) =
                        game.observe(player, Budget::new(100_000))
                    else {
                        panic!("observe");
                    };
                    if player == PlayerId(0)
                        && observation.view.active == Some(player)
                        && observation.view.turn.step == TurnStep::PrecombatMain
                    {
                        return (
                            game.fork(),
                            super::super::search_roots::capture(&game, player).unwrap(),
                        );
                    }
                    let response = match observation.decision.unwrap() {
                        PlayerRequest::Mulligan { .. } => PlayerResponse::Mulligan { take: false },
                        PlayerRequest::Priority { .. } => PlayerResponse::Pass,
                        other => panic!("unexpected preparation choice {other:?}"),
                    };
                    let QueryResult::Complete(QueryValue::Validated(receipt)) = game
                        .check_player_response(
                            player,
                            request_token(&game, player),
                            response,
                            Budget::new(100_000),
                        )
                    else {
                        panic!("response");
                    };
                    game.submit(receipt).unwrap();
                }
                other => panic!("preparation failed {other:?}"),
            }
        }
        panic!("preparation call bound")
    }
    fn request_token(game: &mtg_core::Game, player: mtg_core::PlayerId) -> mtg_core::DecisionToken {
        let mtg_core::QueryResult::Complete(mtg_core::QueryValue::Decision(Some(request))) =
            game.player_decision_for(player, mtg_core::Budget::new(100_000))
        else {
            panic!("decision");
        };
        request.token().clone()
    }

    #[test]
    fn production_sampler_uses_only_indistinguishable_permitted_capture() {
        use mtg_core::*;
        // Independently discovered fixed pair, separate from benchmark seeds.
        let (real_a, a) = natural_root(1);
        let (real_b, b) = natural_root(2);
        assert_eq!(a.data(), b.data());
        assert_eq!(a.context_id(), b.context_id());
        assert_ne!(
            real_a.state_digest().unwrap(),
            real_b.state_digest().unwrap()
        );
        let pa = propose(&a, &[9; 32], 0, 0, &mut || Ok(())).unwrap();
        let pb = propose(&b, &[9; 32], 0, 0, &mut || Ok(())).unwrap();
        assert_eq!(canonical(&pa).unwrap(), canonical(&pb).unwrap());
        let digest = pa.assignment_digest().unwrap();
        let mut relabeled = pa.clone();
        relabeled.future_seed = [17; 32];
        relabeled.world_index = 99;
        for seat in &mut relabeled.seats {
            seat.hidden_hand.reverse();
            for card in seat
                .hidden_hand
                .iter_mut()
                .chain(seat.ordered_library.iter_mut())
            {
                card.symbol.push_str("-new-label");
            }
        }
        assert_eq!(digest, relabeled.assignment_digest().unwrap());
        let catalog = mtg_compiler::match_fixture_catalog().unwrap();
        let build = |capture: &SearchCapture, proposal| {
            let mut outcome =
                SampledGame::build(catalog.clone(), capture, proposal, Budget::new(1024));
            for _ in 0..100_000 {
                match outcome {
                    SampleBuildOutcome::Complete { game, root_map } => return (game, root_map),
                    SampleBuildOutcome::Incomplete { work, .. } => {
                        outcome = work.resume(Budget::new(1024))
                    }
                    SampleBuildOutcome::Rejected(reason) => panic!("sample rejected {reason:?}"),
                }
            }
            panic!("build bound")
        };
        let (sa, ma) = build(&a, pa);
        let (sb, mb) = build(&b, pb);
        assert_eq!(sa.state_digest().unwrap(), sb.state_digest().unwrap());
        for (sample, map, capture) in [(&sa, &ma, &a), (&sb, &mb, &b)] {
            let QueryResult::Complete(QueryValue::Observation(view)) =
                sample.observe(PlayerId(0), Budget::new(100_000))
            else {
                panic!("sample observation");
            };
            assert_eq!(
                map.mapped_observation(&view).unwrap(),
                capture.data().context.observation
            );
        }
        assert!(propose(&a, &[9; 32], 0, 0, &mut || Err("cancelled".into())).is_err());
    }
}
