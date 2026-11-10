//! Bounded OLD OWNED Task::Search oracle, separate from the new Task type and
//! StoredView conversion/writer. The large host proof is a separate follow-up.
use super::*;

#[derive(Clone, Serialize)]
#[serde(tag = "task", rename_all = "snake_case")]
enum OwnedTask {
    Initialize,
    Search {
        builder: ObservationWork,
        observation: Option<Box<Observation>>,
        history: Vec<PermittedView>,
        decks: Vec<OpenDecklist>,
    },
}
#[derive(Serialize)]
struct OwnedContext {
    context_version: u32,
    observation: Observation,
    permitted_history: Vec<PermittedView>,
    open_decklists: Vec<OpenDecklist>,
    catalog_digest: String,
    profile_id: String,
    sampling_seed: String,
}
impl OwnedTask {
    fn unit(&mut self, game: &Game, viewer: PlayerId, seed: &str) -> Option<OwnedContext> {
        if matches!(self, Self::Initialize) {
            *self = Self::Search {
                builder: ObservationWork {
                    cursor: 0,
                    view: game.blank_view(viewer).unwrap(),
                    decision: game.request_header(viewer).unwrap(),
                },
                observation: None,
                history: vec![],
                decks: vec![],
            };
            return None;
        }
        let Self::Search {
            builder,
            observation,
            history,
            decks,
        } = self
        else {
            unreachable!()
        };
        if observation.is_none() {
            *observation = game
                .reference_observation_unit(builder)
                .unwrap()
                .map(Box::new);
            return None;
        }
        let original = &game.state.knowledge.visibility[viewer.0 as usize].history;
        if history.len() < original.len() {
            // Generic old public-view serde is independent of stored to_public,
            // its canonical writer, and PermittedHistory's serializer.
            history.push(
                serde_json::from_value(serde_json::to_value(&original[history.len()]).unwrap())
                    .unwrap(),
            );
            return None;
        }
        if decks.len() < 4 {
            let player = PlayerId(decks.len() as u8);
            let mut definitions = BTreeMap::new();
            for card in game
                .prepared
                .config
                .cards
                .iter()
                .filter(|c| c.owner == player)
            {
                let count = definitions.entry(card.oracle_id.clone()).or_insert(0_u32);
                *count = count.checked_add(1).unwrap();
            }
            decks.push(OpenDecklist {
                player,
                definitions,
            });
            return None;
        }
        Some(OwnedContext {
            context_version: 16,
            observation: *observation.clone().unwrap(),
            permitted_history: history.clone(),
            open_decklists: decks.clone(),
            catalog_digest: game.prepared.catalog_digest.clone(),
            profile_id: game.prepared.config.profile.clone(),
            sampling_seed: seed.into(),
        })
    }
}
fn owned_from_empty(work: &Work) -> OwnedTask {
    assert!(matches!(work.data.task, Task::Initialize));
    assert_eq!(work.data.units_done, 0);
    OwnedTask::Initialize
}

#[test]
fn shared_search_prefix_matches_old_owned_every_unit_and_all_budget_partitions() {
    let game = match_root();
    let before = game.state.clone();
    let trace = game.trace.clone();
    let state_sha = game.state_digest().unwrap();
    let viewer = PlayerId(0);
    let seed = "old-owned-reference:雪";
    let total = game
        .query_work_units(&Query::Search {
            viewer,
            sampling_seed: seed.into(),
        })
        .unwrap();
    assert_eq!(
        total,
        game.observation_work_units(viewer).unwrap()
            + game.state.knowledge.visibility[0].history.len() as u64
            + 6 // explicit initialization unit plus four decks and completion
    );
    for quantum in [1, 64, u64::MAX] {
        let QueryResult::Incomplete {
            mut work,
            required_minimum,
        } = game.search_context(viewer, seed.into(), Budget::new(0))
        else {
            panic!("lazy search")
        };
        assert_eq!(required_minimum, 1);
        assert_eq!(work.data.units_done, 0);
        assert!(work.cache.last_object.is_none());
        assert!(work.cache.effects.is_none());
        let mut owned = owned_from_empty(&work);
        let mut units = 0_u64;
        let mut cold_prefixes = 0;
        loop {
            let mut expected = None;
            for _ in 0..quantum.min(total - units) {
                units += 1;
                expected = owned.unit(&game, viewer, seed);
                if expected.is_some() {
                    break;
                }
            }
            match (game.resume_work(work, Budget::new(quantum)), expected) {
                (
                    QueryResult::Incomplete {
                        work: next,
                        required_minimum,
                    },
                    None,
                ) => {
                    assert_eq!(required_minimum, 1);
                    assert_eq!(next.data.units_done, units);
                    assert_eq!(
                        canonical(&next.data.task).unwrap(),
                        canonical(&owned).unwrap()
                    );
                    // Complete old SavedWork envelope/scratch, not selected fields.
                    #[derive(Serialize)]
                    struct OldData<'a> {
                        query: &'a Query,
                        task: &'a OwnedTask,
                        units_done: u64,
                    }
                    #[derive(Serialize)]
                    struct OldSaved<'a> {
                        work_version: u32,
                        build_id: &'a str,
                        catalog_digest: &'a str,
                        state_sha256: &'a str,
                        data: OldData<'a>,
                    }
                    let history_len = match &next.data.task {
                        Task::Search { history, .. } => history.len(),
                        Task::Initialize => 0,
                        _ => unreachable!(),
                    };
                    work = next;
                    if quantum == 1 && [1, 2].contains(&history_len) {
                        let saved = game.save_work(&work).unwrap();
                        assert_eq!(
                            saved,
                            canonical(&OldSaved {
                                work_version: 16,
                                build_id: BUILD_ID,
                                catalog_digest: &game.prepared.catalog_digest,
                                state_sha256: &state_sha,
                                data: OldData {
                                    query: &work.data.query,
                                    task: &owned,
                                    units_done: units
                                },
                            })
                            .unwrap()
                        );
                        let cold = game.restore_work(&saved).unwrap();
                        assert_eq!(
                            canonical(&cold.data.task).unwrap(),
                            canonical(&owned).unwrap()
                        );
                        work = cold;
                        cold_prefixes += 1;
                    }
                }
                (QueryResult::Complete(QueryValue::SearchContext(actual)), Some(expected)) => {
                    assert_eq!(units, total);
                    // Both public DTOs exhaustively remain the complete old wire.
                    assert_eq!(canonical(&actual).unwrap(), canonical(&expected).unwrap());
                    assert_eq!(
                        actual.permitted_history.iter().collect::<Vec<_>>(),
                        expected.permitted_history
                    );
                    if quantum == 1 {
                        assert!(cold_prefixes >= 2);
                    }
                    break;
                }
                _ => panic!("completion or rejection changed at unit {units}"),
            }
        }
    }
    assert_eq!(game.state, before);
    assert_eq!(game.trace, trace);
    assert_eq!(game.state_digest().unwrap(), state_sha);
}
#[test]
fn shared_search_saved_prefix_recomputes_all_history_and_preserves_executor_authority() {
    let mut game = match_root();
    let viewer = PlayerId(0);
    let prefix_len = game.state.knowledge.visibility[0].history.len();
    assert!(prefix_len >= 2);
    let units = game.observation_work_units(viewer).unwrap() + prefix_len as u64 + 1;
    let QueryResult::Incomplete { work, .. } =
        game.search_context(viewer, "prefix-seed".into(), Budget::new(units))
    else {
        panic!("history prefix")
    };
    let Task::Search { history, .. } = &work.data.task else {
        panic!("search")
    };
    assert_eq!(history.len(), prefix_len);
    let saved = game.save_work(&work).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let rows = wire["data"]["task"]["history"].as_array().unwrap();
    let distinct = (1..rows.len())
        .find(|i| rows[0] != rows[*i])
        .expect("natural root retains distinct setup histories");
    for variant in 0..9 {
        let mut bad = wire.clone();
        match variant {
            0 => {
                bad["data"]["task"]["history"][0]["viewer"] = 3.into();
            }
            1 => {
                bad["data"]["task"]["history"][0]["turn"]["number"] = 123456.into();
            }
            2 => {
                bad["data"]["task"]["history"]
                    .as_array_mut()
                    .unwrap()
                    .remove(0);
            }
            3 => {
                bad["data"]["task"]["history"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, distinct);
            }
            4 => {
                bad["data"]["units_done"] = (units + 1).into();
            }
            5 => {
                bad["data"]["task"]["builder"]["cursor"] = 0.into();
            }
            6 => {
                bad["data"]["query"]["sampling_seed"] = "different".into();
            }
            7 => {
                bad["build_id"] = "foreign".into();
            }
            8 => {
                bad["data"]["task"]["history"][0]["extra"] = true.into();
            }
            _ => unreachable!(),
        }
        if variant == 6 {
            // Seed is caller-supplied query data, not authority: changing it
            // coherently before any dependent scratch is legal. It must remain
            // exact on completion, never be filled from actual hidden RNG.
            let recovered = game.restore_work(&bad.to_string()).unwrap();
            let QueryResult::Complete(QueryValue::SearchContext(c)) =
                game.resume_work(recovered, Budget::unlimited())
            else {
                panic!("seed-only recovery")
            };
            assert_eq!(c.sampling_seed, "different");
        } else {
            assert!(
                game.restore_work(&bad.to_string()).is_err(),
                "forgery {variant}"
            );
        }
    }
    let fork = game.fork();
    assert!(matches!(
        fork.resume_work(work, Budget::unlimited()),
        QueryResult::Rejected(QueryRejection::StaleOrForeignWork)
    ));
    // Explicit saved-work recovery rebinds to the validated same State; direct
    // work transfer cannot do so. Replay every original unit before accepting.
    let rebound = fork.restore_work(&saved).unwrap();
    assert!(matches!(
        fork.resume_work(rebound, Budget::unlimited()),
        QueryResult::Complete(QueryValue::SearchContext(_))
    ));
    game.concede(PlayerId(3)).unwrap();
    assert!(game.restore_work(&saved).is_err());
}
